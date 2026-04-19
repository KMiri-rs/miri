use std::time::Instant;

use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::DebuggerState;
use crate::debugger::tui::pane::FocusPane;
use crate::debugger::tui::pane::locals::PaneLocals;
use crate::debugger::tui::pane::memory::PaneMemory;
use crate::debugger::tui::pane::mir::PaneMir;
use crate::debugger::tui::pane::output::PaneOutput;
use crate::debugger::tui::pane::src::PaneSrc;
use crate::debugger::tui::pane::stack::PaneStack;
use crate::debugger::tui::pane::status_bar::PaneStatusBar;
use crate::debugger::tui::{Context, RunTargetState};

#[derive(Debug)]
pub struct Panes {
    pub focus: FocusPane,
    pub mir: PaneMir,
    pub stack: PaneStack,
    pub src: PaneSrc,
    pub locals: PaneLocals,
    pub memory: PaneMemory,
    pub output: PaneOutput,
    pub status_bar: PaneStatusBar,
}

impl Panes {
    pub fn new(area: Rect) -> Self {
        let [main, status_bar] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area)
        else {
            unreachable!()
        };

        let [left, right] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(main)
        else {
            unreachable!()
        };

        let [mir, stack] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(left)
        else {
            unreachable!()
        };

        let [src, locals, memory, output] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(24),
                Constraint::Percentage(20),
                Constraint::Percentage(22),
            ])
            .split(right)
        else {
            unreachable!()
        };

        Panes {
            focus: FocusPane::Mir,
            mir: PaneMir::new(mir),
            stack: PaneStack::new(stack),
            src: PaneSrc::new(src),
            locals: PaneLocals::new(locals),
            memory: PaneMemory::new(memory),
            output: PaneOutput::new(output),
            status_bar: PaneStatusBar::new(status_bar),
        }
    }

    pub fn update_area(&mut self, area: Rect) {
        let new_layout = Self::new(area);
        self.mir.rect = new_layout.mir.rect;
        self.stack.rect = new_layout.stack.rect;
        self.src.rect = new_layout.src.rect;
        self.locals.rect = new_layout.locals.rect;
        self.memory.rect = new_layout.memory.rect;
        self.output.rect = new_layout.output.rect;
        self.status_bar.rect = new_layout.status_bar.rect;
    }

    /// Find the pane as per the given point.
    pub fn pane_at(&self, x: u16, y: u16) -> FocusPane {
        let position = Position { x, y };
        let is_in = |rect: Rect| rect.contains(position);
        if is_in(self.mir.rect) {
            FocusPane::Mir
        } else if is_in(self.stack.rect) {
            FocusPane::Stack
        } else if is_in(self.src.rect) {
            FocusPane::Src
        } else if is_in(self.locals.rect) {
            FocusPane::Locals
        } else if is_in(self.memory.rect) {
            FocusPane::Memory
        } else if is_in(self.output.rect) {
            FocusPane::Output
        } else {
            FocusPane::StatusBar
        }
    }

    fn is_focused(&self, pane: FocusPane) -> bool {
        self.focus == pane
    }

    pub fn render_mir(&self, frame: &mut Frame<'_>, state: &DebuggerState) {
        let paragraph = self.mir.widget(state, self.is_focused(FocusPane::Mir));
        frame.render_widget(paragraph, self.mir.rect);
    }

    pub fn render_stack(&self, frame: &mut Frame<'_>, state: &DebuggerState, blink_epoch: Instant) {
        let (list, mut list_state) =
            self.stack.widget(state, self.is_focused(FocusPane::Stack), blink_epoch);
        frame.render_stateful_widget(list, self.stack.rect, &mut list_state);
    }

    pub fn render_src(&self, frame: &mut Frame<'_>, state: &DebuggerState) {
        let paragraph = self.src.widget(state, self.is_focused(FocusPane::Src));
        frame.render_widget(paragraph, self.src.rect);
    }

    pub fn render_locals(&self, frame: &mut Frame<'_>, state: &DebuggerState) {
        let table = self.locals.widget(state, self.is_focused(FocusPane::Locals), self.stack.index);
        frame.render_widget(table, self.locals.rect);
    }

    pub fn render_memory(&self, frame: &mut Frame<'_>, state: &DebuggerState) {
        let list = self.memory.widget(state, self.is_focused(FocusPane::Memory));
        frame.render_widget(list, self.memory.rect);
    }

    pub fn render_output(&self, frame: &mut Frame<'_>, state: &DebuggerState) {
        let list = self.output.widget(state, self.is_focused(FocusPane::Output));
        frame.render_widget(list, self.output.rect);
    }

    pub fn render_status_bar(&self, frame: &mut Frame<'_>, state: &DebuggerState, ctx: &Context) {
        let list = self.status_bar.widget(state, self.focus.as_str(), &self.stack.search, ctx);
        frame.render_widget(list, self.status_bar.rect);
    }

    fn up(&mut self, on_stack: impl FnOnce(&mut PaneStack)) {
        match self.focus {
            FocusPane::Mir => self.mir.scroll = self.mir.scroll.saturating_sub(1),
            FocusPane::Stack => on_stack(&mut self.stack),
            FocusPane::Src => {
                self.src.scroll = self.src.scroll.saturating_sub(1);
            }
            FocusPane::Locals => {
                self.locals.scroll = self.locals.scroll.saturating_sub(1);
            }
            FocusPane::Memory => {
                self.memory.scroll = self.memory.scroll.saturating_sub(1);
            }
            FocusPane::Output => {
                self.output.scroll = self.output.scroll.saturating_sub(1);
            }
            FocusPane::StatusBar => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_sub(1);
            }
        }
    }

    /// This is a slightly different with scroll_up, because stack pane will scroll in the list items,
    /// instead of scroll the view of list.
    pub fn navigate_up(&mut self, state: &DebuggerState) {
        self.up(|stack| stack.step_stack_selection(state, false));
    }

    pub fn scroll_up(&mut self) {
        self.up(|stack| stack.index = stack.index.saturating_sub(1));
    }

    fn down(&mut self, state: &DebuggerState, on_stack: impl FnOnce(&mut PaneStack)) {
        match self.focus {
            FocusPane::Mir => self.mir.scroll = self.mir.scroll.saturating_add(1),
            FocusPane::Stack => on_stack(&mut self.stack),
            FocusPane::Src => {
                self.src.scroll = self.src.scroll.saturating_add(1);
            }
            FocusPane::Locals => {
                let len = state
                    .stack_frames
                    .get(self.stack.index)
                    .map(|f| f.locals.len())
                    .unwrap_or(state.locals.len());
                if len > 0 {
                    let max = u16::try_from(len).unwrap() - 1;
                    self.locals.scroll = self.locals.scroll.saturating_add(1).min(max);
                }
            }
            FocusPane::Memory =>
                if !state.memory.is_empty() {
                    let max = u16::try_from(state.memory.len()).unwrap() - 1;
                    self.memory.scroll = self.memory.scroll.saturating_add(1).min(max);
                },
            FocusPane::Output =>
                if !state.output.is_empty() {
                    let max = u16::try_from(state.output.len()).unwrap() - 1;
                    self.output.scroll = self.output.scroll.saturating_add(1).min(max);
                },
            FocusPane::StatusBar => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_add(1);
            }
        }
    }

    pub fn navigate_down(&mut self, state: &DebuggerState) {
        self.up(|stack| stack.step_stack_selection(state, true));
    }

    pub fn scroll_down(&mut self, state: &DebuggerState) {
        self.down(state, |stack| {
            if !state.stack_frames.is_empty() {
                let max = state.stack_frames.len() - 1;
                stack.index = stack.index.saturating_add(1).min(max);
            }
        });
    }

    pub fn scroll_right(&mut self) {
        match self.focus {
            FocusPane::Mir => {
                self.mir.hscroll = self.mir.hscroll.saturating_add(1);
            }
            FocusPane::Stack => {
                self.stack.hscroll = self.stack.hscroll.saturating_add(1);
            }
            FocusPane::Src => {
                self.src.hscroll = self.src.hscroll.saturating_add(1);
            }
            FocusPane::Locals => {
                self.locals.hscroll = self.locals.hscroll.saturating_add(1);
            }
            FocusPane::Memory => {
                self.memory.hscroll = self.memory.hscroll.saturating_add(1);
            }
            FocusPane::Output => {
                self.output.hscroll = self.output.hscroll.saturating_add(1);
            }
            FocusPane::StatusBar => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_add(1);
            }
        }
    }

    pub fn scroll_left(&mut self) {
        match self.focus {
            FocusPane::Mir => {
                self.mir.hscroll = self.mir.hscroll.saturating_sub(1);
            }
            FocusPane::Stack => {
                self.stack.hscroll = self.stack.hscroll.saturating_sub(1);
            }
            FocusPane::Src => {
                self.src.hscroll = self.src.hscroll.saturating_sub(1);
            }
            FocusPane::Locals => {
                self.locals.hscroll = self.locals.hscroll.saturating_sub(1);
            }
            FocusPane::Memory => {
                self.memory.hscroll = self.memory.hscroll.saturating_sub(1);
            }
            FocusPane::Output => {
                self.output.hscroll = self.output.hscroll.saturating_sub(1);
            }
            FocusPane::StatusBar => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_sub(1);
            }
        }
    }

    pub fn edit(&mut self, state: &DebuggerState, code: KeyCode) {
        match code {
            KeyCode::Char('[') => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_sub(1);
            }
            KeyCode::Char(']') => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_add(1);
            }
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('/') => {
                self.stack.search.editing = false;
            }
            KeyCode::Backspace => {
                self.stack.search.query.pop();
                self.stack.refresh(state);
            }
            KeyCode::Char(c) => {
                self.stack.search.query.push(c);
                self.stack.refresh(state);
            }
            _ => {}
        }
    }
}
