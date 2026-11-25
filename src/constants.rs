//! Fixed values baked into the build. Anything a user could reasonably tune
//! belongs in `config` instead.

/// WGSL source for textured blocks, embedded at compile time. The path is
/// relative to this file.
pub const BLOCKS_SHADER: &str = include_str!("rendering/shaders/blocks.wgsl");

/// WGSL source for per-vertex-coloured geometry: the reference grid and the
/// placement highlight.
pub const GRID_SHADER: &str = include_str!("rendering/shaders/grid.wgsl");

/// File name of the texture cache manifest at the repository root.
pub const TEXTURES_MANIFEST_FILE: &str = "_textures.cache.json";

/// File name of the model cache manifest at the repository root.
pub const MODELS_MANIFEST_FILE: &str = "_models.cache.json";
