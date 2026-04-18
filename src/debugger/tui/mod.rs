#![deny(dead_code)]
use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Wrap,
};
use ratatui::{Frame, Terminal};

use super::channel::{CommandSender, StateReceiver};
use super::state::LocalKind;
use super::{DebuggerCommand, DebuggerState};
use crate::debugger::tui::pane::panes::Panes;
use crate::debugger::tui::pane::status_bar::StatusBar;

mod pane;
mod theme;

const EVENT_POLL_MS: u64 = 100;
const HISTORY_CAPACITY: usize = 1000;

#[derive(Clone, Copy, Debug)]
enum FocusPane {
    Stack,
    Mir,
    Locals,
    Memory,
    Output,
}

impl FocusPane {
    fn next(self) -> Self {
        match self {
            FocusPane::Stack => FocusPane::Mir,
            FocusPane::Mir => FocusPane::Locals,
            FocusPane::Locals => FocusPane::Memory,
            FocusPane::Memory => FocusPane::Output,
            FocusPane::Output => FocusPane::Stack,
        }
    }

    fn previous(self) -> Self {
        match self {
            FocusPane::Stack => FocusPane::Output,
            FocusPane::Mir => FocusPane::Stack,
            FocusPane::Locals => FocusPane::Mir,
            FocusPane::Memory => FocusPane::Locals,
            FocusPane::Output => FocusPane::Memory,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            FocusPane::Stack => "stack",
            FocusPane::Mir => "mir",
            FocusPane::Locals => "locals",
            FocusPane::Memory => "memory",
            FocusPane::Output => "output",
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

fn tui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state_rx: StateReceiver,
    command_tx: CommandSender,
) -> io::Result<()> {
    let mut mode = RunMode::Step;
    let mut panes = Panes::new(Rect::default());
    let mut run_target = RunTargetState::default();
    let mut run_to_frame_target: Option<String> = None;
    let mut last_state: Option<DebuggerState> = None;
    let mut history: VecDeque<DebuggerState> = VecDeque::with_capacity(HISTORY_CAPACITY);
    let mut blink_epoch = Instant::now();

    while let Ok(state) = state_rx.recv() {
        history.push_back(state.clone());
        if history.len() > HISTORY_CAPACITY {
            history.pop_front();
        }

        last_state = Some(state.clone());
        panes.stack.refresh(&state);
        if !state.stack_frames.is_empty() {
            panes.stack.index = panes.stack.index.min(state.stack_frames.len() - 1);
        } else {
            panes.stack.index = 0;
        }

        let mut display_state = state.clone();
        let mut reverse_index: Option<usize> = None;

        if matches!(mode, RunMode::RunToFrame)
            && run_to_frame_target.as_ref().is_some_and(|target| state_has_frame(&state, target))
        {
            mode = RunMode::Step;
            run_to_frame_target = None;
        }

        // In fast-forward mode, keep rendering every step without waiting for input.
        if (matches!(mode, RunMode::RunToMain) && !state.in_user_code)
            || matches!(mode, RunMode::RunToFrame)
            || matches!(mode, RunMode::RunToEnd)
        {
            terminal.draw(|frame| {
                let status_bar = StatusBar {
                    run_target: &run_target,
                    program_finished: false,
                    reverse_mode: false,
                    mode,
                    history_len: history.len(),
                };
                render(&mut panes, frame, &display_state, blink_epoch, &status_bar)
            })?;

            // Still allow immediate quit while fast-forwarding.
            if event::poll(Duration::from_millis(0))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
                && key.code == KeyCode::Char('q')
            {
                let _ = command_tx.send(DebuggerCommand::Quit);
                return Ok(());
            }

            continue;
        }

        if matches!(mode, RunMode::RunToMain) && state.in_user_code {
            mode = RunMode::Step;
        }

        loop {
            terminal.draw(|frame| {
                let status_bar = StatusBar {
                    run_target: &run_target,
                    program_finished: false,
                    reverse_mode: reverse_index.is_some(),
                    mode,
                    history_len: history.len(),
                };
                render(&mut panes, frame, &display_state, blink_epoch, &status_bar)
            })?;

            if !event::poll(Duration::from_millis(EVENT_POLL_MS))? {
                continue;
            }
            let ev = event::read()?;
            if let Event::Key(key) = ev {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if run_target.editing {
                    match key.code {
                        KeyCode::Esc => {
                            run_target.editing = false;
                        }
                        KeyCode::Enter => {
                            let target = run_target.query.trim().to_string();
                            run_target.editing = false;
                            if !target.is_empty() {
                                reverse_index = None;
                                run_to_frame_target = Some(target.clone());
                                mode = RunMode::RunToFrame;
                                let _ = command_tx.send(DebuggerCommand::RunToFrame(target));
                                break;
                            }
                        }
                        KeyCode::Backspace => {
                            run_target.query.pop();
                        }
                        KeyCode::Char(c) => {
                            run_target.query.push(c);
                        }
                        _ => {}
                    }
                    continue;
                }
                if panes.stack.search.editing {
                    panes.edit(&display_state, key.code);
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => {
                        let _ = command_tx.send(DebuggerCommand::Quit);
                        return Ok(());
                    }
                    KeyCode::Char('/') => {
                        panes.focus = FocusPane::Stack;
                        panes.stack.search.editing = true;
                    }
                    KeyCode::Char('P') => {
                        run_target.editing = true;
                        run_target.query.clear();
                    }
                    KeyCode::Char('p') => {
                        if let Some(target) = panes.stack.selected_stack_fn_name(&display_state) {
                            reverse_index = None;
                            run_to_frame_target = Some(target.clone());
                            mode = RunMode::RunToFrame;
                            let _ = command_tx.send(DebuggerCommand::RunToFrame(target));
                            break;
                        } else {
                            run_target.editing = true;
                            run_target.query.clear();
                        }
                    }
                    KeyCode::Char('.') => panes.stack.goto_next_search_match(),
                    KeyCode::Char(',') => panes.stack.goto_prev_search_match(),
                    KeyCode::Char('[') => {
                        panes.status_bar.hscroll = panes.status_bar.hscroll.saturating_sub(1);
                    }
                    KeyCode::Char(']') => {
                        panes.status_bar.hscroll = panes.status_bar.hscroll.saturating_add(1);
                    }
                    KeyCode::Esc =>
                        if !panes.stack.search.query.is_empty() {
                            panes.stack.search = Default::default();
                        },
                    KeyCode::Char('n') | KeyCode::Char(' ') => {
                        if let Some(idx) = reverse_index {
                            if idx + 1 < history.len() {
                                let next = idx + 1;
                                if let Some(snapshot) = history.get(next) {
                                    display_state = snapshot.clone();
                                    reverse_index =
                                        if next + 1 == history.len() { None } else { Some(next) };
                                    panes.stack.refresh(&display_state);
                                }
                                continue;
                            }
                            reverse_index = None;
                            display_state = state.clone();
                            panes.stack.refresh(&display_state);
                        }
                        mode = RunMode::Step;
                        let _ = command_tx.send(DebuggerCommand::StepOver);
                        break;
                    }
                    KeyCode::Char('b') => {
                        mode = RunMode::Step;
                        let next_index = match reverse_index {
                            Some(idx) => idx.saturating_sub(1),
                            None => history.len().saturating_sub(2),
                        };
                        if let Some(snapshot) = history.get(next_index) {
                            reverse_index = Some(next_index);
                            display_state = snapshot.clone();
                            panes.stack.refresh(&display_state);
                        }
                    }
                    KeyCode::Char('c') => {
                        reverse_index = None;
                        run_to_frame_target = None;
                        mode = RunMode::Continue;
                        let _ = command_tx.send(DebuggerCommand::Continue);
                        break;
                    }
                    KeyCode::Char('e') => {
                        reverse_index = None;
                        run_to_frame_target = None;
                        mode = RunMode::RunToEnd;
                        let _ = command_tx.send(DebuggerCommand::RunToEnd);
                        break;
                    }
                    KeyCode::Char('m') => {
                        reverse_index = None;
                        run_to_frame_target = None;
                        mode = RunMode::RunToMain;
                        let _ = command_tx.send(DebuggerCommand::RunToMain);
                        break;
                    }
                    KeyCode::BackTab => panes.focus = panes.focus.previous(),
                    KeyCode::Tab => {
                        panes.focus = if key.modifiers == KeyModifiers::SHIFT {
                            panes.focus.previous()
                        } else {
                            panes.focus.next()
                        };
                    }
                    KeyCode::Up => panes.navigate_up(&display_state),
                    KeyCode::Down => panes.navigate_down(&display_state),
                    KeyCode::Left => panes.scroll_left(panes.focus),
                    KeyCode::Right => panes.scroll_right(panes.focus),
                    KeyCode::Char(c)
                        if c.is_ascii_alphanumeric()
                            || c == '_'
                            || c == ':'
                            || c == '<'
                            || c == '>' =>
                    {
                        run_target.editing = true;
                        run_target.query.clear();
                        run_target.query.push(c);
                    }
                    _ => {}
                }
            } else if let Event::Mouse(mouse) = ev {
                let size = terminal.size()?;
                if mouse.row == size.height.saturating_sub(1) {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            panes.status_bar.hscroll = panes.status_bar.hscroll.saturating_sub(1);
                        }
                        MouseEventKind::ScrollDown => {
                            panes.status_bar.hscroll = panes.status_bar.hscroll.saturating_add(1);
                        }
                        _ => {}
                    }
                    continue;
                }
                let area = Panes::new(Rect::new(0, 0, size.width, size.height));
                let hovered = area.pane_at(mouse.column, mouse.row);
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        panes.focus = hovered;
                        panes.scroll_up();
                    }
                    MouseEventKind::ScrollDown => {
                        panes.focus = hovered;
                        panes.scroll_down(&display_state);
                    }
                    MouseEventKind::Down(_) => panes.focus = hovered,
                    _ => {}
                }
            }
        }
    }

    // Program is done; keep the final snapshot visible until the user explicitly quits.
    if let Some(state) = last_state {
        mode = RunMode::Step;
        panes.stack.search.editing = false;
        let mut display_state = state.clone();
        let mut reverse_index: Option<usize> = None;
        loop {
            terminal.draw(|frame| {
                let status_bar = StatusBar {
                    run_target: &run_target,
                    program_finished: true,
                    reverse_mode: reverse_index.is_some(),
                    mode,
                    history_len: todo!(),
                };
                render(&mut panes, frame, &display_state, blink_epoch, &status_bar)
            })?;
            if !event::poll(Duration::from_millis(EVENT_POLL_MS))? {
                continue;
            }
            let ev = event::read()?;
            if let Event::Key(key) = ev {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if panes.stack.search.editing {
                    panes.edit(&display_state, key.code);
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('b') => {
                        let next_index = match reverse_index {
                            Some(idx) => idx.saturating_sub(1),
                            None => history.len().saturating_sub(2),
                        };
                        if let Some(snapshot) = history.get(next_index) {
                            reverse_index = Some(next_index);
                            display_state = snapshot.clone();
                            panes.stack.refresh(&display_state);
                        }
                    }
                    KeyCode::Char('n') | KeyCode::Char(' ') =>
                        if let Some(idx) = reverse_index {
                            if idx + 1 < history.len() {
                                let next = idx + 1;
                                if let Some(snapshot) = history.get(next) {
                                    display_state = snapshot.clone();
                                    reverse_index =
                                        if next + 1 == history.len() { None } else { Some(next) };
                                    panes.stack.refresh(&display_state);
                                }
                            }
                        },
                    KeyCode::Char('/') => {
                        panes.focus = FocusPane::Stack;
                        panes.stack.search.editing = true;
                    }
                    KeyCode::Char('.') => panes.stack.goto_next_search_match(),
                    KeyCode::Char(',') => panes.stack.goto_prev_search_match(),
                    KeyCode::Char('[') => {
                        panes.status_bar.hscroll = panes.status_bar.hscroll.saturating_sub(1);
                    }
                    KeyCode::Char(']') => {
                        panes.status_bar.hscroll = panes.status_bar.hscroll.saturating_add(1);
                    }
                    KeyCode::Esc =>
                        if !panes.stack.search.query.is_empty() {
                            panes.stack.search = Default::default();
                        },
                    KeyCode::Tab => panes.focus = panes.focus.next(),
                    KeyCode::Up => panes.navigate_up(&display_state),
                    KeyCode::Down => panes.navigate_down(&display_state),
                    KeyCode::Left => panes.scroll_left(panes.focus),
                    KeyCode::Right => panes.scroll_right(panes.focus),
                    KeyCode::Char(c)
                        if c.is_ascii_alphanumeric()
                            || c == '_'
                            || c == ':'
                            || c == '<'
                            || c == '>' =>
                    {
                        run_target.editing = true;
                        run_target.query.clear();
                        run_target.query.push(c);
                    }
                    _ => {}
                }
            } else if let Event::Mouse(mouse) = ev {
                let size = terminal.size()?;
                if mouse.row == size.height.saturating_sub(1) {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            panes.status_bar.hscroll = panes.status_bar.hscroll.saturating_sub(1);
                        }
                        MouseEventKind::ScrollDown => {
                            panes.status_bar.hscroll = panes.status_bar.hscroll.saturating_add(1);
                        }
                        _ => {}
                    }
                    continue;
                }
                let area = Panes::new(Rect::new(0, 0, size.width, size.height));
                let hovered = area.pane_at(mouse.column, mouse.row);
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        panes.focus = hovered;
                        panes.scroll_up();
                    }
                    MouseEventKind::ScrollDown => {
                        panes.focus = hovered;
                        panes.scroll_down(&display_state);
                    }
                    _ => {}
                }
            }
        }
    } else {
        // No snapshot was received before the interpreter terminated. Keep a minimal
        // end screen open so users can still quit explicitly.
        let text =
            Paragraph::new("Program finished before first debugger snapshot. Press q to close.")
                .block(Block::default().title("Miri Debugger").borders(Borders::ALL))
                .wrap(Wrap { trim: true });

        loop {
            terminal.draw(|frame| frame.render_widget(text.clone(), frame.area()))?;
            let ev = event::read()?;
            if let Event::Key(key) = ev
                && key.kind == KeyEventKind::Press
                && key.code == KeyCode::Char('q')
            {
                return Ok(());
            }
        }
    }

    Ok(())
}

fn render(
    panes: &mut Panes,
    frame: &mut Frame<'_>,
    state: &DebuggerState,
    blink_epoch: Instant,
    status_bar: &StatusBar<'_>,
) {
    panes.update_area(frame.area());

    panes.render_stack(frame, state, blink_epoch);
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
