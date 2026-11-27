use std::collections::BTreeMap;

use crate::constants::DEFAULT_NAMESPACE;

/// One entry of a structure's `palette`: the block's `Name` and `Properties`,
/// exactly as they are written to the NBT file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockState {
    /// Namespaced block id, e.g. `minecraft:oak_log`.
    pub name: String,
    /// Blockstate properties, e.g. `axis` -> `x`. Empty for the block's
    /// default state, in which case the NBT entry has no `Properties` tag.
    pub properties: BTreeMap<String, String>,
}

impl BlockState {
    /// The default state of block `id`. An id without a namespace, such as a
    /// right-sidebar model stem, is taken to be in `minecraft`.
    pub fn default_of(id: &str) -> Self {
        let name = if id.contains(':') {
            id.to_owned()
        } else {
            format!("{DEFAULT_NAMESPACE}:{id}")
        };
        Self {
            name,
            properties: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_of_adds_the_minecraft_namespace_only_when_missing() {
        assert_eq!(BlockState::default_of("stone").name, "minecraft:stone");
        assert_eq!(
            BlockState::default_of("create:cogwheel").name,
            "create:cogwheel"
        );
        assert!(BlockState::default_of("stone").properties.is_empty());
    }
}
