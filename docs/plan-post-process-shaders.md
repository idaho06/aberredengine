# Plan: Post-process shaders (render-target blit)

Goal: Add **post-process shader support** that runs during the final blit from the fixed-resolution render target to the actual window. Shaders are **loaded from Lua during setup**, selected/cleared from Lua at runtime, and receive a small set of **standardized uniform values**. Per-entity shaders are explicitly out of scope.

This plan is written for an implementation agent to follow; it intentionally does not modify code.

---

## 0) Background: what is a “uniform”?

A **uniform** is a read-only shader variable set by the CPU **before** drawing. It is constant for the entire draw call (i.e., all pixels/vertices drawn in that call see the same uniform values).

Typical uniforms for post-processing:

- Time (`uTime`) to animate effects (scanlines, noise)
- Delta time (`uDeltaTime`) to advance animations predictably
- Resolution (`uResolution`) so the shader can scale effects with pixel size

In contrast:

- **Attributes / varyings** (e.g., UVs) change per-vertex and per-fragment.
- A **texture sampler** (often named `texture0` in raylib) lets the shader sample the rendered scene.

---

## 1) Choose shader source format & file extensions

### Recommended for raylib desktop targets

Raylib expects GLSL source. Common file extensions:

- Vertex shader: `.vs`
- Fragment shader: `.fs`

Standardize in the project:

- `assets/shaders/*.vs`
- `assets/shaders/*.fs`

### GLSL version notes

Raylib supports both desktop OpenGL and GLES in general. This project targets Linux and Windows with raylib 5.5.1.

Plan:

- Use desktop GLSL `#version 330` and ship shaders that work on both Linux/Windows desktop.

---

## 2) Core design

### 2.1 New NonSend resource: ShaderStore

Create a store similar to `FontStore`, because `raylib::prelude::Shader` is a GPU resource and should be treated as main-thread-only.

**Data shape** (suggested):

- `ShaderStore { map: FxHashMap<String, ShaderEntry> }`
- `ShaderEntry { shader: raylib::prelude::Shader, locations: FxHashMap<String, i32> }`

Why cache locations?

- Calling `GetShaderLocation` by name can be expensive if done per-frame.
- Cache lazily (first time asked) or eagerly for a standard set.

### 2.2 New Send resource: PostProcessShader

A simple resource read by the render system.

Suggested shape:

- `PostProcessShader { key: Option<Arc<str>>, uniforms: FxHashMap<Arc<str>, UniformValue> }`

UniformValue should cover the minimum types you want to set from Lua:

- `Float(f32)`
- `Int(i32)`
- `Vec2 { x: f32, y: f32 }`
- `Vec4 { x: f32, y: f32, z: f32, w: f32 }`

(You can expand later if needed.)

Reason to keep PostProcessShader as a separate resource:

- Render system can be purely reactive: if active shader key exists, bind it and apply uniforms.
- Lua can set/clear the active shader without modifying render code paths elsewhere.

---

## 3) Standardized uniforms (automatic per frame)

These uniforms should be set by Rust every frame right before the post-process draw call:

### Required

- `uTime: float` = `WorldTime.elapsed`
- `uDeltaTime: float` = `WorldTime.delta`
- `uResolution: vec2` = render target size in pixels (internal resolution), e.g. `(ScreenSize.w, ScreenSize.h)`
- `uFrame: int` = a monotonically increasing frame counter stored in a resource
- `uWindowResolution: vec2` = actual window size `(WindowSize.w, WindowSize.h)`
- `uLetterbox: vec4` = destination rect of the blit `(x, y, w, h)` so shaders can align to bars

Note: Standard uniforms should not require Lua calls.

---

## 4) Lua-driven uniforms (user parameters)

Since you want parameters set from Lua, provide a tiny API to set uniforms for the *currently active* post-process shader.

### Proposed Lua API

#### Loading

- `engine.load_shader(id, vs_path_or_nil, fs_path_or_nil)`
  - At least one of vs/fs must be provided.
  - Common case: fragment-only post-process shader: `engine.load_shader("crt", nil, "./assets/shaders/crt.fs")`

#### Selecting / clearing

- `engine.post_process_shader(id_or_nil)`
  - If nil: disables post-processing.

#### Setting uniforms

- `engine.post_process_set_float(name, value)`
- `engine.post_process_set_int(name, value)`
- `engine.post_process_set_vec2(name, x, y)`
- `engine.post_process_set_vec4(name, x, y, z, w)`
- `engine.post_process_clear_uniform(name)`
- `engine.post_process_clear_uniforms()`

### Where uniforms live

