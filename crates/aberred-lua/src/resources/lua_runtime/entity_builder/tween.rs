use super::*;

pub(super) fn register<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods,
        meta,
        "with_tween_position",
        "Add position tween animation",
        [
            ("from_x", "number"),
            ("from_y", "number"),
            ("to_x", "number"),
            ("to_y", "number"),
            ("duration", "number"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
            this.cmd.tween_position = Some(TweenPositionData {
                from_x,
                from_y,
                to_x,
                to_y,
                config: TweenConfig::new(duration),
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_position_easing",
        "Set easing for position tween",
        [("easing", "string")],
        |_, this: &mut LuaEntityBuilder, easing: String| {
            let Some(ref mut tween) = this.cmd.tween_position else {
                return Err(LuaError::runtime(
                    "with_tween_position_easing() requires with_tween_position() first",
                ));
            };
            tween.config.easing = checked_easing(easing)?;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_position_loop",
        "Set loop mode for position tween",
        [("loop_mode", "string")],
        |_, this: &mut LuaEntityBuilder, loop_mode: String| {
            let Some(ref mut tween) = this.cmd.tween_position else {
                return Err(LuaError::runtime(
                    "with_tween_position_loop() requires with_tween_position() first",
                ));
            };
            tween.config.loop_mode = checked_loop_mode(loop_mode)?;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_position_backwards",
        "Start position tween in reverse",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(ref mut tween) = this.cmd.tween_position else {
                return Err(LuaError::runtime(
                    "with_tween_position_backwards() requires with_tween_position() first",
                ));
            };
            tween.config.backwards = true;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_position_on_finished",
        "Set a Lua callback to call when the position tween finishes",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut tween) = this.cmd.tween_position else {
                return Err(LuaError::runtime(
                    "with_tween_position_on_finished() requires with_tween_position() first",
                ));
            };
            tween.config.callback = callback;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_screen_position",
        "Add screen position tween animation",
        [
            ("from_x", "number"),
            ("from_y", "number"),
            ("to_x", "number"),
            ("to_y", "number"),
            ("duration", "number"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
            this.cmd.tween_screen_position = Some(TweenScreenPositionData {
                from_x,
                from_y,
                to_x,
                to_y,
                config: TweenConfig::new(duration),
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_screen_position_easing",
        "Set easing for screen position tween",
        [("easing", "string")],
        |_, this: &mut LuaEntityBuilder, easing: String| {
            let Some(ref mut tween) = this.cmd.tween_screen_position else {
                return Err(LuaError::runtime(
                    "with_tween_screen_position_easing() requires with_tween_screen_position() first",
                ));
            };
            tween.config.easing = checked_easing(easing)?;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_screen_position_loop",
        "Set loop mode for screen position tween",
        [("loop_mode", "string")],
        |_, this: &mut LuaEntityBuilder, loop_mode: String| {
            let Some(ref mut tween) = this.cmd.tween_screen_position else {
                return Err(LuaError::runtime(
                    "with_tween_screen_position_loop() requires with_tween_screen_position() first",
                ));
            };
            tween.config.loop_mode = checked_loop_mode(loop_mode)?;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_screen_position_backwards",
        "Start screen position tween in reverse",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(ref mut tween) = this.cmd.tween_screen_position else {
                return Err(LuaError::runtime(
                    "with_tween_screen_position_backwards() requires with_tween_screen_position() first",
                ));
            };
            tween.config.backwards = true;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_screen_position_on_finished",
        "Set a Lua callback to call when the screen position tween finishes",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut tween) = this.cmd.tween_screen_position else {
                return Err(LuaError::runtime(
                    "with_tween_screen_position_on_finished() requires with_tween_screen_position() first",
                ));
            };
            tween.config.callback = callback;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_rotation",
        "Add rotation tween animation",
        [("from", "number"), ("to", "number"), ("duration", "number")],
        |_, this: &mut LuaEntityBuilder, (from, to, duration): (f32, f32, f32)| {
            this.cmd.tween_rotation = Some(TweenRotationData {
                from,
                to,
                config: TweenConfig::new(duration),
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_rotation_easing",
        "Set easing for rotation tween",
        [("easing", "string")],
        |_, this: &mut LuaEntityBuilder, easing: String| {
            let Some(ref mut tween) = this.cmd.tween_rotation else {
                return Err(LuaError::runtime(
                    "with_tween_rotation_easing() requires with_tween_rotation() first",
                ));
            };
            tween.config.easing = checked_easing(easing)?;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_rotation_loop",
        "Set loop mode for rotation tween",
        [("loop_mode", "string")],
        |_, this: &mut LuaEntityBuilder, loop_mode: String| {
            let Some(ref mut tween) = this.cmd.tween_rotation else {
                return Err(LuaError::runtime(
                    "with_tween_rotation_loop() requires with_tween_rotation() first",
                ));
            };
            tween.config.loop_mode = checked_loop_mode(loop_mode)?;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_rotation_backwards",
        "Start rotation tween in reverse",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(ref mut tween) = this.cmd.tween_rotation else {
                return Err(LuaError::runtime(
                    "with_tween_rotation_backwards() requires with_tween_rotation() first",
                ));
            };
            tween.config.backwards = true;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_rotation_on_finished",
        "Set a Lua callback to call when the rotation tween finishes",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut tween) = this.cmd.tween_rotation else {
                return Err(LuaError::runtime(
                    "with_tween_rotation_on_finished() requires with_tween_rotation() first",
                ));
            };
            tween.config.callback = callback;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_scale",
        "Add scale tween animation",
        [
            ("from_x", "number"),
            ("from_y", "number"),
            ("to_x", "number"),
            ("to_y", "number"),
            ("duration", "number"),
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
            this.cmd.tween_scale = Some(TweenScaleData {
                from_x,
                from_y,
                to_x,
                to_y,
                config: TweenConfig::new(duration),
            });
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_scale_easing",
        "Set easing for scale tween",
        [("easing", "string")],
        |_, this: &mut LuaEntityBuilder, easing: String| {
            let Some(ref mut tween) = this.cmd.tween_scale else {
                return Err(LuaError::runtime(
                    "with_tween_scale_easing() requires with_tween_scale() first",
                ));
            };
            tween.config.easing = checked_easing(easing)?;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_scale_loop",
        "Set loop mode for scale tween",
        [("loop_mode", "string")],
        |_, this: &mut LuaEntityBuilder, loop_mode: String| {
            let Some(ref mut tween) = this.cmd.tween_scale else {
                return Err(LuaError::runtime(
                    "with_tween_scale_loop() requires with_tween_scale() first",
                ));
            };
            tween.config.loop_mode = checked_loop_mode(loop_mode)?;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_scale_backwards",
        "Start scale tween in reverse",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(ref mut tween) = this.cmd.tween_scale else {
                return Err(LuaError::runtime(
                    "with_tween_scale_backwards() requires with_tween_scale() first",
                ));
            };
            tween.config.backwards = true;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_tween_scale_on_finished",
        "Set a Lua callback to call when the scale tween finishes",
        [("callback", "string")],
        |_, this: &mut LuaEntityBuilder, callback: String| {
            let Some(ref mut tween) = this.cmd.tween_scale else {
                return Err(LuaError::runtime(
                    "with_tween_scale_on_finished() requires with_tween_scale() first",
                ));
            };
            tween.config.callback = callback;
            Ok(())
        }
    );
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{assert_runtime_error, built_spawn_cmd};
    use super::*;

    const KINDS: [&str; 4] = ["position", "screen_position", "rotation", "scale"];

    fn base_call(kind: &str) -> &'static str {
        match kind {
            "position" => "with_tween_position(1, 2, 3, 4, 0.5)",
            "screen_position" => "with_tween_screen_position(5, 6, 7, 8, 1.5)",
            "rotation" => "with_tween_rotation(0, 90, 2.5)",
            "scale" => "with_tween_scale(1, 1, 2, 3, 3.5)",
            _ => unreachable!(),
        }
    }

    #[test]
    fn tween_modifiers_require_their_own_tween_first() {
        for kind in KINDS {
            for (suffix, args) in [
                ("easing", "'quad_in'"),
                ("loop", "'loop'"),
                ("backwards", ""),
                ("on_finished", "'cb'"),
            ] {
                let method = format!("with_tween_{kind}_{suffix}");
                let msg = format!("{method}() requires with_tween_{kind}() first");
                assert_runtime_error(&format!("engine.spawn():{method}({args})"), &msg);
                // A different tween kind being present does not satisfy the guard.
                let other = KINDS.iter().find(|k| **k != kind).unwrap();
                assert_runtime_error(
                    &format!("engine.spawn():{}:{method}({args})", base_call(other)),
                    &msg,
                );
            }
        }
    }

    #[test]
    fn tween_bases_store_endpoints_and_default_config() {
        let cmd = built_spawn_cmd(&format!(
            "engine.spawn():{}:{}:{}:{}:build()",
            base_call("position"),
            base_call("screen_position"),
            base_call("rotation"),
            base_call("scale")
        ));
        let p = cmd.tween_position.unwrap();
        assert_eq!((p.from_x, p.from_y, p.to_x, p.to_y), (1.0, 2.0, 3.0, 4.0));
        let sp = cmd.tween_screen_position.unwrap();
        assert_eq!((sp.from_x, sp.from_y, sp.to_x, sp.to_y), (5.0, 6.0, 7.0, 8.0));
        let r = cmd.tween_rotation.unwrap();
        assert_eq!((r.from, r.to), (0.0, 90.0));
        let s = cmd.tween_scale.unwrap();
        assert_eq!((s.from_x, s.from_y, s.to_x, s.to_y), (1.0, 1.0, 2.0, 3.0));

        for (config, duration) in [
            (&p.config, 0.5),
            (&sp.config, 1.5),
            (&r.config, 2.5),
            (&s.config, 3.5),
        ] {
            assert_eq!(config.duration, duration);
            assert_eq!(config.easing, "linear");
            assert_eq!(config.loop_mode, "once");
            assert!(!config.backwards);
            assert!(config.callback.is_empty());
        }
    }

    #[test]
    fn tween_modifiers_only_touch_their_own_tween() {
        // Distinct easing + callback per kind (only 3 loop modes exist, so one pair shares);
        // only `rotation` is set backwards.
        let values = |kind: &str| match kind {
            "position" => ("quad_in", "loop"),
            "screen_position" => ("quad_out", "ping_pong"),
            "rotation" => ("cubic_in", "once"),
            "scale" => ("cubic_out", "loop"),
            _ => unreachable!(),
        };
        let mut chain = String::from("engine.spawn()");
        for kind in KINDS {
            let (easing, loop_mode) = values(kind);
            chain.push_str(&format!(
                ":{}:with_tween_{kind}_easing('{easing}'):with_tween_{kind}_loop('{loop_mode}')\
                 :with_tween_{kind}_on_finished('{kind}_done')",
                base_call(kind)
            ));
        }
        chain.push_str(":with_tween_rotation_backwards():build()");
        let cmd = built_spawn_cmd(&chain);

        let configs: [(&str, &TweenConfig); 4] = [
            ("position", &cmd.tween_position.as_ref().unwrap().config),
            ("screen_position", &cmd.tween_screen_position.as_ref().unwrap().config),
            ("rotation", &cmd.tween_rotation.as_ref().unwrap().config),
            ("scale", &cmd.tween_scale.as_ref().unwrap().config),
        ];
        for (kind, config) in configs {
            let (easing, loop_mode) = values(kind);
            assert_eq!((config.easing.as_str(), config.loop_mode.as_str()), (easing, loop_mode), "{kind}");
            assert_eq!(config.callback, format!("{kind}_done"));
            assert_eq!(config.backwards, kind == "rotation", "{kind} backwards");
        }
    }

    #[test]
    fn tween_easing_and_loop_reject_unknown_names() {
        for kind in KINDS {
            let base = base_call(kind);
            assert_runtime_error(
                &format!("engine.spawn():{base}:with_tween_{kind}_easing('quad_inn')"),
                "Unknown easing 'quad_inn'",
            );
            assert_runtime_error(
                &format!("engine.spawn():{base}:with_tween_{kind}_loop('pingpong')"),
                "Unknown loop mode 'pingpong'",
            );
        }
        // Every documented name is still accepted.
        for easing in ["linear", "quad_in", "quad_out", "quad_in_out", "cubic_in", "cubic_out", "cubic_in_out"] {
            built_spawn_cmd(&format!(
                "engine.spawn():{}:with_tween_rotation_easing('{easing}'):build()",
                base_call("rotation")
            ));
        }
        for mode in ["once", "loop", "ping_pong"] {
            built_spawn_cmd(&format!(
                "engine.spawn():{}:with_tween_rotation_loop('{mode}'):build()",
                base_call("rotation")
            ));
        }
    }
}
