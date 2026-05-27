use std::collections::HashMap;

use taffy::prelude::*;
use taffy::style::Overflow;

use crate::document::{WidgetKind, WidgetNode};
use crate::events::WidgetState;
use crate::style::{
    badge_width_for_text, collapsible_header_height_for_style, standalone_badge_width_for_text,
    tabs_header_height_for_style, AlignItemsStyle, DisplayStyle, FlexDirectionStyle,
    GridAutoFlowStyle, GridLineStyle, GridPlacementStyle, GridTemplateAreas,
    GridTrackFitContentSize, GridTrackMaxSize, GridTrackMinSize, GridTrackRepeatKind,
    GridTrackSize, LayoutLength, LineHeight, OverflowStyle, PositionStyle, TextOverflow,
    BADGE_GAP_LP, CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP,
};
use crate::theme::Theme;

const MENU_LABEL_WIDTH_SAFETY_LP: f32 = 6.0;
const PANEL_BODY_VISUAL_INSET_LP: f32 = 1.0;

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
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let root_id = build_node(
        &mut tree,
        root,
        scale_factor,
        theme,
        Some((window_w, window_h)),
        None,
        None,
        false,
        state,
        None,
    );

    tree.compute_layout(
        root_id,
        Size {
            width: AvailableSpace::Definite(window_w),
            height: AvailableSpace::Definite(window_h),
        },
    )
    .expect("taffy layout failed");

    let mut result = LayoutResult {
        scale_factor,
        ..LayoutResult::default()
    };
    collect(&tree, root_id, root, 0.0, 0.0, &mut result);
    apply_titled_container_absolute_offsets(root, &mut result, scale_factor, theme);
    apply_navigation_layout(root, &mut result, scale_factor, theme, state);
    apply_modal_layout(root, &mut result, scale_factor, theme);
    apply_tooltip_layout(root, &mut result, scale_factor, theme, state);
    compute_clips(root, &mut result, scale_factor, theme);
    apply_scroll_offsets(root, &mut result, scale_factor, theme, state);
    apply_fixed_positions(root, &mut result, scale_factor);
    result.clips.clear();
    result.paint_clips.clear();
    compute_clips(root, &mut result, scale_factor, theme);
    result
}

// ---------------------------------------------------------------------------
// Tree builder
// ---------------------------------------------------------------------------

fn build_node(
    tree: &mut TaffyTree<()>,
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    size_override: Option<(f32, f32)>,
    parent_size: Option<(f32, f32)>,
    parent_kind: Option<&WidgetKind>,
    layout_modal_children: bool,
    state: Option<&WidgetState>,
    parent_grid_areas: Option<&GridTemplateAreas>,
) -> NodeId {
    let mut style = style_for(
        node,
        sf,
        theme,
        parent_size,
        parent_kind,
        layout_modal_children,
        state,
    );
    apply_parent_grid_area_placement(&mut style, node, parent_grid_areas);
    if let Some((w, h)) = size_override {
        style.size = taffy::geometry::Size {
            width: Dimension::Length(w),
            height: Dimension::Length(h),
        };
    }
    let child_parent_size = definite_content_size(&style, parent_size);
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
                    layout_modal_children,
                    state,
                    node.style.layout.grid_template_areas.as_ref(),
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
                    layout_modal_children,
                    state,
                    node.style.layout.grid_template_areas.as_ref(),
                )
            })
            .collect()
    };
    if child_ids.is_empty() {
        tree.new_leaf(style).expect("taffy new_leaf failed")
    } else {
        tree.new_with_children(style, &child_ids)
            .expect("taffy new_with_children failed")
    }
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

// ---------------------------------------------------------------------------
// Style mapping
// ---------------------------------------------------------------------------

