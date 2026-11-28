//! Hand-built geometry for the conduit.
//!
//! Vanilla draws the conduit with a block-entity renderer. An inactive conduit
//! is its shell alone: a 6x6x6 box at the centre of the cell, cut from the
//! `base` atlas. The cage, eye and wind of an active one are not drawn.

use super::element::Element;
use super::entity_model::{self, AtlasTile, ModelBox, Placement};
use crate::constants::{CONDUIT_TEXTURE, CONDUIT_TILE_PREFIX};
use crate::custom_block_keys::CustomBlockKey;

/// Whether `name` is the conduit (`minecraft:conduit`).
pub fn is_conduit(name: &str) -> bool {
    CustomBlockKey::from_name(name) == Some(CustomBlockKey::Conduit)
}

/// The shell, which is the same in every state.
pub(crate) fn elements() -> Vec<Element> {
    let atlas = entity_model::atlas_stem(CONDUIT_TEXTURE);
    let shell = shell();
    // Vanilla draws the shell Y up, centred on the cell.
    let placement = Placement::new([0, 1, 2], [1.0; 3], [8.0; 3]);
    vec![placement.element(
        &shell,
        |face| entity_model::tile_key(CONDUIT_TILE_PREFIX, atlas, shell.part, face),
        |_| false,
        0.0,
    )]
}

/// Tiles to cut from the `base` atlas.
pub(crate) fn tiles() -> Vec<AtlasTile> {
    let atlas = entity_model::atlas_stem(CONDUIT_TEXTURE);
    entity_model::tiles(CONDUIT_TILE_PREFIX, atlas, &[shell()], |_, _| false)
}

fn shell() -> ModelBox {
    ModelBox::new("shell", [-3.0; 3], [6, 6, 6], [0, 0])
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::models::block::Face;

    #[test]
    fn conduit_is_a_shell_at_the_centre() {
        assert!(is_conduit("minecraft:conduit"));
        let elements = elements();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0].from(), Vec3::splat(5.0));
        assert_eq!(elements[0].to(), Vec3::splat(11.0));
        let face = elements[0].faces()[Face::North.index()].as_ref().unwrap();
        assert_eq!(face.texture(), "conduit_base_shell_north");
        let tiles = tiles();
        assert_eq!(tiles.len(), 6);
        let region = tiles
            .iter()
            .find(|tile| tile.key == "conduit_base_shell_south")
            .unwrap()
            .region;
        assert_eq!(region, [18, 6, 6, 6]);
    }
}
