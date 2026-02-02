use std::path::Path;
use palette::{IntoColor, Lab, Srgb};

/// Supported image file extensions
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

/// Types of color harmony between two palettes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorHarmony {
    /// Colors are very similar (within 30°)
    Analogous,
    /// Colors are opposite on the color wheel (180° ± 15°)
    Complementary,
    /// Colors form a triadic harmony (120° apart)
    Triadic,
    /// Complement + adjacent colors (150-180°)
    SplitComplementary,
    /// No specific harmony detected
    None,
}

impl ColorHarmony {
    /// Get a display name for the harmony type
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            ColorHarmony::Analogous => "Analogous",
            ColorHarmony::Complementary => "Complementary",
            ColorHarmony::Triadic => "Triadic",
            ColorHarmony::SplitComplementary => "Split-Complementary",
            ColorHarmony::None => "None",
        }
    }

    /// Get the bonus multiplier for this harmony type
    pub fn bonus(&self) -> f32 {
        match self {
            ColorHarmony::Analogous => 1.0,      // Similar colors always work
            ColorHarmony::Complementary => 0.9,  // Strong contrast, usually works
            ColorHarmony::Triadic => 0.7,        // Balanced but can be busy
            ColorHarmony::SplitComplementary => 0.8,
            ColorHarmony::None => 0.0,
        }
    }
}

/// Parse hex color string to RGB tuple
/// Supports "#RRGGBB" and "RRGGBB" formats
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Convert hex color to LAB color space
pub fn hex_to_lab(hex: &str) -> Option<Lab> {
    let (r, g, b) = hex_to_rgb(hex)?;
    let rgb = Srgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    Some(rgb.into_color())
}

