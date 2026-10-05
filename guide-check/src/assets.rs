// 4. Loading Assets
mod loading_assets {
    use aberredengine::prelude::*;

    fn setup(mut assets: AssetLoader) -> Result {
        assets.load_texture("player", "assets/textures/player.png")?;
        assets.load_texture_with("background", "assets/textures/bg.png", TextureFilter::Bilinear)?;
        assets.load_font("arcade", "assets/fonts/arcade.ttf", 32)?;
        assets.load_shader("glow", None, Some("assets/shaders/glow.fs"))?;
        assets.load_sound("jump", "assets/audio/jump.wav")?;
        assets.load_music("bgm", "assets/audio/music.ogg")?;
        Ok(())
    }
}

// AssetLoader passthroughs
mod asset_loader_passthroughs {
    use aberredengine::prelude::*;

    fn play_jump(mut assets: AssetLoader, input: Res<InputState>) {
        if input.action(InputAction::Action1).just_pressed {
            assets.audio().write(AudioCmd::PlayFx { id: "jump".into() });
        }
    }
}

// Waiting for loads
mod waiting_for_loads {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;

    fn on_asset_loaded(ev: On<AssetLoaded>) {
        log::info!("{:?} '{}' is ready", ev.kind, ev.key);
    }

    fn on_asset_failed(ev: On<AssetLoadFailed>, mut signals: ResMut<WorldSignals>) {
        // The engine has already logged the error.
        if ev.kind == AssetKind::Texture && ev.key == "player" {
            signals.request_quit();
        }
    }

    EngineBuilder::new()
        .add_observer(on_asset_loaded)
        .add_observer(on_asset_failed)
        // …
    } // GLUE
}

// Loading screen
mod loading_screen {
    use aberredengine::prelude::*;

    #[derive(Component)]
    struct LoadingText;

    fn load_assets(mut assets: AssetLoader) -> Result {
        assets.load_font("ui", "assets/fonts/ui.ttf", 16)?;
        assets.load_texture("player", "assets/textures/player.png")?;
        Ok(())
    }

    fn show_loading(_: On<SceneEntered>, mut commands: Commands) {
        commands.spawn((
            LoadingText,
            ScreenPosition::new(10.0, 10.0),
            DynamicText::new("Loading…", "ui", 16.0, Color::WHITE),
        ));
    }

    fn update_loading(
        pending: Res<PendingAssets>,
        mut texts: Query<&mut DynamicText, With<LoadingText>>,
    ) {
        if pending.is_changed() {
            for mut text in &mut texts {
                text.set_text(format!("Loading… ({} left)", pending.len()));
            }
        }
    }

    fn main() -> Result<(), EngineError> {
        EngineBuilder::new()
            .on_setup(load_assets)
            .add_scene("loading")
            .add_scene("level01")
            .initial_scene("level01")
            .loading_scene("loading")
            .on_scene_enter("loading", show_loading)
            .add_scene_system("loading", update_loading)
            .try_run()
    }
}

// Textures
mod textures {
    use aberredengine::prelude::*; // GLUE

    fn queue_textures(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    use aberredengine::prelude::*;

    asset_cmds.write(RenderAssetCmd::Texture {
        key: "player".to_string(),
        path: "assets/textures/player.png".to_string(),
        filter: TextureFilter::Nearest,
    });
    asset_cmds.write(RenderAssetCmd::Texture {
        key: "background".to_string(),
        path: "assets/textures/background.png".to_string(),
        filter: TextureFilter::Nearest,
    });
    } // GLUE

    use aberredengine::prelude::*;
    use aberredengine::core::resources::texturedims::TextureDimsStore;

    fn spawn_player_once_texture_ready(
        dims: Res<TextureDimsStore>,
        mut world_signals: ResMut<WorldSignals>,
        mut commands: Commands,
    ) {
        if world_signals.has_flag("player_spawned") {
            return;
        }
        // `.get()` returns None until the render thread's TextureLoaded reply
        // lands — usually a tick or two after the RenderAssetCmd::Texture was
        // queued, never in the same tick.
        let Some((width, height)) = dims.get("player") else {
            return; // still waiting — try again next tick
        };

        commands.spawn((
            MapPosition::new(100.0, 200.0),
            Sprite::new("player", width as f32, height as f32).centered(),
            ZIndex(1.0),
        ));
        world_signals.set_flag("player_spawned");
    }

