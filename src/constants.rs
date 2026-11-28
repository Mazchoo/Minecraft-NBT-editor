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

/// Multipart property joining a block to the block below (chorus plant, glow
/// lichen). The picker lists its `true` states first, so a picked block
/// starts attached to the ground.
pub const DOWN_PROPERTY: &str = "down";

/// Value of a boolean blockstate property that is on.
pub const TRUE_VALUE: &str = "true";

/// Blockstate property naming the direction a block points.
pub const FACING_PROPERTY: &str = "facing";

/// Blockstate property naming which half of a double chest a block is.
pub const CHEST_TYPE_PROPERTY: &str = "type";

/// Side length, in texels, of the layout chest atlases are drawn on. A larger
/// atlas is the same layout scaled up.
pub const CHEST_ATLAS_SIZE: u32 = 64;

/// Prefix of the texture keys of tiles cut from chest atlases.
pub const CHEST_TILE_PREFIX: &str = "chest";

/// Folder, under `textures/`, of the entity textures cut into tiles.
pub const ENTITY_TEXTURE_DIR: &str = "entity";

/// Chest atlases, under `textures/`; one file per material and half.
pub const CHEST_TEXTURE_DIR: &str = "entity/chest";

/// Bed atlases, under `textures/`; one file per colour.
pub const BED_TEXTURE_DIR: &str = "entity/bed";

/// Side length, in texels, of the layout bed atlases are drawn on.
pub const BED_ATLAS_SIZE: u32 = 64;

/// Prefix of the texture keys of tiles cut from bed atlases.
pub const BED_TILE_PREFIX: &str = "bed";

/// Blockstate property naming which half of a bed a block is.
pub const BED_PART_PROPERTY: &str = "part";

/// Value of [`BED_PART_PROPERTY`] for the half with the pillow.
pub const BED_HEAD_PART: &str = "head";

/// Value of [`BED_PART_PROPERTY`] for the other half, a bed's default state.
pub const BED_FOOT_PART: &str = "foot";

/// Colour of a bed or banner whose block name carries none.
pub const DEFAULT_DYE_COLOUR: &str = "white";

/// Atlas of an inactive conduit's shell, under `textures/`.
pub const CONDUIT_TEXTURE: &str = "entity/conduit/base";

/// Width, in texels, of the layout the conduit atlas is drawn on.
pub const CONDUIT_ATLAS_SIZE: u32 = 32;

/// Prefix of the texture keys of tiles cut from the conduit atlas.
pub const CONDUIT_TILE_PREFIX: &str = "conduit";

/// Mob head atlases, under `textures/`.
pub const SKELETON_SKULL_TEXTURE: &str = "entity/skeleton/skeleton";
pub const WITHER_SKELETON_SKULL_TEXTURE: &str = "entity/skeleton/wither_skeleton";
pub const ZOMBIE_HEAD_TEXTURE: &str = "entity/zombie/zombie";
pub const CREEPER_HEAD_TEXTURE: &str = "entity/creeper/creeper";
pub const PIGLIN_HEAD_TEXTURE: &str = "entity/piglin/piglin";
pub const DRAGON_HEAD_TEXTURE: &str = "entity/enderdragon/dragon";
/// The default skin a player head without an owner shows: `wide/steve` from
/// 1.19.3, `steve` before.
pub const PLAYER_HEAD_TEXTURES: [&str; 2] = ["entity/player/wide/steve", "entity/steve"];

/// Width, in texels, of the layout mob head atlases other than the dragon's
/// are drawn on.
pub const HEAD_ATLAS_SIZE: u32 = 64;

/// Width, in texels, of the layout the dragon atlas is drawn on.
pub const DRAGON_ATLAS_SIZE: u32 = 256;

/// Prefix of the texture keys of tiles cut from mob head atlases.
pub const HEAD_TILE_PREFIX: &str = "head";

/// Height, in model units, at which a floor dragon head's model origin sits:
/// vanilla lifts the 3/4-scale model by 0.374375 blocks.
pub const DRAGON_HEAD_LIFT: f32 = 5.99;

/// Scale vanilla draws the dragon head at.
pub const DRAGON_HEAD_SCALE: f32 = 0.75;

/// How much the hat layer of a zombie or player head stands off the head.
pub const HEAD_HAT_INFLATE: f32 = 0.25;

/// Block texture of the top of a bell: 8x8 texels at the top-left.
pub const BELL_TOP_TEXTURE: &str = "bell_top";

/// Block texture of a side of a bell: the 6x7 body above the 8x2 lip.
pub const BELL_SIDE_TEXTURE: &str = "bell_side";

/// Block texture of the underside of a bell: 8x8 texels at the top-left.
pub const BELL_BOTTOM_TEXTURE: &str = "bell_bottom";

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
