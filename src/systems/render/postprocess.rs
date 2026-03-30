use bevy_ecs::prelude::*;
use log::{error, warn};
use raylib::ffi;
use raylib::prelude::*;
use rustc_hash::FxHashMap;

use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::rendertarget::RenderTarget;
use crate::resources::screensize::ScreenSize;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::uniformvalue::UniformValue;
use crate::resources::windowsize::WindowSize;
use crate::resources::worldtime::WorldTime;

use super::SourceBuffer;

/// Apply post-processing shader passes and blit the final image to the window.
///
/// Handles three cases: no shaders (direct blit), single shader, and multi-pass
/// ping-pong. Always guarantees a frame is presented even if shaders are missing
/// or invalid.
///
/// `post_blit` is an optional callback invoked inside `begin_drawing()` after
/// the final blit, used to draw imgui overlays at window resolution.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_postprocess_passes<F: FnOnce(&RaylibDrawHandle<'_>)>(
    rl: &mut RaylibHandle,
    th: &RaylibThread,
    render_target: &mut RenderTarget,
    shader_store: &mut ShaderStore,
    post_process: &PostProcessShader,
    world_time: &WorldTime,
    screensize: &ScreenSize,
    window_size: &WindowSize,
    mut post_blit: Option<F>,
) {
    // Source rectangle (the entire render target, Y-flipped for OpenGL)
    let src = render_target.source_rect();

    // Destination rectangle (letterboxed to fit window)
    let dest = window_size.calculate_letterbox(render_target.game_width, render_target.game_height);

    // Full-screen destination for intermediate passes (no letterboxing)
    let full_dest = Rectangle {
        x: 0.0,
        y: 0.0,
        width: render_target.game_width as f32,
        height: render_target.game_height as f32,
    };

    // Clone shader chain to avoid borrowing issues
    let shader_chain: Vec<_> = post_process.keys.to_vec();

    if shader_chain.is_empty() {
        // No post-processing - draw directly to window
        blit_to_window(rl, th, &render_target.texture, src, dest, post_blit.take());
    } else {
        // Multi-pass: ensure ping-pong buffers exist
        if let Err(e) = render_target.ensure_ping_pong_buffers(rl, th) {
            error!("Failed to create ping-pong buffers: {}", e);
            // Fallback: draw without shader
            blit_to_window(rl, th, &render_target.texture, src, dest, post_blit.take());
            return;
        }

        let mut source_buffer = SourceBuffer::Main;
        let mut valid_passes = 0;
        let mut final_blit_done = false;

        // Get raw pointers to independently borrow texture, ping, and pong
        // SAFETY: These fields are independent and don't alias
        let main_tex_ptr = &render_target.texture as *const RenderTexture2D;
        let ping_tex_ptr = render_target.ping.as_ref().unwrap() as *const RenderTexture2D;
        let pong_tex_ptr = render_target.pong.as_ref().unwrap() as *const RenderTexture2D;

        for (i, shader_key) in shader_chain.iter().enumerate() {
            let is_last_pass = i == shader_chain.len() - 1;

            // Validate shader exists and is valid
            let shader_valid = shader_store
                .get(shader_key.as_ref())
                .map(|e| e.shader.is_shader_valid())
                .unwrap_or(false);

            if !shader_valid {
                if shader_store.get(shader_key.as_ref()).is_none() {
                    warn!("Shader '{}' not found, skipping pass", shader_key);
                } else {
                    warn!("Shader '{}' invalid, skipping pass", shader_key);
                }
                continue;
            }

            // Choose the correct destination rect for uniforms:
            // intermediate passes render to full game-resolution buffers,
            // only the final pass renders to the letterboxed window rect.
            let uniform_dest = if is_last_pass { &dest } else { &full_dest };

            // Set uniforms
            if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                set_standard_uniforms(
                    &mut entry.shader,
                    &mut entry.locations,
                    world_time,
                    screensize,
                    window_size,
                    uniform_dest,
                );
                for (name, value) in post_process.uniforms.iter() {
                    set_uniform_value(&mut entry.shader, &mut entry.locations, name, value);
                }
            }

            // SAFETY: We're only reading from source_tex and writing to dest_tex,
            // and they never alias (main->ping, ping->pong, pong->ping, etc.)
            let source_tex: &RenderTexture2D = unsafe {
                match source_buffer {
                    SourceBuffer::Main => &*main_tex_ptr,
                    SourceBuffer::Ping => &*ping_tex_ptr,
                    SourceBuffer::Pong => &*pong_tex_ptr,
                }
            };

            if is_last_pass {
                // Draw to window
                let mut d = rl.begin_drawing(th);
                d.clear_background(Color::BLACK);

                if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                    let mut d_shader = d.begin_shader_mode(&mut entry.shader);
                    d_shader.draw_texture_pro(
                        source_tex,
                        src,
                        dest,
                        Vector2 { x: 0.0, y: 0.0 },
                        0.0,
                        Color::WHITE,
                    );
                }
                if let Some(f) = post_blit.take() {
                    f(&d);
                }
                final_blit_done = true;
            } else {
                // Draw to intermediate buffer
                // Choose destination buffer (opposite of source for ping-pong)
                let write_to_ping =
                    matches!(source_buffer, SourceBuffer::Main | SourceBuffer::Pong);

                if write_to_ping {
                    let dest_tex = render_target.ping.as_mut().unwrap();
                    let mut d = rl.begin_texture_mode(th, dest_tex);
                    d.clear_background(Color::BLACK);

                    if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                        let mut d_shader = d.begin_shader_mode(&mut entry.shader);
                        d_shader.draw_texture_pro(
                            source_tex,
                            src,
                            full_dest,
                            Vector2 { x: 0.0, y: 0.0 },
                            0.0,
                            Color::WHITE,
                        );
                    }
                    source_buffer = SourceBuffer::Ping;
                } else {
                    let dest_tex = render_target.pong.as_mut().unwrap();
                    let mut d = rl.begin_texture_mode(th, dest_tex);
                    d.clear_background(Color::BLACK);

                    if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                        let mut d_shader = d.begin_shader_mode(&mut entry.shader);
                        d_shader.draw_texture_pro(
                            source_tex,
                            src,
                            full_dest,
                            Vector2 { x: 0.0, y: 0.0 },
                            0.0,
                            Color::WHITE,
                        );
                    }
                    source_buffer = SourceBuffer::Pong;
                }
            }

            valid_passes += 1;
        }

        // Ensure a frame is always presented to the window.
        // This handles two cases:
        // 1. No valid passes ran at all → blit the original scene unshaded
        // 2. Some passes ran but the last shader was invalid → blit the
        //    latest intermediate result (in source_buffer) unshaded
        if !final_blit_done {
            // SAFETY: source_tex points to a buffer independent of the write target (window)
            let source_tex: &RenderTexture2D = unsafe {
                match source_buffer {
                    SourceBuffer::Main => &*main_tex_ptr,
                    SourceBuffer::Ping => &*ping_tex_ptr,
                    SourceBuffer::Pong => &*pong_tex_ptr,
                }
            };
            if valid_passes > 0 {
                warn!(
                    "Last shader in post-process chain was invalid; \
                     blitting last valid intermediate result without shader"
                );
            }
            blit_to_window(rl, th, source_tex, src, dest, post_blit.take());
        }
    }
}

