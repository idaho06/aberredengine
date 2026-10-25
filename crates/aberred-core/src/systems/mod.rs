//! Engine systems.
//!
//! This module groups all ECS systems that advance simulation, input, and
//! rendering.
//!
//! Submodules overview
//! - [`animation`] – advance sprite animations and select tracks via rules
//! - [`asset_gate`] – `AssetGate`: rejects asset changes during deterministic `Playing`
//! - [`asset_loader`] – [`AssetLoader`](asset_loader::AssetLoader) system param: queue render/audio asset loads
//! - [`camera_follow`] – move the camera to track entities with `CameraTarget`
//! - [`asset_tracking`] – settle `PendingAssets` on load replies and trigger `AssetLoaded`/`AssetLoadFailed`
//! - [`audio_bridge`] – logic-thread systems that shuttle `AudioCmd`/`AudioMessage` with the audio thread
//! - [`collision_detector`] – broad/simple overlap checks and event emission
//! - [`collision_rule`] – matches overlaps against `CollisionRule`s and triggers `CollisionOngoing`
//! - [`collision_rule_index`] – rebuilds `CollisionRuleIndex` from rule entities on change
//! - [`game_state`] – check for pending state transitions and trigger events
//! - [`grid_layout`] – spawn entities from JSON-defined grid layouts
//! - [`group`] – count entities per tracked group and publish to [`WorldSignals`](crate::resources::world_signals::WorldSignals)
//! - [`gui_hit_test`] – resolve `GuiInteractable` hover/press/click state from cursor + mouse button
//! - [`gui_layout`] – resolve GUI children's `ScreenPosition` from parent `ScreenPosition` + `GuiOffset`
//! - [`gui_progress_bar_signal_update`] – keep `GuiProgressBar.value` in sync with `WorldSignals` for signal-bound bars
//! - [`gui_spawn`] – spawn a `GuiButton`/`GuiLabel`/`GuiImage`'s `GuiInteractable`/caption/`Sprite` on `Added<T>`
//! - [`input`] – read hardware input and update [`crate::resources::input::InputState`]
//! - [`input_simple_controller`] – translate input state into velocity on entities
//! - [`input_acceleration_controller`] – translate input state into acceleration on entities
//! - [`menu`] – menu spawning, input handling, and selection
//! - [`mouse_controller`] – update entity positions based on mouse position
//! - [`movement`] – integrate positions from rigid body velocities and time
//! - [`phase`] – apply phase state machine transitions and trigger phase events
//! - [`scene_dispatch`] – scene switch and update systems for `SceneManager`-based games
//! - [`signal_intents`] – apply buffered `SignalIntent`s queued by render-side scene callbacks
//! - [`signal_binding`] – update DynamicText components based on signal values
//! - [`stuck_to`] – keep entities attached to other entities
//! - [`time`] – update simulation time and delta
//! - [`tween`] – animate position, rotation, and scale over time
//!
//! The audio thread's systems live in `aberred-audio`, the render thread's in
//! `aberred-render`, and the Lua-callback systems (`lua_collision`,
//! `lua_phase`, `lua_commands`, ...) in `aberred-lua`.

pub mod animation;
pub mod asset_gate;
pub mod asset_loader;
pub mod asset_tracking;
pub mod audio_bridge;
pub mod camera_follow;
pub mod collision;
pub mod collision_detector;
pub mod collision_rule;
pub mod collision_rule_index;
pub mod dynamic_text_size;
pub mod entity_registrations;
pub mod game_state;
pub mod grid_layout;
pub mod group;
pub mod gui_hit_test;
pub mod gui_image_state_sync;
pub mod gui_layout;
pub mod gui_progress_bar_signal_update;
pub mod gui_spawn;
pub mod input;
pub mod input_acceleration_controller;
pub mod input_simple_controller;
pub mod loading_screen;
pub mod logic_bridge;
pub mod map_spawn;
pub mod menu;
pub mod mouse_controller;
pub mod movement;
pub mod particle_emitter;
pub mod phase;
pub mod propagate_transforms;
pub mod render_assets;
pub mod scene_dispatch;
pub mod signal_binding;
pub mod signal_intents;
pub mod sprite_geometry;
pub mod state_hash;
pub mod stuck_to;
pub mod tilemap;
pub mod time;
pub mod timer;
pub mod transform_compose;
pub mod ttl;
pub mod tween;
pub mod window;
