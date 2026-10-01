use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_render_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        engine.set(
            "load_shader",
            self.lua.create_function(
                |lua, (id, vs_path, fs_path): (String, Option<String>, Option<String>)| {
                    if vs_path.is_none() && fs_path.is_none() {
                        return Err(LuaError::runtime(
                            "load_shader: at least one of vs_path or fs_path must be provided",
                        ));
                    }
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::Shader {
                            id,
                            vs_path,
                            fs_path,
                        });
                    Ok(())
                },
            )?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "load_shader",
            "Load a shader (at least one of vs_path/fs_path required)",
            "render",
            &[
                ("id", "string"),
                ("vs_path", "string?"),
                ("fs_path", "string?"),
            ],
            None,
        )?;

        engine.set(
            "post_process_shader",
            self.lua.create_function(|lua, value: LuaValue| {
                let ids: Option<Vec<String>> = match value {
                    LuaValue::Nil => None,
                    LuaValue::Table(t) => {
                        let mut vec = Vec::new();
                        for pair in t.pairs::<i64, String>() {
                            let (_, id) = pair?;
                            vec.push(id);
                        }
                        if vec.is_empty() {
                            return Err(LuaError::runtime(
                                "post_process_shader: table must contain at least one shader ID",
                            ));
                        }
                        Some(vec)
                    }
                    _ => {
                        return Err(LuaError::runtime(
                            "post_process_shader: expected nil or table of shader IDs",
                        ));
                    }
                };
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .render_commands
                    .borrow_mut()
                    .push(RenderCmd::SetPostProcessShader { ids });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "post_process_shader",
            "Set active post-processing shader chain (nil to clear)",
            "render",
            &[("shader_ids", "string[]?")],
            None,
        )?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_float",
            render_commands,
            |(name, value)| (String, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Float(value)
            },
            desc = "Set a float uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("value", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_int",
            render_commands,
            |(name, value)| (String, i32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Int(value)
            },
            desc = "Set an int uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("value", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_vec2",
            render_commands,
            |(name, x, y)| (String, f32, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Vec2 { x, y }
            },
            desc = "Set a vec2 uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("x", "number"), ("y", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_vec4",
            render_commands,
            |(name, x, y, z, w)| (String, f32, f32, f32, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Vec4 { x, y, z, w }
            },
            desc = "Set a vec4 uniform on post-process shader",
            cat = "render",
            params = [
                ("name", "string"),
                ("x", "number"),
                ("y", "number"),
                ("z", "number"),
                ("w", "number")
            ]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_clear_uniform",
            render_commands,
            |name| String,
            RenderCmd::ClearPostProcessUniform { name },
            desc = "Clear a uniform on post-process shader",
            cat = "render",
            params = [("name", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_clear_uniforms",
            render_commands,
            |()| (),
            RenderCmd::ClearPostProcessUniforms,
            desc = "Clear all uniforms on post-process shader",
            cat = "render",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_gui_theme_panel",
            gui_theme_commands,
            |(
                theme_key,
                tex_key,
                source_x,
                source_y,
                source_w,
                source_h,
                left,
                top,
                right,
                bottom,
            )| (String, String, f32, f32, f32, f32, i32, i32, i32, i32),
            RenderCmd::SetGuiThemePanel {
                theme_key,
                tex_key,
                source_x,
                source_y,
                source_w,
                source_h,
                left,
                top,
                right,
                bottom
            },
            desc = "Set the named theme's GuiWindow nine-patch panel texture/region/borders in GuiThemeStore",
            cat = "render",
            params = [
                ("theme_key", "string"),
                ("tex_key", "string"),
                ("source_x", "number"),
                ("source_y", "number"),
                ("source_w", "number"),
                ("source_h", "number"),
                ("left", "integer"),
                ("top", "integer"),
                ("right", "integer"),
                ("bottom", "integer")
            ]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_gui_theme_button",
            gui_theme_commands,
            |(
                theme_key,
                state,
                tex_key,
                source_x,
                source_y,
                source_w,
                source_h,
                left,
                top,
                right,
                bottom,
            )| (
                String, String, String, f32, f32, f32, f32, i32, i32, i32, i32
            ),
            RenderCmd::SetGuiThemeButton {
                theme_key,
                state,
                tex_key,
                source_x,
                source_y,
                source_w,
                source_h,
                left,
                top,
                right,
                bottom
            },
            desc = "Set one button-state nine-patch skin on the named theme. Call once per state: \"normal\"/\"hover\"/\"pressed\"/\"disabled\"",
            cat = "render",
            params = [
                ("theme_key", "string"),
                ("state", "string"),
                ("tex_key", "string"),
                ("source_x", "number"),
                ("source_y", "number"),
                ("source_w", "number"),
                ("source_h", "number"),
                ("left", "integer"),
                ("top", "integer"),
                ("right", "integer"),
                ("bottom", "integer")
            ]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_gui_theme_label",
            gui_theme_commands,
            |(
                theme_key,
                tex_key,
                source_x,
                source_y,
                source_w,
                source_h,
                left,
                top,
                right,
                bottom,
            )| (String, String, f32, f32, f32, f32, i32, i32, i32, i32),
            RenderCmd::SetGuiThemeLabel {
                theme_key,
                tex_key,
                source_x,
                source_y,
                source_w,
                source_h,
                left,
                top,
                right,
                bottom
            },
            desc = "Set the named theme's GuiLabel nine-patch panel texture/region/borders in GuiThemeStore",
            cat = "render",
            params = [
                ("theme_key", "string"),
                ("tex_key", "string"),
                ("source_x", "number"),
                ("source_y", "number"),
                ("source_w", "number"),
                ("source_h", "number"),
                ("left", "integer"),
                ("top", "integer"),
                ("right", "integer"),
                ("bottom", "integer")
            ]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_gui_theme_font",
            gui_theme_commands,
            |(theme_key, font_key, font_size, r, g, b, a)| (String, String, f32, u8, u8, u8, u8),
            RenderCmd::SetGuiThemeFont {
                theme_key,
                font_key,
                font_size,
                r,
                g,
                b,
                a
            },
            desc = "Set the named theme's caption font/size/color in GuiThemeStore, used by every GuiButton/GuiLabel caption that references it",
            cat = "render",
            params = [
                ("theme_key", "string"),
                ("font_key", "string"),
                ("font_size", "number"),
                ("r", "integer"),
                ("g", "integer"),
                ("b", "integer"),
                ("a", "integer")
            ]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_gui_theme_progress_bar",
            gui_theme_commands,
            |(
                theme_key,
                part,
                tex_key,
                source_x,
                source_y,
                source_w,
                source_h,
                left,
                top,
                right,
                bottom,
            )| (
                String, String, String, f32, f32, f32, f32, i32, i32, i32, i32
            ),
            RenderCmd::SetGuiThemeProgressBar {
                theme_key,
                part,
                tex_key,
                source_x,
                source_y,
                source_w,
                source_h,
                left,
                top,
                right,
                bottom
            },
            desc = "Set one part of the named theme's progress bar skin in GuiThemeStore. \
                    `part` is \"track\" (optional background, None = fill-only bar) or \"fill\" (required foreground). \
                    Call from on_setup() — gui_theme_commands has preserve policy and survives scene switches.",
            cat = "render",
            params = [
                ("theme_key", "string"),
                ("part", "string"),
                ("tex_key", "string"),
                ("source_x", "number"),
                ("source_y", "number"),
                ("source_w", "number"),
                ("source_h", "number"),
                ("left", "integer"),
                ("top", "integer"),
                ("right", "integer"),
                ("bottom", "integer")
            ]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_gui_theme_button_shadow",
            gui_theme_commands,
            |(theme_key, state, dx, dy, r, g, b, a)| (String, String, f32, f32, u8, u8, u8, u8),
            RenderCmd::SetGuiThemeButtonShadow {
                theme_key,
                state,
                dx,
                dy,
                r,
                g,
                b,
                a
            },
            desc = "Set the drop shadow for one state of the named theme's GuiButton skin. \
                    state is \"normal\"/\"hover\"/\"pressed\"/\"disabled\"; unset states fall back to \
                    the \"normal\" shadow, which itself falls back to the theme's panel_shadow. \
                    Call from on_setup() — gui_theme_commands has preserve policy.",
            cat = "render",
            params = [
                ("theme_key", "string"),
                ("state", "string"),
                ("dx", "number"),
                ("dy", "number"),
                ("r", "integer"),
                ("g", "integer"),
                ("b", "integer"),
                ("a", "integer")
            ]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_gui_theme_panel_shadow",
            gui_theme_commands,
            |(theme_key, dx, dy, r, g, b, a)| (String, f32, f32, u8, u8, u8, u8),
            RenderCmd::SetGuiThemePanelShadow {
                theme_key,
                dx,
                dy,
                r,
                g,
                b,
                a
            },
            desc = "Set the named theme's panel drop shadow. All nine-patch backgrounds (GuiWindow, GuiButton, GuiLabel, GuiProgressBar) \
                    using this theme will draw a shifted, tinted pre-pass before the main patch. \
                    Call from on_setup() — gui_theme_commands has preserve policy and survives scene switches.",
            cat = "render",
            params = [
                ("theme_key", "string"),
                ("dx", "number"),
                ("dy", "number"),
                ("r", "integer"),
                ("g", "integer"),
                ("b", "integer"),
                ("a", "integer")
            ]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_gui_theme_text_shadow",
            gui_theme_commands,
            |(theme_key, dx, dy, r, g, b, a)| (String, f32, f32, u8, u8, u8, u8),
            RenderCmd::SetGuiThemeTextShadow {
                theme_key,
                dx,
                dy,
                r,
                g,
                b,
                a
            },
            desc = "Set the named theme's caption text drop shadow. The Shadow component is inserted on DynamicText caption \
                    children spawned by gui_button_spawn_system/gui_label_spawn_system when this theme is resolved. \
                    Call from on_setup() — gui_theme_commands has preserve policy and survives scene switches.",
            cat = "render",
            params = [
                ("theme_key", "string"),
                ("dx", "number"),
                ("dy", "number"),
                ("r", "integer"),
                ("g", "integer"),
                ("b", "integer"),
                ("a", "integer")
            ]
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::process_render_command;
    use aberred_core::components::shadow::Shadow;
    use aberred_core::math::{Color, Rect};
    use aberred_core::resources::guitheme::{GuiNinePatch, GuiTheme, GuiThemeStore};
    use aberred_core::resources::postprocessshader::PostProcessShader;
    use aberred_core::resources::uniformvalue::UniformValue;

    /// Runs `script`, then applies every queued render + GUI-theme command.
    fn apply_script(script: &str) -> (PostProcessShader, GuiThemeStore) {
        let runtime = LuaRuntime::new().unwrap();
        runtime.lua().load(script).exec().unwrap();
        // Drains swap the queue with `out`, so each needs its own empty buffer.
        let (mut render, mut theme_cmds) = (Vec::new(), Vec::new());
        runtime.drain_render_commands_into(&mut render);
        runtime.drain_gui_theme_commands_into(&mut theme_cmds);
        let mut post = PostProcessShader::default();
        let mut themes = GuiThemeStore::default();
        for cmd in render.into_iter().chain(theme_cmds) {
            process_render_command(cmd, &mut post, &mut themes);
        }
        (post, themes)
    }

    fn theme<'a>(themes: &'a GuiThemeStore, key: &str) -> &'a GuiTheme {
        themes.themes.get(key).expect("theme staged")
    }

    fn patch(p: &GuiNinePatch) -> (&str, Rect, [i32; 4]) {
        (&p.tex_key, p.source, [p.left, p.top, p.right, p.bottom])
    }

    #[test]
    fn post_process_shader_chain_keeps_order_and_nil_clears() {
        let (post, _) = apply_script("engine.post_process_shader({'bloom', 'crt', 'vignette'})");
        let keys: Vec<&str> = post.keys.iter().map(|k| &**k).collect();
        assert_eq!(keys, ["bloom", "crt", "vignette"]);

        let (post, _) =
            apply_script("engine.post_process_shader({'bloom'}) engine.post_process_shader(nil)");
        assert!(post.keys.is_empty());
    }

    #[test]
    fn post_process_shader_chain_follows_index_order_not_insertion_order() {
        // Filling from the top puts the keys in the table's hash part, where pairs()
        // order is unspecified; the chain must still follow indices 1..n.
        let (post, _) = apply_script(
            "local t = {} t[4] = 'd' t[3] = 'c' t[2] = 'b' t[1] = 'a' \
             engine.post_process_shader(t)",
        );
        let keys: Vec<&str> = post.keys.iter().map(|k| &**k).collect();
        assert_eq!(keys, ["a", "b", "c", "d"]);
    }

    #[test]
    fn post_process_shader_rejects_empty_table_and_non_table() {
        let runtime = LuaRuntime::new().unwrap();
        for (script, msg) in [
            (
                "engine.post_process_shader({})",
                "table must contain at least one shader ID",
            ),
            (
                "engine.post_process_shader('bloom')",
                "expected nil or table of shader IDs",
            ),
        ] {
            let err = runtime.lua().load(script).exec().unwrap_err().to_string();
            assert!(err.contains(msg), "{script}: {err}");
        }
    }

    #[test]
    fn post_process_uniforms_set_each_type_and_clear() {
        let (post, _) = apply_script(
            "engine.post_process_set_float('amp', 1.5) engine.post_process_set_int('mode', 2) \
             engine.post_process_set_vec2('dir', 1, 0) engine.post_process_set_vec4('tint', 1, 0.5, 0.25, 1) \
             engine.post_process_set_float('uTime', 9) engine.post_process_clear_uniform('dir')",
        );
        assert_eq!(post.uniforms.get("amp"), Some(&UniformValue::Float(1.5)));
        assert_eq!(post.uniforms.get("mode"), Some(&UniformValue::Int(2)));
        assert_eq!(post.uniforms.get("dir"), None);
        assert_eq!(
            post.uniforms.get("tint"),
            Some(&UniformValue::Vec4 {
                x: 1.0,
                y: 0.5,
                z: 0.25,
                w: 1.0
            })
        );
        assert_eq!(
            post.uniforms.get("uTime"),
            Some(&UniformValue::Float(9.0)),
            "reserved names are stored (with a warning); the renderer overwrites them"
        );

        let (post, _) = apply_script(
            "engine.post_process_set_float('amp', 1) engine.post_process_clear_uniforms()",
        );
        assert!(post.uniforms.is_empty());
    }

    #[test]
    fn gui_theme_label_font_and_shadows_set_their_fields() {
        let (_, themes) = apply_script(
            "engine.set_gui_theme_label('dark', 'ui', 1, 2, 3, 4, 5, 6, 7, 8) \
             engine.set_gui_theme_font('dark', 'arcade', 18, 10, 20, 30, 40) \
             engine.set_gui_theme_panel_shadow('dark', 2, 3, 0, 0, 0, 128) \
             engine.set_gui_theme_text_shadow('dark', 1, 1, 9, 9, 9, 255)",
        );
        let t = theme(&themes, "dark");
        assert_eq!(
            patch(t.label.as_ref().unwrap()),
            ("ui", Rect::new(1.0, 2.0, 3.0, 4.0), [5, 6, 7, 8])
        );
        assert_eq!((&*t.font, t.font_size), ("arcade", 18.0));
        assert_eq!(t.text_color, Color::new(10, 20, 30, 40));
        assert_eq!(t.panel_shadow, Some(Shadow::new(2.0, 3.0, 0, 0, 0, 128)));
        assert_eq!(t.text_shadow, Some(Shadow::new(1.0, 1.0, 9, 9, 9, 255)));
    }

    #[test]
    fn gui_theme_progress_bar_parts_and_unknown_part_ignored() {
        let (_, themes) = apply_script(
            "engine.set_gui_theme_progress_bar('hud', 'track', 'bar', 0, 0, 32, 8, 2, 2, 2, 2) \
             engine.set_gui_theme_progress_bar('hud', 'fill', 'bar', 0, 8, 32, 8, 1, 1, 1, 1) \
             engine.set_gui_theme_progress_bar('hud', 'glow', 'bar', 0, 16, 32, 8, 0, 0, 0, 0)",
        );
        let skin = theme(&themes, "hud").progress_bar.as_ref().unwrap();
        assert_eq!(
            patch(skin.track.as_ref().unwrap()),
            ("bar", Rect::new(0.0, 0.0, 32.0, 8.0), [2, 2, 2, 2])
        );
        assert_eq!(
            patch(&skin.fill),
            ("bar", Rect::new(0.0, 8.0, 32.0, 8.0), [1, 1, 1, 1])
        );
    }

    #[test]
    fn gui_theme_button_shadow_per_state_and_unknown_states_ignored() {
        let (_, themes) = apply_script(
            "engine.set_gui_theme_button_shadow('b', 'normal', 1, 1, 1, 1, 1, 1) \
             engine.set_gui_theme_button_shadow('b', 'hover', 2, 2, 2, 2, 2, 2) \
             engine.set_gui_theme_button_shadow('b', 'pressed', 3, 3, 3, 3, 3, 3) \
             engine.set_gui_theme_button_shadow('b', 'disabled', 4, 4, 4, 4, 4, 4) \
             engine.set_gui_theme_button_shadow('b', 'focused', 5, 5, 5, 5, 5, 5) \
             engine.set_gui_theme_button('b', 'focused', 'ui', 0, 0, 1, 1, 0, 0, 0, 0)",
        );
        let skin = theme(&themes, "b").button.as_ref().unwrap();
        let dx = |s: Option<Shadow>| s.map(|s| s.offset.x);
        assert_eq!(
            [
                dx(skin.shadow),
                dx(skin.hover_shadow),
                dx(skin.pressed_shadow),
                dx(skin.disabled_shadow)
            ],
            [Some(1.0), Some(2.0), Some(3.0), Some(4.0)]
        );
        assert!(
            skin.hover.is_none() && skin.pressed.is_none() && skin.disabled.is_none(),
            "an unknown button state sets no patch"
        );
        assert!(skin.normal.tex_key.is_empty());
    }
}
