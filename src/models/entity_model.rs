//! Boxes of vanilla entity models, for blocks drawn upstream by block-entity
//! renderers: chests, beds, mob heads and the conduit.
//!
//! A [`ModelBox`] is written as vanilla writes it: an origin and size in
//! entity-model units and the texel where its faces are unwrapped in the
//! atlas. A [`Placement`] carries it into block space. The jar import cuts each
//! unwrapped face out of the atlas as a tile ([`entity_atlas`]); the box's
//! faces draw those tiles, oriented the way vanilla's `ModelPart` maps them.

use glam::Vec3;

use super::bed;
use super::block::{FACE_CORNERS, Face};
use super::chest::ChestHalf;
use super::conduit;
use super::element::{Element, ElementFace};
use super::head::HeadMob;
use super::sign::placed_box;
use crate::constants::{
    BED_ATLAS_SIZE, BED_TEXTURE_DIR, CHEST_ATLAS_SIZE, CHEST_TEXTURE_DIR, CONDUIT_ATLAS_SIZE,
    CONDUIT_TEXTURE,
};

/// One box of an entity model.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ModelBox {
    /// Names the box in tile keys. Boxes sharing a part share their tiles.
    pub part: &'static str,
    /// Minimum corner, in model units.
    pub origin: [f32; 3],
    /// Extent along each model axis, in texels.
    pub size: [u32; 3],
    /// Top-left of the unwrap in the atlas.
    pub tex: [u32; 2],
    /// How far the box stands off its size on every side, as a hat layer does.
    pub inflate: f32,
    /// Drawn reflected across X, as vanilla's `mirror` boxes are.
    pub mirror: bool,
}

/// Carries entity-model space into block space (model units, 0-16): block
/// axis `i` is `offset[i] + scale[i] * model[axis[i]]`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placement {
    axis: [usize; 3],
    scale: [f32; 3],
    offset: [f32; 3],
}

/// One face of a box, cut out of an atlas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtlasTile {
    /// Texture key, and file stem, of the tile.
    pub key: String,
    /// `[x, y, width, height]` within the atlas, in texels of its layout.
    pub region: [u32; 4],
}

/// An entity texture the import cuts into tiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntityAtlas {
    /// Width, in texels, of the layout `tiles` are given in. A wider file is
    /// the same layout scaled up.
    pub size: u32,
    pub tiles: Vec<AtlasTile>,
}

impl ModelBox {
    pub(crate) const fn new(
        part: &'static str,
        origin: [f32; 3],
        size: [u32; 3],
        tex: [u32; 2],
    ) -> Self {
        Self {
            part,
            origin,
            size,
            tex,
            inflate: 0.0,
            mirror: false,
        }
    }

    pub(crate) const fn inflated(self, inflate: f32) -> Self {
        Self { inflate, ..self }
    }

    pub(crate) const fn mirrored(self) -> Self {
        Self {
            mirror: true,
            ..self
        }
    }

    /// `[x, y, width, height]` in the atlas of the face unwrapped as `face`:
    /// `Down` and `Up` side by side above a strip of `West`, `North`, `East`,
    /// `South`.
    pub(crate) fn region(&self, face: Face) -> [u32; 4] {
        let [w, h, d] = self.size;
        let [u, v] = self.tex;
        match face {
            Face::Down => [u + d, v, w, d],
            Face::Up => [u + d + w, v, w, d],
            Face::West => [u, v + d, d, h],
            Face::North => [u + d, v + d, w, h],
            Face::East => [u + d + w, v + d, d, h],
            Face::South => [u + d + w + d, v + d, w, h],
        }
    }

    /// The unwrapped face drawn on model side `side`, and the reverse: a
    /// mirrored box swaps `West` and `East`.
    fn unwrapped(&self, side: Face) -> Face {
        match side {
            Face::West | Face::East if self.mirror => side.opposite(),
            _ => side,
        }
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        let origin = Vec3::from_array(self.origin);
        let size = Vec3::from_array(self.size.map(|v| v as f32));
        let inflate = Vec3::splat(self.inflate);
        (origin - inflate, origin + size + inflate)
    }
}

impl Placement {
    /// Model space that is block space already.
    pub(crate) const IDENTITY: Self = Self::new([0, 1, 2], [1.0; 3], [0.0; 3]);

