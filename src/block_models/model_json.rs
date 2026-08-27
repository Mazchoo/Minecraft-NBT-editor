use std::collections::BTreeMap;

use serde::Deserialize;

/// On-disk Minecraft block model JSON.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // parent/textures live on the manifest; resolve only reads `elements`
pub struct ModelFile {
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub textures: BTreeMap<String, String>,
    #[serde(default)]
    pub elements: Vec<ModelElement>,
}

/// One cuboid in a model JSON. Coordinates are on Minecraft's 0-16 scale.
#[derive(Debug, Clone, Deserialize)]
pub struct ModelElement {
    pub from: [f32; 3],
    pub to: [f32; 3],
    #[serde(default)]
    pub rotation: Option<ModelRotation>,
    #[serde(default)]
    pub faces: BTreeMap<String, ModelFace>,
    #[serde(default = "default_true")]
    pub shade: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // fields are read by the mesh builder via `ElementRotation::from_model`
pub struct ModelRotation {
    pub origin: [f32; 3],
    pub axis: String,
    pub angle: f32,
    #[serde(default)]
    pub rescale: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelFace {
    #[serde(default)]
    pub uv: Option<[f32; 4]>,
    pub texture: String,
    #[serde(default)]
    pub cullface: Option<String>,
    #[serde(default)]
    pub rotation: Option<u32>,
    #[serde(default)]
    pub tintindex: Option<i32>,
}

fn default_true() -> bool {
    true
}

/// Reduces a Minecraft asset id to its file stem.
///
/// `minecraft:block/cube_column` and `block/cube_column` both become `cube_column`.
/// `#slot` indirections are returned unchanged.
pub(crate) fn asset_stem(id: &str) -> String {
    if id.starts_with('#') {
        return id.to_owned();
    }
    let path = id.rsplit_once(':').map(|(_, rest)| rest).unwrap_or(id);
    path.rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(path)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_stem_strips_namespace_and_block_prefix() {
        assert_eq!(asset_stem("minecraft:block/cube_column"), "cube_column");
        assert_eq!(asset_stem("block/cube_column"), "cube_column");
        assert_eq!(asset_stem("cube_column"), "cube_column");
        assert_eq!(asset_stem("minecraft:quartz_pillar"), "quartz_pillar");
        assert_eq!(asset_stem("#all"), "#all");
        assert_eq!(
            asset_stem("minecraft:block/quartz_pillar_top"),
            "quartz_pillar_top"
        );
    }

    #[test]
    fn model_file_deserializes_parent_textures_and_elements() {
        let json = r##"{
            "parent": "minecraft:block/cube",
            "textures": { "all": "minecraft:block/stone" },
            "elements": [{
                "from": [0, 0, 0],
                "to": [16, 16, 16],
                "faces": { "up": { "texture": "#all", "cullface": "up" } }
            }]
        }"##;
        let model: ModelFile = serde_json::from_str(json).unwrap();
        assert_eq!(model.parent.as_deref(), Some("minecraft:block/cube"));
        assert_eq!(model.textures["all"], "minecraft:block/stone");
        assert_eq!(model.elements.len(), 1);
        assert_eq!(model.elements[0].from, [0.0, 0.0, 0.0]);
        assert_eq!(model.elements[0].faces["up"].texture, "#all");
        assert!(model.elements[0].shade);
    }
}
