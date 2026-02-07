//! Intelligent wallpaper pairing based on usage history
//!
//! Tracks which wallpapers are set together on multi-monitor setups
//! and suggests/auto-applies matching wallpapers based on learned patterns.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PairingStyleMode {
    Off,
    #[default]
    Soft,
    Strict,
}

impl PairingStyleMode {
    pub fn next(self) -> Self {
        match self {
            PairingStyleMode::Off => PairingStyleMode::Soft,
            PairingStyleMode::Soft => PairingStyleMode::Strict,
            PairingStyleMode::Strict => PairingStyleMode::Off,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            PairingStyleMode::Off => "Off",
            PairingStyleMode::Soft => "Soft",
            PairingStyleMode::Strict => "Strict",
        }
    }
}

pub struct MatchContext<'a> {
    pub selected_wp: &'a Path,
    pub target_screen: &'a str,
    pub selected_colors: &'a [String],
    pub selected_weights: &'a [f32],
    pub selected_tags: &'a [String],
    pub selected_embedding: Option<&'a [f32]>,
    pub screen_context_weight: f32,
    pub visual_weight: f32,
    pub harmony_weight: f32,
    pub tag_weight: f32,
    pub semantic_weight: f32,
    pub repetition_penalty_weight: f32,
    pub style_mode: PairingStyleMode,
    pub selected_style_tags: &'a [String],
}

const STYLE_TAGS: &[&str] = &[
    "abstract",
    "anime",
    "anime_character",
    "concept_art",
    "digital_art",
    "fantasy",
    "fantasy_landscape",
    "geometric",
    "illustration",
    "line_art",
    "moody_fantasy",
    "painting",
    "painterly",
    "pixel_art",
    "retro",
    "vintage",
];

fn canonical_style_tag(tag: &str) -> Option<&'static str> {
    let normalized = tag
        .trim()
        .to_lowercase()
        .replace(['-', ' '], "_")
        .trim_matches('_')
        .to_string();
    match normalized.as_str() {
        "8bit" | "8_bit" | "pixelart" | "pixel_art" => Some("pixel_art"),
        "anime_character" | "animecharacter" => Some("anime_character"),
        "concept_art" | "conceptart" => Some("concept_art"),
        "digital_painting" | "digital_art" | "digitalpainting" | "digitalart" => {
            Some("digital_art")
        }
        "line_art" | "lineart" => Some("line_art"),
        "fantasy_landscape" | "fantasylandscape" => Some("fantasy_landscape"),
        "moody_fantasy" | "moodyfantasy" => Some("moody_fantasy"),
        "painted" | "painting" | "painterly" => Some("painting"),
        "illustrated" | "illustration" => Some("illustration"),
        other => STYLE_TAGS.iter().copied().find(|style| *style == other),
    }
}

pub fn extract_style_tags(tags: &[String]) -> Vec<String> {
    let mut styles: Vec<String> = tags
        .iter()
        .filter_map(|tag| canonical_style_tag(tag))
        .map(str::to_string)
        .collect();
    styles.sort();
    styles.dedup();
    styles
}

fn collect_style_tags<'a>(tags: impl Iterator<Item = &'a str>) -> HashSet<&'static str> {
    tags.filter_map(canonical_style_tag).collect()
}

fn is_specific_style_tag(tag: &str) -> bool {
    !matches!(tag, "abstract" | "anime" | "fantasy")
}

fn is_content_tag(tag: &str) -> bool {
    if canonical_style_tag(tag).is_some() {
        return false;
    }
    !matches!(
        tag,
        "bright"
            | "dark"
            | "pastel"
            | "vibrant"
            | "minimal"
            | "landscape_orientation"
            | "portrait"
    )
}

