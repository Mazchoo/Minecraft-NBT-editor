//! Hand-built geometry for signs.
//!
//! Minecraft draws signs with a hardcoded block-entity renderer, so their model
//! JSONs carry a `particle` texture and no elements. The boxes here reproduce
//! that renderer's shapes on the 0-16 model scale.

use std::collections::BTreeMap;

use glam::Vec3;

use super::block::Face;
use super::element::{Axis, Element, ElementFace, ElementRotation};

/// Vanilla scales the standing/wall sign entity model by 2/3.
const SIGN_SCALE: f32 = 2.0 / 3.0;
/// Half the thickness of a standing or wall sign board.
const BOARD_HALF_DEPTH: f32 = SIGN_SCALE;
/// Height of the post under a standing sign (14 entity pixels, scaled).
const POST_HEIGHT: f32 = 14.0 * SIGN_SCALE;
/// Height of a standing or wall sign board (12 entity pixels, scaled).
const BOARD_HEIGHT: f32 = 12.0 * SIGN_SCALE;
/// How far a wall sign's board hangs below a standing sign's.
const WALL_DROP: f32 = 5.0;
/// Top of a hanging sign's board; chains run from here upward.
const HANGING_BOARD_TOP: f32 = 10.0;
/// Distance of each chain from the centre of a hanging sign.
const CHAIN_OFFSET: f32 = 5.0;
/// Half the width of one chain plane (matches the vanilla chain block).
const CHAIN_HALF_WIDTH: f32 = 1.5;

/// The four sign shapes, told apart by block name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignKind {
    /// Board on a post, turned in 22.5 degree steps by `rotation`.
    Standing,
    /// Board flat against the block behind it, oriented by `facing`.
    Wall,
    /// Board on chains below a ceiling, turned by `rotation`.
    Hanging,
    /// Board on chains below a bar that juts from a wall, oriented by `facing`.
    WallHanging,
}

impl SignKind {
    /// Recognises a sign from its block name (`minecraft:oak_wall_sign`).
    pub fn from_name(name: &str) -> Option<Self> {
        let path = name.rsplit_once(':').map(|(_, rest)| rest).unwrap_or(name);
        let stem = path.rsplit_once('/').map(|(_, s)| s).unwrap_or(path);
        if stem.ends_with("_wall_hanging_sign") {
            Some(Self::WallHanging)
        } else if stem.ends_with("_hanging_sign") {
            Some(Self::Hanging)
        } else if stem.ends_with("_wall_sign") {
            Some(Self::Wall)
        } else if stem.ends_with("_sign") {
            Some(Self::Standing)
        } else {
            None
        }
    }

    /// Boxes for this sign, already turned to match `properties`.
    ///
    /// `texture` covers the wood; `chain_texture` is the chain block's texture,
    /// and hanging signs draw no chains without it.
    pub(crate) fn elements(
        self,
        properties: &BTreeMap<String, String>,
        texture: &str,
        chain_texture: Option<&str>,
    ) -> Vec<Element> {
        let yaw = self.yaw(properties);
        // Geometry is laid out for a sign whose front faces south (+Z).
        match self {
            Self::Standing => vec![
                wood_box(
                    [8.0, 8.0],
                    [8.0, BOARD_HALF_DEPTH],
                    POST_HEIGHT,
                    POST_HEIGHT + BOARD_HEIGHT,
                    yaw,
                    texture,
                ),
                wood_box(
                    [8.0, 8.0],
                    [SIGN_SCALE, SIGN_SCALE],
                    0.0,
                    POST_HEIGHT,
                    yaw,
                    texture,
                ),
            ],
            Self::Wall => vec![wood_box(
                [8.0, 1.0],
                [8.0, BOARD_HALF_DEPTH],
                POST_HEIGHT - WALL_DROP,
                POST_HEIGHT - WALL_DROP + BOARD_HEIGHT,
                yaw,
                texture,
            )],
            Self::Hanging => {
                let mut elements = vec![wood_box(
                    [8.0, 8.0],
                    [7.0, 1.0],
                    0.0,
                    HANGING_BOARD_TOP,
                    yaw,
                    texture,
                )];
                push_chains(&mut elements, HANGING_BOARD_TOP, 16.0, yaw, chain_texture);
                elements
            }
            Self::WallHanging => {
                let mut elements = vec![
                    wood_box([8.0, 8.0], [7.0, 1.0], 0.0, HANGING_BOARD_TOP, yaw, texture),
                    wood_box([8.0, 8.0], [8.0, 2.0], 14.0, 16.0, yaw, texture),
                ];
                push_chains(&mut elements, HANGING_BOARD_TOP, 14.0, yaw, chain_texture);
                elements
            }
        }
    }

