//! Hand-built geometry for the bell itself.
//!
//! A bell's models hold only its frame. Upstream draws the bell with a
//! hardcoded block-entity renderer; the boxes here stand in for it, textured
//! from the `bell_top`, `bell_side` and `bell_bottom` block textures.

use glam::Vec3;

use super::block::Face;
use super::element::{Element, ElementFace};
use crate::constants::{BELL_BOTTOM_TEXTURE, BELL_SIDE_TEXTURE, BELL_TOP_TEXTURE};
use crate::custom_block_keys::CustomBlockKey;

/// Whether `name` is the bell block (`minecraft:bell`).
pub fn is_bell(name: &str) -> bool {
    CustomBlockKey::from_name(name) == Some(CustomBlockKey::Bell)
}

/// Body and lip of a hanging bell. The same for every attachment and facing.
pub(crate) fn elements() -> Vec<Element> {
    let face = |texture: &str, uv: [f32; 4]| {
        Some(ElementFace::new(texture.to_owned(), uv, 0, None, None))
    };
    let sides = |uv: [f32; 4], up: [f32; 4], down: Option<ElementFace>| {
        let mut faces = [None, None, None, None, None, None];
        faces[Face::Down.index()] = down;
        faces[Face::Up.index()] = face(BELL_TOP_TEXTURE, up);
        for side in [Face::North, Face::South, Face::West, Face::East] {
            faces[side.index()] = face(BELL_SIDE_TEXTURE, uv);
        }
        faces
    };
    vec![
        // Body. Its underside rests on the lip.
        Element::new(
            Vec3::new(5.0, 6.0, 5.0),
            Vec3::new(11.0, 13.0, 11.0),
            None,
            sides([1.0, 0.0, 7.0, 7.0], [1.0, 1.0, 7.0, 7.0], None),
            true,
        ),
        // Lip.
        Element::new(
            Vec3::new(4.0, 4.0, 4.0),
            Vec3::new(12.0, 6.0, 12.0),
            None,
            sides(
                [0.0, 7.0, 8.0, 9.0],
                [0.0, 0.0, 8.0, 8.0],
                face(BELL_BOTTOM_TEXTURE, [0.0, 0.0, 8.0, 8.0]),
            ),
            true,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_bell_block_is_a_bell() {
        assert!(is_bell("minecraft:bell"));
        assert!(is_bell("bell"));
        assert!(!is_bell("minecraft:bellflower"));
        assert!(!is_bell("minecraft:stone"));
    }

    #[test]
    fn bell_is_a_body_on_a_lip() {
        let elements = elements();
        assert_eq!(elements.len(), 2);
        let (body, lip) = (&elements[0], &elements[1]);
        assert_eq!(body.from(), Vec3::new(5.0, 6.0, 5.0));
        assert_eq!(body.to(), Vec3::new(11.0, 13.0, 11.0));
        assert_eq!(lip.from(), Vec3::new(4.0, 4.0, 4.0));
        assert_eq!(lip.to(), Vec3::new(12.0, 6.0, 12.0));

        assert!(body.faces()[Face::Down.index()].is_none());
        let side = body.faces()[Face::North.index()].as_ref().unwrap();
        assert_eq!(side.texture(), "bell_side");
        assert_eq!(side.uv(), [1.0, 0.0, 7.0, 7.0]);
        let under = lip.faces()[Face::Down.index()].as_ref().unwrap();
        assert_eq!(under.texture(), "bell_bottom");
        let rim = lip.faces()[Face::East.index()].as_ref().unwrap();
        assert_eq!(rim.uv(), [0.0, 7.0, 8.0, 9.0]);
    }
}
