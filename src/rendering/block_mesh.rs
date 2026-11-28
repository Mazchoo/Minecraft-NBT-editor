use glam::{IVec3, Vec3};

use crate::models::block::{Block, FACE_CORNERS, Face, element_covers_face};
use crate::models::block_type::BlockType;
use crate::models::element::{Element, ElementFace, StateRotation};

/// GPU vertex for a block face corner.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
pub struct BlockVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    /// Layer index into the block texture array.
    pub layer: u32,
    /// Tint multiplied by directional shading. White when untinted and unshaded.
    pub color: [u8; 4],
}

/// CPU-side vertex and index buffers produced by placing blocks into a mesh.
#[derive(Debug, Clone, Default)]
pub struct BlockMesh {
    pub vertices: Vec<BlockVertex>,
    pub indices: Vec<u32>,
}

impl BlockMesh {
    /// Appends `block` occupying the cell whose min corner is `origin`.
    ///
    /// `layer_of` maps a `TextureLibrary` key to a texture-array layer. Unknown
    /// keys (including the empty missing-texture key) should return 0.
    #[cfg(test)]
    pub fn place(&mut self, block: &Block, origin: IVec3, layer_of: impl Fn(&str) -> u32) {
        self.place_culled(block, origin, layer_of, |_| true);
    }

    /// Like [`Self::place`], but skips every face `visible` rejects.
    ///
    /// `visible` receives the side of the cell a face lies on or is culled
    /// against, in world directions, or `None` for a face strictly inside the
    /// cell. `Visibility::face_visible` answers it for a whole structure.
    pub fn place_culled(
        &mut self,
        block: &Block,
        origin: IVec3,
        layer_of: impl Fn(&str) -> u32,
        visible: impl Fn(Option<Face>) -> bool,
    ) {
        match block.block_type() {
            BlockType::Invisible => {}
            BlockType::Basic(_) => emit_basic(self, block, origin, layer_of, visible),
            BlockType::Elements
            | BlockType::Sign(_)
            | BlockType::Banner(_)
            | BlockType::Chest(_)
            | BlockType::Head(_)
            | BlockType::Bed
            | BlockType::Conduit
            | BlockType::EndPortal(_)
            | BlockType::BlockEntity => {
                emit_parts(self, block, origin, layer_of, visible);
            }
        }
    }
}

fn emit_basic(
    mesh: &mut BlockMesh,
    block: &Block,
    origin: IVec3,
    layer_of: impl Fn(&str) -> u32,
    visible: impl Fn(Option<Face>) -> bool,
) {
    let origin = origin.as_vec3();
    let Some(part) = block.parts().first() else {
        return;
    };
    for (face, corners) in FACE_CORNERS {
        if !visible(Some(face)) {
            continue;
        }
        let texture = face_texture(part.elements(), face);
        let tint = face_tint(block, part.elements(), face);
        let shade = shade_factor(part.elements().first(), face);
        let color = shade_color(tint, shade);
        let layer = layer_of(texture);
        emit_quad(mesh, origin, corners, layer, color);
    }
}

