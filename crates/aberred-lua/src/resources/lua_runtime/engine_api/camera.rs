use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_camera_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        define_camera_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "",
            camera_commands,
            "camera",
            ""
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_camera",
            |lua, ()| {
                let (target_x, target_y, offset_x, offset_y, rotation, zoom) = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| {
                        let snap = data.camera_snapshot.borrow();
                        (
                            snap.target_x,
                            snap.target_y,
                            snap.offset_x,
                            snap.offset_y,
                            snap.rotation,
                            snap.zoom,
                        )
                    })
                    .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0, 1.0));
                let tbl = lua.create_table()?;
                tbl.set("target_x", target_x)?;
                tbl.set("target_y", target_y)?;
                tbl.set("offset_x", offset_x)?;
                tbl.set("offset_y", offset_y)?;
                tbl.set("rotation", rotation)?;
                tbl.set("zoom", zoom)?;
                Ok(tbl)
            },
            desc = "Get the current 2D camera state (target, offset, rotation, zoom). \
             Returns values from the start of this frame after camera_follow_system has run. \
             If called in the same callback as set_camera(), returns pre-override values. \
             Only available during on_update callbacks; returns defaults (zoom=1) from on_setup / on_switch_scene. \
             Each call returns a new table; cache locally if reading multiple fields.",
            cat = "camera",
            params = [],
            returns = "table"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_camera_view_rect",
            |lua, ()| {
                let (x, y, w, h) = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| {
                        let snap = data.camera_snapshot.borrow();
                        (snap.view_x, snap.view_y, snap.view_w, snap.view_h)
                    })
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                let tbl = lua.create_table()?;
                tbl.set("x", x)?;
                tbl.set("y", y)?;
                tbl.set("w", w)?;
                tbl.set("h", h)?;
                Ok(tbl)
            },
            desc = "Get the visible world-space rectangle for the current camera: top-left corner (x, y) \
             plus visible dimensions (w, h) in world units. \
             Assumes zero camera rotation — under non-zero rotation the result is an axis-aligned \
             approximation only. \
             Only available during on_update callbacks; returns {{ x=0, y=0, w=0, h=0 }} from \
             on_setup / on_switch_scene. \
             Each call returns a new table; cache locally if reading multiple fields.",
            cat = "camera",
            params = [],
            returns = "table"
        );

        Ok(())
    }

    pub(in crate::resources::lua_runtime) fn register_camera_follow_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_enable",
            camera_follow_commands,
            |enabled| bool,
            CameraFollowCmd::Enable { enabled },
            desc = "Enable or disable the camera follow system",
            cat = "camera",
            params = [("enabled", "boolean")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_mode",
            camera_follow_commands,
            |mode| String,
            CameraFollowCmd::SetMode {
                mode: checked_follow_mode(mode)?
            },
            desc = "Set camera follow mode (\"instant\", \"lerp\", \"smooth_damp\")",
            cat = "camera",
            params = [("mode", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_deadzone",
            camera_follow_commands,
            |(half_w, half_h)| (f32, f32),
            CameraFollowCmd::SetDeadzone { half_w, half_h },
            desc = "Set camera follow mode to deadzone with given half-dimensions",
            cat = "camera",
            params = [("half_w", "number"), ("half_h", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_easing",
            camera_follow_commands,
            |easing| String,
            CameraFollowCmd::SetEasing {
                easing: checked_follow_easing(easing)?
            },
            desc = "Set camera follow easing curve (\"linear\", \"ease_out\", \"ease_in\", \"ease_in_out\")",
            cat = "camera",
            params = [("easing", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_speed",
            camera_follow_commands,
            |speed| f32,
            CameraFollowCmd::SetSpeed { speed },
            desc = "Set camera follow lerp speed",
            cat = "camera",
            params = [("speed", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_spring",
            camera_follow_commands,
            |(stiffness, damping)| (f32, f32),
            CameraFollowCmd::SetSpring { stiffness, damping },
            desc = "Set camera follow spring stiffness and damping",
            cat = "camera",
            params = [("stiffness", "number"), ("damping", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_offset",
            camera_follow_commands,
            |(x, y)| (f32, f32),
            CameraFollowCmd::SetOffset { x, y },
            desc = "Set camera follow offset from target position",
            cat = "camera",
            params = [("x", "number"), ("y", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_bounds",
            camera_follow_commands,
            |(x, y, w, h)| (f32, f32, f32, f32),
            CameraFollowCmd::SetBounds { x, y, w, h },
            desc = "Set camera follow world-space bounds (x, y, width, height)",
            cat = "camera",
            params = [
                ("x", "number"),
                ("y", "number"),
                ("w", "number"),
                ("h", "number")
            ]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_clear_bounds",
            camera_follow_commands,
            |()| (),
            CameraFollowCmd::ClearBounds,
            desc = "Clear camera follow bounds",
            cat = "camera",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_reset_velocity",
            camera_follow_commands,
            |()| (),
            CameraFollowCmd::ResetVelocity,
            desc = "Reset camera follow spring velocity to zero",
            cat = "camera",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_zoom_speed",
            camera_follow_commands,
            |speed| f32,
            CameraFollowCmd::SetZoomSpeed { speed },
            desc = "Set zoom interpolation speed (higher = faster zoom transition toward CameraTarget zoom)",
            cat = "camera",
            params = [("speed", "number")]
        );
        Ok(())
    }
}

/// Rejects an unknown follow-mode name at the Lua call site (the processor would only warn).
fn checked_follow_mode(name: String) -> LuaResult<String> {
    use aberred_core::resources::camerafollowconfig::FollowMode;
    match FollowMode::from_name(&name) {
        Some(_) => Ok(name),
        None => Err(LuaError::runtime(format!(
            "Unknown camera follow mode '{name}' (expected one of: {}; use \
             camera_follow_set_deadzone for deadzone)",
            FollowMode::NAMES.join(", ")
        ))),
    }
}

/// Rejects an unknown follow-easing name at the Lua call site (the processor would only warn).
fn checked_follow_easing(name: String) -> LuaResult<String> {
    use aberred_core::resources::camerafollowconfig::EasingCurve;
    match name.parse::<EasingCurve>() {
        Ok(_) => Ok(name),
        Err(()) => Err(LuaError::runtime(format!(
            "Unknown camera follow easing '{name}' (expected one of: {})",
            EasingCurve::NAMES.join(", ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::{process_camera_command, process_camera_follow_command};
    use aberred_core::math::{Rect, Vec2};
    use aberred_core::resources::camera2d::{Camera2D, Camera2DRes};
    use aberred_core::resources::camerafollowconfig::{
        CameraFollowConfig, EasingCurve, FollowMode,
    };
    use aberred_core::resources::screensize::ScreenSize;
    use bevy_ecs::prelude::*;
    use bevy_ecs::system::SystemState;

    fn follow_config_after(script: &str, mut config: CameraFollowConfig) -> CameraFollowConfig {
        let runtime = LuaRuntime::new().unwrap();
        runtime.lua().load(script).exec().unwrap();
        let mut cmds = Vec::new();
        runtime.drain_camera_follow_commands_into(&mut cmds);
        for cmd in cmds {
            process_camera_follow_command(cmd, &mut config);
        }
        config
    }

    #[test]
    fn set_camera_inserts_camera_resource_through_the_processor() {
        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load("engine.set_camera(10, 20, 320, 180, 15, 2)")
            .exec()
            .unwrap();
        let mut cmds = Vec::new();
        runtime.drain_camera_commands_into(&mut cmds);

        let mut world = World::new();
        let mut state = SystemState::<Commands>::new(&mut world);
        {
            let mut commands = state.get_mut(&mut world).unwrap();
            for cmd in cmds {
                process_camera_command(&mut commands, cmd);
            }
        }
        state.apply(&mut world);
        let cam = world.resource::<Camera2DRes>().0;
        assert_eq!(
            (cam.target, cam.offset),
            (Vec2::new(10.0, 20.0), Vec2::new(320.0, 180.0))
        );
        assert_eq!((cam.rotation, cam.zoom), (15.0, 2.0));
    }

    #[test]
    fn camera_getters_read_the_cache_with_and_without_pixel_snap() {
        let camera = Camera2DRes(Camera2D {
            target: Vec2::new(10.4, 20.6),
            offset: Vec2::new(320.0, 180.0),
            rotation: 0.0,
            zoom: 2.0,
        });
        let screen = ScreenSize { w: 640, h: 360 };
        let read = |runtime: &LuaRuntime| -> ([f32; 6], [f32; 4]) {
            let cam: mlua::Table = runtime
                .lua()
                .load("return engine.get_camera()")
                .eval()
                .unwrap();
            let rect: mlua::Table = runtime
                .lua()
                .load("return engine.get_camera_view_rect()")
                .eval()
                .unwrap();
            let f = |t: &mlua::Table, k: &str| t.get::<f32>(k).unwrap();
            (
                [
                    "target_x", "target_y", "offset_x", "offset_y", "rotation", "zoom",
                ]
                .map(|k| f(&cam, k)),
                ["x", "y", "w", "h"].map(|k| f(&rect, k)),
            )
        };

        let runtime = LuaRuntime::new().unwrap();
        runtime.update_camera_cache(&camera, &screen, false);
        let raw = camera.world_visible_rect(&screen);
        assert_eq!(
            read(&runtime),
            (
                [10.4, 20.6, 320.0, 180.0, 0.0, 2.0],
                [raw.x, raw.y, raw.width, raw.height]
            )
        );

        runtime.update_camera_cache(&camera, &screen, true);
        let snapped = camera.world_visible_rect_snapped(&screen);
        assert_eq!(
            read(&runtime),
            (
                [10.0, 21.0, 320.0, 180.0, 0.0, 2.0],
                [snapped.x, snapped.y, snapped.width, snapped.height]
            ),
            "pixel snap rounds the target and uses the snapped view rect"
        );
    }

    #[test]
    fn camera_follow_setters_update_the_config() {
        let config = follow_config_after(
            "engine.camera_follow_enable(true) engine.camera_follow_set_mode('smooth_damp') \
             engine.camera_follow_set_easing('ease_in_out') engine.camera_follow_set_speed(7) \
             engine.camera_follow_set_spring(20, 3) engine.camera_follow_set_offset(4, -2) \
             engine.camera_follow_set_bounds(0, 0, 1000, 500) engine.camera_follow_set_zoom_speed(9)",
            CameraFollowConfig::default(),
        );
        assert!(config.enabled);
        assert_eq!(config.mode, FollowMode::SmoothDamp);
        assert_eq!(config.easing, EasingCurve::EaseInOut);
        assert_eq!(config.lerp_speed, 7.0);
        assert_eq!(
            (config.spring_stiffness, config.spring_damping),
            (20.0, 3.0)
        );
        assert_eq!(config.offset, Vec2::new(4.0, -2.0));
        assert_eq!(
            config.bounds,
            Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 500.0
            })
        );
        assert_eq!(config.zoom_lerp_speed, 9.0);
    }

    #[test]
    fn camera_follow_deadzone_clear_bounds_and_reset_velocity() {
        let start = CameraFollowConfig {
            bounds: Some(Rect {
                x: 1.0,
                y: 1.0,
                width: 2.0,
                height: 2.0,
            }),
            velocity: Vec2::new(5.0, 5.0),
            ..CameraFollowConfig::default()
        };
        let config = follow_config_after(
            "engine.camera_follow_set_deadzone(40, 30) engine.camera_follow_clear_bounds() \
             engine.camera_follow_reset_velocity()",
            start,
        );
        assert_eq!(
            config.mode,
            FollowMode::Deadzone {
                half_w: 40.0,
                half_h: 30.0
            }
        );
        assert_eq!(config.bounds, None);
        assert_eq!(config.velocity, Vec2::ZERO);
    }

    #[test]
    fn camera_follow_unknown_mode_or_easing_is_a_lua_error() {
        for (call, msg) in [
            (
                "engine.camera_follow_set_mode('teleport')",
                "Unknown camera follow mode 'teleport'",
            ),
            (
                "engine.camera_follow_set_mode('deadzone')",
                "Unknown camera follow mode 'deadzone'",
            ),
            (
                "engine.camera_follow_set_easing('bouncy')",
                "Unknown camera follow easing 'bouncy'",
            ),
        ] {
            let runtime = LuaRuntime::new().unwrap();
            let err = runtime.lua().load(call).exec().unwrap_err().to_string();
            assert!(err.contains(msg), "{call}: {err}");
            let mut cmds = Vec::new();
            runtime.drain_camera_follow_commands_into(&mut cmds);
            assert!(cmds.is_empty(), "{call}: nothing queued");
        }
        // Every documented name is still accepted.
        follow_config_after(
            "engine.camera_follow_set_mode('instant') engine.camera_follow_set_mode('lerp') \
             engine.camera_follow_set_mode('smooth_damp') \
             engine.camera_follow_set_easing('linear') engine.camera_follow_set_easing('ease_out') \
             engine.camera_follow_set_easing('ease_in') engine.camera_follow_set_easing('ease_in_out')",
            CameraFollowConfig::default(),
        );
    }
}
