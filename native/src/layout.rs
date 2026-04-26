use std::collections::HashMap;

use taffy::prelude::*;

use crate::document::{WidgetKind, WidgetNode};
use crate::events::WidgetState;
use crate::style::{DisplayStyle, FlexDirectionStyle, CHECKBOX_BOX_LP, CHECKBOX_LEFT_PAD_LP};
use crate::theme::Theme;

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

/// Maps each widget `id` to its computed pixel rect.
#[derive(Debug, Default)]
pub struct LayoutResult {
    pub rects: HashMap<String, Rect>,
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
        false,
    );

    tree.compute_layout(
        root_id,
        Size {
            width: AvailableSpace::Definite(window_w),
            height: AvailableSpace::Definite(window_h),
        },
    )
    .expect("taffy layout failed");

    let mut result = LayoutResult::default();
    collect(&tree, root_id, root, 0.0, 0.0, &mut result);
    apply_navigation_layout(root, &mut result, scale_factor, theme, state);
    apply_modal_layout(root, &mut result, scale_factor, theme);
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
    parent_kind: Option<&WidgetKind>,
    layout_modal_children: bool,
) -> NodeId {
    let mut style = style_for(node, sf, theme, parent_kind, layout_modal_children);
    if let Some((w, h)) = size_override {
        style.size = taffy::geometry::Size {
            width: Dimension::Length(w),
            height: Dimension::Length(h),
        };
    }
    let child_ids: Vec<NodeId> = if matches!(
        node.kind,
        WidgetKind::Tabs | WidgetKind::Pages | WidgetKind::Menu | WidgetKind::ContextMenu
    ) || (node.kind == WidgetKind::Modal && !layout_modal_children)
    {
        Vec::new()
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
                    Some(&node.kind),
                    layout_modal_children,
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

// ---------------------------------------------------------------------------
// Style mapping
// ---------------------------------------------------------------------------

// Logical-pixel constants — multiplied by scale_factor before use.
fn style_for(
    node: &WidgetNode,
    sf: f32,
    theme: &Theme,
    parent_kind: Option<&WidgetKind>,
    layout_modal_children: bool,
) -> Style {
    let ctrl_h = theme.control_height() * sf;
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
            ..Default::default()
        },

        WidgetKind::HLayout => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 1.0,
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
            },
            ..Default::default()
        },

        WidgetKind::VLayout => Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            align_items: Some(AlignItems::Stretch),
            flex_grow: 1.0,
            size: Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Percent(1.0),
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
                        .unwrap_or_else(|| theme.control_height())
                        .max(theme.control_height())
                        * sf,
                ),
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

        WidgetKind::Modal if !layout_modal_children => Style {
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

        WidgetKind::Checkbox => Style {
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Length(ctrl_h),
            },
            flex_shrink: 0.0,
            ..Default::default()
        },

        WidgetKind::Label
        | WidgetKind::Slider
        | WidgetKind::ProgressBar
        | WidgetKind::TextInput => Style {
            size: Size {
                width: Dimension::Auto,
                height: Dimension::Length(ctrl_h),
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

        // ── scatter / table: grow to fill remaining space ─────────────────
        WidgetKind::Scatter3D
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
    apply_intrinsic_leaf_width(&mut style, node, parent_kind, sf, theme);
    apply_node_style(&mut style, node, sf);
    if node.kind != WidgetKind::Modal || layout_modal_children {
        reserve_panel_title_space(&mut style, node, sf, theme);
    }
    style
}

fn reserve_panel_title_space(style: &mut Style, node: &WidgetNode, sf: f32, theme: &Theme) {
    if !matches!(
        node.kind,
        WidgetKind::Panel | WidgetKind::Sidebar | WidgetKind::Modal
    ) || !node.props.text.as_deref().is_some_and(|t| !t.is_empty())
    {
        return;
    }
    let title_inset =
        (panel_title_line_height_lp(node, theme) + panel_title_gap_lp(node, theme)) * sf;
    style.padding.top = match style.padding.top {
        LengthPercentage::Length(top) => LengthPercentage::Length(top + title_inset),
        _ => LengthPercentage::Length(title_inset),
    };
}

fn panel_title_line_height_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    let font_size = node
        .style
        .text
        .font_size
        .unwrap_or(theme.font_size)
        .max(8.0);
    (font_size + 5.0).max(theme.font_size + 3.0)
}

fn panel_title_gap_lp(node: &WidgetNode, theme: &Theme) -> f32 {
    node.style
        .layout
        .gap
        .unwrap_or(theme.spacing * 0.75)
        .max(0.0)
}

fn apply_intrinsic_leaf_width(
    style: &mut Style,
    node: &WidgetNode,
    parent_kind: Option<&WidgetKind>,
    sf: f32,
    theme: &Theme,
) {
    if node.style.layout.width.is_some()
        || node.style.layout.min_width.is_some()
        || node.style.layout.max_width.is_some()
    {
        return;
    }
    if !matches!(
        parent_kind,
        Some(WidgetKind::HLayout | WidgetKind::StatusBar | WidgetKind::Tabs)
            | Some(WidgetKind::MenuBar)
    ) {
        return;
    }

    let Some(width) = intrinsic_leaf_width(node, theme) else {
        return;
    };
    style.min_size.width = Dimension::Length(width * sf);
}

