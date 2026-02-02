//! CLIP-based auto-tagging for wallpapers
//!
//! Uses ONNX Runtime with CLIP ViT-B/32 visual encoder to automatically tag images
//! with semantic categories like "nature", "city", "space", etc.
//!
//! The text embeddings are pre-computed and stored in clip_embeddings.rs to avoid
//! needing to download and run the text encoder at runtime.

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

#[cfg(feature = "clip")]
use crate::clip_embeddings::{CATEGORY_EMBEDDINGS, EMBEDDING_DIM};

/// CLIP image input size (ViT-B/32)
pub const CLIP_IMAGE_SIZE: u32 = 224;

/// Auto-generated tag with confidence score
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoTag {
    pub name: String,
    pub confidence: f32,
}

/// Model URLs from HuggingFace
/// Using Qdrant's model which outputs proper 512-dim projected embeddings
#[cfg(feature = "clip")]
const VISUAL_MODEL_URL: &str =
    "https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx";

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
}

#[cfg(feature = "clip")]
impl ClipTagger {
    /// Create a new tagger by loading ONNX models
    pub async fn new() -> Result<Self> {
        let model_manager = ModelManager::new();
        let visual_path = model_manager.ensure_models().await?;

        eprintln!("Loading CLIP visual model...");

        // Try CUDA first, fall back to CPU
        #[cfg(feature = "clip-cuda")]
        let visual_session = {
            use ort::execution_providers::{CUDAExecutionProvider, ExecutionProvider};

            let cuda_available = CUDAExecutionProvider::default().is_available()?;

            if cuda_available {
                eprintln!("Using CUDA GPU acceleration");
                Session::builder()?
                    .with_execution_providers([CUDAExecutionProvider::default().build()])?
                    .commit_from_file(&visual_path)
                    .context("Failed to load visual model with CUDA")?
            } else {
                eprintln!("CUDA not available, using CPU");
                Session::builder()?
                    .with_intra_threads(4)?
                    .commit_from_file(&visual_path)
                    .context("Failed to load visual model")?
            }
        };

        #[cfg(not(feature = "clip-cuda"))]
        let visual_session = Session::builder()?
            .with_intra_threads(4)?
            .commit_from_file(&visual_path)
            .context("Failed to load visual model")?;

        eprintln!("CLIP model loaded successfully");

        Ok(Self { visual_session })
    }

    /// Tag a single image using CLIP visual encoder
    ///
    /// Returns tags sorted by confidence (highest first)
    pub fn tag_image(&mut self, image_path: &Path, threshold: f32) -> Result<Vec<AutoTag>> {
        self.tag_image_verbose(image_path, threshold, false)
    }

