//! Behavioral processors for Lua command queues.
//!
//! Each `process_*` function handles exactly one Lua command domain and is used
//! by the queue-draining systems in the Lua integration layer.

use std::sync::Arc;

use bevy_ecs::prelude::*;
use log::{debug, error, warn};
use raylib::prelude::{Camera2D, Color, Rectangle, Vector2};

use crate::components::phase::Phase;
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::{AnimationResource, AnimationStore};
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::{CameraFollowConfig, EasingCurve, FollowMode};
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::guitheme::{GuiButtonSkin, GuiNinePatch, GuiTheme, GuiThemeStore};
use crate::resources::group::TrackedGroups;
use crate::resources::input_bindings::{InputBindings, binding_from_str};
use crate::resources::lua_runtime::{
    AnimationCmd, AssetCmd, AudioLuaCmd, CameraCmd, CameraFollowCmd, GameConfigCmd, GroupCmd,
    InputCmd, PhaseCmd, RenderCmd, SignalCmd,
};
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::texturefilter::TextureFilter;
use crate::resources::texturestore::TextureStore;
use crate::resources::worldsignals::WorldSignals;
use crate::systems::phase_core::queue_phase_transition;

/// Process a single audio command from Lua and write to the audio command channel.
pub fn process_audio_command(audio_cmd_writer: &mut MessageWriter<AudioCmd>, cmd: AudioLuaCmd) {
    match cmd {
        AudioLuaCmd::PlayMusic { id, looped } => {
            audio_cmd_writer.write(AudioCmd::PlayMusic { id, looped });
        }
        AudioLuaCmd::PlaySound { id } => {
            audio_cmd_writer.write(AudioCmd::PlayFx { id });
        }
        AudioLuaCmd::PlaySoundPitched { id, pitch } => {
            audio_cmd_writer.write(AudioCmd::PlayFxPitched { id, pitch });
        }
        AudioLuaCmd::StopAllMusic => {
            audio_cmd_writer.write(AudioCmd::StopAllMusic);
        }
        AudioLuaCmd::StopMusic { id } => {
            audio_cmd_writer.write(AudioCmd::StopMusic { id });
        }
        AudioLuaCmd::PauseMusic { id } => {
            audio_cmd_writer.write(AudioCmd::PauseMusic { id });
        }
        AudioLuaCmd::ResumeMusic { id } => {
            audio_cmd_writer.write(AudioCmd::ResumeMusic { id });
        }
        AudioLuaCmd::SetMusicVolume { id, vol } => {
            audio_cmd_writer.write(AudioCmd::VolumeMusic { id, vol });
        }
        AudioLuaCmd::UnloadMusic { id } => {
            audio_cmd_writer.write(AudioCmd::UnloadMusic { id });
        }
        AudioLuaCmd::UnloadAllMusic => {
            audio_cmd_writer.write(AudioCmd::UnloadAllMusic);
        }
        AudioLuaCmd::StopAllSounds => {
            audio_cmd_writer.write(AudioCmd::StopAllFx);
        }
        AudioLuaCmd::UnloadSound { id } => {
            audio_cmd_writer.write(AudioCmd::UnloadFx { id });
        }
        AudioLuaCmd::UnloadAllSounds => {
            audio_cmd_writer.write(AudioCmd::UnloadAllFx);
        }
    }
}

