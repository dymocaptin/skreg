//! Semantic color palette and style helpers for the skreg TUI.
use ratatui::style::{Color, Modifier, Style};

/// Semantic color palette. All colors are 16-color ANSI for broad terminal compatibility.
/// `NO_COLOR` env-var enforcement is handled at render time by the crossterm backend.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Cyan — selected row, focused pane border.
    pub primary: Color,
    /// Yellow — active context name, version values.
    pub accent: Color,
    /// Green — installed indicator dot, success toast.
    pub success: Color,
    /// Red — error toast, yanked label.
    pub danger: Color,
    /// Dark gray — borders, timestamps, secondary text.
    pub muted: Color,
    /// White — primary text.
    pub fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Cyan,
            accent: Color::Yellow,
            success: Color::Green,
            danger: Color::Red,
            muted: Color::DarkGray,
            fg: Color::White,
        }
    }
}

impl Theme {
    /// Style for the selected/highlighted row.
    #[must_use]
    pub fn selected(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for column headers and bold labels.
    #[must_use]
    pub fn header(&self) -> Style {
        Style::default().fg(self.fg).add_modifier(Modifier::BOLD)
    }

    /// Style for secondary/muted text.
    #[must_use]
    pub fn muted(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Style for borders and dividers.
    #[must_use]
    pub fn border(&self) -> Style {
        Style::default().fg(self.muted)
    }

    /// Style for accented text (context name, version).
    #[must_use]
    pub fn accent(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for success indicators.
    #[must_use]
    pub fn success(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Style for error/danger indicators.
    #[must_use]
    pub fn danger(&self) -> Style {
        Style::default().fg(self.danger)
    }

    /// Style for key names in the footer hint bar.
    #[must_use]
    pub fn key_hint(&self) -> Style {
        Style::default().fg(self.fg).add_modifier(Modifier::BOLD)
    }

    /// Style for key descriptions in the footer hint bar.
    #[must_use]
    pub fn hint_desc(&self) -> Style {
        Style::default().fg(self.muted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn uses_16color_ansi_palette() {
        let t = Theme::default();
        assert!(matches!(t.primary, Color::Cyan));
        assert!(matches!(t.success, Color::Green));
        assert!(matches!(t.danger, Color::Red));
        assert!(matches!(t.accent, Color::Yellow));
        assert!(matches!(t.muted, Color::DarkGray));
    }
}