    pub(crate) const fn new(axis: [usize; 3], scale: [f32; 3], offset: [f32; 3]) -> Self {
        Self {
            axis,
            scale,
            offset,
        }
    }

    /// One box in block space, swung about the block centre by `yaw`.
    ///
    /// `texture` names the tile of an unwrapped face. A face is left out when
    /// `hidden` reports its model side.
    pub(crate) fn element(
        &self,
        model_box: &ModelBox,
        texture: impl Fn(Face) -> String,
        hidden: impl Fn(Face) -> bool,
        yaw: f32,
    ) -> Element {
        let (from, to) = model_box.bounds();
        let (a, b) = (self.point(from), self.point(to));
        let (min, max) = (a.min(b), a.max(b));
        let mut faces = [None, None, None, None, None, None];
        for (face, corners) in FACE_CORNERS {
            let side = self.model_side(face);
            if hidden(side) {
                continue;
            }
            let unwrapped = model_box.unwrapped(side);
            let [_, _, width, height] = model_box.region(unwrapped);
            let samples = corners.map(|corner| {
                let mut model = self.model_corner([corner[0], corner[1], corner[2]]);
                if model_box.mirror {
                    model[0] = 1.0 - model[0];
                }
                let [u, v] = unwrap_uv(unwrapped, model);
                (
                    [corner[3], corner[4]],
                    [u * width as f32, v * height as f32],
                )
            });
            let (uv, rotation) = fit_uv(samples);
            faces[face.index()] = Some(ElementFace::new(
                texture(unwrapped),
                uv,
                rotation,
                None,
                None,
            ));
        }
        let centre = (min + max) * 0.5;
        let half = (max - min) * 0.5;
        placed_box(
            [centre.x, centre.z],
            [half.x, half.z],
            min.y,
            max.y,
            yaw,
            0.0,
            faces,
        )
    }

    fn point(&self, model: Vec3) -> Vec3 {
        Vec3::from_array(std::array::from_fn(|i| {
            self.offset[i] + self.scale[i] * model[self.axis[i]]
        }))
    }

    /// The model side that lands on block side `face`.
    fn model_side(&self, face: Face) -> Face {
        let (i, positive) = axis_of(face);
        face_on(self.axis[i], positive != (self.scale[i] < 0.0))
    }

    /// The model corner, 0 or 1 per model axis, at block corner `corner`.
    fn model_corner(&self, corner: [f32; 3]) -> [f32; 3] {
        let mut model = [0.0; 3];
        for i in 0..3 {
            model[self.axis[i]] = if self.scale[i] < 0.0 {
                1.0 - corner[i]
            } else {
                corner[i]
            };
        }
        model
    }
}

/// The atlas at `path` under `textures/` (`entity/chest/normal`), when it is
/// one the editor cuts into tiles.
pub(crate) fn entity_atlas(path: &str) -> Option<EntityAtlas> {
    let (dir, stem) = path.rsplit_once('/')?;
    let (size, tiles) = if dir == CHEST_TEXTURE_DIR {
        (CHEST_ATLAS_SIZE, ChestHalf::from_atlas(stem).tiles(stem))
    } else if dir == BED_TEXTURE_DIR {
        (BED_ATLAS_SIZE, bed::tiles(stem))
    } else if path == CONDUIT_TEXTURE {
        (CONDUIT_ATLAS_SIZE, conduit::tiles())
    } else {
        let mob = HeadMob::from_texture(path)?;
        (mob.atlas_size(), mob.tiles())
    };
    Some(EntityAtlas { size, tiles })
}

/// The file stem of a texture path: `base` for `entity/conduit/base`.
pub(crate) fn atlas_stem(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, stem)| stem)
}

/// Texture key of the tile cut for one unwrapped face of a box.
pub(crate) fn tile_key(prefix: &str, atlas: &str, part: &str, face: Face) -> String {
    format!("{prefix}_{atlas}_{part}_{}", face.name())
}

