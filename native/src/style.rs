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
pub(crate) const BADGE_PAD_X_LP: f32 = 8.0;
pub(crate) const BADGE_MIN_HEIGHT_LP: f32 = 16.0;

pub(crate) fn badge_font_size_lp(style: &NodeStyle, theme: &Theme) -> f32 {
    style
        .parts
        .parts
        .get("badge")
        .and_then(|part| part.text.font_size)
        .or(style.text.font_size)
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
    let text_w = badge.chars().count() as f32 * font_size * 0.68;
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

pub(crate) fn uniform_layout_padding(style: &LayoutStyle) -> Option<f32> {
    style.padding.or_else(|| {
        let top = style.padding_top?;
        let right = style.padding_right?;
        let bottom = style.padding_bottom?;
        let left = style.padding_left?;
        (top == right && right == bottom && bottom == left).then_some(top)
    })
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
    pub transition: TransitionStyle,
    pub parts: NodePartStyles,
    pub hover: VisualStyle,
    pub active: VisualStyle,
    pub focus: VisualStyle,
    pub disabled: VisualStyle,
    pub checked: VisualStyle,
    pub open: VisualStyle,
    pub expanded: VisualStyle,
    pub collapsed: VisualStyle,
    pub selected: VisualStyle,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransitionStyle {
    pub properties: Option<Vec<TransitionProperty>>,
    pub duration_ms: Option<u64>,
    pub delay_ms: Option<u64>,
    pub timing_function: Option<TransitionTimingFunction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionProperty {
    All,
    Background,
    Foreground,
    BorderColor,
    BorderWidth,
    BorderRadius,
    Opacity,
    Color,
    Accent,
    TrackColor,
    ThumbColor,
    BoxShadow,
    Transform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransitionTimingFunction {
    Linear,
    Ease,
    EaseIn,
    EaseOut,
    EaseInOut,
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
    pub width_value: Option<LayoutLength>,
    pub height_value: Option<LayoutLength>,
    pub min_width_value: Option<LayoutLength>,
    pub min_height_value: Option<LayoutLength>,
    pub max_width_value: Option<LayoutLength>,
    pub max_height_value: Option<LayoutLength>,
    pub padding: Option<f32>,
    pub padding_left: Option<f32>,
    pub padding_right: Option<f32>,
    pub padding_top: Option<f32>,
    pub padding_bottom: Option<f32>,
    pub margin: Option<f32>,
    pub gap: Option<f32>,
    pub row_gap: Option<f32>,
    pub column_gap: Option<f32>,
    pub overflow: Option<OverflowStyle>,
    pub overflow_x: Option<OverflowStyle>,
    pub overflow_y: Option<OverflowStyle>,
    pub position: Option<PositionStyle>,
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
    pub z_index: Option<i32>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub grid_template_columns: Option<Vec<GridTrackSize>>,
    pub grid_template_rows: Option<Vec<GridTrackSize>>,
    pub grid_column: Option<GridPlacementStyle>,
    pub grid_row: Option<GridPlacementStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalcLength {
    pub percent: f32,
    pub px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutLength {
    LogicalPx(f32),
    Percent(f32),
    Calc(CalcLength),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayStyle {
    Flex,
    Grid,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowStyle {
    Visible,
    Hidden,
    Scroll,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionStyle {
    Static,
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridTrackSize {
    LogicalPx(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridLineStyle {
    Auto,
    Line(i16),
    Span(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPlacementStyle {
    pub start: GridLineStyle,
    pub end: GridLineStyle,
}

#[derive(Debug, Clone, Default)]
pub struct VisualStyle {
    pub background: Option<ColorRef>,
    pub background_paint: Option<BackgroundPaint>,
    pub foreground: Option<ColorRef>,
    pub border_color: Option<ColorRef>,
    pub border_width: Option<f32>,
    pub border_radius: Option<f32>,
    pub corner_radii: CornerRadii,
    pub accent: Option<ColorRef>,
    pub track_color: Option<ColorRef>,
    pub thumb_color: Option<ColorRef>,
    pub opacity: Option<f32>,
    pub background_noise: Option<f32>,
    pub box_shadows: Option<Vec<BoxShadow>>,
    pub transform: Option<TransformStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformStyle {
    pub translate_x: f32,
    pub translate_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotate_deg: f32,
}

impl Default for TransformStyle {
    fn default() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate_deg: 0.0,
        }
    }
}

impl TransformStyle {
    pub fn is_identity(&self) -> bool {
        self.translate_x == 0.0
            && self.translate_y == 0.0
            && self.scale_x == 1.0
            && self.scale_y == 1.0
            && self.rotate_deg == 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundPaint {
    Color(ColorRef),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    Layers(Vec<BackgroundPaint>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    pub angle_deg: f32,
    pub stops: Vec<GradientStop>,
    pub repeating: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    pub stops: Vec<GradientStop>,
    pub repeating: bool,
    pub center: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    pub color: ColorRef,
    pub position: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: Option<f32>,
    pub top_right: Option<f32>,
    pub bottom_right: Option<f32>,
    pub bottom_left: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: ColorRef,
    pub inset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontVariantNumeric {
    Normal,
    TabularNums,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextSpacing {
    LogicalPx(f32),
    Em(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    Multiplier(f32),
    LogicalPx(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextOverflow {
    Clip,
    Ellipsis,
}

#[derive(Debug, Clone, Default)]
pub struct TextStyle {
    pub font_size: Option<f32>,
    pub font_family: Option<FontFamily>,
    pub font_weight: Option<u16>,
    pub color: Option<ColorRef>,
    pub text_align: Option<TextAlign>,
    pub text_transform: Option<TextTransform>,
    pub letter_spacing: Option<TextSpacing>,
    pub line_height: Option<LineHeight>,
    pub font_style: Option<FontStyle>,
    pub font_variant_numeric: Option<FontVariantNumeric>,
    pub text_overflow: Option<TextOverflow>,
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
    pub open: BTreeMap<String, PartStyle>,
    pub expanded: BTreeMap<String, PartStyle>,
    pub collapsed: BTreeMap<String, PartStyle>,
    pub selected: BTreeMap<String, PartStyle>,
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

pub(crate) fn open_part_style_for_state<'a>(
    style: &'a NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    part: &str,
) -> Option<&'a PartStyle> {
    if state.is_open_widget(widget_id) {
        style.parts.open.get(part)
    } else {
        None
    }
}

pub(crate) fn expanded_part_style_for_state<'a>(
    style: &'a NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    part: &str,
) -> Option<&'a PartStyle> {
    if state.is_expanded_widget(widget_id) {
        style.parts.expanded.get(part)
    } else {
        None
    }
}

pub(crate) fn collapsed_part_style_for_state<'a>(
    style: &'a NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    part: &str,
) -> Option<&'a PartStyle> {
    if state.is_collapsed_widget(widget_id) {
        style.parts.collapsed.get(part)
    } else {
        None
    }
}

pub(crate) fn selected_part_style_for_state<'a>(
    style: &'a NodeStyle,
    widget_id: &str,
    state: &WidgetState,
    part: &str,
) -> Option<&'a PartStyle> {
    if state.is_selected_widget(widget_id) {
        style.parts.selected.get(part)
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
        || open_part_style_for_state(style, widget_id, state, part).is_some()
        || expanded_part_style_for_state(style, widget_id, state, part).is_some()
        || collapsed_part_style_for_state(style, widget_id, state, part).is_some()
        || selected_part_style_for_state(style, widget_id, state, part).is_some()
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
    if let Some(open) = open_part_style_for_state(style, widget_id, state, part) {
        visual = visual.merged(&open.visual);
    }
    if let Some(expanded) = expanded_part_style_for_state(style, widget_id, state, part) {
        visual = visual.merged(&expanded.visual);
    }
    if let Some(collapsed) = collapsed_part_style_for_state(style, widget_id, state, part) {
        visual = visual.merged(&collapsed.visual);
    }
    if let Some(selected) = selected_part_style_for_state(style, widget_id, state, part) {
        visual = visual.merged(&selected.visual);
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
        parse_transition(map, &mut style.transition);
        parse_parts(map.get("parts"), &mut style.parts);
        style.hover = nested_visual(map.get("hover"));
        style.active = nested_visual(map.get("active"));
        style.focus = nested_visual(map.get("focus"));
        style.disabled = nested_visual(map.get("disabled"));
        style.checked = nested_visual(map.get("checked"));
        style.open = nested_visual(map.get("open"));
        style.expanded = nested_visual(map.get("expanded"));
        style.collapsed = nested_visual(map.get("collapsed"));
        style.selected = nested_visual(map.get("selected"));
        style
    }
}

impl VisualStyle {
    pub fn merged(&self, other: &VisualStyle) -> VisualStyle {
        VisualStyle {
            background: other.background.clone().or_else(|| self.background.clone()),
            background_paint: other
                .background_paint
                .clone()
                .or_else(|| self.background_paint.clone()),
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
            background_noise: other.background_noise.or(self.background_noise),
            box_shadows: other
                .box_shadows
                .clone()
                .or_else(|| self.box_shadows.clone()),
            transform: other.transform.or(self.transform),
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
    out.width_value = out.width.map(LayoutLength::LogicalPx);
    out.height_value = out.height.map(LayoutLength::LogicalPx);
    out.min_width_value = out.min_width.map(LayoutLength::LogicalPx);
    out.min_height_value = out.min_height.map(LayoutLength::LogicalPx);
    out.max_width_value = out.max_width.map(LayoutLength::LogicalPx);
    out.max_height_value = out.max_height.map(LayoutLength::LogicalPx);
    out.padding = number(map.get("padding"));
    out.padding_left = number(map.get("padding_left"));
    out.padding_right = number(map.get("padding_right"));
    out.padding_top = number(map.get("padding_top"));
    out.padding_bottom = number(map.get("padding_bottom"));
    out.margin = number(map.get("margin"));
    out.gap = number(map.get("gap"));
    out.row_gap = number(map.get("row_gap")).or_else(|| number(map.get("row-gap")));
    out.column_gap = number(map.get("column_gap")).or_else(|| number(map.get("column-gap")));
    out.overflow = text_value(map, "overflow", "overflow").and_then(parse_overflow);
    out.overflow_x = text_value(map, "overflow_x", "overflow-x").and_then(parse_overflow);
    out.overflow_y = text_value(map, "overflow_y", "overflow-y").and_then(parse_overflow);
    out.position = text_value(map, "position", "position").and_then(parse_position);
    out.top = number(map.get("top"));
    out.right = number(map.get("right"));
    out.bottom = number(map.get("bottom"));
    out.left = number(map.get("left"));
    out.z_index = number(map.get("z_index"))
        .or_else(|| number(map.get("z-index")))
        .map(|value| value.round() as i32);
    out.flex_grow = number(map.get("flex_grow")).or_else(|| number(map.get("flex")));
    out.flex_shrink = number(map.get("flex_shrink"));
}

fn parse_visual(map: &serde_json::Map<String, Value>, out: &mut VisualStyle) {
    out.background = color_ref(map.get("background"));
    out.background_paint = out.background.clone().map(BackgroundPaint::Color);
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
    out.background_noise = number(map.get("background_noise"))
        .or_else(|| number(map.get("background-noise")))
        .map(|v| v.clamp(0.0, 0.25));
    out.box_shadows = value_for_keys(map, "box_shadow", "box-shadow").and_then(parse_box_shadows);
    out.transform = map.get("transform").and_then(parse_transform_style);
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
    out.text_transform =
        text_value(map, "text_transform", "text-transform").and_then(parse_text_transform);
    out.letter_spacing =
        value_for_keys(map, "letter_spacing", "letter-spacing").and_then(parse_text_spacing);
    out.line_height = value_for_keys(map, "line_height", "line-height").and_then(parse_line_height);
    out.font_style = text_value(map, "font_style", "font-style").and_then(parse_font_style);
    out.font_variant_numeric = text_value(map, "font_variant_numeric", "font-variant-numeric")
        .and_then(parse_font_variant_numeric);
    out.text_overflow =
        text_value(map, "text_overflow", "text-overflow").and_then(parse_text_overflow);
}

fn parse_widget(map: &serde_json::Map<String, Value>, out: &mut WidgetStyle) {
    out.table_row_height =
        number(map.get("table_row_height")).or_else(|| number(map.get("table-row-height")));
    out.table_header_height =
        number(map.get("table_header_height")).or_else(|| number(map.get("table-header-height")));
}

fn parse_transition(map: &serde_json::Map<String, Value>, out: &mut TransitionStyle) {
    if let Some(duration) = value_for_keys(map, "transition_duration", "transition-duration")
        .and_then(parse_duration_ms)
    {
        out.duration_ms = Some(duration);
    }
    if let Some(delay) =
        value_for_keys(map, "transition_delay", "transition-delay").and_then(parse_duration_ms)
    {
        out.delay_ms = Some(delay);
    }
    if let Some(timing) = text_value(
        map,
        "transition_timing_function",
        "transition-timing-function",
    )
    .and_then(parse_transition_timing_function)
    {
        out.timing_function = Some(timing);
    }
    if let Some(properties) = value_for_keys(map, "transition_property", "transition-property")
        .and_then(parse_transition_properties)
    {
        out.properties = Some(properties);
    }
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
        parse_part_pseudo(map.get("open"), &mut out.open, &name);
        parse_part_pseudo(map.get("expanded"), &mut out.expanded, &name);
        parse_part_pseudo(map.get("collapsed"), &mut out.collapsed, &name);
        parse_part_pseudo(map.get("selected"), &mut out.selected, &name);
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
        && style.background_paint.is_none()
        && style.foreground.is_none()
        && style.border_color.is_none()
        && style.border_width.is_none()
        && style.border_radius.is_none()
        && style.corner_radii.is_empty()
        && style.accent.is_none()
        && style.track_color.is_none()
        && style.thumb_color.is_none()
        && style.opacity.is_none()
        && style.background_noise.is_none()
        && style.box_shadows.is_none()
        && style.transform.is_none()
}

fn text_style_is_empty(style: &TextStyle) -> bool {
    style.font_size.is_none()
        && style.font_family.is_none()
        && style.font_weight.is_none()
        && style.color.is_none()
        && style.text_align.is_none()
        && style.text_transform.is_none()
        && style.letter_spacing.is_none()
        && style.line_height.is_none()
        && style.font_style.is_none()
        && style.font_variant_numeric.is_none()
        && style.text_overflow.is_none()
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

fn parse_text_transform(value: &str) -> Option<TextTransform> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Some(TextTransform::None),
        "uppercase" => Some(TextTransform::Uppercase),
        "lowercase" => Some(TextTransform::Lowercase),
        "capitalize" => Some(TextTransform::Capitalize),
        _ => None,
    }
}

fn parse_font_style(value: &str) -> Option<FontStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(FontStyle::Normal),
        "italic" => Some(FontStyle::Italic),
        _ => None,
    }
}

fn parse_font_variant_numeric(value: &str) -> Option<FontVariantNumeric> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(FontVariantNumeric::Normal),
        "tabular-nums" | "tabular_nums" => Some(FontVariantNumeric::TabularNums),
        _ => None,
    }
}

fn parse_text_overflow(value: &str) -> Option<TextOverflow> {
    match value.trim().to_ascii_lowercase().as_str() {
        "clip" => Some(TextOverflow::Clip),
        "ellipsis" => Some(TextOverflow::Ellipsis),
        _ => None,
    }
}

fn parse_text_spacing(value: &Value) -> Option<TextSpacing> {
    if let Some(n) = value.as_f64() {
        return Some(TextSpacing::LogicalPx(n as f32));
    }
    let text = value.as_str()?.trim().to_ascii_lowercase();
    if text == "normal" {
        return Some(TextSpacing::LogicalPx(0.0));
    }
    if let Some(px) = text.strip_suffix("px") {
        return px.trim().parse::<f32>().ok().map(TextSpacing::LogicalPx);
    }
    if let Some(em) = text.strip_suffix("em") {
        return em.trim().parse::<f32>().ok().map(TextSpacing::Em);
    }
    text.parse::<f32>().ok().map(TextSpacing::LogicalPx)
}

fn parse_line_height(value: &Value) -> Option<LineHeight> {
    if let Some(n) = value.as_f64() {
        return Some(LineHeight::Multiplier(n as f32));
    }
    let text = value.as_str()?.trim().to_ascii_lowercase();
    if let Some(px) = text.strip_suffix("px") {
        return px.trim().parse::<f32>().ok().map(LineHeight::LogicalPx);
    }
    text.parse::<f32>().ok().map(LineHeight::Multiplier)
}

fn parse_box_shadows(value: &Value) -> Option<Vec<BoxShadow>> {
    match value {
        Value::String(value) if value.trim().eq_ignore_ascii_case("none") => Some(Vec::new()),
        Value::Object(map) => parse_box_shadow_object(map).map(|shadow| vec![shadow]),
        Value::Array(items) => {
            let mut shadows = Vec::with_capacity(items.len());
            for item in items {
                let Value::Object(map) = item else {
                    return None;
                };
                shadows.push(parse_box_shadow_object(map)?);
            }
            Some(shadows)
        }
        _ => None,
    }
}

fn parse_box_shadow_object(map: &serde_json::Map<String, Value>) -> Option<BoxShadow> {
    let color = color_ref(map.get("color")).unwrap_or(ColorRef::Rgba([0.0, 0.0, 0.0, 0.35]));
    Some(BoxShadow {
        offset_x: number(value_for_keys(map, "offset_x", "offset-x")).unwrap_or(0.0),
        offset_y: number(value_for_keys(map, "offset_y", "offset-y")).unwrap_or(0.0),
        blur: number(map.get("blur")).unwrap_or(0.0).max(0.0),
        spread: number(map.get("spread")).unwrap_or(0.0),
        color,
        inset: map.get("inset").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn parse_duration_ms(value: &Value) -> Option<u64> {
    if let Some(n) = value.as_f64() {
        return Some(n.max(0.0).round() as u64);
    }
    let text = value.as_str()?.trim().to_ascii_lowercase();
    if let Some(ms) = text.strip_suffix("ms") {
        return ms
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v.max(0.0).round() as u64);
    }
    if let Some(seconds) = text.strip_suffix('s') {
        return seconds
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v.max(0.0) * 1000.0).round() as u64);
    }
    text.parse::<f32>().ok().map(|v| v.max(0.0).round() as u64)
}

fn parse_transition_properties(value: &Value) -> Option<Vec<TransitionProperty>> {
    match value {
        Value::String(text) => {
            let properties: Vec<_> = text
                .split(',')
                .filter_map(parse_transition_property)
                .collect();
            (!properties.is_empty()).then_some(properties)
        }
        Value::Array(items) => {
            let properties: Vec<_> = items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(parse_transition_property)
                .collect();
            (!properties.is_empty()).then_some(properties)
        }
        _ => None,
    }
}

fn parse_transition_property(value: &str) -> Option<TransitionProperty> {
    match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "all" => Some(TransitionProperty::All),
        "background" | "background-color" => Some(TransitionProperty::Background),
        "foreground" => Some(TransitionProperty::Foreground),
        "border-color" => Some(TransitionProperty::BorderColor),
        "border-width" => Some(TransitionProperty::BorderWidth),
        "border-radius" => Some(TransitionProperty::BorderRadius),
        "opacity" => Some(TransitionProperty::Opacity),
        "color" => Some(TransitionProperty::Color),
        "accent" => Some(TransitionProperty::Accent),
        "track-color" => Some(TransitionProperty::TrackColor),
        "thumb-color" => Some(TransitionProperty::ThumbColor),
        "box-shadow" => Some(TransitionProperty::BoxShadow),
        _ => None,
    }
}

fn parse_transition_timing_function(value: &str) -> Option<TransitionTimingFunction> {
    match value.trim().to_ascii_lowercase().as_str() {
        "linear" => Some(TransitionTimingFunction::Linear),
        "ease" => Some(TransitionTimingFunction::Ease),
        "ease-in" | "ease_in" => Some(TransitionTimingFunction::EaseIn),
        "ease-out" | "ease_out" => Some(TransitionTimingFunction::EaseOut),
        "ease-in-out" | "ease_in_out" => Some(TransitionTimingFunction::EaseInOut),
        _ => None,
    }
}

fn parse_transform_style(value: &Value) -> Option<TransformStyle> {
    match value {
        Value::String(text) => parse_transform_functions(text),
        Value::Object(map) => {
            let mut transform = TransformStyle::default();
            if let Some(value) =
                number(map.get("translate_x")).or_else(|| number(map.get("translate-x")))
            {
                transform.translate_x = value;
            }
            if let Some(value) =
                number(map.get("translate_y")).or_else(|| number(map.get("translate-y")))
            {
                transform.translate_y = value;
            }
            if let Some(value) = number(map.get("scale")) {
                transform.scale_x = value;
                transform.scale_y = value;
            }
            if let Some(value) = number(map.get("scale_x")).or_else(|| number(map.get("scale-x"))) {
                transform.scale_x = value;
            }
            if let Some(value) = number(map.get("scale_y")).or_else(|| number(map.get("scale-y"))) {
                transform.scale_y = value;
            }
            if let Some(value) = number(map.get("rotate")).or_else(|| number(map.get("rotate_deg")))
            {
                transform.rotate_deg = value;
            }
            (!transform.is_identity()).then_some(transform)
        }
        _ => None,
    }
}

pub(crate) fn parse_transform_functions(value: &str) -> Option<TransformStyle> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return None;
    }
    let mut rest = value;
    let mut transform = TransformStyle::default();
    while !rest.trim().is_empty() {
        rest = rest.trim_start();
        let open = rest.find('(')?;
        let name = rest[..open].trim().to_ascii_lowercase();
        let after_open = &rest[open + 1..];
        let close = after_open.find(')')?;
        let args = &after_open[..close];
        apply_transform_function(&mut transform, &name, args)?;
        rest = &after_open[close + 1..];
    }
    (!transform.is_identity()).then_some(transform)
}

fn apply_transform_function(transform: &mut TransformStyle, name: &str, args: &str) -> Option<()> {
    match name {
        "translate" => {
            let args = split_transform_args(args);
            let x = parse_transform_length(args.first()?)?;
            let y = args
                .get(1)
                .and_then(|value| parse_transform_length(value))
                .unwrap_or(0.0);
            transform.translate_x += x;
            transform.translate_y += y;
        }
        "translatex" => {
            transform.translate_x += parse_transform_length(args)?;
        }
        "translatey" => {
            transform.translate_y += parse_transform_length(args)?;
        }
        "scale" => {
            let args = split_transform_args(args);
            let x = parse_transform_number(args.first()?)?;
            let y = args
                .get(1)
                .and_then(|value| parse_transform_number(value))
                .unwrap_or(x);
            transform.scale_x *= x;
            transform.scale_y *= y;
        }
        "scalex" => {
            transform.scale_x *= parse_transform_number(args)?;
        }
        "scaley" => {
            transform.scale_y *= parse_transform_number(args)?;
        }
        "rotate" => {
            transform.rotate_deg += parse_transform_angle(args)?;
        }
        _ => return None,
    }
    Some(())
}

fn split_transform_args(args: &str) -> Vec<&str> {
    if args.contains(',') {
        args.split(',')
            .map(str::trim)
            .filter(|arg| !arg.is_empty())
            .collect()
    } else {
        args.split_whitespace().collect()
    }
}

fn parse_transform_length(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(px) = value.strip_suffix("px") {
        return px.trim().parse().ok();
    }
    value.parse().ok()
}

fn parse_transform_number(value: &str) -> Option<f32> {
    value.trim().parse().ok()
}

fn parse_transform_angle(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(deg) = value.strip_suffix("deg") {
        return deg.trim().parse().ok();
    }
    if let Some(rad) = value.strip_suffix("rad") {
        return rad
            .trim()
            .parse::<f32>()
            .ok()
            .map(|radians| radians.to_degrees());
    }
    if let Some(turn) = value.strip_suffix("turn") {
        return turn.trim().parse::<f32>().ok().map(|turns| turns * 360.0);
    }
    value.parse().ok()
}

fn value_for_keys<'a>(
    map: &'a serde_json::Map<String, Value>,
    snake: &str,
    dashed: &str,
) -> Option<&'a Value> {
    map.get(snake).or_else(|| map.get(dashed))
}

fn text_value<'a>(
    map: &'a serde_json::Map<String, Value>,
    snake: &str,
    dashed: &str,
) -> Option<&'a str> {
    value_for_keys(map, snake, dashed).and_then(Value::as_str)
}

fn parse_display(value: &str) -> Option<DisplayStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "flex" => Some(DisplayStyle::Flex),
        "grid" => Some(DisplayStyle::Grid),
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

fn parse_overflow(value: &str) -> Option<OverflowStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "visible" => Some(OverflowStyle::Visible),
        "hidden" | "clip" => Some(OverflowStyle::Hidden),
        "scroll" => Some(OverflowStyle::Scroll),
        "auto" => Some(OverflowStyle::Auto),
        _ => None,
    }
}

fn parse_position(value: &str) -> Option<PositionStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "static" => Some(PositionStyle::Static),
        "relative" => Some(PositionStyle::Relative),
        "absolute" => Some(PositionStyle::Absolute),
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
            "transition_property": ["background", "border-color"],
            "transition_duration": "180ms",
            "transition_delay": "0.05s",
            "transition_timing_function": "ease-out",
            "transform": "translateY(-2px) scale(1.02) rotate(1deg)",
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
        assert_eq!(
            style.transition.properties,
            Some(vec![
                TransitionProperty::Background,
                TransitionProperty::BorderColor
            ])
        );
        assert_eq!(style.transition.duration_ms, Some(180));
        assert_eq!(style.transition.delay_ms, Some(50));
        assert_eq!(
            style.transition.timing_function,
            Some(TransitionTimingFunction::EaseOut)
        );
        let transform = style.visual.transform.expect("parsed transform");
        assert_eq!(transform.translate_y, -2.0);
        assert_eq!(transform.scale_x, 1.02);
        assert_eq!(transform.scale_y, 1.02);
        assert_eq!(transform.rotate_deg, 1.0);
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
