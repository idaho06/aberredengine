//! Internal Dear ImGui backend for raylib + rlgl.
//!
//! This replaces the removed `sola-raylib` built-in `imgui` feature so the
//! engine can keep exposing `imgui::Ui` to `GuiCallback` users while owning the
//! backend lifecycle itself.

use std::ffi::CStr;
use std::time::Duration;

use ::imgui::{
    BackendFlags, ClipboardBackend, ConfigFlags, Context, DrawCmd, DrawData, FontSource, Key,
    MouseButton, MouseCursor, TextureId, Ui,
};
use log::warn;
use raylib::ffi;

const RL_TRIANGLES: i32 = 0x0004;

const KEY_MAPPINGS: &[(i32, Key)] = &[
    (ffi::KeyboardKey::KEY_APOSTROPHE as i32, Key::Apostrophe),
    (ffi::KeyboardKey::KEY_COMMA as i32, Key::Comma),
    (ffi::KeyboardKey::KEY_MINUS as i32, Key::Minus),
    (ffi::KeyboardKey::KEY_PERIOD as i32, Key::Period),
    (ffi::KeyboardKey::KEY_SLASH as i32, Key::Slash),
    (ffi::KeyboardKey::KEY_ZERO as i32, Key::Alpha0),
    (ffi::KeyboardKey::KEY_ONE as i32, Key::Alpha1),
    (ffi::KeyboardKey::KEY_TWO as i32, Key::Alpha2),
    (ffi::KeyboardKey::KEY_THREE as i32, Key::Alpha3),
    (ffi::KeyboardKey::KEY_FOUR as i32, Key::Alpha4),
    (ffi::KeyboardKey::KEY_FIVE as i32, Key::Alpha5),
    (ffi::KeyboardKey::KEY_SIX as i32, Key::Alpha6),
    (ffi::KeyboardKey::KEY_SEVEN as i32, Key::Alpha7),
    (ffi::KeyboardKey::KEY_EIGHT as i32, Key::Alpha8),
    (ffi::KeyboardKey::KEY_NINE as i32, Key::Alpha9),
    (ffi::KeyboardKey::KEY_SEMICOLON as i32, Key::Semicolon),
    (ffi::KeyboardKey::KEY_EQUAL as i32, Key::Equal),
    (ffi::KeyboardKey::KEY_A as i32, Key::A),
    (ffi::KeyboardKey::KEY_B as i32, Key::B),
    (ffi::KeyboardKey::KEY_C as i32, Key::C),
    (ffi::KeyboardKey::KEY_D as i32, Key::D),
    (ffi::KeyboardKey::KEY_E as i32, Key::E),
    (ffi::KeyboardKey::KEY_F as i32, Key::F),
    (ffi::KeyboardKey::KEY_G as i32, Key::G),
    (ffi::KeyboardKey::KEY_H as i32, Key::H),
    (ffi::KeyboardKey::KEY_I as i32, Key::I),
    (ffi::KeyboardKey::KEY_J as i32, Key::J),
    (ffi::KeyboardKey::KEY_K as i32, Key::K),
    (ffi::KeyboardKey::KEY_L as i32, Key::L),
    (ffi::KeyboardKey::KEY_M as i32, Key::M),
    (ffi::KeyboardKey::KEY_N as i32, Key::N),
    (ffi::KeyboardKey::KEY_O as i32, Key::O),
    (ffi::KeyboardKey::KEY_P as i32, Key::P),
    (ffi::KeyboardKey::KEY_Q as i32, Key::Q),
    (ffi::KeyboardKey::KEY_R as i32, Key::R),
    (ffi::KeyboardKey::KEY_S as i32, Key::S),
    (ffi::KeyboardKey::KEY_T as i32, Key::T),
    (ffi::KeyboardKey::KEY_U as i32, Key::U),
    (ffi::KeyboardKey::KEY_V as i32, Key::V),
    (ffi::KeyboardKey::KEY_W as i32, Key::W),
    (ffi::KeyboardKey::KEY_X as i32, Key::X),
    (ffi::KeyboardKey::KEY_Y as i32, Key::Y),
    (ffi::KeyboardKey::KEY_Z as i32, Key::Z),
    (ffi::KeyboardKey::KEY_SPACE as i32, Key::Space),
    (ffi::KeyboardKey::KEY_ESCAPE as i32, Key::Escape),
    (ffi::KeyboardKey::KEY_ENTER as i32, Key::Enter),
    (ffi::KeyboardKey::KEY_TAB as i32, Key::Tab),
    (ffi::KeyboardKey::KEY_BACKSPACE as i32, Key::Backspace),
    (ffi::KeyboardKey::KEY_INSERT as i32, Key::Insert),
    (ffi::KeyboardKey::KEY_DELETE as i32, Key::Delete),
    (ffi::KeyboardKey::KEY_RIGHT as i32, Key::RightArrow),
    (ffi::KeyboardKey::KEY_LEFT as i32, Key::LeftArrow),
    (ffi::KeyboardKey::KEY_DOWN as i32, Key::DownArrow),
    (ffi::KeyboardKey::KEY_UP as i32, Key::UpArrow),
    (ffi::KeyboardKey::KEY_PAGE_UP as i32, Key::PageUp),
    (ffi::KeyboardKey::KEY_PAGE_DOWN as i32, Key::PageDown),
    (ffi::KeyboardKey::KEY_HOME as i32, Key::Home),
    (ffi::KeyboardKey::KEY_END as i32, Key::End),
    (ffi::KeyboardKey::KEY_CAPS_LOCK as i32, Key::CapsLock),
    (ffi::KeyboardKey::KEY_SCROLL_LOCK as i32, Key::ScrollLock),
    (ffi::KeyboardKey::KEY_NUM_LOCK as i32, Key::NumLock),
    (ffi::KeyboardKey::KEY_PRINT_SCREEN as i32, Key::PrintScreen),
    (ffi::KeyboardKey::KEY_PAUSE as i32, Key::Pause),
    (ffi::KeyboardKey::KEY_F1 as i32, Key::F1),
    (ffi::KeyboardKey::KEY_F2 as i32, Key::F2),
    (ffi::KeyboardKey::KEY_F3 as i32, Key::F3),
    (ffi::KeyboardKey::KEY_F4 as i32, Key::F4),
    (ffi::KeyboardKey::KEY_F5 as i32, Key::F5),
    (ffi::KeyboardKey::KEY_F6 as i32, Key::F6),
    (ffi::KeyboardKey::KEY_F7 as i32, Key::F7),
    (ffi::KeyboardKey::KEY_F8 as i32, Key::F8),
    (ffi::KeyboardKey::KEY_F9 as i32, Key::F9),
    (ffi::KeyboardKey::KEY_F10 as i32, Key::F10),
    (ffi::KeyboardKey::KEY_F11 as i32, Key::F11),
    (ffi::KeyboardKey::KEY_F12 as i32, Key::F12),
    (ffi::KeyboardKey::KEY_LEFT_SHIFT as i32, Key::LeftShift),
    (ffi::KeyboardKey::KEY_LEFT_CONTROL as i32, Key::LeftCtrl),
    (ffi::KeyboardKey::KEY_LEFT_ALT as i32, Key::LeftAlt),
    (ffi::KeyboardKey::KEY_LEFT_SUPER as i32, Key::LeftSuper),
    (ffi::KeyboardKey::KEY_RIGHT_SHIFT as i32, Key::RightShift),
    (ffi::KeyboardKey::KEY_RIGHT_CONTROL as i32, Key::RightCtrl),
    (ffi::KeyboardKey::KEY_RIGHT_ALT as i32, Key::RightAlt),
    (ffi::KeyboardKey::KEY_RIGHT_SUPER as i32, Key::RightSuper),
    (ffi::KeyboardKey::KEY_KB_MENU as i32, Key::Menu),
    (ffi::KeyboardKey::KEY_LEFT_BRACKET as i32, Key::LeftBracket),
    (ffi::KeyboardKey::KEY_BACKSLASH as i32, Key::Backslash),
    (
        ffi::KeyboardKey::KEY_RIGHT_BRACKET as i32,
        Key::RightBracket,
    ),
    (ffi::KeyboardKey::KEY_GRAVE as i32, Key::GraveAccent),
    (ffi::KeyboardKey::KEY_KP_0 as i32, Key::Keypad0),
    (ffi::KeyboardKey::KEY_KP_1 as i32, Key::Keypad1),
    (ffi::KeyboardKey::KEY_KP_2 as i32, Key::Keypad2),
    (ffi::KeyboardKey::KEY_KP_3 as i32, Key::Keypad3),
    (ffi::KeyboardKey::KEY_KP_4 as i32, Key::Keypad4),
    (ffi::KeyboardKey::KEY_KP_5 as i32, Key::Keypad5),
    (ffi::KeyboardKey::KEY_KP_6 as i32, Key::Keypad6),
    (ffi::KeyboardKey::KEY_KP_7 as i32, Key::Keypad7),
    (ffi::KeyboardKey::KEY_KP_8 as i32, Key::Keypad8),
    (ffi::KeyboardKey::KEY_KP_9 as i32, Key::Keypad9),
    (ffi::KeyboardKey::KEY_KP_DECIMAL as i32, Key::KeypadDecimal),
    (ffi::KeyboardKey::KEY_KP_DIVIDE as i32, Key::KeypadDivide),
    (
        ffi::KeyboardKey::KEY_KP_MULTIPLY as i32,
        Key::KeypadMultiply,
    ),
    (
        ffi::KeyboardKey::KEY_KP_SUBTRACT as i32,
        Key::KeypadSubtract,
    ),
    (ffi::KeyboardKey::KEY_KP_ADD as i32, Key::KeypadAdd),
    (ffi::KeyboardKey::KEY_KP_ENTER as i32, Key::KeypadEnter),
    (ffi::KeyboardKey::KEY_KP_EQUAL as i32, Key::KeypadEqual),
];

