use crate::pairing::PairingHistory;
use crate::screen::{self, Screen};
use crate::swww::{self, FillColor, ResizeMode, Transition, TransitionType};
use crate::thumbnail::ThumbnailCache;
use crate::ui;
use crate::utils::ColorHarmony;
use crate::wallpaper::{MatchMode, SortMode, Wallpaper, WallpaperCache};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct Config {
    #[serde(default)]
    pub wallpaper: WallpaperConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub transition: TransitionConfig,
    #[serde(default)]
    pub thumbnails: ThumbnailConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
    #[serde(default)]
    pub clip: ClipConfig,
    #[serde(default)]
    pub pairing: PairingConfig,
    #[serde(default)]
    pub time_profiles: crate::timeprofile::TimeProfiles,
    #[serde(default)]
    pub terminal: TerminalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperConfig {
    pub directory: PathBuf,
    pub extensions: Vec<String>,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default)]
    pub match_mode: MatchMode,
    #[serde(default)]
    pub resize_mode: ResizeMode,
    #[serde(default)]
    pub fill_color: FillColor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionConfig {
    pub transition_type: String,
    pub duration: f32,
    pub fps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailConfig {
    pub width: u32,
    pub height: u32,
    pub quality: u8,
    pub grid_columns: usize,
    #[serde(default = "default_preload_count")]
    pub preload_count: usize,
}

fn default_preload_count() -> usize {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub mode: String, // "auto", "light", "dark"
    pub check_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Recommended repaint_delay for kitty.conf (ms)
    #[serde(default = "default_repaint_delay")]
    pub recommended_repaint_delay: u32,
    /// Recommended input_delay for kitty.conf (ms)
    #[serde(default = "default_input_delay")]
    pub recommended_input_delay: u32,
    /// Whether the optimization hint has been shown
    #[serde(default)]
    pub hint_shown: bool,
}

fn default_repaint_delay() -> u32 {
    5
}

fn default_input_delay() -> u32 {
    1
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            recommended_repaint_delay: 5,
            recommended_input_delay: 1,
            hint_shown: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    pub next: String,
    pub prev: String,
    pub apply: String,
    pub quit: String,
    pub random: String,
    pub toggle_match: String,
    pub toggle_resize: String,
    pub next_screen: String,
    pub prev_screen: String,
}

/// Configuration for CLIP auto-tagging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipConfig {
    /// Enable CLIP auto-tagging during scans
    pub enabled: bool,
    /// Confidence threshold for tags (0.0-1.0)
    pub threshold: f32,
    /// Include auto-tags in tag filter UI
    pub show_in_filter: bool,
    /// Cache embeddings for similarity search
    pub cache_embeddings: bool,
}

/// Configuration for intelligent wallpaper pairing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingConfig {
    /// Enable intelligent pairing suggestions
    pub enabled: bool,
    /// Auto-apply suggestions to other screens
    pub auto_apply: bool,
    /// Show undo option duration (seconds)
    pub undo_window_secs: u64,
    /// Minimum confidence to auto-apply (0.0-1.0)
    pub auto_apply_threshold: f32,
    /// Maximum history records to keep
    pub max_history_records: usize,
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            directory: dirs::picture_dir()
                .map(|p| p.join("wallpapers"))
                .unwrap_or_else(|| PathBuf::from("~/Pictures/wallpapers")),
            extensions: vec![
                "jpg".into(), "jpeg".into(), "png".into(),
                "webp".into(), "bmp".into(), "gif".into(),
            ],
            recursive: false,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            match_mode: MatchMode::Flexible,
            resize_mode: ResizeMode::Fit,
            fill_color: FillColor::black(),
        }
    }
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            transition_type: "fade".to_string(),
            duration: 1.0,
            fps: 60,
        }
    }
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            quality: 92,
            grid_columns: 3,
            preload_count: 3,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: "auto".to_string(),
            check_interval_ms: 500,
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            next: "l".to_string(),
            prev: "h".to_string(),
            apply: "Enter".to_string(),
            quit: "q".to_string(),
            random: "r".to_string(),
            toggle_match: "m".to_string(),
            toggle_resize: "f".to_string(),
            next_screen: "Tab".to_string(),
            prev_screen: "BackTab".to_string(),
        }
    }
}

impl Default for ClipConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in by default
            threshold: 0.25,
            show_in_filter: true,
            cache_embeddings: true,
        }
    }
}

impl Default for PairingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_apply: false, // Conservative default
            undo_window_secs: 5,
            auto_apply_threshold: 0.7,
            max_history_records: 1000,
        }
    }
}

impl KeybindingsConfig {
    /// Parse a keybinding string into a KeyCode
    pub fn parse_key(s: &str) -> Option<KeyCode> {
        let s = s.trim();

        // Single character
        if s.len() == 1 {
            return Some(KeyCode::Char(s.chars().next().unwrap()));
        }

        // Named keys (case insensitive)
        match s.to_lowercase().as_str() {
            "enter" | "return" => Some(KeyCode::Enter),
            "esc" | "escape" => Some(KeyCode::Esc),
            "tab" => Some(KeyCode::Tab),
            "backtab" | "shift+tab" | "s-tab" => Some(KeyCode::BackTab),
            "space" => Some(KeyCode::Char(' ')),
            "backspace" => Some(KeyCode::Backspace),
            "delete" | "del" => Some(KeyCode::Delete),
            "insert" | "ins" => Some(KeyCode::Insert),
            "home" => Some(KeyCode::Home),
            "end" => Some(KeyCode::End),
            "pageup" | "pgup" => Some(KeyCode::PageUp),
            "pagedown" | "pgdn" => Some(KeyCode::PageDown),
            "up" | "arrow_up" => Some(KeyCode::Up),
            "down" | "arrow_down" => Some(KeyCode::Down),
            "left" | "arrow_left" => Some(KeyCode::Left),
            "right" | "arrow_right" => Some(KeyCode::Right),
            "f1" => Some(KeyCode::F(1)),
            "f2" => Some(KeyCode::F(2)),
            "f3" => Some(KeyCode::F(3)),
            "f4" => Some(KeyCode::F(4)),
            "f5" => Some(KeyCode::F(5)),
            "f6" => Some(KeyCode::F(6)),
            "f7" => Some(KeyCode::F(7)),
            "f8" => Some(KeyCode::F(8)),
            "f9" => Some(KeyCode::F(9)),
            "f10" => Some(KeyCode::F(10)),
            "f11" => Some(KeyCode::F(11)),
            "f12" => Some(KeyCode::F(12)),
            _ => None,
        }
    }

