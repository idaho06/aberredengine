// Cargo.toml
mod cargo_toml {
    fn main() { // GLUE
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    } // GLUE
}

// config.ini
mod config_ini {
    use aberredengine::engine_app::EngineBuilder; // GLUE
    fn builder() { // GLUE
    let _ = // GLUE
    EngineBuilder::new()
        .config_str("[render]\nwidth = 320\nheight = 180\n[window]\nwidth = 960\nheight = 540\n")
        // …
    ; // GLUE
    } // GLUE
}

// AppState API
mod appstate_api {
    use aberredengine::imgui;
    use aberredengine::core::resources::appstate::AppState;
    use aberredengine::render::resources::fontstore::FontStore;
    use aberredengine::render::resources::texturestore::TextureStore;
    use aberredengine::core::resources::worldsignals::SignalSnapshot;
    use aberredengine::core::resources::signal_intents::SignalIntents;
    use aberredengine::bevy_ecs::prelude::ResMut;

    #[derive(Clone)]
    struct InspectorSnapshot {
        selected_name: String,
    }

    // ECS system writes typed state (logic thread)
    fn inspector_system(mut app_state: ResMut<AppState>) {
        app_state.insert(InspectorSnapshot {
            selected_name: "Player".to_string(),
        });
    }

    // GUI callback reads it (render thread — see Threading Model)
    fn inspector_gui(
        ui: &imgui::Ui,
        _signals: &SignalSnapshot,
        _intents: &mut SignalIntents,
        _textures: &TextureStore,
        _fonts: &FontStore,
        app_state: &AppState,
    ) {
        if let Some(snapshot) = app_state.get::<InspectorSnapshot>() {
            ui.text(format!("Selected: {}", snapshot.selected_name));
        }
    }
}

// Runtime modification
mod runtime_modification {
    use aberredengine::bevy_ecs::prelude::ResMut; // GLUE
    use aberredengine::core::resources::gameconfig::GameConfig; // GLUE

    fn my_system(mut config: ResMut<GameConfig>) {
        config.set_render_size(1280, 720);
        config.set_window_size(1920, 1080);
        config.save_to_file().expect("Failed to save config");
    }
}
