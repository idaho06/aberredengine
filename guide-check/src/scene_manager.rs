// Approach A — SceneManager (recommended for multi-scene games)
use aberredengine::prelude::*;

mod scenes;

fn main() -> Result<(), EngineError> {
    EngineBuilder::new()
        .config("config.ini")
        .title("My Game")
        .on_setup(scenes::load_assets)
        .add_scene("menu")
        .add_scene("level01")
        .on_scene_enter("menu", scenes::menu::enter)
        .add_scene_system("menu", scenes::menu::update)
        .on_scene_enter("level01", scenes::level01::enter)
        .add_scene_system("level01", scenes::level01::update)
        .on_scene_exit("level01", scenes::level01::exit)
        .initial_scene("menu")
        .try_run()
}
