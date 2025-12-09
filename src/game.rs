use std::ffi::CString;
use std::panic;

use bevy_ecs::event::Trigger;
use bevy_ecs::prelude::*;
use raylib::ffi;
use raylib::ffi::TextureFilter::{TEXTURE_FILTER_ANISOTROPIC_8X, TEXTURE_FILTER_BILINEAR};
use raylib::prelude::*;
use rustc_hash::FxHashMap;
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
use crate::components::signals::Signals;
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
//use rand::Rng;

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
                    tex_key: tilemap_tex_key.clone(),
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
            x: 0.0,
            y: 0.0, //x: 0.0,
                    //y: 0.0,
        },
        offset: Vector2 {
            x: rl.get_screen_width() as f32 * 0.5,
            y: rl.get_screen_height() as f32 * 0.5,
        },
        rotation: 0.0,
        zoom: 1.0,
    };
    commands.insert_resource(Camera2DRes(camera));

    // Load fonts
    let font = load_font_with_mipmaps(&mut rl, &th, "./assets/fonts/Arcade_Cabinet.ttf", 128);
    fonts.add("arcade", font);

    let font = load_font_with_mipmaps(&mut rl, &th, "./assets/fonts/Formal_Future.ttf", 128);
    fonts.add("future", font);

    // Load textures
    let title_tex = rl
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
        .expect("load assets/brick_silver.png");

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
    tex_store.insert("title", title_tex);
    tex_store.insert("background", background_tex);
    tex_store.insert("cursor", cursor_tex);
    tex_store.insert("vaus", vaus_tex);
    tex_store.insert("ball", ball_tex);
    tex_store.insert("brick_red", brick_red_tex);
    tex_store.insert("brick_green", brick_green_tex);
    tex_store.insert("brick_blue", brick_blue_tex);
    tex_store.insert("brick_yellow", brick_yellow_tex);
    tex_store.insert("brick_purple", brick_purple_tex);
    tex_store.insert("brick_silver", brick_silver_tex);
    /* tex_store.insert("player-sheet", player_sheet_tex);
    tex_store.insert("dummy", dummy_tex);
    tex_store.insert("enemy", enemy_tex);
    tex_store.insert("tilemap", tilemap_tex);
    tex_store.insert("billboard", billboard_tex); */
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
    audio_cmd_writer.write(AudioCmd::LoadMusic {
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
    });

    // Don't block; the audio thread will emit load messages which are polled by systems.

    // Change GameState to Playing
    next_state.set(GameStates::Playing);
    eprintln!("Game setup_with_commands() done, next state set to Playing");
}

