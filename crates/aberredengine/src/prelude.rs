//! One glob import for downstream games: `use aberredengine::prelude::*;`.
//!
//! Brings in the `bevy_ecs` prelude (and the `bevy_ecs` crate itself, so
//! `#[derive(Component)]`, `#[derive(Resource)]` and `#[derive(Event)]` resolve), the
//! engine's math types, the common components, resources, events and commands, and the
//! builder types. Less common items keep their full `aberredengine::core::...` path.
//!
//! The `bevy_ecs` prelude exports its own `Result` (`Result<T = (), E = BevyError>`) and
//! a lifecycle event named `Add`, so they shadow `std::result::Result` and
//! `std::ops::Add` in a module that glob-imports this prelude. `Result<T, E>` with an
//! explicit error type still means the standard `Result`.

pub use crate::{EngineError, bevy_ecs, imgui};
pub use bevy_ecs::prelude::*;

pub use aberred_core::math::{Color, Rect, Vec2};

pub use aberred_core::components::animation::{Animation, AnimationController};
pub use aberred_core::components::boxcollider::BoxCollider;
pub use aberred_core::components::cameratarget::CameraTarget;
pub use aberred_core::components::collision::{BoxSide, BoxSides, CollisionRule};
pub use aberred_core::components::dynamictext::DynamicText;
pub use aberred_core::components::entityshader::EntityShader;
pub use aberred_core::components::group::Group;
pub use aberred_core::components::guibutton::GuiButton;
pub use aberred_core::components::guiinteractable::GuiInteractable;
pub use aberred_core::components::guioffset::GuiOffset;
pub use aberred_core::components::guiwindow::GuiWindow;
pub use aberred_core::components::inputcontrolled::{AccelerationControlled, InputControlled};
pub use aberred_core::components::mapposition::MapPosition;
pub use aberred_core::components::menu::{Menu, MenuAction, MenuActions};
pub use aberred_core::components::particleemitter::{EmitterShape, ParticleEmitter, TtlSpec};
pub use aberred_core::components::persistent::Persistent;
pub use aberred_core::components::phase::{Phase, PhaseCallbackFns};
pub use aberred_core::components::rigidbody::RigidBody;
pub use aberred_core::components::rotation::Rotation;
pub use aberred_core::components::scale::Scale;
pub use aberred_core::components::scene::SceneName;
pub use aberred_core::components::screenposition::ScreenPosition;
pub use aberred_core::components::signalbinding::SignalBinding;
pub use aberred_core::components::signals::Signals;
pub use aberred_core::components::sprite::Sprite;
pub use aberred_core::components::stuckto::StuckTo;
pub use aberred_core::components::tilemap::TileMap;
pub use aberred_core::components::timer::Timer;
pub use aberred_core::components::tween::{Easing, LoopMode, Tween};
pub use aberred_core::components::zindex::ZIndex;

pub use aberred_core::events::animation::AnimationFinishedEvent;
pub use aberred_core::events::asset::{AssetLoadFailed, AssetLoaded};
pub use aberred_core::events::input::InputAction;
pub use aberred_core::events::scene::{SceneEntered, SceneExited};
pub use aberred_core::events::timer::TimerFired;
pub use aberred_core::events::tween::TweenFinishedEvent;

pub use aberred_core::protocol::asset_kind::AssetKind;
pub use aberred_core::protocol::audio::AudioCmd;
pub use aberred_core::protocol::render_assets::RenderAssetCmd;

pub use aberred_core::resources::animationstore::{AnimationResource, AnimationStore};
pub use aberred_core::resources::appstate::AppState;
pub use aberred_core::resources::camera2d::{Camera2D, Camera2DRes};
pub use aberred_core::resources::camerafollowconfig::{CameraFollowConfig, FollowMode};
pub use aberred_core::resources::gameconfig::GameConfig;
pub use aberred_core::resources::guitheme::{GuiButtonSkin, GuiNinePatch, GuiThemeStore};
pub use aberred_core::resources::input::InputState;
pub use aberred_core::resources::pending_assets::PendingAssets;
pub use aberred_core::resources::postprocessshader::PostProcessShader;
pub use aberred_core::resources::screensize::ScreenSize;
pub use aberred_core::resources::signal_intents::SignalIntents;
pub use aberred_core::resources::signal_keys as sk;
pub use aberred_core::resources::sim_rng::SimRng;
pub use aberred_core::resources::texturefilter::TextureFilter;
pub use aberred_core::resources::uniformvalue::UniformValue;
pub use aberred_core::resources::worldsignals::{SignalSnapshot, SignalsRead, WorldSignals};
pub use aberred_core::resources::worldtime::WorldTime;

pub use aberred_core::systems::GameCtx;
pub use aberred_core::systems::asset_loader::{AssetError, AssetLoader};
pub use aberred_core::systems::gamestate::state_is_playing;
pub use aberred_core::systems::scene_dispatch::{WorldDraw, WorldDrawCtx, in_scene};

pub use crate::render::*;

pub use crate::engine_app::{EngineBuilder, SimSet};
