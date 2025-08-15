use core::str;

use bevy_ecs::prelude::Component;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Group(pub &'static str);
