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
use bevy_ecs::prelude::*;
use raylib::ffi;
use raylib::ffi::TextureFilter::{TEXTURE_FILTER_ANISOTROPIC_8X, TEXTURE_FILTER_BILINEAR};
use raylib::prelude::*;
use rustc_hash::FxHashMap;
//use std::collections::HashMap; // always prefer FxHashMap for performance

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
use crate::components::luacollision::LuaCollisionRule;
use crate::components::mapposition::MapPosition;
use crate::components::menu::{Menu, MenuAction, MenuActions};
use crate::components::persistent::Persistent;
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
use crate::resources::lua_runtime::LuaRuntime;
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

/// Spawn tiles from a Tilemap resource into the ECS world.
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
    lua_runtime: NonSend<LuaRuntime>,
) {
    // This function sets up the game world, loading resources
    use crate::resources::lua_runtime::AssetCmd;

    // Default camera. Needed to start the engine before entering play state
    // The camera will be overridden later in the scene setup
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

    // Call Lua on_setup function to queue asset loading commands
    if lua_runtime.has_function("on_setup") {
        if let Err(e) = lua_runtime.call_function::<_, ()>("on_setup", ()) {
            eprintln!("[Rust] Error calling on_setup: {}", e);
        }
    }

    // Initialize stores
    let mut tex_store = TextureStore::new();
    let mut tilemaps_store = TilemapStore::new();

    // Process asset commands queued by Lua
    for cmd in lua_runtime.drain_asset_commands() {
        match cmd {
            AssetCmd::LoadTexture { id, path } => match rl.load_texture(&th, &path) {
                Ok(tex) => {
                    eprintln!("[Rust] Loaded texture '{}' from '{}'", id, path);
                    tex_store.insert(&id, tex);
                }
                Err(e) => {
                    eprintln!("[Rust] Failed to load texture '{}': {}", path, e);
                }
            },
            AssetCmd::LoadFont { id, path, size } => {
                let font = load_font_with_mipmaps(&mut rl, &th, &path, size);
                eprintln!("[Rust] Loaded font '{}' from '{}'", id, path);
                fonts.add(&id, font);
            }
            AssetCmd::LoadMusic { id, path } => {
                eprintln!("[Rust] Queuing music '{}' from '{}'", id, path);
                audio_cmd_writer.write(AudioCmd::LoadMusic { id, path });
            }
            AssetCmd::LoadSound { id, path } => {
                eprintln!("[Rust] Queuing sound '{}' from '{}'", id, path);
                audio_cmd_writer.write(AudioCmd::LoadFx { id, path });
            }
            AssetCmd::LoadTilemap { id, path } => {
                // Load tilemap texture and JSON metadata
                let (tilemap_tex, tilemap) = load_tilemap(&mut rl, &th, &path);
                let tiles_width = tilemap_tex.width;
                eprintln!(
                    "[Rust] Loaded tilemap '{}' from '{}' ({}x{} texture, tile_size={})",
                    id, path, tiles_width, tilemap_tex.height, tilemap.tile_size
                );
                tex_store.insert(&id, tilemap_tex);
                tilemaps_store.insert(&id, tilemap);
            }
        }
    }

    commands.insert_resource(tex_store);
    commands.insert_resource(tilemaps_store);

    // Process animation registration commands from Lua
    let mut anim_store = AnimationStore {
        animations: FxHashMap::default(),
    };
    for cmd in lua_runtime.drain_animation_commands() {
        match cmd {
            crate::resources::lua_runtime::AnimationCmd::RegisterAnimation {
                id,
                tex_key,
                pos_x,
                pos_y,
                displacement,
                frame_count,
                fps,
                looped,
            } => {
                anim_store.animations.insert(
                    id.clone(),
                    AnimationResource {
                        tex_key,
                        position: Vector2 { x: pos_x, y: pos_y },
                        displacement,
                        frame_count,
                        fps,
                        looped,
                    },
                );
                eprintln!(
                    "[Rust] Registered animation '{}' ({} frames, {} fps)",
                    id, frame_count, fps
                );
            }
        }
    }
    commands.insert_resource(anim_store);

    // Change GameState to Playing
    next_state.set(GameStates::Playing);
    eprintln!("Game setup() done, next state set to Playing");
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
    //mut audio_cmd_writer: bevy_ecs::prelude::MessageWriter<AudioCmd>,
    //tex_store: Res<TextureStore>,
    //tilemaps_store: Res<TilemapStore>, // TODO: Make it optional?
    mut worldsignals: ResMut<WorldSignals>,
    //mut tracked_groups: ResMut<TrackedGroups>,
    systems_store: Res<SystemsStore>,
    lua_runtime: NonSend<LuaRuntime>,
) {
    // Call Lua on_enter_play function if it exists
    if lua_runtime.has_function("on_enter_play") {
        match lua_runtime.call_function::<_, String>("on_enter_play", ()) {
            Ok(result) => {
                eprintln!("[Rust] Lua on_enter_play returned: {}", result);
            }
            Err(e) => {
                eprintln!("[Rust] Error calling on_enter_play: {}", e);
            }
        }
    }
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

    // Finally, run the switch_scene system to spawn initial scene entities
    commands.run_system(
        systems_store
            .get("switch_scene")
            .expect("switch_scene system not found")
            .clone(),
    );
}

