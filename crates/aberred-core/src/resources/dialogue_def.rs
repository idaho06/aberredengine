//! Dialogue definition types: the authoring definition of a conversation,
//! the serializable predicate tree evaluated against world signals, and the
//! effects that mutate those signals.
//!
//! [`DialogueDef`] is the authored conversation: a `start` node id plus
//! [`NodeDef`]s linked through `goto` and [`ChoiceDef`]s.
//! [`DialogueCondition`] describes a check over
//! [`WorldSignals`] — flags,
//! integers, strings — combined with `Not`/`All`/`Any`. [`DialogueCmp`] is the
//! comparison operator used by integer conditions. [`DialogueEffect`]
//! describes a signal mutation applied when a dialogue node is chosen.

use serde::{Deserialize, Serialize};

use crate::resources::world_signals::{SignalsRead, WorldSignals};

/// Comparison operator for a [`DialogueCondition::Integer`] check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialogueCmp {
    /// Equal (`==`).
    Eq,
    /// Not equal (`!=`).
    Ne,
    /// Less than (`<`).
    Lt,
    /// Less than or equal (`<=`).
    Le,
    /// Greater than (`>`).
    Gt,
    /// Greater than or equal (`>=`).
    Ge,
}

impl DialogueCmp {
    /// Apply the operator to two integers: `test(lhs, rhs)` is `lhs <op> rhs`.
    pub fn test(self, lhs: i32, rhs: i32) -> bool {
        match self {
            DialogueCmp::Eq => lhs == rhs,
            DialogueCmp::Ne => lhs != rhs,
            DialogueCmp::Lt => lhs < rhs,
            DialogueCmp::Le => lhs <= rhs,
            DialogueCmp::Gt => lhs > rhs,
            DialogueCmp::Ge => lhs >= rhs,
        }
    }
}

/// A predicate over world signals, evaluated with [`DialogueCondition::eval`].
///
/// Serializes as an internally tagged enum: a `"type"` discriminator with
/// snake_case variant names, e.g. `{"type":"flag","key":"has_pass"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DialogueCondition {
    /// True while the flag `key` is set.
    Flag {
        /// Signal key to test.
        key: String,
    },
    /// Compares the integer signal `key` against `value` with `op`; a missing
    /// key reads as `0`.
    Integer {
        /// Signal key to read.
        key: String,
        /// Comparison operator.
        op: DialogueCmp,
        /// Value to compare against.
        value: i32,
    },
    /// True when the string signal `key` equals `value`; a missing string
    /// never equals.
    StringEquals {
        /// Signal key to read.
        key: String,
        /// Value to compare against.
        value: String,
    },
    /// Negates the inner condition.
    Not {
        /// Condition to negate.
        cond: Box<DialogueCondition>,
    },
    /// True when every sub-condition is true; an empty `All` is true.
    All {
        /// Sub-conditions that must all hold.
        all: Vec<DialogueCondition>,
    },
    /// True when at least one sub-condition is true; an empty `Any` is false.
    Any {
        /// Sub-conditions of which at least one must hold.
        any: Vec<DialogueCondition>,
    },
}

impl DialogueCondition {
    /// Build a [`Flag`](DialogueCondition::Flag) condition on `key`.
    pub fn flag(key: impl Into<String>) -> Self {
        Self::Flag { key: key.into() }
    }

    /// Build an [`Integer`](DialogueCondition::Integer) condition comparing
    /// the signal `key` with `value` using `op`.
    pub fn integer(key: impl Into<String>, op: DialogueCmp, value: i32) -> Self {
        Self::Integer {
            key: key.into(),
            op,
            value,
        }
    }

