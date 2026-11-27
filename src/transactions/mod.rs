//! Undoable edits to the [`World`].
//!
//! Every change the user makes to the world goes through a [`Transaction`],
//! one module per kind. [`History`] applies them and keeps the most recent
//! ones so they can be undone in reverse order.

mod place_block;

use std::collections::VecDeque;

pub use place_block::PlaceBlock;

use crate::world::World;

/// A single reversible edit to the world.
pub trait Transaction {
    /// Performs the edit. Returns `false` when the world was left unchanged,
    /// in which case there is nothing to undo and the transaction is not
    /// recorded.
    fn apply(&mut self, world: &mut World) -> bool;

    /// Restores the world to its state before a successful [`apply`].
    ///
    /// [`apply`]: Transaction::apply
    fn undo(&self, world: &mut World);
}

/// Bounded stack of applied transactions, newest last.
pub struct History {
    applied: VecDeque<Box<dyn Transaction>>,
    limit: usize,
}

impl History {
    /// Creates a history that remembers at most `limit` transactions.
    pub fn new(limit: usize) -> Self {
        Self {
            applied: VecDeque::new(),
            limit,
        }
    }

    /// Changes how many transactions are remembered, forgetting the oldest
    /// ones if more than `limit` are currently recorded.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        self.trim();
    }

    /// Applies `transaction` to `world` and records it for undo. Returns
    /// whether the world changed.
    pub fn execute(
        &mut self,
        world: &mut World,
        mut transaction: impl Transaction + 'static,
    ) -> bool {
        if !transaction.apply(world) {
            return false;
        }
        self.applied.push_back(Box::new(transaction));
        self.trim();
        true
    }

    /// Pops the most recent transaction and undoes it. Returns `false` when
    /// there is nothing to undo.
    pub fn undo(&mut self, world: &mut World) -> bool {
        match self.applied.pop_back() {
            Some(transaction) => {
                transaction.undo(world);
                true
            }
            None => false,
        }
    }

    fn trim(&mut self) {
        while self.applied.len() > self.limit {
            self.applied.pop_front();
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::collections::BTreeMap;

    use super::*;

    /// Order-independent copy of every block in `world`.
    pub fn snapshot(world: &World) -> BTreeMap<[i32; 3], String> {
        world
            .iter()
            .map(|(cell, name)| (cell.to_array(), name.to_owned()))
            .collect()
    }

    /// Asserts that applying `transaction` changes `world` and that undoing
    /// it restores exactly the blocks that were there before. Every
    /// transaction type must pass this for each distinct way it can apply.
    pub fn assert_reversible(world: &mut World, mut transaction: impl Transaction) {
        let before = snapshot(world);
        assert!(transaction.apply(world), "transaction did not apply");
        assert_ne!(snapshot(world), before, "apply left the world unchanged");
        transaction.undo(world);
        assert_eq!(snapshot(world), before, "undo did not restore the world");
    }
}

#[cfg(test)]
mod tests {
    use glam::IVec3;

    use super::testing::snapshot;
    use super::*;

    fn place(x: i32) -> PlaceBlock {
        PlaceBlock::new(IVec3::new(x, 0, 0), "stone")
    }

    #[test]
    fn undo_reverts_transactions_newest_first() {
        let mut world = World::new();
        let mut history = History::new(10);
        let empty = snapshot(&world);
        assert!(history.execute(&mut world, place(0)));
        let one = snapshot(&world);
        assert!(history.execute(&mut world, place(1)));

        assert!(history.undo(&mut world));
        assert_eq!(snapshot(&world), one);
        assert!(history.undo(&mut world));
        assert_eq!(snapshot(&world), empty);
        assert!(!history.undo(&mut world));
    }

    #[test]
    fn transaction_that_changes_nothing_is_not_recorded() {
        let mut world = World::new();
        let mut history = History::new(10);
        history.execute(&mut world, place(0));
        assert!(!history.execute(&mut world, place(0)));

        assert!(history.undo(&mut world));
        assert!(!history.undo(&mut world));
    }

    #[test]
    fn limit_forgets_oldest_transactions() {
        let mut world = World::new();
        let mut history = History::new(2);
        for x in 0..3 {
            history.execute(&mut world, place(x));
        }

        assert!(history.undo(&mut world));
        assert!(history.undo(&mut world));
        assert!(!history.undo(&mut world));
        // The first placement fell out of the history and stays.
        assert_eq!(world.get(IVec3::ZERO), Some("stone"));
        assert_eq!(world.iter().count(), 1);
    }

    #[test]
    fn lowering_limit_trims_existing_history() {
        let mut world = World::new();
        let mut history = History::new(5);
        for x in 0..3 {
            history.execute(&mut world, place(x));
        }
        history.set_limit(1);

        assert!(history.undo(&mut world));
        assert!(!history.undo(&mut world));
        assert_eq!(world.iter().count(), 2);
    }

    #[test]
    fn zero_limit_records_nothing() {
        let mut world = World::new();
        let mut history = History::new(0);
        assert!(history.execute(&mut world, place(0)));
        assert!(!history.undo(&mut world));
        assert!(world.is_occupied(IVec3::ZERO));
    }
}
