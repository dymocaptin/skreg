//! Header widget — app name, context, breadcrumb.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

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
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let crumbs: Vec<Span> = self
            .breadcrumb
            .iter()
            .flat_map(|s| [Span::raw("▸ "), Span::styled(*s, theme.header())])
            .collect();

        let left = Line::from(
            [
                Span::styled("skreg", theme.header()),
                Span::raw("  "),
                Span::styled(
                    format!("[{} · {}]", self.context_name, self.namespace),
                    theme.accent(),
                ),
                Span::raw("  "),
            ]
            .into_iter()
            .chain(crumbs)
            .collect::<Vec<_>>(),
        );

        let right = Line::from(vec![
            Span::styled("?", theme.key_hint()),
            Span::styled(":help", theme.hint_desc()),
        ]);

        let [left_area, right_area] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(7)]).areas(area);

        frame.render_widget(Paragraph::new(left), left_area);
        frame.render_widget(Paragraph::new(right).right_aligned(), right_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn renders_app_name_and_context() {
        let backend = TestBackend::new(60, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = crate::theme::Theme::default();
        terminal
            .draw(|frame| {
                Header {
                    context_name: "public",
                    namespace: "dymo",
                    breadcrumb: &["Packages"],
                }
                .render(frame, frame.area(), &theme);
            })
            .unwrap();
        let content: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(content.contains("skreg"));
        assert!(content.contains("public"));
    }
}