    /// Build a [`StringEquals`](DialogueCondition::StringEquals) condition on
    /// the signal `key` and expected `value`.
    pub fn string_equals(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::StringEquals {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Build a [`Not`](DialogueCondition::Not) condition negating `cond`.
    // A constructor named like its variant, beside `all`/`any`; it takes a
    // condition, not `self`, so `std::ops::Not` would not fit.
    #[allow(clippy::should_implement_trait)]
    pub fn not(cond: DialogueCondition) -> Self {
        Self::Not {
            cond: Box::new(cond),
        }
    }

    /// Build an [`All`](DialogueCondition::All) condition over `conds`.
    pub fn all(conds: Vec<DialogueCondition>) -> Self {
        Self::All { all: conds }
    }

    /// Build an [`Any`](DialogueCondition::Any) condition over `conds`.
    pub fn any(conds: Vec<DialogueCondition>) -> Self {
        Self::Any { any: conds }
    }

    /// Evaluate the condition against `signals`.
    ///
    /// - `Flag` is `signals.has_flag(key)`.
    /// - `Integer` compares `signals.get_integer(key)` with `value` using
    ///   `op`; a missing key reads as `0`, so `gold >= 1` on an unset key is
    ///   false.
    /// - `StringEquals` is `signals.get_string(key) == Some(value)`; a missing
    ///   string never equals.
    /// - `Not` negates the inner condition.
    /// - `All` is true when every sub-condition is true (empty `All` is true).
    /// - `Any` is true when at least one sub-condition is true (empty `Any` is
    ///   false).
    pub fn eval(&self, signals: &dyn SignalsRead) -> bool {
        match self {
            DialogueCondition::Flag { key } => signals.has_flag(key),
            DialogueCondition::Integer { key, op, value } => {
                op.test(signals.get_integer(key).unwrap_or(0), *value)
            }
            DialogueCondition::StringEquals { key, value } => {
                signals.get_string(key) == Some(value.as_str())
            }
            DialogueCondition::Not { cond } => !cond.eval(signals),
            DialogueCondition::All { all } => all.iter().all(|cond| cond.eval(signals)),
            DialogueCondition::Any { any } => any.iter().any(|cond| cond.eval(signals)),
        }
    }
}

/// A mutation applied to world signals when a dialogue node is chosen,
/// executed with [`DialogueEffect::apply`].
///
/// Serializes as an internally tagged enum: a `"type"` discriminator with
/// snake_case variant names, e.g. `{"type":"set_flag","key":"pass_shown"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DialogueEffect {
    /// Sets the flag `key`.
    SetFlag {
        /// Signal key to set.
        key: String,
    },
    /// Removes the flag `key`; a no-op when the flag is unset.
    RemoveFlag {
        /// Signal key to remove.
        key: String,
    },
    /// Sets the integer signal `key` to `value`.
    SetInteger {
        /// Signal key to write.
        key: String,
        /// Value to store.
        value: i32,
    },
    /// Adds `delta` to the integer signal `key`; a missing key counts as `0`.
    AddInteger {
        /// Signal key to update.
        key: String,
        /// Amount to add; may be negative.
        delta: i32,
    },
    /// Sets the scalar signal `key` to `value`.
    SetScalar {
        /// Signal key to write.
        key: String,
        /// Value to store.
        value: f32,
    },
    /// Sets the string signal `key` to `value`.
    SetString {
        /// Signal key to write.
        key: String,
        /// Value to store.
        value: String,
    },
}

impl DialogueEffect {
    /// Build a [`SetFlag`](DialogueEffect::SetFlag) effect on `key`.
    pub fn set_flag(key: impl Into<String>) -> Self {
        Self::SetFlag { key: key.into() }
    }

    /// Build a [`RemoveFlag`](DialogueEffect::RemoveFlag) effect on `key`.
    pub fn remove_flag(key: impl Into<String>) -> Self {
        Self::RemoveFlag { key: key.into() }
    }

    /// Build a [`SetInteger`](DialogueEffect::SetInteger) effect writing
    /// `value` to the signal `key`.
    pub fn set_integer(key: impl Into<String>, value: i32) -> Self {
        Self::SetInteger {
            key: key.into(),
            value,
        }
    }

    /// Build an [`AddInteger`](DialogueEffect::AddInteger) effect adding
    /// `delta` to the signal `key`.
    pub fn add_integer(key: impl Into<String>, delta: i32) -> Self {
        Self::AddInteger {
            key: key.into(),
            delta,
        }
    }

    /// Build a [`SetScalar`](DialogueEffect::SetScalar) effect writing
    /// `value` to the signal `key`.
    pub fn set_scalar(key: impl Into<String>, value: f32) -> Self {
        Self::SetScalar {
            key: key.into(),
            value,
        }
    }

