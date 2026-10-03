//! Scene entity component.
//!
//! Every scene registered with `aberredengine::EngineBuilder::add_scene` has one
//! [`Persistent`](crate::components::persistent::Persistent) scene entity carrying
//! [`SceneName`]. [`SceneEntered`](crate::events::scene::SceneEntered) and
//! [`SceneExited`](crate::events::scene::SceneExited) target it, so observers attached to a
//! scene entity fire only for that scene.

use std::sync::Arc;

use bevy_ecs::prelude::Component;

/// The name of the scene this entity stands for.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct SceneName(pub Arc<str>);
