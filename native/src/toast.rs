use crate::layout::Rect;
use crate::theme::{Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "info" | "Info" => Some(Self::Info),
            "success" | "Success" => Some(Self::Success),
            "warning" | "Warning" => Some(Self::Warning),
            "error" | "Error" => Some(Self::Error),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToastOverlay {
    pub id: String,
    pub message: String,
    pub level: ToastLevel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ToastColors {
    pub fill: Color,
    pub border: Color,
    pub text: Color,
}

pub(crate) fn toast_rect(
    index: usize,
    message: &str,
    window_w: f32,
    _window_h: f32,
    sf: f32,
) -> Rect {
    let margin = 16.0 * sf;
    let gap = 8.0 * sf;
    let min_w = 220.0 * sf;
    let max_w = (window_w - margin * 2.0).max(min_w).min(420.0 * sf);
    let estimated_w = message.chars().count() as f32 * 7.5 * sf + 36.0 * sf;
    let width = estimated_w.clamp(min_w, max_w);
    let height = 52.0 * sf;
    let x = (window_w - margin - width).max(margin);
    let y = margin + index as f32 * (height + gap);
    Rect {
        x,
        y,
        w: width,
        h: height,
    }
}

pub(crate) fn toast_colors(level: ToastLevel, theme: &Theme) -> ToastColors {
    let accent = match level {
        ToastLevel::Info => theme.accent,
        ToastLevel::Success => theme.success,
        ToastLevel::Warning => theme.warning,
        ToastLevel::Error => theme.danger,
    };
    ToastColors {
        fill: mix(theme.surface, accent, 0.18),
        border: mix(theme.border, accent, 0.62),
        text: theme.text,
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_level_parses_public_names() {
        assert_eq!(ToastLevel::from_str("info"), Some(ToastLevel::Info));
        assert_eq!(ToastLevel::from_str("success"), Some(ToastLevel::Success));
        assert_eq!(ToastLevel::from_str("warning"), Some(ToastLevel::Warning));
        assert_eq!(ToastLevel::from_str("error"), Some(ToastLevel::Error));
        assert_eq!(ToastLevel::from_str("debug"), None);
    }

    #[test]
    fn toast_rect_stacks_inside_window() {
        let first = toast_rect(0, "Saved", 800.0, 600.0, 1.0);
        let second = toast_rect(1, "Saved", 800.0, 600.0, 1.0);

        assert!(first.x >= 16.0);
        assert!(first.x + first.w <= 800.0 - 16.0);
        assert!(second.y > first.y + first.h);
    }
}