    /// Build a [`SetString`](DialogueEffect::SetString) effect writing
    /// `value` to the signal `key`.
    pub fn set_string(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::SetString {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Apply the effect to `signals`.
    ///
    /// - `SetFlag` calls [`WorldSignals::set_flag`]; `RemoveFlag` calls
    ///   [`WorldSignals::remove_flag`].
    /// - `SetInteger`, `SetScalar` and `SetString` call the matching
    ///   `WorldSignals::set_*` setter.
    /// - `AddInteger` adds `delta` to the current integer signal (a missing
    ///   key counts as `0`) using `checked_add`; on overflow past `i32::MAX`
    ///   or `i32::MIN` it logs `log::error!` naming the key, the current value
    ///   and the delta, and leaves the signal unchanged — no clamping, no
    ///   wrapping, nothing marked dirty.
    pub fn apply(&self, signals: &mut WorldSignals) {
        match self {
            DialogueEffect::SetFlag { key } => signals.set_flag(key.as_str()),
            DialogueEffect::RemoveFlag { key } => {
                let _ = signals.remove_flag(key);
            }
            DialogueEffect::SetInteger { key, value } => {
                signals.set_integer(key.as_str(), *value);
            }
            DialogueEffect::AddInteger { key, delta } => {
                let current = signals.get_integer(key).unwrap_or(0);
                match current.checked_add(*delta) {
                    Some(value) => signals.set_integer(key.as_str(), value),
                    None => log::error!(
                        "AddInteger overflow on signal \"{key}\": current {current} + delta {delta} does not fit in i32; signal left unchanged"
                    ),
                }
            }
            DialogueEffect::SetScalar { key, value } => {
                signals.set_scalar(key.as_str(), *value);
            }
            DialogueEffect::SetString { key, value } => {
                signals.set_string(key.as_str(), value.as_str());
            }
        }
    }
}

/// The authoring definition of a conversation: the `start` node id and every
/// node of the graph.
///
/// Serializes as a plain JSON object, e.g.
/// `{"start":"greet","nodes":[...]}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DialogueDef {
    /// Id of the [`NodeDef`] the conversation enters first.
    pub start: String,
    /// All nodes of the conversation, in authoring order.
    pub nodes: Vec<NodeDef>,
}

/// One node of a [`DialogueDef`]: the spoken `text` and where the
/// conversation goes next.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeDef {
    /// Node id; referenced by [`DialogueDef::start`], [`NodeDef::goto`] and
    /// [`ChoiceDef::next`].
    pub id: String,
    /// Speaker shown for this line; absent for narration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    /// The line spoken at this node.
    pub text: String,
    /// Opaque labels attached to the node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Effects applied when the node is entered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_enter: Vec<DialogueEffect>,
    /// Unconditional jump to another node id; `None` together with
    /// `choices == None` makes the node terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goto: Option<String>,
    /// Choices offered at this node. `Some(vec![])` is an explicit empty list,
    /// kept distinguishable from an absent key (which stays `None`); validation
    /// rejects the empty list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<ChoiceDef>>,
}

/// One choice of a [`NodeDef`]: the `text` shown to the player and where
/// picking it leads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChoiceDef {
    /// Label shown for this choice.
    pub text: String,
    /// Node id the choice leads to; `None` ends the conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// Condition that must hold for the choice to be offered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<DialogueCondition>,
    /// Effects applied when the choice is picked.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<DialogueEffect>,
    /// Opaque labels attached to the choice.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

/// Builders and fixtures shared by the dialogue tests in this module and in
/// [`crate::resources::dialogue_tree`]'s tests.
#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;

    /// Returns `v` as an owned `String`.
    pub(crate) fn s(v: &str) -> String {
        v.to_owned()
    }

    /// A node with every optional field at its default.
    pub(crate) fn node(id: &str, text: &str) -> NodeDef {
        NodeDef {
            id: s(id),
            speaker: None,
            text: s(text),
            labels: vec![],
            on_enter: vec![],
            goto: None,
            choices: None,
        }
    }

    /// A choice with every optional field at its default.
    pub(crate) fn choice(text: &str) -> ChoiceDef {
        ChoiceDef {
            text: s(text),
            next: None,
            when: None,
            effects: vec![],
            labels: vec![],
        }
    }