/// Per-frame update system for scene-specific logic.
///
/// This system delegates scene behavior to Lua callbacks:
/// - Calls `on_update_<scene>` callback in Lua for the current scene
/// - Lua can queue signal commands (set_flag, set_string, etc.)
/// - Processes signal commands from Lua
/// - Reacts to flags set by Lua: "switch_scene", "quit_game"
pub fn update(
    time: Res<WorldTime>,
    input: Res<InputState>,
    mut commands: Commands,
    systems_store: Res<SystemsStore>,
    mut world_signals: ResMut<WorldSignals>,
    mut next_game_state: ResMut<NextGameState>,
    lua_runtime: NonSend<LuaRuntime>,
) {
    let delta_sec = time.delta;

    let scene = world_signals
        .get_string("scene")
        .cloned()
        .unwrap_or("menu".to_string());

    /*
    match scene.as_str() {
        "menu" => {
            // Menu-specific logic can go here if needed
        }
        "level01" => {
            // Level-specific logic can go here if needed
        }
        _ => {
            // Default or unknown scene logic
        }
    }
    */

    // Update signal cache for Lua to read current values
    lua_runtime.update_signal_cache(
        &world_signals.scalars,
        &world_signals.integers,
        &world_signals.strings,
        &world_signals.flags,
        &world_signals.group_counts(),
        &world_signals
            .entities
            .iter()
            .map(|(k, v)| (k.clone(), v.to_bits()))
            .collect(),
    );

    // Update input cache for Lua to read current input state
    lua_runtime.update_input_cache(&input);

    // Call scene-specific update callback
    let callback_name = format!("on_update_{}", scene);
    if lua_runtime.has_function(&callback_name) {
        if let Err(e) = lua_runtime.call_function::<_, ()>(&callback_name, delta_sec) {
            eprintln!("[Rust] Error calling {}: {}", callback_name, e);
        }
    }

    // Process signal commands queued by Lua
    for cmd in lua_runtime.drain_signal_commands() {
        use crate::resources::lua_runtime::SignalCmd;
        match cmd {
            SignalCmd::SetScalar { key, value } => {
                world_signals.set_scalar(&key, value);
            }
            SignalCmd::SetInteger { key, value } => {
                world_signals.set_integer(&key, value);
            }
            SignalCmd::SetString { key, value } => {
                world_signals.set_string(&key, &value);
            }
            SignalCmd::SetFlag { key } => {
                world_signals.set_flag(&key);
            }
            SignalCmd::ClearFlag { key } => {
                world_signals.clear_flag(&key);
            }
        }
    }

    // Check for quit flag (set by Lua)
    if world_signals.has_flag("quit_game") {
        world_signals.clear_flag("quit_game");
        next_game_state.set(GameStates::Quitting);
        return;
    }

    // Check for scene switch flag (set by Lua)
    if world_signals.has_flag("switch_scene") {
        eprintln!("Scene switch requested in world signals.");
        world_signals.clear_flag("switch_scene");
        let switch_scene_system = systems_store
            .get("switch_scene")
            .expect("switch_scene system not found")
            .clone();
        commands.run_system(switch_scene_system);
    }
}

