use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::DebuggerState;
use crate::debugger::tui::hscroll_text;
use crate::debugger::tui::theme::*;

pub mod panes;

pub mod locals;
pub mod memory;
pub mod mir;
pub mod output;
pub mod stack;
pub mod status_bar;

fn pane_border_style(focus: bool) -> Style {
    if focus {
        Style::default().fg(THEME_ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(THEME_DIM)
    }
}
