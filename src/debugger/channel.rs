use std::sync::mpsc::{self, Receiver, Sender};

use super::{DebuggerCommand, DebuggerState};

pub type StateSender = Sender<Box<DebuggerState>>;
pub type StateReceiver = Receiver<Box<DebuggerState>>;
pub type CommandSender = Sender<DebuggerCommand>;
pub type CommandReceiver = Receiver<DebuggerCommand>;

pub fn state_channel() -> (StateSender, StateReceiver) {
    mpsc::channel()
}

pub fn command_channel() -> (CommandSender, CommandReceiver) {
    mpsc::channel()
}