fn intrinsic_leaf_width(node: &WidgetNode, theme: &Theme) -> Option<f32> {
    let text = intrinsic_text(node);
    let text_w = text.map(|t| estimate_text_width(t, theme.font_size));
    let pad = theme.spacing * 2.0;
    match node.kind {
        WidgetKind::Button => Some((text_w.unwrap_or(0.0) + pad).clamp(72.0, 240.0)),
        WidgetKind::Menu => Some((text_w.unwrap_or(0.0) + pad).clamp(44.0, 180.0)),
        WidgetKind::Dropdown => Some((text_w.unwrap_or(0.0) + pad + 22.0).clamp(112.0, 260.0)),
        WidgetKind::NumberInput => Some((text_w.unwrap_or(0.0) + pad + 34.0).clamp(96.0, 220.0)),
        WidgetKind::TextInput => Some((text_w.unwrap_or(0.0) + pad).clamp(120.0, 280.0)),
        WidgetKind::Checkbox => Some(
            (text_w.unwrap_or(0.0) + CHECKBOX_LEFT_PAD_LP + CHECKBOX_BOX_LP + pad)
                .clamp(48.0, 280.0),
        ),
        WidgetKind::Label | WidgetKind::NavItem | WidgetKind::Tab => {
            Some((text_w.unwrap_or(0.0) + pad).clamp(32.0, 280.0))
        }
        WidgetKind::Slider => Some(140.0),
        WidgetKind::ProgressBar => Some(160.0),
        _ => None,
    }
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
    let chars = text.chars().count() as f32;
    chars * font_size * 0.56
}

fn apply_node_style(style: &mut Style, node: &WidgetNode, sf: f32) {
    let layout = &node.style.layout;
    if let Some(display) = layout.display {
        style.display = match display {
            DisplayStyle::Flex => Display::Flex,
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
    if let Some(width) = layout.width {
        style.size.width = Dimension::Length(width * sf);
    }
    if let Some(height) = layout.height {
        style.size.height = Dimension::Length(height * sf);
    }
    if let Some(width) = layout.min_width {
        style.min_size.width = Dimension::Length(width * sf);
    }
    if let Some(height) = layout.min_height {
        style.min_size.height = Dimension::Length(height * sf);
    }
    if let Some(width) = layout.max_width {
        style.max_size.width = Dimension::Length(width * sf);
    }
    if let Some(height) = layout.max_height {
        style.max_size.height = Dimension::Length(height * sf);
    }
    if let Some(grow) = layout.flex_grow {
        style.flex_grow = grow.max(0.0);
    }
    if let Some(shrink) = layout.flex_shrink {
        style.flex_shrink = shrink.max(0.0);
    }
    if let Some(gap) = layout.gap {
        style.gap = taffy::geometry::Size {
            width: LengthPercentage::Length(gap * sf),
            height: LengthPercentage::Length(gap * sf),
        };
    }
    if let Some(margin) = layout.margin {
        style.margin = taffy::geometry::Rect {
            left: LengthPercentageAuto::Length(margin * sf),
            right: LengthPercentageAuto::Length(margin * sf),
            top: LengthPercentageAuto::Length(margin * sf),
            bottom: LengthPercentageAuto::Length(margin * sf),
        };
    }
    let pad_all = layout.padding.map(|v| v * sf);
    if pad_all.is_some()
        || layout.padding_left.is_some()
        || layout.padding_right.is_some()
        || layout.padding_top.is_some()
        || layout.padding_bottom.is_some()
    {
        let current = style.padding;
        style.padding = taffy::geometry::Rect {
            left: LengthPercentage::Length(
                layout
                    .padding_left
                    .map(|v| v * sf)
                    .or(pad_all)
                    .unwrap_or_else(|| lp_value(current.left)),
            ),
            right: LengthPercentage::Length(
                layout
                    .padding_right
                    .map(|v| v * sf)
                    .or(pad_all)
                    .unwrap_or_else(|| lp_value(current.right)),
            ),
            top: LengthPercentage::Length(
                layout
                    .padding_top
                    .map(|v| v * sf)
                    .or(pad_all)
                    .unwrap_or_else(|| lp_value(current.top)),
            ),
            bottom: LengthPercentage::Length(
                layout
                    .padding_bottom
                    .map(|v| v * sf)
                    .or(pad_all)
                    .unwrap_or_else(|| lp_value(current.bottom)),
            ),
        };
    }
}

fn lp_value(value: LengthPercentage) -> f32 {
    match value {
        LengthPercentage::Length(v) => v,
        LengthPercentage::Percent(_) => 0.0,
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
    for (child_id, child_widget) in child_ids.iter().zip(widget.children.iter()) {
        collect(tree, *child_id, child_widget, abs_x, abs_y, result);
    }
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
        true,
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

    let header_h = theme.control_height() * sf;
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
        layout_region(&active_page.children, r, result, sf, theme, state);
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
        children: children.to_vec(),
    };
    let sub = compute_layout(&synthetic, rect.w, rect.h, sf, theme, state);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::NodeProps;

    fn node(id: &str, kind: WidgetKind, props: NodeProps, children: Vec<WidgetNode>) -> WidgetNode {
        WidgetNode {
            id: id.to_string(),
            key: None,
            class_name: None,
            kind,
            props,
            style_json: Default::default(),
            style: Default::default(),
            children,
        }
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
