//! Package detail view — three-pane layout: versions, files, SKILL.md.

use std::sync::Arc;

use log::warn;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use skreg_client::client::{HttpRegistryClient, PackagePreview, RegistryClient, SkillDiff};
use skreg_client::installer::Installer;
use skreg_client::linker::{
    build_skill_entries, default_claude_md_path, default_links_path, default_tool_skill_dirs,
    Linker,
};
use skreg_core::config::CliConfig;
use skreg_core::package_ref::PackageRef;
use tokio::sync::oneshot;

use crate::theme::Theme;
use crate::widgets::{footer::Footer, header::Header};

use super::installed::packages_dir;
use super::{Action, ToastKind, View};

/// Which pane currently holds keyboard focus in the detail view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// Left pane: versions list.
    Versions,
    /// Right pane: SKILL.md content.
    SkillMd,
}

/// Async load state for the package preview data.
pub enum PreviewState {
    /// No load started yet.
    NotLoaded,
    /// Request in flight.
    Loading,
    /// Data available.
    Loaded(PackagePreview),
    /// Load failed; holds the error message.
    Failed(String),
}

/// Async load state for the version diff.
pub enum DiffState {
    /// Diff request in flight.
    Loading,
    /// Diff available.
    Loaded(SkillDiff),
    /// Diff failed; holds the error message.
    Failed(String),
}

/// Scrollable content state for the detail view.
pub struct DetailState {
    /// Currently focused pane.
    pub focus: Pane,
    /// Scroll offset in the SKILL.md pane (lines from top).
    pub scroll: u16,
    /// Total number of content lines in the SKILL.md pane.
    pub content_lines: u16,
    /// Published version strings, most recent first.
    pub versions: Vec<String>,
    /// Index into `versions` of the selected version.
    pub selected_version: usize,
    /// Preview data for the file tree and SKILL.md pane.
    pub preview: PreviewState,
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
            versions: Vec::new(),
            selected_version: 0,
            preview: PreviewState::NotLoaded,
        }
    }

    /// Toggle focus between the versions and SKILL.md panes.
    pub fn toggle_pane(&mut self) {
        self.focus = if self.focus == Pane::Versions {
            Pane::SkillMd
        } else {
            Pane::Versions
        };
    }

    /// Scroll the SKILL.md pane down one line, clamped at the last line.
    pub fn scroll_down(&mut self) {
        if self.scroll + 1 < self.content_lines {
            self.scroll += 1;
        }
    }

    /// Scroll the SKILL.md pane up one line, clamped at zero.
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    /// Move selection to the next (older) version, clamped at the last.
    pub fn select_next_version(&mut self) {
        if self.selected_version + 1 < self.versions.len() {
            self.selected_version += 1;
        }
    }

    /// Move selection to the previous (newer) version, clamped at the first.
    pub fn select_prev_version(&mut self) {
        self.selected_version = self.selected_version.saturating_sub(1);
    }
}

/// Recursively collect relative file paths for an installed package on disk.
fn collect_installed_files(base: &std::path::Path, dir: &std::path::Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.path();
        let Ok(rel) = path.strip_prefix(base) else {
            continue;
        };
        if path.is_dir() {
            files.extend(collect_installed_files(base, &path));
        } else {
            files.push(rel.to_string_lossy().into_owned());
        }
    }
    files
}

/// Read preview data for a locally installed package from disk.
fn load_preview_from_disk(namespace: &str, name: &str, version: &str) -> PackagePreview {
    const SKILL_MD_MAX: usize = 16 * 1024;
    let base = packages_dir().join(namespace).join(name).join(version);
    let files = collect_installed_files(&base, &base);
    let raw = std::fs::read_to_string(base.join("SKILL.md")).unwrap_or_default();
    let (skill_md, truncated) = if raw.len() > SKILL_MD_MAX {
        let mut at = SKILL_MD_MAX;
        while !raw.is_char_boundary(at) {
            at -= 1;
        }
        (raw[..at].to_string(), true)
    } else {
        (raw, false)
    };
    PackagePreview {
        files,
        skill_md,
        truncated,
    }
}