    fn queue_from_memory(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    asset_cmds.write(RenderAssetCmd::TextureFromMemory {
        key: "intro_logo".to_string(),
        ext: ".png".to_string(), // leading dot, matches raylib's file-type hint
        bytes: include_bytes!("../assets/textures/intro_logo.png").to_vec(),
        filter: TextureFilter::Nearest,
    });
    } // GLUE
}

// Fonts
mod fonts {
    use aberredengine::prelude::*; // GLUE

    fn queue_font(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    asset_cmds.write(RenderAssetCmd::Font {
        key: "arcade".to_string(),
        path: "assets/fonts/arcade.ttf".to_string(),
        size: 32,
        skip_if_loaded: false, // true = don't reload if "arcade" is already loaded
    });
    } // GLUE

    use aberredengine::core::resources::fontmetrics::FontMetricsStore;

    fn measure_label(fonts: Res<FontMetricsStore>) {
        if let Some(metrics) = fonts.0.get("arcade") {
            let size = metrics.measure_text("Hello!", 32.0, 1.0);
            // ... use size.x / size.y ...
        }
    }

    fn queue_label(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    use aberredengine::prelude::*;

    asset_cmds.write(RenderAssetCmd::RasterizeText {
        key: "title_label".to_string(),
        font_key: "arcade".to_string(),
        text: "PRESS START".to_string(),
        font_size: 32.0,
        spacing: 1.0,
        color: Color::new(255, 255, 255, 255),
    });
    } // GLUE
}

// Audio (sounds and music)
mod audio_sounds_and_music {
    use aberredengine::prelude::*; // GLUE

    fn queue_audio(mut audio: MessageWriter<AudioCmd>) { // GLUE
    // Load a sound effect
    audio.write(AudioCmd::LoadFx {
        id: "jump".to_string(),
        path: "assets/audio/jump.wav".to_string(),
    });

    // Load background music
    audio.write(AudioCmd::LoadMusic {
        id: "bgm".to_string(),
        path: "assets/audio/music.ogg".to_string(),
    });
    } // GLUE
}

// Audio replies
mod audio_replies {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;
    use aberredengine::core::protocol::audio::AudioMessage;

    fn on_audio_replies(mut replies: MessageReader<AudioMessage>, mut audio: MessageWriter<AudioCmd>) {
        for reply in replies.read() {
            match reply {
                AudioMessage::MusicLoaded { id } if id == "bgm" => {
                    audio.write(AudioCmd::PlayMusic { id: id.clone(), looped: true });
                }
                AudioMessage::FxLoadFailed { id, error }
                | AudioMessage::MusicLoadFailed { id, error } => {
                    log::error!("audio '{id}' failed to load: {error}");
                }
                AudioMessage::MusicFinished { id } => log::info!("music '{id}' ended"),
                _ => {}
            }
        }
    }

    EngineBuilder::new()
        .configure_schedule(|schedule| {
            schedule.add_systems(on_audio_replies.in_set(SimSet::ScriptUpdate));
        })
        // …
    } // GLUE
}

// Shaders
mod shaders {
    use aberredengine::prelude::*; // GLUE

    fn queue_shader(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    asset_cmds.write(RenderAssetCmd::Shader {
        key: "glow".to_string(),
        vs_path: None, // default vertex shader
        fs_path: Some("assets/shaders/glow.fs".to_string()),
    });
    } // GLUE

    fn queue_shader_from_memory(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    asset_cmds.write(RenderAssetCmd::ShaderFromMemory {
        key: "glitch".to_string(),
        vs_src: None, // default vertex shader
        fs_src: Some(include_str!("../assets/shaders/glitch.fs").to_string()),
    });
    } // GLUE
}

// Per-entity shaders
mod per_entity_shaders {
    use aberredengine::prelude::*;

    fn spawn_glowing(mut commands: Commands) {
        let mut shader = EntityShader::new("glow");
        shader.set_uniform("uIntensity", UniformValue::Float(0.8));
        commands.spawn((
            // MapPosition, Sprite, ZIndex, … as in Section 7
            shader,
        ));
    }

    fn pulse_glow(mut shaders: Query<&mut EntityShader>, time: Res<WorldTime>) {
        for mut shader in &mut shaders {
            let intensity = 0.5 + 0.5 * time.elapsed.sin();
            shader.set_uniform("uIntensity", UniformValue::Float(intensity));
        }
    }
}