pub fn clean_all_entities(mut commands: Commands, query: Query<Entity, Without<Persistent>>) {
    for entity in query.iter() {
        //eprintln!("Despawning entity: {:?}", entity);
        commands.entity(entity).despawn();
    }
}

/// Parse easing string from Lua into Easing enum
fn parse_easing(easing: &str) -> Easing {
    match easing {
        "linear" => Easing::Linear,
        "quad_in" => Easing::QuadIn,
        "quad_out" => Easing::QuadOut,
        "quad_in_out" => Easing::QuadInOut,
        "cubic_in" => Easing::CubicIn,
        "cubic_out" => Easing::CubicOut,
        "cubic_in_out" => Easing::CubicInOut,
        _ => Easing::Linear,
    }
}

/// Parse loop mode string from Lua into LoopMode enum
fn parse_loop_mode(loop_mode: &str) -> LoopMode {
    match loop_mode {
        "once" => LoopMode::Once,
        "loop" => LoopMode::Loop,
        "ping_pong" => LoopMode::PingPong,
        _ => LoopMode::Once,
    }
}

/// Parse comparison operator string from Lua into CmpOp enum
fn parse_cmp_op(op: &str) -> CmpOp {
    match op {
        "lt" => CmpOp::Lt,
        "le" => CmpOp::Le,
        "gt" => CmpOp::Gt,
        "ge" => CmpOp::Ge,
        "eq" => CmpOp::Eq,
        "ne" => CmpOp::Ne,
        _ => CmpOp::Eq,
    }
}

/// Convert AnimationConditionData from Lua to Condition enum
fn convert_animation_condition(
    data: crate::resources::lua_runtime::AnimationConditionData,
) -> Condition {
    use crate::resources::lua_runtime::AnimationConditionData;
    match data {
        AnimationConditionData::ScalarCmp { key, op, value } => Condition::ScalarCmp {
            key,
            op: parse_cmp_op(&op),
            value,
        },
        AnimationConditionData::ScalarRange {
            key,
            min,
            max,
            inclusive,
        } => Condition::ScalarRange {
            key,
            min,
            max,
            inclusive,
        },
        AnimationConditionData::IntegerCmp { key, op, value } => Condition::IntegerCmp {
            key,
            op: parse_cmp_op(&op),
            value,
        },
        AnimationConditionData::IntegerRange {
            key,
            min,
            max,
            inclusive,
        } => Condition::IntegerRange {
            key,
            min,
            max,
            inclusive,
        },
        AnimationConditionData::HasFlag { key } => Condition::HasFlag { key },
        AnimationConditionData::LacksFlag { key } => Condition::LacksFlag { key },
        AnimationConditionData::All(conditions) => Condition::All(
            conditions
                .into_iter()
                .map(convert_animation_condition)
                .collect(),
        ),
        AnimationConditionData::Any(conditions) => Condition::Any(
            conditions
                .into_iter()
                .map(convert_animation_condition)
                .collect(),
        ),
        AnimationConditionData::Not(inner) => {
            Condition::Not(Box::new(convert_animation_condition(*inner)))
        }
    }
}

