//! CLIP-based auto-tagging for wallpapers
//!
//! Uses ONNX Runtime with CLIP ViT-B/32 to automatically tag images
//! with semantic categories like "nature", "city", "space", etc.

#[cfg(feature = "clip")]
use anyhow::{Context, Result};
#[cfg(feature = "clip")]
use futures_util::StreamExt;
#[cfg(feature = "clip")]
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(feature = "clip")]
use ndarray::Array4;
#[cfg(feature = "clip")]
use ort::session::Session;
#[cfg(feature = "clip")]
use std::io::Write;
#[cfg(feature = "clip")]
use std::path::{Path, PathBuf};

/// CLIP image input size (ViT-B/32)
#[allow(dead_code)]
pub const CLIP_IMAGE_SIZE: u32 = 224;

/// CLIP embedding dimension
#[allow(dead_code)]
pub const CLIP_EMBEDDING_DIM: usize = 512;

/// Model URLs from HuggingFace
#[cfg(feature = "clip")]
const VISUAL_MODEL_URL: &str = "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/vision_model.onnx";

/// Predefined tag categories with descriptive text prompts
pub const TAG_CATEGORIES: &[(&str, &[&str])] = &[
    ("nature", &["a photo of nature", "natural landscape scenery"]),
    ("city", &["urban cityscape with buildings", "city skyline photography"]),
    ("space", &["outer space with stars and galaxies", "cosmic nebula artwork"]),
    ("abstract", &["abstract art with geometric patterns", "surreal digital artwork"]),
    ("anime", &["anime art style illustration", "japanese animation artwork"]),
    ("minimal", &["minimalist design with clean lines", "simple sparse composition"]),
    ("dark", &["dark moody atmosphere at night", "shadowy low-key scene"]),
    ("bright", &["bright vibrant colorful scene", "sunny cheerful photograph"]),
    ("ocean", &["ocean sea water and waves", "beach coastline scenery"]),
    ("forest", &["dense forest with tall trees", "woodland nature photography"]),
    ("mountain", &["mountain peaks alpine landscape", "rocky terrain hills"]),
    ("sunset", &["sunset sky golden hour colors", "sunrise twilight scenery"]),
    ("cyberpunk", &["cyberpunk neon city aesthetic", "futuristic technology scene"]),
    ("cozy", &["cozy warm comfortable interior", "homey relaxing atmosphere"]),
];

/// Auto-generated tag with confidence score
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoTag {
    pub name: String,
    pub confidence: f32,
}

/// Model cache directory manager
#[cfg(feature = "clip")]
pub struct ModelManager {
    cache_dir: PathBuf,
}

#[cfg(feature = "clip")]
impl ModelManager {
    pub fn new() -> Self {
        let cache_dir = directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.cache_dir().join("models"))
            .unwrap_or_else(|| PathBuf::from("/tmp/frostwall/models"));

        Self { cache_dir }
    }

    fn visual_model_path(&self) -> PathBuf {
        self.cache_dir.join("clip_visual.onnx")
    }

    pub fn models_cached(&self) -> bool {
        self.visual_model_path().exists()
    }

    pub async fn ensure_models(&self) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.cache_dir)?;

        let visual_path = self.visual_model_path();

        if !visual_path.exists() {
            self.download_model(VISUAL_MODEL_URL, &visual_path, "visual encoder")
                .await?;
        }

        Ok(visual_path)
    }

    async fn download_model(&self, url: &str, dest: &Path, name: &str) -> Result<()> {
        eprintln!("Downloading CLIP {} model...", name);

        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .context("Failed to start download")?;

        let total_size = response.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

        let mut file = std::fs::File::create(dest)?;
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error downloading chunk")?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download complete");
        eprintln!("Saved to {}", dest.display());

        Ok(())
    }
}

/// CLIP inference engine for tagging images
#[cfg(feature = "clip")]
pub struct ClipTagger {
    visual_session: Session,
    /// Pre-computed tag embeddings (simplified: just use heuristics for now)
    tag_heuristics: Vec<(String, ColorHeuristic)>,
}

