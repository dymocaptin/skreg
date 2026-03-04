//! Toast notification widget.

use ratatui::{layout::Rect, Frame};

use crate::theme::Theme;
use crate::views::ToastKind;

/// A transient notification shown in the top-right corner.
pub struct Toast {
    /// Severity of the notification.
    pub kind: ToastKind,
    /// Message text.
    pub message: String,
}

/// Render `toast` in the top-right corner of `area`.
pub fn render_toast(_frame: &mut Frame, _area: Rect, _toast: &Toast, _theme: &Theme) {}
