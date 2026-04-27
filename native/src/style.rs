use std::collections::BTreeMap;

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

pub(crate) const NUMBER_STEPPER_WIDTH_LP: f32 = 26.0;

pub(crate) fn number_stepper_width(widget_width: f32, sf: f32) -> f32 {
    (NUMBER_STEPPER_WIDTH_LP * sf).min(widget_width * 0.45)
}

pub(crate) fn number_stepper_width_for_style(style: &NodeStyle, widget_width: f32, sf: f32) -> f32 {
    let width_lp = style
        .parts
        .parts
        .get("stepper")
        .and_then(|part| part.layout.width)
        .unwrap_or(NUMBER_STEPPER_WIDTH_LP);
    (width_lp.max(1.0) * sf).min(widget_width * 0.45)
}

pub(crate) const TAB_GAP_LP: f32 = 8.0;
pub(crate) const TAB_TOP_INSET_LP: f32 = 4.0;
pub(crate) const TAB_INACTIVE_BOTTOM_INSET_LP: f32 = 3.0;
pub(crate) const TAB_ACTIVE_BAR_LP: f32 = 3.0;

pub(crate) const BADGE_GAP_LP: f32 = 8.0;
pub(crate) const BADGE_PAD_X_LP: f32 = 6.0;
pub(crate) const BADGE_MIN_HEIGHT_LP: f32 = 16.0;

pub(crate) fn badge_font_size_lp(style: &NodeStyle, theme: &Theme) -> f32 {
    style
        .parts
        .parts
        .get("badge")
        .and_then(|part| part.text.font_size)
        .unwrap_or_else(|| (theme.font_size - 2.0).max(10.0))
        .max(8.0)
}

pub(crate) fn badge_height_for_style(style: &NodeStyle, theme: &Theme, sf: f32) -> f32 {
    let height_lp = style
        .parts
        .parts
        .get("badge")
        .and_then(|part| part.layout.height)
        .unwrap_or_else(|| (badge_font_size_lp(style, theme) + 6.0).max(BADGE_MIN_HEIGHT_LP));
    (height_lp.max(1.0) * sf).max(1.0)
}

pub(crate) fn badge_width_for_text(style: &NodeStyle, badge: &str, theme: &Theme, sf: f32) -> f32 {
    if let Some(width_lp) = style
        .parts
        .parts
        .get("badge")
        .and_then(|part| part.layout.width)
    {
        return (width_lp.max(1.0) * sf).max(1.0);
    }
    let font_size = badge_font_size_lp(style, theme);
    let text_w = badge.chars().count() as f32 * font_size * 0.58;
    ((text_w + BADGE_PAD_X_LP * 2.0).max(BADGE_MIN_HEIGHT_LP) * sf).max(1.0)
}

pub(crate) fn tabs_header_height_for_style(style: &NodeStyle, theme: &Theme, sf: f32) -> f32 {
    let height_lp = style
        .parts
        .parts
        .get("header")
        .and_then(|part| part.layout.height)
        .unwrap_or_else(|| theme.control_height());
    (height_lp.max(1.0) * sf).max(1.0)
}

pub(crate) fn collapsible_header_height_for_style(
    style: &NodeStyle,
    theme: &Theme,
    sf: f32,
) -> f32 {
    let height_lp = style
        .parts
        .parts
        .get("header")
        .and_then(|part| part.layout.height)
        .unwrap_or_else(|| theme.control_height());
    (height_lp.max(1.0) * sf).max(1.0)
}

use serde_json::Value;

use crate::events::WidgetState;
use crate::theme::{parse_hex_color, Color, Theme};