    /// Check if a KeyCode matches a keybinding
    pub fn matches(&self, key: KeyCode, binding: &str) -> bool {
        Self::parse_key(binding) == Some(key)
    }
}


impl Config {
    pub fn config_path() -> PathBuf {
        directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
            .join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let data = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&data)?;
            Ok(config)
        } else {
            // Create default config
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = toml::to_string_pretty(self)?;
        fs::write(&path, data)?;

        Ok(())
    }

    /// Check if running in Kitty terminal
    pub fn is_kitty_terminal() -> bool {
        std::env::var("TERM").map(|t| t.contains("kitty")).unwrap_or(false)
            || std::env::var("KITTY_WINDOW_ID").is_ok()
    }

    /// Show terminal optimization hint if not shown before
    /// Returns the hint message if it should be shown
    pub fn check_terminal_hint(&mut self) -> Option<String> {
        if self.terminal.hint_shown || !Self::is_kitty_terminal() {
            return None;
        }

        self.terminal.hint_shown = true;
        let _ = self.save(); // Save that hint was shown

        Some(format!(
            "Tip: För optimal prestanda i Kitty, lägg till i ~/.config/kitty/kitty.conf:\n\n\
             repaint_delay {}\n\
             input_delay {}\n\
             sync_to_monitor yes\n\n\
             Tryck valfri tangent för att fortsätta...",
            self.terminal.recommended_repaint_delay,
            self.terminal.recommended_input_delay
        ))
    }

    pub fn transition(&self) -> Transition {
        let transition_type = match self.transition.transition_type.as_str() {
            "fade" => TransitionType::Fade,
            "wipe" => TransitionType::Wipe,
            "grow" => TransitionType::Grow,
            "center" => TransitionType::Center,
            "outer" => TransitionType::Outer,
            "none" => TransitionType::None,
            _ => TransitionType::Fade,
        };

        Transition {
            transition_type,
            duration: self.transition.duration,
            fps: self.transition.fps,
        }
    }

    /// Get wallpaper directory, expanding ~ if needed
    pub fn wallpaper_dir(&self) -> PathBuf {
        let dir = &self.wallpaper.directory;
        if dir.starts_with("~") {
            if let Some(home) = dirs::home_dir() {
                return home.join(dir.strip_prefix("~").unwrap_or(dir));
            }
        }
        dir.clone()
    }

    /// Check if an extension is supported
    #[allow(dead_code)]
    pub fn is_supported_extension(&self, ext: &str) -> bool {
        self.wallpaper.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext))
    }
}

/// Request to load a thumbnail in background
pub struct ThumbnailRequest {
    pub cache_idx: usize,
    pub source_path: PathBuf,
}

/// Response from thumbnail loading
pub struct ThumbnailResponse {
    pub cache_idx: usize,
    pub image: image::DynamicImage,
}

/// Events from background threads
pub enum AppEvent {
    Key(event::KeyEvent),
    ThumbnailReady(ThumbnailResponse),
    Tick,
}

/// Maximum number of thumbnails to keep in memory
/// Kitty graphics protocol can get confused with too many images
const MAX_THUMBNAIL_CACHE: usize = 20;

pub struct App {
    pub screens: Vec<Screen>,
    pub cache: WallpaperCache,
    pub config: Config,
    pub selected_screen_idx: usize,
    pub selected_wallpaper_idx: usize,
    pub filtered_wallpapers: Vec<usize>,
    pub should_quit: bool,
    pub image_picker: Option<Picker>,
    pub thumbnail_cache: HashMap<usize, Box<dyn StatefulProtocol>>,
    /// Order of cache entries for LRU eviction
    thumbnail_cache_order: Vec<usize>,
    /// Tracks which thumbnails are currently being loaded
    pub loading_thumbnails: std::collections::HashSet<usize>,
    /// Channel to request thumbnail loading
    thumb_request_tx: Option<Sender<ThumbnailRequest>>,
    /// Show help popup
    pub show_help: bool,
    /// Current sort mode
    pub sort_mode: SortMode,
    /// Active tag filter (None = show all)
    pub active_tag_filter: Option<String>,
    /// Show color palette of selected wallpaper
    pub show_colors: bool,
    /// Show color picker popup
    pub show_color_picker: bool,
    /// Available colors for filtering (extracted from all wallpapers)
    pub available_colors: Vec<String>,
    /// Selected color index in picker
    pub color_picker_idx: usize,
    /// Active color filter
    pub active_color_filter: Option<String>,
    /// Export pywal colors on apply
    pub pywal_export: bool,
    /// Last error message (for UI display)
    pub last_error: Option<String>,
    /// Pairing history for intelligent suggestions
    pub pairing_history: PairingHistory,
    /// Suggested wallpapers based on pairing history (for TUI highlighting)
    pub pairing_suggestions: Vec<PathBuf>,
    /// Current wallpaper per screen (for tracking pairings)
    pub current_wallpapers: HashMap<String, PathBuf>,
    /// Remember selected wallpaper index per screen
    screen_positions: HashMap<usize, usize>,
    /// Command mode (vim-style :)
    pub command_mode: bool,
    /// Command input buffer
    pub command_buffer: String,
    /// Show pairing preview popup
    pub show_pairing_preview: bool,
    /// Pairing preview suggestions per screen (screen_name -> [(path, score, harmony)])
    pub pairing_preview_matches: HashMap<String, Vec<(PathBuf, f32, ColorHarmony)>>,
    /// Selected index in pairing preview (which alternative)
    pub pairing_preview_idx: usize,
}

