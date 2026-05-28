use std::borrow::Cow;
use std::time::Duration;

use tui_overlay::{Backdrop, Easing, Overlay, OverlayState};

use super::*;
use crate::borrow_tracker::stacked_borrows::debugger::DebuggerBorrowStacks;
use crate::debugger::state::LocalKind;
use crate::{BorrowTrackerMethod, MemoryKind, MiriMemoryKind};

#[derive(Default, Debug)]
pub struct PaneBorrowStacks {
    pub scroll: u16,
    // pub hscroll: u16,
    pub modal: Box<Option<Modal>>,
}

impl PaneBorrowStacks {
    /// This is a lightly different with Default, because Modal is initialized.
    pub fn new() -> Self {
        PaneBorrowStacks { modal: Box::new(Some(Modal::new())), ..Default::default() }
    }

    pub fn modal(&mut self) -> &mut Modal {
        (*self.modal).as_mut().unwrap()
    }

    pub fn widget(&self, state: &DebuggerState, no_dead: bool) -> Table<'static> {
        let len_alive = state.allocs.iter().filter(|alloc| !alloc.dealloc).count();
        let rows = state
            .allocs
            .iter()
            .skip(self.scroll.into())
            // no_dead=true: don't display allocations
            // !no_dead=true: display dead allocations
            .filter(|alloc| !(no_dead & alloc.dealloc))
            .flat_map(|alloc| alloc.borrow_stacks.to_table_rows());

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
            .block(
                Block::default()
                    .title(format!("{method} (total={}, alive={len_alive})", state.allocs.len()))
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(true)),
            )
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
