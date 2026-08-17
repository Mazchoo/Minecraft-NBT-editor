use serde::{Deserialize, Serialize};

use super::image_format::ImageFormat;

/// Reference to a cached texture file plus extraction-time metadata.
///
/// Pixel data is not stored here; [`crate::textures::TextureLibrary::get`] loads it on demand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Texture {
    /// File name within `./texture_cache/`, including extension.
    file_name: String,
    format: ImageFormat,
    /// True if any pixel has an alpha value below 255.
    has_alpha: bool,
}

impl Texture {
    pub(crate) fn new(file_name: String, format: ImageFormat, has_alpha: bool) -> Self {
        Self {
            file_name,
            format,
            has_alpha,
        }
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    #[allow(dead_code)] // used when sorting opaque vs alpha-discard batches
    pub fn format(&self) -> ImageFormat {
        self.format
    }

    #[allow(dead_code)] // used when sorting opaque vs alpha-discard batches
    pub fn has_alpha(&self) -> bool {
        self.has_alpha
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_serializes_like_the_manifest() {
        let texture = Texture::new("glass.png".into(), ImageFormat::Png, true);
        let json = serde_json::to_value(&texture).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "file_name": "glass.png",
                "format": "png",
                "has_alpha": true
            })
        );
    }
}