impl App {
    pub fn new(wallpaper_dir: PathBuf) -> Result<Self> {
        let config = Config::load()?;
        let cache = WallpaperCache::load_or_scan_recursive(&wallpaper_dir, config.wallpaper.recursive)?;

        // Try to create image picker for thumbnail rendering
        // from_termios() queries terminal for font size
        // guess_protocol() then detects the best graphics protocol (Kitty, Sixel, etc.)
        let image_picker = Picker::from_termios()
            .ok()
            .map(|mut p| {
                // Actively query terminal for graphics protocol support
                p.guess_protocol();
                p
            })
            .or_else(|| Some(Picker::new((8, 16))));

        // Load pairing history
        let pairing_history = PairingHistory::load(config.pairing.max_history_records)
            .unwrap_or_else(|_| PairingHistory::new(config.pairing.max_history_records));

        Ok(Self {
            screens: Vec::new(),
            cache,
            config,
            selected_screen_idx: 0,
            selected_wallpaper_idx: 0,
            filtered_wallpapers: Vec::new(),
            should_quit: false,
            image_picker,
            thumbnail_cache: HashMap::new(),
            thumbnail_cache_order: Vec::new(),
            loading_thumbnails: std::collections::HashSet::new(),
            thumb_request_tx: None,
            show_help: false,
            sort_mode: SortMode::Name,
            active_tag_filter: None,
            show_colors: false,
            show_color_picker: false,
            available_colors: Vec::new(),
            color_picker_idx: 0,
            active_color_filter: None,
            pywal_export: false,
            last_error: None,
            pairing_history,
            pairing_suggestions: Vec::new(),
            current_wallpapers: HashMap::new(),
            screen_positions: HashMap::new(),
            command_mode: false,
            command_buffer: String::new(),
            show_pairing_preview: false,
            pairing_preview_matches: HashMap::new(),
            pairing_preview_idx: 0,
        })
    }

    pub async fn init_screens(&mut self) -> Result<()> {
        self.screens = screen::detect_screens().await?;
        self.update_filtered_wallpapers();
        Ok(())
    }

    pub fn update_filtered_wallpapers(&mut self) {
        let match_mode = self.config.display.match_mode;
        let tag_filter = self.active_tag_filter.clone();
        let color_filter = self.active_color_filter.clone();

        if let Some(screen) = self.screens.get(self.selected_screen_idx) {
            self.filtered_wallpapers = self
                .cache
                .wallpapers
                .iter()
                .enumerate()
                .filter(|(_, wp)| {
                    // Screen matching
                    if !wp.matches_screen_with_mode(screen, match_mode) {
                        return false;
                    }
                    // Tag filtering
                    if let Some(ref tag) = tag_filter {
                        if !wp.has_tag(tag) {
                            return false;
                        }
                    }
                    // Color filtering with perceptual matching
                    if let Some(ref color) = color_filter {
                        // Include if any color is perceptually similar (>0.7 similarity)
                        let has_similar = wp.colors.iter()
                            .any(|c| crate::utils::color_similarity(c, color) > 0.7);
                        if !has_similar {
                            return false;
                        }
                    }
                    true
                })
                .map(|(i, _)| i)
                .collect();
        } else {
            self.filtered_wallpapers = (0..self.cache.wallpapers.len()).collect();
        }

        // Apply current sort
        self.apply_sort();

        if self.selected_wallpaper_idx >= self.filtered_wallpapers.len() {
            self.selected_wallpaper_idx = 0;
        }

        // Clear thumbnail cache when filter changes
        self.thumbnail_cache.clear();
        self.thumbnail_cache_order.clear();
        self.loading_thumbnails.clear();
    }

    /// Toggle match mode and refresh filter
    pub fn toggle_match_mode(&mut self) {
        self.config.display.match_mode = self.config.display.match_mode.next();
        self.update_filtered_wallpapers();
    }

    /// Toggle resize mode
    pub fn toggle_resize_mode(&mut self) {
        self.config.display.resize_mode = self.config.display.resize_mode.next();
    }

    pub fn selected_wallpaper(&self) -> Option<&Wallpaper> {
        self.filtered_wallpapers
            .get(self.selected_wallpaper_idx)
            .and_then(|&i| self.cache.wallpapers.get(i))
    }

    pub fn selected_screen(&self) -> Option<&Screen> {
        self.screens.get(self.selected_screen_idx)
    }

    pub fn next_wallpaper(&mut self) {
        if !self.filtered_wallpapers.is_empty() {
            self.selected_wallpaper_idx =
                (self.selected_wallpaper_idx + 1) % self.filtered_wallpapers.len();
            self.update_pairing_suggestions();
        }
    }

    pub fn prev_wallpaper(&mut self) {
        if !self.filtered_wallpapers.is_empty() {
            self.selected_wallpaper_idx = if self.selected_wallpaper_idx == 0 {
                self.filtered_wallpapers.len() - 1
            } else {
                self.selected_wallpaper_idx - 1
            };
            self.update_pairing_suggestions();
        }
    }

