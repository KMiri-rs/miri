use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use ratatui::prelude::*;
use tui_overlay::{Backdrop, Easing, Overlay, OverlayState};

use crate::DebuggerState;
use crate::debugger::tui::pane::FocusPane;
use crate::debugger::tui::pane::allocs::PaneAllocs;
use crate::debugger::tui::pane::borrow_stacks::PaneBorrowStacks;
use crate::debugger::tui::pane::locals::PaneLocals;
use crate::debugger::tui::pane::mir::PaneMir;
use crate::debugger::tui::pane::output::PaneOutput;
use crate::debugger::tui::pane::src::PaneSrc;
use crate::debugger::tui::pane::stack::PaneStack;
use crate::debugger::tui::pane::status_bar::PaneStatusBar;
use crate::debugger::tui::{Context, RunTargetState};

#[derive(Debug)]
pub struct Panes {
    /// The terminal area to render stuff. When the program starts, the area is zero, but later becomes real area.
    pub area: Rect,
    /// Currently focused pane (including modal).
    pub focus: FocusPane,
    /// Previous focused pane, usually used for modal toggle, meaning main pane switches are not recorded.
    pub prev_focus: FocusPane,
    pub mir: PaneMir,
    pub stack: PaneStack,
    pub src: PaneSrc,
    pub locals: PaneLocals,
    pub allocs: PaneAllocs,
    pub output: PaneOutput,
    pub status_bar: PaneStatusBar,
    pub borrow_stacks: PaneBorrowStacks,
    /// The default value is true to disable manual scrolling for panes (like mir and src)
    /// where the contents are preferred to centering.
    pub freeze: bool,
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
            area,
            focus: FocusPane::Mir,
            prev_focus: FocusPane::Mir,
            mir: PaneMir::new(mir),
            stack: PaneStack::new(stack),
            src: PaneSrc::new(src),
            locals: PaneLocals::new(locals),
            allocs: PaneAllocs::new(memory),
            output: PaneOutput::new(output),
            status_bar: PaneStatusBar::new(status_bar),
            borrow_stacks: PaneBorrowStacks::new(),
            freeze: true,
        }
    }

    pub fn update_area(&mut self, area: Rect) {
        let new_layout = Self::new(area);
        self.area = area;
        self.mir.rect = new_layout.mir.rect;
        self.stack.rect = new_layout.stack.rect;
        self.src.rect = new_layout.src.rect;
        self.locals.rect = new_layout.locals.rect;
        self.allocs.rect = new_layout.allocs.rect;
        self.output.rect = new_layout.output.rect;
        self.status_bar.rect = new_layout.status_bar.rect;
    }

    /// Find the pane as per the given point.
    pub fn pane_at(&self, x: u16, y: u16) -> FocusPane {
        if self.is_focused(FocusPane::BorrowStacks) {
            return FocusPane::BorrowStacks;
        }

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
        } else if is_in(self.allocs.rect) {
            FocusPane::Allocs
        } else if is_in(self.output.rect) {
            FocusPane::Output
        } else {
            FocusPane::StatusBar
        }
    }

    pub fn toggle_modal(&mut self) {
        if self.is_focused(FocusPane::BorrowStacks) {
            // Back up main pane.
            self.focus = self.prev_focus;
            self.prev_focus = FocusPane::BorrowStacks;
        } else {
            // Switch to BorrowStacks pane.
            self.prev_focus = self.focus;
            self.focus = FocusPane::BorrowStacks;
        }
    }

    pub fn is_focused(&self, pane: FocusPane) -> bool {
        self.focus == pane
    }

    pub fn render_mir(&mut self, frame: &mut Frame<'_>, state: &DebuggerState) {
        if self.freeze {
            self.mir.view_centering(state);
        }
        let paragraph = self.mir.widget(state, self.is_focused(FocusPane::Mir));
        frame.render_widget(paragraph, self.mir.rect);
    }

    pub fn render_stack(&self, frame: &mut Frame<'_>, state: &DebuggerState, blink_epoch: Instant) {
        let (list, mut list_state) =
            self.stack.widget(state, self.is_focused(FocusPane::Stack), blink_epoch);
        frame.render_stateful_widget(list, self.stack.rect, &mut list_state);
    }

    pub fn render_src(&mut self, frame: &mut Frame<'_>, state: &DebuggerState) {
        if self.freeze {
            self.src.view_centering(state);
        }
        let paragraph = self.src.widget(state, self.is_focused(FocusPane::Src));
        frame.render_widget(paragraph, self.src.rect);
    }

    pub fn render_locals(&self, frame: &mut Frame<'_>, state: &DebuggerState) {
        let table = self.locals.widget(state, self.is_focused(FocusPane::Locals), self.stack.index);
        frame.render_widget(table, self.locals.rect);
    }

    pub fn render_memory(&self, frame: &mut Frame<'_>, state: &DebuggerState, no_dead: bool) {
        let list = self.allocs.widget(state, self.is_focused(FocusPane::Allocs), no_dead);
        frame.render_widget(list, self.allocs.rect);
    }

    pub fn render_output(&self, frame: &mut Frame<'_>, state: &DebuggerState) {
        let list = self.output.widget(state, self.is_focused(FocusPane::Output));
        frame.render_widget(list, self.output.rect);
    }

    pub fn render_status_bar(&self, frame: &mut Frame<'_>, state: &DebuggerState, ctx: &Context) {
        let paragraph = self.status_bar.widget(state, self.focus.as_str(), &self.stack.search, ctx);
        frame.render_widget(paragraph, self.status_bar.rect);
    }

    /// Render this modal after all main panes are rendered.
    pub fn render_borrow_stack(
        &mut self,
        frame: &mut Frame<'_>,
        state: &DebuggerState,
        no_dead: bool,
    ) {
        let modal = self.borrow_stacks.modal();
        modal.state.open();
        frame.render_stateful_widget(modal.overlay.clone(), self.area, &mut modal.state);
        if let Some(area) = modal.state.inner_area() {
            let [area_borrow_stacks, area_src] = *Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(area)
            else {
                unreachable!()
            };

            self.borrow_stacks.set_rect(area_borrow_stacks);
            let table = self.borrow_stacks.widget(state, no_dead);
            frame.render_stateful_widget(table, area_borrow_stacks, &mut self.borrow_stacks.state);

            if let Some(para) = self.borrow_stacks.find_selected_span(state, area_src.height) {
                frame.render_widget(para, area_src);
            }
        }
    }

    pub fn borrow_stacks_contains(&self, x: u16, y: u16) -> bool {
        self.borrow_stacks.contains(x, y)
    }

    pub fn borrow_stacks_select_at(&mut self, y: u16) {
        self.borrow_stacks.select_at(y);
    }

    fn up(
        &mut self,
        on_stack: impl FnOnce(&mut PaneStack),
        on_borrow_stacks: impl FnOnce(&mut PaneBorrowStacks),
    ) {
        match self.focus {
            FocusPane::Mir => self.mir.scroll = self.mir.scroll.saturating_sub(1),
            FocusPane::Stack => on_stack(&mut self.stack),
            FocusPane::Src => {
                self.src.scroll = self.src.scroll.saturating_sub(1);
            }
            FocusPane::Locals => {
                self.locals.scroll = self.locals.scroll.saturating_sub(1);
            }
            FocusPane::Allocs => {
                self.allocs.scroll = self.allocs.scroll.saturating_sub(1);
            }
            FocusPane::Output => {
                self.output.scroll = self.output.scroll.saturating_sub(1);
            }
            FocusPane::StatusBar => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_sub(1);
            }
            FocusPane::BorrowStacks => on_borrow_stacks(&mut self.borrow_stacks),
        }
    }

    /// This is a slightly different with scroll_up, because stack pane will scroll in the list items,
    /// instead of scroll the view of list.
    pub fn navigate_up(&mut self, state: &DebuggerState) {
        self.up(
            |stack| stack.step_stack_selection(state, false),
            |borrow_statcks| borrow_statcks.navigate_up(),
        );
    }

    pub fn scroll_up(&mut self) {
        self.up(
            |stack| stack.index = stack.index.saturating_sub(1),
            |borrow_stacks| borrow_stacks.scroll_up(),
        );
    }

    fn down(
        &mut self,
        state: &DebuggerState,
        on_stack: impl FnOnce(&mut PaneStack),
        on_borrow_stacks: impl FnOnce(&mut PaneBorrowStacks),
    ) {
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
            FocusPane::Allocs =>
                if !state.allocs.is_empty() {
                    let max = u16::try_from(state.allocs.len()).unwrap() - 1;
                    self.allocs.scroll = self.allocs.scroll.saturating_add(1).min(max);
                },
            FocusPane::Output =>
                if !state.output.is_empty() {
                    let max = u16::try_from(state.output.len()).unwrap() - 1;
                    self.output.scroll = self.output.scroll.saturating_add(1).min(max);
                },
            FocusPane::StatusBar => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_add(1);
            }
            FocusPane::BorrowStacks => on_borrow_stacks(&mut self.borrow_stacks),
        }
    }

    pub fn navigate_down(&mut self, state: &DebuggerState) {
        self.down(
            state,
            |stack| stack.step_stack_selection(state, true),
            |borrow_stacks| borrow_stacks.navigate_down(),
        );
    }

    pub fn scroll_down(&mut self, state: &DebuggerState) {
        self.down(
            state,
            |stack| {
                if !state.stack_frames.is_empty() {
                    let max = state.stack_frames.len() - 1;
                    stack.index = stack.index.saturating_add(1).min(max);
                }
            },
            |borrow_stacks| borrow_stacks.scroll_down(),
        );
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
            FocusPane::Allocs => {
                self.allocs.hscroll = self.allocs.hscroll.saturating_add(1);
            }
            FocusPane::Output => {
                self.output.hscroll = self.output.hscroll.saturating_add(1);
            }
            FocusPane::StatusBar => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_add(1);
            }
            FocusPane::BorrowStacks => {}
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
            FocusPane::Allocs => {
                self.allocs.hscroll = self.allocs.hscroll.saturating_sub(1);
            }
            FocusPane::Output => {
                self.output.hscroll = self.output.hscroll.saturating_sub(1);
            }
            FocusPane::StatusBar => {
                self.status_bar.hscroll = self.status_bar.hscroll.saturating_sub(1);
            }
            FocusPane::BorrowStacks => {}
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
