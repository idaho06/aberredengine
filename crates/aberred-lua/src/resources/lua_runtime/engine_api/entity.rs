use super::super::entity_builder::LuaEntityBuilder;
use super::*;
use aberred_core::components::shape::ShapeKind;
use aberred_core::components::timer::TimerMode;
use aberred_core::math::Vec2;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_entity_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        define_entity_cmds!(engine, self.lua, meta_fns, "", entity_commands);
        Ok(())
    }

    pub(in crate::resources::lua_runtime) fn register_collision_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        define_entity_cmds!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_entity_commands
        );

        define_audio_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_audio_commands,
            "collision",
            " (collision context)"
        );

        define_signal_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_signal_commands,
            "collision",
            " (collision context)"
        );

        define_phase_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_phase_commands,
            "collision",
            " (collision context)"
        );

        define_camera_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_camera_commands,
            "collision",
            " (collision context)"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "collision_spawn",
            |_lua, ()| { Ok(LuaEntityBuilder::new_collision()) },
            desc = "Create a new entity builder (collision context)",
            cat = "collision",
            params = [],
            returns = "CollisionEntityBuilder"
        );

        // source_key is stored into the eventual SpawnCmd, not just read — stays an
        // owned String rather than converting to mlua::LuaString.
        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "collision_clone",
            |_lua, source_key: String| { Ok(LuaEntityBuilder::new_collision_clone(source_key)) },
            desc = "Clone a registered entity (collision context)",
            cat = "collision",
            params = [("source_key", "string")],
            returns = "CollisionEntityBuilder"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::lua_runtime::LuaRuntime;

    fn run_err(script: &str) -> String {
        let runtime = LuaRuntime::new().unwrap();
        runtime.lua().load(script).exec().unwrap_err().to_string()
    }

    #[test]
    fn runtime_tween_inserts_reject_unknown_easing_and_loop_names() {
        for prefix in ["", "collision_"] {
            for (call, bad) in [
                (
                    "entity_insert_tween_position(1, 0, 0, 1, 1, 1, 'bouncy', 'once', false)",
                    "Unknown easing 'bouncy'",
                ),
                (
                    "entity_insert_tween_rotation(1, 0, 90, 1, 'linear', 'forever', false)",
                    "Unknown loop mode 'forever'",
                ),
                (
                    "entity_insert_tween_scale(1, 1, 1, 2, 2, 1, 'Quad_In', 'once', false)",
                    "Unknown easing 'Quad_In'",
                ),
                (
                    "entity_insert_tween_screen_position(1, 0, 0, 1, 1, 1, 'linear', 'pingpong', false)",
                    "Unknown loop mode 'pingpong'",
                ),
            ] {
                let err = run_err(&format!("engine.{prefix}{call}"));
                assert!(err.contains(bad), "{prefix}{call}: {err}");
            }
        }
    }

    #[test]
    fn shape_style_functions_queue_one_command_each_and_have_collision_twins() {
        use crate::resources::lua_runtime::EntityCmd;

        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load(
                "engine.entity_set_shape_color(1, 1, 2, 3, 4)\n\
                 engine.entity_set_shape_outline_color(1, 5, 6, 7, 8)\n\
                 engine.entity_remove_shape_outline_color(1)\n\
                 engine.entity_set_shape_filled(1, true)\n\
                 engine.entity_set_shape_origin(1, 2, 3)\n\
                 engine.entity_remove_shape(1)\n\
                 engine.collision_entity_set_shape_filled(1, false)\n\
                 engine.collision_entity_remove_shape(1)",
            )
            .exec()
            .unwrap();

        let mut regular = Vec::new();
        runtime.drain_entity_commands_into(&mut regular);
        let regular: Vec<&EntityCmd> = regular.iter().map(|c| &c.cmd).collect();
        assert!(
            matches!(
                regular.as_slice(),
                [
                    EntityCmd::SetShapeColor {
                        color: (1, 2, 3, 4),
                        ..
                    },
                    EntityCmd::SetShapeOutlineColor {
                        color: Some((5, 6, 7, 8)),
                        ..
                    },
                    EntityCmd::SetShapeOutlineColor { color: None, .. },
                    EntityCmd::SetShapeFilled { filled: true, .. },
                    EntityCmd::SetShapeOrigin { x: 2.0, y: 3.0, .. },
                    EntityCmd::RemoveShape { .. },
                ]
            ),
            "{regular:?}"
        );

        let mut collision = Vec::new();
        runtime.drain_collision_entity_commands_into(&mut collision);
        let collision: Vec<&EntityCmd> = collision.iter().map(|c| &c.cmd).collect();
        assert!(
            matches!(
                collision.as_slice(),
                [
                    EntityCmd::SetShapeFilled { filled: false, .. },
                    EntityCmd::RemoveShape { .. },
                ]
            ),
            "{collision:?}"
        );
    }

    #[test]
    fn shape_geometry_functions_queue_a_ready_kind_and_have_collision_twins() {
        use crate::resources::lua_runtime::EntityCmd;
        use aberred_core::components::shape::ShapeKind;
        use aberred_core::math::Vec2;

        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load(
                "engine.entity_set_shape_point(1)\n\
                 engine.entity_set_shape_line(1, 3, 4)\n\
                 engine.entity_set_shape_rect(1, 5, 6)\n\
                 engine.entity_set_shape_circle(1, 2)\n\
                 engine.entity_set_shape_ellipse(1, 7, 8)\n\
                 engine.entity_set_shape_polygon(1, { {x=0,y=0}, {x=4,y=0}, {x=2,y=3} })\n\
                 engine.collision_entity_set_shape_circle(1, 9)\n\
                 engine.collision_entity_set_shape_polygon(1, { {x=0,y=0}, {x=1,y=0}, {x=0,y=1} })",
            )
            .exec()
            .unwrap();

        let mut regular = Vec::new();
        runtime.drain_entity_commands_into(&mut regular);
        let kinds: Vec<&ShapeKind> = regular
            .iter()
            .map(|c| match &c.cmd {
                EntityCmd::SetShapeKind { kind, .. } => kind,
                other => panic!("unexpected command {other:?}"),
            })
            .collect();
        assert!(matches!(kinds[0], ShapeKind::Point));
        assert_eq!(
            *kinds[1],
            ShapeKind::Line {
                to: Vec2::new(3.0, 4.0)
            }
        );
        assert_eq!(
            *kinds[2],
            ShapeKind::Rect {
                size: Vec2::new(5.0, 6.0)
            }
        );
        assert_eq!(*kinds[3], ShapeKind::Circle { radius: 2.0 });
        assert_eq!(
            *kinds[4],
            ShapeKind::Ellipse {
                radius: Vec2::new(7.0, 8.0)
            }
        );
        let ShapeKind::Polygon(polygon) = kinds[5] else {
            panic!("a polygon kind");
        };
        assert_eq!(polygon.triangles().len(), 1, "triangulated at call time");
        assert_eq!(kinds.len(), 6);

        let mut collision = Vec::new();
        runtime.drain_collision_entity_commands_into(&mut collision);
        assert!(matches!(
            collision
                .iter()
                .map(|c| &c.cmd)
                .collect::<Vec<_>>()
                .as_slice(),
            [
                EntityCmd::SetShapeKind {
                    kind: ShapeKind::Circle { radius: 9.0 },
                    ..
                },
                EntityCmd::SetShapeKind {
                    kind: ShapeKind::Polygon(_),
                    ..
                },
            ]
        ));
    }

    #[test]
    fn bad_polygon_points_raise_at_the_call_and_queue_nothing() {
        let runtime = LuaRuntime::new().unwrap();
        for points in ["{ {x=0,y=0}, 5 }", "{ {x=0,y=0}, {x=1} }"] {
            let err = runtime
                .lua()
                .load(format!("engine.entity_set_shape_polygon(1, {points})"))
                .exec()
                .unwrap_err()
                .to_string();
            assert!(err.contains("point 2"), "{points}: {err}");
        }
        let mut queued = Vec::new();
        runtime.drain_entity_commands_into(&mut queued);
        assert!(queued.is_empty());
    }

    #[test]
    fn runtime_tween_inserts_accept_documented_names() {
        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load(
                "engine.entity_insert_tween_position(1, 0, 0, 1, 1, 1, 'cubic_in_out', 'ping_pong', false) \
                 engine.collision_entity_insert_tween_rotation(1, 0, 90, 1, 'quad_out', 'loop', true, 'done')",
            )
            .exec()
            .unwrap();
    }
}
