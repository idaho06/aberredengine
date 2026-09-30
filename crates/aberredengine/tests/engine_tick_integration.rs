//! Engine tick integration tests for collision, and other systems.

#![allow(dead_code, unused_imports)]

use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::system::SystemState;
use aberredengine::core::math::Vec2;

use aberredengine::core::components::boxcollider::BoxCollider;
use aberredengine::core::components::collision::CollisionRule;
use aberredengine::core::components::group::Group;
#[cfg(feature = "lua")]
use aberredengine::lua::components::luacollision::{LuaCollisionCallback, LuaCollisionRule};
#[cfg(feature = "lua")]
use aberredengine::lua::components::luaphase::{LuaPhase, PhaseCallbacks};
#[cfg(feature = "lua")]
use aberredengine::lua::components::luatimer::{LuaTimer, LuaTimerCallback};
use aberredengine::core::components::mapposition::MapPosition;
use aberredengine::core::components::rigidbody::RigidBody;
use aberredengine::core::components::signals::Signals;
use aberredengine::core::components::ttl::Ttl;
use aberredengine::core::events::collision::CollisionEvent;
#[cfg(feature = "lua")]
use aberredengine::core::protocol::audio::AudioCmd;
use aberredengine::core::resources::animationstore::AnimationStore;
use aberredengine::core::resources::appstate::AppState;
use aberredengine::core::resources::camerafollowconfig::CameraFollowConfig;
use aberredengine::core::resources::gameconfig::GameConfig;
use aberredengine::core::resources::input::InputState;
use aberredengine::core::resources::input_bindings::InputBindings;
#[cfg(feature = "lua")]
use aberredengine::lua::resources::lua_runtime::LuaRuntime;
use aberredengine::core::resources::postprocessshader::PostProcessShader;
use aberredengine::render::resources::texturestore::TextureStore;
use aberredengine::core::resources::screensize::ScreenSize;
use aberredengine::core::resources::systemsstore::SystemsStore;
use aberredengine::core::resources::texturedims::TextureDimsStore;
use aberredengine::core::resources::worldsignals::WorldSignals;
use aberredengine::core::resources::worldtime::WorldTime;
use aberredengine::core::resources::collision_rule_index::CollisionRuleIndex;
use aberredengine::core::systems::collision_detector::collision_detector;
use aberredengine::systems::collision_rule_index::rebuild_collision_rule_index;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::lua_collision::lua_collision_observer;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use aberredengine::lua::systems::luatimer::{lua_timer_observer, update_lua_timers};

use aberredengine::core::testing::insert_game_ctx_resources;

mod common;

fn make_world(delta: f32) -> World {
    let mut world = World::new();
    insert_game_ctx_resources(&mut world);
    world.insert_resource(WorldTime {
        elapsed: 0.0,
        delta,
        time_scale: 1.0,
        frame_count: 0,
    });
    world.insert_resource(ScreenSize { w: 800, h: 600 });
    world.insert_resource(AnimationStore {
        animations: Default::default(),
    });
    world.init_resource::<TextureStore>();
    world.init_resource::<TextureDimsStore>();
    world.insert_resource(CollisionRuleIndex::default());
    world
}

fn tick_collision_detector(world: &mut World) {
    let mut schedule = Schedule::default();
    schedule.add_systems(rebuild_collision_rule_index.before(collision_detector));
    schedule.add_systems(collision_detector);
    schedule.run(world);
}
