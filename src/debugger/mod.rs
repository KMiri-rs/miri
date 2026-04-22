pub mod channel;
mod state;
pub mod tui;

use std::cell::RefCell;

use self::channel::{CommandReceiver, StateSender};
pub use self::state::DebuggerState;
use crate::MiriInterpCx;
use crate::concurrency::thread::EvalContextExt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebuggerCommand {
    Continue,
    StepOver,
    StepBack,
    RunToTerminator,
    RunToFrame(String),
    RunToMain,
    RunToEnd,
    Quit,
    QuitWithErr(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DebuggerMode {
    Step,
    Continue,
    RunToTerminator,
    RunToFrame(String),
    RunToMain,
    RunToEnd,
}

pub struct MiriDebuggerHandle {
    state_tx: StateSender,
    cmd_rx: CommandReceiver,
    mode: RefCell<DebuggerMode>,
}

impl MiriDebuggerHandle {
    pub fn new(state_tx: StateSender, cmd_rx: CommandReceiver) -> Self {
        Self { state_tx, cmd_rx, mode: RefCell::new(DebuggerMode::Step) }
    }

    fn current_mode(&self) -> DebuggerMode {
        self.mode.borrow().clone()
    }

    fn reached_terminator_or_step(&self, ecx: &MiriInterpCx<'_>) -> bool {
        match self.current_mode() {
            DebuggerMode::RunToTerminator => reached_terminator(ecx),
            DebuggerMode::Step => true,
            _ => false,
        }
    }

    pub fn send(&self, ecx: &MiriInterpCx<'_>) {
        let mode = self.current_mode();
        if mode == DebuggerMode::Continue
            || (mode == DebuggerMode::RunToTerminator && !reached_terminator(ecx))
        {
            return;
        }

        let state = DebuggerState::capture(ecx);
        match mode {
            DebuggerMode::Step | DebuggerMode::RunToTerminator | DebuggerMode::RunToEnd => (),
            DebuggerMode::RunToFrame(ref target) => {
                let target_lc = target.to_ascii_lowercase();
                if state
                    .stack_frames
                    .iter()
                    .any(|frame| frame.fn_name.to_ascii_lowercase().contains(&target_lc))
                {
                    *self.mode.borrow_mut() = DebuggerMode::Step;
                }
                return;
            }
            DebuggerMode::RunToMain => {
                if state.in_user_code {
                    *self.mode.borrow_mut() = DebuggerMode::Step;
                }
                return;
            }
            DebuggerMode::Continue => unreachable!(),
        }

        self.state_tx.send(state).unwrap();
    }

    pub fn wait_for_continue(&self, ecx: &MiriInterpCx<'_>) -> DebuggerCommand {
        if !self.reached_terminator_or_step(ecx) {
            return DebuggerCommand::Continue;
        }

        let cmd = self.cmd_rx.recv().unwrap_or(DebuggerCommand::Continue);
        'm: {
            *self.mode.borrow_mut() = match cmd {
                DebuggerCommand::Continue => DebuggerMode::Continue,
                DebuggerCommand::StepOver => DebuggerMode::Step,
                // Reverse stepping is handled entirely in the TUI thread.
                DebuggerCommand::StepBack => DebuggerMode::Continue,
                DebuggerCommand::RunToTerminator => DebuggerMode::RunToTerminator,
                DebuggerCommand::RunToFrame(_) => DebuggerMode::Continue,
                DebuggerCommand::RunToMain => DebuggerMode::Continue,
                DebuggerCommand::RunToEnd => DebuggerMode::Continue,
                DebuggerCommand::Quit => break 'm,
                DebuggerCommand::QuitWithErr(_) => break 'm,
            }
        };
        cmd
    }
}

fn reached_terminator(ecx: &MiriInterpCx<'_>) -> bool {
    if let Some(frame) = ecx.active_thread_stack().last()
        && let Some(loc) = frame.current_loc().left()
        && loc.statement_index >= frame.body().basic_blocks[loc.block].statements.len()
    {
        return true;
    }
    false
}