/// Process a single signal command from Lua and update world signals.
pub fn process_signal_command(world_signals: &mut WorldSignals, cmd: SignalCmd) {
    match cmd {
        SignalCmd::SetScalar { key, value } => {
            world_signals.set_scalar(&key, value);
        }
        SignalCmd::SetInteger { key, value } => {
            world_signals.set_integer(&key, value);
        }
        SignalCmd::SetFlag { key } => {
            world_signals.set_flag(&key);
        }
        SignalCmd::ClearFlag { key } => {
            world_signals.clear_flag(&key);
        }
        SignalCmd::ToggleFlag { key } => {
            world_signals.toggle_flag(&key);
        }
        SignalCmd::SetString { key, value } => {
            world_signals.set_string(&key, &value);
        }
        SignalCmd::ClearScalar { key } => {
            world_signals.clear_scalar(&key);
        }
        SignalCmd::ClearInteger { key } => {
            world_signals.clear_integer(&key);
        }
        SignalCmd::ClearString { key } => {
            world_signals.remove_string(&key);
        }
        SignalCmd::SetEntity { key, entity_id } => {
            if let Some(entity) = super::entity_cmd::resolve_entity(entity_id) {
                world_signals.set_entity(&key, entity);
            }
        }
        SignalCmd::RemoveEntity { key } => {
            world_signals.remove_entity(&key);
        }
    }
}

/// Process a single group command from Lua and update the tracked groups.
pub fn process_group_command(tracked_groups: &mut TrackedGroups, cmd: GroupCmd) {
    match cmd {
        GroupCmd::TrackGroup { name } => {
            tracked_groups.add_group(&name);
        }
        GroupCmd::UntrackGroup { name } => {
            tracked_groups.remove_group(&name);
        }
        GroupCmd::ClearTrackedGroups => {
            tracked_groups.clear();
        }
    }
}

/// Process a single camera command from Lua and update the camera resource.
pub fn process_camera_command(commands: &mut Commands, cmd: CameraCmd) {
    match cmd {
        CameraCmd::SetCamera2D {
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
        }
    }
}

/// Process a single phase command from Lua and apply it to the appropriate entity.
pub fn process_phase_command<C>(phase_query: &mut Query<(Entity, &mut Phase<C>)>, cmd: PhaseCmd)
where
    C: Send + Sync + 'static,
{
    match cmd {
        PhaseCmd::TransitionTo { entity_id, phase } => {
            if let Some(entity) = super::entity_cmd::resolve_entity(entity_id) {
                queue_phase_transition(phase_query, entity, phase);
            }
        }
    }
}

/// Process a single asset command from Lua and load the corresponding asset.
///
/// Designed for use during `on_setup` / scene initialization, not hot gameplay paths.
#[allow(clippy::too_many_arguments)]
pub fn process_asset_command<F1>(
    rl: &mut raylib::RaylibHandle,
    th: &raylib::RaylibThread,
    cmd: AssetCmd,
    tex_store: &mut TextureStore,
    fonts: &mut FontStore,
    shader_store: &mut ShaderStore,
    audio_cmd_writer: &mut MessageWriter<AudioCmd>,
    load_font_fn: F1,
) where
    F1: FnOnce(
        &mut raylib::RaylibHandle,
        &raylib::RaylibThread,
        &str,
        i32,
    ) -> Result<raylib::prelude::Font, String>,
{
    match cmd {
        AssetCmd::Texture { id, path, filter } => match rl.load_texture(th, &path) {
            Ok(tex) => {
                debug!("Loaded texture '{}' from '{}'", id, path);
                let filter = TextureFilter::from_opt_str_or_warn(filter.as_deref(), &id);
                tex_store.insert(&id, tex, filter, None);
            }
            Err(e) => {
                error!("Failed to load texture '{}': {}", path, e);
            }
        },
        AssetCmd::Font { id, path, size } => match load_font_fn(rl, th, &path, size) {
            Ok(font) => {
                debug!("Loaded font '{}' from '{}'", id, path);
                fonts.add(&id, font);
            }
            Err(err) => {
                error!("Failed to load font '{}' from '{}': {}", id, path, err);
            }
        },
        AssetCmd::Music { id, path } => {
            debug!("Queuing music '{}' from '{}'", id, path);
            audio_cmd_writer.write(AudioCmd::LoadMusic { id, path });
        }
        AssetCmd::Sound { id, path } => {
            debug!("Queuing sound '{}' from '{}'", id, path);
            audio_cmd_writer.write(AudioCmd::LoadFx { id, path });
        }
        AssetCmd::Shader {
            id,
            vs_path,
            fs_path,
        } => {
            let vs_path_c = vs_path.as_deref();
            let fs_path_c = fs_path.as_deref();
            match rl.load_shader(th, vs_path_c, fs_path_c) {
                Ok(shader) if shader.is_shader_valid() => {
                    debug!(
                        "Loaded shader '{}' (vs: {:?}, fs: {:?})",
                        id, vs_path, fs_path
                    );
                    shader_store.add(&id, shader);
                }
                Ok(_) => {
                    error!(
                        "Shader '{}' loaded but is invalid (vs: {:?}, fs: {:?})",
                        id, vs_path, fs_path
                    );
                }
                Err(e) => {
                    error!(
                        "Shader '{}' failed to load: {e} (vs: {:?}, fs: {:?})",
                        id, vs_path, fs_path
                    );
                }
            }
        }
    }
}

