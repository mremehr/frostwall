# FrostWall

**Intelligent wallpaper manager with screen-aware matching for Wayland**

FrostWall automatically detects your screen configurations and intelligently matches wallpapers based on aspect ratio, orientation, and display characteristics. Built as a Rust TUI with image preview support and CLI commands for scripting.

## Vision

Managing wallpapers across multiple monitors with different aspect ratios (ultrawide, portrait, landscape) is tedious. FrostWall transforms this into a seamless, visual experience:

- **Smart matching**: Automatically filters wallpapers that fit each screen's aspect category
- **Multi-monitor aware**: Detects all connected outputs via niri/wlr-randr
- **Visual pairing**: Split-view interface lets you see your selected wallpaper alongside suggested matches for other screens - pick the perfect combination with real thumbnail previews
- **Color harmony**: LAB color space matching ensures your multi-monitor setup looks cohesive
- **Visual browsing**: TUI with real image thumbnails (Kitty/Sixel graphics protocols)
- **Scriptable**: CLI commands for keybindings, scripts, and automation

## Features

### Screen-Aware Matching

Wallpapers are categorized by aspect ratio:
- **Ultrawide** (21:9, 32:9) - for super-wide monitors
- **Landscape** (16:9, 16:10) - standard horizontal monitors
- **Portrait** (9:16) - rotated vertical monitors
- **Square** (~1:1) - versatile for any orientation

Match modes control filtering:
| Mode | Behavior |
|------|----------|
| **Strict** | Only exact aspect category matches |
| **Flexible** | Compatible ratios (landscape works on ultrawide, etc.) |
| **All** | Show every wallpaper regardless of aspect |

### Visual Pairing Preview

The killer feature: press `p` to enter pairing mode and see a split-view with:

```
┌────────────────────────────────┬─────────────────┐
│                                │  Pair 1/3       │
│     [Your selected wallpaper]  │  ┌───────────┐  │
│          65% width             │  │ DP-1 thumb│  │
│                                │  └───────────┘  │
│                                │  ┌───────────┐  │
│                                │  │ DP-2 thumb│  │
│                                │  └───────────┘  │
└────────────────────────────────┴─────────────────┘
```

- **Real thumbnails** - See actual images, not just filenames
- **Multiple alternatives** - Cycle through top 3 matches with `←`/`→`
- **Color-based suggestions** - Matches based on LAB color similarity
- **History learning** - Remembers which wallpapers you pair together
- **One-press apply** - `Enter` sets all screens at once

### Intelligent Pairing System

Multi-monitor wallpaper pairing that learns from your choices:

- **Affinity tracking** - Records which wallpapers you use together
- **LAB color matching** - Suggests wallpapers with perceptually similar colors
- **Score-based ranking** - Combines history + color similarity for best matches
- **Position memory** - TUI remembers your browsing position per screen

### Auto-Tagging

Automatic content detection based on color analysis:

```bash
frostwall color-tag              # Auto-tag all wallpapers
frostwall color-tag --incremental # Only tag new wallpapers
```

12 built-in tag categories:
`nature`, `ocean`, `forest`, `sunset`, `dark`, `bright`, `cyberpunk`, `minimal`, `mountain`, `space`, `autumn`, `pastel`

Tags are assigned based on:
- Brightness range matching
- Saturation range matching
- Color palette similarity (LAB/Delta-E)

### Time-Based Profiles

Automatic wallpaper selection based on time of day:

```bash
frostwall time-profile status    # Show current period & settings
frostwall time-profile enable    # Enable time-based selection
frostwall time-profile preview   # Preview matching wallpapers
frostwall time-profile apply     # Set wallpaper for current time
```

Time periods:
| Period | Hours | Default preferences |
|--------|-------|---------------------|
| Morning | 6-12 | Bright, nature, pastel |
| Afternoon | 12-18 | Nature, ocean, mountain |
| Evening | 18-22 | Sunset, autumn, cyberpunk |
| Night | 22-6 | Dark, space, minimal |

### Web Gallery Import

