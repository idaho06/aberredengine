//! One-shot entity setup component backed by a Lua callback.
//!
//! A [`LuaSetup`] component stores the name of a Lua function. The
//! [`lua_setup_entity_system`](crate::systems::lua_setup_entity::lua_setup_entity_system)
//! reacts to every entity that gains this component (via [`bevy_ecs::prelude::Added`])
//! and calls the named function exactly once, passing the entity context.
//!
//! Because the trigger is `Added`, the callback fires again whenever a new
//! entity is spawned or cloned with the component — it does **not** fire a
//! second time on the original entity.
//!
//! # Lua callback signature
//!
//! ```lua
//! function my_entity_setup(ctx)
//!     -- ctx is the standard entity context (id, pos, group, signals, ...)
//!     -- Use engine.entity_* and engine.spawn() to configure the entity.
//!     engine.entity_add_lua_phase(ctx.id, ...)
//! end
//! ```
//!
//! # Gotchas
//!
//! - The callback fires the **frame after** the entity is spawned, so all
//!   components added in the same spawn call are visible in `ctx`.
//! - Any child entities or components added inside the callback won't appear
//!   until the **following** frame.
//! - Do **not** store references to `ctx` or its sub-tables.
//! - If the named function is missing, a `warn!` is emitted and the entity is
//!   skipped — the engine does not panic.
//!
//! # Usage from Lua
//!
//! ```lua
//! -- Via spawn builder
//! engine.spawn()
//!     :with_position(100, 200)
//!     :with_lua_setup("my_entity_setup")
//!     :build()
//! ```
//!
//! # Usage from map files
//!
//! ```json
//! { "position": [100, 200], "group": "enemy", "lua_setup": "my_entity_setup" }
//! ```

use bevy_ecs::prelude::Component;

/// Attaches a one-shot Lua setup callback to an entity.
///
/// The named function is called once when the component is first detected via
/// `Added<LuaSetup>`. Survives scene transitions only when combined with
/// [`crate::components::persistent::Persistent`].
#[derive(Component, Clone, Debug)]
pub struct LuaSetup {
    /// Name of the Lua function to call.
    pub callback: String,
}

impl LuaSetup {
    pub fn new(callback: impl Into<String>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}
