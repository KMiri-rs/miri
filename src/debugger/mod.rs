pub mod channel;
mod state;
pub mod tui;

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use self::channel::{CommandReceiver, StateSender};
pub use self::state::DebuggerState;
use crate::MiriInterpCx;
use crate::concurrency::thread::EvalContextExt;
use crate::debugger::channel::StateOrEvent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebuggerCommand {
    Continue,
    StepOver(u32),
    StepBack,
    RunToTerminator(u32),
    RunToFrame(String),
    RunToMain,
    RunToEnd,
    Quit,
    QuitWithErr(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DebuggerMode {
    Step(u32),
    Continue,
    RunToTerminator(u32),
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
        let handle =
            std::thread::Builder::new().name("miri-debugger-tui-event".to_string()).spawn({
                let state_tx = state_tx.clone();
                move || {
                    while let Ok(event) = crossterm::event::read() {
                        state_tx.send(StateOrEvent::Event(event));
                    }
                }
            });
        // Make the thread run on the background and never die.
        drop(handle);

        Self { state_tx, cmd_rx, mode: RefCell::new(DebuggerMode::Step(1)) }
    }

    fn current_mode(&self) -> DebuggerMode {
        self.mode.borrow().clone()
    }

    fn set_current_mode(&self, mode: DebuggerMode) {
        *self.mode.borrow_mut() = mode;
    }

    pub fn send(&self, ecx: &MiriInterpCx<'_>) {
        let update_mode = || {
            match self.current_mode() {
                DebuggerMode::Step(2 | 0) => self.set_current_mode(DebuggerMode::Step(1)),
                DebuggerMode::Step(n) if n > 1 => {
                    self.set_current_mode(DebuggerMode::Step(n - 1));
                    return true;
                }
                DebuggerMode::RunToTerminator(n) =>
                    if reached_terminator(ecx) {
                        let n = if n > 1 { n - 1 } else { 1 };
                        // Only decrement when reaching a terminator.
                        self.set_current_mode(DebuggerMode::RunToTerminator(n));
                    } else {
                        return true;
                    },
                DebuggerMode::Continue => return true,
                _ => (),
            }
            false
        };
        let ret = update_mode();
        if ret && !get_record_all_states() {
            return;
        }

        let state = DebuggerState::capture(ecx);
        match self.current_mode() {
            DebuggerMode::Step(_) | DebuggerMode::RunToTerminator(_) | DebuggerMode::RunToEnd => (),
            DebuggerMode::RunToFrame(ref target) => {
                let target_lc = target.to_ascii_lowercase();
                if state
                    .stack_frames
                    .iter()
                    .any(|frame| frame.fn_name.to_ascii_lowercase().contains(&target_lc))
                {
                    self.set_current_mode(DebuggerMode::Step(1));
                }
                return;
            }
            DebuggerMode::RunToMain => {
                if state.in_user_code {
                    self.set_current_mode(DebuggerMode::Step(1));
                }
                return;
            }
            DebuggerMode::Continue => unreachable!(),
        }

        self.state_tx.send(StateOrEvent::State(state)).unwrap();
    }

    fn reached_terminator_or_step(&self, ecx: &MiriInterpCx<'_>) -> bool {
        match self.current_mode() {
            DebuggerMode::RunToTerminator(1) => reached_terminator(ecx),
            DebuggerMode::Step(1) => true,
            _ => false,
        }
    }

    pub fn wait_for_continue(&self, ecx: &MiriInterpCx<'_>) -> DebuggerCommand {
        if !self.reached_terminator_or_step(ecx) {
            return DebuggerCommand::Continue;
        }

        let cmd = self.cmd_rx.recv().unwrap_or(DebuggerCommand::Continue);
        'm: {
            // The step or run count is intentionally added with 1, because the count decrements
            // before send happens.
            *self.mode.borrow_mut() = match cmd {
                DebuggerCommand::Continue => DebuggerMode::Continue,
                DebuggerCommand::StepOver(n) => DebuggerMode::Step(n + 1),
                // Reverse stepping is handled entirely in the TUI thread.
                DebuggerCommand::StepBack => DebuggerMode::Continue,
                DebuggerCommand::RunToTerminator(n) => DebuggerMode::RunToTerminator(n + 1),
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

pub fn debugger_log(s: String) {
    use std::fs::OpenOptions;
    use std::io::Write;

    static OPENED: AtomicBool = AtomicBool::new(false);
    let opened = OPENED.swap(true, Ordering::Relaxed);
    let mut opts = OpenOptions::new();

    if opened {
        opts.append(true);
    } else {
        opts.create(true).write(true).truncate(true);
    };

    let mut file = opts.open("miri_debugger.log").unwrap();
    writeln!(&file, "{s}").unwrap();
    file.flush();
}

static RECORD_ALL_STATES: AtomicBool = AtomicBool::new(true);
pub fn toggle_record_all_states() {
    RECORD_ALL_STATES.fetch_xor(true, Ordering::Relaxed);
}
pub fn get_record_all_states() -> bool {
    RECORD_ALL_STATES.load(Ordering::Acquire)
}
