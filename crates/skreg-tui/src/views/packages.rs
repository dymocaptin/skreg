//! Package list view — the root view of the TUI.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState as RatatuiTableState},
    Frame,
};
use std::collections::HashSet;
use std::sync::Arc;

use skreg_client::client::{HttpRegistryClient, RegistryClient, SearchResult};
use skreg_client::installer::Installer;
use skreg_core::config::CliConfig;
use skreg_core::package_ref::PackageRef;
use tokio::sync::oneshot;

use super::installed::scan_installed;

use crate::theme::Theme;
use crate::widgets::{footer::Footer, header::Header};

use super::{Action, ToastKind, View};

/// Debounce delay before issuing a search fetch after the last keystroke.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);
/// Maximum description length before truncation.
const DESC_MAX: usize = 28;

/// Cursor and items for the package list table.
pub struct ListState {
    /// All currently loaded packages.
    pub items: Vec<SearchResult>,
    /// Index of the highlighted row.
    pub selected: usize,
    /// ratatui table-state for scroll/highlight tracking.
    pub table_state: RatatuiTableState,
}

impl Default for ListState {
    fn default() -> Self {
        Self::new()
    }
}

impl ListState {
    /// Create a new empty list state.
    #[must_use]
    pub fn new() -> Self {
        let mut table_state = RatatuiTableState::default();
        table_state.select(Some(0));
        Self {
            items: vec![],
            selected: 0,
            table_state,
        }
    }

    /// Move the cursor down one row, clamped at the last item.
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
            self.table_state.select(Some(self.selected));
        }
    }

    /// Move the cursor up one row, clamped at zero.
    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.table_state.select(Some(self.selected));
        }
    }

    /// Return a reference to the currently highlighted item, if any.
    #[must_use]
    pub fn selected_item(&self) -> Option<&SearchResult> {
        self.items.get(self.selected)
    }
}

/// Async load state for the package list.
enum Load {
    Loading,
    Loaded,
    Error(String),
}

/// Root view showing a searchable list of registry packages.
pub struct PackageListView {
    config: CliConfig,
    state: ListState,
    load: Load,
    rx: Option<oneshot::Receiver<Result<Vec<SearchResult>, String>>>,
    /// The last query that was actually sent to the registry.
    query: String,
    /// Whether the inline search bar is open.
    searching: bool,
    /// The text currently typed in the search bar (may not yet be sent).
    search_input: String,
    /// When the search input last changed (used for debouncing).
    search_changed_at: Option<Instant>,
    /// In-flight install result: `Ok(label)` on success, `Err(msg)` on failure.
    install_rx: Option<oneshot::Receiver<Result<String, String>>>,
    /// Set of `"namespace/name"` keys that are currently installed locally.
    installed: HashSet<String>,
    /// Whether the uninstall confirmation prompt is active.
    confirming: bool,
}

impl PackageListView {
    /// Create a new view and kick off the initial package fetch.
    #[must_use]
    pub fn new(config: CliConfig) -> Self {
        let mut v = Self {
            config,
            state: ListState::new(),
            load: Load::Loading,
            rx: None,
            query: String::new(),
            searching: false,
            search_input: String::new(),
            search_changed_at: None,
            install_rx: None,
            installed: Self::scan_installed_set(),
            confirming: false,
        };
        v.fetch();
        v
    }

    fn scan_installed_set() -> HashSet<String> {
        let base = dirs::home_dir()
            .unwrap_or_default()
            .join(".skreg")
            .join("packages");
        scan_installed(&base)
            .unwrap_or_default()
            .into_iter()
            .map(|p| format!("{}/{}", p.namespace, p.name))
            .collect()
    }

