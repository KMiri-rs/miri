use std::borrow::Cow;
use std::time::Instant;

use super::*;

#[derive(Default, Debug)]
pub struct StackSearchState {
    pub query: String,
    pub editing: bool,
    pub matches: Vec<usize>,
    pub current_match: usize,
}

impl StackSearchState {
    fn visible_stack_indices(&self, state: &DebuggerState) -> Vec<usize> {
        if self.query.is_empty() {
            (0..state.stack_frames.len()).collect()
        } else {
            self.matches.clone()
        }
    }
}

#[derive(Default, Debug)]
pub struct PaneStack {
    pub rect: Rect,
    pub index: usize,
    pub hscroll: u16,
    pub search: StackSearchState,
}

impl PaneStack {
    pub fn new(rect: Rect) -> Self {
        Self { rect, ..Default::default() }
    }

    pub fn widget(
        &self,
        state: &DebuggerState,
        focus: bool,
        blink_epoch: Instant,
    ) -> (List<'static>, ListState) {
        const CURSOR_BLINK_MS: u128 = 500;

        let search = &self.search;
        let search_cursor_visible = search.editing
            && (blink_epoch.elapsed().as_millis() / CURSOR_BLINK_MS).is_multiple_of(2);
        let search_display: Cow<'_, str> = if self.search.editing {
            if search_cursor_visible {
                format!("{}|", search.query).into()
            } else {
                search.query.as_str().into()
            }
        } else {
            search.query.as_str().into()
        };

        let title = if search.editing && search.query.is_empty() {
            format!("Stack (thread {}) search:{}", state.current_thread.to_u32(), search_display)
        } else if search.query.is_empty() {
            format!("Stack (thread {})", state.current_thread.to_u32())
        } else {
            format!(
                "Stack (thread {}) search:{} [{}{}]",
                state.current_thread.to_u32(),
                search_display,
                search.matches.len(),
                if search.editing { ", editing" } else { "" }
            )
        };

        let visible = search.visible_stack_indices(state);
        let items: Vec<_> = visible
            .iter()
            .map(|&idx| {
                let info = &state.stack_frames[idx];
                let is_match = search.matches.contains(&idx);
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
                .style(if is_match {
                    Style::default().fg(THEME_WARN)
                } else {
                    Style::default()
                })
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
        if !visible.is_empty() {
            let selected = visible.iter().position(|idx| *idx == self.index).unwrap_or(0);
            list_state.select(Some(selected));
        }

        (list, list_state)
    }

    pub fn selected_stack_fn_name(&self, state: &DebuggerState) -> Option<String> {
        let visible = self.search.visible_stack_indices(state);
        if visible.is_empty() {
            return None;
        }
        let idx = if visible.contains(&self.index) { self.index } else { visible[0] };
        state.stack_frames.get(idx).map(|f| f.fn_name.clone())
    }

    pub fn refresh(&mut self, state: &DebuggerState) {
        let search = &mut self.search;
        if search.query.is_empty() {
            search.matches.clear();
            search.current_match = 0;
            return;
        }

        let query = search.query.to_ascii_lowercase();
        let mut hay = String::with_capacity(128);
        search.matches = state
            .stack_frames
            .iter()
            .enumerate()
            .filter_map(|(idx, frame)| {
                hay.clear();
                hay += &frame.fn_name.to_ascii_lowercase();
                hay.push(' ');
                hay += &frame.source_file.to_ascii_lowercase();
                if hay.contains(&query) { Some(idx) } else { None }
            })
            .collect();

        if search.matches.is_empty() {
            search.current_match = 0;
            return;
        }

        if search.current_match >= search.matches.len() {
            search.current_match = 0;
        }
        self.index = search.matches[search.current_match];
    }

    pub fn goto_next_search_match(&mut self) {
        let search = &mut self.search;
        if search.matches.is_empty() {
            return;
        }
        search.current_match = (search.current_match + 1) % search.matches.len();
        self.index = search.matches[search.current_match];
    }

    pub fn goto_prev_search_match(&mut self) {
        let search = &mut self.search;
        if search.matches.is_empty() {
            return;
        }
        if search.current_match == 0 {
            search.current_match = search.matches.len() - 1;
        } else {
            search.current_match -= 1;
        }
        self.index = search.matches[search.current_match];
    }

    pub fn step_stack_selection(&mut self, state: &DebuggerState, forward: bool) {
        let visible = self.search.visible_stack_indices(state);
        if visible.is_empty() {
            return;
        }

        let current_pos = visible.iter().position(|idx| *idx == self.index).unwrap_or(0);
        let next_pos = if forward {
            (current_pos + 1).min(visible.len() - 1)
        } else {
            current_pos.saturating_sub(1)
        };
        self.index = visible[next_pos];
    }
}
