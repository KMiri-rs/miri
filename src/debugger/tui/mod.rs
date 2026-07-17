#![deny(dead_code)]
use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use super::channel::{CommandSender, StateReceiver};
use super::{DebuggerCommand, DebuggerState};
use crate::debugger::tui::event::{Action, is_quit_event};
use crate::debugger::tui::pane::FocusPane;
use crate::debugger::tui::pane::panes::Panes;

mod event;
mod pane;
pub mod theme;

const HISTORY_CAPACITY: usize = 1000;
type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Step,
    Continue,
    RunToTerminator,
    StepFrameTerminator,
    RunToInstance,
    RunToEnd,
}

impl RunMode {
    fn as_str(self) -> &'static str {
        match self {
            RunMode::Step => "step",
            RunMode::Continue => "continue",
            RunMode::RunToTerminator => "run-to-terminator",
            RunMode::StepFrameTerminator => "step-frame-terminator",
            RunMode::RunToInstance => "run-to-instance",
            RunMode::RunToEnd => "run-to-end",
        }
    }
}

pub struct Context {
    mode: RunMode,
    run_to_instance_target: Option<String>,
    last_state: Option<Box<DebuggerState>>,
    history: VecDeque<DebuggerState>,
    blink_epoch: Instant,
    reverse_index: Option<usize>,
    program_finished: bool,
    count: String,
    display_dead_allocs: bool,
}

impl Context {
    fn new() -> Context {
        Context {
            mode: RunMode::Step,
            run_to_instance_target: None,
            last_state: None,
            history: VecDeque::with_capacity(HISTORY_CAPACITY),
            blink_epoch: Instant::now(),
            reverse_index: None,
            program_finished: false,
            count: String::new(),
            // Hide dead allocations by default.
            display_dead_allocs: false,
        }
    }

    fn on_new_state(&mut self, state: &DebuggerState) {
        self.history.push_back(state.clone());
        if self.history.len() > HISTORY_CAPACITY {
            self.history.pop_front();
        }
        self.last_state = Some(Box::new(state.clone()));
        self.reverse_index = None;
    }
}

pub fn spawn_tui(
    state_rx: StateReceiver,
    command_tx: CommandSender,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("miri-debugger-tui".to_string())
        .spawn(move || {
            let cmd_tx = command_tx.clone();
            if let Err(err) = run_tui(state_rx, command_tx) {
                _ = cmd_tx
                    .send(DebuggerCommand::QuitWithErr(format!("Failed to run tui: {err:?}")));
            }
        })
        .expect("failed to spawn debugger TUI thread")
}

#[track_caller]
fn run_tui(state_rx: StateReceiver, command_tx: CommandSender) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = tui_loop(&mut terminal, state_rx, command_tx);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn tui_loop(
    terminal: &mut Terminal,
    state_rx: StateReceiver,
    command_tx: CommandSender,
) -> io::Result<()> {
    let mut panes = Panes::new(Rect::default());
    let mut ctx = Context::new();
    let mut needs_redraw = false;

    loop {
        match state_rx.recv_timeout(Duration::from_millis(16)) {
            Ok(state) => {
                refresh_state(&mut panes, &mut ctx, &state);
                needs_redraw = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        while crossterm::event::poll(Duration::from_millis(0))? {
            let event = crossterm::event::read()?;
            if handle_tui_event(event, &mut panes, &mut ctx, &command_tx)? {
                return Ok(());
            }
            needs_redraw = true;
        }

        if needs_redraw && let Some(state) = ctx.last_state.clone() {
            terminal.draw(|frame| render(&mut panes, frame, &state, &ctx))?;
            needs_redraw = false;
        }
    }

    // Program is done; keep the final snapshot visible until the user explicitly quits.
    ctx.program_finished = true;
    if let Some(state) = ctx.last_state.take() {
        finished(terminal, panes, &state, ctx, command_tx)
    } else {
        finished_without_snapshot(terminal)
    }
}

fn refresh_state(panes: &mut Panes, ctx: &mut Context, state: &DebuggerState) {
    ctx.on_new_state(state);
    panes.stack.refresh(state);
    panes.instances.refresh(state);
    if !state.stack_frames.is_empty() {
        panes.stack.index = panes.stack.index.min(state.stack_frames.len() - 1);
    } else {
        panes.stack.index = 0;
    }
    if !state.function_instances.is_empty() {
        panes.instances.index = panes.instances.index.min(state.function_instances.len() - 1);
    } else {
        panes.instances.index = 0;
    }
}

fn handle_tui_event(
    event: Event,
    panes: &mut Panes,
    ctx: &mut Context,
    command_tx: &CommandSender,
) -> io::Result<bool> {
    let Some(state) = ctx.last_state.clone() else {
        if is_quit_event(&event) {
            let _ = command_tx.send(DebuggerCommand::Quit);
            return Ok(true);
        }
        return Ok(false);
    };

    match event::handle(event, panes, &state, ctx, command_tx)? {
        Action::Continue | Action::Break => Ok(false),
        Action::Return => Ok(true),
    }
}

fn finished(
    terminal: &mut Terminal,
    mut panes: Panes,
    state: &DebuggerState,
    mut ctx: Context,
    command_tx: CommandSender,
) -> io::Result<()> {
    ctx.mode = RunMode::Step;
    ctx.reverse_index = None;
    panes.instances.search.editing = false;
    loop {
        terminal.draw(|frame| render(&mut panes, frame, state, &ctx))?;

        let event = crossterm::event::read()?;
        match event::handle(event, &mut panes, state, &mut ctx, &command_tx)? {
            Action::Continue => (),
            Action::Break | Action::Return => return Ok(()),
        }
    }
}

fn finished_without_snapshot(terminal: &mut Terminal) -> io::Result<()> {
    let text = Paragraph::new("Program finished before first debugger snapshot. Press q to close.")
        .block(Block::default().title("Miri Debugger").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    loop {
        terminal.draw(|frame| frame.render_widget(text.clone(), frame.area()))?;
        if event::fast_quit()? {
            return Ok(());
        }
    }
}

fn render(panes: &mut Panes, frame: &mut Frame<'_>, state: &DebuggerState, ctx: &Context) {
    let display_dead = ctx.display_dead_allocs;
    let query = panes.instances.search.query_as_u64();

    panes.update_area(frame.area());

    panes.render_mir(frame, state);
    panes.render_stack(frame, state);
    panes.render_instances(frame, state, ctx.blink_epoch);
    panes.render_src(frame, state);
    panes.render_locals(frame, state, display_dead, query);
    panes.render_allocations(frame, state, display_dead, query);
    panes.render_output(frame, state);
    panes.render_status_bar(frame, state, ctx);

    if panes.is_focused(FocusPane::BorrowStacks) {
        panes.render_borrow_stack(frame, state, display_dead, query);
    }
}