    fn install_selected(&mut self, namespace: String, name: String, version: String) {
        let registry = self.config.registry().to_string();
        let install_root = dirs::home_dir()
            .unwrap_or_default()
            .join(".skreg")
            .join("packages");
        let (tx, rx) = oneshot::channel();
        self.install_rx = Some(rx);
        tokio::spawn(async move {
            let client = Arc::new(HttpRegistryClient::new(registry));
            let installer = Installer::new(client, install_root);
            let pkg_ref = PackageRef::parse(&format!("{namespace}/{name}@{version}"))
                .unwrap_or_else(|_| PackageRef::parse(&format!("{namespace}/{name}")).unwrap());
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
                            .map_or_else(|| "?".to_string(), std::string::ToString::to_string)
                    )
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    fn fetch(&mut self) {
        let registry = self.config.registry().to_string();
        let query = self.query.clone();
        let (tx, rx) = oneshot::channel();
        self.rx = Some(rx);
        self.load = Load::Loading;
        tokio::spawn(async move {
            let client = HttpRegistryClient::new(registry);
            let result = client.search(&query).await.map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    fn uninstall_selected(&mut self) -> Action {
        let Some(item) = self.state.selected_item() else {
            return Action::None;
        };
        let ns = item.namespace.clone();
        let name = item.name.clone();
        let label = format!("{ns}/{name}");
        let path = dirs::home_dir()
            .unwrap_or_default()
            .join(".skreg")
            .join("packages")
            .join(&ns)
            .join(&name);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => {
                self.installed = Self::scan_installed_set();
                Action::Toast(ToastKind::Success, format!("Uninstalled {label}"))
            }
            Err(e) => Action::Toast(ToastKind::Error, e.to_string()),
        }
    }

    fn commit_search(&mut self) {
        self.query = self.search_input.clone();
        self.search_changed_at = None;
        self.state.selected = 0;
        self.state.table_state.select(Some(0));
        self.fetch();
    }

    fn clear_search(&mut self) {
        self.searching = false;
        self.search_input.clear();
        self.search_changed_at = None;
        if !self.query.is_empty() {
            self.query.clear();
            self.state.selected = 0;
            self.state.table_state.select(Some(0));
            self.fetch();
        }
    }
}

impl View for PackageListView {
    fn tick(&mut self) -> Option<Action> {
        // Debounced search: fire fetch once input settles for SEARCH_DEBOUNCE.
        if let Some(changed_at) = self.search_changed_at {
            if changed_at.elapsed() >= SEARCH_DEBOUNCE {
                self.commit_search();
            }
        }

        if let Some(rx) = &mut self.rx {
            if let Ok(result) = rx.try_recv() {
                self.rx = None;
                match result {
                    Ok(pkgs) => {
                        self.state.items = pkgs;
                        self.load = Load::Loaded;
                    }
                    Err(e) => {
                        self.load = Load::Error(e);
                    }
                }
            }
        }

        if let Some(rx) = &mut self.install_rx {
            if let Ok(result) = rx.try_recv() {
                self.install_rx = None;
                return Some(match result {
                    Ok(label) => {
                        self.installed = Self::scan_installed_set();
                        Action::Toast(ToastKind::Success, format!("Installed {label}"))
                    }
                    Err(e) => Action::Toast(ToastKind::Error, e),
                });
            }
        }

        None
    }

    #[allow(clippy::too_many_lines)]
    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let footer_rows: u16 = if self.searching { 2 } else { 1 };

        let [header_area, main_area, bottom_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(footer_rows),
        ])
        .areas(area);

        let ctx = self.config.active_context_config();
        Header {
            context_name: &self.config.active_context,
            namespace: &ctx.namespace,
            breadcrumb: &["Packages"],
        }
        .render(frame, header_area, theme);

        match &self.load {
            Load::Loading => {
                frame.render_widget(Paragraph::new("⠙ Fetching packages..."), main_area);
            }
            Load::Error(e) => {
                frame.render_widget(
                    Paragraph::new(format!("✗ {e}\n\n<r>retry  <q>quit")).style(theme.danger()),
                    main_area,
                );
            }
            Load::Loaded => {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border());
                let table_area = block.inner(main_area);
                frame.render_widget(block, main_area);

                // Reserve space for the header + separator inside the block.
                let [header_row, sep_row, data_area] = Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .areas(table_area);

                // Column widths (shared between header and data table).
                let widths = [
                    Constraint::Min(18),
                    Constraint::Length(14),
                    Constraint::Length(9),
                    Constraint::Length(30),
                ];

                // Render the header manually so we can draw a separator beneath it.
                let header_cols = Layout::horizontal(widths).spacing(1).areas::<4>(header_row);
                let labels = ["NAME", "NAMESPACE", "VERSION", "DESCRIPTION"];
                for (area, label) in header_cols.iter().zip(labels.iter()) {
                    frame.render_widget(Paragraph::new(*label).style(theme.header()), *area);
                }

                // Separator line under the header.
                let sep_line = "─".repeat(table_area.width as usize);
                frame.render_widget(Paragraph::new(sep_line).style(theme.border()), sep_row);

                // Truncate description to fit the fixed column width.
                let rows: Vec<Row> = self
                    .state
                    .items
                    .iter()
                    .map(|p| {
                        let desc = p.description.as_deref().unwrap_or("");
                        let desc_truncated = if desc.len() > DESC_MAX {
                            format!("{}…", &desc[..DESC_MAX.saturating_sub(1)])
                        } else {
                            desc.to_string()
                        };
                        let key = format!("{}/{}", p.namespace, p.name);
                        let name_cell = if self.installed.contains(&key) {
                            Cell::from(Line::from(vec![
                                Span::raw(p.name.clone()),
                                Span::styled(" ●", theme.success()),
                            ]))
                        } else {
                            Cell::from(p.name.clone())
                        };
                        Row::new(vec![
                            name_cell,
                            Cell::from(p.namespace.clone()),
                            Cell::from(p.latest_version.clone().unwrap_or_default()),
                            Cell::from(desc_truncated),
                        ])
                    })
                    .collect();

                let table = Table::new(rows, widths)
                    .row_highlight_style(theme.selected())
                    .highlight_symbol("▶ ")
                    .column_spacing(1);

                frame.render_stateful_widget(table, data_area, &mut self.state.table_state);
            }
        }

        if self.searching {
            let [search_area, footer_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(bottom_area);

            let filter_line = Line::from(vec![
                Span::styled("/", theme.accent()),
                Span::raw(" "),
                Span::raw(self.search_input.as_str()),
                Span::styled("█", theme.muted()),
            ]);
            frame.render_widget(Paragraph::new(filter_line), search_area);

            Footer {
                hints: &[("esc", "cancel"), ("enter", "search")],
            }
            .render(frame, footer_area, theme);
        } else if !self.query.is_empty() {
            let filter_hint = format!(
                "Filter: \"{}\" · {} result{}",
                self.query,
                self.state.items.len(),
                if self.state.items.len() == 1 { "" } else { "s" },
            );
            frame.render_widget(
                Paragraph::new(filter_hint).style(theme.accent()),
                bottom_area,
            );
        } else if self.confirming {
            Footer {
                hints: &[("y", " confirm uninstall"), ("N", " cancel")],
            }
            .render(frame, bottom_area, theme);
        } else {
            let is_installed_selected = self.state.selected_item().is_some_and(|item| {
                self.installed
                    .contains(&format!("{}/{}", item.namespace, item.name))
            });
            let mut hints: Vec<(&str, &str)> = vec![
                ("/", "search"),
                ("i", "install"),
                ("enter", "detail"),
                ("c", "context"),
                ("q", "quit"),
            ];
            if is_installed_selected {
                hints.push(("del", "uninstall"));
            }
            Footer { hints: &hints }.render(frame, bottom_area, theme);
        }
    }

    fn handle_event(&mut self, event: Event) -> Action {
        // Confirmation prompt takes priority over all other keys.
        if self.confirming {
            if let Event::Key(KeyEvent { code, .. }) = event {
                self.confirming = false;
                if code == KeyCode::Char('y') {
                    return self.uninstall_selected();
                }
            }
            return Action::None;
        }

        if self.searching {
            return self.handle_search_event(&event);
        }

        match event {
            Event::Key(KeyEvent { code, .. }) => match code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    if self.query.is_empty() {
                        Action::Quit
                    } else {
                        self.clear_search();
                        Action::None
                    }
                }
                KeyCode::Char('/') => {
                    self.searching = true;
                    self.search_input = self.query.clone();
                    Action::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.state.move_down();
                    Action::None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.state.move_up();
                    Action::None
                }
                KeyCode::Char('g') => {
                    self.state.selected = 0;
                    self.state.table_state.select(Some(0));
                    Action::None
                }
                KeyCode::Char('G') => {
                    if !self.state.items.is_empty() {
                        self.state.selected = self.state.items.len() - 1;
                        self.state.table_state.select(Some(self.state.selected));
                    }
                    Action::None
                }
                KeyCode::Char('r') => {
                    self.fetch();
                    Action::None
                }
                KeyCode::Delete => {
                    if let Some(item) = self.state.selected_item() {
                        let key = format!("{}/{}", item.namespace, item.name);
                        if self.installed.contains(&key) {
                            self.confirming = true;
                        }
                    }
                    Action::None
                }
                KeyCode::Char('c') => Action::OpenContextSwitcher,
                KeyCode::Char('i') => {
                    if let Some(p) = self.state.selected_item() {
                        let key = format!("{}/{}", p.namespace, p.name);
                        if self.installed.contains(&key) {
                            Action::Toast(ToastKind::Error, "Already installed".to_string())
                        } else {
                            let (ns, name, ver) = (
                                p.namespace.clone(),
                                p.name.clone(),
                                p.latest_version.clone().unwrap_or_default(),
                            );
                            self.install_selected(ns, name, ver);
                            Action::Toast(ToastKind::Success, "Installing…".to_string())
                        }
                    } else {
                        Action::None
                    }
                }
                KeyCode::Enter => {
                    if let Some(p) = self.state.selected_item() {
                        Action::Push(Box::new(super::detail::PackageDetailView::new(
                            self.config.clone(),
                            p.namespace.clone(),
                            p.name.clone(),
                            p.latest_version.clone().unwrap_or_default(),
                        )))
                    } else {
                        Action::None
                    }
                }
                _ => Action::None,
            },
            _ => Action::None,
        }
    }
}

