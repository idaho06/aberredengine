//! The compiled dialogue tree: the validated, index-addressed form of a
//! [`DialogueDef`] conversation, ready for runtime traversal.
//!
//! The `TryFrom<DialogueDef>` implementation on [`DialogueTree`] validates
//! the authored definition and compiles it: nodes are addressed by
//! [`NodeIdx`] instead of string ids, links are resolved into [`Next`]
//! (`End`, `Goto` or `Choices`), and every `String`/`Vec<String>` payload is
//! interned as `Arc<str>`. [`load_dialogue`] reads such a definition from a
//! JSON file and compiles it. Validation failures are reported as
//! [`DialogueError`]; see the `TryFrom<DialogueDef>` contract for the exact
//! check order.

use std::path::Path;
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::resources::dialogue_def::{
    ChoiceDef, DialogueCondition, DialogueDef, DialogueEffect, NodeDef,
};
use crate::resources::signal_text::{SignalTextError, validate_signal_text};

/// Index of a [`DialogueNode`] inside a [`DialogueTree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIdx(u32);

impl NodeIdx {
    /// Returns the index as a `usize`, for slicing into [`DialogueTree::nodes`].
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// Builds the index of the node at position `i` in definition order.
    fn new(i: usize) -> Self {
        Self(u32::try_from(i).expect("a dialogue has fewer than u32::MAX nodes"))
    }
}

/// Where the conversation goes after a [`DialogueNode`].
#[derive(Debug, Clone, PartialEq)]
pub enum Next {
    /// The node is terminal: the definition set neither `next` nor `choices`.
    End,
    /// Unconditional jump to another node.
    Goto(NodeIdx),
    /// Choices offered to the player.
    Choices(Vec<DialogueChoice>),
}

/// One choice offered at a [`DialogueNode`].
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueChoice {
    /// Label shown for this choice.
    pub text: Arc<str>,
    /// Node the choice leads to; `None` ends the conversation.
    pub target: Option<NodeIdx>,
    /// Condition that must hold for the choice to be offered.
    pub condition: Option<DialogueCondition>,
    /// Effects applied when the choice is picked.
    pub effects: Vec<DialogueEffect>,
    /// Opaque labels attached to the choice.
    pub labels: Arc<[Arc<str>]>,
}

/// One node of a [`DialogueTree`].
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueNode {
    /// Node id, as authored.
    pub id: Arc<str>,
    /// Speaker shown for this line; `None` for narration.
    pub speaker: Option<Arc<str>>,
    /// The line spoken at this node.
    pub text: Arc<str>,
    /// Opaque labels attached to the node.
    pub labels: Arc<[Arc<str>]>,
    /// Effects applied when the node is entered.
    pub on_enter: Vec<DialogueEffect>,
    /// Where the conversation goes next.
    pub next: Next,
}

/// A validated, index-addressed dialogue conversation.
///
/// Built from a [`DialogueDef`] via `TryFrom<DialogueDef>` or
/// [`load_dialogue`]; all fields are private and read through the accessors.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueTree {
    /// Index of the node the conversation enters first.
    start: NodeIdx,
    /// All nodes, in definition order.
    nodes: Vec<DialogueNode>,
    /// Lookup from node id to index; never iterated.
    names: FxHashMap<Arc<str>, NodeIdx>,
}

impl DialogueTree {
    /// Returns the node the conversation enters first.
    pub fn start(&self) -> NodeIdx {
        self.start
    }

    /// Returns the node at `idx`.
    ///
    /// # Panics
    /// Panics when `idx` is out of range; resolve ids with
    /// [`DialogueTree::idx_of`] first.
    pub fn node(&self, idx: NodeIdx) -> &DialogueNode {
        &self.nodes[idx.index()]
    }

    /// Returns all nodes, in definition order.
    pub fn nodes(&self) -> &[DialogueNode] {
        &self.nodes
    }

    /// Returns the index of the node with id `id`, or `None` when no node has it.
    pub fn idx_of(&self, id: &str) -> Option<NodeIdx> {
        self.names.get(id).copied()
    }

