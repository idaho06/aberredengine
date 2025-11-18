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
use crate::components::dynamictext::DynamicText;
use crate::components::group::Group;
use crate::components::inputcontrolled::InputControlled;
use crate::components::mapposition::MapPosition;
use crate::components::menu::Menu;
use crate::components::persistent::Persistent;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite;
use crate::components::sprite::Sprite;
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
        id: "growl".into(),
        path: "./assets/audio/growl.wav".into(),
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

    /*     // Update positions based on velocity
       for (mut map_pos, mut rb, _collider) in query_rb.iter_mut() {

           // Update position based on velocity
           map_pos.x += rb.velocity.x * delta_sec;
           map_pos.y += rb.velocity.y * delta_sec;

           // Simple boundary collision with screen edges (assuming 800x450 screen size)
           if map_pos.x < 0.0 {
               map_pos.x = 0.0;
               rb.velocity.x = -rb.velocity.x; // Reverse X velocity
           } else if map_pos.x > 800.0 {
               map_pos.x = 800.0;
               rb.velocity.x = -rb.velocity.x; // Reverse X velocity
           }
           if map_pos.y < 0.0 {
               map_pos.y = 0.0;
               rb.velocity.y = -rb.velocity.y; // Reverse Y velocity
           } else if map_pos.y > 450.0 {
               map_pos.y = 450.0;
               rb.velocity.y = -rb.velocity.y; // Reverse Y velocity
           }
       }
    */
    // Update enemy sprites based on their velocity (flip horizontally)
    /* for (mut sprite, rb) in query_enemies.iter_mut() {
        if rb.velocity.x < 0.0 {
            sprite.flip_h = true;
        } else if rb.velocity.x > 0.0 {
            sprite.flip_h = false;
        }
    } */

    let scene = world_signals
        .get_string("scene")
        .cloned()
        .unwrap_or("menu".to_string());

    match scene.as_str() {
        "menu" => {
            // Look inside world signals for option selected
            let mut menu_selected_option: Option<String> = None;
            if let Some(selected_item) = world_signals.remove_string("selected_item") {
                menu_selected_option = Some(selected_item.clone());
            }
            if let Some(option) = menu_selected_option {
                match option.as_str() {
                    "start_game" => {
                        // Switch to level 1
                        world_signals.set_string("scene", "level01");
                        commands.run_system(
                            systems_store
                                .get("switch_scene")
                                .expect("switch_scene system not found")
                                .clone(),
                        );
                    }
                    "options" => {
                        // Handle options menu (not implemented)
                    }
                    "exit" => {
                        // Exit the game
                        next_game_state.set(GameStates::Quitting);
                    }
                    _ => {}
                }
            }
            // If action_back is pressed, exit game
            if input.action_back.just_pressed {
                next_game_state.set(GameStates::Quitting);
            }
        }
        "level01" => {
            // Level 1 specific updates
            // If action_back is pressed, go back to menu
            if input.action_back.just_pressed {
                world_signals.set_string("scene", "menu");
                commands.run_system(
                    systems_store
                        .get("switch_scene")
                        .expect("switch_scene system not found")
                        .clone(),
                );
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
        eprintln!("Despawning entity: {:?}", entity);
        commands.entity(entity).despawn();
    }
}

pub fn switch_scene(
    mut commands: Commands,
    mut audio_cmd_writer: bevy_ecs::prelude::MessageWriter<AudioCmd>,
    worldsignals: ResMut<WorldSignals>,
    systems_store: Res<SystemsStore>,
    mut tilemaps_store: ResMut<TilemapStore>,
    mut tex_store: ResMut<TextureStore>,
    entities_to_clean: Query<Entity, Without<Persistent>>,
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
        eprintln!("Despawning entity: {:?}", entity);
        commands.entity(entity).despawn();
    }

    tilemaps_store.clear();

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
                .with_cursor(cursor_entity),
                Group::new("main_menu"),
            ));
            // Play menu music
            audio_cmd_writer.write(AudioCmd::PlayMusic {
                id: "menu".into(),
                looped: true,
            });
        }
        "level01" => {
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
            // The Vaus. The player paddle
            commands.spawn((
                Group::new("player"),
                MapPosition::new(
                    400.0,
                    (tilemap_info.tile_size as f32 * tilemap_info.map_height as f32) - 36.0,
                ),
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
        }
        "level02" => {}
        _ => {
            eprintln!("Unknown scene: {}", scene);
            panic!("Unknown scene");
        }
    }

    // Stop any playing music when switching scenes
}
