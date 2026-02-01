use crate::app::App;
use crate::ui::theme::{frost_theme, FrostTheme};
use crate::utils::ColorHarmony;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use ratatui_image::StatefulImage;

const THUMBNAIL_WIDTH: u16 = 48;
const THUMBNAIL_HEIGHT: u16 = 28;

pub fn draw(f: &mut Frame, app: &mut App) {
    let theme = frost_theme();
    let area = f.area();

    // Check if a popup is showing (need to skip image rendering)
    // ratatui-image renders directly to terminal, bypassing widget z-order
    // Note: show_pairing_preview renders thumbnails separately, so don't block carousel
    let popup_active = app.show_help || app.show_color_picker || app.pairing_history.can_undo() || app.command_mode;

    // Main container with frost border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Vertical layout: header, carousel, (optional error), (optional colors), footer
    let has_error = app.last_error.is_some();
    let constraints = if app.show_colors {
        if has_error {
            vec![
                Constraint::Length(2),  // Header
                Constraint::Length(1),  // Error
                Constraint::Min(7),     // Carousel
                Constraint::Length(3),  // Color palette
                Constraint::Length(2),  // Footer
            ]
        } else {
            vec![
                Constraint::Length(2),  // Header
                Constraint::Min(8),     // Carousel
                Constraint::Length(3),  // Color palette
                Constraint::Length(2),  // Footer
            ]
        }
    } else if has_error {
        vec![
            Constraint::Length(2),  // Header
            Constraint::Length(1),  // Error
            Constraint::Min(9),     // Carousel
            Constraint::Length(2),  // Footer
        ]
    } else {
        vec![
            Constraint::Length(2),  // Header
            Constraint::Min(10),    // Carousel
            Constraint::Length(2),  // Footer
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let mut chunk_idx = 0;

    draw_header(f, app, chunks[chunk_idx], &theme);
    chunk_idx += 1;

    if has_error {
        draw_error(f, app, chunks[chunk_idx], &theme);
        chunk_idx += 1;
    }

    // Only draw carousel with images if no popup is active
    // (ratatui-image renders directly to terminal, bypassing widget z-order)
    if popup_active {
        draw_carousel_placeholder(f, chunks[chunk_idx], &theme);
    } else if app.show_pairing_preview && !app.pairing_preview_matches.is_empty() {
        // Split layout: 2/3 carousel, 1/3 pairing preview
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(65),  // Carousel with selected wallpaper
                Constraint::Percentage(35),  // Pairing preview
            ])
            .split(chunks[chunk_idx]);

        draw_carousel_single(f, app, split[0], &theme);
        draw_pairing_panel(f, app, split[1], &theme);
    } else {
        draw_carousel(f, app, chunks[chunk_idx], &theme);
    }
    chunk_idx += 1;

    if app.show_colors {
        draw_color_palette(f, app, chunks[chunk_idx], &theme);
        chunk_idx += 1;
    }

    draw_footer(f, app, chunks[chunk_idx], &theme);

    // Draw popups on top
    if app.show_color_picker {
        draw_color_picker(f, app, area, &theme);
    } else if app.show_help {
        draw_help_popup(f, area, &theme);
    }

    // Draw undo popup (always on top if active)
    if app.pairing_history.can_undo() {
        draw_undo_popup(f, app, area, &theme);
    }
}

fn draw_error(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    if let Some(error) = &app.last_error {
        let error_line = Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(theme.warning)),
            Span::styled(error, Style::default().fg(theme.warning)),
        ]);
        let paragraph = Paragraph::new(error_line).alignment(Alignment::Center);
        f.render_widget(paragraph, area);
    }
}

