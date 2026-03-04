//! Footer widget — keybinding hints.

use ratatui::{layout::Rect, Frame};

use crate::theme::Theme;

/// Renders the single-row footer bar with keybinding hints.
pub struct Footer<'a> {
    /// Pairs of `(key, description)` shown as `<key>description`.
    pub hints: &'a [(&'a str, &'a str)],
}

impl<'a> Footer<'a> {
    /// Render the footer into `area`.
    pub fn render(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
}
