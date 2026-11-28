//! Hand-built geometry for beds.
//!
//! Vanilla draws beds with a block-entity renderer, one atlas per colour; every
//! bed block shares the geometry-less `bed` model. Each half of the bed draws
//! its own mattress and two legs, cut from that atlas.

use std::collections::BTreeMap;

use super::block::Face;
use super::element::Element;
use super::entity_model::{self, AtlasTile, ModelBox, Placement};
use super::sign::facing_yaw;
use crate::constants::{BED_HEAD_PART, BED_PART_PROPERTY, BED_TILE_PREFIX, DEFAULT_DYE_COLOUR};
use crate::custom_block_keys::CustomBlockKey;

/// Whether `name` is a bed (`minecraft:red_bed`).
pub fn is_bed(name: &str) -> bool {
    CustomBlockKey::from_name(name) == Some(CustomBlockKey::Bed)
}

/// Boxes for one half of a bed, already turned so its head points `facing`.
pub(crate) fn elements(name: &str, properties: &BTreeMap<String, String>) -> Vec<Element> {
    let head = properties.get(BED_PART_PROPERTY).map(String::as_str) == Some(BED_HEAD_PART);
    let colour = colour(name);
    let yaw = facing_yaw(properties);
    boxes(head)
        .iter()
        .map(|model_box| {
            placement().element(
                model_box,
                |face| entity_model::tile_key(BED_TILE_PREFIX, colour, model_box.part, face),
                |side| hides(model_box, side),
                yaw,
            )
        })
        .collect()
}

/// Tiles to cut from the atlas of one colour.
pub(crate) fn tiles(colour: &str) -> Vec<AtlasTile> {
    let boxes: Vec<ModelBox> = boxes(true).into_iter().chain(boxes(false)).collect();
    entity_model::tiles(BED_TILE_PREFIX, colour, &boxes, hides)
}

/// The colour picking the bed's atlas. A name without a colour reads as the
/// default.
fn colour(name: &str) -> &str {
    CustomBlockKey::Bed
        .member(name)
        .filter(|colour| !colour.is_empty())
        .unwrap_or(DEFAULT_DYE_COLOUR)
}

/// Mattress and legs of one half, in entity units. The mattress lies in X
/// and Y with its top on -Z; its +Y end is the far end of the half.
fn boxes(head: bool) -> [ModelBox; 3] {
    if head {
        [
            ModelBox::new("head", [0.0; 3], [16, 16, 6], [0, 0]),
            ModelBox::new("head_left_leg", [0.0, 0.0, 6.0], [3, 3, 3], [50, 6]),
            ModelBox::new("head_right_leg", [13.0, 0.0, 6.0], [3, 3, 3], [50, 18]),
        ]
    } else {
        [
            ModelBox::new("foot", [0.0; 3], [16, 16, 6], [0, 22]),
            ModelBox::new("foot_left_leg", [0.0, 13.0, 6.0], [3, 3, 3], [50, 0]),
            ModelBox::new("foot_right_leg", [13.0, 13.0, 6.0], [3, 3, 3], [50, 12]),
        ]
    }
}

/// The mattress end each half shares with the other is never seen.
fn hides(model_box: &ModelBox, side: Face) -> bool {
    matches!(
        (model_box.part, side),
        ("head", Face::Up) | ("foot", Face::Down)
    )
}

/// Vanilla's bed pose with the head to the south: the mattress laid flat,
/// its top 9 units up.
fn placement() -> Placement {
    Placement::new([0, 2, 1], [-1.0, -1.0, -1.0], [16.0, 9.0, 16.0])
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

    #[test]
    fn colour_comes_from_the_block_name() {
        assert!(is_bed("minecraft:light_blue_bed"));
        assert!(!is_bed("minecraft:bedrock"));
        assert_eq!(colour("minecraft:light_blue_bed"), "light_blue");
        assert_eq!(colour("minecraft:bed"), "white");
    }

    #[test]
    fn head_half_is_a_mattress_with_its_pillow_and_legs_at_the_far_end() {
        let elements = elements(
            "minecraft:red_bed",
            &props(&[("part", "head"), ("facing", "south")]),
        );
        assert_eq!(elements.len(), 3);
        let mattress = &elements[0];
        assert_eq!(mattress.from(), Vec3::new(0.0, 3.0, 0.0));
        assert_eq!(mattress.to(), Vec3::new(16.0, 9.0, 16.0));
        let top = mattress.faces()[Face::Up.index()].as_ref().unwrap();
        assert_eq!(top.texture(), "bed_red_head_north");
        assert_eq!(top.uv_rotation(), 0);
        // The pillow, at the top of the tile, lies at the south end.
        assert_eq!(top.uv()[1], 16.0);
        let end = mattress.faces()[Face::South.index()].as_ref().unwrap();
        assert_eq!(end.texture(), "bed_red_head_down");
        assert!(mattress.faces()[Face::North.index()].is_none());
        let leg = &elements[1];
        assert_eq!(leg.from(), Vec3::new(13.0, 0.0, 13.0));
        assert_eq!(leg.to(), Vec3::new(16.0, 3.0, 16.0));
    }

    #[test]
    fn foot_half_hides_the_end_towards_the_head() {
        let elements = elements(
            "minecraft:red_bed",
            &props(&[("part", "foot"), ("facing", "south")]),
        );
        let mattress = &elements[0];
        assert!(mattress.faces()[Face::South.index()].is_none());
        let top = mattress.faces()[Face::Up.index()].as_ref().unwrap();
        assert_eq!(top.texture(), "bed_red_foot_north");
        let leg = &elements[1];
        assert_eq!(leg.from().z, 0.0);
    }

    #[test]
    fn atlas_tiles_cover_both_halves() {
        let tiles = tiles("red");
        let region = |key: &str| tiles.iter().find(|tile| tile.key == key).unwrap().region;
        assert_eq!(region("bed_red_head_north"), [6, 6, 16, 16]);
        assert_eq!(region("bed_red_foot_north"), [6, 28, 16, 16]);
        assert!(tiles.iter().all(|tile| tile.key != "bed_red_head_up"));
        assert!(tiles.iter().all(|tile| tile.key != "bed_red_foot_down"));
    }
}
