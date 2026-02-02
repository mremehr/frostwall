mod app;
mod clip;
mod clip_embeddings;
mod collections;
mod init;
mod pairing;
mod profile;
mod pywal;
mod screen;
mod swww;
mod thumbnail;
mod timeprofile;
mod ui;
mod utils;
mod wallpaper;
mod watch;
mod webimport;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "frostwall")]
#[command(author = "MrMattias")]
#[command(version = "0.4.0")]
#[command(about = "Intelligent wallpaper manager with screen-aware matching")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Wallpaper directory
    #[arg(short, long)]
    dir: Option<PathBuf>,

    /// Use a specific profile
    #[arg(short, long)]
    profile: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Set a random wallpaper (smart-matched to screens)
    Random,
    /// Set next wallpaper in sequence
    Next,
    /// Set previous wallpaper in sequence
    Prev,
    /// List available screens
    Screens,
    /// Rescan wallpaper directory and update cache
    Scan,
    /// Interactive setup wizard for new users
    Init,
    /// Run watch daemon for automatic wallpaper rotation
    Watch {
        /// Rotation interval (e.g., "30m", "1h", "90s")
        #[arg(short, long, default_value = "30m")]
        interval: String,

        /// Shuffle wallpapers randomly
        #[arg(short, long, default_value = "true")]
        shuffle: bool,

        /// Watch directory for new files
        #[arg(short = 'w', long, default_value = "true")]
        watch_dir: bool,
    },
    /// Manage configuration profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Manage wallpaper tags
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Generate pywal color scheme from wallpaper
    Pywal {
        /// Path to wallpaper image
        path: PathBuf,
        /// Apply colors immediately (xrdb merge)
        #[arg(short, long)]
        apply: bool,
    },
    /// Manage intelligent wallpaper pairing
    Pair {
        #[command(subcommand)]
        action: PairAction,
    },
    /// Auto-tag wallpapers using CLIP AI model (requires --features clip)
    #[cfg(feature = "clip")]
    AutoTag {
        /// Only tag wallpapers missing auto-tags
        #[arg(short, long)]
        incremental: bool,

        /// Confidence threshold (0.0-1.0, default 0.55)
        #[arg(short, long, default_value = "0.55")]
        threshold: f32,

        /// Maximum number of tags per image (0 = unlimited)
        #[arg(short = 'n', long, default_value = "5")]
        max_tags: usize,

        /// Show detailed progress
        #[arg(short, long)]
        verbose: bool,
    },
    /// Manage wallpaper collections (saved presets)
    Collection {
        #[command(subcommand)]
        action: CollectionAction,
    },
    /// Find similar wallpapers based on color profile
    Similar {
        /// Path to wallpaper to find similar ones for
        path: PathBuf,
        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Manage time-based wallpaper profiles
    TimeProfile {
        #[command(subcommand)]
        action: TimeProfileAction,
    },
    /// Import wallpapers from web galleries (Unsplash, Wallhaven)
    Import {
        #[command(subcommand)]
        action: ImportAction,
    },
}

#[derive(Subcommand)]
enum TagAction {
    /// List all tags
    List,
    /// Add a tag to a wallpaper
    Add {
        /// Path to wallpaper
        path: PathBuf,
        /// Tag to add
        tag: String,
    },
    /// Remove a tag from a wallpaper
    Remove {
        /// Path to wallpaper
        path: PathBuf,
        /// Tag to remove
        tag: String,
    },
    /// Show wallpapers with a specific tag
    Show {
        /// Tag to filter by
        tag: String,
    },
}

