#![allow(dead_code)] // consumed by the viewport once textured block drawing lands

use std::collections::BTreeMap;

use glam::Vec3;

use crate::block_models::{ResolvedBlock, ResolvedElement, ResolvedFace, ResolvedPart};

use super::block_type::BlockType;
use super::element::{
    Axis, Element, ElementFace, ElementRotation, StateRotation, default_uv, tint_color,
};

/// Inset on each side of a block-entity placeholder, in model units (0-16).
const BLOCK_ENTITY_INSET: f32 = 1.0;

/// Four corners of (x, y, z, u, v) per face, counter-clockwise from outside.
/// Positions are in the unit cube; UVs put `v = 0` at the top of the texture.
pub(crate) const FACE_CORNERS: [(Face, [[f32; 5]; 4]); 6] = [
    (
        Face::West,
        [
            [0., 0., 0., 1., 1.],
            [0., 0., 1., 0., 1.],
            [0., 1., 1., 0., 0.],
            [0., 1., 0., 1., 0.],
        ],
    ),
    (
        Face::East,
        [
            [1., 0., 1., 1., 1.],
            [1., 0., 0., 0., 1.],
            [1., 1., 0., 0., 0.],
            [1., 1., 1., 1., 0.],
        ],
    ),
    (
        Face::Down,
        [
            [0., 0., 0., 0., 1.],
            [1., 0., 0., 1., 1.],
            [1., 0., 1., 1., 0.],
            [0., 0., 1., 0., 0.],
        ],
    ),
    (
        Face::Up,
        [
            [0., 1., 1., 0., 1.],
            [1., 1., 1., 1., 1.],
            [1., 1., 0., 1., 0.],
            [0., 1., 0., 0., 0.],
        ],
    ),
    (
        Face::North,
        [
            [1., 0., 0., 0., 1.],
            [0., 0., 0., 1., 1.],
            [0., 1., 0., 1., 0.],
            [1., 1., 0., 0., 0.],
        ],
    ),
    (
        Face::South,
        [
            [0., 0., 1., 0., 1.],
            [1., 0., 1., 1., 1.],
            [1., 1., 1., 1., 0.],
            [0., 1., 1., 0., 0.],
        ],
    ),
];

/// A cube face. Index order matches [`Element::faces`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl Face {
    pub const ALL: [Face; 6] = [
        Face::Down,
        Face::Up,
        Face::North,
        Face::South,
        Face::West,
        Face::East,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "down" => Some(Self::Down),
            "up" => Some(Self::Up),
            "north" => Some(Self::North),
            "south" => Some(Self::South),
            "west" => Some(Self::West),
            "east" => Some(Self::East),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Up => "up",
            Self::North => "north",
            Self::South => "south",
            Self::West => "west",
            Self::East => "east",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// This face's bit in a per-cell occlusion mask.
    pub fn bit(self) -> u8 {
        1 << self.index()
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Down => Self::Up,
            Self::Up => Self::Down,
            Self::North => Self::South,
            Self::South => Self::North,
            Self::West => Self::East,
            Self::East => Self::West,
        }
    }

    /// The world direction this model-space face points after `rotation`,
    /// derived from the same transform the mesh applies to vertices so culling
    /// always agrees with the emitted geometry.
    pub(crate) fn after_geometry_rotation(self, rotation: StateRotation) -> Self {
        if rotation.is_identity() {
            return self;
        }
        let centre = Vec3::splat(8.0);
        let normal = rotation.apply(centre + self.offset().as_vec3()) - centre;
        Self::ALL
            .into_iter()
            .find(|face| face.offset().as_vec3().dot(normal) > 0.5)
            .unwrap_or(self)
    }

    /// Minecraft directional shading when the element has `shade: true`.
    pub fn shade(self) -> f32 {
        match self {
            Self::Down => 0.5,
            Self::Up => 1.0,
            Self::North | Self::South => 0.8,
            Self::West | Self::East => 0.6,
        }
    }

    /// Neighbour cell offset in this direction.
    pub fn offset(self) -> glam::IVec3 {
        match self {
            Self::Down => glam::IVec3::NEG_Y,
            Self::Up => glam::IVec3::Y,
            Self::North => glam::IVec3::NEG_Z,
            Self::South => glam::IVec3::Z,
            Self::West => glam::IVec3::NEG_X,
            Self::East => glam::IVec3::X,
        }
    }

    /// Four corners of this face of the unit cube at `origin`, shifted along the
    /// outward normal by `outset` so a highlight sits just outside the surface.
    pub fn world_quad(self, origin: glam::IVec3, outset: f32) -> [[f32; 3]; 4] {
        let shift = origin.as_vec3() + self.offset().as_vec3() * outset;
        let corners = FACE_CORNERS
            .iter()
            .find(|(face, _)| *face == self)
            .expect("FACE_CORNERS includes every face")
            .1;
        std::array::from_fn(|i| {
            [
                shift.x + corners[i][0],
                shift.y + corners[i][1],
                shift.z + corners[i][2],
            ]
        })
    }

    /// Rotate this face the same way geometry is rotated (X then Y, 90° steps).
    pub fn rotate(self, rotation: StateRotation) -> Self {
        rotate_face_x(self, rotation.x()).rotate_y(rotation.y())
    }

    fn rotate_y(self, degrees: u16) -> Self {
        match (self, degrees % 360) {
            (face, 0) => face,
            (Self::North, 90) => Self::East,
            (Self::East, 90) => Self::South,
            (Self::South, 90) => Self::West,
            (Self::West, 90) => Self::North,
            (Self::North, 180) => Self::South,
            (Self::South, 180) => Self::North,
            (Self::West, 180) => Self::East,
            (Self::East, 180) => Self::West,
            (Self::North, 270) => Self::West,
            (Self::West, 270) => Self::South,
            (Self::South, 270) => Self::East,
            (Self::East, 270) => Self::North,
            (face, _) => face,
        }
    }
}

