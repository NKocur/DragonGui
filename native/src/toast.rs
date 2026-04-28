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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastPosition {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl Default for ToastPosition {
    fn default() -> Self {
        Self::TopRight
    }
}

impl ToastPosition {
    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "top-right" | "top_right" | "TopRight" => Some(Self::TopRight),
            "top-left" | "top_left" | "TopLeft" => Some(Self::TopLeft),
            "bottom-right" | "bottom_right" | "BottomRight" => Some(Self::BottomRight),
            "bottom-left" | "bottom_left" | "BottomLeft" => Some(Self::BottomLeft),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TopRight => "top-right",
            Self::TopLeft => "top-left",
            Self::BottomRight => "bottom-right",
            Self::BottomLeft => "bottom-left",
        }
    }

    pub(crate) fn stack_slot(self) -> usize {
        match self {
            Self::TopRight => 0,
            Self::TopLeft => 1,
            Self::BottomRight => 2,
            Self::BottomLeft => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToastOverlay {
    pub id: String,
    pub message: String,
    pub level: ToastLevel,
    pub opacity: f32,
    pub radius: Option<f32>,
    pub padding: Option<f32>,
    pub position: ToastPosition,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ToastColors {
    pub fill: Color,
    pub border: Color,
    pub text: Color,
}

const DEFAULT_TOAST_PADDING_LP: f32 = 14.0;
const DEFAULT_TOAST_HEIGHT_TEXT_LP: f32 = 24.0;

pub(crate) fn toast_padding(padding: Option<f32>, sf: f32) -> f32 {
    padding.unwrap_or(DEFAULT_TOAST_PADDING_LP).max(0.0) * sf
}

pub(crate) fn toast_stack_index(position: ToastPosition, counters: &mut [usize; 4]) -> usize {
    let slot = position.stack_slot();
    let index = counters[slot];
    counters[slot] += 1;
    index
}

pub(crate) fn toast_rect(
    index: usize,
    message: &str,
    window_w: f32,
    window_h: f32,
    sf: f32,
    position: ToastPosition,
    padding: Option<f32>,
) -> Rect {
    let margin = 16.0 * sf;
    let gap = 8.0 * sf;
    let pad = toast_padding(padding, sf);
    let min_w = 220.0 * sf;
    let max_w = (window_w - margin * 2.0).max(min_w).min(420.0 * sf);
    let estimated_w = message.chars().count() as f32 * 7.5 * sf + pad * 2.0;
    let width = estimated_w.clamp(min_w, max_w);
    let height = (DEFAULT_TOAST_HEIGHT_TEXT_LP * sf + pad * 2.0).max(44.0 * sf);
    let x = match position {
        ToastPosition::TopLeft | ToastPosition::BottomLeft => margin,
        ToastPosition::TopRight | ToastPosition::BottomRight => {
            (window_w - margin - width).max(margin)
        }
    };
    let stack_offset = index as f32 * (height + gap);
    let y = match position {
        ToastPosition::TopLeft | ToastPosition::TopRight => margin + stack_offset,
        ToastPosition::BottomLeft | ToastPosition::BottomRight => {
            (window_h - margin - height - stack_offset).max(margin)
        }
    };
    Rect {
        x,
        y,
        w: width,
        h: height,
    }
}

pub(crate) fn toast_colors(level: ToastLevel, theme: &Theme, opacity: f32) -> ToastColors {
    let accent = match level {
        ToastLevel::Info => theme.accent,
        ToastLevel::Success => theme.success,
        ToastLevel::Warning => theme.warning,
        ToastLevel::Error => theme.danger,
    };
    let opacity = opacity.clamp(0.0, 1.0);
    ToastColors {
        fill: with_alpha(mix(theme.surface, accent, 0.18), opacity),
        border: with_alpha(mix(theme.border, accent, 0.62), opacity),
        text: with_alpha(theme.text, opacity),
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

fn with_alpha(mut color: Color, opacity: f32) -> Color {
    color[3] *= opacity.clamp(0.0, 1.0);
    color
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
        let first = toast_rect(0, "Saved", 800.0, 600.0, 1.0, ToastPosition::TopRight, None);
        let second = toast_rect(1, "Saved", 800.0, 600.0, 1.0, ToastPosition::TopRight, None);

        assert!(first.x >= 16.0);
        assert!(first.x + first.w <= 800.0 - 16.0);
        assert!(second.y > first.y + first.h);
    }

    #[test]
    fn toast_rect_supports_bottom_left_stack() {
        let first = toast_rect(
            0,
            "Saved",
            800.0,
            600.0,
            1.0,
            ToastPosition::BottomLeft,
            None,
        );
        let second = toast_rect(
            1,
            "Saved",
            800.0,
            600.0,
            1.0,
            ToastPosition::BottomLeft,
            None,
        );

        assert_eq!(first.x, 16.0);
        assert!(second.y < first.y);
    }

    #[test]
    fn toast_colors_apply_opacity() {
        let colors = toast_colors(ToastLevel::Info, &Theme::dark(), 0.42);
        assert!((colors.fill[3] - 0.42).abs() < f32::EPSILON);
        assert!((colors.border[3] - 0.42).abs() < f32::EPSILON);
        assert!((colors.text[3] - 0.42).abs() < f32::EPSILON);
    }
}
