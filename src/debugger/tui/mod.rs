#![deny(dead_code)]
use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Wrap,
};

use super::channel::{CommandSender, StateReceiver};
use super::state::LocalKind;
use super::{DebuggerCommand, DebuggerState};
use crate::debugger::tui::event::Action;
use crate::debugger::tui::pane::panes::Panes;
use crate::debugger::tui::pane::status_bar::StatusBar;

mod event;
mod pane;
mod theme;

const HISTORY_CAPACITY: usize = 1000;
type Terminal = ratatui::Terminal<CrosstermBackend<io::Stdout>>;

#[derive(Clone, Copy, Debug)]
enum FocusPane {
    Stack,
    Mir,
    Locals,
    Memory,
    Output,
    StatusBar,
}

impl FocusPane {
    fn next(self) -> Self {
        match self {
            FocusPane::Stack => FocusPane::Mir,
            FocusPane::Mir => FocusPane::Locals,
            FocusPane::Locals => FocusPane::Memory,
            FocusPane::Memory => FocusPane::Output,
            FocusPane::Output => FocusPane::Stack,
            FocusPane::StatusBar => unreachable!(),
        }
    }

    fn previous(self) -> Self {
        match self {
            FocusPane::Stack => FocusPane::Output,
            FocusPane::Mir => FocusPane::Stack,
            FocusPane::Locals => FocusPane::Mir,
            FocusPane::Memory => FocusPane::Locals,
            FocusPane::Output => FocusPane::Memory,
            FocusPane::StatusBar => unreachable!(),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            FocusPane::Stack => "stack",
            FocusPane::Mir => "mir",
            FocusPane::Locals => "locals",
            FocusPane::Memory => "memory",
            FocusPane::Output => "output",
            FocusPane::StatusBar => "status_bar",
        }
    }
}

#[derive(Clone, Copy)]
enum RunMode {
    Step,
    Continue,
    RunToFrame,
    RunToMain,
    RunToEnd,
}

impl RunMode {
    fn as_str(self) -> &'static str {
        match self {
            RunMode::Step => "step",
            RunMode::Continue => "continue",
            RunMode::RunToFrame => "run-to-frame",
            RunMode::RunToMain => "run-to-main",
            RunMode::RunToEnd => "run-to-end",
        }
    }

    fn is_fast_mode(self, in_user_code: bool) -> bool {
        (matches!(self, RunMode::RunToMain) && !in_user_code)
            || matches!(self, RunMode::RunToFrame | RunMode::RunToEnd)
    }
}

#[derive(Default)]
struct RunTargetState {
    editing: bool,
    query: String,
}

pub fn spawn_tui(
    state_rx: StateReceiver,
    command_tx: CommandSender,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("miri-debugger-tui".to_string())
        .spawn(move || {
            if let Err(err) = run_tui(state_rx, command_tx) {
                eprintln!("debugger TUI error: {err}");
            }
        })
        .expect("failed to spawn debugger TUI thread")
}

fn run_tui(state_rx: StateReceiver, command_tx: CommandSender) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = tui_loop(&mut terminal, state_rx, command_tx);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

pub struct Meta {
    mode: RunMode,
    run_target: RunTargetState,
    run_to_frame_target: Option<String>,
    last_state: Option<DebuggerState>,
    history: VecDeque<DebuggerState>,
    blink_epoch: Instant,
    reverse_index: Option<usize>,
}

impl Meta {
    fn new() -> Meta {
        Meta {
            mode: RunMode::Step,
            run_target: RunTargetState::default(),
            run_to_frame_target: None,
            last_state: None,
            history: VecDeque::with_capacity(HISTORY_CAPACITY),
            blink_epoch: Instant::now(),
            reverse_index: None,
        }
    }

    fn on_new_state(&mut self, state: &DebuggerState) {
        self.history.push_back(state.clone());
        if self.history.len() > HISTORY_CAPACITY {
            self.history.pop_front();
        }
        self.last_state = Some(state.clone());
        self.reverse_index = None;
    }
}

