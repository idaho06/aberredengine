//! Game-specific logic and scene management.
//!
//! This module contains the game's setup, update loop, and scene switching
//! logic. It demonstrates how to use the engine's components and systems
//! to build an Arkanoid-style game.
//!
//! # Key Functions
//!
//! - [`setup`] – loads resources (textures, fonts, audio) during `Setup` state
//! - [`enter_play`] – initializes world signals and observers when entering `Playing` state
//! - [`switch_scene`] – handles scene transitions (menu, level01, etc.)
//! - [`update`] – per-frame game logic for each scene
//!
//! # Scene Architecture
//!
//! Scenes are managed via the `"scene"` string in [`WorldSignals`](crate::resources::worldsignals::WorldSignals).
//! Setting the `"switch_scene"` flag triggers [`switch_scene`] to despawn non-persistent
//! entities and spawn the new scene's entities.
//!
//! # Phase Callbacks
//!
//! The `level01` scene uses [`Phase`](crate::components::phase::Phase) with callbacks:
//! - `init` → `get_started` → `playing` → `lose_life`/`level_cleared`/`game_over`
//!
//! These callbacks manage ball spawning, life tracking, and win/lose conditions.
//!
//! # Collision Callbacks
//!
//! [`CollisionRule`](crate::components::collision::CollisionRule) components define how
//! entities interact: ball-wall bounce, ball-player reflection, ball-brick destruction.

use std::ffi::CString;
use std::panic;

use bevy_ecs::event::Trigger;
use bevy_ecs::{prelude::*, world};
use raylib::ffi;
use raylib::ffi::TextureFilter::{TEXTURE_FILTER_ANISOTROPIC_8X, TEXTURE_FILTER_BILINEAR};
use raylib::prelude::*;
use rustc_hash::FxHashMap;
use serde_json::Map;
//use std::collections::HashMap;

// Import component/resource types from modules
use crate::components::animation::Animation;
use crate::components::animation::{AnimationController, CmpOp, Condition};
use crate::components::boxcollider::BoxCollider;
use crate::components::collision::{
    BoxSide, CollisionCallback, CollisionContext, CollisionRule, get_colliding_sides,
};
use crate::components::dynamictext::DynamicText;
use crate::components::gridlayout::GridLayout;
use crate::components::group::Group;
use crate::components::inputcontrolled::InputControlled;
use crate::components::inputcontrolled::MouseControlled;
use crate::components::mapposition::MapPosition;
use crate::components::menu::{Menu, MenuAction, MenuActions};
use crate::components::persistent::Persistent;
use crate::components::phase::{Phase, PhaseCallback, PhaseContext};
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signalbinding::SignalBinding;
use crate::components::signals::{self, Signals};
use crate::components::sprite;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::components::timer::Timer;
use crate::components::tween::{Easing, LoopMode, TweenPosition, TweenRotation, TweenScale};
use crate::components::zindex::ZIndex;
use crate::events::audio::AudioCmd;
use crate::events::timer::TimerEvent;
use crate::resources::animationstore::AnimationResource;
use crate::resources::animationstore::AnimationStore;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::fontstore::FontStore;
use crate::resources::gamestate::{GameStates, NextGameState};
use crate::resources::group::TrackedGroups;
use crate::resources::input::InputState;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::tilemapstore::{Tilemap, TilemapStore};
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use fastrand as rand;

/// Helper function to create a Texture2D from a text string, font, size, and color
pub fn load_texture_from_text(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    font: &Font,
    text: &str,
    font_size: f32,
    spacing: f32,
    color: Color,
) -> Option<Texture2D> {
    let c_text = CString::new(text).ok()?;
    let image = unsafe {
        let raw = ffi::ImageTextEx(**font, c_text.as_ptr(), font_size, spacing, color.into());
        Image::from_raw(raw)
    };
    let texture = rl.load_texture_from_image(thread, &image).ok()?;
    Some(texture)
}

/// Load a font with mipmaps and anisotropic filtering
fn load_font_with_mipmaps(rl: &mut RaylibHandle, th: &RaylibThread, path: &str, size: i32) -> Font {
    let mut font = rl
        .load_font_ex(th, path, size, None)
        .expect(&format!("Failed to load font '{}'", path));
    unsafe {
        ffi::GenTextureMipmaps(&mut font.texture);
        ffi::SetTextureFilter(font.texture, TEXTURE_FILTER_ANISOTROPIC_8X as i32);
    }
    font
}

/// Helper function to load a png and a json describing a tilemap. The json comes from Tilesetter 2.1.0
fn load_tilemap(rl: &mut RaylibHandle, thread: &RaylibThread, path: &str) -> (Texture2D, Tilemap) {
    let dirname = path.split('/').last().expect("Not a valid dir path.");
    let json_path = format!("{}/{}.txt", path, dirname);
    let png_path = format!("{}/{}.png", path, dirname);

    let texture = rl
        .load_texture(thread, &png_path)
        .expect("Failed to load tilemap texture");
    let json_string = std::fs::read_to_string(json_path).expect("Failed to load tilemap JSON");
    let tilemap: Tilemap =
        serde_json::from_str(&json_string).expect("Failed to parse tilemap JSON");
    (texture, tilemap)
}

fn spawn_tiles(
    commands: &mut Commands,
    tilemap_tex_key: impl Into<String>,
    tex_width: i32, // We assume square tiles, so only width is needed
    tilemap: &Tilemap,
) {
    let tilemap_tex_key: String = tilemap_tex_key.into();

    // texture size in pixels
    let tex_w = tex_width as f32;

    let tile_size = tilemap.tile_size as f32;

    // how many tiles per row in the texture
    let tiles_per_row = ((tex_w / tile_size).floor() as u32).max(1);

    let layer_count = tilemap.layers.len() as i32;
    // iterate layers and spawn tiles; ZIndex: if N layers, first is -N, last is -1
    for (layer_index, layer) in tilemap.layers.iter().enumerate() {
        let z = -(layer_count - (layer_index as i32));

        for pos in layer.positions.iter() {
            // world position = tile coords * tile_size
            let wx = pos.x as f32 * tile_size;
            let wy = pos.y as f32 * tile_size;

            // compute sprite offset in the tileset texture based on id (left-to-right, top-to-bottom)
            // id is assumed zero-based index
            let id = pos.id;
            let col = id % tiles_per_row;
            let row = id / tiles_per_row;

            let offset_x = col as f32 * tile_size;
            let offset_y = row as f32 * tile_size;

            // Sprite origin is the center of the sprite (in pixels)
            let origin = Vector2 {
                //x: tile_size * 0.5,
                //y: tile_size * 0.5,
                x: 0.0,
                y: 0.0,
            };

            commands.spawn((
                Group::new("tiles"),
                MapPosition::new(wx, wy),
                ZIndex(z),
                Sprite {
                    tex_key: tilemap_tex_key.clone().into(),
                    width: tile_size,
                    height: tile_size,
                    offset: Vector2 {
                        x: offset_x,
                        y: offset_y,
                    },
                    origin,
                    flip_h: false,
                    flip_v: false,
                },
            ));
        }
    }
}