/// Returns the `theme_key` a `RenderCmd` touches, if it's one of the
/// `SetGuiTheme*` variants — used by callers to know which `GuiThemeStore`
/// entries were touched in a drain batch, so per-key validation
/// (`GuiTheme::drop_invalid_button_skin`) only re-checks themes that
/// actually changed instead of the whole store every frame.
pub fn render_cmd_theme_key(cmd: &RenderCmd) -> Option<&str> {
    match cmd {
        RenderCmd::SetGuiThemePanel { theme_key, .. }
        | RenderCmd::SetGuiThemeButton { theme_key, .. }
        | RenderCmd::SetGuiThemeLabel { theme_key, .. }
        | RenderCmd::SetGuiThemeFont { theme_key, .. } => Some(theme_key.as_str()),
        _ => None,
    }
}

fn staged_theme_mut<'a>(gui_theme_staging: &'a mut GuiThemeStore, theme_key: &str) -> &'a mut GuiTheme {
    gui_theme_staging.themes.entry(Arc::from(theme_key)).or_default()
}

#[allow(clippy::too_many_arguments)]
fn build_nine_patch(
    tex_key: String,
    source_x: f32,
    source_y: f32,
    source_w: f32,
    source_h: f32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> GuiNinePatch {
    GuiNinePatch {
        tex_key: Arc::from(tex_key),
        source: Rectangle::new(source_x, source_y, source_w, source_h),
        left,
        top,
        right,
        bottom,
    }
}

/// Process a single render command from Lua and update post-process/GUI-theme state.
///
/// `gui_theme_staging` is shared across the whole per-frame drain loop (not
/// reset per command) so that `SetGuiThemePanel`/`SetGuiThemeButton` calls in
/// the same batch — or across frames — mutate one field of one named theme
/// at a time instead of each blindly replacing the whole entry and stomping
/// whichever field the other one had already set, and crucially without
/// disturbing any *other* theme key in the store. Callers seed it from the
/// current resource before draining and write it back with exactly one
/// `commands.insert_resource(...)` after the loop.
pub fn process_render_command(
    cmd: RenderCmd,
    post_process: &mut PostProcessShader,
    gui_theme_staging: &mut GuiThemeStore,
) {
    match cmd {
        RenderCmd::SetPostProcessShader { ids } => {
            post_process.set_shader_chain(ids.clone());
            match &ids {
                Some(list) if !list.is_empty() => {
                    debug!("Post-process shader chain: [{}]", list.join(", "));
                }
                _ => {
                    debug!("Post-process shader disabled");
                }
            }
        }
        RenderCmd::SetPostProcessUniform { name, value } => {
            let is_reserved = post_process.set_uniform(&name, value);
            if is_reserved {
                warn!(
                    "'{}' is a reserved uniform name and will be overwritten by the engine",
                    name
                );
            }
        }
        RenderCmd::ClearPostProcessUniform { name } => {
            post_process.clear_uniform(&name);
        }
        RenderCmd::ClearPostProcessUniforms => {
            post_process.clear_uniforms();
        }
        RenderCmd::SetGuiThemePanel {
            theme_key,
            tex_key,
            source_x,
            source_y,
            source_w,
            source_h,
            left,
            top,
            right,
            bottom,
        } => {
            let theme = staged_theme_mut(gui_theme_staging, &theme_key);
            theme.panel = build_nine_patch(
                tex_key, source_x, source_y, source_w, source_h, left, top, right, bottom,
            );
        }
        RenderCmd::SetGuiThemeButton {
            theme_key,
            state,
            tex_key,
            source_x,
            source_y,
            source_w,
            source_h,
            left,
            top,
            right,
            bottom,
        } => {
            let theme = staged_theme_mut(gui_theme_staging, &theme_key);
            let skin = theme.button.get_or_insert_with(GuiButtonSkin::default);
            let patch = build_nine_patch(
                tex_key, source_x, source_y, source_w, source_h, left, top, right, bottom,
            );
            match state.as_str() {
                "normal" => skin.normal = patch,
                "hover" => skin.hover = Some(patch),
                "pressed" => skin.pressed = Some(patch),
                "disabled" => skin.disabled = Some(patch),
                other => {
                    warn!("set_gui_theme_button: unknown state '{}', ignoring", other);
                }
            }
        }
        RenderCmd::SetGuiThemeLabel {
            theme_key,
            tex_key,
            source_x,
            source_y,
            source_w,
            source_h,
            left,
            top,
            right,
            bottom,
        } => {
            let theme = staged_theme_mut(gui_theme_staging, &theme_key);
            theme.label = Some(build_nine_patch(
                tex_key, source_x, source_y, source_w, source_h, left, top, right, bottom,
            ));
        }
        RenderCmd::SetGuiThemeFont {
            theme_key,
            font_key,
            font_size,
            r,
            g,
            b,
            a,
        } => {
            let theme = staged_theme_mut(gui_theme_staging, &theme_key);
            theme.font = Arc::from(font_key);
            theme.font_size = font_size;
            theme.text_color = Color::new(r, g, b, a);
        }
    }
}

/// Process a single game config command from Lua.
pub fn process_gameconfig_command(cmd: GameConfigCmd, config: &mut GameConfig) {
    match cmd {
        GameConfigCmd::Fullscreen { enabled } => {
            config.fullscreen = enabled;
        }
        GameConfigCmd::Vsync { enabled } => {
            config.vsync = enabled;
        }
        GameConfigCmd::TargetFps { fps } => {
            config.target_fps = fps;
        }
        GameConfigCmd::RenderSize { width, height } => {
            config.render_width = width;
            config.render_height = height;
        }
        GameConfigCmd::BackgroundColor { r, g, b } => {
            config.background_color = Color::new(r, g, b, 255);
        }
        GameConfigCmd::PixelSnapCamera { enabled } => {
            config.pixel_snap_camera = enabled;
        }
        GameConfigCmd::RenderTargetFilter { filter } => {
            config.render_target_filter =
                TextureFilter::from_opt_str_or_warn(Some(&filter), "set_render_target_filter");
        }
    }
}

/// Process a single camera follow command from Lua.
pub fn process_camera_follow_command(cmd: CameraFollowCmd, config: &mut CameraFollowConfig) {
    match cmd {
        CameraFollowCmd::Enable { enabled } => {
            config.enabled = enabled;
        }
        CameraFollowCmd::SetMode { mode } => match mode.as_str() {
            "instant" => config.mode = FollowMode::Instant,
            "lerp" => config.mode = FollowMode::Lerp,
            "smooth_damp" => config.mode = FollowMode::SmoothDamp,
            other => {
                warn!(
                    "Unknown camera follow mode '{}'; expected \"instant\", \"lerp\", or \"smooth_damp\"",
                    other
                );
            }
        },
        CameraFollowCmd::SetDeadzone { half_w, half_h } => {
            config.mode = FollowMode::Deadzone { half_w, half_h };
        }
        CameraFollowCmd::SetEasing { easing } => match easing.parse::<EasingCurve>() {
            Ok(curve) => config.easing = curve,
            Err(_) => warn!(
                "Unknown camera follow easing '{}'; expected \"linear\", \"ease_out\", \"ease_in\", or \"ease_in_out\"",
                easing
            ),
        },
        CameraFollowCmd::SetSpeed { speed } => {
            config.lerp_speed = speed;
        }
        CameraFollowCmd::SetSpring { stiffness, damping } => {
            config.spring_stiffness = stiffness;
            config.spring_damping = damping;
        }
        CameraFollowCmd::SetOffset { x, y } => {
            config.offset = Vector2 { x, y };
        }
        CameraFollowCmd::SetBounds { x, y, w, h } => {
            config.bounds = Some(raylib::prelude::Rectangle {
                x,
                y,
                width: w,
                height: h,
            });
        }
        CameraFollowCmd::ClearBounds => {
            config.bounds = None;
        }
        CameraFollowCmd::ResetVelocity => {
            config.velocity = Vector2 { x: 0.0, y: 0.0 };
        }
        CameraFollowCmd::SetZoomSpeed { speed } => {
            config.zoom_lerp_speed = speed;
        }
    }
}

/// Process a single input rebinding command from Lua.
pub fn process_input_command(cmd: InputCmd, bindings: &mut InputBindings) {
    use crate::resources::lua_runtime::action_from_str;

    match cmd {
        InputCmd::Rebind { action, key } => {
            let Some(a) = action_from_str(&action) else {
                log::warn!("rebind_action: unknown action '{}'", action);
                return;
            };
            let Some(b) = binding_from_str(&key) else {
                log::warn!("rebind_action: unknown binding '{}'", key);
                return;
            };
            bindings.rebind(a, b);
        }
        InputCmd::AddBinding { action, key } => {
            let Some(a) = action_from_str(&action) else {
                log::warn!("add_binding: unknown action '{}'", action);
                return;
            };
            let Some(b) = binding_from_str(&key) else {
                log::warn!("add_binding: unknown binding '{}'", key);
                return;
            };
            bindings.add_binding(a, b);
        }
    }
}

/// Process a single animation registration command from Lua.
pub fn process_animation_command(anim_store: &mut AnimationStore, cmd: AnimationCmd) {
    match cmd {
        AnimationCmd::RegisterAnimation {
            id,
            tex_key,
            pos_x,
            pos_y,
            horizontal_displacement,
            vertical_displacement,
            frame_count,
            fps,
            looped,
        } => {
            anim_store.insert(
                id.clone(),
                AnimationResource {
                    tex_key: Arc::from(tex_key),
                    position: Vector2 { x: pos_x, y: pos_y },
                    horizontal_displacement,
                    vertical_displacement,
                    frame_count,
                    fps,
                    looped,
                },
            );
            debug!(
                "Registered animation '{}' ({} frames, {} fps)",
                id, frame_count, fps
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::{MessageReader, MessageWriter, World};
    use bevy_ecs::system::SystemState;
    use raylib::prelude::{Color, Vector2};

    use super::{
        process_animation_command, process_audio_command, process_render_command,
        process_signal_command,
    };
    use crate::events::audio::AudioCmd;
    use crate::resources::animationstore::AnimationStore;
    use crate::resources::guitheme::GuiThemeStore;
    use crate::resources::lua_runtime::{AnimationCmd, AudioLuaCmd, RenderCmd, SignalCmd};
    use crate::resources::postprocessshader::PostProcessShader;
    use crate::resources::worldsignals::WorldSignals;

    fn set_button_cmd(theme_key: &str, state: &str) -> RenderCmd {
        RenderCmd::SetGuiThemeButton {
            theme_key: theme_key.to_string(),
            state: state.to_string(),
            tex_key: format!("tex_{state}"),
            source_x: 0.0,
            source_y: 0.0,
            source_w: 32.0,
            source_h: 32.0,
            left: 4,
            top: 4,
            right: 4,
            bottom: 4,
        }
    }

    fn set_panel_cmd(theme_key: &str, tex_key: &str) -> RenderCmd {
        RenderCmd::SetGuiThemePanel {
            theme_key: theme_key.to_string(),
            tex_key: tex_key.to_string(),
            source_x: 0.0,
            source_y: 0.0,
            source_w: 64.0,
            source_h: 64.0,
            left: 6,
            top: 6,
            right: 6,
            bottom: 6,
        }
    }

    #[test]
    fn gui_theme_staging_panel_then_all_button_states_survive() {
        let mut post_process = PostProcessShader::default();
        let mut staging = GuiThemeStore::default();

        process_render_command(set_panel_cmd("default", "panel_tex"), &mut post_process, &mut staging);
        for state in ["normal", "hover", "pressed", "disabled"] {
            process_render_command(set_button_cmd("default", state), &mut post_process, &mut staging);
        }
        process_render_command(
            RenderCmd::SetGuiThemeLabel {
                theme_key: "default".to_string(),
                tex_key: "label_tex".to_string(),
                source_x: 0.0,
                source_y: 0.0,
                source_w: 48.0,
                source_h: 32.0,
                left: 6,
                top: 6,
                right: 6,
                bottom: 6,
            },
            &mut post_process,
            &mut staging,
        );
        process_render_command(
            RenderCmd::SetGuiThemeFont {
                theme_key: "default".to_string(),
                font_key: "arcade".to_string(),
                font_size: 20.0,
                r: 10,
                g: 20,
                b: 30,
                a: 255,
            },
            &mut post_process,
            &mut staging,
        );

        let theme = staging.themes.get("default").expect("theme should be staged");
        assert_eq!(&*theme.panel.tex_key, "panel_tex");
        let skin = theme.button.clone().expect("button skin should be staged");
        assert_eq!(&*skin.normal.tex_key, "tex_normal");
        assert_eq!(&*skin.hover.unwrap().tex_key, "tex_hover");
        assert_eq!(&*skin.pressed.unwrap().tex_key, "tex_pressed");
        assert_eq!(&*skin.disabled.unwrap().tex_key, "tex_disabled");
        let label = theme.label.clone().expect("label nine-patch should be staged");
        assert_eq!(&*label.tex_key, "label_tex");
        assert_eq!(&*theme.font, "arcade");
        assert_eq!(theme.font_size, 20.0);
        assert_eq!(theme.text_color, Color::new(10, 20, 30, 255));
    }

    #[test]
    fn gui_theme_staging_button_states_then_panel_survive_reverse_order() {
        let mut post_process = PostProcessShader::default();
        let mut staging = GuiThemeStore::default();

        for state in ["normal", "hover", "pressed", "disabled"] {
            process_render_command(set_button_cmd("default", state), &mut post_process, &mut staging);
        }
        process_render_command(set_panel_cmd("default", "panel_tex"), &mut post_process, &mut staging);

        let theme = staging.themes.get("default").expect("theme should be staged");
        assert_eq!(&*theme.panel.tex_key, "panel_tex");
        let skin = theme.button.clone().expect("button skin should be staged");
        assert_eq!(&*skin.normal.tex_key, "tex_normal");
        assert_eq!(&*skin.disabled.unwrap().tex_key, "tex_disabled");
    }

    #[test]
    fn gui_theme_staging_button_normal_only_leaves_other_states_none() {
        let mut post_process = PostProcessShader::default();
        let mut staging = GuiThemeStore::default();

        process_render_command(set_button_cmd("default", "normal"), &mut post_process, &mut staging);

        let theme = staging.themes.get("default").expect("theme should be staged");
        let skin = theme.button.clone().expect("button skin should be staged");
        assert_eq!(&*skin.normal.tex_key, "tex_normal");
        assert!(skin.hover.is_none());
        assert!(skin.pressed.is_none());
        assert!(skin.disabled.is_none());
    }

    #[test]
    fn gui_theme_staging_two_keys_do_not_interfere() {
        let mut post_process = PostProcessShader::default();
        let mut staging = GuiThemeStore::default();

        process_render_command(set_panel_cmd("theme_a", "panel_a"), &mut post_process, &mut staging);
        process_render_command(set_panel_cmd("theme_b", "panel_b"), &mut post_process, &mut staging);
        process_render_command(set_button_cmd("theme_b", "normal"), &mut post_process, &mut staging);

        let theme_a = staging.themes.get("theme_a").expect("theme_a should be staged");
        assert_eq!(&*theme_a.panel.tex_key, "panel_a");
        assert!(theme_a.button.is_none());

        let theme_b = staging.themes.get("theme_b").expect("theme_b should be staged");
        assert_eq!(&*theme_b.panel.tex_key, "panel_b");
        assert!(theme_b.button.is_some());
    }

    #[test]
    fn gui_theme_staging_existing_other_key_preserved_across_drain() {
        let mut post_process = PostProcessShader::default();
        let mut staging = GuiThemeStore::default();
        process_render_command(set_panel_cmd("theme_a", "panel_a"), &mut post_process, &mut staging);

        // Simulate a later frame's staging seeded from the persisted resource,
        // draining only a "theme_b" command.
        process_render_command(set_panel_cmd("theme_b", "panel_b"), &mut post_process, &mut staging);

        let theme_a = staging.themes.get("theme_a").expect("theme_a should survive");
        assert_eq!(&*theme_a.panel.tex_key, "panel_a");
        let theme_b = staging.themes.get("theme_b").expect("theme_b should be staged");
        assert_eq!(&*theme_b.panel.tex_key, "panel_b");
    }

    #[test]
    fn stop_all_sounds_maps_to_stop_all_fx() {
        let mut world = World::new();
        world.insert_resource(Messages::<AudioCmd>::default());

        let mut system_state = SystemState::<MessageWriter<AudioCmd>>::new(&mut world);
        {
            let mut writer = system_state
                .get_mut(&mut world)
                .expect("Audio message writer should fetch");
            process_audio_command(&mut writer, AudioLuaCmd::StopAllSounds);
        }
        system_state.apply(&mut world);

        world.resource_mut::<Messages<AudioCmd>>().update();

        let mut reader_state = SystemState::<MessageReader<AudioCmd>>::new(&mut world);
        let mut reader = reader_state
            .get_mut(&mut world)
            .expect("Audio message reader should fetch");
        let cmds: Vec<_> = reader.read().collect();

        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], AudioCmd::StopAllFx));
    }

    #[test]
    fn register_animation_uses_animationstore_abstraction() {
        let mut anim_store = AnimationStore::default();

        process_animation_command(
            &mut anim_store,
            AnimationCmd::RegisterAnimation {
                id: "walk".to_string(),
                tex_key: "player_walk".to_string(),
                pos_x: 12.0,
                pos_y: 24.0,
                horizontal_displacement: 16.0,
                vertical_displacement: 32.0,
                frame_count: 6,
                fps: 10.0,
                looped: true,
            },
        );

        let animation = anim_store
            .animations
            .get("walk")
            .expect("animation should be registered in the store");

        assert_eq!(animation.tex_key.as_ref(), "player_walk");
        assert_eq!(animation.position, Vector2 { x: 12.0, y: 24.0 });
        assert_eq!(animation.horizontal_displacement, 16.0);
        assert_eq!(animation.vertical_displacement, 32.0);
        assert_eq!(animation.frame_count, 6);
        assert_eq!(animation.fps, 10.0);
        assert!(animation.looped);
    }

    #[test]
    fn toggle_flag_updates_world_signals() {
        let mut world_signals = WorldSignals::default();

        process_signal_command(
            &mut world_signals,
            SignalCmd::ToggleFlag {
                key: "paused".to_string(),
            },
        );
        assert!(world_signals.has_flag("paused"));

        process_signal_command(
            &mut world_signals,
            SignalCmd::ToggleFlag {
                key: "paused".to_string(),
            },
        );
        assert!(!world_signals.has_flag("paused"));
    }
}
