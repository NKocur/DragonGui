pub(crate) const FOCUS_RING_LP: f32 = 2.0;
pub(crate) const PANEL_ACCENT_WIDTH_LP: f32 = 3.0;
pub(crate) const BORDER_WIDTH_LP: f32 = 1.0;

pub(crate) const CARET_WIDTH_LP: f32 = 1.5;

pub(crate) const CHECKBOX_BOX_LP: f32 = 18.0;
pub(crate) const CHECKBOX_LEFT_PAD_LP: f32 = 6.0;

pub(crate) const DROPDOWN_CHEVRON_WIDTH_LP: f32 = 8.0;

pub(crate) const SLIDER_TRACK_MARGIN_LP: f32 = 8.0;
pub(crate) const SLIDER_TRACK_HEIGHT_LP: f32 = 4.0;
pub(crate) const SLIDER_THUMB_WIDTH_LP: f32 = 16.0;

pub(crate) const TAB_GAP_LP: f32 = 8.0;
pub(crate) const TAB_TOP_INSET_LP: f32 = 4.0;
pub(crate) const TAB_INACTIVE_BOTTOM_INSET_LP: f32 = 3.0;
pub(crate) const TAB_ACTIVE_BAR_LP: f32 = 3.0;

use serde_json::Value;

use crate::theme::{Color, Theme};

#[derive(Debug, Clone, Default)]
pub struct NodeStyle {
    pub layout: LayoutStyle,
    pub visual: VisualStyle,
    pub text: TextStyle,
    pub hover: VisualStyle,
    pub active: VisualStyle,
    pub focus: VisualStyle,
    pub disabled: VisualStyle,
}

