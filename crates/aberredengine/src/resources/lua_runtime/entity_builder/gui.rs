use super::*;

pub(super) fn register<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods,
        meta,
        "with_gui_window",
        "Set GuiWindow component (themed panel, drawn via the named theme looked up in GuiThemeStore (see :with_gui_theme_key)). Requires :with_screen_position() and :with_zindex() to render.",
        [("width", "number"), ("height", "number")],
        |_, this: &mut LuaEntityBuilder, (width, height): (f32, f32)| {
            this.cmd.gui_window = Some(GuiWindow::new(width, height));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_offset",
        "Set GuiOffset (position relative to the parent, resolved each frame by gui_layout_system). Requires :with_parent() first.",
        [("x", "number"), ("y", "number")],
        |_, this: &mut LuaEntityBuilder, (x, y): (f32, f32)| {
            if this.cmd.parent.is_none() {
                return Err(LuaError::runtime(
                    "with_gui_offset() requires with_parent() first",
                ));
            }
            this.cmd.gui_offset = Some((x, y));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_button",
        "Set GuiButton component; gui_button_spawn_system spawns a co-located GuiInteractable plus a caption DynamicText child on Added<GuiButton>, themed via GuiTheme.font/font_size/text_color (see engine.set_gui_theme_font). An empty `label` skips spawning the caption entirely (captionless button). Requires :with_screen_position() (or :with_parent()+:with_gui_offset()) and :with_zindex() to render.",
        [
            ("width", "number"),
            ("height", "number"),
            ("label", "string"),
            ("callback_name", "string")
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (width, height, label, callback_name): (f32, f32, String, String)| {
            this.cmd.gui_button = Some(GuiButton::with_lua_callback(
                width,
                height,
                label,
                callback_name,
            ));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_button_disabled",
        "Mark a GuiButton authored-disabled — gui_button_spawn_system applies this to the spawned GuiInteractable's state. Requires :with_gui_button() first.",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            let Some(btn) = this.cmd.gui_button.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_button_disabled() requires with_gui_button() first",
                ));
            };
            btn.disabled = true;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_label",
        "Set GuiLabel component; gui_label_spawn_system spawns a caption DynamicText child on Added<GuiLabel>, themed via the named theme looked up in GuiThemeStore (see engine.set_gui_theme_font / :with_gui_theme_key). An empty `text` skips spawning the caption entirely (captionless label). Requires :with_screen_position() (or :with_parent()+:with_gui_offset()) and :with_zindex() to render.",
        [
            ("width", "number"),
            ("height", "number"),
            ("text", "string")
        ],
        |_, this: &mut LuaEntityBuilder, (width, height, text): (f32, f32, String)| {
            this.cmd.gui_label = Some(GuiLabel::new(width, height, text));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_label_signal_binding",
        "Bind a GuiLabel's caption to a WorldSignal value -- gui_label_spawn_system attaches a SignalBinding to the caption DynamicText child, kept in sync by update_world_signals_binding_system. The label's caption text (set via :with_gui_label) remains the placeholder shown until the signal key first resolves. Requires :with_gui_label() first.",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            let Some(label) = this.cmd.gui_label.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_label_signal_binding() requires with_gui_label() first",
                ));
            };
            label.signal_binding = Some((key, None));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_label_signal_binding_format",
        "Set format string for a GuiLabel's signal binding (use {} as placeholder). Requires :with_gui_label_signal_binding() first.",
        [("format", "string")],
        |_, this: &mut LuaEntityBuilder, format: String| {
            let Some((_, fmt)) = this
                .cmd
                .gui_label
                .as_mut()
                .and_then(|label| label.signal_binding.as_mut())
            else {
                return Err(LuaError::runtime(
                    "with_gui_label_signal_binding_format() requires with_gui_label_signal_binding() first",
                ));
            };
            *fmt = Some(format);
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_theme_key",
        "Set the theme lookup key (GuiThemeStore) for a GuiWindow/GuiButton/GuiLabel/GuiProgressBar (default \"default\"). Requires one of :with_gui_window()/:with_gui_button()/:with_gui_label()/:with_gui_progress_bar() first.",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            let key: std::sync::Arc<str> = std::sync::Arc::from(key.as_str());
            fn apply<T: Themed>(opt: &mut Option<T>, key: &std::sync::Arc<str>) -> bool {
                if let Some(t) = opt.as_mut() {
                    *t.theme_key_mut() = key.clone();
                    true
                } else {
                    false
                }
            }
            if !apply(&mut this.cmd.gui_window, &key)
                && !apply(&mut this.cmd.gui_button, &key)
                && !apply(&mut this.cmd.gui_label, &key)
                && !apply(&mut this.cmd.gui_progress_bar, &key)
            {
                return Err(LuaError::runtime(
                    "with_gui_theme_key() requires with_gui_window()/with_gui_button()/with_gui_label()/with_gui_progress_bar() first",
                ));
            }
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_image",
        "Set GuiImage component; gui_image_spawn_system spawns a co-located GuiInteractable + Sprite on Added<GuiImage> (no caption child, unlike GuiButton/GuiLabel). `offset_x`/`offset_y` select the atlas sub-rect within `tex_key` (mirrors Sprite.offset; size doubles as source-rect size and render size) — this is the Normal-state offset; see :with_gui_image_hover_offset()/:with_gui_image_pressed_offset()/:with_gui_image_disabled_offset() for per-state offsets (each falls back to this one when unset). An empty `callback_name` skips wiring a click callback (the image still hit-tests/hovers/presses, it just has nothing to dispatch). Requires :with_screen_position() (or :with_parent()+:with_gui_offset()) and :with_zindex() to render.",
        [
            ("width", "number"),
            ("height", "number"),
            ("tex_key", "string"),
            ("offset_x", "number"),
            ("offset_y", "number"),
            ("callback_name", "string")
        ],
        |_,
         this: &mut LuaEntityBuilder,
         (width, height, tex_key, offset_x, offset_y, callback_name): (
            f32,
            f32,
            String,
            f32,
            f32,
            String
        )| {
            this.cmd.gui_image = Some(GuiImage::with_lua_callback(
                width,
                height,
                tex_key,
                offset_x,
                offset_y,
                callback_name,
            ));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_image_hover_offset",
        "Set the atlas offset GuiImage uses while GuiInteractable.state == Hovered. gui_image_state_sync_system applies this to Sprite.offset each frame the widget is hovered. Requires :with_gui_image() first.",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(img) = this.cmd.gui_image.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_image_hover_offset() requires with_gui_image() first",
                ));
            };
            img.offset_hover = Some(Vec2::new(offset_x, offset_y));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_image_pressed_offset",
        "Set the atlas offset GuiImage uses while GuiInteractable.state == Pressed. gui_image_state_sync_system applies this to Sprite.offset each frame the widget is pressed. Requires :with_gui_image() first.",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(img) = this.cmd.gui_image.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_image_pressed_offset() requires with_gui_image() first",
                ));
            };
            img.offset_pressed = Some(Vec2::new(offset_x, offset_y));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_image_disabled_offset",
        "Set the atlas offset GuiImage uses while GuiInteractable.state == Disabled. gui_image_state_sync_system applies this to Sprite.offset each frame the widget is disabled. Requires :with_gui_image() first.",
        [("offset_x", "number"), ("offset_y", "number")],
        |_, this: &mut LuaEntityBuilder, (offset_x, offset_y): (f32, f32)| {
            let Some(img) = this.cmd.gui_image.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_image_disabled_offset() requires with_gui_image() first",
                ));
            };
            img.offset_disabled = Some(Vec2::new(offset_x, offset_y));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_progress_bar",
        "Set GuiProgressBar component (themed nine-patch fill bar, rendered directly by render_system — no spawn system). Requires :with_screen_position() (or :with_parent()+:with_gui_offset()) and :with_zindex() to render. Theme registered via engine.set_gui_theme_progress_bar(); see :with_gui_theme_key() to override the \"default\" key.",
        [
            ("width", "number"),
            ("height", "number"),
            ("value", "number"),
            ("max", "number")
        ],
        |_, this: &mut LuaEntityBuilder, (width, height, value, max): (f32, f32, f32, f32)| {
            this.cmd.gui_progress_bar = Some(GuiProgressBar::new(width, height, value, max));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_progress_bar_vertical",
        "Switch a GuiProgressBar to vertical fill direction (Vertical: fill grows bottom-to-top). Requires :with_gui_progress_bar() first.",
        [],
        |_, this: &mut LuaEntityBuilder, ()| {
            let Some(bar) = this.cmd.gui_progress_bar.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_progress_bar_vertical() requires with_gui_progress_bar() first",
                ));
            };
            bar.direction = ProgressBarDirection::Vertical;
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_progress_bar_reversed",
        "Reverse the fill anchor of a GuiProgressBar: Horizontal becomes HorizontalReversed (right-to-left), Vertical becomes VerticalReversed (top-to-bottom). Requires :with_gui_progress_bar() first.",
        [],
        |_, this: &mut LuaEntityBuilder, ()| {
            let Some(bar) = this.cmd.gui_progress_bar.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_progress_bar_reversed() requires with_gui_progress_bar() first",
                ));
            };
            bar.direction = match bar.direction {
                ProgressBarDirection::Horizontal => ProgressBarDirection::HorizontalReversed,
                ProgressBarDirection::HorizontalReversed => ProgressBarDirection::Horizontal,
                ProgressBarDirection::Vertical => ProgressBarDirection::VerticalReversed,
                ProgressBarDirection::VerticalReversed => ProgressBarDirection::Vertical,
            };
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_gui_progress_bar_signal_binding",
        "Bind a GuiProgressBar's value to a WorldSignals key (integer preferred, scalar fallback). gui_progressbar_signal_update_system reads the signal each frame and clamps to [0, max]. Requires :with_gui_progress_bar() first.",
        [("key", "string")],
        |_, this: &mut LuaEntityBuilder, key: String| {
            let Some(bar) = this.cmd.gui_progress_bar.as_mut() else {
                return Err(LuaError::runtime(
                    "with_gui_progress_bar_signal_binding() requires with_gui_progress_bar() first",
                ));
            };
            bar.signal_binding = Some(key);
            Ok(())
        }
    );
}
