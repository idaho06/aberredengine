use raylib::prelude::Vector2;

use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::input::InputState;
use crate::resources::scenemanager::SceneManager;
use crate::resources::screensize::ScreenSize;
use crate::resources::texturestore::TextureStore;
use crate::resources::windowsize::WindowSize;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use ::imgui::{Condition, TreeNodeFlags, Ui as ImguiUi};

/// Orchestrates all imgui debug panels drawn at window resolution over the game image.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_imgui_debug(
    ui: &ImguiUi,
    overlay_config: &mut DebugOverlayConfig,
    world_signals: &WorldSignals,
    input_state: &InputState,
    camera: &Camera2DRes,
    camera_follow: &CameraFollowConfig,
    scene_manager: Option<&SceneManager>,
    textures: &TextureStore,
    fonts: &FontStore,
    shader_count: usize,
    screensize: &ScreenSize,
    window_size: &WindowSize,
    world_time: &WorldTime,
    config: &GameConfig,
    fps: u32,
    sprite_count: usize,
    collider_count: usize,
    position_count: usize,
    rigidbody_count: usize,
    screen_sprite_count: usize,
    screen_text_count: usize,
    game_mouse_pos: Vector2,
    mouse_world: Vector2,
) {
    draw_performance_panel(ui, fps, world_time);
    draw_ecs_panel(
        ui,
        sprite_count,
        collider_count,
        position_count,
        rigidbody_count,
        screen_sprite_count,
        screen_text_count,
        textures.map.len(),
        fonts.len(),
        shader_count,
    );
    draw_camera_panel(ui, camera, camera_follow);
    draw_world_signals_panel(ui, world_signals);
    draw_input_panel(ui, input_state);
    draw_overlays_panel(ui, overlay_config);
    draw_mouse_config_panel(
        ui,
        game_mouse_pos,
        mouse_world,
        screensize,
        window_size,
        config,
        scene_manager,
    );
}

pub(super) fn draw_performance_panel(ui: &ImguiUi, fps: u32, world_time: &WorldTime) {
    ui.window("Performance")
        .collapsed(false, Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("FPS: {}", fps));
            ui.text(format!("Frame time: {:.2} ms", world_time.delta * 1000.0));
            ui.text(format!("Elapsed: {:.2} s", world_time.elapsed));
            ui.text(format!("Frame: {}", world_time.frame_count));
            ui.text(format!("Time scale: {:.2}x", world_time.time_scale));
            ui.separator();
            ui.text("Press F11 to toggle debug");
        });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_ecs_panel(
    ui: &ImguiUi,
    sprite_count: usize,
    collider_count: usize,
    position_count: usize,
    rigidbody_count: usize,
    screen_sprite_count: usize,
    screen_text_count: usize,
    texture_count: usize,
    font_count: usize,
    shader_count: usize,
) {
    ui.window("ECS")
        .collapsed(true, Condition::FirstUseEver)
        .build(|| {
            if ui.collapsing_header("Entities", TreeNodeFlags::empty()) {
                ui.text(format!("  Map sprites:    {}", sprite_count));
                ui.text(format!("  Colliders:      {}", collider_count));
                ui.text(format!("  Positions:      {}", position_count));
                ui.text(format!("  Rigidbodies:    {}", rigidbody_count));
                ui.text(format!("  Screen sprites: {}", screen_sprite_count));
                ui.text(format!("  Screen texts:   {}", screen_text_count));
            }
            if ui.collapsing_header("Assets", TreeNodeFlags::empty()) {
                ui.text(format!("  Textures: {}", texture_count));
                ui.text(format!("  Fonts:    {}", font_count));
                ui.text(format!("  Shaders:  {}", shader_count));
            }
        });
}

pub(super) fn draw_camera_panel(
    ui: &ImguiUi,
    camera: &Camera2DRes,
    camera_follow: &CameraFollowConfig,
) {
    ui.window("Camera")
        .collapsed(true, Condition::FirstUseEver)
        .build(|| {
            let cam = &camera.0;
            ui.text(format!(
                "Target:   ({:.1}, {:.1})",
                cam.target.x, cam.target.y
            ));
            ui.text(format!(
                "Offset:   ({:.1}, {:.1})",
                cam.offset.x, cam.offset.y
            ));
            ui.text(format!("Rotation: {:.2}°", cam.rotation));
            ui.text(format!("Zoom:     {:.3}", cam.zoom));
            ui.separator();
            ui.text(format!("Enabled:    {}", camera_follow.enabled));
            ui.text(format!("Mode:       {:?}", camera_follow.mode));
            ui.text(format!("Lerp speed: {:.2}", camera_follow.lerp_speed));
            ui.text(format!("Spring K:   {:.2}", camera_follow.spring_stiffness));
            ui.text(format!("Spring D:   {:.2}", camera_follow.spring_damping));
        });
}

