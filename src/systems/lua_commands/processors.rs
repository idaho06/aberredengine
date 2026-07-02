//! Behavioral processors for Lua command queues.
//!
//! Each `process_*` function handles exactly one Lua command domain and is used
//! by the queue-draining systems in the Lua integration layer.

use std::sync::Arc;

use bevy_ecs::prelude::*;
use log::{debug, warn};
use raylib::prelude::{Camera2D, Color, Rectangle, Vector2};

use crate::components::phase::Phase;
use crate::components::shadow::Shadow;
use crate::events::audio::AudioCmd;
use crate::events::render_assets::RenderAssetCmd;
use crate::resources::animationstore::{AnimationResource, AnimationStore};
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::{CameraFollowConfig, EasingCurve, FollowMode};
use crate::resources::gameconfig::GameConfig;
use crate::resources::guitheme::{GuiButtonSkin, GuiNinePatch, GuiProgressBarSkin, GuiTheme, GuiThemeStore};
use crate::resources::group::TrackedGroups;
use crate::resources::input_bindings::{InputBindings, binding_from_str};
use crate::resources::lua_runtime::{
    AnimationCmd, AssetCmd, AudioLuaCmd, CameraCmd, CameraFollowCmd, GameConfigCmd, GroupCmd,
    InputCmd, PhaseCmd, RenderCmd, SignalCmd,
};
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::texturefilter::TextureFilter;
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

/// Translate a single Lua-queued [`AssetCmd`] into either an [`AudioCmd`]
/// (Music/Sound, unchanged) or a [`RenderAssetCmd`] (Texture/Font/Shader).
/// Contains no GL calls — safe to run in a logic-destined system.
pub fn translate_asset_command(
    cmd: AssetCmd,
    audio_cmd_writer: &mut MessageWriter<AudioCmd>,
    render_asset_cmd_writer: &mut MessageWriter<RenderAssetCmd>,
) {
    match asset_cmd_to_audio_cmd(cmd) {
        Ok(audio_cmd) => {
            audio_cmd_writer.write(audio_cmd);
        }
        Err(other) => {
            if let Some(render_cmd) = asset_cmd_to_render_asset_cmd(other) {
                render_asset_cmd_writer.write(render_cmd);
            }
        }
    }
}

/// Convert a Music/Sound [`AssetCmd`] into its [`AudioCmd`] equivalent.
/// Returns the command back (`Err`) for Texture/Font/Shader, which have no
/// `AudioCmd` equivalent — pass those to [`asset_cmd_to_render_asset_cmd`]
/// instead. Shared by [`translate_asset_command`] (the per-frame drain
/// system's path) and [`crate::lua_plugin::setup`] (the one-shot bootstrap
/// exception), so the Music/Sound branch has exactly one implementation.
pub fn asset_cmd_to_audio_cmd(cmd: AssetCmd) -> Result<AudioCmd, AssetCmd> {
    match cmd {
        AssetCmd::Music { id, path } => {
            debug!("Queuing music '{}' from '{}'", id, path);
            Ok(AudioCmd::LoadMusic { id, path })
        }
        AssetCmd::Sound { id, path } => {
            debug!("Queuing sound '{}' from '{}'", id, path);
            Ok(AudioCmd::LoadFx { id, path })
        }
        other => Err(other),
    }
}