/// Tiles for every face of `boxes` that `hidden` does not report by box and
/// model side. Boxes sharing a part give one set.
pub(crate) fn tiles(
    prefix: &str,
    atlas: &str,
    boxes: &[ModelBox],
    hidden: impl Fn(&ModelBox, Face) -> bool,
) -> Vec<AtlasTile> {
    let mut tiles: Vec<AtlasTile> = Vec::new();
    for model_box in boxes {
        for face in Face::ALL {
            if hidden(model_box, model_box.unwrapped(face)) {
                continue;
            }
            let key = tile_key(prefix, atlas, model_box.part, face);
            if tiles.iter().all(|tile| tile.key != key) {
                tiles.push(AtlasTile {
                    key,
                    region: model_box.region(face),
                });
            }
        }
    }
    tiles
}

fn axis_of(face: Face) -> (usize, bool) {
    match face {
        Face::West => (0, false),
        Face::East => (0, true),
        Face::Down => (1, false),
        Face::Up => (1, true),
        Face::North => (2, false),
        Face::South => (2, true),
    }
}

fn face_on(axis: usize, positive: bool) -> Face {
    match (axis, positive) {
        (0, false) => Face::West,
        (0, true) => Face::East,
        (1, false) => Face::Down,
        (1, true) => Face::Up,
        (2, false) => Face::North,
        _ => Face::South,
    }
}

/// Where model corner `corner` (0 or 1 per axis) lands within the region of
/// unwrapped face `face`, as a fraction of its width and height. These are
/// the vertex UVs of vanilla's `ModelPart.Cube` polygons.
fn unwrap_uv(face: Face, corner: [f32; 3]) -> [f32; 2] {
    let [x, y, z] = corner;
    match face {
        Face::Down | Face::Up => [x, 1.0 - z],
        Face::West => [1.0 - z, y],
        Face::North => [x, y],
        Face::East => [z, y],
        Face::South => [1.0 - x, y],
    }
}