fn emit_parts(
    mesh: &mut BlockMesh,
    block: &Block,
    origin: IVec3,
    layer_of: impl Fn(&str) -> u32,
    visible: impl Fn(Option<Face>) -> bool,
) {
    let origin = origin.as_vec3();
    for part in block.parts() {
        for element in part.elements() {
            emit_element(
                mesh,
                element,
                origin,
                part.rotation(),
                part.uvlock(),
                block,
                &layer_of,
                &visible,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_element(
    mesh: &mut BlockMesh,
    element: &Element,
    origin: Vec3,
    state: StateRotation,
    uvlock: bool,
    block: &Block,
    layer_of: &impl Fn(&str) -> u32,
    visible: &impl Fn(Option<Face>) -> bool,
) {
    for (face, unit_corners) in FACE_CORNERS {
        let Some(face_data) = element.faces()[face.index()].as_ref() else {
            continue;
        };
        if !element.face_has_area(face) {
            continue;
        }
        // A face is tested against a neighbour when the model declares a
        // cullface, or when it spans a whole side of the cell without one.
        let toward = face_data
            .cullface()
            .or_else(|| element_covers_face(element, face).then_some(face))
            .map(|dir| dir.after_geometry_rotation(state));
        if !visible(toward) {
            continue;
        }
        let shade = if element.shade() { face.shade() } else { 1.0 };
        let color = shade_color(block.tint(face_data.tint_index()), shade);
        let layer = layer_of(face_data.texture());
        let uv_rotation = uv_rotation_with_lock(face, face_data.uv_rotation(), state, uvlock);
        let mut corners = [[0.0f32; 5]; 4];
        for (i, unit) in unit_corners.iter().enumerate() {
            let mut p = map_unit_to_element(unit, element.from(), element.to());
            if let Some(rotation) = element.rotation() {
                p = rotation.apply(p);
            }
            p = state.apply(p);
            let world = origin + p / 16.0;
            let uv = apply_uv(face_data, unit[3], unit[4], uv_rotation);
            corners[i] = [world.x, world.y, world.z, uv[0], uv[1]];
        }
        emit_quad_abs(mesh, corners, layer, color);
    }
}

fn emit_quad(
    mesh: &mut BlockMesh,
    origin: Vec3,
    corners: [[f32; 5]; 4],
    layer: u32,
    color: [u8; 4],
) {
    let mut abs = [[0.0f32; 5]; 4];
    for (i, c) in corners.iter().enumerate() {
        abs[i] = [
            origin.x + c[0],
            origin.y + c[1],
            origin.z + c[2],
            c[3],
            c[4],
        ];
    }
    emit_quad_abs(mesh, abs, layer, color);
}

fn emit_quad_abs(mesh: &mut BlockMesh, corners: [[f32; 5]; 4], layer: u32, color: [u8; 4]) {
    let base = mesh.vertices.len() as u32;
    for c in corners {
        mesh.vertices.push(BlockVertex {
            position: [c[0], c[1], c[2]],
            uv: [c[3], c[4]],
            layer,
            color,
        });
    }
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn map_unit_to_element(unit: &[f32; 5], from: Vec3, to: Vec3) -> Vec3 {
    Vec3::new(
        from.x + (to.x - from.x) * unit[0],
        from.y + (to.y - from.y) * unit[1],
        from.z + (to.z - from.z) * unit[2],
    )
}

fn apply_uv(face: &ElementFace, s: f32, t: f32, uv_rotation: u16) -> [f32; 2] {
    let (s, t) = match uv_rotation % 360 {
        90 => (t, 1.0 - s),
        180 => (1.0 - s, 1.0 - t),
        270 => (1.0 - t, s),
        _ => (s, t),
    };
    let [u1, v1, u2, v2] = face.uv();
    [(u1 + (u2 - u1) * s) / 16.0, (v1 + (v2 - v1) * t) / 16.0]
}

fn uv_rotation_with_lock(face: Face, uv_rotation: u16, state: StateRotation, uvlock: bool) -> u16 {
    if !uvlock {
        return uv_rotation;
    }
    let extra = match face {
        Face::Up => state.y(),
        Face::Down => (360 - state.y() % 360) % 360,
        _ => 0,
    };
    (uv_rotation + extra) % 360
}

fn face_texture(elements: &[Element], face: Face) -> &str {
    elements
        .first()
        .and_then(|el| el.faces()[face.index()].as_ref())
        .map(ElementFace::texture)
        .unwrap_or("")
}

fn face_tint(block: &Block, elements: &[Element], face: Face) -> [u8; 4] {
    let tint_index = elements
        .first()
        .and_then(|el| el.faces()[face.index()].as_ref())
        .and_then(ElementFace::tint_index);
    block.tint(tint_index)
}

fn shade_factor(element: Option<&Element>, face: Face) -> f32 {
    match element {
        Some(el) if !el.shade() => 1.0,
        _ => face.shade(),
    }
}

fn shade_color(tint: [u8; 4], shade: f32) -> [u8; 4] {
    [
        (tint[0] as f32 * shade) as u8,
        (tint[1] as f32 * shade) as u8,
        (tint[2] as f32 * shade) as u8,
        tint[3],
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::SystemTime;

    use glam::IVec3;

    use crate::block_models::ModelLibrary;
    use crate::models::block::Block;
    use crate::models::block_type::BlockType;

    use super::*;

    fn place_once(block: &Block, origin: IVec3) -> BlockMesh {
        let mut mesh = BlockMesh::default();
        mesh.place(block, origin, |_| 0);
        mesh
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "minecraft-nbt-editor-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_model_cache(dir: &std::path::Path) {
        fs::write(
            dir.join("cube.json"),
            r##"{
                "elements": [{
                    "from": [0, 0, 0],
                    "to": [16, 16, 16],
                    "faces": {
                        "down":  { "texture": "#down", "cullface": "down" },
                        "up":    { "texture": "#up", "cullface": "up" },
                        "north": { "texture": "#north" },
                        "south": { "texture": "#south" },
                        "west":  { "texture": "#west" },
                        "east":  { "texture": "#east" }
                    }
                }]
            }"##,
        )
        .unwrap();
        fs::write(
            dir.join("cross.json"),
            r##"{
                "elements": [
                    {
                        "from": [0.8, 0, 8],
                        "to": [15.2, 16, 8],
                        "faces": {
                            "north": { "texture": "#cross" },
                            "south": { "texture": "#cross" }
                        }
                    }
                ]
            }"##,
        )
        .unwrap();
        fs::write(
            dir.join(crate::constants::MODELS_MANIFEST_FILE),
            r##"{
                "cube": {
                    "file_name": "cube.json",
                    "parent": null,
                    "textures": {},
                    "elements_from": "cube"
                },
                "cube_all": {
                    "file_name": "cube_all.json",
                    "parent": "cube",
                    "textures": {
                        "down": "#all", "east": "#all", "north": "#all", "particle": "#all",
                        "south": "#all", "up": "#all", "west": "#all"
                    },
                    "elements_from": "cube"
                },
                "cube_column": {
                    "file_name": "cube_column.json",
                    "parent": "cube",
                    "textures": {
                        "down": "#end", "up": "#end",
                        "north": "#side", "south": "#side", "west": "#side", "east": "#side",
                        "particle": "#side"
                    },
                    "elements_from": "cube"
                },
                "stone": {
                    "file_name": "stone.json",
                    "parent": "cube_all",
                    "textures": { "all": "stone" },
                    "elements_from": "cube"
                },
                "quartz_pillar": {
                    "file_name": "quartz_pillar.json",
                    "parent": "cube_column",
                    "textures": { "end": "quartz_pillar_top", "side": "quartz_pillar" },
                    "elements_from": "cube"
                },
                "chest": {
                    "file_name": "chest.json",
                    "parent": null,
                    "textures": { "particle": "oak_planks" },
                    "elements_from": null
                },
                "shulker_box": {
                    "file_name": "shulker_box.json",
                    "parent": null,
                    "textures": { "particle": "shulker_box" },
                    "elements_from": null
                },
                "cross": {
                    "file_name": "cross.json",
                    "parent": null,
                    "textures": { "cross": "short_grass" },
                    "elements_from": "cross"
                }
            }"##,
        )
        .unwrap();
    }

    fn library() -> (PathBuf, ModelLibrary) {
        let dir = temp_dir("block-mesh");
        write_model_cache(&dir);
        let library = ModelLibrary::from_cache_dir(&dir);
        (dir, library)
    }

    fn resolve_block(
        library: &ModelLibrary,
        name: &str,
        properties: BTreeMap<String, String>,
    ) -> Block {
        Block::from_resolved(name, &library.resolve(name, &properties))
    }

    fn bounds(mesh: &BlockMesh) -> (Vec3, Vec3) {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for v in &mesh.vertices {
            min = min.min(Vec3::from_array(v.position));
            max = max.max(Vec3::from_array(v.position));
        }
        (min, max)
    }

    #[test]
    fn place_basic_cube_at_origin_is_unit_cube() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:stone", BTreeMap::new());
        assert_eq!(block.name(), "minecraft:stone");
        assert!(block.properties().is_empty());
        assert_eq!(block.texture_keys(), vec!["stone".to_owned()]);
        assert_eq!(block.parts()[0].model(), "stone");
        assert_eq!(
            block.block_type(),
            BlockType::Basic(crate::models::BasicBlock::All)
        );
        assert!(block.is_opaque());
        assert!(block.covers_full_face(crate::models::Face::Up));
        let mesh = place_once(&block, IVec3::ZERO);
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
        let (min, max) = bounds(&mesh);
        assert_eq!(min, Vec3::ZERO);
        assert_eq!(max, Vec3::ONE);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn place_basic_cube_at_offset_occupies_that_cell() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:stone", BTreeMap::new());
        let origin = IVec3::new(5, -2, 10);
        let mesh = place_once(&block, origin);
        let (min, max) = bounds(&mesh);
        assert_eq!(min, origin.as_vec3());
        assert_eq!(max, origin.as_vec3() + Vec3::ONE);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn place_two_blocks_appends_separate_cells() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:stone", BTreeMap::new());
        let mut mesh = BlockMesh::default();
        mesh.place(&block, IVec3::ZERO, |_| 0);
        mesh.place(&block, IVec3::new(1, 0, 0), |_| 0);
        assert_eq!(mesh.vertices.len(), 48);
        assert_eq!(mesh.indices.len(), 72);
        assert_eq!(
            mesh.indices[36], 24,
            "second block indices start after the first"
        );
        let first = &mesh.vertices[..24];
        let second = &mesh.vertices[24..];
        assert!(
            first.iter().all(|v| (0.0..=1.0).contains(&v.position[0])),
            "first block stays in x=[0,1]"
        );
        assert!(
            second.iter().all(|v| (1.0..=2.0).contains(&v.position[0])),
            "second block stays in x=[1,2]"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn place_culled_skips_rejected_faces() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:stone", BTreeMap::new());
        let mut mesh = BlockMesh::default();
        mesh.place_culled(&block, IVec3::ZERO, |_| 0, |face| face != Some(Face::Up));
        assert_eq!(mesh.vertices.len(), 20);
        assert_eq!(mesh.indices.len(), 30);
        assert!(
            mesh.vertices
                .chunks(4)
                .all(|quad| quad.iter().any(|v| v.position[1] < 0.5)),
            "no quad lies entirely on the top of the cell"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rotated_cube_culls_by_world_direction() {
        let (dir, library) = library();
        let mut properties = BTreeMap::new();
        properties.insert("axis".into(), "x".into());
        let block = resolve_block(&library, "minecraft:quartz_pillar", properties);
        assert_eq!(block.occlusion_mask(|_| false), 0b11_1111);
        assert_eq!(block.occlusion_mask(|_| true), 0);
        let mut mesh = BlockMesh::default();
        mesh.place_culled(&block, IVec3::ZERO, |_| 0, |face| face != Some(Face::East));
        assert_eq!(mesh.vertices.len(), 20);
        assert!(
            mesh.vertices
                .chunks(4)
                .all(|quad| quad.iter().any(|v| v.position[0] < 0.5)),
            "the quad on the east side of the cell is the one removed"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn occlusion_mask_follows_shape_and_texture_alpha() {
        let (dir, library) = library();
        let stone = resolve_block(&library, "minecraft:stone", BTreeMap::new());
        assert_eq!(stone.occlusion_mask(|_| false), 0b11_1111);
        assert_eq!(stone.occlusion_mask(|key| key == "stone"), 0);
        let cross = resolve_block(&library, "minecraft:cross", BTreeMap::new());
        assert_eq!(cross.occlusion_mask(|_| false), 0);
        let chest = resolve_block(&library, "minecraft:chest", BTreeMap::new());
        assert_eq!(chest.occlusion_mask(|_| false), 0);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn chest_draws_its_boxes_from_atlas_tiles() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:chest", BTreeMap::new());
        assert!(matches!(block.block_type(), BlockType::Chest(_)));
        assert!(
            block
                .texture_keys()
                .contains(&"chest_normal_lid_up".to_owned())
        );
        // Base, lid and lock, five drawn faces each.
        let mesh = place_once(&block, IVec3::new(2, 0, 0));
        assert_eq!(mesh.vertices.len(), 60);
        let (min, max) = bounds(&mesh);
        assert!(min.x > 2.0 && min.y >= 0.0 && min.z >= 0.0);
        assert!(max.x < 3.0 && max.y < 1.0 && max.z < 1.0);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn invisible_places_nothing() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:air", BTreeMap::new());
        assert_eq!(block.block_type(), BlockType::Invisible);
        let mesh = place_once(&block, IVec3::ZERO);
        assert!(mesh.vertices.is_empty());
        assert!(mesh.indices.is_empty());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn block_entity_placeholder_is_inset_inside_the_cell() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:shulker_box", BTreeMap::new());
        assert_eq!(block.block_type(), BlockType::BlockEntity);
        assert_eq!(block.particle_texture(), Some("shulker_box"));
        let mesh = place_once(&block, IVec3::new(2, 0, 0));
        assert_eq!(mesh.vertices.len(), 24);
        let (min, max) = bounds(&mesh);
        assert!(min.x > 2.0 && min.y > 0.0 && min.z > 0.0);
        assert!(max.x < 3.0 && max.y < 1.0 && max.z < 1.0);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn zero_thickness_plane_emits_two_faces() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:cross", BTreeMap::new());
        assert_eq!(block.block_type(), BlockType::Elements);
        assert!(!block.is_opaque());
        assert!(!block.covers_full_face(crate::models::Face::Up));
        let mesh = place_once(&block, IVec3::ZERO);
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.indices.len(), 12);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn axis_x_column_rotates_geometry_onto_x() {
        let (dir, library) = library();
        let mut properties = BTreeMap::new();
        properties.insert("axis".into(), "x".into());
        let block = resolve_block(&library, "minecraft:quartz_pillar", properties);
        assert_eq!(block.block_type(), BlockType::Elements);
        assert!(!block.parts()[0].rotation().is_identity());
        let mesh = place_once(&block, IVec3::ZERO);
        assert_eq!(mesh.vertices.len(), 24);
        let (min, max) = bounds(&mesh);
        assert!(
            min.abs().max_element() < 1e-5,
            "still occupies the unit cell, got min={min}"
        );
        assert!(
            (max - Vec3::ONE).abs().max_element() < 1e-5,
            "still occupies the unit cell, got max={max}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn west_face_winding_points_outward() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:stone", BTreeMap::new());
        let mesh = place_once(&block, IVec3::ZERO);
        // First quad is West; first triangle should have a -X normal.
        let a = Vec3::from_array(mesh.vertices[0].position);
        let b = Vec3::from_array(mesh.vertices[1].position);
        let c = Vec3::from_array(mesh.vertices[2].position);
        let normal = (b - a).cross(c - a);
        assert!(normal.x < 0.0);
        assert!(normal.y.abs() < 1e-5);
        assert!(normal.z.abs() < 1e-5);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn directional_shading_darkens_down_faces() {
        let (dir, library) = library();
        let block = resolve_block(&library, "minecraft:stone", BTreeMap::new());
        let mesh = place_once(&block, IVec3::ZERO);
        // FACE_CORNERS order: West, East, Down, Up, North, South — 4 verts each.
        let down = mesh.vertices[8].color;
        let up = mesh.vertices[12].color;
        assert_eq!(down[0], 127); // 255 * 0.5
        assert_eq!(up[0], 255);
        fs::remove_dir_all(dir).ok();
    }
}