#[derive(Subcommand)]
enum PairAction {
    /// Show pairing statistics
    Stats,
    /// Clear all pairing history
    Clear,
    /// Show suggestions for a specific wallpaper
    Suggest {
        /// Path to wallpaper
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum CollectionAction {
    /// List all saved collections
    List,
    /// Show details of a collection
    Show {
        /// Collection name
        name: String,
    },
    /// Save current wallpapers as a collection
    Save {
        /// Collection name
        name: String,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Apply a saved collection
    Apply {
        /// Collection name
        name: String,
    },
    /// Delete a collection
    Delete {
        /// Collection name
        name: String,
    },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// List all profiles
    List,
    /// Create a new profile
    Create {
        /// Profile name
        name: String,
    },
    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },
    /// Switch to a profile
    Use {
        /// Profile name
        name: String,
    },
    /// Set a profile option
    Set {
        /// Profile name
        name: String,
        /// Setting key (directory, match_mode, resize_mode, transition, recursive)
        key: String,
        /// Setting value
        value: String,
    },
}

#[derive(Subcommand)]
enum TimeProfileAction {
    /// Show current time period and settings
    Status,
    /// Enable time-based profiles
    Enable,
    /// Disable time-based profiles
    Disable,
    /// Preview wallpapers matching current time
    Preview {
        /// Maximum number of wallpapers to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Set a random wallpaper based on current time
    Apply,
}

#[derive(Subcommand)]
enum ImportAction {
    /// Search and import from Unsplash
    Unsplash {
        /// Search query
        query: String,
        /// Number of images to show
        #[arg(short, long, default_value = "10")]
        count: u32,
    },
    /// Search and import from Wallhaven
    Wallhaven {
        /// Search query
        query: String,
        /// Number of images to show
        #[arg(short, long, default_value = "10")]
        count: u32,
    },
    /// Get featured/top wallpapers from Wallhaven
    Featured {
        /// Number of images to show
        #[arg(short, long, default_value = "10")]
        count: u32,
    },
    /// Download a specific image by URL or ID
    Download {
        /// Image URL or Wallhaven ID (e.g., "w8x7y9")
        url: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = app::Config::load()?;
    let wallpaper_dir = cli.dir.unwrap_or_else(|| config.wallpaper_dir());

    match cli.command {
        Some(Commands::Random) => {
            cmd_random(&wallpaper_dir).await?;
        }
        Some(Commands::Next) => {
            cmd_next(&wallpaper_dir).await?;
        }
        Some(Commands::Prev) => {
            cmd_prev(&wallpaper_dir).await?;
        }
        Some(Commands::Screens) => {
            cmd_screens().await?;
        }
        Some(Commands::Scan) => {
            cmd_scan(&wallpaper_dir).await?;
        }
        Some(Commands::Init) => {
            init::run_init().await?;
        }
        Some(Commands::Watch { interval, shuffle, watch_dir }) => {
            let interval = watch::parse_interval(&interval)
                .unwrap_or_else(|| std::time::Duration::from_secs(30 * 60));
            let watch_config = watch::WatchConfig {
                interval,
                shuffle,
                watch_dir,
            };
            watch::run_watch(watch_config).await?;
        }
        Some(Commands::Profile { action }) => {
            match action {
                ProfileAction::List => profile::cmd_profile_list()?,
                ProfileAction::Create { name } => profile::cmd_profile_create(&name)?,
                ProfileAction::Delete { name } => profile::cmd_profile_delete(&name)?,
                ProfileAction::Use { name } => profile::cmd_profile_use(&name)?,
                ProfileAction::Set { name, key, value } => {
                    profile::cmd_profile_set(&name, &key, &value)?
                }
            }
        }
        Some(Commands::Tag { action }) => {
            cmd_tag(action, &wallpaper_dir)?;
        }
        Some(Commands::Pywal { path, apply }) => {
            pywal::cmd_pywal(&path, apply)?;
        }
        Some(Commands::Pair { action }) => {
            cmd_pair(action, &wallpaper_dir)?;
        }
        #[cfg(feature = "clip")]
        Some(Commands::AutoTag { incremental, threshold, max_tags, verbose }) => {
            cmd_auto_tag(&wallpaper_dir, incremental, threshold, max_tags, verbose).await?;
        }
        Some(Commands::Collection { action }) => {
            cmd_collection(action).await?;
        }
        Some(Commands::Similar { path, limit }) => {
            cmd_similar(&wallpaper_dir, &path, limit)?;
        }
        Some(Commands::TimeProfile { action }) => {
            cmd_time_profile(action, &wallpaper_dir).await?;
        }
        Some(Commands::Import { action }) => {
            cmd_import(action, &wallpaper_dir)?;
        }
        None => {
            // TUI mode
            app::run_tui(wallpaper_dir).await?;
        }
    }

    Ok(())
}

async fn cmd_random(wallpaper_dir: &Path) -> Result<()> {
    let screens = screen::detect_screens().await?;
    let cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;

    for screen in &screens {
        if let Some(wp) = cache.random_for_screen(screen) {
            swww::set_wallpaper(&screen.name, &wp.path, &swww::Transition::default())?;
            println!("{}: {}", screen.name, wp.path.display());
        }
    }

    Ok(())
}

async fn cmd_next(wallpaper_dir: &Path) -> Result<()> {
    let screens = screen::detect_screens().await?;
    let mut cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;

    for screen in &screens {
        if let Some(wp) = cache.next_for_screen(screen) {
            swww::set_wallpaper(&screen.name, &wp.path, &swww::Transition::default())?;
            println!("{}: {}", screen.name, wp.path.display());
        }
    }

    cache.save()?;
    Ok(())
}

async fn cmd_prev(wallpaper_dir: &Path) -> Result<()> {
    let screens = screen::detect_screens().await?;
    let mut cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;

    for screen in &screens {
        if let Some(wp) = cache.prev_for_screen(screen) {
            swww::set_wallpaper(&screen.name, &wp.path, &swww::Transition::default())?;
            println!("{}: {}", screen.name, wp.path.display());
        }
    }

    cache.save()?;
    Ok(())
}

async fn cmd_screens() -> Result<()> {
    let screens = screen::detect_screens().await?;

    for screen in &screens {
        println!(
            "{}: {}x{} ({:?}) - {:?}",
            screen.name, screen.width, screen.height, screen.orientation, screen.aspect_category
        );
    }

    Ok(())
}

async fn cmd_scan(wallpaper_dir: &Path) -> Result<()> {
    println!("Scanning {}...", wallpaper_dir.display());
    let cache = wallpaper::WallpaperCache::scan(wallpaper_dir)?;
    cache.save()?;

    let stats = cache.stats();
    println!("Found {} wallpapers:", stats.total);
    println!("  Ultrawide: {}", stats.ultrawide);
    println!("  Landscape: {}", stats.landscape);
    println!("  Portrait:  {}", stats.portrait);
    println!("  Square:    {}", stats.square);

    Ok(())
}

fn cmd_pair(action: PairAction, wallpaper_dir: &Path) -> Result<()> {
    let config = app::Config::load()?;

    match action {
        PairAction::Stats => {
            let history = pairing::PairingHistory::load(config.pairing.max_history_records)?;
            println!("Pairing Statistics");
            println!("==================");
            println!("  Records: {}", history.record_count());
            println!("  Affinity pairs: {}", history.affinity_count());
            println!();
            println!("Pairing is {}", if config.pairing.enabled { "enabled" } else { "disabled" });
            println!("Auto-apply is {}", if config.pairing.auto_apply { "enabled" } else { "disabled" });
        }
        PairAction::Clear => {
            let history = pairing::PairingHistory::new(config.pairing.max_history_records);
            history.save()?;
            println!("✓ Pairing history cleared");
        }
        PairAction::Suggest { path } => {
            let history = pairing::PairingHistory::load(config.pairing.max_history_records)?;
            let cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;

            // Find wallpapers with affinity to the given path
            let mut suggestions: Vec<_> = cache.wallpapers.iter()
                .filter(|wp| wp.path != path)
                .map(|wp| {
                    let affinity = history.get_affinity(&path, &wp.path);
                    (wp, affinity)
                })
                .filter(|(_, affinity)| *affinity > 0.0)
                .collect();

            suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            if suggestions.is_empty() {
                println!("No pairing suggestions for: {}", path.display());
                println!("Use wallpapers together to build pairing history.");
            } else {
                println!("Pairing suggestions for: {}", path.display());
                println!();
                for (wp, affinity) in suggestions.iter().take(10) {
                    println!("  {:.2} - {}", affinity, wp.path.display());
                }
            }
        }
    }

    Ok(())
}

async fn cmd_collection(action: CollectionAction) -> Result<()> {
    match action {
        CollectionAction::List => {
            collections::cmd_collection_list()?;
        }
        CollectionAction::Show { name } => {
            collections::cmd_collection_show(&name)?;
        }
        CollectionAction::Save { name, description } => {
            // Get the most recent pairing from history
            let config = app::Config::load()?;
            let history = pairing::PairingHistory::load(config.pairing.max_history_records)?;

            // Find the most recent record with multiple screens
            let last_pairing = history.get_last_multi_screen_pairing();

            if let Some(wallpapers) = last_pairing {
                if wallpapers.is_empty() {
                    println!("No recent multi-screen pairing found.");
                    println!("Apply wallpapers to multiple screens first, then save.");
                    return Ok(());
                }

                let mut store = collections::CollectionStore::load()?;
                store.add(name.clone(), wallpapers.clone(), description)?;
                println!("✓ Saved collection '{}' with {} screen(s)", name, wallpapers.len());

                for (screen, path) in &wallpapers {
                    println!("  {}: {}", screen, path.display());
                }
            } else {
                println!("No pairing history found. Apply wallpapers to screens first.");
            }
        }
        CollectionAction::Apply { name } => {
            let store = collections::CollectionStore::load()?;

            if let Some(collection) = store.get(&name) {
                let config = app::Config::load()?;
                let transition = config.transition();

                for (screen_name, wp_path) in &collection.wallpapers {
                    if let Err(e) = swww::set_wallpaper_with_resize(
                        screen_name,
                        wp_path,
                        &transition,
                        config.display.resize_mode,
                        &config.display.fill_color,
                    ) {
                        eprintln!("Warning: Failed to set {} on {}: {}", wp_path.display(), screen_name, e);
                    } else {
                        println!("✓ {}: {}", screen_name, wp_path.display());
                    }
                }
                println!("Applied collection '{}'", name);
            } else {
                println!("Collection '{}' not found", name);
            }
        }
        CollectionAction::Delete { name } => {
            collections::cmd_collection_delete(&name)?;
        }
    }

    Ok(())
}

fn cmd_tag(action: TagAction, wallpaper_dir: &Path) -> Result<()> {
    let mut cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;

    match action {
        TagAction::List => {
            let tags = cache.all_tags();
            if tags.is_empty() {
                println!("No tags defined.");
                println!("Add tags with: frostwall tag add <path> <tag>");
            } else {
                println!("Tags:");
                for tag in tags {
                    let count = cache.with_tag(&tag).len();
                    println!("  {} ({})", tag, count);
                }
            }
        }
        TagAction::Add { path, tag } => {
            if cache.add_tag(&path, &tag) {
                cache.save()?;
                println!("✓ Added tag '{}' to {}", tag, path.display());
            } else {
                println!("Wallpaper not found: {}", path.display());
            }
        }
        TagAction::Remove { path, tag } => {
            if cache.remove_tag(&path, &tag) {
                cache.save()?;
                println!("✓ Removed tag '{}' from {}", tag, path.display());
            } else {
                println!("Wallpaper not found: {}", path.display());
            }
        }
        TagAction::Show { tag } => {
            let wallpapers = cache.with_tag(&tag);
            if wallpapers.is_empty() {
                println!("No wallpapers with tag '{}'", tag);
            } else {
                println!("Wallpapers with tag '{}':", tag);
                for wp in wallpapers {
                    println!("  {}", wp.path.display());
                }
            }
        }
    }

    Ok(())
}

fn cmd_similar(wallpaper_dir: &Path, target_path: &Path, limit: usize) -> Result<()> {
    let cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;

    // Find the target wallpaper
    let target = cache.wallpapers.iter()
        .find(|wp| wp.path == target_path)
        .or_else(|| {
            // Try matching by filename
            let target_name = target_path.file_name();
            cache.wallpapers.iter().find(|wp| wp.path.file_name() == target_name)
        });

    let target = match target {
        Some(t) => t,
        None => {
            println!("Wallpaper not found in cache: {}", target_path.display());
            println!("Run 'frostwall scan' first to index wallpapers.");
            return Ok(());
        }
    };

    if target.colors.is_empty() {
        println!("No color data for this wallpaper. Run 'frostwall scan' to extract colors.");
        return Ok(());
    }

    println!("Finding similar wallpapers to: {}", target.path.display());
    println!();

    // Build list of (index, colors) excluding target
    let wallpaper_colors: Vec<(usize, &[String])> = cache.wallpapers
        .iter()
        .enumerate()
        .filter(|(_, wp)| wp.path != target.path && !wp.colors.is_empty())
        .map(|(i, wp)| (i, wp.colors.as_slice()))
        .collect();

    let similar = utils::find_similar_wallpapers(&target.colors, &wallpaper_colors, limit);

    if similar.is_empty() {
        println!("No similar wallpapers found.");
    } else {
        println!("Similar wallpapers (by color profile):");
        for (score, idx) in similar {
            let wp = &cache.wallpapers[idx];
            let filename = wp.path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            println!("  {:.0}% - {}", score * 100.0, filename);
        }
    }

    Ok(())
}

#[cfg(feature = "clip")]
async fn cmd_auto_tag(
    wallpaper_dir: &Path,
    incremental: bool,
    threshold: f32,
    max_tags: usize,
    verbose: bool,
) -> Result<()> {
    use clip::ClipTagger;

    println!("Initializing CLIP model...");

    let mut tagger = ClipTagger::new().await?;

    let mut cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;

    let to_process: Vec<usize> = cache
        .wallpapers
        .iter()
        .enumerate()
        .filter(|(_, wp)| !incremental || wp.auto_tags.is_empty())
        .map(|(i, _)| i)
        .collect();

    if to_process.is_empty() {
        println!("All wallpapers already tagged.");
        return Ok(());
    }

    println!("Auto-tagging {} wallpapers...", to_process.len());

    for (progress, idx) in to_process.iter().enumerate() {
        let wp = &cache.wallpapers[*idx];
        let path = wp.path.clone();

        // Show verbose debug output only for first image
        let show_debug = verbose && progress == 0;
        if show_debug {
            eprintln!("\n=== Debug output for first image ===");
            eprintln!("Image: {}", path.display());
        }

        match tagger.tag_image_verbose(&path, threshold, show_debug) {
            Ok(mut tags) => {
                // Limit to max_tags (tags are already sorted by confidence)
                if max_tags > 0 && tags.len() > max_tags {
                    tags.truncate(max_tags);
                }

                if verbose {
                    let tag_names: Vec<_> = tags.iter().map(|t| &t.name).collect();
                    println!(
                        "[{}/{}] {}: {:?}",
                        progress + 1,
                        to_process.len(),
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        tag_names
                    );
                } else if (progress + 1) % 10 == 0 || progress + 1 == to_process.len() {
                    eprint!("\rProgress: {}/{}", progress + 1, to_process.len());
                }

                cache.wallpapers[*idx].set_auto_tags(tags);
            }
            Err(e) => {
                eprintln!(
                    "\nWarning: Failed to tag {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    if !verbose {
        eprintln!(); // Newline after progress
    }

    cache.save()?;

    // Show summary
    let tags = clip::ClipTagger::available_tags();
    println!("\nTag distribution:");
    for tag in tags {
        let count = cache.wallpapers.iter()
            .filter(|wp| wp.auto_tags.iter().any(|t| t.name == tag))
            .count();
        if count > 0 {
            println!("  {}: {}", tag, count);
        }
    }

    println!("\nDone! Tags saved to cache.");
    Ok(())
}

async fn cmd_time_profile(action: TimeProfileAction, wallpaper_dir: &Path) -> Result<()> {
    use timeprofile::TimePeriod;

    let mut config = app::Config::load()?;

    match action {
        TimeProfileAction::Status => {
            let period = TimePeriod::current();
            let settings = config.time_profiles.settings_for(period);

            println!("{} Current time period: {}", period.emoji(), period.name());
            println!();
            println!("Time profiles: {}", if config.time_profiles.enabled { "enabled" } else { "disabled" });
            println!();
            println!("Settings for {}:", period.name());
            println!("  Brightness range: {:.0}% - {:.0}%",
                settings.brightness_range.0 * 100.0,
                settings.brightness_range.1 * 100.0
            );
            println!("  Preferred tags: {}", settings.preferred_tags.join(", "));
            println!("  Brightness weight: {:.0}%", settings.brightness_weight * 100.0);
            println!("  Tag weight: {:.0}%", settings.tag_weight * 100.0);
        }
        TimeProfileAction::Enable => {
            config.time_profiles.enabled = true;
            config.save()?;
            println!("Time-based profiles enabled.");
            println!("Run 'frostwall time-profile status' to see current settings.");
        }
        TimeProfileAction::Disable => {
            config.time_profiles.enabled = false;
            config.save()?;
            println!("Time-based profiles disabled.");
        }
        TimeProfileAction::Preview { limit } => {
            let cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;
            let period = TimePeriod::current();

            println!("{} Previewing wallpapers for {} period:", period.emoji(), period.name());
            println!();

            // Score and sort wallpapers
            let mut scored: Vec<_> = cache.wallpapers.iter()
                .filter(|wp| !wp.colors.is_empty())
                .map(|wp| {
                    let score = config.time_profiles.score_wallpaper(&wp.colors, &wp.tags);
                    (wp, score)
                })
                .collect();

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (wp, score) in scored.into_iter().take(limit) {
                let filename = wp.path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?");
                let tags = if wp.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", wp.tags.join(", "))
                };
                println!("  {:.0}% - {}{}", score * 100.0, filename, tags);
            }
        }
        TimeProfileAction::Apply => {
            let cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;
            let screens = screen::detect_screens().await?;
            let transition = config.transition();
            let period = TimePeriod::current();

            println!("{} Setting wallpapers for {} period...", period.emoji(), period.name());

            // Get top wallpapers for current time
            let sorted = timeprofile::sort_by_time_profile(&cache.wallpapers, &config.time_profiles);

            for (i, screen) in screens.iter().enumerate() {
                if let Some(wp) = sorted.get(i) {
                    swww::set_wallpaper_with_resize(
                        &screen.name,
                        &wp.path,
                        &transition,
                        config.display.resize_mode,
                        &config.display.fill_color,
                    )?;
                    println!("  {}: {}", screen.name, wp.path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?"));
                }
            }
        }
    }

    Ok(())
}

fn cmd_import(action: ImportAction, wallpaper_dir: &Path) -> Result<()> {
    use webimport::{Gallery, WebImporter};

    let importer = WebImporter::new();

    match action {
        ImportAction::Unsplash { query, count } => {
            if !importer.is_available(Gallery::Unsplash) {
                println!("Unsplash requires an API key.");
                println!("1. Get a free key at: https://unsplash.com/developers");
                println!("2. Set: export UNSPLASH_ACCESS_KEY=your_key");
                return Ok(());
            }

            println!("Searching Unsplash for \"{}\"...", query);
            let results = importer.search(Gallery::Unsplash, &query, 1, count)?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nFound {} images:\n", results.len());
            for (i, img) in results.iter().enumerate() {
                let author = img.author.as_deref().unwrap_or("Unknown");
                println!("  {}. {}x{} by {} [{}]", i + 1, img.width, img.height, author, img.id);
            }

            println!("\nDownload with: frostwall import download <id>");
            println!("Or download all with: frostwall import download unsplash_<id>");
        }
        ImportAction::Wallhaven { query, count } => {
            println!("Searching Wallhaven for \"{}\"...", query);
            let results = importer.search(Gallery::Wallhaven, &query, 1, count)?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nFound {} images:\n", results.len());
            for (i, img) in results.iter().enumerate() {
                println!("  {}. {}x{} [{}]", i + 1, img.width, img.height, img.id);
            }

            println!("\nDownload with: frostwall import download <id>");
            println!("  e.g.: frostwall import download {}", results[0].id);
        }
        ImportAction::Featured { count } => {
            println!("Fetching top wallpapers from Wallhaven...");
            let results = importer.featured_wallhaven(count)?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nTop {} wallpapers:\n", results.len());
            for (i, img) in results.iter().enumerate() {
                println!("  {}. {}x{} [{}]", i + 1, img.width, img.height, img.id);
            }

            println!("\nDownload with: frostwall import download <id>");
        }
        ImportAction::Download { url } => {
            // Determine source from URL/ID
            let image = if url.starts_with("http") {
                // Full URL - try to determine source
                if url.contains("unsplash.com") {
                    println!("Direct Unsplash URLs require the search command first.");
                    return Ok(());
                } else if url.contains("wallhaven.cc") || url.contains("w.wallhaven") {
                    // Extract ID from Wallhaven URL
                    let id = url.rsplit('/').next().unwrap_or(&url);
                    let id = id.split('.').next().unwrap_or(id);
                    webimport::GalleryImage {
                        id: id.to_string(),
                        url: format!("https://w.wallhaven.cc/full/{}/wallhaven-{}.jpg",
                            &id[..2.min(id.len())], id),
                        thumb_url: String::new(),
                        width: 0,
                        height: 0,
                        author: None,
                        source: Gallery::Wallhaven,
                    }
                } else {
                    println!("Unknown URL source. Supported: Unsplash, Wallhaven");
                    return Ok(());
                }
            } else {
                // Assume Wallhaven ID
                let full_url = format!(
                    "https://w.wallhaven.cc/full/{}/wallhaven-{}.jpg",
                    &url[..2.min(url.len())],
                    url
                );
                webimport::GalleryImage {
                    id: url.clone(),
                    url: full_url,
                    thumb_url: String::new(),
                    width: 0,
                    height: 0,
                    author: None,
                    source: Gallery::Wallhaven,
                }
            };

            println!("Downloading {}...", image.id);

            match importer.download(&image, wallpaper_dir) {
                Ok(path) => {
                    println!("Downloaded to: {}", path.display());
                    println!("\nRun 'frostwall scan' to add it to the cache.");
                }
                Err(e) => {
                    // Try alternative URL formats for Wallhaven
                    if image.source == Gallery::Wallhaven {
                        // Try PNG format
                        let png_url = image.url.replace(".jpg", ".png");
                        let png_image = webimport::GalleryImage {
                            url: png_url,
                            ..image.clone()
                        };
                        if let Ok(path) = importer.download(&png_image, wallpaper_dir) {
                            println!("Downloaded to: {}", path.display());
                            println!("\nRun 'frostwall scan' to add it to the cache.");
                            return Ok(());
                        }
                    }
                    println!("Download failed: {}", e);
                    println!("The image might not exist or the URL format has changed.");
                }
            }
        }
    }

    Ok(())
}