pub(super) fn draw_world_signals_panel(ui: &ImguiUi, world_signals: &WorldSignals) {
    ui.window("World Signals")
        .collapsed(true, Condition::FirstUseEver)
        .build(|| {
            if ui.collapsing_header(
                format!("Flags ({})", world_signals.get_flags().len()),
                TreeNodeFlags::empty(),
            ) {
                let mut flags: Vec<&str> =
                    world_signals.get_flags().iter().map(|s| s.as_str()).collect();
                flags.sort_unstable();
                for flag in flags {
                    ui.text(format!("  {}", flag));
                }
            }
            if ui.collapsing_header(
                format!("Scalars ({})", world_signals.get_scalars().len()),
                TreeNodeFlags::empty(),
            ) {
                let mut entries: Vec<(&str, f32)> = world_signals
                    .get_scalars()
                    .iter()
                    .map(|(k, v)| (k.as_str(), *v))
                    .collect();
                entries.sort_unstable_by_key(|(k, _)| *k);
                for (key, val) in entries {
                    ui.text(format!("  {} = {:.4}", key, val));
                }
            }
            if ui.collapsing_header(
                format!("Integers ({})", world_signals.get_integers().len()),
                TreeNodeFlags::empty(),
            ) {
                let mut entries: Vec<(&str, i32)> = world_signals
                    .get_integers()
                    .iter()
                    .map(|(k, v)| (k.as_str(), *v))
                    .collect();
                entries.sort_unstable_by_key(|(k, _)| *k);
                for (key, val) in entries {
                    ui.text(format!("  {} = {}", key, val));
                }
            }
            if ui.collapsing_header(
                format!("Strings ({})", world_signals.get_strings().len()),
                TreeNodeFlags::empty(),
            ) {
                let mut entries: Vec<(&str, &str)> = world_signals
                    .get_strings()
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                entries.sort_unstable_by_key(|(k, _)| *k);
                for (key, val) in entries {
                    ui.text(format!("  {} = {:?}", key, val));
                }
            }
            if ui.collapsing_header(
                format!("Entities ({})", world_signals.entities.len()),
                TreeNodeFlags::empty(),
            ) {
                let mut entries: Vec<(&str, u64)> = world_signals
                    .entities
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.to_bits()))
                    .collect();
                entries.sort_unstable_by_key(|(k, _)| *k);
                for (key, bits) in entries {
                    ui.text(format!("  {} = {:x}", key, bits));
                }
            }
        });
}

pub(super) fn draw_input_panel(ui: &ImguiUi, input_state: &InputState) {
    ui.window("Input")
        .collapsed(true, Condition::FirstUseEver)
        .build(|| {
            let inputs: &[(&str, &crate::resources::input::BoolState)] = &[
                ("Up (WASD)", &input_state.maindirection_up),
                ("Left (WASD)", &input_state.maindirection_left),
                ("Down (WASD)", &input_state.maindirection_down),
                ("Right (WASD)", &input_state.maindirection_right),
                ("Up (Arrow)", &input_state.secondarydirection_up),
                ("Down (Arrow)", &input_state.secondarydirection_down),
                ("Left (Arrow)", &input_state.secondarydirection_left),
                ("Right (Arrow)", &input_state.secondarydirection_right),
                ("Back (Esc)", &input_state.action_back),
                ("Action 1 (Space/LMB)", &input_state.action_1),
                ("Action 2 (Enter/RMB)", &input_state.action_2),
                ("Action 3 (MMB)", &input_state.action_3),
                ("Debug (F11)", &input_state.mode_debug),
                ("Fullscr (F10)", &input_state.fullscreen_toggle),
                ("Special (F12)", &input_state.action_special),
            ];
            for (name, state) in inputs {
                if state.active {
                    ui.text_colored([0.0, 1.0, 0.0, 1.0], "[ON]");
                } else {
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], "[  ]");
                }
                ui.same_line();
                ui.text(format!("{:20}", name));
                if state.just_pressed {
                    ui.same_line();
                    ui.text_colored([1.0, 1.0, 0.0, 1.0], "PRESS");
                }
                if state.just_released {
                    ui.same_line();
                    ui.text_colored([1.0, 0.5, 0.0, 1.0], "RELEASE");
                }
            }
            ui.separator();
            let scroll_color = if input_state.scroll_y != 0.0 {
                [0.0, 1.0, 0.0, 1.0]
            } else {
                [0.5, 0.5, 0.5, 1.0]
            };
            ui.text_colored(
                scroll_color,
                format!("Scroll Y: {:+.2}", input_state.scroll_y),
            );
        });
}

pub(super) fn draw_overlays_panel(ui: &ImguiUi, overlay_config: &mut DebugOverlayConfig) {
    ui.window("Overlays")
        .collapsed(true, Condition::FirstUseEver)
        .build(|| {
            ui.checkbox("Collider boxes", &mut overlay_config.show_collider_boxes);
            ui.checkbox(
                "Position crosshairs",
                &mut overlay_config.show_position_crosshairs,
            );
            ui.checkbox("Entity signals", &mut overlay_config.show_entity_signals);
            ui.checkbox("Text bounds", &mut overlay_config.show_text_bounds);
            ui.checkbox("Sprite bounds", &mut overlay_config.show_sprite_bounds);
        });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_mouse_config_panel(
    ui: &ImguiUi,
    game_mouse_pos: Vector2,
    mouse_world: Vector2,
    screensize: &ScreenSize,
    window_size: &WindowSize,
    config: &GameConfig,
    scene_manager: Option<&SceneManager>,
) {
    ui.window("Mouse & Config")
        .collapsed(true, Condition::FirstUseEver)
        .build(|| {
            ui.text(format!(
                "Mouse game:  ({:.1}, {:.1})",
                game_mouse_pos.x, game_mouse_pos.y
            ));
            ui.text(format!(
                "Mouse world: ({:.1}, {:.1})",
                mouse_world.x, mouse_world.y
            ));
            ui.separator();
            ui.text(format!("Render size: {}x{}", screensize.w, screensize.h));
            ui.text(format!("Window size: {}x{}", window_size.w, window_size.h));
            ui.separator();
            ui.text(format!("FPS target: {}", config.target_fps));
            ui.text(format!("VSync: {}", config.vsync));
            if let Some(sm) = scene_manager {
                ui.separator();
                if let Some(ref current) = sm.active_scene {
                    ui.text(format!("Scene: {}", current));
                } else {
                    ui.text("Scene: (none)");
                }
            }
        });
}
