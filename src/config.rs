use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

use crate::constants;

/// Mutable app settings. Access via [`CONFIG`].
pub struct Config {
    major_line_block_spacing: AtomicI32,
    undo_history_limit: AtomicUsize,
}

impl Config {
    const DEFAULT_MAJOR_LINE_BLOCK_SPACING: i32 = 8;
    const DEFAULT_UNDO_HISTORY_LIMIT: usize = 100;

    /// Width and height, in pixels, of one block texture on the GPU. Larger
    /// source images and animation strips are resampled to this size.
    pub const TEXTURE_SIZE: u32 = 16;

    /// Largest structure bounding volume, in cells, searched for enclosed
    /// cavities when culling hidden faces (one byte per cell). Larger
    /// structures still cull faces between touching blocks.
    pub const MAX_CAVITY_FLOOD_CELLS: i64 = 1 << 25;

    /// RGBA colour of the ordinary reference-grid lines.
    pub const GRID_MINOR_COLOR: [f32; 4] = [0.45, 0.45, 0.48, 1.0];

    /// RGBA colour of the brighter grid lines drawn every major spacing.
    pub const GRID_MAJOR_COLOR: [f32; 4] = [0.8, 0.8, 0.8, 1.0];

    /// RGBA colours of the world axes through the origin.
    pub const GRID_X_AXIS_COLOR: [f32; 4] = [0.85, 0.28, 0.30, 1.0];
    pub const GRID_Y_AXIS_COLOR: [f32; 4] = [0.42, 0.75, 0.25, 1.0];
    pub const GRID_Z_AXIS_COLOR: [f32; 4] = [0.25, 0.45, 0.90, 1.0];

    /// RGBA colour of the overlay on the block face a placement would target.
    pub const HIGHLIGHT_COLOR: [f32; 4] = [1.0, 0.92, 0.2, 0.42];

    /// RGBA colour of the placement overlay while facing mode (toggled by E)
    /// is on, when a click places the block with its bottom facing the
    /// highlighted face.
    pub const HIGHLIGHT_FACING_COLOR: [f32; 4] = [0.25, 0.8, 1.0, 0.45];

    /// Distance, in blocks, the placement overlay floats above the face it
    /// marks. The overlay is coplanar with that face, so without a small gap
    /// the two fight over the depth buffer and the overlay flickers.
    pub const HIGHLIGHT_OUTSET: f32 = 0.008;

    /// Closest, in blocks, the camera gets when framing an imported
    /// structure, so a single block is not viewed from up close.
    pub const MIN_FRAME_DISTANCE: f32 = 4.0;

    /// Distance, in blocks, the camera sits from a face it is snapped to
    /// look at by a middle double-click.
    pub const FACE_VIEW_DISTANCE: f32 = 2.0;

    /// Blocks per second the camera moves while a WASD key is held.
    pub const PAN_SPEED: f32 = 24.0;

    /// Blocks the camera moves along the cursor ray per point of scroll.
    pub const ZOOM_SPEED: f32 = 0.05;

    /// Closest, in blocks, zooming in brings the camera to the point under
    /// the cursor.
    pub const MIN_ZOOM_DISTANCE: f32 = 0.5;

    /// Radians the camera turns per pixel of middle-mouse drag.
    pub const ORBIT_SENSITIVITY: f32 = 0.008;

    /// Steepest tilt, in radians up or down, a middle-mouse drag reaches.
    pub const MAX_ORBIT_PITCH: f32 = 89.0 * std::f32::consts::PI / 180.0;

    /// Most variants of one block the picker cycles through with Tab.
    pub const MAX_BLOCK_VARIANTS: usize = 1024;

    pub const fn new() -> Self {
        Self {
            major_line_block_spacing: AtomicI32::new(Self::DEFAULT_MAJOR_LINE_BLOCK_SPACING),
            undo_history_limit: AtomicUsize::new(Self::DEFAULT_UNDO_HISTORY_LIMIT),
        }
    }

    /// Directory that holds extracted block textures.
    pub fn texture_cache_dir() -> &'static Path {
        Path::new("./texture_cache")
    }

    /// Manifest mapping texture keys to cached files.
    pub fn textures_manifest_path() -> PathBuf {
        Path::new(".").join(constants::TEXTURES_MANIFEST_FILE)
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
        Path::new(".").join(constants::MODELS_MANIFEST_FILE)
    }

    /// Manifest mapping block names to their blockstate definitions.
    pub fn blockstates_manifest_path() -> PathBuf {
        Path::new(".").join(constants::BLOCKSTATES_MANIFEST_FILE)
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

    /// Number of most recent transactions kept for undo.
    pub fn undo_history_limit(&self) -> usize {
        self.undo_history_limit.load(Ordering::Relaxed)
    }

    /// Sets how many transactions are kept for undo. Zero disables undo.
    #[allow(dead_code)] // wired up when settings UI lands
    pub fn set_undo_history_limit(&self, value: usize) {
        self.undo_history_limit.store(value, Ordering::Relaxed);
    }
}

pub static CONFIG: Config = Config::new();
