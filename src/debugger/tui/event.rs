use std::time::{Duration, Instant};
use std::{io, mem};

use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};

use crate::debugger::channel::CommandSender;
use crate::debugger::tui::pane::FocusPane;
use crate::debugger::tui::pane::panes::Panes;
use crate::debugger::tui::{Context, RunMode};
use crate::{DebuggerCommand, DebuggerState};

/// An action for an event result in a loop.
pub enum Action {
    Continue,
    Break,
    Return,
}

pub fn handle(
    ev: Event,
    panes: &mut Panes,
    state: &DebuggerState,
    ctx: &mut Context,
    command_tx: &CommandSender,
) -> io::Result<Action> {
    let Context {
        mode,
        run_target,
        run_to_frame_target,
        last_state,
        history,
        blink_epoch,
        reverse_index,
        count,
        ..
    } = ctx;

    if let Event::Key(key) = ev {
        if key.kind != KeyEventKind::Press {
            return Ok(Action::Continue);
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
                        *reverse_index = None;
                        *run_to_frame_target = Some(target.clone());
                        *mode = RunMode::RunToFrame;
                        let _ = command_tx.send(DebuggerCommand::RunToFrame(target));
                        return Ok(Action::Break);
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
            return Ok(Action::Continue);
        }
        if panes.stack.search.editing {
            panes.edit(state, key.code);
            return Ok(Action::Continue);
        }
        match key.code {
            KeyCode::Char('q') => {
                let _ = command_tx.send(DebuggerCommand::Quit);
                return Ok(Action::Return);
            }
            KeyCode::Char('/') => {
                panes.focus = FocusPane::Stack;
                panes.stack.search.editing = true;
            }
            KeyCode::Char('P') => {
                run_target.editing = true;
                run_target.query.clear();
            }
            KeyCode::Char('p') =>
                if let Some(target) = panes.stack.selected_stack_fn_name(state) {
                    *reverse_index = None;
                    *run_to_frame_target = Some(target.clone());
                    *mode = RunMode::RunToFrame;
                    let _ = command_tx.send(DebuggerCommand::RunToFrame(target));
                    return Ok(Action::Break);
                } else {
                    run_target.editing = true;
                    run_target.query.clear();
                },
            KeyCode::Char('.') => panes.stack.goto_next_search_match(),
            KeyCode::Char(',') => panes.stack.goto_prev_search_match(),
            KeyCode::Char('F') => panes.freeze ^= true,
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
                if let Some(idx) = *reverse_index {
                    let next = idx + 1;
                    if let Some(snapshot) = history.get(next) {
                        *last_state = Some(Box::new(snapshot.clone()));
                        panes.stack.refresh(snapshot);
                        *reverse_index = Some(next);
                        return Ok(Action::Continue);
                    }
                    *reverse_index = None;
                }
                *mode = RunMode::Step;
                let n = mem::take(count).parse().unwrap_or(0);
                let _ = command_tx.send(DebuggerCommand::StepOver(n));
                return Ok(Action::Break);
            }
            KeyCode::Char('b') => {
                *mode = RunMode::Step;
                let next_index = match reverse_index {
                    Some(idx) => idx.saturating_sub(1),
                    None => history.len().saturating_sub(2),
                };
                if let Some(snapshot) = history.get(next_index) {
                    *reverse_index = Some(next_index);
                    *last_state = Some(Box::new(snapshot.clone()));
                    panes.stack.refresh(snapshot);
                }
            }
            KeyCode::Char('c') => {
                *reverse_index = None;
                *run_to_frame_target = None;
                *mode = RunMode::Continue;
                let _ = command_tx.send(DebuggerCommand::Continue);
                return Ok(Action::Break);
            }
            KeyCode::Char('e') => {
                *reverse_index = None;
                *run_to_frame_target = None;
                *mode = RunMode::RunToEnd;
                let _ = command_tx.send(DebuggerCommand::RunToEnd);
                return Ok(Action::Break);
            }
            KeyCode::Char('m') => {
                *reverse_index = None;
                *run_to_frame_target = None;
                *mode = RunMode::RunToMain;
                let _ = command_tx.send(DebuggerCommand::RunToMain);
                return Ok(Action::Break);
            }
            KeyCode::Char('t') => {
                *reverse_index = None;
                *run_to_frame_target = None;
                *mode = RunMode::RunToTerminator;
                let n = mem::take(count).parse().unwrap_or(0);
                let _ = command_tx.send(DebuggerCommand::RunToTerminator(n));
                return Ok(Action::Break);
            }
            KeyCode::BackTab => panes.focus = panes.focus.previous(),
            KeyCode::Tab => {
                panes.focus = if key.modifiers == KeyModifiers::SHIFT {
                    panes.focus.previous()
                } else {
                    panes.focus.next()
                };
            }
            KeyCode::Up => panes.navigate_up(state),
            KeyCode::Down => panes.navigate_down(state),
            KeyCode::Left => panes.scroll_left(),
            KeyCode::Right => panes.scroll_right(),
            KeyCode::Char(c) if c.is_ascii_digit() => count.push(c),
            _ => {}
        }
    } else if let Event::Mouse(mouse) = ev {
        on_event_mouse(panes, state, mouse);
    }

    Ok(Action::Continue)
}

fn on_event_mouse(panes: &mut Panes, state: &DebuggerState, mouse: MouseEvent) {
    let hovered = panes.pane_at(mouse.column, mouse.row);
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            panes.focus = hovered;
            panes.scroll_up();
        }
        MouseEventKind::ScrollDown => {
            panes.focus = hovered;
            panes.scroll_down(state);
        }
        MouseEventKind::Down(_) => panes.focus = hovered,
        _ => {}
    }
}

pub fn fast_quit() -> io::Result<bool> {
    // Still allow immediate quit while fast-forwarding.
    if event::poll(Duration::from_millis(0))?
        && let Event::Key(key) = event::read()?
        && key.kind == KeyEventKind::Press
        && key.code == KeyCode::Char('q')
    {
        return Ok(true);
    }
    Ok(false)
}
