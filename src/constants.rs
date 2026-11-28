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

/// Blockstate property naming a crop's growth stage.
pub const AGE_PROPERTY: &str = "age";

/// Blockstate property naming the signal strength in redstone wire.
pub const POWER_PROPERTY: &str = "power";

/// Tint that leaves a texture's own colours unchanged.
pub const NO_TINT: [u8; 4] = [255; 4];

/// Tint of grass.
pub const GRASS_TINT: [u8; 4] = [145, 189, 89, 255];

/// Tint of leaves, vines and ferns.
pub const FOLIAGE_TINT: [u8; 4] = [72, 181, 24, 255];

/// Tint of water.
pub const WATER_TINT: [u8; 4] = [63, 118, 228, 255];

/// Tint of a melon or pumpkin stem at each growth stage, youngest first. A
/// stem joined to its fruit takes the last.
pub const STEM_TINTS: [[u8; 4]; 8] = [
    [0, 255, 0, 255],
    [32, 247, 4, 255],
    [64, 239, 8, 255],
    [96, 231, 12, 255],
    [128, 223, 16, 255],
    [160, 215, 20, 255],
    [192, 207, 24, 255],
    [224, 199, 28, 255],
];

/// First two bytes of a gzip stream. Structure files are normally gzipped.
pub const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