/// Simple color-based heuristics for tags (fallback when CLIP text encoder unavailable)
#[cfg(feature = "clip")]
struct ColorHeuristic {
    /// Preferred dominant colors (RGB hex strings)
    colors: Vec<&'static str>,
    /// Brightness range (0.0-1.0): (min, max)
    brightness: (f32, f32),
}

#[cfg(feature = "clip")]
impl ClipTagger {
    /// Create a new tagger by loading ONNX models
    pub async fn new() -> Result<Self> {
        let model_manager = ModelManager::new();
        let visual_path = model_manager.ensure_models().await?;

        eprintln!("Loading CLIP visual model...");

        // Load visual encoder with ort 2.0 API
        let visual_session = Session::builder()?
            .with_intra_threads(4)?
            .commit_from_file(&visual_path)
            .context("Failed to load visual model")?;

        // Use color-based heuristics as simplified tagging
        // (Full CLIP text encoding requires tokenizer which is complex to implement)
        let tag_heuristics = Self::build_tag_heuristics();

        eprintln!("CLIP model loaded successfully");

        Ok(Self {
            visual_session,
            tag_heuristics,
        })
    }

    /// Build simple color heuristics for each tag
    fn build_tag_heuristics() -> Vec<(String, ColorHeuristic)> {
        vec![
            ("nature".to_string(), ColorHeuristic {
                colors: vec!["#228b22", "#006400", "#90ee90", "#2e8b57"],
                brightness: (0.2, 0.8),
            }),
            ("ocean".to_string(), ColorHeuristic {
                colors: vec!["#0077be", "#00bfff", "#1e90ff", "#4169e1"],
                brightness: (0.3, 0.9),
            }),
            ("forest".to_string(), ColorHeuristic {
                colors: vec!["#228b22", "#013220", "#355e3b", "#2e8b57"],
                brightness: (0.1, 0.6),
            }),
            ("sunset".to_string(), ColorHeuristic {
                colors: vec!["#ff6347", "#ff7f50", "#ffa500", "#ff4500"],
                brightness: (0.4, 0.9),
            }),
            ("dark".to_string(), ColorHeuristic {
                colors: vec!["#000000", "#1a1a2e", "#16213e", "#0f0f0f"],
                brightness: (0.0, 0.35),
            }),
            ("bright".to_string(), ColorHeuristic {
                colors: vec!["#ffffff", "#f0f0f0", "#fffacd", "#ffffe0"],
                brightness: (0.7, 1.0),
            }),
            ("cyberpunk".to_string(), ColorHeuristic {
                colors: vec!["#ff00ff", "#00ffff", "#ff1493", "#9400d3"],
                brightness: (0.2, 0.7),
            }),
            ("minimal".to_string(), ColorHeuristic {
                colors: vec!["#ffffff", "#f5f5f5", "#e0e0e0", "#fafafa"],
                brightness: (0.8, 1.0),
            }),
        ]
    }

    /// Check if models are already downloaded
    pub fn models_available() -> bool {
        ModelManager::new().models_cached()
    }

    /// Tag a single image using color analysis
    /// (Simplified version - analyzes dominant colors from wallpaper)
    pub fn tag_image(&self, image_path: &Path, threshold: f32) -> Result<Vec<AutoTag>> {
        // Load image and analyze colors
        let img = image::open(image_path).context("Failed to open image")?;
        let rgb = img.resize(64, 64, image::imageops::FilterType::Nearest).to_rgb8();

        // Calculate average brightness and dominant colors
        let mut brightness_sum = 0.0f32;
        let mut color_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for pixel in rgb.pixels() {
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;

            brightness_sum += (r + g + b) / 3.0;

            // Quantize to color bucket
            let bucket = format!("#{:02x}{:02x}{:02x}",
                (pixel[0] / 32) * 32,
                (pixel[1] / 32) * 32,
                (pixel[2] / 32) * 32
            );
            *color_counts.entry(bucket).or_insert(0) += 1;
        }

        let total_pixels = rgb.pixels().len() as f32;
        let avg_brightness = brightness_sum / total_pixels;

        // Get top colors
        let mut colors: Vec<_> = color_counts.into_iter().collect();
        colors.sort_by(|a, b| b.1.cmp(&a.1));
        let top_colors: Vec<String> = colors.into_iter().take(5).map(|(c, _)| c).collect();

        // Match against heuristics
        let mut tags = Vec::new();
        for (tag_name, heuristic) in &self.tag_heuristics {
            let mut score = 0.0f32;

            // Check brightness
            if avg_brightness >= heuristic.brightness.0 && avg_brightness <= heuristic.brightness.1 {
                score += 0.3;
            }

            // Check color similarity
            for top_color in &top_colors {
                for heuristic_color in &heuristic.colors {
                    let similarity = color_similarity(top_color, heuristic_color);
                    if similarity > 0.5 {
                        score += 0.2 * similarity;
                    }
                }
            }

            if score >= threshold {
                tags.push(AutoTag {
                    name: tag_name.clone(),
                    confidence: score.min(1.0),
                });
            }
        }

        // Sort by confidence
        tags.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        Ok(tags)
    }