#[derive(Debug, Clone, Default)]
pub struct LayoutStyle {
    pub display: Option<DisplayStyle>,
    pub flex_direction: Option<FlexDirectionStyle>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub padding: Option<f32>,
    pub padding_left: Option<f32>,
    pub padding_right: Option<f32>,
    pub padding_top: Option<f32>,
    pub padding_bottom: Option<f32>,
    pub margin: Option<f32>,
    pub gap: Option<f32>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayStyle {
    Flex,
    Block,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirectionStyle {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Debug, Clone, Default)]
pub struct VisualStyle {
    pub background: Option<ColorRef>,
    pub foreground: Option<ColorRef>,
    pub border_color: Option<ColorRef>,
    pub border_width: Option<f32>,
    pub border_radius: Option<f32>,
    pub accent: Option<ColorRef>,
    pub track_color: Option<ColorRef>,
    pub thumb_color: Option<ColorRef>,
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Default)]
pub struct TextStyle {
    pub font_size: Option<f32>,
    pub font_family: Option<FontFamily>,
    pub font_weight: Option<u16>,
    pub color: Option<ColorRef>,
    pub text_align: Option<TextAlign>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Serif,
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Name(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColorRef {
    Rgba(Color),
    Token(String),
}

impl NodeStyle {
    pub fn from_json(value: Option<&Value>) -> Self {
        let Some(Value::Object(map)) = value else {
            return Self::default();
        };
        let mut style = Self::default();
        parse_layout(map, &mut style.layout);
        parse_visual(map, &mut style.visual);
        parse_text(map, &mut style.text);
        style.hover = nested_visual(map.get("hover"));
        style.active = nested_visual(map.get("active"));
        style.focus = nested_visual(map.get("focus"));
        style.disabled = nested_visual(map.get("disabled"));
        style
    }
}

impl VisualStyle {
    pub fn merged(&self, other: &VisualStyle) -> VisualStyle {
        VisualStyle {
            background: other.background.clone().or_else(|| self.background.clone()),
            foreground: other.foreground.clone().or_else(|| self.foreground.clone()),
            border_color: other
                .border_color
                .clone()
                .or_else(|| self.border_color.clone()),
            border_width: other.border_width.or(self.border_width),
            border_radius: other.border_radius.or(self.border_radius),
            accent: other.accent.clone().or_else(|| self.accent.clone()),
            track_color: other
                .track_color
                .clone()
                .or_else(|| self.track_color.clone()),
            thumb_color: other
                .thumb_color
                .clone()
                .or_else(|| self.thumb_color.clone()),
            opacity: other.opacity.or(self.opacity),
        }
    }
}

impl ColorRef {
    pub fn resolve(&self, theme: &Theme) -> Color {
        match self {
            ColorRef::Rgba(color) => *color,
            ColorRef::Token(token) => resolve_token(token, theme).unwrap_or_else(|| {
                eprintln!("DragonGUI: unknown color token {token:?}; using danger fallback");
                theme.danger
            }),
        }
    }
}

fn parse_layout(map: &serde_json::Map<String, Value>, out: &mut LayoutStyle) {
    out.display = map
        .get("display")
        .and_then(Value::as_str)
        .and_then(parse_display);
    out.flex_direction = map
        .get("flex_direction")
        .and_then(Value::as_str)
        .and_then(parse_flex_direction);
    out.width = number(map.get("width"));
    out.height = number(map.get("height"));
    out.min_width = number(map.get("min_width"));
    out.min_height = number(map.get("min_height"));
    out.max_width = number(map.get("max_width"));
    out.max_height = number(map.get("max_height"));
    out.padding = number(map.get("padding"));
    out.padding_left = number(map.get("padding_left"));
    out.padding_right = number(map.get("padding_right"));
    out.padding_top = number(map.get("padding_top"));
    out.padding_bottom = number(map.get("padding_bottom"));
    out.margin = number(map.get("margin"));
    out.gap = number(map.get("gap"));
    out.flex_grow = number(map.get("flex_grow")).or_else(|| number(map.get("flex")));
    out.flex_shrink = number(map.get("flex_shrink"));
}

fn parse_visual(map: &serde_json::Map<String, Value>, out: &mut VisualStyle) {
    out.background = color_ref(map.get("background"));
    out.foreground = color_ref(map.get("foreground")).or_else(|| color_ref(map.get("color")));
    out.border_color = color_ref(map.get("border_color"));
    out.border_width = number(map.get("border_width"));
    out.border_radius = number(map.get("border_radius"));
    out.accent = color_ref(map.get("accent"));
    out.track_color = color_ref(map.get("track_color"));
    out.thumb_color = color_ref(map.get("thumb_color"));
    out.opacity = number(map.get("opacity")).map(|v| v.clamp(0.0, 1.0));
}

fn parse_text(map: &serde_json::Map<String, Value>, out: &mut TextStyle) {
    out.font_size = number(map.get("font_size"));
    out.font_family = map
        .get("font_family")
        .and_then(Value::as_str)
        .and_then(parse_font_family);
    out.font_weight = map.get("font_weight").and_then(parse_font_weight);
    out.color = color_ref(map.get("color")).or_else(|| color_ref(map.get("foreground")));
    out.text_align = map
        .get("text_align")
        .and_then(Value::as_str)
        .and_then(parse_text_align);
}

fn nested_visual(value: Option<&Value>) -> VisualStyle {
    let Some(Value::Object(map)) = value else {
        return VisualStyle::default();
    };
    let mut style = VisualStyle::default();
    parse_visual(map, &mut style);
    style
}

fn parse_text_align(value: &str) -> Option<TextAlign> {
    match value {
        "left" | "start" => Some(TextAlign::Left),
        "center" | "middle" => Some(TextAlign::Center),
        "right" | "end" => Some(TextAlign::Right),
        _ => None,
    }
}

fn parse_display(value: &str) -> Option<DisplayStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "flex" => Some(DisplayStyle::Flex),
        "block" => Some(DisplayStyle::Block),
        "none" => Some(DisplayStyle::None),
        _ => None,
    }
}

fn parse_flex_direction(value: &str) -> Option<FlexDirectionStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "row" => Some(FlexDirectionStyle::Row),
        "column" => Some(FlexDirectionStyle::Column),
        "row_reverse" | "row-reverse" => Some(FlexDirectionStyle::RowReverse),
        "column_reverse" | "column-reverse" => Some(FlexDirectionStyle::ColumnReverse),
        _ => None,
    }
}

fn parse_font_family(value: &str) -> Option<FontFamily> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    match value.to_ascii_lowercase().as_str() {
        "serif" => Some(FontFamily::Serif),
        "sans" | "sans-serif" | "sans_serif" | "system" => Some(FontFamily::SansSerif),
        "mono" | "monospace" => Some(FontFamily::Monospace),
        "cursive" => Some(FontFamily::Cursive),
        "fantasy" => Some(FontFamily::Fantasy),
        _ => Some(FontFamily::Name(value.to_string())),
    }
}

