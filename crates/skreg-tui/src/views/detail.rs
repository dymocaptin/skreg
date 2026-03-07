//! Package detail view — split pane with version info and manifest description.

use std::sync::Arc;

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use skreg_client::client::HttpRegistryClient;
use skreg_client::installer::Installer;
use skreg_core::config::CliConfig;
use skreg_core::manifest::Manifest;
use skreg_core::package_ref::PackageRef;
use tokio::sync::oneshot;

use crate::theme::Theme;
use crate::widgets::{footer::Footer, header::Header};

use super::{Action, ToastKind, View};

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

impl Default for DetailState {
    fn default() -> Self {
        Self::new()
    }
}

impl DetailState {
    /// Create a new state with focus on the versions pane.
    #[must_use]
    pub fn new() -> Self {
        Self {
            focus: Pane::Versions,
            scroll: 0,
            content_lines: 0,
            selected_version: 0,
        }
    }

    /// Toggle focus between the versions and content panes.
    pub fn toggle_pane(&mut self) {
        self.focus = if self.focus == Pane::Versions {
            Pane::SkillMd
        } else {
            Pane::Versions
        };
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
    install_rx: Option<oneshot::Receiver<Result<String, String>>>,
    /// Whether the currently displayed version is locally installed.
    is_installed: bool,
    /// Whether the uninstall confirmation prompt is active.
    /// Resets to false on any keypress; stale `true` resolves itself on next interaction.
    confirming: bool,
}

impl PackageDetailView {
    /// Create a new detail view and begin fetching the given version.
    #[must_use]
    pub fn new(config: CliConfig, namespace: String, name: String, latest: String) -> Self {
        let is_installed = Self::check_installed(&namespace, &name, &latest);
        let mut v = Self {
            config,
            namespace,
            name,
            state: DetailState::new(),
            data: None,
            rx: None,
            install_rx: None,
            is_installed,
            confirming: false,
        };
        v.fetch(latest);
        v
    }

    fn check_installed(namespace: &str, name: &str, version: &str) -> bool {
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(".skreg")
            .join("packages")
            .join(namespace)
            .join(name)
            .join(version);
        path.exists()
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

    fn install(&mut self) {
        let Some(data) = &self.data else { return };
        let registry = self.config.registry().to_string();
        let install_root = dirs::home_dir()
            .unwrap_or_default()
            .join(".skreg")
            .join("packages");
        let ref_str = format!("{}/{}@{}", self.namespace, self.name, data.manifest.version);
        let (tx, rx) = oneshot::channel();
        self.install_rx = Some(rx);
        tokio::spawn(async move {
            let client = Arc::new(HttpRegistryClient::new(registry));
            let installer = Installer::new(client, install_root);
            let pkg_ref = PackageRef::parse(&ref_str)
                .unwrap_or_else(|_| PackageRef::parse(&ref_str).unwrap());
            let result = installer
                .install(&pkg_ref)
                .await
                .map(|p| {
                    format!(
                        "{} v{}",
                        p.pkg_ref.name,
                        p.pkg_ref
                            .version
                            .as_ref()
                            .map_or_else(|| "?".to_string(), std::string::ToString::to_string),
                    )
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    fn do_uninstall(&mut self) -> Action {
        let Some(data) = &self.data else {
            return Action::None;
        };
        let version = data.manifest.version.to_string();
        let label = format!("{}/{} v{}", self.namespace, self.name, version);
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(".skreg")
            .join("packages")
            .join(&self.namespace)
            .join(&self.name)
            .join(&version);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {
                self.is_installed = false;
                Action::Toast(ToastKind::Success, format!("Uninstalled {label}"))
            }
            Err(e) => Action::Toast(ToastKind::Error, e.to_string()),
        }
    }
}

impl View for PackageDetailView {
    fn tick(&mut self) -> Option<Action> {
        if let Some(rx) = &mut self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                if let Ok(data) = result {
                    // Refresh installed state for this version.
                    self.is_installed = Self::check_installed(
                        &self.namespace,
                        &self.name,
                        &data.manifest.version.to_string(),
                    );
                    self.data = Some(data);
                }
            }
        }

        if let Some(rx) = &mut self.install_rx {
            if let Ok(result) = rx.try_recv() {
                self.install_rx = None;
                return Some(match result {
                    Ok(label) => {
                        self.is_installed = true;
                        Action::Toast(ToastKind::Success, format!("Installed {label}"))
                    }
                    Err(e) => Action::Toast(ToastKind::Error, e),
                });
            }
        }

        None
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
        let left_border = if focused_left {
            theme.selected()
        } else {
            theme.border()
        };
        let left_block = Block::default()
            .title(" Versions ")
            .borders(Borders::ALL)
            .border_style(left_border);
        let left_inner = left_block.inner(left);
        frame.render_widget(left_block, left);

        if let Some(data) = &self.data {
            let version_str = data.manifest.version.to_string();
            let prefix = if self.state.selected_version == 0 {
                "▶ "
            } else {
                "  "
            };
            let installed_span = if self.is_installed {
                Span::styled(" ●", theme.success())
            } else {
                Span::raw("")
            };
            let item = ListItem::new(Line::from(vec![
                Span::raw(format!("{prefix}{version_str}")),
                installed_span,
            ]));
            frame.render_widget(List::new(vec![item]), left_inner);
        } else {
            frame.render_widget(Paragraph::new("⠙ Loading..."), left_inner);
        }

        // Description / SKILL.md pane
        let focused_right = self.state.focus == Pane::SkillMd;
        let right_border = if focused_right {
            theme.selected()
        } else {
            theme.border()
        };
        let right_block = Block::default()
            .title(" SKILL.md ")
            .borders(Borders::ALL)
            .border_style(right_border);
        let right_inner = right_block.inner(right);
        frame.render_widget(right_block, right);

        if let Some(data) = &self.data {
            let content = format!(
                "# {}\n\nv{}\n\n{}\n",
                data.manifest.name, data.manifest.version, data.manifest.description,
            );
            #[allow(clippy::cast_possible_truncation)]
            let lines = content.lines().count() as u16;
            self.state.content_lines = lines;
            frame.render_widget(
                Paragraph::new(content)
                    .scroll((self.state.scroll, 0))
                    .wrap(Wrap { trim: false }),
                right_inner,
            );
        }

        let hints_vec: Vec<(&str, &str)> = if self.confirming {
            vec![("y", " confirm uninstall"), ("N", " cancel")]
        } else if self.is_installed {
            vec![
                ("i", "installed"),
                ("del", "uninstall"),
                ("tab", "switch pane"),
                ("j/k", "scroll"),
                ("esc", "back"),
            ]
        } else {
            vec![
                ("i", "install"),
                ("tab", "switch pane"),
                ("j/k", "scroll"),
                ("esc", "back"),
            ]
        };
        Footer { hints: &hints_vec }.render(frame, footer_area, theme);
    }

    fn handle_event(&mut self, event: Event) -> Action {
        if self.confirming {
            if let Event::Key(KeyEvent { code, .. }) = event {
                self.confirming = false;
                if code == KeyCode::Char('y') {
                    return self.do_uninstall();
                }
            }
            return Action::None;
        }
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
                KeyCode::Char('i') => {
                    if self.is_installed {
                        Action::Toast(ToastKind::Error, "Already installed".to_string())
                    } else if self.install_rx.is_none() {
                        self.install();
                        Action::Toast(ToastKind::Success, "Installing…".to_string())
                    } else {
                        Action::None
                    }
                }
                KeyCode::Delete if self.is_installed => {
                    self.confirming = true;
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
