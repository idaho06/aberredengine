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
            tween.config.easing = easing;
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
            tween.config.loop_mode = loop_mode;
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
            tween.config.easing = easing;
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
            tween.config.loop_mode = loop_mode;
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
            tween.config.easing = easing;
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
            tween.config.loop_mode = loop_mode;
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
            tween.config.easing = easing;
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
            tween.config.loop_mode = loop_mode;
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
