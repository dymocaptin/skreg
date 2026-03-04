//! Context switcher overlay.

use ratatui::crossterm::event::Event;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use skreg_core::config::CliConfig;

use crate::theme::Theme;

use super::Action;

/// Modal overlay for switching between named registry contexts.
pub struct ContextOverlay {
    /// The full config (used to persist the new active context).
    pub config: CliConfig,
    /// Sorted list of context names.
    pub names: Vec<String>,
    /// Index of the currently highlighted entry.
    pub selected: usize,
}

impl ContextOverlay {
    /// Create a new overlay; the initially highlighted entry is the active context.
    pub fn new(config: CliConfig) -> Self {
        let mut names: Vec<String> = config.contexts.keys().cloned().collect();
        names.sort();
        let selected = names
            .iter()
            .position(|n| n == &config.active_context)
            .unwrap_or(0);
        Self { config, names, selected }
    }

    /// Return the name of the currently selected context.
    pub fn selected_name(&self) -> &str {
        &self.names[self.selected]
    }

    /// Handle input events for the overlay.
    pub fn handle_event(&mut self, event: Event) -> Action {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        if let Event::Key(KeyEvent { code, .. }) = event {
            match code {
                KeyCode::Esc => return Action::Pop,
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected + 1 < self.names.len() {
                        self.selected += 1;
                    }
                }
                KeyCode::Enter => {
                    return Action::SwitchContext(self.selected_name().to_string());
                }
                _ => {}
            }
        }
        Action::None
    }

    /// Render the overlay over the current frame.
    pub fn render(&self, frame: &mut Frame, _area: Rect, _theme: &Theme) {
        frame.render_widget(Paragraph::new("Context switcher"), frame.area());
    }
}
