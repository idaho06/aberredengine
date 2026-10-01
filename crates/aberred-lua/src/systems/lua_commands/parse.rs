//! Animation condition parsing helpers for Lua spawn commands.
//!
//! Converts the Lua-side `AnimationConditionData` representation into the
//! engine's native `Condition` type used by `AnimationController`.

use crate::resources::lua_runtime::AnimationConditionData;
use aberred_core::components::animation::{CmpOp, Condition};

/// Convert a comparison-operator string from Lua into `CmpOp`. The entity builder rejects
/// unknown spellings, so the `Eq` fallback is unreachable from Lua scripts.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cmp_op_maps_each_documented_spelling() {
        assert!(matches!(parse_cmp_op("lt"), CmpOp::Lt));
        assert!(matches!(parse_cmp_op("le"), CmpOp::Le));
        assert!(matches!(parse_cmp_op("gt"), CmpOp::Gt));
        assert!(matches!(parse_cmp_op("ge"), CmpOp::Ge));
        assert!(matches!(parse_cmp_op("eq"), CmpOp::Eq));
        assert!(matches!(parse_cmp_op("ne"), CmpOp::Ne));
    }

    #[test]
    fn convert_maps_leaf_conditions_field_for_field() {
        let scalar_cmp = convert_animation_condition(AnimationConditionData::ScalarCmp {
            key: "vx".into(),
            op: "ge".into(),
            value: 1.5,
        });
        assert!(matches!(
            scalar_cmp,
            Condition::ScalarCmp { ref key, op: CmpOp::Ge, value } if key == "vx" && value == 1.5
        ));

        let int_cmp = convert_animation_condition(AnimationConditionData::IntegerCmp {
            key: "hp".into(),
            op: "lt".into(),
            value: 2,
        });
        assert!(matches!(
            int_cmp,
            Condition::IntegerCmp { ref key, op: CmpOp::Lt, value: 2 } if key == "hp"
        ));

        let scalar_range = convert_animation_condition(AnimationConditionData::ScalarRange {
            key: "vy".into(),
            min: -1.0,
            max: 1.0,
            inclusive: false,
        });
        assert!(matches!(
            scalar_range,
            Condition::ScalarRange { ref key, min, max, inclusive: false }
                if key == "vy" && min == -1.0 && max == 1.0
        ));

        let int_range = convert_animation_condition(AnimationConditionData::IntegerRange {
            key: "ammo".into(),
            min: 1,
            max: 9,
            inclusive: true,
        });
        assert!(matches!(
            int_range,
            Condition::IntegerRange { ref key, min: 1, max: 9, inclusive: true } if key == "ammo"
        ));

        assert!(matches!(
            convert_animation_condition(AnimationConditionData::HasFlag { key: "a".into() }),
            Condition::HasFlag { ref key } if key == "a"
        ));
        assert!(matches!(
            convert_animation_condition(AnimationConditionData::LacksFlag { key: "b".into() }),
            Condition::LacksFlag { ref key } if key == "b"
        ));
    }

    #[test]
    fn convert_recurses_through_all_any_not_preserving_order() {
        let converted = convert_animation_condition(AnimationConditionData::All(vec![
            AnimationConditionData::HasFlag {
                key: "first".into(),
            },
            AnimationConditionData::Any(vec![
                AnimationConditionData::LacksFlag { key: "x".into() },
                AnimationConditionData::HasFlag { key: "y".into() },
            ]),
            AnimationConditionData::Not(Box::new(AnimationConditionData::HasFlag {
                key: "z".into(),
            })),
        ]));

        let Condition::All(children) = converted else {
            panic!("expected All, got {converted:?}");
        };
        assert_eq!(children.len(), 3);
        assert!(matches!(&children[0], Condition::HasFlag { key } if key == "first"));
        let Condition::Any(any) = &children[1] else {
            panic!("expected Any, got {:?}", children[1]);
        };
        assert!(matches!(
            any.as_slice(),
            [Condition::LacksFlag { key: x }, Condition::HasFlag { key: y }] if x == "x" && y == "y"
        ));
        let Condition::Not(inner) = &children[2] else {
            panic!("expected Not, got {:?}", children[2]);
        };
        assert!(matches!(inner.as_ref(), Condition::HasFlag { key } if key == "z"));
    }
}
