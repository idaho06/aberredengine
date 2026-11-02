//! Per-entity dialogue graph runner.
//!
//! [`DialogueRunner`] walks a shared [`DialogueTree`] on an entity. It holds
//! data only: game code sets a pending [`DialogueRequest`] with
//! [`advance`](DialogueRunner::advance), [`choose`](DialogueRunner::choose) or
//! [`cancel`](DialogueRunner::cancel), and a system — not part of this module —
//! applies the request later and moves the [`DialogueStatus`] forward. The
//! read methods ([`status`](DialogueRunner::status),
//! [`current_node`](DialogueRunner::current_node),
//! [`choices`](DialogueRunner::choices), ...) exist for UI rendering.
//!
//! The runner never hides a locked choice:
//! [`choices`](DialogueRunner::choices) lists every declared choice of the
//! current node, and the `available` field records which ones passed their
//! condition when the node was entered, so the game can grey a locked choice
//! out or skip it.

use std::sync::Arc;

use bevy_ecs::prelude::Component;
use smallvec::SmallVec;

use crate::resources::dialogue_tree::{DialogueChoice, DialogueNode, DialogueTree, Next, NodeIdx};

/// Why a conversation reached [`DialogueStatus::Finished`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueEndReason {
    /// The conversation finished normally.
    Completed,
    /// The conversation was cancelled by request.
    Cancelled,
    /// The conversation entered a choices node where no choice is available.
    NoChoices,
}

/// Where a [`DialogueRunner`] is in its conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueStatus {
    /// The conversation has not started yet; no node has been shown.
    NotStarted,
    /// The conversation is showing the node at this index.
    Active(NodeIdx),
    /// The conversation is over.
    Finished {
        /// The last node shown, or `None` when the conversation was cancelled
        /// before it started.
        last: Option<NodeIdx>,
        /// Why the conversation ended.
        reason: DialogueEndReason,
    },
}

/// A request set by game code; the last one set before the system runs wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueRequest {
    /// Move past the current node.
    Advance,
    /// Pick the choice at this declared index of the current node.
    Choose(usize),
    /// End the conversation immediately.
    Cancel,
}

/// One declared choice of the current node, as exposed to the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceView<'a> {
    /// Declared index of the choice in the node's choice list.
    pub index: usize,
    /// Label shown for the choice.
    pub text: &'a str,
    /// Opaque labels attached to the choice.
    pub labels: &'a [Arc<str>],
    /// Whether the choice's condition passed when the node was entered.
    pub available: bool,
}

/// Per-entity runner over a shared [`DialogueTree`]; see the [module docs](self).
#[derive(Component, Debug, Clone)]
pub struct DialogueRunner {
    /// The conversation being run.
    pub(crate) tree: Arc<DialogueTree>,
    /// Where the runner is in the conversation.
    pub(crate) status: DialogueStatus,
    /// For the current node, whether declared choice `i` passed its condition
    /// when the node was entered; empty before start and for nodes without
    /// choices. Entries missing past the end of the list read as `false`.
    pub(crate) available: SmallVec<[bool; 8]>,
    /// The pending request, applied later by a system.
    pub(crate) pending: Option<DialogueRequest>,
}

impl DialogueRunner {
    /// Creates a runner for `tree` that has not started yet.
    pub fn new(tree: Arc<DialogueTree>) -> Self {
        Self {
            tree,
            status: DialogueStatus::NotStarted,
            available: SmallVec::new(),
            pending: None,
        }
    }

    /// Requests advancing past the current node. Overwrites any pending
    /// request.
    pub fn advance(&mut self) {
        self.pending = Some(DialogueRequest::Advance);
    }

    /// Requests picking declared choice `declared_index` of the current node.
    /// Overwrites any pending request.
    pub fn choose(&mut self, declared_index: usize) {
        self.pending = Some(DialogueRequest::Choose(declared_index));
    }

    /// Requests cancelling the conversation. Overwrites any pending request.
    pub fn cancel(&mut self) {
        self.pending = Some(DialogueRequest::Cancel);
    }

    /// Returns the shared conversation tree.
    pub fn tree(&self) -> &Arc<DialogueTree> {
        &self.tree
    }

    /// Returns the current status.
    pub fn status(&self) -> DialogueStatus {
        self.status
    }

    /// Returns the pending request, if any.
    pub fn pending(&self) -> Option<DialogueRequest> {
        self.pending
    }

    /// Returns true while the status is [`DialogueStatus::Active`].
    pub fn is_active(&self) -> bool {
        matches!(self.status, DialogueStatus::Active(_))
    }

