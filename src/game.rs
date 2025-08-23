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
use crate::resources::animationstore::Animation;
use crate::resources::animationstore::AnimationStore;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::texturestore::TextureStore;
use rand::Rng;

/// Load textures, register resources, and spawn initial entities for the demo.
pub fn setup(world: &mut World, rl: &mut RaylibHandle, thread: &RaylibThread) {
    // Create and insert Camera2D resource (centered to current window size)
    let camera = Camera2D {
        target: Vector2 { x: 400.0, y: 225.0 },
        offset: Vector2 {
            x: rl.get_screen_width() as f32 * 0.5,
            y: rl.get_screen_height() as f32 * 0.5,
        },
        rotation: 0.0,
        zoom: 2.0,
    };
    world.insert_resource(Camera2DRes(camera));

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

    // Insert TextureStore resource
    let mut tex_store = TextureStore {
        map: FxHashMap::default(),
    };
    tex_store.insert("player-sheet", player_sheet_tex);
    tex_store.insert("player", player_tex);
    tex_store.insert("enemy", enemy_tex);
    world.insert_resource(tex_store);

    // Animations
    let mut anim_store = AnimationStore {
        animations: FxHashMap::default(),
    };
    anim_store.animations.insert(
        "player_idle".into(),
        Animation {
            tex_key: "player-sheet".into(),
            position: Vector2 { x: 0.0, y: 16.0 },
            displacement: 80.0, // width of each frame in the spritesheet
            frame_count: 8,
            fps: 6.0, // speed of the animation
            looped: true,
        },
    );
    world.insert_resource(anim_store);

    // Player
    world.spawn((
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
    world.spawn((
        Group::new("player-animation"),
        MapPosition::new(400.0, 225.0),
        ZIndex(1),
        Sprite {
            tex_key: "player-sheet".into(),
            width: 80.0, // width of the sprite frame in the spritesheet
            height: 32.0,
            offset: Vector2 { x: 0.0, y: 16.0 }, // offset to match the sprite frame in the spritesheet
            origin: Vector2 { x: 40.0, y: 32.0 },
        },
        AnimationComponent {
            animation_key: "player_idle".into(),
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

        world.spawn((
            Group::new("enemy"),
            MapPosition::new(50.0 + (i as f32 * 64.0), 164.0 + (i as f32 * 16.0)),
            ZIndex(i % 5),
            Sprite {
                tex_key: "enemy".into(),
                width: enemy_tex_width as f32,
                height: enemy_tex_height as f32,
                offset: Vector2::zero(),
                origin: Vector2::zero(),
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
}
