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

            // Displace x to the right and set width to negative to flip the sprite horizontally
            // src.x += src.width;
            // src.width = -src.width;
            // src.y += src.height;
            // src.height = -src.height;
            // TODO: This works, but it messes up the animation. Study a way to fix this.

            // Destination rect places sprite so that MapPosition is the pivot (origin)
            let dest = Rectangle {
                x: pos.pos.x,
                y: pos.pos.y,
                width: sprite.width,
                height: sprite.height,
            };

            // dest.x += dest.width;
            // dest.width = -dest.width;
            // This does not work for horizontal flipping

            // Use Sprite.origin directly for rendering pivot
            d2.draw_texture_pro(tex, src, dest, sprite.origin, 0.0, Color::WHITE);
        }
    }

    if world.contains_resource::<DebugMode>() {
        // Render debug UI elements
        // query for all BoxColliders
        let mut colliders = world.query::<(&BoxCollider, &MapPosition)>();
        for (collider, position) in colliders.iter(world) {
            // Draw the collider's AABB
            let (x, y, w, h) = collider.get_aabb(position.pos);

            d2.draw_rectangle_lines(x as i32, y as i32, w as i32, h as i32, Color::RED);
        }
        let mut positions = world.query::<&MapPosition>();
        for position in positions.iter(world) {
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

pub fn render_debug_ui(world: &mut World, d: &mut RaylibDrawHandle) {
    if world.contains_resource::<DebugMode>() {
        let screen = *world.resource::<ScreenSize>();

        let debug_text = "DEBUG MODE (press F11 to toggle)";

        let fps = d.get_fps();
        let text = format!("{} | FPS: {}", debug_text, fps);
        d.draw_text(&text, 10, 10, 10, Color::BLACK);

        let entity_count = world.iter_entities().count();
        let text = format!("Entities: {}", entity_count);
        d.draw_text(&text, 10, 30, 10, Color::BLACK);

        let cam = world.resource::<Camera2DRes>().0;
        let cam_text = format!(
            "Camera pos: ({:.1}, {:.1}) Zoom: {:.2}",
            cam.target.x, cam.target.y, cam.zoom
        );
        d.draw_text(&cam_text, 10, (screen.h - 30) as i32, 10, Color::BLACK);

        //d.gui_window_box(Rectangle::new(10.0, 50.0, 200.0, 100.0), "Debug Info");

        let mouse_pos = d.get_mouse_position();
        let mouse_world = d.get_screen_to_world2D(mouse_pos, cam);

        let mouse_text = format!(
            "Mouse screen: ({:.1}, {:.1}) World: ({:.1}, {:.1})",
            mouse_pos.x, mouse_pos.y, mouse_world.x, mouse_world.y
        );

        // d.gui_panel(Rectangle::new(10.0, 50.0, 300.0, 100.0), "Debug Info");

        d.draw_text(&mouse_text, 10, 70, 10, Color::BLACK);

        // let mut forceSquaredChecked = &mut false;

        // d.gui_check_box(
        //     Rectangle::new(25.0, 108.0, 15.0, 15.0),
        //     "FORCE CHECK!",
        //     &mut forceSquaredChecked,
        // );
    }
}
