use std::time::Instant;

use crossterm::event::KeyCode;
use ratatui::prelude::*;

use crate::DebuggerState;
use crate::debugger::tui::pane::locals::PaneLocals;
use crate::debugger::tui::pane::memory::PaneMemory;
use crate::debugger::tui::pane::mir::PaneMir;
use crate::debugger::tui::pane::output::PaneOutput;
use crate::debugger::tui::pane::stack::PaneStack;
use crate::debugger::tui::pane::status_bar::{PaneStatusBar, StatusBar};
use crate::debugger::tui::{FocusPane, RunTargetState};

pub struct Meta {
    pub focus: FocusPane,
    pub blink_epoch: Instant,
}

impl Meta {
    pub fn new() -> Self {
        Self { focus: FocusPane::Stack, blink_epoch: Instant::now() }
    }
}

#[derive(Debug)]
pub struct Panes {
    pub stack: PaneStack,
    pub mir: PaneMir,
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

        let [mir, locals, memory, output] = *Layout::default()
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
            stack: PaneStack::new(left),
            mir: PaneMir::new(mir),
            locals: PaneLocals::new(locals),
            memory: PaneMemory::new(memory),
            output: PaneOutput::new(output),
            status_bar: PaneStatusBar::new(status_bar),
        }
    }

    pub fn update_area(&mut self, area: Rect) {
        let new_layout = Self::new(area);
        self.stack.rect = new_layout.stack.rect;
        self.mir.rect = new_layout.mir.rect;
        self.locals.rect = new_layout.locals.rect;
        self.memory.rect = new_layout.memory.rect;
        self.output.rect = new_layout.output.rect;
        self.status_bar.rect = new_layout.status_bar.rect;
    }

    /// Find the pane as per the given point.
    pub fn pane_at(&self, x: u16, y: u16) -> FocusPane {
        // NOTE: x and y point to the left angle of rect, we use neightborhood to determin pane
        // here, so be careful when the neightborhood changes.
        // (We can use width and height to clearly compute the range as an alternative way tho.)
        if x < self.mir.rect.x {
            return FocusPane::Stack;
        }
        if y < self.locals.rect.y {
            FocusPane::Mir
        } else if y < self.memory.rect.y {
            FocusPane::Locals
        } else if y < self.output.rect.y {
            FocusPane::Memory
        } else {
            FocusPane::Output
        }
    }

    pub fn render_stack(
        &self,
        frame: &mut Frame<'_>,
        state: &DebuggerState,
        focus: FocusPane,
        blink_epoch: Instant,
    ) {
        let (list, mut list_state) =
            self.stack.widget(state, matches!(focus, FocusPane::Stack), blink_epoch);
        frame.render_stateful_widget(list, self.stack.rect, &mut list_state);
    }

    pub fn render_mir(&self, frame: &mut Frame<'_>, state: &DebuggerState, focus: FocusPane) {
        let paragraph = self.mir.widget(state, matches!(focus, FocusPane::Mir));
        frame.render_widget(paragraph, self.mir.rect);
    }

    pub fn render_locals(&self, frame: &mut Frame<'_>, state: &DebuggerState, focus: FocusPane) {
        let table = self.locals.widget(state, matches!(focus, FocusPane::Locals), self.stack.index);
        frame.render_widget(table, self.locals.rect);
    }

    pub fn render_memory(&self, frame: &mut Frame<'_>, state: &DebuggerState, focus: FocusPane) {
        let list = self.memory.widget(state, matches!(focus, FocusPane::Memory));
        frame.render_widget(list, self.memory.rect);
    }

    pub fn render_output(&self, frame: &mut Frame<'_>, state: &DebuggerState, focus: FocusPane) {
        let list = self.output.widget(state, matches!(focus, FocusPane::Output));
        frame.render_widget(list, self.output.rect);
    }

    pub fn render_status_bar(
        &self,
        frame: &mut Frame<'_>,
        state: &DebuggerState,
        focus: FocusPane,
        run_target: &RunTargetState,
        status_bar: &StatusBar,
    ) {
        let list = self.status_bar.widget(
            state,
            focus.as_str(),
            &self.stack.search,
            run_target,
            status_bar,
        );
        frame.render_widget(list, self.status_bar.rect);
    }

    fn up(&mut self, pane: FocusPane, on_stack: impl FnOnce(&mut PaneStack)) {
        match pane {
            FocusPane::Stack => on_stack(&mut self.stack),
            FocusPane::Mir => {
                self.mir.scroll = self.mir.scroll.saturating_sub(1);
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
        }
    }

    /// This is a slightly different with scroll_up, because stack pane will scroll in the list items,
    /// instead of scroll the view of list.
    pub fn navigate_up(&mut self, state: &DebuggerState, focus: FocusPane) {
        self.up(focus, |stack| stack.step_stack_selection(state, false));
    }

    pub fn scroll_up(&mut self, focus: FocusPane) {
        self.up(focus, |stack| stack.index = stack.index.saturating_sub(1));
    }

    fn down(
        &mut self,
        state: &DebuggerState,
        pane: FocusPane,
        on_stack: impl FnOnce(&mut PaneStack),
    ) {
        match pane {
            FocusPane::Stack => on_stack(&mut self.stack),
            FocusPane::Mir => {
                self.mir.scroll = self.mir.scroll.saturating_add(1);
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
        }
    }

    pub fn navigate_down(&mut self, state: &DebuggerState, focus: FocusPane) {
        self.up(focus, |stack| stack.step_stack_selection(state, true));
    }

    pub fn scroll_down(&mut self, state: &DebuggerState, pane: FocusPane) {
        self.down(state, pane, |stack| {
            if !state.stack_frames.is_empty() {
                let max = state.stack_frames.len() - 1;
                stack.index = stack.index.saturating_add(1).min(max);
            }
        });
    }

    pub fn scroll_right(&mut self, focus: FocusPane) {
        match focus {
            FocusPane::Stack => {
                self.stack.hscroll = self.stack.hscroll.saturating_add(1);
            }
            FocusPane::Mir => {
                self.mir.hscroll = self.mir.hscroll.saturating_add(1);
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
        }
    }

    pub fn scroll_left(&mut self, focus: FocusPane) {
        match focus {
            FocusPane::Stack => {
                self.stack.hscroll = self.stack.hscroll.saturating_sub(1);
            }
            FocusPane::Mir => {
                self.mir.hscroll = self.mir.hscroll.saturating_sub(1);
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
