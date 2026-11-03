//! Dialogue node/choice/end events.
//!
//! [`dialogue_system`](crate::systems::dialogue::dialogue_system) triggers
//! [`DialogueNodeEntered`] when a runner enters a node, [`DialogueChoiceSelected`]
//! when the player picks a choice (before the target node's
//! [`DialogueNodeEntered`]), and [`DialogueEnded`] when the conversation ends.
//! All three are [`EntityEvent`]s targeted at the runner's entity, so they
//! reach per-entity observers (`commands.spawn(..).observe(handler)`) as well
//! as global ones (`add_observer(handler)`).
//!
//! Observers run after `dialogue_system`, so they read the new state from the
//! [`DialogueRunner`](crate::components::dialogue::DialogueRunner) — for
//! example
//! [`node_id`](crate::components::dialogue::DialogueRunner::node_id) or
//! [`end_reason`](crate::components::dialogue::DialogueRunner::end_reason) —
//! rather than recomputing it from the event.

use std::sync::Arc;

use bevy_ecs::prelude::*;

/// Why a conversation ended, carried by [`DialogueEnded::reason`]; re-exported
/// from [`DialogueEndReason`] so observers can name it without importing the
/// components module.
pub use crate::components::dialogue::DialogueEndReason;

/// Triggered when a runner enters a dialogue node.
#[derive(EntityEvent, Clone, Debug)]
pub struct DialogueNodeEntered {
    /// The runner entity that entered the node.
    #[event_target]
    pub entity: Entity,
    /// Id of the entered node.
    pub node: Arc<str>,
    /// Node left by this transition; `None` for the start node.
    pub previous: Option<Arc<str>>,
    /// Labels of the entered node.
    pub labels: Arc<[Arc<str>]>,
}

/// Triggered when the player picks a choice, before the target node's
/// [`DialogueNodeEntered`].
#[derive(EntityEvent, Clone, Debug)]
pub struct DialogueChoiceSelected {
    /// The runner entity that picked the choice.
    #[event_target]
    pub entity: Entity,
    /// Node the choice was picked on.
    pub node: Arc<str>,
    /// DECLARED index of the picked choice in the node's choice list.
    pub index: usize,
    /// Text of the picked choice.
    pub text: Arc<str>,
    /// Labels of the picked choice.
    pub labels: Arc<[Arc<str>]>,
}

/// Triggered when a runner's conversation ends.
#[derive(EntityEvent, Clone, Debug)]
pub struct DialogueEnded {
    /// The runner entity whose conversation ended.
    #[event_target]
    pub entity: Entity,
    /// Last node shown; `None` when the conversation was cancelled before start.
    pub node: Option<Arc<str>>,
    /// Why the conversation ended.
    pub reason: DialogueEndReason,
}