    /// Returns the number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true when the tree has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns the nodes not reachable from [`DialogueTree::start`] by
    /// following [`Next::Goto`] and every [`Next::Choices`] target, in
    /// ascending index order.
    pub fn unreachable_nodes(&self) -> Vec<NodeIdx> {
        let mut visited = vec![false; self.nodes.len()];
        let mut stack = vec![self.start];
        while let Some(idx) = stack.pop() {
            let i = idx.index();
            if visited[i] {
                continue;
            }
            visited[i] = true;
            match &self.nodes[i].next {
                Next::Goto(target) => stack.push(*target),
                Next::Choices(choices) => {
                    for choice in choices {
                        if let Some(target) = choice.target {
                            stack.push(target);
                        }
                    }
                }
                Next::End => {}
            }
        }
        visited
            .iter()
            .enumerate()
            .filter(|(_, seen)| !**seen)
            .map(|(i, _)| NodeIdx::new(i))
            .collect()
    }
}

/// Which text field of a dialogue node a malformed placeholder was found in.
///
/// Formats as `text`, `speaker`, or `choice {i} text` with the choice's
/// zero-based index in the node's choices list, for use inside the
/// [`DialogueError::BadPlaceholder`] message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogueTextField {
    /// The node's `text`.
    Text,
    /// The node's `speaker`, when set.
    Speaker,
    /// The `text` of the choice at this zero-based index in the node's choices list.
    Choice(usize),
}

impl std::fmt::Display for DialogueTextField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => f.write_str("text"),
            Self::Speaker => f.write_str("speaker"),
            Self::Choice(i) => write!(f, "choice {i} text"),
        }
    }
}

