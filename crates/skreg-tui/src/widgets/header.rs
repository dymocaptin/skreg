//! Header widget — app name, context, breadcrumb.

use ratatui::{layout::Rect, Frame};

use crate::theme::Theme;

/// Renders the single-row header bar.
pub struct Header<'a> {
    /// Name of the active context.
    pub context_name: &'a str,
    /// Namespace slug from the active context.
    pub namespace: &'a str,
    /// Breadcrumb trail (e.g. `&["Packages"]` or `&["Packages", "color-analysis"]`).
    pub breadcrumb: &'a [&'a str],
}

impl<'a> Header<'a> {
    /// Render the header into `area`.
    pub fn render(&self, _frame: &mut Frame, _area: Rect, _theme: &Theme) {}
}
