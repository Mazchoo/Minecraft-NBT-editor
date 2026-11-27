mod block_state;
pub mod structure;

use std::collections::HashMap;

use glam::IVec3;

pub use block_state::BlockState;

/// Sparse voxel grid of placed blocks, laid out like a structure NBT file: a
/// `palette` of block states, and `blocks` that each refer to one palette
/// entry by index (`state`).
///
/// Palette entries are never removed or reordered, so an index stays valid
/// for as long as the world exists. Entries no block refers to any more are
/// dropped when the structure is written out.
#[derive(Debug, Default)]
pub struct World {
    palette: Vec<BlockState>,
    blocks: HashMap<IVec3, u32>,
    dirty: bool,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a world from a structure file's `palette` and `blocks`. Every
    /// `state` must be an index into `palette`. The palette is kept as it is,
    /// so the indices match the file.
    pub fn from_structure(
        palette: Vec<BlockState>,
        blocks: impl IntoIterator<Item = (IVec3, u32)>,
    ) -> Self {
        let blocks: HashMap<IVec3, u32> = blocks.into_iter().collect();
        debug_assert!(blocks.values().all(|&s| (s as usize) < palette.len()));
        Self {
            palette,
            blocks,
            dirty: true,
        }
    }

    /// Smallest and largest occupied cell on each axis, or `None` when the
    /// world is empty.
    pub fn bounds(&self) -> Option<(IVec3, IVec3)> {
        self.blocks
            .keys()
            .fold(None, |bounds: Option<(IVec3, IVec3)>, &cell| match bounds {
                Some((min, max)) => Some((min.min(cell), max.max(cell))),
                None => Some((cell, cell)),
            })
    }

    pub fn is_occupied(&self, cell: IVec3) -> bool {
        self.blocks.contains_key(&cell)
    }

    /// Block state in `cell`, if any.
    pub fn get(&self, cell: IVec3) -> Option<&BlockState> {
        self.blocks
            .get(&cell)
            .map(|&state| &self.palette[state as usize])
    }

    /// Places `state` in `cell` if that cell is empty, adding it to the
    /// palette when it is not there yet. Returns whether a block was inserted.
    pub fn place(&mut self, cell: IVec3, state: &BlockState) -> bool {
        if self.blocks.contains_key(&cell) {
            return false;
        }
        let index = self.palette_index(state);
        self.blocks.insert(cell, index);
        self.dirty = true;
        true
    }

    /// Removes the block in `cell`. Returns whether a block was there.
    pub fn remove(&mut self, cell: IVec3) -> bool {
        let removed = self.blocks.remove(&cell).is_some();
        self.dirty |= removed;
        removed
    }

    /// Every palette entry, indexed by the `state` values from [`Self::blocks`].
    pub fn palette(&self) -> &[BlockState] {
        &self.palette
    }

    /// Every occupied cell with the palette index of its block state.
    pub fn blocks(&self) -> impl Iterator<Item = (IVec3, u32)> + '_ {
        self.blocks.iter().map(|(cell, state)| (*cell, *state))
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

    fn palette_index(&mut self, state: &BlockState) -> u32 {
        match self.palette.iter().position(|entry| entry == state) {
            Some(index) => index as u32,
            None => {
                self.palette.push(state.clone());
                (self.palette.len() - 1) as u32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(id: &str) -> BlockState {
        BlockState::default_of(id)
    }

    #[test]
    fn place_fills_empty_cell_and_rejects_occupied() {
        let mut world = World::new();
        assert!(world.blocks.is_empty());
        assert!(world.place(IVec3::ZERO, &state("stone")));
        assert!(world.is_occupied(IVec3::ZERO));
        assert!(!world.place(IVec3::ZERO, &state("dirt")));
        assert_eq!(world.blocks().count(), 1);
        assert_eq!(world.get(IVec3::ZERO), Some(&state("stone")));
    }

    #[test]
    fn identical_states_share_one_palette_entry() {
        let mut world = World::new();
        let mut log_x = state("oak_log");
        log_x.properties.insert("axis".into(), "x".into());
        world.place(IVec3::ZERO, &state("oak_log"));
        world.place(IVec3::X, &log_x);
        world.place(IVec3::Y, &state("oak_log"));
        assert_eq!(world.palette(), &[state("oak_log"), log_x.clone()]);
        let mut indices: Vec<_> = world.blocks().map(|(_, i)| i).collect();
        indices.sort();
        assert_eq!(indices, [0, 0, 1]);
        assert_eq!(world.get(IVec3::X), Some(&log_x));
    }

    #[test]
    fn remove_clears_occupied_cell_and_ignores_empty() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        world.take_dirty();
        assert!(!world.remove(IVec3::X));
        assert!(!world.take_dirty());
        assert!(world.remove(IVec3::ZERO));
        assert!(world.blocks.is_empty());
        assert!(world.take_dirty());
    }

    #[test]
    fn palette_indices_stay_valid_after_removal() {
        let mut world = World::new();
        world.place(IVec3::ZERO, &state("stone"));
        world.place(IVec3::X, &state("dirt"));
        world.remove(IVec3::ZERO);
        world.place(IVec3::Y, &state("granite"));
        assert_eq!(world.get(IVec3::X), Some(&state("dirt")));
        assert_eq!(world.get(IVec3::Y), Some(&state("granite")));
    }

    #[test]
    fn take_dirty_clears_after_place() {
        let mut world = World::new();
        assert!(!world.take_dirty());
        world.place(IVec3::new(1, 0, 0), &state("oak_planks"));
        assert!(world.take_dirty());
        assert!(!world.take_dirty());
        world.mark_dirty();
        assert!(world.take_dirty());
    }
}
