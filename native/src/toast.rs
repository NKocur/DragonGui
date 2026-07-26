use crate::document::WidgetKind;
use crate::layout::Rect;
use crate::paint::{native_widget_paint_fallback_with_level, PaintInteraction};
use crate::style::TextStyle;
use crate::text::measure_text_for_layout;
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
    text_style: &TextStyle,
    theme: &Theme,
) -> Rect {
    let margin_x = (16.0 * sf).min(window_w.max(0.0) * 0.5);
    let margin_y = (16.0 * sf).min(window_h.max(0.0) * 0.5);
    let gap = 8.0 * sf;
    let pad = toast_padding(padding, sf);
    let available_w = (window_w - margin_x * 2.0).max(0.0);
    let available_h = (window_h - margin_y * 2.0).max(0.0);
    let min_w = (220.0 * sf).min(available_w);
    let max_w = available_w.min(420.0 * sf);
    let estimated_w = measure_text_for_layout(message, text_style, theme)
        .width
        .ceil()
        * sf
        + pad * 2.0;
    let width = if max_w > 0.0 {
        estimated_w.clamp(min_w, max_w)
    } else {
        0.0
    };
    let height = (DEFAULT_TOAST_HEIGHT_TEXT_LP * sf + pad * 2.0)
        .max(44.0 * sf)
        .min(available_h);
    let x = match position {
        ToastPosition::TopLeft | ToastPosition::BottomLeft => margin_x,
        ToastPosition::TopRight | ToastPosition::BottomRight => {
            (window_w - margin_x - width).max(margin_x)
        }
    };
    let stack_offset = index as f32 * (height + gap);
    let (y, fits_stack) = match position {
        ToastPosition::TopLeft | ToastPosition::TopRight => {
            let y = margin_y + stack_offset;
            (y, y + height <= window_h - margin_y + 0.001)
        }
        ToastPosition::BottomLeft | ToastPosition::BottomRight => {
            let y = window_h - margin_y - height - stack_offset;
            (y, y + 0.001 >= margin_y)
        }
    };
    Rect {
        x,
        y,
        w: width,
        h: if fits_stack { height } else { 0.0 },
    }
}

pub(crate) fn toast_colors(level: ToastLevel, theme: &Theme, opacity: f32) -> ToastColors {
    let fallback = native_widget_paint_fallback_with_level(
        WidgetKind::Toast,
        Some(level.as_str()),
        theme,
        PaintInteraction::Resting,
    );
    let opacity = opacity.clamp(0.0, 1.0);
    ToastColors {
        fill: with_alpha(fallback.background.unwrap_or(theme.surface), opacity),
        border: with_alpha(fallback.border_color.unwrap_or(theme.border), opacity),
        text: with_alpha(theme.text, opacity),
    }
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
        let first = toast_rect(
            0,
            "Saved",
            800.0,
            600.0,
            1.0,
            ToastPosition::TopRight,
            None,
            &TextStyle::default(),
            &Theme::dark(),
        );
        let second = toast_rect(
            1,
            "Saved",
            800.0,
            600.0,
            1.0,
            ToastPosition::TopRight,
            None,
            &TextStyle::default(),
            &Theme::dark(),
        );

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
            &TextStyle::default(),
            &Theme::dark(),
        );
        let second = toast_rect(
            1,
            "Saved",
            800.0,
            600.0,
            1.0,
            ToastPosition::BottomLeft,
            None,
            &TextStyle::default(),
            &Theme::dark(),
        );

        assert_eq!(first.x, 16.0);
        assert!(second.y < first.y);
    }

    #[test]
    fn toast_rect_stays_inside_small_viewport() {
        let rect = toast_rect(
            0,
            "A message that would normally produce a wide toast",
            140.0,
            80.0,
            1.0,
            ToastPosition::TopRight,
            None,
            &TextStyle::default(),
            &Theme::dark(),
        );

        assert!(rect.x >= 0.0 && rect.y >= 0.0);
        assert!(rect.x + rect.w <= 140.0);
        assert!(rect.y + rect.h <= 80.0);
    }

    #[test]
    fn toast_stack_hides_entries_that_cannot_fit_without_overlap() {
        let first = toast_rect(
            0,
            "Saved",
            240.0,
            100.0,
            1.0,
            ToastPosition::BottomRight,
            None,
            &TextStyle::default(),
            &Theme::dark(),
        );
        let second = toast_rect(
            1,
            "Saved again",
            240.0,
            100.0,
            1.0,
            ToastPosition::BottomRight,
            None,
            &TextStyle::default(),
            &Theme::dark(),
        );

        assert!(first.h > 0.0);
        assert_eq!(second.h, 0.0);
    }

    #[test]
    fn toast_colors_apply_opacity() {
        let colors = toast_colors(ToastLevel::Info, &Theme::dark(), 0.42);
        assert!((colors.fill[3] - 0.42).abs() < f32::EPSILON);
        assert!((colors.border[3] - 0.42).abs() < f32::EPSILON);
        assert!((colors.text[3] - 0.42).abs() < f32::EPSILON);
    }
}
