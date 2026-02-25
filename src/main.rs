//! Aberred Engine main entry point.
//!
//! Bootstraps the engine via [`EngineBuilder`]. Lua CLI tools
//! (`--create-lua-stubs`, `--create-luarc`) are handled before the builder
//! is invoked so the engine window is never opened for tool-only runs.

// Do not create console on Windows
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use aberredengine::engine_app::EngineBuilder;
use clap::Parser;
#[cfg(feature = "lua")]
use std::path::PathBuf;

/// Aberred Engine 2D
#[derive(Parser)]
#[command(version, author = "Idaho06 from AkinoSoft! cesar.idaho@gmail.com",
          about = "This is the Aberred Engine 2D! https://github.com/idaho06/aberredengine/")]
struct Cli {
    /// Generate Lua LSP stubs from engine metadata and exit.
    /// Optionally provide a path (default: assets/scripts/engine.lua).
    #[cfg(feature = "lua")]
    #[arg(long, value_name = "PATH")]
    create_lua_stubs: Option<Option<PathBuf>>,

    /// Generate .luarc.json for Lua Language Server and exit.
    /// Optionally provide a path (default: assets/scripts/.luarc.json).
    #[cfg(feature = "lua")]
    #[arg(long, value_name = "PATH")]
    create_luarc: Option<Option<PathBuf>>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let _cli = Cli::parse();

    // Early-exit: generate Lua stubs and quit (no window/audio needed)
    #[cfg(feature = "lua")]
    if let Some(maybe_path) = _cli.create_lua_stubs {
        use aberredengine::resources::lua_runtime::LuaRuntime;
        use aberredengine::stub_generator;

        let path = maybe_path.unwrap_or_else(|| PathBuf::from("assets/scripts/engine.lua"));
        let runtime =
            LuaRuntime::new().expect("Failed to create Lua runtime for stub generation");
        match stub_generator::generate_stubs(&runtime) {
            Ok(content) => {
                if let Err(e) = stub_generator::write_stubs(&path, &content) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                println!("Lua stubs written to {}", path.display());
            }
            Err(e) => {
                eprintln!("Error generating stubs: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Early-exit: generate .luarc.json and quit (no window/audio needed)
    #[cfg(feature = "lua")]
    if let Some(maybe_path) = _cli.create_luarc {
        use aberredengine::luarc_generator;
        use aberredengine::resources::lua_runtime::LuaRuntime;

        let path = maybe_path.unwrap_or_else(|| PathBuf::from("assets/scripts/.luarc.json"));
        let runtime =
            LuaRuntime::new().expect("Failed to create Lua runtime for .luarc.json generation");
        match luarc_generator::generate_luarc(&runtime, "engine.lua") {
            Ok(content) => {
                if let Err(e) = luarc_generator::write_luarc(&path, &content) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                println!(".luarc.json written to {}", path.display());
            }
            Err(e) => {
                eprintln!("Error generating .luarc.json: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Run the engine with the Lua plugin
    #[cfg(feature = "lua")]
    {
        EngineBuilder::new()
            .with_lua("./assets/scripts/main.lua")
            .run();
    }

    // Pure-Rust path: no hooks registered yet (placeholder for downstream games)
    #[cfg(not(feature = "lua"))]
    {
        EngineBuilder::new().run();
    }
}
