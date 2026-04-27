use crate::document::{WidgetKind, WidgetNode};
use crate::events::{NavigationItem, WidgetState};
use crate::layout::{LayoutResult, Rect};
use crate::theme::Theme;

pub(crate) fn dropdown_overlay_rect(
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
) -> Option<Rect> {
    let id = state.open_dropdown.as_ref()?;
    let r = layout.rects.get(id)?;
    let items = state.dropdown_items.get(id)?;
    Some(Rect {
        x: r.x,
        y: r.y + r.h,
        w: r.w,
        h: theme.control_height() * sf * items.len() as f32,
    })
}

pub(crate) fn active_menu_overlay_rects(
    tree: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
) -> [Option<Rect>; 2] {
    [
        state
            .open_menu
            .as_deref()
            .and_then(|id| menu_popup_rect(tree, layout, state, theme, sf, id)),
        state
            .open_context_menu
            .as_deref()
            .and_then(|id| menu_popup_rect(tree, layout, state, theme, sf, id)),
    ]
}

pub(crate) fn menu_popup_rect(
    tree: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
    id: &str,
) -> Option<Rect> {
    let items = state.menu_items.get(id)?;
    if items.is_empty() {
        return None;
    }
    let node = find_node(tree, id)?;
    let row_h = theme.control_height() * sf;
    let mut width = menu_popup_width(items, node.props.fixed_width, theme, sf);
    let height = row_h * items.len() as f32;
    let root = layout.rects.get(&tree.id).copied().unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0 * sf,
        h: 600.0 * sf,
    });

    let (mut x, mut y) = if node.kind == WidgetKind::Menu {
        let r = layout.rects.get(id)?;
        width = width.max(r.w);
        (r.x, r.y + r.h)
    } else {
        let pos = state.context_menu_pos?;
        (pos[0], pos[1])
    };
    x = x.clamp(root.x, (root.x + root.w - width).max(root.x));
    y = y.clamp(root.y, (root.y + root.h - height).max(root.y));
    Some(Rect {
        x,
        y,
        w: width,
        h: height,
    })
}

pub(crate) fn menu_popup_width(
    items: &[NavigationItem],
    fixed_width: Option<f32>,
    theme: &Theme,
    sf: f32,
) -> f32 {
    if let Some(width) = fixed_width {
        return (width * sf).max(80.0 * sf);
    }
    let pad = theme.spacing * sf * 2.5;
    let text_w = items
        .iter()
        .map(|item| estimate_text_width(&item.value, theme.font_size * sf))
        .fold(0.0, f32::max);
    (text_w + pad).clamp(120.0 * sf, 360.0 * sf)
}

pub(crate) fn tooltip_target<'a>(
    tree: &'a WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    state: &WidgetState,
    sf: f32,
) -> Option<(&'a WidgetNode, Rect)> {
    let hovered = state.hovered.as_deref()?;
    let node = find_node(tree, hovered)?;
    let text = node.props.tooltip.as_deref()?.trim();
    if text.is_empty() {
        return None;
    }
    let target = layout.rects.get(&node.id)?;
    let root = layout.rects.get(&tree.id).copied().unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: target.x + target.w,
        h: target.y + target.h,
    });
    let margin = theme.spacing * sf;
    let pad = theme.spacing * sf * 1.25;
    let natural_width = estimate_text_width(text, theme.font_size * sf) + pad * 2.0;
    let max_width = (root.w - margin * 2.0).max(48.0 * sf).min(420.0 * sf);
    let min_width = (96.0 * sf).min(max_width);
    let width = natural_width.max(min_width).min(max_width);
    let line_height = (theme.font_size * sf + 5.0 * sf).max((theme.font_size + 3.0) * sf);
    let content_width = (width - pad * 2.0).max(1.0);
    let text_width = estimate_text_width(text, theme.font_size * sf);
    let lines = (text_width / content_width).ceil().clamp(1.0, 4.0);
    let height = (line_height * lines + pad * 2.0).max(28.0 * sf);
    let mut obstacles = Vec::new();
    collect_tooltip_obstacles(tree, layout, &node.id, &mut obstacles);
    let rect = choose_tooltip_rect(*target, root, width, height, margin, &obstacles);
    Some((node, rect))
}

pub(crate) fn rich_tooltip_target<'a>(
    tree: &'a WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
) -> Option<(&'a WidgetNode, Rect)> {
    let hovered = state.hovered.as_deref()?;
    let node = active_rich_tooltip(tree, hovered)?;
    let rect = layout.rects.get(&node.id).copied()?;
    Some((node, rect))
}

pub(crate) fn active_tooltip_overlay_rect(
    tree: &WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    state: &WidgetState,
    sf: f32,
) -> Option<Rect> {
    rich_tooltip_target(tree, layout, state)
        .map(|(_, rect)| rect)
        .or_else(|| tooltip_target(tree, layout, theme, state, sf).map(|(_, rect)| rect))
}

