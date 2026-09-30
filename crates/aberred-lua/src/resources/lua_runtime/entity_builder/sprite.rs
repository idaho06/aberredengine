use super::*;

pub(super) fn register<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods,
        meta,
        "with_sprite",
        "Set sprite",
        [
            ("tex_key", "string"),
            ("width", "number"),
            ("height", "number"),
            ("origin_x", "number"),
            ("origin_y", "number"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (tex_key, width, height, origin_x, origin_y): (String, f32, f32, f32, f32)| {
            this.cmd.sprite = Some(SpriteData {
                tex_key,
                width,
                height,
                origin_x,
                origin_y,
                offset_x: 0.0,
                offset_y: 0.0,
                flip_h: false,
                flip_v: false,
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_sprite_offset",
        "Set sprite offset",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(ref mut sprite) = this.cmd.sprite else {
                return Err(LuaError::runtime(
                    "with_sprite_offset() requires with_sprite() first",
                ));
            };
            sprite.offset_x = offset_x;
            sprite.offset_y = offset_y;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_sprite_flip",
        "Set sprite flipping",
        [("flip_h", "boolean"), ("flip_v", "boolean")],
        |_, this: &mut LuaEntityBuilder, (flip_h, flip_v): (bool, bool)| {
            let Some(ref mut sprite) = this.cmd.sprite else {
                return Err(LuaError::runtime(
                    "with_sprite_flip() requires with_sprite() first",
                ));
            };
            sprite.flip_h = flip_h;
            sprite.flip_v = flip_v;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_text",
        "Set DynamicText component",
        [
            ("content", "string"),
            ("font", "string"),
            ("font_size", "number"),
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (content, font, font_size, r, g, b, a): (String, String, f32, u8, u8, u8, u8)| {
            this.cmd.text = Some(TextData {
                content,
                font,
                font_size,
                r,
                g,
                b,
                a,
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tint",
        "Set color tint (RGBA 0-255)",
        [
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (r, g, b, a): (u8, u8, u8, u8)| {
            this.cmd.tint = Some((r, g, b, a));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shadow",
        "Set drop shadow (offset dx/dy and RGBA color 0-255)",
        [
            ("dx", "number"),
            ("dy", "number"),
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (dx, dy, r, g, b, a): (f32, f32, u8, u8, u8, u8)| {
            this.cmd.shadow = Some((dx, dy, r, g, b, a));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shader",
        "Set per-entity shader with optional uniforms",
        [("shader_key", "string"), ("uniforms", "table?")],
        |_, this: &mut LuaEntityBuilder, args: mlua::MultiValue| {
            let mut iter = args.into_iter();
            let key_val = iter
                .next()
                .ok_or_else(|| LuaError::runtime("with_shader requires shader_key"))?;
            let shader_key: String = key_val
                .as_string()
                .and_then(|s| s.to_str().ok())
                .ok_or_else(|| LuaError::runtime("shader_key must be string"))?
                .to_string();

            let mut uniforms = Vec::new();

            if let Some(table_val) = iter.next()
                && let Some(table) = table_val.as_table()
            {
                for pair in table.pairs::<String, LuaValue>() {
                    let (name, val) = pair?;
                    let uniform = parse_uniform_value(val)?;
                    uniforms.push((name, uniform));
                }
            }

            this.cmd.shader = Some(EntityShaderData {
                key: shader_key,
                uniforms,
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_animation",
        "Set animation by key",
        [("animation_key", "string")],
        |_, this: &mut LuaEntityBuilder, animation_key: String| {
            this.cmd.animation = Some(AnimationData { animation_key });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_animation_controller",
        "Add animation controller with fallback",
        [("fallback_key", "string")],
        |_, this: &mut LuaEntityBuilder, fallback_key: String| {
            this.cmd.animation_controller = Some(AnimationControllerData {
                fallback_key,
                rules: Vec::new(),
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_animation_rule",
        "Add animation rule to controller",
        [("condition_table", "table"), ("set_key", "string")],
        |_, this: &mut LuaEntityBuilder, (condition_table, set_key): (LuaTable, String)| {
            fn parse_condition(table: &LuaTable) -> LuaResult<AnimationConditionData> {
                let cond_type: String = table.get("type")?;
                match cond_type.as_str() {
                    "has_flag" => {
                        let key: String = table.get("key")?;
                        Ok(AnimationConditionData::HasFlag { key })
                    }
                    "lacks_flag" => {
                        let key: String = table.get("key")?;
                        Ok(AnimationConditionData::LacksFlag { key })
                    }
                    "scalar_cmp" => {
                        let key: String = table.get("key")?;
                        let op: String = table.get("op")?;
                        let value: f32 = table.get("value")?;
                        Ok(AnimationConditionData::ScalarCmp { key, op, value })
                    }
                    "scalar_range" => {
                        let key: String = table.get("key")?;
                        let min: f32 = table.get("min")?;
                        let max: f32 = table.get("max")?;
                        // Option<bool>: a plain bool would read an omitted field as false.
                        let inclusive = table.get::<Option<bool>>("inclusive")?.unwrap_or(true);
                        Ok(AnimationConditionData::ScalarRange {
                            key,
                            min,
                            max,
                            inclusive,
                        })
                    }
                    "integer_cmp" => {
                        let key: String = table.get("key")?;
                        let op: String = table.get("op")?;
                        let value: i32 = table.get("value")?;
                        Ok(AnimationConditionData::IntegerCmp { key, op, value })
                    }
                    "integer_range" => {
                        let key: String = table.get("key")?;
                        let min: i32 = table.get("min")?;
                        let max: i32 = table.get("max")?;
                        // Option<bool>: a plain bool would read an omitted field as false.
                        let inclusive = table.get::<Option<bool>>("inclusive")?.unwrap_or(true);
                        Ok(AnimationConditionData::IntegerRange {
                            key,
                            min,
                            max,
                            inclusive,
                        })
                    }
                    "all" => {
                        let conditions_table: LuaTable = table.get("conditions")?;
                        let mut conditions = Vec::new();
                        for value in conditions_table.sequence_values::<LuaTable>() {
                            conditions.push(parse_condition(&value?)?);
                        }
                        Ok(AnimationConditionData::All(conditions))
                    }
                    "any" => {
                        let conditions_table: LuaTable = table.get("conditions")?;
                        let mut conditions = Vec::new();
                        for value in conditions_table.sequence_values::<LuaTable>() {
                            conditions.push(parse_condition(&value?)?);
                        }
                        Ok(AnimationConditionData::Any(conditions))
                    }
                    "not" => {
                        let inner_table: LuaTable = table.get("condition")?;
                        let inner = parse_condition(&inner_table)?;
                        Ok(AnimationConditionData::Not(Box::new(inner)))
                    }
                    _ => Err(LuaError::runtime(format!(
                        "Unknown condition type: {}",
                        cond_type
                    ))),
                }
            }

            let Some(ref mut controller) = this.cmd.animation_controller else {
                return Err(LuaError::runtime(
                    "with_animation_rule() requires with_animation_controller() first",
                ));
            };

            let condition = parse_condition(&condition_table)?;
            controller
                .rules
                .push(AnimationRuleData { condition, set_key });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_on_animation_end",
        "Attach a callback fired exactly once when the entity's non-looped animation first reaches its last frame. Signature: fn(ctx, input). Looped animations never trigger it.",
        [("fn_name", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            this.cmd.lua_on_animation_end = Some(callback);
            Ok(())
        }
    );
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{assert_runtime_error, built_spawn_cmd};
    use super::*;
    use aberred_core::resources::uniformvalue::UniformValue;

    fn built(chain: &str) -> super::super::SpawnCmd {
        built_spawn_cmd(&format!("engine.spawn(){chain}:build()"))
    }

    fn sole_rule_condition(rule_lua: &str) -> AnimationConditionData {
        let mut ctrl = built(&format!(
            ":with_animation_controller('idle'):with_animation_rule({rule_lua}, 'run')"
        ))
        .animation_controller
        .unwrap();
        assert_eq!(ctrl.rules.len(), 1);
        let rule = ctrl.rules.pop().unwrap();
        assert_eq!(rule.set_key, "run");
        rule.condition
    }

    #[test]
    fn sprite_modifiers_require_their_base_method_first() {
        for (call, msg) in [
            ("with_sprite_flip(true, false)", "with_sprite_flip() requires with_sprite() first"),
            (
                "with_animation_rule({type='has_flag', key='k'}, 'run')",
                "with_animation_rule() requires with_animation_controller() first",
            ),
        ] {
            assert_runtime_error(&format!("engine.spawn():{call}"), msg);
        }
    }

    #[test]
    fn with_sprite_defaults_offset_and_flip_until_set() {
        let s = built(":with_sprite('hero', 32, 48, 16, 24)").sprite.unwrap();
        assert_eq!(s.tex_key, "hero");
        assert_eq!((s.width, s.height, s.origin_x, s.origin_y), (32.0, 48.0, 16.0, 24.0));
        assert_eq!((s.offset_x, s.offset_y, s.flip_h, s.flip_v), (0.0, 0.0, false, false));

        let s = built(":with_sprite('hero', 32, 48, 16, 24):with_sprite_offset(64, 96):with_sprite_flip(false, true)")
            .sprite
            .unwrap();
        assert_eq!((s.offset_x, s.offset_y), (64.0, 96.0));
        assert_eq!((s.flip_h, s.flip_v), (false, true));
    }

    #[test]
    fn with_text_tint_and_shadow_keep_argument_order() {
        let cmd = built(
            ":with_text('Score', 'arcade', 12, 1, 2, 3, 4)\
             :with_tint(5, 6, 7, 8)\
             :with_shadow(2, -3, 9, 10, 11, 12)",
        );
        let t = cmd.text.unwrap();
        assert_eq!((t.content.as_str(), t.font.as_str(), t.font_size), ("Score", "arcade", 12.0));
        assert_eq!((t.r, t.g, t.b, t.a), (1, 2, 3, 4));
        assert_eq!(cmd.tint, Some((5, 6, 7, 8)));
        assert_eq!(cmd.shadow, Some((2.0, -3.0, 9, 10, 11, 12)));
    }

    #[test]
    fn color_channels_out_of_u8_range_are_rejected() {
        assert_runtime_error("engine.spawn():with_tint(256, 0, 0, 255)", "with_tint");
    }

    #[test]
    fn with_shader_without_uniforms_has_empty_list() {
        let shader = built(":with_shader('wave')").shader.unwrap();
        assert_eq!(shader.key, "wave");
        assert!(shader.uniforms.is_empty());
    }

    #[test]
    fn with_shader_parses_float_vec2_and_vec4_uniforms() {
        let mut uniforms = built(
            ":with_shader('wave', { amp = 2, speed = 0.5, dir = {1, 0}, tint = {1, 0.5, 0.25, 1} })",
        )
        .shader
        .unwrap()
        .uniforms;
        uniforms.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            uniforms,
            vec![
                ("amp".to_string(), UniformValue::Float(2.0)),
                ("dir".to_string(), UniformValue::Vec2 { x: 1.0, y: 0.0 }),
                ("speed".to_string(), UniformValue::Float(0.5)),
                (
                    "tint".to_string(),
                    UniformValue::Vec4 { x: 1.0, y: 0.5, z: 0.25, w: 1.0 }
                ),
            ]
        );
    }

    #[test]
    fn with_shader_rejects_invalid_key_and_uniforms() {
        assert_runtime_error("engine.spawn():with_shader()", "with_shader requires shader_key");
        assert_runtime_error("engine.spawn():with_shader({})", "shader_key must be string");
        assert_runtime_error(
            "engine.spawn():with_shader('wave', { v = {1, 2, 3} })",
            "Uniform table must be array of length 2 (vec2) or 4 (vec4)",
        );
        assert_runtime_error(
            "engine.spawn():with_shader('wave', { v = true })",
            "Uniform value must be number or array table",
        );
    }

    #[test]
    fn animation_rule_parses_leaf_conditions() {
        assert!(matches!(
            sole_rule_condition("{type='has_flag', key='moving'}"),
            AnimationConditionData::HasFlag { key } if key == "moving"
        ));
        assert!(matches!(
            sole_rule_condition("{type='lacks_flag', key='grounded'}"),
            AnimationConditionData::LacksFlag { key } if key == "grounded"
        ));
        assert!(matches!(
            sole_rule_condition("{type='scalar_cmp', key='vx', op='gt', value=1.5}"),
            AnimationConditionData::ScalarCmp { key, op, value }
                if key == "vx" && op == "gt" && value == 1.5
        ));
        assert!(matches!(
            sole_rule_condition("{type='integer_cmp', key='hp', op='le', value=3}"),
            AnimationConditionData::IntegerCmp { key, op, value }
                if key == "hp" && op == "le" && value == 3
        ));
    }

    #[test]
    fn animation_rule_ranges_parse_bounds_and_inclusive() {
        assert!(matches!(
            sole_rule_condition("{type='scalar_range', key='vy', min=-1, max=1, inclusive=true}"),
            AnimationConditionData::ScalarRange { min, max, inclusive: true, .. }
                if min == -1.0 && max == 1.0
        ));
        assert!(matches!(
            sole_rule_condition("{type='integer_range', key='hp', min=1, max=3, inclusive=false}"),
            AnimationConditionData::IntegerRange { min: 1, max: 3, inclusive: false, .. }
        ));
    }

    #[test]
    fn animation_rule_ranges_default_to_inclusive_when_omitted() {
        assert!(matches!(
            sole_rule_condition("{type='scalar_range', key='vy', min=-1, max=1}"),
            AnimationConditionData::ScalarRange { inclusive: true, .. }
        ));
        assert!(matches!(
            sole_rule_condition("{type='integer_range', key='hp', min=1, max=3}"),
            AnimationConditionData::IntegerRange { inclusive: true, .. }
        ));
    }

    #[test]
    fn animation_rule_parses_nested_all_any_not() {
        let cond = sole_rule_condition(
            "{type='all', conditions={ \
                {type='has_flag', key='a'}, \
                {type='any', conditions={ {type='lacks_flag', key='b'} }}, \
                {type='not', condition={type='has_flag', key='c'}} }}",
        );
        let AnimationConditionData::All(children) = cond else {
            panic!("expected All, got {cond:?}");
        };
        assert_eq!(children.len(), 3);
        assert!(matches!(&children[0], AnimationConditionData::HasFlag { key } if key == "a"));
        assert!(matches!(
            &children[1],
            AnimationConditionData::Any(inner)
                if matches!(inner.as_slice(), [AnimationConditionData::LacksFlag { key }] if key == "b")
        ));
        assert!(matches!(
            &children[2],
            AnimationConditionData::Not(inner)
                if matches!(inner.as_ref(), AnimationConditionData::HasFlag { key } if key == "c")
        ));
    }

    #[test]
    fn animation_rule_rejects_unknown_type_and_keeps_rule_order() {
        assert_runtime_error(
            "engine.spawn():with_animation_controller('idle'):with_animation_rule({type='sometimes'}, 'x')",
            "Unknown condition type: sometimes",
        );
        let ctrl = built(
            ":with_animation_controller('idle')\
             :with_animation_rule({type='has_flag', key='jump'}, 'jump')\
             :with_animation_rule({type='has_flag', key='run'}, 'run')",
        )
        .animation_controller
        .unwrap();
        assert_eq!(ctrl.fallback_key, "idle");
        let keys: Vec<&str> = ctrl.rules.iter().map(|r| r.set_key.as_str()).collect();
        assert_eq!(keys, ["jump", "run"], "first-match-wins relies on call order");
    }

    #[test]
    fn with_animation_and_on_animation_end_store_keys() {
        let cmd = built(":with_animation('walk'):with_on_animation_end('on_walk_done')");
        assert_eq!(cmd.animation.unwrap().animation_key, "walk");
        assert_eq!(cmd.lua_on_animation_end.as_deref(), Some("on_walk_done"));
    }
}
