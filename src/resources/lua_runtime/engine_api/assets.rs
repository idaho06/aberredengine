use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_asset_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_texture",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::Texture { id, path },
            desc = "Load a texture from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_font",
            asset_commands,
            |(id, path, size)| (String, String, i32),
            AssetCmd::Font { id, path, size },
            desc = "Load a font from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string"), ("size", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_music",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::Music { id, path },
            desc = "Load music from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_sound",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::Sound { id, path },
            desc = "Load a sound effect from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        Ok(())
    }

    /// Registers `engine.load_map(path)` in the Lua `engine` table.
    pub(in crate::resources::lua_runtime) fn register_map_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_map",
            map_commands,
            |path| String,
            MapLuaCmd::LoadMap { path },
            desc = "Load a map JSON file and spawn all its assets and entities",
            cat = "asset",
            params = [("path", "string")]
        );

        Ok(())
    }
}