pub fn quit_game(
    //mut commands: Commands,
    //mut rl: NonSendMut<raylib::RaylibHandle>,
    mut world_signals: ResMut<WorldSignals>,
) {
    eprintln!("Quitting game...");

    // Perform any necessary cleanup here

    // Optionally, set a signal to indicate the game should exit
    world_signals.set_flag("quit_game");
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
    worldsignals.set_integer("score", 0);
    worldsignals.set_integer("high_score", 0);
    worldsignals.set_integer("lives", 3);
    worldsignals.set_integer("level", 1);
    worldsignals.set_string("scene", "menu");

    // Observer for TimerEvent
    commands.add_observer(|trigger: On<TimerEvent>, mut commands: Commands| {
        match trigger.signal.as_str() {
            "stop_title" => {
                //commands.entity(trigger.entity).remove::<RigidBody>();
                commands.entity(trigger.entity).insert(RigidBody {
                    velocity: Vector2 { x: 0.0, y: 0.0 },
                });
                commands.entity(trigger.entity).remove::<Timer>();
                commands.entity(trigger.entity).insert(MapPosition {
                    pos: Vector2 { x: 0.0, y: -220.0 },
                });
            }
            _ => (),
        }
    });

    // Observer to remove the "sticky" flag from the entity (meant to be used by the "player" or "ball" entity)
    commands.add_observer(
        |trigger: On<TimerEvent>, mut signals: Query<&mut Signals>, mut commands: Commands| {
            let entity = trigger.entity;
            let signal = &trigger.signal;

            if signal == "remove_sticky" {
                if let Ok(mut sigs) = signals.get_mut(entity) {
                    sigs.clear_flag("sticky");
                }
                commands.entity(entity).remove::<Timer>();
            }
        },
    );

    // Observer to remove StuckTo component and restore velocity
    commands.add_observer(
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
    );

    // Finally, run the switch_scene system to spawn initial scene entities
    commands.run_system(
        systems_store
            .get("switch_scene")
            .expect("switch_scene system not found")
            .clone(),
    );
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
) {
    let _delta_sec = time.delta;

    let scene = world_signals
        .get_string("scene")
        .cloned()
        .unwrap_or("menu".to_string());

    match scene.as_str() {
        "menu" => {
            // Menu specific updates
            if input.action_back.just_pressed {
                next_game_state.set(GameStates::Quitting);
            }
        }
        "level01" => {
            // Level 1 specific updates
            let switch_scene_system = systems_store
                .get("switch_scene")
                .expect("switch_scene system not found")
                .clone();
            // If action_back is pressed, go back to menu
            if input.action_back.just_pressed {
                world_signals.set_string("scene", "menu");
                commands.run_system(switch_scene_system);
                return;
            }
            if let Some(0) = world_signals.get_group_count("ball") {
                // All balls lost, substract a life
                eprintln!("All balls lost!");
                let lives = world_signals.get_integer("lives").unwrap_or(0);
                if lives > 1 {
                    world_signals.set_integer("lives", lives - 1);
                    // restart scene without changing bricks
                    // TODO: implement
                } else {
                    // Game over, go to menu
                    eprintln!("Game over!");
                    world_signals.set_string("scene", "menu");
                    commands.run_system(switch_scene_system);
                    return;
                }
            }
            if let Some(0) = world_signals.get_group_count("brick") {
                eprintln!("Level cleared!");
                // Level cleared, go to next level
                /* world_signals.set_string("scene", "level02");
                commands.run_system(switch_scene_system);
                return; */
            }
        }
        "level02" => {
            // Level 2 specific updates
        }
        _ => {
            // Default or unknown scene updates
        }
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
        .unwrap_or_else(|| "menu".to_string());

    match scene.as_str() {
        "menu" => {
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
            // test callback for phase "get_ready" of the scene
            fn level01_get_ready_callback(
                entity: Entity,
                time: f32,
                previous: Option<String>,
                ctx: &mut PhaseContext,
            ) -> Option<String> {
                eprintln!(
                    "level01_get_ready_callback: Entity {:?} updating 'get_ready' phase!",
                    entity
                );
                // after 3 seconds, switch to "playing" phase
                if time >= 3.0 {
                    return Some("playing".into());
                }
                None
            }
            commands.spawn((
                Group::new("scene_phases"),
                Phase::new("get_ready")
                    .on_update("get_ready", level01_get_ready_callback as PhaseCallback),
            ));
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
            // reset score to 0
            worldsignals.set_integer("score", 0);

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
            let y = (tilemap_info.tile_size as f32 * tilemap_info.map_height as f32) - 36.0;
            let player_pos = MapPosition::new(400.0, y);
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
                    Timer::new(4.0, "remove_sticky"),
                ))
                .id();
            worldsignals.set_entity("player", player_entity);
            // The Ball
            let y = player_pos.pos().y - 24.0 - 6.0;
            commands.spawn((
                Group::new("ball"),
                MapPosition::new(400.0, y),
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
                    velocity: Vector2 {
                        x: 300.0,
                        y: -300.0,
                    },
                },
                BoxCollider {
                    size: Vector2 { x: 12.0, y: 12.0 },
                    offset: Vector2::zero(),
                    origin: Vector2 { x: 6.0, y: 6.0 },
                },
                Signals::default(),
            ));

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
        "level02" => {}
        _ => {
            eprintln!("Unknown scene: {}", scene);
            panic!("Unknown scene");
        }
    }

    // Stop any playing music when switching scenes
}
