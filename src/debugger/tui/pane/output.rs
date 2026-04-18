use super::*;
use crate::debugger::state::LocalKind;

#[derive(Default, Debug)]
pub struct PaneOutput {
    pub rect: Rect,
    pub scroll: u16,
    pub hscroll: u16,
}

impl PaneOutput {
    pub fn new(rect: Rect) -> Self {
        PaneOutput { rect, ..Default::default() }
    }

    pub fn widget(&self, state: &DebuggerState, focus: bool) -> List<'static> {
        let items: Vec<ListItem<'_>> = state
            .output
            .iter()
            .skip(self.scroll.into())
            .flat_map(|entry| {
                entry.text.lines().map(move |line| {
                    let style = if entry.is_stderr {
                        Style::default().fg(THEME_ERR)
                    } else {
                        Style::default().fg(THEME_ACCENT_SOFT)
                    };
                    ListItem::new(Line::from(hscroll_text(line, self.hscroll)).style(style))
                })
            })
            .collect();

        List::new(items).block(
            Block::default()
                .title("Output")
                .borders(Borders::ALL)
                .border_style(pane_border_style(focus)),
        )
    }
}
