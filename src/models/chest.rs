//! Hand-built geometry for chests.
//!
//! Chests are drawn upstream by a hardcoded block-entity renderer, from one
//! texture atlas per material and half. The jar import cuts each atlas into a
//! tile per box face ([`ChestHalf::tiles`]); the boxes here draw those tiles.

use std::collections::BTreeMap;

use super::block::Face;
use super::element::{Element, ElementFace};
use super::sign::{facing_yaw, placed_box};
use crate::constants::{CHEST_TILE_PREFIX, CHEST_TYPE_PROPERTY};

/// The chest materials, told apart by block name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestKind {
    Normal,
    Trapped,
    /// Never part of a double chest.
    Ender,
}

/// Which part of a chest one block is, from the `type` property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChestHalf {
    Single,
    /// Joined to the block on its west when facing south.
    Left,
    /// Joined to the block on its east when facing south.
    Right,
}

/// One face of a chest box, cut out of an atlas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChestTile {
    /// Texture key, and file stem, of the tile.
    pub key: String,
    /// `[x, y, width, height]` within the atlas, in texels of a 64x64 atlas.
    pub region: [u32; 4],
}

/// One box of a south-facing chest, in model units, unwrapped at `tex` in the
/// atlas the way vanilla entity models are.
struct ChestBox {
    part: &'static str,
    from: [u32; 3],
    size: [u32; 3],
    tex: [u32; 2],
}

impl ChestKind {
    /// Recognises a chest from its block name (`minecraft:trapped_chest`).
    pub fn from_name(name: &str) -> Option<Self> {
        let path = name.rsplit_once(':').map(|(_, rest)| rest).unwrap_or(name);
        let stem = path.rsplit_once('/').map(|(_, s)| s).unwrap_or(path);
        match stem {
            "chest" => Some(Self::Normal),
            "trapped_chest" => Some(Self::Trapped),
            "ender_chest" => Some(Self::Ender),
            _ => None,
        }
    }

    /// Atlas file stem for a single chest of this material.
    fn material(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Trapped => "trapped",
            Self::Ender => "ender",
        }
    }

    /// Boxes for this chest, already turned to match `facing`.
    pub(crate) fn elements(self, properties: &BTreeMap<String, String>) -> Vec<Element> {
        let half = match self {
            Self::Ender => ChestHalf::Single,
            Self::Normal | Self::Trapped => ChestHalf::from_properties(properties),
        };
        let atlas = format!("{}{}", self.material(), half.atlas_suffix());
        let yaw = facing_yaw(properties);
        half.boxes()
            .iter()
            .map(|chest_box| {
                let mut faces = [None, None, None, None, None, None];
                for face in Face::ALL {
                    if half.hides(chest_box.part, face) {
                        continue;
                    }
                    let [_, _, width, height] = chest_box.region(face);
                    let (w, h) = (width as f32, height as f32);
                    // The atlas is drawn for a model whose Y axis points down,
                    // so every tile is upside down on the box.
                    let uv = match face {
                        Face::Down => [0.0, 0.0, w, h],
                        Face::North | Face::South => [w, h, 0.0, 0.0],
                        Face::Up | Face::West | Face::East => [0.0, h, w, 0.0],
                    };
                    faces[face.index()] = Some(ElementFace::new(
                        tile_key(&atlas, chest_box.part, face),
                        uv,
                        0,
                        None,
                        None,
                    ));
                }
                let [x, y, z] = chest_box.from.map(|v| v as f32);
                let [w, h, d] = chest_box.size.map(|v| v as f32);
                placed_box(
                    [x + w * 0.5, z + d * 0.5],
                    [w * 0.5, d * 0.5],
                    y,
                    y + h,
                    yaw,
                    0.0,
                    faces,
                )
            })
            .collect()
    }
}

impl ChestHalf {
    fn from_properties(properties: &BTreeMap<String, String>) -> Self {
        match properties.get(CHEST_TYPE_PROPERTY).map(String::as_str) {
            Some("left") => Self::Left,
            Some("right") => Self::Right,
            _ => Self::Single,
        }
    }

    /// The half an atlas is drawn for, from its file stem (`normal_left`).
    pub(crate) fn from_atlas(stem: &str) -> Self {
        if stem.ends_with("_left") {
            Self::Left
        } else if stem.ends_with("_right") {
            Self::Right
        } else {
            Self::Single
        }
    }

