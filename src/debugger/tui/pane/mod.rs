use ratatui::prelude::*;
use ratatui::widgets::*;

use crate::DebuggerState;
use crate::debugger::tui::theme::*;

pub mod panes;

pub mod allocs;
pub mod borrow_stacks;
pub mod instances;
pub mod locals;
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
    Allocs,
    Output,
    Instances,
    StatusBar,
    BorrowStacks,
}

impl FocusPane {
    pub fn next(self) -> Self {
        match self {
            FocusPane::Mir => FocusPane::Stack,
            FocusPane::Stack => FocusPane::Src,
            FocusPane::Src => FocusPane::Locals,
            FocusPane::Locals => FocusPane::Allocs,
            FocusPane::Allocs => FocusPane::Output,
            FocusPane::Output => FocusPane::Instances,
            FocusPane::Instances => FocusPane::Mir,
            FocusPane::StatusBar => unreachable!(),
            FocusPane::BorrowStacks => FocusPane::BorrowStacks,
        }
    }

    pub fn previous(self) -> Self {
        match self {
            FocusPane::Mir => FocusPane::Instances,
            FocusPane::Stack => FocusPane::Mir,
            FocusPane::Src => FocusPane::Stack,
            FocusPane::Locals => FocusPane::Src,
            FocusPane::Allocs => FocusPane::Locals,
            FocusPane::Output => FocusPane::Allocs,
            FocusPane::Instances => FocusPane::Output,
            FocusPane::StatusBar => unreachable!(),
            FocusPane::BorrowStacks => FocusPane::BorrowStacks,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FocusPane::Mir => "mir",
            FocusPane::Stack => "stack",
            FocusPane::Src => "src",
            FocusPane::Locals => "locals",
            FocusPane::Allocs => "allocs",
            FocusPane::Output => "output",
            FocusPane::Instances => "instances",
            FocusPane::StatusBar => "status_bar",
            FocusPane::BorrowStacks => "borrow_stacks",
        }
    }
}
