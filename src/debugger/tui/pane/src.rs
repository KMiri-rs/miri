use super::*;

#[derive(Default, Debug)]
pub struct PaneSrc {
    pub rect: Rect,
    pub scroll: u16,
    pub hscroll: u16,
}

impl PaneSrc {
    pub fn new(rect: Rect) -> Self {
        PaneSrc { rect, ..Default::default() }
    }

    pub fn widget(&self, state: &DebuggerState, focus: bool) -> Paragraph<'static> {
        let fpath = state
            .stack_frames
            .first()
            .map(|frame| {
                let file = &frame.source_file;
                let start = state.current_location.line_start;
                let end = state.current_location.line_end;
                if start == end {
                    format!("{file}:{start}")
                } else {
                    format!("{file}:{start}:{end}")
                }
            })
            .unwrap_or_else(|| "<none>".to_string());

        let lines = state.current_location.render_src.clone();

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Source")
                    .title(Line::from(fpath).right_aligned())
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(focus)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, self.hscroll))
    }
}
