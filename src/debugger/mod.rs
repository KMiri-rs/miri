pub mod channel;
pub mod reachability;
pub mod state;
pub mod tui;
pub mod utils;

use std::cell::RefCell;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use rustc_middle::ty;

use self::channel::{CommandReceiver, StateSender};
pub use self::state::DebuggerState;
use crate::MiriInterpCx;
use crate::concurrency::thread::EvalContextExt;
use crate::debugger::utils::{instance_name, is_src_line_reached};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebuggerCommand {
    Continue,
    StepOver(u32),
    StepFrameTerminator(u32),
    StepBack,
    RunToTerminator(u32),
    RunToInstance(String),
    RunToSrcLine(String),
    RunToEnd,
    Quit,
    QuitWithErr(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DebuggerMode<'tcx> {
    Step(u32),
    Continue,
    RunToTerminator(u32),
    RunToInstance(String),
    RunToSrcLine(String),
    #[expect(unused)]
    RunToEnd,
    StepFrameTerminator(StepFrameTerminatorState<'tcx>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StepFrameTerminatorState<'tcx> {
    remaining: u32,
    anchor_depth: usize,
    // Bind the command to the exact frame instance that was active when the command started.
    // This keeps recursive calls separate from the caller's frame, even when the function name matches.
    anchor_frame: ty::Instance<'tcx>,
}

pub struct MiriDebuggerHandle<'tcx> {
    state_tx: StateSender,
    cmd_rx: CommandReceiver,
    mode: RefCell<DebuggerMode<'tcx>>,
}

impl<'tcx> MiriDebuggerHandle<'tcx> {
    pub fn new(state_tx: StateSender, cmd_rx: CommandReceiver) -> Self {
        Self { state_tx, cmd_rx, mode: RefCell::new(DebuggerMode::Step(1)) }
    }

    fn current_mode(&self) -> DebuggerMode<'tcx> {
        self.mode.borrow().clone()
    }

    fn set_current_mode(&self, mode: DebuggerMode<'tcx>) {
        *self.mode.borrow_mut() = mode;
    }

    pub fn send(&self, ecx: &MiriInterpCx<'tcx>) {
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
                        .map(|frame| instance_name(ecx, frame.instance().def_id()))
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
                DebuggerMode::RunToSrcLine(src_line) => {
                    return if is_src_line_reached(ecx, &src_line) {
                        self.set_current_mode(DebuggerMode::Step(1));
                        // Send the DebuggerState.
                        false
                    } else {
                        true
                    };
                }
                DebuggerMode::StepFrameTerminator(mut state) => {
                    let Some(top_frame) = ecx.active_thread_stack().last() else {
                        return true;
                    };
                    let current_depth = ecx.active_thread_stack().len();
                    let current_frame = top_frame.instance();

                    // If execution went deeper, we are inside a callee. Keep running until we
                    // return to the anchored frame depth before considering a stop again.
                    if current_depth > state.anchor_depth {
                        self.set_current_mode(DebuggerMode::StepFrameTerminator(state));
                        return true;
                    }

                    // If the active frame changed at the anchored depth, we have returned to an
                    // outer caller or resumed in a different frame. Re-anchor so the command keeps
                    // tracking the current visible frame and still stops on the next terminator.
                    if current_depth < state.anchor_depth
                        || (current_depth == state.anchor_depth
                            && current_frame != state.anchor_frame)
                    {
                        state.anchor_depth = current_depth;
                        state.anchor_frame = current_frame;
                    }

                    // Do not stop in the middle of a basic block. We only pause once the current
                    // frame is sitting on its terminator.
                    if !reached_terminator(ecx) {
                        self.set_current_mode(DebuggerMode::StepFrameTerminator(state));
                        return true;
                    }

                    // Support repeat counts by consuming one stop each time we hit a terminator.
                    if state.remaining > 1 {
                        state.remaining -= 1;
                        self.set_current_mode(DebuggerMode::StepFrameTerminator(state));
                        return true;
                    }

                    self.set_current_mode(DebuggerMode::StepFrameTerminator(state));
                }
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
            | DebuggerMode::RunToInstance(_)
            | DebuggerMode::RunToSrcLine(_)
            | DebuggerMode::StepFrameTerminator(_) => (),
            DebuggerMode::Continue => return,
        }

        let state = DebuggerState::capture(ecx);
        let _ = self.state_tx.send(Box::new(state));
    }

    fn reached_terminator_or_step(&self, ecx: &MiriInterpCx<'tcx>) -> bool {
        match self.current_mode() {
            DebuggerMode::RunToTerminator(1) => reached_terminator(ecx),
            DebuggerMode::StepFrameTerminator(state) =>
                state.remaining == 1 && frame_terminator_reached(ecx, &state),
            DebuggerMode::Step(1) => true,
            _ => false,
        }
    }

    pub fn wait_for_command(&self, ecx: &MiriInterpCx<'tcx>) -> Quit {
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
                DebuggerCommand::StepFrameTerminator(n) => {
                    let current_frame =
                        ecx.active_thread_stack().last().map(|frame| frame.instance());
                    let anchor_frame = current_frame.unwrap_or_else(|| {
                        unreachable!("step-frame-terminator requires an active stack frame")
                    });
                    DebuggerMode::StepFrameTerminator(StepFrameTerminatorState {
                        remaining: n + 1,
                        anchor_depth: ecx.active_thread_stack().len(),
                        anchor_frame,
                    })
                }
                // Reverse stepping is handled entirely in the TUI thread.
                DebuggerCommand::StepBack => DebuggerMode::Continue,
                DebuggerCommand::RunToTerminator(n) => DebuggerMode::RunToTerminator(n + 1),
                DebuggerCommand::RunToInstance(target) => DebuggerMode::RunToInstance(target),
                DebuggerCommand::RunToSrcLine(line) => DebuggerMode::RunToSrcLine(line),
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

fn frame_terminator_reached<'tcx>(
    ecx: &MiriInterpCx<'tcx>,
    state: &StepFrameTerminatorState<'tcx>,
) -> bool {
    let Some(top_frame) = ecx.active_thread_stack().last() else {
        return false;
    };
    let current_depth = ecx.active_thread_stack().len();
    let current_frame = top_frame.instance();
    // Only the anchored frame, or one of its exact recursive re-entries, may satisfy this mode.
    if current_depth > state.anchor_depth {
        return false;
    }
    if current_depth == state.anchor_depth && current_frame != state.anchor_frame {
        return false;
    }
    reached_terminator(ecx)
}

pub fn debugger_log(s: impl std::fmt::Display) {
    use std::fs::{File, OpenOptions};
    use std::io::Write;

    static OPENED: AtomicBool = AtomicBool::new(false);

    static FILE: LazyLock<File> = LazyLock::new(|| {
        let mut opts = OpenOptions::new();
        let opened = OPENED.swap(true, Ordering::Relaxed);

        if opened {
            opts.append(true);
        } else {
            opts.create(true).write(true).truncate(true);
        };
        opts.open("miri_debugger.log").unwrap()
    });

    let mut file = &*FILE;
    writeln!(file, "{s}").unwrap();
    // file.flush();
}

static RECORD_ALL_STATES: AtomicBool = AtomicBool::new(false);
pub fn toggle_record_all_states() {
    RECORD_ALL_STATES.fetch_xor(true, Ordering::Relaxed);
}
pub fn get_record_all_states() -> bool {
    RECORD_ALL_STATES.load(Ordering::Acquire)
}
