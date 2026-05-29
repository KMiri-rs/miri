use std::borrow::Cow;

use rustc_data_structures::fx::FxHashMap;
use tui_overlay::{Backdrop, Easing, Overlay, OverlayState};

use super::*;
use crate::BorrowTrackerMethod;
use crate::borrow_tracker::stacked_borrows::debugger::{DebuggerBorrowStacks, DebuggerSpan};
use crate::debugger::utils::{InverseIdx, src_view_centering};

#[derive(Default, Debug)]
pub struct PaneBorrowStacks {
    pub rect: Rect,
    pub state: TableState,
    pub view_height: u16,
    pub modal: Box<Option<Modal>>,
    pub inverse_idx: FxHashMap<usize, InverseIdx>,
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

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.view_height = rect.height;
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.rect.contains(Position { x, y })
    }

    pub fn select_at(&mut self, y: u16) {
        let Some(relative_y) = y.checked_sub(self.rect.y + 2) else {
            return;
        };
        let row = self.state.offset().saturating_add(relative_y as usize);
        self.state.select(Some(row));
    }

    pub fn widget(&mut self, state: &DebuggerState, no_dead: bool) -> Table<'static> {
        let len_alive = state.allocs.iter().filter(|alloc| !alloc.dealloc).count();
        self.inverse_idx.clear();
        let mut rows_idx = 0;
        let rows: Vec<_> = state
            .allocs
            .iter()
            .enumerate()
            // no_dead=true: don't display allocations
            // !no_dead=true: display dead allocations
            .filter_map(|(idx_alloc, alloc)| {
                (!(no_dead & alloc.dealloc)).then_some((idx_alloc, alloc))
            })
            .flat_map(|(idx_alloc, alloc)| {
                let mut idx = InverseIdx::default();
                idx.alloc = idx_alloc;
                alloc.borrow_stacks.to_table_rows(alloc, &mut idx, |iidx| {
                    self.inverse_idx.insert(rows_idx, iidx);
                    rows_idx += 1;
                })
            })
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
        self.view_height.saturating_sub(3).max(1).into()
    }

    pub fn find_selected_span(
        &self,
        state: &DebuggerState,
        height: u16,
    ) -> Option<Paragraph<'static>> {
        if let Some(row_idx) = self.state.selected() {
            if let Some(idx) = self.inverse_idx.get(&row_idx) {
                if let Some(alloc) = state.allocs.get(idx.alloc) {
                    if let Some(stack) = alloc.borrow_stacks.segments.get(idx.bs_segment) {
                        if let Some(item) = stack.stack.get(idx.bs_stack) {
                            if let Some(span) = alloc.borrow_stacks.span.get(&item.bor_tag_id) {
                                let highlighted_idx = [
                                    span.highlighted_line_start - span.body_line_start,
                                    span.highlighted_line_end - span.body_line_start,
                                ];
                                let para = Paragraph::new(span.src.lines.clone())
                                    .block(
                                        Block::default()
                                            .title(span.title())
                                            .borders(Borders::ALL)
                                            .border_style(pane_border_style(true)),
                                    )
                                    .scroll((src_view_centering(highlighted_idx, height), 0));

                                return Some(para);
                            }
                        }
                    }
                }
            }
        }
        None
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
                .width(Constraint::Percentage(90))
                .height(Constraint::Percentage(90)),
            state: OverlayState::new(),
        }
    }
}