- Store user uniforms inside `PostProcessShader.uniforms` (map of name -> UniformValue).
- These are applied each frame by the render system.

### Resolution of uniform locations

- When applying uniforms, look up/calculate the location via `shader.get_shader_location(name)` and cache in `ShaderEntry.locations`.

---

## 5) Command flow integration (Lua → Rust)

This engine uses a Lua command queue processed on the Rust side. Follow existing patterns for asset loading commands.

### 5.1 Add new commands

Add commands in the Lua-runtime command enums (where `AssetCmd` lives):

- `AssetCmd::LoadShader { id: String, vs_path: Option<String>, fs_path: Option<String> }`

Add a new command enum or extend an existing one for post-process control:

- `RenderCmd::SetPostProcessShader { id: Option<String> }`
- `RenderCmd::SetPostProcessUniform { name: String, value: UniformValue }`
- `RenderCmd::ClearPostProcessUniform { name: String }`
- `RenderCmd::ClearPostProcessUniforms`

(You can name it differently, but keep it clearly render-related.)

### 5.2 Register Lua functions

In the Lua engine table registration:

- Implement `engine.load_shader(...)` pushing `AssetCmd::LoadShader`.
- Implement `engine.post_process_shader(...)` pushing `RenderCmd::SetPostProcessShader`.
- Implement uniform setters pushing `RenderCmd::*` uniform commands.

Lua argument behavior:

- Accept `nil` for optional paths and for disabling post-processing.
- Raise a clear runtime error if both vs and fs are nil.

### 5.3 Process commands on the Rust side

Where asset commands are drained:

- Handle `LoadShader` using `RaylibHandle::load_shader(&RaylibThread, vs_opt, fs_opt)`.
- Insert result into `ShaderStore`.
- Optionally validate `shader.is_shader_valid()` and log warnings on failure.

Where render commands are drained:

- Update the `PostProcessShader` resource accordingly.

---

## 6) Render system integration (post-process only)

The render system currently does:

- Draw everything into `RenderTexture2D` (texture mode).
- Begin drawing to window, then draw the render target texture to the window with letterboxing.

Modify **only phase 2**:

- Look up the active post-process shader key from `PostProcessShader`.
- If no shader is active, draw normally.
- If found and present in `ShaderStore`, set standard uniforms, set user uniforms, then wrap the final `draw_texture_pro(render_target.texture, ...)` in `begin_shader_mode(&mut shader)`.

Implementation details:

- Access `ShaderStore` as `NonSend`/`NonSendMut` in the render system params.
- Since `begin_shader_mode` takes `&mut Shader`, you must retrieve a mutable shader reference from the store.
- The shader mode scope must include the `draw_texture_pro` call.

Uniform application ordering:

- Apply standard uniforms first, then user overrides (so Lua can override standardized names if desired). Alternatively, forbid overriding standardized names.

---

## 7) Resource initialization & lifetime

### Insert resources

- Insert `PostProcessShader` resource early in `main.rs` (default: disabled).
- Insert `ShaderStore` as a NonSend resource (empty).

### Unload behavior

- Shaders are RAII-managed by raylib-rs and will unload when dropped.
- Ensure `ShaderStore` lives for the lifetime of the app.

---

## 8) Minimal validation / test strategy

No formal unit tests needed initially; this is mostly GPU integration.

### Add a trivial “invert colors” fragment shader (later)

Create a simple fragment shader file under `assets/shaders/` that:

- samples `texture0`
- outputs `vec4(1.0 - color.rgb, color.a)`

Then in Lua setup:

- load shader
- enable post process shader

### Runtime sanity checks

- If shader key is set but not found in `ShaderStore`, log a warning and render normally.
- If shader fails `is_shader_valid()`, log error and do not store/activate it.

---

## 9) Documentation updates (recommended)

Update Lua docs to include:

- `engine.load_shader`
- `engine.post_process_shader`
- uniform setters
- standardized uniform names and types

Where:

- `assets/scripts/README.md` (Lua API doc)
- `assets/scripts/engine.lua` (autocomplete stubs)

---

## 10) Decisions

- Standarized uniforms (e.g. `uTime`) should be reserved. No override from Lua.
- Fragment-only shaders are the default supported use case

---

## Appendix: Typical raylib post-process fragment shader structure

Raylib commonly uses:

- `texture0` sampler
- `fragTexCoord` varying

Exact names depend on raylib’s default vertex shader and the GLSL version; when writing your first shader, copy the structure from raylib examples matching your target GLSL version.

Practical recommendation:

- Start from a known-good raylib example shader (desktop GLSL 330), then adapt.
- Keep the first shader extremely simple (invert/greyscale) to validate the pipeline.
