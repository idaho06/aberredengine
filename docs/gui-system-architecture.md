# Aberred Engine — GUI System Reference

## Overview

The GUI system provides screen-space overlaid widgets rendered by raylib's nine-patch, sprite, and
text draw paths, driven by Bevy ECS components. All widgets are positioned in screen space (not
affected by `Camera2D`) and composited over the game world in the same render pass.

**In scope:** themed panels, buttons, image slots, text labels, signal-driven dynamic labels,
nine-patch skins, per-state atlas offsets, show/hide via tween, runtime enable/disable.

**Out of scope:** world-space widgets, automatic layout/flexbox, text input, drag-and-drop,
keyboard/gamepad focus traversal, tooltips. See [Roadmap](#roadmap--pending) for planned additions.

---

## Architecture at a Glance

Each widget type follows the same pipeline:

```
1. Spawn widget component(s)
       ↓
2. Reactive spawn system (Added<T>, runs next frame)
   • Inserts co-located GuiInteractable (for buttons/images)
   • Spawns caption DynamicText child (for buttons/labels)
   • Inserts co-located Sprite (for images)
       ↓
3. gui_layout_system  — parent ScreenPosition + child GuiOffset → child ScreenPosition
       ↓
4. gui_hit_test_system — cursor + ZIndex → GuiWidgetState; fires GuiInteractableClickEvent
       ↓
5. gui_image_state_sync_system — GuiWidgetState → Sprite.offset (for GuiImage only)
       ↓
6. render_system  — Panel < Sprite < Text (lowest to highest, within the same ZIndex)
```

The **one-frame spawn lag** is intentional: reactive spawn systems run after `Added<T>` is detected,
which is the frame after the component is inserted. Children spawned inside a `LuaSetup` callback
bypass this lag since the callback itself fires one frame after the parent spawns — by then the
parent's `GuiInteractable` and `ScreenPosition` already exist.

---

## Themes

Themes centralize the visual style (nine-patches, fonts, colors) shared across multiple widgets.

### Data types

```
GuiThemeStore                         — Resource; always present (inserted at startup)
  themes: FxHashMap<Arc<str>, GuiTheme>

GuiTheme
  panel:        GuiNinePatch              — required; used by GuiWindow
  button:       Option<GuiButtonSkin>     — None = buttons render with no background
  label:        Option<GuiNinePatch>      — None = labels render with no background
  progress_bar: Option<GuiProgressBarSkin> — None = progress bars render with no skin
  font:         Arc<str>                  — font key in FontStore; "" = no caption text rendered
  font_size:    f32                       — default 16.0
  text_color:   Color                     — default Color::WHITE
  panel_shadow: Option<Shadow>            — drop shadow behind all nine-patch backgrounds (window/button/label/progress bar)
  text_shadow:  Option<Shadow>            — Shadow inserted on DynamicText caption children at spawn time

GuiNinePatch
  tex_key:   Arc<str>
  source:    Rectangle                — pixel region within the texture
  left/top/right/bottom: i32          — border widths in pixels (maps 1:1 to raylib NPatchInfo)

GuiButtonSkin
  normal:          GuiNinePatch       — required
  hover:           Option<GuiNinePatch> — None falls back to normal
  pressed:         Option<GuiNinePatch> — None falls back to normal
  disabled:        Option<GuiNinePatch> — None falls back to normal
  shadow:          Option<Shadow>     — per-normal-state shadow; falls back to theme.panel_shadow
  hover_shadow:    Option<Shadow>     — None falls back to shadow (normal)
  pressed_shadow:  Option<Shadow>     — None falls back to shadow (normal)
  disabled_shadow: Option<Shadow>     — None falls back to shadow (normal)

GuiProgressBarSkin
  track: Option<GuiNinePatch>         — background drawn at full bar size; None = fill-only bar
  fill:  GuiNinePatch                 — required; skin is dropped if fill is unset
```

The default theme key is `"default"`. Widgets that never call `:with_gui_theme_key()` use it.
A missing or unregistered theme key causes the widget's themed background to be skipped silently
(caption/sprite still render); a warning is logged once per widget via `GuiThemeWarnCache`.

### Registering themes in Lua

Theme commands must be queued from `on_setup()` — the `gui_theme_commands` queue uses a `preserve`
policy, so commands issued in setup survive the initial scene switch. Textures and fonts must be
loaded first (also in `on_setup()`).

```lua
function on_setup()
    engine.load_texture("gui-window",  "./assets/textures/gui/window.png")
    engine.load_texture("gui-buttons", "./assets/textures/gui/button_atlas.png")
    engine.load_texture("gui-label",   "./assets/textures/gui/label.png")
    engine.load_font("ui_font", "./assets/fonts/MyFont.ttf", 128)

    -- Panel nine-patch: full 64×64 texture, 6px borders on all sides
    engine.set_gui_theme_panel("default", "gui-window", 0, 0, 64, 64, 6, 6, 6, 6)

    -- Button skin: atlas with Normal(0,0) / Pressed(0,64) / Disabled(64,64), 8px borders
    engine.set_gui_theme_button("default", "normal",   "gui-buttons", 0,  0,  64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_button("default", "pressed",  "gui-buttons", 0,  64, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_button("default", "disabled", "gui-buttons", 64, 64, 64, 64, 8, 8, 8, 8)

    -- Label background (optional)
    engine.set_gui_theme_label("default", "gui-label", 0, 0, 64, 64, 6, 6, 6, 6)

    -- Font for all captions in this theme
    engine.set_gui_theme_font("default", "ui_font", 16, 255, 255, 255, 255)

    -- Drop shadow behind all nine-patch backgrounds (panel, buttons, labels, progress bars)
    engine.set_gui_theme_panel_shadow("default", 2, 2, 0, 0, 0, 120)

    -- Per-state button shadows (unset states fall back to the normal shadow above)
    engine.set_gui_theme_button_shadow("default", "normal",   2, 2, 0, 0, 0, 120)
    engine.set_gui_theme_button_shadow("default", "pressed",  0, 0, 0, 0, 0, 0)   -- collapse on press

    -- Drop shadow on caption text children
    engine.set_gui_theme_text_shadow("default", 1, 1, 0, 0, 0, 180)
end
```

Multiple named themes coexist in `GuiThemeStore`. Register a second theme the same way with a
different key:

```lua
engine.set_gui_theme_panel("compact", "gui-window",  0, 0, 64, 64, 6, 6, 6, 6)
engine.set_gui_theme_button("compact", "normal", "gui-buttons", 0, 0, 64, 64, 8, 8, 8, 8)
engine.set_gui_theme_font("compact", "ui_font", 13, 255, 220, 120, 255)
```

### Registering themes in Rust

There is no Lua-style builder API for Rust theming. Mutate `GuiThemeStore` directly in a startup
system registered via `EngineBuilder::add_system`:

```rust
use std::sync::Arc;
use bevy_ecs::prelude::ResMut;
use raylib::prelude::{Color, Rectangle, Vector2};
use aberredengine::components::shadow::Shadow;
use aberredengine::resources::guitheme::{GuiButtonSkin, GuiNinePatch, GuiTheme, GuiThemeStore};

fn setup_gui_theme(mut theme_store: ResMut<GuiThemeStore>) {
    let panel_shadow = Some(Shadow { offset: Vector2::new(2.0, 2.0), color: Color::new(0, 0, 0, 120) });
    let theme = GuiTheme {
        panel: GuiNinePatch {
            tex_key: Arc::from("gui-window"),
            source: Rectangle::new(0.0, 0.0, 64.0, 64.0),
            left: 6, top: 6, right: 6, bottom: 6,
        },
        button: Some(GuiButtonSkin {
            normal: GuiNinePatch {
                tex_key: Arc::from("gui-buttons"),
                source: Rectangle::new(0.0, 0.0, 64.0, 64.0),
                left: 8, top: 8, right: 8, bottom: 8,
            },
            hover: None, pressed: None, disabled: None, // all fall back to normal
            shadow: panel_shadow.clone(),
            pressed_shadow: Some(Shadow { offset: Vector2::new(0.0, 0.0), color: Color::new(0, 0, 0, 0) }), // collapse on press
            hover_shadow: None, disabled_shadow: None,
        }),
        label: None,
        progress_bar: None,
        font: Arc::from("ui_font"),
        font_size: 16.0,
        text_color: Color::WHITE,
        panel_shadow,
        text_shadow: None,
    };
    theme_store.themes.insert(Arc::from("default"), theme);
}

// Registration:
// EngineBuilder::new()
//     .add_system(setup_gui_theme)
//     ...
```

---

## Widget Reference

### GuiWindow

Renders a nine-patch panel using the `panel` field of its theme.

```
GuiWindow { size: Vector2, theme_key: Arc<str> }
```

- Requires `ScreenPosition` to be visible; visibility is controlled by the presence or absence of
  `ScreenPosition` (see [Visibility & Animation](#visibility--animation)).
- Requires `ZIndex` for rendering.
- No hit-testing — `GuiWindow` has no `GuiInteractable`.

**Lua:**
```lua
engine.spawn()
    :with_gui_window(200, 150)        -- width, height
    :with_gui_theme_key("compact")    -- optional; default "default"
    :with_screen_position(10, 80)
    :with_zindex(0)
    :build()
```

**Rust:**
```rust
use aberredengine::components::{guiwindow::GuiWindow, screenposition::ScreenPosition, zindex::ZIndex};

ctx.commands.spawn((
    GuiWindow::new(200.0, 150.0),
    ScreenPosition { pos: Vector2::new(10.0, 80.0) },
    ZIndex(0.0),
));
```

---

### GuiButton

Themed, clickable button. Carries its own caption, callback name, and disabled state.

```
GuiButton { size: Vector2, caption: String, callback_name: String, disabled: bool, theme_key: Arc<str> }
```

`gui_button_spawn_system` reacts on `Added<GuiButton>` and, **one frame later**:
- Inserts a co-located `GuiInteractable` (via `insert_if_new` — a pre-existing one is kept intact).
- Spawns a caption `DynamicText` as a `ChildOf` child (skipped when `caption` is empty).

Caption font, size, and color come from the theme's `font`/`font_size`/`text_color`. An empty
`callback_name` means no click callback is wired.

**Lua:**
```lua
engine.spawn()
    :with_gui_button(100, 24, "Start Game", "on_start_clicked")
    :with_gui_button_disabled()       -- optional; starts in Disabled state
    :with_gui_theme_key("compact")    -- optional; default "default"
    :with_parent(window_entity_id)
    :with_gui_offset(16, 50)
    :with_zindex(2)
    :build()
```

**Rust (with Lua callback):**
```rust
ctx.commands.spawn((
    GuiButton::new(100.0, 24.0, "Start Game"),
    ScreenPosition { pos: Vector2::new(16.0, 50.0) },
    ZIndex(2.0),
));
```

**Rust (with Rust fn-pointer callback):**

Pre-spawn `GuiInteractable::rust(...)` alongside `GuiButton::new(...)`. The spawn system uses
`insert_if_new`, so the pre-existing interactable is kept and the button's `callback_name` is
ignored. The coercion constructor is required — see [Interaction Model](#interaction-model).

```rust
fn on_start_clicked(entity: Entity, ctx: &mut GameCtx) {
    ctx.signals.set_flag("start_game");
}

ctx.commands.spawn((
    GuiButton::new(100.0, 24.0, "Start Game"),
    GuiInteractable::rust(100.0, 24.0, on_start_clicked),
    ScreenPosition { pos: Vector2::new(16.0, 50.0) },
    ZIndex(2.0),
));
```

---

### GuiLabel

Static or signal-driven label. Never hit-tested.

```
GuiLabel { size: Vector2, caption: String, theme_key: Arc<str>,
           signal_binding: Option<(String, Option<String>)> }
```

`gui_label_spawn_system` reacts on `Added<GuiLabel>` and, **one frame later**, spawns a caption
`DynamicText` child (skipped when `caption` is empty and no signal binding is set). The `label`
nine-patch from the theme is optional — when unset, no background is rendered.

When `signal_binding` is set, `gui_label_spawn_system` attaches a `SignalBinding` component to the
caption child. `update_world_signals_binding_system` then updates the caption text from
`WorldSignals` every frame, replacing `{}` in the format string with the current signal value.

> **Note:** An empty `caption` with a signal binding produces no caption child. Use a placeholder
> string like `"0"` or `""` with a non-empty signal key if you want a binding.

**Lua:**
```lua
-- Static label
engine.spawn()
    :with_gui_label(160, 24, "Score")
    :with_parent(window_entity_id)
    :with_gui_offset(16, 16)
    :with_zindex(2)
    :build()

-- Signal-bound label (live value from WorldSignals)
engine.spawn()
    :with_gui_label(120, 24, "0")
    :with_gui_label_signal_binding("score")
    :with_gui_label_signal_binding_format("Score: {}")
    :with_gui_theme_key("compact")
    :with_parent(window_entity_id)
    :with_gui_offset(16, 44)
    :with_zindex(2)
    :build()
```

**Rust:**
```rust
ctx.commands.spawn((
    GuiLabel::new(160.0, 24.0, "0")
        .with_signal_binding("score")
        .with_signal_binding_format("Score: {}"),
    ScreenPosition { pos: Vector2::new(16.0, 44.0) },
    ZIndex(2.0),
));
```

---

### GuiImage

Clickable image slot for icon-style buttons (inventory, skill icons, etc.). No theming.

```
GuiImage { size: Vector2, tex_key: String, offset: Vector2,
           offset_hover: Option<Vector2>, offset_pressed: Option<Vector2>,
           offset_disabled: Option<Vector2>, callback_name: String }
```

`gui_image_spawn_system` reacts on `Added<GuiImage>` and, **one frame later**, inserts a
co-located `GuiInteractable` and `Sprite` **on the same entity** (not a child). The `Sprite`
free-rides the existing screen-sprite render path.

`offset` is the atlas sub-rect's top-left pixel position within `tex_key`; `size` is both the
source-rect size and the render size (same convention as `Sprite`). `gui_image_state_sync_system`
re-resolves `Sprite.offset` from `GuiInteractable.state` every frame; unset per-state offsets fall
back to `offset`.

**Lua:**
```lua
-- 2×2 atlas: Normal(0,0) Hover(32,0) Pressed(0,32) Disabled(32,32)
engine.spawn()
    :with_gui_image(32, 32, "item-atlas", 0, 0, "on_sword_clicked")
    :with_gui_image_hover_offset(32, 0)
    :with_gui_image_pressed_offset(0, 32)
    :with_gui_image_disabled_offset(32, 32)
    :with_parent(window_entity_id)
    :with_gui_offset(8, 36)
    :with_zindex(2)
    :build()
```

**Rust:**
```rust
ctx.commands.spawn((
    GuiImage::new(32.0, 32.0, "item-atlas", 0.0, 0.0)
        .with_offset_hover(32.0, 0.0)
        .with_offset_pressed(0.0, 32.0)
        .with_offset_disabled(32.0, 32.0),
    GuiInteractable::rust(32.0, 32.0, on_sword_clicked),
    ScreenPosition { pos: Vector2::new(8.0, 36.0) },
    ZIndex(2.0),
));
```

---

### GuiProgressBar

Themed nine-patch fill bar for life bars, XP meters, cooldowns, etc. No interactivity; no
spawn system; rendered directly by `render_system`.

```
GuiProgressBar { size: Vector2, value: f32, max: f32,
                 direction: ProgressBarDirection, theme_key: Arc<str>,
                 signal_binding: Option<String> }

ProgressBarDirection: Horizontal (default) | HorizontalReversed | Vertical | VerticalReversed
```

The **track** (background) nine-patch is optional — set `part = "track"` to show a background, or omit
it for a fill-only bar. The **fill** nine-patch is required.

#### Theme registration

Call from `on_setup()` (the `gui_theme_commands` queue has `preserve` policy, surviving scene
switches):

```lua
-- on_setup():
engine.set_gui_theme_progress_bar("default", "track", "gui-bars", 0,  0, 64, 16, 4, 4, 4, 4)
engine.set_gui_theme_progress_bar("default", "fill",  "gui-bars", 0, 16, 64, 16, 4, 4, 4, 4)
```

Signature: `engine.set_gui_theme_progress_bar(theme_key, part, tex_key, sx, sy, sw, sh, left, top, right, bottom)`
- `part`: `"track"` or `"fill"`

#### Direction variants

| Direction          | Fill anchor       | Fill grows toward |
|--------------------|-------------------|-------------------|
| `Horizontal`       | left edge         | right             |
| `HorizontalReversed` | right edge      | left              |
| `Vertical`         | bottom edge       | top               |
| `VerticalReversed` | top edge          | bottom            |

#### Lua spawn

```lua
-- Static value bar
local bar = engine.spawn()
    :with_gui_progress_bar(200, 16, 75, 100)  -- 75/100
    :with_screen_position(10, 10)
    :with_zindex(1)
    :build()

-- Signal-bound (auto-updated each frame)
engine.spawn()
    :with_gui_progress_bar(200, 16, 100, 100)
    :with_gui_progress_bar_signal_binding("player_hp")
    :with_screen_position(10, 30)
    :with_zindex(1)
    :build()
engine.set_integer("player_hp", 40)  -- bar shows 40%

-- Reversed vertical (top-to-bottom fill)
engine.spawn()
    :with_gui_progress_bar(16, 200, 60, 100)
    :with_gui_progress_bar_vertical()
    :with_gui_progress_bar_reversed()
    :with_screen_position(220, 10)
    :with_zindex(1)
    :build()
```

Builder modifiers (all require `:with_gui_progress_bar()` first):
- `:with_gui_progress_bar_vertical()` — sets direction to `Vertical`
- `:with_gui_progress_bar_reversed()` — flips `Horizontal`↔`HorizontalReversed` or `Vertical`↔`VerticalReversed`
- `:with_gui_progress_bar_signal_binding(key)` — integer signal preferred, scalar fallback
- `:with_gui_theme_key(key)` — override the default `"default"` theme

#### Runtime update

```lua
engine.entity_set_gui_progress(bar_id, 40)      -- set value (clamped to [0, max])
engine.entity_set_gui_progress_max(bar_id, 200)  -- change max (value re-clamped)
```

#### Rust spawn

```rust
world.spawn((
    GuiProgressBar::new(200.0, 16.0, 75.0, 100.0),
    ScreenPosition { pos: Vector2::new(10.0, 10.0) },
    ZIndex(1.0),
));
```

#### Signal binding system

`gui_progressbar_signal_update_system` runs every frame before `render_system` and updates all
`GuiProgressBar` components with a `signal_binding`. It reads the integer signal first; if absent, falls
back to the scalar. Values are clamped to `[0, max]`. Skips the write if the new value equals the
current one (avoids spurious change detection).

---

## Layout System

`GuiOffset(Vector2)` stores a child widget's position relative to its `ChildOf` parent.

`gui_layout_system` runs every frame (after `tween_system::<ScreenPosition>`, before
`render_system`) and resolves each child's `ScreenPosition`:

```
child.ScreenPosition = parent.ScreenPosition + child.GuiOffset
```

`ChildOf` is used for two things: lifecycle (cascade despawn when the parent despawns) and
`GuiOffset` resolution. It has no effect on rendering order (use `ZIndex` for that).

**Widget children must always have `ZIndex` higher than their parent** to render in front of the
parent's nine-patch background.

### Window/children build order in Lua

Lua cannot get an entity's ID synchronously on the same frame it is spawned. To attach children
to a window, use `:with_lua_setup("callback")` on the window and spawn children inside the
callback (which fires the frame after the window entity exists):

```lua
-- Spawn window; children are deferred to build_my_window callback
engine.spawn()
    :with_gui_window(200, 150)
    :with_screen_position(10, 80)
    :with_zindex(0)
    :with_lua_setup("build_my_window")
    :build()

-- Called one frame later; ctx.id is the window's live entity id
local function build_my_window(ctx)
    engine.spawn()
        :with_gui_label(160, 24, "Hello!")
        :with_parent(ctx.id)
        :with_gui_offset(16, 16)
        :with_zindex(2)
        :build()

    engine.spawn()
        :with_gui_button(100, 24, "OK", "on_ok_clicked")
        :with_parent(ctx.id)
        :with_gui_offset(16, 50)
        :with_zindex(2)
        :build()
end
```

In Rust, children can be spawned in the same `on_enter` call since entity IDs are available
synchronously via `commands.spawn(...).id()`.

### Visibility cascade

When a parent window's `ScreenPosition` is removed, `gui_layout_system` stops writing the
children's `ScreenPosition`, so they are also hidden that frame (they retain stale positions
but the layout system will overwrite them once the parent reappears). Children's `ScreenPosition`
components are effectively read-only — do not set them manually; use `GuiOffset` instead.

---

## Visibility & Animation

Visibility is the **presence or absence** of `ScreenPosition` — not a boolean flag. A widget with
no `ScreenPosition` component is not rendered and not hit-tested.

| Action | Lua | Rust |
|--------|-----|------|
| Show (instant) | `entity_insert_tween_screen_position(id, x, y, x, y, 0, "linear", "once", false, "")` | insert `ScreenPosition` |
| Move | `entity_set_screen_position(id, x, y)` | mutate `ScreenPosition` |
| Hide (instant) | `entity_remove_screen_position(id)` | remove `ScreenPosition` |
| Slide in | `entity_insert_tween_screen_position(id, from_x, from_y, to_x, to_y, dur, easing, "once", false, "")` | insert `Tween<ScreenPosition>` |
| Slide out then hide | tween with `on_finished` callback → call `entity_remove_screen_position` | observe `TweenFinishedEvent<ScreenPosition>` → remove |

`entity_set_screen_position` only mutates an existing `ScreenPosition` — it is a no-op if the
entity has none. Use `entity_insert_tween_screen_position` with zero duration to both insert and
snap in a single call (the only way to give a hidden widget a ScreenPosition from Lua in one step).

**Slide-in example:**
```lua
-- Slide window in from off-screen bottom
engine.entity_insert_tween_screen_position(
    window_id,
    10, 400,   -- from (off-screen)
    10, 80,    -- to (resting position)
    0.5, "quad_out", "once", false, ""
)
```

**Slide-out + hide example:**
```lua
engine.entity_insert_tween_screen_position(
    window_id,
    10, 80,    -- from
    10, 400,   -- to (off-screen)
    0.5, "quad_in", "once", false, "on_hide_done"
)

local function on_hide_done(ctx)
    engine.entity_remove_screen_position(ctx.id)
end
```

**Tint for fades:** attach a `Tint` component (or mutate it from a callback) to fade a widget's
colors. Removing `ScreenPosition` is separate from and not implied by `Tint`.

---

## Interaction Model

`GuiInteractable` is the shared hit-test and click runtime for all clickable widgets.

```
GuiInteractable { size: Vector2, state: GuiWidgetState,
                  on_click_callback: Option<String>,
                  on_rust_callback:  Option<GuiRustCallback> }

GuiWidgetState: Normal | Hovered | Pressed | Disabled
```

`gui_button_spawn_system` and `gui_image_spawn_system` insert `GuiInteractable` automatically (via
`insert_if_new`). For Rust fn-pointer callbacks, pre-spawn your own `GuiInteractable::rust(...)`
alongside the widget component — the spawn system will leave it intact.

### Hit-test algorithm (`gui_hit_test_system`)

1. For every `GuiInteractable` that has a `ScreenPosition`, test the cursor against the AABB
   `[pos, pos + size)`.
2. Among all hits, the one with the highest `ZIndex` wins.
3. Set winner's state to `Pressed` (if mouse button down) or `Hovered`; set all others to
   `Normal`. `Disabled` state is never overwritten.
4. On press-then-release-inside: fire `GuiInteractableClickEvent { entity }` and set
   `GuiInputState.click_consumed_this_frame = true` to prevent the click from hitting anything
   else this frame.

`Disabled` widgets participate in hit-testing (they block clicks below them in the Z stack) but
their `GuiWidgetState` is never promoted to `Hovered` or `Pressed`.

### Click callbacks

`gui_interactable_click_observer` handles `GuiInteractableClickEvent`:
1. Check `on_click_callback` (Lua function name) — call it if found.
2. Check `on_rust_callback` (fn-pointer) — call it if found.
3. Skip if `state == Disabled`.

**Lua click callback:** receives a table with `evt.entity_id` (u64 entity ID).

```lua
local function on_button_clicked(evt)
    local id = evt.entity_id   -- u64; use to call entity_set_gui_disabled, etc.
    engine.entity_set_gui_disabled(id, true)
end
```

> **Breaking rename:** `evt.entity_id` replaced the old `evt.button_id` field.

**Rust fn-pointer callback:** use `GuiInteractable::rust(...)` — the typed parameter forces
coercion from function-item to `fn(...)` pointer, which `Query<&GuiInteractable>` requires.
Without the coercion the query silently matches nothing (same gotcha as `CollisionRule::rust`).

```rust
pub type GuiRustCallback = for<'w, 's> fn(Entity, &mut GameCtx<'w, 's>);

fn on_my_button_clicked(entity: Entity, ctx: &mut GameCtx) {
    ctx.signals.set_flag("action_triggered");
}

// Correct:
GuiInteractable::rust(100.0, 24.0, on_my_button_clicked)

// Wrong — query won't match:
GuiInteractable { on_rust_callback: Some(on_my_button_clicked), .. }
```

### Runtime enable/disable

```lua
engine.entity_set_gui_disabled(entity_id, true)   -- disable
engine.entity_set_gui_disabled(entity_id, false)  -- re-enable
```

From Rust, mutate `GuiInteractable.state` directly via `Query<&mut GuiInteractable>` (available
in `GameCtx.gui_interactables`).

> **Spawn-frame caveat:** `entity_set_gui_disabled` is a no-op on the same frame as spawn, because
> `GuiInteractable` doesn't exist yet. Use `:with_gui_button_disabled()` (Lua) or
> `GuiButton::new(...).with_disabled()` (Rust) to set the initial disabled state at spawn time.

---

## Per-State Visual Feedback

### GuiButton — nine-patch skins

The render system picks the nine-patch for the current `GuiWidgetState` from `GuiButtonSkin`:

| State    | Patch used           |
|----------|----------------------|
| Normal   | `skin.normal`        |
| Hovered  | `skin.hover` or `normal` |
| Pressed  | `skin.pressed` or `normal` |
| Disabled | `skin.disabled` or `normal` |

All `Option` fields fall back to `normal` when unset — a one-patch theme is valid.

### GuiImage — per-state atlas offsets

`gui_image_state_sync_system` re-resolves `Sprite.offset` from `GuiInteractable.state` every
frame, writing only when the resolved value changes:

| State    | Offset used                        |
|----------|------------------------------------|
| Normal   | `GuiImage.offset`                  |
| Hovered  | `GuiImage.offset_hover` or `offset` |
| Pressed  | `GuiImage.offset_pressed` or `offset` |
| Disabled | `GuiImage.offset_disabled` or `offset` |

Atlas layout example (2×2 cells, 32×32 each):

```
(0,0)  Normal    (32,0)  Hover
(0,32) Pressed   (32,32) Disabled
```

```lua
:with_gui_image(32, 32, "icon-atlas", 0, 0, "on_icon_clicked")
:with_gui_image_hover_offset(32, 0)
:with_gui_image_pressed_offset(0, 32)
:with_gui_image_disabled_offset(32, 32)
```

Tint-based feedback (e.g. darken on press) is not automatic for any widget type — set a `Tint`
component from a click or phase callback when you need it.

---

## Signal-Bound Labels

A `GuiLabel` can display a live value from `WorldSignals` without polling from Lua each frame.

Set `signal_binding` with an optional format string (use `{}` as the value placeholder). The signal
key can be a scalar, integer, or string signal. `update_world_signals_binding_system` writes the
formatted value to the caption `DynamicText.text` every frame.

```lua
engine.spawn()
    :with_gui_label(120, 24, "HP: 100")     -- placeholder until signal resolves
    :with_gui_label_signal_binding("char_hp")
    :with_gui_label_signal_binding_format("HP: {}")
    :with_parent(window_id)
    :with_gui_offset(8, 162)
    :with_zindex(2)
    :build()

-- Update the value from anywhere:
engine.set_integer("char_hp", 75)  -- label auto-updates to "HP: 75"
```

> **Caption child requirement:** the caption child is only spawned when `caption` is non-empty OR
> a signal binding is set. An empty `caption` with no binding produces a captionless label. When
> using a binding, pass a non-empty placeholder like `"0"` so the caption child is spawned.

**Rust:**
```rust
GuiLabel::new(120.0, 24.0, "HP: 100")
    .with_signal_binding("char_hp")
    .with_signal_binding_format("HP: {}")
```

---

## Complete Lua Example

Full scene with two windows, a click handler, a signal-bound label, a `GuiImage`, and
show/hide animation. Derived from `assets/scripts/scenes/gui_demo.lua`.

```lua
-- setup.lua — on_setup() (runs before any scene; safe for theme registration)
function on_setup()
    engine.load_texture("gui-window",  "./assets/textures/gui/window.png")
    engine.load_texture("gui-buttons", "./assets/textures/gui/button_atlas.png")
    engine.load_texture("gui-icons",   "./assets/textures/gui/icon_atlas.png")
    engine.load_texture("gui-bars",    "./assets/textures/gui/progress_bar_8_8_8_8.png")
    engine.load_font("ui_font", "./assets/fonts/MyFont.ttf", 128)

    engine.set_gui_theme_panel("default", "gui-window", 0, 0, 64, 64, 6, 6, 6, 6)
    engine.set_gui_theme_button("default", "normal",   "gui-buttons", 0,  0,  64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_button("default", "pressed",  "gui-buttons", 0,  64, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_button("default", "disabled", "gui-buttons", 64, 64, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_font("default", "ui_font", 16, 255, 255, 255, 255)
    -- Progress bar nine-patches (top row = horizontal track/fill)
    engine.set_gui_theme_progress_bar("default", "track", "gui-bars", 0,  0, 64, 64, 8, 8, 8, 8)
    engine.set_gui_theme_progress_bar("default", "fill",  "gui-bars", 64, 0, 64, 64, 8, 8, 8, 8)
end

-- scenes/my_scene.lua
local M = {}
local PANEL_X, PANEL_Y = 10, 80

--- Spawns children of the main panel one frame after the panel entity exists.
local function build_main_panel(ctx)
    -- HP progress bar (horizontal, signal-bound; starts full at 100/100)
    engine.spawn()
        :with_gui_progress_bar(160, 16, 100, 100)
        :with_gui_progress_bar_signal_binding("player_hp")
        :with_parent(ctx.id)
        :with_gui_offset(16, 16)
        :with_zindex(2)
        :build()

    -- Attack icon button (GuiImage with 2×2 atlas states)
    engine.spawn()
        :with_gui_image(32, 32, "gui-icons", 0, 0, "on_attack_clicked")
        :with_gui_image_hover_offset(32, 0)
        :with_gui_image_pressed_offset(0, 32)
        :with_gui_image_disabled_offset(32, 32)
        :register_as("attack_btn")
        :with_parent(ctx.id)
        :with_gui_offset(16, 44)
        :with_zindex(2)
        :build()

    -- Regular button
    engine.spawn()
        :with_gui_button(100, 24, "Retreat", "on_retreat_clicked")
        :with_parent(ctx.id)
        :with_gui_offset(16, 84)
        :with_zindex(2)
        :build()
end

local function on_attack_clicked(evt)
    -- Disable button until cooldown expires, then re-enable via timer
    engine.entity_set_gui_disabled(evt.entity_id, true)
    engine.entity_insert_lua_timer(evt.entity_id, 3.0, "on_attack_cooldown_done")
    engine.set_integer("player_hp", engine.get_integer("player_hp") - 10)
end

local function on_attack_cooldown_done(ctx)
    engine.entity_remove_lua_timer(ctx.id)
    engine.entity_set_gui_disabled(ctx.id, false)
end

local function on_retreat_clicked()
    engine.change_scene("menu")
end

function M.spawn()
    engine.set_integer("player_hp", 100)  -- integer signal; progress bar binding reads this directly

    -- Main panel: :with_lua_setup defers child spawning to next frame
    engine.spawn()
        :with_gui_window(200, 130)
        :with_screen_position(PANEL_X, PANEL_Y)
        :with_zindex(0)
        :with_lua_setup("build_main_panel")
        :build()
end

M._callbacks = {
    build_main_panel         = build_main_panel,
    on_attack_clicked        = on_attack_clicked,
    on_attack_cooldown_done  = on_attack_cooldown_done,
    on_retreat_clicked       = on_retreat_clicked,
}

return M
```

---

## Complete Rust Example

Equivalent setup in a Rust-only game (no Lua).

```rust
use std::sync::Arc;
use bevy_ecs::prelude::*;
use raylib::prelude::{Color, Rectangle, Vector2};
use aberredengine::{
    components::{
        guibutton::GuiButton,
        guilabel::GuiLabel,
        guiinteractable::GuiInteractable,
        guiwindow::GuiWindow,
        screenposition::ScreenPosition,
        shadow::Shadow,
        zindex::ZIndex,
    },
    resources::guitheme::*,
    systems::{scene_dispatch::SceneDescriptor, GameCtx},
};

// ── Theme setup (runs once at startup) ──────────────────────────────────────

fn setup_gui_theme(mut theme_store: ResMut<GuiThemeStore>) {
    theme_store.themes.insert(Arc::from("default"), GuiTheme {
        panel: GuiNinePatch {
            tex_key: Arc::from("gui-window"),
            source: Rectangle::new(0.0, 0.0, 64.0, 64.0),
            left: 6, top: 6, right: 6, bottom: 6,
        },
        button: Some(GuiButtonSkin {
            normal: GuiNinePatch {
                tex_key: Arc::from("gui-buttons"),
                source: Rectangle::new(0.0, 0.0, 64.0, 64.0),
                left: 8, top: 8, right: 8, bottom: 8,
            },
            hover: None, pressed: None, disabled: None,
            shadow: None, hover_shadow: None, pressed_shadow: None, disabled_shadow: None,
        }),
        label: None,
        progress_bar: None,
        font: Arc::from("ui_font"),
        font_size: 16.0,
        text_color: Color::WHITE,
        panel_shadow: None,
        text_shadow: None,
    });
}

// ── Scene enter callback ─────────────────────────────────────────────────────

fn scene_enter(ctx: &mut GameCtx) {
    // Window
    let window = ctx.commands.spawn((
        GuiWindow::new(200.0, 130.0),
        ScreenPosition { pos: Vector2::new(10.0, 80.0) },
        ZIndex(0.0),
    )).id();

    // HP label child
    ctx.commands.spawn((
        GuiLabel::new(160.0, 20.0, "HP: 100")
            .with_signal_binding("player_hp")
            .with_signal_binding_format("HP: {}"),
        ChildOf(window),
        GuiOffset(Vector2::new(16.0, 16.0)),
        ZIndex(2.0),
    ));

    // Retreat button with Rust fn-pointer callback
    ctx.commands.spawn((
        GuiButton::new(100.0, 24.0, "Retreat"),
        GuiInteractable::rust(100.0, 24.0, on_retreat_clicked),
        ChildOf(window),
        GuiOffset(Vector2::new(16.0, 50.0)),
        ZIndex(2.0),
    ));
}

fn on_retreat_clicked(_entity: Entity, ctx: &mut GameCtx) {
    ctx.signals.set_string("switch_scene", "menu");
}

// ── Engine setup ─────────────────────────────────────────────────────────────

fn main() {
    EngineBuilder::new()
        .add_system(setup_gui_theme)
        .add_scene("game", SceneDescriptor {
            on_enter: scene_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
            world_draw_callback: None,
        })
        .run();
}
```

---

## Rendering Details

- All GUI widgets are rendered in screen space, unaffected by `Camera2D`.
- `ZIndex` is required for all widgets. Lower values render first (behind); higher values render
  in front. Children should have a higher `ZIndex` than their parent window.
- Within the same `ZIndex`, draw order is: **Panel < Sprite < Text**. Nine-patch backgrounds
  always render behind sprites, which render behind captions.
- `GuiWindow.size` is the nine-patch draw size. `GuiInteractable.size` is the click/hover
  hit-test AABB. Both are set from the widget's constructor.

---

## Feature Gating

The GUI systems and components are **always compiled** regardless of the `lua` feature flag:
- `gui_button_spawn_system`, `gui_label_spawn_system`, `gui_image_spawn_system`
- `gui_layout_system`, `gui_hit_test_system`, `gui_image_state_sync_system`
- `gui_interactable_click_observer`
- All component types (`GuiWindow`, `GuiButton`, `GuiLabel`, `GuiImage`, `GuiInteractable`,
  `GuiOffset`, `GuiWidgetState`)
- `GuiThemeStore`, `GuiTheme`, `GuiInputState`

The following are gated on `feature = "lua"`:
- Lua builder methods (`:with_gui_button`, `:with_gui_image`, `:with_gui_theme_key`, etc.)
- `engine.set_gui_theme_*` / `engine.entity_set_gui_disabled` Lua API calls
- The `gui_theme_commands` queue in `LuaAppData`
- `GuiButton::with_lua_callback`, `GuiImage::with_lua_callback` constructors

Rust-only games (built with `default-features = false`) get the full component and system stack
and configure everything via `ResMut<GuiThemeStore>` and direct component spawning.

---

## Roadmap / Pending

Genuinely open items (not yet implemented):

- **Text input widget** — single-line editable field
- **Keyboard/gamepad navigation** — focus traversal between interactable widgets
- **Drag-and-drop** — item dragging between `GuiImage` slots
- **Tooltips** — hover-triggered popup labels
- **Screen-space post-process shaders** — per-widget shader effects
- **Theme hot-reload** — update `GuiThemeStore` at runtime without restart

**Known architectural notes** (acknowledged, not blocking):

- Widget size is expressed in three places: the widget component (e.g. `GuiButton.size`),
  `GuiInteractable.size` (hit-test), and `DynamicText` child size (caption layout). They are
  seeded from the same value at spawn time but diverge if mutated independently post-spawn.
- Two-pass hit-test: `gui_hit_test_system` iterates all `GuiInteractable` components to find
  the highest-ZIndex hit, which is O(n) per frame over all interactable widgets. Acceptable
  for typical in-game UI counts.