/// The UV rectangle and rotation that give each face corner, sampled as
/// `(corner s/t, uv)`, its UV under the mesh builder's mapping.
fn fit_uv(samples: [([f32; 2], [f32; 2]); 4]) -> ([f32; 4], u16) {
    for rotation in [0, 90, 180, 270] {
        let mut rect = [None; 4];
        let fits = samples.iter().all(|&([s, t], [u, v])| {
            let (s, t) = match rotation {
                90 => (t, 1.0 - s),
                180 => (1.0 - s, 1.0 - t),
                270 => (1.0 - t, s),
                _ => (s, t),
            };
            let slots = [
                (if s < 0.5 { 0 } else { 2 }, u),
                (if t < 0.5 { 1 } else { 3 }, v),
            ];
            slots.into_iter().all(|(slot, value)| {
                let held = rect[slot].get_or_insert(value);
                (*held - value).abs() < 1e-4
            })
        });
        if fits {
            return (rect.map(|value| value.unwrap_or(0.0)), rotation);
        }
    }
    ([0.0; 4], 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: ModelBox = ModelBox::new("cube", [0.0; 3], [4, 6, 2], [0, 0]);

    fn face(element: &Element, face: Face) -> &ElementFace {
        element.faces()[face.index()].as_ref().unwrap()
    }

    fn key(face: Face) -> String {
        face.name().to_owned()
    }

    #[test]
    fn regions_follow_the_vanilla_unwrap() {
        let model_box = ModelBox::new("head", [0.0; 3], [8, 8, 8], [32, 0]);
        assert_eq!(model_box.region(Face::Down), [40, 0, 8, 8]);
        assert_eq!(model_box.region(Face::Up), [48, 0, 8, 8]);
        assert_eq!(model_box.region(Face::North), [40, 8, 8, 8]);
        assert_eq!(model_box.region(Face::South), [56, 8, 8, 8]);
    }

    #[test]
    fn y_up_model_draws_each_side_from_its_own_unwrap_upside_down() {
        let element = Placement::IDENTITY.element(&CUBE, key, |_| false, 0.0);
        assert_eq!(element.from(), Vec3::ZERO);
        assert_eq!(element.to(), Vec3::new(4.0, 6.0, 2.0));
        for side in Face::ALL {
            assert_eq!(face(&element, side).texture(), side.name());
            assert_eq!(face(&element, side).uv_rotation(), 0);
        }
        assert_eq!(face(&element, Face::South).uv(), [4.0, 6.0, 0.0, 0.0]);
        assert_eq!(face(&element, Face::Up).uv(), [0.0, 2.0, 4.0, 0.0]);
        assert_eq!(face(&element, Face::Down).uv(), [0.0, 0.0, 4.0, 2.0]);
        assert_eq!(face(&element, Face::West).uv(), [0.0, 6.0, 2.0, 0.0]);
    }

    #[test]
    fn flipped_model_faces_its_front_south_upright() {
        // Vanilla's head pose: Y down, front on -Z, turned to face south.
        let placement = Placement::new([0, 1, 2], [1.0, -1.0, -1.0], [8.0, 0.0, 8.0]);
        let model_box = ModelBox::new("head", [-4.0, -8.0, -4.0], [8, 8, 8], [0, 0]);
        let element = placement.element(&model_box, key, |_| false, 0.0);
        assert_eq!(element.from(), Vec3::new(4.0, 0.0, 4.0));
        assert_eq!(element.to(), Vec3::new(12.0, 8.0, 12.0));
        let front = face(&element, Face::South);
        assert_eq!(front.texture(), "north");
        assert_eq!(front.uv(), [0.0, 0.0, 8.0, 8.0]);
        let top = face(&element, Face::Up);
        assert_eq!(top.texture(), "down");
        assert_eq!(top.uv(), [0.0, 0.0, 8.0, 8.0]);
        assert_eq!(face(&element, Face::Down).texture(), "up");
        // The head's right side is on the west; its edge by the face is south.
        let right = face(&element, Face::West);
        assert_eq!(right.texture(), "west");
        assert_eq!(right.uv(), [8.0, 0.0, 0.0, 8.0]);
    }

    #[test]
    fn a_side_turned_across_axes_takes_a_uv_rotation() {
        // Model Y becomes block Z: the unwrap's V runs along the face's S.
        let placement = Placement::new([0, 2, 1], [1.0; 3], [0.0; 3]);
        let element = placement.element(&CUBE, key, |_| false, 0.0);
        assert_eq!(element.to(), Vec3::new(4.0, 2.0, 6.0));
        let side = face(&element, Face::West);
        assert_eq!(side.texture(), "west");
        assert_ne!(side.uv_rotation(), 0);
    }

    #[test]
    fn mirrored_box_swaps_its_side_unwraps() {
        let element = Placement::IDENTITY.element(&CUBE.mirrored(), key, |_| false, 0.0);
        assert_eq!(face(&element, Face::East).texture(), "west");
        assert_eq!(face(&element, Face::West).texture(), "east");
        let plain = Placement::IDENTITY.element(&CUBE, key, |_| false, 0.0);
        let [u1, v1, u2, v2] = face(&plain, Face::North).uv();
        assert_eq!(face(&element, Face::North).uv(), [u2, v1, u1, v2]);
    }

    #[test]
    fn hidden_sides_are_left_out_of_elements_and_tiles() {
        let element = Placement::IDENTITY.element(&CUBE, key, |side| side == Face::Up, 0.0);
        assert!(element.faces()[Face::Up.index()].is_none());
        let tiles = tiles("p", "a", &[CUBE, CUBE], |_, side| side == Face::Up);
        assert_eq!(tiles.len(), 5);
        assert_eq!(tiles[0].key, "p_a_cube_down");
    }

    #[test]
    fn known_entity_textures_are_atlases() {
        assert_eq!(entity_atlas("entity/chest/normal").unwrap().size, 64);
        assert_eq!(entity_atlas("entity/bed/red").unwrap().size, 64);
        assert_eq!(entity_atlas("entity/conduit/base").unwrap().size, 32);
        assert_eq!(entity_atlas("entity/enderdragon/dragon").unwrap().size, 256);
        assert!(entity_atlas("entity/player/wide/steve").is_some());
        assert!(entity_atlas("entity/conduit/cage").is_none());
        assert!(entity_atlas("entity/skeleton/stray").is_none());
        // Every tile fits a block texture's square.
        for path in [
            "entity/bed/red",
            "entity/enderdragon/dragon",
            "entity/piglin/piglin",
        ] {
            let atlas = entity_atlas(path).unwrap();
            assert!(!atlas.tiles.is_empty());
            for tile in atlas.tiles {
                let [x, y, w, h] = tile.region;
                assert!(w <= 16 && h <= 16, "{}", tile.key);
                assert!(x + w <= atlas.size && y + h <= atlas.size, "{}", tile.key);
            }
        }
    }
}