impl PackageListView {
    fn handle_search_event(&mut self, event: &Event) -> Action {
        match event {
            Event::Key(KeyEvent {
                code, modifiers, ..
            }) => match code {
                KeyCode::Esc => {
                    self.clear_search();
                    Action::None
                }
                KeyCode::Enter => {
                    self.searching = false;
                    self.commit_search();
                    Action::None
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                    self.search_changed_at = Some(Instant::now());
                    Action::None
                }
                KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search_input.clear();
                    self.search_changed_at = Some(Instant::now());
                    Action::None
                }
                KeyCode::Char(c) => {
                    self.search_input.push(*c);
                    self.search_changed_at = Some(Instant::now());
                    Action::None
                }
                _ => Action::None,
            },
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summaries(names: &[&str]) -> Vec<SearchResult> {
        names
            .iter()
            .map(|n| SearchResult {
                namespace: "ns".into(),
                name: (*n).to_string(),
                description: None,
                latest_version: Some("1.0.0".into()),
            })
            .collect()
    }

    #[test]
    fn move_down_stops_at_end() {
        let mut s = ListState::new();
        s.items = summaries(&["a", "b"]);
        s.selected = 1;
        s.move_down();
        assert_eq!(s.selected, 1);
    }

    #[test]
    fn move_up_stops_at_zero() {
        let mut s = ListState::new();
        s.items = summaries(&["a", "b"]);
        s.selected = 0;
        s.move_up();
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn selected_item_returns_correct_entry() {
        let mut s = ListState::new();
        s.items = summaries(&["a", "b", "c"]);
        s.selected = 2;
        assert_eq!(s.selected_item().unwrap().name, "c");
    }
}