/// Processes a SpawnCmd from Lua and spawns the corresponding entity.
/// Returns the spawned entity ID and optional registration key.
fn process_spawn_cmd(
    commands: &mut Commands,
    cmd: crate::resources::lua_runtime::SpawnCmd,
    worldsignals: &mut WorldSignals,
) {
    let mut entity_commands = commands.spawn_empty();
    let entity = entity_commands.id();

    // Group
    if let Some(group_name) = cmd.group {
        entity_commands.insert(Group::new(&group_name));
    }

    // Position
    if let Some((x, y)) = cmd.position {
        entity_commands.insert(MapPosition::new(x, y));
    }

    // Sprite
    if let Some(sprite_data) = cmd.sprite {
        entity_commands.insert(Sprite {
            tex_key: sprite_data.tex_key,
            width: sprite_data.width,
            height: sprite_data.height,
            origin: Vector2 {
                x: sprite_data.origin_x,
                y: sprite_data.origin_y,
            },
            offset: Vector2 {
                x: sprite_data.offset_x,
                y: sprite_data.offset_y,
            },
            flip_h: sprite_data.flip_h,
            flip_v: sprite_data.flip_v,
        });
    }

    // ZIndex
    if let Some(z) = cmd.zindex {
        entity_commands.insert(ZIndex(z));
    }

    // RigidBody
    if let Some(rb_data) = cmd.rigidbody {
        entity_commands.insert(RigidBody {
            velocity: Vector2 {
                x: rb_data.velocity_x,
                y: rb_data.velocity_y,
            },
        });
    }

    // BoxCollider
    if let Some(collider_data) = cmd.collider {
        entity_commands.insert(BoxCollider {
            size: Vector2 {
                x: collider_data.width,
                y: collider_data.height,
            },
            offset: Vector2 {
                x: collider_data.offset_x,
                y: collider_data.offset_y,
            },
            origin: Vector2 {
                x: collider_data.origin_x,
                y: collider_data.origin_y,
            },
        });
    }

    // MouseControlled
    if let Some((follow_x, follow_y)) = cmd.mouse_controlled {
        entity_commands.insert(MouseControlled { follow_x, follow_y });
    }

    // Rotation
    if let Some(degrees) = cmd.rotation {
        entity_commands.insert(Rotation { degrees });
    }

    // Scale
    if let Some((sx, sy)) = cmd.scale {
        entity_commands.insert(Scale {
            scale: Vector2 { x: sx, y: sy },
        });
    }

    // Persistent
    if cmd.persistent {
        entity_commands.insert(Persistent);
    }

    // Signals
    if cmd.has_signals
        || !cmd.signal_scalars.is_empty()
        || !cmd.signal_integers.is_empty()
        || !cmd.signal_flags.is_empty()
        || !cmd.signal_strings.is_empty()
    {
        let mut signals = Signals::default();
        for (key, value) in cmd.signal_scalars {
            signals.set_scalar(&key, value);
        }
        for (key, value) in cmd.signal_integers {
            signals.set_integer(&key, value);
        }
        for flag in cmd.signal_flags {
            signals.set_flag(&flag);
        }
        for (key, value) in cmd.signal_strings {
            signals.set_string(&key, &value);
        }
        entity_commands.insert(signals);
    }

    // ScreenPosition (for UI elements)
    if let Some((x, y)) = cmd.screen_position {
        entity_commands.insert(ScreenPosition::new(x, y));
    }

    // DynamicText
    if let Some(text_data) = cmd.text {
        entity_commands.insert(DynamicText::new(
            text_data.content,
            text_data.font,
            text_data.font_size,
            Color::new(text_data.r, text_data.g, text_data.b, text_data.a),
        ));
    }

    // LuaPhase
    if let Some(phase_data) = cmd.phase_data {
        use crate::components::luaphase::{LuaPhase, PhaseCallbacks};
        // Convert PhaseCallbackData to PhaseCallbacks
        let phases = phase_data
            .phases
            .into_iter()
            .map(|(name, data)| {
                (
                    name,
                    PhaseCallbacks {
                        on_enter: data.on_enter,
                        on_update: data.on_update,
                        on_exit: data.on_exit,
                    },
                )
            })
            .collect();
        entity_commands.insert(LuaPhase::new(phase_data.initial, phases));
    }

    // Timer
    if let Some((duration, signal)) = cmd.timer {
        entity_commands.insert(Timer::new(duration, signal));
    }

    // SignalBinding
    if let Some((key, format)) = cmd.signal_binding {
        let mut binding = SignalBinding::new(&key);
        if let Some(fmt) = format {
            binding = binding.with_format(fmt);
        }
        entity_commands.insert(binding);
    }

    // GridLayout
    if let Some((path, group, zindex)) = cmd.grid_layout {
        entity_commands.insert(GridLayout::new(path, group, zindex));
    }

    // TweenPosition
    if let Some(tween_data) = cmd.tween_position {
        let easing = parse_easing(&tween_data.easing);
        let loop_mode = parse_loop_mode(&tween_data.loop_mode);
        entity_commands.insert(
            TweenPosition::new(
                Vector2 {
                    x: tween_data.from_x,
                    y: tween_data.from_y,
                },
                Vector2 {
                    x: tween_data.to_x,
                    y: tween_data.to_y,
                },
                tween_data.duration,
            )
            .with_easing(easing)
            .with_loop_mode(loop_mode),
        );
    }

    // TweenRotation
    if let Some(tween_data) = cmd.tween_rotation {
        let easing = parse_easing(&tween_data.easing);
        let loop_mode = parse_loop_mode(&tween_data.loop_mode);
        entity_commands.insert(
            TweenRotation::new(tween_data.from, tween_data.to, tween_data.duration)
                .with_easing(easing)
                .with_loop_mode(loop_mode),
        );
    }

    // TweenScale
    if let Some(tween_data) = cmd.tween_scale {
        let easing = parse_easing(&tween_data.easing);
        let loop_mode = parse_loop_mode(&tween_data.loop_mode);
        entity_commands.insert(
            TweenScale::new(
                Vector2 {
                    x: tween_data.from_x,
                    y: tween_data.from_y,
                },
                Vector2 {
                    x: tween_data.to_x,
                    y: tween_data.to_y,
                },
                tween_data.duration,
            )
            .with_easing(easing)
            .with_loop_mode(loop_mode),
        );
    }

    // Menu (Menu + MenuActions)
    if let Some(menu_data) = cmd.menu {
        let labels: Vec<(&str, &str)> = menu_data
            .items
            .iter()
            .map(|(id, label)| (id.as_str(), label.as_str()))
            .collect();

        let mut menu = Menu::new(
            &labels,
            Vector2 {
                x: menu_data.origin_x,
                y: menu_data.origin_y,
            },
            menu_data.font,
            menu_data.font_size,
            menu_data.item_spacing,
            menu_data.use_screen_space,
        );

        if let (Some(normal), Some(selected)) = (menu_data.normal_color, menu_data.selected_color) {
            menu = menu.with_colors(
                Color::new(normal.r, normal.g, normal.b, normal.a),
                Color::new(selected.r, selected.g, selected.b, selected.a),
            );
        }

        if let Some(dynamic) = menu_data.dynamic_text {
            menu = menu.with_dynamic_text(dynamic);
        }

        if let Some(sound) = menu_data.selection_change_sound {
            menu = menu.with_selection_sound(sound);
        }

        if let Some(cursor_key) = menu_data.cursor_entity_key {
            if let Some(cursor_entity) = worldsignals.get_entity(&cursor_key).copied() {
                menu = menu.with_cursor(cursor_entity);
            } else {
                eprintln!(
                    "[Rust] Menu cursor entity key '{}' not found in WorldSignals",
                    cursor_key
                );
            }
        }

        let mut actions = MenuActions::new();
        for (item_id, action_data) in menu_data.actions {
            let action = match action_data {
                crate::resources::lua_runtime::MenuActionData::SetScene { scene } => {
                    MenuAction::SetScene(scene)
                }
                crate::resources::lua_runtime::MenuActionData::ShowSubMenu { menu } => {
                    MenuAction::ShowSubMenu(menu)
                }
                crate::resources::lua_runtime::MenuActionData::QuitGame => MenuAction::QuitGame,
            };
            actions = actions.with(item_id, action);
        }

        entity_commands.insert((menu, actions));
    }

    // LuaCollisionRule
    if let Some(rule_data) = cmd.lua_collision_rule {
        entity_commands.insert(LuaCollisionRule::new(
            rule_data.group_a,
            rule_data.group_b,
            rule_data.callback,
        ));
    }

    // Animation
    if let Some(anim_data) = cmd.animation {
        entity_commands.insert(Animation::new(anim_data.animation_key));
    }

    // AnimationController
    if let Some(controller_data) = cmd.animation_controller {
        let mut controller = AnimationController::new(&controller_data.fallback_key);
        for rule in controller_data.rules {
            let condition = convert_animation_condition(rule.condition);
            controller = controller.with_rule(condition, rule.set_key);
        }
        entity_commands.insert(controller);
    }

    // StuckTo
    if let Some(stuckto_data) = cmd.stuckto {
        use crate::components::stuckto::StuckTo;
        let target = Entity::from_bits(stuckto_data.target_entity_id);
        let stuckto = StuckTo {
            target,
            offset: Vector2 {
                x: stuckto_data.offset_x,
                y: stuckto_data.offset_y,
            },
            follow_x: stuckto_data.follow_x,
            follow_y: stuckto_data.follow_y,
            stored_velocity: stuckto_data
                .stored_velocity
                .map(|(vx, vy)| Vector2 { x: vx, y: vy }),
        };
        entity_commands.insert(stuckto);
    }

    // Register entity in WorldSignals if requested
    if let Some(key) = cmd.register_as {
        worldsignals.set_entity(&key, entity);
    }
}