// This function is meant to load all resources
pub fn setup(
    mut commands: Commands,
    mut next_state: ResMut<NextGameState>,
    mut rl: NonSendMut<raylib::RaylibHandle>,
    th: NonSend<raylib::RaylibThread>,
    mut fonts: NonSendMut<FontStore>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    // This function sets up the game world, loading resources

    let camera = Camera2D {
        target: Vector2 {
            x: rl.get_screen_width() as f32 * 0.5,
            y: rl.get_screen_height() as f32 * 0.5, //x: 0.0,
                                                    //y: 0.0,
        },
        offset: Vector2 {
            x: rl.get_screen_width() as f32 * 0.5,
            y: rl.get_screen_height() as f32 * 0.5,
        },
        rotation: 0.0,
        zoom: 1.0,
    }; // This camera matches the map coordinates to screen coordinates
    // (0,0) at top-left, (screen_width, screen_height) at bottom-right
    commands.insert_resource(Camera2DRes(camera));

    // Load fonts
    let font = load_font_with_mipmaps(&mut rl, &th, "./assets/fonts/Extra_Thick.ttf", 136);
    fonts.add("extra_thick", font);

    // Load textures
    let snowflake_tex = rl
        .load_texture(&th, "./assets/textures/snowflake01.png")
        .expect("load assets/textures/snowflake01.png");
    /* let title_tex = rl
        .load_texture(&th, "./assets/textures/title.png")
        .expect("load assets/title.png");

    let background_tex = rl
        .load_texture(&th, "./assets/textures/background01.png")
        .expect("load assets/background01.png");

    let cursor_tex = rl
        .load_texture(&th, "./assets/textures/cursor.png")
        .expect("load assets/cursor.png");
    let vaus_tex = rl
        .load_texture(&th, "./assets/textures/vaus.png")
        .expect("load assets/vaus.png");
    let ball_tex = rl
        .load_texture(&th, "./assets/textures/ball_12.png")
        .expect("load assets/ball_12.png");
    let brick_red_tex = rl
        .load_texture(&th, "./assets/textures/brick_red.png")
        .expect("load assets/brick_red.png");
    let brick_green_tex = rl
        .load_texture(&th, "./assets/textures/brick_green.png")
        .expect("load assets/brick_green.png");
    let brick_blue_tex = rl
        .load_texture(&th, "./assets/textures/brick_blue.png")
        .expect("load assets/brick_blue.png");
    let brick_yellow_tex = rl
        .load_texture(&th, "./assets/textures/brick_yellow.png")
        .expect("load assets/brick_yellow.png");
    let brick_purple_tex = rl
        .load_texture(&th, "./assets/textures/brick_purple.png")
        .expect("load assets/brick_purple.png");
    let brick_silver_tex = rl
        .load_texture(&th, "./assets/textures/brick_silver.png")
        .expect("load assets/brick_silver.png"); */

    /* let dummy_tex = rl
        .load_texture(&th, "./assets/textures/player.png")
        .expect("load assets/player.png");

    let enemy_tex = rl
        .load_texture(&th, "./assets/textures/enemy.png")
        .expect("load assets/enemy.png");

    let player_sheet_tex = rl
        .load_texture(&th, "./assets/textures/WarriorMan-Sheet.png")
        .expect("load assets/WarriorMan-Sheet.png"); */

    // Load tilemap textures and data
    //let (tilemap_tex, tilemap) = load_tilemap(&mut rl, &th, "./assets/tilemaps/maptest04");
    let mut tilemaps_store = TilemapStore::new();
    //tilemaps_store.insert("tilemap", tilemap);
    commands.insert_resource(tilemaps_store);

    // Create textures from texts, fonts, and sizes for static texts
    /* let arcade_font = fonts
        .get("arcade")
        .expect("Font 'arcade' not found in FontStore");
    let billboard_tex = load_texture_from_text(
        &mut rl,
        &th,
        arcade_font,
        "Static Billboard!",
        32.0,
        1.0,
        Color::RED,
    )
    .expect("Failed to create texture from text"); */

    // Insert TextureStore resource
    let mut tex_store = TextureStore::new();
    /* tex_store.insert("title", title_tex);
    tex_store.insert("background", background_tex);
    tex_store.insert("cursor", cursor_tex);
    tex_store.insert("vaus", vaus_tex);
    tex_store.insert("ball", ball_tex);
    tex_store.insert("brick_red", brick_red_tex);
    tex_store.insert("brick_green", brick_green_tex);
    tex_store.insert("brick_blue", brick_blue_tex);
    tex_store.insert("brick_yellow", brick_yellow_tex);
    tex_store.insert("brick_purple", brick_purple_tex);
    tex_store.insert("brick_silver", brick_silver_tex); */
    /* tex_store.insert("player-sheet", player_sheet_tex);
    tex_store.insert("dummy", dummy_tex);
    tex_store.insert("enemy", enemy_tex);
    tex_store.insert("tilemap", tilemap_tex);
    tex_store.insert("billboard", billboard_tex); */
    tex_store.insert("snowflake", snowflake_tex);
    commands.insert_resource(tex_store);

    // Animations
    let mut anim_store = AnimationStore {
        animations: FxHashMap::default(),
    };
    /* anim_store.animations.insert(
        "player_tired".into(),
        AnimationResource {
            tex_key: "player-sheet".into(),
            position: Vector2 { x: 0.0, y: 16.0 },
            displacement: 80.0, // width of each frame in the spritesheet
            frame_count: 8,
            fps: 6.0, // speed of the animation
            looped: true,
        },
    );
    anim_store.animations.insert(
        "player_stand".into(),
        AnimationResource {
            tex_key: "player-sheet".into(),
            position: Vector2 { x: 0.0, y: 80.0 },
            displacement: 80.0, // width of each frame in the spritesheet
            frame_count: 8,
            fps: 6.0, // speed of the animation
            looped: true,
        },
    );
    anim_store.animations.insert(
        "player_walk".into(),
        AnimationResource {
            tex_key: "player-sheet".into(),
            position: Vector2 { x: 0.0, y: 144.0 },
            displacement: 80.0, // width of each frame in the spritesheet
            frame_count: 8,
            fps: 6.0, // speed of the animation
            looped: true,
        },
    );
    anim_store.animations.insert(
        "player_run".into(),
        AnimationResource {
            tex_key: "player-sheet".into(),
            position: Vector2 { x: 0.0, y: 208.0 },
            displacement: 80.0, // width of each frame in the spritesheet
            frame_count: 8,
            fps: 6.0, // speed of the animation
            looped: true,
        },
    );
    anim_store.animations.insert(
        "player_jump".into(),
        AnimationResource {
            tex_key: "player-sheet".into(),
            position: Vector2 { x: 0.0, y: 272.0 },
            displacement: 80.0, // width of each frame in the spritesheet
            frame_count: 8 + 3,
            fps: 12.0, // speed of the animation
            looped: true,
        },
    ); */
    commands.insert_resource(anim_store);

    // Send messages to load musics and sound effects via ECS Messages<AudioCmd>
    /* audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "music1".into(),
        path: "./assets/audio/chiptun1.mod".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "music2".into(),
        path: "./assets/audio/mini1111.xm".into(),
    }); */
    /* audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "boss_fight".into(),
        path: "./assets/audio/boss_fight.xm".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "journey_begins".into(),
        path: "./assets/audio/journey_begins.xm".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "player_ready".into(),
        path: "./assets/audio/player_ready.xm".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "success".into(),
        path: "./assets/audio/success.xm".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "menu".into(),
        path: "./assets/audio/woffy_-_arkanoid_cover.xm".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadFx {
        id: "ding".into(),
        path: "./assets/audio/ding.wav".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadFx {
        id: "ping".into(),
        path: "./assets/audio/ping.wav".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadFx {
        id: "option".into(),
        path: "./assets/audio/option.wav".into(),
    }); */

    audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "xmas_song".into(),
        path: "./assets/audio/axel_-_christmas_delirium.mod".into(),
    });

    // Don't block; the audio thread will emit load messages which are polled by systems.

    // Change GameState to Playing
    next_state.set(GameStates::Playing);
    eprintln!("Game setup() done, next state set to Playing");
}

pub fn quit_game(
    //mut commands: Commands,
    //mut rl: NonSendMut<raylib::RaylibHandle>,
    mut world_signals: ResMut<WorldSignals>,
) {
    eprintln!("Quitting demo...");

    // Perform any necessary cleanup here

    // Optionally, set a signal to indicate the game should exit
    world_signals.set_flag("quit_demo");
}

