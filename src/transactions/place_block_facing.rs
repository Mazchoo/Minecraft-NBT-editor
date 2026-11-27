use glam::IVec3;

use super::{PlaceBlock, Transaction};
use crate::models::Face;
use crate::world::{BlockState, World};

/// Blockstate property naming the direction a block points.
const FACING: &str = "facing";

/// Places one block in an empty cell with its bottom facing the block it was
/// placed against: `facing` is set to the face that was clicked, so the block
/// points away from it. Undo is the same as for [`PlaceBlock`].
pub struct PlaceBlockFacing {
    placement: PlaceBlock,
}

impl PlaceBlockFacing {
    /// `state` is the palette entry to put in `cell`; `facing` is the face of
    /// the neighbouring block that `cell` sits against.
    pub fn new(cell: IVec3, mut state: BlockState, facing: Face) -> Self {
        state
            .properties
            .insert(FACING.to_owned(), facing.name().to_owned());
        Self {
            placement: PlaceBlock::new(cell, state),
        }
    }
}

impl Transaction for PlaceBlockFacing {
    /// Does nothing and returns `false` when `cell` is already occupied.
    fn apply(&mut self, world: &mut World) -> bool {
        self.placement.apply(world)
    }

    fn undo(&self, world: &mut World) {
        self.placement.undo(world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::testing::assert_reversible;

    fn state(id: &str) -> BlockState {
        BlockState::default_of(id)
    }

    #[test]
    fn sets_facing_to_the_clicked_face() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        PlaceBlockFacing::new(IVec3::X, state("end_rod"), Face::East).apply(&mut world);
        let placed = world.get(IVec3::X).unwrap();
        assert_eq!(placed.name, "minecraft:end_rod");
        assert_eq!(
            placed.properties.get(FACING).map(String::as_str),
            Some("east")
        );
    }

    #[test]
    fn replaces_a_facing_already_on_the_state() {
        let mut selected = state("end_rod");
        selected
            .properties
            .insert(FACING.to_owned(), "north".to_owned());
        let mut world = World::new();
        PlaceBlockFacing::new(IVec3::ZERO, selected, Face::Down).apply(&mut world);
        assert_eq!(
            world.get(IVec3::ZERO).unwrap().properties.get(FACING),
            Some(&"down".to_owned())
        );
    }

    #[test]
    fn placing_against_every_face_is_reversible() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        for face in Face::ALL {
            assert_reversible(
                &mut world,
                PlaceBlockFacing::new(face.offset(), state("end_rod"), face),
            );
        }
    }

    #[test]
    fn placing_in_occupied_cell_changes_nothing() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        assert!(!PlaceBlockFacing::new(IVec3::ZERO, state("end_rod"), Face::Up).apply(&mut world));
        assert_eq!(world.get(IVec3::ZERO), Some(&state("stone")));
    }

    #[test]
    fn undo_leaves_the_block_unfaced_by_another_edit() {
        let mut world = World::new();
        let mut placement = PlaceBlockFacing::new(IVec3::ZERO, state("end_rod"), Face::West);
        placement.apply(&mut world);
        world.remove(IVec3::ZERO);
        world.place(IVec3::ZERO, &state("end_rod"));
        placement.undo(&mut world);
        assert_eq!(world.get(IVec3::ZERO), Some(&state("end_rod")));
    }
}
