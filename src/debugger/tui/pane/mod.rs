use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::DebuggerState;
use crate::debugger::tui::theme::*;

pub mod panes;

pub mod locals;
pub mod memory;
pub mod mir;
pub mod output;
pub mod src;
pub mod stack;
pub mod status_bar;

fn pane_border_style(focus: bool) -> Style {
    if focus {
        Style::default().fg(THEME_ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(THEME_DIM)
    }
}

fn hscroll_text(text: &str, offset: u16) -> String {
    text.chars().skip(usize::from(offset)).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusPane {
    Mir,
    Stack,
    Src,
    Locals,
    Memory,
    Output,
    StatusBar,
}

impl FocusPane {
    pub fn next(self) -> Self {
        match self {
            FocusPane::Mir => FocusPane::Stack,
            FocusPane::Stack => FocusPane::Src,
            FocusPane::Src => FocusPane::Locals,
            FocusPane::Locals => FocusPane::Memory,
            FocusPane::Memory => FocusPane::Output,
            FocusPane::Output => FocusPane::Mir,
            FocusPane::StatusBar => unreachable!(),
        }
    }

    pub fn previous(self) -> Self {
        match self {
            FocusPane::Mir => FocusPane::Output,
            FocusPane::Stack => FocusPane::Mir,
            FocusPane::Src => FocusPane::Stack,
            FocusPane::Locals => FocusPane::Src,
            FocusPane::Memory => FocusPane::Locals,
            FocusPane::Output => FocusPane::Memory,
            FocusPane::StatusBar => unreachable!(),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FocusPane::Mir => "mir",
            FocusPane::Stack => "stack",
            FocusPane::Src => "src",
            FocusPane::Locals => "locals",
            FocusPane::Memory => "memory",
            FocusPane::Output => "output",
            FocusPane::StatusBar => "status_bar",
        }
    }
}