    /// Angle about +Y, in [`ElementRotation`]'s convention, that turns a
    /// south-facing sign to the orientation in `properties`.
    fn yaw(self, properties: &BTreeMap<String, String>) -> f32 {
        match self {
            Self::Standing | Self::Hanging => rotation_yaw(properties),
            Self::Wall | Self::WallHanging => facing_yaw(properties),
        }
    }
}

/// Yaw, in [`ElementRotation`]'s convention, for the 16-step `rotation`
/// property of free-standing signs and banners.
pub(super) fn rotation_yaw(properties: &BTreeMap<String, String>) -> f32 {
    // 0 = south, 4 = west, 8 = north, 12 = east.
    let step = properties
        .get("rotation")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0)
        % 16;
    -22.5 * f32::from(step)
}

/// Yaw, in [`ElementRotation`]'s convention, for the `facing` property of
/// wall-mounted signs and banners.
pub(super) fn facing_yaw(properties: &BTreeMap<String, String>) -> f32 {
    match properties
        .get(crate::constants::FACING_PROPERTY)
        .map(String::as_str)
    {
        Some("west") => -90.0,
        Some("east") => 90.0,
        Some("south") => 0.0,
        // Vanilla's default state faces north.
        _ => 180.0,
    }
}

/// Rotates `(x, z)` about the block centre exactly as [`ElementRotation::apply`] does.
fn turn_about_centre(xz: [f32; 2], yaw: f32) -> [f32; 2] {
    let (sin, cos) = yaw.to_radians().sin_cos();
    let (x, z) = (xz[0] - 8.0, xz[1] - 8.0);
    [8.0 + x * cos + z * sin, 8.0 - x * sin + z * cos]
}

/// An axis-aligned box centred on `centre` (x, z), swung about the block centre
/// by `yaw` and then spun about its own vertical axis by `spin`.
///
/// Turning a box about the block centre is the same as moving its centre and
/// turning it in place, which lets one element rotation carry both angles.
pub(super) fn placed_box(
    centre: [f32; 2],
    half: [f32; 2],
    y0: f32,
    y1: f32,
    yaw: f32,
    spin: f32,
    faces: [Option<ElementFace>; 6],
) -> Element {
    let [cx, cz] = turn_about_centre(centre, yaw);
    let angle = yaw + spin;
    let rotation = (angle.rem_euclid(360.0) != 0.0)
        .then(|| ElementRotation::new(Vec3::new(cx, (y0 + y1) * 0.5, cz), Axis::Y, angle, false));
    Element::new(
        Vec3::new(cx - half[0], y0, cz - half[1]),
        Vec3::new(cx + half[0], y1, cz + half[1]),
        rotation,
        faces,
        true,
    )
}

/// A solid box textured at one texel per model unit from the texture's top-left.
pub(super) fn wood_box(
    centre: [f32; 2],
    half: [f32; 2],
    y0: f32,
    y1: f32,
    yaw: f32,
    texture: &str,
) -> Element {
    let (width, depth, height) = (half[0] * 2.0, half[1] * 2.0, y1 - y0);
    let mut faces = [None, None, None, None, None, None];
    for face in Face::ALL {
        let (u, v) = match face {
            Face::Down | Face::Up => (width, depth),
            Face::North | Face::South => (width, height),
            Face::West | Face::East => (depth, height),
        };
        faces[face.index()] = Some(ElementFace::new(
            texture.to_owned(),
            [0.0, 0.0, u.min(16.0), v.min(16.0)],
            0,
            None,
            None,
        ));
    }
    placed_box(centre, half, y0, y1, yaw, 0.0, faces)
}

