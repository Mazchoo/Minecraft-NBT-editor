use glam::IVec3;

use super::Transaction;
use crate::world::World;

/// Places one block in an empty cell.
pub struct PlaceBlock {
    cell: IVec3,
    name: String,
}

impl PlaceBlock {
    /// `name` is the palette name of the block to put in `cell`.
    pub fn new(cell: IVec3, name: impl Into<String>) -> Self {
        Self {
            cell,
            name: name.into(),
        }
    }
}

impl Transaction for PlaceBlock {
    /// Does nothing and returns `false` when `cell` is already occupied.
    fn apply(&mut self, world: &mut World) -> bool {
        world.place(self.cell, self.name.as_str())
    }

    /// Removes the placed block. Left alone if the cell no longer holds it,
    /// which can happen when it was deleted by an edit that is not recorded.
    fn undo(&self, world: &mut World) {
        if world.get(self.cell) == Some(self.name.as_str()) {
            world.remove(self.cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::testing::{assert_reversible, snapshot};

    #[test]
    fn placing_in_empty_world_is_reversible() {
        let mut world = World::new();
        assert_reversible(&mut world, PlaceBlock::new(IVec3::ZERO, "stone"));
    }

    #[test]
    fn placing_beside_existing_blocks_is_reversible() {
        let mut world = World::new();
        world.place(IVec3::ZERO, "stone");
        world.place(IVec3::new(0, 1, 0), "oak_planks");
        assert_reversible(&mut world, PlaceBlock::new(IVec3::new(1, 0, 0), "dirt"));
        assert_reversible(&mut world, PlaceBlock::new(IVec3::new(-3, 7, 2), "stone"));
    }

    #[test]
    fn placing_in_occupied_cell_changes_nothing() {
        let mut world = World::new();
        world.place(IVec3::ZERO, "stone");
        let before = snapshot(&world);
        assert!(!PlaceBlock::new(IVec3::ZERO, "dirt").apply(&mut world));
        assert_eq!(snapshot(&world), before);
    }

    #[test]
    fn undo_marks_world_dirty() {
        let mut world = World::new();
        let mut placement = PlaceBlock::new(IVec3::ZERO, "stone");
        placement.apply(&mut world);
        world.take_dirty();
        placement.undo(&mut world);
        assert!(world.take_dirty());
    }

    #[test]
    fn undo_leaves_a_different_block_in_the_cell() {
        let mut world = World::new();
        let mut placement = PlaceBlock::new(IVec3::ZERO, "stone");
        placement.apply(&mut world);
        world.remove(IVec3::ZERO);
        world.place(IVec3::ZERO, "dirt");
        placement.undo(&mut world);
        assert_eq!(world.get(IVec3::ZERO), Some("dirt"));
    }
}