/// Errors returned when loading or validating a [`DialogueDef`].
#[derive(Debug, thiserror::Error)]
pub enum DialogueError {
    /// The definition's `start` id names no node.
    #[error("dialogue start \"{start}\" is not among the defined nodes")]
    StartMissing {
        /// The `start` id that matched no node.
        start: String,
    },
    /// Two nodes share the same id.
    #[error("duplicate dialogue node id \"{id}\"")]
    DuplicateNode {
        /// The duplicated node id.
        id: String,
    },
    /// A node or choice `next` names an id no node has.
    #[error("dialogue node \"{node}\" links to unknown node \"{target}\"")]
    UnknownTarget {
        /// Id of the node holding the dangling link.
        node: String,
        /// The unknown target id.
        target: String,
    },
    /// A node sets both `next` and `choices`.
    #[error("dialogue node \"{node}\" sets both next and choices; pick one")]
    NextAndChoices {
        /// Id of the node with both link kinds.
        node: String,
    },
    /// A node has an explicit empty choices list.
    #[error("dialogue node \"{node}\" has an explicit empty choices list")]
    EmptyChoices {
        /// Id of the node with `choices: Some(vec![])`.
        node: String,
    },
    /// A node `text`, `speaker` or choice `text` has a malformed `{...}` placeholder.
    #[error("dialogue node \"{node}\" has a malformed placeholder in its {field}: {source}")]
    BadPlaceholder {
        /// Id of the node holding the malformed template.
        node: String,
        /// Which text field of the node holds it.
        field: DialogueTextField,
        /// Why the template is malformed.
        #[source]
        source: SignalTextError,
    },
    /// A [`DialogueBuilder`](crate::resources::dialogue_builder::DialogueBuilder)
    /// method was called out of order.
    #[error("dialogue builder misuse: {message}")]
    Builder {
        /// What the out-of-order call was, e.g. "`.text()` called before any `.node()`".
        message: String,
    },
    /// The dialogue file could not be read.
    #[error("failed to read dialogue file: {0}")]
    Io(#[from] std::io::Error),
    /// The dialogue file is not valid JSON for a [`DialogueDef`].
    #[error("failed to parse dialogue file as JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Interns owned label strings into one shared slice.
fn intern_labels(labels: Vec<String>) -> Arc<[Arc<str>]> {
    labels.into_iter().map(Arc::from).collect()
}

/// Validates a [`DialogueDef`] and compiles it into a [`DialogueTree`].
///
/// Only the FIRST error is reported, checked in this order:
///
/// 1. duplicate node id, in node order ([`DialogueError::DuplicateNode`]);
/// 2. `start` id not among the nodes ([`DialogueError::StartMissing`]);
/// 3. per node in node order: both `next` and `choices` set
///    ([`DialogueError::NextAndChoices`]); `choices == Some(vec![])`
///    ([`DialogueError::EmptyChoices`]); a malformed `{...}` placeholder in
///    `text`, then in `speaker` when set ([`DialogueError::BadPlaceholder`]);
///    `next` target unknown ([`DialogueError::UnknownTarget`]); then each
///    choice in choice order: a malformed placeholder in the choice `text`
///    ([`DialogueError::BadPlaceholder`]), then that choice's `next` target
///    ([`DialogueError::UnknownTarget`]).
///
/// Placeholder templates are checked with [`validate_signal_text`] but
/// stored raw: substitution happens at render time, not here.
///
/// A node with neither `next` nor `choices` is terminal ([`Next::End`]);
/// `choices: None` and an absent `choices` key are the same. Unreachable
/// nodes are NOT an error: they produce one `log::warn!` listing the
/// unreachable node ids. Loops and joins (several nodes targeting one node,
/// cycles) are valid. Labels/speaker/text convert to `Arc<str>`.
impl TryFrom<DialogueDef> for DialogueTree {
    type Error = DialogueError;

    fn try_from(def: DialogueDef) -> Result<Self, Self::Error> {
        let DialogueDef { start, nodes } = def;

        let mut names: FxHashMap<Arc<str>, NodeIdx> = FxHashMap::default();
        for (i, node) in nodes.iter().enumerate() {
            if names
                .insert(Arc::from(node.id.as_str()), NodeIdx::new(i))
                .is_some()
            {
                return Err(DialogueError::DuplicateNode {
                    id: node.id.clone(),
                });
            }
        }

        let start_idx = match names.get(start.as_str()) {
            Some(idx) => *idx,
            None => return Err(DialogueError::StartMissing { start }),
        };

        let mut built = Vec::with_capacity(nodes.len());
        for node_def in nodes {
            let NodeDef {
                id,
                speaker,
                text,
                labels,
                on_enter,
                next: next_id,
                choices,
            } = node_def;
            let node_id = names
                .get_key_value(id.as_str())
                .expect("the duplicate check inserted every node id")
                .0
                .clone();

            if next_id.is_some() && choices.is_some() {
                return Err(DialogueError::NextAndChoices { node: id.clone() });
            }
            if matches!(&choices, Some(list) if list.is_empty()) {
                return Err(DialogueError::EmptyChoices { node: id.clone() });
            }
            if let Err(source) = validate_signal_text(&text) {
                return Err(DialogueError::BadPlaceholder {
                    node: id.clone(),
                    field: DialogueTextField::Text,
                    source,
                });
            }
            if let Some(speaker) = &speaker
                && let Err(source) = validate_signal_text(speaker)
            {
                return Err(DialogueError::BadPlaceholder {
                    node: id.clone(),
                    field: DialogueTextField::Speaker,
                    source,
                });
            }

            let next = if let Some(target) = &next_id {
                let idx = names.get(target.as_str()).copied().ok_or_else(|| {
                    DialogueError::UnknownTarget {
                        node: id.clone(),
                        target: target.clone(),
                    }
                })?;
                Next::Goto(idx)
            } else if let Some(choice_defs) = choices {
                let mut built_choices = Vec::with_capacity(choice_defs.len());
                for (choice_index, choice_def) in choice_defs.into_iter().enumerate() {
                    let ChoiceDef {
                        text: choice_text,
                        next: choice_next,
                        when,
                        effects,
                        labels: choice_labels,
                    } = choice_def;
                    if let Err(source) = validate_signal_text(&choice_text) {
                        return Err(DialogueError::BadPlaceholder {
                            node: id.clone(),
                            field: DialogueTextField::Choice(choice_index),
                            source,
                        });
                    }
                    let target = match &choice_next {
                        Some(next_id) => {
                            Some(names.get(next_id.as_str()).copied().ok_or_else(|| {
                                DialogueError::UnknownTarget {
                                    node: id.clone(),
                                    target: next_id.clone(),
                                }
                            })?)
                        }
                        None => None,
                    };
                    built_choices.push(DialogueChoice {
                        text: Arc::from(choice_text),
                        target,
                        condition: when,
                        effects,
                        labels: intern_labels(choice_labels),
                    });
                }
                Next::Choices(built_choices)
            } else {
                Next::End
            };

            built.push(DialogueNode {
                id: node_id,
                speaker: speaker.map(Arc::from),
                text: Arc::from(text),
                labels: intern_labels(labels),
                on_enter,
                next,
            });
        }

        let tree = DialogueTree {
            start: start_idx,
            nodes: built,
            names,
        };
        let unreachable = tree.unreachable_nodes();
        if !unreachable.is_empty() {
            let ids: Vec<&str> = unreachable.iter().map(|idx| &*tree.node(*idx).id).collect();
            log::warn!("dialogue has unreachable nodes: {}", ids.join(", "));
        }
        Ok(tree)
    }
}

/// Reads the file at `path`, parses it as JSON into a [`DialogueDef`], and
/// converts it into a validated [`DialogueTree`].
///
/// Read failures surface as [`DialogueError::Io`], malformed JSON as
/// [`DialogueError::Json`], and validation failures as the matching
/// [`DialogueError`] variant.
pub fn load_dialogue(path: impl AsRef<Path>) -> Result<DialogueTree, DialogueError> {
    let text = std::fs::read_to_string(path)?;
    let def: DialogueDef = serde_json::from_str(&text)?;
    DialogueTree::try_from(def)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::dialogue_def::test_helpers::{
        choice, example_def, example_json, node, s,
    };
    use crate::resources::signal_text::SignalTextError;

    /// Converts `def`, expecting a validation error.
    fn expect_err(def: DialogueDef) -> DialogueError {
        match DialogueTree::try_from(def) {
            Ok(tree) => panic!(
                "expected a validation error, but built a tree of {} nodes",
                tree.len()
            ),
            Err(err) => err,
        }
    }

    /// Writes `contents` to a fresh temp file named `name`; the returned
    /// directory keeps the file alive until dropped.
    fn write_temp_file(name: &str, contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, contents).unwrap();
        (dir, path)
    }

    #[test]
    fn a_valid_definition_builds_the_expected_tree() {
        let tree = DialogueTree::try_from(example_def()).unwrap();

        assert_eq!(tree.start(), NodeIdx(0));
        assert_eq!(tree.len(), 3);
        assert!(!tree.is_empty());
        assert_eq!(tree.nodes().len(), 3);
        assert_eq!(tree.idx_of("greet"), Some(NodeIdx(0)));
        assert_eq!(tree.idx_of("friend"), Some(NodeIdx(1)));
        assert_eq!(tree.idx_of("pass"), Some(NodeIdx(2)));
        assert_eq!(tree.idx_of("nope"), None);

        let greet = tree.node(NodeIdx(0));
        assert_eq!(&*greet.id, "greet");
        assert_eq!(greet.speaker.as_deref(), Some("Guard"));
        assert_eq!(&*greet.text, "Halt! Who goes there?");
        assert!(greet.labels.is_empty());
        assert!(greet.on_enter.is_empty());

        let choices = match &greet.next {
            Next::Choices(choices) => choices,
            other => panic!("greet should offer choices, got {other:?}"),
        };
        assert_eq!(choices.len(), 3);

        assert_eq!(&*choices[0].text, "A friend.");
        assert_eq!(choices[0].target, Some(NodeIdx(1)));
        assert_eq!(choices[0].condition, None);
        assert!(choices[0].effects.is_empty());
        assert!(choices[0].labels.is_empty());

        assert_eq!(&*choices[1].text, "Here is my pass.");
        assert_eq!(choices[1].target, Some(NodeIdx(2)));
        assert_eq!(
            choices[1].condition,
            Some(DialogueCondition::flag("has_pass"))
        );
        assert_eq!(
            choices[1].effects,
            vec![DialogueEffect::set_flag("pass_shown")]
        );
        assert_eq!(&*choices[1].labels, [Arc::from("bribe")].as_slice());

        assert_eq!(&*choices[2].text, "Never mind.");
        assert_eq!(
            choices[2].target, None,
            "a choice without next ends the conversation"
        );

        let friend = tree.node(tree.idx_of("friend").unwrap());
        assert_eq!(&*friend.id, "friend");
        assert_eq!(friend.speaker.as_deref(), Some("Guard"));
        assert_eq!(&*friend.text, "Hm. Move along.");
        assert_eq!(friend.next, Next::End, "friend is terminal");

        let pass = tree.node(NodeIdx(2));
        assert_eq!(&*pass.text, "Carry on.");
        assert_eq!(pass.next, Next::Goto(NodeIdx(1)));
        assert_eq!(pass.on_enter, vec![DialogueEffect::add_integer("gold", -1)]);
        assert_eq!(&*pass.labels, [Arc::from("gate")].as_slice());
    }

    #[test]
    fn a_missing_start_id_is_reported() {
        let def = DialogueDef {
            start: s("ghost"),
            nodes: vec![node("a", "t")],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::StartMissing { start } if start == "ghost"),
            "got {err:?}"
        );
    }