// Create initial state of the game and observers
pub fn enter_play(
    mut commands: Commands,
    //mut next_state: ResMut<NextGameState>,
    //mut rl: NonSendMut<raylib::RaylibHandle>,
    //th: NonSend<raylib::RaylibThread>,
    mut audio_cmd_writer: bevy_ecs::prelude::MessageWriter<AudioCmd>,
    tex_store: Res<TextureStore>,
    tilemaps_store: Res<TilemapStore>, // TODO: Make it optional?
    mut worldsignals: ResMut<WorldSignals>,
    mut tracked_groups: ResMut<TrackedGroups>,
    systems_store: Res<SystemsStore>,
) {
    // Get Texture sizes
    /* let dummy_tex = tex_store.get("dummy").expect("dummy texture not found");
    let dummy_tex_width = dummy_tex.width;
    let dummy_tex_height = dummy_tex.height;

    let enemy_tex = tex_store.get("enemy").expect("enemy texture not found");
    let enemy_tex_width = enemy_tex.width;
    let enemy_tex_height = enemy_tex.height;

    let tilemap_tex = tex_store.get("tilemap").expect("tilemap texture not found");
    let tilemap_tex_width = tilemap_tex.width;
    let tilemap = tilemaps_store
        .get("tilemap")
        .expect("tilemap info not found"); */

    // Dummy player
    /* commands.spawn((
        Group::new("dummy"),
        MapPosition::new(40.0, 40.0),
        ZIndex(0),
        Sprite {
            tex_key: "dummy".into(),
            width: dummy_tex_width as f32,
            height: dummy_tex_height as f32,
            offset: Vector2::zero(),
            origin: Vector2 {
                x: dummy_tex_width as f32 * 0.5,
                y: dummy_tex_height as f32,
            }, // origin at the feet of the dummy sprite
            flip_h: false,
            flip_v: false,
        },
        BoxCollider {
            size: Vector2 {
                x: dummy_tex_width as f32 * 0.5,
                y: dummy_tex_height as f32 * 0.5,
            },
            offset: Vector2 {
                x: dummy_tex_width as f32 * 0.25,
                y: dummy_tex_height as f32 * 0.25,
            },
            // Match collider pivot to sprite's origin (feet) to align positions
            origin: Vector2 {
                x: dummy_tex_width as f32 * 0.5,
                y: dummy_tex_height as f32,
            },
        },
        RigidBody::default(),
    )); */

    // Player animation flipped
    /* commands.spawn((
        Group::new("player-animation"),
        MapPosition::new(400.0, 225.0),
        ZIndex(1),
        Sprite {
            tex_key: "player-sheet".into(),
            width: 80.0, // width of the sprite frame in the spritesheet
            height: 32.0,
            offset: Vector2 { x: 0.0, y: 16.0 }, // offset to match the sprite frame in the spritesheet
            origin: Vector2 { x: 40.0, y: 32.0 },
            flip_h: false,
            flip_v: true,
        },
        Animation {
            animation_key: "player_walk".into(),
            frame_index: 0,
            elapsed_time: 0.0,
        },
    )); */
    // Player animation controlled
    /* commands.spawn((
        Group::new("player-animation"),
        Signals::default(),
        MapPosition::new(400.0, 190.0),
        ZIndex(1),
        Sprite {
            tex_key: "player-sheet".into(),
            width: 80.0, // width of the sprite frame in the spritesheet
            height: 32.0,
            offset: Vector2 { x: 0.0, y: 16.0 }, // offset to match the sprite frame in the spritesheet
            origin: Vector2 { x: 40.0, y: 32.0 },
            flip_h: false,
            flip_v: false,
        },
        Animation {
            animation_key: "player_stand".into(),
            frame_index: 0,
            elapsed_time: 0.0,
        },
        AnimationController::new("player_stand")
            // Idle
            .with_rule(
                Condition::LacksFlag {
                    key: "moving".into(),
                },
                "player_stand",
            )
            // Walking
            .with_rule(
                Condition::ScalarRange {
                    key: "speed".into(),
                    min: 5.0,
                    max: 50.0,
                    inclusive: true,
                },
                "player_walk",
            )
            // Running
            .with_rule(
                Condition::ScalarCmp {
                    key: "speed".into(),
                    op: CmpOp::Gt,
                    value: 50.0,
                },
                "player_run",
            ),
        InputControlled::new(
            Vector2 { x: 0.0, y: -32.0 }, // up
            Vector2 { x: 0.0, y: 32.0 },  // down
            Vector2 { x: -32.0, y: 0.0 }, // left
            Vector2 { x: 64.0, y: 0.0 },  // right
        ),
        RigidBody::default(),
    )); */

    // Enemies
    /* let mut rng = rand::thread_rng();
    for i in 0..16 {
        // Random velocity components in a small range
        let vx = rng.gen_range(-40.0f32..40.0f32);
        let vy = rng.gen_range(-20.0f32..20.0f32);

        let flip_h = vx < 0.0;

        commands.spawn((
            Group::new("enemy"),
            MapPosition::new(50.0 + (i as f32 * 16.0), 164.0 + (i as f32 * 16.0)),
            ZIndex(i % 5),
            Sprite {
                tex_key: "enemy".into(),
                width: enemy_tex_width as f32,
                height: enemy_tex_height as f32,
                offset: Vector2::zero(),
                origin: Vector2::zero(),
                flip_h: flip_h,
                flip_v: false,
            },
            {
                let mut rb = RigidBody::new();
                rb.set_velocity(Vector2 { x: vx, y: vy });
                rb
            },
            BoxCollider {
                size: Vector2 {
                    x: enemy_tex_width as f32,
                    y: enemy_tex_height as f32,
                },
                offset: Vector2::zero(),
                origin: Vector2::zero(),
            },
        ));
    } */

    // Create map tiles as spawns of MapPosition, Zindex, and Sprite
    // spawn_tiles(&mut commands, "tilemap", tilemap_tex_width, tilemap);

    // play main_theme looped via ECS messages
    /* audio_cmd_writer.write(AudioCmd::PlayMusic {
        id: "main_theme".into(),
        looped: true,
    }); */

    // Create a couple of texts using DynamicText component
    /* commands.spawn((
        Group::new("texts"),
        MapPosition::new(200.0, 90.0),
        ZIndex(10),
        DynamicText::new("Hello, World!", "arcade", 12.0, Color::WHITE),
    ));

    commands.spawn((
        Group::new("texts"),
        MapPosition::new(100.0, 50.0),
        ZIndex(10),
        DynamicText::new("Aberred Engine!!", "future", 32.0, Color::YELLOW),
        {
            let mut rb = RigidBody::new();
            rb.set_velocity(Vector2 { x: 10.0, y: 10.0 });
            rb
        },
    )); */

    /* let billboard_tex = tex_store
        .get("billboard")
        .expect("billboard texture not found");
    commands.spawn((
        Group::new("texts"),
        MapPosition::new(100.0, 50.0),
        ZIndex(10),
        Sprite {
            tex_key: "billboard".into(),
            width: billboard_tex.width as f32,
            height: billboard_tex.height as f32,
            offset: Vector2::zero(),
            origin: Vector2::zero(),
            flip_h: false,
            flip_v: false,
        },
        {
            let mut rb = RigidBody::new();
            rb.set_velocity(Vector2 { x: -10.0, y: -10.0 });
            rb
        },
    )); */

    /* commands.spawn((
        Group::new("texts"),
        ScreenPosition::new(10.0, 20.0),
        DynamicText::new("Screen Text Example", "future", 24.0, Color::GREEN),
    )); */

    // Setup initial status of the game in the WorldSignals resource
    // integer for current score
    // integer for high score
    // integer for remaining lives
    // integer for current level
    // string for scene ("menu", "playing", "gameover", etc.)
    /* worldsignals.set_integer("score", 0);
    worldsignals.set_integer("high_score", 0);
    worldsignals.set_integer("lives", 3);
    worldsignals.set_integer("level", 1); */

    // Observer for TimerEvent
    commands.add_observer(|trigger: On<TimerEvent>, mut commands: Commands| {
        match trigger.signal.as_str() {
            /* "stop_title" => {
                //commands.entity(trigger.entity).remove::<RigidBody>();
                commands.entity(trigger.entity).insert(RigidBody {
                    velocity: Vector2 { x: 0.0, y: 0.0 },
                });
                commands.entity(trigger.entity).remove::<Timer>();
                commands.entity(trigger.entity).insert(MapPosition {
                    pos: Vector2 { x: 0.0, y: -220.0 },
                });
            } */
            _ => (),
        }
    });

    // Observer to remove the "sticky" flag from the entity (meant to be used by the "player" or "ball" entity)
    commands.add_observer(
        |trigger: On<TimerEvent>, mut signals: Query<&mut Signals>, mut commands: Commands| {
            let entity = trigger.entity;
            let signal = &trigger.signal;

            /* if signal == "remove_sticky" {
                if let Ok(mut sigs) = signals.get_mut(entity) {
                    sigs.clear_flag("sticky");
                }
                commands.entity(entity).remove::<Timer>();
            } */
        },
    );

    // Observer to remove StuckTo component and restore velocity
    /* commands.add_observer(
        |trigger: On<TimerEvent>,
         stuck_to_query: Query<&StuckTo>,
         mut rigid_body: Query<&mut RigidBody>,
         mut commands: Commands| {
            let entity = trigger.entity;
            let signal = &trigger.signal;

            if signal == "remove_stuck_to" {
                // Restore stored velocity from StuckTo component before removing it
                if let Ok(stuck_to) = stuck_to_query.get(entity) {
                    if let Some(stored_velocity) = stuck_to.stored_velocity {
                        if let Ok(mut rb) = rigid_body.get_mut(entity) {
                            rb.velocity = stored_velocity;
                        }
                    }
                }
                commands.entity(entity).remove::<StuckTo>();
                commands.entity(entity).remove::<Timer>();
            }
        },
    ); */

    let text = r"This is a test of the scrolling text system.
It should display this text character by character, moving from right to left across the screen. ";

    let one_line_text = text.replace('\n', " ");

    worldsignals.set_string("scrolling_text", one_line_text); // The text to show in the scrolling text
    worldsignals.set_integer("char_pos", 0); // The next character to spawn in the scrolling text
    worldsignals.set_flag("spawn_char");
    // Finally, run the switch_scene system to spawn initial scene entities
    worldsignals.set_string("scene", "intro");
    commands.run_system(
        systems_store
            .get("switch_scene")
            .expect("switch_scene system not found")
            .clone(),
    );
}

/// on_update callback for "bouncing" phase of the letters
fn bouncing_on_update(
    entity: Entity,
    time: f32,
    _previous: Option<String>,
    ctx: &mut PhaseContext,
) -> Option<String> {
    /* // Skip the first frame to allow the letters group count to update
    if time < 0.1 {
        return None;
    } */
    // change y position of the entity based on a sine wave function over time
    let amplitude = 20.0;
    let frequency = 1.0; // oscillations per second
    let new_y = 425.0 + amplitude * (frequency * time * std::f32::consts::TAU).sin();
    let mut pos = ctx
        .positions
        .get_mut(entity)
        .expect("Map position not found!");
    pos.pos.y = new_y;
    None
}

