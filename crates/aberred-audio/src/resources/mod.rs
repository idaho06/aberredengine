//! Resources used only by the audio thread's own `bevy_ecs::World`. See the
//! crate root docs for the sim/logic-world boundary this crate keeps; the
//! `AudioCmd`/`AudioMessage` contract is carried over the channels wrapped
//! by `CmdReceiver`/`MsgSender` below.

pub mod channels;
pub mod store;
