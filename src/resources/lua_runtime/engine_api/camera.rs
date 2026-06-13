use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_camera_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        define_camera_cmd_twins!(engine, self.lua, meta_fns, "", camera_commands, "camera", "");

        engine.set(
            "get_camera",
            self.lua.create_function(|lua, ()| {
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
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_camera",
            "Get the current 2D camera state (target, offset, rotation, zoom). \
             Returns values from the start of this frame after camera_follow_system has run. \
             If called in the same callback as set_camera(), returns pre-override values. \
             Only available during on_update callbacks; returns defaults (zoom=1) from on_setup / on_switch_scene. \
             Each call returns a new table; cache locally if reading multiple fields.",
            "camera",
            &[],
            Some("table"),
        )?;

        engine.set(
            "get_camera_view_rect",
            self.lua.create_function(|lua, ()| {
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
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_camera_view_rect",
            "Get the visible world-space rectangle for the current camera: top-left corner (x, y) \
             plus visible dimensions (w, h) in world units. \
             Assumes zero camera rotation — under non-zero rotation the result is an axis-aligned \
             approximation only. \
             Only available during on_update callbacks; returns {{ x=0, y=0, w=0, h=0 }} from \
             on_setup / on_switch_scene. \
             Each call returns a new table; cache locally if reading multiple fields.",
            "camera",
            &[],
            Some("table"),
        )?;

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
            CameraFollowCmd::SetMode { mode },
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
            CameraFollowCmd::SetEasing { easing },
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