fn draw_carousel_placeholder(f: &mut Frame, area: Rect, theme: &FrostTheme) {
    // Simple placeholder when popup is active (images would render over popup)
    let text = Paragraph::new("(popup active)")
        .style(Style::default().fg(theme.fg_muted))
        .alignment(Alignment::Center);
    let centered = center_vertically(area, 1);
    f.render_widget(text, centered);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let screen_info = if let Some(screen) = app.selected_screen() {
        format!(
            "{} · {}x{} · {:?}",
            screen.name, screen.width, screen.height, screen.aspect_category
        )
    } else {
        "No screens".to_string()
    };

    let count_info = format!(
        "{}/{}",
        app.selected_wallpaper_idx + 1,
        app.filtered_wallpapers.len()
    );

    // Show current modes
    let match_mode = app.config.display.match_mode.display_name();
    let resize_mode = app.config.display.resize_mode.display_name();
    let sort_mode = app.sort_mode.display_name();

    let mut header_spans = vec![
        Span::styled(
            " FrostWall ",
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    header_spans.extend(vec![
        Span::styled("│ ", Style::default().fg(theme.fg_muted)),
        Span::styled(screen_info, Style::default().fg(theme.fg_secondary)),
        Span::styled(" │ ", Style::default().fg(theme.fg_muted)),
        Span::styled(count_info, Style::default().fg(theme.accent_primary)),
        Span::styled(" │ ", Style::default().fg(theme.fg_muted)),
        Span::styled(format!("[{}]", match_mode), Style::default().fg(theme.accent_primary)),
        Span::styled(" ", Style::default()),
        Span::styled(format!("[{}]", resize_mode), Style::default().fg(theme.accent_secondary)),
        Span::styled(" ", Style::default()),
        Span::styled(format!("[⇅{}]", sort_mode), Style::default().fg(theme.fg_secondary)),
    ]);

    // Tag filter indicator
    if let Some(tag) = &app.active_tag_filter {
        header_spans.push(Span::styled(" ", Style::default()));
        header_spans.push(Span::styled(
            format!("[#{}]", tag),
            Style::default().fg(theme.accent_highlight),
        ));
    }

    // Color filter indicator
    if let Some(color) = &app.active_color_filter {
        header_spans.push(Span::styled(" ", Style::default()));
        if let Some(c) = parse_hex_color(color) {
            header_spans.push(Span::styled("█", Style::default().fg(c)));
        }
        header_spans.push(Span::styled(
            format!("[{}]", color),
            Style::default().fg(theme.fg_secondary),
        ));
    }

    // Pywal indicator
    if app.pywal_export {
        header_spans.push(Span::styled(" ", Style::default()));
        header_spans.push(Span::styled(
            "[wal]",
            Style::default().fg(theme.success),
        ));
    }

    // Pairing suggestions indicator
    if !app.pairing_suggestions.is_empty() {
        header_spans.push(Span::styled(" ", Style::default()));
        header_spans.push(Span::styled(
            format!("[⚡{}]", app.pairing_suggestions.len()),
            Style::default().fg(theme.success),
        ));
    }

    let header = Line::from(header_spans);

    let paragraph = Paragraph::new(header).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

/// Draw single selected wallpaper (for pairing split view)
fn draw_carousel_single(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    if app.filtered_wallpapers.is_empty() {
        let empty = Paragraph::new("No matching wallpapers")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        let centered = center_vertically(area, 1);
        f.render_widget(empty, centered);
        return;
    }

    let cache_idx = app.filtered_wallpapers[app.selected_wallpaper_idx];

    // Get wallpaper info
    let filename = app.cache.wallpapers
        .get(cache_idx)
        .map(|wp| wp.path.file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string())
        .unwrap_or("?".to_string());

    // Request thumbnail
    app.request_thumbnail(cache_idx);

    // Calculate centered thumbnail area (larger than normal)
    let thumb_w = (area.width - 4).min(THUMBNAIL_WIDTH + 20);
    let thumb_h = (area.height - 3).min(THUMBNAIL_HEIGHT + 10);
    let thumb_x = area.x + (area.width.saturating_sub(thumb_w)) / 2;
    let thumb_y = area.y + (area.height.saturating_sub(thumb_h + 2)) / 2;
    let thumb_area = Rect::new(thumb_x, thumb_y, thumb_w, thumb_h);

    // Draw frame
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_highlight))
        .style(Style::default().bg(theme.bg_medium));

    let inner = block.inner(thumb_area);
    f.render_widget(block, thumb_area);

    // Render image
    if let Some(protocol) = app.get_thumbnail(cache_idx) {
        let image = StatefulImage::new(None);
        f.render_stateful_widget(image, inner, protocol);
    } else {
        // Fallback: show filename
        let label = Paragraph::new(filename)
            .style(Style::default().fg(theme.fg_secondary))
            .alignment(Alignment::Center);
        f.render_widget(label, center_vertically(inner, 1));
    }

    // Selection indicator below
    if thumb_area.bottom() < area.y + area.height {
        let indicator_area = Rect::new(thumb_x, thumb_area.bottom(), thumb_w, 1);
        let indicator = Paragraph::new("▲ Selected")
            .style(Style::default().fg(theme.accent_highlight))
            .alignment(Alignment::Center);
        f.render_widget(indicator, indicator_area);
    }
}

/// Draw pairing preview panel (right side in split view)
fn draw_pairing_panel(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    let alternatives = app.pairing_preview_alternatives();
    let preview_idx = app.pairing_preview_idx;

    // Panel border
    let title = format!(" Pair {}/{} ", preview_idx + 1, alternatives);
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(theme.accent_highlight).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.success))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.pairing_preview_matches.is_empty() {
        let text = Paragraph::new("No suggestions")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        f.render_widget(text, center_vertically(inner, 1));
        return;
    }

    // Collect preview data: (screen_name, cache_idx, filename, harmony)
    let preview_data: Vec<(String, Option<usize>, String, ColorHarmony)> = app.pairing_preview_matches
        .iter()
        .map(|(screen_name, matches)| {
            let idx = preview_idx.min(matches.len().saturating_sub(1));
            if let Some((path, _, harmony)) = matches.get(idx) {
                let cache_idx = app.cache.wallpapers.iter()
                    .position(|wp| &wp.path == path);
                let filename = path.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                (screen_name.clone(), cache_idx, filename, *harmony)
            } else {
                (screen_name.clone(), None, "?".to_string(), ColorHarmony::None)
            }
        })
        .collect();

    // Request all thumbnails
    for (_, cache_idx, _, _) in &preview_data {
        if let Some(ci) = cache_idx {
            app.request_thumbnail(*ci);
        }
    }

    // Calculate layout - vertical stack of thumbnails
    let num_items = preview_data.len();
    let available_height = inner.height.saturating_sub(1);
    let item_height = (available_height / num_items as u16).min(18).max(8);
    let thumb_h = item_height.saturating_sub(2);
    let thumb_w = (inner.width - 2).min(thumb_h * 2); // Maintain rough aspect ratio

    let mut y_offset = inner.y;

    for (screen_name, cache_idx, filename, harmony) in preview_data {
        if y_offset + item_height > inner.y + inner.height {
            break;
        }

        // Screen name header with harmony indicator
        let harmony_icon = match harmony {
            ColorHarmony::Analogous => "~",        // Similar
            ColorHarmony::Complementary => "◐",    // Opposite
            ColorHarmony::Triadic => "△",          // Triangle
            ColorHarmony::SplitComplementary => "⋈", // Split
            ColorHarmony::None => "",
        };
        let screen_short: String = screen_name.chars().take(inner.width as usize - 4).collect();
        let header_text = if harmony_icon.is_empty() {
            screen_short
        } else {
            format!("{} {}", harmony_icon, screen_short)
        };
        let header = Paragraph::new(header_text)
            .style(Style::default().fg(theme.accent_secondary).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        f.render_widget(header, Rect::new(inner.x, y_offset, inner.width, 1));
        y_offset += 1;

        // Thumbnail area (centered horizontally)
        let thumb_x = inner.x + (inner.width.saturating_sub(thumb_w)) / 2;
        let thumb_area = Rect::new(thumb_x, y_offset, thumb_w, thumb_h);

        let thumb_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.bg_medium));
        let thumb_inner = thumb_block.inner(thumb_area);
        f.render_widget(thumb_block, thumb_area);

        // Render thumbnail
        if let Some(ci) = cache_idx {
            if let Some(protocol) = app.get_thumbnail(ci) {
                let image = StatefulImage::new(None);
                f.render_stateful_widget(image, thumb_inner, protocol);
            } else {
                // Fallback: filename
                let name_short: String = filename.chars().take(thumb_inner.width as usize).collect();
                let label = Paragraph::new(name_short)
                    .style(Style::default().fg(theme.fg_secondary))
                    .alignment(Alignment::Center);
                f.render_widget(label, center_vertically(thumb_inner, 1));
            }
        }

        y_offset += thumb_h + 1;
    }
}

