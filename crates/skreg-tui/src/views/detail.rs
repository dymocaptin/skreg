//! Package detail view — stub until Task 8.

use ratatui::crossterm::event::Event;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};

use crate::theme::Theme;

use super::{Action, View};

/// Split-pane view showing package versions and SKILL.md content.
pub struct PackageDetailView {
    namespace: String,
    name: String,
}

impl PackageDetailView {
    /// Create a new detail view for the given package.
    pub fn new(
        _config: skreg_core::config::CliConfig,
        namespace: String,
        name: String,
        _latest: String,
    ) -> Self {
        Self { namespace, name }
    }
}

impl View for PackageDetailView {
    fn render(&mut self, frame: &mut Frame, area: Rect, _theme: &Theme) {
        frame.render_widget(
            Paragraph::new(format!("Detail: {}/{}", self.namespace, self.name)),
            area,
        );
    }

    fn handle_event(&mut self, event: Event) -> Action {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        if let Event::Key(KeyEvent { code: KeyCode::Esc | KeyCode::Char('q'), .. }) = event {
            return Action::Pop;
        }
        Action::None
    }
}
