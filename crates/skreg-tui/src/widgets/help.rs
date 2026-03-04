//! Help overlay — keybindings reference shown over any view.

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
    Frame,
};

use crate::theme::Theme;

/// All keybindings shown in the help overlay.
const BINDINGS: &[(&str, &str, &str)] = &[
    // (key, description, views)
    ("j / ↓",       "Move down",              "all lists"),
    ("k / ↑",       "Move up",                "all lists"),
    ("g",           "Jump to top",            "all lists"),
    ("G",           "Jump to bottom",         "all lists"),
    ("Enter",       "Open detail",            "package list"),
    ("Esc / q",     "Back / quit",            "all views"),
    ("Ctrl+C",      "Force quit",             "always"),
    ("/",           "Open search",            "package list"),
    ("i",           "Install selected",       "package list, detail"),
    ("c",           "Context switcher",       "all views"),
    ("r",           "Reload",                 "all views"),
    ("Tab",         "Switch pane focus",      "detail view"),
    ("Del",         "Uninstall selected",     "installed list"),
    ("?",           "Toggle this help",       "always"),
];

/// Render the help overlay centred over `area`.
pub fn render_help(frame: &mut Frame, area: Rect, theme: &Theme) {
    let width: u16 = 62;
    let height: u16 = (BINDINGS.len() as u16 + 3).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let modal = Rect::new(x, y, width, height);

    frame.render_widget(Clear, modal);

    let block = Block::default()
        .title(" Keybindings  ?:close ")
        .borders(Borders::ALL)
        .border_style(theme.border());
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let items: Vec<ListItem> = BINDINGS
        .iter()
        .map(|(key, desc, scope)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{key:<18}"), theme.key_hint()),
                Span::styled(format!("{desc:<28}"), theme.muted()),
                Span::styled(*scope, theme.hint_desc()),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}
