use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_animation_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "register_animation",
            animation_commands,
            |(
                id,
                tex_key,
                pos_x,
                pos_y,
                horizontal_displacement,
                vertical_displacement,
                frame_count,
                fps,
                looped,
            )| (String, String, f32, f32, f32, f32, usize, f32, bool),
            AnimationCmd::RegisterAnimation {
                id,
                tex_key,
                pos_x,
                pos_y,
                horizontal_displacement,
                vertical_displacement,
                frame_count,
                fps,
                looped,
            },
            desc = "Register an animation definition",
            cat = "animation",
            params = [
                ("id", "string"),
                ("tex_key", "string"),
                ("pos_x", "number"),
                ("pos_y", "number"),
                ("horizontal_displacement", "number"),
                ("vertical_displacement", "number"),
                ("frame_count", "integer"),
                ("fps", "number"),
                ("looped", "boolean")
            ]
        );
        Ok(())
    }
}