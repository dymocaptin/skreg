//! Context switcher modal overlay.

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem},
    Frame,
};
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

    /// Render the overlay centred over `area`.
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let width: u16 = 48;
        let height: u16 = (self.names.len() as u16 + 4).min(area.height.saturating_sub(4));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        let modal = Rect::new(x, y, width, height);

        frame.render_widget(Clear, modal);

        let block = Block::default()
            .title("─── Switch Context ")
            .borders(Borders::ALL)
            .border_style(theme.border());
        let inner = block.inner(modal);
        frame.render_widget(block, modal);

        let items: Vec<ListItem> = self
            .names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let ctx = &self.config.contexts[name];
                let is_active = name == &self.config.active_context;
                let is_selected = i == self.selected;
                let prefix = if is_selected { "▶ " } else { "  " };
                let name_style =
                    if is_active { theme.accent() } else { theme.muted() };
                ListItem::new(Line::from(vec![
                    Span::raw(prefix),
                    Span::styled(format!("{name:<14}"), name_style),
                    Span::styled(format!("  {}", ctx.namespace), theme.muted()),
                ]))
            })
            .collect();

        frame.render_widget(List::new(items), inner);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use skreg_core::config::{CliConfig, ContextConfig};

    use super::*;

    fn config_with(names: &[&str], active: &str) -> CliConfig {
        let mut contexts = HashMap::new();
        for n in names {
            contexts.insert(
                (*n).to_string(),
                ContextConfig {
                    registry: format!("https://{n}.example.com"),
                    namespace: (*n).to_string(),
                    api_key: "k".to_string(),
                },
            );
        }
        CliConfig { active_context: active.to_string(), contexts }
    }

    #[test]
    fn lists_contexts_sorted() {
        let overlay = ContextOverlay::new(config_with(&["zzz", "aaa"], "aaa"));
        assert_eq!(overlay.names[0], "aaa");
        assert_eq!(overlay.names[1], "zzz");
    }

    #[test]
    fn selected_starts_at_active() {
        let overlay = ContextOverlay::new(config_with(&["a", "b", "c"], "b"));
        assert_eq!(overlay.selected_name(), "b");
    }
}