/// Convert a Texture/Font/Shader [`AssetCmd`] into its [`RenderAssetCmd`]
/// equivalent. `skip_if_loaded` is always `false` here (Lua's
/// `engine.load_font` always reloads) — `spawn_map` builds its own
/// `RenderAssetCmd::Font { skip_if_loaded: true, .. }` directly since it
/// has a different reload policy. Music/Sound have no `RenderAssetCmd`
/// equivalent and return `None`; `TextureFilter` string resolution happens
/// here (pure string parsing, no GL) so warnings stay attributable to the
/// right `id`.
pub fn asset_cmd_to_render_asset_cmd(cmd: AssetCmd) -> Option<RenderAssetCmd> {
    match cmd {
        AssetCmd::Texture { id, path, filter } => {
            let filter = TextureFilter::from_opt_str_or_warn(filter.as_deref(), &id);
            Some(RenderAssetCmd::Texture { id, path, filter })
        }
        AssetCmd::Font { id, path, size } => Some(RenderAssetCmd::Font {
            id,
            path,
            size,
            skip_if_loaded: false,
        }),
        AssetCmd::Shader {
            id,
            vs_path,
            fs_path,
        } => Some(RenderAssetCmd::Shader {
            id,
            vs_path,
            fs_path,
        }),
        AssetCmd::Music { .. } | AssetCmd::Sound { .. } => None,
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
        RenderCmd::SetGuiThemeProgressBar {
            theme_key,
            part,
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
            let skin = theme.progress_bar.get_or_insert_with(GuiProgressBarSkin::default);
            let patch = build_nine_patch(
                tex_key, source_x, source_y, source_w, source_h, left, top, right, bottom,
            );
            match part.as_str() {
                "track" => skin.track = Some(patch),
                "fill" => skin.fill = patch,
                other => {
                    warn!("set_gui_theme_progress_bar: unknown part '{}', expected \"track\" or \"fill\"", other);
                }
            }
        }
        RenderCmd::SetGuiThemeButtonShadow { theme_key, state, dx, dy, r, g, b, a } => {
            let theme = staged_theme_mut(gui_theme_staging, &theme_key);
            let skin = theme.button.get_or_insert_with(GuiButtonSkin::default);
            let shadow = Some(Shadow::new(dx, dy, r, g, b, a));
            match state.as_str() {
                "normal"   => skin.shadow = shadow,
                "hover"    => skin.hover_shadow = shadow,
                "pressed"  => skin.pressed_shadow = shadow,
                "disabled" => skin.disabled_shadow = shadow,
                other => warn!(
                    "set_gui_theme_button_shadow: unknown state '{}', expected \"normal\", \"hover\", \"pressed\", or \"disabled\"",
                    other
                ),
            }
        }
        RenderCmd::SetGuiThemePanelShadow { theme_key, dx, dy, r, g, b, a } => {
            staged_theme_mut(gui_theme_staging, &theme_key).panel_shadow =
                Some(Shadow::new(dx, dy, r, g, b, a));
        }
        RenderCmd::SetGuiThemeTextShadow { theme_key, dx, dy, r, g, b, a } => {
            staged_theme_mut(gui_theme_staging, &theme_key).text_shadow =
                Some(Shadow::new(dx, dy, r, g, b, a));
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
        process_signal_command, translate_asset_command,
    };
    use crate::events::audio::AudioCmd;
    use crate::events::render_assets::RenderAssetCmd;
    use crate::resources::animationstore::AnimationStore;
    use crate::resources::guitheme::GuiThemeStore;
    use crate::resources::lua_runtime::{AnimationCmd, AssetCmd, AudioLuaCmd, RenderCmd, SignalCmd};
    use crate::resources::postprocessshader::PostProcessShader;
    use crate::resources::texturefilter::TextureFilter;
    use crate::resources::worldsignals::WorldSignals;

    /// Bundled writers for both message types `translate_asset_command`
    /// can produce, plus a helper to read back what was written.
    struct AssetCmdTestHarness {
        world: World,
    }

    impl AssetCmdTestHarness {
        fn new() -> Self {
            let mut world = World::new();
            world.insert_resource(Messages::<AudioCmd>::default());
            world.insert_resource(Messages::<RenderAssetCmd>::default());
            Self { world }
        }

        fn translate(&mut self, cmd: AssetCmd) {
            let mut state = SystemState::<(
                MessageWriter<AudioCmd>,
                MessageWriter<RenderAssetCmd>,
            )>::new(&mut self.world);
            {
                let (mut audio_writer, mut render_writer) = state
                    .get_mut(&mut self.world)
                    .expect("writers should fetch");
                translate_asset_command(cmd, &mut audio_writer, &mut render_writer);
            }
            state.apply(&mut self.world);
        }

        fn audio_cmds(&mut self) -> Vec<AudioCmd> {
            self.world.resource_mut::<Messages<AudioCmd>>().update();
            let mut state = SystemState::<MessageReader<AudioCmd>>::new(&mut self.world);
            let mut reader = state
                .get_mut(&mut self.world)
                .expect("audio reader should fetch");
            reader.read().cloned().collect()
        }

        fn render_asset_cmds(&mut self) -> Vec<RenderAssetCmd> {
            self.world
                .resource_mut::<Messages<RenderAssetCmd>>()
                .update();
            let mut state = SystemState::<MessageReader<RenderAssetCmd>>::new(&mut self.world);
            let mut reader = state
                .get_mut(&mut self.world)
                .expect("render asset reader should fetch");
            reader.read().cloned().collect()
        }
    }

    #[test]
    fn translate_texture_resolves_filter_and_writes_render_asset_cmd() {
        let mut h = AssetCmdTestHarness::new();
        h.translate(AssetCmd::Texture {
            id: "tex1".into(),
            path: "assets/tex1.png".into(),
            filter: Some("bilinear".into()),
        });

        let render_cmds = h.render_asset_cmds();
        assert_eq!(render_cmds.len(), 1);
        match &render_cmds[0] {
            RenderAssetCmd::Texture { id, path, filter } => {
                assert_eq!(id, "tex1");
                assert_eq!(path, "assets/tex1.png");
                assert_eq!(*filter, TextureFilter::Bilinear);
            }
            other => panic!("expected RenderAssetCmd::Texture, got {other:?}"),
        }
        assert!(h.audio_cmds().is_empty());
    }

    #[test]
    fn translate_font_sets_skip_if_loaded_false() {
        let mut h = AssetCmdTestHarness::new();
        h.translate(AssetCmd::Font {
            id: "font1".into(),
            path: "assets/font1.ttf".into(),
            size: 24,
        });

        let render_cmds = h.render_asset_cmds();
        assert_eq!(render_cmds.len(), 1);
        match &render_cmds[0] {
            RenderAssetCmd::Font {
                id,
                path,
                size,
                skip_if_loaded,
            } => {
                assert_eq!(id, "font1");
                assert_eq!(path, "assets/font1.ttf");
                assert_eq!(*size, 24);
                assert!(!skip_if_loaded, "Lua font loads always reload");
            }
            other => panic!("expected RenderAssetCmd::Font, got {other:?}"),
        }
    }

    #[test]
    fn translate_shader_writes_render_asset_cmd() {
        let mut h = AssetCmdTestHarness::new();
        h.translate(AssetCmd::Shader {
            id: "shader1".into(),
            vs_path: Some("a.vs".into()),
            fs_path: None,
        });

        let render_cmds = h.render_asset_cmds();
        assert_eq!(render_cmds.len(), 1);
        assert!(matches!(&render_cmds[0], RenderAssetCmd::Shader { id, .. } if id == "shader1"));
    }

    #[test]
    fn translate_music_and_sound_never_touch_render_asset_queue() {
        let mut h = AssetCmdTestHarness::new();
        h.translate(AssetCmd::Music {
            id: "bgm".into(),
            path: "a.xm".into(),
        });
        h.translate(AssetCmd::Sound {
            id: "sfx".into(),
            path: "b.wav".into(),
        });

        let audio_cmds = h.audio_cmds();
        assert_eq!(audio_cmds.len(), 2);
        assert!(matches!(audio_cmds[0], AudioCmd::LoadMusic { .. }));
        assert!(matches!(audio_cmds[1], AudioCmd::LoadFx { .. }));
        assert!(
            h.render_asset_cmds().is_empty(),
            "Music/Sound must never produce a RenderAssetCmd"
        );
    }

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
