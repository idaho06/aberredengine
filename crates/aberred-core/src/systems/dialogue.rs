//! Dialogue stepping system.
//!
//! [`step_runner`] advances one [`DialogueRunner`] by at most one
//! player-driven transition per call: it takes the runner's pending
//! [`DialogueRequest`], mutates
//! [`WorldSignals`] with the effects of the entered node and of the picked
//! choice, and reports each transition as a [`DialogueStep`] so a later
//! layer can turn it into an event.

use std::sync::Arc;

use smallvec::SmallVec;

use crate::components::dialogue::{
    DialogueEndReason, DialogueRequest, DialogueRunner, DialogueStatus,
};
use crate::resources::dialogue_tree::{DialogueTree, Next, NodeIdx};
use crate::resources::world_signals::WorldSignals;

/// What one call of [`step_runner`] did, in order; a later layer turns each into an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueStep {
    /// A node was entered; `previous` is the node left, `None` for the start node.
    Entered {
        node: NodeIdx,
        previous: Option<NodeIdx>,
    },
    /// A choice was picked on `node`; `index` is its DECLARED index.
    ChoiceSelected { node: NodeIdx, index: usize },
    /// The conversation ended. `node` is the last node shown, `None` when cancelled before start.
    Ended {
        node: Option<NodeIdx>,
        reason: DialogueEndReason,
    },
}

