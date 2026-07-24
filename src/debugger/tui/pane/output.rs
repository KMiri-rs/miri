use super::*;

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
        let mut lines = Vec::<Line<'static>>::new();
        let mut still_last = true;
        for output in &state.output {
            if output.text.is_empty() {
                continue;
            }

            const STYLE_NORMAL: Style = Style::new().fg(THEME_ACCENT_SOFT);
            const STYLE_ERR: Style = Style::new().fg(THEME_ACCENT_SOFT);
            let style = if output.is_stderr { STYLE_ERR } else { STYLE_NORMAL };

            let output_lines: Vec<_> = output.text.lines().collect();
            // content contains multiple lines
            let mut iter = output_lines.iter();

            // push the previous line; content belongs to the previous line
            if still_last && let Some(last) = lines.last_mut() {
                let span = Span::styled(hscroll_text(iter.next().unwrap(), self.hscroll), style);
                last.push_span(span);
            }
            // push lines
            lines.extend(iter.map(|line| Line::styled(hscroll_text(line, self.hscroll), style)));
            // determine if the following outputs still belongs to the last line
            still_last = !output.text.ends_with("\n");
        }

        let items = lines.into_iter().skip(self.scroll.into()).map(ListItem::new);
        List::new(items).block(
            Block::default()
                .title("Output")
                .borders(Borders::ALL)
                .border_style(pane_border_style(focus)),
        )
    }
}
