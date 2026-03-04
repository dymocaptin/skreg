//! Package detail view — split pane with version info and manifest description.

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use skreg_client::client::HttpRegistryClient;
use skreg_core::config::CliConfig;
use skreg_core::manifest::Manifest;
use tokio::sync::oneshot;

use crate::theme::Theme;
use crate::widgets::{footer::Footer, header::Header};

use super::{Action, View};

/// Which pane currently holds keyboard focus in the detail view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// Left pane: versions list.
    Versions,
    /// Right pane: SKILL.md / description content.
    SkillMd,
}

/// Scrollable content state for the detail view.
pub struct DetailState {
    /// Currently focused pane.
    pub focus: Pane,
    /// Scroll offset in the right pane (lines from top).
    pub scroll: u16,
    /// Total number of content lines in the right pane.
    pub content_lines: u16,
    /// Index of the selected version in the versions list.
    pub selected_version: usize,
}

impl DetailState {
    /// Create a new state with focus on the versions pane.
    pub fn new() -> Self {
        Self { focus: Pane::Versions, scroll: 0, content_lines: 0, selected_version: 0 }
    }

    /// Toggle focus between the versions and content panes.
    pub fn toggle_pane(&mut self) {
        self.focus = if self.focus == Pane::Versions { Pane::SkillMd } else { Pane::Versions };
    }

    /// Scroll the content pane down one line, clamped at the last line.
    pub fn scroll_down(&mut self) {
        if self.scroll + 1 < self.content_lines {
            self.scroll += 1;
        }
    }

    /// Scroll the content pane up one line, clamped at zero.
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }
}

/// Loaded package data shown in the detail view.
struct DetailData {
    manifest: Manifest,
}

/// Split-pane view showing a package's version and description content.
pub struct PackageDetailView {
    config: CliConfig,
    namespace: String,
    name: String,
    state: DetailState,
    data: Option<DetailData>,
    rx: Option<oneshot::Receiver<Result<DetailData, String>>>,
}

impl PackageDetailView {
    /// Create a new detail view and begin fetching the given version.
    pub fn new(config: CliConfig, namespace: String, name: String, latest: String) -> Self {
        let mut v = Self {
            config,
            namespace,
            name,
            state: DetailState::new(),
            data: None,
            rx: None,
        };
        v.fetch(latest);
        v
    }

    fn fetch(&mut self, version: String) {
        let registry = self.config.registry().to_string();
        let ns = self.namespace.clone();
        let name = self.name.clone();
        let (tx, rx) = oneshot::channel();
        self.rx = Some(rx);
        tokio::spawn(async move {
            let client = HttpRegistryClient::new(registry);
            let result = client
                .fetch_manifest(&ns, &name, &version)
                .await
                .map(|manifest| DetailData { manifest })
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }
}

impl View for PackageDetailView {
    fn tick(&mut self) {
        if let Some(rx) = &mut self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                if let Ok(data) = result {
                    self.data = Some(data);
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

        let ctx = self.config.active_context_config();
        let name = self.name.clone();
        Header {
            context_name: &self.config.active_context,
            namespace: &ctx.namespace,
            breadcrumb: &["Packages", &name],
        }
        .render(frame, header_area, theme);

        let [left, right] =
            Layout::horizontal([Constraint::Length(28), Constraint::Min(0)]).areas(main_area);

        // Versions pane
        let focused_left = self.state.focus == Pane::Versions;
        let left_border = if focused_left { theme.selected() } else { theme.border() };
        let left_block = Block::default()
            .title(" Versions ")
            .borders(Borders::ALL)
            .border_style(left_border);
        let left_inner = left_block.inner(left);
        frame.render_widget(left_block, left);

        if let Some(data) = &self.data {
            let version_str = data.manifest.version.to_string();
            let prefix =
                if self.state.selected_version == 0 { "▶ " } else { "  " };
            let item = ListItem::new(format!("{prefix}{version_str}"));
            frame.render_widget(List::new(vec![item]), left_inner);
        } else {
            frame.render_widget(Paragraph::new("⠙ Loading..."), left_inner);
        }

        // Description / SKILL.md pane
        let focused_right = self.state.focus == Pane::SkillMd;
        let right_border = if focused_right { theme.selected() } else { theme.border() };
        let right_block = Block::default()
            .title(" SKILL.md ")
            .borders(Borders::ALL)
            .border_style(right_border);
        let right_inner = right_block.inner(right);
        frame.render_widget(right_block, right);

        if let Some(data) = &self.data {
            let content = format!(
                "# {}\n\nv{}\n\n{}\n",
                data.manifest.name,
                data.manifest.version,
                data.manifest.description,
            );
            self.state.content_lines = content.lines().count() as u16;
            frame.render_widget(
                Paragraph::new(content)
                    .scroll((self.state.scroll, 0))
                    .wrap(Wrap { trim: false }),
                right_inner,
            );
        }

        Footer {
            hints: &[
                ("i", "install"),
                ("tab", "switch pane"),
                ("j/k", "scroll"),
                ("esc", "back"),
            ],
        }
        .render(frame, footer_area, theme);
    }

    fn handle_event(&mut self, event: Event) -> Action {
        match event {
            Event::Key(KeyEvent { code, .. }) => match code {
                KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
                KeyCode::Tab => {
                    self.state.toggle_pane();
                    Action::None
                }
                KeyCode::Down | KeyCode::Char('j') if self.state.focus == Pane::SkillMd => {
                    self.state.scroll_down();
                    Action::None
                }
                KeyCode::Up | KeyCode::Char('k') if self.state.focus == Pane::SkillMd => {
                    self.state.scroll_up();
                    Action::None
                }
                KeyCode::Char('c') => Action::OpenContextSwitcher,
                _ => Action::None,
            },
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_pane_focus() {
        let mut s = DetailState::new();
        assert_eq!(s.focus, Pane::Versions);
        s.toggle_pane();
        assert_eq!(s.focus, Pane::SkillMd);
        s.toggle_pane();
        assert_eq!(s.focus, Pane::Versions);
    }

    #[test]
    fn scroll_clamps_to_content_height() {
        let mut s = DetailState::new();
        s.content_lines = 5;
        s.scroll = 4;
        s.scroll_down();
        assert_eq!(s.scroll, 4);
        s.scroll_up();
        assert_eq!(s.scroll, 3);
    }
}
