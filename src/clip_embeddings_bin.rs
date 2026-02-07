//! Compact binary CLIP text embeddings loader.
//!
//! Replaces the 13K-line clip_embeddings.rs with a ~52 KB binary file
//! that is included at compile time and parsed lazily on first access.
//!
//! Binary format (little-endian):
//!   [u32] number_of_categories
//!   Per category:
//!     [u32] name_length
//!     [u8 * name_length] UTF-8 name
//!     [f32 * 512] embedding values

use std::sync::OnceLock;

/// CLIP embedding dimension (ViT-B/32)
pub const EMBEDDING_DIM: usize = 512;

/// Raw binary data included at compile time
const EMBEDDINGS_DATA: &[u8] = include_bytes!("../data/embeddings.bin");

/// Parsed embeddings, lazily initialized on first access.
static PARSED: OnceLock<Vec<(String, [f32; EMBEDDING_DIM])>> = OnceLock::new();

/// Get the pre-computed category embeddings.
///
/// Returns a slice of (category_name, embedding_vector) pairs.
/// Parsed from the binary data on first call, cached for subsequent calls.
pub fn category_embeddings() -> &'static [(String, [f32; EMBEDDING_DIM])] {
    PARSED.get_or_init(|| parse_embeddings(EMBEDDINGS_DATA))
}

fn parse_embeddings(data: &[u8]) -> Vec<(String, [f32; EMBEDDING_DIM])> {
    let mut offset = 0;

    let num_categories = read_u32(data, &mut offset);
    let mut result = Vec::with_capacity(num_categories as usize);

    for _ in 0..num_categories {
        let name_len = read_u32(data, &mut offset) as usize;
        let name = std::str::from_utf8(&data[offset..offset + name_len])
            .expect("Invalid UTF-8 in embeddings data")
            .to_string();
        offset += name_len;

        let mut embedding = [0.0f32; EMBEDDING_DIM];
        for value in &mut embedding {
            *value = read_f32(data, &mut offset);
        }

        result.push((name, embedding));
    }

    result
}

fn read_u32(data: &[u8], offset: &mut usize) -> u32 {
    let bytes: [u8; 4] = data[*offset..*offset + 4]
        .try_into()
        .expect("Not enough bytes for u32");
    *offset += 4;
    u32::from_le_bytes(bytes)
}

fn read_f32(data: &[u8], offset: &mut usize) -> f32 {
    let bytes: [u8; 4] = data[*offset..*offset + 4]
        .try_into()
        .expect("Not enough bytes for f32");
    *offset += 4;
    f32::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embeddings_load() {
        let embeddings = category_embeddings();
        assert_eq!(embeddings.len(), 57, "Should have 57 base categories");
    }

    #[test]
    fn test_embeddings_dimension() {
        let embeddings = category_embeddings();
        for (name, embedding) in embeddings {
            assert_eq!(
                embedding.len(),
                EMBEDDING_DIM,
                "Category '{}' should have {} dimensions",
                name,
                EMBEDDING_DIM
            );
        }
    }

    #[test]
    fn test_embeddings_normalized() {
        let embeddings = category_embeddings();
        for (name, embedding) in embeddings {
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(
                (norm - 1.0).abs() < 0.01,
                "Category '{}' should be normalized (norm = {})",
                name,
                norm
            );
        }
    }

    #[test]
    fn test_known_categories_present() {
        let embeddings = category_embeddings();
        let names: Vec<&str> = embeddings.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"abstract"));
        assert!(names.contains(&"nature"));
        assert!(names.contains(&"space"));
        assert!(names.contains(&"anime"));
    }
}
