use std::collections::HashMap;

use glam::IVec3;

/// Sparse voxel grid of placed blocks. Values are palette names from the
/// right-sidebar picker (model stems), stored in default orientation.
#[derive(Debug, Default)]
pub struct World {
    cells: HashMap<IVec3, String>,
    dirty: bool,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn is_occupied(&self, cell: IVec3) -> bool {
        self.cells.contains_key(&cell)
    }

    /// Places `name` in `cell` if that cell is empty.
    ///
    /// `name` is the current right-sidebar selection. Returns whether a block
    /// was inserted.
    pub fn place(&mut self, cell: IVec3, name: impl Into<String>) -> bool {
        use std::collections::hash_map::Entry;
        match self.cells.entry(cell) {
            Entry::Occupied(_) => false,
            Entry::Vacant(slot) => {
                slot.insert(name.into());
                self.dirty = true;
                true
            }
        }
    }

    /// Removes the block in `cell`. Returns whether a block was there.
    pub fn remove(&mut self, cell: IVec3) -> bool {
        let removed = self.cells.remove(&cell).is_some();
        self.dirty |= removed;
        removed
    }

    pub fn iter(&self) -> impl Iterator<Item = (IVec3, &str)> {
        self.cells.iter().map(|(cell, name)| (*cell, name.as_str()))
    }

    /// Marks the GPU mesh out of date (for example after a model-cache reload).
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn take_dirty(&mut self) -> bool {
        let dirty = self.dirty;
        self.dirty = false;
        dirty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_fills_empty_cell_and_rejects_occupied() {
        let mut world = World::new();
        assert!(world.is_empty());
        assert!(world.place(IVec3::ZERO, "stone"));
        assert!(world.is_occupied(IVec3::ZERO));
        assert!(!world.place(IVec3::ZERO, "dirt"));
        assert_eq!(world.iter().count(), 1);
        assert_eq!(world.iter().next().unwrap(), (IVec3::ZERO, "stone"));
    }

    #[test]
    fn remove_clears_occupied_cell_and_ignores_empty() {
        let mut world = World::new();
        world.place(IVec3::ZERO, "stone");
        world.take_dirty();
        assert!(!world.remove(IVec3::X));
        assert!(!world.take_dirty());
        assert!(world.remove(IVec3::ZERO));
        assert!(world.is_empty());
        assert!(world.take_dirty());
    }

    #[test]
    fn take_dirty_clears_after_place() {
        let mut world = World::new();
        assert!(!world.take_dirty());
        world.place(IVec3::new(1, 0, 0), "oak_planks");
        assert!(world.take_dirty());
        assert!(!world.take_dirty());
        world.mark_dirty();
        assert!(world.take_dirty());
    }
}
