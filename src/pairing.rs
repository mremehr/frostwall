//! Intelligent wallpaper pairing based on usage history
//!
//! Tracks which wallpapers are set together on multi-monitor setups
//! and suggests/auto-applies matching wallpapers based on learned patterns.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A record of wallpapers set together at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRecord {
    /// Wallpaper paths for each screen (screen_name -> wallpaper_path)
    pub wallpapers: HashMap<String, PathBuf>,
    /// When this pairing was applied (Unix timestamp)
    pub timestamp: u64,
    /// How long this pairing was kept (seconds), if known
    #[serde(default)]
    pub duration: Option<u64>,
    /// Was it manually selected or auto-applied?
    pub manual: bool,
}

/// Affinity score between two wallpapers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityScore {
    pub wallpaper_a: PathBuf,
    pub wallpaper_b: PathBuf,
    /// Combined affinity score (higher = better match)
    pub score: f32,
    /// How many times they've been paired together
    pub pair_count: u32,
    /// Average duration when paired (seconds)
    pub avg_duration_secs: f32,
}

/// Persistent pairing history and affinity cache
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PairingHistoryData {
    pub records: Vec<PairingRecord>,
    pub affinity_scores: Vec<AffinityScore>,
}

/// Runtime state for undo functionality
pub struct UndoState {
    pub previous_wallpapers: HashMap<String, PathBuf>,
    pub started_at: Instant,
    pub duration: Duration,
    pub message: String,
}

/// Manages pairing history and suggestions
pub struct PairingHistory {
    data: PairingHistoryData,
    cache_path: PathBuf,
    /// Current active pairing (for duration tracking)
    current_pairing_start: Option<u64>,
    /// Undo state
    undo_state: Option<UndoState>,
    /// Maximum records to keep
    max_records: usize,
}

impl PairingHistory {
    /// Create new pairing history manager
    pub fn new(max_records: usize) -> Self {
        let cache_path = directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.cache_dir().join("pairing_history.json"))
            .unwrap_or_else(|| PathBuf::from("/tmp/frostwall/pairing_history.json"));

