// 4. Loading Assets
mod loading_assets {
    use aberredengine::core::protocol::render_assets::RenderAssetCmd;
    use aberredengine::core::resources::animationstore::{AnimationStore, AnimationResource};
    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::protocol::audio::AudioCmd;
    use std::sync::Arc;

    fn setup(
        mut anim_store: ResMut<AnimationStore>,
        mut asset_cmds: MessageWriter<RenderAssetCmd>,
        mut audio: MessageWriter<AudioCmd>,
    ) {
        // ... queue asset loads here (see subsections below) ...
    }
}

// Textures
mod textures {
    use aberredengine::core::protocol::render_assets::RenderAssetCmd; // GLUE
    use aberredengine::core::resources::texturefilter::TextureFilter; // GLUE

    fn queue_textures(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    use aberredengine::core::protocol::render_assets::RenderAssetCmd;
    use aberredengine::core::resources::texturefilter::TextureFilter;

    asset_cmds.write(RenderAssetCmd::Texture {
        id: "player".to_string(),
        path: "assets/textures/player.png".to_string(),
        filter: TextureFilter::Nearest,
    });
    asset_cmds.write(RenderAssetCmd::Texture {
        id: "background".to_string(),
        path: "assets/textures/background.png".to_string(),
        filter: TextureFilter::Nearest,
    });
    } // GLUE

    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::components::mapposition::MapPosition;
    use aberredengine::core::components::sprite::Sprite;
    use aberredengine::core::components::zindex::ZIndex;
    use aberredengine::core::math::Vec2;
    use aberredengine::core::resources::texturedims::TextureDimsStore;
    use aberredengine::core::resources::worldsignals::WorldSignals;
    use std::sync::Arc;

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
            Sprite {
                tex_key: Arc::from("player"),
                width: width as f32,
                height: height as f32,
                offset: Vec2::ZERO,
                origin: Vec2::new(width as f32 * 0.5, height as f32 * 0.5),
                flip_h: false,
                flip_v: false,
            },
            ZIndex(1.0),
        ));
        world_signals.set_flag("player_spawned");
    }

    fn queue_from_memory(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    asset_cmds.write(RenderAssetCmd::TextureFromMemory {
        id: "intro_logo".to_string(),
        ext: ".png".to_string(), // leading dot, matches raylib's file-type hint
        bytes: include_bytes!("../assets/textures/intro_logo.png").to_vec(),
        filter: TextureFilter::Nearest,
    });
    } // GLUE
}

// Fonts
mod fonts {
    use aberredengine::bevy_ecs::prelude::*; // GLUE
    use aberredengine::core::protocol::render_assets::RenderAssetCmd; // GLUE

