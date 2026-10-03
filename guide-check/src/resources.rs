// Cargo.toml
mod cargo_toml {
    fn main() { // GLUE
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    } // GLUE
}

// config.ini
mod config_ini {
    use aberredengine::prelude::*; // GLUE
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
    use aberredengine::prelude::*;

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
    fn inspector_gui(ctx: &mut GuiCtx) {
        if let Some(snapshot) = ctx.app_state.get::<InspectorSnapshot>() {
            ctx.ui.text(format!("Selected: {}", snapshot.selected_name));
        }
    }
    const _: GuiCallback = inspector_gui; // GLUE
}

// Runtime modification
mod runtime_modification {
    use aberredengine::prelude::*; // GLUE

    fn my_system(mut config: ResMut<GameConfig>) {
        config.set_render_size(1280, 720);
        config.set_window_size(1920, 1080);
        config.save_to_file().expect("Failed to save config");
    }
}

// InputBindings resource
mod inputbindings_resource {
    use aberredengine::prelude::*;
    use aberredengine::core::resources::input_bindings::{
        AxisDirection, GamepadAxis, InputBinding, InputBindings, Key,
    };

    fn setup_controls(mut bindings: ResMut<InputBindings>) {
        // J becomes the only Action1 binding
        bindings.rebind(InputAction::Action1, InputBinding::Keyboard(Key::KEY_J));

        // Backspace also means Back; Escape still works
        bindings.add_binding(InputAction::Back, InputBinding::Keyboard(Key::KEY_BACKSPACE));

        // Pad 1's left stick also drives MainDirectionRight
        bindings.add_binding(
            InputAction::MainDirectionRight,
            InputBinding::GamepadAxis {
                pad: 1,
                axis: GamepadAxis::GAMEPAD_AXIS_LEFT_X,
                direction: AxisDirection::Positive,
            },
        );

        let current: &[InputBinding] = bindings.get_bindings(InputAction::Action1);
    }
}