    /// The JSON example shared by the dialogue tests.
    pub(crate) fn example_json() -> &'static str {
        r#"{"start":"greet","nodes":[
 {"id":"greet","speaker":"Guard","text":"Halt! Who goes there?","choices":[
   {"text":"A friend.","next":"friend"},
   {"text":"Here is my pass.","next":"pass","when":{"type":"flag","key":"has_pass"},"effects":[{"type":"set_flag","key":"pass_shown"}],"labels":["bribe"]},
   {"text":"Never mind."}]},
 {"id":"friend","speaker":"Guard","text":"Hm. Move along."},
 {"id":"pass","speaker":"Guard","text":"Carry on.","goto":"friend","on_enter":[{"type":"add_integer","key":"gold","delta":-1}],"labels":["gate"]}]}"#
    }

    /// The [`DialogueDef`] the example JSON parses to.
    pub(crate) fn example_def() -> DialogueDef {
        DialogueDef {
            start: s("greet"),
            nodes: vec![
                NodeDef {
                    speaker: Some(s("Guard")),
                    choices: Some(vec![
                        ChoiceDef {
                            next: Some(s("friend")),
                            ..choice("A friend.")
                        },
                        ChoiceDef {
                            next: Some(s("pass")),
                            when: Some(DialogueCondition::flag("has_pass")),
                            effects: vec![DialogueEffect::set_flag("pass_shown")],
                            labels: vec![s("bribe")],
                            ..choice("Here is my pass.")
                        },
                        choice("Never mind."),
                    ]),
                    ..node("greet", "Halt! Who goes there?")
                },
                NodeDef {
                    speaker: Some(s("Guard")),
                    ..node("friend", "Hm. Move along.")
                },
                NodeDef {
                    speaker: Some(s("Guard")),
                    labels: vec![s("gate")],
                    on_enter: vec![DialogueEffect::add_integer("gold", -1)],
                    goto: Some(s("friend")),
                    ..node("pass", "Carry on.")
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::{choice, example_def, example_json, node, s};
    use super::*;

    /// Builds All[Flag a, Not(Any[Integer gold Lt 5, StringEquals name "x"])].
    fn nested_condition() -> DialogueCondition {
        DialogueCondition::all(vec![
            DialogueCondition::flag("a"),
            DialogueCondition::not(DialogueCondition::any(vec![
                DialogueCondition::integer("gold", DialogueCmp::Lt, 5),
                DialogueCondition::string_equals("name", "x"),
            ])),
        ])
    }

    #[test]
    fn flag_condition_matches_flag_presence() {
        let mut signals = WorldSignals::default();
        let cond = DialogueCondition::flag("has_pass");
        assert!(!cond.eval(&signals), "unset flag is false");
        signals.set_flag("has_pass");
        assert!(cond.eval(&signals), "set flag is true");
    }

    #[test]
    fn integer_condition_applies_each_operator() {
        let mut signals = WorldSignals::default();
        signals.set_integer("gold", 10);

        assert!(DialogueCondition::integer("gold", DialogueCmp::Eq, 10).eval(&signals));
        assert!(!DialogueCondition::integer("gold", DialogueCmp::Eq, 9).eval(&signals));
        assert!(DialogueCondition::integer("gold", DialogueCmp::Ne, 9).eval(&signals));
        assert!(!DialogueCondition::integer("gold", DialogueCmp::Ne, 10).eval(&signals));
        assert!(DialogueCondition::integer("gold", DialogueCmp::Lt, 11).eval(&signals));
        assert!(!DialogueCondition::integer("gold", DialogueCmp::Lt, 10).eval(&signals));
        assert!(DialogueCondition::integer("gold", DialogueCmp::Le, 10).eval(&signals));
        assert!(!DialogueCondition::integer("gold", DialogueCmp::Le, 9).eval(&signals));
        assert!(DialogueCondition::integer("gold", DialogueCmp::Gt, 9).eval(&signals));
        assert!(!DialogueCondition::integer("gold", DialogueCmp::Gt, 10).eval(&signals));
        assert!(DialogueCondition::integer("gold", DialogueCmp::Ge, 10).eval(&signals));
        assert!(!DialogueCondition::integer("gold", DialogueCmp::Ge, 11).eval(&signals));
    }

    #[test]
    fn integer_condition_treats_a_missing_key_as_zero() {
        let signals = WorldSignals::default();
        assert!(!DialogueCondition::integer("gold", DialogueCmp::Ge, 1).eval(&signals));
        assert!(DialogueCondition::integer("gold", DialogueCmp::Eq, 0).eval(&signals));
        assert!(DialogueCondition::integer("gold", DialogueCmp::Lt, 1).eval(&signals));
    }

    #[test]
    fn string_equals_condition_compares_the_string_signal() {
        let mut signals = WorldSignals::default();
        signals.set_string("name", "x");
        assert!(DialogueCondition::string_equals("name", "x").eval(&signals));
        assert!(!DialogueCondition::string_equals("name", "y").eval(&signals));
        assert!(!DialogueCondition::string_equals("missing", "x").eval(&signals));
    }

    #[test]
    fn not_all_any_compose_conditions() {
        let mut signals = WorldSignals::default();
        signals.set_flag("a");
        signals.set_integer("gold", 10);
        signals.set_string("name", "y");

        assert!(!DialogueCondition::not(DialogueCondition::flag("a")).eval(&signals));
        assert!(DialogueCondition::not(DialogueCondition::flag("b")).eval(&signals));

        assert!(
            DialogueCondition::all(vec![]).eval(&signals),
            "empty All is true"
        );
        assert!(
            !DialogueCondition::any(vec![]).eval(&signals),
            "empty Any is false"
        );

        assert!(
            DialogueCondition::all(vec![
                DialogueCondition::flag("a"),
                DialogueCondition::integer("gold", DialogueCmp::Ge, 5),
            ])
            .eval(&signals)
        );
        assert!(
            !DialogueCondition::all(vec![
                DialogueCondition::flag("a"),
                DialogueCondition::flag("b"),
            ])
            .eval(&signals)
        );

        assert!(
            DialogueCondition::any(vec![
                DialogueCondition::flag("b"),
                DialogueCondition::integer("gold", DialogueCmp::Ge, 5),
            ])
            .eval(&signals)
        );
        assert!(
            !DialogueCondition::any(vec![
                DialogueCondition::flag("b"),
                DialogueCondition::integer("gold", DialogueCmp::Lt, 5),
            ])
            .eval(&signals)
        );

        let nested = nested_condition();
        assert!(
            nested.eval(&signals),
            "gold 10 >= 5 and name \"y\" != \"x\""
        );
        signals.set_integer("gold", 3);
        assert!(
            !nested.eval(&signals),
            "gold 3 < 5 makes the inner Any true"
        );
    }

    #[test]
    fn serde_round_trips_a_nested_condition() {
        let nested = nested_condition();
        let json = serde_json::to_string(&nested).unwrap();
        let back: DialogueCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(back, nested);
    }

    #[test]
    fn serde_parses_condition_literals() {
        let flag: DialogueCondition =
            serde_json::from_str(r#"{"type":"flag","key":"has_pass"}"#).unwrap();
        assert_eq!(flag, DialogueCondition::flag("has_pass"));

        let integer: DialogueCondition =
            serde_json::from_str(r#"{"type":"integer","key":"gold","op":"ge","value":3}"#).unwrap();
        assert_eq!(
            integer,
            DialogueCondition::integer("gold", DialogueCmp::Ge, 3)
        );
    }

    #[test]
    fn set_flag_effect_sets_and_remove_flag_effect_removes_the_flag() {
        let mut signals = WorldSignals::default();
        DialogueEffect::set_flag("pass_shown").apply(&mut signals);
        assert!(signals.has_flag("pass_shown"));

        DialogueEffect::remove_flag("pass_shown").apply(&mut signals);
        assert!(!signals.has_flag("pass_shown"));

        // Removing an unset flag is a no-op.
        DialogueEffect::remove_flag("pass_shown").apply(&mut signals);
        assert!(!signals.has_flag("pass_shown"));
    }

    #[test]
    fn set_integer_scalar_string_effects_write_their_values() {
        let mut signals = WorldSignals::default();
        DialogueEffect::set_integer("gold", 5).apply(&mut signals);
        assert_eq!(signals.get_integer("gold"), Some(5));
        DialogueEffect::set_scalar("speed", 1.5).apply(&mut signals);
        assert_eq!(signals.get_scalar("speed"), Some(1.5));
        DialogueEffect::set_string("mood", "calm").apply(&mut signals);
        assert_eq!(signals.get_string("mood"), Some("calm"));
    }

    #[test]
    fn add_integer_effect_adds_the_delta() {
        let mut signals = WorldSignals::default();
        signals.set_integer("gold", 5);
        DialogueEffect::add_integer("gold", 3).apply(&mut signals);
        assert_eq!(signals.get_integer("gold"), Some(8));

        DialogueEffect::add_integer("gold", -3).apply(&mut signals);
        assert_eq!(signals.get_integer("gold"), Some(5));

        // A missing key starts at 0.
        DialogueEffect::add_integer("mana", 4).apply(&mut signals);
        assert_eq!(signals.get_integer("mana"), Some(4));
    }

    #[test]
    fn add_integer_overflow_leaves_the_signal_unchanged_and_clean() {
        let mut signals = WorldSignals::default();
        signals.set_integer("gold", i32::MAX);
        let _ = signals.snapshot(); // clear dirty
        DialogueEffect::add_integer("gold", 1).apply(&mut signals);
        assert_eq!(signals.get_integer("gold"), Some(i32::MAX));
        assert!(!signals.is_dirty());

        signals.set_integer("gold", i32::MIN);
        let _ = signals.snapshot(); // clear dirty
        DialogueEffect::add_integer("gold", -1).apply(&mut signals);
        assert_eq!(signals.get_integer("gold"), Some(i32::MIN));
        assert!(!signals.is_dirty());
    }

    #[test]
    fn effect_that_changes_nothing_does_not_mark_dirty() {
        let mut signals = WorldSignals::default();
        signals.set_integer("gold", 5);
        let _ = signals.snapshot(); // clear dirty
        DialogueEffect::set_integer("gold", 5).apply(&mut signals);
        assert_eq!(signals.get_integer("gold"), Some(5));
        assert!(!signals.is_dirty());
    }

    #[test]
    fn serde_parses_effect_literals() {
        let set_flag: DialogueEffect =
            serde_json::from_str(r#"{"type":"set_flag","key":"pass_shown"}"#).unwrap();
        assert_eq!(set_flag, DialogueEffect::set_flag("pass_shown"));

        let add_integer: DialogueEffect =
            serde_json::from_str(r#"{"type":"add_integer","key":"gold","delta":-2}"#).unwrap();
        assert_eq!(add_integer, DialogueEffect::add_integer("gold", -2));

        let set_string: DialogueEffect =
            serde_json::from_str(r#"{"type":"set_string","key":"mood","value":"calm"}"#).unwrap();
        assert_eq!(set_string, DialogueEffect::set_string("mood", "calm"));
    }

    #[test]
    fn serde_round_trips_all_effect_variants() {
        let effects = vec![
            DialogueEffect::set_flag("pass_shown"),
            DialogueEffect::remove_flag("pass_shown"),
            DialogueEffect::set_integer("gold", 5),
            DialogueEffect::add_integer("gold", -2),
            DialogueEffect::set_scalar("speed", 1.5),
            DialogueEffect::set_string("mood", "calm"),
        ];
        let json = serde_json::to_string(&effects).unwrap();
        let back: Vec<DialogueEffect> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, effects);
    }

    #[test]
    fn the_authoring_json_example_parses() {
        let def: DialogueDef = serde_json::from_str(example_json()).unwrap();
        assert_eq!(def, example_def());
    }

    #[test]
    fn a_definition_round_trips_through_json() {
        let def = example_def();
        let json = serde_json::to_string_pretty(&def).unwrap();
        let back: DialogueDef = serde_json::from_str(&json).unwrap();
        assert_eq!(back, def);
    }

    #[test]
    fn defaults_are_skipped_on_save() {
        assert_eq!(
            serde_json::to_value(node("a", "t")).unwrap(),
            serde_json::json!({"id": "a", "text": "t"})
        );
        assert_eq!(
            serde_json::to_value(choice("c")).unwrap(),
            serde_json::json!({"text": "c"})
        );
        let def = DialogueDef {
            start: s("a"),
            nodes: vec![],
        };
        assert_eq!(
            serde_json::to_value(&def).unwrap(),
            serde_json::json!({"start": "a", "nodes": []})
        );
    }

    #[test]
    fn an_explicit_empty_choices_list_is_preserved() {
        let node: NodeDef = serde_json::from_str(r#"{"id":"a","text":"t","choices":[]}"#).unwrap();
        assert_eq!(node.choices, Some(vec![]));
        let value = serde_json::to_value(&node).unwrap();
        assert_eq!(value["choices"], serde_json::json!([]));

        let absent: NodeDef = serde_json::from_str(r#"{"id":"a","text":"t"}"#).unwrap();
        assert_eq!(absent.choices, None);
        let value = serde_json::to_value(&absent).unwrap();
        assert!(value.get("choices").is_none());
    }
}