fn active_rich_tooltip<'a>(node: &'a WidgetNode, hovered: &str) -> Option<&'a WidgetNode> {
    for child in node.children.iter().rev() {
        if let Some(found) = active_rich_tooltip(child, hovered) {
            return Some(found);
        }
    }
    (node.kind == WidgetKind::Tooltip && node.props.target.as_deref() == Some(hovered))
        .then_some(node)
}

pub(crate) fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    text.chars()
        .map(|ch| char_width_factor(ch) * font_size)
        .sum()
}

pub(crate) fn find_node<'a>(node: &'a WidgetNode, id: &str) -> Option<&'a WidgetNode> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_node(child, id))
}

fn char_width_factor(ch: char) -> f32 {
    match ch {
        ' ' | '\t' => 0.35,
        'i' | 'j' | 'l' | 'I' | '1' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 0.32,
        'm' | 'w' | 'M' | 'W' | '@' | '#' | '%' | '&' => 0.82,
        'A'..='Z' => 0.64,
        '0'..='9' => 0.56,
        _ => 0.54,
    }
}

fn choose_tooltip_rect(
    target: Rect,
    root: Rect,
    width: f32,
    height: f32,
    margin: f32,
    obstacles: &[(Rect, f32)],
) -> Rect {
    let center_x = target.x + target.w * 0.5 - width * 0.5;
    let center_y = target.y + target.h * 0.5 - height * 0.5;
    let candidates = [
        (
            Rect {
                x: target.x,
                y: target.y + target.h + margin,
                w: width,
                h: height,
            },
            0.0,
        ),
        (
            Rect {
                x: target.x,
                y: target.y - height - margin,
                w: width,
                h: height,
            },
            4.0,
        ),
        (
            Rect {
                x: center_x,
                y: target.y + target.h + margin,
                w: width,
                h: height,
            },
            6.0,
        ),
        (
            Rect {
                x: center_x,
                y: target.y - height - margin,
                w: width,
                h: height,
            },
            8.0,
        ),
        (
            Rect {
                x: target.x + target.w + margin,
                y: center_y,
                w: width,
                h: height,
            },
            12.0,
        ),
        (
            Rect {
                x: target.x - width - margin,
                y: center_y,
                w: width,
                h: height,
            },
            14.0,
        ),
    ];

    candidates
        .into_iter()
        .map(|(rect, bias)| {
            let rect = clamp_rect_to_root(rect, root, margin);
            let overlap_score = obstacles
                .iter()
                .map(|(obstacle, weight)| intersection_area(rect, *obstacle) * weight)
                .sum::<f32>();
            let target_overlap = intersection_area(rect, target) * 5.0;
            let distance = ((rect.x - target.x).abs() + (rect.y - target.y).abs()) * 0.01;
            (rect, overlap_score + target_overlap + distance + bias)
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(rect, _)| rect)
        .unwrap_or(Rect {
            x: target.x,
            y: target.y + target.h + margin,
            w: width,
            h: height,
        })
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

fn collect_tooltip_obstacles(
    node: &WidgetNode,
    layout: &LayoutResult,
    tooltip_node_id: &str,
    out: &mut Vec<(Rect, f32)>,
) {
    if node.kind == WidgetKind::Modal && !node.props.open.unwrap_or(false) {
        return;
    }
    if node.id != tooltip_node_id {
        if let (Some(rect), Some(weight)) = (
            layout.rects.get(&node.id).copied(),
            tooltip_obstacle_weight(&node.kind),
        ) {
            if rect.w > 0.0 && rect.h > 0.0 {
                out.push((rect, weight));
            }
        }
    }
    for child in &node.children {
        collect_tooltip_obstacles(child, layout, tooltip_node_id, out);
    }
}

fn tooltip_obstacle_weight(kind: &WidgetKind) -> Option<f32> {
    match kind {
        WidgetKind::Window
        | WidgetKind::HLayout
        | WidgetKind::VLayout
        | WidgetKind::Pages
        | WidgetKind::Page
        | WidgetKind::Spacer
        | WidgetKind::Separator
        | WidgetKind::ContextMenu
        | WidgetKind::MenuItem
        | WidgetKind::Unknown => None,
        WidgetKind::Panel
        | WidgetKind::Sidebar
        | WidgetKind::StatusBar
        | WidgetKind::MenuBar
        | WidgetKind::Modal => Some(0.05),
        WidgetKind::Scatter3D | WidgetKind::DataFrameTable | WidgetKind::Image => Some(0.35),
        _ => Some(1.0),
    }
}

fn intersection_area(a: Rect, b: Rect) -> f32 {
    let x = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let y = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    x.max(0.0) * y.max(0.0)
}