fn draw_carousel(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    // Horizontal layout: left arrow, thumbnails, right arrow
    let arrow_width = 3;
    let thumbnails_area_width = area.width.saturating_sub(arrow_width * 2);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(arrow_width),
            Constraint::Min(thumbnails_area_width),
            Constraint::Length(arrow_width),
        ])
        .split(area);

    // Left arrow
    let can_go_left = app.selected_wallpaper_idx > 0;
    let left_arrow = Paragraph::new(if can_go_left { "❮" } else { " " })
        .style(Style::default().fg(if can_go_left {
            theme.accent_primary
        } else {
            theme.fg_muted
        }))
        .alignment(Alignment::Center);

    // Center vertically
    let left_area = center_vertically(chunks[0], 1);
    f.render_widget(left_arrow, left_area);

    // Right arrow
    let can_go_right = app.selected_wallpaper_idx < app.filtered_wallpapers.len().saturating_sub(1);
    let right_arrow = Paragraph::new(if can_go_right { "❯" } else { " " })
        .style(Style::default().fg(if can_go_right {
            theme.accent_primary
        } else {
            theme.fg_muted
        }))
        .alignment(Alignment::Center);

    let right_area = center_vertically(chunks[2], 1);
    f.render_widget(right_arrow, right_area);

    // Thumbnails area
    draw_thumbnails(f, app, chunks[1], theme);
}

