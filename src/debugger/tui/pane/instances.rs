use std::borrow::Cow;
use std::time::Instant;

use super::*;

#[derive(Default, Debug)]
pub struct StackSearchState {
    pub query: String,
    pub editing: bool,
    pub matches: Vec<usize>,
    pub current_match: usize,
    history: Vec<String>,
    history_cursor: Option<usize>,
    history_draft: Option<String>,
}

impl StackSearchState {
    fn clear_history_navigation(&mut self) {
        self.history_cursor = None;
        self.history_draft = None;
    }

    pub fn record_current_query(&mut self) {
        self.clear_history_navigation();
        if self.query.is_empty() {
            return;
        }
        if self.history.last().is_some_and(|prev| prev == &self.query) {
            return;
        }
        self.history.push(self.query.clone());
    }

    pub fn begin_history_search(&mut self, clear_query: bool) {
        self.record_current_query();
        if clear_query {
            self.query.clear();
        }
        self.editing = true;
    }

    pub fn finish_history_search(&mut self) {
        self.record_current_query();
        self.editing = false;
    }

    pub fn previous_history_entry(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.history_cursor {
            None => {
                self.history_draft = Some(self.query.clone());
                self.history_cursor = Some(self.history.len() - 1);
                self.query = self.history[self.history.len() - 1].clone();
            }
            Some(0) => {
                self.query = self.history[0].clone();
            }
            Some(cursor) => {
                self.history_cursor = Some(cursor - 1);
                self.query = self.history[cursor - 1].clone();
            }
        }
    }

    pub fn next_history_entry(&mut self) {
        let Some(cursor) = self.history_cursor else {
            return;
        };

        if cursor + 1 < self.history.len() {
            self.history_cursor = Some(cursor + 1);
            self.query = self.history[cursor + 1].clone();
            return;
        }

        self.history_cursor = None;
        if let Some(draft) = self.history_draft.take() {
            self.query = draft;
        }
    }

    pub fn type_char(&mut self, c: char) {
        self.clear_history_navigation();
        self.query.push(c);
    }

    pub fn backspace(&mut self) {
        self.clear_history_navigation();
        self.query.pop();
    }
}

#[derive(Default, Debug)]
pub struct PaneInstances {
    pub rect: Rect,
    pub index: usize,
    pub hscroll: u16,
    pub search: StackSearchState,
}

impl PaneInstances {
    pub fn new(rect: Rect) -> Self {
        Self { rect, ..Default::default() }
    }

    fn visible_instance_indices(&self, state: &DebuggerState) -> Vec<usize> {
        if self.search.query.is_empty() {
            (0..state.function_instances.len()).collect()
        } else {
            self.search.matches.clone()
        }
    }

    fn move_selection(&mut self, state: &DebuggerState, delta: isize) {
        let visible = self.visible_instance_indices(state);
        if visible.is_empty() {
            return;
        }

        let current_pos = visible.iter().position(|idx| *idx == self.index).unwrap_or(0);
        let max_pos = visible.len().saturating_sub(1);
        let next_pos = if delta >= 0 {
            current_pos.saturating_add(delta as usize).min(max_pos)
        } else {
            current_pos.saturating_sub(delta.unsigned_abs())
        };
        self.index = visible[next_pos];

        if !self.search.query.is_empty() {
            self.search.current_match = next_pos;
        }
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

        let visible = self.visible_instance_indices(state);
        let items: Vec<_> = visible
            .iter()
            .map(|&idx| {
                let info = &state.function_instances[idx];
                let is_match = search.matches.contains(&idx);
                let first = hscroll_text(&format!("#{idx} {}", info.instance), self.hscroll);
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

        let len = items.len();
        let title = if search.editing && search.query.is_empty() {
            format!("Instances search: `{}` [{len}]", search_display)
        } else if search.query.is_empty() {
            "Instances".to_string()
        } else {
            format!(
                "Instances search: `{}` [{}{}]",
                search_display,
                search.matches.len(),
                if search.editing { ", editing" } else { "" }
            )
        };

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

    pub fn selected_instance_target(&self, state: &DebuggerState) -> Option<String> {
        let visible = self.visible_instance_indices(state);
        if visible.is_empty() {
            return None;
        }
        let idx = if visible.contains(&self.index) { self.index } else { visible[0] };
        state.function_instances.get(idx).map(|f| f.instance.clone())
    }

    pub fn refresh(&mut self, state: &DebuggerState) {
        let search = &mut self.search;
        if search.query.is_empty() {
            search.matches.clear();
            search.current_match = 0;
            if self.index >= state.function_instances.len() {
                self.index = state.function_instances.len().saturating_sub(1);
            }
            return;
        }

        let query = search.query.to_ascii_lowercase();
        let mut hay = String::with_capacity(128);
        search.matches = state
            .function_instances
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                hay.clear();
                hay += &item.instance.to_ascii_lowercase();
                hay.push(' ');
                hay += &item.source_file.to_ascii_lowercase();
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

    pub fn step_selection(&mut self, state: &DebuggerState, forward: bool) {
        self.move_selection(state, if forward { 1 } else { -1 });
    }

    pub fn page_selection(&mut self, state: &DebuggerState, forward: bool) {
        // Approximate each result as two rows. Wrapping long names or paths can make the jump
        // land a bit early or late, but this keeps paging simple and predictable.
        let page = usize::from(self.rect.height.saturating_sub(2) / 2).max(1);
        let delta = if forward {
            isize::try_from(page).unwrap_or(isize::MAX)
        } else {
            -isize::try_from(page).unwrap_or(isize::MAX)
        };
        self.move_selection(state, delta);
        if !self.search.query.is_empty() {
            let visible = self.visible_instance_indices(state);
            if let Some(pos) = visible.iter().position(|idx| *idx == self.index) {
                self.search.current_match = pos;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StackSearchState;

    #[test]
    fn records_only_consecutive_duplicates() {
        let mut search = StackSearchState::default();

        search.query = "alpha".to_string();
        search.record_current_query();
        search.record_current_query();

        search.query = "beta".to_string();
        search.record_current_query();

        search.query = "alpha".to_string();
        search.record_current_query();

        assert_eq!(search.history, vec!["alpha", "beta", "alpha"]);
    }

    #[test]
    fn history_navigation_restores_draft() {
        let mut search = StackSearchState::default();
        search.history = vec!["first".to_string(), "second".to_string()];
        search.query = "draft".to_string();

        search.previous_history_entry();
        assert_eq!(search.query, "second");

        search.previous_history_entry();
        assert_eq!(search.query, "first");

        search.next_history_entry();
        assert_eq!(search.query, "second");

        search.next_history_entry();
        assert_eq!(search.query, "draft");
    }
}
