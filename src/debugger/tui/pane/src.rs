use super::*;
use crate::debugger::utils::src_view_centering;

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

    /// Center highlighted mir in view scope. Should be called prior to widget being rendered.
    pub fn view_centering(&mut self, state: &DebuggerState) {
        let Some(highlighted_idx) = state.current_location.render_src.highlighted_idx else {
            return;
        };
        self.scroll = src_view_centering(highlighted_idx, self.rect.height);
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

        // Don't enable wrapping, because it confuses the gap in view_centering:
        // wrapping increases the rendered lines, and the idx of start and end becomes wrong.
        Paragraph::new(state.current_location.render_src.lines.clone())
            .block(
                Block::default()
                    .title("Source")
                    .title(Line::from(fpath).right_aligned())
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(focus)),
            )
            .scroll((self.scroll, self.hscroll))
    }
}
