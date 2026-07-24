use super::*;

#[derive(Default, Debug)]
pub struct PaneMir {
    pub rect: Rect,
    pub scroll: u16,
    pub hscroll: u16,
}

impl PaneMir {
    pub fn new(rect: Rect) -> Self {
        PaneMir { rect, ..Default::default() }
    }

    /// Center highlighted mir in view scope. Should be called prior to widget being rendered.
    pub fn view_centering(&mut self, state: &DebuggerState) {
        let height: usize = self.rect.height.into();
        let mir_idx: usize = state.current_location.render_mir_highlighted_idx.into();
        let rendered_lines_before_mir = Self::rendered_lines_before_mir(state);

        self.scroll =
            (rendered_lines_before_mir + mir_idx).saturating_sub(height / 2).try_into().unwrap();
    }

    fn rendered_lines_before_mir(state: &DebuggerState) -> usize {
        state.cfg_lines.len() + 1
    }

    /// The exact number of lines to render the widget.
    fn rendered_lines(state: &DebuggerState) -> usize {
        Self::rendered_lines_before_mir(state) + state.current_location.render_mir.len()
    }

    pub fn widget(&self, state: &DebuggerState, focus: bool) -> Paragraph<'static> {
        let rendered_lines = Self::rendered_lines(state);
        let mut lines = Vec::with_capacity(rendered_lines);

        lines.extend(state.cfg_lines.iter().map(|line| {
            let mut diagram = format!("bb{}", line.block);
            if line.successors.is_empty() {
                diagram.push_str(" ─┤ END");
            } else {
                let succs = line
                    .successors
                    .iter()
                    .map(|s| format!("bb{s}"))
                    .collect::<Vec<_>>()
                    .join(" │ ");
                diagram.push_str(" ─┬─> ");
                diagram.push_str(&succs);
            }

            if line.is_current {
                Line::from(hscroll_text(&format!("▣ {}", diagram), self.hscroll))
                    .style(Style::default().fg(THEME_OK).add_modifier(Modifier::BOLD))
            } else {
                Line::from(hscroll_text(&format!("□ {}", diagram), self.hscroll))
                    .style(Style::default().fg(THEME_DIM))
            }
        }));

        lines.push(Line::default());
        assert_eq!(lines.len(), Self::rendered_lines_before_mir(state));

        let len = state.current_location.render_mir.len();
        let highlighted_idx: usize = state.current_location.render_mir_highlighted_idx.into();
        lines.extend(state.current_location.render_mir.iter().enumerate().map(|(idx, mir)| {
            let line = Line::from(hscroll_text(&format!("[{:2}] {mir}", idx + 1), self.hscroll));
            if idx == highlighted_idx {
                line.style(if idx + 1 == len { STYLE_TERMINATOR } else { STYLE_HIGHTLIGHTED })
            } else {
                line
            }
        }));

        assert_eq!(lines.len(), rendered_lines);
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(format!("MIR (len={})", state.current_location.render_mir.len()))
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(focus)),
            )
            .scroll((self.scroll, self.hscroll))
    }
}
