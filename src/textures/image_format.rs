use serde::{Deserialize, Serialize};

/// On-disk image format for a cached block texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    Bmp,
    Jpg,
}

impl ImageFormat {
    /// Parses a file extension (`png`, `bmp`, `jpg`). Other extensions are rejected.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "bmp" => Some(Self::Bmp),
            "jpg" => Some(Self::Jpg),
            _ => None,
        }
    }

    /// PNG is the only supported format that can carry an alpha channel.
    pub fn carries_alpha_channel(self) -> bool {
        matches!(self, Self::Png)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_extension_accepts_png_bmp_jpg_only() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("PNG"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("bmp"), Some(ImageFormat::Bmp));
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpg));
        assert_eq!(ImageFormat::from_extension("jpeg"), None);
        assert_eq!(ImageFormat::from_extension("json"), None);
    }
}
