pub mod channel;
pub mod reachability;
pub mod state;
pub mod tui;
pub mod utils;

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
    RunToInstance(String),
    RunToEnd,
    Quit,
    QuitWithErr(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DebuggerMode {
    Step(u32),
    Continue,
    RunToTerminator(u32),
    RunToInstance(String),
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
        // Return value of the closure means that send early returns.
        let update_mode = || {
            let mode = self.current_mode();
            match mode {
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
                DebuggerMode::RunToInstance(target) =>
                    return match ecx
                        .active_thread_stack()
                        .last()
                        .map(|frame| frame.instance().to_string())
                    {
                        Some(current_fn) => {
                            if target == current_fn {
                                self.set_current_mode(DebuggerMode::Step(1));
                                // Send the DebuggerState.
                                false
                            } else {
                                true
                            }
                        }
                        None => true,
                    },
                _ => (),
            }
            false
        };
        let early_return = update_mode();
        if early_return && !get_record_all_states() {
            // Early return if we don't record all intermediate states.
            return;
        }

        match self.current_mode() {
            DebuggerMode::Step(_)
            | DebuggerMode::RunToTerminator(_)
            | DebuggerMode::RunToEnd
            | DebuggerMode::RunToInstance(_) => (),
            DebuggerMode::Continue => return,
        }

        let state = DebuggerState::capture(ecx);
        let _ = self.state_tx.send(StateOrEvent::State(Box::new(state)));
    }

    fn reached_terminator_or_step(&self, ecx: &MiriInterpCx<'_>) -> bool {
        match self.current_mode() {
            DebuggerMode::RunToTerminator(1) => reached_terminator(ecx),
            DebuggerMode::Step(1) => true,
            _ => false,
        }
    }

    pub fn wait_for_command(&self, ecx: &MiriInterpCx<'_>) -> Quit {
        if !self.reached_terminator_or_step(ecx) {
            return Quit::No;
        }

        let cmd = self.cmd_rx.recv().unwrap_or(DebuggerCommand::Continue);
        let quit = match &cmd {
            DebuggerCommand::Quit => Quit::Yes,
            DebuggerCommand::QuitWithErr(err) => Quit::YesWithErr(err.clone()),
            _ => Quit::No,
        };
        'm: {
            // The step or run count is intentionally added with 1, because the count decrements
            // before send happens.
            *self.mode.borrow_mut() = match cmd {
                DebuggerCommand::Continue => DebuggerMode::Continue,
                DebuggerCommand::StepOver(n) => DebuggerMode::Step(n + 1),
                // Reverse stepping is handled entirely in the TUI thread.
                DebuggerCommand::StepBack => DebuggerMode::Continue,
                DebuggerCommand::RunToTerminator(n) => DebuggerMode::RunToTerminator(n + 1),
                DebuggerCommand::RunToInstance(target) => DebuggerMode::RunToInstance(target),
                DebuggerCommand::RunToEnd => DebuggerMode::Continue,
                DebuggerCommand::Quit => break 'm,
                DebuggerCommand::QuitWithErr(_) => break 'm,
            }
        };
        quit
    }
}

pub enum Quit {
    No,
    Yes,
    YesWithErr(String),
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

static RECORD_ALL_STATES: AtomicBool = AtomicBool::new(false);
pub fn toggle_record_all_states() {
    RECORD_ALL_STATES.fetch_xor(true, Ordering::Relaxed);
}
pub fn get_record_all_states() -> bool {
    RECORD_ALL_STATES.load(Ordering::Acquire)
}