// Logical-pixel constants — multiplied by scale_factor before use.
fn style_for(
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_size: Option<(f32, f32)>,
    parent_kind: Option<&WidgetKind>,
    layout_modal_children: bool,
    state: Option<&WidgetState>,
) -> Style {
    let ctrl_h = node_control_height_lp(node, theme) * sf;
    let ctrl_gap = (theme.spacing * 0.75) * sf;
    let panel_pad = (theme.spacing + 2.0) * sf;
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
            flex_shrink: 0.0,
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
            flex_shrink: 0.0,
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
            flex_shrink: 0.0,
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

        WidgetKind::StatusBar => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Center),
            flex_grow: 0.0,
            flex_shrink: 0.0,
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(node.props.fixed_height.unwrap_or(28.0) * sf),
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
                    node.props
                        .fixed_height
                        .unwrap_or_else(|| node_control_height_lp(node, theme))
                        .max(node_control_height_lp(node, theme))
                        * sf,
                ),
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
            let width = match node.props.fixed_width {
                Some(w) => Dimension::Length(w * sf), // logical → physical pixels
                None => Dimension::Auto,
            };
            let flex_grow = if node.props.fixed_width.is_some() {
                0.0
            } else {
                1.0
            };
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                flex_grow,
                flex_shrink: if node.props.fixed_width.is_some() {
                    0.0
                } else {
                    1.0
                },
                size: Size {
                    width,
                    height: Dimension::Auto,
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
        WidgetKind::Button
        | WidgetKind::Dropdown
        | WidgetKind::Menu
        | WidgetKind::MenuItem
        | WidgetKind::NumberInput
        | WidgetKind::NavItem
        | WidgetKind::Tab => Style {
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Length(ctrl_h),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Badge | WidgetKind::Tag => Style {
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Length((node_font_size_lp(node, theme) + 8.0).max(20.0) * sf),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Led => {
            let led_size = node.props.led_size.unwrap_or(14.0).max(1.0) * sf;
            Style {
                size: Size {
                    width: Dimension::Length(led_size),
                    height: Dimension::Length(led_size),
                },
                flex_shrink: 0.0,
                ..Default::default()
            }
        }

        WidgetKind::Checkbox => Style {
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Length(ctrl_h),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Label => Style {
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Length(label_height_lp(node, theme, parent_size) * sf),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Slider | WidgetKind::ProgressBar | WidgetKind::TextInput => Style {
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Length(ctrl_h),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::TextArea => Style {
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Length(text_area_height_lp(node, theme) * sf),
            },
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
            let fixed = width.is_some() || height.is_some();
            Style {
                flex_grow: if fixed { 0.0 } else { 1.0 },
                flex_shrink: if fixed { 0.0 } else { 1.0 },
                size: Size {
                    width: width.unwrap_or(Dimension::Auto),
                    height: height.unwrap_or(Dimension::Auto),
                },
                min_size: Size {
                    width: Dimension::Length(48.0 * sf),
                    height: Dimension::Length(48.0 * sf),
                },
                ..Default::default()
            }
        }

        WidgetKind::HtmlReport => {
            let width = node.props.fixed_width.map(|w| Dimension::Length(w * sf));
            let height = node.props.fixed_height.map(|h| Dimension::Length(h * sf));
            let fixed = width.is_some() || height.is_some();
            Style {
                flex_grow: if fixed { 0.0 } else { 1.0 },
                flex_shrink: 1.0,
                size: Size {
                    width: width.unwrap_or(Dimension::Auto),
                    height: height.unwrap_or(Dimension::Length(360.0 * sf)),
                },
                min_size: Size {
                    width: Dimension::Length(240.0 * sf),
                    height: Dimension::Length(160.0 * sf),
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
            let grow = if width.is_none() && height.is_none() {
                1.0
            } else {
                0.0
            };
            Style {
                flex_grow: grow,
                flex_shrink: if grow > 0.0 { 1.0 } else { 0.0 },
                size: Size {
                    width: width.unwrap_or(Dimension::Auto),
                    height: height.unwrap_or(Dimension::Auto),
                },
                ..Default::default()
            }
        }

        // ── plot / table: grow to fill remaining space ────────────────────
        WidgetKind::AttitudeSphere
        | WidgetKind::TranslationTrace
        | WidgetKind::PieChart
        | WidgetKind::Histogram
        | WidgetKind::LinePlot
        | WidgetKind::Scatter3D
        | WidgetKind::DataFrameTable
        | WidgetKind::Tabs
        | WidgetKind::Pages => Style {
            flex_grow: 1.0,
            flex_shrink: 0.0,
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Auto,
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
            ..Default::default()
        },

        WidgetKind::Unknown => Style {
            flex_grow: 1.0,
            ..Default::default()
        },
    };
    apply_intrinsic_leaf_width(&mut style, node, parent_kind, sf, theme, parent_size);
    if !matches!(node.kind, WidgetKind::Tooltip | WidgetKind::Toast) {
        apply_node_style(&mut style, node, sf, parent_size);
    }
    apply_grid_layout_default_tracks(&mut style, node, sf, parent_size);
    apply_flow_layout_intrinsic_height(&mut style, node, sf, theme, parent_size);
    apply_flow_layout_alignment(&mut style, node);
    if !titled_container_uses_body_layout(node)
        && (node.kind != WidgetKind::Modal || layout_modal_children)
        && node.kind != WidgetKind::Collapsible
    {
        reserve_panel_title_space(&mut style, node, sf, theme);
    }
    style
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
    let top_margin = (panel_title_line_height_lp(node, theme)
        + panel_title_body_gap_lp(node, theme)
        + PANEL_BODY_VISUAL_INSET_LP)
        * sf;
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
        gap: taffy::geometry::Size {
            width: column_gap,
            height: row_gap,
        },
        ..Default::default()
    }
}

fn apply_grid_layout_default_tracks(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    parent_size: Option<(f32, f32)>,
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
    let min_fn = node
        .props
        .grid_min_column_width
        .map(|w| MinTrackSizingFunction::Fixed(LengthPercentage::Length((w * sf).max(1.0))));
    style.grid_template_columns = match (node.props.grid_columns, min_fn) {
        (Some(max_columns), Some(min)) => repeat_grid_track(
            responsive_grid_column_count(
                max_columns.max(1),
                grid_min_track_width_px(&min),
                grid_available_width_px(style, parent_size),
                grid_column_gap_px(style),
            ) as usize,
            min,
        ),
        (Some(columns), None) => {
            repeat_grid_track(columns.max(1) as usize, MinTrackSizingFunction::Auto)
        }
        (None, Some(min)) => vec![TrackSizingFunction::Repeat(
            GridTrackRepetition::AutoFill,
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
    style.justify_content = match node.props.flow_align.as_deref().unwrap_or("start") {
        "center" => Some(JustifyContent::Center),
        "end" => Some(JustifyContent::FlexEnd),
        _ => Some(JustifyContent::FlexStart),
    };
    style.align_items = match node.props.flow_cross_align.as_deref().unwrap_or("start") {
        "center" => Some(AlignItems::Center),
        "end" => Some(AlignItems::FlexEnd),
        "stretch" => Some(AlignItems::Stretch),
        _ => Some(AlignItems::FlexStart),
    };
}

fn apply_flow_layout_intrinsic_height(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_size: Option<(f32, f32)>,
) {
    if node.kind != WidgetKind::FlowLayout
        || node.children.is_empty()
        || node.style.layout.height.is_some()
        || node.style.layout.height_value.is_some()
    {
        return;
    }
    let Some(available_w) = flow_layout_available_width_px(style, parent_size) else {
        return;
    };
    if available_w <= 0.0 {
        return;
    }
    let column_gap = flow_layout_gap_px(node, sf, parent_size.map(|size| size.0));
    let row_gap = flow_layout_row_gap_px(node, sf, parent_size.map(|size| size.1));
    let mut line_w = 0.0_f32;
    let mut line_h = 0.0_f32;
    let mut total_h = 0.0_f32;
    let mut has_line = false;

    for child in &node.children {
        if is_fixed_positioned_node(child) || child.style.layout.display == Some(DisplayStyle::None)
        {
            continue;
        }
        let child_w = flow_child_width_px(child, theme, sf, available_w).min(available_w);
        let child_h = flow_child_height_px(child, theme, sf);
        let needed_w = if has_line {
            line_w + column_gap + child_w
        } else {
            child_w
        };
        if has_line && needed_w > available_w {
            total_h += line_h + row_gap;
            line_w = child_w;
            line_h = child_h;
        } else {
            line_w = needed_w;
            line_h = line_h.max(child_h);
        }
        has_line = true;
    }

    if has_line {
        total_h += line_h;
        let padded_h = total_h + lp_value(style.padding.top) + lp_value(style.padding.bottom);
        style.min_size.height = max_dimension_length(style.min_size.height, padded_h);
    }
}

fn flow_layout_available_width_px(style: &Style, parent_size: Option<(f32, f32)>) -> Option<f32> {
    let parent_width = parent_size.map(|size| size.0);
    let width = resolve_dimension_px(style.size.width, parent_width).or(parent_width)?;
    Some((width - lp_value(style.padding.left) - lp_value(style.padding.right)).max(0.0))
}

fn flow_layout_gap_px(node: &WidgetNode, sf: f32, parent_axis: Option<f32>) -> f32 {
    layout_length_percentage(
        node.style
            .layout
            .column_gap_value
            .or(node.style.layout.gap_value),
        node.style.layout.column_gap.or(node.style.layout.gap),
        sf,
        parent_axis,
    )
    .map(lp_value)
    .unwrap_or(0.0)
}

fn flow_layout_row_gap_px(node: &WidgetNode, sf: f32, parent_axis: Option<f32>) -> f32 {
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

fn flow_child_width_px(child: &WidgetNode, theme: &Theme, sf: f32, parent_width: f32) -> f32 {
    let width = layout_dimension(
        child.style.layout.width_value,
        child.style.layout.width,
        sf,
        Some(parent_width),
    )
    .and_then(|dimension| resolve_dimension_px(dimension, Some(parent_width)))
    .or_else(|| child.props.fixed_width.map(|width| width * sf))
    .or_else(|| intrinsic_leaf_width(child, theme).map(|width| width * sf))
    .unwrap_or(0.0)
    .max(0.0);
    if let Some(max_width) = max_width_px(child, sf, Some(parent_width)) {
        width.min(max_width)
    } else {
        width
    }
}

fn flow_child_height_px(child: &WidgetNode, theme: &Theme, sf: f32) -> f32 {
    layout_dimension(
        child.style.layout.height_value,
        child.style.layout.height,
        sf,
        None,
    )
    .and_then(|dimension| resolve_dimension_px(dimension, None))
    .or_else(|| child.props.fixed_height.map(|height| height * sf))
    .unwrap_or_else(|| default_leaf_height_px(child, theme, sf))
    .max(0.0)
}

fn default_leaf_height_px(node: &WidgetNode, theme: &Theme, sf: f32) -> f32 {
    match node.kind {
        WidgetKind::Button
        | WidgetKind::Dropdown
        | WidgetKind::Menu
        | WidgetKind::MenuItem
        | WidgetKind::NumberInput
        | WidgetKind::NavItem
        | WidgetKind::Tab
        | WidgetKind::Checkbox
        | WidgetKind::Slider
        | WidgetKind::ProgressBar
        | WidgetKind::TextInput => node_control_height_lp(node, theme) * sf,
        WidgetKind::Label => label_height_lp(node, theme, None) * sf,
        WidgetKind::Badge | WidgetKind::Tag => {
            (node_font_size_lp(node, theme) + 8.0).max(20.0) * sf
        }
        WidgetKind::Led => node.props.led_size.unwrap_or(14.0).max(1.0) * sf,
        WidgetKind::TextArea => text_area_height_lp(node, theme) * sf,
        WidgetKind::HtmlReport => node.props.fixed_height.unwrap_or(360.0) * sf,
        WidgetKind::Separator => sf,
        _ => 0.0,
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

fn reserve_panel_title_space(style: &mut Style, node: &WidgetNode, sf: f32, theme: &Theme) {
    if !matches!(
        node.kind,
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal
    ) || !node.props.text.as_deref().is_some_and(|t| !t.is_empty())
    {
        return;
    }
    let title_inset = (panel_title_line_height_lp(node, theme)
        + panel_title_body_gap_lp(node, theme)
        + PANEL_BODY_VISUAL_INSET_LP)
        * sf;
    style.padding.top = match style.padding.top {
        LengthPercentage::Length(top) => LengthPercentage::Length(top + title_inset),
        _ => LengthPercentage::Length(title_inset),
    };
}

pub(crate) fn panel_title_line_height_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let font_size = node_font_size_lp(node, theme);
    match node.style.text.line_height {
        Some(LineHeight::Multiplier(value)) => (font_size * value.max(0.1)).max(1.0),
        Some(LineHeight::LogicalPx(value)) => value.max(1.0),
        None => (font_size + 5.0).max(theme.font_size + 3.0),
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
    let default = theme.spacing + 2.0;
    let layout = &node.style.layout;
    layout
        .padding_top
        .or(layout.padding)
        .unwrap_or(default)
        .max(0.0)
}

fn apply_intrinsic_leaf_width(
    style: &mut Style,
    node: &WidgetNode,
    parent_kind: Option<&WidgetKind>,
    sf: f32,
    theme: &Theme,
    parent_size: Option<(f32, f32)>,
) {
    if node.style.layout.width.is_some()
        || node.style.layout.width_value.is_some()
        || node.style.layout.min_width.is_some()
        || node.style.layout.min_width_value.is_some()
    {
        return;
    }
    if !matches!(
        parent_kind,
        Some(
            WidgetKind::HLayout | WidgetKind::StatusBar | WidgetKind::Tabs | WidgetKind::FlowLayout
        ) | Some(WidgetKind::GridLayout)
            | Some(WidgetKind::MenuBar)
    ) {
        return;
    }

    let Some(width) = intrinsic_leaf_width(node, theme) else {
        return;
    };
    let width_px = max_width_px(node, sf, parent_size.map(|size| size.0))
        .map(|max_width| (width * sf).min(max_width))
        .unwrap_or(width * sf);
    style.min_size.width = Dimension::Length(width_px);
}

fn max_width_px(node: &WidgetNode, sf: f32, parent_width: Option<f32>) -> Option<f32> {
    layout_dimension(
        node.style.layout.max_width_value,
        node.style.layout.max_width,
        sf,
        parent_width,
    )
    .and_then(|dimension| resolve_dimension_px(dimension, parent_width))
    .map(|width| width.max(0.0))
}

fn intrinsic_leaf_width(node: &WidgetNode, theme: &Theme) -> Option<f32> {
    let text = intrinsic_text(node);
    let text_w = text.map(|t| estimate_text_width(t, node_font_size_lp(node, theme)));
    let pad = theme.spacing * 2.0;
    let badge_w = badge_extra_width(node, theme);
    match node.kind {
        WidgetKind::Button => Some((text_w.unwrap_or(0.0) + pad + badge_w).clamp(72.0, 280.0)),
        WidgetKind::Badge | WidgetKind::Tag => node
            .props
            .text
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(|text| {
                standalone_badge_width_for_text(&node.style, text, theme, 1.0).clamp(24.0, 220.0)
            }),
        WidgetKind::Menu => {
            let menu_pad = theme.spacing;
            Some((text_w.unwrap_or(0.0) + menu_pad + MENU_LABEL_WIDTH_SAFETY_LP).clamp(28.0, 180.0))
        }
        WidgetKind::Dropdown => Some((text_w.unwrap_or(0.0) + pad + 22.0).clamp(112.0, 260.0)),
        WidgetKind::NumberInput => Some((text_w.unwrap_or(0.0) + pad + 34.0).clamp(96.0, 220.0)),
        WidgetKind::TextInput => Some((text_w.unwrap_or(0.0) + pad).clamp(120.0, 280.0)),
        WidgetKind::TextArea => Some((text_w.unwrap_or(0.0) + pad).clamp(180.0, 420.0)),
        WidgetKind::Checkbox => Some(
            (text_w.unwrap_or(0.0) + CHECKBOX_LEFT_PAD_LP + checkbox_box_width_lp(node) + pad)
                .clamp(48.0, 280.0),
        ),
        WidgetKind::Label | WidgetKind::NavItem | WidgetKind::Tab => {
            Some((text_w.unwrap_or(0.0) + pad + badge_w).clamp(32.0, 320.0))
        }
        WidgetKind::Slider => Some(140.0),
        WidgetKind::ProgressBar => Some(160.0),
        _ => None,
    }
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

fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let chars = text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as f32;
    chars * font_size * 0.56
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

fn label_height_lp(node: &WidgetNode, theme: &Theme, parent_size: Option<(f32, f32)>) -> f32 {
    let control_h = node_control_height_lp(node, theme);
    if !label_wraps(node) {
        return control_h;
    }
    let Some(text) = node.props.text.as_deref().filter(|text| !text.is_empty()) else {
        return control_h;
    };
    let Some((parent_width, _)) = parent_size else {
        return control_h;
    };
    let font_size = node_font_size_lp(node, theme);
    let line_height = node_line_height_lp(node, theme);
    let available_width = parent_width.max(font_size);
    (estimate_wrapped_text_lines(text, font_size, available_width) as f32 * line_height)
        .max(control_h)
}

fn label_wraps(node: &WidgetNode) -> bool {
    node.props.wrap.unwrap_or(true) && node.style.text.text_overflow != Some(TextOverflow::Ellipsis)
}

fn node_line_height_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let font_size = node_font_size_lp(node, theme);
    match node.style.text.line_height {
        Some(LineHeight::Multiplier(value)) => (font_size * value.max(0.1)).max(1.0),
        Some(LineHeight::LogicalPx(value)) => value.max(1.0),
        None => (font_size + 5.0).max(theme.font_size + 3.0),
    }
}

fn estimate_wrapped_text_lines(text: &str, font_size: f32, available_width: f32) -> usize {
    let approx_char_width = (font_size * 0.56).max(1.0);
    let max_chars = (available_width / approx_char_width).floor().max(1.0) as usize;
    text.lines()
        .map(|line| estimate_wrapped_line_count(line, max_chars))
        .sum::<usize>()
        .max(1)
}

fn estimate_wrapped_line_count(line: &str, max_chars: usize) -> usize {
    if line.trim().is_empty() {
        return 1;
    }
    let mut lines = 1usize;
    let mut current = 0usize;
    for word in line.split_whitespace() {
        let word_len = word.chars().count();
        if current == 0 {
            current = word_len;
            while current > max_chars {
                lines += 1;
                current = current.saturating_sub(max_chars);
            }
        } else if current + 1 + word_len <= max_chars {
            current += 1 + word_len;
        } else {
            lines += 1;
            current = word_len;
            while current > max_chars {
                lines += 1;
                current = current.saturating_sub(max_chars);
            }
        }
    }
    lines
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

fn node_font_size_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    node.style
        .text
        .font_size
        .unwrap_or(theme.font_size)
        .max(8.0)
}

fn node_control_height_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let font_size = node_font_size_lp(node, theme);
    (font_size + theme.spacing * 2.0 + 4.0).max(28.0)
}

fn apply_node_style(
    style: &mut Style,
    node: &WidgetNode,
    sf: f32,
    parent_size: Option<(f32, f32)>,
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
    if (layout.width.is_some()
        || layout.height.is_some()
        || layout.width_value.is_some()
        || layout.height_value.is_some())
        && layout.flex_grow.is_none()
    {
        style.flex_grow = 0.0;
    }
    if (layout.width.is_some()
        || layout.height.is_some()
        || layout.width_value.is_some()
        || layout.height_value.is_some())
        && layout.flex_shrink.is_none()
    {
        style.flex_shrink = 0.0;
    }
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
        style.grid_template_columns = tracks
            .iter()
            .cloned()
            .map(|track| grid_track_size(track, sf))
            .collect();
    }
    if let Some(tracks) = &layout.grid_template_rows {
        style.grid_template_rows = tracks
            .iter()
            .cloned()
            .map(|track| grid_track_size(track, sf))
            .collect();
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
            left: layout_length_percentage_auto(
                layout.margin_left_value.or(margin_all_value),
                layout.margin_left.or(layout.margin),
                sf,
                parent_width,
            )
            .unwrap_or(current.left),
            right: layout_length_percentage_auto(
                layout.margin_right_value.or(margin_all_value),
                layout.margin_right.or(layout.margin),
                sf,
                parent_width,
            )
            .unwrap_or(current.right),
            top: layout_length_percentage_auto(
                layout.margin_top_value.or(margin_all_value),
                layout.margin_top.or(layout.margin),
                sf,
                parent_width,
            )
            .unwrap_or(current.top),
            bottom: layout_length_percentage_auto(
                layout.margin_bottom_value.or(margin_all_value),
                layout.margin_bottom.or(layout.margin),
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
            left: layout_length_percentage(
                layout.padding_left_value.or(pad_all_value),
                layout.padding_left.or(layout.padding),
                sf,
                parent_width,
            )
            .unwrap_or(current.left),
            right: layout_length_percentage(
                layout.padding_right_value.or(pad_all_value),
                layout.padding_right.or(layout.padding),
                sf,
                parent_width,
            )
            .unwrap_or(current.right),
            top: layout_length_percentage(
                layout.padding_top_value.or(pad_all_value),
                layout.padding_top.or(layout.padding),
                sf,
                parent_width,
            )
            .unwrap_or(current.top),
            bottom: layout_length_percentage(
                layout.padding_bottom_value.or(pad_all_value),
                layout.padding_bottom.or(layout.padding),
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
    let width = definite_dimension(style.size.width).or_else(|| parent_size.map(|size| size.0))?;
    let height =
        definite_dimension(style.size.height).or_else(|| parent_size.map(|size| size.1))?;
    let padding_x = lp_value(style.padding.left) + lp_value(style.padding.right);
    let padding_y = lp_value(style.padding.top) + lp_value(style.padding.bottom);
    Some(((width - padding_x).max(0.0), (height - padding_y).max(0.0)))
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

fn collect(
    tree: &TaffyTree<()>,
    node_id: NodeId,
    widget: &WidgetNode,
    parent_x: f32,
    parent_y: f32,
    result: &mut LayoutResult,
) {
    let layout = tree.layout(node_id).expect("taffy layout missing");
    let abs_x = parent_x + layout.location.x;
    let abs_y = parent_y + layout.location.y;
    result.rects.insert(
        widget.id.clone(),
        Rect {
            x: abs_x,
            y: abs_y,
            w: layout.size.width,
            h: layout.size.height,
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

fn compute_clips(root: &WidgetNode, result: &mut LayoutResult, sf: f32, theme: &Theme) {
    result.clips.clear();
    result.paint_clips.clear();
    let Some(root_rect) = result.rects.get(&root.id).copied() else {
        return;
    };
    compute_node_clips(root, result, root_rect, root_rect, sf, theme);
}

fn compute_node_clips(
    node: &WidgetNode,
    result: &mut LayoutResult,
    parent_clip: Rect,
    root_clip: Rect,
    sf: f32,
    theme: &Theme,
) {
    let Some(rect) = result.rects.get(&node.id).copied() else {
        return;
    };
    let parent_clip = if is_fixed_positioned_node(node) {
        root_clip
    } else {
        parent_clip
    };
    result.paint_clips.insert(node.id.clone(), parent_clip);
    let clip = rect.intersect(parent_clip).unwrap_or(Rect {
        x: rect.x,
        y: rect.y,
        w: 0.0,
        h: 0.0,
    });
    result.clips.insert(node.id.clone(), clip);
    let child_clip = scroll_container_child_clip(node, result, clip, sf, theme)
        .unwrap_or_else(|| child_clip_for_overflow(node, parent_clip, clip));
    for child in &node.children {
        compute_node_clips(child, result, child_clip, root_clip, sf, theme);
    }
}

pub(crate) fn is_scroll_container_kind(kind: &WidgetKind) -> bool {
    matches!(
        kind,
        WidgetKind::Panel | WidgetKind::Page | WidgetKind::Sidebar
    )
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
    scroll_container_max_x_with_viewport(node, result, false, result.scale_factor, &Theme::dark())
}

fn scroll_container_max_x_with_viewport(
    node: &WidgetNode,
    result: &LayoutResult,
    use_own_viewport: bool,
    sf: f32,
    theme: &Theme,
) -> f32 {
    if !scroll_container_scrolls_x(node) {
        return 0.0;
    }
    let Some(rect) = result.rects.get(&node.id).copied() else {
        return 0.0;
    };
    let viewport = if use_own_viewport {
        scroll_container_body_viewport(node, rect, sf, theme)
    } else {
        let clip = result.clips.get(&node.id).copied().unwrap_or(rect);
        let body = scroll_container_body_viewport(node, rect, sf, theme);
        clip.intersect(body).unwrap_or(Rect {
            x: body.x,
            y: body.y,
            w: 0.0,
            h: 0.0,
        })
    };
    let Some(content) = scroll_content_bounds(node, result) else {
        return 0.0;
    };
    (content.right - (viewport.x + viewport.w)).max(0.0)
}

pub(crate) fn scroll_container_max_y(node: &WidgetNode, result: &LayoutResult) -> f32 {
    scroll_container_max_y_with_viewport(node, result, false, result.scale_factor, &Theme::dark())
}

fn scroll_container_max_y_with_viewport(
    node: &WidgetNode,
    result: &LayoutResult,
    use_own_viewport: bool,
    sf: f32,
    theme: &Theme,
) -> f32 {
    if !scroll_container_scrolls_y(node) {
        return 0.0;
    }
    let Some(rect) = result.rects.get(&node.id).copied() else {
        return 0.0;
    };
    let viewport = if use_own_viewport {
        scroll_container_body_viewport(node, rect, sf, theme)
    } else {
        let clip = result.clips.get(&node.id).copied().unwrap_or(rect);
        let body = scroll_container_body_viewport(node, rect, sf, theme);
        clip.intersect(body).unwrap_or(Rect {
            x: body.x,
            y: body.y,
            w: 0.0,
            h: 0.0,
        })
    };
    let Some(content) = scroll_content_bounds(node, result) else {
        return 0.0;
    };
    (content.bottom - (viewport.y + viewport.h)).max(0.0)
}

fn scroll_container_body_viewport(node: &WidgetNode, rect: Rect, sf: f32, theme: &Theme) -> Rect {
    if !node
        .props
        .text
        .as_deref()
        .is_some_and(|text| !text.is_empty())
        || !matches!(
            node.kind,
            WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal
        )
    {
        return rect;
    }
    let title_inset = titled_container_body_offset_px(node, sf, theme).min(rect.h.max(0.0));
    Rect {
        x: rect.x,
        y: rect.y + title_inset,
        w: rect.w,
        h: (rect.h - title_inset).max(0.0),
    }
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
    let rect = result.rects.get(&node.id).copied()?;
    let content = scroll_container_body_viewport(node, rect, sf, theme);
    clip.intersect(content).or(Some(Rect {
        x: clip.x,
        y: content.y.max(clip.y),
        w: 0.0,
        h: 0.0,
    }))
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

#[derive(Debug, Clone, Copy)]
struct ScrollContentBounds {
    right: f32,
    bottom: f32,
}

fn scroll_content_bounds(node: &WidgetNode, result: &LayoutResult) -> Option<ScrollContentBounds> {
    let mut right = f32::NEG_INFINITY;
    let mut top = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for child in &node.children {
        if is_fixed_positioned_node(child) {
            continue;
        }
        if let Some(bounds) = scroll_content_subtree_bounds(child, result) {
            right = right.max(bounds.right);
            top = top.min(bounds.top);
            bottom = bottom.max(bounds.bottom);
        }
    }
    if top.is_finite() {
        let scale_factor = if result.scale_factor > 0.0 {
            result.scale_factor
        } else {
            1.0
        };
        right += scroll_container_right_padding_lp(node) * scale_factor;
        bottom += scroll_container_bottom_padding_lp(node) * scale_factor;
        Some(ScrollContentBounds { right, bottom })
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct ScrollContentSubtreeBounds {
    right: f32,
    top: f32,
    bottom: f32,
}

fn scroll_content_subtree_bounds(
    node: &WidgetNode,
    result: &LayoutResult,
) -> Option<ScrollContentSubtreeBounds> {
    let rect = result.rects.get(&node.id)?;
    let mut bounds = ScrollContentSubtreeBounds {
        right: rect.x + rect.w,
        top: rect.y,
        bottom: rect.y + rect.h,
    };

    if is_scroll_container_node(node) {
        return Some(bounds);
    }

    for child in &node.children {
        if is_fixed_positioned_node(child) {
            continue;
        }
        if let Some(child_bounds) = scroll_content_subtree_bounds(child, result) {
            bounds.right = bounds.right.max(child_bounds.right);
            bounds.top = bounds.top.min(child_bounds.top);
            bounds.bottom = bounds.bottom.max(child_bounds.bottom);
        }
    }
    Some(bounds)
}

fn scroll_container_right_padding_lp(node: &WidgetNode) -> f32 {
    node.style
        .layout
        .padding_right
        .or(node.style.layout.padding)
        .unwrap_or(10.0)
        .max(0.0)
}

fn scroll_container_bottom_padding_lp(node: &WidgetNode) -> f32 {
    node.style
        .layout
        .padding_bottom
        .or(node.style.layout.padding)
        .unwrap_or(10.0)
        .max(0.0)
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
        let max_scroll_x =
            scroll_container_max_x_with_viewport(node, result, use_own_viewport, sf, theme);
        let max_scroll_y =
            scroll_container_max_y_with_viewport(node, result, use_own_viewport, sf, theme);
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

fn apply_titled_container_absolute_offsets(
    node: &WidgetNode,
    result: &mut LayoutResult,
    sf: f32,
    theme: &Theme,
) {
    if titled_container_has_body_offset(node) {
        let body_offset = titled_container_body_offset_px(node, sf, theme);
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

fn titled_container_body_offset_px(node: &WidgetNode, sf: f32, theme: &Theme) -> f32 {
    (panel_title_top_padding_lp(node, theme)
        + panel_title_line_height_lp(node, theme)
        + panel_title_body_gap_lp(node, theme))
        * sf
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
    let pad = (theme.spacing + 2.0) * sf;
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
        WidgetKind::TextArea => text_area_height_lp(node, theme) * sf,
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
    let margin = (theme.spacing * 3.0 * sf).max(16.0 * sf);
    let max_w = (root_rect.w - margin * 2.0).max(80.0 * sf);
    let max_h = (root_rect.h - margin * 2.0).max(80.0 * sf);
    let modal_w = modal
        .props
        .fixed_width
        .map(|w| w * sf)
        .unwrap_or(420.0 * sf)
        .clamp(80.0 * sf, max_w);
    let modal_h = modal
        .props
        .fixed_height
        .map(|h| h * sf)
        .unwrap_or(220.0 * sf)
        .clamp(80.0 * sf, max_h);
    let x = root_rect.x + (root_rect.w - modal_w) * 0.5;
    let y = root_rect.y + (root_rect.h - modal_h) * 0.5;

    let mut tree: TaffyTree<()> = TaffyTree::new();
    let root_id = build_node(
        &mut tree,
        modal,
        sf,
        theme,
        Some((modal_w, modal_h)),
        None,
        None,
        true,
        None,
        None,
    );
    tree.compute_layout(
        root_id,
        Size {
            width: AvailableSpace::Definite(modal_w),
            height: AvailableSpace::Definite(modal_h),
        },
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
    let tab_w = (r.w / tabs.len() as f32).max(1.0);
    for (idx, tab) in tabs.iter().enumerate() {
        result.rects.insert(
            tab.id.clone(),
            Rect {
                x: r.x + idx as f32 * tab_w,
                y: r.y,
                w: tab_w,
                h: header_h,
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
        kind: WidgetKind::VLayout,
        props: Default::default(),
        style: Default::default(),
        style_json: Default::default(),
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
        kind: WidgetKind::VLayout,
        props: Default::default(),
        style: container.style.clone(),
        style_json: Default::default(),
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
    use crate::document::NodeProps;
    use std::hint::black_box;
    use std::time::Instant;

    fn node(id: &str, kind: WidgetKind, props: NodeProps, children: Vec<WidgetNode>) -> WidgetNode {
        WidgetNode {
            id: id.to_string(),
            key: None,
            class_name: None,
            kind,
            props,
            style_json: Default::default(),
            inline_style: Default::default(),
            style: Default::default(),
            children,
        }
    }

    fn count_widgets(node: &WidgetNode) -> usize {
        1 + node.children.iter().map(count_widgets).sum::<usize>()
    }

    fn make_layout_benchmark_tree(rows: usize, cols: usize) -> WidgetNode {
        let mut panels = Vec::with_capacity(rows);
        for row in 0..rows {
            let mut children = Vec::with_capacity(cols);
            for col in 0..cols {
                let idx = row * cols + col;
                let mut props = NodeProps::default();
                props.text = Some(format!("Metric {idx}: {}", idx % 100));
                props.value = Some((idx % 100) as f32);
                props.fixed_height = Some(if idx % 4 == 0 { 24.0 } else { 20.0 });
                let kind = match idx % 6 {
                    0 => WidgetKind::Label,
                    1 => WidgetKind::Button,
                    2 => WidgetKind::Slider,
                    3 => WidgetKind::Checkbox,
                    4 => WidgetKind::Badge,
                    _ => WidgetKind::ProgressBar,
                };
                children.push(node(&format!("control-{idx}"), kind, props, Vec::new()));
            }
            panels.push(node(
                &format!("panel-{row}"),
                WidgetKind::FlowLayout,
                NodeProps {
                    fixed_height: Some(120.0),
                    flow_align: Some("start".to_string()),
                    flow_cross_align: Some("start".to_string()),
                    ..NodeProps::default()
                },
                children,
            ));
        }
        node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "main",
                WidgetKind::HLayout,
                NodeProps::default(),
                vec![
                    node(
                        "nav",
                        WidgetKind::ScrollArea,
                        NodeProps {
                            fixed_width: Some(220.0),
                            ..NodeProps::default()
                        },
                        panels[..10.min(panels.len())].to_vec(),
                    ),
                    node(
                        "content-scroll",
                        WidgetKind::ScrollArea,
                        NodeProps::default(),
                        panels,
                    ),
                ],
            )],
        )
    }

    #[test]
    #[ignore = "benchmark"]
    fn bench_layout_many_widgets() {
        let root = make_layout_benchmark_tree(80, 20);
        let widget_count = count_widgets(&root);
        let theme = Theme::dark();
        let iterations = std::env::var("DRAGONGUI_BENCH_ITERS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(40);

        let start = Instant::now();
        for _ in 0..iterations {
            let layout = compute_layout(&root, 1280.0, 720.0, 1.0, &theme, None);
            black_box(layout);
        }
        let elapsed = start.elapsed();
        let ns_per_widget = elapsed.as_nanos() / (iterations as u128 * widget_count as u128);
        println!(
            "layout many widgets: {ns_per_widget} ns/widget ({widget_count} widgets, {iterations} iters, {:?})",
            elapsed
        );
    }

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
            badge_rect.w >= 135.0,
            "margin-auto badge should get enough intrinsic width, got {}",
            badge_rect.w
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

        assert!(badge.w >= 24.0);
        assert_eq!(badge.h, 22.0);
        assert!(tag.w > badge.w);
        assert_eq!(tag.h, 22.0);
    }

    #[test]
    fn standalone_badge_respects_max_width_intrinsic_cap() {
        let mut badge = node(
            "badge",
            WidgetKind::Badge,
            NodeProps {
                text: Some("HtmlReport".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        badge.style.layout.max_width = Some(72.0);

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

        let layout = compute_layout(&root, 220.0, 90.0, 1.0, &Theme::dark(), None);
        let badge = layout.rects.get("badge").unwrap();

        assert!(
            badge.w <= 72.5,
            "badge max_width should cap intrinsic pill width, got {badge:?}"
        );
        assert_eq!(badge.h, 22.0);
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

        assert!(
            tag.w >= 36.0,
            "tag width should be intrinsic, got {}",
            tag.w
        );
        assert_eq!(tag.h, 22.0);
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
            headline.h >= 50.0,
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

        let layout = compute_layout(&root, 420.0, 260.0, 1.0, &Theme::dark(), None);
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

        assert_eq!(menu_bar.h, Theme::dark().control_height());
        assert!(file.w >= 44.0, "file menu collapsed: {file:?}");
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
        let text_w = estimate_text_width("Debug", theme.font_size);
        let available_text_w = debug.w - theme.spacing;

        assert!(
            available_text_w >= text_w + MENU_LABEL_WIDTH_SAFETY_LP - 0.5,
            "menu label can clip: rect={debug:?}, available={available_text_w}, estimated={text_w}"
        );
    }

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
        let flexible = node("flexible", WidgetKind::Panel, NodeProps::default(), vec![]);
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
        let expected_body_top = panel_rect.y
            + titled_container_body_offset_px(&panel, 1.0, &theme)
            + PANEL_BODY_VISUAL_INSET_LP;

        assert_eq!(pin.x, panel_rect.x + theme.spacing + 2.0 + 16.0);
        assert_eq!(pin.y, expected_body_top + 18.0);
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
    fn page_scroll_range_includes_overflowing_grid_descendants() {
        let mut tall_panel = node(
            "tall-panel",
            WidgetKind::Panel,
            NodeProps::default(),
            vec![node(
                "tall-child",
                WidgetKind::Label,
                NodeProps {
                    text: Some("Tall content".to_string()),
                    ..NodeProps::default()
                },
                vec![],
            )],
        );
        tall_panel.style.layout.height = Some(420.0);

        let mut grid = node(
            "grid",
            WidgetKind::GridLayout,
            NodeProps::default(),
            vec![tall_panel],
        );
        grid.style.layout.height = Some(160.0);
        grid.style.layout.overflow_y = Some(OverflowStyle::Visible);

        let mut page = node("page", WidgetKind::Page, NodeProps::default(), vec![grid]);
        page.style.layout.overflow_y = Some(OverflowStyle::Auto);
        page.style.layout.padding = Some(0.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![page],
        );

        let layout = compute_layout(
            &root,
            320.0,
            240.0,
            1.0,
            &Theme::dark(),
            Some(&WidgetState::default()),
        );
        let page_rect = layout.rects.get("page").expect("page rect");
        let tall_rect = layout.rects.get("tall-panel").expect("tall panel rect");
        let max_scroll_y = layout.scroll_max_y.get("page").copied().unwrap_or(0.0);

        assert!(
            tall_rect.y + tall_rect.h > page_rect.y + page_rect.h,
            "test fixture should overflow through grid descendant: page={page_rect:?} tall={tall_rect:?}"
        );
        assert!(
            max_scroll_y > 0.0,
            "page should get scroll range from overflowing grid descendants: {:?}",
            layout.scroll_max_y
        );
    }

    #[test]
    fn parent_scroll_range_stops_at_nested_scroll_container() {
        let mut inner_rows = Vec::new();
        for index in 0..12 {
            inner_rows.push(node(
                &format!("row-{index}"),
                WidgetKind::Button,
                NodeProps {
                    text: Some(format!("Row {index}")),
                    ..NodeProps::default()
                },
                vec![],
            ));
        }

        let mut inner = node(
            "inner-scroll",
            WidgetKind::ScrollArea,
            NodeProps::default(),
            inner_rows,
        );
        inner.style.layout.height = Some(120.0);
        inner.style.layout.overflow_y = Some(OverflowStyle::Auto);

        let mut page = node("page", WidgetKind::Page, NodeProps::default(), vec![inner]);
        page.style.layout.overflow_y = Some(OverflowStyle::Auto);
        page.style.layout.padding = Some(0.0);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![page],
        );

        let layout = compute_layout(
            &root,
            320.0,
            240.0,
            1.0,
            &Theme::dark(),
            Some(&WidgetState::default()),
        );
        assert_eq!(layout.scroll_max_y.get("page").copied(), Some(0.0));
        assert!(
            layout
                .scroll_max_y
                .get("inner-scroll")
                .copied()
                .unwrap_or(0.0)
                > 0.0,
            "nested scroll area should own its own overflow: {:?}",
            layout.scroll_max_y
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
    fn sidebar_flow_badges_respect_scrollbar_gutter_and_max_width() {
        let mut html = node(
            "html",
            WidgetKind::Tag,
            NodeProps {
                text: Some("HtmlReport".to_string()),
                ..NodeProps::default()
            },
            vec![],
        );
        html.style.layout.max_width = Some(74.0);

        let mut badges = node(
            "badges",
            WidgetKind::FlowLayout,
            NodeProps::default(),
            vec![
                node(
                    "grid",
                    WidgetKind::Badge,
                    NodeProps {
                        text: Some("Grid".to_string()),
                        ..NodeProps::default()
                    },
                    vec![],
                ),
                html,
            ],
        );
        badges.style.layout.width_value = Some(LayoutLength::Percent(100.0));
        badges.style.layout.gap = Some(6.0);
        badges.style.layout.row_gap = Some(4.0);

        let mut sidebar = node(
            "sidebar",
            WidgetKind::Sidebar,
            NodeProps::default(),
            vec![
                badges,
                node("nav", WidgetKind::NavItem, NodeProps::default(), vec![]),
            ],
        );
        sidebar.style.layout.width = Some(184.0);
        sidebar.style.layout.height = Some(80.0);
        sidebar.style.layout.padding = Some(8.0);
        sidebar.style.layout.gap = Some(8.0);
        sidebar.style.layout.overflow_y = Some(OverflowStyle::Auto);

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![sidebar],
        );

        let layout = compute_layout(&root, 220.0, 120.0, 1.0, &Theme::dark(), None);
        let sidebar_rect = layout.rects.get("sidebar").unwrap();
        let html_rect = layout.rects.get("html").unwrap();
        let reserve = scrollbar_gutter_reserve_px(root.children.first().unwrap(), 1.0);

        assert!(
            html_rect.w <= 74.5,
            "long badge should honor max_width inside sidebar flow: {html_rect:?}"
        );
        assert!(
            html_rect.x + html_rect.w <= sidebar_rect.x + sidebar_rect.w - reserve + 0.5,
            "badge should stay clear of the sidebar scrollbar gutter: sidebar={sidebar_rect:?} html={html_rect:?} reserve={reserve}"
        );
    }

    #[test]
    fn sidebar_scrolls_independently_from_main_content() {
        let mut nav_items = Vec::new();
        for index in 0..10 {
            let mut item = node(
                &format!("nav-{index}"),
                WidgetKind::NavItem,
                NodeProps {
                    text: Some(format!("Item {index}")),
                    ..NodeProps::default()
                },
                vec![],
            );
            item.style.layout.height = Some(30.0);
            item.style.layout.flex_shrink = Some(0.0);
            nav_items.push(item);
        }

        let mut sidebar = node(
            "sidebar",
            WidgetKind::Sidebar,
            NodeProps::default(),
            nav_items,
        );
        sidebar.style.layout.width = Some(160.0);
        sidebar.style.layout.height = Some(120.0);
        sidebar.style.layout.padding = Some(8.0);
        sidebar.style.layout.gap = Some(6.0);
        sidebar.style.layout.overflow_y = Some(OverflowStyle::Auto);
        sidebar.style.layout.flex_shrink = Some(0.0);

        let mut main = node(
            "main",
            WidgetKind::Page,
            NodeProps::default(),
            vec![node(
                "content",
                WidgetKind::Panel,
                NodeProps::default(),
                vec![],
            )],
        );
        main.style.layout.flex_grow = Some(1.0);
        main.style.layout.min_width = Some(0.0);

        let mut row = node(
            "row",
            WidgetKind::HLayout,
            NodeProps::default(),
            vec![sidebar, main],
        );
        row.style.layout.height_value = Some(LayoutLength::Percent(100.0));
        row.style.layout.width_value = Some(LayoutLength::Percent(100.0));

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![row],
        );
        let mut state = WidgetState::default();
        state.container_scroll_y.insert("sidebar".to_string(), 42.0);

        let layout = compute_layout(&root, 320.0, 140.0, 1.0, &Theme::dark(), Some(&state));
        let sidebar_rect = layout.rects.get("sidebar").unwrap();
        let main_rect = layout.rects.get("main").unwrap();
        let first_nav = layout.rects.get("nav-0").unwrap();
        let applied_scroll = layout.scroll_y.get("sidebar").copied().unwrap_or(0.0);
        let max_scroll = layout.scroll_max_y.get("sidebar").copied().unwrap_or(0.0);

        assert!(max_scroll > 0.0, "sidebar should own overflow scroll range");
        assert!(
            applied_scroll > 0.0,
            "sidebar scroll state should be applied"
        );
        assert!(
            first_nav.y < sidebar_rect.y + 8.0,
            "sidebar children should move with sidebar scroll: sidebar={sidebar_rect:?} first={first_nav:?} scroll={applied_scroll}"
        );
        assert!(
            main_rect.x >= sidebar_rect.x + sidebar_rect.w - 0.5,
            "main content should remain beside independently scrolling sidebar: sidebar={sidebar_rect:?} main={main_rect:?}"
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

        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![flow],
        );
        let layout = compute_layout(&root, 320.0, 220.0, 1.0, &Theme::dark(), None);
        let grid = layout.rects.get("check-0").expect("grid checkbox");
        let planes = layout.rects.get("check-1").expect("planes checkbox");
        let orientation = layout.rects.get("check-2").expect("orientation checkbox");
        assert!(
            grid.w >= 70.0 && planes.w >= 118.0 && orientation.w >= 118.0,
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
}
