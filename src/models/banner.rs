//! Hand-built geometry for banners.
//!
//! Like signs, banners are drawn upstream by a hardcoded block-entity renderer.
//! All sixteen colours, standing and wall-mounted, share the single geometry-less
//! `banner` model, so the colour has to come from the block name.

use std::collections::BTreeMap;

use super::element::Element;
use super::sign::{facing_yaw, rotation_yaw, wood_box};
use crate::constants::DEFAULT_DYE_COLOUR;
use crate::custom_block_keys::CustomBlockKey;

/// Vanilla scales the banner entity model by 2/3.
const BANNER_SCALE: f32 = 2.0 / 3.0;
/// Half the width of the pole, and half the thickness of the crossbar.
const POLE_HALF: f32 = BANNER_SCALE;
/// Height of a standing banner's pole (42 entity pixels, scaled).
const POLE_HEIGHT: f32 = 42.0 * BANNER_SCALE;
/// Half the width of the crossbar and the flag (20 entity pixels, scaled).
const FLAG_HALF_WIDTH: f32 = 10.0 * BANNER_SCALE;
/// Height of the crossbar (2 entity pixels, scaled).
const BAR_HEIGHT: f32 = 2.0 * BANNER_SCALE;
/// Length of the flag (40 entity pixels, scaled).
const FLAG_HEIGHT: f32 = 40.0 * BANNER_SCALE;
/// Half the thickness of the flag (1 entity pixel, scaled).
const FLAG_HALF_DEPTH: f32 = 0.5 * BANNER_SCALE;
/// How far the flag's centre sits in front of the crossbar's.
const FLAG_FORWARD: f32 = 1.5 * BANNER_SCALE;
/// How far a wall banner hangs below a standing banner's crossbar.
const WALL_DROP: f32 = 47.0 / 3.0;
/// Distance from the wall behind a wall banner to its crossbar's centre.
const WALL_OFFSET: f32 = 1.0;

/// The two banner shapes, told apart by block name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    /// Flag on a pole, turned in 22.5 degree steps by `rotation`.
    Standing,
    /// Flag on a crossbar against the block behind it, oriented by `facing`.
    Wall,
}

impl BannerKind {
    /// Recognises a banner from its block name (`minecraft:red_wall_banner`).
    pub fn from_name(name: &str) -> Option<Self> {
        match CustomBlockKey::from_name(name)? {
            CustomBlockKey::WallBanner => Some(Self::Wall),
            CustomBlockKey::Banner => Some(Self::Standing),
            _ => None,
        }
    }

    /// Boxes for this banner, already turned to match `properties`.
    ///
    /// `wood` covers the pole and crossbar, `flag` the cloth. Both overhang
    /// the cell: a standing banner is nearly two blocks tall, and a wall
    /// banner's flag hangs into the cell below.
    pub(crate) fn elements(
        self,
        properties: &BTreeMap<String, String>,
        wood: &str,
        flag: &str,
    ) -> Vec<Element> {
        // Geometry is laid out for a banner whose front faces south (+Z).
        match self {
            Self::Standing => {
                let yaw = rotation_yaw(properties);
                let bar_top = POLE_HEIGHT + BAR_HEIGHT;
                vec![
                    wood_box(
                        [8.0, 8.0 + FLAG_FORWARD],
                        [FLAG_HALF_WIDTH, FLAG_HALF_DEPTH],
                        bar_top - FLAG_HEIGHT,
                        bar_top,
                        yaw,
                        flag,
                    ),
                    wood_box(
                        [8.0, 8.0],
                        [POLE_HALF, POLE_HALF],
                        0.0,
                        POLE_HEIGHT,
                        yaw,
                        wood,
                    ),
                    wood_box(
                        [8.0, 8.0],
                        [FLAG_HALF_WIDTH, POLE_HALF],
                        POLE_HEIGHT,
                        bar_top,
                        yaw,
                        wood,
                    ),
                ]
            }
            Self::Wall => {
                let yaw = facing_yaw(properties);
                let bar_top = POLE_HEIGHT + BAR_HEIGHT - WALL_DROP;
                vec![
                    wood_box(
                        [8.0, WALL_OFFSET + FLAG_FORWARD],
                        [FLAG_HALF_WIDTH, FLAG_HALF_DEPTH],
                        bar_top - FLAG_HEIGHT,
                        bar_top,
                        yaw,
                        flag,
                    ),
                    wood_box(
                        [8.0, WALL_OFFSET],
                        [FLAG_HALF_WIDTH, POLE_HALF],
                        bar_top - BAR_HEIGHT,
                        bar_top,
                        yaw,
                        wood,
                    ),
                ]
            }
        }
    }
}

