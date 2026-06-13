use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_audio_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "play_music",
            audio_commands,
            |(id, looped)| (String, bool),
            AudioLuaCmd::PlayMusic { id, looped },
            desc = "Play music track",
            cat = "audio",
            params = [("id", "string"), ("looped", "boolean")]
        );
        define_audio_cmd_twins!(engine, self.lua, meta_fns, "", audio_commands, "audio", "");
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "stop_all_music",
            audio_commands,
            |()| (),
            AudioLuaCmd::StopAllMusic,
            desc = "Stop all playing music",
            cat = "audio",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "stop_all_sounds",
            audio_commands,
            |()| (),
            AudioLuaCmd::StopAllSounds,
            desc = "Stop all playing sounds",
            cat = "audio",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "stop_music",
            audio_commands,
            |id| String,
            AudioLuaCmd::StopMusic { id },
            desc = "Stop a specific music track",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "pause_music",
            audio_commands,
            |id| String,
            AudioLuaCmd::PauseMusic { id },
            desc = "Pause a specific music track",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "resume_music",
            audio_commands,
            |id| String,
            AudioLuaCmd::ResumeMusic { id },
            desc = "Resume a previously paused music track",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_music_volume",
            audio_commands,
            |(id, vol)| (String, f32),
            AudioLuaCmd::SetMusicVolume { id, vol },
            desc = "Set the volume of a music track (0.0 to 1.0)",
            cat = "audio",
            params = [("id", "string"), ("vol", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "unload_music",
            audio_commands,
            |id| String,
            AudioLuaCmd::UnloadMusic { id },
            desc = "Unload a specific music track from memory",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "unload_all_music",
            audio_commands,
            |()| (),
            AudioLuaCmd::UnloadAllMusic,
            desc = "Unload all music tracks from memory",
            cat = "audio",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "unload_sound",
            audio_commands,
            |id| String,
            AudioLuaCmd::UnloadSound { id },
            desc = "Unload a specific sound effect from memory",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "unload_all_sounds",
            audio_commands,
            |()| (),
            AudioLuaCmd::UnloadAllSounds,
            desc = "Unload all sound effects from memory",
            cat = "audio",
            params = []
        );
        Ok(())
    }
}