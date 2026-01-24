# Plan: Multi-pass post-process shaders

## Goal

Allow multiple post-process shaders to be applied in sequence (multi-pass). Update Lua API so `engine.post_process_shader()` accepts `nil` (disable) or a **table of one or more shader IDs** to apply in order. Keep existing uniforms system working, and ensure the render pipeline supports ping-pong passes.

## Constraints / Notes

- Raylib supports only one shader per draw call, so multiple shaders require multi-pass rendering with render targets.
- Existing engine uses a single `PostProcessShader` resource (`key` + `uniforms`). This must evolve to support a list/stack of shader keys.
- Standard uniforms (`uTime`, `uDeltaTime`, `uResolution`, `uFrame`, `uWindowResolution`, `uLetterbox`) remain reserved and should be applied in **every pass**.

## High-level approach

1. **Represent a shader chain** instead of a single shader key.
2. **Expose Lua API** that sets the chain via `engine.post_process_shader(nil|{"wave","invert",...})`.
3. **Render multi-pass** using ping-pong render targets and one shader per pass.

---

## Implementation Plan (for later)

### 1) Data model changes (resources)

- Update `PostProcessShader` in [src/resources/postprocessshader.rs](../src/resources/postprocessshader.rs):
  - Replace `key: Option<Arc<str>>` with something like `keys: Vec<Arc<str>>` (empty = disabled).
  - Keep `uniforms: FxHashMap<Arc<str>, UniformValue>` as-is (shared across all passes).
  - Add helper methods:
    - `set_shaders(Option<Vec<String>>)` or `set_shader_chain(Vec<&str>)`.
    - `clear_shaders()` or `set_shader_chain(Vec::new())`.

### 2) Lua command surface (runtime + commands)

- Update `RenderCmd::SetPostProcessShader` variant (in `resources/lua_runtime/commands.rs`) to accept:
  - `None` for disabling.
  - `Vec<String>` for a chain.
- Update Lua registration in [src/resources/lua_runtime/runtime.rs](../src/resources/lua_runtime/runtime.rs):
  - `engine.post_process_shader(nil)` → clears chain.
  - `engine.post_process_shader({"wave"})` → single shader.
  - `engine.post_process_shader({"wave","invert"})` → multi-pass.
  - Validate input: require a table with **at least one string** when not nil. Throw Lua error if invalid.
- Ensure `engine.post_process_set_*` still sets a **global uniform map** applied to each pass.

### 3) Command processing (Lua → resource)

- Update [src/systems/lua_commands.rs](../src/systems/lua_commands.rs) in `process_render_command`:
  - Replace `set_shader(id.as_deref())` with `set_shader_chain(...)`.
  - Log the chain in a single line, e.g. `Post-process shader chain set to: [wave, invert]`.

### 4) Render pipeline changes (multi-pass)

- Update [src/systems/render.rs](../src/systems/render.rs):
  - Introduce a **second render target** for ping-pong (e.g., `post_process_target_a`, `post_process_target_b`), or reuse an existing target if available.
  - For each shader key in `PostProcessShader.keys`:
    1) Bind destination render target.
    2) `BeginShaderMode(shader)`.
    3) Draw the source texture fullscreen (same quad as the current post-process blit).
    4) Set **standard uniforms** + **user uniforms** for that shader.
    5) `EndShaderMode()`.
    6) Swap source/destination targets (ping-pong).
  - After the last pass, draw the final texture to the window (no shader or with a final pass if needed).
  - Edge cases:
    - Zero shaders → draw render target directly to window as today.
    - One shader → identical behavior to current single-pass flow.

### 5) Shader store / uniforms

- Ensure each shader in the chain is looked up in `ShaderStore` and validated.
- For any missing shader key, log and skip that pass (or abort chain).
- Uniform application should be **per-pass**; keep reserved uniform protection as-is.

### 6) Lua stubs and docs

- Update [assets/scripts/engine.lua](../assets/scripts/engine.lua) to show new signature:
  - `engine.post_process_shader(nil | {"id", ...})`
- Update [assets/scripts/README.md](../assets/scripts/README.md) with new usage examples.

### 7) Tests / validation

- Add or update a lightweight integration test (if any) or manual test steps in `README.md`:
  - Activate `wave` + `invert` from menu scene.
  - Verify effects are stacked (wave distortion + inverted colors).
  - Confirm `engine.post_process_shader(nil)` disables all post-processing.

---

## Example Lua usage (post-implementation)

```lua
engine.post_process_shader({"wave", "invert"})
engine.post_process_set_float("amplitude", 0.003)
engine.post_process_set_float("lenght", 20.0)
engine.post_process_set_float("speed", 3.0)
```

## Non-goals

- Per-shader uniform namespaces (all uniforms remain shared across passes).
- Dynamic shader graph editing beyond setting the ordered list.