/// Applies the pending request of `runner` — at most one player-driven
/// transition per call — and returns what the call did, in order.
///
/// The pending request is always taken: [`DialogueRunner::pending`] is `None`
/// after the call, even when the request was ignored. A call with no pending
/// request enters the start node while
/// [`NotStarted`](crate::components::dialogue::DialogueStatus::NotStarted) and
/// changes nothing while active or finished.
///
/// - [`NotStarted`](crate::components::dialogue::DialogueStatus::NotStarted):
///   `Cancel` sets
///   [`Finished { last: None, reason: Cancelled }`](crate::components::dialogue::DialogueStatus::Finished)
///   and returns `[DialogueStep::Ended { node: None, reason: Cancelled }]`
///   without applying any `on_enter` effect and without an `Entered` step;
///   `Advance` and `Choose` are dropped with `log::debug!`. Unless the call
///   was cancelled, the start node is then entered with `previous: None`.
/// - [`Active(idx)`](crate::components::dialogue::DialogueStatus::Active):
///   - `Advance` on a [`Goto`](crate::resources::dialogue_tree::Next::Goto)
///     node enters the target with `previous: Some(idx)`; on an
///     [`End`](crate::resources::dialogue_tree::Next::End) node it finishes
///     with [`DialogueEndReason::Completed`] and returns
///     `[DialogueStep::Ended { node: Some(idx), .. }]`; on a
///     [`Choices`](crate::resources::dialogue_tree::Next::Choices) node it is
///     ignored with `log::debug!`.
///   - `Choose(i)` on a choices node with `i` in range and cached
///     availability `i` true applies the choice's effects, pushes
///     `DialogueStep::ChoiceSelected { node: idx, index: i }`, then enters
///     the choice's target with `previous: Some(idx)` — or, when the target
///     is `None`, finishes with `Completed` and pushes
///     `DialogueStep::Ended { node: Some(idx), .. }`. `Choose` on a
///     non-choices node, out of range, or on an unavailable choice logs
///     `log::warn!` naming which of the three and leaves the node, the
///     availability cache and `signals` untouched (no effect is applied).
///   - `Cancel` finishes with [`DialogueEndReason::Cancelled`] and returns
///     `[DialogueStep::Ended { node: Some(idx), .. }]`.
/// - [`Finished`](crate::components::dialogue::DialogueStatus::Finished): the
///   pending request is cleared and ignored with `log::debug!`; the call
///   returns empty.
///
/// Entering a node `n` applies `n.on_enter` to `signals`, then, for a choices
/// node, evaluates each choice's condition against the post-effect signals
/// and caches the result in the runner's availability (cleared for
/// non-choices nodes), sets the status to `Active(n)` and pushes
/// `DialogueStep::Entered { node: n, previous }`. When a choices node has no
/// available choice, the call also logs `log::warn!`, sets
/// `Finished { last: Some(n), reason: NoChoices }` and pushes
/// `DialogueStep::Ended { node: Some(n), reason: NoChoices }` after the
/// `Entered`. Within one transition the order is: choice effects, then
/// `on_enter`, then conditions.
pub fn step_runner(
    runner: &mut DialogueRunner,
    signals: &mut WorldSignals,
) -> SmallVec<[DialogueStep; 3]> {
    let mut steps = SmallVec::new();
    let request = runner.pending.take();
    let tree = Arc::clone(&runner.tree);
    match runner.status {
        DialogueStatus::NotStarted => {
            if request == Some(DialogueRequest::Cancel) {
                finish(runner, None, DialogueEndReason::Cancelled, &mut steps);
            } else {
                if let Some(request) = request {
                    log::debug!(
                        "dropping pending request {request:?} on a not-started dialogue runner"
                    );
                }
                let start = tree.start();
                enter_node(runner, &tree, start, None, signals, &mut steps);
            }
        }
        DialogueStatus::Active(idx) => match request {
            Some(DialogueRequest::Advance) => match &tree.node(idx).next {
                Next::Goto(target) => {
                    let target = *target;
                    enter_node(runner, &tree, target, Some(idx), signals, &mut steps);
                }
                Next::End => finish(runner, Some(idx), DialogueEndReason::Completed, &mut steps),
                Next::Choices(_) => {
                    log::debug!("ignoring Advance on the choices node {idx:?}");
                }
            },
            Some(DialogueRequest::Choose(index)) => match &tree.node(idx).next {
                Next::Choices(choices) => {
                    if index >= choices.len() {
                        log::warn!(
                            "ignoring Choose({index}): out of range, node {idx:?} declares {} choice(s)",
                            choices.len()
                        );
                    } else if !runner.available.get(index).copied().unwrap_or(false) {
                        log::warn!(
                            "ignoring Choose({index}): choice {index} of node {idx:?} is unavailable"
                        );
                    } else {
                        let choice = &choices[index];
                        for effect in &choice.effects {
                            effect.apply(signals);
                        }
                        steps.push(DialogueStep::ChoiceSelected { node: idx, index });
                        match choice.target {
                            Some(target) => {
                                enter_node(runner, &tree, target, Some(idx), signals, &mut steps)
                            }
                            None => {
                                finish(runner, Some(idx), DialogueEndReason::Completed, &mut steps)
                            }
                        }
                    }
                }
                _ => {
                    log::warn!("ignoring Choose({index}): node {idx:?} is not a choices node");
                }
            },
            Some(DialogueRequest::Cancel) => {
                finish(runner, Some(idx), DialogueEndReason::Cancelled, &mut steps);
            }
            None => {}
        },
        DialogueStatus::Finished { .. } => {
            if let Some(request) = request {
                log::debug!("ignoring pending request {request:?} on a finished dialogue runner");
            }
        }
    }
    steps
}

/// Enters `node` of `tree` on `runner`: applies `on_enter` effects, caches
/// choice availability, sets `Active`, and appends [`DialogueStep::Entered`] —
/// plus [`DialogueStep::Ended`] with [`DialogueEndReason::NoChoices`] when a
/// choices node offers no available choice.
fn enter_node(
    runner: &mut DialogueRunner,
    tree: &DialogueTree,
    node: NodeIdx,
    previous: Option<NodeIdx>,
    signals: &mut WorldSignals,
    steps: &mut SmallVec<[DialogueStep; 3]>,
) {
    let dialogue_node = tree.node(node);
    for effect in &dialogue_node.on_enter {
        effect.apply(signals);
    }
    runner.available.clear();
    let no_available_choices = match &dialogue_node.next {
        Next::Choices(choices) => {
            let mut any_available = false;
            for choice in choices {
                let available = choice
                    .condition
                    .as_ref()
                    .is_none_or(|condition| condition.eval(&*signals));
                any_available |= available;
                runner.available.push(available);
            }
            !any_available
        }
        _ => false,
    };
    runner.status = DialogueStatus::Active(node);
    steps.push(DialogueStep::Entered { node, previous });
    if no_available_choices {
        log::warn!("node {node:?} offers no available choice; ending with NoChoices");
        finish(runner, Some(node), DialogueEndReason::NoChoices, steps);
    }
}