#[derive(Debug, Clone, Default)]
pub struct NodeStyle {
    pub layout: LayoutStyle,
    pub visual: VisualStyle,
    pub text: TextStyle,
    pub widget: WidgetStyle,
    pub parts: NodePartStyles,
    pub hover: VisualStyle,
    pub active: VisualStyle,
    pub focus: VisualStyle,
    pub disabled: VisualStyle,
    pub checked: VisualStyle,
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
    pub corner_radii: CornerRadii,
    pub accent: Option<ColorRef>,
    pub track_color: Option<ColorRef>,
    pub thumb_color: Option<ColorRef>,
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: Option<f32>,
    pub top_right: Option<f32>,
    pub bottom_right: Option<f32>,
    pub bottom_left: Option<f32>,
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

#[derive(Debug, Clone, Default)]
pub struct WidgetStyle {
    pub table_row_height: Option<f32>,
    pub table_header_height: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct PartLayoutStyle {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub padding: Option<f32>,
    pub gap: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct PartStyle {
    pub layout: PartLayoutStyle,
    pub visual: VisualStyle,
    pub text: TextStyle,
}

#[derive(Debug, Clone, Default)]
pub struct NodePartStyles {
    pub parts: BTreeMap<String, PartStyle>,
    pub hover: BTreeMap<String, PartStyle>,
    pub active: BTreeMap<String, PartStyle>,
    pub focus: BTreeMap<String, PartStyle>,
    pub disabled: BTreeMap<String, PartStyle>,
    pub checked: BTreeMap<String, PartStyle>,
}

pub(crate) fn base_part_style<'a>(style: &'a NodeStyle, part: &str) -> Option<&'a PartStyle> {
    style.parts.parts.get(part)
}

pub(crate) fn checked_part_style_for_state<'a>(
    style: &'a NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    part: &str,
) -> Option<&'a PartStyle> {
    if state.checked.get(widget_id).copied().unwrap_or(false) {
        style.parts.checked.get(part)
    } else {
        None
    }
}

pub(crate) fn state_part_style_for_state<'a>(
    style: &'a NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    part: &str,
) -> Option<&'a PartStyle> {
    if state.is_disabled(widget_id) {
        style.parts.disabled.get(part)
    } else if state.pressed.as_deref() == Some(widget_id) {
        style.parts.active.get(part)
    } else if state.hovered.as_deref() == Some(widget_id) {
        style.parts.hover.get(part)
    } else if state.focused.as_deref() == Some(widget_id) {
        style.parts.focus.get(part)
    } else {
        None
    }
}

pub(crate) fn part_style_active_for_state(
    style: &NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    part: &str,
) -> bool {
    base_part_style(style, part).is_some()
        || checked_part_style_for_state(style, widget_id, state, part).is_some()
        || state_part_style_for_state(style, widget_id, state, part).is_some()
}

pub(crate) fn part_visual_for_state(
    style: &NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    part: &str,
) -> VisualStyle {
    let mut visual = base_part_style(style, part)
        .map(|style| style.visual.clone())
        .unwrap_or_default();
    if let Some(checked) = checked_part_style_for_state(style, widget_id, state, part) {
        visual = visual.merged(&checked.visual);
    }
    if let Some(pseudo) = state_part_style_for_state(style, widget_id, state, part) {
        visual = visual.merged(&pseudo.visual);
    }
    visual
}