fn compare_scored_match(a: &(PathBuf, f32), b: &(PathBuf, f32)) -> std::cmp::Ordering {
    match b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal) {
        std::cmp::Ordering::Equal => a.0.cmp(&b.0),
        order => order,
    }
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
            history.data =
                serde_json::from_str(&content).context("Failed to parse pairing history")?;
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
        let entry = self
            .data
            .affinity_scores
            .iter_mut()
            .find(|s| s.wallpaper_a == a && s.wallpaper_b == b);

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
        if a < b {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// Get the best matching wallpaper for other screens
    /// Returns the wallpaper with highest affinity score, or falls back to
    /// a wallpaper with similar colors if no history exists.
    pub fn get_best_match(
        &self,
        context: &MatchContext<'_>,
        available_wallpapers: &[&crate::wallpaper::Wallpaper],
    ) -> Option<PathBuf> {
        self.get_top_matches(context, available_wallpapers, 1)
            .into_iter()
            .next()
            .map(|(path, _)| path)
    }

    /// Get top N matching wallpapers for other screens
    /// Returns wallpapers sorted by affinity score (highest first)
    ///
    /// Scoring formula:
    /// - Base: pairing history affinity
    /// - Color similarity: weighted palette match (0-5 points)
    /// - Color harmony: complementary/analogous/triadic bonus (0-3 points)
    /// - Tag matching: shared tags bonus (0-6 points, max 3 tags)
    pub fn get_top_matches(
        &self,
        context: &MatchContext<'_>,
        available_wallpapers: &[&crate::wallpaper::Wallpaper],
        limit: usize,
    ) -> Vec<(PathBuf, f32)> {
        if limit == 0 || available_wallpapers.is_empty() {
            return Vec::new();
        }

        const STRICT_VISUAL_MIN: f32 = 0.62;
        const STRICT_SEMANTIC_MIN: f32 = 0.58;
        const STRICT_COMBINED_QUALITY_MIN: f32 = 0.63;

        let selected_weights: Cow<'_, [f32]> = if context.selected_weights.is_empty() {
            Cow::Owned(vec![
                1.0 / context.selected_colors.len().max(1) as f32;
                context.selected_colors.len()
            ])
        } else {
            Cow::Borrowed(context.selected_weights)
        };
        let selected_tags: HashSet<&str> =
            context.selected_tags.iter().map(String::as_str).collect();
        let selected_style_tags: HashSet<&str> = context
            .selected_style_tags
            .iter()
            .map(String::as_str)
            .collect();
        let selected_specific_style_tags: HashSet<&str> = selected_style_tags
            .iter()
            .copied()
            .filter(|tag| is_specific_style_tag(tag))
            .collect();
        let selected_content_tags: HashSet<&str> = selected_tags
            .iter()
            .copied()
            .filter(|tag| is_content_tag(tag))
            .collect();

        // Strict mode should prioritize "what the image depicts" and visual coherence
        // over historical co-occurrence.
        let (
            screen_context_weight,
            visual_weight,
            harmony_weight,
            tag_weight,
            semantic_weight,
            repetition_penalty_weight,
        ) = match context.style_mode {
            PairingStyleMode::Strict => (
                context.screen_context_weight * 0.55,
                context.visual_weight * 1.20,
                context.harmony_weight * 1.10,
                context.tag_weight * 1.55,
                context.semantic_weight * 1.80,
                context.repetition_penalty_weight * 1.15,
            ),
            PairingStyleMode::Soft => (
                context.screen_context_weight * 0.90,
                context.visual_weight * 1.05,
                context.harmony_weight,
                context.tag_weight * 1.15,
                context.semantic_weight * 1.20,
                context.repetition_penalty_weight,
            ),
            PairingStyleMode::Off => (
                context.screen_context_weight,
                context.visual_weight,
                context.harmony_weight,
                context.tag_weight,
                context.semantic_weight,
                context.repetition_penalty_weight,
            ),
        };

        // Build one lookup table instead of scanning affinity_scores for each candidate.
        let affinity_lookup: HashMap<&Path, f32> = self
            .data
            .affinity_scores
            .iter()
            .filter_map(|score| {
                if score.wallpaper_a == context.selected_wp {
                    Some((score.wallpaper_b.as_path(), score.score))
                } else if score.wallpaper_b == context.selected_wp {
                    Some((score.wallpaper_a.as_path(), score.score))
                } else {
                    None
                }
            })
            .collect();
        let screen_context_lookup =
            self.screen_context_scores(context.selected_wp, context.target_screen);

        let mut scored: Vec<(PathBuf, f32)> = available_wallpapers
            .iter()
            .filter(|wp| wp.path != context.selected_wp)
            .filter_map(|wp| {
                // Base score from pairing history
                let mut score = affinity_lookup
                    .get(wp.path.as_path())
                    .copied()
                    .unwrap_or(0.0);
                // Screen-aware co-occurrence for this specific target output.
                score += screen_context_lookup
                    .get(wp.path.as_path())
                    .copied()
                    .unwrap_or(0.0)
                    * screen_context_weight;

                // Tag matching bonus (0-6 points, 2 points per shared tag, max 3)
                let mut unique_tags = HashSet::new();
                let candidate_tags: Vec<&str> = wp
                    .tags
                    .iter()
                    .map(String::as_str)
                    .chain(wp.auto_tags.iter().map(|tag| tag.name.as_str()))
                    .filter(|tag| unique_tags.insert(*tag))
                    .collect();
                let shared_tags = candidate_tags
                    .iter()
                    .filter(|tag| selected_tags.contains(**tag))
                    .count();
                let content_overlap = if selected_content_tags.is_empty() {
                    0
                } else {
                    candidate_tags
                        .iter()
                        .filter(|tag| selected_content_tags.contains(**tag))
                        .count()
                };

                let (style_overlap, specific_style_overlap) =
                    if context.style_mode == PairingStyleMode::Off || selected_style_tags.is_empty()
                    {
                        (0, 0)
                    } else {
                        let candidate_style_tags = collect_style_tags(candidate_tags.iter().copied());
                        let style_overlap = candidate_style_tags
                            .iter()
                            .filter(|tag| selected_style_tags.contains(**tag))
                            .count();
                        let specific_style_overlap = candidate_style_tags
                            .iter()
                            .filter(|tag| selected_specific_style_tags.contains(**tag))
                            .count();
                        (style_overlap, specific_style_overlap)
                    };

                // Semantic similarity from CLIP embeddings (0-1 normalized)
                let semantic_similarity = if let (Some(selected_embedding), Some(candidate_embedding)) =
                    (context.selected_embedding, wp.embedding.as_deref())
                {
                    Some(normalize_cosine_similarity(selected_embedding, candidate_embedding))
                } else {
                    None
                };

                // Strict mode can reject weak candidates early before running color/harmony scoring.
                if context.style_mode == PairingStyleMode::Strict {
                    if !selected_style_tags.is_empty() {
                        let overlap = if selected_specific_style_tags.is_empty() {
                            style_overlap
                        } else {
                            specific_style_overlap
                        };
                        let basis = if selected_specific_style_tags.is_empty() {
                            selected_style_tags.len()
                        } else {
                            selected_specific_style_tags.len()
                        };

                        if overlap == 0 {
                            return None;
                        }
                        if basis >= 2 && (overlap as f32 / basis as f32) < 0.5 {
                            return None;
                        }
                    }

                    if !selected_content_tags.is_empty() {
                        if content_overlap == 0 {
                            return None;
                        }
                        if selected_content_tags.len() >= 3
                            && (content_overlap as f32 / selected_content_tags.len() as f32)
                                < 0.34
                        {
                            return None;
                        }
                    }

                    if let Some(similarity) = semantic_similarity {
                        if similarity < STRICT_SEMANTIC_MIN {
                            return None;
                        }
                    }
                }

                // Get candidate weights (or default to equal)
                let wp_weights: Cow<'_, [f32]> = if wp.color_weights.is_empty() {
                    Cow::Owned(vec![1.0 / wp.colors.len().max(1) as f32; wp.colors.len()])
                } else {
                    Cow::Borrowed(wp.color_weights.as_slice())
                };

                // Visual similarity with weighted palette, brightness and saturation (0-5 points)
                let visual_similarity = crate::utils::image_similarity_weighted(
                    context.selected_colors,
                    selected_weights.as_ref(),
                    &wp.colors,
                    wp_weights.as_ref(),
                );
                score += visual_similarity * visual_weight;

                // Color harmony bonus (0-3 points)
                let (harmony, strength) = crate::utils::detect_harmony(
                    context.selected_colors,
                    selected_weights.as_ref(),
                    &wp.colors,
                    wp_weights.as_ref(),
                );
                let harmony_bonus = harmony.bonus() * strength * harmony_weight;
                score += harmony_bonus;
                let tag_bonus = (shared_tags as f32).min(3.0) * tag_weight;
                score += tag_bonus;

                match context.style_mode {
                    PairingStyleMode::Off => {}
                    PairingStyleMode::Soft => {
                        if !selected_style_tags.is_empty() {
                            if style_overlap > 0 {
                                score += (style_overlap as f32).min(2.0) * (tag_weight * 1.25);
                            } else {
                                score -= tag_weight * 0.75;
                            }
                        }
                        if !selected_content_tags.is_empty() {
                            if content_overlap > 0 {
                                score += (content_overlap as f32).min(3.0) * (tag_weight * 0.90);
                            } else {
                                score -= tag_weight * 0.55;
                            }
                        }
                    }
                    PairingStyleMode::Strict => {
                        if !selected_style_tags.is_empty() {
                            let overlap = if selected_specific_style_tags.is_empty() {
                                style_overlap
                            } else {
                                specific_style_overlap
                            };

                            score += (overlap as f32).min(2.0) * (tag_weight * 1.5);
                        }

                        if !selected_content_tags.is_empty() {
                            score += (content_overlap as f32).min(3.0) * (tag_weight * 1.25);
                        } else {
                            // No explicit style tags on the selected image:
                            // strict mode still enforces close visual/semantic similarity.
                            if visual_similarity < STRICT_VISUAL_MIN {
                                return None;
                            }
                        }

                        let strict_quality = if let Some(similarity) = semantic_similarity {
                            (similarity * 0.58) + (visual_similarity * 0.42)
                        } else {
                            visual_similarity
                        };
                        if strict_quality < STRICT_COMBINED_QUALITY_MIN {
                            return None;
                        }
                    }
                }

                if let Some(similarity) = semantic_similarity {
                    score += similarity * semantic_weight;
                }

                score -= self.recent_repetition_penalty(
                    context.target_screen,
                    &wp.path,
                    repetition_penalty_weight,
                );

                Some((wp.path.clone(), score))
            })
            .collect();

        // Keep top-N efficiently and return deterministic ordering.
        if scored.len() > limit {
            let pivot = limit - 1;
            scored.select_nth_unstable_by(pivot, compare_scored_match);
            scored.truncate(limit);
        }
        scored.sort_unstable_by(compare_scored_match);
        scored
    }

    /// Build a screen-specific affinity map for selected wallpaper -> candidate on target screen.
    fn screen_context_scores(
        &self,
        selected_wp: &Path,
        target_screen: &str,
    ) -> HashMap<&Path, f32> {
        // Recent pairings matter most; old pairings still contribute but decay smoothly.
        const HALF_LIFE_SECS: f32 = 7.0 * 24.0 * 3600.0;
        const LOOKBACK_RECORDS: usize = 600;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut raw: HashMap<&Path, f32> = HashMap::new();
        for record in self.data.records.iter().rev().take(LOOKBACK_RECORDS) {
            let Some(target_path) = record.wallpapers.get(target_screen) else {
                continue;
            };
            if target_path.as_path() == selected_wp {
                continue;
            }
            if !record
                .wallpapers
                .values()
                .any(|path| path.as_path() == selected_wp)
            {
                continue;
            }

            let age_secs = now.saturating_sub(record.timestamp) as f32;
            let recency = 1.0 / (1.0 + age_secs / HALF_LIFE_SECS);
            let duration_factor = (record.duration.unwrap_or(90) as f32 / 900.0).clamp(0.35, 1.6);
            let manual_factor = if record.manual { 1.1 } else { 1.0 };
            let contribution = recency * duration_factor * manual_factor;
            *raw.entry(target_path.as_path()).or_insert(0.0) += contribution;
        }

        // Normalize to 0..1 for stable weighting in final score.
        let max_score = raw.values().copied().fold(0.0, f32::max);
        if max_score > 0.0 {
            raw.values_mut().for_each(|score| *score /= max_score);
        }
        raw
    }

    /// Penalize exact repetition on same target output to encourage variety.
    fn recent_repetition_penalty(&self, target_screen: &str, candidate: &Path, weight: f32) -> f32 {
        if weight <= 0.0 {
            return 0.0;
        }

        const LOOKBACK_RECORDS: usize = 12;

        let raw_penalty = self
            .data
            .records
            .iter()
            .rev()
            .take(LOOKBACK_RECORDS)
            .enumerate()
            .filter_map(|(idx, record)| {
                record
                    .wallpapers
                    .get(target_screen)
                    .filter(|path| path.as_path() == candidate)
                    .map(|_| 1.0 / (idx as f32 + 1.0))
            })
            .sum::<f32>();

        (raw_penalty * 0.35 * weight).min(1.5 * weight)
    }

    /// Get affinity score between two wallpapers
    pub fn get_affinity(&self, wp_a: &Path, wp_b: &Path) -> f32 {
        let (a, b) = Self::ordered_pair(wp_a, wp_b);

        self.data
            .affinity_scores
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
    #[allow(dead_code)]
    pub fn begin_undo(
        &mut self,
        previous: HashMap<String, PathBuf>,
        message: String,
        duration_secs: u64,
    ) {
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
        self.undo_state
            .as_ref()
            .filter(|s| s.started_at.elapsed() < s.duration)
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
        self.undo_state()
            .map(|s| s.duration.saturating_sub(s.started_at.elapsed()).as_secs())
    }

    /// Get undo message
    pub fn undo_message(&self) -> Option<&str> {
        self.undo_state().map(|s| s.message.as_str())
    }

    /// Get number of records
    pub fn record_count(&self) -> usize {
        self.data.records.len()
    }

    /// Get the most recent pairing with multiple screens
    pub fn get_last_multi_screen_pairing(&self) -> Option<HashMap<String, PathBuf>> {
        self.data
            .records
            .iter()
            .rev()
            .find(|r| r.wallpapers.len() > 1)
            .map(|r| r.wallpapers.clone())
    }

    /// Get number of affinity pairs
    pub fn affinity_count(&self) -> usize {
        self.data.affinity_scores.len()
    }
}

fn normalize_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..len {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }

    let cosine = dot / (norm_a.sqrt() * norm_b.sqrt());
    ((cosine + 1.0) / 2.0).clamp(0.0, 1.0)
}