    pub fn next_screen(&mut self) {
        if !self.screens.is_empty() {
            // Save current position
            self.screen_positions.insert(self.selected_screen_idx, self.selected_wallpaper_idx);

            self.selected_screen_idx = (self.selected_screen_idx + 1) % self.screens.len();
            self.update_filtered_wallpapers();

            // Restore position for new screen (if saved)
            if let Some(&pos) = self.screen_positions.get(&self.selected_screen_idx) {
                if pos < self.filtered_wallpapers.len() {
                    self.selected_wallpaper_idx = pos;
                }
            }

            self.update_pairing_suggestions();
        }
    }

    pub fn prev_screen(&mut self) {
        if !self.screens.is_empty() {
            // Save current position
            self.screen_positions.insert(self.selected_screen_idx, self.selected_wallpaper_idx);

            self.selected_screen_idx = if self.selected_screen_idx == 0 {
                self.screens.len() - 1
            } else {
                self.selected_screen_idx - 1
            };
            self.update_filtered_wallpapers();

            // Restore position for new screen (if saved)
            if let Some(&pos) = self.screen_positions.get(&self.selected_screen_idx) {
                if pos < self.filtered_wallpapers.len() {
                    self.selected_wallpaper_idx = pos;
                }
            }

            self.update_pairing_suggestions();
        }
    }

    pub fn apply_wallpaper(&mut self) -> Result<()> {
        if let (Some(screen), Some(wp)) = (self.selected_screen(), self.selected_wallpaper()) {
            let screen_name = screen.name.clone();
            let wp_path = wp.path.clone();
            let wp_colors = wp.colors.clone();

            // Update current wallpaper for this screen
            self.current_wallpapers.insert(screen_name.clone(), wp_path.clone());

            swww::set_wallpaper_with_resize(
                &screen_name,
                &wp_path,
                &self.config.transition(),
                self.config.display.resize_mode,
                &self.config.display.fill_color,
            )?;

            // Export pywal colors if enabled
            if self.pywal_export {
                if let Err(e) = crate::pywal::generate_from_wallpaper(&wp_colors, &wp_path) {
                    self.last_error = Some(format!("pywal: {}", e));
                }
            }
        }
        Ok(())
    }

    /// Handle undo action (restore previous wallpapers)
    pub fn do_undo(&mut self) -> Result<()> {
        if let Some(previous) = self.pairing_history.do_undo() {
            for (screen_name, wp_path) in &previous {
                swww::set_wallpaper_with_resize(
                    screen_name,
                    wp_path,
                    &self.config.transition(),
                    self.config.display.resize_mode,
                    &self.config.display.fill_color,
                )?;
            }
            // Restore current_wallpapers tracking
            self.current_wallpapers = previous;
        }
        Ok(())
    }

    /// Check and clear expired undo window
    pub fn tick_undo(&mut self) {
        self.pairing_history.clear_expired_undo();
    }

    pub fn random_wallpaper(&mut self) -> Result<()> {
        if !self.filtered_wallpapers.is_empty() {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            self.selected_wallpaper_idx = rng.gen_range(0..self.filtered_wallpapers.len());

            // Apply immediately
            self.apply_wallpaper()?;
        }
        Ok(())
    }

    /// Request a thumbnail to be loaded in background
    pub fn request_thumbnail(&mut self, cache_idx: usize) {
        // Bounds check
        if cache_idx >= self.cache.wallpapers.len() {
            return;
        }

        // Skip if already loaded or loading
        if self.thumbnail_cache.contains_key(&cache_idx)
            || self.loading_thumbnails.contains(&cache_idx)
        {
            return;
        }

        if let Some(wp) = self.cache.wallpapers.get(cache_idx) {
            if let Some(tx) = &self.thumb_request_tx {
                let request = ThumbnailRequest {
                    cache_idx,
                    source_path: wp.path.clone(),
                };
                if tx.send(request).is_ok() {
                    self.loading_thumbnails.insert(cache_idx);
                }
            }
        }
    }

    /// Handle a loaded thumbnail from background thread
    pub fn handle_thumbnail_ready(&mut self, response: ThumbnailResponse) {
        self.loading_thumbnails.remove(&response.cache_idx);

        if let Some(picker) = &mut self.image_picker {
            // Evict oldest entries if cache is full
            while self.thumbnail_cache.len() >= MAX_THUMBNAIL_CACHE {
                if let Some(oldest_idx) = self.thumbnail_cache_order.first().copied() {
                    self.thumbnail_cache.remove(&oldest_idx);
                    self.thumbnail_cache_order.remove(0);
                } else {
                    break;
                }
            }

            let protocol = picker.new_resize_protocol(response.image);
            self.thumbnail_cache.insert(response.cache_idx, protocol);
            self.thumbnail_cache_order.push(response.cache_idx);
        }
    }

    /// Check if a thumbnail is ready (also updates LRU order)
    pub fn get_thumbnail(&mut self, cache_idx: usize) -> Option<&mut Box<dyn StatefulProtocol>> {
        if self.thumbnail_cache.contains_key(&cache_idx) {
            // Move to end of LRU order (most recently used)
            if let Some(pos) = self.thumbnail_cache_order.iter().position(|&i| i == cache_idx) {
                self.thumbnail_cache_order.remove(pos);
                self.thumbnail_cache_order.push(cache_idx);
            }
        }
        self.thumbnail_cache.get_mut(&cache_idx)
    }

    /// Check if a thumbnail is currently loading
    pub fn is_loading(&self, cache_idx: usize) -> bool {
        self.loading_thumbnails.contains(&cache_idx)
    }

    /// Set the thumbnail request channel
    pub fn set_thumb_channel(&mut self, tx: Sender<ThumbnailRequest>) {
        self.thumb_request_tx = Some(tx);
    }

