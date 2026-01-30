mod app;
mod clip;
mod init;
mod pairing;
mod profile;
mod pywal;
mod screen;
mod swww;
mod thumbnail;
mod ui;
mod utils;
mod wallpaper;
mod watch;

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

        /// Confidence threshold (0.0-1.0)
        #[arg(short, long, default_value = "0.25")]
        threshold: f32,

        /// Show detailed progress
        #[arg(short, long)]
        verbose: bool,
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
        Some(Commands::AutoTag { incremental, threshold, verbose }) => {
            cmd_auto_tag(&wallpaper_dir, incremental, threshold, verbose).await?;
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

#[cfg(feature = "clip")]
async fn cmd_auto_tag(
    wallpaper_dir: &Path,
    incremental: bool,
    threshold: f32,
    verbose: bool,
) -> Result<()> {
    use clip::ClipTagger;

    println!("Initializing CLIP model...");

    let tagger = ClipTagger::new().await?;

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

        match tagger.tag_image(&path, threshold) {
            Ok(tags) => {
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
