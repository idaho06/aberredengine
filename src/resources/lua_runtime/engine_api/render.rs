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
            &[("id", "string"), ("vs_path", "string?"), ("fs_path", "string?")],
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
            render_commands,
            |(tex_key, source_x, source_y, source_w, source_h, left, top, right, bottom)| (
                String, f32, f32, f32, f32, i32, i32, i32, i32
            ),
            RenderCmd::SetGuiThemePanel {
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
            desc = "Set the GuiWindow theme's nine-patch panel texture/region/borders",
            cat = "render",
            params = [
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
            render_commands,
            |(state, tex_key, source_x, source_y, source_w, source_h, left, top, right, bottom)| (
                String, String, f32, f32, f32, f32, i32, i32, i32, i32
            ),
            RenderCmd::SetGuiThemeButton {
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
            desc = "Set one button-state nine-patch skin. Call once per state: \"normal\"/\"hover\"/\"pressed\"/\"disabled\"",
            cat = "render",
            params = [
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
            render_commands,
            |(tex_key, source_x, source_y, source_w, source_h, left, top, right, bottom)| (
                String, f32, f32, f32, f32, i32, i32, i32, i32
            ),
            RenderCmd::SetGuiThemeLabel {
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
            desc = "Set the GuiLabel theme's nine-patch panel texture/region/borders",
            cat = "render",
            params = [
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

        Ok(())
    }
}