//! One glob import for downstream games: `use aberredengine::prelude::*;`.
//!
//! Brings in the `bevy_ecs` prelude (and the `bevy_ecs` crate itself, so
//! `#[derive(Component)]`, `#[derive(Resource)]` and `#[derive(Event)]` resolve),
//! `SystemParam` for `#[derive(SystemParam)]` bundles (the `bevy_ecs` prelude leaves it
//! out), the engine's math types, the common components, resources, events and commands,
//! and the builder types. Less common items keep their full `aberredengine::core::...`
//! path.
//!
//! The `bevy_ecs` prelude exports its own `Result` (`Result<T = (), E = BevyError>`) and
//! a lifecycle event named `Add`, so they shadow `std::result::Result` and
//! `std::ops::Add` in a module that glob-imports this prelude. `Result<T, E>` with an
//! explicit error type still means the standard `Result`.

pub use crate::{EngineError, bevy_ecs, imgui};
pub use bevy_ecs::prelude::*;
pub use bevy_ecs::system::SystemParam;

pub use aberred_core::math::{Color, Rect, Vec2};

pub use aberred_core::components::animation::{Animation, AnimationController};
pub use aberred_core::components::box_collider::BoxCollider;
pub use aberred_core::components::camera_target::CameraTarget;
pub use aberred_core::components::collision::{BoxSide, BoxSides, CollisionRule};
pub use aberred_core::components::dynamic_text::DynamicText;
pub use aberred_core::components::entity_shader::EntityShader;
pub use aberred_core::components::group::Group;
pub use aberred_core::components::gui_button::GuiButton;
pub use aberred_core::components::gui_interactable::GuiInteractable;
pub use aberred_core::components::gui_offset::GuiOffset;
pub use aberred_core::components::gui_window::GuiWindow;
pub use aberred_core::components::input_controlled::{AccelerationControlled, InputControlled};
pub use aberred_core::components::menu::{Menu, MenuAction, MenuActions};
pub use aberred_core::components::particle_emitter::{EmitterShape, ParticleEmitter, TtlSpec};
pub use aberred_core::components::persistent::Persistent;
pub use aberred_core::components::phase::Phase;
pub use aberred_core::components::rigid_body::RigidBody;
pub use aberred_core::components::rotation::Rotation;
pub use aberred_core::components::scale::Scale;
pub use aberred_core::components::scene::SceneName;
pub use aberred_core::components::screen_position::ScreenPosition;
pub use aberred_core::components::shape::{Shape, ShapeKind};
pub use aberred_core::components::signal_binding::SignalBinding;
pub use aberred_core::components::signals::Signals;
pub use aberred_core::components::sprite::Sprite;
pub use aberred_core::components::stuck_to::StuckTo;
pub use aberred_core::components::tilemap::Tilemap;
pub use aberred_core::components::timer::{Timer, TimerMode};
pub use aberred_core::components::tween::{Easing, LoopMode, Tween};
pub use aberred_core::components::world_position::WorldPosition;
pub use aberred_core::components::z_index::ZIndex;

pub use aberred_core::events::animation::AnimationFinished;
pub use aberred_core::events::asset::{AssetLoadFailed, AssetLoaded};
pub use aberred_core::events::collision::{CollisionEnded, CollisionOngoing, CollisionStarted};
pub use aberred_core::events::gui_interactable::GuiClicked;
pub use aberred_core::events::menu::MenuSelected;
pub use aberred_core::events::phase::{PhaseEntered, PhaseExited};
pub use aberred_core::events::scene::{SceneEntered, SceneExited, SceneReady};
pub use aberred_core::events::timer::TimerFired;
pub use aberred_core::events::tween::TweenFinished;
pub use aberred_core::resources::input::InputAction;

pub use aberred_core::protocol::asset_kind::AssetKind;
pub use aberred_core::protocol::audio::AudioCmd;
pub use aberred_core::protocol::render_assets::RenderAssetCmd;

pub use aberred_core::resources::animation_store::{AnimationClip, AnimationStore};
pub use aberred_core::resources::app_state::AppState;
pub use aberred_core::resources::camera_follow_config::{CameraFollowConfig, FollowMode};
pub use aberred_core::resources::camera2d::{Camera2D, MainCamera};
pub use aberred_core::resources::game_config::GameConfig;
pub use aberred_core::resources::game_state::{GameState, NextGameState};
pub use aberred_core::resources::gui_theme::{GuiButtonSkin, GuiNinePatch, GuiThemeStore};
pub use aberred_core::resources::input::InputState;
pub use aberred_core::resources::next_scene::NextScene;
pub use aberred_core::resources::pending_assets::PendingAssets;
pub use aberred_core::resources::post_process_shader::PostProcessShader;
pub use aberred_core::resources::render_size::RenderSize;
pub use aberred_core::resources::signal_intents::SignalIntents;
pub use aberred_core::resources::signal_keys as sk;
pub use aberred_core::resources::sim_rng::SimRng;
pub use aberred_core::resources::texture_filter::TextureFilter;
pub use aberred_core::resources::uniform_value::UniformValue;
pub use aberred_core::resources::world_signals::{SignalSnapshot, SignalsRead, WorldSignals};
pub use aberred_core::resources::world_time::WorldTime;

pub use aberred_core::systems::asset_loader::{AssetError, AssetLoader};
pub use aberred_core::systems::game_state::state_is_playing;
pub use aberred_core::systems::scene_dispatch::{WorldDraw, WorldDrawCtx, in_scene};

pub use crate::render::*;

pub use crate::engine_app::{EngineBuilder, SimSet};
