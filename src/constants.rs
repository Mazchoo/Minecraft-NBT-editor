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

/// File name of the blockstate cache manifest at the repository root.
pub const BLOCKSTATES_MANIFEST_FILE: &str = "_blockstates.cache.json";

/// Share of the hotbar's remaining width a label may fill before it is cut
/// off, leaving room for the labels after it.
pub const HOTBAR_LABEL_WIDTH_FRACTION: f32 = 0.5;

/// Blockstate property naming the axis a block lies along.
pub const AXIS_PROPERTY: &str = "axis";

/// Value sets of blockstate properties whose multipart conditions do not
/// test every value: booleans, wall heights, redstone wire sides and bamboo
/// leaves. A
/// property whose tested values all fall in one set can take any of its
/// values, in this order.
pub const PROPERTY_VALUE_SETS: [&[&str]; 4] = [
    &["false", "true"],
    &["none", "low", "tall"],
    &["none", "side", "up"],
    &["none", "small", "large"],
];

/// Blockstate property naming the direction a block points.
pub const FACING_PROPERTY: &str = "facing";

/// Blockstate property naming which half of a double chest a block is.
pub const CHEST_TYPE_PROPERTY: &str = "type";

/// Side length, in texels, of the layout chest atlases are drawn on. A larger
/// atlas is the same layout scaled up.
pub const CHEST_ATLAS_SIZE: u32 = 64;

/// Prefix of the texture keys of tiles cut from chest atlases.
pub const CHEST_TILE_PREFIX: &str = "chest";

/// Namespace assumed for a block id written without one, as Minecraft does.
pub const DEFAULT_NAMESPACE: &str = "minecraft";

/// Blocks that stand for an empty cell. A structure import leaves their
/// cells empty, so they stay free to place into.
pub const AIR_BLOCK_NAMES: [&str; 3] =
    ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"];

/// First two bytes of a gzip stream. Structure files are normally gzipped.
pub const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
