use bevy_ecs::prelude::*;
use raylib::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::mapposition::MapPosition;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::debugmode::DebugMode;
use crate::resources::screensize::ScreenSize;
use crate::resources::texturestore::TextureStore;

/// We render inside raylib's drawing scopes and query the ECS World.
/// For culling we compute the world-rect visible by the camera using
/// Camera2D::screen_to_world and then do AABB intersection.
pub fn render_pass(
    world: &mut World,
    d2: &mut RaylibMode2D<RaylibDrawHandle>, // drawing in 2D camera space
) {
    // Pull resources we need
    let cam = world.resource::<Camera2DRes>().0;
    let screen = *world.resource::<ScreenSize>();

    // Compute visible world-rectangle (camera view) from screen corners.
    // Use raylib's helper on the draw handle to convert screen->world.
    // We'll build an axis-aligned world rect from TL (0,0) and BR (w,h).
    let tl = d2.get_screen_to_world2D(Vector2 { x: 0.0, y: 0.0 }, cam);
    let br = d2.get_screen_to_world2D(
        Vector2 {
            x: screen.w as f32,
            y: screen.h as f32,
        },
        cam,
    );
    let view_min = Vector2 {
        x: tl.x.min(br.x),
        y: tl.y.min(br.y),
    };
    let view_max = Vector2 {
        x: tl.x.max(br.x),
        y: tl.y.max(br.y),
    };

    // Query: (Sprite, Position, ZIndex)
    // We'll collect, sort by z, then draw.
    let mut to_draw: Vec<(Sprite, MapPosition, ZIndex)> = {
        let mut q = world.query::<(&Sprite, &MapPosition, &ZIndex)>();
        q.iter(world)
            .filter_map(|(s, p, z)| {
                // World-space sprite AABB with MapPosition representing the pivot (origin)
                // The AABB min/max are computed from the position minus origin to position minus origin plus size.
                let min = Vector2 {
                    x: p.pos.x - s.origin.x,
                    y: p.pos.y - s.origin.y,
                };
                let max = Vector2 {
                    x: min.x + s.width,
                    y: min.y + s.height,
                };

                // Cull against camera's world rect
                let overlap = !(max.x < view_min.x
                    || min.x > view_max.x
                    || max.y < view_min.y
                    || min.y > view_max.y);
                if overlap {
                    Some((s.clone(), *p, *z))
                } else {
                    None
                }
            })
            .collect()
    };

    to_draw.sort_by_key(|(_, _, z)| *z);

    let textures = world.resource::<TextureStore>();

    for (sprite, pos, _z) in to_draw.iter() {
        //if let Some(tex) = textures.map.get(sprite.tex_key.as_str()) {
        if let Some(tex) = textures.get(sprite.tex_key.clone()) {
            // Source rect selects a frame from the spritesheet
            let src = Rectangle {
                x: sprite.offset.x,
                y: sprite.offset.y,
                width: sprite.width,
                height: sprite.height,
            };

            // Destination rect places sprite so that MapPosition is the pivot (origin)
            let dest = Rectangle {
                x: pos.pos.x,
                y: pos.pos.y,
                width: sprite.width,
                height: sprite.height,
            };

            // Use Sprite.origin directly for rendering pivot
            d2.draw_texture_pro(tex, src, dest, sprite.origin, 0.0, Color::WHITE);
        }
    }

    if world.contains_resource::<DebugMode>() {
        // Render debug UI elements
        // query for all BoxColliders and their MapPositions
        let mut colliders = world.query::<(&BoxCollider, &MapPosition)>();
        for (collider, position) in colliders.iter(world) {
            // Draw the collider's AABB
            let (x, y, w, h) = collider.get_aabb(position.pos);

            d2.draw_rectangle_lines(x as i32, y as i32, w as i32, h as i32, Color::RED);
            // Draw a small cross in the MapPosition
            d2.draw_line(
                position.pos.x as i32 - 5,
                position.pos.y as i32,
                position.pos.x as i32 + 5,
                position.pos.y as i32,
                Color::GREEN,
            );
            d2.draw_line(
                position.pos.x as i32,
                position.pos.y as i32 - 5,
                position.pos.x as i32,
                position.pos.y as i32 + 5,
                Color::GREEN,
            );
        }
    }
}
