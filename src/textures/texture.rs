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

/// Scans decoded pixels for transparency. BMP and JPG are recorded as opaque without decoding.
pub(crate) fn inspect_alpha(bytes: &[u8], format: ImageFormat) -> bool {
    if !format.carries_alpha_channel() {
        return false;
    }
    match image::load_from_memory(bytes) {
        Ok(img) => img.to_rgba8().pixels().any(|pixel| pixel.0[3] < 255),
        Err(err) => {
            log::warn!("could not decode image to inspect alpha: {err}");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::io::Cursor;

    fn encode(img: &RgbaImage, format: image::ImageFormat) -> Vec<u8> {
        let mut bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut bytes), format)
            .expect("encode test image");
        bytes
    }

    #[test]
    fn inspect_alpha_skips_bmp_and_jpg() {
        assert!(!inspect_alpha(&[], ImageFormat::Bmp));
        assert!(!inspect_alpha(&[], ImageFormat::Jpg));
    }

    #[test]
    fn inspect_alpha_scans_png() {
        let opaque = encode(
            &RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255])),
            image::ImageFormat::Png,
        );
        let transparent = encode(
            &RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 128])),
            image::ImageFormat::Png,
        );
        assert!(!inspect_alpha(&opaque, ImageFormat::Png));
        assert!(inspect_alpha(&transparent, ImageFormat::Png));
    }

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
