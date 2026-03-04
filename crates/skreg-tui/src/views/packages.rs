//! Package list view — the root view of the TUI.

use ratatui::crossterm::event::Event;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use skreg_core::config::CliConfig;

use crate::theme::Theme;

use super::{Action, View};

/// Root view showing a searchable list of registry packages.
pub struct PackageListView {
    config: CliConfig,
}

impl PackageListView {
    /// Create a new view and kick off the initial package fetch.
    pub fn new(config: CliConfig) -> Self {
        Self { config }
    }
}

impl View for PackageListView {
    fn render(&mut self, frame: &mut Frame, area: Rect, _theme: &Theme) {
        let _ = &self.config;
        frame.render_widget(Paragraph::new("Loading packages..."), area);
    }

    fn handle_event(&mut self, event: Event) -> Action {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        if let Event::Key(KeyEvent { code: KeyCode::Char('q'), .. }) = event {
            return Action::Quit;
        }
        Action::None
    }
}
