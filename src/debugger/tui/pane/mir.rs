use super::*;
use crate::debugger::state::LocalKind;
use crate::helpers::ToUsize;

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

    pub fn widget(&self, state: &DebuggerState, focus: bool) -> Paragraph<'static> {
        let mut lines =
            Vec::with_capacity(1 + state.cfg_lines.len() + state.current_location.render_mir.len());

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
        let len = state.current_location.render_mir.len();
        lines.extend(state.current_location.render_mir.iter().enumerate().map(|(idx, mir)| {
            let line = Line::from(hscroll_text(mir, self.hscroll));
            if idx == state.current_location.render_mir_highlighted_idx.to_usize() {
                line.style(if idx + 1 == len { STYLE_TERMINATOR } else { STYLE_HIGHTLIGHTED })
            } else {
                line
            }
        }));

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("MIR")
                    .borders(Borders::ALL)
                    .border_style(pane_border_style(focus)),
            )
            .scroll((self.scroll, self.hscroll))
    }
}
