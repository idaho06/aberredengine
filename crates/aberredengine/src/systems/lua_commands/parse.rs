//! Animation condition parsing helpers for Lua spawn commands.
//!
//! Converts the Lua-side `AnimationConditionData` representation into the
//! engine's native `Condition` type used by `AnimationController`.

use crate::components::animation::{CmpOp, Condition};
use crate::resources::lua_runtime::AnimationConditionData;

/// Convert a comparison-operator string from Lua into `CmpOp`. Defaults to `Eq`.
pub(super) fn parse_cmp_op(op: &str) -> CmpOp {
    match op {
        "lt" => CmpOp::Lt,
        "le" => CmpOp::Le,
        "gt" => CmpOp::Gt,
        "ge" => CmpOp::Ge,
        "eq" => CmpOp::Eq,
        "ne" => CmpOp::Ne,
        _ => CmpOp::Eq,
    }
}

/// Recursively convert `AnimationConditionData` from Lua into a native `Condition`.
pub(super) fn convert_animation_condition(data: AnimationConditionData) -> Condition {
    match data {
        AnimationConditionData::ScalarCmp { key, op, value } => Condition::ScalarCmp {
            key,
            op: parse_cmp_op(&op),
            value,
        },
        AnimationConditionData::ScalarRange {
            key,
            min,
            max,
            inclusive,
        } => Condition::ScalarRange {
            key,
            min,
            max,
            inclusive,
        },
        AnimationConditionData::IntegerCmp { key, op, value } => Condition::IntegerCmp {
            key,
            op: parse_cmp_op(&op),
            value,
        },
        AnimationConditionData::IntegerRange {
            key,
            min,
            max,
            inclusive,
        } => Condition::IntegerRange {
            key,
            min,
            max,
            inclusive,
        },
        AnimationConditionData::HasFlag { key } => Condition::HasFlag { key },
        AnimationConditionData::LacksFlag { key } => Condition::LacksFlag { key },
        AnimationConditionData::All(conditions) => Condition::All(
            conditions
                .into_iter()
                .map(convert_animation_condition)
                .collect(),
        ),
        AnimationConditionData::Any(conditions) => Condition::Any(
            conditions
                .into_iter()
                .map(convert_animation_condition)
                .collect(),
        ),
        AnimationConditionData::Not(inner) => {
            Condition::Not(Box::new(convert_animation_condition(*inner)))
        }
    }
}
