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

/// Blockstate property naming the direction a block points.
pub const FACING_PROPERTY: &str = "facing";

/// Namespace assumed for a block id written without one, as Minecraft does.
pub const DEFAULT_NAMESPACE: &str = "minecraft";

/// Blocks that stand for an empty cell. A structure import leaves their
/// cells empty, so they stay free to place into.
pub const AIR_BLOCK_NAMES: [&str; 3] =
    ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"];

/// First two bytes of a gzip stream. Structure files are normally gzipped.
pub const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
