//! Lua callback components for an entity's core dialogue events.
//!
//! The entity holds up to one callback per kind, each naming a Lua function
//! `fn(ctx, dialogue)` called on the entity's core dialogue event. `ctx` is
//! the entity context; `dialogue` is a fresh table built per event:
//!
//! - [`LuaOnDialogueNodeEntered`]: for each
//!   [`DialogueNodeEntered`](aberred_core::events::dialogue::DialogueNodeEntered).
//! - [`LuaOnDialogueChoiceSelected`]: for each
//!   [`DialogueChoiceSelected`](aberred_core::events::dialogue::DialogueChoiceSelected).
//! - [`LuaOnDialogueEnded`]: for each
//!   [`DialogueEnded`](aberred_core::events::dialogue::DialogueEnded).

use std::sync::Arc;

use bevy_ecs::prelude::Component;

/// Names the Lua function to call when the entity's dialogue enters a node.
#[derive(Component, Clone, Debug)]
pub struct LuaOnDialogueNodeEntered {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnDialogueNodeEntered {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}

/// Names the Lua function to call when the player picks a choice in the entity's dialogue.
#[derive(Component, Clone, Debug)]
pub struct LuaOnDialogueChoiceSelected {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnDialogueChoiceSelected {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}

/// Names the Lua function to call when the entity's dialogue ends.
#[derive(Component, Clone, Debug)]
pub struct LuaOnDialogueEnded {
    /// Name of the Lua function to call.
    pub callback: Arc<str>,
}

impl LuaOnDialogueEnded {
    pub fn new(callback: impl Into<Arc<str>>) -> Self {
        Self {
            callback: callback.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_the_callback_name() {
        assert_eq!(
            &*LuaOnDialogueNodeEntered::new("on_node").callback,
            "on_node"
        );
        assert_eq!(
            &*LuaOnDialogueChoiceSelected::new("on_choice").callback,
            "on_choice"
        );
        assert_eq!(&*LuaOnDialogueEnded::new("on_end").callback, "on_end");
    }
}