    /// Toggle help popup
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Cycle through sort modes
    pub fn toggle_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.apply_sort();
    }

    /// Apply current sort mode to filtered wallpapers
    fn apply_sort(&mut self) {
        let cache = &self.cache;
        let sort_mode = self.sort_mode;

        self.filtered_wallpapers.sort_by(|&a, &b| {
            let wp_a = &cache.wallpapers[a];
            let wp_b = &cache.wallpapers[b];

            match sort_mode {
                SortMode::Name => wp_a.path.cmp(&wp_b.path),
                SortMode::Size => {
                    // Use cached file_size (no filesystem calls)
                    wp_b.file_size.cmp(&wp_a.file_size) // Largest first
                }
                SortMode::Date => {
                    // Use cached modified_at (no filesystem calls)
                    wp_b.modified_at.cmp(&wp_a.modified_at) // Newest first
                }
            }
        });

        // Reset selection after sort
        self.selected_wallpaper_idx = 0;
    }

    /// Toggle color display for selected wallpaper
    pub fn toggle_colors(&mut self) {
        self.show_colors = !self.show_colors;
    }

    /// Cycle through available tags as filter
    pub fn cycle_tag_filter(&mut self) {
        let all_tags = self.cache.all_tags();

        if all_tags.is_empty() {
            self.active_tag_filter = None;
            self.last_error = Some("No tags defined. Use 'frostwall tag add <path> <tag>' to add tags.".to_string());
            return;
        }

        self.active_tag_filter = match &self.active_tag_filter {
            None => Some(all_tags[0].clone()),
            Some(current) => {
                // Find current position and move to next
                if let Some(pos) = all_tags.iter().position(|t| t == current) {
                    if pos + 1 < all_tags.len() {
                        Some(all_tags[pos + 1].clone())
                    } else {
                        None // Wrap around to "all"
                    }
                } else {
                    None
                }
            }
        };

        // Clear any previous error
        self.last_error = None;
        self.update_filtered_wallpapers();
    }

    /// Clear tag filter
    pub fn clear_tag_filter(&mut self) {
        self.active_tag_filter = None;
        self.update_filtered_wallpapers();
    }

    /// Get available tags
    #[allow(dead_code)]
    pub fn available_tags(&self) -> Vec<String> {
        self.cache.all_tags()
    }

    // ===== Command Mode (vim-style :) =====

    /// Enter command mode
    pub fn enter_command_mode(&mut self) {
        self.command_mode = true;
        self.command_buffer.clear();
    }

    /// Exit command mode without executing
    pub fn exit_command_mode(&mut self) {
        self.command_mode = false;
        self.command_buffer.clear();
    }

    /// Add character to command buffer
    pub fn command_input(&mut self, c: char) {
        self.command_buffer.push(c);
    }

    /// Remove last character from command buffer
    pub fn command_backspace(&mut self) {
        self.command_buffer.pop();
    }

    /// Execute the current command
    pub fn execute_command(&mut self) {
        let cmd = self.command_buffer.trim().to_string();
        self.command_mode = false;
        self.command_buffer.clear();

        if cmd.is_empty() {
            return;
        }

        // Parse command and args
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match command.as_str() {
            // Quit
            "q" | "quit" | "exit" => {
                self.should_quit = true;
            }

            // Tag filter
            "t" | "tag" => {
                if args.is_empty() {
                    // List available tags
                    let tags = self.cache.all_tags();
                    if tags.is_empty() {
                        self.last_error = Some("No tags available".to_string());
                    } else {
                        self.last_error = Some(format!("Tags: {}", tags.join(", ")));
                    }
                } else {
                    // Filter by tag
                    let tag = args.to_string();
                    let tags = self.cache.all_tags();
                    // Fuzzy match - find tag that contains the search term
                    if let Some(matched) = tags.iter().find(|t| t.to_lowercase().contains(&args.to_lowercase())) {
                        self.active_tag_filter = Some(matched.clone());
                        self.update_filtered_wallpapers();
                    } else {
                        self.last_error = Some(format!("Tag not found: {}", tag));
                    }
                }
            }

            // Clear filters
            "c" | "clear" => {
                self.active_tag_filter = None;
                self.active_color_filter = None;
                self.update_filtered_wallpapers();
            }

            // Random wallpaper
            "r" | "random" => {
                let _ = self.random_wallpaper();
            }

            // Apply current wallpaper
            "a" | "apply" => {
                let _ = self.apply_wallpaper();
            }

            // Sort mode
            "sort" => {
                match args.to_lowercase().as_str() {
                    "name" | "n" => {
                        self.sort_mode = SortMode::Name;
                        self.update_filtered_wallpapers();
                    }
                    "date" | "d" => {
                        self.sort_mode = SortMode::Date;
                        self.update_filtered_wallpapers();
                    }
                    "size" | "s" => {
                        self.sort_mode = SortMode::Size;
                        self.update_filtered_wallpapers();
                    }
                    _ => {
                        self.last_error = Some("Sort modes: name, date, size".to_string());
                    }
                }
            }

            // Similar wallpapers
            "similar" | "sim" => {
                if let Some(wp) = self.selected_wallpaper() {
                    let colors = wp.colors.clone();
                    let path = wp.path.clone();
                    self.find_and_select_similar(&colors, &path);
                }
            }

            // Help
            "h" | "help" => {
                self.show_help = true;
            }

            // Screen navigation
            "screen" => {
                if let Ok(n) = args.parse::<usize>() {
                    if n > 0 && n <= self.screens.len() {
                        // Save current position
                        self.screen_positions.insert(self.selected_screen_idx, self.selected_wallpaper_idx);
                        self.selected_screen_idx = n - 1;
                        self.update_filtered_wallpapers();
                        // Restore position for new screen
                        if let Some(&pos) = self.screen_positions.get(&self.selected_screen_idx) {
                            if pos < self.filtered_wallpapers.len() {
                                self.selected_wallpaper_idx = pos;
                            }
                        }
                    } else {
                        self.last_error = Some(format!("Screen {} not found", n));
                    }
                }
            }

            // Go to wallpaper by number
            "go" | "g" => {
                if let Ok(n) = args.parse::<usize>() {
                    if n > 0 && n <= self.filtered_wallpapers.len() {
                        self.selected_wallpaper_idx = n - 1;
                    }
                }
            }

            _ => {
                self.last_error = Some(format!("Unknown command: {}", command));
            }
        }
    }

    /// Find similar wallpapers and select the best match
    fn find_and_select_similar(&mut self, colors: &[String], current_path: &std::path::Path) {
        let wallpaper_colors: Vec<(usize, &[String])> = self.cache.wallpapers
            .iter()
            .enumerate()
            .filter(|(_, wp)| wp.path != current_path && !wp.colors.is_empty())
            .map(|(i, wp)| (i, wp.colors.as_slice()))
            .collect();

        let similar = crate::utils::find_similar_wallpapers(colors, &wallpaper_colors, 1);
        if let Some((_, idx)) = similar.first() {
            // Find this index in filtered wallpapers
            if let Some(pos) = self.filtered_wallpapers.iter().position(|&i| i == *idx) {
                self.selected_wallpaper_idx = pos;
            }
        }
    }

    /// Toggle color picker popup
    pub fn toggle_color_picker(&mut self) {
        if !self.show_color_picker {
            // Build list of unique colors from all wallpapers
            self.available_colors = self.get_unique_colors();
            self.color_picker_idx = 0;
        }
        self.show_color_picker = !self.show_color_picker;
    }

    /// Get unique colors across all wallpapers
    fn get_unique_colors(&self) -> Vec<String> {
        let mut colors: Vec<String> = self.cache.wallpapers
            .iter()
            .flat_map(|wp| wp.colors.iter().cloned())
            .collect();
        colors.sort();
        colors.dedup();
        // Limit to reasonable number
        colors.truncate(32);
        colors
    }

    /// Navigate color picker
    pub fn color_picker_next(&mut self) {
        if !self.available_colors.is_empty() {
            self.color_picker_idx = (self.color_picker_idx + 1) % self.available_colors.len();
        }
    }

    pub fn color_picker_prev(&mut self) {
        if !self.available_colors.is_empty() {
            self.color_picker_idx = if self.color_picker_idx == 0 {
                self.available_colors.len() - 1
            } else {
                self.color_picker_idx - 1
            };
        }
    }

    /// Apply selected color filter
    pub fn apply_color_filter(&mut self) {
        if let Some(color) = self.available_colors.get(self.color_picker_idx) {
            self.active_color_filter = Some(color.clone());
            self.show_color_picker = false;
            self.update_filtered_wallpapers();
        }
    }

    /// Clear color filter
    pub fn clear_color_filter(&mut self) {
        self.active_color_filter = None;
        self.update_filtered_wallpapers();
    }

    /// Export pywal colors for current wallpaper
    pub fn export_pywal(&self) -> Result<()> {
        if let Some(wp) = self.selected_wallpaper() {
            crate::pywal::generate_from_wallpaper(&wp.colors, &wp.path)?;
        }
        Ok(())
    }

    /// Toggle pywal export on apply
    pub fn toggle_pywal_export(&mut self) {
        self.pywal_export = !self.pywal_export;
    }

    /// Update pairing suggestions based on currently selected wallpaper
    pub fn update_pairing_suggestions(&mut self) {
        self.pairing_suggestions.clear();

        if !self.config.pairing.enabled {
            return;
        }

        // Get selected wallpaper path and colors
        let (selected_path, selected_colors) = match self.selected_wallpaper() {
            Some(wp) => (wp.path.clone(), wp.colors.clone()),
            None => return,
        };

        // Get suggestions from pairing history
        let match_mode = self.config.display.match_mode;

        // For each other screen, find suggested wallpapers
        for (screen_idx, screen) in self.screens.iter().enumerate() {
            if screen_idx == self.selected_screen_idx {
                continue;
            }

            // Get wallpapers that match this screen
            let matching: Vec<_> = self.cache.wallpapers.iter()
                .filter(|wp| wp.matches_screen_with_mode(screen, match_mode))
                .collect();

            // Find best match based on pairing history + color similarity
            if let Some(suggested_path) = self.pairing_history.get_best_match(
                &selected_path,
                &screen.name,
                &matching,
                &selected_colors,
            ) {
                if !self.pairing_suggestions.contains(&suggested_path) {
                    self.pairing_suggestions.push(suggested_path);
                }
            }
        }
    }

    /// Check if a wallpaper is in the pairing suggestions
    pub fn is_pairing_suggestion(&self, path: &std::path::Path) -> bool {
        self.pairing_suggestions.iter().any(|p| p == path)
    }

    /// Toggle pairing preview popup
    pub fn toggle_pairing_preview(&mut self) {
        if !self.show_pairing_preview {
            self.update_pairing_preview_matches();
        }
        self.show_pairing_preview = !self.show_pairing_preview;
        self.pairing_preview_idx = 0;
    }

    /// Update pairing preview matches for all other screens
    fn update_pairing_preview_matches(&mut self) {
        self.pairing_preview_matches.clear();

        if !self.config.pairing.enabled || self.screens.len() <= 1 {
            return;
        }

        let (selected_path, selected_colors, selected_weights) = match self.selected_wallpaper() {
            Some(wp) => (wp.path.clone(), wp.colors.clone(), wp.color_weights.clone()),
            None => return,
        };

        // Default weights if empty
        let selected_weights = if selected_weights.is_empty() {
            vec![1.0 / selected_colors.len().max(1) as f32; selected_colors.len()]
        } else {
            selected_weights
        };

        let match_mode = self.config.display.match_mode;

        for (screen_idx, screen) in self.screens.iter().enumerate() {
            if screen_idx == self.selected_screen_idx {
                continue;
            }

            // Get wallpapers that match this screen
            let matching: Vec<_> = self.cache.wallpapers.iter()
                .filter(|wp| wp.matches_screen_with_mode(screen, match_mode))
                .collect();

            // Get top 5 matches
            let top_matches = self.pairing_history.get_top_matches(
                &selected_path,
                &screen.name,
                &matching,
                &selected_colors,
                5,
            );

            // Calculate harmony for each match
            let matches_with_harmony: Vec<(PathBuf, f32, ColorHarmony)> = top_matches
                .into_iter()
                .map(|(path, score)| {
                    // Find the wallpaper to get its colors and weights
                    let harmony = self.cache.wallpapers.iter()
                        .find(|wp| wp.path == path)
                        .map(|wp| {
                            let wp_weights = if wp.color_weights.is_empty() {
                                vec![1.0 / wp.colors.len().max(1) as f32; wp.colors.len()]
                            } else {
                                wp.color_weights.clone()
                            };
                            let (harmony, _strength) = crate::utils::detect_harmony(
                                &selected_colors,
                                &selected_weights,
                                &wp.colors,
                                &wp_weights,
                            );
                            harmony
                        })
                        .unwrap_or(ColorHarmony::None);
                    (path, score, harmony)
                })
                .collect();

            if !matches_with_harmony.is_empty() {
                self.pairing_preview_matches.insert(screen.name.clone(), matches_with_harmony);
            }
        }
    }

    /// Cycle through pairing preview alternatives
    pub fn pairing_preview_next(&mut self) {
        let max_alternatives = self.pairing_preview_matches.values()
            .map(|v| v.len())
            .max()
            .unwrap_or(1);

        if max_alternatives > 0 {
            self.pairing_preview_idx = (self.pairing_preview_idx + 1) % max_alternatives;
        }
    }

    pub fn pairing_preview_prev(&mut self) {
        let max_alternatives = self.pairing_preview_matches.values()
            .map(|v| v.len())
            .max()
            .unwrap_or(1);

        if max_alternatives > 0 {
            self.pairing_preview_idx = if self.pairing_preview_idx == 0 {
                max_alternatives - 1
            } else {
                self.pairing_preview_idx - 1
            };
        }
    }

    /// Apply the currently selected pairing preview
    pub fn apply_pairing_preview(&mut self) -> Result<()> {
        if !self.show_pairing_preview {
            return Ok(());
        }

        // First apply the selected wallpaper to current screen
        self.apply_wallpaper()?;

        // Then apply the preview selections to other screens
        for (screen_name, matches) in &self.pairing_preview_matches {
            let idx = self.pairing_preview_idx.min(matches.len().saturating_sub(1));
            if let Some((wp_path, _, _)) = matches.get(idx) {
                if let Err(e) = swww::set_wallpaper_with_resize(
                    screen_name,
                    wp_path,
                    &self.config.transition(),
                    self.config.display.resize_mode,
                    &self.config.display.fill_color,
                ) {
                    self.last_error = Some(format!("Pairing {}: {}", screen_name, e));
                } else {
                    self.current_wallpapers.insert(screen_name.clone(), wp_path.clone());
                }
            }
        }

        // Record the pairing
        if self.current_wallpapers.len() > 1 {
            self.pairing_history.record_pairing(self.current_wallpapers.clone(), true);
        }

        self.show_pairing_preview = false;
        Ok(())
    }

    /// Get the number of alternatives available in pairing preview
    pub fn pairing_preview_alternatives(&self) -> usize {
        self.pairing_preview_matches.values()
            .map(|v| v.len())
            .max()
            .unwrap_or(0)
    }
}

