//! Entity builder for fluent entity construction from Lua.
//!
//! This module provides the `LuaEntityBuilder` struct which implements
//! a fluent interface for building entities from Lua scripts using method chaining.
//!
//! The builder supports both spawning new entities and cloning existing ones,
//! in both regular and collision contexts.

use super::commands::{CloneCmd, UniformValue};
use super::runtime::LuaAppData;
use super::spawn_data::*;
use super::stub_meta::BuilderMethodDef;
use aberred_core::components::Themed;
use aberred_core::components::guibutton::GuiButton;
use aberred_core::components::guiimage::GuiImage;
use aberred_core::components::guilabel::GuiLabel;
use aberred_core::components::guiprogressbar::{GuiProgressBar, ProgressBarDirection};
use aberred_core::components::guiwindow::GuiWindow;
use mlua::MaybeSend;
use mlua::prelude::*;
use aberred_core::math::Vec2;

mod behavior;
mod gui;
mod menu;
mod physics;
mod sprite;
mod transform;
mod tween;

/// Parse a Lua value into a UniformValue.
///
/// Numbers are treated as Float, tables of length 2 as Vec2, and tables of length 4 as Vec4.
fn parse_uniform_value(val: LuaValue) -> LuaResult<UniformValue> {
    match val {
        LuaValue::Number(n) => Ok(UniformValue::Float(n as f32)),
        LuaValue::Integer(n) => Ok(UniformValue::Float(n as f32)),
        LuaValue::Table(t) => {
            let len = t.raw_len();
            match len {
                2 => {
                    let x: f32 = t.get(1)?;
                    let y: f32 = t.get(2)?;
                    Ok(UniformValue::Vec2 { x, y })
                }
                4 => {
                    let x: f32 = t.get(1)?;
                    let y: f32 = t.get(2)?;
                    let z: f32 = t.get(3)?;
                    let w: f32 = t.get(4)?;
                    Ok(UniformValue::Vec4 { x, y, z, w })
                }
                _ => Err(LuaError::runtime(
                    "Uniform table must be array of length 2 (vec2) or 4 (vec4)",
                )),
            }
        }
        _ => Err(LuaError::runtime(
            "Uniform value must be number or array table",
        )),
    }
}

/// Builder mode: spawn a new entity or clone an existing one.
#[derive(Debug, Clone, Copy, Default)]
pub enum BuilderMode {
    /// Spawn a new entity from scratch
    #[default]
    Spawn,
    /// Clone an existing entity (looked up by WorldSignals key)
    Clone,
}

/// Builder context: regular or collision callback.
#[derive(Debug, Clone, Copy, Default)]
pub enum BuilderContext {
    /// Regular context (scene setup, phase callbacks, timer callbacks)
    #[default]
    Regular,
    /// Collision callback context (processed immediately after collision)
    Collision,
}

/// Entity builder exposed to Lua for fluent entity construction.
///
/// This struct implements `UserData` so Lua can call methods on it using
/// the colon syntax: `engine.spawn():with_position(x, y):build()`
///
/// Each `with_*` method returns `Self` to allow chaining.
/// The `build()` method queues the entity for spawning or cloning.
#[derive(Debug, Clone, Default)]
pub struct LuaEntityBuilder {
    mode: BuilderMode,
    context: BuilderContext,
    /// Only used in Clone mode - WorldSignals key for source entity
    source_key: Option<String>,
    cmd: SpawnCmd,
}

impl LuaEntityBuilder {
    /// Create a new spawn builder (regular context).
    pub fn new() -> Self {
        Self {
            mode: BuilderMode::Spawn,
            context: BuilderContext::Regular,
            source_key: None,
            cmd: SpawnCmd::default(),
        }
    }

    /// Create a new spawn builder (collision context).
    pub fn new_collision() -> Self {
        Self {
            mode: BuilderMode::Spawn,
            context: BuilderContext::Collision,
            source_key: None,
            cmd: SpawnCmd::default(),
        }
    }

    /// Create a new clone builder (regular context).
    pub fn new_clone(source_key: String) -> Self {
        Self {
            mode: BuilderMode::Clone,
            context: BuilderContext::Regular,
            source_key: Some(source_key),
            cmd: SpawnCmd::default(),
        }
    }

    /// Create a new clone builder (collision context).
    pub fn new_collision_clone(source_key: String) -> Self {
        Self {
            mode: BuilderMode::Clone,
            context: BuilderContext::Collision,
            source_key: Some(source_key),
            cmd: SpawnCmd::default(),
        }
    }
}