    fn atlas_suffix(self) -> &'static str {
        match self {
            Self::Single => "",
            Self::Left => "_left",
            Self::Right => "_right",
        }
    }

    /// Base, lid and lock. A double chest's halves reach the shared side of
    /// their cells and split the lock between them.
    fn boxes(self) -> [ChestBox; 3] {
        let (x, width, lock_x, lock_width) = match self {
            Self::Single => (1, 14, 7, 2),
            Self::Left => (0, 15, 0, 1),
            Self::Right => (1, 15, 15, 1),
        };
        [
            ChestBox {
                part: "base",
                from: [x, 0, 1],
                size: [width, 10, 14],
                tex: [0, 19],
            },
            ChestBox {
                part: "lid",
                from: [x, 9, 1],
                size: [width, 5, 14],
                tex: [0, 0],
            },
            ChestBox {
                part: "lock",
                from: [lock_x, 7, 15],
                size: [lock_width, 4, 1],
                tex: [0, 0],
            },
        ]
    }

    /// Faces a closed chest never shows: the inside, the back of the lock, and
    /// the side a double chest's halves share.
    fn hides(self, part: &str, face: Face) -> bool {
        matches!(
            (part, face),
            ("base", Face::Up) | ("lid", Face::Down) | ("lock", Face::North)
        ) || matches!(
            (self, face),
            (Self::Left, Face::West) | (Self::Right, Face::East)
        )
    }

    /// Tiles to cut from the atlas with file stem `atlas`.
    pub(crate) fn tiles(self, atlas: &str) -> Vec<ChestTile> {
        let mut tiles = Vec::new();
        for chest_box in self.boxes() {
            for face in Face::ALL {
                if !self.hides(chest_box.part, face) {
                    tiles.push(ChestTile {
                        key: tile_key(atlas, chest_box.part, face),
                        region: chest_box.region(face),
                    });
                }
            }
        }
        tiles
    }
}

