use super::*;
use crate::debugger::state::LocalKind;

#[derive(Default, Debug)]
pub struct PaneLocals {
    pub rect: Rect,
    pub scroll: u16,
    pub hscroll: u16,
}

impl PaneLocals {
    pub fn new(rect: Rect) -> Self {
        PaneLocals { rect, ..Default::default() }
    }

    pub fn widget(
        &self,
        state: &DebuggerState,
        focus: bool,
        stack_index: usize,
        display_dead: bool,
        query: Option<u64>,
    ) -> Table<'static> {
        let selected_locals = state
            .stack_frames
            .get(stack_index)
            .map(|f| f.locals.as_slice())
            .unwrap_or_else(|| state.locals.as_slice());

        let rows = selected_locals
            .iter()
            .filter(|local| {
                // filter in queried or locals alive
                local.queried(query) || (if display_dead { local.value != "-" } else { true })
            })
            .skip(self.scroll.into())
            .map(|local| {
                let value_style = match local.state {
                    LocalKind::Dead => Style::default().fg(THEME_DIM),
                    LocalKind::Uninitialized =>
                        Style::default().fg(THEME_ERR).add_modifier(Modifier::BOLD),
                    LocalKind::Pointer =>
                        Style::default().fg(THEME_WARN).add_modifier(Modifier::BOLD),
                    LocalKind::Initialized => Style::default().fg(THEME_OK),
                };
                let name_style = if local.state == LocalKind::Dead {
                    Style::default().fg(THEME_DIM)
                } else {
                    Style::default().fg(THEME_ACCENT_SOFT)
                };
                Row::new([
                    Cell::from(local.idx.clone()).style(name_style),
                    Cell::from(local.name.clone()).style(name_style),
                    Cell::from(local.ty.clone()).style(Style::default().fg(THEME_DIM)),
                    Cell::from(hscroll_text(&local.value, self.hscroll)).style(value_style),
                ])
            });

        Table::new(
            rows,
            [
                Constraint::Length(5),
                Constraint::Percentage(10),
                Constraint::Percentage(30),
                Constraint::Percentage(55),
            ],
        )
        .header(
            Row::new(["Local", "Name", "Type", "Value"])
                .style(Style::default().fg(THEME_ACCENT).add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .title("Locals")
                .borders(Borders::ALL)
                .border_style(pane_border_style(focus)),
        )
    }
}
