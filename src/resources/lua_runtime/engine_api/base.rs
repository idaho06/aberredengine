use super::*;
use log::{debug, error, info, warn};

impl LuaRuntime {
    /// Registers the base `engine` table with logging functions.
    pub(in crate::resources::lua_runtime) fn register_base_api(&self) -> LuaResult<()> {
        let engine = self.lua.create_table()?;

        // Create __meta table with functions and classes subtables
        let meta = self.lua.create_table()?;
        let meta_fns = self.lua.create_table()?;
        let meta_classes = self.lua.create_table()?;
        let meta_types = self.lua.create_table()?;
        let meta_enums = self.lua.create_table()?;
        let meta_callbacks = self.lua.create_table()?;
        meta.set("functions", &meta_fns)?;
        meta.set("classes", &meta_classes)?;
        meta.set("types", &meta_types)?;
        meta.set("enums", &meta_enums)?;
        meta.set("callbacks", &meta_callbacks)?;
        engine.set("__meta", meta)?;

        register_log_fn!(
            engine,
            self.lua,
            meta_fns,
            "log",
            info,
            "General purpose logging"
        );
        register_log_fn!(
            engine,
            self.lua,
            meta_fns,
            "log_info",
            info,
            "Info level logging"
        );
        register_log_fn!(
            engine,
            self.lua,
            meta_fns,
            "log_warn",
            warn,
            "Warning level logging"
        );
        register_log_fn!(
            engine,
            self.lua,
            meta_fns,
            "log_error",
            error,
            "Error level logging"
        );
        register_log_fn!(
            engine,
            self.lua,
            meta_fns,
            "log_debug",
            debug,
            "Debug level logging"
        );

        self.lua.globals().set("engine", engine)?;

        Ok(())
    }
}