// snow emitter
fn snow_emitter_on_update(
    entity: Entity,
    time: f32,
    _previous: Option<String>,
    ctx: &mut PhaseContext,
) -> Option<String> {
    let mut signals = ctx.signals.get_mut(entity).unwrap();
    let last_time = signals.get_scalar("time_last_emission").unwrap_or(0.0);
    let emit_interval = signals.get_scalar("emission_interval").unwrap_or(0.1);
    let num_snowflakes = signals.get_integer("particles_per_emission").unwrap_or(100);
    let current_time = ctx.world_time.elapsed;
    if current_time - last_time >= emit_interval {
        // get size of the collider from the entity
        let collider = ctx.box_colliders.get(entity).unwrap();
        let position = ctx.positions.get(entity).unwrap();
        let collider_rectangle = collider.as_rectangle(position.pos);
        // emit snowflakes
        for _ in 0..num_snowflakes {
            // spawn snowflake entity from inside the collider rectangle
            let spawn_x = rand::f32() * collider_rectangle.width + collider_rectangle.x;
            let spawn_y = rand::f32() * collider_rectangle.height + collider_rectangle.y;

            let size = 4.0 + rand::f32() * 32.0;
            // sprite is 256x256, calculate Scale to match size
            let scale = size / 256.0;
            let fall_speed = 50.0 + rand::f32() * 100.0;

            // Create signals for jiggle parameters
            let mut snowflake_signals = Signals::default();
            // Random jiggle frequency (oscillations per second)
            let jiggle_frequency = 0.05 + rand::f32() * 0.8;
            // Random jiggle amplitude (pixels)
            let jiggle_amplitude = 10.0 + rand::f32() * 30.0;
            // Random phase offset so snowflakes don't all jiggle in sync
            let jiggle_phase_offset = rand::f32() * std::f32::consts::TAU;
            snowflake_signals.set_scalar("jiggle_frequency", jiggle_frequency);
            snowflake_signals.set_scalar("jiggle_amplitude", jiggle_amplitude);
            snowflake_signals.set_scalar("jiggle_phase_offset", jiggle_phase_offset);
            snowflake_signals.set_scalar("base_x", spawn_x);

            ctx.commands.spawn((
                Group::new("snowflake"),
                MapPosition::new(spawn_x, spawn_y),
                ZIndex(15),
                Sprite {
                    tex_key: "snowflake".into(),
                    width: 256.0,
                    height: 256.0,
                    offset: Vector2::zero(),
                    origin: Vector2::zero(),
                    flip_h: false,
                    flip_v: false,
                },
                Scale::new(scale, scale),
                RigidBody {
                    velocity: Vector2 {
                        x: 0.0,
                        y: fall_speed,
                    },
                },
                BoxCollider::new(size, size),
                snowflake_signals,
                Phase::new("falling").on_update("falling", snowflake_falling_on_update),
            ));
        }
        // update last emission time
        signals.set_scalar("time_last_emission", current_time);
    }

    None
}

// Phase callback for snowflakes falling - adds horizontal jiggle effect
fn snowflake_falling_on_update(
    entity: Entity,
    time: f32,
    _previous: Option<String>,
    ctx: &mut PhaseContext,
) -> Option<String> {
    // Get the snowflake's signals for jiggle parameters
    let Ok(signals) = ctx.signals.get(entity) else {
        return None;
    };

    let jiggle_frequency = signals.get_scalar("jiggle_frequency").unwrap_or(2.0);
    let jiggle_amplitude = signals.get_scalar("jiggle_amplitude").unwrap_or(20.0);
    let jiggle_phase_offset = signals.get_scalar("jiggle_phase_offset").unwrap_or(0.0);
    let base_x = signals.get_scalar("base_x").unwrap_or(0.0);

    // Calculate horizontal offset using sine wave
    let jiggle_offset = jiggle_amplitude
        * (jiggle_frequency * time * std::f32::consts::TAU + jiggle_phase_offset).sin();

    // Update the x position with jiggle
    if let Ok(mut pos) = ctx.positions.get_mut(entity) {
        pos.pos.x = base_x + jiggle_offset;
    }

    None
}

// Collision rule for despawning snowflakes when they collide with the bottom wall
fn snowflake_snow_destroyer_collision_callback(
    snowflake: Entity,
    wall: Entity,
    ctx: &mut CollisionContext,
) {
    //eprintln!("snowflake_snow_destroyer_collision_callback: Snowflake collided with snow destroyer!");
    // despawn the snowflake
    ctx.commands.entity(snowflake).despawn();
}

// collision rules for scrolling text
fn letter_wall_collision_callback(letter: Entity, wall: Entity, ctx: &mut CollisionContext) {
    //eprintln!("letter_wall_collision_callback: Letter collided with wall!");
    // depending of the possition of the letter, either despawn it or set the flag to spawn the next one
    let letter_pos = ctx
        .positions
        .get(letter)
        .cloned()
        .unwrap_or(MapPosition::new(0.0, 0.0));
    let letter_rectangle = ctx
        .box_colliders
        .get(letter)
        .unwrap()
        .as_rectangle(letter_pos.pos);
    // get position and rectangle of the wall
    let wall_pos = ctx
        .positions
        .get(wall)
        .cloned()
        .unwrap_or(MapPosition::new(0.0, 0.0));
    let wall_rectangle = ctx
        .box_colliders
        .get(wall)
        .unwrap()
        .as_rectangle(wall_pos.pos);
    // check if letter is fully inside the wall rectangle
    let Some((colliding_sides_letter, _colliding_sides_wall)) =
        get_colliding_sides(&letter_rectangle, &wall_rectangle)
    else {
        return;
    };
    if colliding_sides_letter.len() != 4 {
        return;
    }

    if letter_pos.pos.x < 100.0 {
        // despawn the letter
        ctx.commands.entity(letter).despawn();
    } else if letter_pos.pos.x > 760.0 {
        // check if the letter has the "last" flag in Signals component
        let mut letter_signals = ctx.signals.get_mut(letter).unwrap();
        if letter_signals.has_flag("last") {
            // set the flag to spawn the next character
            ctx.world_signals.set_flag("spawn_char");
            // adjust letter_spawn_x to letter's current position + letter width
            let letter_width = letter_rectangle.width;
            ctx.world_signals
                .set_scalar("letter_spawn_x", letter_pos.pos.x + letter_width + 1.0);
            // remove the "last" flag from the letter's Signals component
            letter_signals.clear_flag("last");
        }
    }
}

pub fn update(
    time: Res<WorldTime>,
    // mut _query_rb: Query<(&mut MapPosition, &mut RigidBody, &BoxCollider), With<Group>>,
    // mut query_enemies: Query<(&mut Sprite, &RigidBody), With<Group>>,
    //mut query_player: Query<(&mut Sprite, &RigidBody), With<Group>>,
    input: Res<InputState>,
    mut commands: Commands,
    systems_store: Res<SystemsStore>,
    mut world_signals: ResMut<WorldSignals>,
    mut next_game_state: ResMut<NextGameState>,
    font_store: NonSend<FontStore>,
) {
    let delta_sec = time.delta;

    let scene = world_signals
        .get_string("scene")
        .cloned()
        .unwrap_or("menu".to_string());

    match scene.as_str() {
        "intro" => {
            // check for flag "spawn_char" in world_signals
            // if set, spawn next character in scrolling text
            // update "char_pos" integer in world_signals
            // remove "spawn_char" flag when done
            // spawn a timer to set "spawn_char" flag again after a short delay
            if world_signals.has_flag("spawn_char") {
                let Some(scrolling_text) = world_signals.get_string("scrolling_text") else {
                    world_signals.clear_flag("spawn_char");
                    return;
                };
                let scrolling_text_lenght = scrolling_text.chars().count();
                let Some(char_pos) = world_signals.get_integer("char_pos") else {
                    world_signals.clear_flag("spawn_char");
                    return;
                };
                let char_to_spawn = scrolling_text
                    .chars()
                    .nth(char_pos as usize % scrolling_text_lenght)
                    .unwrap_or('\0');
                // spwan a DynamicText entity for the character at the middle right of the screen
                if char_to_spawn != '\0' {
                    let char_c_string = std::ffi::CString::new(char_to_spawn.to_string())
                        .expect("Failed to create CString from char");
                    let font_size = 136.0;
                    let font = font_store
                        .get("extra_thick")
                        .expect("Font 'extra_thick' not found");
                    let text_width = unsafe {
                        ffi::MeasureTextEx(
                            **font,
                            char_c_string.as_ptr() as *const i8,
                            font_size,
                            0.0,
                        )
                    };
                    let collision_width = text_width.x;
                    let collision_height = text_width.y;
                    let mut signals = Signals::default();
                    signals.set_flag("last");
                    let letter_spawn_x = world_signals
                        .get_scalar("letter_spawn_x")
                        .unwrap_or(960.0 + 201.0);
                    commands.spawn((
                        Group::new("letters_scroller"),
                        MapPosition::new(letter_spawn_x, 425.0),
                        ZIndex(20),
                        DynamicText::new(
                            char_to_spawn.to_string(),
                            "extra_thick",
                            font_size,
                            Color::WHITE,
                        ),
                        BoxCollider::new(collision_width, collision_height),
                        RigidBody {
                            velocity: Vector2 { x: -400.0, y: 0.0 },
                        },
                        signals,
                        Phase::new("bouncing").on_update("bouncing", bouncing_on_update),
                    ));
                    // update char_pos
                    world_signals.set_integer("char_pos", char_pos + 1);
                    // spawn timer to set "spawn_char" flag again after 0.1 seconds
                    // commands.spawn((Timer::new(0.1, "next_spawn_char"),));
                }
                // clear the flag
                world_signals.clear_flag("spawn_char");
                // done spawning all characters
            }
        }
        _ => {
            // Default or unknown scene updates
        } /* "menu" => {
              // Menu specific updates
              if input.action_back.just_pressed {
                  next_game_state.set(GameStates::Quitting);
              }
              let switch_scene_system = systems_store
                  .get("switch_scene")
                  .expect("switch_scene system not found")
                  .clone();

              // Check if a phase callback requested a scene switch
              //eprintln!("Checking world signals for scene switch...");
              if world_signals.has_flag("switch_scene") {
                  eprintln!("Scene switch requested in world signals.");
                  world_signals.clear_flag("switch_scene");
                  commands.run_system(switch_scene_system);
                  return;
              }
          }
          "level01" => {
              // Level 1 specific updates
              let switch_scene_system = systems_store
                  .get("switch_scene")
                  .expect("switch_scene system not found")
                  .clone();

              // Check if a phase callback requested a scene switch
              //eprintln!("Checking world signals for scene switch...");
              if world_signals.has_flag("switch_scene") {
                  eprintln!("Scene switch requested in world signals.");
                  world_signals.clear_flag("switch_scene");
                  commands.run_system(switch_scene_system);
                  return;
              }

              // If action_back is pressed, go back to menu
              if input.action_back.just_pressed {
                  world_signals.set_string("scene", "menu");
                  commands.run_system(switch_scene_system);
                  return;
              }

              // NOTE: Ball loss, life management, and level cleared are now handled
              // by the Phase system (see phase callbacks in switch_scene)
          }
          "level02" => {
              // Level 2 specific updates
          } */
    }
}

