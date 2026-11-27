use glam::IVec3;

use super::Transaction;
use crate::world::{BlockState, World};

/// Places one block in an empty cell.
pub struct PlaceBlock {
    cell: IVec3,
    state: BlockState,
}

impl PlaceBlock {
    /// `state` is the palette entry to put in `cell`.
    pub fn new(cell: IVec3, state: BlockState) -> Self {
        Self { cell, state }
    }
}

impl Transaction for PlaceBlock {
    /// Does nothing and returns `false` when `cell` is already occupied.
    fn apply(&mut self, world: &mut World) -> bool {
        world.place(self.cell, &self.state)
    }

    /// Removes the placed block. Left alone if the cell no longer holds it,
    /// which can happen when it was deleted by an edit that is not recorded.
    fn undo(&self, world: &mut World) {
        if world.get(self.cell) == Some(&self.state) {
            world.remove(self.cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::testing::{assert_reversible, snapshot};

    fn state(id: &str) -> BlockState {
        BlockState::default_of(id)
    }

    #[test]
    fn placing_in_empty_world_is_reversible() {
        let mut world = World::new();
        assert_reversible(&mut world, PlaceBlock::new(IVec3::ZERO, state("stone")));
    }

    #[test]
    fn placing_beside_existing_blocks_is_reversible() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        world.place(IVec3::new(0, 1, 0), &state("oak_planks"));
        assert_reversible(
            &mut world,
            PlaceBlock::new(IVec3::new(1, 0, 0), state("dirt")),
        );
        assert_reversible(
            &mut world,
            PlaceBlock::new(IVec3::new(-3, 7, 2), state("stone")),
        );
    }

    #[test]
    fn placing_in_occupied_cell_changes_nothing() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        let before = snapshot(&world);
        assert!(!PlaceBlock::new(IVec3::ZERO, state("dirt")).apply(&mut world));
        assert_eq!(snapshot(&world), before);
    }

    #[test]
    fn undo_marks_world_dirty() {
        let mut world = World::new();
        let mut placement = PlaceBlock::new(IVec3::ZERO, state("stone"));
        placement.apply(&mut world);
        world.take_dirty();
        placement.undo(&mut world);
        assert!(world.take_dirty());
    }

    #[test]
    fn undo_leaves_a_different_block_in_the_cell() {
        let mut world = World::new();
        let mut placement = PlaceBlock::new(IVec3::ZERO, state("stone"));
        placement.apply(&mut world);
        world.remove(IVec3::ZERO);
        world.place(IVec3::ZERO, &state("dirt"));
        placement.undo(&mut world);
        assert_eq!(world.get(IVec3::ZERO), Some(&state("dirt")));
    }

    #[test]
    fn undo_leaves_the_same_block_in_a_different_state() {
        let mut world = World::new();
        let mut placement = PlaceBlock::new(IVec3::ZERO, state("oak_log"));
        placement.apply(&mut world);
        world.remove(IVec3::ZERO);
        let mut log_x = state("oak_log");
        log_x.properties.insert("axis".into(), "x".into());
        world.place(IVec3::ZERO, &log_x);
        placement.undo(&mut world);
        assert_eq!(world.get(IVec3::ZERO), Some(&log_x));
    }
}