fn rotate_face_x(face: Face, degrees: u16) -> Face {
    match (face, degrees % 360) {
        (face, 0) => face,
        (Face::Down, 90) => Face::South,
        (Face::South, 90) => Face::Up,
        (Face::Up, 90) => Face::North,
        (Face::North, 90) => Face::Down,
        (Face::Down, 180) => Face::Up,
        (Face::Up, 180) => Face::Down,
        (Face::North, 180) => Face::South,
        (Face::South, 180) => Face::North,
        (Face::Down, 270) => Face::North,
        (Face::North, 270) => Face::Up,
        (Face::Up, 270) => Face::South,
        (Face::South, 270) => Face::Down,
        (face, _) => face,
    }
}

/// Full-cube fast path, distinguished by how texture slots map onto faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasicBlock {
    All,
    Column,
}

impl BasicBlock {
    pub fn from_parent(stem: &str) -> Option<Self> {
        match stem {
            "cube_all" => Some(Self::All),
            "cube_column" => Some(Self::Column),
            _ => None,
        }
    }

    /// Texture slot this member reads for `face`.
    pub fn slot(self, face: Face) -> &'static str {
        match (self, face) {
            (Self::All, _) => "all",
            (Self::Column, Face::Down | Face::Up) => "end",
            (Self::Column, _) => "side",
        }
    }
}

/// One NBT palette entry: identity, classification, and drawable parts.
#[derive(Debug, Clone)]
pub struct Block {
    name: String,
    properties: BTreeMap<String, String>,
    block_type: BlockType,
    parts: Vec<BlockPart>,
    particle_texture: Option<String>,
    covers: [bool; 6],
    opaque: bool,
}

/// Geometry from one resolved model, plus palette-driven orientation.
#[derive(Debug, Clone)]
pub struct BlockPart {
    model: String,
    elements: Vec<Element>,
    rotation: StateRotation,
    uvlock: bool,
}

