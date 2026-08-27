use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};

/// Mutable app settings. Access via [`CONFIG`].
pub struct Config {
    major_line_block_spacing: AtomicI32,
}

impl Config {
    const DEFAULT_MAJOR_LINE_BLOCK_SPACING: i32 = 8;

    /// Manifest file name at the repository root.
    pub const TEXTURES_MANIFEST_FILE: &'static str = "_textures.cache.json";

    /// Manifest file name at the repository root.
    pub const MODELS_MANIFEST_FILE: &'static str = "_models.cache.json";

    pub const fn new() -> Self {
        Self {
            major_line_block_spacing: AtomicI32::new(Self::DEFAULT_MAJOR_LINE_BLOCK_SPACING),
        }
    }

    /// Directory that holds extracted block textures.
    pub fn texture_cache_dir() -> &'static Path {
        Path::new("./texture_cache")
    }

    /// Manifest mapping texture keys to cached files.
    pub fn textures_manifest_path() -> PathBuf {
        Path::new(".").join(Self::TEXTURES_MANIFEST_FILE)
    }

    /// Fallback image used when a texture key cannot be resolved.
    pub fn missing_texture_path() -> &'static Path {
        Path::new("./assets/image-missing.png")
    }

    /// Directory that holds extracted block model JSONs.
    pub fn model_cache_dir() -> &'static Path {
        Path::new("./model_cache")
    }

    /// Manifest mapping model keys to cached files.
    pub fn models_manifest_path() -> PathBuf {
        Path::new(".").join(Self::MODELS_MANIFEST_FILE)
    }

    /// Blocks between brighter major grid lines.
    pub fn major_line_block_spacing(&self) -> i32 {
        self.major_line_block_spacing.load(Ordering::Relaxed)
    }

    /// Sets major-line spacing in blocks. Values below 1 are clamped to 1.
    #[allow(dead_code)] // wired up when settings UI lands
    pub fn set_major_line_block_spacing(&self, value: i32) {
        self.major_line_block_spacing
            .store(value.max(1), Ordering::Relaxed);
    }
}

pub static CONFIG: Config = Config::new();
