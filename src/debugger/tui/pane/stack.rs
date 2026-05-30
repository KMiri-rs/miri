use super::*;

#[derive(Default, Debug)]
pub struct PaneStack {
    pub rect: Rect,
    pub index: usize,
    pub hscroll: u16,
}

impl PaneStack {
    pub fn new(rect: Rect) -> Self {
        Self { rect, ..Default::default() }
    }

    pub fn widget(&self, state: &DebuggerState, focus: bool) -> (List<'static>, ListState) {
        let min_sp =
            state.min_stack_ptr.map(|ptr| format!(", min_sp=0x{ptr:x}")).unwrap_or_default();

        let title = format!("Stack (thread {}{min_sp})", state.current_thread.to_u32());

        let items: Vec<_> = state
            .stack_frames
            .iter()
            .enumerate()
            .map(|(idx, info)| {
                let first = hscroll_text(&format!("#{idx} {}", info.fn_name), self.hscroll);
                let src_file = {
                    let file = &info.source_file;
                    let start = info.line_start;
                    let end = info.line_end;
                    if start == end {
                        format!("{file}:{start}")
                    } else {
                        format!("{file}:{start}:{end}")
                    }
                };
                let second = hscroll_text(&src_file, self.hscroll);
                ListItem::new(vec![
                    Line::from(first).style(Style::default().fg(THEME_ACCENT_SOFT)),
                    Line::from(second).style(Style::default().fg(THEME_DIM)),
                ])
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(focus)),
            )
            .highlight_style(
                Style::default().bg(THEME_ACCENT).fg(THEME_BG).add_modifier(Modifier::BOLD),
            );

        let mut list_state = ListState::default();
        if !state.stack_frames.is_empty() {
            let selected = self.index.min(state.stack_frames.len() - 1);
            list_state.select(Some(selected));
        }

        (list, list_state)
    }

    pub fn refresh(&mut self, state: &DebuggerState) {
        if self.index >= state.stack_frames.len() {
            self.index = state.stack_frames.len().saturating_sub(1);
        }
    }

    pub fn step_stack_selection(&mut self, state: &DebuggerState, forward: bool) {
        if state.stack_frames.is_empty() {
            return;
        }

        let current_pos = self.index.min(state.stack_frames.len() - 1);
        let next_pos = if forward {
            (current_pos + 1).min(state.stack_frames.len() - 1)
        } else {
            current_pos.saturating_sub(1)
        };
        self.index = next_pos;
    }
}