    /// Returns true once the status is [`DialogueStatus::Finished`].
    pub fn is_finished(&self) -> bool {
        matches!(self.status, DialogueStatus::Finished { .. })
    }

    /// Returns why the conversation ended, or `None` while it has not
    /// finished.
    pub fn end_reason(&self) -> Option<DialogueEndReason> {
        match self.status {
            DialogueStatus::Finished { reason, .. } => Some(reason),
            _ => None,
        }
    }

    /// Returns the current node: the node of [`DialogueStatus::Active`], or of
    /// [`DialogueStatus::Finished`] when its `last` is `Some` — a finished
    /// conversation still shows its last line — and `None` otherwise.
    pub fn current_node(&self) -> Option<&DialogueNode> {
        self.current_idx().map(|idx| self.tree.node(idx))
    }

    /// Returns the index of the node the status points at, if any.
    fn current_idx(&self) -> Option<NodeIdx> {
        match self.status {
            DialogueStatus::Active(idx) => Some(idx),
            DialogueStatus::Finished { last, .. } => last,
            DialogueStatus::NotStarted => None,
        }
    }

    /// Returns the id of the current node, if any; see
    /// [`current_node`](Self::current_node).
    pub fn node_id(&self) -> Option<&str> {
        self.current_node().map(|node| &*node.id)
    }

    /// Returns the speaker of the current node, if any.
    pub fn speaker(&self) -> Option<&str> {
        self.current_node().and_then(|node| node.speaker.as_deref())
    }

    /// Returns the line of the current node, if any.
    pub fn text(&self) -> Option<&str> {
        self.current_node().map(|node| &*node.text)
    }

    /// Returns the labels of the current node; empty when there is no current
    /// node.
    pub fn labels(&self) -> &[Arc<str>] {
        self.current_node().map_or(&[], |node| &node.labels[..])
    }

    /// Returns the choice list of the current node while active on a choices
    /// node, and an empty slice in every other state. A validated tree never
    /// declares an empty choice list, so the result is non-empty exactly when
    /// the active node offers choices.
    fn active_choices(&self) -> &[DialogueChoice] {
        match self.status {
            DialogueStatus::Active(_) => match self.current_node().map(|node| &node.next) {
                Some(Next::Choices(list)) => list.as_slice(),
                _ => &[],
            },
            _ => &[],
        }
    }

    /// Returns true while active on a node whose [`Next`] is a choice list.
    pub fn has_choices(&self) -> bool {
        !self.active_choices().is_empty()
    }