    #[test]
    fn a_duplicate_node_id_is_reported() {
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![node("a", "first"), node("a", "second")],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::DuplicateNode { id } if id == "a"),
            "got {err:?}"
        );
    }

    #[test]
    fn an_unknown_next_target_is_reported() {
        let mut a = node("a", "t");
        a.next = Some(s("ghost"));
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![a],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::UnknownTarget { node, target } if node == "a" && target == "ghost"),
            "got {err:?}"
        );
    }

    #[test]
    fn an_unknown_choice_target_is_reported() {
        let mut a = node("a", "t");
        a.choices = Some(vec![ChoiceDef {
            next: Some(s("ghost")),
            ..choice("go")
        }]);
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![a],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::UnknownTarget { node, target } if node == "a" && target == "ghost"),
            "got {err:?}"
        );
    }

    #[test]
    fn next_and_choices_together_is_reported() {
        let mut a = node("a", "t");
        a.next = Some(s("b"));
        a.choices = Some(vec![choice("stay")]);
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![a, node("b", "t2")],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::NextAndChoices { node } if node == "a"),
            "got {err:?}"
        );
    }

    #[test]
    fn an_explicit_empty_choices_list_is_reported() {
        let mut a = node("a", "t");
        a.choices = Some(vec![]);
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![a],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::EmptyChoices { node } if node == "a"),
            "got {err:?}"
        );
    }

    #[test]
    fn only_the_first_error_is_reported_in_documented_order() {
        // A duplicate id wins over a missing start: duplicates are checked first.
        let def = DialogueDef {
            start: s("ghost"),
            nodes: vec![node("a", "one"), node("a", "two")],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::DuplicateNode { id } if id == "a"),
            "got {err:?}"
        );

        // A missing start wins over an unknown next target: the start is
        // checked before any node link.
        let mut a = node("a", "t");
        a.next = Some(s("ghost"));
        let def = DialogueDef {
            start: s("ghost"),
            nodes: vec![a],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::StartMissing { start } if start == "ghost"),
            "got {err:?}"
        );
    }

    #[test]
    fn a_malformed_text_placeholder_is_reported() {
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![node("a", "hi {")],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::BadPlaceholder { node, field, source }
                if node == "a"
                    && *field == DialogueTextField::Text
                    && *source == SignalTextError::UnclosedBrace { at: 3 }),
            "got {err:?}"
        );
    }

    #[test]
    fn a_malformed_speaker_placeholder_is_reported() {
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![NodeDef {
                speaker: Some(s("{}")),
                ..node("a", "t")
            }],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::BadPlaceholder { node, field, source }
                if node == "a"
                    && *field == DialogueTextField::Speaker
                    && *source == SignalTextError::EmptyKey { at: 0 }),
            "got {err:?}"
        );
    }

    #[test]
    fn a_malformed_choice_text_placeholder_is_reported_with_its_index() {
        let mut a = node("a", "t");
        a.choices = Some(vec![
            choice("fine"),
            ChoiceDef {
                text: s("{k:.9}"),
                ..choice("bad")
            },
        ]);
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![a],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::BadPlaceholder { node, field, source }
                if node == "a"
                    && *field == DialogueTextField::Choice(1)
                    && *source == SignalTextError::BadPrecision { at: 0 }),
            "got {err:?}"
        );
    }

    #[test]
    fn valid_placeholders_build_and_are_stored_raw() {
        let mut a = node(
            "a",
            "I have {self.apples} apples, {grain:.1} kg, {{literal}}",
        );
        a.speaker = Some(s("{player_name}"));
        a.choices = Some(vec![ChoiceDef {
            text: s("Hi {player_name}"),
            ..choice("hi")
        }]);
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![a],
        };
        let tree = DialogueTree::try_from(def).unwrap();
        let a = tree.node(tree.idx_of("a").unwrap());
        assert_eq!(
            &*a.text, "I have {self.apples} apples, {grain:.1} kg, {{literal}}",
            "the template is stored raw, not substituted"
        );
        assert_eq!(a.speaker.as_deref(), Some("{player_name}"));
        let choices = match &a.next {
            Next::Choices(choices) => choices,
            other => panic!("a should offer choices, got {other:?}"),
        };
        assert_eq!(&*choices[0].text, "Hi {player_name}");
    }

    #[test]
    fn next_and_choices_wins_over_a_malformed_text() {
        // The structural NextAndChoices check runs before the placeholder
        // checks, so it is reported even though the text is also malformed.
        let mut a = node("a", "hi {");
        a.next = Some(s("b"));
        a.choices = Some(vec![choice("stay")]);
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![a, node("b", "t2")],
        };
        let err = expect_err(def);
        assert!(
            matches!(&err, DialogueError::NextAndChoices { node } if node == "a"),
            "got {err:?}"
        );
    }

    #[test]
    fn unreachable_nodes_are_reported_in_index_order_but_still_build() {
        let mut b = node("b", "t");
        b.next = Some(s("c"));
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![node("a", "t"), b, node("c", "t")],
        };
        let tree = DialogueTree::try_from(def).unwrap();
        let b_idx = tree.idx_of("b").unwrap();
        let c_idx = tree.idx_of("c").unwrap();
        assert_eq!(tree.unreachable_nodes(), vec![b_idx, c_idx]);
        assert!(
            b_idx.index() < c_idx.index(),
            "unreachable nodes come back in ascending index order"
        );
    }

    #[test]
    fn loops_and_joins_build() {
        let mut a = node("a", "t");
        a.choices = Some(vec![
            ChoiceDef {
                next: Some(s("b")),
                ..choice("one")
            },
            ChoiceDef {
                next: Some(s("b")),
                ..choice("two")
            },
        ]);
        let mut b = node("b", "t");
        b.next = Some(s("a"));
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![a, b],
        };
        let tree = DialogueTree::try_from(def).unwrap();
        let a_idx = tree.idx_of("a").unwrap();
        let b_idx = tree.idx_of("b").unwrap();

        let choices = match &tree.node(a_idx).next {
            Next::Choices(choices) => choices,
            other => panic!("a should offer choices, got {other:?}"),
        };
        assert_eq!(choices[0].target, Some(b_idx));
        assert_eq!(
            choices[1].target,
            Some(b_idx),
            "two choices join on the same node"
        );
        assert_eq!(
            tree.node(b_idx).next,
            Next::Goto(a_idx),
            "b loops back to a"
        );
    }

    #[test]
    fn load_dialogue_reads_a_json_file() {
        let (_dir, path) = write_temp_file("dialogue.json", example_json());

        let loaded = load_dialogue(&path).unwrap();
        let def: DialogueDef = serde_json::from_str(example_json()).unwrap();
        let expected = DialogueTree::try_from(def).unwrap();
        assert_eq!(loaded, expected);
    }

    #[test]
    fn load_dialogue_reports_a_missing_file_as_io() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_dialogue(dir.path().join("missing.json")).unwrap_err();
        assert!(
            matches!(&err, DialogueError::Io(e) if e.kind() == std::io::ErrorKind::NotFound),
            "got {err:?}"
        );
    }

    #[test]
    fn load_dialogue_reports_invalid_json_as_json() {
        let (_dir, path) = write_temp_file("bad.json", "{ not json");
        let err = load_dialogue(&path).unwrap_err();
        assert!(matches!(&err, DialogueError::Json(_)), "got {err:?}");
    }

    #[test]
    fn load_dialogue_reports_validation_errors() {
        let (_dir, path) = write_temp_file(
            "bad_target.json",
            r#"{"start":"a","nodes":[{"id":"a","text":"t","next":"ghost"}]}"#,
        );
        let err = load_dialogue(&path).unwrap_err();
        assert!(
            matches!(&err, DialogueError::UnknownTarget { node, target } if node == "a" && target == "ghost"),
            "got {err:?}"
        );
    }

    #[test]
    fn load_dialogue_reports_a_malformed_placeholder() {
        let (_dir, path) = write_temp_file(
            "bad_placeholder.json",
            r#"{"start":"a","nodes":[{"id":"a","text":"hi {"}]}"#,
        );
        let err = load_dialogue(&path).unwrap_err();
        assert!(
            matches!(&err, DialogueError::BadPlaceholder { node, field, source }
                if node == "a"
                    && *field == DialogueTextField::Text
                    && *source == SignalTextError::UnclosedBrace { at: 3 }),
            "got {err:?}"
        );
    }
}
