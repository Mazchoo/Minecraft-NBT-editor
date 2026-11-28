#![allow(dead_code)] // consumed by the viewport once textured block drawing lands

use super::banner::BannerKind;
use super::block::BasicBlock;
use super::chest::ChestKind;
use super::sign::SignKind;
use crate::custom_block_keys::CustomBlockKey;

/// Classification of a resolved block. Families are opt-in specialisations;
/// anything with geometry that no family claims is [`Self::Elements`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    /// Full cube from `cube_all` / `cube_column`. Fast path, skips element parsing.
    Basic(BasicBlock),
    /// Anything else with geometry: drawn from the element list.
    Elements,
    /// A sign: no model geometry, drawn from hand-built boxes.
    Sign(SignKind),
    /// A banner: no model geometry, drawn from hand-built boxes.
    Banner(BannerKind),
    /// A chest: no model geometry, drawn from hand-built boxes.
    Chest(ChestKind),
    /// No geometry anywhere in the model chain: drawn as an inset placeholder.
    BlockEntity,
    /// Deliberately draws nothing: air, barrier, structure void.
    Invisible,
}

impl BlockType {
    /// Classifies a resolved block. Degenerate cases first, then families,
    /// then [`Self::Elements`].
    pub fn classify(name: &str, parent_stem: Option<&str>, has_geometry: bool) -> Self {
        if is_invisible(name) {
            return Self::Invisible;
        }
        if !has_geometry {
            if let Some(kind) = SignKind::from_name(name) {
                return Self::Sign(kind);
            }
            if let Some(kind) = BannerKind::from_name(name) {
                return Self::Banner(kind);
            }
            return match ChestKind::from_name(name) {
                Some(kind) => Self::Chest(kind),
                None => Self::BlockEntity,
            };
        }
        match parent_stem.and_then(BasicBlock::from_parent) {
            Some(basic) => Self::Basic(basic),
            None => Self::Elements,
        }
    }
}

fn is_invisible(name: &str) -> bool {
    CustomBlockKey::from_name(name).is_some_and(|key| {
        key.is_air()
            || matches!(
                key,
                CustomBlockKey::Barrier | CustomBlockKey::StructureVoid | CustomBlockKey::Light
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_order_invisible_then_entity_then_family() {
        assert_eq!(
            BlockType::classify("minecraft:air", Some("cube_all"), true),
            BlockType::Invisible
        );
        assert_eq!(
            BlockType::classify("minecraft:shulker_box", None, false),
            BlockType::BlockEntity
        );
        assert_eq!(
            BlockType::classify("minecraft:trapped_chest", None, false),
            BlockType::Chest(ChestKind::Trapped)
        );
        assert_eq!(
            BlockType::classify("minecraft:oak_wall_sign", None, false),
            BlockType::Sign(SignKind::Wall)
        );
        assert_eq!(
            BlockType::classify("minecraft:red_wall_banner", None, false),
            BlockType::Banner(BannerKind::Wall)
        );
        assert_eq!(
            BlockType::classify("minecraft:stone", Some("cube_all"), true),
            BlockType::Basic(BasicBlock::All)
        );
        assert_eq!(
            BlockType::classify("minecraft:quartz_pillar", Some("cube_column"), true),
            BlockType::Basic(BasicBlock::Column)
        );
        assert_eq!(
            BlockType::classify("minecraft:oak_stairs", Some("stairs"), true),
            BlockType::Elements
        );
    }
}