Download wallpapers from popular galleries:

```bash
# Wallhaven (no API key required)
frostwall import wallhaven "nature 4k"
frostwall import featured --count 20
frostwall import download <wallhaven-id>

# Unsplash (requires API key)
export UNSPLASH_ACCESS_KEY=your_key
frostwall import unsplash "mountains"
```

### Collections (Presets)

Save and recall multi-screen wallpaper combinations:

```bash
frostwall collection save "work-setup"        # Save current wallpapers
frostwall collection save "gaming" -d "RGB!"  # With description
frostwall collection list                      # List all collections
frostwall collection show "work-setup"         # Show details
frostwall collection apply "work-setup"        # Restore collection
frostwall collection delete "work-setup"       # Delete collection
```

### Image Similarity Search

Find wallpapers with similar color profiles:

```bash
frostwall similar ~/Pictures/wallpapers/favorite.jpg --limit 10
```

Uses LAB color space for perceptually accurate matching.

### TUI Mode

Interactive terminal interface with:
- Real image thumbnails via ratatui-image (Kitty/Sixel protocols)
- Carousel navigation with selection highlighting
- Live screen switching (Tab/Shift+Tab)
- **Visual pairing preview** (`p` key) with split-view thumbnails
- Instant wallpaper application with animated transitions
- Auto-detects terminal theme (Frostglow Light / Deep Cracked Ice Dark)
- **Vim-style command mode** (`:` key)

### Command Mode

Press `:` in TUI for vim-style commands:

| Command | Description |
|---------|-------------|
| `:t <tag>` | Filter by tag (fuzzy match) |
| `:tag` | List all available tags |
| `:clear` / `:c` | Clear all filters |
| `:random` / `:r` | Random wallpaper |
| `:apply` / `:a` | Apply current wallpaper |
| `:similar` / `:sim` | Find similar wallpapers |
| `:sort name/date/size` | Change sort mode |
| `:screen <n>` | Switch to screen n |
| `:go <n>` | Go to wallpaper n |
| `:help` / `:h` | Show help |
| `:q` / `:quit` | Quit |

### CLI Commands

```bash
frostwall              # Launch TUI
frostwall random       # Set random matching wallpaper per screen
frostwall next         # Cycle to next wallpaper
frostwall prev         # Cycle to previous wallpaper
frostwall screens      # List detected screens
frostwall scan         # Rescan wallpaper directory
frostwall init         # Interactive setup wizard
frostwall watch        # Background daemon for auto-rotation

# Tag management
frostwall tag list
frostwall tag add ~/wallpapers/forest.jpg nature
frostwall tag show nature
frostwall color-tag                    # Auto-tag by colors

# Pairing management
frostwall pair stats
frostwall pair suggest ~/wallpapers/forest.jpg

# Collections
frostwall collection save "my-preset"
frostwall collection apply "my-preset"

# Time profiles
frostwall time-profile status
frostwall time-profile apply

# Web import
frostwall import wallhaven "nature"
frostwall import featured

# Similarity search
frostwall similar ~/wallpapers/forest.jpg

# Profile management
frostwall profile list
frostwall profile create work
frostwall profile use work

# pywal color export
frostwall pywal ~/wallpapers/forest.jpg --apply
```

### Watch Daemon

Auto-rotate wallpapers in the background:

```bash
frostwall watch --interval 30m          # Every 30 minutes
frostwall watch --interval 1h --shuffle # Hourly, random order
frostwall watch --watch-dir false       # Disable file monitoring
```

Features:
- Configurable interval (30s, 5m, 1h, etc.)
- File system monitoring (inotify) - auto-updates cache when files change
- Shuffle or sequential mode
- **Time-profile aware** - respects time-based preferences when enabled

### Resize Modes

Control how wallpapers fit the screen:
- **Crop** (default) - Fill screen, crop excess
- **Fit** - Fit inside screen with letterboxing
- **Center** - No resize, center image
- **Stretch** - Fill screen (distorts aspect)

### Additional Features

