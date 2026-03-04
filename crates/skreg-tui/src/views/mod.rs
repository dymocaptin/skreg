//! Full-screen views and the `View` trait.

use ratatui::crossterm::event::Event;
use ratatui::{layout::Rect, Frame};

use crate::theme::Theme;

pub mod context;
pub mod detail;
pub mod installed;
pub mod packages;

pub use packages::PackageListView;

/// A full-screen view that can render itself and respond to input.
pub trait View {
    /// Render this view into `area`.
    fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme);
    /// Handle a terminal event and return the resulting action.
    fn handle_event(&mut self, event: Event) -> Action;
    /// Called once per event-loop tick (default: no-op).
    fn tick(&mut self) {}
}

/// Actions returned by views to drive the app event loop.
pub enum Action {
    /// No state change.
    None,
    /// Push a new view onto the stack.
    Push(Box<dyn View>),
    /// Pop the current view.
    Pop,
    /// Quit the application.
    Quit,
    /// Display a toast notification.
    Toast(ToastKind, String),
    /// Open the context switcher overlay.
    OpenContextSwitcher,
    /// Switch to the named context and reload.
    SwitchContext(String),
}

/// Toast notification severity.
pub enum ToastKind {
    /// Operation succeeded.
    Success,
    /// Operation failed.
    Error,
}