    /// Iterates every declared choice of the current node in declared order —
    /// locked ones included, with `available == false` — while active on a
    /// choices node. Empty in every other state, including
    /// [`DialogueStatus::Finished`].
    pub fn choices(&self) -> impl Iterator<Item = ChoiceView<'_>> + '_ {
        self.active_choices()
            .iter()
            .enumerate()
            .map(move |(index, choice)| ChoiceView {
                index,
                text: &choice.text,
                labels: &choice.labels[..],
                available: self.available.get(index).copied().unwrap_or(false),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::dialogue_builder::DialogueBuilder;
    use crate::resources::dialogue_def::DialogueCondition;

    /// Builds the shared test conversation: `ask` (Guard, "Pass?", label "q")
    /// offers "Yes" -> `yes` (label "polite"), "Secret" -> `yes` (needs flag
    /// "s") and "No" -> end; `yes` (Guard, "Welcome") goes to `bye`; `bye`
    /// says "Bye".
    fn test_tree() -> Arc<DialogueTree> {
        Arc::new(
            DialogueBuilder::new("ask")
                .node("ask")
                .speaker("Guard")
                .text("Pass?")
                .label("q")
                .choice("Yes", "yes")
                .label("polite")
                .choice("Secret", "yes")
                .when(DialogueCondition::flag("s"))
                .choice_end("No")
                .node("yes")
                .speaker("Guard")
                .text("Welcome")
                .goto("bye")
                .node("bye")
                .text("Bye")
                .build()
                .unwrap(),
        )
    }

    /// A runner over `tree`, set active on the node `id` with availability
    /// `[true, false, true]`.
    fn active_runner(tree: &Arc<DialogueTree>, id: &str) -> DialogueRunner {
        let mut runner = DialogueRunner::new(Arc::clone(tree));
        runner.status = DialogueStatus::Active(tree.idx_of(id).unwrap());
        runner.available = smallvec::smallvec![true, false, true];
        runner
    }

    #[test]
    fn a_new_runner_has_not_started() {
        let runner = DialogueRunner::new(test_tree());

        assert_eq!(runner.status(), DialogueStatus::NotStarted);
        assert!(!runner.is_active());
        assert!(!runner.is_finished());
        assert_eq!(runner.pending(), None);
        assert_eq!(runner.node_id(), None);
        assert_eq!(runner.speaker(), None);
        assert_eq!(runner.text(), None);
        assert!(runner.labels().is_empty());
        assert!(!runner.has_choices());
        assert_eq!(runner.choices().count(), 0);
        assert_eq!(runner.end_reason(), None);
    }

    #[test]
    fn requests_set_pending_and_the_last_one_wins() {
        let mut runner = DialogueRunner::new(test_tree());

        runner.advance();
        assert_eq!(runner.pending(), Some(DialogueRequest::Advance));
        runner.choose(2);
        assert_eq!(runner.pending(), Some(DialogueRequest::Choose(2)));
        runner.cancel();
        assert_eq!(runner.pending(), Some(DialogueRequest::Cancel));
    }

    #[test]
    fn an_active_choices_node_exposes_its_line() {
        let tree = test_tree();
        let runner = active_runner(&tree, "ask");

        assert_eq!(runner.node_id(), Some("ask"));
        assert_eq!(runner.speaker(), Some("Guard"));
        assert_eq!(runner.text(), Some("Pass?"));
        assert_eq!(runner.labels(), [Arc::from("q")].as_slice());
        assert!(runner.is_active());
        assert!(runner.has_choices());
    }

    #[test]
    fn choices_lists_every_declared_choice_with_availability() {
        let tree = test_tree();
        let runner = active_runner(&tree, "ask");

        let views: Vec<ChoiceView<'_>> = runner.choices().collect();
        assert_eq!(views.len(), 3);

        assert_eq!(views[0].index, 0);
        assert_eq!(views[0].text, "Yes");
        assert_eq!(views[0].labels, [Arc::from("polite")].as_slice());
        assert!(views[0].available);

        assert_eq!(views[1].index, 1);
        assert_eq!(views[1].text, "Secret");
        assert!(views[1].labels.is_empty());
        assert!(!views[1].available, "the locked Secret is still listed");

        assert_eq!(views[2].index, 2);
        assert_eq!(views[2].text, "No");
        assert!(views[2].labels.is_empty());
        assert!(views[2].available);
    }

    #[test]
    fn missing_availability_entries_read_as_unavailable() {
        let tree = test_tree();
        let mut runner = active_runner(&tree, "ask");
        runner.available = smallvec::smallvec![true];

        let views: Vec<ChoiceView<'_>> = runner.choices().collect();
        assert_eq!(views.len(), 3);
        assert!(views[0].available);
        assert!(!views[1].available);
        assert!(!views[2].available);
    }

    #[test]
    fn a_goto_node_has_no_choices() {
        let tree = test_tree();
        let runner = active_runner(&tree, "yes");

        assert!(!runner.has_choices());
        assert_eq!(runner.choices().count(), 0);
        assert_eq!(runner.text(), Some("Welcome"));
    }

    #[test]
    fn a_finished_runner_keeps_its_last_line_but_offers_no_choices() {
        let tree = test_tree();
        let mut runner = DialogueRunner::new(Arc::clone(&tree));
        runner.status = DialogueStatus::Finished {
            last: Some(tree.idx_of("ask").unwrap()),
            reason: DialogueEndReason::Completed,
        };

        assert!(runner.is_finished());
        assert!(!runner.is_active());
        assert_eq!(runner.end_reason(), Some(DialogueEndReason::Completed));
        assert_eq!(runner.text(), Some("Pass?"));
        assert_eq!(runner.node_id(), Some("ask"));
        assert!(!runner.has_choices());
        assert_eq!(runner.choices().count(), 0);
    }

    #[test]
    fn a_runner_cancelled_before_it_started_has_no_line() {
        let tree = test_tree();
        let mut runner = DialogueRunner::new(Arc::clone(&tree));
        runner.status = DialogueStatus::Finished {
            last: None,
            reason: DialogueEndReason::Cancelled,
        };

        assert!(runner.is_finished());
        assert_eq!(runner.end_reason(), Some(DialogueEndReason::Cancelled));
        assert_eq!(runner.node_id(), None);
        assert_eq!(runner.text(), None);
        assert!(runner.labels().is_empty());
    }

    #[test]
    fn tree_returns_the_shared_tree() {
        let tree = test_tree();
        let runner = DialogueRunner::new(Arc::clone(&tree));

        assert!(Arc::ptr_eq(runner.tree(), &tree));
    }
}