const CURSOR_MAP: [i32; MouseCursor::COUNT] = [
    ffi::MouseCursor::MOUSE_CURSOR_ARROW as i32,
    ffi::MouseCursor::MOUSE_CURSOR_IBEAM as i32,
    ffi::MouseCursor::MOUSE_CURSOR_RESIZE_ALL as i32,
    ffi::MouseCursor::MOUSE_CURSOR_RESIZE_NS as i32,
    ffi::MouseCursor::MOUSE_CURSOR_RESIZE_EW as i32,
    ffi::MouseCursor::MOUSE_CURSOR_RESIZE_NESW as i32,
    ffi::MouseCursor::MOUSE_CURSOR_RESIZE_NWSE as i32,
    ffi::MouseCursor::MOUSE_CURSOR_POINTING_HAND as i32,
    ffi::MouseCursor::MOUSE_CURSOR_NOT_ALLOWED as i32,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScissorRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

struct RaylibClipboardBackend;

impl ClipboardBackend for RaylibClipboardBackend {
    fn get(&mut self) -> Option<String> {
        let ptr = unsafe { ffi::GetClipboardText() };
        if ptr.is_null() {
            return None;
        }
        let text = unsafe { CStr::from_ptr(ptr) };
        Some(text.to_string_lossy().into_owned())
    }

    fn set(&mut self, value: &str) {
        if let Ok(value) = std::ffi::CString::new(value) {
            unsafe {
                ffi::SetClipboardText(value.as_ptr());
            }
        }
    }
}

/// Non-send ImGui backend resource owned by the engine.
pub struct ImguiBridge {
    context: Context,
    font_texture: Box<ffi::Texture2D>,
    key_states: Vec<bool>,
    mouse_button_states: [bool; MouseButton::COUNT],
    modifier_states: [bool; 4],
    current_mouse_cursor: Option<MouseCursor>,
    prev_mouse_draw_cursor: bool,
    warned_reset_render_state: bool,
    warned_raw_callback: bool,
}

impl ImguiBridge {
    /// Create a new bridge using the dark style that the engine previously
    /// requested from `sola-raylib`.
    pub fn new_dark() -> Result<Self, String> {
        let mut context = Context::create();
        context.set_platform_name(Some("imgui_impl_raylib".to_string()));
        context.set_renderer_name(Some("imgui_impl_raylib_rlgl".to_string()));
        context.set_clipboard_backend(RaylibClipboardBackend);
        context
            .io_mut()
            .backend_flags
            .insert(BackendFlags::HAS_MOUSE_CURSORS);
        context.style_mut().use_dark_colors();
        context
            .fonts()
            .add_font(&[FontSource::DefaultFontData { config: None }]);

        // Box keeps the Texture2D address stable; ImGui's TextureId is a raw
        // pointer into it, so this field must not be inlined or moved out.
        let font_texture = Box::new(build_font_texture(context.fonts())?);
        context.fonts().tex_id = TextureId::from(font_texture.as_ref() as *const ffi::Texture2D);
        context.fonts().clear_tex_data();

        Ok(Self {
            context,
            font_texture,
            key_states: vec![false; KEY_MAPPINGS.len()],
            mouse_button_states: [false; MouseButton::COUNT],
            modifier_states: [false; 4],
            current_mouse_cursor: None,
            prev_mouse_draw_cursor: false,
            warned_reset_render_state: false,
            warned_raw_callback: false,
        })
    }

    /// Run an ImGui frame and render the resulting draw data through rlgl.
    pub fn render<F>(&mut self, callback: F)
    where
        F: FnOnce(&Ui),
    {
        self.prepare_frame();
        let ui = self.context.new_frame();
        callback(ui);
        let draw_data = self.context.render();
        render_draw_data(
            draw_data,
            &mut self.warned_reset_render_state,
            &mut self.warned_raw_callback,
        );
    }

    fn prepare_frame(&mut self) {
        self.update_display_metrics();
        self.update_delta_time();
        self.update_mouse_position();
        self.update_mouse_buttons();
        self.update_mouse_wheel();
        self.update_keyboard();
        self.update_text_input();
        self.update_mouse_cursor();
    }

    fn update_display_metrics(&mut self) {
        let io = self.context.io_mut();
        let display_size = current_display_size();
        io.display_size = display_size;

        let render_width = unsafe { ffi::GetRenderWidth() } as f32;
        let render_height = unsafe { ffi::GetRenderHeight() } as f32;
        io.display_framebuffer_scale = if display_size[0] > 0.0 && display_size[1] > 0.0 {
            [
                render_width / display_size[0],
                render_height / display_size[1],
            ]
        } else {
            [1.0, 1.0]
        };
    }

    fn update_delta_time(&mut self) {
        let frame_time = unsafe { ffi::GetFrameTime() }.max(f32::MIN_POSITIVE);
        self.context
            .io_mut()
            .update_delta_time(Duration::from_secs_f32(frame_time));
    }

    fn update_mouse_position(&mut self) {
        let io = self.context.io_mut();
        if io.want_set_mouse_pos {
            unsafe {
                ffi::SetMousePosition(io.mouse_pos[0] as i32, io.mouse_pos[1] as i32);
            }
        } else {
            io.add_mouse_pos_event([
                unsafe { ffi::GetMouseX() } as f32,
                unsafe { ffi::GetMouseY() } as f32,
            ]);
        }
    }

    fn update_mouse_buttons(&mut self) {
        let next = [
            mouse_button_down(ffi::MouseButton::MOUSE_BUTTON_LEFT as i32),
            mouse_button_down(ffi::MouseButton::MOUSE_BUTTON_RIGHT as i32),
            mouse_button_down(ffi::MouseButton::MOUSE_BUTTON_MIDDLE as i32),
            mouse_button_down(ffi::MouseButton::MOUSE_BUTTON_SIDE as i32),
            mouse_button_down(ffi::MouseButton::MOUSE_BUTTON_EXTRA as i32),
        ];
        let io = self.context.io_mut();
        for (index, (&down, state)) in next
            .iter()
            .zip(self.mouse_button_states.iter_mut())
            .enumerate()
        {
            if down != *state {
                io.add_mouse_button_event(MouseButton::VARIANTS[index], down);
                *state = down;
            }
        }
    }

    fn update_mouse_wheel(&mut self) {
        let wheel = unsafe { ffi::GetMouseWheelMoveV() };
        if wheel.x != 0.0 || wheel.y != 0.0 {
            self.context
                .io_mut()
                .add_mouse_wheel_event([wheel.x, wheel.y]);
        }
    }

    fn update_keyboard(&mut self) {
        let io = self.context.io_mut();
        for ((raylib_key, imgui_key), state) in KEY_MAPPINGS.iter().zip(self.key_states.iter_mut())
        {
            let down = key_down(*raylib_key);
            if down != *state {
                io.add_key_event(*imgui_key, down);
                *state = down;
            }
        }

        let modifier_values = [
            key_ctrl_down(),
            key_shift_down(),
            key_alt_down(),
            key_super_down(),
        ];
        for (modifier_index, modifier_key) in
            [Key::ModCtrl, Key::ModShift, Key::ModAlt, Key::ModSuper]
                .into_iter()
                .enumerate()
        {
            let down = modifier_values[modifier_index];
            if down != self.modifier_states[modifier_index] {
                io.add_key_event(modifier_key, down);
                self.modifier_states[modifier_index] = down;
            }
        }
    }

    fn update_text_input(&mut self) {
        let io = self.context.io_mut();
        loop {
            let codepoint = unsafe { ffi::GetCharPressed() };
            if codepoint == 0 {
                break;
            }
            if let Some(ch) = char::from_u32(codepoint as u32) {
                io.add_input_character(ch);
            }
        }
    }

    fn update_mouse_cursor(&mut self) {
        let desired_cursor = self.context.mouse_cursor();
        let io = self.context.io_mut();
        if io
            .config_flags
            .contains(ConfigFlags::NO_MOUSE_CURSOR_CHANGE)
        {
            return;
        }
        let draw_cursor = io.mouse_draw_cursor;
        if desired_cursor == self.current_mouse_cursor && draw_cursor == self.prev_mouse_draw_cursor
        {
            return;
        }

        self.current_mouse_cursor = desired_cursor;
        self.prev_mouse_draw_cursor = draw_cursor;
        if draw_cursor || desired_cursor.is_none() {
            unsafe {
                ffi::HideCursor();
            }
            return;
        }

        unsafe {
            ffi::ShowCursor();
        }
        if let Some(cursor) = desired_cursor {
            unsafe {
                ffi::SetMouseCursor(CURSOR_MAP[cursor as usize]);
            }
        }
    }
}

fn render_draw_data(
    draw_data: &DrawData,
    warned_reset_render_state: &mut bool,
    warned_raw_callback: &mut bool,
) {
    if draw_data.total_vtx_count == 0 {
        return;
    }

    unsafe {
        ffi::rlDrawRenderBatchActive();
        ffi::rlDisableBackfaceCulling();
    }

    for draw_list in draw_data.draw_lists() {
        let index_buffer = draw_list.idx_buffer();
        let vertex_buffer = draw_list.vtx_buffer();

        for command in draw_list.commands() {
            match command {
                DrawCmd::Elements { count, cmd_params } => {
                    let Some(scissor) = scissor_rect(
                        cmd_params.clip_rect,
                        draw_data.display_pos,
                        draw_data.display_size,
                        draw_data.framebuffer_scale,
                    ) else {
                        continue;
                    };
                    unsafe {
                        ffi::rlEnableScissorTest();
                        ffi::rlScissor(scissor.x, scissor.y, scissor.width, scissor.height);
                    }
                    render_elements(
                        count,
                        cmd_params.idx_offset,
                        index_buffer,
                        vertex_buffer,
                        cmd_params.texture_id,
                    );
                    // Flush before the next scissor change; rlgl scissor is GPU
                    // state outside the batch, so pending vertices must be
                    // submitted before the rectangle changes.
                    unsafe {
                        ffi::rlDrawRenderBatchActive();
                    }
                }
                DrawCmd::ResetRenderState => {
                    if !*warned_reset_render_state {
                        warn!(
                            "ImguiBridge encountered DrawCmd::ResetRenderState; using the bridge default rlgl state"
                        );
                        *warned_reset_render_state = true;
                    }
                }
                DrawCmd::RawCallback { .. } => {
                    if !*warned_raw_callback {
                        warn!(
                            "ImguiBridge encountered an unsupported raw ImGui draw callback; skipping it"
                        );
                        *warned_raw_callback = true;
                    }
                }
            }
        }
    }

    unsafe {
        ffi::rlSetTexture(0);
        ffi::rlDisableScissorTest();
        ffi::rlEnableBackfaceCulling();
    }
}

impl Drop for ImguiBridge {
    fn drop(&mut self) {
        if self.font_texture.id != 0 {
            unsafe {
                ffi::UnloadTexture(*self.font_texture);
            }
        }
    }
}

fn build_font_texture(fonts: &mut imgui::FontAtlas) -> Result<ffi::Texture2D, String> {
    let texture = fonts.build_rgba32_texture();
    let image_width =
        i32::try_from(texture.width).map_err(|_| "imgui font atlas width overflowed i32")?;
    let image_height =
        i32::try_from(texture.height).map_err(|_| "imgui font atlas height overflowed i32")?;
    let image = unsafe {
        let image = ffi::GenImageColor(
            image_width,
            image_height,
            ffi::Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            },
        );
        if image.data.is_null() {
            return Err("failed to allocate imgui font atlas image".to_string());
        }
        std::ptr::copy_nonoverlapping(
            texture.data.as_ptr(),
            image.data.cast::<u8>(),
            texture.data.len(),
        );
        image
    };
    let font_texture = unsafe { ffi::LoadTextureFromImage(image) };
    unsafe {
        ffi::UnloadImage(image);
    }
    if font_texture.id == 0 {
        return Err("failed to upload imgui font atlas texture".to_string());
    }
    Ok(font_texture)
}

