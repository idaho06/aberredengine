use bevy_ecs::prelude::*;
use log::{error, warn};
use raylib::ffi;
use raylib::prelude::*;
use rustc_hash::FxHashMap;

use crate::components::mapposition::MapPosition;
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

        let Some(ping_tex) = render_target.ping.as_ref() else {
            error!(
                "Post-process ping buffer missing after initialization; falling back to direct blit"
            );
            blit_to_window(rl, th, &render_target.texture, src, dest, post_blit.take());
            return;
        };
        let Some(pong_tex) = render_target.pong.as_ref() else {
            error!(
                "Post-process pong buffer missing after initialization; falling back to direct blit"
            );
            blit_to_window(rl, th, &render_target.texture, src, dest, post_blit.take());
            return;
        };

        // Get raw pointers to independently borrow texture, ping, and pong
        // SAFETY: These fields are independent and don't alias
        let main_tex_ptr = &render_target.texture as *const RenderTexture2D;
        let ping_tex_ptr = ping_tex as *const RenderTexture2D;
        let pong_tex_ptr = pong_tex as *const RenderTexture2D;

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
                {
                    crate::tracy::tracy_span!("render/draw_commands");
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
                }
                {
                    // Drop the drawing handle here: EndDrawing() → SwapBuffers → vsync wait.
                    crate::tracy::tracy_span!("render/present_vsync");
                    drop(d);
                }
                final_blit_done = true;
            } else {
                // Draw to intermediate buffer
                // Choose destination buffer (opposite of source for ping-pong)
                let write_to_ping =
                    matches!(source_buffer, SourceBuffer::Main | SourceBuffer::Pong);

                if write_to_ping {
                    let Some(dest_tex) = render_target.ping.as_mut() else {
                        error!(
                            "Post-process ping buffer missing during render pass; falling back to direct blit"
                        );
                        blit_to_window(rl, th, source_tex, src, dest, post_blit.take());
                        return;
                    };
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
                    let Some(dest_tex) = render_target.pong.as_mut() else {
                        error!(
                            "Post-process pong buffer missing during render pass; falling back to direct blit"
                        );
                        blit_to_window(rl, th, source_tex, src, dest, post_blit.take());
                        return;
                    };
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
    {
        crate::tracy::tracy_span!("render/draw_commands");
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
    {
        // Drop the drawing handle here: EndDrawing() → SwapBuffers → vsync wait.
        crate::tracy::tracy_span!("render/present_vsync");
        drop(d);
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

/// Raw `SetShaderValue` call. All `set_*` helpers below funnel through this
/// single `unsafe` site so the `SHADER_UNIFORM_*` tag and pointer type can
/// only mismatch in one place.
fn set_shader_value_raw(
    shader: &mut Shader,
    loc: i32,
    ptr: *const std::ffi::c_void,
    ty: ffi::ShaderUniformDataType,
) {
    unsafe {
        ffi::SetShaderValue(**shader, loc, ptr, ty as i32);
    }
}

/// Set a `float` uniform by name, if present in the shader.
fn set_float(shader: &mut Shader, locations: &mut FxHashMap<String, i32>, name: &str, value: &f32) {
    let loc = get_uniform_loc(shader, locations, name);
    if loc >= 0 {
        set_shader_value_raw(
            shader,
            loc,
            value as *const f32 as *const std::ffi::c_void,
            ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT,
        );
    }
}

/// Set an `int` uniform by name, if present in the shader.
fn set_int(shader: &mut Shader, locations: &mut FxHashMap<String, i32>, name: &str, value: &i32) {
    let loc = get_uniform_loc(shader, locations, name);
    if loc >= 0 {
        set_shader_value_raw(
            shader,
            loc,
            value as *const i32 as *const std::ffi::c_void,
            ffi::ShaderUniformDataType::SHADER_UNIFORM_INT,
        );
    }
}

/// Set a `vec2` uniform by name, if present in the shader.
fn set_vec2(shader: &mut Shader, locations: &mut FxHashMap<String, i32>, name: &str, value: &[f32; 2]) {
    let loc = get_uniform_loc(shader, locations, name);
    if loc >= 0 {
        set_shader_value_raw(
            shader,
            loc,
            value.as_ptr() as *const std::ffi::c_void,
            ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2,
        );
    }
}

/// Set a `vec4` uniform by name, if present in the shader.
fn set_vec4(shader: &mut Shader, locations: &mut FxHashMap<String, i32>, name: &str, value: &[f32; 4]) {
    let loc = get_uniform_loc(shader, locations, name);
    if loc >= 0 {
        set_shader_value_raw(
            shader,
            loc,
            value.as_ptr() as *const std::ffi::c_void,
            ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC4,
        );
    }
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
    set_float(shader, locations, "uTime", &world_time.elapsed);

    // uDeltaTime (float)
    set_float(shader, locations, "uDeltaTime", &world_time.delta);

    // uResolution (vec2) - game resolution
    set_vec2(
        shader,
        locations,
        "uResolution",
        &[screensize.w as f32, screensize.h as f32],
    );

    // uFrame (int)
    set_int(shader, locations, "uFrame", &(world_time.frame_count as i32));

    // uWindowResolution (vec2)
    set_vec2(
        shader,
        locations,
        "uWindowResolution",
        &[window_size.w as f32, window_size.h as f32],
    );

    // uLetterbox (vec4) - destination rectangle
    set_vec4(
        shader,
        locations,
        "uLetterbox",
        &[dest.x, dest.y, dest.width, dest.height],
    );
}

/// Set a user-defined uniform value on a shader. Silently skips uniforms
/// not found in the shader (handled by the `set_*` helpers).
pub(super) fn set_uniform_value(
    shader: &mut Shader,
    locations: &mut FxHashMap<String, i32>,
    name: &str,
    value: &UniformValue,
) {
    match value {
        UniformValue::Float(v) => set_float(shader, locations, name, v),
        UniformValue::Int(v) => set_int(shader, locations, name, v),
        UniformValue::Vec2 { x, y } => set_vec2(shader, locations, name, &[*x, *y]),
        UniformValue::Vec4 { x, y, z, w } => set_vec4(shader, locations, name, &[*x, *y, *z, *w]),
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
/// - uVelocity (vec2) - velocity (if RigidBody present at capture time)
pub(super) fn set_entity_uniforms(
    shader: &mut Shader,
    locations: &mut FxHashMap<String, i32>,
    entity: Entity,
    pos: &MapPosition,
    rotation: Option<&Rotation>,
    scale: Option<&Scale>,
    size: Vector2,
    velocity: Option<Vector2>,
) {
    // uEntityId (int) - use bits representation truncated to i32
    set_int(
        shader,
        locations,
        "uEntityId",
        &((entity.to_bits() & 0xFFFFFFFF) as i32),
    );

    // uEntityPos (vec2)
    set_vec2(shader, locations, "uEntityPos", &[pos.pos.x, pos.pos.y]);

    // uSpriteSize (vec2)
    set_vec2(shader, locations, "uSpriteSize", &[size.x, size.y]);

    // uRotation (float) - only if Rotation component present
    if let Some(rot) = rotation {
        set_float(shader, locations, "uRotation", &rot.degrees);
    }

    // uScale (vec2) - only if Scale component present
    if let Some(s) = scale {
        set_vec2(shader, locations, "uScale", &[s.scale.x, s.scale.y]);
    }

    // uVelocity (vec2) - only if the entity had a RigidBody at capture time
    // (velocity comes from the DrawableSnapshot entry, not a live query --
    // Phase 4)
    if let Some(velocity) = velocity {
        set_vec2(shader, locations, "uVelocity", &[velocity.x, velocity.y]);
    }
}
