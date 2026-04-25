use std::sync::mpsc::{self, Receiver, Sender};

use crossterm::event::Event;

use super::{DebuggerCommand, DebuggerState};

pub type StateSender = Sender<StateOrEvent>;
pub type StateReceiver = Receiver<StateOrEvent>;
pub type CommandSender = Sender<DebuggerCommand>;
pub type CommandReceiver = Receiver<DebuggerCommand>;

pub enum StateOrEvent {
    State(DebuggerState),
    Event(Event),
}

pub fn state_channel() -> (StateSender, StateReceiver) {
    mpsc::channel()
}

pub fn command_channel() -> (CommandSender, CommandReceiver) {
    mpsc::channel()
}