/// Blit a render texture to the window with optional post-blit callback.
pub(super) fn blit_to_window<F: FnOnce(&RaylibDrawHandle<'_>)>(
    rl: &mut RaylibHandle,
    th: &RaylibThread,
    tex: &RenderTexture2D,
    src: Rectangle,
    dest: Rectangle,
    post_blit: Option<F>,
) {
    let mut d = rl.begin_drawing(th);
    d.clear_background(Color::BLACK);
    d.draw_texture_pro(
        tex,
        src,
        dest,
        Vector2 { x: 0.0, y: 0.0 },
        0.0,
        Color::WHITE,
    );
    if let Some(f) = post_blit {
        f(&d);
    }
}

/// Get or cache a uniform location by name.
pub(super) fn get_uniform_loc(
    shader: &Shader,
    locations: &mut FxHashMap<String, i32>,
    name: &str,
) -> i32 {
    *locations
        .entry(name.to_string())
        .or_insert_with(|| shader.get_shader_location(name))
}

/// Set standard uniforms on a shader for post-processing.
///
/// Standard uniforms:
/// - uTime: elapsed time in seconds
/// - uDeltaTime: frame delta time in seconds
/// - uResolution: render target resolution (game resolution)
/// - uFrame: frame count
/// - uWindowResolution: window resolution
/// - uLetterbox: letterbox destination rectangle (x, y, w, h)
pub(super) fn set_standard_uniforms(
    shader: &mut Shader,
    locations: &mut FxHashMap<String, i32>,
    world_time: &WorldTime,
    screensize: &ScreenSize,
    window_size: &WindowSize,
    dest: &Rectangle,
) {
    // uTime (float)
    let loc = get_uniform_loc(shader, locations, "uTime");
    if loc >= 0 {
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                &world_time.elapsed as *const f32 as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
            );
        }
    }

    // uDeltaTime (float)
    let loc = get_uniform_loc(shader, locations, "uDeltaTime");
    if loc >= 0 {
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                &world_time.delta as *const f32 as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
            );
        }
    }

    // uResolution (vec2) - game resolution
    let loc = get_uniform_loc(shader, locations, "uResolution");
    if loc >= 0 {
        let resolution = [screensize.w as f32, screensize.h as f32];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                resolution.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }

    // uFrame (int)
    let loc = get_uniform_loc(shader, locations, "uFrame");
    if loc >= 0 {
        let frame = world_time.frame_count as i32;
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                &frame as *const i32 as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_INT as i32,
            );
        }
    }

    // uWindowResolution (vec2)
    let loc = get_uniform_loc(shader, locations, "uWindowResolution");
    if loc >= 0 {
        let window_res = [window_size.w as f32, window_size.h as f32];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                window_res.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }

    // uLetterbox (vec4) - destination rectangle
    let loc = get_uniform_loc(shader, locations, "uLetterbox");
    if loc >= 0 {
        let letterbox = [dest.x, dest.y, dest.width, dest.height];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                letterbox.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC4 as i32,
            );
        }
    }
}