fn tui_loop(
    terminal: &mut Terminal,
    state_rx: StateReceiver,
    command_tx: CommandSender,
) -> io::Result<()> {
    let mut panes = Panes::new(Rect::default());
    let mut meta = Meta::new();
    let command_tx = &command_tx;

    while let Ok(state) = state_rx.recv() {
        meta.on_new_state(&state);

        panes.stack.refresh(&state);
        if !state.stack_frames.is_empty() {
            panes.stack.index = panes.stack.index.min(state.stack_frames.len() - 1);
        } else {
            panes.stack.index = 0;
        }

        let mut display_state = state.clone();

        if matches!(meta.mode, RunMode::RunToFrame)
            && meta
                .run_to_frame_target
                .as_ref()
                .is_some_and(|target| state_has_frame(&state, target))
        {
            meta.mode = RunMode::Step;
            meta.run_to_frame_target = None;
        }

        // In fast-forward mode, keep rendering every step without waiting for input.
        if meta.mode.is_fast_mode(state.in_user_code) {
            terminal.draw(|frame| {
                let status_bar = StatusBar { meta: &meta, program_finished: false };
                render(&mut panes, frame, &display_state, &status_bar)
            })?;

            if event::fast_quit()? {
                let _ = command_tx.send(DebuggerCommand::Quit);
                return Ok(());
            }
            continue;
        }

        if matches!(meta.mode, RunMode::RunToMain) && state.in_user_code {
            meta.mode = RunMode::Step;
        }

        loop {
            terminal.draw(|frame| {
                let status_bar = StatusBar { meta: &meta, program_finished: false };
                render(&mut panes, frame, &display_state, &status_bar)
            })?;

            match event::handle(&mut panes, &mut display_state, &state, &mut meta, command_tx)? {
                Action::Continue => (),
                Action::Break => break,
                Action::Return => return Ok(()),
            }
        }
    }

    // Program is done; keep the final snapshot visible until the user explicitly quits.
    if let Some(state) = meta.last_state.take() {
        finished(terminal, panes, state, meta, command_tx)
    } else {
        finished_without_snapshot(terminal)
    }
}

fn finished(
    terminal: &mut Terminal,
    mut panes: Panes,
    state: DebuggerState,
    mut meta: Meta,
    command_tx: &CommandSender,
) -> Result<(), io::Error> {
    meta.mode = RunMode::Step;
    meta.reverse_index = None;
    panes.stack.search.editing = false;
    let mut display_state = state.clone();
    loop {
        terminal.draw(|frame| {
            let status_bar = StatusBar { meta: &meta, program_finished: true };
            render(&mut panes, frame, &display_state, &status_bar)
        })?;

        match event::handle(&mut panes, &mut display_state, &state, &mut meta, command_tx)? {
            Action::Continue => (),
            Action::Break | Action::Return => return Ok(()),
        }
    }
}

fn finished_without_snapshot(terminal: &mut Terminal) -> Result<(), io::Error> {
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

fn render(
    panes: &mut Panes,
    frame: &mut Frame<'_>,
    state: &DebuggerState,
    status_bar: &StatusBar<'_>,
) {
    panes.update_area(frame.area());

    panes.render_stack(frame, state, status_bar.meta.blink_epoch);
    panes.render_mir(frame, state);
    panes.render_locals(frame, state);
    panes.render_memory(frame, state);
    panes.render_output(frame, state);
    panes.render_status_bar(frame, state, status_bar);
}

fn state_has_frame(state: &DebuggerState, target: &str) -> bool {
    let target_lc = target.to_ascii_lowercase();
    state.stack_frames.iter().any(|frame| frame.fn_name.to_ascii_lowercase().contains(&target_lc))
}

fn hscroll_text(text: &str, offset: u16) -> String {
    text.chars().skip(usize::from(offset)).collect()
}