impl ChestBox {
    /// `[x, y, width, height]` of `face` in the atlas: `Down` and `Up` side by
    /// side above a strip of `West`, `North`, `East`, `South`.
    fn region(&self, face: Face) -> [u32; 4] {
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
}

fn tile_key(atlas: &str, part: &str, face: Face) -> String {
    format!("{CHEST_TILE_PREFIX}_{atlas}_{part}_{}", face.name())
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    /// Bounds of an element after its own rotation.
    fn bounds(element: &Element) -> (Vec3, Vec3) {
        let (from, to) = (element.from(), element.to());
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for i in 0..8 {
            let mut p = Vec3::new(
                if i & 1 == 0 { from.x } else { to.x },
                if i & 2 == 0 { from.y } else { to.y },
                if i & 4 == 0 { from.z } else { to.z },
            );
            if let Some(rotation) = element.rotation() {
                p = rotation.apply(p);
            }
            min = min.min(p);
            max = max.max(p);
        }
        (min, max)
    }

    fn close(a: Vec3, b: [f32; 3]) -> bool {
        (a - Vec3::from_array(b)).abs().max_element() < 1e-3
    }

    #[test]
    fn kind_comes_from_the_block_name() {
        assert_eq!(
            ChestKind::from_name("minecraft:chest"),
            Some(ChestKind::Normal)
        );
        assert_eq!(
            ChestKind::from_name("trapped_chest"),
            Some(ChestKind::Trapped)
        );
        assert_eq!(
            ChestKind::from_name("minecraft:ender_chest"),
            Some(ChestKind::Ender)
        );
        assert_eq!(ChestKind::from_name("minecraft:shulker_box"), None);
    }

    #[test]
    fn single_chest_is_a_base_a_lid_and_a_lock_at_the_front() {
        let elements = ChestKind::Normal.elements(&props(&[("facing", "south")]));
        assert_eq!(elements.len(), 3);
        let (min, max) = bounds(&elements[0]);
        assert!(close(min, [1.0, 0.0, 1.0]) && close(max, [15.0, 10.0, 15.0]));
        let (min, max) = bounds(&elements[1]);
        assert!(close(min, [1.0, 9.0, 1.0]) && close(max, [15.0, 14.0, 15.0]));
        let (min, max) = bounds(&elements[2]);
        assert!(close(min, [7.0, 7.0, 15.0]) && close(max, [9.0, 11.0, 16.0]));

        let front = elements[0].faces()[Face::South.index()].as_ref().unwrap();
        assert_eq!(front.texture(), "chest_normal_base_south");
        assert_eq!(front.uv(), [14.0, 10.0, 0.0, 0.0]);
        let top = elements[1].faces()[Face::Up.index()].as_ref().unwrap();
        assert_eq!(top.texture(), "chest_normal_lid_up");
        assert_eq!(top.uv(), [0.0, 14.0, 14.0, 0.0]);
        // The inside is never drawn.
        assert!(elements[0].faces()[Face::Up.index()].is_none());
        assert!(elements[1].faces()[Face::Down.index()].is_none());
    }

    #[test]
    fn facing_turns_the_lock_to_that_side() {
        let lock = |facing: &str| {
            let elements = ChestKind::Normal.elements(&props(&[("facing", facing)]));
            bounds(&elements[2])
        };
        let (min, max) = lock("north");
        assert!(close(min, [7.0, 7.0, 0.0]) && close(max, [9.0, 11.0, 1.0]));
        let (min, max) = lock("west");
        assert!(close(min, [0.0, 7.0, 7.0]) && close(max, [1.0, 11.0, 9.0]));
        let (min, max) = lock("east");
        assert!(close(min, [15.0, 7.0, 7.0]) && close(max, [16.0, 11.0, 9.0]));
    }

    #[test]
    fn double_chest_halves_meet_at_the_shared_side() {
        let left = ChestKind::Trapped.elements(&props(&[("facing", "south"), ("type", "left")]));
        let (min, max) = bounds(&left[0]);
        assert!(close(min, [0.0, 0.0, 1.0]) && close(max, [15.0, 10.0, 15.0]));
        let (min, max) = bounds(&left[2]);
        assert!(close(min, [0.0, 7.0, 15.0]) && close(max, [1.0, 11.0, 16.0]));
        assert!(left[0].faces()[Face::West.index()].is_none());
        assert_eq!(
            left[0].faces()[Face::East.index()]
                .as_ref()
                .unwrap()
                .texture(),
            "chest_trapped_left_base_east"
        );

        let right = ChestKind::Trapped.elements(&props(&[("facing", "south"), ("type", "right")]));
        let (min, max) = bounds(&right[0]);
        assert!(close(min, [1.0, 0.0, 1.0]) && close(max, [16.0, 10.0, 15.0]));
        assert!(right[0].faces()[Face::East.index()].is_none());

        // A left half facing north joins the block on its east.
        let left = ChestKind::Normal.elements(&props(&[("facing", "north"), ("type", "left")]));
        let (min, max) = bounds(&left[0]);
        assert!(close(min, [1.0, 0.0, 1.0]) && close(max, [16.0, 10.0, 15.0]));
    }

    #[test]
    fn ender_chest_ignores_type() {
        let elements = ChestKind::Ender.elements(&props(&[("type", "left")]));
        let face = elements[0].faces()[Face::West.index()].as_ref().unwrap();
        assert_eq!(face.texture(), "chest_ender_base_west");
    }

    #[test]
    fn tiles_follow_the_entity_box_unwrap() {
        let tiles = ChestHalf::Single.tiles("normal");
        // Three boxes of six faces, less base top, lid underside and lock back.
        assert_eq!(tiles.len(), 15);
        let region = |key: &str| tiles.iter().find(|tile| tile.key == key).unwrap().region;
        assert_eq!(region("chest_normal_lid_up"), [28, 0, 14, 14]);
        assert_eq!(region("chest_normal_lid_south"), [42, 14, 14, 5]);
        assert_eq!(region("chest_normal_base_down"), [14, 19, 14, 14]);
        assert_eq!(region("chest_normal_base_west"), [0, 33, 14, 10]);
        assert_eq!(region("chest_normal_lock_south"), [4, 1, 2, 4]);

        let tiles = ChestHalf::from_atlas("normal_left").tiles("normal_left");
        assert_eq!(tiles.len(), 12);
        let region = |key: &str| tiles.iter().find(|tile| tile.key == key).unwrap().region;
        assert_eq!(region("chest_normal_left_base_south"), [43, 33, 15, 10]);
        assert_eq!(region("chest_normal_left_lock_south"), [3, 1, 1, 4]);
    }
}