/// Set a user-defined uniform value on a shader.
pub(super) fn set_uniform_value(
    shader: &mut Shader,
    locations: &mut FxHashMap<String, i32>,
    name: &str,
    value: &UniformValue,
) {
    let loc = get_uniform_loc(shader, locations, name);

    if loc < 0 {
        return; // Uniform not found in shader, silently skip
    }

    unsafe {
        match value {
            UniformValue::Float(v) => {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    v as *const f32 as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
                );
            }
            UniformValue::Int(v) => {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    v as *const i32 as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_INT as i32,
                );
            }
            UniformValue::Vec2 { x, y } => {
                let vec = [*x, *y];
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    vec.as_ptr() as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
                );
            }
            UniformValue::Vec4 { x, y, z, w } => {
                let vec = [*x, *y, *z, *w];
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    vec.as_ptr() as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC4 as i32,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// Set entity-specific uniforms on a shader for per-entity rendering.
///
/// Entity-specific uniforms:
/// - uEntityId (int) - entity index
/// - uEntityPos (vec2) - world position
/// - uSpriteSize (vec2) - entity dimensions (sprite size or text bounding box)
/// - uRotation (float) - rotation degrees (if present)
/// - uScale (vec2) - scale factor (if present)
/// - uVelocity (vec2) - velocity (if RigidBody present)
pub(super) fn set_entity_uniforms(
    shader: &mut Shader,
    locations: &mut FxHashMap<String, i32>,
    entity: Entity,
    pos: &MapPosition,
    rotation: Option<&Rotation>,
    scale: Option<&Scale>,
    size: Vector2,
    rigidbody_query: &Query<&RigidBody>,
) {
    // uEntityId (int) - use bits representation truncated to i32
    let loc = get_uniform_loc(shader, locations, "uEntityId");
    if loc >= 0 {
        let entity_id = (entity.to_bits() & 0xFFFFFFFF) as i32;
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                &entity_id as *const i32 as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_INT as i32,
            );
        }
    }

    // uEntityPos (vec2)
    let loc = get_uniform_loc(shader, locations, "uEntityPos");
    if loc >= 0 {
        let entity_pos = [pos.pos.x, pos.pos.y];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                entity_pos.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }

    // uSpriteSize (vec2)
    let loc = get_uniform_loc(shader, locations, "uSpriteSize");
    if loc >= 0 {
        let sprite_size = [size.x, size.y];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                sprite_size.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }

    // uRotation (float) - only if Rotation component present
    if let Some(rot) = rotation {
        let loc = get_uniform_loc(shader, locations, "uRotation");
        if loc >= 0 {
            unsafe {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    &rot.degrees as *const f32 as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
                );
            }
        }
    }

    // uScale (vec2) - only if Scale component present
    if let Some(s) = scale {
        let loc = get_uniform_loc(shader, locations, "uScale");
        if loc >= 0 {
            let scale_vec = [s.scale.x, s.scale.y];
            unsafe {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    scale_vec.as_ptr() as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
                );
            }
        }
    }

    // uVelocity (vec2) - only if RigidBody component present
    if let Ok(rb) = rigidbody_query.get(entity) {
        let loc = get_uniform_loc(shader, locations, "uVelocity");
        if loc >= 0 {
            let velocity = [rb.velocity.x, rb.velocity.y];
            unsafe {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    velocity.as_ptr() as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
                );
            }
        }
    }
}
