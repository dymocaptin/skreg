//! Footer widget — keybinding hints.

use ratatui::{
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
    layout::Rect,
};

use crate::theme::Theme;

/// Renders the single-row footer bar with keybinding hints.
pub struct Footer<'a> {
    /// Pairs of `(key, description)` shown as `<key>description`.
    pub hints: &'a [(&'a str, &'a str)],
}

impl<'a> Footer<'a> {
    /// Render the footer into `area`.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let spans: Vec<Span> = self
            .hints
            .iter()
            .flat_map(|(key, desc)| {
                vec![
                    Span::styled(format!("<{key}>"), theme.key_hint()),
                    Span::styled(desc.to_string(), theme.hint_desc()),
                    Span::raw("  "),
                ]
            })
            .collect();
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