pub async fn run_tui(wallpaper_dir: PathBuf) -> Result<()> {
    let mut app = App::new(wallpaper_dir)?;

    // Show terminal optimization hint if first run in Kitty
    if let Some(hint) = app.config.check_terminal_hint() {
        println!("\n{}\n", hint);
        // Wait for keypress
        enable_raw_mode()?;
        let _ = event::read();
        disable_raw_mode()?;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    app.init_screens().await?;

    // Set up channels for background thumbnail loading
    let (thumb_tx, thumb_rx) = mpsc::channel::<ThumbnailRequest>();
    let (event_tx, event_rx) = mpsc::channel::<AppEvent>();

    app.set_thumb_channel(thumb_tx);

    // Spawn thumbnail worker thread
    let event_tx_thumb = event_tx.clone();
    let disk_cache = ThumbnailCache::new();
    thread::spawn(move || {
        thumbnail_worker(thumb_rx, event_tx_thumb, disk_cache);
    });

    // Spawn event polling thread
    let event_tx_input = event_tx.clone();
    thread::spawn(move || {
        input_worker(event_tx_input);
    });

    let res = run_app(&mut terminal, &mut app, event_rx);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    app.cache.save()?;
    app.config.save()?;

    res
}

/// Background thread that loads thumbnails using fast_image_resize
fn thumbnail_worker(
    rx: Receiver<ThumbnailRequest>,
    tx: Sender<AppEvent>,
    disk_cache: ThumbnailCache,
) {
    while let Ok(request) = rx.recv() {
        // Load thumbnail (uses fast_image_resize with disk caching)
        match disk_cache.load(&request.source_path) {
            Ok(image) => {
                let response = ThumbnailResponse {
                    cache_idx: request.cache_idx,
                    image,
                };
                if tx.send(AppEvent::ThumbnailReady(response)).is_err() {
                    break;
                }
            }
            Err(e) => {
                eprintln!(
                    "Thumbnail failed for {}: {}",
                    request.source_path.display(),
                    e
                );
            }
        }
    }
}

/// Background thread that polls for input events
fn input_worker(tx: Sender<AppEvent>) {
    loop {
        if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                if tx.send(AppEvent::Key(key)).is_err() {
                    break;
                }
            }
        } else if tx.send(AppEvent::Tick).is_err() {
            break;
        }
    }
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    event_rx: Receiver<AppEvent>,
) -> Result<()> {
    let mut last_theme_check = std::time::Instant::now();
    let mut current_theme_is_light = crate::ui::theme::is_light_theme();
    let mut needs_redraw = true;

    loop {
        // Check for theme change every 500ms and force full redraw
        if last_theme_check.elapsed() >= std::time::Duration::from_millis(500) {
            let new_is_light = crate::ui::theme::is_light_theme();
            if new_is_light != current_theme_is_light {
                current_theme_is_light = new_is_light;
                terminal.clear()?;  // Force full terminal redraw
                needs_redraw = true;
            }
            last_theme_check = std::time::Instant::now();
        }

        // Only redraw when needed (event received or state changed)
        if needs_redraw {
            terminal.draw(|f| ui::draw(f, app))?;
            needs_redraw = false;
        }

        // Block until event arrives (with timeout for theme checks)
        let events: Vec<AppEvent> = match event_rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(event) => {
                needs_redraw = true;
                let mut events = vec![event];
                while let Ok(e) = event_rx.try_recv() {
                    events.push(e);
                }
                events
            }
            Err(_) => continue, // Timeout, check theme and loop
        };

        for event in events {
            match event {
                AppEvent::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    // Handle help popup first (blocks other input)
                    if app.show_help {
                        match key.code {
                            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Enter => {
                                app.show_help = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Handle color picker popup
                    if app.show_color_picker {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('C') => {
                                app.show_color_picker = false;
                            }
                            KeyCode::Char('l') | KeyCode::Right => {
                                app.color_picker_next();
                            }
                            KeyCode::Char('h') | KeyCode::Left => {
                                app.color_picker_prev();
                            }
                            KeyCode::Enter => {
                                app.apply_color_filter();
                            }
                            KeyCode::Char('x') | KeyCode::Backspace => {
                                app.clear_color_filter();
                                app.show_color_picker = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Handle pairing preview popup
                    if app.show_pairing_preview {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('p') => {
                                app.show_pairing_preview = false;
                            }
                            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char('n') => {
                                app.pairing_preview_next();
                            }
                            KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('N') => {
                                app.pairing_preview_prev();
                            }
                            KeyCode::Enter => {
                                if let Err(e) = app.apply_pairing_preview() {
                                    app.last_error = Some(format!("{}", e));
                                }
                            }
                            KeyCode::Char('1') => {
                                app.pairing_preview_idx = 0;
                            }
                            KeyCode::Char('2') => {
                                app.pairing_preview_idx = 1;
                            }
                            KeyCode::Char('3') => {
                                app.pairing_preview_idx = 2;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Handle command mode (vim-style :)
                    if app.command_mode {
                        match key.code {
                            KeyCode::Esc => {
                                app.exit_command_mode();
                            }
                            KeyCode::Enter => {
                                app.execute_command();
                            }
                            KeyCode::Backspace => {
                                app.command_backspace();
                            }
                            KeyCode::Char(c) => {
                                app.command_input(c);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Use configurable keybindings
                    let kb = &app.config.keybindings;
                    let code = key.code;

                    // Quit (configurable + Esc always works)
                    if kb.matches(code, &kb.quit.clone()) || code == KeyCode::Esc {
                        app.should_quit = true;
                    }
                    // Navigation (configurable + arrow keys always work)
                    else if kb.matches(code, &kb.next.clone()) || code == KeyCode::Right {
                        app.next_wallpaper();
                    } else if kb.matches(code, &kb.prev.clone()) || code == KeyCode::Left {
                        app.prev_wallpaper();
                    }
                    // Screen navigation (configurable)
                    else if kb.matches(code, &kb.next_screen.clone()) {
                        app.next_screen();
                    } else if kb.matches(code, &kb.prev_screen.clone()) {
                        app.prev_screen();
                    }
                    // Apply wallpaper (configurable)
                    else if kb.matches(code, &kb.apply.clone()) {
                        if let Err(e) = app.apply_wallpaper() {
                            app.last_error = Some(format!("{}", e));
                        }
                    }
                    // Random wallpaper (configurable)
                    else if kb.matches(code, &kb.random.clone()) {
                        if let Err(e) = app.random_wallpaper() {
                            app.last_error = Some(format!("{}", e));
                        }
                    }
                    // Toggle match mode (configurable)
                    else if kb.matches(code, &kb.toggle_match.clone()) {
                        app.toggle_match_mode();
                    }
                    // Toggle resize mode (configurable)
                    else if kb.matches(code, &kb.toggle_resize.clone()) {
                        app.toggle_resize_mode();
                    }
                    // Non-configurable keys
                    else {
                        match code {
                            KeyCode::Char(':') => app.enter_command_mode(),
                            KeyCode::Char('?') => app.toggle_help(),
                            KeyCode::Char('s') => app.toggle_sort_mode(),
                            KeyCode::Char('c') => app.toggle_colors(),
                            KeyCode::Char('C') => app.toggle_color_picker(),
                            KeyCode::Char('p') => app.toggle_pairing_preview(),
                            KeyCode::Char('t') => app.cycle_tag_filter(),
                            KeyCode::Char('T') => app.clear_tag_filter(),
                            KeyCode::Char('w') => {
                                if let Err(e) = app.export_pywal() {
                                    app.last_error = Some(format!("pywal: {}", e));
                                }
                            }
                            KeyCode::Char('W') => app.toggle_pywal_export(),
                            KeyCode::Char('u') => {
                                // Undo pairing
                                if let Err(e) = app.do_undo() {
                                    app.last_error = Some(format!("Undo: {}", e));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                AppEvent::ThumbnailReady(response) => {
                    app.handle_thumbnail_ready(response);
                }
                AppEvent::Tick => {
                    // Check for expired undo window
                    app.tick_undo();
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
