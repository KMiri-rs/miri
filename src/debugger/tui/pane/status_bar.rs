use super::*;
use crate::debugger::get_record_all_states;
use crate::debugger::tui::Context;
use crate::debugger::tui::pane::instances::StackSearchState;

const HISTORY_CAPACITY: usize = 1000;

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
        instance_search: &StackSearchState,
        ctx: &Context,
    ) -> Paragraph<'static> {
        let search_text = search_text("instances", instance_search);
        let keys_text = if instance_search.editing {
            "keys: type to filter instances  up/down select  pageup/pagedown page  enter run-to-instance  esc exit search  backspace delete  F toggle-freeze  q quit"
        } else if ctx.program_finished {
            "keys: q quit  / search  . next  , prev  b step-back  [ ] scroll-cmds  F toggle-freeze  esc clear  tab switch  arrows scroll"
        } else if focus_name == "instances" {
            "keys: enter run-to-instance  / search  ? search-clear  . next  , prev  b step-back  [ ] scroll-cmds  F toggle-freeze  q quit  tab switch  arrows scroll"
        } else {
            "keys: n/space step  b step-back  / search  ? search-clear  c continue  e run-to-end  . next  , prev  [ ] scroll-cmds F toggle-freeze  q quit  tab switch  arrows scroll"
        };
        let finished_text = if ctx.program_finished { "  status=finished" } else { "" };
        let reverse_mode = ctx.reverse_index.is_some();
        let mode_text = if reverse_mode { "reverse" } else { ctx.mode.as_str() };
        let run_target_text = ctx
            .run_to_instance_target
            .as_ref()
            .map(|target| format!("  instance={target}"))
            .unwrap_or_default();
        let record_all_state =
            if get_record_all_states() { " S record_always " } else { " S record_on_demand " };
        let text = format!(
            "mode={}  steps={}  thread={}  focus={}  history={}/{} {record_all_state} {}{}{}  {}",
            mode_text,
            state.step_count,
            state.current_thread.to_u32(),
            focus_name,
            ctx.history.len(),
            HISTORY_CAPACITY,
            search_text,
            finished_text,
            run_target_text,
            keys_text,
        );

        Paragraph::new(text)
            .style(Style::default().fg(THEME_BG).bg(THEME_ACCENT).add_modifier(Modifier::BOLD))
            .scroll((0, self.hscroll))
    }
}

fn search_text(label: &str, search: &StackSearchState) -> String {
    if search.editing && search.query.is_empty() {
        format!("{label}=editing")
    } else if search.editing {
        format!("{label}=/{}, matches={} (editing)", search.query, search.matches.len())
    } else if search.query.is_empty() {
        format!("{label}=off")
    } else {
        format!("{label}=/{}, matches={}", search.query, search.matches.len())
    }
}
