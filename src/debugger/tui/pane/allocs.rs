use std::borrow::Cow;

use super::*;
use crate::debugger::state::LocalKind;

#[derive(Default, Debug)]
pub struct PaneAllocs {
    pub rect: Rect,
    pub scroll: u16,
    pub hscroll: u16,
}

impl PaneAllocs {
    pub fn new(rect: Rect) -> Self {
        PaneAllocs { rect, ..Default::default() }
    }

    pub fn widget(&self, state: &DebuggerState, focus: bool) -> Table<'static> {
        let rows: Vec<_> = state
            .allocs
            .iter()
            .map(|alloc| {
                let alive = !alloc.dealloc;
                Row::new([
                    right_cell_with_alive(format!("{}", alloc.alloc_id.0), alive),
                    right_cell_with_alive(
                        alloc.base_addr.map(|addr| format!("0x{addr:x}")).unwrap_or_default(),
                        alive,
                    ),
                    right_cell_with_alive(if alloc.dealloc { "yes" } else { "" }, alive),
                    right_cell_with_alive(
                        alloc.kind.map(|k| format!("{k:?}")).unwrap_or_default(),
                        alive,
                    ),
                    right_cell_with_alive(alloc.size.map(hsize).unwrap_or_default(), alive),
                    right_cell_with_alive(alloc.align.map(hsize).unwrap_or_default(), alive),
                    right_cell_with_alive(if alloc.provenance_exposed { "yes" } else { "" }, alive),
                    right_cell_with_alive(alloc.locals.join(","), alive),
                ])
            })
            .collect();

        let header =
            ["AllocID", "BaseAddr", "Dealloc", "Kind", "Size", "Align", "ProvExposed", "Locals"];
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
