//! Cross-thread communication contract for the engine's render/logic/audio
//! split.
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
//! - [`raw_input`] – `RawDeviceSnapshot`/`InputSample`, the dedicated
//!   bounded-channel payload carrying unresolved device input from the
//!   render thread to the sim thread
//! - [`snapshot`] – `SnapshotPublisher`/`SnapshotConsumer`, the `triple_buffer`
//!   transport for `DrawableSnapshot` — the one exception to "everything
//!   here is a crossbeam channel payload": `Input`/`Output` are still plain
//!   `Send + Sync` data, just not channel-shaped.
//! - [`shutdown`] – global running flag + panic hook, the emergency-path
//!   shutdown signal checked by every thread's loop alongside the primary
//!   message-based shutdown path

pub mod audio;
pub mod endpoints;
pub mod raw_input;
pub mod render_logic;
pub mod shutdown;
pub mod snapshot;