fn parse_font_weight(value: &Value) -> Option<u16> {
    if let Some(n) = value.as_u64() {
        return Some((n as u16).clamp(100, 900));
    }
    let text = value.as_str()?.trim().to_ascii_lowercase();
    match text.as_str() {
        "thin" => Some(100),
        "extra_light" | "extra-light" | "ultralight" => Some(200),
        "light" => Some(300),
        "normal" | "regular" => Some(400),
        "medium" => Some(500),
        "semibold" | "semi-bold" | "demibold" | "demi-bold" => Some(600),
        "bold" => Some(700),
        "extra_bold" | "extra-bold" | "ultrabold" => Some(800),
        "black" | "heavy" => Some(900),
        _ => text.parse::<u16>().ok().map(|n| n.clamp(100, 900)),
    }
}

fn number(value: Option<&Value>) -> Option<f32> {
    value.and_then(Value::as_f64).map(|v| v as f32)
}

fn color_ref(value: Option<&Value>) -> Option<ColorRef> {
    match value? {
        Value::String(s) => parse_hex_color(s)
            .map(ColorRef::Rgba)
            .or_else(|| Some(ColorRef::Token(s.to_string()))),
        Value::Array(items) if items.len() == 3 || items.len() == 4 => {
            let r = items.first()?.as_f64()? as f32;
            let g = items.get(1)?.as_f64()? as f32;
            let b = items.get(2)?.as_f64()? as f32;
            let a = items.get(3).and_then(Value::as_f64).unwrap_or(1.0) as f32;
            Some(ColorRef::Rgba([
                normalize_channel(r),
                normalize_channel(g),
                normalize_channel(b),
                a.clamp(0.0, 1.0),
            ]))
        }
        _ => None,
    }
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    Some([r, g, b, 1.0])
}

fn normalize_channel(value: f32) -> f32 {
    if value > 1.0 {
        (value / 255.0).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn resolve_token(token: &str, theme: &Theme) -> Option<Color> {
    match token {
        "background" => Some(theme.background),
        "surface" => Some(theme.surface),
        "surface_alt" => Some(theme.surface_alt),
        "text" | "foreground" => Some(theme.text),
        "muted_text" | "muted" => Some(theme.muted_text),
        "accent" => Some(theme.accent),
        "border" => Some(theme.border),
        "danger" => Some(theme.danger),
        "warning" => Some(theme.warning),
        "success" => Some(theme.success),
        "focus" => Some(theme.focus),
        "disabled" => Some(theme.disabled),
        "accent_mix_20" => Some(mix(theme.surface_alt, theme.accent, 0.20)),
        "accent_mix_12" => Some(mix(theme.surface_alt, theme.accent, 0.12)),
        "accent_dark" => Some(mix(theme.accent, [0.0, 0.0, 0.0, theme.accent[3]], 0.15)),
        _ => None,
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
    use serde_json::json;

    #[test]
    fn parses_layout_visual_and_pseudo_style() {
        let style = NodeStyle::from_json(Some(&json!({
            "width": 240,
            "display": "flex",
            "flex_direction": "row",
            "padding": 12,
            "background": "surface_alt",
            "border_color": "#33ffaa",
            "border_radius": 9,
            "track_color": "border",
            "thumb_color": "accent",
            "font_size": 18,
            "font_family": "monospace",
            "font_weight": "bold",
            "color": "accent",
            "text_align": "center",
            "hover": {"background": "accent_mix_20", "color": "success"}
        })));

        assert_eq!(style.layout.width, Some(240.0));
        assert_eq!(style.layout.display, Some(DisplayStyle::Flex));
        assert_eq!(style.layout.flex_direction, Some(FlexDirectionStyle::Row));
        assert_eq!(style.layout.padding, Some(12.0));
        assert_eq!(
            style.visual.border_color,
            Some(ColorRef::Rgba([
                0x33 as f32 / 255.0,
                1.0,
                0xaa as f32 / 255.0,
                1.0
            ]))
        );
        assert_eq!(
            style.hover.background,
            Some(ColorRef::Token("accent_mix_20".to_string()))
        );
        assert_eq!(
            style.hover.foreground,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(
            style.visual.track_color,
            Some(ColorRef::Token("border".to_string()))
        );
        assert_eq!(
            style.visual.thumb_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(style.text.font_size, Some(18.0));
        assert_eq!(style.text.font_family, Some(FontFamily::Monospace));
        assert_eq!(style.text.font_weight, Some(700));
        assert_eq!(
            style.text.color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(style.text.text_align, Some(TextAlign::Center));
    }
}