/// Three-pane view showing a package's version, file tree, and SKILL.md content.
#[allow(clippy::struct_excessive_bools)]
pub struct PackageDetailView {
    config: CliConfig,
    namespace: String,
    name: String,
    version: String,
    /// Whether the package's namespace has a valid publisher cert.
    trusted: bool,
    state: DetailState,
    /// In-flight preview fetch (None when reading from disk).
    preview_rx: Option<oneshot::Receiver<Result<PackagePreview, String>>>,
    /// In-flight versions list fetch.
    versions_rx: Option<oneshot::Receiver<Result<Vec<String>, String>>>,
    install_rx: Option<oneshot::Receiver<Result<String, String>>>,
    /// In-flight diff fetch.
    diff_rx: Option<oneshot::Receiver<Result<SkillDiff, String>>>,
    /// Whether the currently displayed version is locally installed.
    is_installed: bool,
    /// Whether the uninstall confirmation prompt is active.
    confirming: bool,
    /// Whether diff mode is active (SKILL.md pane replaced by diff output).
    diff_mode: bool,
    /// Current diff data (None, Loading, Loaded, or Failed).
    diff: Option<DiffState>,
    /// Scroll offset in the diff pane (lines from top).
    diff_scroll: u16,
}

impl PackageDetailView {
    /// Create a new detail view.
    ///
    /// If the package is installed, preview data is read synchronously from disk.
    /// If not installed, an async fetch via the preview endpoint is dispatched.
    #[must_use]
    pub fn new(
        config: CliConfig,
        namespace: String,
        name: String,
        version: String,
        trusted: bool,
    ) -> Self {
        let is_installed = Self::check_installed(&namespace, &name, &version);
        let mut v = Self {
            config,
            namespace,
            name,
            version,
            trusted,
            state: DetailState::new(),
            preview_rx: None,
            versions_rx: None,
            install_rx: None,
            diff_rx: None,
            is_installed,
            confirming: false,
            diff_mode: false,
            diff: None,
            diff_scroll: 0,
        };
        if is_installed {
            let preview = load_preview_from_disk(&v.namespace, &v.name, &v.version);
            v.state.preview = PreviewState::Loaded(preview);
        } else {
            v.state.preview = PreviewState::Loading;
            v.fetch_preview();
        }
        v.fetch_versions();
        v
    }

    fn check_installed(namespace: &str, name: &str, version: &str) -> bool {
        packages_dir()
            .join(namespace)
            .join(name)
            .join(version)
            .exists()
    }