- **Dominant color extraction** - k-means clustering extracts 5 primary colors per wallpaper
- **LAB color space** - Perceptually accurate color matching (Delta-E/CIE76)
- **2-phase scanning** - Fast header scan, then parallel color extraction
- **Thumbnail caching** - SIMD-accelerated (fast_image_resize) with disk cache
- **Transition effects** - Fade, wipe, grow, center, outer via swww
- **TOML configuration** - Customize paths, keybindings, transitions

## Requirements

- **Wayland compositor**: niri, Sway, Hyprland, or any wlr-based compositor
- **swww**: Wallpaper daemon (`swww` and `swww-daemon`)
- **Screen detection**: niri (preferred) or wlr-randr
- **Terminal with graphics**: Kitty, WezTerm, or Sixel-capable terminal for image previews

## Installation

```bash
cd FrostWall
cargo build --release
```

Binary: `target/release/frostwall`

## Configuration

Config file: `~/.config/frostwall/config.toml`

```toml
[wallpaper]
directory = "~/Pictures/wallpapers"
extensions = ["jpg", "jpeg", "png", "webp", "bmp", "gif"]
recursive = false

[display]
match_mode = "Flexible"    # Strict, Flexible, All
resize_mode = "Fit"        # Crop, Fit, No, Stretch

[display.fill_color]       # Padding color (RGBA)
r = 0
g = 0
b = 0
a = 255

[transition]
transition_type = "fade"   # fade, wipe, grow, center, outer, none
duration = 1.0
fps = 60

[thumbnails]
width = 800
height = 600
quality = 92
grid_columns = 3

[theme]
mode = "auto"              # auto, light, dark

[pairing]
enabled = true             # Enable intelligent pairing
max_history_records = 1000 # Maximum pairing records to keep

[time_profiles]
enabled = false            # Enable time-based wallpaper selection

[time_profiles.morning]
brightness_range = [0.5, 0.9]
preferred_tags = ["nature", "bright", "pastel"]

[time_profiles.afternoon]
brightness_range = [0.4, 0.8]
preferred_tags = ["nature", "ocean", "mountain"]

[time_profiles.evening]
brightness_range = [0.2, 0.6]
preferred_tags = ["sunset", "autumn", "cyberpunk"]

[time_profiles.night]
brightness_range = [0.0, 0.4]
preferred_tags = ["dark", "space", "minimal"]
```

## Keybindings (TUI)

| Key | Action |
|-----|--------|
| `h` / `←` | Previous wallpaper |
| `l` / `→` | Next wallpaper |
| `Enter` | Apply selected wallpaper |
| `p` | **Pairing preview** - split-view with suggestions |
| `r` | Random wallpaper (apply immediately) |
| `:` | **Command mode** (vim-style) |
| `m` | Toggle match mode (Strict/Flexible/All) |
| `f` | Toggle resize mode (Crop/Fit/Center/Stretch) |
| `s` | Toggle sort mode (Name/Size/Date) |
| `c` | Show/hide color palette |
| `C` | Open color filter picker |
| `t` | Cycle tag filter |
| `T` | Clear tag filter |
| `w` | Export pywal colors |
| `W` | Toggle auto pywal export |
| `Tab` | Next screen (remembers position) |
| `Shift+Tab` | Previous screen (remembers position) |
| `?` | Show help popup |
| `q` / `Esc` | Quit |

### Pairing Preview Mode (`p`)

| Key | Action |
|-----|--------|
| `←` / `→` | Cycle through alternatives (1/3, 2/3, 3/3) |
| `1` / `2` / `3` | Jump to specific alternative |
| `Enter` | Apply all wallpapers (selected + suggestions) |
| `p` / `Esc` | Close pairing preview |

## Architecture

