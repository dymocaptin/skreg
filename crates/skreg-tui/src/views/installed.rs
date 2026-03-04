//! Installed skills view — lists locally installed packages and supports uninstall.

use std::path::{Path, PathBuf};

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    widgets::{Row, Table, TableState},
    Frame,
};
use skreg_core::config::CliConfig;

use crate::theme::Theme;
use crate::widgets::{footer::Footer, header::Header};

use super::{Action, ToastKind, View};

/// A single installed package entry.
#[derive(Debug)]
pub struct InstalledPkg {
    /// Publisher namespace slug.
    pub namespace: String,
    /// Package name slug.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Path to the version directory on disk.
    pub path: PathBuf,
}

/// Scan `base` for installed packages structured as `{ns}/{name}/{version}/`.
///
/// Returns an empty `Vec` if `base` does not exist.
///
/// # Errors
///
/// Returns an error if any directory entry cannot be read.
pub fn scan_installed(base: &Path) -> anyhow::Result<Vec<InstalledPkg>> {
    let mut out = Vec::new();
    if !base.exists() {
        return Ok(out);
    }
    for ns_entry in std::fs::read_dir(base)? {
        let ns_entry = ns_entry?;
        if !ns_entry.file_type()?.is_dir() {
            continue;
        }
        let namespace = ns_entry.file_name().to_string_lossy().into_owned();
        for pkg_entry in std::fs::read_dir(ns_entry.path())? {
            let pkg_entry = pkg_entry?;
            if !pkg_entry.file_type()?.is_dir() {
                continue;
            }
            let name = pkg_entry.file_name().to_string_lossy().into_owned();
            for ver_entry in std::fs::read_dir(pkg_entry.path())? {
                let ver_entry = ver_entry?;
                if !ver_entry.file_type()?.is_dir() {
                    continue;
                }
                out.push(InstalledPkg {
                    namespace: namespace.clone(),
                    name: name.clone(),
                    version: ver_entry.file_name().to_string_lossy().into_owned(),
                    path: ver_entry.path(),
                });
            }
        }
    }
    Ok(out)
}

fn packages_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".skreg").join("packages")
}

/// View listing locally installed skill packages with uninstall support.
pub struct InstalledView {
    config: CliConfig,
    packages: Vec<InstalledPkg>,
    selected: usize,
    table_state: TableState,
    /// Whether the uninstall confirmation prompt is active.
    confirming: bool,
}

impl InstalledView {
    /// Create a new view, scanning `~/.skreg/packages/` immediately.
    pub fn new(config: CliConfig) -> Self {
        let packages = scan_installed(&packages_dir()).unwrap_or_default();
        let mut table_state = TableState::default();
        if !packages.is_empty() {
            table_state.select(Some(0));
        }
        Self { config, packages, selected: 0, table_state, confirming: false }
    }

    fn uninstall_selected(&mut self) -> anyhow::Result<String> {
        let pkg = &self.packages[self.selected];
        let label = format!("{}/{} v{}", pkg.namespace, pkg.name, pkg.version);
        std::fs::remove_dir_all(&pkg.path)?;
        self.packages.remove(self.selected);
        if self.selected > 0 {
            self.selected -= 1;
        }
        let sel = if self.packages.is_empty() { None } else { Some(self.selected) };
        self.table_state.select(sel);
        Ok(label)
    }
}

impl View for InstalledView {
    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

        let ctx = self.config.active_context_config();
        Header {
            context_name: &self.config.active_context,
            namespace: &ctx.namespace,
            breadcrumb: &["Installed"],
        }
        .render(frame, header_area, theme);

        let rows: Vec<Row> = self
            .packages
            .iter()
            .map(|p| Row::new(vec![p.name.clone(), p.namespace.clone(), p.version.clone()]))
            .collect();

        let table = Table::new(
            rows,
            [Constraint::Min(20), Constraint::Length(14), Constraint::Length(10)],
        )
        .header(Row::new(["NAME", "NAMESPACE", "VERSION"]).style(theme.header()))
        .row_highlight_style(theme.selected())
        .highlight_symbol("▶ ");

        frame.render_stateful_widget(table, main_area, &mut self.table_state);

        let hints: &[(&str, &str)] = if self.confirming {
            &[("y", " confirm uninstall"), ("N", " cancel")]
        } else {
            &[("enter", "detail"), ("Del", "uninstall"), ("esc", "back")]
        };
        Footer { hints }.render(frame, footer_area, theme);
    }

    fn handle_event(&mut self, event: Event) -> Action {
        match event {
            Event::Key(KeyEvent { code, .. }) => {
                if self.confirming {
                    self.confirming = false;
                    if code == KeyCode::Char('y') {
                        return match self.uninstall_selected() {
                            Ok(label) => {
                                Action::Toast(ToastKind::Success, format!("Uninstalled {label}"))
                            }
                            Err(e) => Action::Toast(ToastKind::Error, e.to_string()),
                        };
                    }
                    return Action::None;
                }
                match code {
                    KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.selected + 1 < self.packages.len() {
                            self.selected += 1;
                            self.table_state.select(Some(self.selected));
                        }
                        Action::None
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if self.selected > 0 {
                            self.selected -= 1;
                            self.table_state.select(Some(self.selected));
                        }
                        Action::None
                    }
                    KeyCode::Delete if !self.packages.is_empty() => {
                        self.confirming = true;
                        Action::None
                    }
                    KeyCode::Char('c') => Action::OpenContextSwitcher,
                    _ => Action::None,
                }
            }
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn make_entry(tmp: &TempDir, ns: &str, name: &str, ver: &str) {
        fs::create_dir_all(tmp.path().join(ns).join(name).join(ver)).unwrap();
    }

    #[test]
    fn scan_finds_all_installed() {
        let tmp = TempDir::new().unwrap();
        make_entry(&tmp, "dymo", "color-analysis", "1.2.0");
        make_entry(&tmp, "tools", "palette-gen", "2.0.0");
        let pkgs = scan_installed(tmp.path()).unwrap();
        assert_eq!(pkgs.len(), 2);
        assert!(pkgs.iter().any(|p| p.name == "color-analysis"));
    }

    #[test]
    fn scan_empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let pkgs = scan_installed(tmp.path()).unwrap();
        assert!(pkgs.is_empty());
    }
}