/// Convert hex color to HSL and return hue (0-360), saturation (0-1), lightness (0-1)
pub fn hex_to_hsl(hex: &str) -> Option<(f32, f32, f32)> {
    let (r, g, b) = hex_to_rgb(hex)?;
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let lightness = (max + min) / 2.0;

    if delta < 0.0001 {
        // Achromatic (gray)
        return Some((0.0, 0.0, lightness));
    }

    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let hue = if (max - r).abs() < 0.0001 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 0.0001 {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    let hue = if hue < 0.0 { hue + 360.0 } else { hue };

    Some((hue, saturation, lightness))
}

/// Calculate the angular difference between two hue values (0-180)
fn hue_difference(h1: f32, h2: f32) -> f32 {
    let diff = (h1 - h2).abs();
    if diff > 180.0 { 360.0 - diff } else { diff }
}

/// Detect the color harmony between two palettes
/// Returns the harmony type and a strength score (0.0-1.0)
pub fn detect_harmony(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> (ColorHarmony, f32) {
    if colors1.is_empty() || colors2.is_empty() {
        return (ColorHarmony::None, 0.0);
    }

    // Get the dominant (highest weight) saturated color from each palette
    let get_dominant_hue = |colors: &[String], weights: &[f32]| -> Option<(f32, f32)> {
        colors.iter()
            .zip(weights.iter().chain(std::iter::repeat(&(1.0 / colors.len() as f32))))
            .filter_map(|(c, w)| {
                hex_to_hsl(c).and_then(|(h, s, l)| {
                    // Only consider colors with enough saturation
                    if s > 0.15 && l > 0.1 && l < 0.9 {
                        Some((h, s * w))
                    } else {
                        None
                    }
                })
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(h, _)| (h, 1.0))
    };

    let hue1 = get_dominant_hue(colors1, weights1);
    let hue2 = get_dominant_hue(colors2, weights2);

    match (hue1, hue2) {
        (Some((h1, _)), Some((h2, _))) => {
            let diff = hue_difference(h1, h2);

            // Determine harmony type based on hue difference
            let (harmony, strength) = if diff < 30.0 {
                // Analogous: similar hues
                (ColorHarmony::Analogous, 1.0 - diff / 30.0)
            } else if (165.0..=195.0).contains(&diff) {
                // Complementary: opposite hues
                let center_diff = (diff - 180.0).abs();
                (ColorHarmony::Complementary, 1.0 - center_diff / 15.0)
            } else if (105.0..=135.0).contains(&diff) {
                // Triadic: 120° apart
                let center_diff = (diff - 120.0).abs();
                (ColorHarmony::Triadic, 1.0 - center_diff / 15.0)
            } else if (135.0..165.0).contains(&diff) {
                // Split-complementary
                let center_diff = (diff - 150.0).abs();
                (ColorHarmony::SplitComplementary, 1.0 - center_diff / 15.0)
            } else {
                (ColorHarmony::None, 0.0)
            };

            (harmony, strength.max(0.0))
        }
        _ => {
            // One or both palettes are achromatic - check brightness match instead
            (ColorHarmony::None, 0.0)
        }
    }
}

/// Calculate Delta E (CIE76) color distance between two LAB colors
/// Lower values = more similar, 0 = identical
/// < 1.0: Not perceptible by human eye
/// 1-2: Perceptible through close observation
/// 2-10: Perceptible at a glance
/// 11-49: Colors are more similar than opposite
/// 100: Colors are exact opposite
#[allow(dead_code)]
pub fn delta_e(lab1: &Lab, lab2: &Lab) -> f32 {
    let dl = lab1.l - lab2.l;
    let da = lab1.a - lab2.a;
    let db = lab1.b - lab2.b;
    (dl * dl + da * da + db * db).sqrt()
}

/// Calculate Delta E 2000 (CIEDE2000) - perceptually uniform color difference
///
/// This is more accurate than CIE76, especially for:
/// - Dark colors
/// - Saturated colors
/// - Neutral/gray colors
///
/// Reference: https://en.wikipedia.org/wiki/Color_difference#CIEDE2000
pub fn delta_e_2000(lab1: &Lab, lab2: &Lab) -> f32 {
    use std::f32::consts::PI;

    let l1 = lab1.l;
    let a1 = lab1.a;
    let b1 = lab1.b;
    let l2 = lab2.l;
    let a2 = lab2.a;
    let b2 = lab2.b;

    // Weighting factors (standard values)
    let k_l = 1.0_f32;
    let k_c = 1.0_f32;
    let k_h = 1.0_f32;

    // Calculate C'ab (chroma)
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let c_avg = (c1 + c2) / 2.0;

    // Calculate G factor
    let c_avg_pow7 = c_avg.powi(7);
    let g = 0.5 * (1.0 - (c_avg_pow7 / (c_avg_pow7 + 6103515625.0_f32)).sqrt()); // 25^7 = 6103515625

    // Calculate a' (adjusted a)
    let a1_prime = a1 * (1.0 + g);
    let a2_prime = a2 * (1.0 + g);

    // Calculate C' (adjusted chroma)
    let c1_prime = (a1_prime * a1_prime + b1 * b1).sqrt();
    let c2_prime = (a2_prime * a2_prime + b2 * b2).sqrt();

    // Calculate h' (hue angle in radians)
    let h1_prime = if a1_prime == 0.0 && b1 == 0.0 {
        0.0
    } else {
        let h = b1.atan2(a1_prime);
        if h < 0.0 { h + 2.0 * PI } else { h }
    };

    let h2_prime = if a2_prime == 0.0 && b2 == 0.0 {
        0.0
    } else {
        let h = b2.atan2(a2_prime);
        if h < 0.0 { h + 2.0 * PI } else { h }
    };

    // Calculate differences
    let delta_l = l2 - l1;
    let delta_c = c2_prime - c1_prime;

    // Calculate delta h'
    let delta_h_prime = if c1_prime * c2_prime == 0.0 {
        0.0
    } else {
        let dh = h2_prime - h1_prime;
        if dh.abs() <= PI {
            dh
        } else if dh > PI {
            dh - 2.0 * PI
        } else {
            dh + 2.0 * PI
        }
    };

    // Calculate delta H'
    let delta_h = 2.0 * (c1_prime * c2_prime).sqrt() * (delta_h_prime / 2.0).sin();

    // Calculate average values
    let l_avg = (l1 + l2) / 2.0;
    let c_avg_prime = (c1_prime + c2_prime) / 2.0;

    // Calculate h' average
    let h_avg_prime = if c1_prime * c2_prime == 0.0 {
        h1_prime + h2_prime
    } else {
        let sum = h1_prime + h2_prime;
        let diff = (h1_prime - h2_prime).abs();
        if diff <= PI {
            sum / 2.0
        } else if sum < 2.0 * PI {
            (sum + 2.0 * PI) / 2.0
        } else {
            (sum - 2.0 * PI) / 2.0
        }
    };

    // Calculate T
    let t = 1.0
        - 0.17 * (h_avg_prime - PI / 6.0).cos()
        + 0.24 * (2.0 * h_avg_prime).cos()
        + 0.32 * (3.0 * h_avg_prime + PI / 30.0).cos()
        - 0.20 * (4.0 * h_avg_prime - 63.0 * PI / 180.0).cos();

    // Calculate S_L, S_C, S_H
    let l_avg_minus_50_sq = (l_avg - 50.0).powi(2);
    let s_l = 1.0 + (0.015 * l_avg_minus_50_sq) / (20.0 + l_avg_minus_50_sq).sqrt();
    let s_c = 1.0 + 0.045 * c_avg_prime;
    let s_h = 1.0 + 0.015 * c_avg_prime * t;

    // Calculate R_T (rotation term)
    let delta_theta = 30.0 * (-(((h_avg_prime * 180.0 / PI) - 275.0) / 25.0).powi(2)).exp();
    let r_c = 2.0 * (c_avg_prime.powi(7) / (c_avg_prime.powi(7) + 6103515625.0_f32)).sqrt();
    let r_t = -(r_c * (2.0 * delta_theta * PI / 180.0).sin());

    // Calculate final delta E 2000
    let term1 = delta_l / (k_l * s_l);
    let term2 = delta_c / (k_c * s_c);
    let term3 = delta_h / (k_h * s_h);
    let term4 = r_t * (delta_c / (k_c * s_c)) * (delta_h / (k_h * s_h));

    (term1 * term1 + term2 * term2 + term3 * term3 + term4).sqrt()
}

/// Calculate color similarity score between two hex colors
/// Returns a score from 0.0 (opposite) to 1.0 (identical)
/// Uses Delta-E 2000 for perceptually accurate comparison
pub fn color_similarity(hex1: &str, hex2: &str) -> f32 {
    match (hex_to_lab(hex1), hex_to_lab(hex2)) {
        (Some(lab1), Some(lab2)) => {
            let distance = delta_e_2000(&lab1, &lab2);
            // Convert distance to similarity (0-1 range)
            // Delta-E 2000 values: 0 = identical, 1 = barely noticeable, 100 = very different
            // Use a curve that's more sensitive to small differences
            (1.0 - (distance / 100.0).powf(0.7)).max(0.0)
        }
        _ => 0.0,
    }
}

/// Find the best color match between two palettes, weighted by color dominance
/// Each color's contribution is scaled by its weight (proportion of the image)
/// Returns a weighted similarity score (0.0-1.0)
pub fn palette_similarity_weighted(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    // Normalize weights in case they don't sum to 1
    let sum1: f32 = weights1.iter().sum();
    let sum2: f32 = weights2.iter().sum();
    let norm_weights1: Vec<f32> = if sum1 > 0.0 {
        weights1.iter().map(|w| w / sum1).collect()
    } else {
        vec![1.0 / colors1.len() as f32; colors1.len()]
    };
    let norm_weights2: Vec<f32> = if sum2 > 0.0 {
        weights2.iter().map(|w| w / sum2).collect()
    } else {
        vec![1.0 / colors2.len() as f32; colors2.len()]
    };

    let mut total_similarity = 0.0;

    // For each color in palette 1, find best match in palette 2
    // Weight the match by both the source color's weight and the best match's weight
    for (i, c1) in colors1.iter().enumerate() {
        let w1 = norm_weights1.get(i).copied().unwrap_or(0.0);
        if w1 < 0.01 {
            continue; // Skip very minor colors
        }

        let mut best_sim = 0.0;

        for (j, c2) in colors2.iter().enumerate() {
            let w2 = norm_weights2.get(j).copied().unwrap_or(0.0);
            let sim = color_similarity(c1, c2);

            // Boost similarity when matching dominant colors with dominant colors
            let weight_boost = (w2 * 2.0).min(1.0);
            let boosted_sim = sim * (0.7 + 0.3 * weight_boost);

            if boosted_sim > best_sim {
                best_sim = boosted_sim;
            }
        }

        // Scale contribution by source color's weight
        total_similarity += best_sim * w1;
    }

    total_similarity
}

/// Calculate brightness of a hex color (0.0-1.0)
pub fn color_brightness(hex: &str) -> f32 {
    match hex_to_rgb(hex) {
        Some((r, g, b)) => {
            // Perceived brightness formula
            (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0
        }
        None => 0.5,
    }
}

/// Calculate saturation of a hex color (0.0-1.0)
pub fn color_saturation(hex: &str) -> f32 {
    match hex_to_rgb(hex) {
        Some((r, g, b)) => {
            let r = r as f32 / 255.0;
            let g = g as f32 / 255.0;
            let b = b as f32 / 255.0;
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            if max == 0.0 {
                0.0
            } else {
                (max - min) / max
            }
        }
        None => 0.0,
    }
}

/// Calculate overall image similarity based on color profile
/// Returns a score from 0.0 (very different) to 1.0 (very similar)
pub fn image_similarity(
    colors1: &[String],
    colors2: &[String],
) -> f32 {
    // Use equal weights for backward compatibility
    let weights1: Vec<f32> = vec![1.0 / colors1.len().max(1) as f32; colors1.len()];
    let weights2: Vec<f32> = vec![1.0 / colors2.len().max(1) as f32; colors2.len()];
    image_similarity_weighted(colors1, &weights1, colors2, &weights2)
}

/// Calculate overall image similarity based on color profile with weights
/// Returns a score from 0.0 (very different) to 1.0 (very similar)
pub fn image_similarity_weighted(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    // Component 1: Palette similarity (color matching) with weights
    let color_sim = palette_similarity_weighted(colors1, weights1, colors2, weights2);

    // Component 2: Weighted brightness similarity
    let sum1: f32 = weights1.iter().sum();
    let sum2: f32 = weights2.iter().sum();
    let bright1: f32 = colors1.iter()
        .zip(weights1.iter())
        .map(|(c, w)| color_brightness(c) * w)
        .sum::<f32>() / sum1.max(0.001);
    let bright2: f32 = colors2.iter()
        .zip(weights2.iter())
        .map(|(c, w)| color_brightness(c) * w)
        .sum::<f32>() / sum2.max(0.001);
    let bright_sim = 1.0 - (bright1 - bright2).abs();

    // Component 3: Weighted saturation similarity
    let sat1: f32 = colors1.iter()
        .zip(weights1.iter())
        .map(|(c, w)| color_saturation(c) * w)
        .sum::<f32>() / sum1.max(0.001);
    let sat2: f32 = colors2.iter()
        .zip(weights2.iter())
        .map(|(c, w)| color_saturation(c) * w)
        .sum::<f32>() / sum2.max(0.001);
    let sat_sim = 1.0 - (sat1 - sat2).abs();

    // Weighted combination
    color_sim * 0.6 + bright_sim * 0.25 + sat_sim * 0.15
}

/// Find similar wallpapers based on color profile
/// Returns Vec of (similarity_score, wallpaper_index) sorted by similarity
pub fn find_similar_wallpapers(
    target_colors: &[String],
    all_wallpapers: &[(usize, &[String])], // (index, colors)
    limit: usize,
) -> Vec<(f32, usize)> {
    let mut similarities: Vec<(f32, usize)> = all_wallpapers
        .iter()
        .map(|(idx, colors)| {
            let sim = image_similarity(target_colors, colors);
            (sim, *idx)
        })
        .collect();

    // Sort by similarity descending
    similarities.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    similarities.into_iter().take(limit).collect()
}

/// Check if a path is a supported image file
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let ext = e.to_lowercase();
            IMAGE_EXTENSIONS.iter().any(|&supported| supported == ext)
        })
        .unwrap_or(false)
}

/// Expand tilde (~) in path
pub fn expand_tilde(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}
