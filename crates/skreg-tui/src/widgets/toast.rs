//! Toast notification widget.

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

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
pub fn render_toast(frame: &mut Frame, area: Rect, toast: &Toast, theme: &Theme) {
    let (icon, style) = match toast.kind {
        ToastKind::Success => ("✓", theme.success()),
        ToastKind::Error => ("✗", theme.danger()),
    };
    let width: u16 = 36;
    let height: u16 = 3;
    if area.width < width || area.height < height {
        return;
    }

    let x = area.right().saturating_sub(width + 1);
    let y = area.top() + 1;
    let toast_area = Rect::new(x, y, width, height);

    let max_msg = width.saturating_sub(6) as usize;
    let msg = if toast.message.len() > max_msg {
        &toast.message[..max_msg]
    } else {
        &toast.message
    };

    let text = Line::from(vec![
        Span::styled(format!(" {icon} "), style),
        Span::raw(msg),
    ]);

    frame.render_widget(Clear, toast_area);
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).border_style(style)),
        toast_area,
    );
}
