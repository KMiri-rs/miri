use std::borrow::Cow;

use tui_overlay::{Backdrop, Easing, Overlay, OverlayState};

use super::*;
use crate::borrow_tracker::stacked_borrows::debugger::DebuggerBorrowStacks;
use crate::BorrowTrackerMethod;

#[derive(Default, Debug)]
pub struct PaneBorrowStacks {
    pub state: TableState,
    pub view_height: u16,
    pub modal: Box<Option<Modal>>,
}

impl PaneBorrowStacks {
    /// This is a lightly different with Default, because Modal is initialized.
    pub fn new() -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        PaneBorrowStacks { state, modal: Box::new(Some(Modal::new())), ..Default::default() }
    }

    pub fn modal(&mut self) -> &mut Modal {
        (*self.modal).as_mut().unwrap()
    }

    pub fn widget(&self, state: &DebuggerState, no_dead: bool) -> Table<'static> {
        let len_alive = state.allocs.iter().filter(|alloc| !alloc.dealloc).count();
        let rows: Vec<_> = state
            .allocs
            .iter()
            // no_dead=true: don't display allocations
            // !no_dead=true: display dead allocations
            .filter(|alloc| !(no_dead & alloc.dealloc))
            .flat_map(|alloc| alloc.borrow_stacks.to_table_rows())
            .collect();

        let method: Cow<'_, _> = match state.borrow_tracker_method {
            Some(BorrowTrackerMethod::StackedBorrows) => "Stacked Borrows".into(),
            Some(BorrowTrackerMethod::TreeBorrows(params)) =>
                format!(
                    "Tree Borrows{}",
                    if params.precise_interior_mut { "" } else { " - not precise_interior_mut" }
                )
                .into(),
            None => "Unknown Borrows".into(),
        };
        let (header, widths) = DebuggerBorrowStacks::header();
        Table::new(rows, widths)
            .header(header.style(Style::default().add_modifier(Modifier::BOLD)))
            .highlight_symbol(">> ")
            .highlight_spacing(HighlightSpacing::WhenSelected)
            .row_highlight_style(STYLE_HIGHTLIGHTED_BG)
            .block(
                Block::default()
                    .title(format!("{method} (total={}, alive={len_alive})", state.allocs.len()))
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(true)),
            )
    }

    /// Move the selction to the previous row.
    pub fn navigate_up(&mut self) {
        if let Some(select) = self.state.selected() {
            self.state.select(Some(select.saturating_sub(1)));
        } else {
            self.state.select(Some(0));
        }
    }

    /// Move the selction to the next row.
    pub fn navigate_down(&mut self) {
        let next = self.state.selected().map_or(0, |select| select.saturating_add(1));
        self.state.select(Some(next));
    }

    /// Move one page up in the table.
    pub fn scroll_up(&mut self) {
        let page = self.page_size();
        let next = self.state.selected().map_or(0, |select| select.saturating_sub(page));
        self.state.select(Some(next));
    }

    /// Move one page down in the table.
    pub fn scroll_down(&mut self) {
        let page = self.page_size();
        let next = self.state.selected().map_or(0, |select| select.saturating_add(page));
        self.state.select(Some(next));
    }

    fn page_size(&self) -> usize {
        self.view_height.saturating_sub(1).max(1).into()
    }
}

#[derive(Debug)]
pub struct Modal {
    pub overlay: Overlay<'static>,
    pub state: OverlayState,
}

impl Modal {
    fn new() -> Modal {
        Modal {
            overlay: Overlay::new()
                .backdrop(Backdrop::new(Color::Rgb(11, 14, 27)))
                .width(Constraint::Percentage(80))
                .height(Constraint::Percentage(80)),
            state: OverlayState::new(),
        }
    }
}
