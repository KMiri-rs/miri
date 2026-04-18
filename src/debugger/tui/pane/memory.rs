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

    pub fn widget(&self, state: &DebuggerState, focus: bool) -> List<'static> {
        let items: Vec<ListItem<'_>> = state
            .memory
            .iter()
            .skip(self.scroll.into())
            .map(|mem| {
                let live = mem.detail.contains("live") || mem.name.contains("ptr");
                let blocks = if live { "■■■■■■" } else { "□□□□□□" };
                let block_style = if live {
                    Style::default().fg(THEME_ACCENT)
                } else {
                    Style::default().fg(THEME_DIM)
                };
                let line = format!(
                    "{}  {} => {}",
                    blocks,
                    hscroll_text(&mem.name, self.hscroll),
                    hscroll_text(&mem.detail, self.hscroll)
                );
                ListItem::new(Line::from(line).style(block_style))
            })
            .collect();

        List::new(items).block(
            Block::default()
                .title("Memory")
                .borders(Borders::ALL)
                .border_style(pane_border_style(focus)),
        )
    }
}