    fn queue_font(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    asset_cmds.write(RenderAssetCmd::Font {
        id: "arcade".to_string(),
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
    use aberredengine::core::math::Color;

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
    use aberredengine::bevy_ecs::prelude::*; // GLUE
    use aberredengine::core::protocol::audio::AudioCmd; // GLUE

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
    use aberredengine::engine_app::EngineBuilder; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::protocol::audio::{AudioCmd, AudioMessage};
    use aberredengine::engine_app::SimSet;

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
    use aberredengine::bevy_ecs::prelude::*; // GLUE
    use aberredengine::core::protocol::render_assets::RenderAssetCmd; // GLUE

    fn queue_shader(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    asset_cmds.write(RenderAssetCmd::Shader {
        id: "glow".to_string(),
        vs_path: None, // default vertex shader
        fs_path: Some("assets/shaders/glow.fs".to_string()),
    });
    } // GLUE

    fn queue_shader_from_memory(mut asset_cmds: MessageWriter<RenderAssetCmd>) { // GLUE
    asset_cmds.write(RenderAssetCmd::ShaderFromMemory {
        id: "glitch".to_string(),
        vs_src: None, // default vertex shader
        fs_src: Some(include_str!("../assets/shaders/glitch.fs").to_string()),
    });
    } // GLUE
}

// Per-entity shaders
mod per_entity_shaders {
    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::components::entityshader::EntityShader;
    use aberredengine::core::resources::uniformvalue::UniformValue;
    use aberredengine::core::resources::worldtime::WorldTime;

    fn spawn_glowing(mut commands: Commands) {
        let mut shader = EntityShader::new("glow");
        shader.set_uniform("uIntensity", UniformValue::Float(0.8));
        commands.spawn((
            // MapPosition, Sprite, ZIndex, … as in Section 5
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
    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::resources::postprocessshader::PostProcessShader;
    use aberredengine::core::resources::uniformvalue::UniformValue;

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
    use aberredengine::bevy_ecs::prelude::*; // GLUE
    use aberredengine::core::resources::animationstore::{AnimationStore, AnimationResource}; // GLUE
    use std::sync::Arc; // GLUE

    fn register_animations(mut anim_store: ResMut<AnimationStore>) { // GLUE
    use aberredengine::core::math::Vec2;

    anim_store.animations.insert("player_idle".to_string(), AnimationResource {
        tex_key: Arc::from("player"),              // must match a TextureStore key
        position: Vec2::new(0.0, 0.0),            // base offset in spritesheet
        horizontal_displacement: 32.0,             // per-frame X step (= frame width)
        vertical_displacement: 0.0,                // non-zero enables row-wrapping
        frame_count: 4,                            // number of frames
        fps: 8.0,                                  // playback speed
        looped: true,                              // restart after last frame
    });

    anim_store.animations.insert("player_run".to_string(), AnimationResource {
        tex_key: Arc::from("player"),
        position: Vec2::new(0.0, 64.0),           // second row of spritesheet
        horizontal_displacement: 32.0,
        vertical_displacement: 0.0,
        frame_count: 6,
        fps: 12.0,
        looped: true,
    });
    } // GLUE
}

// Tilemaps
mod tilemaps {
    use aberredengine::bevy_ecs::prelude::*; // GLUE
    use aberredengine::core::components::mapposition::MapPosition; // GLUE
    use aberredengine::engine_app::EngineBuilder; // GLUE

    fn spawn_tilemaps(mut commands: Commands) { // GLUE
    use aberredengine::core::components::tilemap::TileMap;
    use aberredengine::core::components::mapposition::MapPosition;
    use aberredengine::core::components::scale::Scale;

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
    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::components::boxcollider::BoxCollider;
    use aberredengine::core::components::group::Group;
    use aberredengine::core::components::sprite::Sprite;
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
    use aberredengine::bevy_ecs::prelude::*; // GLUE

    use aberredengine::core::resources::camera2d::{Camera2D, Camera2DRes};
    use aberredengine::core::resources::screensize::ScreenSize;
    use aberredengine::core::math::Vec2;

    fn setup_camera(mut camera: ResMut<Camera2DRes>, screen: Res<ScreenSize>) {
        camera.0 = Camera2D {
            target: Vec2::new(0.0, 0.0),
            offset: Vec2::new(screen.w as f32 * 0.5, screen.h as f32 * 0.5),
            rotation: 0.0,
            zoom: 1.0,
        };
    }
}

// Following an entity
mod following_an_entity {
    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::components::cameratarget::CameraTarget;
    use aberredengine::core::components::mapposition::MapPosition;
    use aberredengine::core::math::Rect;
    use aberredengine::core::resources::camerafollowconfig::{CameraFollowConfig, FollowMode};

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
    use aberredengine::bevy_ecs::prelude::*; // GLUE
    use aberredengine::core::math::Vec2; // GLUE
    use aberredengine::core::protocol::audio::AudioCmd; // GLUE
    use aberredengine::core::protocol::render_assets::RenderAssetCmd; // GLUE
    use aberredengine::core::resources::animationstore::{AnimationStore, AnimationResource}; // GLUE
    use aberredengine::core::resources::texturefilter::TextureFilter; // GLUE
    use std::sync::Arc; // GLUE

    fn setup(
        mut anim_store: ResMut<AnimationStore>,
        mut asset_cmds: MessageWriter<RenderAssetCmd>,
        mut audio: MessageWriter<AudioCmd>,
    ) {
        // Textures — queued, loaded asynchronously on the render thread
        asset_cmds.write(RenderAssetCmd::Texture {
            id: "player".to_string(),
            path: "assets/textures/player.png".to_string(),
            filter: TextureFilter::Nearest,
        });

        // Fonts — mipmap generation is handled internally by the render thread
        asset_cmds.write(RenderAssetCmd::Font {
            id: "arcade".to_string(),
            path: "assets/fonts/arcade.ttf".to_string(),
            size: 32,
            skip_if_loaded: false,
        });

        // Audio — same message-queue pattern
        audio.write(AudioCmd::LoadFx { id: "jump".into(), path: "assets/audio/jump.wav".into() });
        audio.write(AudioCmd::LoadMusic { id: "bgm".into(), path: "assets/audio/music.ogg".into() });

        // Shaders
        asset_cmds.write(RenderAssetCmd::Shader {
            id: "glow".to_string(),
            vs_path: None,
            fs_path: Some("assets/shaders/glow.fs".to_string()),
        });

        // Animations (AnimationStore is pre-inserted, logic-owned — just populate it)
        anim_store.animations.insert("player_idle".into(), AnimationResource {
            tex_key: Arc::from("player"),
            position: Vec2::new(0.0, 0.0),
            horizontal_displacement: 32.0,
            vertical_displacement: 0.0,
            frame_count: 4,
            fps: 8.0,
            looped: true,
        });
    }
}
