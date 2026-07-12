//! Cross-thread communication contract for the engine's render/logic/audio
//! split (Phase 5e onward; see `docs/plans/phase7a-protocol-module.md`).
//!
//! Every type in this module (and its submodules) is plain `Send` data:
//! channel message enums, bridge endpoint resources, and shutdown
//! signaling. **Nothing here may hold a raylib handle, a GL resource
//! (`Texture2D`/`Font`/`Shader`), or any other `NonSend` value** — that
//! invariant is what structurally guarantees the render, logic, and audio
//! threads never share non-`Send` state. `render_logic.rs` asserts this at
//! compile time for `LogicMsg`/`RenderMsg`.
//!
//! Submodules:
//! - [`render_logic`] – `LogicMsg`/`RenderMsg`, the render<->logic channel
//!   payloads
//! - [`audio`] – `AudioCmd`/`AudioMessage`, the ECS<->audio-thread channel
//!   payloads (also used as bevy `Message`s in `Messages<T>` queues)
//! - [`endpoints`] – `LogicBridge`/`LogicTx`/`RenderTx`/`AudioBridge` bridge
//!   resources plus their setup/shutdown helpers
//! - [`shutdown`] – global running flag + panic hook, the emergency-path
//!   shutdown signal checked by every thread's loop alongside the primary
//!   message-based shutdown path

pub mod audio;
pub mod endpoints;
pub mod render_logic;
pub mod shutdown;
