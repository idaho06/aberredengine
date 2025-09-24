use bevy_ecs::prelude::*;
use raylib::prelude::*;
use rustc_hash::FxHashMap;
//use std::collections::HashMap;

// Import component/resource types from modules
use crate::components::animation::AnimationComponent;
use crate::components::boxcollider::BoxCollider;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::Animation;
use crate::resources::animationstore::AnimationStore;
use crate::resources::audio::AudioBridge;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::gamestate::{GameStates, NextGameState};
use crate::resources::texturestore::TextureStore;
use crate::resources::tilemap::Tilemap;
use rand::Rng;

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

fn spawn_tilemaps(
    commands: &mut Commands,
    tilemap_key: impl Into<String>,
    tex_width: i32,
    tilemap: Tilemap,
) {
    let tilemap_key: String = tilemap_key.into();

    // texture size in pixels
    let tex_w = tex_width as f32;

    let tile_size = tilemap.tile_size as f32;

    // how many tiles per row in the texture
    let tiles_per_row = ((tex_w / tile_size).floor() as u32).max(1);

    let layer_count = tilemap.layers.len() as i32;
    // iterate layers and spawn tiles; ZIndex: if N layers, first is -N, last is -1
    for (layer_index, layer) in tilemap.layers.into_iter().enumerate() {
        let z = -(layer_count - (layer_index as i32));

        for pos in layer.positions.into_iter() {
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
                x: tile_size * 0.5,
                y: tile_size * 0.5,
            };

            commands.spawn((
                Group::new("tiles"),
                MapPosition::new(wx, wy),
                ZIndex(z),
                Sprite {
                    tex_key: tilemap_key.clone(),
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

pub fn setup(
    mut commands: &mut Commands,
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    audio_bridge: &mut AudioBridge,
    next_state: &mut NextGameState,
) {
    eprintln!("Game setup_with_commands()");

    let camera = Camera2D {
        target: Vector2 {
            x: 400.0,
            y: 225.0, //x: 0.0,
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

    // Load textures
    let player_tex = rl
        .load_texture(thread, "./assets/textures/player.png")
        .expect("load assets/player.png");
    let player_tex_width = player_tex.width;
    let player_tex_height = player_tex.height;

    let enemy_tex = rl
        .load_texture(thread, "./assets/textures/enemy.png")
        .expect("load assets/enemy.png");
    let enemy_tex_width = enemy_tex.width;
    let enemy_tex_height = enemy_tex.height;

    let player_sheet_tex = rl
        .load_texture(thread, "./assets/textures/WarriorMan-Sheet.png")
        .expect("load assets/WarriorMan-Sheet.png");

    // Load tilemap textures and data
    let (tilemap_tex, tilemap) = load_tilemap(rl, thread, "./assets/tilemaps/maptest04");
    let tilemap_tex_width = tilemap_tex.width;

    // Insert TextureStore resource
    let mut tex_store = TextureStore {
        map: FxHashMap::default(),
    };
    tex_store.insert("player-sheet", player_sheet_tex);
    tex_store.insert("player", player_tex);
    tex_store.insert("enemy", enemy_tex);
    tex_store.insert("tilemap", tilemap_tex);
    commands.insert_resource(tex_store);

    // Animations
    let mut anim_store = AnimationStore {
        animations: FxHashMap::default(),
    };
    anim_store.animations.insert(
        "player_tired".into(),
        Animation {
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
        Animation {
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
        Animation {
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
        Animation {
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
        Animation {
            tex_key: "player-sheet".into(),
            position: Vector2 { x: 0.0, y: 272.0 },
            displacement: 80.0, // width of each frame in the spritesheet
            frame_count: 8 + 3,
            fps: 12.0, // speed of the animation
            looped: true,
        },
    );
    commands.insert_resource(anim_store);

    // Player
    commands.spawn((
        Group::new("player"),
        MapPosition::new(40.0, 40.0),
        ZIndex(0),
        Sprite {
            tex_key: "player".into(),
            width: player_tex_width as f32,
            height: player_tex_height as f32,
            offset: Vector2::zero(),
            origin: Vector2 {
                x: player_tex_width as f32 * 0.5,
                y: player_tex_height as f32,
            }, // origin at the feet of the player sprite
            flip_h: false,
            flip_v: false,
        },
        BoxCollider {
            size: Vector2 {
                x: player_tex_width as f32 * 0.5,
                y: player_tex_height as f32 * 0.5,
            },
            offset: Vector2 {
                x: player_tex_width as f32 * 0.25,
                y: player_tex_height as f32 * 0.25,
            },
            // Match collider pivot to sprite's origin (feet) to align positions
            origin: Vector2 {
                x: player_tex_width as f32 * 0.5,
                y: player_tex_height as f32,
            },
        },
    ));

    // Player animations
    commands.spawn((
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
        AnimationComponent {
            animation_key: "player_walk".into(),
            frame_index: 0,
            elapsed_time: 0.0,
        },
    ));
    commands.spawn((
        Group::new("player-animation"),
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
        AnimationComponent {
            animation_key: "player_walk".into(),
            frame_index: 0,
            elapsed_time: 0.0,
        },
    ));

    // Enemies
    let mut rng = rand::thread_rng();
    for i in 0..30 {
        // Random velocity components in a small range
        let vx = rng.gen_range(-40.0f32..40.0f32);
        let vy = rng.gen_range(-20.0f32..20.0f32);

        let flip_h = vx < 0.0;

        commands.spawn((
            Group::new("enemy"),
            MapPosition::new(50.0 + (i as f32 * 64.0), 164.0 + (i as f32 * 16.0)),
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
    }

    // Create map tiles as spawns of MapPosition, Zindex, and Sprite
    spawn_tilemaps(&mut commands, "tilemap", tilemap_tex_width, tilemap);

    // Send messages to load musics
    {
        let _ = audio_bridge.tx_cmd.send(AudioCmd::Load {
            id: "music1".into(),
            path: "./assets/audio/chiptun1.mod".into(),
        });
        let _ = audio_bridge.tx_cmd.send(AudioCmd::Load {
            id: "music2".into(),
            path: "./assets/audio/mini1111.xm".into(),
        });
    }

    // TODO: music_load(world, "music1".into(), "./assets/audio/chiptun1.mod".into());
    // TODO: music_load(world, "music2".into(), "./assets/audio/mini1111.xm".into());

    // block until both audio files are loaded
    {
        let _ = audio_bridge.rx_evt.recv();
        let _ = audio_bridge.rx_evt.recv();
    }

    // play music2 looped
    {
        let _ = audio_bridge.tx_cmd.send(AudioCmd::Play {
            id: "music2".into(),
            looped: true,
        });
    }
    // TODO: music_play(world, "music2".into(), true);

    // Change GameState to Playing
    next_state.set(GameStates::Playing);
    eprintln!("Game setup_with_commands() done, next state set to Playing");
}