fn draw_thumbnails(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    if app.filtered_wallpapers.is_empty() {
        let empty = Paragraph::new("No matching wallpapers")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        let centered = center_vertically(area, 1);
        f.render_widget(empty, centered);
        return;
    }

    // Calculate visible range centered on selection
    let total = app.filtered_wallpapers.len();
    let grid_columns = app.config.thumbnails.grid_columns;
    let visible = grid_columns.min(total);
    let half = visible / 2;

    let start = if app.selected_wallpaper_idx <= half {
        0
    } else if app.selected_wallpaper_idx >= total.saturating_sub(half + 1) {
        total.saturating_sub(visible)
    } else {
        app.selected_wallpaper_idx - half
    };

    let end = (start + visible).min(total);

    // Calculate thumbnail positions
    let thumb_total_width = THUMBNAIL_WIDTH + 2; // +2 for spacing
    let total_thumbs_width = (visible as u16) * thumb_total_width;
    let start_x = area.x + (area.width.saturating_sub(total_thumbs_width)) / 2;

    // Center vertically
    let thumb_y = area.y + (area.height.saturating_sub(THUMBNAIL_HEIGHT + 2)) / 2;

    // Collect cache indices that need loading
    let indices_to_load: Vec<usize> = (start..end)
        .map(|idx| app.filtered_wallpapers[idx])
        .collect();

    // Request thumbnails for visible items (non-blocking)
    for &cache_idx in &indices_to_load {
        app.request_thumbnail(cache_idx);
    }

    for (i, idx) in (start..end).enumerate() {
        let cache_idx = app.filtered_wallpapers[idx];
        let is_selected = idx == app.selected_wallpaper_idx;

        // Get wallpaper info before mutable borrow
        let (filename, is_suggestion) = app.cache.wallpapers
            .get(cache_idx)
            .map(|wp| {
                let name = wp.path.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let suggested = app.is_pairing_suggestion(&wp.path);
                (name, suggested)
            })
            .unwrap_or(("?".to_string(), false));

        let is_loading = app.is_loading(cache_idx);

        let thumb_x = start_x + (i as u16) * thumb_total_width;

        // Bounds check - skip if outside visible area
        if thumb_x + THUMBNAIL_WIDTH > area.x + area.width {
            continue;
        }
        if thumb_y + THUMBNAIL_HEIGHT + 2 > area.y + area.height {
            continue;
        }

        let thumb_area = Rect::new(thumb_x, thumb_y, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT + 2);

        // Draw thumbnail frame - green for suggestions, highlight for selected
        let border_color = if is_selected {
            theme.accent_highlight
        } else if is_suggestion {
            theme.success  // Green for pairing suggestions
        } else {
            theme.border
        };

        let border_style = if is_suggestion && !is_selected {
            Style::default().fg(border_color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(border_color)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(theme.bg_medium));

        let inner = block.inner(thumb_area);
        f.render_widget(block, thumb_area);

        // Try to render image if available
        if let Some(protocol) = app.get_thumbnail(cache_idx) {
            let image = StatefulImage::new(None);
            f.render_stateful_widget(image, inner, protocol);
        } else if is_loading {
            // Show loading indicator
            let loading = Paragraph::new("...")
                .style(Style::default().fg(theme.accent_primary))
                .alignment(Alignment::Center);
            let loading_area = center_vertically(inner, 1);
            f.render_widget(loading, loading_area);
        } else {
            // Fallback: show filename
            let max_chars = inner.width as usize;
            let display = if max_chars == 0 {
                String::new()
            } else if filename.chars().count() <= max_chars {
                filename.clone()
            } else {
                // Safe truncation using char boundaries
                let truncated: String = filename.chars().take(max_chars.saturating_sub(1)).collect();
                format!("{}…", truncated)
            };

            let label = Paragraph::new(display)
                .style(Style::default().fg(theme.fg_secondary))
                .alignment(Alignment::Center);

            let label_area = center_vertically(inner, 1);
            f.render_widget(label, label_area);
        }

        // Indicators below thumbnail (with bounds check)
        if thumb_area.bottom() < area.y + area.height {
            let indicator_area = Rect::new(thumb_x, thumb_area.bottom(), THUMBNAIL_WIDTH, 1);

            if is_selected {
                // Selection indicator
                let indicator = Paragraph::new("▲")
                    .style(Style::default().fg(theme.accent_highlight))
                    .alignment(Alignment::Center);
                f.render_widget(indicator, indicator_area);
            } else if is_suggestion {
                // Pairing suggestion indicator
                let indicator = Paragraph::new("★ paired")
                    .style(Style::default().fg(theme.success))
                    .alignment(Alignment::Center);
                f.render_widget(indicator, indicator_area);
            }
        }
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    // Command mode - show command input line
    if app.command_mode {
        let cmd_line = Line::from(vec![
            Span::styled(":", Style::default().fg(theme.accent_primary).add_modifier(Modifier::BOLD)),
            Span::styled(&app.command_buffer, Style::default().fg(theme.fg_primary)),
            Span::styled("█", Style::default().fg(theme.accent_primary)), // Cursor
        ]);
        let paragraph = Paragraph::new(cmd_line);
        f.render_widget(paragraph, area);
        return;
    }

    // Pairing preview mode - show pairing-specific help
    if app.show_pairing_preview {
        let sep = Span::styled(" │ ", Style::default().fg(theme.fg_muted));

        let help = Line::from(vec![
            Span::styled("←/→", Style::default().fg(theme.success)),
            Span::styled(" cycle", Style::default().fg(theme.fg_muted)),
            sep.clone(),
            Span::styled("1-3", Style::default().fg(theme.success)),
            Span::styled(" select", Style::default().fg(theme.fg_muted)),
            sep.clone(),
            Span::styled("Enter", Style::default().fg(theme.success)),
            Span::styled(" apply", Style::default().fg(theme.fg_muted)),
            sep.clone(),
            Span::styled("p/Esc", Style::default().fg(theme.success)),
            Span::styled(" close", Style::default().fg(theme.fg_muted)),
        ]);
        let paragraph = Paragraph::new(help).alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    draw_help_line(f, area, theme);
}

fn draw_help_line(f: &mut Frame, area: Rect, theme: &FrostTheme) {
    let sep = Span::styled(" │ ", Style::default().fg(theme.fg_muted));

    let help = Line::from(vec![
        Span::styled("←/→", Style::default().fg(theme.accent_primary)),
        Span::styled(" nav", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled("Enter", Style::default().fg(theme.accent_primary)),
        Span::styled(" apply", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled("p", Style::default().fg(theme.accent_primary)),
        Span::styled(" pair", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled(":", Style::default().fg(theme.accent_primary)),
        Span::styled(" cmd", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled("?", Style::default().fg(theme.accent_primary)),
        Span::styled(" help", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled("q", Style::default().fg(theme.accent_primary)),
        Span::styled(" quit", Style::default().fg(theme.fg_muted)),
    ]);

    let paragraph = Paragraph::new(help).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

fn center_vertically(area: Rect, height: u16) -> Rect {
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(area.x, y, area.width, height)
}

fn draw_color_palette(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    // Get colors from selected wallpaper
    let colors = app.selected_wallpaper()
        .map(|wp| wp.colors.clone())
        .unwrap_or_default();

    if colors.is_empty() {
        let text = Paragraph::new("No color data")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
        return;
    }

    // Build color swatches
    let mut spans = vec![
        Span::styled("Colors: ", Style::default().fg(theme.fg_secondary)),
    ];

    for (i, color_hex) in colors.iter().enumerate() {
        // Parse hex color
        if let Some(color) = parse_hex_color(color_hex) {
            // Color block using background color
            spans.push(Span::styled(
                "  █████  ",
                Style::default().fg(color),
            ));
            spans.push(Span::styled(
                color_hex,
                Style::default().fg(theme.fg_muted),
            ));

            if i < colors.len() - 1 {
                spans.push(Span::styled(" ", Style::default()));
            }
        }
    }

    // Get tags too
    let tags = app.selected_wallpaper()
        .map(|wp| wp.tags.clone())
        .unwrap_or_default();

    if !tags.is_empty() {
        spans.push(Span::styled("  │  Tags: ", Style::default().fg(theme.fg_secondary)));
        for (i, tag) in tags.iter().enumerate() {
            spans.push(Span::styled(
                format!("#{}", tag),
                Style::default().fg(theme.accent_highlight),
            ));
            if i < tags.len() - 1 {
                spans.push(Span::styled(" ", Style::default()));
            }
        }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

fn parse_hex_color(hex: &str) -> Option<ratatui::style::Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(ratatui::style::Color::Rgb(r, g, b))
    } else {
        None
    }
}

fn draw_color_picker(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let colors = &app.available_colors;
    if colors.is_empty() {
        return;
    }

    // Calculate popup size based on color count
    let cols = 8; // Colors per row
    let rows = colors.len().div_ceil(cols);
    let popup_width = 60.min(area.width.saturating_sub(4));
    let popup_height = (rows as u16 * 2 + 6).min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear background
    let clear = Block::default().style(Style::default().bg(theme.bg_dark));
    f.render_widget(clear, popup_area);

    // Popup border
    let title = if let Some(ref color) = app.active_color_filter {
        format!(" Color Filter [{}] ", color)
    } else {
        " Color Filter ".to_string()
    };

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(theme.accent_highlight).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_primary))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Draw color swatches in a grid
    let swatch_width = 6;
    let swatch_height = 1;
    let spacing = 1;

    for (i, color_hex) in colors.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;

        let x = inner.x + (col as u16) * (swatch_width + spacing);
        let y = inner.y + (row as u16) * (swatch_height + spacing);

        if x + swatch_width > inner.x + inner.width || y + swatch_height > inner.y + inner.height {
            continue;
        }

        let swatch_area = Rect::new(x, y, swatch_width, swatch_height);

        // Parse color
        let color = parse_hex_color(color_hex).unwrap_or(theme.fg_muted);

        // Highlight selected
        let is_selected = i == app.color_picker_idx;
        let style = if is_selected {
            Style::default().bg(color).fg(theme.bg_dark).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(color)
        };

        let text = if is_selected { "▶▶▶▶" } else { "████" };
        let swatch = Paragraph::new(text).style(style);
        f.render_widget(swatch, swatch_area);
    }

    // Footer with instructions
    let footer_y = inner.y + inner.height.saturating_sub(2);
    if footer_y > inner.y {
        let footer_area = Rect::new(inner.x, footer_y, inner.width, 2);
        let footer = Line::from(vec![
            Span::styled("←/→", Style::default().fg(theme.accent_primary)),
            Span::styled(" select ", Style::default().fg(theme.fg_muted)),
            Span::styled("Enter", Style::default().fg(theme.accent_primary)),
            Span::styled(" apply ", Style::default().fg(theme.fg_muted)),
            Span::styled("x", Style::default().fg(theme.accent_primary)),
            Span::styled(" clear ", Style::default().fg(theme.fg_muted)),
            Span::styled("Esc", Style::default().fg(theme.accent_primary)),
            Span::styled(" close", Style::default().fg(theme.fg_muted)),
        ]);
        let para = Paragraph::new(footer).alignment(Alignment::Center);
        f.render_widget(para, footer_area);
    }
}

fn draw_help_popup(f: &mut Frame, area: Rect, theme: &FrostTheme) {
    // Center the popup
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 35.min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear background
    let clear = Block::default().style(Style::default().bg(theme.bg_dark));
    f.render_widget(clear, popup_area);

    // Popup border
    let block = Block::default()
        .title(" ❄️ FrostWall Help ")
        .title_style(Style::default().fg(theme.accent_highlight).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_primary))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Help content
    let help_text = vec![
        Line::from(vec![
            Span::styled("Navigation", Style::default().fg(theme.accent_highlight).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  h/←     ", Style::default().fg(theme.accent_primary)),
            Span::styled("Previous wallpaper", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  l/→     ", Style::default().fg(theme.accent_primary)),
            Span::styled("Next wallpaper", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  Tab     ", Style::default().fg(theme.accent_primary)),
            Span::styled("Next screen", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  S-Tab   ", Style::default().fg(theme.accent_primary)),
            Span::styled("Previous screen", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Actions", Style::default().fg(theme.accent_highlight).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Enter   ", Style::default().fg(theme.accent_primary)),
            Span::styled("Apply wallpaper", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  r       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Random wallpaper", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  :       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Command mode (vim-style)", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Commands (:)", Style::default().fg(theme.accent_highlight).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  :t <tag>", Style::default().fg(theme.accent_primary)),
            Span::styled(" Filter by tag", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  :clear  ", Style::default().fg(theme.accent_primary)),
            Span::styled(" Clear all filters", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  :sim    ", Style::default().fg(theme.accent_primary)),
            Span::styled(" Find similar", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  :sort n ", Style::default().fg(theme.accent_primary)),
            Span::styled(" Sort (name/date/size)", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Options", Style::default().fg(theme.accent_highlight).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  m       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Toggle match mode", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  f       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Toggle resize mode", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  s       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Toggle sort mode", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  c       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Show/hide colors", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  t       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Cycle tag filter", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  T       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Clear tag filter", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  C       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Open color picker", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  p       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Pairing preview", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  w       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Export pywal colors", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  W       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Toggle auto pywal", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  q/Esc   ", Style::default().fg(theme.accent_primary)),
            Span::styled("Quit", Style::default().fg(theme.fg_secondary)),
        ]),
    ];

    let paragraph = Paragraph::new(help_text);
    f.render_widget(paragraph, inner);
}

/// Draw undo popup at bottom of screen
fn draw_undo_popup(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let remaining_secs = app.pairing_history.undo_remaining_secs().unwrap_or(0);
    let message = app.pairing_history.undo_message().unwrap_or("Undo available");

    // Position at bottom center
    let popup_width = 45.min(area.width.saturating_sub(4));
    let popup_height = 3;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.height.saturating_sub(popup_height + 2);

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear background
    let clear = Block::default().style(Style::default().bg(theme.bg_dark));
    f.render_widget(clear, popup_area);

    // Popup border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.warning))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Content
    let text = Line::from(vec![
        Span::styled(message, Style::default().fg(theme.fg_primary)),
        Span::styled(" | ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!("Undo (u) {}s", remaining_secs),
            Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
        ),
    ]);

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    f.render_widget(paragraph, inner);
}