/// Registers a `with_*` builder method and, when a metadata collector is present, records its
/// stub info. The PARAMS const inside the macro body is `'static` because const items always are.
macro_rules! builder_method {
    (
        $methods:expr, $meta:expr,
        $name:literal, $desc:literal,
        [$( ($pname:literal, $ptype:literal) ),* $(,)?],
        $closure:expr
    ) => {
        $methods.add_function($name, |lua, (ud, args): (LuaAnyUserData, _)| {
            let mut this = ud.borrow_mut::<LuaEntityBuilder>()?;
            let f: fn(&Lua, &mut LuaEntityBuilder, _) -> LuaResult<()> = $closure;
            f(lua, &mut *this, args)?;
            Ok(ud)
        });
        if let Some(collector) = $meta.as_mut() {
            const PARAMS: &[(&str, &str)] = &[$( ($pname, $ptype) ),*];
            collector.push(($name, $desc, PARAMS));
        }
    };
}
pub(crate) use builder_method;

/// No-op `UserDataMethods` impl used only by `collect_builder_meta` to harvest stub metadata
/// without performing real method registration.
struct DummyMethods;

impl LuaUserDataMethods<LuaEntityBuilder> for DummyMethods {
    fn add_method<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: Fn(&Lua, &LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_method_mut<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: FnMut(&Lua, &mut LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_function<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: Fn(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_function_mut<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: FnMut(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_meta_method<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: Fn(&Lua, &LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_meta_method_mut<M, A, R>(&mut self, _name: impl Into<String>, _method: M)
    where
        M: FnMut(&Lua, &mut LuaEntityBuilder, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_meta_function<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: Fn(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }

    fn add_meta_function_mut<F, A, R>(&mut self, _name: impl Into<String>, _function: F)
    where
        F: FnMut(&Lua, A) -> LuaResult<R> + MaybeSend + 'static,
        A: FromLuaMulti,
        R: IntoLuaMulti,
    {
    }
}

/// Collect stub metadata for all builder methods.
/// Used by the stub generator path only; has no effect during normal gameplay.
pub fn collect_builder_meta() -> Vec<BuilderMethodDef> {
    let mut meta = Some(Vec::new());
    let mut dummy = DummyMethods;
    register_methods(&mut dummy, &mut meta);
    let mut v = meta.unwrap();
    // register_as and build are not with_* methods so the macro doesn't capture them;
    // append their entries manually so the stub generator includes them.
    const REGISTER_AS_PARAMS: &[(&str, &str)] = &[("key", "string")];
    v.push((
        "register_as",
        "Register entity in WorldSignals for later retrieval",
        REGISTER_AS_PARAMS,
    ));
    v.push(("build", "Queue entity for spawning or cloning", &[]));
    v
}

fn register_methods<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    transform::register(methods, meta);
    physics::register(methods, meta);
    sprite::register(methods, meta);
    gui::register(methods, meta);
    menu::register(methods, meta);
    tween::register(methods, meta);
    behavior::register(methods, meta);

    // Non-builder-stub methods — kept as plain registrations.
    // These have their own entries in stub_meta's non-BUILDER_METHODS sections.

    methods.add_function("register_as", |_, (ud, key): (LuaAnyUserData, String)| {
        let mut this = ud.borrow_mut::<LuaEntityBuilder>()?;
        this.cmd.register_as = Some(key);
        Ok(ud)
    });

    methods.add_method_mut("build", |lua, this, ()| {
        let app_data = lua
            .app_data_ref::<LuaAppData>()
            .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;

        // Take the built command out of the builder rather than cloning it — the
        // builder is consumed by build() in normal (single-call) usage.
        match (this.mode, this.context) {
            (BuilderMode::Spawn, BuilderContext::Regular) => {
                app_data
                    .spawn_commands
                    .borrow_mut()
                    .push(Box::new(std::mem::take(&mut this.cmd)));
            }
            (BuilderMode::Spawn, BuilderContext::Collision) => {
                app_data
                    .collision_spawn_commands
                    .borrow_mut()
                    .push(Box::new(std::mem::take(&mut this.cmd)));
            }
            (BuilderMode::Clone, BuilderContext::Regular) => {
                let source_key = this.source_key.take().unwrap_or_default();
                app_data.clone_commands.borrow_mut().push(CloneCmd {
                    source_key,
                    overrides: Box::new(std::mem::take(&mut this.cmd)),
                });
            }
            (BuilderMode::Clone, BuilderContext::Collision) => {
                let source_key = this.source_key.take().unwrap_or_default();
                app_data
                    .collision_clone_commands
                    .borrow_mut()
                    .push(CloneCmd {
                        source_key,
                        overrides: Box::new(std::mem::take(&mut this.cmd)),
                    });
            }
        }
        Ok(())
    });
}

impl LuaUserData for LuaEntityBuilder {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        register_methods(methods, &mut None);
    }
}

#[cfg(test)]
mod tests {
    use crate::resources::lua_runtime::LuaRuntime;

    fn assert_runtime_error(script: &str, expected_msg: &str) {
        let runtime = LuaRuntime::new().unwrap();
        let err = runtime
            .lua()
            .load(script)
            .exec()
            .expect_err("expected script to raise an error");
        let message = err.to_string();
        assert!(
            message.contains(expected_msg),
            "expected error containing {expected_msg:?}, got {message:?}"
        );
    }

    #[test]
    fn with_sprite_offset_requires_with_sprite() {
        assert_runtime_error(
            "engine.spawn():with_sprite_offset(1, 1)",
            "with_sprite_offset() requires with_sprite() first",
        );
    }

    #[test]
    fn with_collider_offset_requires_with_collider() {
        assert_runtime_error(
            "engine.spawn():with_collider_offset(1, 1)",
            "with_collider_offset() requires with_collider() first",
        );
    }

    #[test]
    fn with_stuckto_offset_requires_with_stuckto() {
        assert_runtime_error(
            "engine.spawn():with_stuckto_offset(1, 1)",
            "with_stuckto_offset() requires with_stuckto() first",
        );
    }

    #[test]
    fn with_signal_binding_format_requires_with_signal_binding() {
        assert_runtime_error(
            "engine.spawn():with_signal_binding_format('{}')",
            "with_signal_binding_format() requires with_signal_binding() first",
        );
    }

    #[test]
    fn with_tween_position_easing_requires_with_tween_position() {
        assert_runtime_error(
            "engine.spawn():with_tween_position_easing('linear')",
            "with_tween_position_easing() requires with_tween_position() first",
        );
    }

    #[test]
    fn with_tween_rotation_loop_requires_with_tween_rotation() {
        assert_runtime_error(
            "engine.spawn():with_tween_rotation_loop('loop')",
            "with_tween_rotation_loop() requires with_tween_rotation() first",
        );
    }

    #[test]
    fn with_tween_scale_backwards_requires_with_tween_scale() {
        assert_runtime_error(
            "engine.spawn():with_tween_scale_backwards()",
            "with_tween_scale_backwards() requires with_tween_scale() first",
        );
    }

    /// `with_*` chaining must return the *same* userdata handle (in-place mutation),
    /// not a clone, otherwise the O(n) chain cost regresses back to O(n^2).
    #[test]
    fn chaining_returns_same_userdata() {
        let runtime = LuaRuntime::new().unwrap();
        let same: bool = runtime
            .lua()
            .load(
                "local b = engine.spawn() \
                 return rawequal(b, b:with_position(1, 2):with_velocity(0, 0))",
            )
            .eval()
            .unwrap();
        assert!(same, "chained with_* calls must return the same userdata");
    }

    #[test]
    fn long_chain_builds_expected_spawn_cmd() {
        use super::super::runtime::LuaAppData;

        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load(
                "engine.spawn() \
                    :with_group('asteroids') \
                    :with_position(10, 20) \
                    :with_sprite('rock', 64, 64, 32, 32) \
                    :with_rotation(45) \
                    :with_velocity(1, 2) \
                    :with_zindex(5) \
                    :with_collider(40, 40, 20, 20) \
                    :with_signal_integer('hp', 3) \
                    :build()",
            )
            .exec()
            .unwrap();

        let app_data = runtime.lua().app_data_ref::<LuaAppData>().unwrap();
        let queued = app_data.spawn_commands.borrow();
        assert_eq!(queued.len(), 1, "expected exactly one queued spawn command");
        let cmd = &queued[0];
        assert_eq!(cmd.group.as_deref(), Some("asteroids"));
        assert_eq!(cmd.position, Some((10.0, 20.0)));
        assert!(cmd.sprite.is_some());
        assert_eq!(cmd.rotation, Some(45.0));
        assert_eq!(cmd.zindex, Some(5.0));
        assert!(cmd.collider.is_some());
        assert_eq!(cmd.signal_integers, vec![("hp".to_string(), 3)]);
    }
}