pub(crate) fn merged_part_visual_for_state(
    style: &NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    parts: &[&str],
) -> VisualStyle {
    let mut visual = VisualStyle::default();
    for part in parts {
        visual = visual.merged(&part_visual_for_state(style, widget_id, state, part));
    }
    visual
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
        parse_widget(map, &mut style.widget);
        parse_parts(map.get("parts"), &mut style.parts);
        style.hover = nested_visual(map.get("hover"));
        style.active = nested_visual(map.get("active"));
        style.focus = nested_visual(map.get("focus"));
        style.disabled = nested_visual(map.get("disabled"));
        style.checked = nested_visual(map.get("checked"));
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
            corner_radii: self.corner_radii.merged(&other.corner_radii),
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

impl CornerRadii {
    pub fn merged(&self, other: &CornerRadii) -> CornerRadii {
        CornerRadii {
            top_left: other.top_left.or(self.top_left),
            top_right: other.top_right.or(self.top_right),
            bottom_right: other.bottom_right.or(self.bottom_right),
            bottom_left: other.bottom_left.or(self.bottom_left),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.top_left.is_none()
            && self.top_right.is_none()
            && self.bottom_right.is_none()
            && self.bottom_left.is_none()
    }

    pub fn resolve(&self, uniform_radius: f32) -> [f32; 4] {
        [
            self.top_left.unwrap_or(uniform_radius),
            self.top_right.unwrap_or(uniform_radius),
            self.bottom_right.unwrap_or(uniform_radius),
            self.bottom_left.unwrap_or(uniform_radius),
        ]
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
    out.corner_radii.top_left = number(map.get("border_top_left_radius"))
        .or_else(|| number(map.get("border-top-left-radius")));
    out.corner_radii.top_right = number(map.get("border_top_right_radius"))
        .or_else(|| number(map.get("border-top-right-radius")));
    out.corner_radii.bottom_right = number(map.get("border_bottom_right_radius"))
        .or_else(|| number(map.get("border-bottom-right-radius")));
    out.corner_radii.bottom_left = number(map.get("border_bottom_left_radius"))
        .or_else(|| number(map.get("border-bottom-left-radius")));
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

fn parse_widget(map: &serde_json::Map<String, Value>, out: &mut WidgetStyle) {
    out.table_row_height =
        number(map.get("table_row_height")).or_else(|| number(map.get("table-row-height")));
    out.table_header_height =
        number(map.get("table_header_height")).or_else(|| number(map.get("table-header-height")));
}

fn parse_parts(value: Option<&Value>, out: &mut NodePartStyles) {
    let Some(Value::Object(parts)) = value else {
        return;
    };
    for (name, value) in parts {
        let Some(name) = normalize_part_name(name) else {
            continue;
        };
        let Value::Object(map) = value else {
            continue;
        };
        let base = part_style_from_map(map);
        if !part_style_is_empty(&base) {
            out.parts.insert(name.clone(), base);
        }
        parse_part_pseudo(map.get("hover"), &mut out.hover, &name);
        parse_part_pseudo(map.get("active"), &mut out.active, &name);
        parse_part_pseudo(map.get("focus"), &mut out.focus, &name);
        parse_part_pseudo(map.get("disabled"), &mut out.disabled, &name);
        parse_part_pseudo(map.get("checked"), &mut out.checked, &name);
    }
}

fn parse_part_pseudo(
    value: Option<&Value>,
    out: &mut BTreeMap<String, PartStyle>,
    part_name: &str,
) {
    let Some(Value::Object(map)) = value else {
        return;
    };
    let style = part_style_from_map(map);
    if !part_style_is_empty(&style) {
        out.insert(part_name.to_string(), style);
    }
}

fn part_style_from_map(map: &serde_json::Map<String, Value>) -> PartStyle {
    let mut style = PartStyle::default();
    parse_part_layout(map, &mut style.layout);
    parse_visual(map, &mut style.visual);
    parse_text(map, &mut style.text);
    style
}

fn parse_part_layout(map: &serde_json::Map<String, Value>, out: &mut PartLayoutStyle) {
    out.width = number(map.get("width"));
    out.height = number(map.get("height"));
    out.padding = number(map.get("padding"));
    out.gap = number(map.get("gap"));
}

fn normalize_part_name(name: &str) -> Option<String> {
    let normalized = name.trim().replace('_', "-").to_ascii_lowercase();
    (!normalized.is_empty()).then_some(normalized)
}

fn part_style_is_empty(style: &PartStyle) -> bool {
    style.layout.width.is_none()
        && style.layout.height.is_none()
        && style.layout.padding.is_none()
        && style.layout.gap.is_none()
        && visual_style_is_empty(&style.visual)
        && text_style_is_empty(&style.text)
}

fn visual_style_is_empty(style: &VisualStyle) -> bool {
    style.background.is_none()
        && style.foreground.is_none()
        && style.border_color.is_none()
        && style.border_width.is_none()
        && style.border_radius.is_none()
        && style.corner_radii.is_empty()
        && style.accent.is_none()
        && style.track_color.is_none()
        && style.thumb_color.is_none()
        && style.opacity.is_none()
}

fn text_style_is_empty(style: &TextStyle) -> bool {
    style.font_size.is_none()
        && style.font_family.is_none()
        && style.font_weight.is_none()
        && style.color.is_none()
        && style.text_align.is_none()
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
            "border_top_right_radius": 12,
            "border-bottom-left-radius": 4,
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
        assert_eq!(style.visual.border_radius, Some(9.0));
        assert_eq!(style.visual.corner_radii.top_left, None);
        assert_eq!(style.visual.corner_radii.top_right, Some(12.0));
        assert_eq!(style.visual.corner_radii.bottom_right, None);
        assert_eq!(style.visual.corner_radii.bottom_left, Some(4.0));
        assert_eq!(
            style
                .visual
                .corner_radii
                .resolve(style.visual.border_radius.unwrap()),
            [9.0, 12.0, 9.0, 4.0]
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

    #[test]
    fn parses_inline_part_styles_and_normalizes_part_names() {
        let style = NodeStyle::from_json(Some(&json!({
            "border_radius": 10,
            "parts": {
                "stepper_up": {
                    "background": "surface_alt",
                    "width": 32,
                    "border_top_right_radius": 10,
                    "color": "accent",
                    "hover": {
                        "background": "accent_mix_20",
                        "color": "text"
                    }
                },
                "row-selected": {
                    "background": "accent",
                    "font_weight": 700
                }
            }
        })));

        let stepper = style.parts.parts.get("stepper-up").unwrap();
        assert_eq!(stepper.layout.width, Some(32.0));
        assert_eq!(
            stepper.visual.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(stepper.visual.corner_radii.top_right, Some(10.0));
        assert_eq!(
            stepper.text.color,
            Some(ColorRef::Token("accent".to_string()))
        );

        let stepper_hover = style.parts.hover.get("stepper-up").unwrap();
        assert_eq!(
            stepper_hover.visual.background,
            Some(ColorRef::Token("accent_mix_20".to_string()))
        );
        assert_eq!(
            stepper_hover.text.color,
            Some(ColorRef::Token("text".to_string()))
        );

        let selected = style.parts.parts.get("row-selected").unwrap();
        assert_eq!(
            selected.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(selected.text.font_weight, Some(700));
    }

    #[test]
    fn resolves_part_visual_with_checked_then_pseudo_precedence() {
        let mut style = NodeStyle::default();
        style.parts.parts.insert(
            "thumb".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(ColorRef::Token("base".to_string())),
                    border_width: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        style.parts.checked.insert(
            "thumb".to_string(),
            PartStyle {
                visual: VisualStyle {
                    border_width: Some(2.0),
                    accent: Some(ColorRef::Token("checked".to_string())),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        style.parts.hover.insert(
            "thumb".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(ColorRef::Token("hover".to_string())),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut state = WidgetState::default();
        state.checked.insert("slider".to_string(), true);
        state.hovered = Some("slider".to_string());

        let visual = part_visual_for_state(&style, "slider", &state, "thumb");
        assert_eq!(
            visual.background,
            Some(ColorRef::Token("hover".to_string()))
        );
        assert_eq!(visual.border_width, Some(2.0));
        assert_eq!(visual.accent, Some(ColorRef::Token("checked".to_string())));
        assert!(part_style_active_for_state(
            &style, "slider", &state, "thumb"
        ));
    }

    #[test]
    fn resolves_disabled_part_style_before_pointer_state() {
        let mut style = NodeStyle::default();
        style.parts.hover.insert(
            "field".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(ColorRef::Token("hover".to_string())),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        style.parts.disabled.insert(
            "field".to_string(),
            PartStyle {
                visual: VisualStyle {
                    background: Some(ColorRef::Token("disabled".to_string())),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let mut state = WidgetState::default();
        state.hovered = Some("input".to_string());
        state.disabled.insert("input".to_string());

        let style = state_part_style_for_state(&style, "input", &state, "field").unwrap();
        assert_eq!(
            style.visual.background,
            Some(ColorRef::Token("disabled".to_string()))
        );
    }
}
