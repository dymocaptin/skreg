//! Installed skills view — stub until Task 9.

use ratatui::crossterm::event::Event;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};

use crate::theme::Theme;

use super::{Action, View};

/// View listing locally installed skill packages.
pub struct InstalledView;

impl InstalledView {
    /// Create a new installed view.
    pub fn new(_config: skreg_core::config::CliConfig) -> Self {
        Self
    }
}

impl View for InstalledView {
    fn render(&mut self, frame: &mut Frame, area: Rect, _theme: &Theme) {
        frame.render_widget(Paragraph::new("Installed packages"), area);
    }

    fn handle_event(&mut self, event: Event) -> Action {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        if let Event::Key(KeyEvent { code: KeyCode::Esc | KeyCode::Char('q'), .. }) = event {
            return Action::Pop;
        }
        Action::None
    }
}