/// Texture standing in for the flag's cloth: the wool of the banner's colour.
/// A name without a colour (the bare `banner` model) reads as white.
pub(crate) fn flag_texture(name: &str) -> String {
    let color = CustomBlockKey::from_name(name)
        .and_then(|key| key.member(name))
        .filter(|color| !color.is_empty())
        .unwrap_or(DEFAULT_DYE_COLOUR);
    format!("{color}_wool")
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

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
    fn kind_and_colour_come_from_the_block_name() {
        assert_eq!(
            BannerKind::from_name("minecraft:white_banner"),
            Some(BannerKind::Standing)
        );
        assert_eq!(
            BannerKind::from_name("minecraft:light_blue_wall_banner"),
            Some(BannerKind::Wall)
        );
        assert_eq!(BannerKind::from_name("banner"), Some(BannerKind::Standing));
        assert_eq!(BannerKind::from_name("minecraft:oak_sign"), None);
        assert_eq!(flag_texture("minecraft:white_banner"), "white_wool");
        assert_eq!(
            flag_texture("minecraft:light_blue_wall_banner"),
            "light_blue_wool"
        );
        assert_eq!(flag_texture("minecraft:banner"), "white_wool");
    }

    #[test]
    fn standing_banner_is_a_flag_in_front_of_a_pole() {
        let elements = BannerKind::Standing.elements(&BTreeMap::new(), "oak_planks", "red_wool");
        assert_eq!(elements.len(), 3);
        let flag = &elements[0];
        assert!(
            flag.faces()
                .iter()
                .flatten()
                .all(|f| f.texture() == "red_wool")
        );
        let (flag_min, flag_max) = bounds(flag);
        let (pole_min, pole_max) = bounds(&elements[1]);
        assert!(close(pole_min.y, 0.0) && close(pole_max.y, 28.0));
        assert!(close(flag_max.y, 28.0 + BAR_HEIGHT));
        assert!(flag_min.y > 0.0, "the flag stops short of the ground");
        assert!(
            flag_min.z >= pole_max.z - 1e-3,
            "the flag hangs to the south"
        );
        // 8 = north: the flag swings round to the other side of the pole.
        let elements =
            BannerKind::Standing.elements(&props("rotation", "8"), "oak_planks", "red_wool");
        let (flag_min, flag_max) = bounds(&elements[0]);
        assert!(flag_max.z < 8.0 && flag_min.z > 6.0);
    }

    #[test]
    fn wall_banner_hangs_from_the_block_behind_it() {
        let side = |facing: &str| {
            let elements =
                BannerKind::Wall.elements(&props("facing", facing), "oak_planks", "red_wool");
            assert_eq!(elements.len(), 2);
            (bounds(&elements[0]), bounds(&elements[1]))
        };
        let ((flag_min, flag_max), (bar_min, bar_max)) = side("south");
        assert!(
            bar_min.z > 0.0 && bar_max.z < 2.0,
            "bar sits on the north side"
        );
        assert!(flag_min.z >= bar_max.z - 1e-3 && flag_max.z < 3.0);
        assert!(flag_min.y < 0.0, "the flag hangs into the cell below");
        assert!(close(flag_max.y, bar_max.y));
        let ((flag_min, flag_max), (bar_min, _)) = side("west");
        assert!(bar_min.x > 14.0);
        assert!(flag_max.x <= bar_min.x + 1e-3 && flag_min.x > 13.0);
    }
}