impl Block {
    /// Builds a drawable block from a resolved palette entry.
    pub fn from_resolved(name: impl Into<String>, resolved: &ResolvedBlock) -> Self {
        let name = name.into();
        let properties = resolved.properties.clone();
        let rotation = StateRotation::from_properties(&properties);
        let has_geometry = resolved.parts.iter().any(|part| !part.block_entity);
        let mut block_type =
            BlockType::classify(&name, resolved.parent_stem.as_deref(), has_geometry);
        if matches!(block_type, BlockType::Basic(_)) && !rotation.is_identity() {
            // Oriented cubes (e.g. `axis=x` pillars) take the general path in 5.6.
            block_type = BlockType::Elements;
        }

        let particle_texture = resolved.particle_texture.clone();
        let parts = match block_type {
            BlockType::Invisible => Vec::new(),
            BlockType::BlockEntity => vec![placeholder_part(
                resolved
                    .parts
                    .first()
                    .map(|part| part.model.as_str())
                    .unwrap_or("missing"),
                particle_texture.as_deref(),
                rotation,
            )],
            BlockType::Basic(_) | BlockType::Elements => resolved
                .parts
                .iter()
                .map(|part| BlockPart::from_resolved(part, rotation))
                .collect(),
        };

        let covers = compute_covers(block_type, &parts);
        let opaque = match block_type {
            BlockType::Basic(_) => true,
            BlockType::Elements => covers.iter().all(|&c| c),
            BlockType::BlockEntity | BlockType::Invisible => false,
        };

        Self {
            name,
            properties,
            block_type,
            parts,
            particle_texture,
            covers,
            opaque,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn properties(&self) -> &BTreeMap<String, String> {
        &self.properties
    }

    pub fn block_type(&self) -> BlockType {
        self.block_type
    }

    pub fn parts(&self) -> &[BlockPart] {
        &self.parts
    }

    pub fn particle_texture(&self) -> Option<&str> {
        self.particle_texture.as_deref()
    }

    pub fn covers_full_face(&self, face: Face) -> bool {
        match self.block_type {
            BlockType::Basic(_) => true,
            BlockType::Elements => self.covers[face.index()],
            BlockType::BlockEntity | BlockType::Invisible => false,
        }
    }

    pub fn is_opaque(&self) -> bool {
        self.opaque
    }

    /// Sides of the cell this block hides completely, as [`Face::bit`] flags in
    /// world directions: the side is spanned by a full 16x16 face whose texture
    /// has no transparent pixels. `has_alpha` reports transparency per texture key.
    pub fn occlusion_mask(&self, has_alpha: impl Fn(&str) -> bool) -> u8 {
        let mut mask = 0;
        match self.block_type {
            BlockType::Invisible | BlockType::BlockEntity => {}
            BlockType::Basic(_) => {
                // Mirrors the fast path in the mesh builder: six unit-cube
                // faces textured from the first element.
                let Some(part) = self.parts.first() else {
                    return 0;
                };
                for face in Face::ALL {
                    let texture = part
                        .elements
                        .first()
                        .and_then(|el| el.faces()[face.index()].as_ref())
                        .map_or("", ElementFace::texture);
                    if !has_alpha(texture) {
                        mask |= face.bit();
                    }
                }
            }
            BlockType::Elements => {
                for part in &self.parts {
                    for element in &part.elements {
                        for face in Face::ALL {
                            if element_covers_face(element, face)
                                && element.faces()[face.index()]
                                    .as_ref()
                                    .is_some_and(|f| !has_alpha(f.texture()))
                            {
                                mask |= face.after_geometry_rotation(part.rotation).bit();
                            }
                        }
                    }
                }
            }
        }
        mask
    }

    /// Distinct texture keys referenced by this block's faces (and particle, if any).
    pub fn texture_keys(&self) -> Vec<String> {
        let mut keys = BTreeMap::new();
        if let Some(particle) = &self.particle_texture
            && !particle.is_empty()
        {
            keys.insert(particle.clone(), ());
        }
        for part in &self.parts {
            for element in &part.elements {
                for face in element.faces().iter().flatten() {
                    if !face.texture().is_empty() {
                        keys.insert(face.texture().to_owned(), ());
                    }
                }
            }
        }
        keys.into_keys().collect()
    }

    pub(crate) fn tint(&self, tint_index: Option<u8>) -> [u8; 4] {
        tint_color(&self.name, tint_index, &self.properties)
    }
}

impl BlockPart {
    fn from_resolved(part: &ResolvedPart, rotation: StateRotation) -> Self {
        let elements = if part.block_entity {
            placeholder_elements(None)
        } else {
            part.elements.iter().map(Element::from_resolved).collect()
        };
        Self {
            model: part.model.clone(),
            elements,
            rotation,
            uvlock: false,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    pub fn rotation(&self) -> StateRotation {
        self.rotation
    }

    pub fn uvlock(&self) -> bool {
        self.uvlock
    }
}

fn placeholder_part(model: &str, particle: Option<&str>, rotation: StateRotation) -> BlockPart {
    BlockPart {
        model: model.to_owned(),
        elements: placeholder_elements(particle),
        rotation,
        uvlock: false,
    }
}

fn placeholder_elements(particle: Option<&str>) -> Vec<Element> {
    let from = Vec3::splat(BLOCK_ENTITY_INSET);
    let to = Vec3::splat(16.0 - BLOCK_ENTITY_INSET);
    let texture = particle.unwrap_or("").to_owned();
    let mut faces = [None, None, None, None, None, None];
    for face in Face::ALL {
        faces[face.index()] = Some(ElementFace::new(
            texture.clone(),
            default_uv(face, from, to),
            0,
            None,
            None,
        ));
    }
    vec![Element::new(from, to, None, faces, true)]
}

fn compute_covers(block_type: BlockType, parts: &[BlockPart]) -> [bool; 6] {
    match block_type {
        BlockType::Basic(_) => [true; 6],
        BlockType::BlockEntity | BlockType::Invisible => [false; 6],
        BlockType::Elements => {
            let mut covers = [false; 6];
            for part in parts {
                if !part.rotation.is_identity() {
                    continue;
                }
                for element in &part.elements {
                    for face in Face::ALL {
                        if element_covers_face(element, face) {
                            covers[face.index()] = true;
                        }
                    }
                }
            }
            covers
        }
    }
}

/// True when `face` of an unrotated element spans the whole side of the cell.
pub(crate) fn element_covers_face(element: &Element, face: Face) -> bool {
    if element.rotation().is_some() || element.faces()[face.index()].is_none() {
        return false;
    }
    const LO: f32 = 0.0;
    const HI: f32 = 16.0;
    const EPS: f32 = 1e-4;
    let from = element.from();
    let to = element.to();
    let spans = |a: f32, b: f32| (a - LO).abs() < EPS && (b - HI).abs() < EPS;
    match face {
        Face::Down => (from.y - LO).abs() < EPS && spans(from.x, to.x) && spans(from.z, to.z),
        Face::Up => (to.y - HI).abs() < EPS && spans(from.x, to.x) && spans(from.z, to.z),
        Face::North => (from.z - LO).abs() < EPS && spans(from.x, to.x) && spans(from.y, to.y),
        Face::South => (to.z - HI).abs() < EPS && spans(from.x, to.x) && spans(from.y, to.y),
        Face::West => (from.x - LO).abs() < EPS && spans(from.y, to.y) && spans(from.z, to.z),
        Face::East => (to.x - HI).abs() < EPS && spans(from.y, to.y) && spans(from.z, to.z),
    }
}

impl Element {
    fn from_resolved(element: &ResolvedElement) -> Self {
        let from = Vec3::from_array(element.from);
        let to = Vec3::from_array(element.to);
        let rotation = element
            .rotation
            .as_ref()
            .and_then(ElementRotation::from_model);
        let mut faces = [None, None, None, None, None, None];
        for (name, face) in &element.faces {
            let Some(dir) = Face::from_name(name) else {
                continue;
            };
            faces[dir.index()] = Some(ElementFace::from_resolved(face, from, to, dir));
        }
        Self::new(from, to, rotation, faces, element.shade)
    }
}

impl ElementFace {
    fn from_resolved(face: &ResolvedFace, from: Vec3, to: Vec3, dir: Face) -> Self {
        let tint_index = face.tintindex.and_then(|i| u8::try_from(i).ok());
        Self::new(
            face.texture.clone().unwrap_or_default(),
            face.uv.unwrap_or_else(|| default_uv(dir, from, to)),
            face.rotation.unwrap_or(0) as u16,
            face.cullface.as_deref().and_then(Face::from_name),
            tint_index,
        )
    }
}

impl ElementRotation {
    fn from_model(rot: &crate::block_models::ModelRotation) -> Option<Self> {
        let axis = match rot.axis.as_str() {
            "x" => Axis::X,
            "y" => Axis::Y,
            "z" => Axis::Z,
            _ => return None,
        };
        Some(Self::new(
            Vec3::from_array(rot.origin),
            axis,
            rot.angle,
            rot.rescale,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_slot_maps_all_and_column() {
        assert_eq!(BasicBlock::from_parent("cube_all"), Some(BasicBlock::All));
        assert_eq!(
            BasicBlock::from_parent("cube_column"),
            Some(BasicBlock::Column)
        );
        assert_eq!(BasicBlock::from_parent("cube"), None);
        assert_eq!(BasicBlock::All.slot(Face::Up), "all");
        assert_eq!(BasicBlock::Column.slot(Face::Up), "end");
        assert_eq!(BasicBlock::Column.slot(Face::North), "side");
    }

    #[test]
    fn face_corners_table_has_six_faces_of_four_corners() {
        assert_eq!(FACE_CORNERS.len(), 6);
        for (_, corners) in FACE_CORNERS {
            assert_eq!(corners.len(), 4);
        }
    }

    #[test]
    fn y90_sends_north_to_east() {
        let rot = StateRotation::new(0, 90);
        assert_eq!(Face::North.rotate(rot), Face::East);
        assert_eq!(Face::East.rotate(rot), Face::South);
        assert_eq!(Face::Up.rotate(rot), Face::Up);
        assert_eq!(Face::North.name(), "north");
        assert_eq!(Face::from_name(Face::West.name()), Some(Face::West));
        assert_eq!(Face::East.offset(), glam::IVec3::X);
        let quad = Face::Up.world_quad(glam::IVec3::ZERO, 0.0);
        assert!(quad.iter().all(|p| (p[1] - 1.0).abs() < 1e-5));
    }
}
