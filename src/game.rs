use bevy_ecs::prelude::*;
use raylib::prelude::*;
use rustc_hash::FxHashMap;
//use std::collections::HashMap;

// Import component/resource types from modules
use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::group::Group;
use crate::components::inputcontrolled::InputControlled;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::AnimationResource;
use crate::resources::animationstore::AnimationStore;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::gamestate::{GameStates, NextGameState};
use crate::resources::texturestore::TextureStore;
use crate::resources::tilemapstore::{Tilemap, TilemapStore};
use crate::resources::worldtime::WorldTime;
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
                x: tile_size * 0.5,
                y: tile_size * 0.5,
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

pub fn setup(
    mut commands: Commands,
    mut next_state: ResMut<NextGameState>,
    mut rl: NonSendMut<raylib::RaylibHandle>,
    th: NonSend<raylib::RaylibThread>,
    // audio_bridge: ResMut<AudioBridge>,
    mut audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    // This function sets up the game world, loading resources

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
    let dummy_tex = rl
        .load_texture(&th, "./assets/textures/player.png")
        .expect("load assets/player.png");

    let enemy_tex = rl
        .load_texture(&th, "./assets/textures/enemy.png")
        .expect("load assets/enemy.png");

    let player_sheet_tex = rl
        .load_texture(&th, "./assets/textures/WarriorMan-Sheet.png")
        .expect("load assets/WarriorMan-Sheet.png");

    // Load tilemap textures and data
    let (tilemap_tex, tilemap) = load_tilemap(&mut rl, &th, "./assets/tilemaps/maptest04");
    //let tilemap_tex_width = tilemap_tex.width;
    let mut tilemaps_store = TilemapStore::new();
    tilemaps_store.insert("tilemap", tilemap);
    commands.insert_resource(tilemaps_store);

    // Insert TextureStore resource
    let mut tex_store = TextureStore::new();
    tex_store.insert("player-sheet", player_sheet_tex);
    tex_store.insert("dummy", dummy_tex);
    tex_store.insert("enemy", enemy_tex);
    tex_store.insert("tilemap", tilemap_tex);
    commands.insert_resource(tex_store);

    // Animations
    let mut anim_store = AnimationStore {
        animations: FxHashMap::default(),
    };
    anim_store.animations.insert(
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
    );
    commands.insert_resource(anim_store);

    // Send messages to load musics and sound effects via ECS Messages<AudioCmd>
    audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "music1".into(),
        path: "./assets/audio/chiptun1.mod".into(),
    });
    audio_cmd_writer.write(AudioCmd::LoadMusic {
        id: "music2".into(),
        path: "./assets/audio/mini1111.xm".into(),
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

pub fn enter_play(
    mut commands: Commands,
    //mut next_state: ResMut<NextGameState>,
    //mut rl: NonSendMut<raylib::RaylibHandle>,
    //th: NonSend<raylib::RaylibThread>,
    // audio_bridge: ResMut<AudioBridge>,
    mut audio_cmd_writer: bevy_ecs::prelude::MessageWriter<AudioCmd>,
    tex_store: Res<TextureStore>,
    tilemaps_store: Res<TilemapStore>, // TODO: Make it optional
) {
    // Get Texture sizes
    let dummy_tex = tex_store.get("dummy").expect("dummy texture not found");
    let dummy_tex_width = dummy_tex.width;
    let dummy_tex_height = dummy_tex.height;

    let enemy_tex = tex_store.get("enemy").expect("enemy texture not found");
    let enemy_tex_width = enemy_tex.width;
    let enemy_tex_height = enemy_tex.height;

    let tilemap_tex = tex_store.get("tilemap").expect("tilemap texture not found");
    let tilemap_tex_width = tilemap_tex.width;
    let tilemap = tilemaps_store
        .get("tilemap")
        .expect("tilemap info not found");

    // Dummy player
    commands.spawn((
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
    ));

    // Player animation flipped
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
        Animation {
            animation_key: "player_walk".into(),
            frame_index: 0,
            elapsed_time: 0.0,
        },
    ));
    // Player animation controlled
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
        Animation {
            animation_key: "player_stand".into(),
            frame_index: 0,
            elapsed_time: 0.0,
        },
        InputControlled::new(
            Vector2 { x: 0.0, y: -32.0 }, // up
            Vector2 { x: 0.0, y: 32.0 },  // down
            Vector2 { x: -32.0, y: 0.0 }, // left
            Vector2 { x: 32.0, y: 0.0 },  // right
        ),
        RigidBody::default(),
    ));

    // Enemies
    let mut rng = rand::thread_rng();
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
    }

    // Create map tiles as spawns of MapPosition, Zindex, and Sprite
    spawn_tiles(&mut commands, "tilemap", tilemap_tex_width, tilemap);

    // play music2 looped via ECS messages
    audio_cmd_writer.write(AudioCmd::PlayMusic {
        id: "music2".into(),
        looped: true,
    });
    // TODO: music_play(world, "music2".into(), true);
}

pub fn update(
    time: Res<WorldTime>,
    // mut _query_rb: Query<(&mut MapPosition, &mut RigidBody, &BoxCollider), With<Group>>,
    mut query_enemies: Query<(&mut Sprite, &RigidBody), With<Group>>,
    //mut query_player: Query<(&mut Sprite, &RigidBody), With<Group>>,
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
    for (mut sprite, rb) in query_enemies.iter_mut() {
        if rb.velocity.x < 0.0 {
            sprite.flip_h = true;
        } else if rb.velocity.x > 0.0 {
            sprite.flip_h = false;
        }
    }
}
