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
        let source_file = state
            .stack_frames
            .last()
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
        let mut lines = vec![Line::from(source_file), Line::from("")];

        lines.extend_from_slice(&state.current_location.render);

        lines.push(Line::from(""));
        lines.push(
            Line::from("CFG:")
                .style(Style::default().fg(THEME_ACCENT).add_modifier(Modifier::BOLD)),
        );
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

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Source")
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(focus)),
            )
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, self.hscroll))
    }
}
