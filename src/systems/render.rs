use bevy_ecs::prelude::*;
use raylib::prelude::*;

use crate::components::mapposition::MapPosition;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::resources::camera2d::Camera2DRes;
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
                // World-space sprite AABB centered on Position
                let half_w = s.width * 0.5;
                let half_h = s.height * 0.5;
                let min = Vector2 {
                    x: p.pos.x - half_w,
                    y: p.pos.y - half_h,
                };
                let max = Vector2 {
                    x: p.pos.x + half_w,
                    y: p.pos.y + half_h,
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
        if let Some(tex) = textures.map.get(sprite.tex_key) {
            // Source rect selects a frame from the spritesheet
            let src = Rectangle {
                x: sprite.offset_x,
                y: sprite.offset_y,
                width: sprite.width,
                height: sprite.height,
            };

            // Destination rect places and scales the sprite in world coords
            let dest = Rectangle {
                x: pos.pos.x,
                y: pos.pos.y,
                width: sprite.width,
                height: sprite.height,
            };

            // Center origin so position is at sprite center
            let origin = Vector2 {
                x: sprite.width * 0.5,
                y: sprite.height * 0.5,
            };

            d2.draw_texture_pro(tex, src, dest, origin, 0.0, Color::WHITE);
        }
    }
}
