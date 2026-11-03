//! Named registry of loaded dialogue trees.
//!
//! [`DialogueStore`] keeps each validated [`DialogueTree`] behind an [`Arc`]
//! under a caller-chosen key, so any number of
//! [`DialogueRunner`](crate::components::dialogue::DialogueRunner)s can share
//! one tree: look the tree up with [`get`](DialogueStore::get) and clone the
//! `Arc` into [`DialogueRunner::new`](crate::components::dialogue::DialogueRunner::new).

use std::sync::Arc;

use bevy_ecs::prelude::*;
use rustc_hash::FxHashMap;

use crate::resources::dialogue_tree::DialogueTree;

/// Loaded dialogue trees keyed by name.
#[derive(Resource, Debug, Clone, Default)]
pub struct DialogueStore {
    trees: FxHashMap<String, Arc<DialogueTree>>,
}

impl DialogueStore {
    /// Stores `tree` under `key` and returns the tree it replaced, if any.
    /// Runners already holding the replaced tree keep running it.
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        tree: Arc<DialogueTree>,
    ) -> Option<Arc<DialogueTree>> {
        self.trees.insert(key.into(), tree)
    }

    /// Returns the tree stored under `key`, or `None` for an unknown key.
    pub fn get(&self, key: &str) -> Option<&Arc<DialogueTree>> {
        self.trees.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::dialogue_builder::DialogueBuilder;

    /// A one-node tree whose only node is `id`.
    fn tree(id: &str) -> Arc<DialogueTree> {
        Arc::new(
            DialogueBuilder::new(id)
                .node(id)
                .text("line")
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn get_of_an_unknown_key_is_none() {
        let mut store = DialogueStore::default();
        assert!(store.get("missing").is_none());

        store.insert("intro", tree("a"));
        assert!(store.get("missing").is_none());
    }

    #[test]
    fn get_returns_the_inserted_tree() {
        let mut store = DialogueStore::default();
        let intro = tree("a");

        assert!(store.insert("intro", Arc::clone(&intro)).is_none());

        let got = store.get("intro").expect("intro is stored");
        assert!(
            Arc::ptr_eq(got, &intro),
            "the stored Arc is shared, not copied"
        );
    }

    #[test]
    fn insert_replaces_and_returns_the_old_tree() {
        let mut store = DialogueStore::default();
        let first = tree("a");
        let second = tree("b");
        store.insert("intro", Arc::clone(&first));

        let old = store
            .insert("intro", Arc::clone(&second))
            .expect("a tree was stored under the key");

        assert!(Arc::ptr_eq(&old, &first));
        assert!(Arc::ptr_eq(store.get("intro").unwrap(), &second));
    }

    #[test]
    fn keys_are_independent() {
        let mut store = DialogueStore::default();
        let a = tree("a");
        let b = tree("b");
        store.insert("one", Arc::clone(&a));
        store.insert("two", Arc::clone(&b));

        assert!(Arc::ptr_eq(store.get("one").unwrap(), &a));
        assert!(Arc::ptr_eq(store.get("two").unwrap(), &b));
    }
}
