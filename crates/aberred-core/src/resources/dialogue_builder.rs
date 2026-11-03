//! A consuming builder that assembles a [`DialogueDef`] node by node and
//! compiles it into a validated [`DialogueTree`].
//!
//! [`DialogueBuilder`] chains calls against a current-node cursor (the node
//! started by the last [`DialogueBuilder::node`] call) and a current-choice
//! cursor (the last choice appended to that node). Out-of-order calls do not
//! panic: the first misuse is recorded and [`DialogueBuilder::into_def`] and
//! [`DialogueBuilder::build`] report it as [`DialogueError::Builder`] before
//! any validation error.

use crate::resources::dialogue_def::{
    ChoiceDef, DialogueCondition, DialogueDef, DialogueEffect, NodeDef,
};
use crate::resources::dialogue_tree::{DialogueError, DialogueTree};

/// A consuming builder for a [`DialogueDef`] conversation.
///
/// Every method takes `self` and returns `Self`, so calls chain. Node-level
/// calls ([`speaker`](DialogueBuilder::speaker), [`text`](DialogueBuilder::text),
/// [`on_enter`](DialogueBuilder::on_enter), [`goto`](DialogueBuilder::goto))
/// always target the current node, even after choices were added to it.
/// [`choice`](DialogueBuilder::choice) and [`choice_end`](DialogueBuilder::choice_end)
/// append a choice to the current node and make it the current choice;
/// [`when`](DialogueBuilder::when) and [`effect`](DialogueBuilder::effect)
/// target the current choice. [`label`](DialogueBuilder::label) appends to
/// the current choice when one is active and to the current node otherwise,
/// so a node is labeled by calling `label` before its first choice.
///
/// Calling a method out of order (a node-level method before any
/// [`node`](DialogueBuilder::node) call, or a choice-level method with no
/// current choice) records a misuse instead of panicking: the first misuse
/// wins, later calls keep chaining and leave the builder otherwise unchanged,
/// and [`into_def`](DialogueBuilder::into_def) / [`build`](DialogueBuilder::build)
/// report it as [`DialogueError::Builder`] before any validation error.
///
/// ```
/// use aberred_core::resources::dialogue_builder::DialogueBuilder;
///
/// let def = DialogueBuilder::new("greet")
///     .node("greet")
///     .speaker("Guard")
///     .text("Halt! Who goes there?")
///     .choice("A friend.", "friend")
///     .choice_end("Never mind.")
///     .node("friend")
///     .speaker("Guard")
///     .text("Hm. Move along.")
///     .into_def()
///     .unwrap();
/// ```
#[derive(Debug)]
pub struct DialogueBuilder {
    /// Id of the node the conversation enters first.
    start: String,
    /// Nodes started so far, in call order.
    nodes: Vec<NodeDef>,
    /// The first recorded misuse message; later misuses are ignored.
    misuse: Option<String>,
    /// Whether the current node has a current choice (its last appended one).
    choice_cursor: bool,
}