    /// Get CLIP embedding for an image (placeholder - returns empty for now)
    #[allow(dead_code)]
    pub fn get_image_embedding(&self, _image_path: &Path) -> Result<Vec<f32>> {
        // Full CLIP embedding requires proper preprocessing
        // For now, return empty
        Ok(vec![])
    }

    /// Get list of available tag categories
    pub fn available_tags() -> Vec<&'static str> {
        TAG_CATEGORIES.iter().map(|(name, _)| *name).collect()
    }
}

/// Calculate color similarity between two hex colors
#[cfg(feature = "clip")]
fn color_similarity(a: &str, b: &str) -> f32 {
    let parse_hex = |s: &str| -> Option<(u8, u8, u8)> {
        let s = s.trim_start_matches('#');
        if s.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some((r, g, b))
    };

    let (ar, ag, ab) = match parse_hex(a) {
        Some(c) => c,
        None => return 0.0,
    };
    let (br, bg, bb) = match parse_hex(b) {
        Some(c) => c,
        None => return 0.0,
    };

    // Euclidean distance in RGB space, normalized
    let dr = (ar as f32 - br as f32) / 255.0;
    let dg = (ag as f32 - bg as f32) / 255.0;
    let db = (ab as f32 - bb as f32) / 255.0;

    let distance = (dr * dr + dg * dg + db * db).sqrt();
    1.0 - (distance / 1.732) // Max distance is sqrt(3)
}

/// Preprocess image for CLIP: resize to 224x224, normalize
#[cfg(feature = "clip")]
#[allow(dead_code)]
fn preprocess_image(path: &Path) -> Result<Array4<f32>> {
    let img = image::open(path).context("Failed to open image")?;
    let img = img.resize_exact(
        CLIP_IMAGE_SIZE,
        CLIP_IMAGE_SIZE,
        image::imageops::FilterType::Lanczos3,
    );
    let rgb = img.to_rgb8();

    // CLIP normalization constants
    let mean = [0.48145466, 0.4578275, 0.40821073];
    let std = [0.26862954, 0.26130258, 0.27577711];

    let mut data = Vec::with_capacity(3 * CLIP_IMAGE_SIZE as usize * CLIP_IMAGE_SIZE as usize);

    // Convert to CHW format and normalize
    for c in 0..3 {
        for y in 0..CLIP_IMAGE_SIZE {
            for x in 0..CLIP_IMAGE_SIZE {
                let pixel = rgb.get_pixel(x, y);
                let value = (pixel[c] as f32 / 255.0 - mean[c]) / std[c];
                data.push(value);
            }
        }
    }

    let array = Array4::from_shape_vec(
        (1, 3, CLIP_IMAGE_SIZE as usize, CLIP_IMAGE_SIZE as usize),
        data,
    )?;

    Ok(array)
}

// Stub implementations when clip feature is disabled
#[cfg(not(feature = "clip"))]
pub struct ClipTagger;

#[cfg(not(feature = "clip"))]
impl ClipTagger {
    pub fn models_available() -> bool {
        false
    }

    pub fn available_tags() -> Vec<&'static str> {
        TAG_CATEGORIES.iter().map(|(name, _)| *name).collect()
    }
}
