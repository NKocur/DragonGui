use std::collections::{HashMap, HashSet};

use taffy::prelude::*;
use taffy::style::Overflow;

use crate::document::{WidgetKind, WidgetNode};
use crate::events::WidgetState;
use crate::style::{
    badge_width_for_text, base_part_style, code_editor_gutter_width_for_style,
    collapsible_header_height_for_style, standalone_badge_horizontal_padding_lp,
    tabs_header_height_for_style, AlignItemsStyle, DisplayStyle, FlexDirectionStyle, FlexWrapStyle,
    GridAutoFlowStyle, GridLineStyle, GridPlacementStyle, GridTemplateAreas,
    GridTrackFitContentSize, GridTrackMaxSize, GridTrackMinSize, GridTrackRepeatKind,
    GridTrackSize, JustifyContentStyle, LayoutLength, LineHeight, NodeStyle, OverflowStyle,
    PositionStyle, TextOverflow, BADGE_GAP_LP, BADGE_MIN_HEIGHT_LP, CHECKBOX_BOX_LP,
    CHECKBOX_LEFT_PAD_LP, TOGGLE_SWITCH_TRACK_WIDTH_LP,
};
use crate::text::{measure_text_for_layout, measure_wrapped_text_for_layout};
use crate::theme::Theme;

const MENU_LABEL_WIDTH_SAFETY_LP: f32 = 6.0;
// Plain labels render into an integer-scissored text box after shaping. Keep a
// small amount of width beyond the measured advance so fractional glyph
// overhang and rasterization do not clip the final character at an exact fit.
const LABEL_TEXT_WIDTH_SAFETY_LP: f32 = 2.0;
const PANEL_BODY_VISUAL_INSET_LP: f32 = 1.0;
const LOADING_SPINNER_DEFAULT_SIZE_LP: f32 = 18.0;
const LOADING_SPINNER_GAP_LP: f32 = 8.0;
const NAV_ITEM_MIN_HEIGHT_LP: f32 = 28.0;
const SIDEBAR_COMPACT_BREAKPOINT_LP: f32 = 700.0;
const SIDEBAR_MOBILE_BREAKPOINT_LP: f32 = 480.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct NativeLayoutFallback {
    pub display: Option<DisplayStyle>,
    pub flex_direction: Option<FlexDirectionStyle>,
    pub flex_wrap: Option<FlexWrapStyle>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct NativeGeometryFallback {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct NativeLayoutFallbackContext {
    pub parent_kind: Option<WidgetKind>,
    pub parent_flex_direction: Option<FlexDirectionStyle>,
    pub parent_preserves_preferred_main_size: bool,
}

pub(crate) fn invariant_widget_layout_fallback(kind: WidgetKind) -> NativeLayoutFallback {
    use WidgetKind::*;

    match kind {
        Window => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Column),
            ..Default::default()
        },
        HLayout => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Row),
            flex_grow: Some(1.0),
            ..Default::default()
        },
        VLayout => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Column),
            flex_grow: Some(1.0),
            ..Default::default()
        },
        ScrollArea | Page => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Column),
            flex_grow: Some(1.0),
            flex_shrink: Some(1.0),
            ..Default::default()
        },
        GridLayout => NativeLayoutFallback {
            display: Some(DisplayStyle::Grid),
            flex_grow: Some(1.0),
            flex_shrink: Some(1.0),
            ..Default::default()
        },
        FlowLayout => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Row),
            flex_wrap: Some(FlexWrapStyle::Wrap),
            flex_grow: Some(0.0),
            flex_shrink: Some(1.0),
        },
        TreeView | DragSource | DropTarget => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Column),
            flex_grow: Some(0.0),
            flex_shrink: Some(1.0),
            ..Default::default()
        },
        StatusBar | MenuBar => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Row),
            flex_grow: Some(0.0),
            flex_shrink: Some(0.0),
            ..Default::default()
        },
        Collapsible => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Column),
            flex_grow: Some(0.0),
            flex_shrink: Some(0.0),
            ..Default::default()
        },
        Panel => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Column),
            flex_grow: Some(0.0),
            flex_shrink: Some(1.0),
            ..Default::default()
        },
        Sidebar | Pane => NativeLayoutFallback {
            display: Some(DisplayStyle::Flex),
            flex_direction: Some(FlexDirectionStyle::Column),
            flex_shrink: Some(1.0),
            ..Default::default()
        },
        Pages | PieChart | Histogram | BarChart | Heatmap | LinePlot | Scatter3D
        | DataFrameTable => NativeLayoutFallback {
            flex_grow: Some(1.0),
            flex_shrink: Some(1.0),
            ..Default::default()
        },
        _ => NativeLayoutFallback::default(),
    }
}

pub(crate) fn stable_widget_geometry_fallback(node: &WidgetNode) -> NativeGeometryFallback {
    match node.kind {
        WidgetKind::Image => NativeGeometryFallback {
            min_width: Some(48.0),
            min_height: Some(48.0),
            ..Default::default()
        },
        WidgetKind::HtmlReport => NativeGeometryFallback {
            height: node.props.fixed_height.is_none().then_some(360.0),
            min_width: Some(240.0),
            min_height: Some(160.0),
            ..Default::default()
        },
        WidgetKind::Extension => NativeGeometryFallback {
            height: (node.props.fixed_height.is_none() && node.props.intrinsic_height.is_none())
                .then_some(80.0),
            min_width: Some(0.0),
            min_height: Some(0.0),
            ..Default::default()
        },
        _ => NativeGeometryFallback::default(),
    }
}

pub(crate) fn resolved_widget_geometry_fallback(
    node: &WidgetNode,
    computed_style: &NodeStyle,
    theme: &Theme,
) -> NativeGeometryFallback {
    let mut fallback = stable_widget_geometry_fallback(node);
    let font_size = computed_style
        .text
        .font_size
        .unwrap_or_else(|| crate::style::native_fallback_font_size(theme))
        .max(8.0);
    let control_height = (font_size + theme.spacing * 2.0 + 2.0).max(25.0);
    match node.kind {
        WidgetKind::IconButton | WidgetKind::ImageButton | WidgetKind::ArrowButton => {
            fallback.width = Some(control_height);
            fallback.height = Some(control_height);
        }
        WidgetKind::Button
        | WidgetKind::SmallButton
        | WidgetKind::Selectable
        | WidgetKind::RadioButton
        | WidgetKind::Dropdown
        | WidgetKind::Menu
        | WidgetKind::MenuItem
        | WidgetKind::NumberInput
        | WidgetKind::DragNumber
        | WidgetKind::Tab
        | WidgetKind::Checkbox
        | WidgetKind::ToggleSwitch
        | WidgetKind::Slider
        | WidgetKind::RangeSlider
        | WidgetKind::ProgressBar
        | WidgetKind::LimitsBar
        | WidgetKind::TextInput => {
            fallback.height = Some(control_height);
        }
        WidgetKind::NavItem => {
            fallback.height = Some(control_height.max(NAV_ITEM_MIN_HEIGHT_LP));
        }
        WidgetKind::Badge | WidgetKind::Tag => {
            fallback.height = Some((font_size + 8.0).max(20.0));
        }
        WidgetKind::Led => {
            let size = node.props.led_size.unwrap_or(14.0).max(1.0);
            fallback.width = Some(size);
            fallback.height = Some(size);
        }
        WidgetKind::Label
            if !node.props.wrap.unwrap_or(true)
                || computed_style.text.text_overflow == Some(TextOverflow::Ellipsis) =>
        {
            fallback.height = Some(control_height);
        }
        WidgetKind::LoadingSpinner => {
            fallback.height = Some(
                loading_spinner_size_lp(node)
                    .max(1.0)
                    .max(control_height * 0.82),
            );
        }
        WidgetKind::Sidebar => {
            let state = node
                .props
                .raw_props
                .get("state")
                .and_then(|value| value.as_str())
                .unwrap_or("auto");
            fallback.width = if state == "collapsed" {
                raw_prop_f32(node, "collapsed_width")
                    .filter(|width| *width > 0.0)
                    .or(node.props.fixed_width)
            } else {
                None
            };
        }
        WidgetKind::TextArea | WidgetKind::CodeEditor | WidgetKind::LogView => {
            let rows = computed_style
                .widget
                .text_area_rows
                .unwrap_or_else(|| node.props.rows.unwrap_or(4) as f32)
                .round()
                .max(1.0);
            let line_height = (font_size + 6.0).max(theme.font_size + 4.0);
            fallback.height = Some(rows * line_height + theme.spacing * 2.0);
        }
        WidgetKind::Tabs => {
            let has_tab_content = node
                .children
                .iter()
                .any(|child| child.kind == WidgetKind::Tab && !child.children.is_empty());
            if !has_tab_content {
                // Empty-content Tabs act as a standalone strip. Its owning
                // layout box must track a CSS-authored ::header height because
                // layout_tabs() assigns that same height to every Tab child.
                // Otherwise the children paint past the Tabs box and overlap
                // the following Body/Pages sibling.
                fallback.height = Some(tabs_header_height_for_style(computed_style, theme, 1.0));
            }
        }
        _ => {}
    }
    fallback
}

pub(crate) fn resolved_widget_layout_fallback(
    node: &WidgetNode,
    computed_style: &NodeStyle,
    context: NativeLayoutFallbackContext,
    live_pane_size: Option<f32>,
) -> NativeLayoutFallback {
    let mut fallback = invariant_widget_layout_fallback(node.kind);
    match node.kind {
        WidgetKind::HLayout | WidgetKind::VLayout => {
            fallback.flex_shrink = Some(if context.parent_kind == Some(WidgetKind::Window) {
                1.0
            } else {
                0.0
            });
        }
        WidgetKind::Splitter => {
            fallback.display = Some(DisplayStyle::Flex);
            fallback.flex_direction = Some(
                if node.props.orientation.as_deref().unwrap_or("horizontal") == "vertical" {
                    FlexDirectionStyle::Column
                } else {
                    FlexDirectionStyle::Row
                },
            );
            fallback.flex_grow = Some(1.0);
            fallback.flex_shrink = Some(if node.props.fixed_width.is_some() {
                0.0
            } else {
                1.0
            });
        }
        WidgetKind::Panel => {
            fallback.flex_grow = Some(0.0);
        }
        WidgetKind::Sidebar => {
            let sidebar_state = node
                .props
                .raw_props
                .get("state")
                .and_then(|value| value.as_str())
                .unwrap_or("auto");
            if sidebar_state == "hidden" {
                fallback.display = Some(DisplayStyle::None);
            }
            fallback.flex_grow = Some(if node.props.fixed_width.is_some() {
                0.0
            } else {
                1.0
            });
        }
        WidgetKind::Pane => {
            let requested_size = live_pane_size
                .or(node.props.pane_size)
                .filter(|size| size.is_finite())
                .map(|size| size.max(0.0));
            let fractional_flex = requested_size.filter(|size| *size > 0.0 && *size < 1.0);
            let active_size = requested_size.filter(|size| !(*size > 0.0 && *size < 1.0));
            fallback.flex_grow = Some(if active_size.is_some() {
                0.0
            } else {
                fractional_flex
                    .or(node.props.pane_flex)
                    .unwrap_or(1.0)
                    .max(0.0)
            });
        }
        WidgetKind::Image => {
            let fixed = node.props.fixed_width.is_some() || node.props.fixed_height.is_some();
            fallback.flex_grow = Some(if fixed { 0.0 } else { 1.0 });
            fallback.flex_shrink = Some(if fixed { 0.0 } else { 1.0 });
        }
        WidgetKind::HtmlReport => {
            let fixed = node.props.fixed_width.is_some() || node.props.fixed_height.is_some();
            fallback.flex_grow = Some(if fixed { 0.0 } else { 1.0 });
            fallback.flex_shrink = Some(1.0);
        }
        WidgetKind::Spacer => {
            let flexible = node.props.fixed_width.is_none() && node.props.fixed_height.is_none();
            fallback.flex_grow = Some(if flexible { 1.0 } else { 0.0 });
            fallback.flex_shrink = Some(if flexible { 1.0 } else { 0.0 });
        }
        WidgetKind::Tabs => {
            let has_tab_content = node
                .children
                .iter()
                .any(|child| child.kind == WidgetKind::Tab && !child.children.is_empty());
            fallback.flex_grow = Some(if has_tab_content { 1.0 } else { 0.0 });
            fallback.flex_shrink = Some(if has_tab_content { 1.0 } else { 0.0 });
        }
        _ => {}
    }

    let props_can_fix_layout = !matches!(
        node.kind,
        WidgetKind::Tooltip | WidgetKind::Toast | WidgetKind::ContextMenu | WidgetKind::Modal
    );
    let fixed_width_applies = node.kind != WidgetKind::Sidebar
        && computed_style.layout.width.is_none()
        && computed_style.layout.width_value.is_none()
        && node
            .props
            .fixed_width
            .is_some_and(|value| value.is_finite() && value >= 0.0);
    let fixed_height_applies = !matches!(node.kind, WidgetKind::MenuBar | WidgetKind::StatusBar)
        && computed_style.layout.height.is_none()
        && computed_style.layout.height_value.is_none()
        && node
            .props
            .fixed_height
            .is_some_and(|value| value.is_finite() && value >= 0.0);
    if props_can_fix_layout && (fixed_width_applies || fixed_height_applies) {
        fallback.flex_grow = Some(0.0);
        if node.kind != WidgetKind::Sidebar {
            fallback.flex_shrink = Some(0.0);
        }
    }

    let authored_preferred_main_size = match context.parent_flex_direction {
        Some(FlexDirectionStyle::Row | FlexDirectionStyle::RowReverse) => {
            computed_style.layout.width.is_some() || computed_style.layout.width_value.is_some()
        }
        Some(FlexDirectionStyle::Column | FlexDirectionStyle::ColumnReverse) => {
            computed_style.layout.height.is_some() || computed_style.layout.height_value.is_some()
        }
        None => false,
    };
    if authored_preferred_main_size && computed_style.layout.flex_grow.is_none() {
        fallback.flex_grow = Some(0.0);
    }
    if authored_preferred_main_size
        && context.parent_preserves_preferred_main_size
        && computed_style.layout.flex_shrink.is_none()
    {
        fallback.flex_shrink = Some(0.0);
    }
    if context.parent_kind == Some(WidgetKind::ScrollArea) {
        fallback.flex_shrink = Some(0.0);
    }
    if context.parent_preserves_preferred_main_size
        && computed_style.layout.flex_shrink.is_none()
        && matches!(
            node.kind,
            WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Collapsible
        )
    {
        fallback.flex_shrink = Some(0.0);
    }
    fallback
}

pub(crate) fn child_layout_fallback_context(parent: &WidgetNode) -> NativeLayoutFallbackContext {
    let parent_fallback = resolved_widget_layout_fallback(
        parent,
        &parent.style,
        NativeLayoutFallbackContext::default(),
        None,
    );
    let parent_flex_direction = parent
        .style
        .layout
        .flex_direction
        .or(parent_fallback.flex_direction)
        .or(Some(FlexDirectionStyle::Row));
    let parent_preserves_preferred_main_size = match parent_flex_direction {
        Some(FlexDirectionStyle::Row | FlexDirectionStyle::RowReverse) => {
            node_overflow_x(parent).is_some()
        }
        Some(FlexDirectionStyle::Column | FlexDirectionStyle::ColumnReverse) => {
            node_overflow_y(parent).is_some()
        }
        None => false,
    };
    NativeLayoutFallbackContext {
        parent_kind: Some(parent.kind),
        parent_flex_direction,
        parent_preserves_preferred_main_size,
    }
}

fn apply_resolved_widget_layout_fallback(
    style: &mut Style,
    node: &WidgetNode,
    context: NativeLayoutFallbackContext,
    state: Option<&WidgetState>,
) {
    let fallback = resolved_widget_layout_fallback(
        node,
        &node.style,
        context,
        state.and_then(|state| state.pane_size(&node.id)),
    );
    if let Some(display) = fallback.display {
        style.display = match display {
            DisplayStyle::Flex => Display::Flex,
            DisplayStyle::Grid => Display::Grid,
            DisplayStyle::Block => Display::Block,
            DisplayStyle::None => Display::None,
        };
    }
    if let Some(direction) = fallback.flex_direction {
        style.flex_direction = match direction {
            FlexDirectionStyle::Row => FlexDirection::Row,
            FlexDirectionStyle::Column => FlexDirection::Column,
            FlexDirectionStyle::RowReverse => FlexDirection::RowReverse,
            FlexDirectionStyle::ColumnReverse => FlexDirection::ColumnReverse,
        };
    }
    if let Some(wrap) = fallback.flex_wrap {
        style.flex_wrap = match wrap {
            FlexWrapStyle::NoWrap => FlexWrap::NoWrap,
            FlexWrapStyle::Wrap => FlexWrap::Wrap,
            FlexWrapStyle::WrapReverse => FlexWrap::WrapReverse,
        };
    }
    if let Some(grow) = fallback.flex_grow {
        style.flex_grow = grow;
    }
    if let Some(shrink) = fallback.flex_shrink {
        style.flex_shrink = shrink;
    }
}

fn apply_resolved_widget_geometry_fallback(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
) {
    let fallback = resolved_widget_geometry_fallback(node, &node.style, theme);
    if let Some(width) = fallback.width {
        style.size.width = Dimension::Length(width * sf);
    }
    if let Some(height) = fallback.height {
        style.size.height = Dimension::Length(height * sf);
    }
    if let Some(min_width) = fallback.min_width {
        style.min_size.width = Dimension::Length(min_width * sf);
    }
    if let Some(min_height) = fallback.min_height {
        style.min_size.height = Dimension::Length(min_height * sf);
    }
}

fn raw_prop_f32(node: &WidgetNode, name: &str) -> Option<f32> {
    node.props
        .raw_props
        .get(name)
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
}

fn loading_spinner_size_lp(node: &WidgetNode) -> f32 {
    raw_prop_f32(node, "size")
        .filter(|value| *value > 0.0)
        .unwrap_or(LOADING_SPINNER_DEFAULT_SIZE_LP)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarPresentation {
    Expanded,
    Collapsed,
    Hidden,
    Drawer,
}

fn sidebar_presentation(
    node: &WidgetNode,
    parent_size: Option<(f32, f32)>,
    sf: f32,
) -> SidebarPresentation {
    let state = node
        .props
        .raw_props
        .get("state")
        .and_then(|value| value.as_str())
        .unwrap_or("auto");
    match state {
        "expanded" => SidebarPresentation::Expanded,
        "collapsed" => SidebarPresentation::Collapsed,
        "hidden" => SidebarPresentation::Hidden,
        "drawer" => SidebarPresentation::Drawer,
        _ => {
            let logical_width = parent_size
                .map(|(width, _)| width / sf.max(0.001))
                .unwrap_or(f32::INFINITY);
            if logical_width <= SIDEBAR_MOBILE_BREAKPOINT_LP {
                match node
                    .props
                    .raw_props
                    .get("mobile_mode")
                    .and_then(|value| value.as_str())
                    .unwrap_or("drawer")
                {
                    "rail" => SidebarPresentation::Collapsed,
                    "hidden" => SidebarPresentation::Hidden,
                    _ => SidebarPresentation::Hidden,
                }
            } else if logical_width <= SIDEBAR_COMPACT_BREAKPOINT_LP {
                match node
                    .props
                    .raw_props
                    .get("compact_mode")
                    .and_then(|value| value.as_str())
                    .unwrap_or("rail")
                {
                    "hidden" => SidebarPresentation::Hidden,
                    _ => SidebarPresentation::Collapsed,
                }
            } else {
                SidebarPresentation::Expanded
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public result type
// ---------------------------------------------------------------------------

/// Axis-aligned pixel rectangle in window space (top-left origin).
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn intersect(self, other: Rect) -> Option<Rect> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = (self.x + self.w).min(other.x + other.w);
        let bottom = (self.y + self.h).min(other.y + other.h);
        if right <= left || bottom <= top {
            return None;
        }
        Some(Rect {
            x: left,
            y: top,
            w: right - left,
            h: bottom - top,
        })
    }
}

fn empty_rect_within(rect: Rect, bounds: Rect) -> Rect {
    let right = bounds.x + bounds.w.max(0.0);
    let bottom = bounds.y + bounds.h.max(0.0);
    Rect {
        x: rect.x.max(bounds.x).min(right),
        y: rect.y.max(bounds.y).min(bottom),
        w: 0.0,
        h: 0.0,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ResolvedEdges {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct ResolvedBox {
    border_box: Rect,
    padding_box: Rect,
    content_box: Rect,
    padding: ResolvedEdges,
}

impl ResolvedBox {
    fn from_rect(rect: Rect, border: ResolvedEdges, padding: ResolvedEdges) -> Self {
        let padding_box = inset_rect(rect, border);
        let content_box = inset_rect(padding_box, padding);
        Self {
            border_box: rect,
            padding_box,
            content_box,
            padding,
        }
    }
}

fn inset_rect(rect: Rect, edges: ResolvedEdges) -> Rect {
    Rect {
        x: rect.x + edges.left,
        y: rect.y + edges.top,
        w: (rect.w - edges.left - edges.right).max(0.0),
        h: (rect.h - edges.top - edges.bottom).max(0.0),
    }
}

/// Maps each widget `id` to its computed pixel rect and visible clipped rect.
#[derive(Debug, Default)]
pub struct LayoutResult {
    pub rects: HashMap<String, Rect>,
    pub clips: HashMap<String, Rect>,
    pub paint_clips: HashMap<String, Rect>,
    pub scroll_x: HashMap<String, f32>,
    pub scroll_y: HashMap<String, f32>,
    pub scroll_max_x: HashMap<String, f32>,
    pub scroll_max_y: HashMap<String, f32>,
    pub scale_factor: f32,
    pub reconciliation_iterations: usize,
    pub reconciliation_converged: bool,
    /// Resolved physical-pixel column tracks for each populated GridLayout.
    pub resolved_grid_tracks: HashMap<String, ResolvedGridTracks>,
    resolved_borders: HashMap<String, ResolvedEdges>,
    resolved_padding: HashMap<String, ResolvedEdges>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResolvedGridTracks {
    pub column_count: usize,
    pub column_widths: Vec<f32>,
}

impl LayoutResult {
    pub fn visible_rect(&self, id: &str) -> Option<Rect> {
        self.clips
            .get(id)
            .copied()
            .or_else(|| self.rects.get(id).copied())
            .filter(|rect| rect.w > 0.0 && rect.h > 0.0)
    }

    pub fn paint_clip_rect(&self, id: &str) -> Option<Rect> {
        self.paint_clips
            .get(id)
            .copied()
            .filter(|rect| rect.w > 0.0 && rect.h > 0.0)
    }

    fn resolved_box(&self, id: &str) -> Option<ResolvedBox> {
        let rect = self.rects.get(id).copied()?;
        Some(ResolvedBox::from_rect(
            rect,
            self.resolved_borders.get(id).copied().unwrap_or_default(),
            self.resolved_padding.get(id).copied().unwrap_or_default(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Compute a flexbox layout for `root` given a `window_w × window_h` physical
/// pixel canvas and a HiDPI `scale_factor`.
///
/// Layout constants (control height, padding, gap) are defined in logical
/// pixels and multiplied by `scale_factor` to yield physical pixel sizes,
/// matching the physical pixel coordinates that wgpu uses.
///
/// Returns absolute physical pixel rects for every node in the tree.
pub fn compute_layout(
    root: &WidgetNode,
    window_w: f32,
    window_h: f32,
    scale_factor: f32,
    theme: &Theme,
    state: Option<&WidgetState>,
) -> LayoutResult {
    let mut tree: TaffyTree<LeafMeasureContext> = TaffyTree::new();
    let root_id = build_node(
        &mut tree,
        root,
        scale_factor,
        theme,
        Some((window_w, window_h)),
        None,
        None,
        None,
        None,
        None,
        false,
        false,
        false,
        state,
        None,
        window_w / scale_factor.max(0.001),
    );

    compute_taffy_layout(
        &mut tree,
        root_id,
        Size {
            width: AvailableSpace::Definite(window_w),
            height: AvailableSpace::Definite(window_h),
        },
        theme,
    )
    .expect("taffy layout failed");

    let mut result = LayoutResult {
        scale_factor,
        reconciliation_converged: true,
        ..LayoutResult::default()
    };
    collect(&tree, root_id, root, 0.0, 0.0, &mut result);
    apply_titled_container_absolute_offsets(root, &mut result, scale_factor, theme);
    apply_navigation_layout(root, &mut result, scale_factor, theme, state);
    apply_modal_layout(root, &mut result, scale_factor, theme);
    apply_grid_auto_row_positions(root, &mut result, scale_factor, theme);
    collect_resolved_grid_tracks(root, &mut result);
    apply_grid_last_row_balance(root, &mut result);
    compute_pre_scroll_clips(root, &mut result, scale_factor, theme);
    apply_scroll_offsets(root, &mut result, scale_factor, theme, state);
    apply_fixed_positions(root, &mut result, scale_factor);
    apply_tooltip_layout(root, &mut result, scale_factor, theme, state);
    compute_clips(root, &mut result, scale_factor, theme);
    retain_active_layout_maps(&mut result);
    result
}

fn retain_active_layout_maps(result: &mut LayoutResult) {
    let active: HashSet<String> = result.rects.keys().cloned().collect();
    result.clips.retain(|id, _| active.contains(id));
    result.paint_clips.retain(|id, _| active.contains(id));
    result.scroll_x.retain(|id, _| active.contains(id));
    result.scroll_y.retain(|id, _| active.contains(id));
    result.scroll_max_x.retain(|id, _| active.contains(id));
    result.scroll_max_y.retain(|id, _| active.contains(id));
    result
        .resolved_grid_tracks
        .retain(|id, _| active.contains(id));
}

// ---------------------------------------------------------------------------
// Tree builder
// ---------------------------------------------------------------------------

fn build_node(
    tree: &mut TaffyTree<LeafMeasureContext>,
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    size_override: Option<(f32, f32)>,
    parent_size: Option<(f32, f32)>,
    parent_kind: Option<&WidgetKind>,
    parent_flex_direction: Option<FlexDirection>,
    parent_align_items: Option<AlignItems>,
    parent_splitter_pane_budget: Option<f32>,
    parent_preserves_preferred_main_size: bool,
    parent_allows_intrinsic_leaf_width: bool,
    layout_modal_children: bool,
    state: Option<&WidgetState>,
    parent_grid_areas: Option<&GridTemplateAreas>,
    viewport_width_lp: f32,
) -> NodeId {
    let mut style = style_for_with_viewport(
        node,
        sf,
        theme,
        parent_size,
        parent_kind,
        parent_flex_direction,
        parent_align_items,
        parent_splitter_pane_budget,
        parent_preserves_preferred_main_size,
        parent_allows_intrinsic_leaf_width,
        layout_modal_children,
        state,
        viewport_width_lp,
    );
    apply_parent_grid_area_placement(&mut style, node, parent_grid_areas);
    if let Some((w, h)) = size_override {
        let viewport_size = taffy::geometry::Size {
            width: Dimension::Length(w),
            height: Dimension::Length(h),
        };
        style.size = viewport_size;
        style.min_size = viewport_size;
        style.max_size = viewport_size;
    }
    let child_allows_intrinsic_leaf_width = allows_intrinsic_leaf_width_for_children(node, &style);
    let child_parent_size = definite_content_size(&style, parent_size);
    let child_splitter_pane_budget = splitter_child_pane_budget(node, child_parent_size, sf);
    let skip_children = matches!(
        node.kind,
        WidgetKind::Tabs
            | WidgetKind::Pages
            | WidgetKind::Menu
            | WidgetKind::ContextMenu
            | WidgetKind::Tooltip
            | WidgetKind::Toast
    ) || (node.kind == WidgetKind::Modal && !layout_modal_children)
        || (node.kind == WidgetKind::Collapsible && !collapsible_expanded(node, state));
    let skip_children =
        skip_children || (node.kind == WidgetKind::TreeNode && !tree_node_expanded(node, state));
    let child_ids: Vec<NodeId> = if skip_children {
        Vec::new()
    } else if titled_container_uses_body_layout(node) {
        let body_style = titled_container_body_style(node, sf, theme);
        let body_parent_size = definite_content_size(&body_style, child_parent_size);
        let body_child_ids: Vec<NodeId> = node
            .children
            .iter()
            .map(|c| {
                build_node(
                    tree,
                    c,
                    sf,
                    theme,
                    None,
                    body_parent_size,
                    Some(&node.kind),
                    Some(body_style.flex_direction),
                    body_style.align_items,
                    None,
                    preserves_child_preferred_main_size(node, body_style.flex_direction),
                    child_allows_intrinsic_leaf_width,
                    layout_modal_children,
                    state,
                    node.style.layout.grid_template_areas.as_ref(),
                    viewport_width_lp,
                )
            })
            .collect();
        vec![tree
            .new_with_children(body_style, &body_child_ids)
            .expect("taffy titled body node failed")]
    } else {
        node.children
            .iter()
            .map(|c| {
                build_node(
                    tree,
                    c,
                    sf,
                    theme,
                    None,
                    child_parent_size,
                    Some(&node.kind),
                    Some(style.flex_direction),
                    style.align_items,
                    child_splitter_pane_budget,
                    preserves_child_preferred_main_size(node, style.flex_direction),
                    child_allows_intrinsic_leaf_width,
                    layout_modal_children,
                    state,
                    node.style.layout.grid_template_areas.as_ref(),
                    viewport_width_lp,
                )
            })
            .collect()
    };
    if child_ids.is_empty() {
        if let Some(context) = leaf_measure_context(node, sf, theme) {
            tree.new_leaf_with_context(style, context)
                .expect("taffy new_leaf_with_context failed")
        } else {
            tree.new_leaf(style).expect("taffy new_leaf failed")
        }
    } else {
        tree.new_with_children(style, &child_ids)
            .expect("taffy new_with_children failed")
    }
}

#[derive(Debug, Clone)]
struct LeafMeasureContext {
    text: String,
    text_style: crate::style::TextStyle,
    scale_factor: f32,
    control_height_lp: f32,
    intrinsic_width_px: f32,
    wraps: bool,
    wrapped_heights_px: Vec<(i32, f32)>,
}

fn leaf_measure_context(
    node: &WidgetNode,
    scale_factor: f32,
    theme: &Theme,
) -> Option<LeafMeasureContext> {
    if node.kind != WidgetKind::Label {
        return None;
    }
    let text = node.props.text.clone().unwrap_or_default();
    let scale_factor = scale_factor.max(0.001);
    let intrinsic_width_px = measure_text_for_layout(&text, &node.style.text, theme)
        .width
        .ceil()
        * scale_factor;
    Some(LeafMeasureContext {
        text,
        text_style: node.style.text.clone(),
        scale_factor,
        control_height_lp: node_control_height_lp(node, theme),
        intrinsic_width_px,
        wraps: label_wraps(node),
        wrapped_heights_px: Vec::new(),
    })
}

fn compute_taffy_layout(
    tree: &mut TaffyTree<LeafMeasureContext>,
    root_id: NodeId,
    available_space: Size<AvailableSpace>,
    theme: &Theme,
) -> Result<(), taffy::TaffyError> {
    tree.compute_layout_with_measure(
        root_id,
        available_space,
        |known, available, _node_id, context, _style| {
            let Some(context) = context else {
                return Size::ZERO;
            };
            let sf = context.scale_factor;
            let width = known.width.unwrap_or(context.intrinsic_width_px);
            let height = known.height.unwrap_or_else(|| {
                if !context.wraps || context.text.is_empty() {
                    return context.control_height_lp * sf;
                }
                let available_width = known.width.or_else(|| match available.width {
                    AvailableSpace::Definite(value) => Some(value),
                    AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
                });
                available_width.map_or(context.control_height_lp * sf, |width| {
                    let cache_key = (width * 4.0).round() as i32;
                    if let Some((_, height)) = context
                        .wrapped_heights_px
                        .iter()
                        .find(|(key, _)| *key == cache_key)
                    {
                        return *height;
                    }
                    let height = measure_wrapped_text_for_layout(
                        &context.text,
                        &context.text_style,
                        theme,
                        (width / sf).max(node_font_size_for_measure(&context.text_style, theme)),
                    )
                    .height
                    .max(context.control_height_lp)
                        * sf;
                    context.wrapped_heights_px.push((cache_key, height));
                    height
                })
            });
            Size { width, height }
        },
    )
}

fn node_font_size_for_measure(text_style: &crate::style::TextStyle, theme: &Theme) -> f32 {
    text_style
        .font_size
        .unwrap_or_else(|| crate::style::native_fallback_font_size(theme))
        .max(1.0)
}

fn splitter_child_pane_budget(
    node: &WidgetNode,
    parent_size: Option<(f32, f32)>,
    sf: f32,
) -> Option<f32> {
    if node.kind != WidgetKind::Splitter || node.children.is_empty() {
        return None;
    }
    let horizontal = node.props.orientation.as_deref().unwrap_or("horizontal") != "vertical";
    let main_size = parent_size.map(|size| if horizontal { size.0 } else { size.1 })?;
    let gutter = node.props.gutter_size.unwrap_or(6.0).max(1.0) * sf;
    let total_gutter = gutter * node.children.len().saturating_sub(1) as f32;
    Some(((main_size - total_gutter).max(0.0) / node.children.len() as f32).max(0.0))
}

fn preserves_child_preferred_main_size(node: &WidgetNode, flex_direction: FlexDirection) -> bool {
    let overflow = if matches!(
        flex_direction,
        FlexDirection::Row | FlexDirection::RowReverse
    ) {
        node_overflow_x(node)
    } else {
        node_overflow_y(node)
    };
    overflow.is_some()
}

fn apply_parent_grid_area_placement(
    style: &mut Style,
    node: &WidgetNode,
    parent_grid_areas: Option<&GridTemplateAreas>,
) {
    if style.grid_column.start != GridPlacement::Auto || style.grid_row.start != GridPlacement::Auto
    {
        return;
    }
    let Some(area_name) = node.style.layout.grid_area.as_deref() else {
        return;
    };
    let Some(area) = parent_grid_areas.and_then(|areas| areas.area_named(area_name)) else {
        return;
    };
    style.grid_column = taffy::geometry::Line {
        start: GridPlacement::from_line_index(area.column_start as i16),
        end: GridPlacement::from_line_index(area.column_end as i16),
    };
    style.grid_row = taffy::geometry::Line {
        start: GridPlacement::from_line_index(area.row_start as i16),
        end: GridPlacement::from_line_index(area.row_end as i16),
    };
}

fn allows_intrinsic_leaf_width_for_children(node: &WidgetNode, style: &Style) -> bool {
    if matches!(
        node.kind,
        WidgetKind::GridLayout
            | WidgetKind::MenuBar
            | WidgetKind::DragSource
            | WidgetKind::DropTarget
    ) {
        return true;
    }
    matches!(
        style.flex_direction,
        FlexDirection::Row | FlexDirection::RowReverse
    )
}

// ---------------------------------------------------------------------------
// Style mapping
// ---------------------------------------------------------------------------

// Logical-pixel constants — multiplied by scale_factor before use.
#[cfg(test)]
fn style_for(
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_size: Option<(f32, f32)>,
    parent_kind: Option<&WidgetKind>,
    parent_flex_direction: Option<FlexDirection>,
    parent_splitter_pane_budget: Option<f32>,
    parent_preserves_preferred_main_size: bool,
    parent_allows_intrinsic_leaf_width: bool,
    layout_modal_children: bool,
    state: Option<&WidgetState>,
) -> Style {
    let viewport_width_lp = parent_size
        .map(|(width, _)| width / sf.max(0.001))
        .unwrap_or(f32::INFINITY);
    style_for_with_viewport(
        node,
        sf,
        theme,
        parent_size,
        parent_kind,
        parent_flex_direction,
        None,
        parent_splitter_pane_budget,
        parent_preserves_preferred_main_size,
        parent_allows_intrinsic_leaf_width,
        layout_modal_children,
        state,
        viewport_width_lp,
    )
}

fn style_for_with_viewport(
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_size: Option<(f32, f32)>,
    parent_kind: Option<&WidgetKind>,
    parent_flex_direction: Option<FlexDirection>,
    parent_align_items: Option<AlignItems>,
    parent_splitter_pane_budget: Option<f32>,
    parent_preserves_preferred_main_size: bool,
    parent_allows_intrinsic_leaf_width: bool,
    layout_modal_children: bool,
    state: Option<&WidgetState>,
    viewport_width_lp: f32,
) -> Style {
    let ctrl_gap = (theme.spacing * 0.75) * sf;
    let panel_pad = theme.panel_padding.max(0.0) * sf;
    let mut style = match node.kind {
        // ── containers ──────────────────────────────────────────────────────
        WidgetKind::Window => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::HLayout => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 1.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::VLayout => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 1.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::ScrollArea => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            overflow: taffy::geometry::Point {
                x: Overflow::Hidden,
                y: Overflow::Scroll,
            },
            ..Default::default()
        },

        WidgetKind::GridLayout => Style {
            display: Display::Grid,
            flex_grow: 1.0,
            flex_shrink: 1.0,
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::FlowLayout => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: Some(AlignItems::FlexStart),
            justify_content: Some(JustifyContent::FlexStart),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Splitter => {
            let horizontal =
                node.props.orientation.as_deref().unwrap_or("horizontal") != "vertical";
            let gutter = node.props.gutter_size.unwrap_or(6.0).max(1.0) * sf;
            Style {
                align_items: Some(AlignItems::Stretch),
                size: Size {
                    width: Dimension::Auto,
                    height: Dimension::Auto,
                },
                min_size: Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(0.0),
                },
                gap: taffy::geometry::Size {
                    width: LengthPercentage::Length(if horizontal { gutter } else { 0.0 }),
                    height: LengthPercentage::Length(if horizontal { 0.0 } else { gutter }),
                },
                ..Default::default()
            }
        }

        WidgetKind::Pane => {
            let horizontal =
                node.props.orientation.as_deref().unwrap_or("horizontal") != "vertical";
            let requested_size = state
                .and_then(|state| state.pane_size(&node.id))
                .or(node.props.pane_size)
                .filter(|size| size.is_finite())
                .map(|size| size.max(0.0));
            let active_size = requested_size
                .filter(|size| !(*size > 0.0 && *size < 1.0))
                .map(|size| size * sf);
            let min_size = (node.props.pane_min_size.unwrap_or(0.0).max(0.0) * sf)
                .min(parent_splitter_pane_budget.unwrap_or(f32::INFINITY));
            let max_size = node
                .props
                .pane_max_size
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| value * sf);
            let mut pane_style = Style {
                align_items: Some(AlignItems::Stretch),
                size: Size {
                    width: Dimension::Auto,
                    height: Dimension::Auto,
                },
                min_size: Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(0.0),
                },
                max_size: Size {
                    width: Dimension::Auto,
                    height: Dimension::Auto,
                },
                ..Default::default()
            };
            if horizontal {
                if let Some(size) = active_size {
                    pane_style.size.width = Dimension::Length(size);
                }
                pane_style.min_size.width = Dimension::Length(min_size);
                if let Some(size) = max_size {
                    pane_style.max_size.width = Dimension::Length(size);
                }
            } else {
                if let Some(size) = active_size {
                    pane_style.size.height = Dimension::Length(size);
                }
                pane_style.min_size.height = Dimension::Length(min_size);
                if let Some(size) = max_size {
                    pane_style.max_size.height = Dimension::Length(size);
                }
            }
            pane_style
        }

        WidgetKind::TreeView => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            gap: taffy::geometry::Size {
                width: LengthPercentage::Length(0.0),
                height: LengthPercentage::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::DragSource | WidgetKind::DropTarget => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 0.0,
            flex_shrink: 1.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::StatusBar => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(
                    parent_size
                        .map(|(_, parent_h)| {
                            (node.props.fixed_height.unwrap_or(28.0) * sf)
                                .min(parent_h.max(0.0) * 0.25)
                        })
                        .unwrap_or_else(|| node.props.fixed_height.unwrap_or(28.0) * sf),
                ),
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            padding: taffy::geometry::Rect {
                left: LengthPercentage::Length(panel_pad),
                right: LengthPercentage::Length(panel_pad),
                top: LengthPercentage::Length(0.0),
                bottom: LengthPercentage::Length(0.0),
            },
            gap: taffy::geometry::Size {
                width: LengthPercentage::Length(ctrl_gap),
                height: LengthPercentage::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::MenuBar => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(
                    parent_size
                        .map(|(_, parent_h)| {
                            (node
                                .props
                                .fixed_height
                                .unwrap_or_else(|| node_control_height_lp(node, theme))
                                .max(node_control_height_lp(node, theme))
                                * sf)
                                .min(parent_h.max(0.0) * 0.25)
                        })
                        .unwrap_or_else(|| {
                            node.props
                                .fixed_height
                                .unwrap_or_else(|| node_control_height_lp(node, theme))
                                .max(node_control_height_lp(node, theme))
                                * sf
                        }),
                ),
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            padding: taffy::geometry::Rect {
                left: LengthPercentage::Length(theme.spacing * 0.5 * sf),
                right: LengthPercentage::Length(theme.spacing * 0.5 * sf),
                top: LengthPercentage::Length(0.0),
                bottom: LengthPercentage::Length(0.0),
            },
            gap: taffy::geometry::Size {
                width: LengthPercentage::Length(2.0 * sf),
                height: LengthPercentage::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Panel | WidgetKind::Sidebar => {
            let preferred_width = if node.kind == WidgetKind::Sidebar
                && sidebar_presentation(node, parent_size, sf) == SidebarPresentation::Collapsed
            {
                raw_prop_f32(node, "collapsed_width")
                    .filter(|width| *width > 0.0)
                    .or(node.props.fixed_width)
            } else {
                node.props.fixed_width
            };
            let width = match preferred_width {
                Some(w) => {
                    let requested = w * sf;
                    let responsive = if node.kind == WidgetKind::Sidebar {
                        parent_size
                            .map(|(parent_w, _)| requested.min(parent_w.max(0.0) * 0.5))
                            .unwrap_or(requested)
                    } else {
                        requested
                    };
                    Dimension::Length(responsive)
                }
                None => Dimension::Auto,
            };
            Style {
                size: Size {
                    width,
                    height: Dimension::Auto,
                },
                min_size: Size {
                    width: Dimension::Length(0.0),
                    height: if node.kind == WidgetKind::Panel
                        && matches!(parent_kind, Some(WidgetKind::GridLayout))
                    {
                        // Preserve the content contribution of auto-height
                        // framed grid items. An authored min-height: 0 still
                        // overrides this when deliberate shrinking is wanted.
                        Dimension::Auto
                    } else {
                        Dimension::Length(0.0)
                    },
                },
                padding: taffy::geometry::Rect {
                    left: LengthPercentage::Length(panel_pad),
                    right: LengthPercentage::Length(panel_pad),
                    top: LengthPercentage::Length(panel_pad),
                    bottom: LengthPercentage::Length(panel_pad),
                },
                gap: taffy::geometry::Size {
                    width: LengthPercentage::Length(0.0),
                    height: LengthPercentage::Length(ctrl_gap),
                },
                ..Default::default()
            }
        }

        WidgetKind::Collapsible => {
            let expanded = collapsible_expanded(node, state);
            let header_h = collapsible_header_height_for_style(&node.style, theme, sf);
            let body_pad = if expanded { panel_pad } else { 0.0 };
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: Some(AlignItems::Stretch),
                flex_grow: 0.0,
                flex_shrink: 0.0,
                size: Size {
                    width: Dimension::Auto,
                    height: Dimension::Auto,
                },
                min_size: Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(header_h),
                },
                padding: taffy::geometry::Rect {
                    left: LengthPercentage::Length(body_pad),
                    right: LengthPercentage::Length(body_pad),
                    top: LengthPercentage::Length(header_h + body_pad),
                    bottom: LengthPercentage::Length(body_pad),
                },
                gap: taffy::geometry::Size {
                    width: LengthPercentage::Length(0.0),
                    height: LengthPercentage::Length(ctrl_gap),
                },
                ..Default::default()
            }
        }

        WidgetKind::Modal if !layout_modal_children => Style {
            flex_grow: 0.0,
            flex_shrink: 0.0,
            size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Tooltip | WidgetKind::Toast => Style {
            flex_grow: 0.0,
            flex_shrink: 0.0,
            size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Modal => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            padding: taffy::geometry::Rect {
                left: LengthPercentage::Length(panel_pad),
                right: LengthPercentage::Length(panel_pad),
                top: LengthPercentage::Length(panel_pad),
                bottom: LengthPercentage::Length(panel_pad),
            },
            gap: taffy::geometry::Size {
                width: LengthPercentage::Length(0.0),
                height: LengthPercentage::Length(ctrl_gap),
            },
            ..Default::default()
        },

        // ── leaf controls ────────────────────────────────────────────────────
        WidgetKind::TreeNode => {
            let expanded = tree_node_expanded(node, state);
            let row_h = tree_node_row_height_for_style(node, theme, sf, parent_size.map(|s| s.1));
            if expanded && !node.children.is_empty() {
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: Some(AlignItems::Stretch),
                    flex_grow: 0.0,
                    flex_shrink: 0.0,
                    size: Size {
                        width: Dimension::Auto,
                        height: Dimension::Auto,
                    },
                    min_size: Size {
                        width: Dimension::Length(0.0),
                        height: Dimension::Length(row_h),
                    },
                    padding: taffy::geometry::Rect {
                        left: LengthPercentage::Length((theme.spacing + 8.0) * sf),
                        right: LengthPercentage::Length(0.0),
                        top: LengthPercentage::Length(row_h),
                        bottom: LengthPercentage::Length(0.0),
                    },
                    gap: taffy::geometry::Size {
                        width: LengthPercentage::Length(0.0),
                        height: LengthPercentage::Length(0.0),
                    },
                    ..Default::default()
                }
            } else {
                Style {
                    size: Size {
                        width: Dimension::Auto,
                        height: Dimension::Length(row_h),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            }
        }

        WidgetKind::IconButton | WidgetKind::ImageButton | WidgetKind::ArrowButton => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Button
        | WidgetKind::SmallButton
        | WidgetKind::Selectable
        | WidgetKind::RadioButton
        | WidgetKind::Dropdown
        | WidgetKind::Menu
        | WidgetKind::MenuItem
        | WidgetKind::NumberInput
        | WidgetKind::DragNumber
        | WidgetKind::NavItem
        | WidgetKind::Tab => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Badge | WidgetKind::Tag => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Led => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Checkbox | WidgetKind::ToggleSwitch => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Label => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::LoadingSpinner => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Slider
        | WidgetKind::RangeSlider
        | WidgetKind::ProgressBar
        | WidgetKind::TextInput => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        // Telemetry limit bars are frequently placed after a fixed readout
        // label. Let percentage-sized bars yield the label/gap allocation
        // instead of escaping their row or painting beneath a scrollbar.
        WidgetKind::LimitsBar => Style {
            flex_shrink: 1.0,
            ..Default::default()
        },

        WidgetKind::TextArea | WidgetKind::CodeEditor | WidgetKind::LogView => Style {
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Separator => {
            let orientation = separator_orientation(node, parent_kind);
            if orientation == SeparatorOrientation::Vertical {
                Style {
                    size: Size {
                        width: Dimension::Length(1.0 * sf),
                        height: Dimension::Auto,
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            } else {
                Style {
                    size: Size {
                        width: Dimension::Auto,
                        height: Dimension::Length(1.0 * sf),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                }
            }
        }

        WidgetKind::Image => {
            let width = node.props.fixed_width.map(|w| Dimension::Length(w * sf));
            let height = node.props.fixed_height.map(|h| Dimension::Length(h * sf));
            Style {
                size: Size {
                    width: width.unwrap_or(Dimension::Auto),
                    height: height.unwrap_or(Dimension::Auto),
                },
                ..Default::default()
            }
        }

        WidgetKind::HtmlReport => {
            let width = node.props.fixed_width.map(|w| Dimension::Length(w * sf));
            let height = node.props.fixed_height.map(|h| Dimension::Length(h * sf));
            Style {
                size: Size {
                    width: width.unwrap_or(Dimension::Auto),
                    height: height.unwrap_or(Dimension::Auto),
                },
                ..Default::default()
            }
        }

        WidgetKind::Extension => {
            let width = node
                .props
                .fixed_width
                .or(node.props.intrinsic_width)
                .map(|w| Dimension::Length(w * sf));
            let height = node
                .props
                .fixed_height
                .or(node.props.intrinsic_height)
                .map(|h| Dimension::Length(h * sf));
            Style {
                flex_grow: 0.0,
                flex_shrink: 1.0,
                size: Size {
                    width: width.unwrap_or(Dimension::Auto),
                    height: height.unwrap_or(Dimension::Auto),
                },
                ..Default::default()
            }
        }

        WidgetKind::ContextMenu => Style {
            flex_grow: 0.0,
            flex_shrink: 0.0,
            size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Spacer => {
            let width = node.props.fixed_width.map(|w| Dimension::Length(w * sf));
            let height = node.props.fixed_height.map(|h| Dimension::Length(h * sf));
            Style {
                size: Size {
                    width: width.unwrap_or(Dimension::Auto),
                    height: height.unwrap_or(Dimension::Auto),
                },
                ..Default::default()
            }
        }

        // ── plot / table: grow to fill remaining space ────────────────────
        WidgetKind::PieChart
        | WidgetKind::Histogram
        | WidgetKind::BarChart
        | WidgetKind::Heatmap
        | WidgetKind::LinePlot
        | WidgetKind::Scatter3D
        | WidgetKind::DataFrameTable => Style {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Tabs => Style {
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Pages => Style {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Page => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 1.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..Default::default()
        },

        WidgetKind::Unknown => Style {
            flex_grow: 1.0,
            ..Default::default()
        },
    };
    apply_resolved_widget_layout_fallback(
        &mut style,
        node,
        NativeLayoutFallbackContext {
            parent_kind: parent_kind.copied(),
            parent_flex_direction: parent_flex_direction.map(|direction| match direction {
                FlexDirection::Row => FlexDirectionStyle::Row,
                FlexDirection::Column => FlexDirectionStyle::Column,
                FlexDirection::RowReverse => FlexDirectionStyle::RowReverse,
                FlexDirection::ColumnReverse => FlexDirectionStyle::ColumnReverse,
            }),
            parent_preserves_preferred_main_size,
        },
        state,
    );
    apply_resolved_widget_geometry_fallback(&mut style, node, sf, theme);
    apply_node_prop_fixed_size(&mut style, node, sf, layout_modal_children);
    if !matches!(node.kind, WidgetKind::Tooltip | WidgetKind::Toast) {
        apply_node_style(
            &mut style,
            node,
            sf,
            parent_size,
            parent_flex_direction,
            parent_preserves_preferred_main_size,
        );
    }
    apply_intrinsic_leaf_width(
        &mut style,
        node,
        parent_kind,
        parent_preserves_preferred_main_size,
        parent_allows_intrinsic_leaf_width,
        sf,
        theme,
    );
    apply_compact_boolean_leaf_alignment(&mut style, node, parent_align_items);
    apply_inline_status_leaf_alignment(&mut style, node, parent_flex_direction);
    apply_scroll_area_child_content_sizing(&mut style, parent_kind);
    reserve_collapsible_header_space(&mut style, node, sf, theme, parent_size, state);
    normalize_tree_node_layout_style(&mut style, node, sf, theme, parent_size, state);
    apply_grid_masonry_item_alignment(&mut style, node);
    apply_grid_layout_default_tracks(&mut style, node, sf, parent_size, viewport_width_lp);
    apply_flow_layout_alignment(&mut style, node);
    if !titled_container_uses_body_layout(node)
        && (node.kind != WidgetKind::Modal || layout_modal_children)
        && node.kind != WidgetKind::Collapsible
    {
        reserve_panel_title_space(&mut style, node, sf, theme);
    }
    style
}

fn apply_node_prop_fixed_size(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    layout_modal_children: bool,
) {
    if matches!(
        node.kind,
        WidgetKind::Tooltip | WidgetKind::Toast | WidgetKind::ContextMenu
    ) || (node.kind == WidgetKind::Modal && !layout_modal_children)
    {
        return;
    }

    let mut fixed = false;
    if node.kind != WidgetKind::Sidebar
        && node.style.layout.width.is_none()
        && node.style.layout.width_value.is_none()
    {
        if let Some(width) = node
            .props
            .fixed_width
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            style.size.width = Dimension::Length(width * sf);
            fixed = true;
        }
    }
    if !matches!(node.kind, WidgetKind::MenuBar | WidgetKind::StatusBar)
        && node.style.layout.height.is_none()
        && node.style.layout.height_value.is_none()
    {
        if let Some(height) = node
            .props
            .fixed_height
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            style.size.height = Dimension::Length(height * sf);
            fixed = true;
        }
    }
    if fixed {
        style.flex_grow = 0.0;
        if node.kind != WidgetKind::Sidebar {
            style.flex_shrink = 0.0;
        }
    }
}

fn apply_scroll_area_child_content_sizing(style: &mut Style, parent_kind: Option<&WidgetKind>) {
    if matches!(parent_kind, Some(WidgetKind::ScrollArea)) {
        style.flex_shrink = 0.0;
    }
}

fn apply_grid_masonry_item_alignment(style: &mut Style, node: &WidgetNode) {
    if node.kind == WidgetKind::GridLayout
        && node.props.grid_masonry
        && node.style.layout.align_items.is_none()
    {
        style.align_items = Some(AlignItems::FlexStart);
    }
}

fn apply_grid_auto_row_positions(
    root: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
) {
    const MAX_RECONCILIATION_ITERATIONS: usize = 4;
    let mut stretched_height_floors = HashMap::new();
    collect_stretched_grid_height_floors(root, result, &mut stretched_height_floors);

    for iteration in 1..=MAX_RECONCILIATION_ITERATIONS {
        let mut changed = HashSet::new();
        apply_grid_auto_row_positions_node(
            root,
            result,
            sf,
            &stretched_height_floors,
            &mut changed,
        );
        if changed.is_empty() {
            return;
        }
        result.reconciliation_iterations = iteration;
        reconcile_layout_after_grid_adjustments(
            root,
            result,
            sf,
            theme,
            &stretched_height_floors,
            &changed,
        );
    }
    result.reconciliation_converged = false;
}

fn collect_resolved_grid_tracks(root: &WidgetNode, result: &mut LayoutResult) {
    result.resolved_grid_tracks.clear();
    collect_resolved_grid_tracks_node(root, result);
}

fn collect_resolved_grid_tracks_node(node: &WidgetNode, result: &mut LayoutResult) {
    if node.kind == WidgetKind::GridLayout {
        let mut tracks: Vec<(f32, f32)> = Vec::new();
        for child in node.children.iter().filter(|child| {
            !matches!(
                child.style.layout.position,
                Some(PositionStyle::Absolute | PositionStyle::Fixed)
            )
        }) {
            let Some(rect) = result.rects.get(&child.id).copied() else {
                continue;
            };
            if let Some((_, width)) = tracks.iter_mut().find(|(x, _)| (rect.x - *x).abs() <= 0.5) {
                *width = width.max(rect.w);
            } else {
                tracks.push((rect.x, rect.w));
            }
        }
        tracks.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if !tracks.is_empty() {
            result.resolved_grid_tracks.insert(
                node.id.clone(),
                ResolvedGridTracks {
                    column_count: tracks.len(),
                    column_widths: tracks.into_iter().map(|(_, width)| width).collect(),
                },
            );
        }
    }
    for child in &node.children {
        collect_resolved_grid_tracks_node(child, result);
    }
}

fn apply_grid_last_row_balance(root: &WidgetNode, result: &mut LayoutResult) {
    apply_grid_last_row_balance_node(root, result);
}

fn apply_grid_last_row_balance_node(node: &WidgetNode, result: &mut LayoutResult) {
    if node.kind == WidgetKind::GridLayout
        && node.props.grid_balance_last_row
        && !node.props.grid_masonry
        && node.style.layout.grid_template_columns.is_none()
        && node.props.grid_template_columns.is_none()
        && !node.children.iter().any(|child| {
            child.style.layout.grid_row.is_some() || child.style.layout.grid_column.is_some()
        })
    {
        balance_grid_last_row(node, result);
    }
    for child in &node.children {
        apply_grid_last_row_balance_node(child, result);
    }
}

fn balance_grid_last_row(grid: &WidgetNode, result: &mut LayoutResult) {
    let Some(tracks) = result.resolved_grid_tracks.get(&grid.id) else {
        return;
    };
    if tracks.column_count < 2 {
        return;
    }
    let entries: Vec<(usize, Rect)> = grid
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| {
            !matches!(
                child.style.layout.position,
                Some(PositionStyle::Absolute | PositionStyle::Fixed)
            )
        })
        .filter_map(|(index, child)| {
            result
                .rects
                .get(&child.id)
                .copied()
                .map(|rect| (index, rect))
        })
        .collect();
    let Some(last_y) = entries
        .iter()
        .map(|(_, rect)| rect.y)
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
    else {
        return;
    };
    let last_row: Vec<(usize, Rect)> = entries
        .into_iter()
        .filter(|(_, rect)| (rect.y - last_y).abs() <= 0.5)
        .collect();
    if last_row.is_empty() || last_row.len() >= tracks.column_count {
        return;
    }
    let used_left = last_row
        .iter()
        .map(|(_, rect)| rect.x)
        .fold(f32::INFINITY, f32::min);
    let used_right = last_row
        .iter()
        .map(|(_, rect)| rect.x + rect.w)
        .fold(f32::NEG_INFINITY, f32::max);
    let Some(grid_box) = result.resolved_box(&grid.id) else {
        return;
    };
    let content = grid_box.content_box;
    let target_left = content.x + (content.w - (used_right - used_left)).max(0.0) * 0.5;
    let dx = target_left - used_left;
    if dx.abs() <= 0.5 {
        return;
    }
    for (index, _) in last_row {
        translate_subtree(&grid.children[index], result, dx, 0.0);
    }
}

fn apply_grid_auto_row_positions_node(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    stretched_height_floors: &HashMap<String, f32>,
    changed: &mut HashSet<String>,
) {
    if node.kind == WidgetKind::GridLayout {
        let adjusted = if node.props.grid_masonry {
            pack_grid_masonry_columns(node, result, sf, stretched_height_floors)
        } else {
            grow_and_repack_ordinary_grid_rows(node, result, sf, stretched_height_floors)
        };
        if adjusted {
            changed.insert(node.id.clone());
        }
    }
    for child in &node.children {
        apply_grid_auto_row_positions_node(child, result, sf, stretched_height_floors, changed);
    }
}

fn grow_and_repack_ordinary_grid_rows(
    grid: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    stretched_height_floors: &HashMap<String, f32>,
) -> bool {
    if grid.style.layout.grid_template_rows.is_some()
        || grid.props.grid_template_rows.is_some()
        || matches!(
            grid.style.layout.grid_auto_flow,
            Some(GridAutoFlowStyle::Column | GridAutoFlowStyle::ColumnDense)
        )
        || grid.children.iter().any(|child| {
            child.style.layout.grid_row.is_some() || child.style.layout.grid_column.is_some()
        })
    {
        return false;
    }

    let mut grew_child = false;
    for child in &grid.children {
        if !auto_height_container_can_grow_to_overflowing_content(child, result) {
            continue;
        }
        let old_height = result
            .rects
            .get(&child.id)
            .map(|rect| rect.h)
            .unwrap_or(0.0);
        if resize_auto_height_container_to_children(child, result, sf, Some(old_height)) {
            grew_child = true;
        }
    }
    let Some(grid_rect) = result.rects.get(&grid.id).copied() else {
        return false;
    };
    let mut entries: Vec<(usize, Rect)> = grid
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| is_reflowable_normal_child(child))
        .filter_map(|(index, child)| {
            result
                .rects
                .get(&child.id)
                .copied()
                .map(|rect| (index, rect))
        })
        .collect();
    entries.sort_by(|left, right| {
        left.1
            .y
            .partial_cmp(&right.1.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.1
                    .x
                    .partial_cmp(&right.1.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    let Some(first) = entries.first().copied() else {
        return false;
    };
    let bottom_inset = resolved_bottom_outer_inset(result, &grid.id);
    let grid_content_bottom = entries
        .iter()
        .map(|(_, rect)| rect.y + rect.h)
        .fold(grid_rect.y, f32::max);
    let grid_bounds_are_stale =
        grid_content_bottom + bottom_inset > grid_rect.y + grid_rect.h + 0.5;
    if !grew_child && !grid_bounds_are_stale {
        return false;
    }

    let mut rows: Vec<Vec<(usize, Rect)>> = Vec::new();
    for entry in entries {
        if rows.last().is_some_and(|row| {
            row.first()
                .is_some_and(|(_, first_rect)| (entry.1.y - first_rect.y).abs() <= 0.5)
        }) {
            rows.last_mut().unwrap().push(entry);
        } else {
            rows.push(vec![entry]);
        }
    }

    let row_gap = grid_row_gap_px(grid, sf, Some(grid_rect.h));
    let mut cursor_y = first.1.y;
    let mut changed = false;
    for row in rows {
        let row_height = row
            .iter()
            .filter_map(|(index, _)| result.rects.get(&grid.children[*index].id))
            .map(|rect| rect.h)
            .fold(0.0, f32::max);
        for (index, old_rect) in row {
            let dy = cursor_y - old_rect.y;
            if dy.abs() > 0.5 {
                translate_subtree(&grid.children[index], result, 0.0, dy);
                changed = true;
            }
        }
        cursor_y += row_height + row_gap;
    }
    let content_bottom = cursor_y - row_gap.max(0.0);
    let new_height = (content_bottom + bottom_inset - grid_rect.y)
        .max(
            stretched_height_floors
                .get(&grid.id)
                .copied()
                .unwrap_or(0.0),
        )
        .max(0.0);
    if let Some(rect) = result.rects.get_mut(&grid.id) {
        if (rect.h - new_height).abs() > 0.5 {
            rect.h = new_height;
            changed = true;
        }
    }
    grew_child || changed
}

fn auto_height_container_can_grow_to_overflowing_content(
    node: &WidgetNode,
    result: &LayoutResult,
) -> bool {
    if !matches!(
        node.kind,
        WidgetKind::Panel
            | WidgetKind::Sidebar
            | WidgetKind::Modal
            | WidgetKind::VLayout
            | WidgetKind::FlowLayout
            | WidgetKind::Collapsible
            | WidgetKind::Page
    ) || node.props.fixed_height.is_some()
        || node.style.layout.height.is_some()
        || !matches!(
            node.style.layout.height_value,
            None | Some(LayoutLength::Auto)
        )
    {
        return false;
    }
    let Some(rect) = result.rects.get(&node.id) else {
        return false;
    };
    let content_bottom = node
        .children
        .iter()
        .filter(|child| is_reflowable_normal_child(child))
        .filter_map(|child| result.rects.get(&child.id))
        .map(|child| child.y + child.h)
        .fold(rect.y, f32::max);
    let bottom_inset = resolved_bottom_outer_inset(result, &node.id);
    content_bottom + bottom_inset > rect.y + rect.h + 0.5
}

fn pack_grid_masonry_columns(
    grid: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    stretched_height_floors: &HashMap<String, f32>,
) -> bool {
    if grid.style.layout.grid_template_rows.is_some()
        || grid.props.grid_template_rows.is_some()
        || grid.style.layout.grid_template_columns.is_some()
        || grid.props.grid_template_columns.is_some()
        || matches!(
            grid.style.layout.grid_auto_flow,
            Some(GridAutoFlowStyle::Column | GridAutoFlowStyle::ColumnDense)
        )
        || grid.children.iter().any(|child| {
            child.style.layout.grid_row.is_some() || child.style.layout.grid_column.is_some()
        })
    {
        return false;
    }

    let Some(grid_rect) = result.rects.get(&grid.id).copied() else {
        return false;
    };
    let entries: Vec<(usize, Rect)> = grid
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| {
            !matches!(
                child.style.layout.position,
                Some(PositionStyle::Absolute | PositionStyle::Fixed)
            )
        })
        .filter_map(|(index, child)| {
            result
                .rects
                .get(&child.id)
                .copied()
                .map(|rect| (index, rect))
        })
        .collect();
    if entries.len() < 2 {
        return false;
    }

    let mut columns: Vec<f32> = Vec::new();
    for (_, rect) in &entries {
        if !columns.iter().any(|x| (rect.x - *x).abs() <= 0.5) {
            columns.push(rect.x);
        }
    }
    columns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let row_gap = grid_row_gap_px(grid, sf, Some(grid_rect.h));
    let bottom_inset = resolved_bottom_outer_inset(result, &grid.id);
    let content_top = entries
        .iter()
        .map(|(_, rect)| rect.y)
        .fold(f32::INFINITY, f32::min);
    if !content_top.is_finite() {
        return false;
    }
    let mut column_bottoms = vec![content_top; columns.len()];
    let mut changed = false;

    for (child_index, rect) in entries {
        let column_index = column_bottoms
            .iter()
            .enumerate()
            .min_by(|a, b| {
                a.1.partial_cmp(b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        columns[a.0]
                            .partial_cmp(&columns[b.0])
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
            })
            .map(|(index, _)| index)
            .unwrap_or(0);
        let target_x = columns[column_index];
        let target_y = column_bottoms[column_index];
        let dx = target_x - rect.x;
        let dy = target_y - rect.y;
        if dx.abs() > 0.5 || dy.abs() > 0.5 {
            translate_subtree(&grid.children[child_index], result, dx, dy);
            changed = true;
        }
        column_bottoms[column_index] = target_y + rect.h + row_gap;
    }

    let content_bottom = column_bottoms.into_iter().fold(content_top, f32::max) - row_gap.max(0.0);
    if let Some(rect) = result.rects.get_mut(&grid.id) {
        let new_height = (content_bottom + bottom_inset - grid_rect.y)
            .max(
                stretched_height_floors
                    .get(&grid.id)
                    .copied()
                    .unwrap_or(0.0),
            )
            .max(0.0);
        if (rect.h - new_height).abs() > 0.5 {
            rect.h = new_height;
            changed = true;
        }
    }
    changed
}

fn reconcile_layout_after_grid_adjustments(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    stretched_height_floors: &HashMap<String, f32>,
    changed: &HashSet<String>,
) -> bool {
    let mut subtree_changed = changed.contains(&node.id);
    for child in &node.children {
        if reconcile_layout_after_grid_adjustments(
            child,
            result,
            sf,
            theme,
            stretched_height_floors,
            changed,
        ) {
            subtree_changed = true;
        }
    }
    if !subtree_changed {
        return false;
    }

    let mut adjusted = false;
    if reflows_column_children_after_grid(node) {
        adjusted |= repack_column_children_after_grid(node, result, sf, theme);
    }
    adjusted |= realign_row_children_after_grid(node, result);
    if auto_height_container_can_follow_packed_content(node) {
        adjusted |= resize_auto_height_container_to_children(
            node,
            result,
            sf,
            stretched_height_floors.get(&node.id).copied(),
        );
    }
    subtree_changed || adjusted
}

fn collect_stretched_grid_height_floors(
    node: &WidgetNode,
    result: &LayoutResult,
    out: &mut HashMap<String, f32>,
) {
    if let Some(parent_alignment) = row_container_alignment(node) {
        let stretched_height = result
            .resolved_box(&node.id)
            .map(|resolved| resolved.content_box.h)
            .or_else(|| result.rects.get(&node.id).map(|rect| rect.h))
            .unwrap_or(0.0);
        for child in &node.children {
            let alignment = child.style.layout.align_self.unwrap_or(parent_alignment);
            if child.kind == WidgetKind::GridLayout
                && alignment == AlignItemsStyle::Stretch
                && child.props.fixed_height.is_none()
                && child.style.layout.height.is_none()
                && child.style.layout.height_value.is_none()
            {
                if stretched_height > 0.0 {
                    out.insert(child.id.clone(), stretched_height);
                }
            }
        }
    }
    let preserves_allocated_grid_height = node.kind == WidgetKind::Pane
        || (node.kind == WidgetKind::Splitter
            && node.props.orientation.as_deref().unwrap_or("horizontal") == "vertical");
    if preserves_allocated_grid_height {
        for child in &node.children {
            if child.kind == WidgetKind::GridLayout
                && child.props.fixed_height.is_none()
                && child.style.layout.height.is_none()
                && child.style.layout.height_value.is_none()
            {
                if let Some(rect) = result.rects.get(&child.id) {
                    out.entry(child.id.clone())
                        .and_modify(|floor| *floor = floor.max(rect.h))
                        .or_insert(rect.h);
                }
            }
        }
    }
    for child in &node.children {
        collect_stretched_grid_height_floors(child, result, out);
    }
}

fn row_container_alignment(node: &WidgetNode) -> Option<AlignItemsStyle> {
    let is_row = matches!(
        node.kind,
        WidgetKind::HLayout | WidgetKind::MenuBar | WidgetKind::StatusBar
    ) || (node.kind == WidgetKind::Splitter
        && node.props.orientation.as_deref().unwrap_or("horizontal") != "vertical")
        || matches!(
            node.style.layout.flex_direction,
            Some(FlexDirectionStyle::Row | FlexDirectionStyle::RowReverse)
        );
    if !is_row {
        return None;
    }
    node.style.layout.align_items.or_else(|| match node.kind {
        WidgetKind::MenuBar | WidgetKind::StatusBar => Some(AlignItemsStyle::Center),
        _ => Some(AlignItemsStyle::Stretch),
    })
}

fn realign_row_children_after_grid(node: &WidgetNode, result: &mut LayoutResult) -> bool {
    let Some(parent_alignment) = row_container_alignment(node) else {
        return false;
    };
    let content = result
        .resolved_box(&node.id)
        .map(|resolved| resolved.content_box)
        .or_else(|| result.rects.get(&node.id).copied());
    let Some(content) = content else {
        return false;
    };

    let mut changed = false;
    for child in &node.children {
        if !is_reflowable_normal_child(child) {
            continue;
        }
        let Some(rect) = result.rects.get(&child.id).copied() else {
            continue;
        };
        let alignment = child.style.layout.align_self.unwrap_or(parent_alignment);
        let target_y = match alignment {
            AlignItemsStyle::Start => content.y,
            AlignItemsStyle::Center => content.y + ((content.h - rect.h).max(0.0) * 0.5),
            AlignItemsStyle::End => content.y + (content.h - rect.h).max(0.0),
            AlignItemsStyle::Stretch => continue,
        };
        let dy = target_y - rect.y;
        if dy.abs() > 0.5 {
            translate_subtree(child, result, 0.0, dy);
            changed = true;
        }
    }
    changed
}

fn reflows_column_children_after_grid(node: &WidgetNode) -> bool {
    if matches!(
        node.style.layout.display,
        Some(DisplayStyle::None | DisplayStyle::Grid)
    ) {
        return false;
    }
    if matches!(
        node.style.layout.flex_direction,
        Some(FlexDirectionStyle::Row | FlexDirectionStyle::RowReverse)
    ) {
        return false;
    }
    matches!(
        node.kind,
        WidgetKind::Window
            | WidgetKind::VLayout
            | WidgetKind::Panel
            | WidgetKind::Sidebar
            | WidgetKind::Modal
            | WidgetKind::ScrollArea
            | WidgetKind::Collapsible
            | WidgetKind::Page
    )
}

fn repack_column_children_after_grid(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
) -> bool {
    let Some(node_rect) = result.rects.get(&node.id).copied() else {
        return false;
    };
    let entries: Vec<(usize, Rect)> = node
        .children
        .iter()
        .enumerate()
        .filter(|(_, child)| is_reflowable_normal_child(child))
        .filter_map(|(index, child)| {
            result
                .rects
                .get(&child.id)
                .copied()
                .map(|rect| (index, rect))
        })
        .collect();
    if entries.len() < 2 {
        return false;
    }

    let row_gap = column_container_row_gap_px(node, sf, theme, Some(node_rect.h));
    let mut cursor_y = entries[0].1.y;
    let mut changed = false;
    for (child_index, rect) in entries {
        let dy = cursor_y - rect.y;
        if dy.abs() > 0.5 {
            translate_subtree(&node.children[child_index], result, 0.0, dy);
            changed = true;
        }
        let current_rect = result
            .rects
            .get(&node.children[child_index].id)
            .copied()
            .unwrap_or(rect);
        cursor_y = current_rect.y + current_rect.h + row_gap;
    }
    changed
}

fn auto_height_container_can_follow_packed_content(node: &WidgetNode) -> bool {
    if !matches!(
        node.kind,
        WidgetKind::VLayout
            | WidgetKind::Panel
            | WidgetKind::Sidebar
            | WidgetKind::Modal
            | WidgetKind::GridLayout
            | WidgetKind::FlowLayout
            | WidgetKind::Collapsible
            | WidgetKind::Page
    ) {
        return false;
    }
    if matches!(
        node.kind,
        WidgetKind::Window | WidgetKind::HLayout | WidgetKind::ScrollArea
    ) {
        return false;
    }
    if matches!(
        node.style.layout.display,
        Some(DisplayStyle::None | DisplayStyle::Grid)
    ) {
        return false;
    }
    if matches!(
        node.style.layout.flex_direction,
        Some(FlexDirectionStyle::Row | FlexDirectionStyle::RowReverse)
    ) {
        return false;
    }
    node.props.fixed_height.is_none()
        && node.style.layout.height.is_none()
        && !matches!(
            node.style.layout.height_value,
            Some(LayoutLength::LogicalPx(_) | LayoutLength::Percent(_) | LayoutLength::Calc(_))
        )
        && node.style.layout.flex_grow.is_none()
}

fn resize_auto_height_container_to_children(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    height_floor: Option<f32>,
) -> bool {
    let Some(rect) = result.rects.get(&node.id).copied() else {
        return false;
    };
    let mut content_bottom: Option<f32> = None;
    for child in &node.children {
        if !is_reflowable_normal_child(child) {
            continue;
        }
        if let Some(child_rect) = result.rects.get(&child.id).copied() {
            content_bottom = Some(
                content_bottom
                    .unwrap_or(child_rect.y + child_rect.h)
                    .max(child_rect.y + child_rect.h),
            );
        }
    }
    let Some(content_bottom) = content_bottom else {
        return false;
    };

    let bottom_inset = resolved_bottom_outer_inset(result, &node.id);
    let mut new_height = (content_bottom + bottom_inset - rect.y)
        .max(height_floor.unwrap_or(0.0))
        .max(0.0);
    if let Some(min_height) = authored_axis_size_px(
        node.style.layout.min_height_value,
        node.style.layout.min_height,
        sf,
        Some(rect.h),
    ) {
        new_height = new_height.max(min_height);
    }
    if let Some(max_height) = authored_axis_size_px(
        node.style.layout.max_height_value,
        node.style.layout.max_height,
        sf,
        Some(rect.h),
    ) {
        new_height = new_height.min(max_height);
    }
    if (rect.h - new_height).abs() <= 0.5 {
        return false;
    }
    if let Some(current) = result.rects.get_mut(&node.id) {
        current.h = new_height;
    }
    true
}

fn resolved_bottom_outer_inset(result: &LayoutResult, node_id: &str) -> f32 {
    let padding = result
        .resolved_padding
        .get(node_id)
        .map(|edges| edges.bottom)
        .unwrap_or(0.0);
    let border = result
        .resolved_borders
        .get(node_id)
        .map(|edges| edges.bottom)
        .unwrap_or(0.0);
    padding + border
}

fn is_reflowable_normal_child(child: &WidgetNode) -> bool {
    !matches!(
        child.style.layout.position,
        Some(PositionStyle::Absolute | PositionStyle::Fixed)
    ) && !matches!(
        child.kind,
        WidgetKind::Modal | WidgetKind::Tooltip | WidgetKind::Toast | WidgetKind::ContextMenu
    )
}

fn column_container_row_gap_px(
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_axis: Option<f32>,
) -> f32 {
    layout_length_percentage(
        node.style
            .layout
            .row_gap_value
            .or(node.style.layout.gap_value),
        node.style.layout.row_gap.or(node.style.layout.gap),
        sf,
        parent_axis,
    )
    .map(lp_value)
    .unwrap_or_else(|| match node.kind {
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal => {
            let multiplier = if titled_container_uses_body_layout(node) {
                1.0
            } else {
                0.75
            };
            theme.spacing * multiplier * sf
        }
        WidgetKind::Collapsible => theme.spacing * 0.75 * sf,
        _ => 0.0,
    })
}

fn authored_axis_size_px(
    value: Option<LayoutLength>,
    legacy_px: Option<f32>,
    sf: f32,
    parent_axis_size: Option<f32>,
) -> Option<f32> {
    match layout_dimension(value, legacy_px, sf, parent_axis_size)? {
        Dimension::Length(value) => Some(value),
        Dimension::Percent(value) => parent_axis_size.map(|parent| parent * value),
        Dimension::Auto => None,
    }
}

fn grid_row_gap_px(node: &WidgetNode, sf: f32, parent_axis: Option<f32>) -> f32 {
    layout_length_percentage(
        node.style
            .layout
            .row_gap_value
            .or(node.style.layout.gap_value),
        node.style.layout.row_gap.or(node.style.layout.gap),
        sf,
        parent_axis,
    )
    .map(lp_value)
    .unwrap_or(0.0)
}

fn titled_container_uses_body_layout(node: &WidgetNode) -> bool {
    matches!(
        node.kind,
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal
    ) && node
        .props
        .text
        .as_deref()
        .is_some_and(|text| !text.is_empty())
        && !node.children.is_empty()
}

fn titled_container_body_style(node: &WidgetNode, sf: f32, theme: &Theme) -> Style {
    let metrics = titled_container_metrics(node, sf, theme, None);
    let default_panel_padding = theme.panel_padding.max(0.0) * sf;
    let panel_top_padding = authored_padding_top(node, sf, None)
        .map(lp_value)
        .unwrap_or(default_panel_padding)
        .max(0.0);
    let header_height =
        panel_header_band_height_px(node, metrics.title_line_height, panel_top_padding, sf);
    let top_margin =
        (header_height - panel_top_padding).max(0.0) + metrics.body_gap + metrics.body_visual_inset;
    let body_padding = panel_body_padding_lp(node) * sf;
    let row_gap = node
        .style
        .layout
        .row_gap_value
        .or(node.style.layout.gap_value)
        .and_then(|gap| layout_length_percentage(Some(gap), None, sf, None))
        .or_else(|| {
            node.style
                .layout
                .row_gap
                .or(node.style.layout.gap)
                .map(|gap| LengthPercentage::Length(gap.max(0.0) * sf))
        })
        .unwrap_or_else(|| LengthPercentage::Length(theme.spacing * sf));
    let column_gap = node
        .style
        .layout
        .column_gap_value
        .or(node.style.layout.gap_value)
        .and_then(|gap| layout_length_percentage(Some(gap), None, sf, None))
        .or_else(|| {
            node.style
                .layout
                .column_gap
                .or(node.style.layout.gap)
                .map(|gap| LengthPercentage::Length(gap.max(0.0) * sf))
        })
        .unwrap_or_else(|| LengthPercentage::Length(0.0));
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Stretch),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        size: Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        min_size: Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(0.0),
        },
        margin: taffy::geometry::Rect {
            left: LengthPercentageAuto::Length(0.0),
            right: LengthPercentageAuto::Length(0.0),
            top: LengthPercentageAuto::Length(top_margin),
            bottom: LengthPercentageAuto::Length(0.0),
        },
        padding: taffy::geometry::Rect {
            left: LengthPercentage::Length(body_padding),
            right: LengthPercentage::Length(body_padding),
            top: LengthPercentage::Length(body_padding),
            bottom: LengthPercentage::Length(body_padding),
        },
        gap: taffy::geometry::Size {
            width: column_gap,
            height: row_gap,
        },
        ..Default::default()
    }
}

fn panel_body_padding_lp(node: &WidgetNode) -> f32 {
    node.style
        .parts
        .parts
        .get("body")
        .and_then(|part| part.layout.padding)
        .unwrap_or(0.0)
        .max(0.0)
}

fn panel_header_padding_lp(node: &WidgetNode) -> Option<f32> {
    node.style
        .parts
        .parts
        .get("header")
        .and_then(|part| part.layout.padding)
        .map(|padding| padding.max(0.0))
}

fn panel_header_height_lp(node: &WidgetNode) -> Option<f32> {
    node.style
        .parts
        .parts
        .get("header")
        .and_then(|part| part.layout.height)
        .map(|height| height.max(0.0))
}

fn panel_header_band_height_px(
    node: &WidgetNode,
    title_line_height: f32,
    panel_top_padding: f32,
    sf: f32,
) -> f32 {
    let authored_padding = panel_header_padding_lp(node).map(|padding| padding * sf);
    let minimum = title_line_height + authored_padding.unwrap_or(0.0) * 2.0;
    panel_header_height_lp(node)
        .map(|height| height * sf)
        .or_else(|| authored_padding.map(|_| minimum))
        .unwrap_or(panel_top_padding + title_line_height)
        .max(minimum)
}

fn reserve_collapsible_header_space(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_size: Option<(f32, f32)>,
    state: Option<&WidgetState>,
) {
    if node.kind != WidgetKind::Collapsible {
        return;
    }

    let header_h = collapsible_header_height_for_style(&node.style, theme, sf);
    let default_body_pad = if collapsible_expanded(node, state) {
        theme.panel_padding.max(0.0) * sf
    } else {
        0.0
    };
    let body_top = authored_padding_top(node, sf, parent_size.map(|size| size.0))
        .unwrap_or_else(|| LengthPercentage::Length(default_body_pad));
    style.padding.top = add_fixed_padding(body_top, header_h, parent_size.map(|size| size.0));
    style.min_size.height = max_dimension_length(style.min_size.height, header_h);
}

fn authored_padding_top(
    node: &WidgetNode,
    sf: f32,
    parent_width: Option<f32>,
) -> Option<LengthPercentage> {
    let layout = &node.style.layout;
    let pad_all_value = layout
        .padding_value
        .or_else(|| layout.padding.map(LayoutLength::LogicalPx));
    layout_length_percentage(
        layout.padding_top_value.or(pad_all_value),
        layout.padding_top.or(layout.padding),
        sf,
        parent_width,
    )
}

fn add_fixed_padding(
    value: LengthPercentage,
    add_px: f32,
    parent_width: Option<f32>,
) -> LengthPercentage {
    match value {
        LengthPercentage::Length(current) => LengthPercentage::Length(current + add_px),
        LengthPercentage::Percent(percent) => parent_width
            .map(|width| LengthPercentage::Length(width * percent + add_px))
            .unwrap_or_else(|| LengthPercentage::Length(add_px)),
    }
}

fn apply_grid_layout_default_tracks(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    parent_size: Option<(f32, f32)>,
    viewport_width_lp: f32,
) {
    if node.kind != WidgetKind::GridLayout {
        return;
    }
    if node.style.layout.grid_template_rows.is_none() {
        if let Some(tracks) = &node.props.grid_template_rows {
            style.grid_template_rows = tracks
                .iter()
                .cloned()
                .map(|track| grid_track_size(track, sf))
                .collect();
        }
    }
    if node.style.layout.grid_template_columns.is_some() {
        return;
    }
    if let Some(tracks) = &node.props.grid_template_columns {
        style.grid_template_columns = tracks
            .iter()
            .cloned()
            .map(|track| grid_track_size(track, sf))
            .collect();
        return;
    }
    let available_width = grid_available_width_px(style, parent_size);
    let min_fn = node.props.grid_min_column_width.map(|w| {
        let authored_min = (w * sf).max(1.0);
        let bounded_min = available_width
            .filter(|available| *available > 0.0)
            .map(|available| authored_min.min(available))
            .unwrap_or(authored_min);
        MinTrackSizingFunction::Fixed(LengthPercentage::Length(bounded_min))
    });
    let configured_columns = node
        .props
        .grid_column_breakpoints
        .iter()
        .find(|rule| viewport_width_lp <= rule.max_width)
        .map(|rule| rule.columns)
        .or(node.props.grid_columns);
    style.grid_template_columns = match (configured_columns, min_fn) {
        (Some(max_columns), Some(min)) => repeat_grid_track(
            responsive_grid_column_count(
                max_columns.max(1),
                grid_min_track_width_px(&min),
                available_width,
                grid_column_gap_px(style),
            ) as usize,
            min,
        ),
        (Some(columns), None) => {
            repeat_grid_track(columns.max(1) as usize, MinTrackSizingFunction::Auto)
        }
        (None, Some(min)) => vec![TrackSizingFunction::Repeat(
            if node.props.grid_auto_fit {
                GridTrackRepetition::AutoFit
            } else {
                GridTrackRepetition::AutoFill
            },
            vec![NonRepeatedTrackSizingFunction {
                min,
                max: MaxTrackSizingFunction::Fraction(1.0),
            }],
        )],
        (None, None) => repeat_grid_track(2, MinTrackSizingFunction::Auto),
    };
}

fn repeat_grid_track(count: usize, min: MinTrackSizingFunction) -> Vec<TrackSizingFunction> {
    (0..count.max(1))
        .map(|_| {
            TrackSizingFunction::Single(NonRepeatedTrackSizingFunction {
                min: min.clone(),
                max: MaxTrackSizingFunction::Fraction(1.0),
            })
        })
        .collect()
}

fn responsive_grid_column_count(
    max_columns: u16,
    min_track_width: f32,
    available_width: Option<f32>,
    column_gap: f32,
) -> u16 {
    let Some(width) = available_width.filter(|w| *w > 0.0) else {
        return max_columns.max(1);
    };
    let min_track_width = min_track_width.max(1.0);
    let column_gap = column_gap.max(0.0);
    let fit = ((width + column_gap) / (min_track_width + column_gap))
        .floor()
        .max(1.0) as u16;
    fit.min(max_columns.max(1)).max(1)
}

fn grid_min_track_width_px(min: &MinTrackSizingFunction) -> f32 {
    match min {
        MinTrackSizingFunction::Fixed(LengthPercentage::Length(value)) => *value,
        _ => 1.0,
    }
}

fn grid_available_width_px(style: &Style, parent_size: Option<(f32, f32)>) -> Option<f32> {
    let parent_width = parent_size.map(|size| size.0);
    let width = resolve_dimension_px(style.size.width, parent_width).or(parent_width)?;
    Some((width - lp_value(style.padding.left) - lp_value(style.padding.right)).max(0.0))
}

fn resolve_dimension_px(value: Dimension, parent_axis: Option<f32>) -> Option<f32> {
    match value {
        Dimension::Length(value) => Some(value),
        Dimension::Percent(value) => parent_axis.map(|parent| parent * value),
        Dimension::Auto => parent_axis,
    }
}

fn grid_column_gap_px(style: &Style) -> f32 {
    lp_value(style.gap.width)
}

fn apply_flow_layout_alignment(style: &mut Style, node: &WidgetNode) {
    if node.kind != WidgetKind::FlowLayout {
        return;
    }
    if let Some(align) = node.props.flow_align.as_deref() {
        style.justify_content = match align {
            "center" => Some(JustifyContent::Center),
            "end" => Some(JustifyContent::FlexEnd),
            _ => Some(JustifyContent::FlexStart),
        };
    }
    if let Some(cross_align) = node.props.flow_cross_align.as_deref() {
        style.align_items = match cross_align {
            "center" => Some(AlignItems::Center),
            "end" => Some(AlignItems::FlexEnd),
            "stretch" => Some(AlignItems::Stretch),
            _ => Some(AlignItems::FlexStart),
        };
    }
}

fn max_dimension_length(value: Dimension, min_px: f32) -> Dimension {
    match value {
        Dimension::Length(current) => Dimension::Length(current.max(min_px)),
        _ => Dimension::Length(min_px),
    }
}

fn collapsible_expanded(node: &WidgetNode, state: Option<&WidgetState>) -> bool {
    state
        .and_then(|state| state.expanded.get(&node.id).copied())
        .or(node.props.expanded)
        .unwrap_or(true)
}

fn tree_node_expanded(node: &WidgetNode, state: Option<&WidgetState>) -> bool {
    !node.children.is_empty()
        && state
            .and_then(|state| state.expanded.get(&node.id).copied())
            .or(node.props.expanded)
            .unwrap_or(false)
}

pub(crate) fn tree_node_row_height_for_style(
    node: &WidgetNode,
    theme: &Theme,
    sf: f32,
    parent_axis_size: Option<f32>,
) -> f32 {
    let fallback = node_control_height_lp(node, theme) * sf;
    layout_dimension(
        node.style.layout.height_value,
        node.style.layout.height,
        sf,
        parent_axis_size,
    )
    .and_then(|dimension| match dimension {
        Dimension::Length(value) => Some(value),
        Dimension::Percent(percent) => parent_axis_size.map(|size| size * percent),
        Dimension::Auto => None,
    })
    .unwrap_or(fallback)
    .max(1.0)
}

fn normalize_tree_node_layout_style(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_size: Option<(f32, f32)>,
    state: Option<&WidgetState>,
) {
    if node.kind != WidgetKind::TreeNode || !tree_node_expanded(node, state) {
        return;
    }
    let row_h = tree_node_row_height_for_style(node, theme, sf, parent_size.map(|s| s.1));
    style.size.height = Dimension::Auto;
    style.min_size.height = Dimension::Length(row_h);
    style.padding.top = LengthPercentage::Length(row_h);
}

fn reserve_panel_title_space(style: &mut Style, node: &WidgetNode, sf: f32, theme: &Theme) {
    if !matches!(
        node.kind,
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal
    ) || !node.props.text.as_deref().is_some_and(|t| !t.is_empty())
    {
        return;
    }
    let metrics = titled_container_metrics(node, sf, theme, None);
    let title_inset = metrics.title_line_height + metrics.body_gap + metrics.body_visual_inset;
    style.padding.top = match style.padding.top {
        LengthPercentage::Length(top) => LengthPercentage::Length(top + title_inset),
        _ => LengthPercentage::Length(title_inset),
    };
}

pub(crate) fn panel_title_line_height_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let title_text = base_part_style(&node.style, "title").map(|part| &part.text);
    let font_size = title_text
        .and_then(|text| text.font_size)
        .or(node.style.text.font_size)
        .unwrap_or_else(|| crate::style::native_fallback_font_size(theme))
        .max(8.0);
    match title_text
        .and_then(|text| text.line_height)
        .or(node.style.text.line_height)
    {
        Some(LineHeight::Multiplier(value)) => (font_size * value.max(0.1)).max(1.0),
        Some(LineHeight::LogicalPx(value)) => value.max(1.0),
        None => {
            let base_leading = (theme.font_size * (theme.base_line_height.max(0.1) - 1.0)).max(0.0);
            (font_size + base_leading).max(theme.font_size + 3.0)
        }
    }
}

pub(crate) fn panel_title_gap_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    node.style
        .layout
        .gap
        .unwrap_or(theme.spacing * 0.75)
        .max(0.0)
}

pub(crate) fn panel_title_body_gap_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let gap = panel_title_gap_lp(node, theme);
    if node.kind == WidgetKind::Modal {
        gap * 2.0
    } else {
        gap
    }
}

pub(crate) fn panel_title_top_padding_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let default = theme.panel_padding.max(0.0);
    let layout = &node.style.layout;
    let panel_padding = layout
        .padding_top
        .or(layout.padding)
        .unwrap_or(default)
        .max(0.0);
    if panel_header_padding_lp(node).is_some() || panel_header_height_lp(node).is_some() {
        let line_height = panel_title_line_height_lp(node, theme);
        let header_height = panel_header_band_height_px(node, line_height, panel_padding, 1.0);
        panel_header_padding_lp(node)
            .unwrap_or(0.0)
            .max((header_height - line_height) * 0.5)
    } else {
        panel_padding
    }
}

#[derive(Debug, Clone, Copy)]
struct TitledContainerMetrics {
    title_line_height: f32,
    body_gap: f32,
    body_visual_inset: f32,
}

fn titled_container_metrics(
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_width: Option<f32>,
) -> TitledContainerMetrics {
    let raw_gap = layout_length_percentage(
        node.style.layout.gap_value,
        node.style.layout.gap,
        sf,
        parent_width,
    )
    .map(lp_value)
    .unwrap_or(theme.spacing * 0.75 * sf)
    .max(0.0);
    TitledContainerMetrics {
        title_line_height: panel_title_line_height_lp(node, theme) * sf,
        body_gap: if node.kind == WidgetKind::Modal {
            raw_gap * 2.0
        } else {
            raw_gap
        },
        body_visual_inset: PANEL_BODY_VISUAL_INSET_LP * sf,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TitledContainerGeometry {
    pub title_box: Rect,
    pub title_band: Rect,
    pub body_viewport: Rect,
    pub body_content_origin_y: f32,
}

pub(crate) fn titled_container_geometry(
    node: &WidgetNode,
    layout: &LayoutResult,
    sf: f32,
    theme: &Theme,
) -> Option<TitledContainerGeometry> {
    if !matches!(
        node.kind,
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal
    ) || !node
        .props
        .text
        .as_deref()
        .is_some_and(|text| !text.is_empty())
    {
        return None;
    }
    let mut resolved_box = layout.resolved_box(&node.id)?;
    if !layout.resolved_padding.contains_key(&node.id) {
        let default = theme.panel_padding.max(0.0) * sf;
        let authored = &node.style.layout;
        let all_value = authored
            .padding_value
            .or_else(|| authored.padding.map(LayoutLength::LogicalPx));
        let edge = |value: Option<LayoutLength>, legacy: Option<f32>| {
            cascaded_edge_length_percentage(
                value,
                legacy,
                all_value,
                authored.padding,
                sf,
                Some(resolved_box.border_box.w),
            )
            .map(lp_value)
            .unwrap_or(default)
            .max(0.0)
        };
        let padding = ResolvedEdges {
            left: edge(authored.padding_left_value, authored.padding_left),
            right: edge(authored.padding_right_value, authored.padding_right),
            top: edge(authored.padding_top_value, authored.padding_top),
            bottom: edge(authored.padding_bottom_value, authored.padding_bottom),
        };
        resolved_box =
            ResolvedBox::from_rect(resolved_box.border_box, ResolvedEdges::default(), padding);
    }
    let metrics = titled_container_metrics(node, sf, theme, Some(resolved_box.padding_box.w));
    let header_layout_authored =
        panel_header_padding_lp(node).is_some() || panel_header_height_lp(node).is_some();
    let header_height = panel_header_band_height_px(
        node,
        metrics.title_line_height,
        resolved_box.padding.top,
        sf,
    )
    .min(resolved_box.padding_box.h)
    .max(0.0);
    let title_inset = panel_header_padding_lp(node)
        .map(|padding| padding * sf)
        .unwrap_or(0.0);
    let title_x = if panel_header_padding_lp(node).is_some() {
        resolved_box.padding_box.x + title_inset
    } else {
        resolved_box.content_box.x
    };
    let title_y = if header_layout_authored {
        resolved_box.padding_box.y
            + title_inset.max((header_height - metrics.title_line_height) * 0.5)
    } else {
        resolved_box
            .content_box
            .y
            .min(resolved_box.padding_box.y + (header_height - metrics.title_line_height).max(0.0))
    };
    let title_box = Rect {
        x: title_x,
        y: title_y,
        w: if panel_header_padding_lp(node).is_some() {
            (resolved_box.padding_box.w - title_inset * 2.0).max(0.0)
        } else {
            resolved_box.content_box.w
        },
        h: metrics
            .title_line_height
            .min((resolved_box.padding_box.y + header_height - title_y).max(0.0)),
    };
    let title_band = Rect {
        x: resolved_box.padding_box.x,
        y: resolved_box.padding_box.y,
        w: resolved_box.padding_box.w,
        h: header_height,
    };
    let body_top = (title_band.y + title_band.h + metrics.body_gap)
        .min(resolved_box.padding_box.y + resolved_box.padding_box.h);
    Some(TitledContainerGeometry {
        title_box,
        title_band,
        body_viewport: Rect {
            x: resolved_box.padding_box.x,
            y: body_top,
            w: resolved_box.padding_box.w,
            h: (resolved_box.padding_box.y + resolved_box.padding_box.h - body_top).max(0.0),
        },
        body_content_origin_y: (body_top
            + metrics.body_visual_inset
            + panel_body_padding_lp(node) * sf)
            .min(resolved_box.padding_box.y + resolved_box.padding_box.h),
    })
}

fn apply_intrinsic_leaf_width(
    style: &mut Style,
    node: &WidgetNode,
    parent_kind: Option<&WidgetKind>,
    parent_preserves_preferred_main_size: bool,
    parent_allows_intrinsic_leaf_width: bool,
    sf: f32,
    theme: &Theme,
) {
    if !parent_allows_intrinsic_leaf_width && !is_compact_boolean_leaf(node.kind) {
        return;
    }
    if node.props.fixed_width.is_some()
        || authored_width_locks_intrinsic_leaf(node)
        || authored_zero_min_width(node)
    {
        return;
    }

    let Some(width) = intrinsic_leaf_width(node, theme) else {
        return;
    };
    let mut preferred_width = width * sf;
    match style.max_size.width {
        Dimension::Length(max_width) => preferred_width = preferred_width.min(max_width.max(0.0)),
        Dimension::Percent(_) => return,
        Dimension::Auto => {}
    }
    if matches!(style.min_size.width, Dimension::Percent(_)) {
        return;
    }
    let parent_is_grid = matches!(parent_kind, Some(WidgetKind::GridLayout));
    if parent_is_grid {
        style.min_size.width = min_dimension_at_least(style.min_size.width, preferred_width);
        return;
    }
    if (is_compact_boolean_leaf(node.kind) && !parent_allows_intrinsic_leaf_width)
        || matches!(
            parent_kind,
            Some(WidgetKind::DragSource | WidgetKind::DropTarget)
        )
    {
        style.size.width = Dimension::Length(preferred_width);
    } else {
        style.flex_basis = Dimension::Length(preferred_width);
        if node.style.layout.flex_shrink.is_none() && !parent_preserves_preferred_main_size {
            style.flex_shrink = 1.0;
        }
    }
    let min_width = (intrinsic_leaf_min_width(node, theme) * sf).min(preferred_width);
    style.min_size.width = min_dimension_at_least(style.min_size.width, min_width);
}

fn apply_compact_boolean_leaf_alignment(
    style: &mut Style,
    node: &WidgetNode,
    parent_align_items: Option<AlignItems>,
) {
    if !is_compact_boolean_leaf(node.kind)
        || style.align_self.is_some()
        || node.props.fixed_width.is_some()
        || authored_width_locks_intrinsic_leaf(node)
        || matches!(
            parent_align_items,
            Some(AlignItems::FlexStart | AlignItems::Center | AlignItems::FlexEnd)
        )
    {
        return;
    }
    style.align_self = Some(AlignItems::FlexStart);
}

fn is_compact_boolean_leaf(kind: WidgetKind) -> bool {
    matches!(kind, WidgetKind::Checkbox | WidgetKind::ToggleSwitch)
}

fn apply_inline_status_leaf_alignment(
    style: &mut Style,
    node: &WidgetNode,
    parent_flex_direction: Option<FlexDirection>,
) {
    if !matches!(
        node.kind,
        WidgetKind::Badge | WidgetKind::Tag | WidgetKind::Led
    ) || style.align_self.is_some()
        || !matches!(
            parent_flex_direction,
            Some(FlexDirection::Row | FlexDirection::RowReverse)
        )
    {
        return;
    }
    style.align_self = Some(AlignItems::Center);
}

fn authored_width_locks_intrinsic_leaf(node: &WidgetNode) -> bool {
    node.style.layout.width.is_some()
        || matches!(
            node.style.layout.width_value,
            Some(LayoutLength::LogicalPx(_) | LayoutLength::Percent(_) | LayoutLength::Calc(_))
        )
}

fn authored_zero_min_width(node: &WidgetNode) -> bool {
    // `min-width: 0` is the CSS-compatible opt-out that lets dense flex/grid
    // children shrink below intrinsic text width. Nonzero logical/calc min-width
    // values can still be raised by intrinsic leaf minimums; percent min-width
    // remains a parent-relative Taffy constraint.
    matches!(node.style.layout.min_width, Some(value) if value <= 0.5)
        || matches!(
            node.style.layout.min_width_value,
            Some(LayoutLength::LogicalPx(value)) if value <= 0.5
        )
}

fn min_dimension_at_least(value: Dimension, min_px: f32) -> Dimension {
    match value {
        Dimension::Length(current) => Dimension::Length(current.max(min_px)),
        Dimension::Auto => Dimension::Length(min_px),
        Dimension::Percent(_) => value,
    }
}

fn intrinsic_leaf_width(node: &WidgetNode, theme: &Theme) -> Option<f32> {
    let text = intrinsic_text(node);
    // Taffy and the renderer eventually snap several bounds to whole physical
    // pixels. Rounding shaped widths up prevents a fractional final glyph from
    // being clipped after that snapping.
    let text_w = text.map(|text| {
        measure_text_for_layout(text, &node.style.text, theme)
            .width
            .ceil()
    });
    let pad = theme.spacing * 2.0;
    let badge_w = badge_extra_width(node, theme);
    match node.kind {
        WidgetKind::Button => Some((text_w.unwrap_or(0.0) + pad + badge_w).clamp(72.0, 280.0)),
        WidgetKind::SmallButton => {
            Some((text_w.unwrap_or(0.0) + pad * 1.4 + badge_w).clamp(48.0, 180.0))
        }
        WidgetKind::IconButton | WidgetKind::ImageButton | WidgetKind::ArrowButton => {
            Some(node_control_height_lp(node, theme))
        }
        WidgetKind::Selectable => Some((text_w.unwrap_or(0.0) + pad * 2.0).clamp(72.0, 320.0)),
        WidgetKind::RadioButton => {
            Some((text_w.unwrap_or(0.0) + pad * 2.0 + 18.0).clamp(72.0, 320.0))
        }
        WidgetKind::TreeNode => Some((text_w.unwrap_or(0.0) + pad * 2.0 + 18.0).clamp(72.0, 360.0)),
        WidgetKind::Badge | WidgetKind::Tag => node
            .props
            .text
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(|_| {
                let (left, right) = standalone_badge_horizontal_padding_lp(&node.style);
                let border = node.style.visual.border_width.unwrap_or(0.0).max(0.0);
                (text_w.unwrap_or(0.0) + left + right + border * 2.0 + 8.0).max(BADGE_MIN_HEIGHT_LP)
            }),
        WidgetKind::Menu => {
            let menu_pad = theme.spacing;
            Some((text_w.unwrap_or(0.0) + menu_pad + MENU_LABEL_WIDTH_SAFETY_LP).clamp(28.0, 180.0))
        }
        WidgetKind::Dropdown => Some((text_w.unwrap_or(0.0) + pad + 22.0).clamp(112.0, 260.0)),
        WidgetKind::NumberInput => Some((text_w.unwrap_or(0.0) + pad + 34.0).clamp(96.0, 220.0)),
        WidgetKind::DragNumber => {
            Some((text_w.unwrap_or(0.0) + pad * 2.0 + 18.0).clamp(88.0, 180.0))
        }
        WidgetKind::TextInput => Some((text_w.unwrap_or(0.0) + pad).clamp(120.0, 280.0)),
        WidgetKind::TextArea => Some((text_w.unwrap_or(0.0) + pad).clamp(180.0, 420.0)),
        WidgetKind::CodeEditor => Some(
            (text_w.unwrap_or(0.0) + pad + code_editor_gutter_width_for_style(&node.style, 1.0))
                .clamp(220.0, 560.0),
        ),
        WidgetKind::LogView => Some((text_w.unwrap_or(0.0) + pad).clamp(220.0, 560.0)),
        WidgetKind::Checkbox => {
            let box_w = checkbox_box_width_lp(node);
            if text.is_some() {
                Some(
                    (text_w.unwrap_or(0.0) + CHECKBOX_LEFT_PAD_LP + box_w + pad).clamp(48.0, 280.0),
                )
            } else {
                Some((box_w + CHECKBOX_LEFT_PAD_LP * 2.0).max(1.0))
            }
        }
        WidgetKind::ToggleSwitch => {
            let track_w = toggle_switch_track_width_lp(node);
            if text.is_some() {
                Some(
                    (text_w.unwrap_or(0.0) + CHECKBOX_LEFT_PAD_LP + track_w + pad)
                        .clamp(64.0, 320.0),
                )
            } else {
                Some((track_w + CHECKBOX_LEFT_PAD_LP * 2.0).max(1.0))
            }
        }
        WidgetKind::Label => Some(
            (text_w.unwrap_or(0.0) + pad + badge_w + LABEL_TEXT_WIDTH_SAFETY_LP).clamp(32.0, 320.0),
        ),
        WidgetKind::NavItem | WidgetKind::Tab => {
            Some((text_w.unwrap_or(0.0) + pad + badge_w).clamp(32.0, 320.0))
        }
        WidgetKind::Slider | WidgetKind::RangeSlider => Some(140.0),
        WidgetKind::ProgressBar => Some(160.0),
        WidgetKind::LimitsBar => Some(200.0),
        WidgetKind::LoadingSpinner => {
            let size = loading_spinner_size_lp(node);
            let text_w = text_w.unwrap_or(0.0);
            let gap = if text_w > 0.0 {
                LOADING_SPINNER_GAP_LP
            } else {
                0.0
            };
            Some((size + gap + text_w + pad).clamp(size, 360.0))
        }
        _ => None,
    }
}

fn intrinsic_leaf_min_width(node: &WidgetNode, theme: &Theme) -> f32 {
    match node.kind {
        WidgetKind::Button => 72.0,
        WidgetKind::SmallButton => 48.0,
        WidgetKind::IconButton | WidgetKind::ImageButton | WidgetKind::ArrowButton => {
            node_control_height_lp(node, theme)
        }
        WidgetKind::Selectable | WidgetKind::RadioButton | WidgetKind::TreeNode => 72.0,
        WidgetKind::Badge | WidgetKind::Tag => BADGE_MIN_HEIGHT_LP,
        WidgetKind::Menu => 28.0,
        WidgetKind::Dropdown => 72.0,
        WidgetKind::NumberInput => 72.0,
        WidgetKind::DragNumber => 64.0,
        WidgetKind::TextInput => 96.0,
        WidgetKind::TextArea => 120.0,
        WidgetKind::CodeEditor | WidgetKind::LogView => 160.0,
        WidgetKind::Checkbox => (checkbox_box_width_lp(node) + CHECKBOX_LEFT_PAD_LP * 2.0).max(1.0),
        WidgetKind::ToggleSwitch => {
            (toggle_switch_track_width_lp(node) + CHECKBOX_LEFT_PAD_LP * 2.0).max(1.0)
        }
        WidgetKind::Label => 0.0,
        WidgetKind::NavItem | WidgetKind::Tab => 32.0,
        WidgetKind::Slider | WidgetKind::RangeSlider => 80.0,
        WidgetKind::ProgressBar => 80.0,
        WidgetKind::LimitsBar => 120.0,
        WidgetKind::LoadingSpinner => loading_spinner_size_lp(node),
        _ => 0.0,
    }
}

pub(crate) fn debug_intrinsic_size_lp(
    node: &WidgetNode,
    theme: &Theme,
) -> (Option<f32>, Option<f32>) {
    let fallback = resolved_widget_geometry_fallback(node, &node.style, theme);
    (
        node.props
            .intrinsic_width
            .or(node.props.fixed_width)
            .or_else(|| intrinsic_leaf_width(node, theme))
            .or(fallback.width),
        node.props
            .intrinsic_height
            .or(node.props.fixed_height)
            .or(fallback.height),
    )
}

pub(crate) fn debug_semantic_minimum_lp(node: &WidgetNode, theme: &Theme) -> (f32, f32) {
    let fallback = resolved_widget_geometry_fallback(node, &node.style, theme);
    let semantic_height = match node.kind {
        WidgetKind::Button
        | WidgetKind::SmallButton
        | WidgetKind::IconButton
        | WidgetKind::ImageButton
        | WidgetKind::ArrowButton
        | WidgetKind::Selectable
        | WidgetKind::RadioButton
        | WidgetKind::Dropdown
        | WidgetKind::Menu
        | WidgetKind::MenuItem
        | WidgetKind::NumberInput
        | WidgetKind::DragNumber
        | WidgetKind::NavItem
        | WidgetKind::Tab
        | WidgetKind::Checkbox
        | WidgetKind::ToggleSwitch
        | WidgetKind::Slider
        | WidgetKind::RangeSlider
        | WidgetKind::ProgressBar
        | WidgetKind::LimitsBar
        | WidgetKind::TextInput => node_control_height_lp(node, theme),
        _ => fallback.min_height.unwrap_or(0.0),
    };
    (
        intrinsic_leaf_min_width(node, theme).max(fallback.min_width.unwrap_or(0.0)),
        semantic_height.max(fallback.min_height.unwrap_or(0.0)),
    )
}

fn badge_extra_width(node: &WidgetNode, theme: &Theme) -> f32 {
    node.props
        .badge
        .as_deref()
        .filter(|badge| !badge.is_empty())
        .map(|badge| badge_width_for_text(&node.style, badge, theme, 1.0) + BADGE_GAP_LP)
        .unwrap_or(0.0)
}

fn intrinsic_text(node: &WidgetNode) -> Option<&str> {
    node.props
        .text
        .as_deref()
        .or_else(|| node.props.placeholder.as_deref())
        .or_else(|| node.props.items.first().map(String::as_str))
        .filter(|text| !text.is_empty())
}

fn text_area_height_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let rows = node
        .style
        .widget
        .text_area_rows
        .unwrap_or_else(|| node.props.rows.unwrap_or(4) as f32)
        .round()
        .max(1.0);
    let font_size = node_font_size_lp(node, theme);
    let line_height = (font_size + 6.0).max(theme.font_size + 4.0);
    rows * line_height + theme.spacing * 2.0
}

pub(crate) fn label_wraps(node: &WidgetNode) -> bool {
    node.props.wrap.unwrap_or(true) && node.style.text.text_overflow != Some(TextOverflow::Ellipsis)
}

fn checkbox_box_width_lp(node: &WidgetNode) -> f32 {
    node.style
        .parts
        .parts
        .get("box")
        .and_then(|part| part.layout.width)
        .unwrap_or(CHECKBOX_BOX_LP)
        .max(1.0)
}

fn toggle_switch_track_width_lp(node: &WidgetNode) -> f32 {
    node.style
        .parts
        .parts
        .get("track")
        .and_then(|part| part.layout.width)
        .unwrap_or(TOGGLE_SWITCH_TRACK_WIDTH_LP)
        .max(1.0)
}

fn node_font_size_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    node.style
        .text
        .font_size
        .unwrap_or_else(|| crate::style::native_fallback_font_size(theme))
        .max(8.0)
}

fn node_control_height_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let font_size = node_font_size_lp(node, theme);
    let control_height = (font_size + theme.spacing * 2.0 + 2.0).max(25.0);
    if node.kind == WidgetKind::NavItem {
        control_height.max(NAV_ITEM_MIN_HEIGHT_LP)
    } else {
        control_height
    }
}

fn apply_node_style(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    parent_size: Option<(f32, f32)>,
    parent_flex_direction: Option<FlexDirection>,
    parent_preserves_preferred_main_size: bool,
) {
    let layout = &node.style.layout;
    if let Some(display) = layout.display {
        style.display = match display {
            DisplayStyle::Flex => Display::Flex,
            DisplayStyle::Grid => Display::Grid,
            DisplayStyle::Block => Display::Block,
            DisplayStyle::None => Display::None,
        };
    }
    if let Some(direction) = layout.flex_direction {
        style.flex_direction = match direction {
            FlexDirectionStyle::Row => FlexDirection::Row,
            FlexDirectionStyle::Column => FlexDirection::Column,
            FlexDirectionStyle::RowReverse => FlexDirection::RowReverse,
            FlexDirectionStyle::ColumnReverse => FlexDirection::ColumnReverse,
        };
    }
    if let Some(wrap) = layout.flex_wrap {
        style.flex_wrap = match wrap {
            FlexWrapStyle::NoWrap => FlexWrap::NoWrap,
            FlexWrapStyle::Wrap => FlexWrap::Wrap,
            FlexWrapStyle::WrapReverse => FlexWrap::WrapReverse,
        };
    }
    if let Some(align_items) = layout.align_items {
        style.align_items = Some(match align_items {
            AlignItemsStyle::Start => AlignItems::FlexStart,
            AlignItemsStyle::Center => AlignItems::Center,
            AlignItemsStyle::End => AlignItems::FlexEnd,
            AlignItemsStyle::Stretch => AlignItems::Stretch,
        });
    }
    if let Some(align_self) = layout.align_self {
        style.align_self = Some(match align_self {
            AlignItemsStyle::Start => AlignItems::FlexStart,
            AlignItemsStyle::Center => AlignItems::Center,
            AlignItemsStyle::End => AlignItems::FlexEnd,
            AlignItemsStyle::Stretch => AlignItems::Stretch,
        });
    }
    if let Some(justify_content) = layout.justify_content {
        style.justify_content = Some(match justify_content {
            JustifyContentStyle::Start => JustifyContent::FlexStart,
            JustifyContentStyle::Center => JustifyContent::Center,
            JustifyContentStyle::End => JustifyContent::FlexEnd,
            JustifyContentStyle::SpaceBetween => JustifyContent::SpaceBetween,
            JustifyContentStyle::SpaceAround => JustifyContent::SpaceAround,
            JustifyContentStyle::SpaceEvenly => JustifyContent::SpaceEvenly,
        });
    }
    if let Some(width) = layout_dimension(
        layout.width_value,
        layout.width,
        sf,
        parent_size.map(|size| size.0),
    ) {
        style.size.width = width;
    }
    if let Some(height) = layout_dimension(
        layout.height_value,
        layout.height,
        sf,
        parent_size.map(|size| size.1),
    ) {
        style.size.height = height;
    }
    let has_authored_preferred_size =
        layout_has_authored_preferred_size(layout, parent_flex_direction);
    if has_authored_preferred_size && layout.flex_grow.is_none() {
        style.flex_grow = 0.0;
    }
    if has_authored_preferred_size
        && parent_preserves_preferred_main_size
        && layout.flex_shrink.is_none()
    {
        style.flex_shrink = 0.0;
    }
    if parent_preserves_preferred_main_size
        && layout.flex_shrink.is_none()
        && matches!(
            node.kind,
            WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Collapsible
        )
    {
        // A scroll/clip owner must measure its direct children at their natural
        // main-axis size. Shrinking auto-sized framed sections to the viewport
        // before scroll ranges are derived collapses their grid tracks and can
        // make descendants overlap. Flexible ScrollArea siblings deliberately
        // remain shrinkable between fixed controls.
        style.flex_shrink = 0.0;
    }
    // Only an authored main-axis dimension affects flex growth/shrink. A
    // cross-axis height in a row (or width in a column) must not make the item
    // inflexible on its parent's main axis. Authored dimensions are preferred
    // sizes; `flex_shrink: 0` or a fixed-size widget prop is the explicit
    // non-shrinking escape hatch.
    if let Some(width) = layout_dimension(
        layout.min_width_value,
        layout.min_width,
        sf,
        parent_size.map(|size| size.0),
    ) {
        style.min_size.width = width;
    }
    if let Some(height) = layout_dimension(
        layout.min_height_value,
        layout.min_height,
        sf,
        parent_size.map(|size| size.1),
    ) {
        style.min_size.height = height;
    }
    if let Some(width) = layout_dimension(
        layout.max_width_value,
        layout.max_width,
        sf,
        parent_size.map(|size| size.0),
    ) {
        style.max_size.width = width;
    }
    if let Some(height) = layout_dimension(
        layout.max_height_value,
        layout.max_height,
        sf,
        parent_size.map(|size| size.1),
    ) {
        style.max_size.height = height;
    }
    if let Some(grow) = layout.flex_grow {
        style.flex_grow = grow.max(0.0);
    }
    if let Some(shrink) = layout.flex_shrink {
        style.flex_shrink = shrink.max(0.0);
    }
    // Borders are painted inside the widget's border box, so layout must
    // reserve the same inset before positioning children. Previously the
    // visual border width never reached Taffy: bordered flex composites such
    // as SearchBox consequently placed their first and last children against
    // the outer edge, underneath the painted border.
    let border_widths = node.style.visual.effective_border_widths();
    if border_widths.iter().any(|width| *width > 0.0) {
        style.border = taffy::geometry::Rect {
            left: LengthPercentage::Length(border_widths[3] * sf),
            right: LengthPercentage::Length(border_widths[1] * sf),
            top: LengthPercentage::Length(border_widths[0] * sf),
            bottom: LengthPercentage::Length(border_widths[2] * sf),
        };
    }
    if let Some(basis) = layout_dimension(
        layout.flex_basis_value,
        layout.flex_basis,
        sf,
        parent_size.map(|size| size.0),
    ) {
        style.flex_basis = basis;
    }
    if let Some(gap) = layout_length_percentage(
        layout.gap_value,
        layout.gap,
        sf,
        parent_size.map(|size| size.0),
    ) {
        style.gap.width = gap;
    }
    if let Some(gap) = layout_length_percentage(
        layout.gap_value,
        layout.gap,
        sf,
        parent_size.map(|size| size.1),
    ) {
        style.gap.height = gap;
    }
    if let Some(column_gap) = layout_length_percentage(
        layout.column_gap_value,
        layout.column_gap,
        sf,
        parent_size.map(|size| size.0),
    ) {
        style.gap.width = column_gap;
    }
    if let Some(row_gap) = layout_length_percentage(
        layout.row_gap_value,
        layout.row_gap,
        sf,
        parent_size.map(|size| size.1),
    ) {
        style.gap.height = row_gap;
    }
    if let Some(tracks) = &layout.grid_template_columns {
        let available = authored_axis_size_px(
            layout.width_value,
            layout.width,
            sf,
            parent_size.map(|size| size.0),
        )
        .or_else(|| parent_size.map(|size| size.0));
        let gap = layout_length_percentage(
            layout.column_gap_value.or(layout.gap_value),
            layout.column_gap.or(layout.gap),
            sf,
            parent_size.map(|size| size.0),
        )
        .map(lp_value)
        .unwrap_or(0.0);
        style.grid_template_columns =
            grid_track_sizes(tracks, sf, available, gap, node.children.len());
    }
    if let Some(tracks) = &layout.grid_template_rows {
        let available = authored_axis_size_px(
            layout.height_value,
            layout.height,
            sf,
            parent_size.map(|size| size.1),
        )
        .or_else(|| parent_size.map(|size| size.1));
        let gap = layout_length_percentage(
            layout.row_gap_value.or(layout.gap_value),
            layout.row_gap.or(layout.gap),
            sf,
            parent_size.map(|size| size.1),
        )
        .map(lp_value)
        .unwrap_or(0.0);
        style.grid_template_rows =
            grid_track_sizes(tracks, sf, available, gap, node.children.len());
    }
    if let Some(flow) = layout.grid_auto_flow {
        style.grid_auto_flow = grid_auto_flow(flow);
    }
    if let Some(placement) = layout.grid_column {
        style.grid_column = grid_placement(placement);
    }
    if let Some(placement) = layout.grid_row {
        style.grid_row = grid_placement(placement);
    }
    if matches!(
        layout.position,
        Some(PositionStyle::Absolute | PositionStyle::Fixed)
    ) {
        style.position = taffy::style::Position::Absolute;
        if let Some(left) = layout.left {
            style.inset.left = LengthPercentageAuto::Length(left * sf);
        }
        if let Some(right) = layout.right {
            style.inset.right = LengthPercentageAuto::Length(right * sf);
        }
        if let Some(top) = layout.top {
            style.inset.top = LengthPercentageAuto::Length(top * sf);
        }
        if let Some(bottom) = layout.bottom {
            style.inset.bottom = LengthPercentageAuto::Length(bottom * sf);
        }
    }
    let overflow_x = layout.overflow_x.or(layout.overflow);
    let overflow_y = layout.overflow_y.or(layout.overflow);
    if overflow_x.is_some() || overflow_y.is_some() {
        style.overflow = taffy::geometry::Point {
            x: taffy_overflow(overflow_x.unwrap_or(OverflowStyle::Hidden)),
            y: taffy_overflow(overflow_y.unwrap_or(OverflowStyle::Hidden)),
        };
    }
    let margin_all_value = layout
        .margin_value
        .or_else(|| layout.margin.map(LayoutLength::LogicalPx));
    if margin_all_value.is_some()
        || layout.margin.is_some()
        || layout.margin_left.is_some()
        || layout.margin_right.is_some()
        || layout.margin_top.is_some()
        || layout.margin_bottom.is_some()
        || layout.margin_left_value.is_some()
        || layout.margin_right_value.is_some()
        || layout.margin_top_value.is_some()
        || layout.margin_bottom_value.is_some()
    {
        let current = style.margin;
        let parent_width = parent_size.map(|size| size.0);
        style.margin = taffy::geometry::Rect {
            left: cascaded_edge_length_percentage_auto(
                layout.margin_left_value,
                layout.margin_left,
                margin_all_value,
                layout.margin,
                sf,
                parent_width,
            )
            .unwrap_or(current.left),
            right: cascaded_edge_length_percentage_auto(
                layout.margin_right_value,
                layout.margin_right,
                margin_all_value,
                layout.margin,
                sf,
                parent_width,
            )
            .unwrap_or(current.right),
            top: cascaded_edge_length_percentage_auto(
                layout.margin_top_value,
                layout.margin_top,
                margin_all_value,
                layout.margin,
                sf,
                parent_width,
            )
            .unwrap_or(current.top),
            bottom: cascaded_edge_length_percentage_auto(
                layout.margin_bottom_value,
                layout.margin_bottom,
                margin_all_value,
                layout.margin,
                sf,
                parent_width,
            )
            .unwrap_or(current.bottom),
        };
    }
    let pad_all_value = layout
        .padding_value
        .or_else(|| layout.padding.map(LayoutLength::LogicalPx));
    if pad_all_value.is_some()
        || layout.padding.is_some()
        || layout.padding_left.is_some()
        || layout.padding_right.is_some()
        || layout.padding_top.is_some()
        || layout.padding_bottom.is_some()
        || layout.padding_left_value.is_some()
        || layout.padding_right_value.is_some()
        || layout.padding_top_value.is_some()
        || layout.padding_bottom_value.is_some()
    {
        let current = style.padding;
        let parent_width = parent_size.map(|size| size.0);
        style.padding = taffy::geometry::Rect {
            left: cascaded_edge_length_percentage(
                layout.padding_left_value,
                layout.padding_left,
                pad_all_value,
                layout.padding,
                sf,
                parent_width,
            )
            .unwrap_or(current.left),
            right: cascaded_edge_length_percentage(
                layout.padding_right_value,
                layout.padding_right,
                pad_all_value,
                layout.padding,
                sf,
                parent_width,
            )
            .unwrap_or(current.right),
            top: cascaded_edge_length_percentage(
                layout.padding_top_value,
                layout.padding_top,
                pad_all_value,
                layout.padding,
                sf,
                parent_width,
            )
            .unwrap_or(current.top),
            bottom: cascaded_edge_length_percentage(
                layout.padding_bottom_value,
                layout.padding_bottom,
                pad_all_value,
                layout.padding,
                sf,
                parent_width,
            )
            .unwrap_or(current.bottom),
        };
    }
    reserve_scrollbar_gutter_padding(style, node, sf);
}

fn taffy_overflow(value: OverflowStyle) -> Overflow {
    match value {
        OverflowStyle::Visible => Overflow::Visible,
        OverflowStyle::Hidden => Overflow::Hidden,
        OverflowStyle::Scroll | OverflowStyle::Auto => Overflow::Scroll,
    }
}

fn layout_has_authored_preferred_size(
    layout: &crate::style::LayoutStyle,
    parent_flex_direction: Option<FlexDirection>,
) -> bool {
    match parent_flex_direction {
        Some(FlexDirection::Row | FlexDirection::RowReverse) => {
            layout.width.is_some() || layout.width_value.is_some()
        }
        Some(FlexDirection::Column | FlexDirection::ColumnReverse) => {
            layout.height.is_some() || layout.height_value.is_some()
        }
        None => {
            layout.width.is_some()
                || layout.height.is_some()
                || layout.width_value.is_some()
                || layout.height_value.is_some()
        }
    }
}

fn reserve_scrollbar_gutter_padding(style: &mut Style, node: &WidgetNode, sf: f32) {
    if reserves_vertical_scrollbar_gutter(node) {
        let reserve = scrollbar_gutter_reserve_px(node, sf);
        style.padding.right = max_length_padding(style.padding.right, reserve);
    }
    if explicitly_scrolls_x(node) {
        let reserve = scrollbar_gutter_reserve_px(node, sf);
        style.padding.bottom = max_length_padding(style.padding.bottom, reserve);
    }
}

fn explicitly_scrolls_x(node: &WidgetNode) -> bool {
    matches!(
        node_overflow_x(node),
        Some(OverflowStyle::Scroll | OverflowStyle::Auto)
    )
}

fn explicitly_scrolls_y(node: &WidgetNode) -> bool {
    matches!(
        node_overflow_y(node),
        Some(OverflowStyle::Scroll | OverflowStyle::Auto)
    )
}

fn reserves_vertical_scrollbar_gutter(node: &WidgetNode) -> bool {
    explicitly_scrolls_y(node) || (implicit_panel_may_need_vertical_scrollbar_gutter(node))
}

fn implicit_panel_may_need_vertical_scrollbar_gutter(node: &WidgetNode) -> bool {
    is_scroll_container_kind(&node.kind)
        && node.style.layout.display != Some(DisplayStyle::Grid)
        && node_overflow_x(node).is_none()
        && node_overflow_y(node).is_none()
        && (node.style.layout.height.is_some()
            || node.style.layout.height_value.is_some()
            || node.props.fixed_height.is_some())
}

fn max_length_padding(value: LengthPercentage, min_px: f32) -> LengthPercentage {
    match value {
        LengthPercentage::Length(current) => LengthPercentage::Length(current.max(min_px)),
        LengthPercentage::Percent(_) => value,
    }
}

fn scrollbar_gutter_reserve_px(node: &WidgetNode, sf: f32) -> f32 {
    let track = scrollbar_part_width_lp(node, "scrollbar-track", 4.0);
    let thumb = scrollbar_part_width_lp(node, "scrollbar-thumb", track);
    let track_padding = node
        .style
        .parts
        .parts
        .get("scrollbar-track")
        .and_then(|part| part.layout.padding)
        .unwrap_or(0.0)
        .max(0.0);
    let edge_pad = track_padding.max(8.0);
    let content_gap = 8.0;
    (track.max(thumb).max(2.0) + edge_pad + content_gap) * sf
}

fn scrollbar_part_width_lp(node: &WidgetNode, part: &str, fallback: f32) -> f32 {
    node.style
        .parts
        .parts
        .get(part)
        .and_then(|part| part.layout.width)
        .unwrap_or(fallback)
        .max(0.0)
}

fn grid_track_size(value: GridTrackSize, sf: f32) -> TrackSizingFunction {
    if let GridTrackSize::Repeat { kind, tracks } = value {
        let mut repeated: Vec<_> = tracks
            .into_iter()
            .filter_map(|track| grid_non_repeated_track_size(track, sf))
            .collect();
        if repeated.is_empty() {
            repeated.push(NonRepeatedTrackSizingFunction::AUTO);
        }
        return TrackSizingFunction::Repeat(grid_track_repeat_kind(kind), repeated);
    }
    TrackSizingFunction::Single(
        grid_non_repeated_track_size(value, sf).unwrap_or(NonRepeatedTrackSizingFunction::AUTO),
    )
}

fn grid_track_sizes(
    values: &[GridTrackSize],
    sf: f32,
    available: Option<f32>,
    gap: f32,
    child_count: usize,
) -> Vec<TrackSizingFunction> {
    if values.len() == 1 {
        if let GridTrackSize::Repeat { kind, tracks } = &values[0] {
            if let Some(available) = available {
                let repeated_min = tracks
                    .iter()
                    .map(|track| grid_track_definite_min(track.clone(), sf, available))
                    .sum::<Option<f32>>();
                if let Some(repeated_min) = repeated_min.filter(|value| *value > 0.0) {
                    // Taffy 0.5 panics while resolving a definite auto-repeat
                    // whose minmax maximum is fractional. Expand the common
                    // single-repeat form ourselves and retain the original
                    // minmax tracks so they still absorb free space.
                    let group_gap = gap.max(0.0) * tracks.len().saturating_sub(1) as f32;
                    let group_width = repeated_min + group_gap;
                    let mut count = ((available.max(0.0) + gap.max(0.0))
                        / (group_width + gap.max(0.0)))
                    .floor()
                    .max(1.0) as usize;
                    if matches!(kind, GridTrackRepeatKind::AutoFit) && child_count > 0 {
                        count = count.min(child_count.div_ceil(tracks.len()));
                    }
                    return (0..count)
                        .flat_map(|_| tracks.iter().cloned())
                        .map(|track| {
                            TrackSizingFunction::Single(
                                grid_non_repeated_track_size(track, sf)
                                    .unwrap_or(NonRepeatedTrackSizingFunction::AUTO),
                            )
                        })
                        .collect();
                }
            }
        }
    }
    values
        .iter()
        .cloned()
        .map(|track| grid_track_size(track, sf))
        .collect()
}

fn grid_track_definite_min(value: GridTrackSize, sf: f32, available: f32) -> Option<f32> {
    match value {
        GridTrackSize::LogicalPx(value) => Some(value * sf),
        GridTrackSize::Percent(value) => Some(available * value / 100.0),
        GridTrackSize::MinMax { min, .. } => match min {
            GridTrackMinSize::LogicalPx(value) => Some(value * sf),
            GridTrackMinSize::Percent(value) => Some(available * value / 100.0),
            GridTrackMinSize::Auto => None,
        },
        GridTrackSize::FitContent(GridTrackFitContentSize::LogicalPx(value)) => Some(value * sf),
        GridTrackSize::FitContent(GridTrackFitContentSize::Percent(value)) => {
            Some(available * value / 100.0)
        }
        GridTrackSize::Fraction(_) | GridTrackSize::Auto | GridTrackSize::Repeat { .. } => None,
    }
}

fn grid_non_repeated_track_size(
    value: GridTrackSize,
    sf: f32,
) -> Option<NonRepeatedTrackSizingFunction> {
    match value {
        GridTrackSize::LogicalPx(value) => Some(non_repeated_track_size_fixed(
            LengthPercentage::Length(value * sf),
        )),
        GridTrackSize::Percent(value) => Some(non_repeated_track_size_fixed(
            LengthPercentage::Percent(value / 100.0),
        )),
        GridTrackSize::Fraction(value) => Some(NonRepeatedTrackSizingFunction {
            min: MinTrackSizingFunction::Auto,
            max: MaxTrackSizingFunction::Fraction(value),
        }),
        GridTrackSize::Auto => Some(NonRepeatedTrackSizingFunction {
            min: MinTrackSizingFunction::Auto,
            max: MaxTrackSizingFunction::Auto,
        }),
        GridTrackSize::FitContent(value) => Some(NonRepeatedTrackSizingFunction::fit_content(
            grid_track_fit_content_size(value, sf),
        )),
        GridTrackSize::MinMax { min, max } => Some(NonRepeatedTrackSizingFunction {
            min: grid_track_min_size(min, sf),
            max: grid_track_max_size(max, sf),
        }),
        GridTrackSize::Repeat { .. } => None,
    }
}

fn grid_track_repeat_kind(value: GridTrackRepeatKind) -> GridTrackRepetition {
    match value {
        GridTrackRepeatKind::AutoFit => GridTrackRepetition::AutoFit,
        GridTrackRepeatKind::AutoFill => GridTrackRepetition::AutoFill,
    }
}

fn grid_auto_flow(value: GridAutoFlowStyle) -> taffy::style::GridAutoFlow {
    match value {
        GridAutoFlowStyle::Row => taffy::style::GridAutoFlow::Row,
        GridAutoFlowStyle::Column => taffy::style::GridAutoFlow::Column,
        GridAutoFlowStyle::RowDense => taffy::style::GridAutoFlow::RowDense,
        GridAutoFlowStyle::ColumnDense => taffy::style::GridAutoFlow::ColumnDense,
    }
}

fn non_repeated_track_size_fixed(value: LengthPercentage) -> NonRepeatedTrackSizingFunction {
    NonRepeatedTrackSizingFunction {
        min: MinTrackSizingFunction::Fixed(value),
        max: MaxTrackSizingFunction::Fixed(value),
    }
}

fn grid_track_fit_content_size(value: GridTrackFitContentSize, sf: f32) -> LengthPercentage {
    match value {
        GridTrackFitContentSize::LogicalPx(value) => LengthPercentage::Length(value * sf),
        GridTrackFitContentSize::Percent(value) => LengthPercentage::Percent(value / 100.0),
    }
}

fn grid_track_min_size(value: GridTrackMinSize, sf: f32) -> MinTrackSizingFunction {
    match value {
        GridTrackMinSize::LogicalPx(value) => {
            MinTrackSizingFunction::Fixed(LengthPercentage::Length(value * sf))
        }
        GridTrackMinSize::Percent(value) => {
            MinTrackSizingFunction::Fixed(LengthPercentage::Percent(value / 100.0))
        }
        GridTrackMinSize::Auto => MinTrackSizingFunction::Auto,
    }
}

fn grid_track_max_size(value: GridTrackMaxSize, sf: f32) -> MaxTrackSizingFunction {
    match value {
        GridTrackMaxSize::LogicalPx(value) => {
            MaxTrackSizingFunction::Fixed(LengthPercentage::Length(value * sf))
        }
        GridTrackMaxSize::Percent(value) => {
            MaxTrackSizingFunction::Fixed(LengthPercentage::Percent(value / 100.0))
        }
        GridTrackMaxSize::Fraction(value) => MaxTrackSizingFunction::Fraction(value),
        GridTrackMaxSize::Auto => MaxTrackSizingFunction::Auto,
    }
}

fn grid_placement(value: GridPlacementStyle) -> taffy::geometry::Line<GridPlacement> {
    taffy::geometry::Line {
        start: grid_line(value.start),
        end: grid_line(value.end),
    }
}

fn grid_line(value: GridLineStyle) -> GridPlacement {
    match value {
        GridLineStyle::Auto => GridPlacement::Auto,
        GridLineStyle::Line(value) => GridPlacement::from_line_index(value),
        GridLineStyle::Span(value) => GridPlacement::from_span(value),
    }
}

fn lp_value(value: LengthPercentage) -> f32 {
    match value {
        LengthPercentage::Length(v) => v,
        LengthPercentage::Percent(_) => 0.0,
    }
}

fn definite_content_size(style: &Style, parent_size: Option<(f32, f32)>) -> Option<(f32, f32)> {
    let width =
        definite_dimension_for_content(style.size.width, style, parent_size.map(|size| size.0))?;
    let height =
        definite_dimension_for_content(style.size.height, style, parent_size.map(|size| size.1))?;
    let padding_x = lp_value(style.padding.left) + lp_value(style.padding.right);
    let padding_y = lp_value(style.padding.top) + lp_value(style.padding.bottom);
    Some(((width - padding_x).max(0.0), (height - padding_y).max(0.0)))
}

fn definite_dimension_for_content(
    value: Dimension,
    style: &Style,
    parent_axis_size: Option<f32>,
) -> Option<f32> {
    match value {
        Dimension::Length(value) if value <= 0.5 && flex_basis_is_zero(style) => parent_axis_size,
        other => definite_dimension(other).or(parent_axis_size),
    }
}

fn flex_basis_is_zero(style: &Style) -> bool {
    style.flex_grow > 0.0 && matches!(style.flex_basis, Dimension::Length(value) if value <= 0.5)
}

fn definite_dimension(value: Dimension) -> Option<f32> {
    match value {
        Dimension::Length(value) => Some(value),
        Dimension::Percent(_) | Dimension::Auto => None,
    }
}

fn layout_dimension(
    value: Option<LayoutLength>,
    legacy_px: Option<f32>,
    sf: f32,
    parent_axis_size: Option<f32>,
) -> Option<Dimension> {
    match value {
        Some(LayoutLength::LogicalPx(value)) => Some(Dimension::Length(value * sf)),
        Some(LayoutLength::Percent(value)) => Some(Dimension::Percent(value / 100.0)),
        Some(LayoutLength::Calc(value)) if value.percent == 0.0 => {
            Some(Dimension::Length(value.px * sf))
        }
        Some(LayoutLength::Calc(value)) if value.px == 0.0 => {
            Some(Dimension::Percent(value.percent / 100.0))
        }
        Some(LayoutLength::Calc(value)) => parent_axis_size.map(|parent| {
            Dimension::Length((parent * (value.percent / 100.0) + value.px * sf).max(0.0))
        }),
        Some(LayoutLength::Auto) => Some(Dimension::Auto),
        None => legacy_px.map(|value| Dimension::Length(value * sf)),
    }
}

fn layout_length_percentage(
    value: Option<LayoutLength>,
    legacy_px: Option<f32>,
    sf: f32,
    parent_axis_size: Option<f32>,
) -> Option<LengthPercentage> {
    match value {
        Some(LayoutLength::LogicalPx(value)) => Some(LengthPercentage::Length(value * sf)),
        Some(LayoutLength::Percent(value)) => Some(LengthPercentage::Percent(value / 100.0)),
        Some(LayoutLength::Calc(value)) if value.percent == 0.0 => {
            Some(LengthPercentage::Length(value.px * sf))
        }
        Some(LayoutLength::Calc(value)) if value.px == 0.0 => {
            Some(LengthPercentage::Percent(value.percent / 100.0))
        }
        Some(LayoutLength::Calc(value)) => parent_axis_size.map(|parent| {
            LengthPercentage::Length(parent * (value.percent / 100.0) + value.px * sf)
        }),
        Some(LayoutLength::Auto) => None,
        None => legacy_px.map(|value| LengthPercentage::Length(value * sf)),
    }
}

fn cascaded_edge_length_percentage(
    side_value: Option<LayoutLength>,
    side_legacy: Option<f32>,
    all_value: Option<LayoutLength>,
    all_legacy: Option<f32>,
    sf: f32,
    parent_axis_size: Option<f32>,
) -> Option<LengthPercentage> {
    if side_value.is_some() {
        layout_length_percentage(side_value, None, sf, parent_axis_size)
    } else if side_legacy.is_some() {
        layout_length_percentage(None, side_legacy, sf, parent_axis_size)
    } else {
        layout_length_percentage(all_value, all_legacy, sf, parent_axis_size)
    }
}

fn layout_length_percentage_auto(
    value: Option<LayoutLength>,
    legacy_px: Option<f32>,
    sf: f32,
    parent_axis_size: Option<f32>,
) -> Option<LengthPercentageAuto> {
    match value {
        Some(LayoutLength::Auto) => Some(LengthPercentageAuto::Auto),
        Some(other) => layout_length_percentage(Some(other), legacy_px, sf, parent_axis_size)
            .map(LengthPercentageAuto::from),
        None => legacy_px.map(|value| LengthPercentageAuto::Length(value * sf)),
    }
}

fn cascaded_edge_length_percentage_auto(
    side_value: Option<LayoutLength>,
    side_legacy: Option<f32>,
    all_value: Option<LayoutLength>,
    all_legacy: Option<f32>,
    sf: f32,
    parent_axis_size: Option<f32>,
) -> Option<LengthPercentageAuto> {
    if side_value.is_some() {
        layout_length_percentage_auto(side_value, None, sf, parent_axis_size)
    } else if side_legacy.is_some() {
        layout_length_percentage_auto(None, side_legacy, sf, parent_axis_size)
    } else {
        layout_length_percentage_auto(all_value, all_legacy, sf, parent_axis_size)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeparatorOrientation {
    Horizontal,
    Vertical,
}

fn separator_orientation(
    node: &WidgetNode,
    parent_kind: Option<&WidgetKind>,
) -> SeparatorOrientation {
    match node.props.orientation.as_deref() {
        Some("vertical") => SeparatorOrientation::Vertical,
        Some("horizontal") => SeparatorOrientation::Horizontal,
        _ if parent_kind == Some(&WidgetKind::HLayout)
            || parent_kind == Some(&WidgetKind::StatusBar) =>
        {
            SeparatorOrientation::Vertical
        }
        _ => SeparatorOrientation::Horizontal,
    }
}

// ---------------------------------------------------------------------------
// Layout collector — DFS with accumulated absolute offset
// ---------------------------------------------------------------------------

fn collect<NodeContext>(
    tree: &TaffyTree<NodeContext>,
    node_id: NodeId,
    widget: &WidgetNode,
    parent_x: f32,
    parent_y: f32,
    result: &mut LayoutResult,
) {
    let layout = tree.layout(node_id).expect("taffy layout missing");
    let abs_x = parent_x + layout.location.x;
    let abs_y = parent_y + layout.location.y;
    let rect = Rect {
        x: abs_x,
        y: abs_y,
        w: layout.size.width,
        h: layout.size.height,
    };
    result.rects.insert(widget.id.clone(), rect);
    result.resolved_borders.insert(
        widget.id.clone(),
        ResolvedEdges {
            left: layout.border.left,
            right: layout.border.right,
            top: layout.border.top,
            bottom: layout.border.bottom,
        },
    );
    result.resolved_padding.insert(
        widget.id.clone(),
        ResolvedEdges {
            left: layout.padding.left,
            right: layout.padding.right,
            top: layout.padding.top,
            bottom: layout.padding.bottom,
        },
    );

    let child_ids = tree.children(node_id).expect("taffy children missing");
    if titled_container_uses_body_layout(widget) {
        if let Some(body_id) = child_ids.first() {
            let body_layout = tree.layout(*body_id).expect("taffy body layout missing");
            let body_abs_x = abs_x + body_layout.location.x;
            let body_abs_y = abs_y + body_layout.location.y;
            let body_child_ids = tree
                .children(*body_id)
                .expect("taffy body children missing");
            for (child_id, child_widget) in body_child_ids.iter().zip(widget.children.iter()) {
                collect(
                    tree,
                    *child_id,
                    child_widget,
                    body_abs_x,
                    body_abs_y,
                    result,
                );
            }
        }
    } else {
        for (child_id, child_widget) in child_ids.iter().zip(widget.children.iter()) {
            collect(tree, *child_id, child_widget, abs_x, abs_y, result);
        }
    }
}

fn compute_pre_scroll_clips(root: &WidgetNode, result: &mut LayoutResult, sf: f32, theme: &Theme) {
    result.clips.clear();
    result.paint_clips.clear();
    let mut clip_path_ids = HashSet::new();
    collect_pre_scroll_clip_path_ids(root, &mut clip_path_ids);
    if clip_path_ids.is_empty() {
        return;
    }
    let Some(root_rect) = result.rects.get(&root.id).copied() else {
        return;
    };
    compute_node_clips(
        root,
        result,
        root_rect,
        root_rect,
        sf,
        theme,
        false,
        Some(&clip_path_ids),
    );
}

fn compute_clips(root: &WidgetNode, result: &mut LayoutResult, sf: f32, theme: &Theme) {
    result.clips.clear();
    result.paint_clips.clear();
    let Some(root_rect) = result.rects.get(&root.id).copied() else {
        return;
    };
    compute_node_clips(root, result, root_rect, root_rect, sf, theme, true, None);
}

fn collect_pre_scroll_clip_path_ids(
    node: &WidgetNode,
    clip_path_ids: &mut HashSet<String>,
) -> bool {
    let mut contains_scroll_owner = is_scroll_container_node(node);
    for child in &node.children {
        contains_scroll_owner |= collect_pre_scroll_clip_path_ids(child, clip_path_ids);
    }
    if contains_scroll_owner {
        clip_path_ids.insert(node.id.clone());
    }
    contains_scroll_owner
}

fn compute_node_clips(
    node: &WidgetNode,
    result: &mut LayoutResult,
    parent_clip: Rect,
    root_clip: Rect,
    sf: f32,
    theme: &Theme,
    collect_paint_clips: bool,
    clip_path_ids: Option<&HashSet<String>>,
) {
    if clip_path_ids.is_some_and(|ids| !ids.contains(&node.id)) {
        return;
    }
    let Some(rect) = result.rects.get(&node.id).copied() else {
        return;
    };
    let parent_clip = if is_fixed_positioned_node(node) || is_viewport_overlay_node(node) {
        root_clip
    } else {
        parent_clip
    };
    if collect_paint_clips {
        result.paint_clips.insert(node.id.clone(), parent_clip);
    }
    let clip = rect
        .intersect(parent_clip)
        .unwrap_or_else(|| empty_rect_within(rect, parent_clip));
    result.clips.insert(node.id.clone(), clip);
    let child_clip = active_tab_content_clip(node, parent_clip, clip).unwrap_or_else(|| {
        scroll_container_child_clip(node, result, clip, sf, theme)
            .unwrap_or_else(|| child_clip_for_overflow(node, parent_clip, clip))
    });
    for child in &node.children {
        compute_node_clips(
            child,
            result,
            child_clip,
            root_clip,
            sf,
            theme,
            collect_paint_clips,
            clip_path_ids,
        );
    }
}

fn active_tab_content_clip(node: &WidgetNode, parent_clip: Rect, node_clip: Rect) -> Option<Rect> {
    if node.kind != WidgetKind::Tab || node.children.is_empty() {
        return None;
    }
    let top = node_clip.y + node_clip.h;
    let bottom = parent_clip.y + parent_clip.h;
    Some(Rect {
        x: parent_clip.x,
        y: top,
        w: parent_clip.w,
        h: (bottom - top).max(0.0),
    })
}

pub(crate) fn is_scroll_container_kind(kind: &WidgetKind) -> bool {
    matches!(kind, WidgetKind::Panel | WidgetKind::Modal)
}

pub(crate) fn is_scroll_container_node(node: &WidgetNode) -> bool {
    scroll_container_scrolls_x(node) || scroll_container_scrolls_y(node)
}

fn scroll_container_scrolls_x(node: &WidgetNode) -> bool {
    matches!(
        node_overflow_x(node),
        Some(OverflowStyle::Scroll | OverflowStyle::Auto)
    )
}

fn scroll_container_scrolls_y(node: &WidgetNode) -> bool {
    match node_overflow_y(node) {
        Some(OverflowStyle::Scroll | OverflowStyle::Auto) => true,
        Some(OverflowStyle::Visible | OverflowStyle::Hidden) => false,
        None => is_scroll_container_kind(&node.kind),
    }
}

pub(crate) fn scroll_container_max_x(node: &WidgetNode, result: &LayoutResult) -> f32 {
    scroll_geometry(node, result, false, result.scale_factor, &Theme::dark()).max_x
}

#[derive(Debug, Clone, Copy, Default)]
struct ScrollGeometry {
    viewport: Rect,
    content_bounds: Rect,
    max_x: f32,
    max_y: f32,
}

fn scroll_geometry(
    node: &WidgetNode,
    result: &LayoutResult,
    use_own_viewport: bool,
    sf: f32,
    theme: &Theme,
) -> ScrollGeometry {
    let Some(resolved_box) = result.resolved_box(&node.id) else {
        return ScrollGeometry::default();
    };
    let body_viewport = scroll_container_body_viewport(node, result, resolved_box, sf, theme);
    let viewport = if use_own_viewport {
        body_viewport
    } else {
        let clip = result
            .clips
            .get(&node.id)
            .copied()
            .unwrap_or(resolved_box.border_box);
        clip.intersect(body_viewport)
            .unwrap_or_else(|| empty_rect_within(body_viewport, clip))
    };
    let content_bounds =
        scroll_content_bounds(node, result, resolved_box, sf).unwrap_or(resolved_box.content_box);
    let mut geometry = ScrollGeometry {
        viewport,
        content_bounds,
        max_x: 0.0,
        max_y: 0.0,
    };
    if scroll_container_scrolls_x(node) {
        geometry.max_x = (geometry.content_bounds.x + geometry.content_bounds.w
            - (geometry.viewport.x + geometry.viewport.w))
            .max(0.0);
    }
    if scroll_container_scrolls_y(node) {
        geometry.max_y = (geometry.content_bounds.y + geometry.content_bounds.h
            - (geometry.viewport.y + geometry.viewport.h))
            .max(0.0);
    }
    geometry
}

pub(crate) fn scroll_container_max_y(node: &WidgetNode, result: &LayoutResult) -> f32 {
    scroll_geometry(node, result, false, result.scale_factor, &Theme::dark()).max_y
}

fn scroll_container_body_viewport(
    node: &WidgetNode,
    layout: &LayoutResult,
    resolved_box: ResolvedBox,
    sf: f32,
    theme: &Theme,
) -> Rect {
    titled_container_geometry(node, layout, sf, theme)
        .map(|geometry| {
            debug_assert!(geometry.body_content_origin_y + 0.1 >= geometry.body_viewport.y);
            geometry.body_viewport
        })
        .unwrap_or(resolved_box.padding_box)
}

fn scroll_container_child_clip(
    node: &WidgetNode,
    result: &LayoutResult,
    clip: Rect,
    sf: f32,
    theme: &Theme,
) -> Option<Rect> {
    if !is_scroll_container_node(node)
        || !node
            .props
            .text
            .as_deref()
            .is_some_and(|text| !text.is_empty())
    {
        return None;
    }
    let resolved_box = result.resolved_box(&node.id)?;
    let content = scroll_container_body_viewport(node, result, resolved_box, sf, theme);
    clip.intersect(content)
        .or_else(|| Some(empty_rect_within(content, clip)))
}

fn child_clip_for_overflow(node: &WidgetNode, parent_clip: Rect, node_clip: Rect) -> Rect {
    match node_overflow_y(node).or_else(|| node_overflow_x(node)) {
        Some(OverflowStyle::Hidden | OverflowStyle::Scroll | OverflowStyle::Auto) => node_clip,
        Some(OverflowStyle::Visible) => parent_clip,
        None if matches!(
            node.kind,
            WidgetKind::HLayout
                | WidgetKind::VLayout
                | WidgetKind::ScrollArea
                | WidgetKind::GridLayout
                | WidgetKind::FlowLayout
                | WidgetKind::Pages
                | WidgetKind::Page
        ) =>
        {
            parent_clip
        }
        None => node_clip,
    }
}

fn node_overflow_x(node: &WidgetNode) -> Option<OverflowStyle> {
    node.style
        .layout
        .overflow_x
        .or(node.style.layout.overflow)
        .or_else(|| (node.kind == WidgetKind::ScrollArea).then_some(OverflowStyle::Hidden))
}

fn node_overflow_y(node: &WidgetNode) -> Option<OverflowStyle> {
    node.style
        .layout
        .overflow_y
        .or(node.style.layout.overflow)
        .or_else(|| (node.kind == WidgetKind::ScrollArea).then_some(OverflowStyle::Auto))
}

fn scroll_content_bounds(
    node: &WidgetNode,
    result: &LayoutResult,
    resolved_box: ResolvedBox,
    sf: f32,
) -> Option<Rect> {
    let mut left = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    let mut top = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for child in &node.children {
        scroll_content_bounds_for_child(
            child,
            result,
            &mut left,
            &mut right,
            &mut top,
            &mut bottom,
        );
    }
    if top.is_finite() {
        let body_padding = if titled_container_uses_body_layout(node) {
            panel_body_padding_lp(node) * sf
        } else {
            0.0
        };
        left = left.min(resolved_box.content_box.x);
        top = top.min(resolved_box.content_box.y);
        right += resolved_box.padding.right + body_padding;
        bottom += resolved_box.padding.bottom + body_padding;
        Some(Rect {
            x: left,
            y: top,
            w: (right - left).max(0.0),
            h: (bottom - top).max(0.0),
        })
    } else {
        None
    }
}

fn scroll_content_bounds_for_child(
    node: &WidgetNode,
    result: &LayoutResult,
    left: &mut f32,
    right: &mut f32,
    top: &mut f32,
    bottom: &mut f32,
) {
    if is_fixed_positioned_node(node) {
        return;
    }
    let Some(rect) = result.rects.get(&node.id).copied() else {
        return;
    };
    if rect.w <= 0.0 || rect.h <= 0.0 {
        return;
    }

    *left = left.min(rect.x);
    *right = right.max(rect.x + rect.w);
    *top = top.min(rect.y);
    *bottom = bottom.max(rect.y + rect.h);

    if subtree_scroll_bounds_stop_at_node(node) {
        return;
    }
    for child in &node.children {
        scroll_content_bounds_for_child(child, result, left, right, top, bottom);
    }
}

fn subtree_scroll_bounds_stop_at_node(node: &WidgetNode) -> bool {
    if is_scroll_container_node(node) {
        return true;
    }
    matches!(
        node_overflow_x(node),
        Some(OverflowStyle::Hidden | OverflowStyle::Scroll | OverflowStyle::Auto)
    ) || matches!(
        node_overflow_y(node),
        Some(OverflowStyle::Hidden | OverflowStyle::Scroll | OverflowStyle::Auto)
    )
}

fn apply_scroll_offsets(
    root: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: Option<&WidgetState>,
) {
    let Some(state) = state else {
        return;
    };
    apply_node_scroll_offsets(root, result, sf, theme, state, false);
}

fn apply_node_scroll_offsets(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: &WidgetState,
    inside_scrolled_ancestor: bool,
) {
    if is_scroll_container_node(node) {
        let use_own_viewport = inside_scrolled_ancestor;
        let geometry = scroll_geometry(node, result, use_own_viewport, sf, theme);
        let max_scroll_x = geometry.max_x;
        let max_scroll_y = geometry.max_y;
        let scroll_x = state.container_scroll_x(&node.id, max_scroll_x);
        let scroll_y = state.container_scroll_y(&node.id, max_scroll_y);
        result.scroll_max_x.insert(node.id.clone(), max_scroll_x);
        result.scroll_max_y.insert(node.id.clone(), max_scroll_y);
        result.scroll_x.insert(node.id.clone(), scroll_x);
        result.scroll_y.insert(node.id.clone(), scroll_y);
        if scroll_x > 0.0 || scroll_y > 0.0 {
            for child in &node.children {
                if is_fixed_positioned_node(child) {
                    continue;
                }
                translate_subtree(child, result, -scroll_x, -scroll_y);
            }
        }
        let inside_scrolled_ancestor =
            inside_scrolled_ancestor || max_scroll_x > 0.0 || max_scroll_y > 0.0;
        for child in &node.children {
            apply_node_scroll_offsets(child, result, sf, theme, state, inside_scrolled_ancestor);
        }
        return;
    }
    for child in &node.children {
        apply_node_scroll_offsets(child, result, sf, theme, state, inside_scrolled_ancestor);
    }
}

fn translate_subtree(node: &WidgetNode, result: &mut LayoutResult, dx: f32, dy: f32) {
    if let Some(rect) = result.rects.get_mut(&node.id) {
        rect.x += dx;
        rect.y += dy;
    }
    for child in &node.children {
        translate_subtree(child, result, dx, dy);
    }
}

fn undo_scroll_offsets(node: &WidgetNode, result: &mut LayoutResult) {
    if is_scroll_container_node(node) {
        let scroll_x = result.scroll_x.get(&node.id).copied().unwrap_or(0.0);
        let scroll_y = result.scroll_y.get(&node.id).copied().unwrap_or(0.0);
        if scroll_x > 0.0 || scroll_y > 0.0 {
            for child in &node.children {
                if is_fixed_positioned_node(child) {
                    continue;
                }
                translate_subtree(child, result, scroll_x, scroll_y);
            }
        }
    }
    for child in &node.children {
        undo_scroll_offsets(child, result);
    }
}

fn apply_fixed_positions(root: &WidgetNode, result: &mut LayoutResult, sf: f32) {
    let Some(root_rect) = result.rects.get(&root.id).copied() else {
        return;
    };
    apply_fixed_positions_for_node(root, result, root_rect, sf);
}

fn apply_fixed_positions_for_node(
    node: &WidgetNode,
    result: &mut LayoutResult,
    root_rect: Rect,
    sf: f32,
) {
    if is_fixed_positioned_node(node) {
        rebase_fixed_node(node, result, root_rect, sf);
    }
    for child in &node.children {
        apply_fixed_positions_for_node(child, result, root_rect, sf);
    }
}

fn rebase_fixed_node(node: &WidgetNode, result: &mut LayoutResult, root_rect: Rect, sf: f32) {
    let Some(old_rect) = result.rects.get(&node.id).copied() else {
        return;
    };
    let layout = &node.style.layout;
    let x = match (layout.left, layout.right) {
        (Some(left), _) => root_rect.x + left * sf,
        (None, Some(right)) => root_rect.x + root_rect.w - right * sf - old_rect.w,
        (None, None) => old_rect.x,
    };
    let y = match (layout.top, layout.bottom) {
        (Some(top), _) => root_rect.y + top * sf,
        (None, Some(bottom)) => root_rect.y + root_rect.h - bottom * sf - old_rect.h,
        (None, None) => old_rect.y,
    };
    if let Some(rect) = result.rects.get_mut(&node.id) {
        rect.x = x;
        rect.y = y;
    }
    let dx = x - old_rect.x;
    let dy = y - old_rect.y;
    if dx != 0.0 || dy != 0.0 {
        for child in &node.children {
            translate_subtree(child, result, dx, dy);
        }
    }
}

fn is_fixed_positioned_node(node: &WidgetNode) -> bool {
    node.style.layout.position == Some(PositionStyle::Fixed)
}

fn is_viewport_overlay_node(node: &WidgetNode) -> bool {
    // Rich tooltips remain children of their target's retained widget subtree,
    // but apply_tooltip_layout() promotes their geometry into window space.
    // Their paint and descendant clips must follow that promoted geometry rather
    // than an overflow clip inherited from the target's panel.
    node.kind == WidgetKind::Tooltip
}

fn apply_titled_container_absolute_offsets(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
) {
    if titled_container_has_body_offset(node) {
        let body_offset = titled_container_geometry(node, result, sf, theme)
            .and_then(|geometry| {
                result
                    .rects
                    .get(&node.id)
                    .map(|rect| geometry.body_viewport.y - rect.y)
            })
            .unwrap_or(0.0);
        if body_offset > 0.0 {
            for child in &node.children {
                if child.style.layout.position == Some(PositionStyle::Absolute)
                    && child.style.layout.top.is_some()
                {
                    translate_subtree(child, result, 0.0, body_offset);
                }
            }
        }
    }
    for child in &node.children {
        apply_titled_container_absolute_offsets(child, result, sf, theme);
    }
}

fn titled_container_has_body_offset(node: &WidgetNode) -> bool {
    if titled_container_uses_body_layout(node) {
        return false;
    }
    matches!(
        node.kind,
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal
    ) && node
        .props
        .text
        .as_deref()
        .is_some_and(|text| !text.is_empty())
}

fn apply_navigation_layout(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: Option<&WidgetState>,
) {
    match node.kind {
        WidgetKind::Tabs => layout_tabs(node, result, sf, theme, state),
        WidgetKind::Pages => layout_pages(node, result, sf, theme, state),
        _ => {
            for child in &node.children {
                apply_navigation_layout(child, result, sf, theme, state);
            }
        }
    }
}

fn apply_modal_layout(root: &WidgetNode, result: &mut LayoutResult, sf: f32, theme: &Theme) {
    let root_rect = result.rects.get(&root.id).copied().unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0 * sf,
        h: 600.0 * sf,
    });
    for modal in open_modals(root) {
        layout_modal(modal, root_rect, result, sf, theme);
    }
}

fn apply_tooltip_layout(
    root: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: Option<&WidgetState>,
) {
    let Some(state) = state else {
        return;
    };
    let Some(hovered) = state.hovered.as_deref() else {
        return;
    };
    let Some(tooltip) = active_tooltip(root, hovered) else {
        return;
    };
    let Some(target) = result.rects.get(hovered).copied() else {
        return;
    };
    let root_rect = result.rects.get(&root.id).copied().unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: target.x + target.w,
        h: target.y + target.h,
    });
    let margin = theme.spacing * sf;
    let width = tooltip.props.fixed_width.unwrap_or(280.0).max(80.0) * sf;
    let height = tooltip
        .props
        .fixed_height
        .map(|height| height.max(32.0) * sf)
        .unwrap_or_else(|| estimate_tooltip_height(tooltip, theme, sf));
    let rect = place_tooltip_rect(target, root_rect, width, height, margin);
    result.rects.insert(tooltip.id.clone(), rect);
    layout_overlay_children(tooltip, rect, result, sf, theme, state);
}

fn active_tooltip<'a>(node: &'a WidgetNode, hovered: &str) -> Option<&'a WidgetNode> {
    for child in node.children.iter().rev() {
        if let Some(found) = active_tooltip(child, hovered) {
            return Some(found);
        }
    }
    (node.kind == WidgetKind::Tooltip && node.props.target.as_deref() == Some(hovered))
        .then_some(node)
}

fn estimate_tooltip_height(node: &WidgetNode, theme: &Theme, sf: f32) -> f32 {
    let pad = theme.panel_padding.max(0.0) * sf;
    let gap = (theme.spacing * 0.75) * sf;
    let child_count = node.children.len().max(1) as f32;
    let child_height = node
        .children
        .iter()
        .map(|child| estimated_node_height(child, theme, sf))
        .sum::<f32>();
    (child_height + pad * 2.0 + gap * (child_count - 1.0)).clamp(32.0 * sf, 320.0 * sf)
}

fn estimated_node_height(node: &WidgetNode, theme: &Theme, sf: f32) -> f32 {
    match node.kind {
        WidgetKind::TextArea | WidgetKind::CodeEditor | WidgetKind::LogView => {
            text_area_height_lp(node, theme) * sf
        }
        WidgetKind::Panel | WidgetKind::VLayout | WidgetKind::HLayout => {
            estimate_tooltip_height(node, theme, sf)
        }
        WidgetKind::Separator => 1.0 * sf,
        WidgetKind::Spacer => node.props.fixed_height.unwrap_or(theme.spacing) * sf,
        _ => node_control_height_lp(node, theme) * sf,
    }
}

fn place_tooltip_rect(target: Rect, root: Rect, width: f32, height: f32, margin: f32) -> Rect {
    let below_y = target.y + target.h + margin;
    let above_y = target.y - height - margin;
    let y = if below_y + height <= root.y + root.h - margin {
        below_y
    } else {
        above_y
    };
    let x = target.x + target.w * 0.5 - width * 0.5;
    clamp_rect_to_root(
        Rect {
            x,
            y,
            w: width,
            h: height,
        },
        root,
        margin,
    )
}

fn clamp_rect_to_root(rect: Rect, root: Rect, margin: f32) -> Rect {
    let available_w = (root.w - margin * 2.0).max(0.0);
    let available_h = (root.h - margin * 2.0).max(0.0);
    let rect = Rect {
        w: rect.w.max(0.0).min(available_w),
        h: rect.h.max(0.0).min(available_h),
        ..rect
    };
    let min_x = root.x + margin;
    let max_x = (root.x + root.w - rect.w - margin).max(min_x);
    let min_y = root.y + margin;
    let max_y = (root.y + root.h - rect.h - margin).max(min_y);
    Rect {
        x: rect.x.clamp(min_x, max_x),
        y: rect.y.clamp(min_y, max_y),
        ..rect
    }
}

fn open_modals(node: &WidgetNode) -> Vec<&WidgetNode> {
    let mut out = Vec::new();
    collect_open_modals(node, &mut out);
    out
}

fn collect_open_modals<'a>(node: &'a WidgetNode, out: &mut Vec<&'a WidgetNode>) {
    if node.kind == WidgetKind::Modal && node.props.open.unwrap_or(false) {
        out.push(node);
    }
    for child in &node.children {
        collect_open_modals(child, out);
    }
}

fn layout_modal(
    modal: &WidgetNode,
    root_rect: Rect,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
) {
    let preferred_margin = (theme.spacing * 3.0 * sf).max(16.0 * sf);
    let margin_x = preferred_margin.min(root_rect.w.max(0.0) * 0.25);
    let margin_y = preferred_margin.min(root_rect.h.max(0.0) * 0.25);
    let max_w = (root_rect.w - margin_x * 2.0).max(0.0);
    let max_h = (root_rect.h - margin_y * 2.0).max(0.0);
    let min_w = (80.0 * sf).min(max_w);
    let min_h = (80.0 * sf).min(max_h);
    let modal_w = modal
        .props
        .fixed_width
        .map(|w| w * sf)
        .unwrap_or(420.0 * sf)
        .clamp(min_w, max_w);
    let modal_h = modal
        .props
        .fixed_height
        .map(|h| h * sf)
        .unwrap_or(220.0 * sf)
        .clamp(min_h, max_h);
    let x = root_rect.x + (root_rect.w - modal_w) * 0.5;
    let y = root_rect.y + (root_rect.h - modal_h) * 0.5;

    let mut tree: TaffyTree<LeafMeasureContext> = TaffyTree::new();
    let root_id = build_node(
        &mut tree,
        modal,
        sf,
        theme,
        Some((modal_w, modal_h)),
        None,
        None,
        None,
        None,
        None,
        false,
        false,
        true,
        None,
        None,
        root_rect.w / sf.max(0.001),
    );
    compute_taffy_layout(
        &mut tree,
        root_id,
        Size {
            width: AvailableSpace::Definite(modal_w),
            height: AvailableSpace::Definite(modal_h),
        },
        theme,
    )
    .expect("taffy modal layout failed");
    collect(&tree, root_id, modal, x, y, result);
}

fn layout_tabs(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: Option<&WidgetState>,
) {
    let Some(r) = result.rects.get(&node.id).copied() else {
        return;
    };
    let tabs: Vec<&WidgetNode> = node
        .children
        .iter()
        .filter(|child| child.kind == WidgetKind::Tab)
        .collect();
    if tabs.is_empty() {
        return;
    }

    let header_h = tabs_header_height_for_style(&node.style, theme, sf);
    let tab_area = result
        .resolved_box(&node.id)
        .map(|resolved| resolved.content_box)
        .unwrap_or(r);
    let gap = layout_length_percentage(
        node.style
            .layout
            .column_gap_value
            .or(node.style.layout.gap_value),
        node.style.layout.column_gap.or(node.style.layout.gap),
        sf,
        Some(tab_area.w),
    )
    .map(lp_value)
    .unwrap_or(0.0)
    .max(0.0);
    let total_gap = gap * tabs.len().saturating_sub(1) as f32;
    let tab_w = ((tab_area.w - total_gap).max(0.0) / tabs.len() as f32).max(1.0);
    let tab_h = header_h.min(tab_area.h.max(1.0));
    for (idx, tab) in tabs.iter().enumerate() {
        result.rects.insert(
            tab.id.clone(),
            Rect {
                x: tab_area.x + idx as f32 * (tab_w + gap),
                y: tab_area.y,
                w: tab_w,
                h: tab_h,
            },
        );
    }

    let active = state
        .and_then(|s| s.active_tab(&node.id))
        .or_else(|| node.props.route_value.as_deref())
        .or_else(|| {
            tabs.first()
                .and_then(|tab| tab.props.route_value.as_deref())
        });
    let active_tab = active
        .and_then(|active| {
            tabs.iter()
                .find(|tab| tab.props.route_value.as_deref() == Some(active))
                .copied()
        })
        .or_else(|| tabs.first().copied());
    if let Some(active_tab) = active_tab {
        for tab in &tabs {
            if tab.id != active_tab.id {
                remove_children_layout(tab, result);
            }
        }
    }
    if let Some(active_tab) = active_tab {
        let content = Rect {
            x: r.x,
            y: r.y + header_h,
            w: r.w,
            h: (r.h - header_h).max(0.0),
        };
        layout_region(&active_tab.children, content, result, sf, theme, state);
    }
}

fn layout_pages(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: Option<&WidgetState>,
) {
    let Some(r) = result.rects.get(&node.id).copied() else {
        return;
    };
    let pages: Vec<&WidgetNode> = node
        .children
        .iter()
        .filter(|child| child.kind == WidgetKind::Page)
        .collect();
    if pages.is_empty() {
        return;
    }

    let active = state
        .and_then(|s| s.active_page(&node.id))
        .or_else(|| node.props.route_value.as_deref())
        .or_else(|| {
            pages
                .first()
                .and_then(|page| page.props.route_value.as_deref())
        });
    let active_page = active
        .and_then(|active| {
            pages
                .iter()
                .find(|page| page.props.route_value.as_deref() == Some(active))
                .copied()
        })
        .or_else(|| pages.first().copied());
    if let Some(active_page) = active_page {
        for page in &pages {
            if page.id != active_page.id {
                remove_subtree_layout(page, result);
            }
        }
    }
    if let Some(active_page) = active_page {
        result.rects.insert(active_page.id.clone(), r);
        layout_page_region(active_page, r, result, sf, theme, state);
    }
}

fn layout_page_region(
    page: &WidgetNode,
    rect: Rect,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: Option<&WidgetState>,
) {
    if rect.w <= 0.0 || rect.h <= 0.0 || page.children.is_empty() {
        return;
    }
    let mut sub = compute_layout(page, rect.w, rect.h, sf, theme, state);
    undo_scroll_offsets(page, &mut sub);
    for (id, child_rect) in sub.rects {
        if id == page.id {
            continue;
        }
        result.rects.insert(
            id,
            Rect {
                x: child_rect.x + rect.x,
                y: child_rect.y + rect.y,
                w: child_rect.w,
                h: child_rect.h,
            },
        );
    }
}

fn remove_children_layout(node: &WidgetNode, result: &mut LayoutResult) {
    for child in &node.children {
        remove_subtree_layout(child, result);
    }
}

fn remove_subtree_layout(node: &WidgetNode, result: &mut LayoutResult) {
    result.rects.remove(&node.id);
    result.clips.remove(&node.id);
    result.paint_clips.remove(&node.id);
    result.scroll_x.remove(&node.id);
    result.scroll_y.remove(&node.id);
    result.scroll_max_x.remove(&node.id);
    result.scroll_max_y.remove(&node.id);
    for child in &node.children {
        remove_subtree_layout(child, result);
    }
}

fn layout_region(
    children: &[WidgetNode],
    rect: Rect,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: Option<&WidgetState>,
) {
    if rect.w <= 0.0 || rect.h <= 0.0 || children.is_empty() {
        return;
    }
    let synthetic = WidgetNode {
        id: "__dg_nav_region".to_string(),
        key: None,
        class_name: None,
        css_types: Vec::new(),
        kind: WidgetKind::VLayout,
        props: Default::default(),
        style: Default::default(),
        style_json: Default::default(),
        default_style: Default::default(),
        inline_style: Default::default(),
        children: children.to_vec(),
    };
    let mut sub = compute_layout(&synthetic, rect.w, rect.h, sf, theme, state);
    undo_scroll_offsets(&synthetic, &mut sub);
    for (id, child_rect) in sub.rects {
        if id == "__dg_nav_region" {
            continue;
        }
        result.rects.insert(
            id,
            Rect {
                x: child_rect.x + rect.x,
                y: child_rect.y + rect.y,
                w: child_rect.w,
                h: child_rect.h,
            },
        );
    }
}

fn layout_overlay_children(
    container: &WidgetNode,
    rect: Rect,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
    state: &WidgetState,
) {
    if rect.w <= 0.0 || rect.h <= 0.0 || container.children.is_empty() {
        return;
    }
    let synthetic = WidgetNode {
        id: "__dg_tooltip_region".to_string(),
        key: None,
        class_name: None,
        css_types: Vec::new(),
        kind: WidgetKind::VLayout,
        props: Default::default(),
        style: container.style.clone(),
        style_json: Default::default(),
        default_style: Default::default(),
        inline_style: Default::default(),
        children: container.children.clone(),
    };
    let mut sub = compute_layout(&synthetic, rect.w, rect.h, sf, theme, Some(state));
    undo_scroll_offsets(&synthetic, &mut sub);
    for (id, child_rect) in sub.rects {
        if id == "__dg_tooltip_region" {
            continue;
        }
        result.rects.insert(
            id,
            Rect {
                x: child_rect.x + rect.x,
                y: child_rect.y + rect.y,
                w: child_rect.w,
                h: child_rect.h,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Shared fixtures and geometry assertions
    // -----------------------------------------------------------------------
    use crate::document::NodeProps;

    fn node(id: &str, kind: WidgetKind, props: NodeProps, children: Vec<WidgetNode>) -> WidgetNode {
        WidgetNode {
            id: id.to_string(),
            key: None,
            class_name: None,
            css_types: Vec::new(),
            kind,
            props,
            style_json: Default::default(),
            default_style: Default::default(),
            inline_style: Default::default(),
            style: Default::default(),
            children,
        }
    }

    fn env_usize(name: &str, default: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(default)
    }

    // -----------------------------------------------------------------------
    // Root, resize, responsive chrome, and generated invariance contracts
    // -----------------------------------------------------------------------

    #[test]
    fn invariant_layout_fallback_catalog_matches_taffy_widget_defaults() {
        let theme = Theme::dark();
        let cases = [
            (
                WidgetKind::ScrollArea,
                Display::Flex,
                FlexDirection::Column,
                FlexWrap::NoWrap,
                1.0,
                1.0,
            ),
            (
                WidgetKind::GridLayout,
                Display::Grid,
                FlexDirection::Row,
                FlexWrap::NoWrap,
                1.0,
                1.0,
            ),
            (
                WidgetKind::FlowLayout,
                Display::Flex,
                FlexDirection::Row,
                FlexWrap::Wrap,
                0.0,
                1.0,
            ),
            (
                WidgetKind::StatusBar,
                Display::Flex,
                FlexDirection::Row,
                FlexWrap::NoWrap,
                0.0,
                0.0,
            ),
            (
                WidgetKind::MenuBar,
                Display::Flex,
                FlexDirection::Row,
                FlexWrap::NoWrap,
                0.0,
                0.0,
            ),
            (
                WidgetKind::Panel,
                Display::Flex,
                FlexDirection::Column,
                FlexWrap::NoWrap,
                0.0,
                1.0,
            ),
        ];

        for (kind, display, direction, wrap, grow, shrink) in cases {
            let widget = node("widget", kind, NodeProps::default(), Vec::new());
            let style = style_for(
                &widget, 1.0, &theme, None, None, None, None, false, false, false, None,
            );
            assert_eq!(style.display, display, "{kind:?} display");
            assert_eq!(style.flex_direction, direction, "{kind:?} direction");
            assert_eq!(style.flex_wrap, wrap, "{kind:?} wrap");
            assert_eq!(style.flex_grow, grow, "{kind:?} grow");
            assert_eq!(style.flex_shrink, shrink, "{kind:?} shrink");
        }
    }

    #[test]
    fn long_client_window_title_shrinks_before_fixed_controls_at_all_dpi_scales() {
        let mut title = node(
            "window--dg-window-title",
            WidgetKind::Label,
            NodeProps {
                text: Some("THEME FORGE - DragonGUI theming and CSS stress console".to_string()),
                wrap: Some(false),
                ..NodeProps::default()
            },
            vec![],
        );
        title.style.layout.width = Some(0.0);
        title.style.layout.flex_grow = Some(1.0);
        title.style.layout.flex_shrink = Some(1.0);
        title.style.layout.min_width = Some(0.0);
        title.style.layout.height = Some(34.0);
        title.style.layout.padding_left = Some(12.0);
        title.style.layout.padding_right = Some(8.0);
        title.style.layout.overflow = Some(OverflowStyle::Hidden);
        title.style.text.text_overflow = Some(TextOverflow::Ellipsis);

        let controls = [
            ("window--dg-window-minimize", "—"),
            ("window--dg-window-maximize", "□"),
            ("window--dg-window-close", "×"),
        ]
        .into_iter()
        .map(|(id, glyph)| {
            let mut control = node(
                id,
                WidgetKind::Button,
                NodeProps {
                    text: Some(glyph.to_string()),
                    ..NodeProps::default()
                },
                vec![],
            );
            control.style.layout.width = Some(46.0);
            control.style.layout.height = Some(34.0);
            control.style.layout.min_width = Some(0.0);
            control.style.layout.flex_shrink = Some(0.0);
            control
        })
        .collect::<Vec<_>>();

        let mut titlebar_children = vec![title];
        titlebar_children.extend(controls);
        let mut titlebar = node(
            "window--dg-window-titlebar",
            WidgetKind::HLayout,
            NodeProps::default(),
            titlebar_children,
        );
        titlebar.style.layout.height = Some(34.0);
        titlebar.style.layout.min_height = Some(34.0);
        titlebar.style.layout.flex_shrink = Some(0.0);
        titlebar.style.layout.gap = Some(0.0);
        titlebar.style.layout.align_items = Some(AlignItemsStyle::Center);
        titlebar.style.layout.overflow = Some(OverflowStyle::Hidden);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![titlebar],
        );

        for scale_factor in [1.0, 1.5, 2.0] {
            for logical_width in [320.0, 390.0, 640.0] {
                let layout = compute_layout(
                    &root,
                    logical_width * scale_factor,
                    120.0 * scale_factor,
                    scale_factor,
                    &Theme::dark(),
                    None,
                );
                let titlebar = layout.rects["window--dg-window-titlebar"];
                let title = layout.rects["window--dg-window-title"];
                let minimize = layout.rects["window--dg-window-minimize"];
                let maximize = layout.rects["window--dg-window-maximize"];
                let close = layout.rects["window--dg-window-close"];

                if logical_width <= 390.0 {
                    assert!(
                        title.w < 458.0 * scale_factor,
                        "long title did not shrink at {logical_width} logical px / {scale_factor}x"
                    );
                }
                assert!(title.x + title.w <= minimize.x + 0.01);
                assert!(minimize.x + minimize.w <= maximize.x + 0.01);
                assert!(maximize.x + maximize.w <= close.x + 0.01);
                assert!(
                    close.x + close.w <= titlebar.x + titlebar.w + 0.01,
                    "close control escaped at {logical_width} logical px / {scale_factor}x: "
                );
                for id in [
                    "window--dg-window-minimize",
                    "window--dg-window-maximize",
                    "window--dg-window-close",
                ] {
                    let rect = layout.rects[id];
                    let clip = layout.clips[id];
                    assert!(rect.w > 0.0 && rect.h > 0.0, "{id} has empty layout");
                    assert!(clip.w > 0.0 && clip.h > 0.0, "{id} is fully clipped");
                }
            }
        }
    }

    #[test]
    fn machine_readable_native_widget_sizing_contracts_are_exhaustive() {
        let table: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/native_widget_sizing_contracts.json"
        ))
        .expect("valid native sizing contract table");
        let groups = table["contracts"].as_array().expect("contract groups");
        let theme = Theme::dark();
        let by_name: HashMap<String, WidgetKind> = crate::document::ALL_WIDGET_KINDS
            .iter()
            .map(|kind| (format!("{kind:?}"), *kind))
            .collect();
        assert_eq!(
            by_name.len(),
            WidgetKind::Unknown as usize + 1,
            "ALL_WIDGET_KINDS must be updated when WidgetKind gains a variant"
        );
        let mut covered = HashSet::new();

        let expected_dimension = |value: &serde_json::Value| match value {
            serde_json::Value::String(value) if value == "auto" => Dimension::Auto,
            serde_json::Value::Number(value) => {
                Dimension::Length(value.as_f64().expect("dimension number") as f32)
            }
            value => panic!("unsupported sizing-contract dimension {value}"),
        };
        let expected_overflow = |value: &str| match value {
            "visible" => Overflow::Visible,
            "hidden" => Overflow::Hidden,
            "scroll" => Overflow::Scroll,
            value => panic!("unsupported sizing-contract overflow {value}"),
        };

        for group in groups {
            let role = group["role"].as_str().expect("contract role");
            let grow = group["grow"].as_f64().expect("contract grow") as f32;
            let shrink = group["shrink"].as_f64().expect("contract shrink") as f32;
            let min_width = expected_dimension(&group["min_width"]);
            let min_height = expected_dimension(&group["min_height"]);
            let overflow_x = group
                .get("overflow_x")
                .and_then(|value| value.as_str())
                .map(expected_overflow)
                .unwrap_or(Overflow::Visible);
            let overflow_y = group
                .get("overflow_y")
                .and_then(|value| value.as_str())
                .map(expected_overflow)
                .unwrap_or(Overflow::Visible);

            for name in group["widgets"].as_array().expect("contract widgets") {
                let name = name.as_str().expect("widget name");
                let kind = *by_name
                    .get(name)
                    .unwrap_or_else(|| panic!("unknown sizing-contract widget {name}"));
                assert!(
                    covered.insert(kind),
                    "{name} appears in multiple sizing-contract roles"
                );
                let widget = node("widget", kind, NodeProps::default(), Vec::new());
                let style = style_for(
                    &widget,
                    1.0,
                    &theme,
                    Some((800.0, 600.0)),
                    Some(&WidgetKind::Window),
                    Some(FlexDirection::Column),
                    None,
                    false,
                    false,
                    false,
                    None,
                );
                assert_eq!(style.flex_grow, grow, "{name} ({role}) grow");
                assert_eq!(style.flex_shrink, shrink, "{name} ({role}) shrink");
                assert_eq!(style.flex_basis, Dimension::Auto, "{name} ({role}) basis");
                assert_eq!(style.min_size.width, min_width, "{name} ({role}) min width");
                assert_eq!(
                    style.min_size.height, min_height,
                    "{name} ({role}) min height"
                );
                assert_eq!(style.overflow.x, overflow_x, "{name} ({role}) overflow x");
                assert_eq!(style.overflow.y, overflow_y, "{name} ({role}) overflow y");
            }
        }

        assert_eq!(
            covered.len(),
            by_name.len(),
            "every shipping WidgetKind needs exactly one native sizing contract"
        );
    }

    #[test]
    fn stable_geometry_fallback_catalog_matches_taffy_widget_defaults() {
        let theme = Theme::dark();
        let image = node("image", WidgetKind::Image, NodeProps::default(), vec![]);
        let image_style = style_for(
            &image, 1.0, &theme, None, None, None, None, false, false, false, None,
        );
        assert_eq!(
            stable_widget_geometry_fallback(&image).min_width,
            Some(48.0)
        );
        assert_eq!(image_style.min_size.width, Dimension::Length(48.0));
        assert_eq!(image_style.min_size.height, Dimension::Length(48.0));

        let report = node(
            "report",
            WidgetKind::HtmlReport,
            NodeProps::default(),
            vec![],
        );
        let report_style = style_for(
            &report, 1.0, &theme, None, None, None, None, false, false, false, None,
        );
        assert_eq!(report_style.size.height, Dimension::Length(360.0));
        assert_eq!(report_style.min_size.width, Dimension::Length(240.0));
        assert_eq!(report_style.min_size.height, Dimension::Length(160.0));

        let fixed_report = node(
            "fixed-report",
            WidgetKind::HtmlReport,
            NodeProps {
                fixed_height: Some(420.0),
                ..Default::default()
            },
            vec![],
        );
        assert_eq!(stable_widget_geometry_fallback(&fixed_report).height, None);
        let fixed_report_style = style_for(
            &fixed_report,
            1.0,
            &theme,
            None,
            None,
            None,
            None,
            false,
            false,
            false,
            None,
        );
        assert_eq!(fixed_report_style.size.height, Dimension::Length(420.0));

        let extension = node(
            "extension",
            WidgetKind::Extension,
            NodeProps::default(),
            vec![],
        );
        let extension_style = style_for(
            &extension, 1.0, &theme, None, None, None, None, false, false, false, None,
        );
        assert_eq!(extension_style.size.height, Dimension::Length(80.0));
        assert_eq!(extension_style.min_size.width, Dimension::Length(0.0));
        assert_eq!(extension_style.min_size.height, Dimension::Length(0.0));
    }

    #[test]
    fn contextual_control_geometry_catalog_matches_taffy_defaults() {
        let mut theme = Theme::dark();
        theme.font_size = 15.0;
        theme.spacing = 10.0;
        let control_height = 37.0;
        let style = |widget: &WidgetNode| {
            style_for(
                widget, 1.0, &theme, None, None, None, None, false, false, false, None,
            )
        };

        let button = node("button", WidgetKind::Button, NodeProps::default(), vec![]);
        assert_eq!(
            resolved_widget_geometry_fallback(&button, &button.style, &theme).height,
            Some(control_height)
        );
        assert_eq!(
            style(&button).size.height,
            Dimension::Length(control_height)
        );

        let icon = node("icon", WidgetKind::IconButton, NodeProps::default(), vec![]);
        assert_eq!(style(&icon).size.width, Dimension::Length(control_height));
        assert_eq!(style(&icon).size.height, Dimension::Length(control_height));

        let badge = node("badge", WidgetKind::Badge, NodeProps::default(), vec![]);
        assert_eq!(style(&badge).size.height, Dimension::Length(23.0));

        let led = node(
            "led",
            WidgetKind::Led,
            NodeProps {
                led_size: Some(22.0),
                ..Default::default()
            },
            vec![],
        );
        assert_eq!(style(&led).size.width, Dimension::Length(22.0));
        assert_eq!(style(&led).size.height, Dimension::Length(22.0));

        let text_area = node(
            "text-area",
            WidgetKind::TextArea,
            NodeProps {
                rows: Some(3),
                ..Default::default()
            },
            vec![],
        );
        assert_eq!(style(&text_area).size.height, Dimension::Length(83.0));

        let no_wrap_label = node(
            "label",
            WidgetKind::Label,
            NodeProps {
                wrap: Some(false),
                ..Default::default()
            },
            vec![],
        );
        assert_eq!(
            style(&no_wrap_label).size.height,
            Dimension::Length(control_height)
        );
        let wrapping_label = node(
            "wrapping-label",
            WidgetKind::Label,
            NodeProps::default(),
            vec![],
        );
        assert_eq!(style(&wrapping_label).size.height, Dimension::Auto);
    }

    #[test]
    fn node_conditional_layout_fallback_catalog_matches_taffy_widget_defaults() {
        let theme = Theme::dark();
        let assert_matches = |widget: &WidgetNode,
                              expected_direction: Option<FlexDirectionStyle>,
                              expected_grow: f32,
                              expected_shrink: f32| {
            let fallback = resolved_widget_layout_fallback(
                widget,
                &widget.style,
                NativeLayoutFallbackContext::default(),
                None,
            );
            let style = style_for(
                widget, 1.0, &theme, None, None, None, None, false, false, false, None,
            );
            assert_eq!(
                fallback.flex_direction, expected_direction,
                "{:?} fallback direction",
                widget.kind
            );
            assert_eq!(
                fallback.flex_grow,
                Some(expected_grow),
                "{:?} fallback grow",
                widget.kind
            );
            assert_eq!(
                fallback.flex_shrink,
                Some(expected_shrink),
                "{:?} fallback shrink",
                widget.kind
            );
            assert_eq!(
                style.flex_grow, expected_grow,
                "{:?} Taffy grow",
                widget.kind
            );
            assert_eq!(
                style.flex_shrink, expected_shrink,
                "{:?} Taffy shrink",
                widget.kind
            );
            if let Some(direction) = expected_direction {
                let expected = match direction {
                    FlexDirectionStyle::Row => FlexDirection::Row,
                    FlexDirectionStyle::Column => FlexDirection::Column,
                    FlexDirectionStyle::RowReverse => FlexDirection::RowReverse,
                    FlexDirectionStyle::ColumnReverse => FlexDirection::ColumnReverse,
                };
                assert_eq!(style.flex_direction, expected);
            }
        };

        let fixed_splitter = node(
            "splitter",
            WidgetKind::Splitter,
            NodeProps {
                orientation: Some("vertical".to_string()),
                fixed_width: Some(280.0),
                ..Default::default()
            },
            vec![],
        );
        assert_matches(&fixed_splitter, Some(FlexDirectionStyle::Column), 0.0, 0.0);

        let fixed_panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                fixed_width: Some(240.0),
                ..Default::default()
            },
            vec![],
        );
        assert_matches(&fixed_panel, Some(FlexDirectionStyle::Column), 0.0, 0.0);

        let flexible_image = node(
            "flex-image",
            WidgetKind::Image,
            NodeProps::default(),
            vec![],
        );
        assert_matches(&flexible_image, None, 1.0, 1.0);
        let fixed_image = node(
            "fixed-image",
            WidgetKind::Image,
            NodeProps {
                fixed_height: Some(96.0),
                ..Default::default()
            },
            vec![],
        );
        assert_matches(&fixed_image, None, 0.0, 0.0);

        let flexible_spacer = node(
            "flex-spacer",
            WidgetKind::Spacer,
            NodeProps::default(),
            vec![],
        );
        assert_matches(&flexible_spacer, None, 1.0, 1.0);
        let fixed_spacer = node(
            "fixed-spacer",
            WidgetKind::Spacer,
            NodeProps {
                fixed_width: Some(12.0),
                ..Default::default()
            },
            vec![],
        );
        assert_matches(&fixed_spacer, None, 0.0, 0.0);

        let content_tabs = node(
            "tabs",
            WidgetKind::Tabs,
            NodeProps::default(),
            vec![node(
                "tab",
                WidgetKind::Tab,
                NodeProps::default(),
                vec![node(
                    "content",
                    WidgetKind::Label,
                    NodeProps::default(),
                    vec![],
                )],
            )],
        );
        assert_matches(&content_tabs, None, 1.0, 1.0);
    }

    #[test]
    fn parent_context_layout_fallbacks_match_taffy_main_axis_behavior() {
        let theme = Theme::dark();
        let layout = node("layout", WidgetKind::HLayout, NodeProps::default(), vec![]);
        let window_context = NativeLayoutFallbackContext {
            parent_kind: Some(WidgetKind::Window),
            parent_flex_direction: Some(FlexDirectionStyle::Column),
            parent_preserves_preferred_main_size: false,
        };
        let panel_context = NativeLayoutFallbackContext {
            parent_kind: Some(WidgetKind::Panel),
            ..window_context
        };
        assert_eq!(
            resolved_widget_layout_fallback(&layout, &layout.style, window_context, None)
                .flex_shrink,
            Some(1.0)
        );
        assert_eq!(
            resolved_widget_layout_fallback(&layout, &layout.style, panel_context, None)
                .flex_shrink,
            Some(0.0)
        );
        let window_child_style = style_for(
            &layout,
            1.0,
            &theme,
            None,
            Some(&WidgetKind::Window),
            Some(FlexDirection::Column),
            None,
            false,
            false,
            false,
            None,
        );
        let panel_child_style = style_for(
            &layout,
            1.0,
            &theme,
            None,
            Some(&WidgetKind::Panel),
            Some(FlexDirection::Column),
            None,
            false,
            false,
            false,
            None,
        );
        assert_eq!(window_child_style.flex_shrink, 1.0);
        assert_eq!(panel_child_style.flex_shrink, 0.0);

        let mut sized_panel = node("sized", WidgetKind::Panel, NodeProps::default(), vec![]);
        sized_panel.style.layout.width = Some(200.0);
        let row_context = NativeLayoutFallbackContext {
            parent_kind: Some(WidgetKind::HLayout),
            parent_flex_direction: Some(FlexDirectionStyle::Row),
            parent_preserves_preferred_main_size: false,
        };
        let column_context = NativeLayoutFallbackContext {
            parent_kind: Some(WidgetKind::VLayout),
            parent_flex_direction: Some(FlexDirectionStyle::Column),
            parent_preserves_preferred_main_size: false,
        };
        assert_eq!(
            resolved_widget_layout_fallback(&sized_panel, &sized_panel.style, row_context, None)
                .flex_grow,
            Some(0.0)
        );
        assert_eq!(
            resolved_widget_layout_fallback(
                &sized_panel,
                &sized_panel.style,
                column_context,
                None,
            )
            .flex_grow,
            Some(0.0)
        );
        let row_child_style = style_for(
            &sized_panel,
            1.0,
            &theme,
            None,
            Some(&WidgetKind::HLayout),
            Some(FlexDirection::Row),
            None,
            false,
            false,
            false,
            None,
        );
        let column_child_style = style_for(
            &sized_panel,
            1.0,
            &theme,
            None,
            Some(&WidgetKind::VLayout),
            Some(FlexDirection::Column),
            None,
            false,
            false,
            false,
            None,
        );
        assert_eq!(row_child_style.flex_grow, 0.0);
        assert_eq!(column_child_style.flex_grow, 0.0);

        let preserving_row_context = NativeLayoutFallbackContext {
            parent_preserves_preferred_main_size: true,
            ..row_context
        };
        assert_eq!(
            resolved_widget_layout_fallback(
                &sized_panel,
                &sized_panel.style,
                preserving_row_context,
                None,
            )
            .flex_shrink,
            Some(0.0)
        );
    }

    #[test]
    fn live_pane_fallback_tracks_fixed_and_fractional_state() {
        let theme = Theme::dark();
        let pane = node(
            "pane",
            WidgetKind::Pane,
            NodeProps {
                orientation: Some("horizontal".to_string()),
                pane_flex: Some(2.0),
                ..Default::default()
            },
            vec![],
        );
        let context = NativeLayoutFallbackContext {
            parent_kind: Some(WidgetKind::Splitter),
            parent_flex_direction: Some(FlexDirectionStyle::Row),
            parent_preserves_preferred_main_size: false,
        };
        let fallback = resolved_widget_layout_fallback(&pane, &pane.style, context, None);
        assert_eq!(fallback.flex_grow, Some(2.0));
        let initial_style = style_for(
            &pane,
            1.0,
            &theme,
            None,
            Some(&WidgetKind::Splitter),
            Some(FlexDirection::Row),
            None,
            false,
            false,
            false,
            None,
        );
        assert_eq!(initial_style.flex_grow, 2.0);

        let mut state = WidgetState::from_tree(&pane);
        assert_eq!(state.set_pane_size("pane", Some(240.0)), Some(Some(240.0)));
        let fixed_fallback =
            resolved_widget_layout_fallback(&pane, &pane.style, context, state.pane_size("pane"));
        assert_eq!(fixed_fallback.flex_grow, Some(0.0));
        let fixed_style = style_for(
            &pane,
            1.0,
            &theme,
            None,
            Some(&WidgetKind::Splitter),
            Some(FlexDirection::Row),
            None,
            false,
            false,
            false,
            Some(&state),
        );
        assert_eq!(fixed_style.flex_grow, 0.0);

        assert_eq!(state.set_pane_size("pane", Some(0.35)), Some(Some(0.35)));
        let fractional_fallback =
            resolved_widget_layout_fallback(&pane, &pane.style, context, state.pane_size("pane"));
        assert_eq!(fractional_fallback.flex_grow, Some(0.35));
        let fractional_style = style_for(
            &pane,
            1.0,
            &theme,
            None,
            Some(&WidgetKind::Splitter),
            Some(FlexDirection::Row),
            None,
            false,
            false,
            false,
            Some(&state),
        );
        assert_eq!(fractional_style.flex_grow, 0.35);
    }

    #[test]
    fn pre_scroll_clips_do_not_publish_paint_state() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node(
                    "scroller",
                    WidgetKind::ScrollArea,
                    NodeProps::default(),
                    vec![],
                ),
                node("unrelated", WidgetKind::Label, NodeProps::default(), vec![]),
            ],
        );
        let mut result = LayoutResult::default();
        let root_rect = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 80.0,
        };
        result.rects.insert("window".to_string(), root_rect);
        result.rects.insert(
            "scroller".to_string(),
            Rect {
                x: 10.0,
                y: 12.0,
                w: 40.0,
                h: 20.0,
            },
        );
        result.rects.insert(
            "unrelated".to_string(),
            Rect {
                x: 10.0,
                y: 40.0,
                w: 40.0,
                h: 20.0,
            },
        );
        result.paint_clips.insert("stale".to_string(), root_rect);

        compute_pre_scroll_clips(&root, &mut result, 1.0, &Theme::dark());

        assert!(result.clips.contains_key("window"));
        assert!(result.clips.contains_key("scroller"));
        assert!(
            !result.clips.contains_key("unrelated"),
            "pre-scroll geometry should skip branches without scroll owners"
        );
        assert!(
            result.paint_clips.is_empty(),
            "pre-scroll geometry must not expose stale or provisional paint clips"
        );

        compute_clips(&root, &mut result, 1.0, &Theme::dark());

        assert!(result.paint_clips.contains_key("window"));
        assert!(result.paint_clips.contains_key("scroller"));
        assert!(result.paint_clips.contains_key("unrelated"));
        assert!(!result.paint_clips.contains_key("stale"));
    }

    #[test]
    fn oversized_tooltip_is_bounded_by_root_viewport() {
        let root = Rect {
            x: 10.0,
            y: 20.0,
            w: 180.0,
            h: 120.0,
        };
        let target = Rect {
            x: 150.0,
            y: 110.0,
            w: 30.0,
            h: 20.0,
        };
        let rect = place_tooltip_rect(target, root, 420.0, 300.0, 8.0);

        assert_eq!(rect.x, 18.0);
        assert_eq!(rect.y, 28.0);
        assert_eq!(rect.w, 164.0);
        assert_eq!(rect.h, 104.0);
        assert!(rect.x + rect.w <= root.x + root.w - 8.0);
        assert!(rect.y + rect.h <= root.y + root.h - 8.0);
    }

    #[test]
    fn oversized_modal_shrinks_inside_tiny_root_viewport() {
        let modal = node(
            "modal",
            WidgetKind::Modal,
            NodeProps {
                open: Some(true),
                fixed_width: Some(420.0),
                fixed_height: Some(260.0),
                ..NodeProps::default()
            },
            vec![node(
                "modal-label",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Tiny viewport".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![modal],
        );

        let layout = compute_layout(&root, 72.0, 54.0, 1.0, &Theme::dark(), None);
        let window = layout.rects["window"];
        let modal = layout.rects["modal"];

        assert_eq!(
            [window.x, window.y, window.w, window.h],
            [0.0, 0.0, 72.0, 54.0]
        );
        assert!(modal.w > 0.0 && modal.h > 0.0);
        assert!(modal.x >= window.x && modal.y >= window.y);
        assert!(modal.x + modal.w <= window.x + window.w);
        assert!(modal.y + modal.h <= window.y + window.h);
    }

    #[test]
    fn alternating_viewport_sizes_are_deterministic_and_reset_scroll_geometry() {
        let mut body = node(
            "body",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            (0..4)
                .map(|index| {
                    node(
                        &format!("row-{index}"),
                        WidgetKind::Panel,
                        NodeProps {
                            fixed_height: Some(90.0),
                            ..NodeProps::default()
                        },
                        vec![],
                    )
                })
                .collect(),
        );
        body.style.layout.gap = Some(10.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node("menu", WidgetKind::MenuBar, NodeProps::default(), vec![]),
                body,
                node(
                    "status",
                    WidgetKind::StatusBar,
                    NodeProps::default(),
                    vec![],
                ),
            ],
        );
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("body".to_string(), 999.0);
        let theme = Theme::dark();

        let small_first = compute_layout(&root, 320.0, 180.0, 1.0, &theme, Some(&state));
        let large = compute_layout(&root, 640.0, 520.0, 1.0, &theme, Some(&state));
        let small_second = compute_layout(&root, 320.0, 180.0, 1.0, &theme, Some(&state));

        for (layout, expected_size) in [
            (&small_first, [320.0, 180.0]),
            (&large, [640.0, 520.0]),
            (&small_second, [320.0, 180.0]),
        ] {
            let window = layout.rects["window"];
            assert_eq!([window.w, window.h], expected_size);
            for rect in layout
                .rects
                .values()
                .chain(layout.clips.values())
                .chain(layout.paint_clips.values())
            {
                assert!(
                    rect.x.is_finite()
                        && rect.y.is_finite()
                        && rect.w.is_finite()
                        && rect.h.is_finite()
                        && rect.w >= 0.0
                        && rect.h >= 0.0,
                    "layout geometry must stay finite and nonnegative: {rect:?}"
                );
            }
        }

        let small_max = small_first.scroll_max_y.get("body").copied().unwrap_or(0.0);
        assert!(small_max > 0.0);
        assert_eq!(small_first.scroll_y.get("body").copied(), Some(small_max));
        assert_eq!(large.scroll_max_y.get("body").copied(), Some(0.0));
        assert_eq!(large.scroll_y.get("body").copied(), Some(0.0));

        assert_eq!(small_first.rects.len(), small_second.rects.len());
        for (id, first) in &small_first.rects {
            let second = small_second.rects.get(id).expect("repeat rect");
            assert!(
                (first.x - second.x).abs() <= 0.001
                    && (first.y - second.y).abs() <= 0.001
                    && (first.w - second.w).abs() <= 0.001
                    && (first.h - second.h).abs() <= 0.001,
                "repeated small solve changed {id}: first={first:?} second={second:?}"
            );
        }
        assert_eq!(small_first.scroll_x, small_second.scroll_x);
        assert_eq!(small_first.scroll_y, small_second.scroll_y);
        assert_eq!(small_first.scroll_max_x, small_second.scroll_max_x);
        assert_eq!(small_first.scroll_max_y, small_second.scroll_max_y);
    }

    #[test]
    fn raw_vlayout_root_generated_resize_matrix_is_bounded_and_deterministic() {
        let mut body = node(
            "body",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            vec![node(
                "oversized-content",
                WidgetKind::Panel,
                NodeProps {
                    fixed_width: Some(420.0),
                    fixed_height: Some(300.0),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        body.style.layout.overflow_x = Some(OverflowStyle::Auto);
        let column = node(
            "column",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![
                node(
                    "header",
                    WidgetKind::Panel,
                    NodeProps {
                        fixed_height: Some(30.0),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                body,
                node(
                    "footer",
                    WidgetKind::Panel,
                    NodeProps {
                        fixed_height: Some(26.0),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
            ],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![column],
        );
        let mut state = WidgetState::default();
        state.container_scroll_x.insert("body".to_string(), 999.0);
        state.container_scroll_y.insert("body".to_string(), 999.0);
        let theme = Theme::dark();
        let sizes = [
            (96.0, 80.0),
            (180.0, 120.0),
            (640.0, 520.0),
            (240.0, 160.0),
            (96.0, 80.0),
        ];
        let layouts = sizes
            .iter()
            .map(|(width, height)| {
                compute_layout(&root, *width, *height, 1.0, &theme, Some(&state))
            })
            .collect::<Vec<_>>();

        for (layout, (width, height)) in layouts.iter().zip(sizes) {
            let window = layout.rects["window"];
            let column = layout.rects["column"];
            let header = layout.rects["header"];
            let body = layout.rects["body"];
            let footer = layout.rects["footer"];
            assert_eq!([window.w, window.h], [width, height]);
            assert_eq!(
                [column.x, column.y, column.w, column.h],
                [window.x, window.y, window.w, window.h]
            );
            assert!(body.h > 0.0);
            assert!(body.y + 0.5 >= header.y + header.h);
            assert!(footer.y + 0.5 >= body.y + body.h);
            assert!(footer.y + footer.h <= window.y + window.h + 0.5);
            for rect in layout
                .rects
                .values()
                .chain(layout.clips.values())
                .chain(layout.paint_clips.values())
            {
                assert!(
                    rect.x.is_finite()
                        && rect.y.is_finite()
                        && rect.w.is_finite()
                        && rect.h.is_finite()
                        && rect.w >= 0.0
                        && rect.h >= 0.0,
                    "generated resize geometry must stay finite and nonnegative: {rect:?}"
                );
            }
        }

        let tiny_first = &layouts[0];
        let large = &layouts[2];
        let tiny_second = &layouts[4];
        assert!(tiny_first.scroll_max_x["body"] > 0.0);
        assert!(tiny_first.scroll_max_y["body"] > 0.0);
        assert_eq!(tiny_first.scroll_x["body"], tiny_first.scroll_max_x["body"]);
        assert_eq!(tiny_first.scroll_y["body"], tiny_first.scroll_max_y["body"]);
        assert_eq!(large.scroll_max_x["body"], 0.0);
        assert_eq!(large.scroll_max_y["body"], 0.0);
        assert_eq!(large.scroll_x["body"], 0.0);
        assert_eq!(large.scroll_y["body"], 0.0);
        for (first_map, second_map) in [
            (&tiny_first.rects, &tiny_second.rects),
            (&tiny_first.clips, &tiny_second.clips),
            (&tiny_first.paint_clips, &tiny_second.paint_clips),
        ] {
            assert_eq!(first_map.len(), second_map.len());
            for (id, first) in first_map {
                let second = second_map.get(id).expect("repeated geometry");
                assert!(
                    (first.x - second.x).abs() <= 0.001
                        && (first.y - second.y).abs() <= 0.001
                        && (first.w - second.w).abs() <= 0.001
                        && (first.h - second.h).abs() <= 0.001,
                    "repeated generated solve changed {id}: first={first:?} second={second:?}"
                );
            }
        }
        assert_eq!(tiny_first.scroll_x, tiny_second.scroll_x);
        assert_eq!(tiny_first.scroll_y, tiny_second.scroll_y);
        assert_eq!(tiny_first.scroll_max_x, tiny_second.scroll_max_x);
        assert_eq!(tiny_first.scroll_max_y, tiny_second.scroll_max_y);
    }

    #[test]
    fn generated_flex_matrix_preserves_geometry_and_overflow_invariants() {
        let theme = Theme::dark();
        for direction in [FlexDirectionStyle::Row, FlexDirectionStyle::Column] {
            for child_count in [1usize, 3] {
                for gap in [0.0, 7.0] {
                    for padding in [0.0, 9.0] {
                        for (window_w, window_h) in [(72.0, 54.0), (240.0, 160.0)] {
                            for overflow in [OverflowStyle::Hidden, OverflowStyle::Auto] {
                                for scale_factor in [1.0, 1.5] {
                                    let horizontal = direction == FlexDirectionStyle::Row;
                                    let mut children = Vec::with_capacity(child_count);
                                    for index in 0..child_count {
                                        let mut child = node(
                                            &format!("child-{index}"),
                                            WidgetKind::Panel,
                                            NodeProps::default(),
                                            vec![],
                                        );
                                        if horizontal {
                                            child.style.layout.width = Some(90.0);
                                            child.style.layout.width_value =
                                                Some(LayoutLength::LogicalPx(90.0));
                                            child.style.layout.min_width = Some(24.0);
                                            child.style.layout.min_width_value =
                                                Some(LayoutLength::LogicalPx(24.0));
                                        } else {
                                            child.style.layout.height = Some(70.0);
                                            child.style.layout.height_value =
                                                Some(LayoutLength::LogicalPx(70.0));
                                            child.style.layout.min_height = Some(20.0);
                                            child.style.layout.min_height_value =
                                                Some(LayoutLength::LogicalPx(20.0));
                                        }
                                        child.style.layout.flex_grow = Some(0.0);
                                        child.style.layout.flex_shrink =
                                            Some(if overflow == OverflowStyle::Auto {
                                                0.0
                                            } else {
                                                1.0
                                            });
                                        children.push(child);
                                    }

                                    let mut flex = node(
                                        "flex",
                                        if horizontal {
                                            WidgetKind::HLayout
                                        } else {
                                            WidgetKind::VLayout
                                        },
                                        NodeProps::default(),
                                        children,
                                    );
                                    flex.style.layout.flex_direction = Some(direction);
                                    flex.style.layout.gap = Some(gap);
                                    flex.style.layout.padding = Some(padding);
                                    if horizontal {
                                        flex.style.layout.overflow_x = Some(overflow);
                                        flex.style.layout.overflow_y = Some(OverflowStyle::Hidden);
                                    } else {
                                        flex.style.layout.overflow_x = Some(OverflowStyle::Hidden);
                                        flex.style.layout.overflow_y = Some(overflow);
                                    }
                                    let root = node(
                                        "window",
                                        WidgetKind::Window,
                                        NodeProps::default(),
                                        vec![flex],
                                    );
                                    let mut state = WidgetState::default();
                                    state
                                        .container_scroll_x
                                        .insert("flex".to_string(), 10_000.0);
                                    state
                                        .container_scroll_y
                                        .insert("flex".to_string(), 10_000.0);
                                    let first = compute_layout(
                                        &root,
                                        window_w,
                                        window_h,
                                        scale_factor,
                                        &theme,
                                        Some(&state),
                                    );
                                    let repeated = compute_layout(
                                        &root,
                                        window_w,
                                        window_h,
                                        scale_factor,
                                        &theme,
                                        Some(&state),
                                    );
                                    let window = first.rects["window"];
                                    let flex_rect = first.rects["flex"];
                                    assert_eq!([window.w, window.h], [window_w, window_h]);
                                    assert_eq!(
                                        [flex_rect.x, flex_rect.y, flex_rect.w, flex_rect.h],
                                        [window.x, window.y, window.w, window.h]
                                    );
                                    for rect in first
                                        .rects
                                        .values()
                                        .chain(first.clips.values())
                                        .chain(first.paint_clips.values())
                                    {
                                        assert!(
                                            rect.x.is_finite()
                                                && rect.y.is_finite()
                                                && rect.w.is_finite()
                                                && rect.h.is_finite()
                                                && rect.w >= 0.0
                                                && rect.h >= 0.0,
                                            "generated flex geometry must stay finite and nonnegative: direction={direction:?} children={child_count} gap={gap} padding={padding} viewport={window_w}x{window_h} scale={scale_factor} overflow={overflow:?} rect={rect:?}"
                                        );
                                    }

                                    let mut previous_end = f32::NEG_INFINITY;
                                    for index in 0..child_count {
                                        let id = format!("child-{index}");
                                        let rect = first.rects[&id];
                                        let start = if horizontal { rect.x } else { rect.y };
                                        let end = if horizontal {
                                            rect.x + rect.w
                                        } else {
                                            rect.y + rect.h
                                        };
                                        assert!(
                                            start + 0.5 >= previous_end,
                                            "normal flex siblings overlap: direction={direction:?} previous_end={previous_end} child={rect:?}"
                                        );
                                        previous_end = end;
                                        let clip = first.clips[&id];
                                        assert!(
                                            clip.x + 0.5 >= flex_rect.x
                                                && clip.y + 0.5 >= flex_rect.y
                                                && clip.x + clip.w
                                                    <= flex_rect.x + flex_rect.w + 0.5
                                                && clip.y + clip.h
                                                    <= flex_rect.y + flex_rect.h + 0.5,
                                            "non-visible overflow clip escaped owner: flex={flex_rect:?} child={rect:?} clip={clip:?}"
                                        );
                                    }

                                    let max_scroll = if horizontal {
                                        first.scroll_max_x.get("flex").copied().unwrap_or(0.0)
                                    } else {
                                        first.scroll_max_y.get("flex").copied().unwrap_or(0.0)
                                    };
                                    assert!(max_scroll.is_finite() && max_scroll >= 0.0);
                                    if overflow == OverflowStyle::Hidden {
                                        assert_eq!(max_scroll, 0.0);
                                    } else if max_scroll > 0.0 {
                                        let last = first.rects
                                            [&format!("child-{}", child_count.saturating_sub(1))];
                                        let content_end = if horizontal {
                                            last.x + last.w
                                        } else {
                                            last.y + last.h
                                        };
                                        let owner_end = if horizontal {
                                            flex_rect.x + flex_rect.w
                                        } else {
                                            flex_rect.y + flex_rect.h
                                        };
                                        assert!(
                                            content_end <= owner_end + 0.5,
                                            "maximum scroll must reveal the content end: direction={direction:?} max={max_scroll} flex={flex_rect:?} last={last:?}"
                                        );
                                    }

                                    assert_eq!(first.rects.len(), repeated.rects.len());
                                    for (id, rect) in &first.rects {
                                        let repeat = repeated.rects.get(id).expect("repeat rect");
                                        assert!(
                                            (rect.x - repeat.x).abs() <= 0.001
                                                && (rect.y - repeat.y).abs() <= 0.001
                                                && (rect.w - repeat.w).abs() <= 0.001
                                                && (rect.h - repeat.h).abs() <= 0.001,
                                            "generated flex solve is not deterministic for {id}: first={rect:?} repeat={repeat:?}"
                                        );
                                    }
                                    assert_eq!(first.scroll_x, repeated.scroll_x);
                                    assert_eq!(first.scroll_y, repeated.scroll_y);
                                    assert_eq!(first.scroll_max_x, repeated.scroll_max_x);
                                    assert_eq!(first.scroll_max_y, repeated.scroll_max_y);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fixed_sidebar_and_pages_share_below_minimum_viewport_width() {
        let sidebar = node(
            "sidebar",
            WidgetKind::Sidebar,
            NodeProps {
                fixed_width: Some(220.0),
                ..NodeProps::default()
            },
            vec![node(
                "nav",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Navigation".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        let page = node(
            "page",
            WidgetKind::Page,
            NodeProps {
                route_value: Some("main".to_string()),
                ..NodeProps::default()
            },
            vec![node(
                "page-button",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Main action".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        let pages = node(
            "pages",
            WidgetKind::Pages,
            NodeProps {
                route_value: Some("main".to_string()),
                ..NodeProps::default()
            },
            vec![page],
        );
        let body = node(
            "body",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, pages],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![body],
        );

        let layout = compute_layout(&root, 180.0, 120.0, 1.0, &Theme::dark(), None);
        let body = layout.rects["body"];
        let sidebar = layout.rects["sidebar"];
        let pages = layout.rects["pages"];

        assert!(sidebar.w < 220.0, "sidebar should yield width: {sidebar:?}");
        assert!(
            pages.w > 0.0,
            "main pages must retain usable width: {pages:?}"
        );
        assert!(pages.x + 0.5 >= sidebar.x + sidebar.w);
        assert!(pages.x + pages.w <= body.x + body.w + 0.5);
    }

    #[test]
    fn app_shell_main_content_minimum_makes_eligible_sidebar_yield_first() {
        let sidebar = node(
            "sidebar",
            WidgetKind::Sidebar,
            NodeProps {
                fixed_width: Some(220.0),
                ..NodeProps::default()
            },
            vec![node(
                "navigation",
                WidgetKind::NavItem,
                NodeProps {
                    text: Some("Navigation".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        let mut body = node(
            "body",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            vec![node(
                "content",
                WidgetKind::Panel,
                NodeProps {
                    text: Some("Reachable main content".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        body.style.layout.flex_grow = Some(1.0);
        body.style.layout.flex_shrink = Some(1.0);
        body.style.layout.min_width = Some(160.0);
        let shell = node(
            "shell",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, body],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![shell],
        );

        let layout = compute_layout(&root, 300.0, 180.0, 1.0, &Theme::dark(), None);
        let sidebar = layout.rects["sidebar"];
        let body = layout.rects["body"];

        assert!(
            body.w >= 159.5,
            "protected main content should retain its safeguard: {body:?}"
        );
        assert!(
            sidebar.w < 220.0,
            "eligible sidebar should yield before main content: {sidebar:?}"
        );
        assert!(body.x + body.w <= 300.5);
    }

    #[test]
    fn fixed_panes_and_hard_minima_yield_to_narrow_splitter() {
        for (orientation, window_w, window_h) in
            [("horizontal", 180.0, 100.0), ("vertical", 100.0, 180.0)]
        {
            let pane = |id: &str| {
                node(
                    id,
                    WidgetKind::Pane,
                    NodeProps {
                        orientation: Some(orientation.to_string()),
                        pane_size: Some(240.0),
                        pane_min_size: Some(240.0),
                        ..NodeProps::default()
                    },
                    vec![node(
                        &format!("{id}-content"),
                        WidgetKind::Panel,
                        NodeProps::default(),
                        vec![],
                    )],
                )
            };
            let splitter = node(
                "splitter",
                WidgetKind::Splitter,
                NodeProps {
                    orientation: Some(orientation.to_string()),
                    gutter_size: Some(6.0),
                    ..NodeProps::default()
                },
                vec![pane("first"), pane("second")],
            );
            let root = node(
                "window",
                WidgetKind::Window,
                NodeProps::default(),
                vec![splitter],
            );

            let layout = compute_layout(&root, window_w, window_h, 1.0, &Theme::dark(), None);
            let splitter = layout.rects["splitter"];
            let first = layout.rects["first"];
            let second = layout.rects["second"];

            if orientation == "horizontal" {
                assert!(first.w > 0.0 && second.w > 0.0);
                assert!(second.x + 0.5 >= first.x + first.w);
                assert!(
                    second.x + second.w <= splitter.x + splitter.w + 0.5,
                    "horizontal pane minima must yield to the splitter viewport: splitter={splitter:?} first={first:?} second={second:?}"
                );
            } else {
                assert!(first.h > 0.0 && second.h > 0.0);
                assert!(second.y + 0.5 >= first.y + first.h);
                assert!(
                    second.y + second.h <= splitter.y + splitter.h + 0.5,
                    "vertical pane minima must yield to the splitter viewport: splitter={splitter:?} first={first:?} second={second:?}"
                );
            }
        }
    }

    #[test]
    fn framework_css_does_not_override_public_splitter_gutter_size() {
        let mut root = crate::document::parse_widget_node(&serde_json::json!({
            "id": "window",
            "type": "window",
            "children": [{
                "id": "splitter",
                "type": "splitter",
                "props": {
                    "orientation": "horizontal",
                    "gutter_size": 12
                },
                "style": {
                    "width": 400,
                    "height": 100
                },
                "children": [{
                    "id": "left",
                    "type": "pane",
                    "props": {"orientation": "horizontal", "flex": 1}
                }, {
                    "id": "right",
                    "type": "pane",
                    "props": {"orientation": "horizontal", "flex": 1}
                }]
            }]
        }))
        .expect("splitter tree");
        let theme = Theme::dark();
        let mut stylesheets = crate::css_style::StylesheetStore::default();
        stylesheets.install_framework_defaults(&theme);
        crate::css_style::apply_stylesheets_to_tree(&mut root, &mut stylesheets);

        let layout = compute_layout(&root, 400.0, 100.0, 1.0, &theme, None);
        let left = layout.rects["left"];
        let right = layout.rects["right"];
        let gutter = right.x - (left.x + left.w);

        assert!(
            (gutter - 12.0).abs() <= 0.1,
            "public gutter_size should own splitter layout spacing: {gutter}"
        );
    }

    #[test]
    fn fixed_menu_and_status_chrome_preserve_body_in_tiny_window() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node(
                    "menu",
                    WidgetKind::MenuBar,
                    NodeProps {
                        fixed_height: Some(36.0),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "body",
                    WidgetKind::VLayout,
                    NodeProps::default(),
                    vec![node(
                        "body-label",
                        WidgetKind::Label,
                        NodeProps {
                            text: Some("Body".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    )],
                ),
                node(
                    "status",
                    WidgetKind::StatusBar,
                    NodeProps {
                        fixed_height: Some(32.0),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
            ],
        );

        let layout = compute_layout(&root, 120.0, 40.0, 1.0, &Theme::dark(), None);
        let window = layout.rects["window"];
        let menu = layout.rects["menu"];
        let body = layout.rects["body"];
        let status = layout.rects["status"];

        assert!(
            body.h > 0.0,
            "fixed chrome must preserve a body slot: {body:?}"
        );
        assert!(body.y + 0.5 >= menu.y + menu.h);
        assert!(status.y + 0.5 >= body.y + body.h);
        assert!(status.y + status.h <= window.y + window.h + 0.5);
    }

    #[test]
    fn wrapped_toolbar_rows_expand_auto_height_without_overlap() {
        let tool = |id: &str| {
            node(
                id,
                WidgetKind::Button,
                NodeProps {
                    fixed_width: Some(60.0),
                    fixed_height: Some(24.0),
                    ..NodeProps::default()
                },
                vec![],
            )
        };
        let mut toolbar = node(
            "toolbar",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![tool("first"), tool("second"), tool("third")],
        );
        toolbar.style.layout.width = Some(130.0);
        toolbar.style.layout.width_value = Some(LayoutLength::LogicalPx(130.0));
        toolbar.style.layout.min_height = Some(30.0);
        toolbar.style.layout.min_height_value = Some(LayoutLength::LogicalPx(30.0));
        toolbar.style.layout.gap = Some(6.0);
        toolbar.style.layout.flex_grow = Some(0.0);
        toolbar.style.layout.flex_wrap = Some(FlexWrapStyle::Wrap);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![toolbar],
        );
        let layout = compute_layout(&root, 180.0, 120.0, 1.0, &Theme::dark(), None);
        let toolbar = layout.rects["toolbar"];
        let first = layout.rects["first"];
        let second = layout.rects["second"];
        let third = layout.rects["third"];

        assert_eq!(first.y, second.y);
        assert!(third.y >= first.y + first.h + 5.5);
        assert!(
            toolbar.h + 0.5 >= third.y + third.h - toolbar.y,
            "wrapped toolbar must grow around its final row: toolbar={toolbar:?} third={third:?}"
        );
        assert!(third.x + third.w <= toolbar.x + toolbar.w + 0.5);
    }

    // -----------------------------------------------------------------------
    // Visibility, navigation, and conditional subtree contracts
    // -----------------------------------------------------------------------

    #[test]
    fn inactive_tab_content_is_removed_from_layout() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "tabs",
                WidgetKind::Tabs,
                NodeProps {
                    route_value: Some("objects".to_string()),
                    ..NodeProps::default()
                },
                vec![
                    node(
                        "editor-tab",
                        WidgetKind::Tab,
                        NodeProps {
                            text: Some("Editor".to_string()),
                            route_value: Some("editor".to_string()),
                            ..NodeProps::default()
                        },
                        vec![node(
                            "editor-report",
                            WidgetKind::HtmlReport,
                            NodeProps::default(),
                            vec![],
                        )],
                    ),
                    node(
                        "objects-tab",
                        WidgetKind::Tab,
                        NodeProps {
                            text: Some("Objects".to_string()),
                            route_value: Some("objects".to_string()),
                            ..NodeProps::default()
                        },
                        vec![node(
                            "objects-panel",
                            WidgetKind::Panel,
                            NodeProps::default(),
                            vec![node(
                                "objects-button",
                                WidgetKind::Button,
                                NodeProps::default(),
                                vec![],
                            )],
                        )],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 800.0, 480.0, 1.0, &Theme::dark(), None);

        assert!(layout.rects.contains_key("editor-tab"));
        assert!(layout.rects.contains_key("objects-tab"));
        assert!(!layout.rects.contains_key("editor-report"));
        assert!(layout.rects.contains_key("objects-panel"));
        assert!(layout.rects.contains_key("objects-button"));
        let panel_clip = layout.clips.get("objects-panel").copied().unwrap();
        let button_clip = layout.clips.get("objects-button").copied().unwrap();
        assert!(
            panel_clip.h > 0.0,
            "active tab body panel should not be clipped by the tab header"
        );
        assert!(
            button_clip.h > 0.0,
            "active tab body control should not be clipped by the tab header"
        );
    }

    #[test]
    fn empty_tabs_strip_does_not_consume_workbench_body_space() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "workbench",
                WidgetKind::VLayout,
                NodeProps::default(),
                vec![
                    node(
                        "tabs",
                        WidgetKind::Tabs,
                        NodeProps {
                            route_value: Some("overview".to_string()),
                            ..NodeProps::default()
                        },
                        vec![
                            node(
                                "overview-tab",
                                WidgetKind::Tab,
                                NodeProps {
                                    text: Some("Overview".to_string()),
                                    route_value: Some("overview".to_string()),
                                    ..NodeProps::default()
                                },
                                vec![],
                            ),
                            node(
                                "data-tab",
                                WidgetKind::Tab,
                                NodeProps {
                                    text: Some("Data".to_string()),
                                    route_value: Some("data".to_string()),
                                    ..NodeProps::default()
                                },
                                vec![],
                            ),
                        ],
                    ),
                    node(
                        "body",
                        WidgetKind::HLayout,
                        NodeProps::default(),
                        vec![node(
                            "body-panel",
                            WidgetKind::Panel,
                            NodeProps::default(),
                            vec![],
                        )],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 1000.0, 700.0, 1.0, &Theme::dark(), None);
        let tabs = layout.rects.get("tabs").copied().unwrap();
        let body = layout.rects.get("body").copied().unwrap();

        assert!(
            tabs.h <= 40.0,
            "empty tab-strip chrome should stay near control height, got {tabs:?}"
        );
        assert!(
            body.y <= tabs.y + tabs.h + 1.0,
            "body should begin immediately after tab strip, tabs={tabs:?} body={body:?}"
        );
        assert!(
            body.h >= 640.0,
            "body should receive remaining workbench height, got {body:?}"
        );
    }

    #[test]
    fn empty_tabs_strip_owns_its_css_header_height_without_overlapping_body() {
        let mut tabs = node(
            "tabs",
            WidgetKind::Tabs,
            NodeProps {
                route_value: Some("overview".to_string()),
                ..NodeProps::default()
            },
            vec![
                node(
                    "overview-tab",
                    WidgetKind::Tab,
                    NodeProps {
                        text: Some("Overview".to_string()),
                        route_value: Some("overview".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "data-tab",
                    WidgetKind::Tab,
                    NodeProps {
                        text: Some("Data".to_string()),
                        route_value: Some("data".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
            ],
        );
        tabs.style
            .parts
            .parts
            .entry("header".to_string())
            .or_default()
            .layout
            .height = Some(34.0);
        tabs.style.layout.padding_left = Some(8.0);
        tabs.style.layout.padding_right = Some(8.0);
        tabs.style.layout.column_gap = Some(4.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "workbench",
                WidgetKind::VLayout,
                NodeProps::default(),
                vec![
                    tabs,
                    node(
                        "body",
                        WidgetKind::HLayout,
                        NodeProps::default(),
                        vec![node(
                            "body-panel",
                            WidgetKind::Panel,
                            NodeProps::default(),
                            vec![],
                        )],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 1000.0, 700.0, 1.0, &Theme::dark(), None);
        let tabs = layout.rects["tabs"];
        let overview = layout.rects["overview-tab"];
        let data = layout.rects["data-tab"];
        let body = layout.rects["body"];

        assert!((tabs.h - 34.0).abs() <= 0.5, "styled tab strip={tabs:?}");
        assert!(
            overview.y + overview.h <= tabs.y + tabs.h + 0.5,
            "tab child must remain inside its styled strip: tabs={tabs:?} tab={overview:?}"
        );
        assert!(
            body.y + 0.5 >= tabs.y + tabs.h,
            "body must start below styled tab strip: tabs={tabs:?} body={body:?}"
        );
        assert!(
            overview.x >= tabs.x + 7.5 && data.x + data.w <= tabs.x + tabs.w - 7.5,
            "tabs must respect strip horizontal padding: tabs={tabs:?} first={overview:?} last={data:?}"
        );
        assert!(
            data.x >= overview.x + overview.w + 3.5,
            "tabs must respect strip column gap: first={overview:?} second={data:?}"
        );
    }

    #[test]
    fn inactive_page_content_is_removed_from_layout() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "pages",
                WidgetKind::Pages,
                NodeProps {
                    route_value: Some("objects".to_string()),
                    ..NodeProps::default()
                },
                vec![
                    node(
                        "editor-page",
                        WidgetKind::Page,
                        NodeProps {
                            route_value: Some("editor".to_string()),
                            ..NodeProps::default()
                        },
                        vec![node(
                            "editor-report",
                            WidgetKind::HtmlReport,
                            NodeProps::default(),
                            vec![],
                        )],
                    ),
                    node(
                        "objects-page",
                        WidgetKind::Page,
                        NodeProps {
                            route_value: Some("objects".to_string()),
                            ..NodeProps::default()
                        },
                        vec![node(
                            "objects-panel",
                            WidgetKind::Panel,
                            NodeProps::default(),
                            vec![node(
                                "objects-button",
                                WidgetKind::Button,
                                NodeProps::default(),
                                vec![],
                            )],
                        )],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 800.0, 480.0, 1.0, &Theme::dark(), None);

        assert!(!layout.rects.contains_key("editor-page"));
        assert!(!layout.rects.contains_key("editor-report"));
        assert!(layout.rects.contains_key("objects-page"));
        assert!(layout.rects.contains_key("objects-panel"));
        assert!(layout.rects.contains_key("objects-button"));
    }

    fn many_controls_layout_tree(count: usize) -> WidgetNode {
        let mut children = Vec::with_capacity(count);
        for index in 0..count {
            let kind = match index % 8 {
                0 => WidgetKind::Label,
                1 => WidgetKind::Button,
                2 => WidgetKind::Checkbox,
                3 => WidgetKind::Slider,
                4 => WidgetKind::ProgressBar,
                5 => WidgetKind::TextInput,
                6 => WidgetKind::Badge,
                _ => WidgetKind::Tag,
            };
            let mut props = NodeProps {
                text: Some(format!("Item {index}")),
                fixed_width: Some(180.0),
                ..NodeProps::default()
            };
            if matches!(kind, WidgetKind::Slider | WidgetKind::ProgressBar) {
                props.value = Some((index % 100) as f32 / 100.0);
                props.min = Some(0.0);
                props.max = Some(1.0);
            }
            if kind == WidgetKind::TextInput {
                props.placeholder = Some("Search".to_string());
            }
            children.push(node(&format!("w{index}"), kind, props, vec![]));
        }

        let mut flow = node(
            "flow",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            children,
        );
        flow.style.layout.gap = Some(6.0);
        flow.style.layout.row_gap = Some(6.0);
        node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![flow],
        )
    }

    #[test]
    #[ignore]
    fn bench_layout_many_controls() {
        let count = env_usize("DRAGONGUI_BENCH_LAYOUT_WIDGETS", 2_000);
        let iterations = env_usize("DRAGONGUI_BENCH_LAYOUT_ITERS", 300);
        let warmup = env_usize("DRAGONGUI_BENCH_LAYOUT_WARMUP", 20);
        let root = many_controls_layout_tree(count);
        let theme = Theme::dark();

        for _ in 0..warmup {
            let layout = compute_layout(&root, 1280.0, 900.0, 1.0, &theme, None);
            std::hint::black_box(layout.rects.len());
        }

        let start = std::time::Instant::now();
        let mut rects = 0usize;
        for _ in 0..iterations {
            let layout = compute_layout(&root, 1280.0, 900.0, 1.0, &theme, None);
            rects += layout.rects.len();
            std::hint::black_box(&layout);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "layout many controls: widgets={count} iterations={iterations} total_ms={:.3} ns_per_widget={:.1} rects_per_iter={:.1}",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_nanos() as f64 / (iterations * count) as f64,
            rects as f64 / iterations as f64
        );
    }

    // -----------------------------------------------------------------------
    // Intrinsic leaf and composite measurement contracts
    // -----------------------------------------------------------------------

    #[test]
    fn top_level_hlayout_fills_window_height() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![
                    node(
                        "panel",
                        WidgetKind::Panel,
                        NodeProps {
                            fixed_width: Some(280.0),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                    node(
                        "scatter",
                        WidgetKind::Scatter3D,
                        NodeProps::default(),
                        vec![],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 1200.0, 800.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let panel = layout.rects.get("panel").unwrap();
        let scatter = layout.rects.get("scatter").unwrap();

        assert_eq!(row.h, 800.0);
        assert_eq!(panel.h, 800.0);
        assert_eq!(scatter.x, 280.0);
        assert_eq!(scatter.w, 920.0);
        assert_eq!(scatter.h, 800.0);
    }

    #[test]
    fn extension_leaf_uses_intrinsic_height_without_overlapping_siblings() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "column",
                WidgetKind::VLayout,
                NodeProps::default(),
                vec![
                    node(
                        "spark",
                        WidgetKind::Extension,
                        NodeProps {
                            extension_type: Some("sparkline".to_string()),
                            intrinsic_width: Some(160.0),
                            intrinsic_height: Some(44.0),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                    node(
                        "gauge",
                        WidgetKind::Extension,
                        NodeProps {
                            extension_type: Some("gauge".to_string()),
                            intrinsic_width: Some(160.0),
                            intrinsic_height: Some(52.0),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 320.0, 220.0, 1.0, &Theme::dark(), None);
        let spark = layout.rects.get("spark").unwrap();
        let gauge = layout.rects.get("gauge").unwrap();

        assert_eq!(spark.h, 44.0);
        assert_eq!(gauge.h, 52.0);
        assert!(gauge.y >= spark.y + spark.h);
        assert!(spark.w > 0.0);
        assert!(gauge.w > 0.0);
    }

    #[test]
    fn standalone_badge_intrinsic_width_fits_long_text_in_row() {
        let mut badge = node(
            "margin-auto",
            WidgetKind::Badge,
            NodeProps {
                text: Some("margin auto".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        badge.style.layout.padding_left = Some(10.0);
        badge.style.layout.padding_right = Some(10.0);
        let expected_width =
            intrinsic_leaf_width(&badge, &Theme::dark()).expect("badge intrinsic width");

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![badge],
            )],
        );

        let layout = compute_layout(&root, 420.0, 120.0, 1.0, &Theme::dark(), None);
        let badge_rect = layout.rects.get("margin-auto").expect("badge rect");

        assert!(
            badge_rect.w + 0.5 >= expected_width,
            "margin-auto badge should fit its shaped intrinsic width: rect={badge_rect:?} expected={expected_width}"
        );
    }

    #[test]
    fn row_styled_vlayout_children_keep_intrinsic_width() {
        let radio = |id: &str, label: &str| {
            node(
                id,
                WidgetKind::RadioButton,
                NodeProps {
                    text: Some(label.to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )
        };
        let mut group = node(
            "group",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![
                radio("low", "Low"),
                radio("medium", "Medium"),
                radio("high", "High"),
            ],
        );
        group.style.layout.flex_direction = Some(FlexDirectionStyle::Row);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![group],
        );

        let layout = compute_layout(&root, 420.0, 120.0, 1.0, &Theme::dark(), None);
        for id in ["low", "medium", "high"] {
            let rect = layout.rects.get(id).expect("radio rect");
            assert!(
                rect.w >= 72.0,
                "row-styled VLayout child should keep intrinsic width: {id}={rect:?}"
            );
        }
    }

    #[test]
    fn drag_source_badge_wrapper_uses_child_intrinsic_width() {
        let badge = node(
            "badge",
            WidgetKind::Badge,
            NodeProps {
                text: Some("latency p95".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        let expected_badge_width =
            intrinsic_leaf_width(&badge, &Theme::dark()).expect("badge intrinsic width");
        let source = node(
            "source",
            WidgetKind::DragSource,
            NodeProps::default(),
            vec![badge],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "flow",
                WidgetKind::FlowLayout,
                NodeProps::default(),
                vec![source],
            )],
        );

        let layout = compute_layout(&root, 420.0, 120.0, 1.0, &Theme::dark(), None);
        let source_rect = layout.rects.get("source").expect("drag source rect");
        let badge_rect = layout.rects.get("badge").expect("badge rect");
        assert!(
            source_rect.w >= badge_rect.w
                && badge_rect.w + 0.5 >= expected_badge_width,
            "drag source should size to shaped badge width: source={source_rect:?} badge={badge_rect:?} expected={expected_badge_width}"
        );
    }

    #[test]
    fn inactive_tooltip_does_not_consume_window_flow() {
        let mut tooltip = node(
            "tip",
            WidgetKind::Tooltip,
            NodeProps {
                target: Some("button".to_string()),
                fixed_width: Some(260.0),
                fixed_height: Some(120.0),
                ..NodeProps::default()
            },
            vec![node(
                "tip-label",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Details".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        tooltip.style.layout.padding = Some(12.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node(
                    "content",
                    WidgetKind::HLayout,
                    NodeProps::default(),
                    vec![node(
                        "button",
                        WidgetKind::Button,
                        NodeProps::default(),
                        vec![],
                    )],
                ),
                tooltip,
            ],
        );

        let layout = compute_layout(&root, 800.0, 600.0, 1.0, &Theme::dark(), None);
        let content = layout.rects.get("content").unwrap();
        let tip = layout.rects.get("tip").unwrap();

        assert_eq!(content.h, 600.0);
        assert_eq!(tip.w, 0.0);
        assert_eq!(tip.h, 0.0);
    }

    #[test]
    fn standalone_badge_and_tag_keep_intrinsic_pill_size() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![
                    node(
                        "badge",
                        WidgetKind::Badge,
                        NodeProps {
                            text: Some("live".to_string()),
                            level: Some("success".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                    node(
                        "tag",
                        WidgetKind::Tag,
                        NodeProps {
                            text: Some("owner:data".to_string()),
                            level: Some("neutral".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 320.0, 90.0, 1.0, &Theme::dark(), None);
        let badge = layout.rects.get("badge").unwrap();
        let tag = layout.rects.get("tag").unwrap();
        let expected_height = (Theme::dark().font_size + 8.0).max(20.0);

        assert!(badge.w >= 24.0);
        assert_eq!(badge.h, expected_height);
        assert!(tag.w > badge.w);
        assert_eq!(tag.h, expected_height);
    }

    #[test]
    fn grid_auto_track_uses_badge_intrinsic_width() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "grid",
                WidgetKind::GridLayout,
                NodeProps {
                    grid_template_columns: Some(vec![GridTrackSize::Auto]),
                    ..NodeProps::default()
                },
                vec![node(
                    "tag",
                    WidgetKind::Tag,
                    NodeProps {
                        text: Some("busy".to_string()),
                        level: Some("warning".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                )],
            )],
        );

        let layout = compute_layout(&root, 320.0, 90.0, 1.0, &Theme::dark(), None);
        let tag = layout.rects.get("tag").unwrap();
        let expected_height = (Theme::dark().font_size + 8.0).max(20.0);

        assert!(
            tag.w >= 36.0,
            "tag width should be intrinsic, got {}",
            tag.w
        );
        assert_eq!(tag.h, expected_height);
    }

    #[test]
    fn hovered_tooltip_gets_overlay_layout_and_children() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node(
                    "content",
                    WidgetKind::HLayout,
                    NodeProps::default(),
                    vec![node(
                        "button",
                        WidgetKind::Button,
                        NodeProps::default(),
                        vec![],
                    )],
                ),
                node(
                    "tip",
                    WidgetKind::Tooltip,
                    NodeProps {
                        target: Some("button".to_string()),
                        fixed_width: Some(260.0),
                        fixed_height: Some(120.0),
                        ..NodeProps::default()
                    },
                    vec![node(
                        "tip-label",
                        WidgetKind::Label,
                        NodeProps {
                            text: Some("Details".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    )],
                ),
            ],
        );
        let mut state = WidgetState::default();
        state.hovered = Some("button".to_string());

        let layout = compute_layout(&root, 800.0, 600.0, 1.0, &Theme::dark(), Some(&state));
        let tip = layout.rects.get("tip").unwrap();
        let label = layout.rects.get("tip-label").unwrap();

        assert_eq!(tip.w, 260.0);
        assert_eq!(tip.h, 120.0);
        assert!(label.w > 0.0);
        assert!(label.h > 0.0);
        assert!(label.x >= tip.x);
        assert!(label.y >= tip.y);
    }

    #[test]
    fn rich_tooltip_and_its_text_escape_an_overflow_hidden_parent_clip() {
        let tooltip = node(
            "tip",
            WidgetKind::Tooltip,
            NodeProps {
                target: Some("button".to_string()),
                fixed_width: Some(220.0),
                fixed_height: Some(80.0),
                ..NodeProps::default()
            },
            vec![node(
                "tip-label",
                WidgetKind::Label,
                NodeProps {
                    text: Some(
                        "Tooltip text remains visible beyond the parent panel boundary."
                            .to_string(),
                    ),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![
                node("button", WidgetKind::Button, NodeProps::default(), vec![]),
                tooltip,
            ],
        );
        panel.style.layout.width = Some(160.0);
        panel.style.layout.height = Some(60.0);
        panel.style.layout.flex_grow = Some(0.0);
        panel.style.layout.flex_shrink = Some(0.0);
        panel.style.layout.overflow = Some(OverflowStyle::Hidden);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let mut state = WidgetState::default();
        state.hovered = Some("button".to_string());

        let layout = compute_layout(&root, 480.0, 320.0, 1.0, &Theme::dark(), Some(&state));
        let root_rect = layout.rects.get("window").unwrap();
        let panel_rect = layout.rects.get("panel").unwrap();
        let tip_rect = layout.rects.get("tip").unwrap();
        let tip_clip = layout.clips.get("tip").unwrap();
        let tip_paint_clip = layout.paint_clips.get("tip").unwrap();
        let label_rect = layout.rects.get("tip-label").unwrap();
        let label_clip = layout.clips.get("tip-label").unwrap();

        assert!(
            tip_rect.y + tip_rect.h > panel_rect.y + panel_rect.h,
            "test tooltip must cross the parent panel boundary: panel={panel_rect:?} tip={tip_rect:?}"
        );
        assert!(
            (tip_paint_clip.x - root_rect.x).abs() <= 0.01
                && (tip_paint_clip.y - root_rect.y).abs() <= 0.01
                && (tip_paint_clip.w - root_rect.w).abs() <= 0.01
                && (tip_paint_clip.h - root_rect.h).abs() <= 0.01,
            "promoted tooltip paint should be clipped by the window: root={root_rect:?} paint_clip={tip_paint_clip:?}"
        );
        assert!(
            (tip_clip.x - tip_rect.x).abs() <= 0.01
                && (tip_clip.y - tip_rect.y).abs() <= 0.01
                && (tip_clip.w - tip_rect.w).abs() <= 0.01
                && (tip_clip.h - tip_rect.h).abs() <= 0.01,
            "tooltip surface should remain fully visible outside its retained parent: rect={tip_rect:?} clip={tip_clip:?}"
        );
        assert!(
            label_clip.w > 0.0
                && label_clip.h > 0.0
                && label_clip.y + label_clip.h > panel_rect.y + panel_rect.h,
            "tooltip child text should inherit the tooltip clip, not the panel clip: panel={panel_rect:?} label={label_rect:?} clip={label_clip:?}"
        );
    }

    #[test]
    fn tooltip_anchors_to_final_masonry_target_geometry() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(120.0),
                grid_masonry: true,
                ..NodeProps::default()
            },
            vec![
                card("tall", 100.0),
                card("short", 40.0),
                card("target", 40.0),
            ],
        );
        grid.style.layout.gap = Some(10.0);
        let tooltip = node(
            "tip",
            WidgetKind::Tooltip,
            NodeProps {
                target: Some("target".to_string()),
                fixed_width: Some(80.0),
                fixed_height: Some(32.0),
                ..NodeProps::default()
            },
            vec![],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid, tooltip],
        );
        let mut state = WidgetState::default();
        state.hovered = Some("target".to_string());

        let layout = compute_layout(&root, 420.0, 300.0, 1.0, &Theme::dark(), Some(&state));
        let target = layout.rects.get("target").unwrap();
        let tip = layout.rects.get("tip").unwrap();
        let margin = Theme::dark().spacing;

        assert!(
            (tip.x - (target.x + target.w * 0.5 - tip.w * 0.5)).abs() <= 0.5,
            "tooltip should use the target's packed horizontal position: target={target:?} tip={tip:?}"
        );
        assert!(
            (tip.y - (target.y + target.h + margin)).abs() <= 0.5,
            "tooltip should use the target's packed vertical position: target={target:?} tip={tip:?}"
        );
    }

    #[test]
    fn tooltip_anchors_to_final_scrolled_target_geometry() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
                node("target", WidgetKind::Button, NodeProps::default(), vec![]),
                node("fourth", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.gap = Some(0.0);
        panel.style.layout.height = Some(100.0);
        for child in &mut panel.children {
            child.style.layout.height = Some(34.0);
        }
        let tooltip = node(
            "tip",
            WidgetKind::Tooltip,
            NodeProps {
                target: Some("target".to_string()),
                fixed_width: Some(80.0),
                fixed_height: Some(32.0),
                ..NodeProps::default()
            },
            vec![],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel, tooltip],
        );
        let mut state = WidgetState::default();
        state.hovered = Some("target".to_string());
        state.container_scroll_y.insert("panel".to_string(), 40.0);

        let layout = compute_layout(&root, 240.0, 200.0, 1.0, &Theme::dark(), Some(&state));
        let target = layout.rects.get("target").unwrap();
        let tip = layout.rects.get("tip").unwrap();
        let margin = Theme::dark().spacing;

        assert!(
            layout.scroll_y.get("panel").copied().unwrap_or(0.0) > 0.0,
            "test panel should apply a nonzero clamped scroll offset"
        );
        assert!(
            (tip.x - (target.x + target.w * 0.5 - tip.w * 0.5)).abs() <= 0.5,
            "tooltip should use the target's scrolled horizontal position: target={target:?} tip={tip:?}"
        );
        assert!(
            (tip.y - (target.y + target.h + margin)).abs() <= 0.5,
            "tooltip should use the target's scrolled vertical position: target={target:?} tip={tip:?}"
        );
    }

    #[test]
    fn collapsed_collapsible_hides_children_from_layout() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "section",
                WidgetKind::Collapsible,
                NodeProps {
                    text: Some("Advanced".to_string()),
                    expanded: Some(false),
                    ..NodeProps::default()
                },
                vec![node(
                    "child",
                    WidgetKind::Button,
                    NodeProps::default(),
                    vec![],
                )],
            )],
        );

        let layout = compute_layout(&root, 400.0, 300.0, 1.0, &Theme::dark(), None);
        let section = layout.rects.get("section").unwrap();

        assert_eq!(section.h, Theme::dark().control_height());
        assert!(!layout.rects.contains_key("child"));
    }

    #[test]
    fn expanded_collapsible_lays_out_children_below_header() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "section",
                WidgetKind::Collapsible,
                NodeProps {
                    text: Some("Advanced".to_string()),
                    expanded: Some(true),
                    ..NodeProps::default()
                },
                vec![node(
                    "child",
                    WidgetKind::Button,
                    NodeProps::default(),
                    vec![],
                )],
            )],
        );

        let layout = compute_layout(&root, 400.0, 300.0, 1.0, &Theme::dark(), None);
        let section = layout.rects.get("section").unwrap();
        let child = layout.rects.get("child").unwrap();

        assert!(section.h > Theme::dark().control_height());
        assert!(child.y >= section.y + Theme::dark().control_height());
    }

    #[test]
    fn expanded_collapsible_styled_padding_is_body_padding() {
        let mut section = node(
            "section",
            WidgetKind::Collapsible,
            NodeProps {
                text: Some("Advanced".to_string()),
                expanded: Some(true),
                ..NodeProps::default()
            },
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        section.style.layout.padding = Some(8.0);
        section.style.layout.gap = Some(6.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![section],
        );

        let theme = Theme::dark();
        let layout = compute_layout(&root, 400.0, 300.0, 1.0, &theme, None);
        let section = layout.rects.get("section").unwrap();
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();

        assert!(
            first.y >= section.y + theme.control_height() + 8.0,
            "styled collapsible padding must start below the header: section={section:?} first={first:?}"
        );
        assert!(
            second.y >= first.y + first.h + 6.0,
            "styled collapsible gap should separate body rows: first={first:?} second={second:?}"
        );
    }

    #[test]
    fn expanded_tree_node_height_includes_children_when_row_height_is_styled() {
        let mut branch = node(
            "branch",
            WidgetKind::TreeNode,
            NodeProps {
                text: Some("Branch".to_string()),
                expanded: Some(true),
                ..NodeProps::default()
            },
            vec![
                node(
                    "leaf-a",
                    WidgetKind::TreeNode,
                    NodeProps {
                        text: Some("Leaf A".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "leaf-b",
                    WidgetKind::TreeNode,
                    NodeProps {
                        text: Some("Leaf B".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
            ],
        );
        branch.style.layout.height_value = Some(LayoutLength::LogicalPx(28.0));
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "tree",
                WidgetKind::TreeView,
                NodeProps::default(),
                vec![
                    branch,
                    node(
                        "sibling",
                        WidgetKind::TreeNode,
                        NodeProps {
                            text: Some("Sibling".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 400.0, 300.0, 1.0, &Theme::dark(), None);
        let branch = layout.rects.get("branch").unwrap();
        let leaf_a = layout.rects.get("leaf-a").unwrap();
        let leaf_b = layout.rects.get("leaf-b").unwrap();
        let sibling = layout.rects.get("sibling").unwrap();

        assert!(branch.h > 28.0);
        assert!(leaf_a.y >= branch.y + 28.0);
        assert!(leaf_b.y >= leaf_a.y + leaf_a.h);
        assert!(sibling.y >= leaf_b.y + leaf_b.h);
    }

    #[test]
    fn text_area_rows_drive_preferred_height() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "notes",
                WidgetKind::TextArea,
                NodeProps {
                    text: Some("one\ntwo".to_string()),
                    rows: Some(6),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );

        let layout = compute_layout(&root, 400.0, 300.0, 1.0, &Theme::dark(), None);
        let notes = layout.rects.get("notes").unwrap();

        assert!(notes.h > Theme::dark().control_height() * 2.0);
    }

    #[test]
    fn text_area_css_rows_override_constructor_rows() {
        let mut notes = node(
            "notes",
            WidgetKind::TextArea,
            NodeProps {
                text: Some("one\ntwo".to_string()),
                rows: Some(2),
                ..NodeProps::default()
            },
            vec![],
        );
        notes.style.widget.text_area_rows = Some(6.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![notes],
        );

        let layout = compute_layout(&root, 400.0, 300.0, 1.0, &Theme::dark(), None);
        let notes = layout.rects.get("notes").unwrap();

        assert!(notes.h > Theme::dark().control_height() * 2.0);
    }

    // -----------------------------------------------------------------------
    // Window shell, app chrome, and workbench allocation contracts
    // -----------------------------------------------------------------------

    #[test]
    fn window_body_flexes_between_menu_and_status_bars() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node(
                    "menu",
                    WidgetKind::MenuBar,
                    NodeProps::default(),
                    vec![node(
                        "file",
                        WidgetKind::Menu,
                        NodeProps {
                            text: Some("File".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    )],
                ),
                node(
                    "body",
                    WidgetKind::HLayout,
                    NodeProps::default(),
                    vec![node(
                        "content",
                        WidgetKind::Panel,
                        NodeProps::default(),
                        vec![],
                    )],
                ),
                node(
                    "status",
                    WidgetKind::StatusBar,
                    NodeProps::default(),
                    vec![],
                ),
            ],
        );

        let layout = compute_layout(&root, 1000.0, 800.0, 1.0, &Theme::dark(), None);
        let menu = layout.rects.get("menu").unwrap();
        let body = layout.rects.get("body").unwrap();
        let content = layout.rects.get("content").unwrap();
        let status = layout.rects.get("status").unwrap();

        assert_eq!(menu.y, 0.0);
        assert_eq!(body.y, menu.h);
        assert_eq!(status.y, 800.0 - status.h);
        assert_eq!(body.h, 800.0 - menu.h - status.h);
        assert_eq!(content.h, body.h);
        assert!(
            status.y + status.h <= 800.0,
            "status bar overflowed window: status={status:?}"
        );
    }

    #[test]
    fn vertical_window_body_shrinks_and_scrolls_between_shell_bars() {
        let mut sections = Vec::new();
        for index in 0..3 {
            let mut section = node(
                &format!("section-{index}"),
                WidgetKind::Panel,
                NodeProps::default(),
                vec![],
            );
            section.style.layout.height = Some(80.0);
            section.style.layout.flex_shrink = Some(0.0);
            sections.push(section);
        }
        let mut body = node("body", WidgetKind::VLayout, NodeProps::default(), sections);
        body.style.layout.padding = Some(0.0);
        body.style.layout.gap = Some(0.0);
        body.style.layout.overflow_y = Some(OverflowStyle::Auto);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node("menu", WidgetKind::MenuBar, NodeProps::default(), vec![]),
                body,
                node(
                    "status",
                    WidgetKind::StatusBar,
                    NodeProps::default(),
                    vec![],
                ),
            ],
        );

        let state = WidgetState::default();
        let layout = compute_layout(&root, 360.0, 200.0, 1.0, &Theme::dark(), Some(&state));
        let menu = layout.rects.get("menu").unwrap();
        let body = layout.rects.get("body").unwrap();
        let status = layout.rects.get("status").unwrap();

        assert!(
            (body.y - menu.h).abs() <= 0.5,
            "menu={menu:?} body={body:?}"
        );
        assert!(
            status.y >= body.y + body.h - 0.5 && status.y + status.h <= 200.0,
            "vertical body must not push the status bar outside the window: body={body:?} status={status:?}"
        );
        assert!(
            layout.scroll_max_y.get("body").copied().unwrap_or(0.0) > 0.0,
            "bounded vertical body should own overflow from its fixed sections: body={body:?}"
        );
    }

    #[test]
    fn nav_items_keep_equal_physical_height_and_gap_at_125_percent_scale() {
        let items: Vec<WidgetNode> = (0..5)
            .map(|index| {
                node(
                    &format!("nav-{index}"),
                    WidgetKind::NavItem,
                    NodeProps {
                        text: Some(format!("Section {index}")),
                        ..NodeProps::default()
                    },
                    vec![],
                )
            })
            .collect();
        let mut sidebar = node(
            "sidebar",
            WidgetKind::Sidebar,
            NodeProps {
                fixed_width: Some(236.0),
                ..NodeProps::default()
            },
            items,
        );
        sidebar.style.layout.padding = Some(16.0);
        sidebar.style.layout.gap = Some(8.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![sidebar],
        );
        let mut theme = Theme::dark();
        theme.spacing = 6.0;

        let layout = compute_layout(&root, 360.0, 500.0, 1.25, &theme, None);
        let rects: Vec<Rect> = (0..5)
            .map(|index| layout.rects[&format!("nav-{index}")])
            .collect();

        for rect in &rects {
            assert_eq!(
                rect.h, 35.0,
                "28 logical pixels should resolve to 35 physical pixels at 125%: {rect:?}"
            );
        }
        for pair in rects.windows(2) {
            let gap = pair[1].y - (pair[0].y + pair[0].h);
            assert_eq!(
                gap, 10.0,
                "8 logical pixels should resolve to 10 physical pixels at 125%: {pair:?}"
            );
        }
    }

    #[test]
    fn app_shell_pages_shrink_between_menu_and_status_bars() {
        let sidebar_items: Vec<WidgetNode> = (0..16)
            .map(|index| {
                node(
                    &format!("nav-{index}"),
                    WidgetKind::NavItem,
                    NodeProps {
                        text: Some(format!("Section {index}")),
                        ..NodeProps::default()
                    },
                    vec![],
                )
            })
            .collect();
        let sidebar = node(
            "sidebar",
            WidgetKind::Sidebar,
            NodeProps {
                fixed_width: Some(220.0),
                ..NodeProps::default()
            },
            sidebar_items,
        );

        let mut cards = Vec::new();
        for index in 0..12 {
            cards.push(node(
                &format!("card-{index}"),
                WidgetKind::Panel,
                NodeProps {
                    text: Some(format!("Card {index}")),
                    ..NodeProps::default()
                },
                vec![node(
                    &format!("card-label-{index}"),
                    WidgetKind::Label,
                    NodeProps {
                        text: Some("Content".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                )],
            ));
        }
        let mut grid = node(
            "overview-grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(180.0),
                ..NodeProps::default()
            },
            cards,
        );
        grid.style.layout.padding = Some(10.0);
        grid.style.layout.gap = Some(12.0);

        let page = node(
            "overview-page",
            WidgetKind::Page,
            NodeProps {
                text: Some("Overview".to_string()),
                route_value: Some("overview".to_string()),
                ..NodeProps::default()
            },
            vec![grid],
        );
        let pages = node(
            "pages",
            WidgetKind::Pages,
            NodeProps {
                route_value: Some("overview".to_string()),
                ..NodeProps::default()
            },
            vec![page],
        );
        let body = node(
            "body",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, pages],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node("menu", WidgetKind::MenuBar, NodeProps::default(), vec![]),
                body,
                node(
                    "status",
                    WidgetKind::StatusBar,
                    NodeProps::default(),
                    vec![],
                ),
            ],
        );

        let layout = compute_layout(&root, 900.0, 520.0, 1.0, &Theme::dark(), None);
        let menu = layout.rects.get("menu").unwrap();
        let body = layout.rects.get("body").unwrap();
        let sidebar = layout.rects.get("sidebar").unwrap();
        let pages = layout.rects.get("pages").unwrap();
        let page = layout.rects.get("overview-page").unwrap();
        let status = layout.rects.get("status").unwrap();

        let expected_body_h = 520.0 - menu.h - status.h;
        assert_eq!(body.y, menu.h);
        assert!((body.h - expected_body_h).abs() <= 0.5, "body={body:?}");
        assert!(
            (sidebar.h - body.h).abs() <= 0.5,
            "sidebar={sidebar:?} body={body:?}"
        );
        assert!(
            (pages.h - body.h).abs() <= 0.5,
            "pages={pages:?} body={body:?}"
        );
        assert!(
            (page.h - body.h).abs() <= 0.5,
            "page={page:?} body={body:?}"
        );
        assert!(
            status.y >= body.y + body.h - 0.5 && status.y + status.h <= 520.0,
            "status should remain below the bounded body: body={body:?} status={status:?}"
        );
    }

    #[test]
    fn app_shell_workbench_body_uses_flex_basis_without_collapsing_content() {
        let sidebar = node(
            "sidebar",
            WidgetKind::Sidebar,
            NodeProps {
                fixed_width: Some(190.0),
                ..NodeProps::default()
            },
            vec![node(
                "nav",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Overview".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );

        let mut title = node(
            "title",
            WidgetKind::Label,
            NodeProps {
                text: Some("AppShell + Body + WorkbenchLayout".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        title.style.text.font_size = Some(20.0);

        let mut caption = node(
            "caption",
            WidgetKind::Label,
            NodeProps {
                text: Some(
                    "Bounded dashboard shell with a fixed sidebar, flexible scroll-owning body, \
                     workbench main region, and fixed-height status bar."
                        .to_string(),
                ),
                ..NodeProps::default()
            },
            vec![],
        );
        caption.style.text.line_height = Some(LineHeight::Multiplier(1.14));

        let mut main = node(
            "main",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            vec![node(
                "main-row",
                WidgetKind::Panel,
                NodeProps::default(),
                vec![node(
                    "main-label",
                    WidgetKind::Label,
                    NodeProps {
                        text: Some("Workbench row".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                )],
            )],
        );
        main.style.layout.height_value = Some(LayoutLength::LogicalPx(292.0));
        main.style.layout.flex_grow = Some(0.0);
        main.style.layout.flex_shrink = Some(0.0);

        let mut body = node(
            "body",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            vec![title, caption, main],
        );
        body.style.layout.height_value = Some(LayoutLength::LogicalPx(0.0));
        body.style.layout.flex_grow = Some(1.0);
        body.style.layout.flex_shrink = Some(1.0);
        body.style.layout.flex_basis_value = Some(LayoutLength::LogicalPx(0.0));
        body.style.layout.min_width = Some(0.0);
        body.style.layout.min_height = Some(0.0);
        body.style.layout.overflow_y = Some(OverflowStyle::Auto);
        body.style.layout.gap = Some(14.0);

        let status = node(
            "status",
            WidgetKind::StatusBar,
            NodeProps {
                fixed_height: Some(34.0),
                ..NodeProps::default()
            },
            vec![node(
                "status-label",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Ready".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );

        let mut workbench = node(
            "workbench",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![body, status],
        );
        workbench.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        workbench.style.layout.height_value = Some(LayoutLength::Percent(100.0));
        workbench.style.layout.flex_grow = Some(1.0);
        workbench.style.layout.flex_shrink = Some(1.0);
        workbench.style.layout.flex_basis_value = Some(LayoutLength::LogicalPx(0.0));
        workbench.style.layout.min_width = Some(0.0);
        workbench.style.layout.min_height = Some(0.0);

        let mut shell = node(
            "shell",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, workbench],
        );
        shell.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        shell.style.layout.height_value = Some(LayoutLength::Percent(100.0));
        shell.style.layout.min_width = Some(0.0);
        shell.style.layout.min_height = Some(0.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![shell],
        );

        let layout = compute_layout(&root, 1040.0, 720.0, 1.0, &Theme::dark(), None);
        let shell = layout.rects.get("shell").unwrap();
        let sidebar = layout.rects.get("sidebar").unwrap();
        let workbench = layout.rects.get("workbench").unwrap();
        let body = layout.rects.get("body").unwrap();
        let title = layout.rects.get("title").unwrap();
        let caption = layout.rects.get("caption").unwrap();
        let main = layout.rects.get("main").unwrap();
        let status = layout.rects.get("status").unwrap();

        assert_eq!(shell.h, 720.0);
        assert_eq!(sidebar.w, 190.0);
        assert!(
            workbench.x >= sidebar.x + sidebar.w - 0.5 && workbench.x + workbench.w <= 1040.5,
            "workbench should fill remaining visible width: sidebar={sidebar:?} workbench={workbench:?}"
        );
        assert!(
            body.w > 800.0,
            "scroll body content width should not collapse to the scrollbar gutter: {body:?}"
        );
        assert!(
            title.w > 700.0 && caption.w > 700.0,
            "body children should be measured against the grown body width: title={title:?} caption={caption:?}"
        );
        assert!(
            main.h >= 291.0,
            "fixed-height workbench main should remain visible in body content: {main:?}"
        );
        assert!(
            (status.y - (720.0 - status.h)).abs() <= 0.5,
            "status bar should stay pinned inside the workbench viewport: body={body:?} status={status:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Text-aware control sizing and minimum hit-target contracts
    // -----------------------------------------------------------------------

    #[test]
    fn row_controls_keep_intrinsic_text_width() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![
                    node(
                        "apply",
                        WidgetKind::Button,
                        NodeProps {
                            text: Some("Apply".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                    node(
                        "mode",
                        WidgetKind::Dropdown,
                        NodeProps {
                            text: Some("summary".to_string()),
                            items: vec!["summary".to_string(), "layout".to_string()],
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 420.0, 120.0, 1.0, &Theme::dark(), None);
        let apply = layout.rects.get("apply").unwrap();
        let mode = layout.rects.get("mode").unwrap();

        assert!(apply.w >= 72.0, "button collapsed to {:?}", apply);
        assert!(mode.w >= 112.0, "dropdown collapsed to {:?}", mode);
    }

    #[test]
    fn capped_control_preferred_widths_shrink_to_non_text_minimums() {
        let buttons = ["first", "second"]
            .into_iter()
            .map(|id| {
                node(
                    id,
                    WidgetKind::Button,
                    NodeProps {
                        text: Some(
                            "A deliberately long action label that exceeds the preferred cap"
                                .to_string(),
                        ),
                        ..NodeProps::default()
                    },
                    vec![],
                )
            })
            .collect();
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                buttons,
            )],
        );

        let layout = compute_layout(&root, 160.0, 80.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();

        assert!(first.w >= 72.0 && second.w >= 72.0);
        assert!(first.w < 280.0 && second.w < 280.0);
        assert!(
            second.x + second.w <= 160.5,
            "preferred text widths should shrink inside the row: first={first:?} second={second:?}"
        );
    }

    #[test]
    fn styled_font_size_increases_intrinsic_leaf_height_and_width() {
        let mut tall = node(
            "headline",
            WidgetKind::Label,
            NodeProps {
                text: Some("Large headline".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        tall.style.text.font_size = Some(30.0);
        let height_root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![tall],
        );
        let height_layout = compute_layout(&height_root, 600.0, 240.0, 1.0, &Theme::dark(), None);
        let headline = height_layout.rects.get("headline").unwrap();

        assert!(
            headline.h > Theme::dark().control_height(),
            "large CSS font-size should increase label height: {headline:?}"
        );

        let mut wide = node(
            "wide",
            WidgetKind::Label,
            NodeProps {
                text: Some("Large headline".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        wide.style.text.font_size = Some(30.0);
        let mut narrow = node(
            "narrow",
            WidgetKind::Label,
            NodeProps {
                text: Some("Large headline".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        narrow.style.text.font_size = Some(12.0);
        let width_root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![wide, narrow],
            )],
        );
        let width_layout = compute_layout(&width_root, 800.0, 120.0, 1.0, &Theme::dark(), None);
        let wide = width_layout.rects.get("wide").unwrap();
        let narrow = width_layout.rects.get("narrow").unwrap();
        assert!(
            wide.w > narrow.w,
            "large CSS font-size should increase intrinsic text width: wide={wide:?} narrow={narrow:?}"
        );
    }

    #[test]
    fn label_wraps_and_reserves_multiline_height_in_narrow_panel() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "panel",
                WidgetKind::Panel,
                NodeProps {
                    fixed_width: Some(180.0),
                    ..NodeProps::default()
                },
                vec![node(
                    "label",
                    WidgetKind::Label,
                    NodeProps {
                        text: Some(
                            "This label should wrap onto several lines inside a narrow panel"
                                .to_string(),
                        ),
                        ..NodeProps::default()
                    },
                    vec![],
                )],
            )],
        );

        let state = WidgetState::default();
        let layout = compute_layout(&root, 420.0, 260.0, 1.0, &Theme::dark(), Some(&state));
        let label = layout.rects.get("label").unwrap();

        assert!(
            label.h
                > node_control_height_lp(
                    &node("baseline", WidgetKind::Label, NodeProps::default(), vec![]),
                    &Theme::dark()
                ),
            "wrapped label did not reserve multiline height: {label:?}"
        );
    }

    #[test]
    fn checkbox_intrinsic_width_uses_styled_box_width() {
        let mut normal = node(
            "normal",
            WidgetKind::Checkbox,
            NodeProps {
                text: Some("Network".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        normal.style.text.font_size = Some(12.0);

        let mut switch = normal.clone();
        switch.id = "switch".to_string();
        switch
            .style
            .parts
            .parts
            .entry("box".to_string())
            .or_default()
            .layout
            .width = Some(36.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![normal, switch],
            )],
        );

        let layout = compute_layout(&root, 480.0, 120.0, 1.0, &Theme::dark(), None);
        let normal = layout.rects.get("normal").unwrap();
        let switch = layout.rects.get("switch").unwrap();

        assert!(
            switch.w >= normal.w + 18.0,
            "styled switch checkbox did not reserve its wider box: normal={normal:?} switch={switch:?}"
        );
    }

    #[test]
    fn toggle_switch_intrinsic_width_uses_styled_track_width() {
        let mut normal = node(
            "normal",
            WidgetKind::ToggleSwitch,
            NodeProps {
                text: Some("Network".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        normal.style.text.font_size = Some(12.0);

        let mut wide = normal.clone();
        wide.id = "wide".to_string();
        wide.style
            .parts
            .parts
            .entry("track".to_string())
            .or_default()
            .layout
            .width = Some(60.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![normal, wide],
            )],
        );

        let layout = compute_layout(&root, 520.0, 120.0, 1.0, &Theme::dark(), None);
        let normal = layout.rects.get("normal").unwrap();
        let wide = layout.rects.get("wide").unwrap();

        assert!(
            wide.w >= normal.w + 17.0,
            "styled toggle switch did not reserve its wider track: normal={normal:?} wide={wide:?}"
        );
    }

    #[test]
    fn boolean_controls_in_column_use_intrinsic_width_by_default() {
        let checkbox = node(
            "enabled",
            WidgetKind::Checkbox,
            NodeProps {
                text: Some("Enable analysis".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        let toggle = node(
            "live",
            WidgetKind::ToggleSwitch,
            NodeProps {
                text: Some("Live acquisition".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        let root = node(
            "root",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![checkbox, toggle],
        );

        let layout = compute_layout(&root, 360.0, 120.0, 1.0, &Theme::dark(), None);
        let root = layout.rects.get("root").expect("root rect");
        let checkbox = layout.rects.get("enabled").expect("checkbox rect");
        let toggle = layout.rects.get("live").expect("toggle rect");

        assert_eq!(root.w, 360.0);
        assert!(
            checkbox.w < root.w * 0.60,
            "checkbox should not stretch across the full column: {checkbox:?}"
        );
        assert!(
            toggle.w < root.w * 0.70,
            "toggle switch should not stretch across the full column: {toggle:?}"
        );
    }

    #[test]
    fn explicit_boolean_control_width_can_still_fill_column() {
        let mut checkbox = node(
            "enabled",
            WidgetKind::Checkbox,
            NodeProps {
                text: Some("Enable analysis".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        checkbox.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        let root = node(
            "root",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![checkbox],
        );

        let layout = compute_layout(&root, 360.0, 80.0, 1.0, &Theme::dark(), None);
        let checkbox = layout.rects.get("enabled").expect("checkbox rect");

        assert_eq!(checkbox.w, 360.0);
    }

    #[test]
    fn unlabeled_boolean_editor_uses_tight_intrinsic_width_in_property_row() {
        let label = {
            let mut label = node(
                "label",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Enabled".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            );
            label.style.layout.width = Some(84.0);
            label.style.layout.flex_shrink = Some(0.0);
            label
        };
        let editor = {
            let mut editor = node(
                "editor",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![node(
                    "enabled",
                    WidgetKind::Checkbox,
                    NodeProps {
                        text: Some(String::new()),
                        ..NodeProps::default()
                    },
                    vec![],
                )],
            );
            editor.style.layout.flex_grow = Some(1.0);
            editor.style.layout.min_width = Some(0.0);
            editor
        };
        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![label, editor],
        );
        row.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        row.style.layout.gap = Some(10.0);
        let root = node("root", WidgetKind::VLayout, NodeProps::default(), vec![row]);

        let layout = compute_layout(&root, 360.0, 80.0, 1.0, &Theme::dark(), None);
        let checkbox = layout.rects.get("enabled").expect("checkbox rect");

        assert!(
            checkbox.w <= CHECKBOX_BOX_LP + CHECKBOX_LEFT_PAD_LP * 2.0 + 0.5,
            "unlabeled property checkbox should stay tight to the box: {checkbox:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Titled panel, menu, and header/body geometry contracts
    // -----------------------------------------------------------------------

    #[test]
    fn titled_panel_style_padding_still_reserves_title_space() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Live style controls".to_string()),
                ..NodeProps::default()
            },
            vec![node(
                "button",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Cycle".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.gap = Some(10.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 400.0, 240.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let button = layout.rects.get("button").unwrap();

        assert!(
            button.y - panel.y >= 39.0,
            "custom padding let titled panel child overlap title: panel={panel:?} button={button:?}"
        );
    }

    #[test]
    fn titled_panel_offsets_first_child_from_content_clip_for_control_antialiasing() {
        let mut button = node(
            "button",
            WidgetKind::Button,
            NodeProps {
                text: Some("Run".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        button.style.layout.width = Some(96.0);

        let mut controls = node(
            "controls",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            vec![button],
        );
        controls.style.layout.gap = Some(10.0);

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Controls".to_string()),
                ..NodeProps::default()
            },
            vec![controls],
        );
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.gap = Some(10.0);
        panel.style.layout.width = Some(260.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let theme = Theme::dark();
        let layout = compute_layout(&root, 420.0, 240.0, 1.0, &theme, None);
        let button = layout.rects.get("button").unwrap();
        let button_paint_clip = layout.paint_clip_rect("button").unwrap();
        let panel_node = root.children.first().unwrap();
        let content_clip_top = layout.rects.get("panel").unwrap().y
            + panel_title_top_padding_lp(panel_node, &theme)
            + panel_title_line_height_lp(panel_node, &theme)
            + panel_title_body_gap_lp(panel_node, &theme);

        assert!(
            (button.y - content_clip_top) >= PANEL_BODY_VISUAL_INSET_LP - 0.1,
            "first child should have a small paint inset below titled content clip: button={button:?} content_clip_top={content_clip_top}"
        );
        assert!(
            button.y > button_paint_clip.y,
            "button should not be flush against its inherited paint clip: button={button:?} paint_clip={button_paint_clip:?}"
        );
    }

    #[test]
    fn titled_panel_reservation_uses_title_font_size_and_gap() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Large title".to_string()),
                ..NodeProps::default()
            },
            vec![node(
                "button",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Child".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.gap = Some(12.0);
        panel.style.text.font_size = Some(22.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 400.0, 240.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let button = layout.rects.get("button").unwrap();

        assert!(
            button.y - panel.y >= 53.0,
            "large titled panel did not reserve font+gap space: panel={panel:?} button={button:?}"
        );
    }

    #[test]
    fn titled_panel_reservation_uses_title_part_typography() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Part-styled title".to_string()),
                ..NodeProps::default()
            },
            vec![node(
                "button",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Child".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.gap = Some(0.0);
        let mut title = crate::style::PartStyle::default();
        title.text.font_size = Some(20.0);
        title.text.line_height = Some(LineHeight::LogicalPx(36.0));
        panel.style.parts.parts.insert("title".to_string(), title);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 240.0, 100.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let button = layout.rects.get("button").unwrap();

        assert_eq!(
            panel_title_line_height_lp(root.children.first().unwrap(), &Theme::dark()),
            36.0
        );
        assert!(
            button.y >= panel.y + 36.0,
            "title-part line-height was not reserved: panel={panel:?} button={button:?}"
        );
    }

    #[test]
    fn panel_header_part_centers_title_and_grows_for_large_fonts() {
        fn layout_for_font(font_size: f32) -> (WidgetNode, LayoutResult) {
            let mut panel = node(
                "panel",
                WidgetKind::Panel,
                NodeProps {
                    text: Some("Centered title".to_string()),
                    ..NodeProps::default()
                },
                vec![node(
                    "child",
                    WidgetKind::Button,
                    NodeProps {
                        text: Some("Body".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                )],
            );
            panel.style.layout.width = Some(260.0);
            panel.style.layout.height = Some(160.0);
            panel.style.layout.padding = Some(8.0);
            panel.style.layout.gap = Some(6.0);
            let mut header = crate::style::PartStyle::default();
            header.layout.height = Some(60.0);
            header.layout.padding = Some(10.0);
            panel.style.parts.parts.insert("header".to_string(), header);
            let mut title = crate::style::PartStyle::default();
            title.text.font_size = Some(font_size);
            panel.style.parts.parts.insert("title".to_string(), title);
            let root = node(
                "window",
                WidgetKind::Window,
                NodeProps::default(),
                vec![panel],
            );
            let layout = compute_layout(&root, 320.0, 220.0, 1.0, &Theme::dark(), None);
            (root, layout)
        }

        for (font_size, expected_header_height) in [(14.0, 60.0), (52.0, 77.0)] {
            let (root, layout) = layout_for_font(font_size);
            let panel = &root.children[0];
            let geometry = titled_container_geometry(panel, &layout, 1.0, &Theme::dark()).unwrap();
            let child = layout.rects["child"];
            let title_center = geometry.title_box.y + geometry.title_box.h * 0.5;
            let header_center = geometry.title_band.y + geometry.title_band.h * 0.5;

            assert!((geometry.title_band.h - expected_header_height).abs() <= 0.5);
            assert!((title_center - header_center).abs() <= 0.5);
            assert!((geometry.title_box.x - (geometry.title_band.x + 10.0)).abs() <= 0.5);
            assert!(
                child.y >= geometry.body_content_origin_y - 0.5,
                "body overlapped header for font size {font_size}: geometry={geometry:?} child={child:?}"
            );
        }
    }

    #[test]
    fn titled_panel_reservation_uses_custom_title_line_height() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Tall title".to_string()),
                ..NodeProps::default()
            },
            vec![node(
                "button",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Child".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.gap = Some(0.0);
        panel.style.layout.height = Some(100.0);
        panel.style.text.line_height = Some(LineHeight::LogicalPx(44.0));
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 240.0, 100.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let button = layout.rects.get("button").unwrap();

        assert_eq!(layout.scroll_max_y.get("panel").copied(), None);
        assert!(
            button.y >= panel.y + 44.0,
            "custom title line-height was not reserved: panel={panel:?} button={button:?}"
        );
    }

    #[test]
    fn status_bar_labels_keep_intrinsic_text_width() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "status",
                WidgetKind::StatusBar,
                NodeProps::default(),
                vec![
                    node(
                        "ready",
                        WidgetKind::Label,
                        NodeProps {
                            text: Some("Ready".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                    node("spacer", WidgetKind::Spacer, NodeProps::default(), vec![]),
                    node(
                        "rows",
                        WidgetKind::Label,
                        NodeProps {
                            text: Some("100,000 rows".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 640.0, 80.0, 1.0, &Theme::dark(), None);
        let ready = layout.rects.get("ready").unwrap();
        let rows = layout.rects.get("rows").unwrap();

        assert!(ready.w > 0.0, "left status label collapsed to {:?}", ready);
        assert!(rows.w > ready.w, "right status label did not size to text");
    }

    #[test]
    fn menu_bar_lays_out_menus_without_popup_children() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "menu-bar",
                WidgetKind::MenuBar,
                NodeProps {
                    fixed_height: Some(32.0),
                    ..NodeProps::default()
                },
                vec![
                    node(
                        "file",
                        WidgetKind::Menu,
                        NodeProps {
                            text: Some("File".to_string()),
                            ..NodeProps::default()
                        },
                        vec![node(
                            "open",
                            WidgetKind::MenuItem,
                            NodeProps {
                                text: Some("Open".to_string()),
                                ..NodeProps::default()
                            },
                            vec![],
                        )],
                    ),
                    node(
                        "help",
                        WidgetKind::Menu,
                        NodeProps {
                            text: Some("Help".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                ],
            )],
        );

        let layout = compute_layout(&root, 640.0, 200.0, 1.0, &Theme::dark(), None);
        let menu_bar = layout.rects.get("menu-bar").unwrap();
        let file = layout.rects.get("file").unwrap();
        let help = layout.rects.get("help").unwrap();
        let file_text_width =
            measure_text_for_layout("File", &crate::style::TextStyle::default(), &Theme::dark())
                .width;

        assert_eq!(menu_bar.h, 32.0);
        assert!(
            file.w - Theme::dark().spacing
                >= file_text_width + MENU_LABEL_WIDTH_SAFETY_LP - 0.5,
            "file menu should retain shaped glyph width and safety inset: rect={file:?} text={file_text_width}"
        );
        assert!(help.x > file.x, "help menu did not flow after file menu");
        assert!(
            !layout.rects.contains_key("open"),
            "menu item should be popup-only, not normal layout"
        );
    }

    #[test]
    fn menu_intrinsic_width_keeps_label_glyphs_inside_clip() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "menu-bar",
                WidgetKind::MenuBar,
                NodeProps::default(),
                vec![node(
                    "debug",
                    WidgetKind::Menu,
                    NodeProps {
                        text: Some("Debug".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                )],
            )],
        );

        let theme = Theme::dark();
        let layout = compute_layout(&root, 240.0, 80.0, 1.0, &theme, None);
        let debug = layout.rects.get("debug").unwrap();
        let text_w =
            measure_text_for_layout("Debug", &crate::style::TextStyle::default(), &theme).width;
        let available_text_w = debug.w - theme.spacing;

        assert!(
            available_text_w >= text_w + MENU_LABEL_WIDTH_SAFETY_LP - 0.5,
            "menu label can clip: rect={debug:?}, available={available_text_w}, estimated={text_w}"
        );
    }

    #[test]
    fn intrinsic_control_width_uses_proportional_shaped_text() {
        let theme = Theme::dark();
        let narrow = node(
            "narrow",
            WidgetKind::Button,
            NodeProps {
                text: Some("iiiiiiiiii".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        let wide = node(
            "wide",
            WidgetKind::Button,
            NodeProps {
                text: Some("WWWWWWWWWW".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );

        let narrow_width = intrinsic_leaf_width(&narrow, &theme).expect("narrow width");
        let wide_width = intrinsic_leaf_width(&wide, &theme).expect("wide width");

        assert!(
            wide_width > narrow_width + 40.0,
            "equal character counts should not receive equal intrinsic widths: narrow={narrow_width} wide={wide_width}"
        );
    }

    // -----------------------------------------------------------------------
    // Flex preferred/minimum sizing and constrained-row contracts
    // -----------------------------------------------------------------------

    #[test]
    fn inline_style_overrides_width_and_gap() {
        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![
                node("panel", WidgetKind::Panel, NodeProps::default(), vec![]),
                node(
                    "scatter",
                    WidgetKind::Scatter3D,
                    NodeProps::default(),
                    vec![],
                ),
            ],
        );
        row.style.layout.gap = Some(16.0);
        row.children[0].style.layout.width = Some(300.0);
        row.children[0].style.layout.flex_grow = Some(0.0);
        row.children[0].style.layout.flex_shrink = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 1000.0, 500.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let scatter = layout.rects.get("scatter").unwrap();

        assert_eq!(panel.w, 300.0);
        assert_eq!(scatter.x, 316.0);
    }

    #[test]
    fn explicit_style_width_does_not_grow_without_explicit_flex_grow() {
        let mut fixed = node("fixed", WidgetKind::Panel, NodeProps::default(), vec![]);
        fixed.style.layout.width = Some(320.0);
        let mut flexible = node("flexible", WidgetKind::Panel, NodeProps::default(), vec![]);
        flexible.style.layout.flex_grow = Some(1.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![fixed, flexible],
            )],
        );

        let layout = compute_layout(&root, 900.0, 400.0, 1.0, &Theme::dark(), None);
        let fixed = layout.rects.get("fixed").unwrap();
        let flexible = layout.rects.get("flexible").unwrap();

        assert_eq!(fixed.w, 320.0);
        assert_eq!(flexible.x, 320.0);
        assert_eq!(flexible.w, 580.0);
    }

    #[test]
    fn ordinary_panel_inside_tall_sidebar_remains_content_sized() {
        let health = node(
            "health",
            WidgetKind::Panel,
            NodeProps {
                text: Some("System health".to_string()),
                ..NodeProps::default()
            },
            vec![node(
                "status-label",
                WidgetKind::Label,
                NodeProps {
                    text: Some("All systems operational".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        let sidebar = node(
            "sidebar",
            WidgetKind::Sidebar,
            NodeProps {
                fixed_width: Some(220.0),
                ..NodeProps::default()
            },
            vec![health],
        );
        let mut body = node("body", WidgetKind::Panel, NodeProps::default(), vec![]);
        body.style.layout.flex_grow = Some(1.0);
        body.style.layout.flex_shrink = Some(1.0);
        let shell = node(
            "shell",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, body],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![shell],
        );

        let layout = compute_layout(&root, 640.0, 720.0, 1.0, &Theme::dark(), None);
        let sidebar = layout.rects.get("sidebar").expect("sidebar rect");
        let health = layout.rects.get("health").expect("health panel rect");
        let label = layout.rects.get("status-label").expect("status label rect");

        assert!(
            sidebar.h >= 700.0,
            "sidebar should fill the window: {sidebar:?}"
        );
        assert!(
            health.h < sidebar.h * 0.35,
            "ordinary sidebar panel absorbed unused height: sidebar={sidebar:?} health={health:?}"
        );
        assert!(
            health.h >= label.h,
            "panel must retain its content: health={health:?} label={label:?}"
        );
    }

    #[test]
    fn search_box_composite_sizing_preserves_preferred_compact_and_growing_modes() {
        fn search_box(id: &str, grow: bool, clearable: bool, border: f32) -> WidgetNode {
            let mut icon = node(
                &format!("{id}-icon"),
                WidgetKind::IconButton,
                NodeProps {
                    fixed_width: Some(28.0),
                    fixed_height: Some(28.0),
                    ..NodeProps::default()
                },
                vec![],
            );
            icon.style.layout.flex_shrink = Some(0.0);

            let mut input = node(
                &format!("{id}-input"),
                WidgetKind::TextInput,
                NodeProps {
                    placeholder: Some("Search routes, owners, or commands".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            );
            input.style.layout.width = Some(0.0);
            input.style.layout.flex_grow = Some(1.0);
            input.style.layout.flex_shrink = Some(1.0);
            input.style.layout.min_width = Some(0.0);

            let mut children = vec![icon, input];
            if clearable {
                let mut clear = node(
                    &format!("{id}-clear"),
                    WidgetKind::IconButton,
                    NodeProps {
                        fixed_width: Some(28.0),
                        fixed_height: Some(28.0),
                        ..NodeProps::default()
                    },
                    vec![],
                );
                clear.style.layout.flex_shrink = Some(0.0);
                children.push(clear);
            }

            let mut search = node(id, WidgetKind::HLayout, NodeProps::default(), children);
            search.style.layout.width = Some(340.0);
            search.style.layout.min_width = Some(180.0);
            search.style.layout.height = Some(38.0);
            search.style.layout.gap = Some(6.0);
            search.style.layout.flex_grow = Some(if grow { 1.0 } else { 0.0 });
            search.style.layout.flex_shrink = Some(1.0);
            search.style.visual.border_width = Some(border);
            search
        }

        fn layout_search(
            width: f32,
            grow: bool,
            clearable: bool,
            border: f32,
            scale_factor: f32,
        ) -> LayoutResult {
            let toolbar = node(
                "toolbar",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![search_box("search", grow, clearable, border)],
            );
            let root = node(
                "window",
                WidgetKind::Window,
                NodeProps::default(),
                vec![toolbar],
            );
            compute_layout(
                &root,
                width * scale_factor,
                100.0 * scale_factor,
                scale_factor,
                &Theme::dark(),
                None,
            )
        }

        let standalone = layout_search(520.0, false, true, 2.0, 1.0);
        assert_eq!(standalone.rects["search"].w, 340.0);
        let search = standalone.rects["search"];
        let icon = standalone.rects["search-icon"];
        let clear = standalone.rects["search-clear"];
        assert_eq!(icon.x, search.x + 2.0);
        assert_eq!(clear.x + clear.w, search.x + search.w - 2.0);
        assert!(
            icon.y >= search.y + 2.0 && icon.y + icon.h <= search.y + search.h - 2.0,
            "search icon must remain inside the bordered content box: search={search:?} icon={icon:?}"
        );
        assert!(
            clear.y >= search.y + 2.0 && clear.y + clear.h <= search.y + search.h - 2.0,
            "clear button must remain inside the bordered content box: search={search:?} clear={clear:?}"
        );

        let compact = layout_search(240.0, false, true, 2.0, 1.0);
        assert_eq!(compact.rects["search"].w, 240.0);
        assert!(
            compact.rects["search-input"].w >= 100.0,
            "default minimum should retain a useful input region: {:?}",
            compact.rects["search-input"]
        );

        let growing = layout_search(520.0, true, true, 2.0, 1.0);
        assert_eq!(growing.rects["search"].w, 520.0);

        let without_clear = layout_search(340.0, false, false, 2.0, 1.0);
        assert_eq!(
            without_clear.rects["search-input"].w,
            standalone.rects["search-input"].w + 34.0,
            "removing clear chrome should release its width and one gap"
        );

        for border in [0.0, 1.0, 2.0, 4.0] {
            for scale_factor in [1.0, 1.25, 2.0] {
                for clearable in [false, true] {
                    let layout = layout_search(180.0, false, clearable, border, scale_factor);
                    let search = layout.rects["search"];
                    for child_id in ["search-icon", "search-input"] {
                        let child = layout.rects[child_id];
                        assert!(
                            child.x >= search.x - 0.5
                                && child.x + child.w <= search.x + search.w + 0.5
                                && child.y >= search.y - 0.5
                                && child.y + child.h <= search.y + search.h + 0.5,
                            "{child_id} escaped compact SearchBox: border={border} scale={scale_factor} search={search:?} child={child:?}"
                        );
                    }
                    if clearable {
                        let clear = layout.rects["search-clear"];
                        assert!(
                            clear.x >= search.x - 0.5
                                && clear.x + clear.w <= search.x + search.w + 0.5
                                && clear.y >= search.y - 0.5
                                && clear.y + clear.h <= search.y + search.h + 0.5,
                            "clear escaped compact SearchBox: border={border} scale={scale_factor} search={search:?} clear={clear:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn asymmetric_visual_borders_reserve_each_content_edge() {
        let mut child = node("child", WidgetKind::Panel, NodeProps::default(), vec![]);
        child.style.layout.flex_grow = Some(1.0);
        child.style.layout.align_self = Some(AlignItemsStyle::Stretch);

        let mut root = node(
            "root",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![child],
        );
        root.style.layout.width = Some(100.0);
        root.style.layout.height = Some(50.0);
        root.style.visual.border_top_width = Some(2.0);
        root.style.visual.border_right_width = Some(3.0);
        root.style.visual.border_bottom_width = Some(5.0);
        root.style.visual.border_left_width = Some(4.0);

        let result = compute_layout(&root, 100.0, 50.0, 1.0, &Theme::dark(), None);
        let root_rect = result.rects["root"];
        let child_rect = result.rects["child"];

        assert!((child_rect.x - (root_rect.x + 4.0)).abs() < 0.01);
        assert!((child_rect.y - (root_rect.y + 2.0)).abs() < 0.01);
        assert!((child_rect.w - 93.0).abs() < 0.01);
        assert!((child_rect.h - 43.0).abs() < 0.01);
    }

    #[test]
    fn patterned_outline_width_and_offset_do_not_affect_layout() {
        let mut child = node("child", WidgetKind::Panel, NodeProps::default(), vec![]);
        child.style.layout.flex_grow = Some(1.0);
        child.style.layout.align_self = Some(AlignItemsStyle::Stretch);

        let mut root = node(
            "root",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![child],
        );
        root.style.layout.width = Some(100.0);
        root.style.layout.height = Some(50.0);
        root.style.visual.outline_width = Some(8.0);
        root.style.visual.outline_offset = Some(6.0);
        root.style.visual.outline_style = Some(crate::style::BorderLineStyle::Dashed);

        let result = compute_layout(&root, 100.0, 50.0, 1.0, &Theme::dark(), None);
        let root_rect = result.rects["root"];
        let child_rect = result.rects["child"];

        assert!((child_rect.x - root_rect.x).abs() < 0.01);
        assert!((child_rect.y - root_rect.y).abs() < 0.01);
        assert!((child_rect.w - root_rect.w).abs() < 0.01);
        assert!((child_rect.h - root_rect.h).abs() < 0.01);
    }

    #[test]
    fn percentage_width_flex_child_can_shrink_after_fixed_sibling() {
        let mut label = node("label", WidgetKind::Label, NodeProps::default(), vec![]);
        label.style.layout.width = Some(172.0);
        label.style.layout.flex_shrink = Some(0.0);

        let mut flow = node(
            "flow",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            vec![
                node(
                    "first",
                    WidgetKind::Tag,
                    NodeProps {
                        text: Some("calibration-waiting".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "second",
                    WidgetKind::Tag,
                    NodeProps {
                        text: Some("manual-override-armed".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
            ],
        );
        flow.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        flow.style.layout.min_width = Some(0.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![label, flow],
        );
        row.style.layout.width = Some(360.0);
        row.style.layout.gap = Some(8.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 500.0, 220.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let flow = layout.rects.get("flow").unwrap();

        assert_eq!(flow.x, row.x + 180.0);
        assert!(
            flow.x + flow.w <= row.x + row.w + 0.5,
            "percentage width flex child should shrink into remaining row space: row={row:?} flow={flow:?}"
        );
    }

    #[test]
    fn percentage_width_grid_shrinks_after_fixed_sibling() {
        let mut sidebar = node("sidebar", WidgetKind::Panel, NodeProps::default(), vec![]);
        sidebar.style.layout.width = Some(180.0);
        sidebar.style.layout.height = Some(120.0);
        sidebar.style.layout.flex_shrink = Some(0.0);

        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps::default(),
            vec![node(
                "cell",
                WidgetKind::Panel,
                NodeProps::default(),
                vec![],
            )],
        );
        grid.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        grid.style.layout.height = Some(120.0);
        grid.style.layout.min_width = Some(0.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, grid],
        );
        row.style.layout.width = Some(360.0);
        row.style.layout.height = Some(120.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 500.0, 220.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let grid = layout.rects.get("grid").unwrap();

        assert_eq!(grid.x, row.x + 180.0);
        assert!(
            grid.x + grid.w <= row.x + row.w + 0.5,
            "percentage grid should shrink into remaining row space: row={row:?} grid={grid:?}"
        );
    }

    #[test]
    fn percentage_width_plot_shrinks_after_fixed_sibling() {
        let mut sidebar = node("sidebar", WidgetKind::Panel, NodeProps::default(), vec![]);
        sidebar.style.layout.width = Some(180.0);
        sidebar.style.layout.height = Some(120.0);
        sidebar.style.layout.flex_shrink = Some(0.0);

        let mut plot = node("plot", WidgetKind::LinePlot, NodeProps::default(), vec![]);
        plot.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        plot.style.layout.height = Some(120.0);
        plot.style.layout.min_width = Some(0.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, plot],
        );
        row.style.layout.width = Some(360.0);
        row.style.layout.height = Some(120.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 500.0, 220.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let plot = layout.rects.get("plot").unwrap();

        assert_eq!(plot.x, row.x + 180.0);
        assert!(
            plot.x + plot.w <= row.x + row.w + 0.5,
            "percentage plot should shrink into remaining row space: row={row:?} plot={plot:?}"
        );
    }

    #[test]
    fn calc_width_grid_shrinks_on_main_axis_at_supported_scales() {
        for scale_factor in [1.0, 1.25, 1.5, 2.0] {
            let mut sidebar = node("sidebar", WidgetKind::Panel, NodeProps::default(), vec![]);
            sidebar.style.layout.width = Some(180.0);
            sidebar.style.layout.height = Some(120.0);
            sidebar.style.layout.flex_shrink = Some(0.0);

            let mut grid = node("grid", WidgetKind::GridLayout, NodeProps::default(), vec![]);
            grid.style.layout.width_value = Some(LayoutLength::Calc(crate::style::CalcLength {
                percent: 100.0,
                px: -12.0,
            }));
            grid.style.layout.height = Some(120.0);

            let mut row = node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![sidebar, grid],
            );
            row.style.layout.width = Some(360.0);
            row.style.layout.height = Some(120.0);
            let root = node(
                "window",
                WidgetKind::Window,
                NodeProps::default(),
                vec![row],
            );

            let layout = compute_layout(
                &root,
                500.0 * scale_factor,
                220.0 * scale_factor,
                scale_factor,
                &Theme::dark(),
                None,
            );
            let row = layout.rects.get("row").unwrap();
            let grid = layout.rects.get("grid").unwrap();

            assert!((grid.x - (row.x + 180.0 * scale_factor)).abs() <= 0.5);
            assert!(
                grid.x + grid.w <= row.x + row.w + 0.5,
                "calc grid should shrink into remaining space at scale {scale_factor}: row={row:?} grid={grid:?}"
            );
        }
    }

    #[test]
    fn explicit_flex_shrink_zero_preserves_intentional_percentage_overflow() {
        let mut sidebar = node("sidebar", WidgetKind::Panel, NodeProps::default(), vec![]);
        sidebar.style.layout.width = Some(180.0);
        sidebar.style.layout.flex_shrink = Some(0.0);

        let mut grid = node("grid", WidgetKind::GridLayout, NodeProps::default(), vec![]);
        grid.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        grid.style.layout.flex_shrink = Some(0.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, grid],
        );
        row.style.layout.width = Some(360.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 500.0, 220.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let grid = layout.rects.get("grid").unwrap();

        assert!(
            grid.x + grid.w > row.x + row.w + 0.5,
            "explicit flex_shrink: 0 should retain the intentional overflow escape hatch: row={row:?} grid={grid:?}"
        );
    }

    #[test]
    fn percentage_limits_bar_shrinks_beside_fixed_row_label() {
        let mut label = node("readout", WidgetKind::Label, NodeProps::default(), vec![]);
        label.style.layout.width = Some(190.0);
        label.style.layout.flex_shrink = Some(0.0);

        let mut bar = node(
            "limits",
            WidgetKind::LimitsBar,
            NodeProps::default(),
            vec![],
        );
        bar.style.layout.width_value = Some(LayoutLength::Percent(100.0));

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![label, bar],
        );
        row.style.layout.width = Some(600.0);
        row.style.layout.gap = Some(12.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 640.0, 180.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let bar = layout.rects.get("limits").unwrap();

        assert!(
            bar.x + bar.w <= row.x + row.w + 0.5,
            "limits bar should consume the remaining row width: row={row:?} bar={bar:?}"
        );
        assert!(
            bar.w >= 120.0,
            "bar should retain its semantic minimum: {bar:?}"
        );
    }

    #[test]
    fn explicit_auto_width_can_still_flex_grow_when_requested() {
        let mut fixed = node("fixed", WidgetKind::Panel, NodeProps::default(), vec![]);
        fixed.style.layout.width = Some(210.0);
        fixed.style.layout.height = Some(120.0);
        fixed.style.layout.flex_shrink = Some(0.0);

        let mut flexible = node("flexible", WidgetKind::Panel, NodeProps::default(), vec![]);
        flexible.style.layout.width_value = Some(LayoutLength::Auto);
        flexible.style.layout.height = Some(120.0);
        flexible.style.layout.flex_grow = Some(1.0);
        flexible.style.layout.flex_shrink = Some(1.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![fixed, flexible],
        );
        row.style.layout.width = Some(600.0);
        row.style.layout.gap = Some(12.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 700.0, 300.0, 1.0, &Theme::dark(), None);
        let fixed = layout.rects.get("fixed").unwrap();
        let flexible = layout.rects.get("flexible").unwrap();

        assert_eq!(fixed.w, 210.0);
        assert!(
            flexible.w > 340.0,
            "explicit width:auto with flex-grow should fill the remaining row: {flexible:?}"
        );
    }

    #[test]
    fn flex_drop_targets_split_available_row_width_without_clipping() {
        let mut left = node("left", WidgetKind::DropTarget, NodeProps::default(), vec![]);
        left.style.layout.flex_grow = Some(1.0);
        left.style.layout.flex_shrink = Some(1.0);
        left.style.layout.flex_basis = Some(0.0);
        left.style.layout.min_width = Some(0.0);
        let mut right = node(
            "right",
            WidgetKind::DropTarget,
            NodeProps::default(),
            vec![],
        );
        right.style.layout.flex_grow = Some(1.0);
        right.style.layout.flex_shrink = Some(1.0);
        right.style.layout.flex_basis = Some(0.0);
        right.style.layout.min_width = Some(0.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![left, right],
        );
        row.style.layout.width = Some(600.0);
        row.style.layout.gap = Some(12.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 700.0, 300.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let left = layout.rects.get("left").unwrap();
        let right = layout.rects.get("right").unwrap();

        assert_eq!(left.w, 294.0);
        assert_eq!(right.w, 294.0);
        assert_eq!(right.x + right.w, row.x + row.w);
    }

    #[test]
    fn explicit_calc_width_children_can_shrink_inside_flex_sized_parent() {
        let mut sidebar = node("sidebar", WidgetKind::Panel, NodeProps::default(), vec![]);
        sidebar.style.layout.width = Some(300.0);
        sidebar.style.layout.flex_grow = Some(0.0);
        sidebar.style.layout.flex_shrink = Some(0.0);

        let mut left = node("left", WidgetKind::DropTarget, NodeProps::default(), vec![]);
        left.style.layout.width_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 50.0,
            px: -6.0,
        }));
        let mut right = node(
            "right",
            WidgetKind::DropTarget,
            NodeProps::default(),
            vec![],
        );
        right.style.layout.width_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 50.0,
            px: -6.0,
        }));

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![left, right],
        );
        row.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        row.style.layout.gap = Some(12.0);

        let mut target_panel = node(
            "target-panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![row],
        );
        target_panel.style.layout.flex_grow = Some(1.0);
        target_panel.style.layout.flex_shrink = Some(1.0);
        target_panel.style.layout.min_width = Some(0.0);
        target_panel.style.layout.padding = Some(0.0);

        let mut shell = node(
            "shell",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, target_panel],
        );
        shell.style.layout.gap = Some(12.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![shell],
        );

        let layout = compute_layout(&root, 980.0, 300.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let left = layout.rects.get("left").unwrap();
        let right = layout.rects.get("right").unwrap();

        assert!(right.x + right.w <= row.x + row.w + 0.5);
        assert!(
            (left.w - right.w).abs() <= 1.0,
            "lane widths should remain balanced: left={left:?} right={right:?}"
        );
    }

    #[test]
    fn zero_flex_basis_fill_slot_uses_remaining_row_width() {
        let mut label = node("label", WidgetKind::Panel, NodeProps::default(), vec![]);
        label.style.layout.width = Some(112.0);
        label.style.layout.flex_grow = Some(0.0);
        label.style.layout.flex_shrink = Some(0.0);

        let mut input = node("input", WidgetKind::TextInput, NodeProps::default(), vec![]);
        input.style.layout.width_value = Some(LayoutLength::Percent(100.0));

        let mut editor = node(
            "editor",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![input],
        );
        editor.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        editor.style.layout.flex_grow = Some(1.0);
        editor.style.layout.flex_shrink = Some(1.0);
        editor.style.layout.flex_basis_value = Some(LayoutLength::LogicalPx(0.0));

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![label, editor],
        );
        row.style.layout.width = Some(366.0);
        row.style.layout.gap = Some(10.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 500.0, 120.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let label = layout.rects.get("label").unwrap();
        let editor = layout.rects.get("editor").unwrap();
        let input = layout.rects.get("input").unwrap();

        assert_eq!(row.w, 366.0);
        assert_eq!(label.w, 112.0);
        assert_eq!(editor.x, label.x + label.w + 10.0);
        assert_eq!(editor.w, 244.0);
        assert_eq!(input.w, 244.0);
        assert!(
            input.x + input.w <= row.x + row.w,
            "flex:1-style slot should not overflow row: row={row:?} input={input:?}"
        );
    }

    #[test]
    fn flex_badges_respect_explicit_min_width_zero_in_row() {
        let mut left = node(
            "left",
            WidgetKind::Badge,
            NodeProps {
                text: Some("ExtensionWidget".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        left.style.layout.flex_grow = Some(1.0);
        left.style.layout.flex_shrink = Some(1.0);
        left.style.layout.flex_basis_value = Some(LayoutLength::LogicalPx(0.0));
        left.style.layout.min_width = Some(0.0);

        let mut right = node(
            "right",
            WidgetKind::Badge,
            NodeProps {
                text: Some("CSS type selector".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        right.style.layout.flex_grow = Some(1.0);
        right.style.layout.flex_shrink = Some(1.0);
        right.style.layout.flex_basis_value = Some(LayoutLength::LogicalPx(0.0));
        right.style.layout.min_width = Some(0.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![left, right],
        );
        row.style.layout.width = Some(220.0);
        row.style.layout.gap = Some(8.0);
        row.style.layout.flex_grow = Some(0.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 320.0, 120.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let left = layout.rects.get("left").unwrap();
        let right = layout.rects.get("right").unwrap();

        assert_eq!(row.w, 220.0);
        assert!(left.w < 120.0, "left badge should shrink: {left:?}");
        assert!(right.w < 120.0, "right badge should shrink: {right:?}");
        assert!(
            right.x + right.w <= row.x + row.w + 0.5,
            "flex badges should stay inside row: row={row:?} left={left:?} right={right:?}"
        );
    }

    #[test]
    fn nonzero_logical_min_width_keeps_control_hit_target_without_text_lock() {
        let mut button = node(
            "button",
            WidgetKind::Button,
            NodeProps {
                text: Some("Deploy diagnostics".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        button.style.layout.min_width = Some(24.0);

        let row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![button],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 60.0, 80.0, 1.0, &Theme::dark(), None);
        let button = layout.rects.get("button").unwrap();

        assert_eq!(
            button.w, 72.0,
            "long text should not raise a button's minimum above its hit target: {button:?}"
        );
    }

    #[test]
    fn percent_min_width_remains_parent_relative_constraint() {
        let mut button = node(
            "button",
            WidgetKind::Button,
            NodeProps {
                text: Some("Deploy diagnostics".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        button.style.layout.min_width_value = Some(LayoutLength::Percent(10.0));

        let row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![button],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 60.0, 80.0, 1.0, &Theme::dark(), None);
        let button = layout.rects.get("button").unwrap();

        assert_eq!(
            button.w, 6.0,
            "percent min-width should remain parent-relative instead of being converted to an intrinsic fixed minimum: {button:?}"
        );
    }

    #[test]
    fn calc_min_width_keeps_control_hit_target_without_text_lock() {
        let mut button = node(
            "button",
            WidgetKind::Button,
            NodeProps {
                text: Some("Deploy diagnostics".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        button.style.layout.min_width_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 5.0,
            px: 8.0,
        }));

        let row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![button],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 60.0, 80.0, 1.0, &Theme::dark(), None);
        let button = layout.rects.get("button").unwrap();

        assert_eq!(
            button.w, 72.0,
            "long text should not raise a button's calc minimum above its hit target: {button:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Positioning, logical lengths, spacing, and splitter contracts
    // -----------------------------------------------------------------------

    #[test]
    fn absolute_position_child_uses_insets_without_consuming_flow() {
        let flow = node("flow", WidgetKind::Label, NodeProps::default(), vec![]);
        let mut pin = node("pin", WidgetKind::Badge, NodeProps::default(), vec![]);
        pin.style.layout.position = Some(PositionStyle::Absolute);
        pin.style.layout.top = Some(14.0);
        pin.style.layout.right = Some(18.0);
        pin.style.layout.width = Some(64.0);
        pin.style.layout.height = Some(20.0);

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![flow, pin],
        );
        panel.style.layout.width = Some(320.0);
        panel.style.layout.height = Some(180.0);
        panel.style.layout.flex_grow = Some(0.0);
        panel.style.layout.flex_shrink = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 640.0, 360.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let flow = layout.rects.get("flow").unwrap();
        let pin = layout.rects.get("pin").unwrap();

        assert_eq!(panel.w, 320.0);
        assert_eq!(panel.h, 180.0);
        assert_eq!(pin.w, 64.0);
        assert_eq!(pin.h, 20.0);
        assert_eq!(pin.x, panel.x + panel.w - 18.0 - pin.w);
        assert_eq!(pin.y, panel.y + 14.0);
        assert!(
            flow.y < pin.y,
            "absolute child should not push the flow child down: flow={flow:?} pin={pin:?}"
        );
    }

    #[test]
    fn absolute_child_in_titled_panel_uses_panel_body_top() {
        let flow = node("flow", WidgetKind::Label, NodeProps::default(), vec![]);
        let mut pin = node("pin", WidgetKind::Badge, NodeProps::default(), vec![]);
        pin.style.layout.position = Some(PositionStyle::Absolute);
        pin.style.layout.top = Some(18.0);
        pin.style.layout.left = Some(16.0);
        pin.style.layout.width = Some(120.0);
        pin.style.layout.height = Some(24.0);

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Titled panel".to_string()),
                ..NodeProps::default()
            },
            vec![flow, pin],
        );
        panel.style.layout.width = Some(320.0);
        panel.style.layout.height = Some(180.0);
        panel.style.layout.flex_grow = Some(0.0);
        panel.style.layout.flex_shrink = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel.clone()],
        );

        let theme = Theme::dark();
        let layout = compute_layout(&root, 640.0, 360.0, 1.0, &theme, None);
        let panel_rect = layout.rects.get("panel").unwrap();
        let pin = layout.rects.get("pin").unwrap();
        let expected_body_top = titled_container_geometry(&panel, &layout, 1.0, &theme)
            .expect("titled panel geometry")
            .body_content_origin_y;

        assert_eq!(pin.x, panel_rect.x + theme.spacing + 2.0 + 16.0);
        assert!(
            (pin.y - (expected_body_top + 18.0)).abs() <= 0.5,
            "absolute child should use the titled panel body origin: pin={pin:?} expected_y={}",
            expected_body_top + 18.0
        );
    }

    #[test]
    fn titled_geometry_aligns_resolved_padding_gap_body_and_absolute_origin() {
        let button = node("button", WidgetKind::Button, NodeProps::default(), vec![]);
        let mut pin = node("pin", WidgetKind::Badge, NodeProps::default(), vec![]);
        pin.style.layout.position = Some(PositionStyle::Absolute);
        pin.style.layout.top = Some(5.0);
        pin.style.layout.left = Some(4.0);
        pin.style.layout.width = Some(40.0);
        pin.style.layout.height = Some(20.0);

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Resolved geometry".to_string()),
                ..NodeProps::default()
            },
            vec![button, pin],
        );
        panel.style.layout.width = Some(280.0);
        panel.style.layout.height = Some(180.0);
        panel.style.layout.padding = Some(12.0);
        panel.style.layout.padding_top_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 0.0,
            px: 18.0,
        }));
        panel.style.layout.gap_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 0.0,
            px: 7.0,
        }));
        panel.style.layout.overflow_y = Some(OverflowStyle::Auto);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let theme = Theme::dark();
        let layout = compute_layout(
            &root,
            360.0,
            220.0,
            1.0,
            &theme,
            Some(&WidgetState::default()),
        );
        let panel_node = root.children.first().expect("panel node");
        let panel_rect = layout.rects.get("panel").expect("panel rect");
        let button = layout.rects.get("button").expect("button rect");
        let pin = layout.rects.get("pin").expect("pin rect");
        let geometry =
            titled_container_geometry(panel_node, &layout, 1.0, &theme).expect("title geometry");

        assert_eq!(geometry.title_box.y, panel_rect.y + 18.0);
        assert_eq!(
            geometry.body_viewport.y,
            geometry.title_box.y + geometry.title_box.h + 7.0
        );
        assert!(
            (button.y - geometry.body_content_origin_y).abs() <= 0.5,
            "first body child should use the shared content origin: button={button:?} geometry={geometry:?}"
        );
        assert!(
            (pin.y - (geometry.body_content_origin_y + 5.0)).abs() <= 0.5,
            "absolute body child should use the same origin: pin={pin:?} geometry={geometry:?}"
        );
    }

    #[test]
    fn compact_empty_titled_panel_keeps_title_inside_header_band() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("A deliberately long compact panel title".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        panel.style.layout.width = Some(220.0);
        panel.style.layout.height = Some(32.0);
        panel.style.layout.flex_grow = Some(0.0);
        panel.style.layout.flex_shrink = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let theme = Theme::dark();
        let layout = compute_layout(
            &root,
            320.0,
            120.0,
            1.0,
            &theme,
            Some(&WidgetState::default()),
        );
        let panel = root.children.first().expect("panel node");
        let panel_rect = layout.rects.get("panel").expect("panel rect");
        let geometry =
            titled_container_geometry(panel, &layout, 1.0, &theme).expect("title geometry");

        assert!(geometry.title_box.y >= panel_rect.y);
        assert!(
            geometry.title_box.y + geometry.title_box.h <= panel_rect.y + panel_rect.h + 0.5,
            "compact title must remain visible: panel={panel_rect:?} geometry={geometry:?}"
        );
        assert!(
            geometry.title_box.h >= panel_title_line_height_lp(panel, &theme) - 0.5,
            "compact panel should retain a complete title line: geometry={geometry:?}"
        );
    }

    #[test]
    fn fixed_position_child_uses_viewport_insets_and_escapes_parent_clip() {
        let flow = node("flow", WidgetKind::Label, NodeProps::default(), vec![]);
        let mut dock = node("dock", WidgetKind::Panel, NodeProps::default(), vec![]);
        dock.style.layout.position = Some(PositionStyle::Fixed);
        dock.style.layout.right = Some(24.0);
        dock.style.layout.bottom = Some(16.0);
        dock.style.layout.width = Some(120.0);
        dock.style.layout.height = Some(32.0);

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![flow, dock],
        );
        panel.style.layout.width = Some(240.0);
        panel.style.layout.height = Some(120.0);
        panel.style.layout.flex_grow = Some(0.0);
        panel.style.layout.flex_shrink = Some(0.0);
        panel.style.layout.overflow = Some(OverflowStyle::Hidden);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 800.0, 600.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let flow = layout.rects.get("flow").unwrap();
        let dock = layout.rects.get("dock").unwrap();
        let dock_clip = layout.clips.get("dock").unwrap();

        assert_eq!(panel.w, 240.0);
        assert_eq!(panel.h, 120.0);
        assert_eq!(dock.w, 120.0);
        assert_eq!(dock.h, 32.0);
        assert_eq!(dock.x, 800.0 - 24.0 - dock.w);
        assert_eq!(dock.y, 600.0 - 16.0 - dock.h);
        assert!(
            flow.y < panel.y + panel.h,
            "fixed child should not affect normal flow: flow={flow:?} dock={dock:?}"
        );
        assert_eq!(dock_clip.x, dock.x);
        assert_eq!(dock_clip.y, dock.y);
        assert_eq!(dock_clip.w, dock.w);
        assert_eq!(dock_clip.h, dock.h);
    }

    #[test]
    fn percent_style_width_uses_parent_space() {
        let mut left = node("left", WidgetKind::Panel, NodeProps::default(), vec![]);
        left.style.layout.width_value = Some(LayoutLength::Percent(50.0));
        let right = node("right", WidgetKind::Panel, NodeProps::default(), vec![]);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![left, right],
            )],
        );

        let layout = compute_layout(&root, 800.0, 400.0, 1.0, &Theme::dark(), None);
        let left = layout.rects.get("left").unwrap();

        assert_eq!(left.w, 400.0);
    }

    #[test]
    fn calc_style_width_lowers_when_expression_is_single_unit() {
        let mut left = node("left", WidgetKind::Panel, NodeProps::default(), vec![]);
        left.style.layout.width_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 0.0,
            px: 280.0,
        }));
        let mut right = node("right", WidgetKind::Panel, NodeProps::default(), vec![]);
        right.style.layout.width_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 50.0,
            px: 0.0,
        }));
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![left, right],
            )],
        );

        let layout = compute_layout(&root, 800.0, 400.0, 1.0, &Theme::dark(), None);
        let left = layout.rects.get("left").unwrap();
        let right = layout.rects.get("right").unwrap();

        assert_eq!(left.w, 280.0);
        assert_eq!(right.w, 400.0);
    }

    #[test]
    fn mixed_calc_style_width_resolves_against_definite_parent_space() {
        let mut left = node("left", WidgetKind::Panel, NodeProps::default(), vec![]);
        left.style.layout.width_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 100.0,
            px: -240.0,
        }));
        let mut right = node("right", WidgetKind::Panel, NodeProps::default(), vec![]);
        right.style.layout.width_value = Some(LayoutLength::LogicalPx(240.0));
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![left, right],
            )],
        );

        let layout = compute_layout(&root, 800.0, 400.0, 1.0, &Theme::dark(), None);
        let left = layout.rects.get("left").unwrap();
        let right = layout.rects.get("right").unwrap();

        assert_eq!(left.w, 560.0);
        assert_eq!(right.w, 240.0);
    }

    #[test]
    fn fractional_pane_sizes_distribute_splitter_space() {
        let left = node(
            "left",
            WidgetKind::Pane,
            NodeProps {
                orientation: Some("horizontal".to_string()),
                pane_size: Some(0.7),
                pane_min_size: Some(360.0),
                ..NodeProps::default()
            },
            vec![node(
                "left-panel",
                WidgetKind::Panel,
                NodeProps::default(),
                vec![],
            )],
        );
        let right = node(
            "right",
            WidgetKind::Pane,
            NodeProps {
                orientation: Some("horizontal".to_string()),
                pane_size: Some(0.3),
                pane_min_size: Some(280.0),
                ..NodeProps::default()
            },
            vec![node(
                "right-panel",
                WidgetKind::Panel,
                NodeProps::default(),
                vec![],
            )],
        );
        let mut splitter = node(
            "splitter",
            WidgetKind::Splitter,
            NodeProps {
                orientation: Some("horizontal".to_string()),
                gutter_size: Some(6.0),
                ..NodeProps::default()
            },
            vec![left, right],
        );
        splitter.style.layout.width = Some(1000.0);
        splitter.style.layout.height = Some(240.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![splitter],
        );

        let layout = compute_layout(&root, 1000.0, 260.0, 1.0, &Theme::dark(), None);
        let splitter = layout.rects.get("splitter").unwrap();
        let left = layout.rects.get("left").unwrap();
        let right = layout.rects.get("right").unwrap();
        let consumed = right.x + right.w - left.x;

        assert!(
            left.w > 360.0 && right.w > 280.0,
            "fractional pane sizes should flex beyond min sizes: left={left:?} right={right:?}"
        );
        assert!(
            (consumed - splitter.w).abs() <= 1.0,
            "splitter panes should consume available width: splitter={splitter:?} left={left:?} right={right:?}"
        );
        assert!(
            left.w > right.w,
            "larger fractional pane should receive more width: left={left:?} right={right:?}"
        );
    }

    #[test]
    fn percent_and_calc_spacing_values_lower_to_taffy() {
        let mut first = node("first", WidgetKind::Panel, NodeProps::default(), vec![]);
        first.style.layout.width_value = Some(LayoutLength::LogicalPx(50.0));
        first.style.layout.flex_grow = Some(0.0);
        first.style.layout.flex_shrink = Some(0.0);
        let mut second = node("second", WidgetKind::Panel, NodeProps::default(), vec![]);
        second.style.layout.width_value = Some(LayoutLength::LogicalPx(50.0));
        second.style.layout.flex_grow = Some(0.0);
        second.style.layout.flex_shrink = Some(0.0);
        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![first, second],
        );
        row.style.layout.padding_left_value = Some(LayoutLength::Percent(10.0));
        row.style.layout.padding_right_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 5.0,
            px: 10.0,
        }));
        row.style.layout.gap_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 5.0,
            px: 10.0,
        }));
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 400.0, 160.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();

        assert_eq!(first.x, 40.0);
        assert_eq!(second.x, 120.0);
    }

    #[test]
    fn padding_longhand_overrides_legacy_shorthand_after_type_lowering() {
        let mut fixed = node("fixed", WidgetKind::Panel, NodeProps::default(), vec![]);
        fixed.style.layout.width = Some(50.0);
        fixed.style.layout.flex_grow = Some(0.0);
        fixed.style.layout.flex_shrink = Some(0.0);
        let mut flexible = node("flexible", WidgetKind::Panel, NodeProps::default(), vec![]);
        flexible.style.layout.flex_grow = Some(1.0);
        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![fixed, flexible],
        );
        row.style.layout.width = Some(300.0);
        row.style.layout.padding = Some(16.0);
        row.style.layout.padding_right = Some(20.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 360.0, 160.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let fixed = layout.rects.get("fixed").unwrap();
        let flexible = layout.rects.get("flexible").unwrap();

        assert_eq!(fixed.x, row.x + 16.0);
        assert_eq!(flexible.x + flexible.w, row.x + row.w - 20.0);
    }

    #[test]
    fn uniform_auto_margin_lowers_to_taffy() {
        let mut centered = node("centered", WidgetKind::Panel, NodeProps::default(), vec![]);
        centered.style.layout.width_value = Some(LayoutLength::LogicalPx(120.0));
        centered.style.layout.height_value = Some(LayoutLength::LogicalPx(40.0));
        centered.style.layout.margin_value = Some(LayoutLength::Auto);
        centered.style.layout.flex_grow = Some(0.0);
        centered.style.layout.flex_shrink = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![centered],
            )],
        );

        let layout = compute_layout(&root, 400.0, 160.0, 1.0, &Theme::dark(), None);
        let centered = layout.rects.get("centered").unwrap();

        assert_eq!(centered.x, 140.0);
    }

    #[test]
    fn margin_edges_lower_to_taffy() {
        let mut first = node("first", WidgetKind::Panel, NodeProps::default(), vec![]);
        first.style.layout.width_value = Some(LayoutLength::LogicalPx(50.0));
        first.style.layout.height_value = Some(LayoutLength::LogicalPx(20.0));
        first.style.layout.margin_right_value = Some(LayoutLength::LogicalPx(10.0));
        first.style.layout.flex_grow = Some(0.0);
        first.style.layout.flex_shrink = Some(0.0);

        let mut second = node("second", WidgetKind::Panel, NodeProps::default(), vec![]);
        second.style.layout.width_value = Some(LayoutLength::LogicalPx(50.0));
        second.style.layout.height_value = Some(LayoutLength::LogicalPx(20.0));
        second.style.layout.margin_left_value = Some(LayoutLength::LogicalPx(20.0));
        second.style.layout.flex_grow = Some(0.0);
        second.style.layout.flex_shrink = Some(0.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![first, second],
            )],
        );

        let layout = compute_layout(&root, 400.0, 160.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();

        assert_eq!(first.x, 0.0);
        assert_eq!(second.x, 80.0);
    }

    // -----------------------------------------------------------------------
    // Grid, masonry, and deterministic reconciliation contracts
    // -----------------------------------------------------------------------

    #[test]
    fn grid_layout_places_children_on_template_tracks() {
        let mut grid = node("grid", WidgetKind::Panel, NodeProps::default(), vec![]);
        grid.style.layout.display = Some(DisplayStyle::Grid);
        grid.style.layout.width_value = Some(LayoutLength::LogicalPx(600.0));
        grid.style.layout.height_value = Some(LayoutLength::LogicalPx(220.0));
        grid.style.layout.padding = Some(0.0);
        grid.style.layout.gap = Some(0.0);
        grid.style.layout.grid_template_columns = Some(vec![
            GridTrackSize::LogicalPx(180.0),
            GridTrackSize::Fraction(1.0),
            GridTrackSize::Fraction(2.0),
        ]);
        grid.style.layout.grid_template_rows = Some(vec![
            GridTrackSize::LogicalPx(80.0),
            GridTrackSize::LogicalPx(120.0),
        ]);

        let mut sidebar = node("sidebar", WidgetKind::Panel, NodeProps::default(), vec![]);
        sidebar.style.layout.grid_column = Some(GridPlacementStyle {
            start: GridLineStyle::Line(1),
            end: GridLineStyle::Line(2),
        });
        sidebar.style.layout.grid_row = Some(GridPlacementStyle {
            start: GridLineStyle::Line(1),
            end: GridLineStyle::Span(2),
        });
        let mut main = node("main", WidgetKind::Panel, NodeProps::default(), vec![]);
        main.style.layout.grid_column = Some(GridPlacementStyle {
            start: GridLineStyle::Line(2),
            end: GridLineStyle::Line(4),
        });
        main.style.layout.grid_row = Some(GridPlacementStyle {
            start: GridLineStyle::Line(1),
            end: GridLineStyle::Line(2),
        });
        grid.children = vec![sidebar, main];
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 800.0, 400.0, 1.0, &Theme::dark(), None);
        let sidebar = layout.rects.get("sidebar").unwrap();
        let main = layout.rects.get("main").unwrap();

        assert_eq!(sidebar.w, 180.0);
        assert_eq!(sidebar.h, 200.0);
        assert_eq!(main.x, 180.0);
        assert_eq!(main.w, 420.0);
        assert_eq!(main.h, 80.0);
    }

    #[test]
    fn grid_layout_uses_max_columns_when_min_tracks_fit() {
        let props = NodeProps {
            grid_columns: Some(2),
            grid_min_column_width: Some(240.0),
            ..NodeProps::default()
        };
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![
                node("first", WidgetKind::Panel, NodeProps::default(), vec![]),
                node("second", WidgetKind::Panel, NodeProps::default(), vec![]),
            ],
        );
        grid.style.layout.gap = Some(20.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 520.0, 220.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();
        let grid = layout.rects.get("grid").unwrap();

        assert!(
            second.x > first.x,
            "second panel should be in the next column"
        );
        assert_eq!(second.y, first.y);
        assert!(first.w <= grid.w);
        assert!(second.w <= grid.w);
    }

    #[test]
    fn responsive_grid_breakpoints_use_logical_viewport_width_and_report_tracks() {
        let props = NodeProps {
            grid_columns: Some(4),
            grid_column_breakpoints: vec![
                crate::document::GridColumnBreakpoint {
                    max_width: 700.0,
                    columns: 1,
                },
                crate::document::GridColumnBreakpoint {
                    max_width: 1100.0,
                    columns: 2,
                },
            ],
            grid_min_column_width: None,
            ..NodeProps::default()
        };
        let cards = (0..4)
            .map(|index| {
                node(
                    &format!("card-{index}"),
                    WidgetKind::Panel,
                    NodeProps::default(),
                    vec![],
                )
            })
            .collect();
        let grid = node("grid", WidgetKind::GridLayout, props, cards);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        for (physical_width, scale_factor, expected) in [
            (2400.0, 2.0, 4usize),
            (2000.0, 2.0, 2usize),
            (1200.0, 2.0, 1usize),
            (1000.0, 1.0, 2usize),
        ] {
            let layout = compute_layout(
                &root,
                physical_width,
                800.0,
                scale_factor,
                &Theme::dark(),
                None,
            );
            assert_eq!(
                layout.resolved_grid_tracks["grid"].column_count, expected,
                "physical={physical_width} scale={scale_factor}"
            );
            assert_eq!(
                layout.resolved_grid_tracks["grid"].column_widths.len(),
                expected
            );
            assert!(layout.reconciliation_converged);
        }
    }

    #[test]
    fn grid_auto_fit_props_lower_to_auto_fit_tracks() {
        let grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_min_column_width: Some(210.0),
                grid_auto_fit: true,
                ..NodeProps::default()
            },
            vec![],
        );
        let mut style = Style::default();
        apply_grid_layout_default_tracks(&mut style, &grid, 1.0, Some((900.0, 400.0)), 900.0);
        assert!(matches!(
            style.grid_template_columns.as_slice(),
            [TrackSizingFunction::Repeat(GridTrackRepetition::AutoFit, _)]
        ));
    }

    #[test]
    fn responsive_masonry_and_nested_grids_reconcile_after_breakpoint_resize() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut card = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            card.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            card
        }
        let responsive = || NodeProps {
            grid_columns: Some(3),
            grid_column_breakpoints: vec![crate::document::GridColumnBreakpoint {
                max_width: 700.0,
                columns: 1,
            }],
            grid_min_column_width: None,
            ..NodeProps::default()
        };
        let mut inner_props = responsive();
        inner_props.grid_masonry = true;
        let inner = node(
            "inner",
            WidgetKind::GridLayout,
            inner_props,
            vec![card("short", 40.0), card("tall", 100.0), card("tail", 40.0)],
        );
        let outer = node(
            "outer",
            WidgetKind::GridLayout,
            responsive(),
            vec![inner, card("peer", 80.0)],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![outer],
        );

        let wide = compute_layout(&root, 1200.0, 800.0, 1.0, &Theme::dark(), None);
        let narrow = compute_layout(&root, 640.0, 800.0, 1.0, &Theme::dark(), None);
        assert_eq!(wide.resolved_grid_tracks["outer"].column_count, 2);
        assert_eq!(wide.resolved_grid_tracks["inner"].column_count, 3);
        assert_eq!(narrow.resolved_grid_tracks["outer"].column_count, 1);
        assert_eq!(narrow.resolved_grid_tracks["inner"].column_count, 1);
        assert!(wide.reconciliation_converged);
        assert!(narrow.reconciliation_converged);
        assert_eq!(narrow.rects["short"].x, narrow.rects["tail"].x);
    }

    #[test]
    fn grid_last_row_balance_centers_an_incomplete_row() {
        let props = NodeProps {
            grid_columns: Some(3),
            grid_min_column_width: None,
            grid_balance_last_row: true,
            ..NodeProps::default()
        };
        let cards = (0..4)
            .map(|index| {
                node(
                    &format!("card-{index}"),
                    WidgetKind::Panel,
                    NodeProps::default(),
                    vec![],
                )
            })
            .collect();
        let grid = node("grid", WidgetKind::GridLayout, props, cards);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );
        let layout = compute_layout(&root, 900.0, 400.0, 1.0, &Theme::dark(), None);
        let grid = layout.rects["grid"];
        let orphan = layout.rects["card-3"];
        assert!((orphan.x + orphan.w * 0.5 - (grid.x + grid.w * 0.5)).abs() <= 0.5);
        assert_eq!(layout.resolved_grid_tracks["grid"].column_count, 3);
    }

    #[test]
    fn grid_layout_masonry_packs_children_into_shortest_column() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }

        let props = NodeProps {
            grid_columns: Some(2),
            grid_min_column_width: Some(120.0),
            grid_masonry: true,
            ..NodeProps::default()
        };
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![
                card("tall", 100.0),
                card("short", 40.0),
                card("packed", 40.0),
                card("tail", 40.0),
            ],
        );
        grid.style.layout.gap = Some(10.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 260.0, 400.0, 1.0, &Theme::dark(), None);
        let tall = layout.rects.get("tall").unwrap();
        let short = layout.rects.get("short").unwrap();
        let packed = layout.rects.get("packed").unwrap();
        let tail = layout.rects.get("tail").unwrap();
        let grid = layout.rects.get("grid").unwrap();

        assert_eq!(short.y, tall.y);
        assert_eq!(packed.x, short.x);
        assert!(
            packed.y < tall.y + tall.h,
            "masonry child should pack under the shorter card: tall={tall:?} packed={packed:?}"
        );
        assert_eq!(tail.x, short.x);
        assert!(tail.y > packed.y);
        assert!(
            grid.h < 200.0,
            "masonry grid should shrink below aligned row height: {grid:?}"
        );
        assert!(layout.reconciliation_converged);
        assert_eq!(layout.reconciliation_iterations, 1);
    }

    #[test]
    fn grid_layout_masonry_repacks_asymmetric_cards_after_collapsing_to_one_column() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }

        let props = NodeProps {
            grid_columns: Some(3),
            grid_min_column_width: Some(180.0),
            grid_masonry: true,
            ..NodeProps::default()
        };
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![
                card("first", 90.0),
                card("second", 180.0),
                card("third", 70.0),
            ],
        );
        grid.style.layout.gap = Some(12.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 210.0, 500.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();
        let third = layout.rects.get("third").unwrap();

        assert_eq!(first.x, second.x);
        assert_eq!(second.x, third.x);
        assert!(second.y >= first.y + first.h + 11.5);
        assert!(third.y >= second.y + second.h + 11.5);
        assert!(layout.reconciliation_converged);
    }

    #[test]
    fn grid_layout_masonry_uses_natural_card_heights() {
        fn card(id: &str, child_count: usize) -> WidgetNode {
            let children = (0..child_count)
                .map(|index| {
                    let mut label = node(
                        &format!("{id}-label-{index}"),
                        WidgetKind::Label,
                        NodeProps {
                            text: Some(format!("row {index}")),
                            ..NodeProps::default()
                        },
                        vec![],
                    );
                    label.style.layout.height = Some(22.0);
                    label.style.layout.flex_shrink = Some(0.0);
                    label
                })
                .collect();
            node(id, WidgetKind::Panel, NodeProps::default(), children)
        }

        let props = NodeProps {
            grid_columns: Some(2),
            grid_min_column_width: Some(120.0),
            grid_masonry: true,
            ..NodeProps::default()
        };
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![card("short", 1), card("tall", 5), card("packed", 1)],
        );
        grid.style.layout.gap = Some(10.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 260.0, 420.0, 1.0, &Theme::dark(), None);
        let short = layout.rects.get("short").unwrap();
        let tall = layout.rects.get("tall").unwrap();
        let packed = layout.rects.get("packed").unwrap();

        assert!(
            short.h < tall.h,
            "masonry cards should keep natural heights: short={short:?} tall={tall:?}"
        );
        assert_eq!(packed.x, short.x);
        assert!(
            packed.y < tall.y + tall.h,
            "packed card should move under short card instead of waiting for tall row: tall={tall:?} packed={packed:?}"
        );
    }

    #[test]
    fn nested_masonry_grids_reconcile_until_parent_geometry_is_stable() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }
        fn masonry(id: &str, children: Vec<WidgetNode>) -> WidgetNode {
            let mut grid = node(
                id,
                WidgetKind::GridLayout,
                NodeProps {
                    grid_columns: Some(2),
                    grid_min_column_width: Some(80.0),
                    grid_masonry: true,
                    ..NodeProps::default()
                },
                children,
            );
            grid.style.layout.gap = Some(10.0);
            grid
        }

        let inner = masonry(
            "inner",
            vec![
                card("inner-tall", 100.0),
                card("inner-short", 40.0),
                card("inner-packed", 40.0),
                card("inner-tail", 40.0),
            ],
        );
        let mut outer = masonry(
            "outer",
            vec![inner, card("outer-short", 40.0), card("outer-tail", 40.0)],
        );
        outer.style.layout.display = Some(DisplayStyle::Grid);
        // Model the parent-first geometry produced before the masonry pass:
        // the inner grid still has aligned-row height, so the outer grid packs
        // against 200 px. The inner pass then shrinks it to 140 px, requiring
        // a second parent round.
        let mut layout = LayoutResult {
            scale_factor: 1.0,
            reconciliation_converged: true,
            ..LayoutResult::default()
        };
        for (id, rect) in [
            (
                "outer",
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 520.0,
                    h: 250.0,
                },
            ),
            (
                "inner",
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 250.0,
                    h: 200.0,
                },
            ),
            (
                "outer-short",
                Rect {
                    x: 260.0,
                    y: 0.0,
                    w: 250.0,
                    h: 40.0,
                },
            ),
            (
                "outer-tail",
                Rect {
                    x: 0.0,
                    y: 210.0,
                    w: 250.0,
                    h: 40.0,
                },
            ),
            (
                "inner-tall",
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 110.0,
                    h: 100.0,
                },
            ),
            (
                "inner-short",
                Rect {
                    x: 120.0,
                    y: 0.0,
                    w: 110.0,
                    h: 40.0,
                },
            ),
            (
                "inner-packed",
                Rect {
                    x: 0.0,
                    y: 110.0,
                    w: 110.0,
                    h: 40.0,
                },
            ),
            (
                "inner-tail",
                Rect {
                    x: 120.0,
                    y: 110.0,
                    w: 110.0,
                    h: 40.0,
                },
            ),
        ] {
            layout.rects.insert(id.to_string(), rect);
        }

        apply_grid_auto_row_positions(&outer, &mut layout, 1.0, &Theme::dark());
        let outer_rect = layout.rects.get("outer").unwrap();
        let inner_rect = layout.rects.get("inner").unwrap();
        let outer_tail = layout.rects.get("outer-tail").unwrap();

        assert!(layout.reconciliation_converged);
        assert!(
            layout.reconciliation_iterations >= 2,
            "nested child packing should trigger a parent reconciliation round: {}",
            layout.reconciliation_iterations
        );
        assert!(inner_rect.y + inner_rect.h <= outer_rect.y + outer_rect.h + 0.5);
        assert!(outer_tail.y + outer_tail.h <= outer_rect.y + outer_rect.h + 0.5);
    }

    #[test]
    fn masonry_height_change_realigns_centered_row_children() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }

        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(100.0),
                grid_masonry: true,
                ..NodeProps::default()
            },
            vec![
                card("tall", 100.0),
                card("short", 40.0),
                card("packed", 40.0),
                card("tail", 40.0),
            ],
        );
        grid.style.layout.width_value = Some(LayoutLength::LogicalPx(300.0));
        grid.style.layout.gap = Some(10.0);

        let mut sibling = card("sibling", 40.0);
        sibling.style.layout.width_value = Some(LayoutLength::LogicalPx(80.0));

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps {
                fixed_height: Some(200.0),
                ..NodeProps::default()
            },
            vec![grid, sibling],
        );
        row.style.layout.align_items = Some(AlignItemsStyle::Center);
        row.style.layout.gap = Some(10.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 420.0, 240.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let grid = layout.rects.get("grid").unwrap();
        let sibling = layout.rects.get("sibling").unwrap();
        let centered_y = |height: f32| row.y + (row.h - height) * 0.5;

        assert!(
            (grid.y - centered_y(grid.h)).abs() <= 0.5,
            "packed grid should be recentered from its final height: row={row:?} grid={grid:?}"
        );
        assert!(
            (sibling.y - centered_y(sibling.h)).abs() <= 0.5,
            "row sibling should retain center alignment: row={row:?} sibling={sibling:?}"
        );
        assert!(layout.reconciliation_converged);
    }

    #[test]
    fn masonry_packing_preserves_auto_height_grid_stretched_by_row() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }

        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(100.0),
                grid_masonry: true,
                ..NodeProps::default()
            },
            vec![
                card("tall", 100.0),
                card("short", 40.0),
                card("packed", 40.0),
                card("tail", 40.0),
            ],
        );
        grid.style.layout.width_value = Some(LayoutLength::LogicalPx(300.0));
        grid.style.layout.gap = Some(10.0);
        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps {
                fixed_height: Some(200.0),
                ..NodeProps::default()
            },
            vec![grid],
        );
        row.style.layout.flex_grow = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 420.0, 240.0, 1.0, &Theme::dark(), None);
        let row = layout.rects.get("row").unwrap();
        let grid = layout.rects.get("grid").unwrap();
        let packed = layout.rects.get("packed").unwrap();
        let tall = layout.rects.get("tall").unwrap();

        assert!(
            (grid.h - row.h).abs() <= 0.5,
            "auto-height grid should retain the row's stretch constraint: row={row:?} grid={grid:?}"
        );
        assert!(
            packed.y < tall.y + tall.h,
            "retaining the stretched outer box must not disable masonry content packing"
        );
        assert!(layout.reconciliation_converged);
    }

    #[test]
    fn masonry_grid_preserves_vertical_splitter_allocation() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }

        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(100.0),
                grid_masonry: true,
                ..NodeProps::default()
            },
            vec![
                card("tall", 100.0),
                card("short", 40.0),
                card("packed", 40.0),
                card("tail", 40.0),
            ],
        );
        grid.style.layout.gap = Some(10.0);
        let lower = node(
            "lower",
            WidgetKind::Pane,
            NodeProps {
                orientation: Some("vertical".to_string()),
                pane_flex: Some(1.0),
                ..NodeProps::default()
            },
            vec![],
        );
        let splitter = node(
            "splitter",
            WidgetKind::Splitter,
            NodeProps {
                orientation: Some("vertical".to_string()),
                gutter_size: Some(6.0),
                fixed_height: Some(300.0),
                ..NodeProps::default()
            },
            vec![grid, lower],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![splitter],
        );

        let layout = compute_layout(&root, 420.0, 340.0, 1.0, &Theme::dark(), None);
        let splitter = layout.rects.get("splitter").unwrap();
        let grid = layout.rects.get("grid").unwrap();
        let lower = layout.rects.get("lower").unwrap();
        let packed = layout.rects.get("packed").unwrap();
        let tall = layout.rects.get("tall").unwrap();

        assert!(
            ((lower.y + lower.h) - (splitter.y + splitter.h)).abs() <= 0.5,
            "vertical splitter children should retain the full allocation: splitter={splitter:?} grid={grid:?} lower={lower:?}"
        );
        assert!(
            (lower.y - (grid.y + grid.h + 6.0)).abs() <= 0.5,
            "masonry shrink must not open a gap before the following split region"
        );
        assert!(packed.y < tall.y + tall.h);
        assert!(layout.reconciliation_converged);
    }

    #[test]
    fn masonry_grid_shrink_updates_auto_height_parent_panel() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }

        let mut caption = node(
            "caption",
            WidgetKind::Label,
            NodeProps {
                text: Some("Uneven cards should keep the parent compact.".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        caption.style.layout.height = Some(22.0);
        caption.style.layout.flex_shrink = Some(0.0);

        let props = NodeProps {
            grid_columns: Some(2),
            grid_min_column_width: Some(120.0),
            grid_masonry: true,
            ..NodeProps::default()
        };
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![
                card("tall", 100.0),
                card("short", 40.0),
                card("packed", 40.0),
                card("tail", 40.0),
            ],
        );
        grid.style.layout.gap = Some(10.0);

        let mut section = node(
            "section",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Masonry card packing".to_string()),
                ..NodeProps::default()
            },
            vec![caption, grid],
        );
        section.style.layout.padding = Some(10.0);
        section.style.layout.gap = Some(8.0);

        let mut footer = node(
            "footer",
            WidgetKind::Label,
            NodeProps {
                text: Some("After section".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        footer.style.layout.height = Some(24.0);
        footer.style.layout.flex_shrink = Some(0.0);

        let mut stack = node(
            "stack",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![section, footer],
        );
        stack.style.layout.height_value = Some(LayoutLength::Percent(100.0));
        stack.style.layout.gap = Some(10.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![stack],
        );

        let layout = compute_layout(&root, 300.0, 520.0, 1.0, &Theme::dark(), None);
        let section = layout.rects.get("section").unwrap();
        let grid = layout.rects.get("grid").unwrap();
        let footer = layout.rects.get("footer").unwrap();

        let expected_section_bottom = grid.y + grid.h + 10.0;
        assert!(
            ((section.y + section.h) - expected_section_bottom).abs() <= 1.0,
            "auto-height parent panel should hug packed masonry content: section={section:?} grid={grid:?}"
        );
        assert!(
            (footer.y - (section.y + section.h + 10.0)).abs() <= 1.0,
            "following siblings should move up after masonry shrink: section={section:?} footer={footer:?}"
        );
    }

    #[test]
    fn grid_layout_collapses_to_one_column_when_min_tracks_do_not_fit() {
        let props = NodeProps {
            grid_columns: Some(2),
            grid_min_column_width: Some(240.0),
            ..NodeProps::default()
        };
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![
                node("first", WidgetKind::Panel, NodeProps::default(), vec![]),
                node("second", WidgetKind::Panel, NodeProps::default(), vec![]),
            ],
        );
        grid.style.layout.gap = Some(20.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 470.0, 260.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();
        let grid = layout.rects.get("grid").unwrap();

        assert_eq!(second.x, first.x);
        assert!(
            second.y > first.y,
            "second panel should wrap below the first"
        );
        assert!(first.w <= grid.w);
        assert!(second.w <= grid.w);
    }

    #[test]
    fn grid_min_column_width_yields_to_a_narrower_container() {
        let props = NodeProps {
            grid_columns: Some(2),
            grid_min_column_width: Some(240.0),
            ..NodeProps::default()
        };
        let grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![node(
                "child",
                WidgetKind::Panel,
                NodeProps::default(),
                vec![],
            )],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 180.0, 220.0, 1.0, &Theme::dark(), None);
        let child = layout.rects.get("child").unwrap();
        let grid = layout.rects.get("grid").unwrap();

        assert!(
            child.w <= grid.w,
            "a responsive grid minimum must not force its only track beyond the container: child={child:?} grid={grid:?}"
        );
    }

    #[test]
    fn grid_layout_props_template_columns_keep_compact_tracks() {
        let props = NodeProps {
            grid_template_columns: Some(vec![
                GridTrackSize::LogicalPx(44.0),
                GridTrackSize::Fraction(1.0),
            ]),
            ..NodeProps::default()
        };
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![
                node("key", WidgetKind::Label, NodeProps::default(), vec![]),
                node("value", WidgetKind::Label, NodeProps::default(), vec![]),
            ],
        );
        grid.style.layout.gap = Some(6.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 240.0, 120.0, 1.0, &Theme::dark(), None);
        let key = layout.rects.get("key").unwrap();
        let value = layout.rects.get("value").unwrap();

        assert_eq!(key.w, 44.0);
        assert_eq!(value.x, key.x + 50.0);
        assert!(value.w > key.w);
    }

    #[test]
    fn taffy_auto_rows_own_asymmetric_grid_heights_without_repacking() {
        fn item(id: &str, height: f32) -> WidgetNode {
            let mut item = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            item.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            item
        }
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(100.0),
                ..NodeProps::default()
            },
            vec![
                item("first", 100.0),
                item("second", 40.0),
                item("third", 30.0),
                item("fourth", 50.0),
            ],
        );
        grid.style.layout.gap = Some(10.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 320.0, 240.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();
        let third = layout.rects.get("third").unwrap();
        let fourth = layout.rects.get("fourth").unwrap();

        assert_eq!(first.y, second.y);
        assert!(
            third.y + 0.5 >= first.y + first.h.max(second.h) + 10.0,
            "Taffy should place the second row after the tallest first-row item while retaining any distributed auto-track space: first={first:?} second={second:?} third={third:?} fourth={fourth:?}"
        );
        assert_eq!(third.y, fourth.y);
        assert_eq!(
            layout.reconciliation_iterations, 0,
            "ordinary auto-row grids should not enter post-layout reconciliation"
        );
    }

    #[test]
    fn auto_height_panel_in_grid_preserves_bottom_padding_after_multiline_editor() {
        let mut code = node(
            "code",
            WidgetKind::CodeEditor,
            NodeProps {
                rows: Some(4),
                text: Some("Panel.card {\n  padding: 12px;\n}".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        code.style.layout.flex_shrink = Some(0.0);

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Text entry".to_string()),
                ..NodeProps::default()
            },
            vec![
                node("first", WidgetKind::TextInput, NodeProps::default(), vec![]),
                node(
                    "area",
                    WidgetKind::TextArea,
                    NodeProps {
                        rows: Some(3),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                code,
            ],
        );
        panel.style.layout.padding = Some(15.0);
        panel.style.layout.gap = Some(10.0);
        panel.style.visual.border_bottom_width = Some(9.0);

        let grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(420.0),
                ..NodeProps::default()
            },
            vec![panel],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let mut layout = compute_layout(&root, 750.0, 700.0, 1.0, &Theme::dark(), None);
        let code_bottom = {
            let code = layout.rects.get("code").unwrap();
            code.y + code.h
        };
        let panel_y = layout.rects.get("panel").unwrap().y;
        layout.rects.get_mut("panel").unwrap().h = code_bottom - panel_y + 1.0;
        assert!(resize_auto_height_container_to_children(
            &root.children[0].children[0],
            &mut layout,
            1.0,
            None,
        ));
        let panel = layout.rects.get("panel").unwrap();
        let code = layout.rects.get("code").unwrap();
        let bottom_gap = panel.y + panel.h - (code.y + code.h);

        assert!(
            bottom_gap >= 23.5,
            "auto-height grid panel must include its authored bottom padding and border: panel={panel:?} code={code:?} gap={bottom_gap}"
        );
    }

    #[test]
    fn high_dpi_grid_reconciliation_grows_stale_bounds_to_last_row() {
        fn fixed_panel(id: &str) -> WidgetNode {
            let mut panel = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            panel.style.layout.height_value = Some(LayoutLength::LogicalPx(100.0));
            panel
        }

        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(120.0),
                ..NodeProps::default()
            },
            vec![
                fixed_panel("one"),
                fixed_panel("two"),
                fixed_panel("three"),
                fixed_panel("four"),
                fixed_panel("five"),
            ],
        );
        grid.style.layout.gap = Some(12.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let mut layout = compute_layout(&root, 520.0, 700.0, 1.5, &Theme::dark(), None);
        let last_bottom = {
            let last = layout.rects.get("five").unwrap();
            last.y + last.h
        };
        let grid_y = layout.rects.get("grid").unwrap().y;
        layout.rects.get_mut("grid").unwrap().h = last_bottom - grid_y - 30.0;

        assert!(grow_and_repack_ordinary_grid_rows(
            &root.children[0],
            &mut layout,
            1.5,
            &HashMap::new(),
        ));
        let grid = layout.rects.get("grid").unwrap();
        let final_last_bottom = {
            let last = layout.rects.get("five").unwrap();
            last.y + last.h
        };
        assert!(
            grid.y + grid.h >= final_last_bottom - 0.5,
            "reconciled high-DPI grid must contain its final row: grid={grid:?} final_last_bottom={final_last_bottom}"
        );
    }

    #[test]
    fn drop_target_justify_content_centers_its_label_on_the_main_axis() {
        let label = node(
            "label",
            WidgetKind::Label,
            NodeProps {
                text: Some("Drop a sheet name here".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        let mut drop_target = node(
            "drop",
            WidgetKind::DropTarget,
            NodeProps::default(),
            vec![label],
        );
        drop_target.style.layout.width = Some(280.0);
        drop_target.style.layout.height = Some(96.0);
        drop_target.style.layout.align_items = Some(AlignItemsStyle::Center);
        drop_target.style.layout.justify_content = Some(JustifyContentStyle::Center);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![drop_target],
        );

        let layout = compute_layout(&root, 320.0, 180.0, 1.0, &Theme::dark(), None);
        let drop_target = layout.rects.get("drop").unwrap();
        let label = layout.rects.get("label").unwrap();
        let drop_center = drop_target.y + drop_target.h * 0.5;
        let label_center = label.y + label.h * 0.5;

        assert!(
            (drop_center - label_center).abs() <= 0.5,
            "DropZone label should be vertically centered: drop={drop_target:?} label={label:?}"
        );
        let text_width = measure_text_for_layout(
            "Drop a sheet name here",
            &crate::style::TextStyle::default(),
            &Theme::dark(),
        )
        .width;
        let available_text_width = label.w - Theme::dark().spacing * 2.0;
        assert!(
            available_text_width >= text_width + LABEL_TEXT_WIDTH_SAFETY_LP - 0.5,
            "DropZone label must leave rasterization safety beyond the shaped text: label={label:?} available={available_text_width} shaped={text_width}"
        );
    }

    // -----------------------------------------------------------------------
    // Flow wrapping and auto-height contracts
    // -----------------------------------------------------------------------

    #[test]
    fn flow_layout_wraps_fixed_width_children_and_keeps_row_gap() {
        let mut first = node("first", WidgetKind::Panel, NodeProps::default(), vec![]);
        first.style.layout.width_value = Some(LayoutLength::LogicalPx(120.0));
        first.style.layout.height_value = Some(LayoutLength::LogicalPx(30.0));
        let mut second = node("second", WidgetKind::Panel, NodeProps::default(), vec![]);
        second.style.layout.width_value = Some(LayoutLength::LogicalPx(120.0));
        second.style.layout.height_value = Some(LayoutLength::LogicalPx(30.0));
        let mut third = node("third", WidgetKind::Panel, NodeProps::default(), vec![]);
        third.style.layout.width_value = Some(LayoutLength::LogicalPx(120.0));
        third.style.layout.height_value = Some(LayoutLength::LogicalPx(30.0));
        let mut flow = node(
            "flow",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            vec![first, second, third],
        );
        flow.style.layout.gap = Some(10.0);
        flow.style.layout.row_gap = Some(12.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![flow],
        );

        let layout = compute_layout(&root, 260.0, 180.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").unwrap();
        let second = layout.rects.get("second").unwrap();
        let third = layout.rects.get("third").unwrap();

        assert!(second.x > first.x);
        assert_eq!(second.y, first.y);
        assert_eq!(third.x, first.x);
        assert!(
            third.y >= first.y + first.h + 12.0,
            "third child should wrap with at least row_gap spacing"
        );
    }

    #[test]
    fn flow_layout_authored_center_alignment_keeps_search_button_and_toggle_centers_equal() {
        let search_icon = node(
            "search-icon",
            WidgetKind::IconButton,
            NodeProps {
                fixed_width: Some(28.0),
                fixed_height: Some(28.0),
                ..NodeProps::default()
            },
            vec![],
        );
        let search_input = node(
            "search-input",
            WidgetKind::TextInput,
            NodeProps {
                fixed_height: Some(28.0),
                ..NodeProps::default()
            },
            vec![],
        );
        let mut search = node(
            "search",
            WidgetKind::HLayout,
            NodeProps {
                fixed_width: Some(260.0),
                fixed_height: Some(38.0),
                ..NodeProps::default()
            },
            vec![search_icon, search_input],
        );
        search.style.layout.align_items = Some(AlignItemsStyle::Center);

        let button = node(
            "button",
            WidgetKind::Button,
            NodeProps {
                fixed_width: Some(96.0),
                fixed_height: Some(28.0),
                ..NodeProps::default()
            },
            vec![],
        );
        let toggle = node(
            "toggle",
            WidgetKind::ToggleSwitch,
            NodeProps {
                text: Some("Anomalies only".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        let mut flow = node(
            "controls",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            vec![search, button, toggle],
        );
        flow.style.layout.align_items = Some(AlignItemsStyle::Center);
        flow.style.layout.gap = Some(9.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![flow],
        );
        let layout = compute_layout(&root, 700.0, 120.0, 1.0, &Theme::dark(), None);
        let input = layout.rects["search-input"];
        let button = layout.rects["button"];
        let toggle = layout.rects["toggle"];
        let input_center = input.y + input.h * 0.5;
        let button_center = button.y + button.h * 0.5;
        let toggle_center = toggle.y + toggle.h * 0.5;

        assert!(
            (input_center - button_center).abs() <= 0.5,
            "mixed-height search and button surfaces should share a vertical center: input={input:?} button={button:?}"
        );
        assert!(
            (input_center - toggle_center).abs() <= 0.5,
            "compact toggles should inherit the row's vertical center: controls={:?} search={:?} input={input:?} button={button:?} toggle={toggle:?}",
            layout.rects["controls"],
            layout.rects["search"],
        );
    }

    #[test]
    fn horizontal_flow_centers_badges_tags_and_leds_with_normal_controls_at_125_percent_scale() {
        let led = node(
            "led",
            WidgetKind::Led,
            NodeProps {
                led_size: Some(14.0),
                ..NodeProps::default()
            },
            vec![],
        );
        let mut badge = node(
            "badge",
            WidgetKind::Badge,
            NodeProps {
                text: Some("online".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        badge.style.text.font_size = Some(13.0);
        let mut tag = node(
            "tag",
            WidgetKind::Tag,
            NodeProps {
                text: Some("beta".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        tag.style.text.font_size = Some(13.0);
        let button = node(
            "button",
            WidgetKind::Button,
            NodeProps {
                text: Some("Launch".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        let mut flow = node(
            "status-row",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            vec![led, badge, tag, button],
        );
        flow.style.layout.gap = Some(8.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![flow],
        );
        let layout = compute_layout(&root, 700.0, 120.0, 1.25, &Theme::dark(), None);
        let center = |id: &str| {
            let rect = layout.rects[id];
            rect.y + rect.h * 0.5
        };
        let button_center = center("button");

        for id in ["led", "badge", "tag"] {
            assert!(
                (center(id) - button_center).abs() <= 0.5,
                "{id} should share the normal control's physical vertical center at 125%: status={:?} button={:?}",
                layout.rects[id],
                layout.rects["button"],
            );
        }
    }

    #[test]
    fn flow_layout_taffy_rows_define_auto_height_across_widths() {
        let make_root = |flow_width: f32| {
            let fixed_child = |id: &str, height: f32| {
                node(
                    id,
                    WidgetKind::Button,
                    NodeProps {
                        fixed_width: Some(80.0),
                        fixed_height: Some(height),
                        ..NodeProps::default()
                    },
                    vec![],
                )
            };
            let mut flow = node(
                "flow",
                WidgetKind::FlowLayout,
                NodeProps {
                    fixed_width: Some(flow_width),
                    ..NodeProps::default()
                },
                vec![
                    fixed_child("first", 24.0),
                    fixed_child("second", 32.0),
                    fixed_child("third", 40.0),
                ],
            );
            flow.style.layout.padding_left = Some(10.0);
            flow.style.layout.padding_right = Some(10.0);
            flow.style.layout.padding_top = Some(6.0);
            flow.style.layout.padding_bottom = Some(9.0);
            flow.style.layout.column_gap = Some(12.0);
            flow.style.layout.row_gap = Some(14.0);
            node(
                "window",
                WidgetKind::Window,
                NodeProps::default(),
                vec![flow],
            )
        };

        let theme = Theme::dark();
        let wide = compute_layout(&make_root(210.0), 320.0, 240.0, 1.0, &theme, None);
        let narrow = compute_layout(&make_root(150.0), 320.0, 240.0, 1.0, &theme, None);
        let wide_flow = wide.rects["flow"];
        let narrow_flow = narrow.rects["flow"];

        assert_eq!(wide.rects["first"].y, wide.rects["second"].y);
        assert!(wide.rects["third"].y >= wide.rects["second"].y + wide.rects["second"].h + 14.0);
        assert!(narrow.rects["second"].y > narrow.rects["first"].y);
        assert!(narrow.rects["third"].y > narrow.rects["second"].y);
        assert!(
            narrow_flow.h > wide_flow.h + 30.0,
            "narrow wrapped rows should drive a taller Taffy auto-height: wide={wide_flow:?} narrow={narrow_flow:?}"
        );

        for (layout, flow) in [(&wide, wide_flow), (&narrow, narrow_flow)] {
            let child_bottom = ["first", "second", "third"]
                .iter()
                .map(|id| {
                    let rect = layout.rects[*id];
                    rect.y + rect.h
                })
                .fold(flow.y, f32::max);
            assert!(
                (flow.y + flow.h - child_bottom - 9.0).abs() <= 0.6,
                "flow auto-height should end at the final Taffy row plus bottom padding: flow={flow:?} child_bottom={child_bottom}"
            );
        }
    }

    #[test]
    fn flow_layout_auto_width_controls_do_not_reserve_wrapped_height() {
        let mut buttons = Vec::new();
        for (id, label) in [
            ("fit", "Fit All"),
            ("refresh", "Refresh Data"),
            ("iso", "3D Iso"),
        ] {
            let mut button = node(
                id,
                WidgetKind::Button,
                NodeProps {
                    text: Some(label.to_string()),
                    ..NodeProps::default()
                },
                vec![],
            );
            button.style.layout.width_value = Some(LayoutLength::Auto);
            button.style.layout.min_width = Some(74.0);
            button.style.layout.height = Some(32.0);
            buttons.push(button);
        }
        let expected_refresh_width =
            intrinsic_leaf_width(&buttons[1], &Theme::dark()).expect("button intrinsic width");
        let mut flow = node(
            "controls",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            buttons,
        );
        flow.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        flow.style.layout.gap = Some(8.0);
        flow.style.layout.row_gap = Some(8.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![flow],
        );

        let layout = compute_layout(&root, 360.0, 180.0, 1.0, &Theme::dark(), None);
        let controls = layout.rects.get("controls").unwrap();
        let fit = layout.rects.get("fit").unwrap();
        let refresh = layout.rects.get("refresh").unwrap();
        let iso = layout.rects.get("iso").unwrap();

        assert!(
            controls.h <= 40.0,
            "auto-width controls should reserve one row, not wrapped rows: {controls:?}"
        );
        assert_eq!(fit.y, refresh.y);
        assert_eq!(refresh.y, iso.y);
        assert!(
            refresh.w + 0.5 >= expected_refresh_width,
            "auto-width button should fit its intrinsic label: rect={refresh:?} expected_width={expected_refresh_width}"
        );
    }

    #[test]
    fn grid_template_areas_place_named_children() {
        let mut grid = node("grid", WidgetKind::Panel, NodeProps::default(), vec![]);
        grid.style.layout.display = Some(DisplayStyle::Grid);
        grid.style.layout.width_value = Some(LayoutLength::LogicalPx(600.0));
        grid.style.layout.height_value = Some(LayoutLength::LogicalPx(220.0));
        grid.style.layout.padding = Some(0.0);
        grid.style.layout.gap = Some(0.0);
        grid.style.layout.grid_template_columns = Some(vec![
            GridTrackSize::LogicalPx(180.0),
            GridTrackSize::Fraction(1.0),
            GridTrackSize::Fraction(2.0),
        ]);
        grid.style.layout.grid_template_rows = Some(vec![
            GridTrackSize::LogicalPx(80.0),
            GridTrackSize::LogicalPx(120.0),
        ]);
        grid.style.layout.grid_template_areas = Some(GridTemplateAreas {
            columns: 3,
            rows: 2,
            areas: vec![
                crate::style::GridTemplateArea {
                    name: "sidebar".to_string(),
                    row_start: 1,
                    row_end: 3,
                    column_start: 1,
                    column_end: 2,
                },
                crate::style::GridTemplateArea {
                    name: "main".to_string(),
                    row_start: 1,
                    row_end: 2,
                    column_start: 2,
                    column_end: 4,
                },
            ],
        });

        let mut sidebar = node("sidebar", WidgetKind::Panel, NodeProps::default(), vec![]);
        sidebar.style.layout.grid_area = Some("sidebar".to_string());
        let mut main = node("main", WidgetKind::Panel, NodeProps::default(), vec![]);
        main.style.layout.grid_area = Some("main".to_string());
        grid.children = vec![sidebar, main];
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![grid],
        );

        let layout = compute_layout(&root, 800.0, 400.0, 1.0, &Theme::dark(), None);
        let sidebar = layout.rects.get("sidebar").unwrap();
        let main = layout.rects.get("main").unwrap();

        assert_eq!(sidebar.w, 180.0);
        assert_eq!(sidebar.h, 200.0);
        assert_eq!(main.x, 180.0);
        assert_eq!(main.w, 420.0);
        assert_eq!(main.h, 80.0);
    }

    #[test]
    fn grid_minmax_tracks_lower_to_taffy() {
        let track = grid_track_size(
            GridTrackSize::MinMax {
                min: GridTrackMinSize::LogicalPx(120.0),
                max: GridTrackMaxSize::Fraction(1.0),
            },
            2.0,
        );

        let TrackSizingFunction::Single(track) = track else {
            panic!("minmax should lower to a single non-repeated track");
        };
        assert_eq!(
            track.min,
            MinTrackSizingFunction::Fixed(LengthPercentage::Length(240.0))
        );
        assert_eq!(track.max, MaxTrackSizingFunction::Fraction(1.0));
    }

    #[test]
    fn grid_fit_content_tracks_lower_to_taffy() {
        let track = grid_track_size(
            GridTrackSize::FitContent(GridTrackFitContentSize::Percent(40.0)),
            2.0,
        );

        let TrackSizingFunction::Single(track) = track else {
            panic!("fit-content should lower to a single non-repeated track");
        };
        assert_eq!(track.min, MinTrackSizingFunction::Auto);
        assert_eq!(
            track.max,
            MaxTrackSizingFunction::FitContent(LengthPercentage::Percent(0.4))
        );
    }

    #[test]
    fn grid_auto_repeat_tracks_lower_to_taffy() {
        let track = grid_track_size(
            GridTrackSize::Repeat {
                kind: GridTrackRepeatKind::AutoFit,
                tracks: vec![GridTrackSize::MinMax {
                    min: GridTrackMinSize::LogicalPx(120.0),
                    max: GridTrackMaxSize::Fraction(1.0),
                }],
            },
            2.0,
        );

        let TrackSizingFunction::Repeat(kind, tracks) = track else {
            panic!("auto-repeat should lower to a repeated track");
        };
        assert_eq!(kind, GridTrackRepetition::AutoFit);
        assert_eq!(tracks.len(), 1);
        assert_eq!(
            tracks[0].min,
            MinTrackSizingFunction::Fixed(LengthPercentage::Length(240.0))
        );
        assert_eq!(tracks[0].max, MaxTrackSizingFunction::Fraction(1.0));
    }

    #[test]
    fn definite_auto_repeat_expands_without_taffy_fractional_minmax_panic() {
        let tracks = grid_track_sizes(
            &[GridTrackSize::Repeat {
                kind: GridTrackRepeatKind::AutoFit,
                tracks: vec![GridTrackSize::MinMax {
                    min: GridTrackMinSize::LogicalPx(120.0),
                    max: GridTrackMaxSize::Fraction(1.0),
                }],
            }],
            1.0,
            Some(600.0),
            10.0,
            10,
        );

        assert_eq!(tracks.len(), 4);
        assert!(tracks.iter().all(|track| matches!(
            track,
            TrackSizingFunction::Single(NonRepeatedTrackSizingFunction {
                min: MinTrackSizingFunction::Fixed(LengthPercentage::Length(120.0)),
                max: MaxTrackSizingFunction::Fraction(1.0),
            })
        )));
    }

    #[test]
    fn final_layout_maps_drop_inactive_scroll_owners() {
        let mut result = LayoutResult::default();
        result.rects.insert(
            "active".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
        );
        result.scroll_max_x.insert("active".to_string(), 1.0);
        result.scroll_max_x.insert("inactive".to_string(), 2.0);
        result.scroll_max_y.insert("inactive".to_string(), 3.0);

        retain_active_layout_maps(&mut result);

        assert_eq!(result.scroll_max_x.get("active"), Some(&1.0));
        assert!(!result.scroll_max_x.contains_key("inactive"));
        assert!(!result.scroll_max_y.contains_key("inactive"));
    }

    #[test]
    fn grid_auto_flow_lowers_to_taffy() {
        assert_eq!(
            grid_auto_flow(GridAutoFlowStyle::Row),
            taffy::style::GridAutoFlow::Row
        );
        assert_eq!(
            grid_auto_flow(GridAutoFlowStyle::ColumnDense),
            taffy::style::GridAutoFlow::ColumnDense
        );
    }

    // -----------------------------------------------------------------------
    // Clipping, overflow ownership, and scroll geometry contracts
    // -----------------------------------------------------------------------

    #[test]
    fn child_visible_clip_does_not_escape_fixed_height_parent() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![
                node(
                    "first",
                    WidgetKind::Label,
                    NodeProps {
                        text: Some("First".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "second",
                    WidgetKind::Label,
                    NodeProps {
                        text: Some("Second".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
            ],
        );
        panel.style.layout.height = Some(40.0);
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.gap = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 300.0, 200.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let second = layout.clips.get("second").unwrap();

        assert!(
            second.y + second.h <= panel.y + panel.h,
            "child visible clip escaped parent: panel={panel:?} second_clip={second:?}"
        );
    }

    #[test]
    fn panel_scroll_offset_moves_children_and_preserves_clip() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
                node("third", WidgetKind::Button, NodeProps::default(), vec![]),
                node("fourth", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.gap = Some(0.0);
        panel.style.layout.height = Some(100.0);
        for child in &mut panel.children {
            child.style.layout.height = Some(34.0);
        }
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let unscrolled = compute_layout(&root, 240.0, 100.0, 1.0, &Theme::dark(), None);
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("panel".to_string(), 40.0);

        let scrolled = compute_layout(&root, 240.0, 100.0, 1.0, &Theme::dark(), Some(&state));

        let first_before = unscrolled.rects.get("first").unwrap();
        let first_after = scrolled.rects.get("first").unwrap();
        let panel = scrolled.rects.get("panel").unwrap();
        let first_clip = scrolled.clips.get("first").unwrap();

        let applied_scroll = scrolled.scroll_y.get("panel").copied().unwrap_or(0.0);
        assert_eq!(first_after.y, first_before.y - applied_scroll);
        assert_eq!(panel.y, 0.0);
        if first_clip.h > 0.0 {
            assert!(first_clip.y >= panel.y);
            assert!(first_clip.y + first_clip.h <= panel.y + panel.h);
        }
        assert!(scroll_container_max_y(root.children.first().unwrap(), &unscrolled) > 0.0);
        assert_eq!(scrolled.scroll_y.get("panel").copied(), Some(36.0));
        assert_eq!(scrolled.scroll_max_y.get("panel").copied(), Some(36.0));
    }

    #[test]
    fn child_paint_clip_tracks_inherited_scroll_viewport_not_child_rect() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        panel.style.layout.height = Some(44.0);
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.gap = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 240.0, 80.0, 1.0, &Theme::dark(), None);

        let panel_clip = layout.clips.get("panel").copied().unwrap();
        let second_rect = layout.rects.get("second").copied().unwrap();
        let second_clip = layout.clips.get("second").copied().unwrap();
        let second_paint_clip = layout.paint_clip_rect("second").unwrap();

        assert!(
            second_clip.h < second_rect.h,
            "test needs second child partially clipped: rect={second_rect:?} clip={second_clip:?}"
        );
        assert_eq!(
            (
                second_paint_clip.x,
                second_paint_clip.y,
                second_paint_clip.w,
                second_paint_clip.h,
            ),
            (panel_clip.x, panel_clip.y, panel_clip.w, panel_clip.h)
        );
    }

    #[test]
    fn horizontal_scroll_offset_moves_children_and_preserves_clip() {
        let mut scroller = node(
            "scroller",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![
                node(
                    "first",
                    WidgetKind::Button,
                    NodeProps {
                        fixed_width: Some(100.0),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "second",
                    WidgetKind::Button,
                    NodeProps {
                        fixed_width: Some(100.0),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "third",
                    WidgetKind::Button,
                    NodeProps {
                        fixed_width: Some(100.0),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
            ],
        );
        scroller.style.layout.width = Some(140.0);
        scroller.style.layout.height = Some(40.0);
        scroller.style.layout.padding = Some(0.0);
        scroller.style.layout.gap = Some(0.0);
        scroller.style.layout.flex_grow = Some(0.0);
        scroller.style.layout.flex_shrink = Some(0.0);
        scroller.style.layout.overflow_x = Some(OverflowStyle::Auto);
        scroller.style.layout.overflow_y = Some(OverflowStyle::Hidden);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![scroller],
        );
        let unscrolled = compute_layout(&root, 240.0, 80.0, 1.0, &Theme::dark(), None);
        let mut state = WidgetState::default();
        state
            .container_scroll_x
            .insert("scroller".to_string(), 50.0);

        let scrolled = compute_layout(&root, 240.0, 80.0, 1.0, &Theme::dark(), Some(&state));

        let first_before = unscrolled.rects.get("first").unwrap();
        let first_after = scrolled.rects.get("first").unwrap();
        let scroller_rect = scrolled.rects.get("scroller").unwrap();
        let first_clip = scrolled.clips.get("first").unwrap();
        let applied_scroll = scrolled.scroll_x.get("scroller").copied().unwrap_or(0.0);

        assert!(scroll_container_max_x(root.children.first().unwrap(), &unscrolled) > 0.0);
        assert_eq!(applied_scroll, 50.0);
        assert_eq!(first_after.x, first_before.x - applied_scroll);
        if first_clip.w > 0.0 {
            assert!(first_clip.x >= scroller_rect.x);
            assert!(first_clip.x + first_clip.w <= scroller_rect.x + scroller_rect.w);
        }
        assert!(
            scrolled
                .scroll_max_x
                .get("scroller")
                .copied()
                .unwrap_or(0.0)
                > 0.0
        );
    }

    #[test]
    fn panel_scroll_range_preserves_bottom_padding() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
                node("third", WidgetKind::Button, NodeProps::default(), vec![]),
                node("fourth", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.gap = Some(0.0);
        panel.style.layout.height = Some(100.0);
        for child in &mut panel.children {
            child.style.layout.height = Some(34.0);
        }
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("panel".to_string(), 999.0);

        let layout = compute_layout(&root, 240.0, 100.0, 1.0, &Theme::dark(), Some(&state));
        let panel = layout.rects.get("panel").unwrap();
        let fourth = layout.rects.get("fourth").unwrap();

        assert_eq!(layout.scroll_max_y.get("panel").copied(), Some(64.0));
        assert_eq!(layout.scroll_y.get("panel").copied(), Some(64.0));
        assert_eq!(panel.y + panel.h - (fourth.y + fourth.h), 14.0);
    }

    #[test]
    fn both_axis_scroll_geometry_uses_resolved_calc_end_padding() {
        let child = node(
            "content",
            WidgetKind::Panel,
            NodeProps {
                fixed_width: Some(200.0),
                fixed_height: Some(160.0),
                ..NodeProps::default()
            },
            vec![],
        );
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![child],
        );
        panel.style.layout.width = Some(160.0);
        panel.style.layout.height = Some(120.0);
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.padding_right_value =
            Some(LayoutLength::Calc(crate::style::CalcLength {
                percent: 0.0,
                px: 32.0,
            }));
        panel.style.layout.padding_bottom_value =
            Some(LayoutLength::Calc(crate::style::CalcLength {
                percent: 0.0,
                px: 28.0,
            }));
        panel.style.layout.overflow_x = Some(OverflowStyle::Auto);
        panel.style.layout.overflow_y = Some(OverflowStyle::Auto);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let layout = compute_layout(
            &root,
            240.0,
            180.0,
            1.0,
            &Theme::dark(),
            Some(&WidgetState::default()),
        );
        let panel_node = root.children.first().expect("panel node");
        let resolved_box = layout.resolved_box("panel").expect("resolved panel box");
        let geometry = scroll_geometry(panel_node, &layout, true, 1.0, &Theme::dark());

        assert_eq!(resolved_box.padding.right, 32.0);
        assert_eq!(resolved_box.padding.bottom, 28.0);
        assert_eq!(geometry.max_x, 72.0);
        assert_eq!(geometry.max_y, 68.0);
        assert_eq!(layout.scroll_max_x.get("panel").copied(), Some(72.0));
        assert_eq!(layout.scroll_max_y.get("panel").copied(), Some(68.0));
        assert_eq!(geometry.content_bounds.x + geometry.content_bounds.w, 232.0);
        assert_eq!(geometry.content_bounds.y + geometry.content_bounds.h, 188.0);
    }

    #[test]
    fn scroll_area_counts_active_page_descendant_overflow() {
        let mut active_children = Vec::new();
        for index in 0..5 {
            let mut panel = node(
                &format!("active-panel-{index}"),
                WidgetKind::Panel,
                NodeProps {
                    text: Some(format!("Active {index}")),
                    ..NodeProps::default()
                },
                vec![],
            );
            panel.style.layout.height = Some(96.0);
            panel.style.layout.width_value = Some(LayoutLength::Percent(100.0));
            panel.style.layout.flex_shrink = Some(0.0);
            active_children.push(panel);
        }

        let active_page = node(
            "active-page",
            WidgetKind::Page,
            NodeProps {
                route_value: Some("active".to_string()),
                ..NodeProps::default()
            },
            active_children,
        );
        let mut inactive_panel = node(
            "inactive-panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![],
        );
        inactive_panel.style.layout.height = Some(900.0);
        let inactive_page = node(
            "inactive-page",
            WidgetKind::Page,
            NodeProps {
                route_value: Some("inactive".to_string()),
                ..NodeProps::default()
            },
            vec![inactive_panel],
        );
        let pages = node(
            "pages",
            WidgetKind::Pages,
            NodeProps {
                route_value: Some("active".to_string()),
                ..NodeProps::default()
            },
            vec![active_page, inactive_page],
        );
        let mut scroller = node(
            "body",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            vec![pages],
        );
        scroller.style.layout.height = Some(180.0);
        scroller.style.layout.padding = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![scroller],
        );

        let mut state = WidgetState::default();
        let layout = compute_layout(&root, 260.0, 220.0, 1.0, &Theme::dark(), Some(&state));
        let body = layout.rects.get("body").copied().unwrap();
        let pages = layout.rects.get("pages").copied().unwrap();
        let last = layout.rects.get("active-panel-4").copied().unwrap();
        let max_scroll = layout.scroll_max_y.get("body").copied().unwrap_or(0.0);

        assert!(
            pages.h <= body.h + 0.1,
            "fixture expects viewport-sized pages wrapper: body={body:?} pages={pages:?}"
        );
        assert!(
            last.y + last.h > body.y + body.h,
            "active descendant should overflow the body viewport: body={body:?} last={last:?}"
        );
        assert!(
            max_scroll >= last.y + last.h - (body.y + body.h) - 0.1,
            "body scroll range should include active page descendants: body={body:?} last={last:?} max_scroll={max_scroll}"
        );
        assert!(
            !layout.rects.contains_key("inactive-panel"),
            "inactive page content must not inflate body scroll range"
        );

        state
            .container_scroll_y
            .insert("body".to_string(), max_scroll + 100.0);
        let scrolled = compute_layout(&root, 260.0, 220.0, 1.0, &Theme::dark(), Some(&state));
        let last_clip = scrolled.clips.get("active-panel-4").copied().unwrap();
        assert!(
            last_clip.h > 0.0,
            "last active page descendant should be reachable at max scroll: clip={last_clip:?}"
        );
    }

    #[test]
    fn titled_panel_scroll_range_counts_body_descendant_overflow() {
        let mut rows = Vec::new();
        for index in 0..5 {
            let mut row = node(
                &format!("row-{index}"),
                WidgetKind::Button,
                NodeProps {
                    text: Some(format!("Row {index}")),
                    ..NodeProps::default()
                },
                vec![],
            );
            row.style.layout.height = Some(34.0);
            row.style.layout.flex_shrink = Some(0.0);
            rows.push(row);
        }
        let mut wrapper = node(
            "body-wrapper",
            WidgetKind::VLayout,
            NodeProps::default(),
            rows,
        );
        wrapper.style.layout.height = Some(82.0);
        wrapper.style.layout.gap = Some(8.0);
        wrapper.style.layout.overflow = Some(OverflowStyle::Visible);

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Scrollable panel".to_string()),
                ..NodeProps::default()
            },
            vec![wrapper],
        );
        panel.style.layout.height = Some(150.0);
        panel.style.layout.padding = Some(8.0);
        panel.style.layout.gap = Some(0.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let mut state = WidgetState::default();
        let layout = compute_layout(&root, 260.0, 180.0, 1.0, &Theme::dark(), Some(&state));
        let panel_rect = layout.rects.get("panel").copied().unwrap();
        let wrapper_rect = layout.rects.get("body-wrapper").copied().unwrap();
        let last = layout.rects.get("row-4").copied().unwrap();
        let max_scroll = layout.scroll_max_y.get("panel").copied().unwrap_or(0.0);

        assert!(
            wrapper_rect.y + wrapper_rect.h < last.y + last.h,
            "fixture expects descendants to overflow wrapper rect: wrapper={wrapper_rect:?} last={last:?}"
        );
        assert!(
            max_scroll > 0.0,
            "titled panel should expose body scroll range for overflowing descendants"
        );

        state
            .container_scroll_y
            .insert("panel".to_string(), max_scroll + 100.0);
        let scrolled = compute_layout(&root, 260.0, 180.0, 1.0, &Theme::dark(), Some(&state));
        let scrolled_panel = scrolled.rects.get("panel").copied().unwrap();
        let last_clip = scrolled.clips.get("row-4").copied().unwrap();

        assert_eq!(scrolled_panel.y, panel_rect.y);
        assert!(
            last_clip.h > 0.0,
            "last panel body descendant should be reachable at max scroll: clip={last_clip:?}"
        );
        assert!(
            last_clip.y >= scrolled_panel.y,
            "panel body descendant should remain clipped inside titled panel: panel={scrolled_panel:?} clip={last_clip:?}"
        );
    }

    #[test]
    fn overflow_visible_allows_child_clip_to_escape_parent() {
        let mut child = node("child", WidgetKind::Panel, NodeProps::default(), vec![]);
        child.style.layout.height_value = Some(LayoutLength::LogicalPx(90.0));
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![child],
        );
        panel.style.layout.height_value = Some(LayoutLength::LogicalPx(40.0));
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.overflow = Some(OverflowStyle::Visible);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(&root, 200.0, 120.0, 1.0, &Theme::dark(), None);
        let panel = layout.rects.get("panel").unwrap();
        let child_clip = layout.clips.get("child").unwrap();

        assert!(child_clip.y + child_clip.h > panel.y + panel.h);
    }

    #[test]
    fn plain_hlayout_allows_child_paint_to_escape_for_outlines() {
        let mut button = node("button", WidgetKind::Button, NodeProps::default(), vec![]);
        button.style.visual.outline_width = Some(2.0);
        button.style.visual.outline_offset = Some(2.0);
        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![button],
        );
        row.style.layout.height = Some(30.0);
        row.style.layout.flex_grow = Some(0.0);
        row.style.layout.flex_shrink = Some(0.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 220.0, 80.0, 1.0, &Theme::dark(), None);
        let row_rect = layout.rects.get("row").unwrap();
        let button_paint_clip = layout.paint_clip_rect("button").unwrap();

        assert!(
            button_paint_clip.y < row_rect.y
                || button_paint_clip.y + button_paint_clip.h > row_rect.y + row_rect.h,
            "button paint clip should not be confined to a plain HLayout row: row={row_rect:?} paint_clip={button_paint_clip:?}"
        );
    }

    #[test]
    fn explicit_hidden_hlayout_clips_child_paint() {
        let mut button = node("button", WidgetKind::Button, NodeProps::default(), vec![]);
        button.style.visual.outline_width = Some(2.0);
        button.style.visual.outline_offset = Some(2.0);
        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![button],
        );
        row.style.layout.height = Some(30.0);
        row.style.layout.flex_grow = Some(0.0);
        row.style.layout.flex_shrink = Some(0.0);
        row.style.layout.overflow = Some(OverflowStyle::Hidden);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );

        let layout = compute_layout(&root, 220.0, 80.0, 1.0, &Theme::dark(), None);
        let row_rect = layout.rects.get("row").unwrap();
        let button_paint_clip = layout.paint_clip_rect("button").unwrap();

        assert_eq!(
            (
                button_paint_clip.y,
                button_paint_clip.h,
                row_rect.y,
                row_rect.h
            ),
            (row_rect.y, row_rect.h, row_rect.y, row_rect.h)
        );
    }

    #[test]
    fn overflow_auto_opts_non_panel_container_into_scroll() {
        let mut scroller = node(
            "scroller",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
                node("third", WidgetKind::Button, NodeProps::default(), vec![]),
                node("fourth", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        scroller.style.layout.height_value = Some(LayoutLength::LogicalPx(70.0));
        scroller.style.layout.overflow_y = Some(OverflowStyle::Auto);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![scroller],
        );
        let mut state = WidgetState::default();
        state
            .container_scroll_y
            .insert("scroller".to_string(), 20.0);

        let layout = compute_layout(&root, 240.0, 100.0, 1.0, &Theme::dark(), Some(&state));

        assert!(layout.scroll_max_y.get("scroller").copied().unwrap_or(0.0) > 0.0);
        assert_eq!(layout.scroll_y.get("scroller").copied(), Some(20.0));
    }

    #[test]
    fn nested_vlayout_scroll_body_inside_titled_panel_gets_scroll_range() {
        let mut body_children = vec![node(
            "intro",
            WidgetKind::Label,
            NodeProps {
                text: Some("The title should stay above the scrollable body.".to_string()),
                ..NodeProps::default()
            },
            vec![],
        )];
        for index in 1..=10 {
            let mut button = node(
                &format!("row-{index}"),
                WidgetKind::Button,
                NodeProps::default(),
                vec![],
            );
            button.style.layout.height = Some(30.0);
            button.style.layout.flex_shrink = Some(0.0);
            body_children.push(button);
        }
        body_children.push(node(
            "pass",
            WidgetKind::Label,
            NodeProps {
                text: Some("PASS: final row can scroll fully into view.".to_string()),
                ..NodeProps::default()
            },
            vec![],
        ));

        let mut body = node(
            "scroll-body",
            WidgetKind::VLayout,
            NodeProps::default(),
            body_children,
        );
        body.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        body.style.layout.height = Some(210.0);
        body.style.layout.overflow_y = Some(OverflowStyle::Auto);
        body.style.layout.overflow_x = Some(OverflowStyle::Hidden);
        body.style.layout.padding_right = Some(26.0);
        body.style.layout.padding_bottom = Some(22.0);
        body.style.layout.gap = Some(10.0);

        let mut shell = node(
            "shell",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Scrollable titled panel".to_string()),
                ..NodeProps::default()
            },
            vec![body],
        );
        shell.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        shell.style.layout.height = Some(318.0);
        shell.style.layout.overflow = Some(OverflowStyle::Hidden);
        shell.style.layout.padding = Some(14.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![shell],
        );
        let mut state = WidgetState::default();

        let layout = compute_layout(&root, 700.0, 430.0, 1.0, &Theme::dark(), Some(&state));
        let max_scroll = layout
            .scroll_max_y
            .get("scroll-body")
            .copied()
            .unwrap_or(0.0);
        assert!(
            max_scroll > 0.0,
            "nested fixed-height VLayout clips children and should be scrollable"
        );
        let pass_rect = layout.rects.get("pass").unwrap();
        let pass_clip = layout.clips.get("pass").unwrap();
        assert!(
            pass_clip.h < pass_rect.h,
            "final label should start clipped before scrolling"
        );

        state
            .container_scroll_y
            .insert("scroll-body".to_string(), 999.0);
        let scrolled = compute_layout(&root, 700.0, 430.0, 1.0, &Theme::dark(), Some(&state));
        let pass_clip = scrolled.clips.get("pass").unwrap();
        assert!(
            pass_clip.h > 0.0,
            "final label should become visible after scrolling the nested body"
        );
    }

    #[test]
    fn parsed_probe_style_nested_scroll_body_gets_scroll_range() {
        let mut body_children = vec![serde_json::json!({
            "id": "intro",
            "type": "label",
            "class": "caption",
            "props": {"text": "The title should stay above the scrollable body."},
            "style": {"flex_shrink": 0}
        })];
        for index in 1..=10 {
            body_children.push(serde_json::json!({
                "id": format!("row-{index}"),
                "type": "button",
                "class": "scroll-row",
                "props": {"text": format!("Scrollable row {index}")},
                "style": {"height": 30, "flex_shrink": 0}
            }));
        }
        body_children.push(serde_json::json!({
            "id": "pass",
            "type": "label",
            "class": "pass",
            "props": {"text": "PASS: final row can scroll fully into view."},
            "style": {"flex_shrink": 0}
        }));
        let scroll_body = serde_json::json!({
            "id": "layout-scroll-body",
            "type": "v_layout",
            "class": "scroll-case",
            "props": {},
            "style": {
                "width": "100%",
                "height": 210,
                "overflow_y": "auto",
                "overflow_x": "hidden",
                "padding_right": 26,
                "padding_bottom": 22,
                "gap": 10
            },
            "children": body_children
        });
        let shell = serde_json::json!({
            "id": "shell",
            "type": "panel",
            "class": "scroll-shell",
            "props": {"text": "Scrollable titled panel"},
            "children": [scroll_body]
        });
        let root = serde_json::json!({
            "id": "root",
            "type": "v_layout",
            "class": "root",
            "props": {},
            "children": [shell]
        });
        let doc = serde_json::json!({
            "schema": 1,
            "type": "app",
            "window": {
                "id": "window",
                "type": "window",
                "props": {"title": "probe", "width": 900, "height": 720},
                "children": [root]
            },
            "stylesheets": [{
                "origin": "user",
                "source": r#"
                    VLayout.root {
                        width: 100%;
                        height: 100%;
                        overflow-y: auto;
                        padding-right: 22px;
                        padding-bottom: 76px;
                        gap: 12px;
                    }
                    Panel {
                        padding: 14px;
                        gap: 10px;
                    }
                    Panel.scroll-shell {
                        width: 100%;
                        height: 318px;
                        overflow: hidden;
                    }
                    Button.scroll-row {
                        height: 30px;
                        flex-shrink: 0;
                    }
                "#
            }]
        });
        let mut stylesheets = crate::document::parse_stylesheets_from_doc(&doc);
        let theme = Theme::dark();
        stylesheets.install_framework_defaults(&theme);
        let mut tree = crate::document::parse_widget_tree(&doc).expect("tree");
        crate::css_style::apply_stylesheets_to_tree(&mut tree, &mut stylesheets);
        let state = WidgetState::default();

        let layout = compute_layout(&tree, 900.0, 720.0, 1.0, &theme, Some(&state));
        let body = layout.rects.get("layout-scroll-body").expect("body rect");
        let pass = layout.rects.get("pass").expect("pass rect");
        let max_scroll = layout
            .scroll_max_y
            .get("layout-scroll-body")
            .copied()
            .unwrap_or(0.0);

        assert_eq!(body.h, 210.0);
        assert!(
            pass.y + pass.h > body.y + body.h,
            "test fixture should overflow: body={body:?} pass={pass:?}"
        );
        assert!(
            max_scroll > 0.0,
            "parsed probe document should produce scroll range for layout-scroll-body"
        );
    }

    #[test]
    fn scroll_area_default_takes_remaining_space_without_covering_siblings() {
        let mut rows = Vec::new();
        for index in 0..8 {
            let mut row = node(
                &format!("row-{index}"),
                WidgetKind::Button,
                NodeProps {
                    text: Some(format!("Row {index}")),
                    ..NodeProps::default()
                },
                vec![],
            );
            row.style.layout.height = Some(30.0);
            row.style.layout.flex_shrink = Some(0.0);
            rows.push(row);
        }

        let mut refresh = node(
            "refresh",
            WidgetKind::Button,
            NodeProps {
                text: Some("Refresh stats".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        refresh.style.layout.height = Some(34.0);
        refresh.style.layout.flex_shrink = Some(0.0);

        let mut scroller = node(
            "controls-scroll",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            rows,
        );
        scroller.style.layout.gap = Some(8.0);

        let mut auto_stats = node(
            "auto-stats",
            WidgetKind::Checkbox,
            NodeProps {
                text: Some("Auto stats".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        auto_stats.style.layout.height = Some(34.0);
        auto_stats.style.layout.flex_shrink = Some(0.0);

        let mut controls = node(
            "controls",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![refresh, scroller, auto_stats],
        );
        controls.style.layout.height = Some(150.0);
        controls.style.layout.padding = Some(8.0);
        controls.style.layout.gap = Some(8.0);
        controls.style.layout.overflow = Some(OverflowStyle::Hidden);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![controls],
        );

        let state = WidgetState::default();
        let layout = compute_layout(&root, 260.0, 180.0, 1.0, &Theme::dark(), Some(&state));
        let refresh_rect = layout.rects.get("refresh").unwrap();
        let refresh_clip = layout.clips.get("refresh").unwrap();
        let scroller_rect = layout.rects.get("controls-scroll").unwrap();
        let auto_rect = layout.rects.get("auto-stats").unwrap();
        let auto_clip = layout.clips.get("auto-stats").unwrap();
        let max_scroll = layout
            .scroll_max_y
            .get("controls-scroll")
            .copied()
            .unwrap_or(0.0);
        let first_row = layout.rects.get("row-0").unwrap();
        let last_row = layout.rects.get("row-7").unwrap();

        assert!(
            refresh_clip.h > 0.0 && auto_clip.h > 0.0,
            "fixed controls should remain visible around the scroll area: refresh={refresh_rect:?} refresh_clip={refresh_clip:?} auto={auto_rect:?} auto_clip={auto_clip:?}"
        );
        assert!(
            refresh_rect.y + refresh_rect.h <= scroller_rect.y
                && scroller_rect.y + scroller_rect.h <= auto_rect.y,
            "scroll area should be laid out between fixed controls, not over them: refresh={refresh_rect:?} scroller={scroller_rect:?} auto={auto_rect:?}"
        );
        assert!(
            max_scroll > 0.0,
            "scroll area should own overflow from its rows: scroller={scroller_rect:?} first_row={first_row:?} last_row={last_row:?} max_scroll={max_scroll}"
        );
    }

    #[test]
    fn scroll_area_grid_child_uses_content_height_for_rows() {
        let mut first = node("first", WidgetKind::Panel, NodeProps::default(), vec![]);
        first.style.layout.height = Some(220.0);
        let mut second = node("second", WidgetKind::Panel, NodeProps::default(), vec![]);
        second.style.layout.height = Some(80.0);
        let mut third = node("third", WidgetKind::Panel, NodeProps::default(), vec![]);
        third.style.layout.height = Some(220.0);
        let mut fourth = node("fourth", WidgetKind::Panel, NodeProps::default(), vec![]);
        fourth.style.layout.height = Some(80.0);

        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps {
                grid_columns: Some(2),
                grid_min_column_width: Some(120.0),
                ..NodeProps::default()
            },
            vec![first, second, third, fourth],
        );
        grid.style.layout.padding = Some(10.0);
        grid.style.layout.gap = Some(12.0);
        grid.style.layout.flex_grow = Some(0.0);
        grid.style.layout.flex_shrink = Some(1.0);

        let mut scroller = node(
            "scroller",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            vec![grid],
        );
        scroller.style.layout.height = Some(260.0);
        scroller.style.layout.padding = Some(0.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![scroller],
        );

        let state = WidgetState::default();
        let layout = compute_layout(&root, 420.0, 260.0, 1.0, &Theme::dark(), Some(&state));
        let scroller_rect = layout.rects.get("scroller").expect("scroller rect");
        let grid_rect = layout.rects.get("grid").expect("grid rect");
        let first_rect = layout.rects.get("first").expect("first panel rect");
        let third_rect = layout.rects.get("third").expect("third panel rect");
        let max_scroll = layout.scroll_max_y.get("scroller").copied().unwrap_or(0.0);

        assert!(
            third_rect.y >= first_rect.y + first_rect.h + 11.5,
            "auto grid row should start after tallest previous row inside ScrollArea: first={first_rect:?} third={third_rect:?}"
        );
        assert!(
            grid_rect.h > scroller_rect.h && max_scroll > 0.0,
            "ScrollArea grid child should keep content height and produce scroll range: scroller={scroller_rect:?} grid={grid_rect:?} max_scroll={max_scroll}"
        );
    }

    #[test]
    fn nested_scroll_body_keeps_scroll_range_when_parent_is_scrolled() {
        let mut body_children = vec![node(
            "intro",
            WidgetKind::Label,
            NodeProps {
                text: Some("The title should stay above the scrollable body.".to_string()),
                ..NodeProps::default()
            },
            vec![],
        )];
        body_children[0].style.layout.flex_shrink = Some(0.0);
        for index in 1..=10 {
            let mut button = node(
                &format!("row-{index}"),
                WidgetKind::Button,
                NodeProps::default(),
                vec![],
            );
            button.style.layout.height = Some(30.0);
            button.style.layout.flex_shrink = Some(0.0);
            body_children.push(button);
        }
        let mut pass = node(
            "pass",
            WidgetKind::Label,
            NodeProps {
                text: Some("PASS: final row can scroll fully into view.".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        pass.style.layout.flex_shrink = Some(0.0);
        body_children.push(pass);

        let mut body = node(
            "layout-scroll-body",
            WidgetKind::VLayout,
            NodeProps::default(),
            body_children,
        );
        body.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        body.style.layout.height = Some(210.0);
        body.style.layout.overflow_y = Some(OverflowStyle::Auto);
        body.style.layout.overflow_x = Some(OverflowStyle::Hidden);
        body.style.layout.padding_right = Some(26.0);
        body.style.layout.padding_bottom = Some(22.0);
        body.style.layout.gap = Some(10.0);

        let mut shell = node(
            "shell",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Scrollable titled panel".to_string()),
                ..NodeProps::default()
            },
            vec![body],
        );
        shell.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        shell.style.layout.height = Some(318.0);
        shell.style.layout.overflow = Some(OverflowStyle::Hidden);
        shell.style.layout.padding = Some(14.0);

        let mut before = node("before", WidgetKind::Panel, NodeProps::default(), vec![]);
        before.style.layout.height = Some(1250.0);
        before.style.layout.flex_shrink = Some(0.0);
        let mut after = node("after", WidgetKind::Panel, NodeProps::default(), vec![]);
        after.style.layout.height = Some(260.0);
        after.style.layout.flex_shrink = Some(0.0);

        let mut root_scroller = node(
            "root",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![before, shell, after],
        );
        root_scroller.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        root_scroller.style.layout.height_value = Some(LayoutLength::Percent(100.0));
        root_scroller.style.layout.overflow_y = Some(OverflowStyle::Auto);
        root_scroller.style.layout.padding_right = Some(22.0);
        root_scroller.style.layout.padding_bottom = Some(76.0);
        root_scroller.style.layout.gap = Some(12.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![root_scroller],
        );
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("root".to_string(), 1200.0);

        let layout = compute_layout(&root, 900.0, 720.0, 1.0, &Theme::dark(), Some(&state));
        let body_rect = layout.rects.get("layout-scroll-body").unwrap();
        let body_clip = layout.clips.get("layout-scroll-body").unwrap();
        let max_scroll = layout
            .scroll_max_y
            .get("layout-scroll-body")
            .copied()
            .unwrap_or(0.0);

        assert!(
            body_clip.h > 0.0,
            "body should be visible: {body_rect:?} {body_clip:?}"
        );
        assert!(
            max_scroll > 0.0,
            "nested scroll range should not collapse when parent is scrolled: body={body_rect:?} clip={body_clip:?}"
        );
    }

    #[test]
    fn scrollable_panel_reserves_padding_for_styled_vertical_scrollbar() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
                node("third", WidgetKind::Button, NodeProps::default(), vec![]),
                node("fourth", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        panel.style.layout.width = Some(180.0);
        panel.style.layout.height = Some(80.0);
        panel.style.layout.padding = Some(4.0);
        panel.style.layout.gap = Some(4.0);
        panel.style.layout.overflow_y = Some(OverflowStyle::Auto);
        panel.style.parts.parts.insert(
            "scrollbar-track".to_string(),
            crate::style::PartStyle {
                layout: crate::style::PartLayoutStyle {
                    width: Some(8.0),
                    padding: Some(8.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        panel.style.parts.parts.insert(
            "scrollbar-thumb".to_string(),
            crate::style::PartStyle {
                layout: crate::style::PartLayoutStyle {
                    width: Some(6.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("panel".to_string(), 10.0);

        let layout = compute_layout(&root, 240.0, 120.0, 1.0, &Theme::dark(), Some(&state));
        let panel_rect = layout.rects.get("panel").unwrap();
        let first_rect = layout.rects.get("first").unwrap();

        assert!(layout.scroll_max_y.get("panel").copied().unwrap_or(0.0) > 0.0);
        assert!(
            first_rect.x + first_rect.w <= panel_rect.x + panel_rect.w - 24.0 + 0.5,
            "stretched child should leave room for styled scrollbar gutter: panel={panel_rect:?} first={first_rect:?}"
        );
    }

    #[test]
    fn implicit_scrollable_panel_reserves_padding_for_styled_vertical_scrollbar() {
        let mut left = node("left", WidgetKind::Panel, NodeProps::default(), vec![]);
        left.style.layout.width = Some(140.0);
        left.style.layout.height = Some(50.0);
        left.style.layout.flex_shrink = Some(0.0);
        let mut spacer = node("spacer", WidgetKind::Spacer, NodeProps::default(), vec![]);
        spacer.style.layout.flex_grow = Some(1.0);
        let mut right = node("right", WidgetKind::Panel, NodeProps::default(), vec![]);
        right.style.layout.width = Some(140.0);
        right.style.layout.height = Some(50.0);
        right.style.layout.flex_shrink = Some(0.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![left, spacer, right],
        );
        row.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        row.style.layout.height = Some(48.0);
        row.style.layout.gap = Some(12.0);

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Spacer behavior".to_string()),
                ..NodeProps::default()
            },
            vec![row],
        );
        panel.style.layout.width = Some(420.0);
        panel.style.layout.height = Some(180.0);
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.gap = Some(10.0);
        panel.style.parts.parts.insert(
            "scrollbar-track".to_string(),
            crate::style::PartStyle {
                layout: crate::style::PartLayoutStyle {
                    width: Some(8.0),
                    padding: Some(1.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        panel.style.parts.parts.insert(
            "scrollbar-thumb".to_string(),
            crate::style::PartStyle {
                layout: crate::style::PartLayoutStyle {
                    width: Some(6.0),
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );

        let layout = compute_layout(
            &root,
            480.0,
            220.0,
            1.0,
            &Theme::dark(),
            Some(&WidgetState::default()),
        );
        let panel_rect = layout.rects.get("panel").unwrap();
        let right_rect = layout.rects.get("right").unwrap();
        let reserve = scrollbar_gutter_reserve_px(root.children.first().expect("panel"), 1.0);

        assert!(
            right_rect.x + right_rect.w <= panel_rect.x + panel_rect.w - reserve + 0.5,
            "right tile should leave implicit scrollbar gutter: panel={panel_rect:?} right={right_rect:?} reserve={reserve}"
        );
        assert!(
            reserve >= 24.0,
            "implicit scrollbar gutter should include edge inset and content gap: {reserve}"
        );
    }

    #[test]
    fn titled_scroll_panel_with_clipped_buttons_gets_scroll_range() {
        let mut children = vec![node(
            "intro",
            WidgetKind::Label,
            NodeProps {
                text: Some("The title should stay above the scrollable body.".to_string()),
                ..NodeProps::default()
            },
            vec![],
        )];
        for index in 1..=10 {
            let mut button = node(
                &format!("button-{index}"),
                WidgetKind::Button,
                NodeProps::default(),
                vec![],
            );
            button.style.layout.height = Some(30.0);
            children.push(button);
        }
        children.push(node(
            "pass",
            WidgetKind::Label,
            NodeProps {
                text: Some("PASS: final row can scroll fully into view.".to_string()),
                ..NodeProps::default()
            },
            vec![],
        ));

        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Scrollable titled panel".to_string()),
                ..NodeProps::default()
            },
            children,
        );
        panel.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        panel.style.layout.height = Some(250.0);
        panel.style.layout.overflow_y = Some(OverflowStyle::Auto);
        panel.style.layout.overflow_x = Some(OverflowStyle::Hidden);
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.padding_right = Some(26.0);
        panel.style.layout.padding_bottom = Some(22.0);
        panel.style.layout.gap = Some(10.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let mut state = WidgetState::default();
        let layout = compute_layout(&root, 700.0, 400.0, 1.0, &Theme::dark(), Some(&state));

        let max_scroll = layout.scroll_max_y.get("panel").copied().unwrap_or(0.0);
        assert!(
            max_scroll > 0.0,
            "titled panel has clipped children and should be scrollable"
        );
        let pass_rect = layout.rects.get("pass").unwrap();
        let pass_clip = layout.clips.get("pass").unwrap();
        assert!(
            pass_clip.h < pass_rect.h,
            "test should start with the final label clipped before scrolling"
        );

        state.container_scroll_y.insert("panel".to_string(), 999.0);
        let scrolled = compute_layout(&root, 700.0, 400.0, 1.0, &Theme::dark(), Some(&state));
        let pass_clip = scrolled.clips.get("pass").unwrap();
        assert!(
            pass_clip.h > 0.0,
            "final label should become visible after scrolling the titled panel"
        );
    }

    #[test]
    fn titled_scroll_panel_clips_children_below_title() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Form controls".to_string()),
                ..NodeProps::default()
            },
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
                node("third", WidgetKind::Button, NodeProps::default(), vec![]),
                node("fourth", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        panel.style.layout.padding = Some(0.0);
        panel.style.layout.gap = Some(0.0);
        panel.style.layout.height = Some(100.0);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("panel".to_string(), 40.0);
        let theme = Theme::dark();

        let layout = compute_layout(&root, 240.0, 100.0, 1.0, &theme, Some(&state));

        let panel = layout.rects.get("panel").unwrap();
        let title_bottom = panel.y
            + panel_title_top_padding_lp(root.children.first().unwrap(), &theme)
            + panel_title_line_height_lp(root.children.first().unwrap(), &theme)
            + panel_title_gap_lp(root.children.first().unwrap(), &theme);

        let mut saw_visible_child = false;
        for id in ["first", "second", "third", "fourth"] {
            let clip = layout.clips.get(id).unwrap();
            if clip.h > 0.0 {
                saw_visible_child = true;
                assert!(
                    clip.y >= title_bottom,
                    "{id} clip overlapped title: {clip:?}"
                );
                assert!(clip.y >= panel.y);
            }
        }
        assert!(saw_visible_child);
    }

    #[test]
    fn titled_scroll_panel_max_scroll_reveals_last_child() {
        let mut children = vec![node(
            "intro",
            WidgetKind::Label,
            NodeProps {
                text: Some("Wheel inside this panel.".to_string()),
                ..NodeProps::default()
            },
            vec![],
        )];
        for index in 1..=9 {
            children.push(node(
                &format!("action-{index}"),
                WidgetKind::Button,
                NodeProps {
                    text: Some(format!("Scrollable action {index}")),
                    ..NodeProps::default()
                },
                vec![],
            ));
        }
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Vertical auto".to_string()),
                ..NodeProps::default()
            },
            children,
        );
        panel.style.layout.height = Some(205.0);
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.gap = Some(8.0);
        panel.style.layout.overflow_y = Some(OverflowStyle::Auto);
        panel.style.layout.overflow_x = Some(OverflowStyle::Hidden);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("panel".to_string(), 999.0);

        let layout = compute_layout(&root, 360.0, 240.0, 1.0, &Theme::dark(), Some(&state));
        let last_rect = layout.rects.get("action-9").unwrap();
        let last_clip = layout.clips.get("action-9").unwrap();

        assert!(
            last_clip.h >= last_rect.h - 0.5,
            "last child should be fully visible at max scroll: rect={last_rect:?} clip={last_clip:?} scroll={:?} max={:?}",
            layout.scroll_y.get("panel"),
            layout.scroll_max_y.get("panel")
        );
    }

    #[test]
    fn overflow_probe_vertical_panel_reveals_last_child_at_startup_size() {
        let mut vertical_children = vec![
            node(
                "vertical-title",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Vertical auto".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            ),
            node(
                "vertical-intro",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Wheel inside this panel.".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            ),
        ];
        for index in 1..=9 {
            vertical_children.push(node(
                &format!("probe-action-{index}"),
                WidgetKind::Button,
                NodeProps {
                    text: Some(format!("Scrollable action {index}")),
                    ..NodeProps::default()
                },
                vec![],
            ));
        }
        let mut vertical = node(
            "vertical-panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vertical_children,
        );
        vertical.style.layout.width = Some(330.0);
        vertical.style.layout.height = Some(220.0);
        vertical.style.layout.padding = Some(14.0);
        vertical.style.layout.padding_bottom = Some(20.0);
        vertical.style.layout.gap = Some(8.0);
        vertical.style.layout.overflow_y = Some(OverflowStyle::Auto);
        vertical.style.layout.overflow_x = Some(OverflowStyle::Hidden);

        let mut hidden = node(
            "hidden-panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![],
        );
        hidden.style.layout.width = Some(330.0);
        hidden.style.layout.height = Some(220.0);
        hidden.style.layout.padding = Some(14.0);
        hidden.style.layout.gap = Some(8.0);

        let mut row = node(
            "top-row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![vertical, hidden],
        );
        row.style.layout.gap = Some(12.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node(
                    "title",
                    WidgetKind::Label,
                    NodeProps {
                        text: Some("Overflow and scrollbar parts".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "caption",
                    WidgetKind::Label,
                    NodeProps {
                        text: Some(
                            "This probe isolates overflow clipping, vertical scroll, horizontal scroll, both-axis scroll, and ::scrollbar-track / ::scrollbar-thumb styling.".to_string(),
                        ),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                row,
            ],
        );
        let mut state = WidgetState::default();
        state
            .container_scroll_y
            .insert("vertical-panel".to_string(), 999.0);

        let layout = compute_layout(&root, 780.0, 700.0, 1.0, &Theme::dark(), Some(&state));
        let panel_rect = layout.rects.get("vertical-panel").unwrap();
        let panel_clip = layout.clips.get("vertical-panel").unwrap();
        let last_rect = layout.rects.get("probe-action-9").unwrap();
        let last_clip = layout.clips.get("probe-action-9").unwrap();

        assert!(
            panel_clip.h >= panel_rect.h - 0.5,
            "vertical panel itself should be visible at startup size: rect={panel_rect:?} clip={panel_clip:?}"
        );
        assert!(
            last_clip.h >= last_rect.h - 0.5,
            "last probe button should be fully visible at max scroll: rect={last_rect:?} clip={last_clip:?} scroll={:?} max={:?}",
            layout.scroll_y.get("vertical-panel"),
            layout.scroll_max_y.get("vertical-panel")
        );
    }

    #[test]
    fn partially_clipped_scroll_panel_uses_visible_viewport_for_max_scroll() {
        let mut children = Vec::new();
        for index in 1..=9 {
            children.push(node(
                &format!("clipped-action-{index}"),
                WidgetKind::Button,
                NodeProps {
                    text: Some(format!("Scrollable action {index}")),
                    ..NodeProps::default()
                },
                vec![],
            ));
        }
        let mut panel = node("panel", WidgetKind::Panel, NodeProps::default(), children);
        panel.style.layout.height = Some(220.0);
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.gap = Some(8.0);
        panel.style.layout.overflow_y = Some(OverflowStyle::Auto);
        panel.style.layout.flex_shrink = Some(0.0);

        let mut spacer = node("spacer", WidgetKind::Spacer, NodeProps::default(), vec![]);
        spacer.style.layout.height = Some(130.0);
        spacer.style.layout.flex_grow = Some(0.0);
        spacer.style.layout.flex_shrink = Some(0.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![spacer, panel],
        );
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("panel".to_string(), 999.0);

        let layout = compute_layout(&root, 300.0, 300.0, 1.0, &Theme::dark(), Some(&state));
        let panel_rect = layout.rects.get("panel").unwrap();
        let panel_clip = layout.clips.get("panel").unwrap();
        let last_rect = layout.rects.get("clipped-action-9").unwrap();
        let last_clip = layout.clips.get("clipped-action-9").unwrap();

        assert!(
            panel_clip.h < panel_rect.h,
            "test requires panel to be clipped by the window: rect={panel_rect:?} clip={panel_clip:?}"
        );
        assert!(
            last_clip.h >= last_rect.h - 0.5,
            "last child should be fully visible inside clipped viewport at max scroll: rect={last_rect:?} clip={last_clip:?} scroll={:?} max={:?}",
            layout.scroll_y.get("panel"),
            layout.scroll_max_y.get("panel")
        );
    }

    #[test]
    fn scroll_vlayout_preserves_hlayout_row_content_height() {
        let mut rows = Vec::new();
        for row_index in 1..=4 {
            let mut panel = node(
                &format!("panel-{row_index}"),
                WidgetKind::Panel,
                NodeProps::default(),
                vec![
                    node(
                        &format!("label-{row_index}"),
                        WidgetKind::Label,
                        NodeProps {
                            text: Some(format!("Row {row_index} content")),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                    node(
                        &format!("button-{row_index}"),
                        WidgetKind::Button,
                        NodeProps::default(),
                        vec![],
                    ),
                    node(
                        &format!("extra-{row_index}"),
                        WidgetKind::Button,
                        NodeProps::default(),
                        vec![],
                    ),
                ],
            );
            panel.style.layout.min_height = Some(150.0);
            panel.style.layout.padding = Some(12.0);
            panel.style.layout.gap = Some(8.0);
            let row = node(
                &format!("row-{row_index}"),
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![panel],
            );
            rows.push(row);
        }
        let mut root_scroller = node("scroller", WidgetKind::VLayout, NodeProps::default(), rows);
        root_scroller.style.layout.height = Some(300.0);
        root_scroller.style.layout.gap = Some(12.0);
        root_scroller.style.layout.overflow_y = Some(OverflowStyle::Auto);
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![root_scroller],
        );

        let state = WidgetState::default();
        let layout = compute_layout(&root, 420.0, 320.0, 1.0, &Theme::dark(), Some(&state));
        for row_index in 1..=4 {
            let row = layout.rects.get(&format!("row-{row_index}")).unwrap();
            let panel = layout.rects.get(&format!("panel-{row_index}")).unwrap();
            assert!(
                row.h >= panel.h - 0.5,
                "row should not clip its panel content height: row={row:?} panel={panel:?}"
            );
            assert!(
                panel.h >= 150.0,
                "panel min-height should survive inside scroll row: {panel:?}"
            );
        }
        assert!(
            layout.scroll_max_y.get("scroller").copied().unwrap_or(0.0) > 0.0,
            "content rows should overflow the root scroller instead of shrinking"
        );
    }

    #[test]
    fn explicit_window_vertical_scroll_handles_oversized_content() {
        let mut top = node("top", WidgetKind::Panel, NodeProps::default(), vec![]);
        top.style.layout.height = Some(120.0);
        top.style.layout.flex_shrink = Some(0.0);
        let mut middle = node("middle", WidgetKind::Panel, NodeProps::default(), vec![]);
        middle.style.layout.height = Some(120.0);
        middle.style.layout.flex_shrink = Some(0.0);
        let mut bottom = node("bottom", WidgetKind::Panel, NodeProps::default(), vec![]);
        bottom.style.layout.height = Some(120.0);
        bottom.style.layout.flex_shrink = Some(0.0);

        let mut stack = node(
            "stack",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![top, middle, bottom],
        );
        stack.style.layout.gap = Some(12.0);
        stack.style.layout.flex_shrink = Some(0.0);
        let mut root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![stack],
        );
        root.style.layout.overflow_y = Some(OverflowStyle::Auto);
        root.style.layout.overflow_x = Some(OverflowStyle::Hidden);

        let state = WidgetState::default();
        let layout = compute_layout(&root, 420.0, 240.0, 1.0, &Theme::dark(), Some(&state));

        assert!(
            layout.scroll_max_y.get("window").copied().unwrap_or(0.0) > 100.0,
            "explicitly scrollable window should expose a root scroll range when direct content is taller than the viewport"
        );
        let bottom_clip = layout.clips.get("bottom").copied().unwrap();
        assert!(
            bottom_clip.h < layout.rects.get("bottom").unwrap().h,
            "overflowing bottom panel should be clipped by the window viewport before scrolling"
        );
    }

    #[test]
    fn explicit_window_vertical_scroll_reveals_bottom_content() {
        let mut top = node("top", WidgetKind::Panel, NodeProps::default(), vec![]);
        top.style.layout.height = Some(120.0);
        top.style.layout.flex_shrink = Some(0.0);
        let mut middle = node("middle", WidgetKind::Panel, NodeProps::default(), vec![]);
        middle.style.layout.height = Some(120.0);
        middle.style.layout.flex_shrink = Some(0.0);
        let mut bottom = node("bottom", WidgetKind::Panel, NodeProps::default(), vec![]);
        bottom.style.layout.height = Some(120.0);
        bottom.style.layout.flex_shrink = Some(0.0);

        let mut stack = node(
            "stack",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![top, middle, bottom],
        );
        stack.style.layout.gap = Some(12.0);
        stack.style.layout.flex_shrink = Some(0.0);
        let mut root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![stack],
        );
        root.style.layout.overflow_y = Some(OverflowStyle::Auto);
        root.style.layout.overflow_x = Some(OverflowStyle::Hidden);

        let mut state = WidgetState::default();
        state.container_scroll_y.insert("window".to_string(), 999.0);
        let layout = compute_layout(&root, 420.0, 240.0, 1.0, &Theme::dark(), Some(&state));

        let bottom_clip = layout.clips.get("bottom").copied().unwrap();
        let bottom_rect = layout.rects.get("bottom").copied().unwrap();
        assert!(
            bottom_clip.h > bottom_rect.h - 1.0,
            "scrolling the window should reveal the bottom panel: rect={bottom_rect:?} clip={bottom_clip:?}"
        );
    }

    #[test]
    fn parent_scroll_clipping_does_not_create_child_panel_scroll_range() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Table panel".to_string()),
                ..NodeProps::default()
            },
            vec![
                node("first", WidgetKind::Button, NodeProps::default(), vec![]),
                node("second", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        panel.style.layout.height = Some(140.0);
        panel.style.layout.padding = Some(10.0);
        panel.style.layout.gap = Some(8.0);

        let mut before = node("before", WidgetKind::Spacer, NodeProps::default(), vec![]);
        before.style.layout.height = Some(170.0);
        let mut after = node("after", WidgetKind::Spacer, NodeProps::default(), vec![]);
        after.style.layout.height = Some(200.0);

        let mut scroller = node(
            "scroller",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![before, panel, after],
        );
        scroller.style.layout.height = Some(220.0);
        scroller.style.layout.overflow_y = Some(OverflowStyle::Auto);
        scroller.style.layout.gap = Some(10.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![scroller],
        );
        let mut state = WidgetState::default();
        for scroll_y in [0.0, 190.0] {
            state
                .container_scroll_y
                .insert("scroller".to_string(), scroll_y);

            let layout = compute_layout(&root, 320.0, 220.0, 1.0, &Theme::dark(), Some(&state));
            let panel_rect = layout.rects.get("panel").expect("panel rect");
            let panel_clip = layout.clips.get("panel").expect("panel clip");

            assert!(
                panel_clip.h < panel_rect.h,
                "test should leave panel partially clipped by parent scroll: scroll_y={scroll_y} panel={panel_rect:?} clip={panel_clip:?}"
            );
            assert_eq!(
                layout.scroll_max_y.get("panel").copied(),
                Some(0.0),
                "parent clipping should not make a fitting panel grow an internal scroll range at scroll_y={scroll_y}"
            );
        }
    }

    #[test]
    fn parent_scroll_does_not_flash_implicit_panel_scrollbars_across_offsets() {
        fn make_metric_panel(id: &str, table_h: f32) -> WidgetNode {
            let mut table = node(
                &format!("{id}-table"),
                WidgetKind::DataFrameTable,
                NodeProps::default(),
                vec![],
            );
            table.style.layout.height = Some(table_h);

            let mut panel = node(
                id,
                WidgetKind::Panel,
                NodeProps {
                    text: Some(format!("{id} metrics")),
                    ..NodeProps::default()
                },
                vec![
                    node(
                        &format!("{id}-title"),
                        WidgetKind::Label,
                        NodeProps {
                            text: Some("Metric sizing case".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                    table,
                    node(
                        &format!("{id}-pass"),
                        WidgetKind::Label,
                        NodeProps {
                            text: Some("PASS: panel should not get its own scrollbar".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    ),
                ],
            );
            panel.style.layout.min_width = Some(390.0);
            panel.style.layout.padding = Some(14.0);
            panel.style.layout.gap = Some(10.0);
            panel
        }

        let mut first_row = node(
            "first-row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![
                make_metric_panel("text-rows", 180.0),
                make_metric_panel("text-type", 210.0),
            ],
        );
        first_row.style.layout.gap = Some(12.0);

        let mut second_row = node(
            "second-row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![
                make_metric_panel("compact-table", 214.0),
                make_metric_panel("roomy-table", 274.0),
            ],
        );
        second_row.style.layout.gap = Some(12.0);

        let mut root_scroller = node(
            "root-scroller",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![
                node(
                    "heading",
                    WidgetKind::Label,
                    NodeProps {
                        text: Some("Widget metrics".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "caption",
                    WidgetKind::Label,
                    NodeProps {
                        text: Some("Probe caption".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                first_row,
                second_row,
            ],
        );
        root_scroller.style.layout.height = Some(640.0);
        root_scroller.style.layout.padding_right = Some(20.0);
        root_scroller.style.layout.padding_bottom = Some(48.0);
        root_scroller.style.layout.gap = Some(12.0);
        root_scroller.style.layout.overflow_y = Some(OverflowStyle::Auto);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![root_scroller],
        );
        let mut state = WidgetState::default();
        for scroll_y in (0..=260).step_by(5) {
            state
                .container_scroll_y
                .insert("root-scroller".to_string(), scroll_y as f32);
            let layout = compute_layout(&root, 940.0, 720.0, 1.0, &Theme::dark(), Some(&state));
            for panel_id in ["text-rows", "text-type", "compact-table", "roomy-table"] {
                assert_eq!(
                    layout.scroll_max_y.get(panel_id).copied(),
                    Some(0.0),
                    "implicit panel {panel_id} should not gain an internal vertical scrollbar at root scroll_y={scroll_y}"
                );
            }
        }
    }

    #[test]
    fn page_panel_scroll_offset_is_applied_once() {
        let mut panel = node(
            "form",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Form controls".to_string()),
                fixed_width: Some(330.0),
                ..NodeProps::default()
            },
            vec![
                node(
                    "tags",
                    WidgetKind::HLayout,
                    NodeProps::default(),
                    vec![
                        node(
                            "live",
                            WidgetKind::Badge,
                            NodeProps {
                                text: Some("live".to_string()),
                                ..NodeProps::default()
                            },
                            vec![],
                        ),
                        node(
                            "queued",
                            WidgetKind::Badge,
                            NodeProps {
                                text: Some("queued".to_string()),
                                ..NodeProps::default()
                            },
                            vec![],
                        ),
                        node(
                            "review",
                            WidgetKind::Tag,
                            NodeProps {
                                text: Some("review".to_string()),
                                ..NodeProps::default()
                            },
                            vec![],
                        ),
                    ],
                ),
                node("input", WidgetKind::TextInput, NodeProps::default(), vec![]),
                node(
                    "dropdown",
                    WidgetKind::Dropdown,
                    NodeProps::default(),
                    vec![],
                ),
                node("slider", WidgetKind::Slider, NodeProps::default(), vec![]),
                node(
                    "number",
                    WidgetKind::NumberInput,
                    NodeProps::default(),
                    vec![],
                ),
                node("button-a", WidgetKind::Button, NodeProps::default(), vec![]),
                node("button-b", WidgetKind::Button, NodeProps::default(), vec![]),
                node("button-c", WidgetKind::Button, NodeProps::default(), vec![]),
            ],
        );
        panel.style.layout.padding = Some(14.0);
        panel.style.layout.gap = Some(10.0);
        let page = node(
            "controls",
            WidgetKind::Page,
            NodeProps {
                route_value: Some("controls".to_string()),
                ..NodeProps::default()
            },
            vec![node(
                "row",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![panel],
            )],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "pages",
                WidgetKind::Pages,
                NodeProps {
                    route_value: Some("controls".to_string()),
                    ..NodeProps::default()
                },
                vec![page],
            )],
        );
        let theme = Theme::dark();
        let unscrolled = compute_layout(&root, 480.0, 180.0, 1.0, &theme, None);
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("form".to_string(), 12.0);
        let scrolled = compute_layout(&root, 480.0, 180.0, 1.0, &theme, Some(&state));

        let applied_scroll = scrolled.scroll_y.get("form").copied().unwrap_or(0.0);
        assert!(applied_scroll > 0.0);
        let tags_before = unscrolled.rects.get("tags").unwrap();
        let tags_after = scrolled.rects.get("tags").unwrap();
        assert_eq!(tags_after.y, tags_before.y - applied_scroll);

        let form = unscrolled.rects.get("form").unwrap();
        let title_bottom = form.y
            + panel_title_top_padding_lp(
                root.children[0].children[0].children[0]
                    .children
                    .first()
                    .unwrap(),
                &theme,
            )
            + panel_title_line_height_lp(
                root.children[0].children[0].children[0]
                    .children
                    .first()
                    .unwrap(),
                &theme,
            )
            + panel_title_gap_lp(
                root.children[0].children[0].children[0]
                    .children
                    .first()
                    .unwrap(),
                &theme,
            );
        assert!(
            tags_before.y >= title_bottom,
            "tag row should start below fixed title: tags={tags_before:?} title_bottom={title_bottom}"
        );
    }

    // -----------------------------------------------------------------------
    // Composite panels, pages, and nested scrolling contracts
    // -----------------------------------------------------------------------

    #[test]
    fn active_page_style_bounds_scroll_area_child() {
        let mut buttons = Vec::new();
        for index in 0..10 {
            buttons.push(node(
                &format!("button-{index}"),
                WidgetKind::Button,
                NodeProps {
                    text: Some(format!("Action {index}")),
                    ..NodeProps::default()
                },
                vec![],
            ));
        }
        let mut scroller = node(
            "scroller",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            buttons,
        );
        scroller.style.layout.gap = Some(8.0);

        let mut page = node(
            "active-page",
            WidgetKind::Page,
            NodeProps {
                route_value: Some("active".to_string()),
                ..NodeProps::default()
            },
            vec![scroller],
        );
        page.style.layout.padding = Some(20.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "pages",
                WidgetKind::Pages,
                NodeProps {
                    route_value: Some("active".to_string()),
                    ..NodeProps::default()
                },
                vec![page],
            )],
        );
        let state = WidgetState::default();
        let layout = compute_layout(&root, 320.0, 180.0, 1.0, &Theme::dark(), Some(&state));
        let page_rect = layout.rects.get("active-page").expect("page rect");
        let scroller_rect = layout.rects.get("scroller").expect("scroll rect");
        assert!(
            scroller_rect.x >= page_rect.x + 19.5 && scroller_rect.y >= page_rect.y + 19.5,
            "active Page padding should be honored for children: page={page_rect:?} scroller={scroller_rect:?}"
        );
        let last_button_rect = layout.rects.get("button-9").expect("last button rect");
        let max_scroll_y = layout.scroll_max_y.get("scroller").copied().unwrap_or(0.0);
        assert!(
            max_scroll_y > 0.0,
            "ScrollArea should be a bounded vertical scroll container inside the active page: scroller={scroller_rect:?} last={last_button_rect:?} max_scroll_y={max_scroll_y} scroll_maps={:?}",
            layout.scroll_max_y
        );
    }

    #[test]
    fn titled_panel_bounds_nested_scroll_area_to_body() {
        let mut controls = Vec::new();
        controls.push(node(
            "data-label",
            WidgetKind::Label,
            NodeProps {
                text: Some("Data".to_string()),
                ..NodeProps::default()
            },
            vec![],
        ));
        for (id, text) in [
            ("append", "Append batch"),
            ("start", "Start stream"),
            ("stop", "Stop stream"),
            ("reset", "Reset plots"),
            ("fit", "Fit all plots"),
            ("follow-10", "Follow 10s"),
            ("follow-30", "Follow 30s"),
            ("history", "Full history"),
        ] {
            controls.push(node(
                id,
                WidgetKind::Button,
                NodeProps {
                    text: Some(text.to_string()),
                    ..NodeProps::default()
                },
                vec![],
            ));
        }

        let mut scroller = node(
            "body-scroll",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            controls,
        );
        scroller.style.layout.gap = Some(8.0);
        scroller.style.layout.overflow_y = Some(OverflowStyle::Auto);
        scroller.style.layout.min_height = Some(0.0);
        scroller.style.layout.flex_grow = Some(1.0);
        scroller.style.layout.flex_shrink = Some(1.0);

        let mut panel = node(
            "line-controls",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Line plot controls".to_string()),
                ..NodeProps::default()
            },
            vec![scroller],
        );
        panel.style.layout.width = Some(360.0);
        panel.style.layout.height = Some(190.0);
        panel.style.layout.padding = Some(10.0);
        panel.style.layout.gap = Some(8.0);
        panel.style.layout.overflow_y = Some(OverflowStyle::Hidden);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let state = WidgetState::default();
        let theme = Theme::dark();
        let layout = compute_layout(&root, 420.0, 240.0, 1.0, &theme, Some(&state));
        let panel_rect = layout.rects.get("line-controls").expect("panel rect");
        let scroller_rect = layout.rects.get("body-scroll").expect("scroller rect");
        let append_rect = layout.rects.get("append").expect("append button rect");
        let title_bottom = panel_rect.y
            + panel_title_top_padding_lp(root.children.first().unwrap(), &theme)
            + panel_title_line_height_lp(root.children.first().unwrap(), &theme)
            + panel_title_body_gap_lp(root.children.first().unwrap(), &theme);

        assert!(
            scroller_rect.y >= title_bottom - 0.5,
            "nested scroll area should begin in panel body: panel={panel_rect:?} scroller={scroller_rect:?} title_bottom={title_bottom}"
        );
        assert!(
            append_rect.h >= 20.0,
            "nested scroll area controls should keep their intrinsic height: {append_rect:?}"
        );
        assert!(
            layout
                .scroll_max_y
                .get("body-scroll")
                .copied()
                .unwrap_or(0.0)
                > 0.0,
            "nested body scroller should get scroll range: {:?}",
            layout.scroll_max_y
        );
    }

    #[test]
    fn titled_panel_body_preserves_child_gap() {
        let mut panel = node(
            "panel",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Controls".to_string()),
                ..NodeProps::default()
            },
            vec![
                node(
                    "first",
                    WidgetKind::Button,
                    NodeProps {
                        text: Some("First".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "second",
                    WidgetKind::Button,
                    NodeProps {
                        text: Some("Second".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
            ],
        );
        panel.style.layout.width = Some(260.0);
        panel.style.layout.height = Some(180.0);
        panel.style.layout.padding = Some(10.0);
        panel.style.layout.gap = Some(14.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let layout = compute_layout(&root, 320.0, 240.0, 1.0, &Theme::dark(), None);
        let first = layout.rects.get("first").expect("first button");
        let second = layout.rects.get("second").expect("second button");
        let actual_gap = second.y - (first.y + first.h);
        assert!(
            (actual_gap - 14.0).abs() <= 0.5,
            "titled panel body did not preserve child gap: first={first:?} second={second:?} actual_gap={actual_gap}"
        );
    }

    #[test]
    fn titled_panel_body_part_padding_controls_content_and_scroll_extent() {
        fn layout_with_body_padding(padding: f32) -> (WidgetNode, LayoutResult) {
            let mut child = node(
                "content",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Tall content".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            );
            child.style.layout.height = Some(180.0);
            child.style.layout.flex_shrink = Some(0.0);
            let mut panel = node(
                "panel",
                WidgetKind::Panel,
                NodeProps {
                    text: Some("Padded body".to_string()),
                    ..NodeProps::default()
                },
                vec![child],
            );
            panel.style.layout.width = Some(240.0);
            panel.style.layout.height = Some(130.0);
            panel.style.layout.padding = Some(8.0);
            panel.style.layout.overflow_y = Some(OverflowStyle::Auto);
            if padding > 0.0 {
                let mut body = crate::style::PartStyle::default();
                body.layout.padding = Some(padding);
                panel.style.parts.parts.insert("body".to_string(), body);
            }
            let root = node(
                "window",
                WidgetKind::Window,
                NodeProps::default(),
                vec![panel],
            );
            let layout = compute_layout(
                &root,
                300.0,
                180.0,
                1.0,
                &Theme::dark(),
                Some(&WidgetState::default()),
            );
            (root, layout)
        }

        let (plain_root, plain) = layout_with_body_padding(0.0);
        let (padded_root, padded) = layout_with_body_padding(12.0);
        let plain_content = plain.rects["content"];
        let padded_content = padded.rects["content"];
        let geometry =
            titled_container_geometry(&padded_root.children[0], &padded, 1.0, &Theme::dark())
                .expect("padded body geometry");

        assert!((padded_content.x - plain_content.x - 12.0).abs() <= 0.5);
        assert!((padded_content.y - plain_content.y - 12.0).abs() <= 0.5);
        assert!((plain_content.w - padded_content.w - 24.0).abs() <= 0.5);
        assert!((padded_content.y - geometry.body_content_origin_y).abs() <= 0.5);
        let plain_scroll = plain.scroll_max_y["panel"];
        let padded_scroll = padded.scroll_max_y["panel"];
        assert!(
            (padded_scroll - plain_scroll - 24.0).abs() <= 0.5,
            "body padding must contribute leading and trailing scroll extent: plain={plain_scroll} padded={padded_scroll}"
        );
        assert_eq!(panel_body_padding_lp(&plain_root.children[0]), 0.0);
        assert_eq!(panel_body_padding_lp(&padded_root.children[0]), 12.0);
    }

    #[test]
    fn flow_layout_checkboxes_keep_text_width_and_row_gap() {
        let mut flow = node(
            "flow",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            [
                "Grid",
                "Grid planes",
                "Orientation",
                "Sticky grid",
                "All edges",
            ]
            .iter()
            .enumerate()
            .map(|(idx, text)| {
                node(
                    &format!("check-{idx}"),
                    WidgetKind::Checkbox,
                    NodeProps {
                        text: Some((*text).to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                )
            })
            .collect(),
        );
        flow.style.layout.width = Some(260.0);
        flow.style.layout.gap = Some(8.0);
        flow.style.layout.row_gap = Some(6.0);
        let theme = Theme::dark();
        let expected_grid =
            intrinsic_leaf_width(&flow.children[0], &theme).expect("grid checkbox width");
        let expected_planes =
            intrinsic_leaf_width(&flow.children[1], &theme).expect("planes checkbox width");
        let expected_orientation =
            intrinsic_leaf_width(&flow.children[2], &theme).expect("orientation checkbox width");

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![flow],
        );
        let layout = compute_layout(&root, 320.0, 220.0, 1.0, &theme, None);
        let grid = layout.rects.get("check-0").expect("grid checkbox");
        let planes = layout.rects.get("check-1").expect("planes checkbox");
        let orientation = layout.rects.get("check-2").expect("orientation checkbox");
        assert!(
            grid.w + 0.5 >= expected_grid
                && planes.w + 0.5 >= expected_planes
                && orientation.w + 0.5 >= expected_orientation,
            "checkboxes should reserve room for box plus text: grid={grid:?} planes={planes:?} orientation={orientation:?}"
        );
        assert!(
            planes.x >= grid.x + grid.w + 7.5 || planes.y >= grid.y + grid.h + 5.5,
            "checkboxes should not overlap in flow layout: grid={grid:?} planes={planes:?}"
        );
    }

    #[test]
    fn active_page_v3_line_controls_keep_visible_scroll_body() {
        let flow = node(
            "line-actions",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            ["Append batch", "Start stream", "Stop stream", "Reset plots"]
                .iter()
                .enumerate()
                .map(|(idx, text)| {
                    let mut button = node(
                        &format!("line-action-{idx}"),
                        WidgetKind::Button,
                        NodeProps {
                            text: Some((*text).to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    );
                    button.style.layout.height = Some(34.0);
                    button
                })
                .collect(),
        );
        let mut flow = flow;
        flow.style.layout.gap = Some(8.0);
        flow.style.layout.row_gap = Some(8.0);

        let mut controls = vec![
            node(
                "line-data-label",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Data".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            ),
            flow,
            node(
                "line-fit",
                WidgetKind::Button,
                NodeProps {
                    text: Some("Fit all plots".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            ),
            node(
                "line-separator",
                WidgetKind::Separator,
                NodeProps::default(),
                vec![],
            ),
            node(
                "line-window-label",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Streaming window".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            ),
        ];
        for idx in 0..12 {
            controls.push(node(
                &format!("line-extra-{idx}"),
                WidgetKind::Button,
                NodeProps {
                    text: Some(format!("Extra control {idx}")),
                    ..NodeProps::default()
                },
                vec![],
            ));
        }

        let mut scroll = node(
            "line-control-scroll",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            controls,
        );
        scroll.style.layout.gap = Some(8.0);
        scroll.style.layout.padding_bottom = Some(26.0);
        scroll.style.layout.flex_grow = Some(1.0);
        scroll.style.layout.flex_shrink = Some(1.0);
        scroll.style.layout.min_height = Some(0.0);
        scroll.style.layout.overflow_y = Some(OverflowStyle::Auto);

        let mut panel = node(
            "line-controls",
            WidgetKind::Panel,
            NodeProps {
                text: Some("Line plot controls".to_string()),
                ..NodeProps::default()
            },
            vec![scroll],
        );
        panel.style.layout.width = Some(280.0);
        panel.style.layout.height_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 100.0,
            px: -8.0,
        }));
        panel.style.layout.max_height_value = Some(LayoutLength::Calc(crate::style::CalcLength {
            percent: 100.0,
            px: -8.0,
        }));
        panel.style.layout.padding = Some(10.0);
        panel.style.layout.gap = Some(8.0);
        panel.style.layout.flex_grow = Some(0.0);
        panel.style.layout.flex_shrink = Some(0.0);
        panel.style.layout.min_height = Some(0.0);
        panel.style.layout.overflow_y = Some(OverflowStyle::Hidden);

        let mut stack = node(
            "line-stack",
            WidgetKind::VLayout,
            NodeProps::default(),
            vec![node(
                "plot-panel",
                WidgetKind::Panel,
                NodeProps {
                    text: Some("Sensors".to_string()),
                    ..NodeProps::default()
                },
                vec![node(
                    "line-plot",
                    WidgetKind::LinePlot,
                    NodeProps::default(),
                    vec![],
                )],
            )],
        );
        stack.style.layout.flex_grow = Some(1.0);
        stack.style.layout.flex_shrink = Some(1.0);
        stack.style.layout.min_width = Some(0.0);
        stack.style.layout.min_height = Some(0.0);

        let mut line_layout = node(
            "line-layout",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![panel, stack],
        );
        line_layout.style.layout.padding = Some(10.0);
        line_layout.style.layout.gap = Some(12.0);
        line_layout.style.layout.flex_grow = Some(1.0);
        line_layout.style.layout.flex_shrink = Some(1.0);
        line_layout.style.layout.min_width = Some(0.0);
        line_layout.style.layout.min_height = Some(0.0);
        line_layout.style.layout.overflow_y = Some(OverflowStyle::Hidden);

        let page = node(
            "lineplots-page",
            WidgetKind::Page,
            NodeProps {
                route_value: Some("lineplots".to_string()),
                ..NodeProps::default()
            },
            vec![line_layout],
        );
        let pages = node(
            "pages",
            WidgetKind::Pages,
            NodeProps {
                route_value: Some("lineplots".to_string()),
                ..NodeProps::default()
            },
            vec![page],
        );
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![pages],
        );

        let layout = compute_layout(
            &root,
            900.0,
            420.0,
            1.0,
            &Theme::dark(),
            Some(&WidgetState::default()),
        );
        let panel_rect = layout
            .rects
            .get("line-controls")
            .expect("controls panel rect");
        let scroll_rect = layout
            .rects
            .get("line-control-scroll")
            .expect("controls scroll rect");
        let first_button = layout
            .rects
            .get("line-action-0")
            .expect("first action button rect");
        let flow_rect = layout.rects.get("line-actions").expect("flow rect");
        let reset_button = layout
            .rects
            .get("line-action-3")
            .expect("last flow button rect");
        let fit_button = layout.rects.get("line-fit").expect("fit button rect");
        assert!(
            panel_rect.w >= 279.0,
            "controls panel collapsed: {panel_rect:?}"
        );
        assert!(
            scroll_rect.h > 280.0,
            "controls scroll body should fill the titled panel body: panel={panel_rect:?} scroll={scroll_rect:?}"
        );
        assert!(
            first_button.h >= 30.0 && first_button.y >= scroll_rect.y,
            "first control should be visible inside scroll body: scroll={scroll_rect:?} button={first_button:?}"
        );
        assert!(
            flow_rect.h >= reset_button.y + reset_button.h - flow_rect.y - 0.5,
            "flow container should reserve the height of wrapped controls: flow={flow_rect:?} reset={reset_button:?}"
        );
        assert!(
            fit_button.y >= flow_rect.y + flow_rect.h + 7.5,
            "next control should be laid out after wrapped flow controls: flow={flow_rect:?} fit={fit_button:?}"
        );
        assert!(
            layout
                .scroll_max_y
                .get("line-control-scroll")
                .copied()
                .unwrap_or(0.0)
                > 0.0,
            "line controls should remain scrollable: {:?}",
            layout.scroll_max_y
        );
    }

    // -----------------------------------------------------------------------
    // Modal and overlay placement contracts
    // -----------------------------------------------------------------------

    #[test]
    fn open_modal_is_centered_and_does_not_consume_window_flow() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node(
                    "button",
                    WidgetKind::Button,
                    NodeProps {
                        text: Some("Background".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                node(
                    "modal",
                    WidgetKind::Modal,
                    NodeProps {
                        text: Some("Confirm".to_string()),
                        fixed_width: Some(400.0),
                        fixed_height: Some(220.0),
                        open: Some(true),
                        ..NodeProps::default()
                    },
                    vec![node(
                        "ok",
                        WidgetKind::Button,
                        NodeProps {
                            text: Some("OK".to_string()),
                            ..NodeProps::default()
                        },
                        vec![],
                    )],
                ),
            ],
        );

        let layout = compute_layout(&root, 800.0, 600.0, 1.0, &Theme::dark(), None);
        let button = layout.rects.get("button").unwrap();
        let modal = layout.rects.get("modal").unwrap();
        let ok = layout.rects.get("ok").unwrap();

        assert!(
            button.y < 10.0,
            "background flow moved by modal: {button:?}"
        );
        assert!(
            (modal.x - 200.0).abs() < 0.1,
            "modal not centered: {modal:?}"
        );
        assert!(
            (modal.y - 190.0).abs() < 0.1,
            "modal not centered: {modal:?}"
        );
        assert!(
            ok.x > modal.x && ok.y > modal.y,
            "child not inside modal: {ok:?}"
        );
    }

    #[test]
    fn open_modal_before_grid_does_not_shift_reconciled_content() {
        fn card(id: &str, height: f32) -> WidgetNode {
            let mut node = node(id, WidgetKind::Panel, NodeProps::default(), vec![]);
            node.style.layout.height_value = Some(LayoutLength::LogicalPx(height));
            node
        }

        let modal = node(
            "modal",
            WidgetKind::Modal,
            NodeProps {
                text: Some("Overlay".to_string()),
                fixed_width: Some(360.0),
                fixed_height: Some(180.0),
                open: Some(true),
                ..NodeProps::default()
            },
            vec![node("ok", WidgetKind::Button, NodeProps::default(), vec![])],
        );

        let props = NodeProps {
            grid_columns: Some(2),
            grid_min_column_width: Some(120.0),
            grid_masonry: true,
            ..NodeProps::default()
        };
        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            props,
            vec![
                card("tall", 100.0),
                card("short", 40.0),
                card("packed", 40.0),
            ],
        );
        grid.style.layout.gap = Some(10.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                modal,
                node(
                    "content",
                    WidgetKind::VLayout,
                    NodeProps::default(),
                    vec![grid],
                ),
            ],
        );

        let layout = compute_layout(&root, 420.0, 420.0, 1.0, &Theme::dark(), None);
        let content = layout.rects.get("content").unwrap();
        let modal = layout.rects.get("modal").unwrap();
        let packed = layout.rects.get("packed").unwrap();
        let tall = layout.rects.get("tall").unwrap();

        assert!(
            content.y < 1.0,
            "open modal before content should not push content down: content={content:?} modal={modal:?}"
        );
        assert!(
            packed.y < tall.y + tall.h,
            "fixture should still exercise masonry packing: tall={tall:?} packed={packed:?}"
        );
        assert!(
            modal.y > 0.0 && modal.h > 0.0,
            "modal should still receive overlay layout: {modal:?}"
        );
    }

    #[test]
    fn fixed_height_modal_exposes_body_overflow_as_scroll_container() {
        let rows = (0..9)
            .map(|index| {
                node(
                    &format!("row-{index}"),
                    WidgetKind::Label,
                    NodeProps {
                        text: Some(format!("Overflow row {index}")),
                        ..NodeProps::default()
                    },
                    vec![],
                )
            })
            .collect();
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "modal",
                WidgetKind::Modal,
                NodeProps {
                    text: Some("Scrollable modal".to_string()),
                    fixed_width: Some(320.0),
                    fixed_height: Some(150.0),
                    open: Some(true),
                    ..NodeProps::default()
                },
                rows,
            )],
        );
        let state = WidgetState::default();

        let layout = compute_layout(&root, 480.0, 360.0, 1.0, &Theme::dark(), Some(&state));
        let modal = layout.rects.get("modal").unwrap();
        let last = layout.rects.get("row-8").unwrap();
        let max_scroll = layout.scroll_max_y.get("modal").copied().unwrap_or(0.0);

        assert!(
            max_scroll > 0.0,
            "fixed-height modal should be scrollable when body content overflows: modal={modal:?} last={last:?}"
        );
        assert!(
            last.y + last.h > modal.y + modal.h,
            "test fixture should overflow modal body: modal={modal:?} last={last:?}"
        );
    }
}