    /// Tag with optional verbose output for debugging
    pub fn tag_image_verbose(&mut self, image_path: &Path, threshold: f32, verbose: bool) -> Result<Vec<AutoTag>> {
        // 1. Preprocess image to CLIP format
        let input = preprocess_image(image_path)?;

        // 2. Create input tensor from ndarray
        let (input_data, _offset) = input.into_raw_vec_and_offset();
        let input_tensor = ort::value::Tensor::<f32>::from_array((
            [1usize, 3, CLIP_IMAGE_SIZE as usize, CLIP_IMAGE_SIZE as usize],
            input_data,
        ))?;

        // 3. Run visual encoder inference
        let outputs = self.visual_session.run(ort::inputs![input_tensor])?;

        // 4. Extract image embedding from output
        // Get first output tensor
        let (_, output_value) = outputs.iter().next().context("No output tensor found")?;

        let tensor_ref = output_value
            .try_extract_tensor::<f32>()
            .context("Failed to extract embedding tensor")?;

        let shape: Vec<usize> = tensor_ref.0.iter().map(|&x| x as usize).collect();
        let embedding_data: &[f32] = tensor_ref.1;

        if verbose {
            eprintln!("  Output shape: {:?}", shape);
            eprintln!("  Output data length: {}", embedding_data.len());
        }

        // Get the [CLS] token embedding (first token) or pooled output
        let embedding: Vec<f32> = if shape.len() == 3 {
            // Shape: [batch, seq_len, hidden_dim] - take first token (CLS)
            let hidden_dim = shape[2];
            if verbose {
                eprintln!("  3D tensor, taking first {} values (CLS token)", hidden_dim);
            }
            embedding_data[..hidden_dim].to_vec()
        } else if shape.len() == 2 {
            // Shape: [batch, hidden_dim]
            let hidden_dim = shape[1];
            if verbose {
                eprintln!("  2D tensor, taking {} values", hidden_dim);
            }
            embedding_data[..hidden_dim].to_vec()
        } else {
            if verbose {
                eprintln!("  Using all {} values", embedding_data.len());
            }
            embedding_data.to_vec()
        };

        if verbose {
            eprintln!("  Embedding dimension: {}", embedding.len());
            eprintln!("  Expected dimension: {}", EMBEDDING_DIM);
            eprintln!("  First 5 values: {:?}", &embedding[..5.min(embedding.len())]);
        }

        // 4. Project to CLIP embedding space if needed (512 dim)
        let projected = if embedding.len() != EMBEDDING_DIM {
            // The raw hidden state is 768 dim, but we compare against 512-dim text embeddings
            // For now, truncate or warn - ideally we'd have the projection layer
            eprintln!(
                "WARNING: embedding dim {} != expected {}! Model may be incompatible.",
                embedding.len(),
                EMBEDDING_DIM
            );
            embedding
        } else {
            embedding
        };

        // 5. Normalize embedding
        let norm: f32 = projected.iter().map(|x| x * x).sum::<f32>().sqrt();
        let normalized: Vec<f32> = if norm > 0.0 {
            projected.iter().map(|x| x / norm).collect()
        } else {
            projected
        };

        // 6. Compute cosine similarity with each category embedding
        let mut tags = Vec::new();
        let mut all_scores: Vec<(&str, f32, f32)> = Vec::new();

        for (name, cat_embedding) in CATEGORY_EMBEDDINGS {
            let similarity: f32 = if normalized.len() == cat_embedding.len() {
                normalized
                    .iter()
                    .zip(cat_embedding.iter())
                    .map(|(a, b)| a * b)
                    .sum()
            } else {
                // Dimension mismatch - skip or use partial
                0.0
            };

            // CLIP similarities are typically in range [-1, 1], normalize to [0, 1]
            let confidence = (similarity + 1.0) / 2.0;

            all_scores.push((name, similarity, confidence));

            if confidence >= threshold {
                tags.push(AutoTag {
                    name: name.to_string(),
                    confidence,
                });
            }
        }

        if verbose {
            eprintln!("  Raw similarities (top 5):");
            let mut sorted_scores = all_scores.clone();
            sorted_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (name, sim, conf) in sorted_scores.iter().take(5) {
                eprintln!("    {}: raw={:.4}, conf={:.4}", name, sim, conf);
            }
            eprintln!("  Tags above threshold {}: {}", threshold, tags.len());
        }

        // Sort by confidence descending
        tags.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(tags)
    }

    /// Get list of available tag categories
    pub fn available_tags() -> Vec<&'static str> {
        CATEGORY_EMBEDDINGS.iter().map(|(name, _)| *name).collect()
    }
}

/// Get cached thumbnail path if it exists
#[cfg(feature = "clip")]
fn get_cached_thumbnail(source_path: &Path) -> Option<PathBuf> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let cache_dir = directories::ProjectDirs::from("com", "mrmattias", "frostwall")
        .map(|dirs| dirs.cache_dir().join("thumbs_v2"))
        .unwrap_or_else(|| PathBuf::from("/tmp/frostwall/thumbs_v2"));

    let mut hasher = DefaultHasher::new();
    source_path.to_string_lossy().hash(&mut hasher);
    if let Ok(metadata) = std::fs::metadata(source_path) {
        if let Ok(modified) = metadata.modified() {
            modified.hash(&mut hasher);
        }
    }
    let hash = hasher.finish();
    let thumb_path = cache_dir.join(format!("{:016x}.jpg", hash));

    if thumb_path.exists() {
        Some(thumb_path)
    } else {
        None
    }
}

/// Preprocess image for CLIP: resize to 224x224, normalize with CLIP constants
#[cfg(feature = "clip")]
fn preprocess_image(path: &Path) -> Result<Array4<f32>> {
    // Try to use cached thumbnail first (800x600 vs 4K original = much faster)
    let img = if let Some(thumb_path) = get_cached_thumbnail(path) {
        image::open(&thumb_path).unwrap_or_else(|_| image::open(path).unwrap())
    } else {
        image::open(path).context("Failed to open image")?
    };

    // Resize to CLIP input size (Triangle is fast and good enough for 224x224)
    let img = img.resize_exact(
        CLIP_IMAGE_SIZE,
        CLIP_IMAGE_SIZE,
        image::imageops::FilterType::Triangle,
    );
    let rgb = img.to_rgb8();

    // CLIP normalization constants (ImageNet stats used by CLIP)
    let mean = [0.48145466, 0.4578275, 0.40821073];
    let std = [0.26862954, 0.26130258, 0.27577711];

    let mut data = Vec::with_capacity(3 * CLIP_IMAGE_SIZE as usize * CLIP_IMAGE_SIZE as usize);

    // Convert to CHW format (channels first) and normalize
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

