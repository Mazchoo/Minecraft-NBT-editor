#![allow(dead_code)] // consumed by the viewport once textured block drawing lands

use super::block::BasicBlock;

/// Classification of a resolved block. Families are opt-in specialisations;
/// anything with geometry that no family claims is [`Self::Elements`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    /// Full cube from `cube_all` / `cube_column`. Fast path, skips element parsing.
    Basic(BasicBlock),
    /// Anything else with geometry: drawn from the element list.
    Elements,
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
            return Self::BlockEntity;
        }
        match parent_stem.and_then(BasicBlock::from_parent) {
            Some(basic) => Self::Basic(basic),
            None => Self::Elements,
        }
    }
}

fn is_invisible(name: &str) -> bool {
    matches!(
        block_stem(name),
        "air" | "cave_air" | "void_air" | "barrier" | "structure_void" | "light"
    )
}

fn block_stem(name: &str) -> &str {
    let path = name.rsplit_once(':').map(|(_, rest)| rest).unwrap_or(name);
    path.rsplit_once('/').map(|(_, stem)| stem).unwrap_or(path)
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
            BlockType::classify("minecraft:chest", None, false),
            BlockType::BlockEntity
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
