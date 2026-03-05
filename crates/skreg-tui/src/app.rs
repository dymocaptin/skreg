//! Application state and top-level event loop.

use std::io;
use std::time::Duration;

use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::Terminal;
use skreg_core::config::{default_config_path, save_config, CliConfig};

use crate::theme::Theme;
use crate::views::{Action, PackageListView, View};
use crate::widgets::toast::Toast;

/// Start the TUI event loop.
///
/// # Errors
/// Returns an error if terminal initialisation fails or an I/O error occurs.
pub fn run(config: CliConfig) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, config);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut config: CliConfig,
) -> Result<()> {
    let theme = Theme::default();
    let mut stack: Vec<Box<dyn View>> = vec![Box::new(PackageListView::new(config.clone()))];
    let mut toast: Option<(Toast, std::time::Instant)> = None;
    let mut context_overlay: Option<crate::views::context::ContextOverlay> = None;
    let mut show_help = false;

    loop {
        for view in &mut stack {
            if let Some(Action::Toast(kind, msg)) = view.tick() {
                toast = Some((Toast { kind, message: msg }, std::time::Instant::now()));
            }
        }

        terminal.draw(|frame| {
            let area = frame.area();
            if let Some(view) = stack.last_mut() {
                view.render(frame, area, &theme);
            }
            if let Some(overlay) = &context_overlay {
                overlay.render(frame, area, &theme);
            }
            if let Some((t, _)) = &toast {
                crate::widgets::toast::render_toast(frame, area, t, &theme);
            }
            if show_help {
                crate::widgets::help::render_help(frame, area, &theme);
            }
        })?;

        // Expire toasts after 3 seconds.
        if let Some((_, started)) = &toast {
            if started.elapsed() > Duration::from_secs(3) {
                toast = None;
            }
        }

        if event::poll(Duration::from_millis(50))? {
            let ev = event::read()?;

            // Ctrl+C always quits.
            if matches!(&ev, Event::Key(k)
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
            {
                break;
            }

            // ? toggles help from anywhere; help overlay consumes all other keys while open.
            if matches!(&ev, Event::Key(k) if k.code == KeyCode::Char('?')) {
                show_help = !show_help;
                continue;
            }
            if show_help {
                if matches!(&ev, Event::Key(k)
                    if matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')))
                {
                    show_help = false;
                }
                continue;
            }

            toast = None;

            if let Some(overlay) = &mut context_overlay {
                match overlay.handle_event(&ev) {
                    Action::Pop => {
                        context_overlay = None;
                    }
                    Action::SwitchContext(name) => {
                        context_overlay = None;
                        config.active_context = name;
                        save_config(&config, &default_config_path())?;
                        stack = vec![Box::new(PackageListView::new(config.clone()))];
                    }
                    _ => {}
                }
            } else if let Some(view) = stack.last_mut() {
                match view.handle_event(ev) {
                    Action::Push(v) => stack.push(v),
                    Action::Pop | Action::Quit if stack.len() == 1 => break,
                    Action::Pop => {
                        stack.pop();
                    }
                    Action::Quit => break,
                    Action::Toast(kind, msg) => {
                        toast = Some((Toast { kind, message: msg }, std::time::Instant::now()));
                    }
                    Action::OpenContextSwitcher => {
                        context_overlay =
                            Some(crate::views::context::ContextOverlay::new(config.clone()));
                    }
                    Action::None | Action::SwitchContext(_) => {}
                }
            }
        }
    }

    Ok(())
}