```
src/
  main.rs        # CLI entry point (clap)
  app.rs         # TUI state & event loop
  screen.rs      # Screen detection (niri/wlr-randr)
  wallpaper.rs   # Wallpaper scanning, caching, matching logic
  swww.rs        # swww daemon interface
  thumbnail.rs   # SIMD thumbnail generation & disk cache
  pywal.rs       # pywal color export
  profile.rs     # Profile management
  pairing.rs     # Intelligent wallpaper pairing & history
  collections.rs # Wallpaper collections/presets
  timeprofile.rs # Time-based wallpaper profiles
  webimport.rs   # Web gallery import (Unsplash/Wallhaven)
  utils.rs       # Color utilities, LAB matching, auto-tagging
  watch.rs       # Watch daemon with inotify
  init.rs        # Interactive setup wizard
  clip.rs        # CLIP auto-tagging (optional feature)
  ui/
    mod.rs       # UI module exports
    theme.rs     # Frost theme (light/dark auto-detection)
    layout.rs    # TUI layout & rendering
```

### Data Flow

1. **Startup**: Detect screens via `niri msg outputs` or `wlr-randr`
2. **Scan**: Load wallpaper metadata (dimensions, colors, auto-tags) into cache
3. **Filter**: Match wallpapers to selected screen's aspect category
4. **Pair**: Calculate pairing suggestions based on history + color similarity
5. **Preview**: Split-view shows selected wallpaper + thumbnail suggestions
6. **Apply**: Call `swww img` with transition parameters for all screens

### Cache Locations

- **Config**: `~/.config/frostwall/config.toml`
- **Wallpaper metadata**: `~/.cache/frostwall/wallpaper_cache.json`
- **Thumbnails**: `~/.cache/frostwall/thumbs_v2/`
- **Pairing history**: `~/.cache/frostwall/pairing_history.json`
- **Collections**: `~/.local/share/frostwall/collections.json`

## Theme Integration

FrostWall automatically detects terminal theme by checking:
1. `~/.config/alacritty/.current-theme` marker file
2. Kitty/Alacritty config headers for "frostglow" or "light"/"dark" keywords
3. `ALACRITTY_THEME` environment variable

Two built-in themes:
- **Frostglow Light** - For light terminal backgrounds
- **Deep Cracked Ice** - For dark terminal backgrounds

Both use transparent backgrounds (`Color::Reset`) to inherit terminal colors.

## Integration Examples

### Keybinding (niri)

```kdl
binds {
    Mod+W { spawn "frostwall" "random"; }
    Mod+Shift+W { spawn "frostwall"; }
}
```

### Startup Script

```bash
#!/bin/bash
swww-daemon &
sleep 0.5
frostwall random
```

### Cron Job for Time-Based Rotation

```bash
# Change wallpaper based on time of day every hour
0 * * * * frostwall time-profile apply
```

## Color Data Format

Each wallpaper stores metadata including colors and auto-tags:

```json
{
  "path": "/home/user/wallpapers/forest.jpg",
  "width": 3840,
  "height": 2160,
  "aspect_category": "Landscape",
  "colors": ["#1a2b3c", "#4d5e6f", "#7a8b9c", "#adbccd", "#d0e1f2"],
  "tags": ["nature", "forest"],
  "auto_tags": [
    {"name": "forest", "confidence": 0.85},
    {"name": "nature", "confidence": 0.72},
    {"name": "dark", "confidence": 0.45}
  ]
}
```

## Changelog

### v0.5.0

- **Visual pairing preview** - Split-view with real thumbnails for multi-monitor pairing
- **Manual pairing control** - Press `p` to preview and select matching wallpapers
- **Removed auto-apply** - Pairing is now intentional, not automatic
- **Cleaner UX** - 65/35 split layout shows your selection alongside suggestions

### v0.4.0

- **Command mode** - Vim-style `:` commands in TUI
- **Auto-tagging** - Color-based automatic tag assignment
- **Time-based profiles** - Wallpapers based on time of day
- **Web gallery import** - Download from Unsplash/Wallhaven
- **Collections** - Save/restore multi-screen presets
- **Image similarity** - Find wallpapers with similar colors
- **LAB color matching** - Perceptually accurate color comparison
- **2-phase scanning** - Faster startup with parallel processing

## License

GPL-2.0

## Author

MrMattias & Claude
