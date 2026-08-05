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
                        let inclusive: bool = table.get("inclusive").unwrap_or(true);
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
                        let inclusive: bool = table.get("inclusive").unwrap_or(true);
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