    fn fetch_preview(&mut self) {
        let registry = self.config.registry().to_string();
        let ns = self.namespace.clone();
        let name = self.name.clone();
        let version = self.version.clone();
        let (tx, rx) = oneshot::channel();
        self.preview_rx = Some(rx);
        tokio::spawn(async move {
            let client = HttpRegistryClient::new(registry);
            let result = client
                .preview_package(&ns, &name, &version)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    fn fetch_versions(&mut self) {
        let registry = self.config.registry().to_string();
        let ns = self.namespace.clone();
        let name = self.name.clone();
        let (tx, rx) = oneshot::channel();
        self.versions_rx = Some(rx);
        tokio::spawn(async move {
            let client = HttpRegistryClient::new(registry);
            let result = client
                .list_versions(&ns, &name)
                .await
                .map(|list| list.versions.into_iter().map(|v| v.version).collect())
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    fn fetch_diff(&mut self, from: String, to: String) {
        let registry = self.config.registry().to_string();
        let ns = self.namespace.clone();
        let name = self.name.clone();
        let (tx, rx) = oneshot::channel();
        self.diff_rx = Some(rx);
        self.diff = Some(DiffState::Loading);
        tokio::spawn(async move {
            let client = HttpRegistryClient::new(registry);
            let result = client
                .diff(&ns, &name, Some(&from), Some(&to))
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    /// Enter diff mode comparing the selected version against its predecessor.
    /// Returns a toast action when there is no predecessor to compare against.
    fn enter_diff_mode(&mut self) -> Action {
        let versions = &self.state.versions;
        let sel = self.state.selected_version;
        let (Some(to), Some(from)) = (versions.get(sel), versions.get(sel + 1)) else {
            return Action::Toast(
                ToastKind::Error,
                "No older version to diff against".to_string(),
            );
        };
        let (from, to) = (from.clone(), to.clone());
        self.diff_mode = true;
        self.diff_scroll = 0;
        self.fetch_diff(from, to);
        Action::None
    }

    fn install(&mut self) {
        let registry = self.config.registry().to_string();
        let install_root = packages_dir();
        let enforcement = self.config.policy.enforcement.clone();
        let ref_str = format!("{}/{}@{}", self.namespace, self.name, self.version);
        let (tx, rx) = oneshot::channel();
        self.install_rx = Some(rx);
        tokio::spawn(async move {
            let result = match PackageRef::parse(&ref_str) {
                Ok(pkg_ref) => {
                    let client = Arc::new(HttpRegistryClient::new(registry));
                    let installer = Installer::new(client, install_root);
                    match installer.install(&pkg_ref).await {
                        Ok((installed_pkg, _manifest)) => {
                            let label = format!(
                                "{} v{}",
                                installed_pkg.pkg_ref.name,
                                installed_pkg.pkg_ref.version.as_ref().map_or_else(
                                    || "?".to_string(),
                                    std::string::ToString::to_string
                                ),
                            );
                            // Symlink into tool directories and update CLAUDE.md,
                            // mirroring the same steps as `skreg install`.
                            if let (Some(links_path), Some(tool_dirs)) =
                                (default_links_path(), default_tool_skill_dirs())
                            {
                                let ns = installed_pkg.pkg_ref.namespace.as_str();
                                let name = installed_pkg.pkg_ref.name.as_str();
                                let version = installed_pkg
                                    .install_path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("");
                                let mut linker = Linker::new(links_path);
                                if let Err(e) = linker.create_symlinks(
                                    ns,
                                    name,
                                    version,
                                    &installed_pkg.install_path,
                                    &tool_dirs,
                                    true,
                                ) {
                                    warn!("failed to create symlinks for {ns}/{name}: {e}");
                                }
                                if let Some(claude_md) = default_claude_md_path() {
                                    if claude_md.parent().is_some_and(std::path::Path::exists) {
                                        let today =
                                            chrono::Local::now().format("%Y-%m-%d").to_string();
                                        let entries = build_skill_entries(linker.links(), &today);
                                        if let Err(e) = linker.write_skreg_rules(
                                            &claude_md,
                                            &entries,
                                            &enforcement,
                                        ) {
                                            warn!("failed to update CLAUDE.md: {e}");
                                        }
                                    }
                                }
                            }
                            Ok(label)
                        }
                        Err(e) => Err(e.to_string()),
                    }
                }
                Err(e) => Err(format!("invalid package ref '{ref_str}': {e}")),
            };
            let _ = tx.send(result);
        });
    }

    fn do_uninstall(&mut self) -> Action {
        let label = format!("{}/{} v{}", self.namespace, self.name, self.version);
        let path = packages_dir()
            .join(&self.namespace)
            .join(&self.name)
            .join(&self.version);
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
        if let Some(rx) = &mut self.preview_rx {
            if let Ok(result) = rx.try_recv() {
                self.preview_rx = None;
                self.state.preview = match result {
                    Ok(p) => PreviewState::Loaded(p),
                    Err(e) => PreviewState::Failed(e),
                };
            }
        }

        if let Some(rx) = &mut self.versions_rx {
            if let Ok(result) = rx.try_recv() {
                self.versions_rx = None;
                if let Ok(versions) = result {
                    if !versions.is_empty() {
                        self.state.versions = versions;
                        // Keep the currently displayed version selected if present.
                        if let Some(idx) =
                            self.state.versions.iter().position(|v| *v == self.version)
                        {
                            self.state.selected_version = idx;
                        }
                    }
                }
            }
        }

        if let Some(rx) = &mut self.install_rx {
            if let Ok(result) = rx.try_recv() {
                self.install_rx = None;
                return Some(match result {
                    Ok(label) => {
                        self.is_installed = true;
                        // Reload preview from disk now that the package is installed.
                        let preview =
                            load_preview_from_disk(&self.namespace, &self.name, &self.version);
                        self.state.preview = PreviewState::Loaded(preview);
                        Action::Toast(ToastKind::Success, format!("Installed {label}"))
                    }
                    Err(e) => Action::Toast(ToastKind::Error, e),
                });
            }
        }

        if let Some(rx) = &mut self.diff_rx {
            if let Ok(result) = rx.try_recv() {
                self.diff_rx = None;
                self.diff = Some(match result {
                    Ok(d) => DiffState::Loaded(d),
                    Err(e) => DiffState::Failed(e),
                });
            }
        }

        None
    }

    #[allow(clippy::too_many_lines)]
    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let [header_area, main_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

        let ctx = self.config.active_context_config();
        // Build breadcrumb label: append " ✓ trusted" when the namespace is trusted.
        let trusted_label;
        let breadcrumb_name: &str = if self.trusted {
            trusted_label = format!("{}  \u{2713} trusted", self.name);
            &trusted_label
        } else {
            &self.name
        };
        Header {
            context_name: &self.config.active_context,
            namespace: &ctx.namespace,
            breadcrumb: &["Packages", breadcrumb_name],
        }
        .render(frame, header_area, theme);

        // Three-pane layout: Versions | Files | SKILL.md
        let [versions_area, files_area, skill_area] = Layout::horizontal([
            Constraint::Length(14),
            Constraint::Length(26),
            Constraint::Min(0),
        ])
        .areas(main_area);

        // ── Versions pane ──────────────────────────────────────────────────────
        let focused_versions = self.state.focus == Pane::Versions;
        let versions_block = Block::default()
            .title(" Versions ")
            .borders(Borders::ALL)
            .border_style(if focused_versions {
                theme.selected()
            } else {
                theme.border()
            });
        let versions_inner = versions_block.inner(versions_area);
        frame.render_widget(versions_block, versions_area);

        let version_strings: Vec<String> = if self.state.versions.is_empty() {
            vec![self.version.clone()]
        } else {
            self.state.versions.clone()
        };
        let version_items: Vec<ListItem> = version_strings
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let selected = i == self.state.selected_version;
                let cursor = if selected { "\u{25b6} " } else { "  " };
                let mut spans = vec![Span::raw(format!("{cursor}{v}"))];
                if *v == self.version && self.is_installed {
                    spans.push(Span::styled(" \u{25cf}", theme.success()));
                }
                let item = ListItem::new(Line::from(spans));
                if selected {
                    item.style(theme.selected())
                } else {
                    item
                }
            })
            .collect();
        frame.render_widget(List::new(version_items), versions_inner);

        // ── Files pane ─────────────────────────────────────────────────────────
        let files_title = if self.is_installed {
            " Files \u{25cf} "
        } else {
            " Files "
        };
        let files_block = Block::default()
            .title(files_title)
            .title_style(if self.is_installed {
                theme.success()
            } else {
                theme.border()
            })
            .borders(Borders::ALL)
            .border_style(theme.border());
        let files_inner = files_block.inner(files_area);
        frame.render_widget(files_block, files_area);

        match &self.state.preview {
            PreviewState::NotLoaded | PreviewState::Loading => {
                frame.render_widget(
                    Paragraph::new("\u{2819} Loading...").style(theme.muted()),
                    files_inner,
                );
            }
            PreviewState::Failed(e) => {
                frame.render_widget(
                    Paragraph::new(format!("error fetching preview\n{e}")).style(theme.danger()),
                    files_inner,
                );
            }
            PreviewState::Loaded(preview) => {
                let items: Vec<ListItem> = preview
                    .files
                    .iter()
                    .map(|f| ListItem::new(f.as_str()))
                    .collect();
                frame.render_widget(List::new(items), files_inner);
            }
        }

        // ── SKILL.md / Diff pane ───────────────────────────────────────────────
        let focused_skill = self.state.focus == Pane::SkillMd;
        let skill_block = Block::default()
            .title(if self.diff_mode {
                " Diff "
            } else {
                " SKILL.md "
            })
            .borders(Borders::ALL)
            .border_style(if focused_skill {
                theme.selected()
            } else {
                theme.border()
            });
        let skill_inner = skill_block.inner(skill_area);
        frame.render_widget(skill_block, skill_area);

        if self.diff_mode {
            match &self.diff {
                Some(DiffState::Loading) | None => {
                    frame.render_widget(
                        Paragraph::new("\u{2819} Loading diff...").style(theme.muted()),
                        skill_inner,
                    );
                }
                Some(DiffState::Failed(e)) => {
                    frame.render_widget(
                        Paragraph::new(format!("error fetching diff\n{e}")).style(theme.danger()),
                        skill_inner,
                    );
                }
                Some(DiffState::Loaded(d)) => {
                    let lines = diff_lines(d, theme);
                    #[allow(clippy::cast_possible_truncation)]
                    let total = lines.len() as u16;
                    self.state.content_lines = total;
                    frame.render_widget(
                        Paragraph::new(lines)
                            .scroll((self.diff_scroll, 0))
                            .wrap(Wrap { trim: false }),
                        skill_inner,
                    );
                }
            }
        } else {
            match &self.state.preview {
                PreviewState::NotLoaded | PreviewState::Loading => {
                    frame.render_widget(
                        Paragraph::new("\u{2819} Loading...").style(theme.muted()),
                        skill_inner,
                    );
                }
                PreviewState::Failed(_) => {
                    frame.render_widget(
                        Paragraph::new("error fetching preview").style(theme.danger()),
                        skill_inner,
                    );
                }
                PreviewState::Loaded(preview) => {
                    let mut content = preview.skill_md.clone();
                    if preview.truncated {
                        content.push_str("\n\n[truncated]");
                    }
                    #[allow(clippy::cast_possible_truncation)]
                    let lines = content.lines().count() as u16;
                    self.state.content_lines = lines;
                    frame.render_widget(
                        Paragraph::new(content)
                            .scroll((self.state.scroll, 0))
                            .wrap(Wrap { trim: false }),
                        skill_inner,
                    );
                }
            }
        }

        let hints_vec: Vec<(&str, &str)> = if self.confirming {
            vec![("y", " confirm uninstall"), ("N", " cancel")]
        } else if self.diff_mode {
            vec![("j/k", "scroll"), ("d/esc", "exit diff")]
        } else if self.is_installed {
            vec![
                ("i", "installed"),
                ("d", "diff"),
                ("del", "uninstall"),
                ("tab", "switch pane"),
                ("j/k", "nav/scroll"),
                ("esc", "back"),
            ]
        } else {
            vec![
                ("i", "install"),
                ("d", "diff"),
                ("tab", "switch pane"),
                ("j/k", "nav/scroll"),
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
                KeyCode::Esc | KeyCode::Char('q') => {
                    if self.diff_mode {
                        self.diff_mode = false;
                        Action::None
                    } else {
                        Action::Pop
                    }
                }
                KeyCode::Tab => {
                    self.state.toggle_pane();
                    Action::None
                }
                KeyCode::Char('d') if !self.diff_mode && self.state.focus == Pane::Versions => {
                    self.enter_diff_mode()
                }
                KeyCode::Char('d') if self.diff_mode => {
                    self.diff_mode = false;
                    Action::None
                }
                KeyCode::Down | KeyCode::Char('j') if self.diff_mode => {
                    if self.diff_scroll + 1 < self.state.content_lines {
                        self.diff_scroll += 1;
                    }
                    Action::None
                }
                KeyCode::Up | KeyCode::Char('k') if self.diff_mode => {
                    self.diff_scroll = self.diff_scroll.saturating_sub(1);
                    Action::None
                }
                KeyCode::Down | KeyCode::Char('j') if self.state.focus == Pane::Versions => {
                    self.state.select_next_version();
                    Action::None
                }
                KeyCode::Up | KeyCode::Char('k') if self.state.focus == Pane::Versions => {
                    self.state.select_prev_version();
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
                        Action::Toast(ToastKind::Success, "Installing\u{2026}".to_string())
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

/// Build styled lines for a diff, using `+`/`-`/space prefixes plus semantic
/// color so the diff is legible without relying on color alone.
fn diff_lines<'a>(diff: &'a SkillDiff, theme: &Theme) -> Vec<Line<'a>> {
    use skreg_client::client::{FileStatus, LineKind};
    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("Comparing {} \u{2192} {}", diff.from, diff.to),
        theme.muted(),
    )));
    if diff.files.is_empty() {
        lines.push(Line::from(Span::raw("No changes between these versions.")));
        return lines;
    }
    for file in &diff.files {
        let status = match file.status {
            FileStatus::Added => "added",
            FileStatus::Removed => "removed",
            FileStatus::Modified => "modified",
        };
        lines.push(Line::from(Span::raw("")));
        lines.push(Line::from(Span::styled(
            format!(
                "\u{2500}\u{2500} {} [{}] \u{2500}\u{2500}",
                file.path, status
            ),
            theme.selected(),
        )));
        if file.binary {
            lines.push(Line::from(Span::styled(
                "Binary file differs",
                theme.muted(),
            )));
            continue;
        }
        for hunk in &file.hunks {
            lines.push(Line::from(Span::styled(
                format!(
                    "@@ -{},{} +{},{} @@",
                    hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
                ),
                theme.muted(),
            )));
            for line in &hunk.lines {
                let (prefix, style) = match line.kind {
                    LineKind::Context => (" ", theme.muted()),
                    LineKind::Insert => ("+", theme.success()),
                    LineKind::Delete => ("-", theme.danger()),
                };
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{}", line.text),
                    style,
                )));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreg_client::client::PackagePreview;

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

    #[test]
    fn preview_state_starts_not_loaded() {
        let s = DetailState::new();
        assert!(matches!(s.preview, PreviewState::NotLoaded));
    }

    #[test]
    fn preview_state_set_to_loading() {
        let mut s = DetailState::new();
        s.preview = PreviewState::Loading;
        assert!(matches!(s.preview, PreviewState::Loading));
    }

    #[test]
    fn version_selection_moves_and_clamps() {
        let mut s = DetailState::new();
        s.versions = vec!["2.0.0".into(), "1.0.0".into()];
        assert_eq!(s.selected_version, 0);
        s.select_next_version();
        assert_eq!(s.selected_version, 1);
        s.select_next_version(); // clamp at last
        assert_eq!(s.selected_version, 1);
        s.select_prev_version();
        assert_eq!(s.selected_version, 0);
        s.select_prev_version(); // clamp at first
        assert_eq!(s.selected_version, 0);
    }

    #[test]
    fn preview_state_loaded_holds_data() {
        let p = PackagePreview {
            files: vec!["SKILL.md".to_string()],
            skill_md: "# hello".to_string(),
            truncated: false,
        };
        let mut s = DetailState::new();
        s.preview = PreviewState::Loaded(p);
        if let PreviewState::Loaded(ref data) = s.preview {
            assert_eq!(data.files.len(), 1);
        } else {
            panic!("expected Loaded");
        }
    }

    #[test]
    fn diff_lines_render_headers_and_changes() {
        use skreg_client::client::{DiffLine, FileDiff, FileStatus, Hunk, LineKind, SkillDiff};
        let diff = SkillDiff {
            from: "1.0.0".into(),
            to: "1.0.1".into(),
            files: vec![FileDiff {
                path: "SKILL.md".into(),
                status: FileStatus::Modified,
                binary: false,
                hunks: vec![Hunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    lines: vec![
                        DiffLine {
                            kind: LineKind::Delete,
                            text: "old".into(),
                        },
                        DiffLine {
                            kind: LineKind::Insert,
                            text: "new".into(),
                        },
                    ],
                }],
            }],
        };
        let theme = Theme::default();
        let lines = diff_lines(&diff, &theme);
        let text: String = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("SKILL.md"));
        assert!(text.contains("-old"));
        assert!(text.contains("+new"));
    }
}
