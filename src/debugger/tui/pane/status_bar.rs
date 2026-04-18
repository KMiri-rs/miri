use super::*;
use crate::debugger::state::LocalKind;
use crate::debugger::tui::pane::stack::StackSearchState;
use crate::debugger::tui::{RunMode, RunTargetState};

const HISTORY_CAPACITY: usize = 1000;

pub struct StatusBar<'a> {
    pub run_target: &'a RunTargetState,
    pub program_finished: bool,
    pub reverse_mode: bool,
    pub mode: RunMode,
    pub history_len: usize,
}

#[derive(Default, Debug)]
pub struct PaneStatusBar {
    pub rect: Rect,
    pub hscroll: u16,
}

impl PaneStatusBar {
    pub fn new(rect: Rect) -> Self {
        PaneStatusBar { rect, ..Default::default() }
    }

    pub fn widget(
        &self,
        state: &DebuggerState,
        focus_name: &str,
        search: &StackSearchState,
        status: &StatusBar<'_>,
    ) -> Paragraph<'static> {
        let search_text = if search.editing && search.query.is_empty() {
            "search=editing".to_string()
        } else if search.editing {
            format!("search=/{}, matches={} (editing)", search.query, search.matches.len())
        } else if search.query.is_empty() {
            "search=off".to_string()
        } else {
            format!("search=/{}, matches={}", search.query, search.matches.len())
        };
        let keys_text = if status.run_target.editing {
            "keys: type function name  enter run-to-frame  esc cancel  backspace delete"
        } else if search.editing {
            "keys: type to filter stack  enter/esc// exit search  backspace delete  [ ] scroll-cmds  q quit"
        } else if status.program_finished {
            "keys: q quit  / search  . next  , prev  b step-back  [ ] scroll-cmds  esc clear  tab switch  arrows scroll"
        } else {
            "keys: n/space step  b step-back  p run-to-selected  P run-to-name  c continue  m run-to-main  e run-to-end  / search  . next  , prev  [ ] scroll-cmds  q quit  tab switch  arrows scroll"
        };
        let finished_text = if status.program_finished { "  status=finished" } else { "" };
        let mode_text = if status.reverse_mode { "reverse" } else { status.mode.as_str() };
        let target_text = if status.run_target.editing {
            format!("  target={}|", status.run_target.query)
        } else {
            String::new()
        };
        let text = format!(
            "mode={}  steps={}  thread={}  focus={}  history={}/{}  {}{}{}  {}",
            mode_text,
            state.step_count,
            state.current_thread.to_u32(),
            focus_name,
            status.history_len,
            HISTORY_CAPACITY,
            search_text,
            finished_text,
            target_text,
            keys_text,
        );

        Paragraph::new(text)
            .style(Style::default().fg(THEME_BG).bg(THEME_ACCENT).add_modifier(Modifier::BOLD))
            .scroll((0, self.hscroll))
    }
}
