# FrostWall

**Intelligent wallpaper manager with screen-aware matching for Wayland**

FrostWall automatically detects your screen configurations and intelligently matches wallpapers based on aspect ratio, orientation, and display characteristics. Built as a Rust TUI with image preview support and CLI commands for scripting.

## Vision

Managing wallpapers across multiple monitors with different aspect ratios (ultrawide, portrait, landscape) is tedious. FrostWall solves this by:

- **Smart matching**: Automatically filters wallpapers that fit each screen's aspect category
- **Multi-monitor aware**: Detects all connected outputs via niri/wlr-randr
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

### Resize Modes

Control how wallpapers fit the screen:
- **Crop** (default) - Fill screen, crop excess
- **Fit** - Fit inside screen with letterboxing
- **Center** - No resize, center image
- **Stretch** - Fill screen (distorts aspect)

### TUI Mode

Interactive terminal interface with:
- Real image thumbnails via ratatui-image (Kitty/Sixel protocols)
- Carousel navigation with selection highlighting
- Live screen switching (Tab/Shift+Tab)
- Instant wallpaper application with animated transitions
- Auto-detects terminal theme (Frostglow Light / Deep Cracked Ice Dark)

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

# Profile management
frostwall profile list
frostwall profile create work
frostwall profile use work
frostwall profile set work directory ~/Pictures/work
frostwall profile delete work

# Tag management
frostwall tag list
frostwall tag add ~/Pictures/wallpapers/forest.jpg nature
frostwall tag remove ~/Pictures/wallpapers/forest.jpg nature
frostwall tag show nature

# pywal color export
frostwall pywal ~/Pictures/wallpapers/forest.jpg         # Export colors
frostwall pywal ~/Pictures/wallpapers/forest.jpg --apply # Export + apply
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

### Additional Features

- **Dominant color extraction** - k-means clustering extracts 5 primary colors per wallpaper
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
```

## Keybindings (TUI)

| Key | Action |
|-----|--------|
| `h` / `←` | Previous wallpaper |
| `l` / `→` | Next wallpaper |
| `Enter` | Apply selected wallpaper |
| `r` | Random wallpaper (apply immediately) |
| `m` | Toggle match mode (Strict/Flexible/All) |
| `f` | Toggle resize mode (Crop/Fit/Center/Stretch) |
| `s` | Toggle sort mode (Name/Size/Date) |
| `c` | Show/hide color palette |
| `C` | Open color filter picker |
| `t` | Cycle tag filter |
| `T` | Clear tag filter |
| `w` | Export pywal colors |
| `W` | Toggle auto pywal export |
| `Tab` | Next screen |
| `Shift+Tab` | Previous screen |
| `?` | Show help popup |
| `q` / `Esc` | Quit |

## Architecture

```
src/
  main.rs       # CLI entry point (clap)
  app.rs        # TUI state & event loop
  screen.rs     # Screen detection (niri/wlr-randr)
  wallpaper.rs  # Wallpaper scanning, caching, matching logic
  swww.rs       # swww daemon interface
  thumbnail.rs  # SIMD thumbnail generation & disk cache
  pywal.rs      # pywal color export
  profile.rs    # Profile management
  watch.rs      # Watch daemon with inotify
  init.rs       # Interactive setup wizard
  utils.rs      # Shared utilities
  ui/
    mod.rs      # UI module exports
    theme.rs    # Frost theme (light/dark auto-detection)
    layout.rs   # TUI layout & rendering
```

### Data Flow

1. **Startup**: Detect screens via `niri msg outputs` or `wlr-randr`
2. **Scan**: Load wallpaper metadata (dimensions, colors) into cache
3. **Filter**: Match wallpapers to selected screen's aspect category
4. **Display**: Render thumbnails using background thread + SIMD resize
5. **Apply**: Call `swww img` with transition parameters

### Cache Locations

- **Config**: `~/.config/frostwall/config.toml`
- **Wallpaper metadata**: `~/.cache/frostwall/wallpaper_cache.json`
- **Thumbnails**: `~/.cache/frostwall/thumbs_v2/`

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

## Roadmap

### ✓ Implemented (v0.4.0)

- **Interactive Setup** (`frostwall init`) - Guided wizard for new users
- **Profile System** - Multiple configs for work/gaming/etc contexts
- **Watch Daemon** (`frostwall watch`) - Auto-rotation with file monitoring
- **Live Cache Updates** - inotify-based hot reload when files change
- **Tagging System** - CLI and TUI tag management, filtering by tag
- **Color Palette Display** - View dominant colors in TUI (`c` key)
- **Sorting Modes** - Sort by name, size, or date
- **Help Popup** - Full keybinding reference (`?` key)
- **pywal Integration** - Export colors to ~/.cache/wal/ (`w` key or `frostwall pywal`)
- **Color Filtering** - Filter wallpapers by color (`C` to open picker)
- **Parallel Scanning** - Multi-core wallpaper scanning with rayon
- **Recursive Scanning** - Scan subdirectories with `recursive = true`
- **Configurable Keybindings** - Customize keyboard shortcuts in config

### Planned Features

#### Auto-tagging
Automatic content detection:
- ML-based image classification (ONNX runtime)
- Auto-suggest tags on scan

### Color Data Format

Each wallpaper stores 5 dominant colors (k-means extracted):

```json
{
  "path": "/home/user/wallpapers/forest.jpg",
  "width": 3840,
  "height": 2160,
  "aspect_category": "Landscape",
  "colors": ["#1a2b3c", "#4d5e6f", "#7a8b9c", "#adbccd", "#d0e1f2"],
  "tags": ["nature", "forest", "dark"]
}
```

## License

GPL-2.0

## Author

MrMattias
