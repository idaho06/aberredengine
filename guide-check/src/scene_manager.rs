// Approach A — SceneManager (recommended for multi-scene games)
use aberredengine::engine_app::EngineBuilder;
use aberredengine::engine_app::SceneDescriptor;

mod scenes;

fn main() -> Result<(), aberredengine::EngineError> {
    EngineBuilder::new()
        .config("config.ini")
        .title("My Game")
        .on_setup(scenes::load_assets)
        .add_scene("menu", SceneDescriptor {
            on_enter:     scenes::menu::enter,
            on_update:    Some(scenes::menu::update),
            on_exit:      None,
            gui_callback: None,
            world_draw_callback: None,
        })
        .add_scene("level01", SceneDescriptor {
            on_enter:     scenes::level01::enter,
            on_update:    Some(scenes::level01::update),
            on_exit:      Some(scenes::level01::exit),
            gui_callback: None,
            world_draw_callback: None,
        })
        .initial_scene("menu")
        .try_run()
}
