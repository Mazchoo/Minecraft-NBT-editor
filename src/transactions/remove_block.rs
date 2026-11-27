use glam::IVec3;

use super::Transaction;
use crate::world::{BlockState, World};

/// Removes the block in one cell.
pub struct RemoveBlock {
    cell: IVec3,
    /// The block that was removed, captured by `apply` for undo.
    removed: Option<BlockState>,
}

impl RemoveBlock {
    pub fn new(cell: IVec3) -> Self {
        Self {
            cell,
            removed: None,
        }
    }
}

impl Transaction for RemoveBlock {
    /// Does nothing and returns `false` when `cell` is empty.
    fn apply(&mut self, world: &mut World) -> bool {
        self.removed = world.get(self.cell).cloned();
        world.remove(self.cell)
    }

    /// Puts the removed block back. Left alone if the cell has since been
    /// filled.
    fn undo(&self, world: &mut World) {
        if let Some(state) = &self.removed {
            world.place(self.cell, state);
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
    fn removing_the_only_block_is_reversible() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        assert_reversible(&mut world, RemoveBlock::new(IVec3::ZERO));
    }

    #[test]
    fn removing_among_other_blocks_is_reversible() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        world.place(IVec3::new(0, 1, 0), &state("oak_planks"));
        world.place(IVec3::new(1, 0, 0), &state("stone"));
        assert_reversible(&mut world, RemoveBlock::new(IVec3::new(0, 1, 0)));
        assert_reversible(&mut world, RemoveBlock::new(IVec3::ZERO));
    }

    #[test]
    fn removing_a_block_with_properties_is_reversible() {
        let mut world = World::new();
        let mut log_x = state("oak_log");
        log_x.properties.insert("axis".into(), "x".into());
        world.place(IVec3::ZERO, &log_x);
        assert_reversible(&mut world, RemoveBlock::new(IVec3::ZERO));
    }

    #[test]
    fn removing_from_empty_cell_changes_nothing() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        let before = snapshot(&world);
        assert!(!RemoveBlock::new(IVec3::new(0, -1, 0)).apply(&mut world));
        assert_eq!(snapshot(&world), before);
    }

    #[test]
    fn undo_marks_world_dirty() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        let mut removal = RemoveBlock::new(IVec3::ZERO);
        removal.apply(&mut world);
        world.take_dirty();
        removal.undo(&mut world);
        assert!(world.take_dirty());
    }

    #[test]
    fn undo_leaves_a_block_placed_since() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        let mut removal = RemoveBlock::new(IVec3::ZERO);
        removal.apply(&mut world);
        world.place(IVec3::ZERO, &state("dirt"));
        removal.undo(&mut world);
        assert_eq!(world.get(IVec3::ZERO), Some(&state("dirt")));
    }
}
