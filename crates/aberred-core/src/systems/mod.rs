//! Engine systems.
//!
//! This module groups all ECS systems that advance simulation, input, and
//! rendering.
//!
//! Submodules overview
//! - [`animation`] – advance sprite animations and select tracks via rules
//! - [`camera_follow`] – move the camera to track entities with `CameraTarget`
//! - [`audio_bridge`] – logic-thread systems that shuttle `AudioCmd`/`AudioMessage` with the audio thread
//! - [`collision_detector`] – broad/simple overlap checks and event emission
//! - [`collision_rule_index`] – rebuilds `CollisionRuleIndex` from rule entities on change
//! - [`gamestate`] – check for pending state transitions and trigger events
//! - [`gridlayout`] – spawn entities from JSON-defined grid layouts
//! - [`group`] – count entities per tracked group and publish to [`WorldSignals`](crate::resources::worldsignals::WorldSignals)
//! - [`gui_interactable_click`] – dispatch the Lua/Rust callback chain for a clicked GUI widget (`GuiButton`/`GuiImage`)
//! - [`gui_hit_test`] – resolve `GuiInteractable` hover/press/click state from cursor + mouse button
//! - [`gui_layout`] – resolve GUI children's `ScreenPosition` from parent `ScreenPosition` + `GuiOffset`
//! - [`gui_progressbar_signal_update`] – keep `GuiProgressBar.value` in sync with `WorldSignals` for signal-bound bars
//! - [`gui_spawn`] – spawn a `GuiButton`/`GuiLabel`/`GuiImage`'s `GuiInteractable`/caption/`Sprite` on `Added<T>`
//! - [`input`] – read hardware input and update [`crate::resources::input::InputState`]
//! - [`inputsimplecontroller`] – translate input state into velocity on entities
//! - [`inputaccelerationcontroller`] – translate input state into acceleration on entities
//! - [`menu`] – menu spawning, input handling, and selection
//! - [`mousecontroller`] – update entity positions based on mouse position
//! - [`movement`] – integrate positions from rigid body velocities and time
//! - [`phase`] – process Rust phase state machine transitions and callbacks
//! - [`rust_collision`] – Rust-native collision observer and callback dispatch
//! - [`scene_dispatch`] – scene switch and update systems for `SceneManager`-based games
//! - [`signal_intents`] – apply buffered `SignalIntent`s queued by render-side scene callbacks
//! - [`signalbinding`] – update DynamicText components based on signal values
//! - [`stuckto`] – keep entities attached to other entities
//! - [`time`] – update simulation time and delta
//! - [`tween`] – animate position, rotation, and scale over time
//!
//! The audio thread's systems live in `aberred-audio`, the render thread's in
//! `aberred-render`, and the Lua-callback systems (`lua_collision`,
//! `luaphase`, `lua_commands`, ...) in `aberred-lua`.

pub use game_ctx::GameCtx;

pub mod animation;
pub mod audio_bridge;
pub mod camera_follow;
pub mod collision;
pub mod collision_detector;
pub mod collision_rule_index;
pub mod dynamictext_size;
pub mod entity_registrations;
pub mod game_ctx;
pub mod gamestate;
pub mod gridlayout;
pub mod group;
pub mod gui_hit_test;
pub mod gui_image_state_sync;
pub mod gui_interactable_click;
pub mod gui_layout;
pub mod gui_progressbar_signal_update;
pub mod gui_spawn;
pub mod input;
pub mod inputaccelerationcontroller;
pub mod inputsimplecontroller;
pub mod logic_bridge;
pub mod mapspawn;
pub mod menu;
pub mod mousecontroller;
pub mod movement;
pub mod particleemitter;
pub mod phase;
pub mod phase_core;
pub mod propagate_transforms;
pub mod render_assets;
pub mod rust_collision;
pub mod scene_dispatch;
pub mod signal_intents;
pub mod signalbinding;
pub mod state_hash;
pub mod stuckto;
pub mod tilemap;
pub mod time;
pub mod timer;
pub mod timer_core;
pub mod transform_compose;
pub mod ttl;
pub mod tween;
pub mod window;
