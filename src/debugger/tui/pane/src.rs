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

    /// Center highlighted mir in view scope. Should be called prior to widget being rendered.
    pub fn view_centering(&mut self, state: &DebuggerState) {
        let [start_idx, end_idx]: [usize; 2] =
            if let Some([start_idx, end_idx]) = state.current_location.render_src.highlighted_idx {
                [start_idx.into(), end_idx.into()]
            } else {
                // Nothing to be highlighted.
                return;
            };

        let height: usize = self.rect.height.into();

        self.scroll = if end_idx + 2 < height {
            // The highlighted lines fit into current view from first line.
            0
        } else {
            // Pan the view to the first highlighted line.
            let gap = height.checked_sub(1 + end_idx - start_idx).unwrap_or(height) / 2;
            start_idx.saturating_sub(gap)
        }
        .try_into()
        .unwrap();
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