fn current_display_size() -> [f32; 2] {
    if unsafe { ffi::IsWindowFullscreen() } {
        let monitor = unsafe { ffi::GetCurrentMonitor() };
        [
            unsafe { ffi::GetMonitorWidth(monitor) } as f32,
            unsafe { ffi::GetMonitorHeight(monitor) } as f32,
        ]
    } else {
        [
            unsafe { ffi::GetScreenWidth() } as f32,
            unsafe { ffi::GetScreenHeight() } as f32,
        ]
    }
}

fn key_down(key: i32) -> bool {
    unsafe { ffi::IsKeyDown(key) }
}

fn mouse_button_down(button: i32) -> bool {
    unsafe { ffi::IsMouseButtonDown(button) }
}

fn key_ctrl_down() -> bool {
    key_down(ffi::KeyboardKey::KEY_RIGHT_CONTROL as i32)
        || key_down(ffi::KeyboardKey::KEY_LEFT_CONTROL as i32)
}

fn key_shift_down() -> bool {
    key_down(ffi::KeyboardKey::KEY_RIGHT_SHIFT as i32)
        || key_down(ffi::KeyboardKey::KEY_LEFT_SHIFT as i32)
}

fn key_alt_down() -> bool {
    key_down(ffi::KeyboardKey::KEY_RIGHT_ALT as i32)
        || key_down(ffi::KeyboardKey::KEY_LEFT_ALT as i32)
}

