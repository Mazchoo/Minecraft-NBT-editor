use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Reference to a cached model JSON plus extraction-time metadata.
///
/// Geometry is not stored here; [`crate::block_models::ModelLibrary::resolve`] reads
/// `elements_from` on demand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockModel {
    /// File name within `./model_cache/`, including extension.
    file_name: String,
    /// Stem of the `parent` model id, e.g. `cube_column`. `None` for root models.
    parent: Option<String>,
    /// Texture slot -> reference: a `TextureLibrary` key or a `#slot` indirection.
    textures: BTreeMap<String, String>,
    /// Stem of the nearest model in the chain that declares `elements`.
    /// `None` means the chain has no geometry.
    elements_from: Option<String>,
}

impl BlockModel {
    pub(crate) fn new(
        file_name: String,
        parent: Option<String>,
        textures: BTreeMap<String, String>,
        elements_from: Option<String>,
    ) -> Self {
        Self {
            file_name,
            parent,
            textures,
            elements_from,
        }
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    pub fn textures(&self) -> &BTreeMap<String, String> {
        &self.textures
    }

    pub fn elements_from(&self) -> Option<&str> {
        self.elements_from.as_deref()
    }

    pub(crate) fn set_elements_from(&mut self, value: Option<String>) {
        self.elements_from = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_model_serializes_like_the_manifest() {
        let model = BlockModel::new(
            "cube_all.json".into(),
            Some("cube".into()),
            BTreeMap::from([
                ("down".into(), "#all".into()),
                ("particle".into(), "#all".into()),
            ]),
            Some("cube".into()),
        );
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "file_name": "cube_all.json",
                "parent": "cube",
                "textures": { "down": "#all", "particle": "#all" },
                "elements_from": "cube"
            })
        );
    }

    #[test]
    fn root_model_serializes_null_parent_and_elements() {
        let model = BlockModel::new(
            "chest.json".into(),
            None,
            BTreeMap::from([("particle".into(), "oak_planks".into())]),
            None,
        );
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["parent"], serde_json::Value::Null);
        assert_eq!(json["elements_from"], serde_json::Value::Null);
    }
}
