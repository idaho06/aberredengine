//! Resources used only by the audio thread's own `bevy_ecs::World` (Phase
//! 7e). Nothing here is ever inserted into or read from the sim/logic
//! world -- the only contract between the two worlds is
//! `AudioCmd`/`AudioMessage` (`crate::protocol::audio`), carried over the
//! channels wrapped by `CmdReceiver`/`MsgSender` below.

pub mod channels;
pub mod store;
