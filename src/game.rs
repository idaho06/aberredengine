use bevy_ecs::prelude::*;
use raylib::prelude::*;
use std::collections::HashMap;

// Import component/resource types from modules
use crate::components::boxcollider::BoxCollider;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
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
        zoom: 1.0,
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

    // Insert TextureStore resource
    let mut tex_store = TextureStore {
        map: HashMap::new(),
    };
    tex_store.map.insert("player", player_tex);
    tex_store.map.insert("enemy", enemy_tex);
    world.insert_resource(tex_store);

    // Player
    world.spawn((
        Group("player"),
        MapPosition::new(40.0, 40.0),
        ZIndex(0),
        Sprite {
            tex_key: "player",
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

    // Enemies
    let mut rng = rand::thread_rng();
    for i in 0..30 {
        // Random velocity components in a small range
        let vx = rng.gen_range(-40.0f32..40.0f32);
        let vy = rng.gen_range(-20.0f32..20.0f32);

        world.spawn((
            Group("enemy"),
            MapPosition::new(50.0 + (i as f32 * 64.0), 164.0 + (i as f32 * 16.0)),
            ZIndex(i % 5),
            Sprite {
                tex_key: "enemy",
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