fn key_super_down() -> bool {
    key_down(ffi::KeyboardKey::KEY_RIGHT_SUPER as i32)
        || key_down(ffi::KeyboardKey::KEY_LEFT_SUPER as i32)
}

fn scissor_rect(
    clip_rect: [f32; 4],
    display_pos: [f32; 2],
    display_size: [f32; 2],
    framebuffer_scale: [f32; 2],
) -> Option<ScissorRect> {
    let clip_min_x = clip_rect[0] - display_pos[0];
    let clip_min_y = clip_rect[1] - display_pos[1];
    let clip_max_x = clip_rect[2] - display_pos[0];
    let clip_max_y = clip_rect[3] - display_pos[1];
    let width = clip_max_x - clip_min_x;
    let height = clip_max_y - clip_min_y;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some(ScissorRect {
        x: (clip_min_x * framebuffer_scale[0]) as i32,
        y: ((display_size[1] - (clip_min_y + height)) * framebuffer_scale[1]) as i32,
        width: (width * framebuffer_scale[0]) as i32,
        height: (height * framebuffer_scale[1]) as i32,
    })
}

fn render_elements(
    count: usize,
    index_start: usize,
    index_buffer: &[imgui::DrawIdx],
    vertex_buffer: &[imgui::DrawVert],
    texture_id: TextureId,
) {
    let texture_ptr = texture_id.id() as *const ffi::Texture2D;
    let texture_gl_id = if texture_ptr.is_null() {
        0
    } else {
        unsafe { (*texture_ptr).id }
    };

    unsafe {
        ffi::rlBegin(RL_TRIANGLES);
        ffi::rlSetTexture(texture_gl_id);
    }

    for chunk in index_buffer[index_start..index_start + count].chunks_exact(3) {
        unsafe {
            if ffi::rlCheckRenderBatchLimit(3) {
                ffi::rlBegin(RL_TRIANGLES);
                ffi::rlSetTexture(texture_gl_id);
            }
        }

        render_vertex(vertex_buffer[chunk[0] as usize]);
        render_vertex(vertex_buffer[chunk[1] as usize]);
        render_vertex(vertex_buffer[chunk[2] as usize]);
    }

    unsafe {
        ffi::rlEnd();
    }
}

fn render_vertex(vertex: imgui::DrawVert) {
    let color = vertex.col;
    unsafe {
        ffi::rlColor4ub(color[0], color[1], color[2], color[3]);
        ffi::rlTexCoord2f(vertex.uv[0], vertex.uv[1]);
        ffi::rlVertex2f(vertex.pos[0], vertex.pos[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::{ScissorRect, scissor_rect};

    #[test]
    fn scissor_rect_flips_y_for_raylib() {
        let rect = scissor_rect(
            [10.0, 20.0, 30.0, 50.0],
            [0.0, 0.0],
            [100.0, 80.0],
            [1.0, 1.0],
        );
        assert_eq!(
            rect,
            Some(ScissorRect {
                x: 10,
                y: 30,
                width: 20,
                height: 30,
            })
        );
    }

    #[test]
    fn scissor_rect_returns_none_for_empty_rect() {
        assert_eq!(
            scissor_rect(
                [10.0, 20.0, 10.0, 50.0],
                [0.0, 0.0],
                [100.0, 80.0],
                [1.0, 1.0],
            ),
            None
        );
    }
}
