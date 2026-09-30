use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_gameconfig_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_fullscreen",
            gameconfig_commands,
            |enabled| bool,
            GameConfigCmd::Fullscreen { enabled },
            desc = "Set fullscreen mode",
            cat = "render",
            params = [("enabled", "boolean")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_vsync",
            gameconfig_commands,
            |enabled| bool,
            GameConfigCmd::Vsync { enabled },
            desc = "Set vertical sync",
            cat = "render",
            params = [("enabled", "boolean")]
        );

        engine.set(
            "set_target_fps",
            self.lua.create_function(|lua, fps: Option<u32>| {
                let fps = fps.unwrap_or(60);
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .gameconfig_commands
                    .borrow_mut()
                    .push(GameConfigCmd::TargetFps { fps });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "set_target_fps",
            "Set target FPS (nil resets to 60)",
            "render",
            &[("fps", "integer?")],
            None,
        )?;

        register_getter!(engine, self.lua, meta_fns, "get_fullscreen",
            |lua, ()| {
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.gameconfig_snapshot.borrow().fullscreen)
                    .unwrap_or(false))
            },
            desc = "Get current fullscreen state", cat = "render",
            params = [], returns = "boolean");

        register_getter!(engine, self.lua, meta_fns, "get_vsync",
            |lua, ()| {
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.gameconfig_snapshot.borrow().vsync)
                    .unwrap_or(false))
            },
            desc = "Get current vsync state", cat = "render",
            params = [], returns = "boolean");

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_pixel_snap_camera",
            gameconfig_commands,
            |enabled| bool,
            GameConfigCmd::PixelSnapCamera { enabled },
            desc = "Snap the camera/view rect to integer pixels before rendering (reduces sprite atlas bleeding; disable for smooth rotation/zoom)",
            cat = "render",
            params = [("enabled", "boolean")]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_render_target_filter",
            gameconfig_commands,
            |filter| String,
            GameConfigCmd::RenderTargetFilter { filter },
            desc = "Set the texture filter for the final render-target-to-window blit (\"nearest\", \"bilinear\", \"trilinear\", \"anisotropic_4x\", \"anisotropic_8x\", \"anisotropic_16x\")",
            cat = "render",
            params = [("filter", "string")]
        );

        register_getter!(engine, self.lua, meta_fns, "get_pixel_snap_camera",
            |lua, ()| {
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.gameconfig_snapshot.borrow().pixel_snap_camera)
                    .unwrap_or(true))
            },
            desc = "Get whether the camera/view rect is snapped to integer pixels", cat = "render",
            params = [], returns = "boolean");

        register_getter!(engine, self.lua, meta_fns, "get_target_fps",
            |lua, ()| {
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.gameconfig_snapshot.borrow().target_fps)
                    .unwrap_or(60))
            },
            desc = "Get current target FPS", cat = "render",
            params = [], returns = "integer");

        engine.set(
            "set_render_size",
            self.lua
                .create_function(|lua, (width, height): (u32, u32)| {
                    let width = width.clamp(120, 7680);
                    let height = height.clamp(120, 4320);
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .gameconfig_commands
                        .borrow_mut()
                        .push(GameConfigCmd::RenderSize { width, height });
                    Ok(())
                })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "set_render_size",
            "Set internal render resolution (min 120x120, max 7680x4320)",
            "render",
            &[("width", "integer"), ("height", "integer")],
            None,
        )?;

        register_getter!(engine, self.lua, meta_fns, "get_render_size",
            |lua, ()| {
                let (w, h) = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| {
                        let snap = data.gameconfig_snapshot.borrow();
                        (snap.render_width, snap.render_height)
                    })
                    .unwrap_or((640, 360));
                let table = lua.create_table()?;
                table.set("width", w)?;
                table.set("height", h)?;
                Ok(table)
            },
            desc = "Get current internal render resolution", cat = "render",
            params = [], returns = "table");

        engine.set(
            "set_background_color",
            self.lua.create_function(|lua, (r, g, b): (u8, u8, u8)| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .gameconfig_commands
                    .borrow_mut()
                    .push(GameConfigCmd::BackgroundColor { r, g, b });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "set_background_color",
            "Set background clear color (RGB 0-255)",
            "render",
            &[("r", "integer"), ("g", "integer"), ("b", "integer")],
            None,
        )?;

        register_getter!(engine, self.lua, meta_fns, "get_background_color",
            |lua, ()| {
                let (r, g, b) = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| {
                        let snap = data.gameconfig_snapshot.borrow();
                        (snap.background_r, snap.background_g, snap.background_b)
                    })
                    .unwrap_or((80, 80, 80));
                let table = lua.create_table()?;
                table.set("r", r)?;
                table.set("g", g)?;
                table.set("b", b)?;
                Ok(table)
            },
            desc = "Get current background clear color", cat = "render",
            params = [], returns = "table");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::process_gameconfig_command;
    use aberred_core::math::Color;
    use aberred_core::resources::gameconfig::GameConfig;
    use aberred_core::resources::texturefilter::TextureFilter;

    /// Runs `script`, then applies every queued game-config command to `config`.
    fn apply_script(script: &str, config: &mut GameConfig) {
        let runtime = LuaRuntime::new().unwrap();
        runtime.lua().load(script).exec().unwrap();
        let mut cmds = Vec::new();
        runtime.drain_gameconfig_commands_into(&mut cmds);
        for cmd in cmds {
            process_gameconfig_command(cmd, config);
        }
    }

    #[test]
    fn setters_update_game_config_through_the_processor() {
        let mut config = GameConfig::default();
        apply_script(
            "engine.set_fullscreen(true) engine.set_vsync(false) engine.set_target_fps(144) \
             engine.set_render_size(320, 180) engine.set_background_color(10, 20, 30) \
             engine.set_pixel_snap_camera(false) engine.set_render_target_filter('bilinear')",
            &mut config,
        );
        assert!(config.fullscreen && !config.vsync);
        assert_eq!(config.target_fps, 144);
        assert_eq!((config.render_width, config.render_height), (320, 180));
        assert_eq!(config.background_color, Color::new(10, 20, 30, 255));
        assert!(!config.pixel_snap_camera);
        assert_eq!(config.render_target_filter, TextureFilter::Bilinear);
    }

    #[test]
    fn set_target_fps_nil_resets_to_60_and_render_size_clamps() {
        let mut config = GameConfig::default();
        apply_script(
            "engine.set_target_fps(30) engine.set_target_fps() engine.set_render_size(50, 99999)",
            &mut config,
        );
        assert_eq!(config.target_fps, 60);
        assert_eq!((config.render_width, config.render_height), (120, 4320));

        apply_script("engine.set_render_size(99999, 50)", &mut config);
        assert_eq!((config.render_width, config.render_height), (7680, 120));
    }

    #[test]
    fn unknown_render_target_filter_falls_back_to_nearest() {
        let mut config = GameConfig {
            render_target_filter: TextureFilter::Bilinear,
            ..GameConfig::default()
        };
        apply_script("engine.set_render_target_filter('blurry')", &mut config);
        assert_eq!(config.render_target_filter, TextureFilter::Nearest);
    }

    #[test]
    fn getters_read_the_cached_game_config() {
        let runtime = LuaRuntime::new().unwrap();
        let config = GameConfig {
            fullscreen: true,
            vsync: false,
            target_fps: 75,
            render_width: 400,
            render_height: 300,
            background_color: Color::new(1, 2, 3, 255),
            pixel_snap_camera: false,
            ..GameConfig::default()
        };
        runtime.update_gameconfig_cache(&config);

        let (fullscreen, vsync, fps, snap): (bool, bool, u32, bool) = runtime
            .lua()
            .load(
                "return engine.get_fullscreen(), engine.get_vsync(), engine.get_target_fps(), \
                 engine.get_pixel_snap_camera()",
            )
            .eval()
            .unwrap();
        assert_eq!((fullscreen, vsync, fps, snap), (true, false, 75, false));

        let (w, h, r, g, b): (u32, u32, u8, u8, u8) = runtime
            .lua()
            .load(
                "local s = engine.get_render_size() local c = engine.get_background_color() \
                 return s.width, s.height, c.r, c.g, c.b",
            )
            .eval()
            .unwrap();
        assert_eq!((w, h, r, g, b), (400, 300, 1, 2, 3));
    }
}
