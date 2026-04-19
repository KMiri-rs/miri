use ratatui::style::{Color, Modifier, Style};

pub const THEME_ACCENT: Color = Color::Cyan;
pub const THEME_ACCENT_SOFT: Color = Color::LightCyan;
pub const THEME_DIM: Color = Color::DarkGray;
pub const THEME_BG: Color = Color::Black;
pub const THEME_OK: Color = Color::Green;
pub const THEME_WARN: Color = Color::Yellow;
pub const THEME_ERR: Color = Color::Red;

pub const STYLE_HIGHTLIGHTED: Style = Style::new().add_modifier(Modifier::BOLD).fg(Color::Cyan);
pub const STYLE_TERMINATOR: Style = Style::new().add_modifier(Modifier::BOLD).fg(Color::Yellow);