/// Two chains, each a pair of crossed planes like the vanilla chain block.
fn push_chains(elements: &mut Vec<Element>, y0: f32, y1: f32, yaw: f32, texture: Option<&str>) {
    let Some(texture) = texture else {
        return;
    };
    let height = (y1 - y0).min(16.0);
    for x in [8.0 - CHAIN_OFFSET, 8.0 + CHAIN_OFFSET] {
        // The chain texture holds two link patterns side by side, 3 texels each.
        for (spin, u) in [(45.0, 0.0), (-45.0, 3.0)] {
            let mut faces = [None, None, None, None, None, None];
            faces[Face::North.index()] = Some(ElementFace::new(
                texture.to_owned(),
                [u, 0.0, u + 3.0, height],
                0,
                None,
                None,
            ));
            faces[Face::South.index()] = Some(ElementFace::new(
                texture.to_owned(),
                [u + 3.0, 0.0, u, height],
                0,
                None,
                None,
            ));
            elements.push(placed_box(
                [x, 8.0],
                [CHAIN_HALF_WIDTH, 0.0],
                y0,
                y1,
                yaw,
                spin,
                faces,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(key: &str, value: &str) -> BTreeMap<String, String> {
        BTreeMap::from([(key.to_owned(), value.to_owned())])
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

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn kind_comes_from_the_block_name() {
        assert_eq!(
            SignKind::from_name("minecraft:oak_sign"),
            Some(SignKind::Standing)
        );
        assert_eq!(SignKind::from_name("oak_wall_sign"), Some(SignKind::Wall));
        assert_eq!(
            SignKind::from_name("minecraft:cherry_hanging_sign"),
            Some(SignKind::Hanging)
        );
        assert_eq!(
            SignKind::from_name("minecraft:cherry_wall_hanging_sign"),
            Some(SignKind::WallHanging)
        );
        assert_eq!(SignKind::from_name("minecraft:chest"), None);
    }

    #[test]
    fn standing_sign_is_a_board_on_a_post() {
        let elements = SignKind::Standing.elements(&BTreeMap::new(), "oak_planks", None);
        assert_eq!(elements.len(), 2);
        let (board_min, board_max) = bounds(&elements[0]);
        assert!(close(board_min.x, 0.0) && close(board_max.x, 16.0));
        assert!(close(board_min.y, POST_HEIGHT));
        assert!(close(board_max.z - board_min.z, 2.0 * BOARD_HALF_DEPTH));
        let (post_min, post_max) = bounds(&elements[1]);
        assert!(close(post_min.y, 0.0) && close(post_max.y, POST_HEIGHT));
        assert!(close(post_min.x, 8.0 - SIGN_SCALE) && close(post_max.x, 8.0 + SIGN_SCALE));
        assert!(elements.iter().all(|el| el.rotation().is_none()));
    }

    #[test]
    fn standing_rotation_turns_the_board() {
        // 4 = west: the board's long side now runs along Z.
        let elements = SignKind::Standing.elements(&props("rotation", "4"), "oak_planks", None);
        let (min, max) = bounds(&elements[0]);
        assert!(close(min.z, 0.0) && close(max.z, 16.0));
        assert!(close(max.x - min.x, 2.0 * BOARD_HALF_DEPTH));
        // 2 = south-west: a diagonal board spans both axes equally.
        let elements = SignKind::Standing.elements(&props("rotation", "2"), "oak_planks", None);
        let (min, max) = bounds(&elements[0]);
        assert!(close(max.x - min.x, max.z - min.z));
    }

    #[test]
    fn wall_sign_sits_against_the_block_behind_it() {
        let side = |facing: &str| {
            let elements = SignKind::Wall.elements(&props("facing", facing), "oak_planks", None);
            assert_eq!(elements.len(), 1);
            bounds(&elements[0])
        };
        let (min, max) = side("north");
        assert!(
            min.z > 14.0 && max.z < 16.0,
            "north-facing hangs on the south side"
        );
        assert!(close(min.x, 0.0) && close(max.x, 16.0));
        let (min, max) = side("south");
        assert!(min.z > 0.0 && max.z < 2.0);
        let (min, max) = side("east");
        assert!(
            min.x > 0.0 && max.x < 2.0,
            "east-facing hangs on the west side"
        );
        assert!(close(min.z, 0.0) && close(max.z, 16.0));
        let (min, max) = side("west");
        assert!(min.x > 14.0 && max.x < 16.0);
    }

    #[test]
    fn hanging_signs_draw_chains_only_with_a_chain_texture() {
        let none = BTreeMap::new();
        assert_eq!(SignKind::Hanging.elements(&none, "log", None).len(), 1);
        let hanging = SignKind::Hanging.elements(&none, "log", Some("chain"));
        assert_eq!(hanging.len(), 5);
        for chain in &hanging[1..] {
            let (min, max) = bounds(chain);
            assert!(close(min.y, HANGING_BOARD_TOP) && close(max.y, 16.0));
            assert_eq!(chain.faces().iter().flatten().count(), 2);
            assert!(
                chain
                    .faces()
                    .iter()
                    .flatten()
                    .all(|f| f.texture() == "chain")
            );
        }
        let wall = SignKind::WallHanging.elements(&props("facing", "east"), "log", Some("chain"));
        assert_eq!(wall.len(), 6);
        let (bar_min, bar_max) = bounds(&wall[1]);
        assert!(close(bar_min.y, 14.0) && close(bar_max.y, 16.0));
        assert!(
            close(bar_min.z, 0.0) && close(bar_max.z, 16.0),
            "bar runs along Z"
        );
        // Chains follow the board: offset along Z, centred on X.
        let (chain_min, chain_max) = bounds(&wall[2]);
        assert!(close((chain_min.x + chain_max.x) * 0.5, 8.0));
        assert!(!close((chain_min.z + chain_max.z) * 0.5, 8.0));
    }
}