pub fn switch_scene(
    mut commands: Commands,
    mut audio_cmd_writer: bevy_ecs::prelude::MessageWriter<AudioCmd>,
    mut worldsignals: ResMut<WorldSignals>,
    //systems_store: Res<SystemsStore>,
    tilemaps_store: Res<TilemapStore>,
    tex_store: Res<TextureStore>,
    entities_to_clean: Query<Entity, Without<Persistent>>,
    mut tracked_groups: ResMut<TrackedGroups>,
    //mut rl: NonSendMut<raylib::RaylibHandle>,
    //th: NonSend<raylib::RaylibThread>,
    lua_runtime: NonSend<LuaRuntime>,
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

    // NOTE: tilemaps_store is NOT cleared - tilemaps are assets loaded during setup

    tracked_groups.clear();
    worldsignals.clear_group_counts();

    let scene = worldsignals
        .get_string("scene")
        .cloned()
        .unwrap_or_else(|| "menu".to_string());

    // Call Lua on_switch_scene function if it exists
    if lua_runtime.has_function("on_switch_scene") {
        if let Err(e) = lua_runtime.call_function::<_, ()>("on_switch_scene", scene.clone()) {
            eprintln!("[Rust] Error calling on_switch_scene: {}", e);
        }
    }

    // Process spawn commands from Lua
    for cmd in lua_runtime.drain_spawn_commands() {
        process_spawn_cmd(&mut commands, cmd, &mut worldsignals);
    }

    // Process group commands from Lua
    for cmd in lua_runtime.drain_group_commands() {
        match cmd {
            crate::resources::lua_runtime::GroupCmd::TrackGroup { name } => {
                tracked_groups.add_group(&name);
            }
            crate::resources::lua_runtime::GroupCmd::UntrackGroup { name } => {
                tracked_groups.remove_group(&name);
            }
            crate::resources::lua_runtime::GroupCmd::ClearTrackedGroups => {
                tracked_groups.clear();
            }
        }
    }

    // Update the tracked groups cache for Lua
    lua_runtime.update_tracked_groups_cache(&tracked_groups.groups);

    // Process tilemap commands from Lua
    for cmd in lua_runtime.drain_tilemap_commands() {
        match cmd {
            crate::resources::lua_runtime::TilemapCmd::SpawnTiles { id } => {
                if let Some(tilemap_info) = tilemaps_store.get(&id) {
                    // Get texture width for calculating tile offsets
                    if let Some(tilemap_tex) = tex_store.get(&id) {
                        let tiles_width = tilemap_tex.width;
                        spawn_tiles(&mut commands, &id, tiles_width, tilemap_info);
                        eprintln!("[Rust] Spawned tiles for tilemap '{}'", id);
                    } else {
                        eprintln!("[Rust] Tilemap texture '{}' not found", id);
                    }
                } else {
                    eprintln!("[Rust] Tilemap '{}' not found in store", id);
                }
            }
        }
    }

    // Process camera commands from Lua
    for cmd in lua_runtime.drain_camera_commands() {
        match cmd {
            crate::resources::lua_runtime::CameraCmd::SetCamera2D {
                target_x,
                target_y,
                offset_x,
                offset_y,
                rotation,
                zoom,
            } => {
                commands.insert_resource(Camera2DRes(Camera2D {
                    target: Vector2 {
                        x: target_x,
                        y: target_y,
                    },
                    offset: Vector2 {
                        x: offset_x,
                        y: offset_y,
                    },
                    rotation,
                    zoom,
                }));
                eprintln!(
                    "[Rust] Camera set to target ({}, {}), offset ({}, {})",
                    target_x, target_y, offset_x, offset_y
                );
            }
        }
    }

    /*     match scene.as_str() {
           "menu" => {
               // NOTE: Camera is now set by menu.lua spawn() function via engine.set_camera()
               // NOTE: Title entity with tweens is now spawned by menu.lua spawn() function
               // NOTE: Background sprite is now spawned by menu.lua spawn() function
               // NOTE: Cursor, Menu, MenuActions, and menu music are now spawned/played by menu.lua
           }
           "level01" => {
               // Phase callbacks are now handled via LuaPhase in level01.lua
               // The Lua script spawns a scene_phases entity with :with_phase({...})

               // ==================== COLLISION CALLBACKS ====================
               // NOTE: Collision rules are now handled via LuaCollisionRule in level01.lua
               // The Lua script spawns collision_rules entities with :with_lua_collision_rule()

               // ==================== SCENE SETUP ====================

               // NOTE: Score and lives are now reset by level01.lua spawn() function
               // NOTE: Tilemap is now loaded in setup() via Lua's engine.load_tilemap()
               // NOTE: Tiles are now spawned via Lua's engine.spawn_tiles() in level01.lua
               // NOTE: Bricks are now spawned via Lua's engine.spawn():with_grid_layout() in level01.lua

               // NOTE: Player is now spawned by level01.lua spawn() function
               // NOTE: Ball is now spawned by the "get_started" phase on_enter callback
               // NOTE: Walls are now spawned by level01.lua spawn() function
               // NOTE: Score UI texts are now spawned by level01.lua spawn() function
               // NOTE: Camera is now set by level01.lua spawn() function via engine.set_camera()

               // NOTE: Tracked groups (ball, brick) are now set up by level01.lua spawn()
           }
           "level02" => {}
           _ => {
               eprintln!("Unknown scene: {}", scene);
               panic!("Unknown scene");
           }
       }
    */
    // Stop any playing music when switching scenes
}
