//! Event types and observers used by the engine.
//!
//! This module groups the domain events exchanged across systems and the
//! corresponding observers that react to them. Events provide a decoupled
//! way for systems to communicate without tight coupling or direct
//! dependencies.
//!
//! Submodules:
//! - [`asset`] – asset load completion (`AssetLoaded`/`AssetLoadFailed`)
//! - [`collision`] – collision notifications emitted by the physics/collision system
//! - [`gamestate`] – state transition notifications for the high-level game flow
//! - [`gui_interactable`] – GUI interactable (button/image) click events
//! - [`input`] – input action events (key press/release)
//! - [`menu`] – menu selection events
//! - [`phase`] – phase enter/exit events (`PhaseEntered`/`PhaseExited`)
//! - [`scene`] – scene enter/exit events (`SceneEntered`/`SceneExited`)
//! - [`switchdebug`] – toggle debug rendering and diagnostics on/off (F11, stays logic-side)
//!
//! Lua timer callbacks observe [`timer::TimerFired`] (`aberred_lua::systems::lua_timer_fired`);
//! the render thread's F10 `SwitchFullScreenEvent` lives in `aberred-render`.
//!
//! See each submodule for concrete event data, semantics, and example usage.

pub mod animation;
pub mod asset;
pub mod collision;
pub mod gamestate;
pub mod gui_interactable;
pub mod input;
pub mod menu;
pub mod phase;
pub mod scene;
pub mod spawnmap;
pub mod switchdebug;
pub mod timer;
pub mod tween;
pub mod window;
