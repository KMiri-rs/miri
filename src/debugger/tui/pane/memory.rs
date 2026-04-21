use std::borrow::Cow;

use super::*;
use crate::debugger::state::LocalKind;

#[derive(Default, Debug)]
pub struct PaneMemory {
    pub rect: Rect,
    pub scroll: u16,
    pub hscroll: u16,
}

impl PaneMemory {
    pub fn new(rect: Rect) -> Self {
        PaneMemory { rect, ..Default::default() }
    }

    pub fn widget(&self, state: &DebuggerState, focus: bool) -> Table<'static> {
        let rows: Vec<_> = state
            .alloc
            .iter()
            .map(|alloc| {
                let alive = alloc.alive;
                Row::new([
                    right_cell_with_alive(format!("{}", alloc.alloc_id.0), alive),
                    right_cell_with_alive(format!("0x{:x}", alloc.base_addr), alive),
                    right_cell_with_alive(format!("{alive:?}"), alive),
                    right_cell_with_alive(format!("{:?}", alloc.kind), alive),
                    right_cell_with_alive(hsize(alloc.size), alive),
                    right_cell_with_alive(hsize(alloc.align), alive),
                    right_cell_with_alive(format!("{}", alloc.provenance_exposed), alive),
                    right_cell_with_alive(alloc.locals.join(","), alive),
                ])
            })
            .collect();

        let header =
            ["AllocID", "BaseAddr", "Alive", "Kind", "Size", "Align", "ProvExposed", "Locals"];
        let widths = {
            let widths = [10u16, 15, 8, 12, 10, 10, 12, 0];
            let sum: u16 = widths.iter().sum();
            let mut widths = widths.map(|w| Constraint::Percentage(w * 80 / sum));
            *widths.last_mut().unwrap() = Constraint::Fill(1);
            widths
        };
        Table::new(rows, widths)
            .header(
                Row::new(header.map(right_cell))
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .block(
                Block::default()
                    .title("Allocations")
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(focus)),
            )
    }
}

fn right_cell_with_alive(s: impl Into<Cow<'static, str>>, alive: bool) -> Cell<'static> {
    let mut text = Text::from(s.into()).right_aligned();
    if !alive {
        text = text.fg(THEME_DIM);
    }
    Cell::from(text)
}

fn right_cell(s: impl Into<Cow<'static, str>>) -> Cell<'static> {
    Cell::from(Text::from(s.into()).right_aligned())
}

fn hsize(n: impl humansize::ToF64 + humansize::Unsigned) -> String {
    humansize::format_size(n, humansize::BINARY)
}
