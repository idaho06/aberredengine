//! Channel-wrapping resources for the audio thread's own `World`.

use bevy_ecs::prelude::Resource;
use crossbeam_channel::{Receiver, Sender};

use aberred_core::protocol::audio::{AudioCmd, AudioMessage};

#[derive(Resource)]
pub struct CmdReceiver(pub Receiver<AudioCmd>);

#[derive(Resource)]
pub struct MsgSender(pub Sender<AudioMessage>);

#[derive(Resource, Default)]
pub struct ShouldExit(pub bool);