// Post-process shaders
mod post_process_shaders {
    use aberredengine::prelude::*;

    fn enable_crt(mut post: ResMut<PostProcessShader>) {
        post.set_shader_chain(Some(vec!["crt".to_string(), "vignette".to_string()]));
        post.set_uniform("uCurvature", UniformValue::Float(0.1));
    }

    fn disable_post_processing(mut post: ResMut<PostProcessShader>) {
        post.set_shader_chain(None);
    }
}

// Animations
mod animations {
    use aberredengine::prelude::*; // GLUE

    fn register_animations(mut anim_store: ResMut<AnimationStore>) { // GLUE
    use aberredengine::prelude::*;

    anim_store.insert("player_idle", AnimationResource::new("player", 32.0, 4, 8.0));

    anim_store.insert(
        "player_run",
        AnimationResource::new("player", 32.0, 6, 12.0)
            .with_position(Vec2::new(0.0, 64.0)), // second row of the spritesheet
    );
    } // GLUE
}

// Tilemaps
mod tilemaps {
    use aberredengine::prelude::*; // GLUE

    fn spawn_tilemaps(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    // Minimal — tiles appear at world origin (default MapPosition inserted automatically)
    commands.spawn(TileMap::new("assets/tilemaps/level01"));

    // Positioned and scaled
    commands.spawn((
        TileMap::new("assets/tilemaps/level01"),
        MapPosition::new(100.0, 200.0),
        Scale::new(2.0, 2.0),
    ));
    } // GLUE

    fn move_tilemap(mut commands: Commands, tilemap_root: Entity, new_x: f32, new_y: f32) { // GLUE
    commands.entity(tilemap_root).insert(MapPosition::new(new_x, new_y));
    } // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;
    use aberredengine::core::systems::tilemap::TILES_GROUP;

    type NewUncollidedTiles = (Added<Group>, Without<BoxCollider>);

    fn add_tile_colliders(
        mut commands: Commands,
        tiles: Query<(Entity, &Group, &Sprite), NewUncollidedTiles>,
    ) {
        for (entity, group, sprite) in &tiles {
            if group.0 == TILES_GROUP {
                commands
                    .entity(entity)
                    .insert(BoxCollider::new(sprite.width, sprite.height));
            }
        }
    }

    EngineBuilder::new()
        .add_system(add_tile_colliders)
        // …
    } // GLUE
}

// Camera
mod camera {

    use aberredengine::prelude::*;

    fn setup_camera(mut camera: ResMut<Camera2DRes>, screen: Res<ScreenSize>) {
        // Look at (512, 256) from the screen center, zoomed in 2×
        camera.0 = Camera2D::screen_centered(&screen).with_zoom(2.0);
        camera.0.target = Vec2::new(512.0, 256.0);
    }
}

// Following an entity
mod following_an_entity {
    use aberredengine::prelude::*;

    fn follow_player(mut commands: Commands, mut follow: ResMut<CameraFollowConfig>) {
        commands.spawn((
            MapPosition::new(100.0, 200.0),
            // … sprite, physics, etc. …
            CameraTarget::new(1).with_zoom(2.0),
        ));

        follow.enabled = true;
        follow.mode = FollowMode::Deadzone { half_w: 32.0, half_h: 24.0 };
        follow.lerp_speed = 6.0;
        follow.bounds = Some(Rect::new(0.0, 0.0, 2048.0, 1024.0)); // the level's extent
    }
}

// Complete setup example
mod complete_setup_example {
    use aberredengine::prelude::*; // GLUE

    fn setup(mut assets: AssetLoader, mut anim_store: ResMut<AnimationStore>) -> Result {
        // Textures — queued, loaded asynchronously on the render thread
        assets.load_texture("player", "assets/textures/player.png")?;

        // Fonts — mipmap generation is handled internally by the render thread
        assets.load_font("arcade", "assets/fonts/arcade.ttf", 32)?;

        // Audio — loaded asynchronously on the audio thread
        assets.load_sound("jump", "assets/audio/jump.wav")?;
        assets.load_music("bgm", "assets/audio/music.ogg")?;

        // Shaders
        assets.load_shader("glow", None, Some("assets/shaders/glow.fs"))?;

        // Animations (AnimationStore is pre-inserted, logic-owned — just populate it)
        anim_store.insert("player_idle", AnimationResource::new("player", 32.0, 4, 8.0));
        Ok(())
    }
}