impl DialogueBuilder {
    /// Starts a builder for a conversation that enters the node `start` first.
    pub fn new(start: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            nodes: Vec::new(),
            misuse: None,
            choice_cursor: false,
        }
    }

    /// Starts a new node with id `id`, appends it after the previous nodes,
    /// and makes it the current node; its text defaults to the empty string
    /// when [`text`](DialogueBuilder::text) is never called. Clears the
    /// choice cursor.
    pub fn node(mut self, id: impl Into<String>) -> Self {
        self.nodes.push(NodeDef {
            id: id.into(),
            speaker: None,
            text: String::new(),
            labels: vec![],
            on_enter: vec![],
            goto: None,
            choices: None,
        });
        self.choice_cursor = false;
        self
    }

    /// Sets the speaker of the current node.
    ///
    /// When no node has been started, records the misuse "`.speaker()` called
    /// before any `.node()`" and leaves the builder otherwise unchanged.
    pub fn speaker(self, speaker: impl Into<String>) -> Self {
        self.on_current_node("`.speaker()` called before any `.node()`", |node| {
            node.speaker = Some(speaker.into());
        })
    }

    /// Sets the text of the current node.
    ///
    /// When no node has been started, records the misuse "`.text()` called
    /// before any `.node()`" and leaves the builder otherwise unchanged.
    pub fn text(self, text: impl Into<String>) -> Self {
        self.on_current_node("`.text()` called before any `.node()`", |node| {
            node.text = text.into();
        })
    }

    /// Appends `effect` to the `on_enter` effects of the current node.
    ///
    /// When no node has been started, records the misuse "`.on_enter()` called
    /// before any `.node()`" and leaves the builder otherwise unchanged.
    pub fn on_enter(self, effect: DialogueEffect) -> Self {
        self.on_current_node("`.on_enter()` called before any `.node()`", |node| {
            node.on_enter.push(effect);
        })
    }

    /// Sets the `goto` target of the current node to the node id `id`.
    ///
    /// When no node has been started, records the misuse "`.goto()` called
    /// before any `.node()`" and leaves the builder otherwise unchanged.
    pub fn goto(self, id: impl Into<String>) -> Self {
        self.on_current_node("`.goto()` called before any `.node()`", |node| {
            node.goto = Some(id.into());
        })
    }

    /// Appends `label` to the current choice when the current node has one,
    /// otherwise to the current node.
    ///
    /// When no node has been started and no choice is current, records the
    /// misuse "`.label()` called before any `.node()`" and leaves the builder
    /// otherwise unchanged.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        if let Some(choice) = self.current_choice_mut() {
            choice.labels.push(label.into());
        } else if let Some(node) = self.current_node_mut() {
            node.labels.push(label.into());
        } else {
            self.record_misuse("`.label()` called before any `.node()`");
        }
        self
    }

    /// Appends a choice with text `text` and `next = Some(next)` to the
    /// current node, and makes it the current choice.
    pub fn choice(self, text: impl Into<String>, next: impl Into<String>) -> Self {
        self.push_choice(
            text.into(),
            Some(next.into()),
            "`.choice()` called before any `.node()`",
        )
    }

    /// Appends a choice with text `text` and `next = None` — picking it ends
    /// the conversation — to the current node, and makes it the current
    /// choice.
    pub fn choice_end(self, text: impl Into<String>) -> Self {
        self.push_choice(
            text.into(),
            None,
            "`.choice_end()` called before any `.node()`",
        )
    }

    /// Sets the `when` condition of the current choice; a second call
    /// replaces the previous condition.
    ///
    /// When there is no current choice, records the misuse "`.when()` called
    /// with no current choice" and leaves the builder otherwise unchanged.
    pub fn when(self, condition: DialogueCondition) -> Self {
        self.on_current_choice("`.when()` called with no current choice", |choice| {
            choice.when = Some(condition);
        })
    }

    /// Appends `effect` to the effects of the current choice.
    ///
    /// When there is no current choice, records the misuse "`.effect()` called
    /// with no current choice" and leaves the builder otherwise unchanged.
    pub fn effect(self, effect: DialogueEffect) -> Self {
        self.on_current_choice("`.effect()` called with no current choice", |choice| {
            choice.effects.push(effect);
        })
    }

    /// Returns the recorded misuse as [`DialogueError::Builder`] when one was
    /// recorded — before any validation — otherwise the [`DialogueDef`] with
    /// the `start` id and the nodes in call order. Performs no id validation
    /// itself.
    pub fn into_def(self) -> Result<DialogueDef, DialogueError> {
        if let Some(message) = self.misuse {
            return Err(DialogueError::Builder { message });
        }
        Ok(DialogueDef {
            start: self.start,
            nodes: self.nodes,
        })
    }

    /// Calls [`into_def`](DialogueBuilder::into_def) and then
    /// `DialogueTree::try_from`: a recorded misuse comes back as
    /// [`DialogueError::Builder`], and validation failures
    /// ([`DialogueError::DuplicateNode`], [`DialogueError::StartMissing`],
    /// [`DialogueError::UnknownTarget`], [`DialogueError::GotoAndChoices`],
    /// [`DialogueError::EmptyChoices`]) pass through unchanged.
    pub fn build(self) -> Result<DialogueTree, DialogueError> {
        DialogueTree::try_from(self.into_def()?)
    }

    /// Returns the current node when at least one node has been started.
    fn current_node_mut(&mut self) -> Option<&mut NodeDef> {
        self.nodes.last_mut()
    }

    /// Returns the current choice — the last choice of the current node while
    /// the choice cursor is set — when there is one.
    fn current_choice_mut(&mut self) -> Option<&mut ChoiceDef> {
        if !self.choice_cursor {
            return None;
        }
        self.nodes.last_mut()?.choices.as_mut()?.last_mut()
    }

    /// Applies `f` to the current node, or records `misuse` when no node has
    /// been started yet.
    fn on_current_node(mut self, misuse: &str, f: impl FnOnce(&mut NodeDef)) -> Self {
        if let Some(node) = self.current_node_mut() {
            f(node);
        } else {
            self.record_misuse(misuse);
        }
        self
    }

    /// Applies `f` to the current choice, or records `misuse` when there is no
    /// current choice.
    fn on_current_choice(mut self, misuse: &str, f: impl FnOnce(&mut ChoiceDef)) -> Self {
        if let Some(choice) = self.current_choice_mut() {
            f(choice);
        } else {
            self.record_misuse(misuse);
        }
        self
    }

    /// Appends a choice with `text` and `next` to the current node and makes it
    /// the current choice, or records `misuse` when no node has been started.
    fn push_choice(mut self, text: String, next: Option<String>, misuse: &str) -> Self {
        if let Some(node) = self.current_node_mut() {
            node.choices.get_or_insert_with(Vec::new).push(ChoiceDef {
                text,
                next,
                when: None,
                effects: vec![],
                labels: vec![],
            });
            self.choice_cursor = true;
        } else {
            self.record_misuse(misuse);
        }
        self
    }

    /// Records `message` as the misuse; the first recorded message wins.
    fn record_misuse(&mut self, message: &str) {
        self.misuse.get_or_insert_with(|| message.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::dialogue_def::test_helpers::{example_def, example_json, s};

    /// The guard-conversation chain shared by the happy-path tests.
    fn guard_chain() -> DialogueBuilder {
        DialogueBuilder::new("greet")
            .node("greet")
            .speaker("Guard")
            .text("Halt! Who goes there?")
            .choice("A friend.", "friend")
            .choice("Here is my pass.", "pass")
            .when(DialogueCondition::flag("has_pass"))
            .effect(DialogueEffect::set_flag("pass_shown"))
            .label("bribe")
            .choice_end("Never mind.")
            .node("friend")
            .speaker("Guard")
            .text("Hm. Move along.")
            .node("pass")
            .speaker("Guard")
            .text("Carry on.")
            .label("gate")
            .on_enter(DialogueEffect::add_integer("gold", -1))
            .goto("friend")
    }

    /// A builder with start id `"a"` and one started node `"a"`, shared by the
    /// small-scenario tests.
    fn node_a() -> DialogueBuilder {
        DialogueBuilder::new("a").node("a")
    }

    /// Extracts the misuse message from an `into_def` result, panicking on
    /// anything else.
    fn misuse_of(result: Result<DialogueDef, DialogueError>) -> String {
        match result {
            Err(DialogueError::Builder { message }) => message,
            other => panic!("expected DialogueError::Builder, got {other:?}"),
        }
    }

    #[test]
    fn the_builder_produces_the_same_definition_as_the_json_example() {
        let def = guard_chain().into_def().unwrap();
        assert_eq!(def, example_def());
        let from_json: DialogueDef = serde_json::from_str(example_json()).unwrap();
        assert_eq!(def, from_json);
    }

    #[test]
    fn build_returns_the_validated_tree() {
        let tree = guard_chain().build().unwrap();
        assert_eq!(tree, DialogueTree::try_from(example_def()).unwrap());
    }

    #[test]
    fn node_level_setters_still_target_the_node_after_choices() {
        let def = node_a()
            .choice_end("x")
            .label("on-choice")
            .speaker("S")
            .text("T")
            .into_def()
            .unwrap();
        let node = &def.nodes[0];
        assert_eq!(node.id, "a");
        assert_eq!(node.speaker.as_deref(), Some("S"));
        assert_eq!(node.text, "T");
        assert!(
            node.labels.is_empty(),
            "label after a choice goes to the choice, not the node"
        );
        let choices = node.choices.as_ref().expect("one choice was added");
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].text, "x");
        assert_eq!(choices[0].next, None);
        assert_eq!(choices[0].labels, vec![s("on-choice")]);
    }

    #[test]
    fn a_node_without_text_defaults_to_empty() {
        let def = node_a().into_def().unwrap();
        assert_eq!(def.nodes[0].text, "");
    }

    #[test]
    fn calling_when_twice_replaces_the_condition() {
        let def = node_a()
            .choice_end("x")
            .when(DialogueCondition::flag("one"))
            .when(DialogueCondition::flag("two"))
            .into_def()
            .unwrap();
        let choice = &def.nodes[0].choices.as_ref().unwrap()[0];
        assert_eq!(choice.when, Some(DialogueCondition::flag("two")));
    }

    #[test]
    fn effects_append_in_order() {
        let def = node_a()
            .choice_end("x")
            .effect(DialogueEffect::set_flag("e1"))
            .effect(DialogueEffect::add_integer("gold", 2))
            .into_def()
            .unwrap();
        let choice = &def.nodes[0].choices.as_ref().unwrap()[0];
        assert_eq!(
            choice.effects,
            vec![
                DialogueEffect::set_flag("e1"),
                DialogueEffect::add_integer("gold", 2)
            ]
        );
    }

    #[test]
    fn text_before_any_node_is_a_misuse() {
        let message = misuse_of(DialogueBuilder::new("a").text("t").into_def());
        assert!(message.contains(".text()"), "got {message}");
    }

    #[test]
    fn speaker_before_any_node_is_a_misuse() {
        let message = misuse_of(DialogueBuilder::new("a").speaker("S").into_def());
        assert!(message.contains(".speaker()"), "got {message}");
    }

    #[test]
    fn on_enter_before_any_node_is_a_misuse() {
        let message = misuse_of(
            DialogueBuilder::new("a")
                .on_enter(DialogueEffect::set_flag("f"))
                .into_def(),
        );
        assert!(message.contains(".on_enter()"), "got {message}");
    }

    #[test]
    fn goto_before_any_node_is_a_misuse() {
        let message = misuse_of(DialogueBuilder::new("a").goto("b").into_def());
        assert!(message.contains(".goto()"), "got {message}");
    }

    #[test]
    fn label_before_any_node_is_a_misuse() {
        let message = misuse_of(DialogueBuilder::new("a").label("l").into_def());
        assert!(message.contains(".label()"), "got {message}");
    }

    #[test]
    fn when_with_no_current_choice_is_a_misuse() {
        let message = misuse_of(node_a().when(DialogueCondition::flag("f")).into_def());
        assert!(message.contains(".when()"), "got {message}");
    }

    #[test]
    fn effect_with_no_current_choice_is_a_misuse() {
        let message = misuse_of(node_a().effect(DialogueEffect::set_flag("f")).into_def());
        assert!(message.contains(".effect()"), "got {message}");
    }

    #[test]
    fn when_right_after_a_new_node_is_a_misuse() {
        let message = misuse_of(
            node_a()
                .choice_end("x")
                .node("b")
                .when(DialogueCondition::flag("f"))
                .into_def(),
        );
        assert!(message.contains(".when()"), "got {message}");
    }

    #[test]
    fn the_first_misuse_wins_over_later_ones() {
        let message = misuse_of(
            DialogueBuilder::new("a")
                .text("t")
                .speaker("S")
                .goto("b")
                .into_def(),
        );
        assert!(message.contains(".text()"), "got {message}");
        assert!(!message.contains(".speaker()"), "got {message}");
    }

    #[test]
    fn a_misuse_precedes_validation_errors() {
        // Two nodes with id "a" would fail validation as DuplicateNode, but
        // the recorded misuse is reported first.
        let result = DialogueBuilder::new("a")
            .goto("b")
            .node("a")
            .node("a")
            .build();
        assert!(
            matches!(&result, Err(DialogueError::Builder { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn goto_and_choices_on_one_node_passes_through_build() {
        let err = node_a()
            .choice_end("x")
            .goto("b")
            .node("b")
            .build()
            .unwrap_err();
        assert!(
            matches!(&err, DialogueError::GotoAndChoices { node } if node == "a"),
            "got {err:?}"
        );
    }

    #[test]
    fn an_unknown_choice_target_passes_through_build() {
        let err = node_a().choice("x", "ghost").build().unwrap_err();
        assert!(
            matches!(&err, DialogueError::UnknownTarget { node, target } if node == "a" && target == "ghost"),
            "got {err:?}"
        );
    }

    #[test]
    fn a_duplicate_node_id_passes_through_build() {
        let err = node_a().node("a").build().unwrap_err();
        assert!(
            matches!(&err, DialogueError::DuplicateNode { id } if id == "a"),
            "got {err:?}"
        );
    }

    #[test]
    fn a_missing_start_with_no_nodes_passes_through_build() {
        let err = DialogueBuilder::new("a").build().unwrap_err();
        assert!(
            matches!(&err, DialogueError::StartMissing { start } if start == "a"),
            "got {err:?}"
        );
    }
}