        Self {
            data: PairingHistoryData::default(),
            cache_path,
            current_pairing_start: None,
            undo_state: None,
            max_records,
        }
    }

    /// Load history from cache file
    pub fn load(max_records: usize) -> Result<Self> {
        let mut history = Self::new(max_records);

        if history.cache_path.exists() {
            let content = std::fs::read_to_string(&history.cache_path)
                .context("Failed to read pairing history")?;
            history.data = serde_json::from_str(&content)
                .context("Failed to parse pairing history")?;
        }

        Ok(history)
    }

    /// Save history to cache file
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.data)?;
        std::fs::write(&self.cache_path, content)?;

        Ok(())
    }

    /// Record a new pairing
    pub fn record_pairing(&mut self, wallpapers: HashMap<String, PathBuf>, manual: bool) {
        // End previous pairing (for duration tracking)
        self.end_current_pairing();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let record = PairingRecord {
            wallpapers: wallpapers.clone(),
            timestamp,
            duration: None,
            manual,
        };

        self.data.records.push(record);
        self.current_pairing_start = Some(timestamp);

        // Update affinity scores for all pairs in this pairing
        let paths: Vec<_> = wallpapers.values().cloned().collect();
        for i in 0..paths.len() {
            for j in (i + 1)..paths.len() {
                self.update_affinity(&paths[i], &paths[j], None);
            }
        }

        // Prune old records if needed
        self.prune_old_records();

        // Auto-save
        let _ = self.save();
    }

    /// Mark end of current pairing (for duration tracking)
    fn end_current_pairing(&mut self) {
        if let Some(start) = self.current_pairing_start.take() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let duration = now.saturating_sub(start);

            // Update the last record with duration
            if let Some(last) = self.data.records.last_mut() {
                last.duration = Some(duration);

                // Update affinity scores with duration info
                let paths: Vec<_> = last.wallpapers.values().cloned().collect();
                for i in 0..paths.len() {
                    for j in (i + 1)..paths.len() {
                        self.update_affinity(&paths[i], &paths[j], Some(duration));
                    }
                }
            }
        }
    }

    /// Update affinity score between two wallpapers
    fn update_affinity(&mut self, wp_a: &Path, wp_b: &Path, duration: Option<u64>) {
        let (a, b) = Self::ordered_pair(wp_a, wp_b);

        // Find or create affinity entry
        let entry = self.data.affinity_scores.iter_mut().find(|s| {
            s.wallpaper_a == a && s.wallpaper_b == b
        });

        if let Some(score) = entry {
            score.pair_count += 1;
            if let Some(dur) = duration {
                // Update rolling average duration
                let total_duration = score.avg_duration_secs * (score.pair_count - 1) as f32;
                score.avg_duration_secs = (total_duration + dur as f32) / score.pair_count as f32;
            }
            // Recalculate score
            score.score = Self::calculate_base_score(score.pair_count, score.avg_duration_secs);
        } else {
            let new_score = AffinityScore {
                wallpaper_a: a.to_path_buf(),
                wallpaper_b: b.to_path_buf(),
                score: Self::calculate_base_score(1, duration.unwrap_or(0) as f32),
                pair_count: 1,
                avg_duration_secs: duration.unwrap_or(0) as f32,
            };
            self.data.affinity_scores.push(new_score);
        }
    }

    /// Calculate base affinity score from pairing stats
    fn calculate_base_score(pair_count: u32, avg_duration_secs: f32) -> f32 {
        // More pairings = higher score (with diminishing returns)
        let count_score = (pair_count as f32).ln().max(0.0) * 10.0;

        // Longer durations = higher score (capped)
        let duration_score = (avg_duration_secs / 300.0).min(5.0);

        count_score + duration_score
    }

    /// Get ordered pair of paths (for consistent key)
    fn ordered_pair<'a>(a: &'a Path, b: &'a Path) -> (&'a Path, &'a Path) {
        if a < b { (a, b) } else { (b, a) }
    }

    /// Get the best matching wallpaper for other screens
    /// Returns the wallpaper with highest affinity score, or falls back to
    /// a wallpaper with similar colors if no history exists.
    pub fn get_best_match(
        &self,
        selected_wp: &Path,
        _target_screen: &str,
        available_wallpapers: &[&crate::wallpaper::Wallpaper],
        selected_colors: &[String],
    ) -> Option<PathBuf> {
        if available_wallpapers.is_empty() {
            return None;
        }

        let mut best_match: Option<(PathBuf, f32)> = None;

        for wp in available_wallpapers {
            // Skip the same wallpaper
            if wp.path == selected_wp {
                continue;
            }

            // Base score from pairing history
            let mut score = self.get_affinity(selected_wp, &wp.path);

            // Bonus for shared colors (fallback when no history)
            let shared_colors = wp.colors.iter()
                .filter(|c| selected_colors.contains(c))
                .count();
            score += shared_colors as f32 * 0.5;

            // Bonus for shared tags
            // (would need selected wallpaper's tags passed in)

            if let Some((_, best_score)) = &best_match {
                if score > *best_score {
                    best_match = Some((wp.path.clone(), score));
                }
            } else if score > 0.0 {
                best_match = Some((wp.path.clone(), score));
            }
        }

        best_match.map(|(path, _)| path)
    }

    /// Get affinity score between two wallpapers
    pub fn get_affinity(&self, wp_a: &Path, wp_b: &Path) -> f32 {
        let (a, b) = Self::ordered_pair(wp_a, wp_b);

        self.data.affinity_scores
            .iter()
            .find(|s| s.wallpaper_a == a && s.wallpaper_b == b)
            .map(|s| s.score)
            .unwrap_or(0.0)
    }

    /// Prune old records to stay under max limit
    fn prune_old_records(&mut self) {
        if self.data.records.len() > self.max_records {
            let to_remove = self.data.records.len() - self.max_records;
            self.data.records.drain(0..to_remove);
        }
    }

    /// Begin undo window
    pub fn begin_undo(&mut self, previous: HashMap<String, PathBuf>, message: String, duration_secs: u64) {
        self.undo_state = Some(UndoState {
            previous_wallpapers: previous,
            started_at: Instant::now(),
            duration: Duration::from_secs(duration_secs),
            message,
        });
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        if let Some(state) = &self.undo_state {
            state.started_at.elapsed() < state.duration
        } else {
            false
        }
    }

    /// Get undo state for display
    pub fn undo_state(&self) -> Option<&UndoState> {
        self.undo_state.as_ref().filter(|s| s.started_at.elapsed() < s.duration)
    }

    /// Execute undo, returns the wallpapers to restore
    pub fn do_undo(&mut self) -> Option<HashMap<String, PathBuf>> {
        if self.can_undo() {
            self.undo_state.take().map(|s| s.previous_wallpapers)
        } else {
            None
        }
    }

    /// Clear undo state (called when timeout expires)
    pub fn clear_expired_undo(&mut self) {
        if let Some(state) = &self.undo_state {
            if state.started_at.elapsed() >= state.duration {
                self.undo_state = None;
            }
        }
    }

    /// Get remaining undo time in seconds
    pub fn undo_remaining_secs(&self) -> Option<u64> {
        self.undo_state().map(|s| {
            s.duration.saturating_sub(s.started_at.elapsed()).as_secs()
        })
    }

    /// Get undo message
    pub fn undo_message(&self) -> Option<&str> {
        self.undo_state().map(|s| s.message.as_str())
    }

    /// Get number of records
    pub fn record_count(&self) -> usize {
        self.data.records.len()
    }

    /// Get number of affinity pairs
    pub fn affinity_count(&self) -> usize {
        self.data.affinity_scores.len()
    }
}