/// Sets `runner` to `Finished` with `last`/`reason` and appends the matching
/// [`DialogueStep::Ended`] step.
fn finish(
    runner: &mut DialogueRunner,
    last: Option<NodeIdx>,
    reason: DialogueEndReason,
    steps: &mut SmallVec<[DialogueStep; 3]>,
) {
    runner.status = DialogueStatus::Finished { last, reason };
    steps.push(DialogueStep::Ended { node: last, reason });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::dialogue_builder::DialogueBuilder;
    use crate::resources::dialogue_def::{DialogueCmp, DialogueCondition, DialogueEffect};

    /// Builds and validates a tree from a builder configured by `build`.
    fn build_tree(
        start: &str,
        build: impl FnOnce(DialogueBuilder) -> DialogueBuilder,
    ) -> Arc<DialogueTree> {
        Arc::new(build(DialogueBuilder::new(start)).build().unwrap())
    }

    /// Returns a fresh runner over `tree` and a fresh signal store.
    fn fresh(tree: &Arc<DialogueTree>) -> (DialogueRunner, WorldSignals) {
        (
            DialogueRunner::new(Arc::clone(tree)),
            WorldSignals::default(),
        )
    }

    /// Returns a runner that has entered the start node of `tree`, with a
    /// fresh signal store.
    fn started(tree: &Arc<DialogueTree>) -> (DialogueRunner, WorldSignals) {
        let (mut runner, mut signals) = fresh(tree);
        step_runner(&mut runner, &mut signals);
        (runner, signals)
    }

    /// The `a` -> `b` tree of the start and goto tests: `a` says "A" and goes
    /// to `b`, which says "B" and ends.
    fn a_to_b_tree() -> Arc<DialogueTree> {
        build_tree("a", |b| b.node("a").text("A").goto("b").node("b").text("B"))
    }

    /// The tree of the choice tests: `n` offers a locked choice 0 (needs flag
    /// `"open"`) and an unlocked choice 1, both leading to `x`.
    fn locked_and_open_tree() -> Arc<DialogueTree> {
        build_tree("n", |b| {
            b.node("n")
                .text("pick")
                .choice("locked", "x")
                .when(DialogueCondition::flag("open"))
                .choice("open", "x")
                .node("x")
                .text("X")
        })
    }

    /// The availability of every declared choice of the runner's current node.
    fn availability(runner: &DialogueRunner) -> Vec<bool> {
        runner.choices().map(|choice| choice.available).collect()
    }

    #[test]
    fn first_call_enters_the_start_node_and_drops_preset_requests() {
        let tree = a_to_b_tree();
        let start = tree.start();

        // A pre-set Advance is dropped: the first call enters the start node.
        let (mut runner, mut signals) = fresh(&tree);
        runner.advance();
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Entered {
                node: start,
                previous: None
            }]
        );
        assert_eq!(runner.status(), DialogueStatus::Active(start));
        assert_eq!(runner.pending(), None);

        // A pre-set Choose is dropped the same way on a fresh runner.
        let (mut runner, mut signals) = fresh(&tree);
        runner.choose(0);
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Entered {
                node: start,
                previous: None
            }]
        );
        assert_eq!(runner.status(), DialogueStatus::Active(start));
        assert_eq!(runner.node_id(), Some("a"), "the node stays the start");
        assert_eq!(runner.pending(), None);
    }

    #[test]
    fn advance_walks_a_goto_chain_and_ends_on_an_end_node() {
        let tree = a_to_b_tree();
        let a = tree.idx_of("a").unwrap();
        let b = tree.idx_of("b").unwrap();
        let (mut runner, mut signals) = fresh(&tree);

        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Entered {
                node: a,
                previous: None
            }]
        );

        runner.advance();
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Entered {
                node: b,
                previous: Some(a)
            }]
        );
        assert_eq!(runner.status(), DialogueStatus::Active(b));

        runner.advance();
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Ended {
                node: Some(b),
                reason: DialogueEndReason::Completed
            }]
        );
        assert!(runner.is_finished());
        assert_eq!(runner.end_reason(), Some(DialogueEndReason::Completed));
        assert_eq!(
            runner.text(),
            Some("B"),
            "a finished runner keeps its last line"
        );
    }

    #[test]
    fn choose_uses_the_declared_index() {
        let tree = locked_and_open_tree();
        let n = tree.idx_of("n").unwrap();
        let x = tree.idx_of("x").unwrap();
        let (mut runner, mut signals) = started(&tree);

        runner.choose(1); // the second declared choice, not the locked first
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[
                DialogueStep::ChoiceSelected { node: n, index: 1 },
                DialogueStep::Entered {
                    node: x,
                    previous: Some(n)
                }
            ]
        );
        assert_eq!(runner.status(), DialogueStatus::Active(x));
    }

    #[test]
    fn choices_lists_locked_choices_with_available_false() {
        let tree = locked_and_open_tree();
        let (runner, _) = started(&tree);

        let views: Vec<_> = runner.choices().collect();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].index, 0);
        assert_eq!(views[0].text, "locked");
        assert!(!views[0].available);
        assert_eq!(views[1].index, 1);
        assert_eq!(views[1].text, "open");
        assert!(views[1].available);
    }

    #[test]
    fn conditions_see_choice_effects_and_on_enter_effects() {
        let tree = build_tree("a", |b| {
            b.node("a")
                .text("A")
                .choice("go", "b")
                .effect(DialogueEffect::set_flag("f"))
                .node("b")
                .text("B")
                .on_enter(DialogueEffect::add_integer("n", 1))
                .choice("next", "c")
                .when(DialogueCondition::all(vec![
                    DialogueCondition::flag("f"),
                    DialogueCondition::integer("n", DialogueCmp::Ge, 1),
                ]))
                .choice_end("stop")
                .node("c")
                .text("C")
        });
        let (mut runner, mut signals) = started(&tree);

        runner.choose(0); // pick "go": its effect sets flag "f", then b's on_enter adds n = 1
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(steps.len(), 2, "ChoiceSelected then Entered");

        assert!(signals.has_flag("f"), "the choice effect ran");
        assert_eq!(signals.get_integer("n"), Some(1), "the on_enter effect ran");
        assert_eq!(
            availability(&runner),
            [true, true],
            "b's first choice sees flag \"f\" and n >= 1 from the post-effect signals"
        );
    }

    #[test]
    fn advance_on_choices_and_choose_on_goto_are_ignored() {
        let tree = build_tree("g", |b| {
            b.node("g")
                .text("G")
                .goto("a")
                .node("a")
                .text("A")
                .choice_end("bye")
        });
        let g = tree.idx_of("g").unwrap();
        let a = tree.idx_of("a").unwrap();
        let (mut runner, mut signals) = started(&tree);
        assert_eq!(runner.status(), DialogueStatus::Active(g));
        let _ = signals.snapshot(); // clear dirty

        runner.choose(0); // Choose on a goto node
        let steps = step_runner(&mut runner, &mut signals);
        assert!(steps.is_empty());
        assert_eq!(runner.status(), DialogueStatus::Active(g));
        assert!(!signals.is_dirty(), "signals untouched");
        assert_eq!(runner.pending(), None);

        runner.advance(); // g -> a
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Entered {
                node: a,
                previous: Some(g)
            }]
        );
        let _ = signals.snapshot(); // clear dirty

        runner.advance(); // Advance on a choices node
        let steps = step_runner(&mut runner, &mut signals);
        assert!(steps.is_empty());
        assert_eq!(runner.status(), DialogueStatus::Active(a));
        assert!(!signals.is_dirty(), "signals untouched");
        assert_eq!(runner.pending(), None);
    }

    #[test]
    fn out_of_range_and_locked_chooses_change_nothing() {
        let tree = build_tree("n", |b| {
            b.node("n")
                .text("pick")
                .choice("locked", "x")
                .when(DialogueCondition::flag("never"))
                .effect(DialogueEffect::set_flag("applied"))
                .choice("open", "x")
                .node("x")
                .text("X")
        });
        let n = tree.idx_of("n").unwrap();
        let (mut runner, mut signals) = started(&tree);
        assert_eq!(runner.status(), DialogueStatus::Active(n));
        assert_eq!(availability(&runner), [false, true]);
        let _ = signals.snapshot(); // clear dirty

        runner.choose(9); // out of range
        let steps = step_runner(&mut runner, &mut signals);
        assert!(steps.is_empty());
        assert_eq!(runner.status(), DialogueStatus::Active(n));
        assert_eq!(availability(&runner), [false, true], "available untouched");
        assert!(!signals.has_flag("applied"), "no choice effect ran");
        assert!(!signals.is_dirty(), "signals untouched");
        assert_eq!(runner.pending(), None);

        runner.choose(0); // locked
        let steps = step_runner(&mut runner, &mut signals);
        assert!(steps.is_empty());
        assert_eq!(runner.status(), DialogueStatus::Active(n));
        assert_eq!(availability(&runner), [false, true], "available untouched");
        assert!(
            !signals.has_flag("applied"),
            "the locked choice's effect did not run"
        );
        assert!(!signals.is_dirty(), "signals untouched");
        assert_eq!(runner.pending(), None);
    }

    #[test]
    fn a_choices_node_with_no_available_choice_ends_with_no_choices() {
        // Every choice locked: entering the node ends the conversation.
        let tree = build_tree("s", |b| {
            b.node("s")
                .text("S")
                .goto("n")
                .node("n")
                .text("N")
                .choice("x", "e")
                .when(DialogueCondition::flag("nope"))
                .choice_end("y")
                .when(DialogueCondition::flag("nope2"))
                .node("e")
                .text("E")
        });
        let s = tree.idx_of("s").unwrap();
        let n = tree.idx_of("n").unwrap();
        let (mut runner, mut signals) = started(&tree);

        runner.advance();
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[
                DialogueStep::Entered {
                    node: n,
                    previous: Some(s)
                },
                DialogueStep::Ended {
                    node: Some(n),
                    reason: DialogueEndReason::NoChoices
                }
            ]
        );
        assert_eq!(
            runner.status(),
            DialogueStatus::Finished {
                last: Some(n),
                reason: DialogueEndReason::NoChoices
            }
        );
        assert_eq!(runner.end_reason(), Some(DialogueEndReason::NoChoices));

        // Some locked plus one available: the conversation does not end.
        let tree = build_tree("s", |b| {
            b.node("s")
                .text("S")
                .goto("n")
                .node("n")
                .text("N")
                .choice("x", "e")
                .when(DialogueCondition::flag("nope"))
                .choice("ok", "e")
                .node("e")
                .text("E")
        });
        let s = tree.idx_of("s").unwrap();
        let n = tree.idx_of("n").unwrap();
        let (mut runner, mut signals) = started(&tree);

        runner.advance();
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Entered {
                node: n,
                previous: Some(s)
            }]
        );
        assert_eq!(runner.status(), DialogueStatus::Active(n));
        assert!(!runner.is_finished());
    }

    #[test]
    fn cancel_finishes_the_conversation() {
        let tree = build_tree("s", |b| {
            b.node("s")
                .text("S")
                .on_enter(DialogueEffect::set_flag("entered"))
        });
        let s = tree.idx_of("s").unwrap();

        // Cancel before start: no node entered, no on_enter effect applied.
        let (mut runner, mut signals) = fresh(&tree);
        runner.cancel();
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Ended {
                node: None,
                reason: DialogueEndReason::Cancelled
            }]
        );
        assert_eq!(
            runner.status(),
            DialogueStatus::Finished {
                last: None,
                reason: DialogueEndReason::Cancelled
            }
        );
        assert!(
            !signals.has_flag("entered"),
            "the start node's on_enter effect did not run"
        );
        assert_eq!(runner.pending(), None);

        // Cancel while active.
        let (mut runner, mut signals) = started(&tree);
        assert!(signals.has_flag("entered"));
        runner.cancel();
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Ended {
                node: Some(s),
                reason: DialogueEndReason::Cancelled
            }]
        );
        assert_eq!(
            runner.status(),
            DialogueStatus::Finished {
                last: Some(s),
                reason: DialogueEndReason::Cancelled
            }
        );
    }

    #[test]
    fn requests_on_a_finished_runner_are_ignored() {
        let tree = build_tree("a", |b| b.node("a").text("A"));
        let a = tree.idx_of("a").unwrap();
        let (mut runner, mut signals) = started(&tree);

        runner.advance();
        let steps = step_runner(&mut runner, &mut signals); // a is End: finished
        assert_eq!(
            steps.as_slice(),
            &[DialogueStep::Ended {
                node: Some(a),
                reason: DialogueEndReason::Completed
            }]
        );
        let finished = runner.status();

        runner.advance();
        let steps = step_runner(&mut runner, &mut signals);
        assert!(steps.is_empty());
        assert_eq!(runner.status(), finished, "status unchanged");
        assert_eq!(runner.pending(), None);

        runner.cancel();
        let steps = step_runner(&mut runner, &mut signals);
        assert!(steps.is_empty());
        assert_eq!(runner.status(), finished, "status unchanged");
        assert_eq!(runner.pending(), None);
    }

    #[test]
    fn re_entering_a_node_reevaluates_choice_availability() {
        let tree = build_tree("a", |b| {
            b.node("a")
                .text("A")
                .choice("to_b", "b")
                .choice("secret", "s")
                .when(DialogueCondition::integer("passes", DialogueCmp::Ge, 1))
                .node("b")
                .text("B")
                .on_enter(DialogueEffect::add_integer("passes", 1))
                .choice("back", "a")
                .node("s")
                .text("S")
        });
        let a = tree.idx_of("a").unwrap();
        let b = tree.idx_of("b").unwrap();
        let (mut runner, mut signals) = started(&tree);

        assert_eq!(
            availability(&runner),
            [true, false],
            "first pass: secret is locked"
        );

        runner.choose(0); // to_b
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[
                DialogueStep::ChoiceSelected { node: a, index: 0 },
                DialogueStep::Entered {
                    node: b,
                    previous: Some(a)
                }
            ]
        );
        assert_eq!(signals.get_integer("passes"), Some(1), "b's on_enter ran");

        runner.choose(0); // back to a
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[
                DialogueStep::ChoiceSelected { node: b, index: 0 },
                DialogueStep::Entered {
                    node: a,
                    previous: Some(b)
                }
            ]
        );
        assert_eq!(
            availability(&runner),
            [true, true],
            "second pass: the counter from b's on_enter unlocked secret"
        );
    }

    #[test]
    fn a_choice_without_a_target_ends_the_conversation() {
        let tree = build_tree("n", |b| b.node("n").text("pick").choice_end("bye"));
        let n = tree.idx_of("n").unwrap();
        let (mut runner, mut signals) = started(&tree);

        runner.choose(0);
        let steps = step_runner(&mut runner, &mut signals);
        assert_eq!(
            steps.as_slice(),
            &[
                DialogueStep::ChoiceSelected { node: n, index: 0 },
                DialogueStep::Ended {
                    node: Some(n),
                    reason: DialogueEndReason::Completed
                }
            ]
        );
        assert_eq!(
            runner.status(),
            DialogueStatus::Finished {
                last: Some(n),
                reason: DialogueEndReason::Completed
            }
        );
        assert_eq!(runner.pending(), None);
    }
}
