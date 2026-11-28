//! Block keys the editor gives behaviour their models do not describe: tints,
//! hand-built geometry, shared models and blocks that draw nothing.

macro_rules! custom_block_keys {
    ($($(#[$doc:meta])* $variant:ident => $key:literal,)*) => {
        /// A block key with custom behaviour. Every such key is named here and
        /// nowhere else.
        ///
        /// Most variants are one block. A family variant ([`Self::is_family`])
        /// covers every block whose key ends in the variant's key, one per
        /// wood or colour: [`Self::WallSign`] is `oak_wall_sign`,
        /// `birch_wall_sign` and so on.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CustomBlockKey {
            $($(#[$doc])* $variant,)*
        }

        impl CustomBlockKey {
            /// Every variant. Families run from the longest key to the
            /// shortest, so the first one that fits a name is the right one.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// The key without its namespace. For a family, the ending its
            /// blocks share.
            pub const fn key(self) -> &'static str {
                match self {
                    $(Self::$variant => $key,)*
                }
            }
        }
    };
}

custom_block_keys! {
    Air => "air",
    CaveAir => "cave_air",
    VoidAir => "void_air",
    Barrier => "barrier",
    StructureVoid => "structure_void",
    Light => "light",

    Chest => "chest",
    TrappedChest => "trapped_chest",
    EnderChest => "ender_chest",

    /// Its models hold only the frame.
    Bell => "bell",

    /// Its texture is drawn on hanging signs. Named `iron_chain` from 1.21.9.
    Chain => "chain",
    IronChain => "iron_chain",

    MelonStem => "melon_stem",
    PumpkinStem => "pumpkin_stem",
    AttachedMelonStem => "attached_melon_stem",
    AttachedPumpkinStem => "attached_pumpkin_stem",

    RedstoneWire => "redstone_wire",

    GrassBlock => "grass_block",
    ShortGrass => "short_grass",
    TallGrass => "tall_grass",

    Fern => "fern",
    LargeFern => "large_fern",
    PottedFern => "potted_fern",
    Vine => "vine",
    OakLeaves => "oak_leaves",
    SpruceLeaves => "spruce_leaves",
    BirchLeaves => "birch_leaves",
    JungleLeaves => "jungle_leaves",
    AcaciaLeaves => "acacia_leaves",
    DarkOakLeaves => "dark_oak_leaves",
    MangroveLeaves => "mangrove_leaves",
    CherryLeaves => "cherry_leaves",

    WaterCauldron => "water_cauldron",

    /// Family: one per wood.
    WallHangingSign => "wall_hanging_sign",
    /// Family: one per wood.
    HangingSign => "hanging_sign",
    /// Family: one per wood.
    WallSign => "wall_sign",
    /// Family: one per wood.
    Sign => "sign",
    /// Family: one per colour.
    WallBanner => "wall_banner",
    /// Family: one per colour. Its key alone is the model all banners share.
    Banner => "banner",
}

impl CustomBlockKey {
    /// Recognises a block name (`minecraft:oak_wall_sign`). Single blocks are
    /// tried before families.
    pub fn from_name(name: &str) -> Option<Self> {
        let stem = block_stem(name);
        let single = |key: &&Self| !key.is_family() && key.key() == stem;
        let family = |key: &&Self| key.member(stem).is_some();
        Self::ALL
            .iter()
            .find(single)
            .or_else(|| Self::ALL.iter().find(family))
            .copied()
    }

    /// Whether this variant covers one block per wood or colour.
    pub const fn is_family(self) -> bool {
        matches!(
            self,
            Self::WallHangingSign
                | Self::HangingSign
                | Self::WallSign
                | Self::Sign
                | Self::WallBanner
                | Self::Banner
        )
    }

    /// The wood or colour that picks `name` out of this family: `oak` for
    /// `oak_wall_sign`, empty for the family's key alone. `None` when `name`
    /// is not in this family.
    pub fn member(self, name: &str) -> Option<&str> {
        if !self.is_family() {
            return None;
        }
        let rest = block_stem(name).strip_suffix(self.key())?;
        if rest.is_empty() {
            Some(rest)
        } else {
            rest.strip_suffix('_')
        }
    }

    /// The key of this family's block for one wood or colour:
    /// `Sign.of_member("oak")` is `oak_sign`.
    pub fn of_member(self, member: &str) -> String {
        format!("{member}_{}", self.key())
    }

    /// Whether the block stands for an empty cell.
    pub const fn is_air(self) -> bool {
        matches!(self, Self::Air | Self::CaveAir | Self::VoidAir)
    }
}

/// A block name without its namespace or folder.
fn block_stem(name: &str) -> &str {
    let path = name.rsplit_once(':').map(|(_, rest)| rest).unwrap_or(name);
    path.rsplit_once('/').map(|(_, stem)| stem).unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_blocks_match_their_whole_key() {
        assert_eq!(
            CustomBlockKey::from_name("minecraft:melon_stem"),
            Some(CustomBlockKey::MelonStem)
        );
        assert_eq!(
            CustomBlockKey::from_name("attached_melon_stem"),
            Some(CustomBlockKey::AttachedMelonStem)
        );
        assert_eq!(CustomBlockKey::from_name("minecraft:mushroom_stem"), None);
        assert_eq!(CustomBlockKey::from_name("minecraft:stone"), None);
    }

    #[test]
    fn families_match_the_longest_key() {
        let cases = [
            ("minecraft:oak_wall_hanging_sign", CustomBlockKey::WallHangingSign),
            ("minecraft:oak_hanging_sign", CustomBlockKey::HangingSign),
            ("minecraft:dark_oak_wall_sign", CustomBlockKey::WallSign),
            ("minecraft:dark_oak_sign", CustomBlockKey::Sign),
            ("minecraft:light_blue_wall_banner", CustomBlockKey::WallBanner),
            ("minecraft:light_blue_banner", CustomBlockKey::Banner),
            ("banner", CustomBlockKey::Banner),
        ];
        for (name, key) in cases {
            assert_eq!(CustomBlockKey::from_name(name), Some(key), "{name}");
        }
        assert_eq!(CustomBlockKey::from_name("minecraft:design"), None);
    }

    #[test]
    fn member_is_the_wood_or_colour() {
        let key = CustomBlockKey::WallSign;
        assert_eq!(key.member("minecraft:dark_oak_wall_sign"), Some("dark_oak"));
        assert_eq!(key.member("minecraft:dark_oak_wall_banner"), None);
        assert_eq!(CustomBlockKey::Banner.member("banner"), Some(""));
        assert_eq!(CustomBlockKey::Chest.member("chest"), None);
        assert_eq!(CustomBlockKey::Sign.of_member("dark_oak"), "dark_oak_sign");
    }
}