pub fn clean_all_entities(mut commands: Commands, query: Query<Entity, Without<Persistent>>) {
    for entity in query.iter() {
        //eprintln!("Despawning entity: {:?}", entity);
        commands.entity(entity).despawn();
    }
}

pub fn switch_scene(
    mut commands: Commands,
    mut audio_cmd_writer: bevy_ecs::prelude::MessageWriter<AudioCmd>,
    mut worldsignals: ResMut<WorldSignals>,
    systems_store: Res<SystemsStore>,
    mut tilemaps_store: ResMut<TilemapStore>,
    mut tex_store: ResMut<TextureStore>,
    entities_to_clean: Query<Entity, Without<Persistent>>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut rl: NonSendMut<raylib::RaylibHandle>,
    th: NonSend<raylib::RaylibThread>,
    world_time: Res<WorldTime>,
) {
    audio_cmd_writer.write(AudioCmd::StopAllMusic);
    // Race condition for cleaning entities and spawning new ones?
    /* commands.run_system(
        systems_store
            .get("clean_all_entities")
            .expect("clean_all_entities system not found")
            .clone(),
    ); */
    for entity in entities_to_clean.iter() {
        commands.entity(entity).log_components();
        //eprintln!("Despawning entity: {:?}", entity);
        commands.entity(entity).despawn();
    }

    tilemaps_store.clear();

    tracked_groups.clear();
    worldsignals.clear_group_counts();

    let scene = worldsignals
        .get_string("scene")
        .cloned()
        .unwrap_or_else(|| "intro".to_string());

    match scene.as_str() {
        "intro" => {
            // box colliders to control spawning of scrolling text characters
            // one on the left side of the screen to despawn characters
            // one on the right side of the screen to spawn next characters
            let screen_width = rl.get_screen_width() as f32;
            let screen_height = rl.get_screen_height() as f32;
            commands.spawn((
                Group::new("walls_scroller"),
                MapPosition::new(-200.0, -200.0),
                ZIndex(5),
                BoxCollider::new(200.0, screen_height + 400.0),
            ));
            commands.spawn((
                Group::new("walls_scroller"),
                MapPosition::new(screen_width, -200.0),
                ZIndex(5),
                BoxCollider::new(200.0, screen_height + 400.0),
            ));
            let mut signals = Signals::default();
            signals.set_scalar("time_last_emission", world_time.elapsed);
            signals.set_scalar("emission_interval", 0.5);
            signals.set_integer("particles_per_emission", 4);
            commands.spawn((
                Group::new("snow_emitter"),
                MapPosition::new(0.0, -100.0),
                BoxCollider::new(screen_width, 100.0),
                signals,
                Phase::new("snow_emission").on_update("snow_emission", snow_emitter_on_update),
            ));
            commands.spawn((
                Group::new("wall_snow_destroyer"),
                MapPosition::new(-200.0, screen_height + 100.0),
                BoxCollider::new(screen_width + 400.0, 200.0),
            ));
            commands.spawn((
                CollisionRule::new(
                    "snowflake",
                    "wall_snow_destroyer",
                    snowflake_snow_destroyer_collision_callback as CollisionCallback,
                ),
                Group::new("collision_rules"),
            ));

            worldsignals.set_scalar("letter_spawn_x", screen_width + 201.0);
            audio_cmd_writer.write(AudioCmd::PlayMusic {
                id: "xmas_song".into(),
                looped: true,
            });

            commands.spawn((
                CollisionRule::new(
                    "letters_scroller",
                    "walls_scroller",
                    letter_wall_collision_callback as CollisionCallback,
                ),
                Group::new("collision_rules"),
            ));
        }
        _ => {
            eprintln!("Unknown scene: {}", scene);
            panic!("Unknown scene");
        } /* "menu" => {
              let camera = Camera2D {
                  target: Vector2 { x: 0.0, y: 0.0 },
                  offset: Vector2 {
                      x: rl.get_screen_width() as f32 * 0.5,
                      y: rl.get_screen_height() as f32 * 0.5,
                  },
                  rotation: 0.0,
                  zoom: 1.0,
              };
              commands.insert_resource(Camera2DRes(camera));
              commands.spawn((
                  MapPosition::new(0.0, 0.0),
                  ZIndex(0),
                  Sprite {
                      tex_key: "background".into(),
                      width: 672.0,
                      height: 768.0,
                      origin: Vector2 { x: 336.0, y: 384.0 },
                      offset: Vector2 { x: 0.0, y: 0.0 },
                      flip_h: false,
                      flip_v: false,
                  },
                  Scale {
                      scale: Vector2 { x: 3.0, y: 3.0 },
                  },
              ));
              commands.spawn((
                  MapPosition::new(0.0, 384.0),
                  ZIndex(1),
                  Sprite {
                      tex_key: "title".into(),
                      width: 672.0,
                      height: 198.0,
                      origin: Vector2 { x: 336.0, y: 99.0 },
                      offset: Vector2 { x: 0.0, y: 0.0 },
                      flip_h: false,
                      flip_v: false,
                  },
                  Rotation { degrees: 0.0 },
                  Scale {
                      scale: Vector2 { x: 1.0, y: 1.0 },
                  },
                  /* RigidBody {
                      velocity: Vector2 { x: 0.0, y: -300.0 },
                  },
                  Timer::new(2.0, "stop_title"), */
                  TweenPosition::new(
                      Vector2 { x: 0.0, y: 384.0 },
                      Vector2 { x: 0.0, y: -220.0 },
                      2.0,
                  )
                  .with_easing(Easing::QuadOut)
                  .with_loop_mode(LoopMode::Once),
                  TweenRotation::new(-10.0, 10.0, 2.0)
                      .with_easing(Easing::QuadInOut)
                      .with_loop_mode(LoopMode::PingPong),
                  TweenScale::new(Vector2 { x: 0.9, y: 0.9 }, Vector2 { x: 1.1, y: 1.1 }, 1.0)
                      .with_easing(Easing::QuadInOut)
                      .with_loop_mode(LoopMode::PingPong),
              ));
              // Menu
              let cursor_entity = commands
                  .spawn(Sprite {
                      tex_key: "cursor".into(),
                      width: 48.0,
                      height: 48.0,
                      origin: Vector2 { x: 56.0, y: 0.0 },
                      offset: Vector2 { x: 0.0, y: 0.0 },
                      flip_h: false,
                      flip_v: false,
                  })
                  .id();

              let actions = MenuActions::new()
                  .with("start_game", MenuAction::SetScene("level01".into()))
                  .with("options", MenuAction::ShowSubMenu("options".into()))
                  .with("exit", MenuAction::QuitGame);

              commands.spawn((
                  Menu::new(
                      &[
                          ("start_game", "Start Game"),
                          ("options", "Options"),
                          ("exit", "Exit"),
                      ],
                      Vector2 { x: 250.0, y: 350.0 },
                      "arcade",
                      48.0,
                      48.0 + 16.0,
                      true,
                  )
                  .with_colors(Color::YELLOW, Color::WHITE)
                  .with_dynamic_text(true)
                  .with_cursor(cursor_entity)
                  .with_selection_sound("option"),
                  actions,
                  Group::new("main_menu"),
              ));
              // Play menu music
              audio_cmd_writer.write(AudioCmd::PlayMusic {
                  id: "menu".into(),
                  looped: true,
              });
          }
          "level01" => {
              // ==================== PHASE CALLBACKS ====================

              /// on_enter callback for "get_started" phase
              /// - Plays "player_ready" music (no loop)
              /// - Spawns the ball attached to the player with StuckTo
              fn get_started_on_enter(
                  _entity: Entity,
                  _time: f32,
                  _previous: Option<String>,
                  ctx: &mut PhaseContext,
              ) -> Option<String> {
                  // Play "player_ready" music (no loop)
                  ctx.audio_cmds.write(AudioCmd::PlayMusic {
                      id: "player_ready".into(),
                      looped: false,
                  });

                  // Get player entity and position from world_signals
                  let player_entity = match ctx.world_signals.get_entity("player") {
                      Some(e) => *e,
                      None => return None,
                  };
                  let player_y = ctx.world_signals.get_scalar("player_y").unwrap_or(700.0);

                  // Get current player X position
                  let player_x = if let Ok(pos) = ctx.positions.get(player_entity) {
                      pos.pos.x
                  } else {
                      400.0
                  };

                  // Ball Y position: above the player paddle
                  let ball_y = player_y - 24.0 - 6.0;

                  // Spawn the ball with StuckTo component
                  let ball_entity = ctx
                      .commands
                      .spawn((
                          Group::new("ball"),
                          MapPosition::new(player_x, ball_y),
                          ZIndex(10),
                          Sprite {
                              tex_key: "ball".into(),
                              width: 12.0,
                              height: 12.0,
                              offset: Vector2::zero(),
                              origin: Vector2 { x: 6.0, y: 6.0 },
                              flip_h: false,
                              flip_v: false,
                          },
                          RigidBody {
                              velocity: Vector2 { x: 0.0, y: 0.0 }, // Start with no velocity
                          },
                          BoxCollider {
                              size: Vector2 { x: 12.0, y: 12.0 },
                              offset: Vector2::zero(),
                              origin: Vector2 { x: 6.0, y: 6.0 },
                          },
                          Signals::default(),
                          // Attach ball to player (follow X only)
                          StuckTo::follow_x_only(player_entity)
                              .with_offset(Vector2 { x: 0.0, y: 0.0 })
                              .with_stored_velocity(Vector2 {
                                  x: 300.0,
                                  y: -300.0,
                              }),
                          // Timer to release the ball
                          Timer::new(2.0, "remove_stuck_to"),
                      ))
                      .id();
                  ctx.world_signals.set_entity("ball", ball_entity);

                  None
              }

              /// on_update callback for "get_started" phase
              /// - Immediately transitions to "playing"
              fn get_started_on_update(
                  _entity: Entity,
                  _time: f32,
                  _previous: Option<String>,
                  _ctx: &mut PhaseContext,
              ) -> Option<String> {
                  Some("playing".into())
              }

              /// on_update callback for "playing" phase
              /// - If no balls remain (after the first frame), transition to "lose_life"
              /// - If no bricks remain, transition to "level_cleared"
              fn playing_on_update(
                  _entity: Entity,
                  time: f32,
                  _previous: Option<String>,
                  ctx: &mut PhaseContext,
              ) -> Option<String> {
                  // Skip the first frame to allow the ball/brick group counts to update
                  if time < 0.1 {
                      return None;
                  }
                  // Check for level cleared (no bricks)
                  if let Some(0) = ctx.world_signals.get_group_count("brick") {
                      eprintln!("All bricks destroyed, level cleared!");
                      return Some("level_cleared".into());
                  }
                  // Check for ball lost
                  if let Some(0) = ctx.world_signals.get_group_count("ball") {
                      eprintln!("No balls remain, go to lose_life.");
                      return Some("lose_life".into());
                  }
                  None
              }

              /// on_update callback for "lose_life" phase
              /// - Subtracts one life
              /// - If lives < 1, transition to "game_over"
              /// - Otherwise, transition to "get_started"
              fn lose_life_on_update(
                  _entity: Entity,
                  _time: f32,
                  _previous: Option<String>,
                  ctx: &mut PhaseContext,
              ) -> Option<String> {
                  eprintln!("Player lost a life!");
                  let lives = ctx.world_signals.get_integer("lives").unwrap_or(0);
                  ctx.world_signals.set_integer("lives", lives - 1);

                  if lives - 1 < 1 {
                      Some("game_over".into())
                  } else {
                      Some("get_started".into())
                  }
              }

              /// on_enter callback for "game_over" phase
              /// - Spawns "Game Over" text centered on screen
              fn game_over_on_enter(
                  _entity: Entity,
                  _time: f32,
                  _previous: Option<String>,
                  ctx: &mut PhaseContext,
              ) -> Option<String> {
                  // Spawn "Game Over" text using ScreenPosition (centered)
                  // Screen size is 672x768, so center is around (336, 384)
                  // With font size 48, offset by half the text width/height
                  eprintln!("Game Over! Spawning game over text.");
                  ctx.commands.spawn((
                      Group::new("game_over_text"),
                      ScreenPosition::new(200.0, 350.0), // Approximate center
                      ZIndex(100),
                      DynamicText::new("GAME OVER", "future", 48.0, Color::RED),
                  ));
                  None
              }

              /// on_update callback for "game_over" phase
              /// - After 4 seconds, change scene to "menu"
              fn game_over_on_update(
                  _entity: Entity,
                  time: f32,
                  _previous: Option<String>,
                  ctx: &mut PhaseContext,
              ) -> Option<String> {
                  //eprintln!("In game_over phase, time elapsed: {:.2}", time);
                  if time >= 3.0 {
                      ctx.world_signals.set_string("scene", "menu");
                      ctx.world_signals.set_flag("switch_scene");
                      eprintln!("Game over time exceeded 3 seconds, switching to menu.");
                  }
                  None
              }

              /// on_enter callback for "level_cleared" phase
              /// - Plays "success" music (no loop)
              /// - Spawns "LEVEL CLEARED" text centered on screen
              fn level_cleared_on_enter(
                  _entity: Entity,
                  _time: f32,
                  _previous: Option<String>,
                  ctx: &mut PhaseContext,
              ) -> Option<String> {
                  eprintln!("Level cleared! Spawning level cleared text.");
                  // Play success music
                  ctx.audio_cmds.write(AudioCmd::PlayMusic {
                      id: "success".into(),
                      looped: false,
                  });
                  // Spawn "LEVEL CLEARED" text using ScreenPosition (centered)
                  ctx.commands.spawn((
                      Group::new("level_cleared_text"),
                      ScreenPosition::new(150.0, 350.0), // Approximate center
                      ZIndex(100),
                      DynamicText::new("LEVEL CLEARED", "future", 48.0, Color::GREEN),
                  ));
                  None
              }

              /// on_update callback for "level_cleared" phase
              /// - After 4 seconds, change scene to "menu" (TODO: go to next level)
              fn level_cleared_on_update(
                  _entity: Entity,
                  time: f32,
                  _previous: Option<String>,
                  ctx: &mut PhaseContext,
              ) -> Option<String> {
                  if time >= 4.0 {
                      // TODO: Go to next level instead of menu
                      ctx.world_signals.set_string("scene", "menu");
                      ctx.world_signals.set_flag("switch_scene");
                      eprintln!("Level cleared time exceeded 4 seconds, switching to menu.");
                  }
                  None
              }

              // Spawn the scene phase entity with all callbacks
              // Start in "init" phase which immediately transitions to "get_started"
              // This ensures the on_enter callback for "get_started" runs on the first frame
              fn init_on_update(
                  _entity: Entity,
                  _time: f32,
                  _previous: Option<String>,
                  _ctx: &mut PhaseContext,
              ) -> Option<String> {
                  Some("get_started".into())
              }

              commands.spawn((
                  Group::new("scene_phases"),
                  Phase::new("init")
                      .on_update("init", init_on_update as PhaseCallback)
                      .on_enter("get_started", get_started_on_enter as PhaseCallback)
                      .on_update("get_started", get_started_on_update as PhaseCallback)
                      .on_update("playing", playing_on_update as PhaseCallback)
                      .on_update("lose_life", lose_life_on_update as PhaseCallback)
                      .on_enter("game_over", game_over_on_enter as PhaseCallback)
                      .on_update("game_over", game_over_on_update as PhaseCallback)
                      .on_enter("level_cleared", level_cleared_on_enter as PhaseCallback)
                      .on_update("level_cleared", level_cleared_on_update as PhaseCallback),
              ));

              // ==================== COLLISION CALLBACKS ====================

              // callback for player-wall collision
              fn player_wall_collision_callback(
                  player_entity: Entity,
                  _wall_entity: Entity,
                  ctx: &mut CollisionContext,
              ) {
                  //eprintln!("player_wall_collision_callback: Player collided with wall!");
                  // Stop the player's movement upon collision with the wall
                  if let Ok(mut pos) = ctx.positions.get_mut(player_entity) {
                      pos.pos.x = pos.pos.x.max(72.0).min(600.0);
                      //eprintln!("Corrected player X position to: {}", pos.pos.x);
                  }
              }
              commands.spawn((
                  CollisionRule::new(
                      "player",
                      "walls",
                      player_wall_collision_callback as CollisionCallback,
                  ),
                  Group::new("collision_rules"),
              ));
              // Callback for ball-wall collision
              fn ball_wall_collision_callback(
                  ball_entity: Entity,
                  wall_entity: Entity,
                  ctx: &mut CollisionContext,
              ) {
                  // eprintln!("ball_wall_collision_callback: Ball collided with wall!");
                  let (mut ball_pos, mut ball_rb) = match (
                      ctx.positions.get_mut(ball_entity),
                      ctx.rigid_bodies.get_mut(ball_entity),
                  ) {
                      (Ok(pos), Ok(rb)) => (pos.pos, rb),
                      _ => return,
                  };
                  let ball_size = if let Ok(ball_collider) = ctx.box_colliders.get(ball_entity) {
                      ball_collider.size
                  } else {
                      return;
                  };
                  // Get the relative position of the ball to the wall to determine bounce direction
                  if let Ok(wall_pos) = ctx.positions.get(wall_entity) {
                      // positions of the lateral walls are at the bottom left/right corners
                      // and position of the top wall is at its center top
                      // If ball is bellow the wall, collision is from top
                      // and y velocity should be changed to positive (down)
                      if wall_pos.pos.y < ball_pos.y {
                          // Collision with top wall
                          ball_rb.velocity.y = ball_rb.velocity.y.abs();

                          // fix ball position to be just below the wall to prevent more collisions in the next frame
                          let wall_height =
                              if let Ok(wall_collider) = ctx.box_colliders.get(wall_entity) {
                                  wall_collider.size.y
                              } else {
                                  0.0
                              };
                          ball_pos.y = wall_pos.pos.y + wall_height + (ball_size.y * 0.5);
                      }
                      // If ball is above the wall, collision is one of the lateral walls
                      // If ball is to the left of the wall, collision is from right
                      // and x velocity should be changed to negative (go left)
                      // If ball is to the right of the wall, collision is from left
                      // and x velocity should be changed to positive (go right)
                      else {
                          let wall_width =
                              if let Ok(wall_collider) = ctx.box_colliders.get(wall_entity) {
                                  wall_collider.size.x
                              } else {
                                  0.0
                              };
                          if ball_pos.x < wall_pos.pos.x {
                              // Collision with right wall
                              ball_rb.velocity.x = -ball_rb.velocity.x.abs();
                              // fix ball position to be just left of the wall to prevent more collisions in the next frame
                              ball_pos.x = wall_pos.pos.x - wall_width - (ball_size.x * 0.5);
                          } else {
                              // Collision with left wall
                              ball_rb.velocity.x = ball_rb.velocity.x.abs();
                              // fix ball position to be just right of the wall to prevent more collisions in the next frame
                              ball_pos.x = wall_pos.pos.x + wall_width + (ball_size.x * 0.5);
                          }
                      }
                  }
              }
              commands.spawn((
                  CollisionRule::new(
                      "ball",
                      "walls",
                      ball_wall_collision_callback as CollisionCallback,
                  ),
                  Group::new("collision_rules"),
              ));
              // Callback for ball-player collision
              fn ball_player_collision_callback(
                  ball_entity: Entity,
                  player_entity: Entity,
                  ctx: &mut CollisionContext,
              ) {
                  // Reflect the ball's velocity based on where it hit the player paddle
                  // We know that the ball_entity and player_entity are correct from the `collision_observer`
                  // `collision_observer` ensures ball_entity is "ball" group and player_entity is "player" group because of the alphabetical order of the names

                  // First, get the player position immutably
                  let player_pos = if let Ok(player_pos) = ctx.positions.get(player_entity) {
                      player_pos.pos
                  } else {
                      return;
                  };

                  let player_height =
                      if let Ok(player_collider) = ctx.box_colliders.get(player_entity) {
                          player_collider.size.y
                      } else {
                          return;
                      };

                  let ball_height = if let Ok(ball_collider) = ctx.box_colliders.get(ball_entity) {
                      ball_collider.size.y
                  } else {
                      return;
                  };
                  // Now we can borrow ball_pos mutably and ball_rb mutably
                  if let (Ok(mut ball_pos), Ok(mut ball_rb)) = (
                      ctx.positions.get_mut(ball_entity),
                      ctx.rigid_bodies.get_mut(ball_entity),
                  ) {
                      let hit_pos = ball_pos.pos.x - player_pos.x;
                      let paddle_half_width = 96.0 * 0.5;
                      let relative_hit_pos = hit_pos / paddle_half_width;
                      let bounce_angle = relative_hit_pos * std::f32::consts::FRAC_PI_3; // Max 60 degrees
                      let speed = (ball_rb.velocity.x.powi(2) + ball_rb.velocity.y.powi(2)).sqrt();
                      ball_rb.velocity.x = speed * bounce_angle.sin();
                      ball_rb.velocity.y = -speed * bounce_angle.cos();
                      // Fix ball position to be just above the paddle to prevent more collisions in the next frame
                      ball_pos.pos.y = player_pos.y - player_height - (ball_height * 0.5);
                  }
                  // If the player is sticky, set the ball's velocity to zero and stuck it to the player
                  if let Ok(player_signals) = ctx.signals.get(player_entity) {
                      if player_signals.has_flag("sticky") {
                          // Get current velocity to store in StuckTo component
                          let stored_velocity =
                              ctx.rigid_bodies.get(ball_entity).map(|rb| rb.velocity).ok();

                          // Calculate offset: ball position relative to player position
                          let offset_x = if let Ok(ball_pos) = ctx.positions.get(ball_entity) {
                              ball_pos.pos.x - player_pos.x
                          } else {
                              0.0
                          };

                          // Stop the ball
                          if let Ok(mut ball_rb) = ctx.rigid_bodies.get_mut(ball_entity) {
                              ball_rb.velocity = Vector2 { x: 0.0, y: 0.0 };
                          }

                          // Attach ball to player using StuckTo component (follow X only, with offset)
                          let mut stuck_to =
                              StuckTo::follow_x_only(player_entity).with_offset(Vector2 {
                                  x: offset_x,
                                  y: 0.0,
                              });
                          if let Some(vel) = stored_velocity {
                              stuck_to = stuck_to.with_stored_velocity(vel);
                          }
                          ctx.commands.entity(ball_entity).insert(stuck_to);

                          // Set timer to remove StuckTo after 2.0 seconds
                          ctx.commands
                              .entity(ball_entity)
                              .insert(Timer::new(2.0, "remove_stuck_to"));
                      }
                  }
                  ctx.audio_cmds.write(AudioCmd::PlayFx { id: "ping".into() });
              }
              commands.spawn((
                  CollisionRule::new(
                      "ball",
                      "player",
                      ball_player_collision_callback as CollisionCallback,
                  ),
                  Group::new("collision_rules"),
              ));
              // Callback for ball-brick collision
              fn ball_brick_collision_callback(
                  ball_entity: Entity,
                  brick_entity: Entity,
                  ctx: &mut CollisionContext,
              ) {
                  // Reflect the ball's velocity based on where it hit the brick
                  let position = ctx.positions.get(ball_entity).unwrap().pos();
                  let ball_rect = ctx
                      .box_colliders
                      .get(ball_entity)
                      .unwrap()
                      .as_rectangle(position);
                  let position = ctx.positions.get(brick_entity).unwrap().pos();
                  let brick_rect = ctx
                      .box_colliders
                      .get(brick_entity)
                      .unwrap()
                      .as_rectangle(position);
                  let Some((_colliding_sides_ball, colliding_sides_brick)) =
                      get_colliding_sides(&ball_rect, &brick_rect)
                  else {
                      return;
                  };
                  let (mut ball_rb, mut ball_pos) = match (
                      ctx.rigid_bodies.get_mut(ball_entity),
                      ctx.positions.get_mut(ball_entity),
                  ) {
                      (Ok(rb), Ok(pos)) => (rb, pos.pos()),
                      _ => return,
                  };
                  for brick_side in colliding_sides_brick {
                      match brick_side {
                          BoxSide::Top => {
                              ball_rb.velocity.y = -ball_rb.velocity.y.abs();
                              ball_pos.y = brick_rect.y - (ball_rect.height * 0.5);
                          }
                          BoxSide::Bottom => {
                              ball_rb.velocity.y = ball_rb.velocity.y.abs();
                              ball_pos.y =
                                  brick_rect.y + brick_rect.height + (ball_rect.height * 0.5);
                          }
                          BoxSide::Left => {
                              ball_rb.velocity.x = -ball_rb.velocity.x.abs();
                              ball_pos.x = brick_rect.x - (ball_rect.width * 0.5);
                          }
                          BoxSide::Right => {
                              ball_rb.velocity.x = ball_rb.velocity.x.abs();
                              ball_pos.x = brick_rect.x + brick_rect.width + (ball_rect.width * 0.5);
                          }
                      }
                  }
                  // substract 1 hit point from the brick's Signals
                  if let Ok(mut signals) = ctx.signals.get_mut(brick_entity) {
                      let hit_points = signals.get_integer("hp").unwrap_or(1);
                      if hit_points > 1 {
                          signals.set_integer("hp", hit_points - 1);
                      } else {
                          // Increment score
                          if let Some(points) = signals.get_integer("points") {
                              let current_score = ctx.world_signals.get_integer("score").unwrap_or(0);
                              ctx.world_signals
                                  .set_integer("score", current_score + points);
                          }
                          // Update high score if necessary
                          let current_score = ctx.world_signals.get_integer("score").unwrap_or(0);
                          let high_score = ctx.world_signals.get_integer("high_score").unwrap_or(0);
                          if current_score > high_score {
                              ctx.world_signals.set_integer("high_score", current_score);
                          }
                          // despawn brick entity
                          ctx.commands.entity(brick_entity).despawn();
                      }
                  }
                  ctx.audio_cmds.write(AudioCmd::PlayFx { id: "ding".into() });
              }
              commands.spawn((
                  CollisionRule::new(
                      "ball",
                      "brick",
                      ball_brick_collision_callback as CollisionCallback,
                  ),
                  Group::new("collision_rules"),
              ));
              // Callback for ball-oob_wall collision (bottom wall)
              fn ball_oob_wall_collision_callback(
                  ball_entity: Entity,
                  oob_wall_entity: Entity,
                  ctx: &mut CollisionContext,
              ) {
                  // eprintln!("ball_oob_wall_collision_callback: Ball collided with oob wall!");
                  // Despawn the ball if the ball collider is inside the oob wall collider
                  let (ball_pos, wall_pos) = match (
                      ctx.positions.get(ball_entity),
                      ctx.positions.get(oob_wall_entity),
                  ) {
                      (Ok(pos), Ok(wpos)) => (pos.pos, wpos.pos),
                      _ => return,
                  };
                  let (ball_rect, wall_rect) = match (
                      ctx.box_colliders.get(ball_entity),
                      ctx.box_colliders.get(oob_wall_entity),
                  ) {
                      (Ok(bcollider), Ok(wcollider)) => (
                          bcollider.as_rectangle(ball_pos),
                          wcollider.as_rectangle(wall_pos),
                      ),
                      _ => return,
                  };
                  let Some((colliding_sides_ball, _colliding_sides_wall)) =
                      get_colliding_sides(&ball_rect, &wall_rect)
                  else {
                      return;
                  };
                  if colliding_sides_ball.len() == 4 {
                      // All sides are colliding, meaning ball is fully inside the oob wall
                      // Despawn the ball
                      ctx.commands.entity(ball_entity).despawn();
                  }
              }
              commands.spawn((
                  CollisionRule::new(
                      "ball",
                      "oob_wall",
                      ball_oob_wall_collision_callback as CollisionCallback,
                  ),
                  Group::new("collision_rules"),
              ));
              // ==================== WORLDSIGNALS SETUP ====================
              // reset score to 0
              worldsignals.set_integer("score", 0);

              // reset lives to 3
              worldsignals.set_integer("lives", 3);

              // ==================== SCENE SETUP ====================

              // Load tilemap for level 1
              let (tilemap_tex, tilemap) = load_tilemap(&mut rl, &th, "./assets/tilemaps/level01");
              tilemaps_store.insert("level01", tilemap);
              let tiles_width = tilemap_tex.width;
              let tiles_height = tilemap_tex.height;
              tex_store.insert("level01", tilemap_tex);
              // Spawn tiles
              let tilemap_info = tilemaps_store
                  .get("level01")
                  .expect("tilemap info not found for level01");
              spawn_tiles(&mut commands, "level01", tiles_width, tilemap_info);
              // Bricks
              commands.spawn((GridLayout::new("./assets/levels/level01.json", "brick", 5),));
              // The Vaus. The player paddle
              let mut player_signals = Signals::default();
              player_signals.set_flag("sticky");
              let player_y = (tilemap_info.tile_size as f32 * tilemap_info.map_height as f32) - 36.0;
              let player_pos = MapPosition::new(400.0, player_y);
              let player_entity = commands
                  .spawn((
                      Group::new("player"),
                      player_pos.clone(),
                      ZIndex(10),
                      Sprite {
                          tex_key: "vaus".into(),
                          width: 96.0,
                          height: 24.0,
                          offset: Vector2::zero(),
                          origin: Vector2 { x: 48.0, y: 24.0 },
                          flip_h: false,
                          flip_v: false,
                      },
                      //RigidBody::default(),
                      MouseControlled::new(true, false),
                      player_signals,
                      BoxCollider {
                          size: Vector2 { x: 96.0, y: 24.0 },
                          offset: Vector2::zero(),
                          origin: Vector2 { x: 48.0, y: 24.0 },
                      },
                      Timer::new(3.0, "remove_sticky"),
                  ))
                  .id();
              worldsignals.set_entity("player", player_entity);
              worldsignals.set_scalar("player_y", player_y);

              // NOTE: Ball is now spawned by the "get_started" phase on_enter callback

              // Create walls as BoxColliders
              commands.spawn((
                  // Left wall
                  Group::new("walls"),
                  MapPosition::new(
                      // position is at left, bottom of the map
                      0.0,
                      (tilemap_info.tile_size * tilemap_info.map_height) as f32,
                  ),
                  BoxCollider {
                      size: Vector2 {
                          x: tilemap_info.tile_size as f32 * 1.0,
                          y: tilemap_info.tile_size as f32 * (tilemap_info.map_height - 2) as f32,
                      },
                      offset: Vector2::zero(),
                      origin: Vector2 {
                          x: 0.0,
                          y: tilemap_info.tile_size as f32 * (tilemap_info.map_height - 2) as f32,
                      },
                  },
              ));
              commands.spawn((
                  // Right wall
                  Group::new("walls"),
                  MapPosition::new(
                      // position is at right, bottom of the map
                      (tilemap_info.tile_size * tilemap_info.map_width) as f32,
                      (tilemap_info.tile_size * tilemap_info.map_height) as f32,
                  ),
                  BoxCollider {
                      size: Vector2 {
                          x: tilemap_info.tile_size as f32 * 1.0,
                          y: tilemap_info.tile_size as f32 * (tilemap_info.map_height - 2) as f32,
                      },
                      offset: Vector2::zero(),
                      origin: Vector2 {
                          x: tilemap_info.tile_size as f32 * 1.0,
                          y: tilemap_info.tile_size as f32 * (tilemap_info.map_height - 2) as f32,
                      },
                  },
              ));
              commands.spawn((
                  // Top wall
                  Group::new("walls"),
                  MapPosition::new(
                      // position is at center top of the map
                      (tilemap_info.tile_size * (tilemap_info.map_width)) as f32 * 0.5,
                      tilemap_info.tile_size as f32 * 2.0,
                  ),
                  BoxCollider {
                      size: Vector2 {
                          x: tilemap_info.tile_size as f32 * (tilemap_info.map_width - 2) as f32,
                          y: tilemap_info.tile_size as f32 * 1.0,
                      },
                      offset: Vector2::zero(),
                      origin: Vector2 {
                          x: tilemap_info.tile_size as f32
                              * (tilemap_info.map_width - 2) as f32
                              * 0.5,
                          y: tilemap_info.tile_size as f32 * 0.0,
                      },
                  },
              ));
              // Out of bounds (bottom) wall
              commands.spawn((
                  Group::new("oob_wall"),
                  MapPosition::new(
                      // position is at left, bottom of the map
                      -((tilemap_info.tile_size * 5) as f32),
                      (tilemap_info.tile_size * tilemap_info.map_height) as f32,
                  ),
                  BoxCollider {
                      size: Vector2 {
                          x: tilemap_info.tile_size as f32 * (tilemap_info.map_width + 10) as f32,
                          y: tilemap_info.tile_size as f32 * 10.0,
                      },
                      offset: Vector2::zero(),
                      origin: Vector2 { x: 0.0, y: 0.0 },
                  },
              ));
              // Score Text
              commands.spawn((
                  Group::new("ui"),
                  DynamicText::new(
                      "1UP   HIGH SCORE",
                      "arcade",
                      tilemap_info.tile_size as f32,
                      Color::RED,
                  ),
                  MapPosition::new((tilemap_info.tile_size * 3) as f32, 0.0),
                  ZIndex(20),
              ));
              commands.spawn((
                  Group::new("player_score"),
                  DynamicText::new("0", "arcade", tilemap_info.tile_size as f32, Color::WHITE),
                  MapPosition::new(
                      (tilemap_info.tile_size * 3) as f32,
                      tilemap_info.tile_size as f32,
                  ),
                  ZIndex(20),
                  SignalBinding::new("score"), // updates with "score" signal
              ));
              commands.spawn((
                  Group::new("high_score"),
                  DynamicText::new(
                      format!("{}", worldsignals.get_integer("high_score").unwrap_or(0)),
                      "arcade",
                      tilemap_info.tile_size as f32,
                      Color::WHITE,
                  ),
                  MapPosition::new(
                      (tilemap_info.tile_size * 10) as f32,
                      tilemap_info.tile_size as f32,
                  ),
                  ZIndex(20),
                  SignalBinding::new("high_score"),
              ));

              // Move camera to the center of the level
              commands.insert_resource(Camera2DRes(Camera2D {
                  target: Vector2 {
                      x: (tilemap_info.tile_size as f32 * (tilemap_info.map_width) as f32 * 0.5),
                      y: (tilemap_info.tile_size as f32 * (tilemap_info.map_height) as f32 * 0.5),
                  },
                  offset: Vector2 {
                      x: rl.get_screen_width() as f32 * 0.5,
                      y: rl.get_screen_height() as f32 * 0.5,
                  },
                  rotation: 0.0,
                  zoom: 1.0,
              }));
              // Initialize tracked groups resource
              tracked_groups.add_group("ball");
              tracked_groups.add_group("brick");
          }
          "level02" => {} */
    }

    // Stop any playing music when switching scenes
}
