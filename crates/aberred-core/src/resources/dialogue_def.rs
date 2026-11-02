//! Dialogue condition types: the serializable predicate tree evaluated
//! against world signals.
//!
//! [`DialogueCondition`] describes a check over
//! [`WorldSignals`](crate::resources::world_signals::WorldSignals) — flags,
//! integers, strings — combined with `Not`/`All`/`Any`. [`DialogueCmp`] is the
//! comparison operator used by integer conditions.

use serde::{Deserialize, Serialize};

use crate::resources::world_signals::SignalsRead;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::world_signals::WorldSignals;

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
}
