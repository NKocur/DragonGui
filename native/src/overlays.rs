use crate::css_style::{
    computed_style_for_virtual_element_with_media, DgMediaEnvironment, StylesheetStore,
};
use crate::document::{WidgetKind, WidgetNode};
use crate::events::{NavigationItem, WidgetState};
use crate::layout::{LayoutResult, Rect};
use crate::style::TextStyle;
use crate::text::{measure_text_for_layout, measure_wrapped_text_for_layout};
use crate::theme::Theme;

pub(crate) fn dropdown_overlay_rect(
    tree: &WidgetNode,
    layout: &LayoutResult,
    state: &WidgetState,
    theme: &Theme,
    sf: f32,
) -> Option<Rect> {
    let id = state.open_dropdown.as_ref()?;
    let r = layout.rects.get(id)?;
    let items = state.dropdown_items.get(id)?;
    let root = layout.rects.get(&tree.id).copied().unwrap_or(Rect {
        x: 0.0,
        y: 0.0,
        w: r.x + r.w,
        h: r.y + r.h,
    });
    let height = theme.control_height() * sf * items.len() as f32;
    let below_y = r.y + r.h;
    let above_y = r.y - height;
    let y = if below_y + height <= root.y + root.h {
        below_y
    } else if above_y >= root.y {
        above_y
    } else {
        below_y
    };
    Some(clamp_rect_to_root(
        Rect {
            x: r.x,
            y,
            w: r.w,
            h: height,
        },
        root,
        0.0,
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css_style::StylesheetOrigin;
    use crate::document::NodeProps;

    fn node(id: &str, kind: WidgetKind, children: Vec<WidgetNode>) -> WidgetNode {
        WidgetNode {
            id: id.to_string(),
            key: None,
            class_name: None,
            css_types: Vec::new(),
            kind,
            props: NodeProps::default(),
            style_json: Default::default(),
            default_style: Default::default(),
            inline_style: Default::default(),
            style: Default::default(),
            children,
        }
    }

    #[test]
    fn dropdown_popup_flips_and_clamps_to_viewport() {
        let root = node(
            "window",
            WidgetKind::Window,
            vec![node("dropdown", WidgetKind::Dropdown, vec![])],
        );
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 120.0,
                h: 100.0,
            },
        );
        layout.rects.insert(
            "dropdown".to_string(),
            Rect {
                x: 90.0,
                y: 70.0,
                w: 60.0,
                h: 24.0,
            },
        );
        let mut state = WidgetState::default();
        state.open_dropdown = Some("dropdown".to_string());
        state.dropdown_items.insert(
            "dropdown".to_string(),
            (0..6).map(|index| format!("Item {index}")).collect(),
        );

        let rect =
            dropdown_overlay_rect(&root, &layout, &state, &Theme::dark(), 1.0).expect("popup");

        assert!(rect.x >= 0.0 && rect.y >= 0.0);
        assert!(rect.x + rect.w <= 120.0);
        assert!(rect.y + rect.h <= 100.0);
        assert_eq!(rect.h, 100.0);
    }

    #[test]
    fn oversized_menu_popup_is_bounded_by_viewport() {
        let root = Rect {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 80.0,
        };
        let rect = clamp_rect_to_root(
            Rect {
                x: 90.0,
                y: 90.0,
                w: 180.0,
                h: 160.0,
            },
            root,
            0.0,
        );
        assert_eq!(
            [rect.x, rect.y, rect.w, rect.h],
            [root.x, root.y, root.w, root.h]
        );
    }

    #[test]
    fn static_tooltip_geometry_uses_authored_font_and_padding() {
        let mut target = node("target", WidgetKind::Button, vec![]);
        target.props.tooltip = Some(
            "A deliberately long client chrome tooltip label that must wrap across more than four complete lines without clipping any ordinary words."
                .to_string(),
        );
        let root = node("window", WidgetKind::Window, vec![target]);
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 180.0,
                h: 500.0,
            },
        );
        layout.rects.insert(
            "target".to_string(),
            Rect {
                x: 140.0,
                y: 0.0,
                w: 40.0,
                h: 34.0,
            },
        );
        let state = WidgetState {
            hovered: Some("target".to_string()),
            ..Default::default()
        };
        let theme = Theme::dark();
        let media = DgMediaEnvironment::new(180.0, 500.0);
        let baseline = tooltip_target(
            &root,
            &layout,
            &theme,
            &state,
            1.0,
            &StylesheetStore::default(),
            media,
        )
        .expect("baseline tooltip")
        .1;

        let mut stylesheets = StylesheetStore::default();
        stylesheets
            .set_stylesheet(
                StylesheetOrigin::User,
                "Tooltip.static { font-size: 22px; line-height: 1.5; padding: 14px; }",
            )
            .expect("tooltip stylesheet");
        let styled = tooltip_target(&root, &layout, &theme, &state, 1.0, &stylesheets, media)
            .expect("styled tooltip")
            .1;

        assert!(
            styled.h > baseline.h,
            "styled tooltip must reserve its larger wrapped text: baseline={baseline:?} styled={styled:?}"
        );
        let tooltip_style = computed_style_for_virtual_element_with_media(
            WidgetKind::Tooltip,
            "__dg_static_tooltip",
            &["static"],
            &stylesheets,
            Some(media),
        );
        let measured = measure_wrapped_text_for_layout(
            root.children[0]
                .props
                .tooltip
                .as_deref()
                .expect("tooltip text"),
            &tooltip_style.text,
            &theme,
            styled.w - 28.0,
        );
        assert!(
            measured.line_count > 4,
            "regression must exercise more than four wrapped lines: {measured:?}"
        );
        assert!(
            (styled.h - (measured.height + 30.0)).abs() <= 1.0,
            "tooltip surface must use the shaped wrapped height: styled={styled:?} measured={measured:?}"
        );
        assert!(styled.x >= 0.0 && styled.y >= 0.0);
        assert!(styled.x + styled.w <= 180.0);
        assert!(styled.y + styled.h <= 500.0);
    }
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

    let (x, y) = if node.kind == WidgetKind::Menu {
        let r = layout.rects.get(id)?;
        width = width.max(r.w);
        (r.x, r.y + r.h)
    } else {
        let pos = state.context_menu_pos?;
        (pos[0], pos[1])
    };
    Some(clamp_rect_to_root(
        Rect {
            x,
            y,
            w: width,
            h: height,
        },
        root,
        0.0,
    ))
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
        .map(|item| measure_text_for_layout(&item.value, &TextStyle::default(), theme).width * sf)
        .fold(0.0, f32::max);
    (text_w + pad).clamp(120.0 * sf, 360.0 * sf)
}

pub(crate) fn tooltip_target<'a>(
    tree: &'a WidgetNode,
    layout: &LayoutResult,
    theme: &Theme,
    state: &WidgetState,
    sf: f32,
    stylesheets: &StylesheetStore,
    media: DgMediaEnvironment,
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
    let style = computed_style_for_virtual_element_with_media(
        WidgetKind::Tooltip,
        "__dg_static_tooltip",
        &["static"],
        stylesheets,
        Some(media),
    );
    let fallback_pad = theme.spacing * 1.25;
    let pad_left = style
        .layout
        .padding_left
        .or(style.layout.padding)
        .unwrap_or(fallback_pad)
        .max(0.0)
        * sf;
    let pad_right = style
        .layout
        .padding_right
        .or(style.layout.padding)
        .unwrap_or(fallback_pad)
        .max(0.0)
        * sf;
    let pad_top = style
        .layout
        .padding_top
        .or(style.layout.padding)
        .unwrap_or(fallback_pad)
        .max(0.0)
        * sf;
    let pad_bottom = style
        .layout
        .padding_bottom
        .or(style.layout.padding)
        .unwrap_or(fallback_pad)
        .max(0.0)
        * sf;
    // Shaping produces fractional metrics while the final scissor bounds use
    // integral pixels. Reserve a little space so rounding cannot clip the last
    // glyph column or the descenders on the final wrapped line.
    let text_slop = 2.0 * sf;
    let margin = theme.spacing * sf;
    let text_width = measure_text_for_layout(text, &style.text, theme).width * sf;
    let natural_width = text_width + pad_left + pad_right + text_slop;
    let max_width = (root.w - margin * 2.0).max(48.0 * sf).min(420.0 * sf);
    let min_width = (96.0 * sf).min(max_width);
    let width = natural_width.max(min_width).min(max_width);
    let content_width = (width - pad_left - pad_right).max(1.0);
    let wrapped =
        measure_wrapped_text_for_layout(text, &style.text, theme, content_width / sf.max(0.001));
    let height = (wrapped.height * sf + pad_top + pad_bottom + text_slop).max(28.0 * sf);
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
    stylesheets: &StylesheetStore,
    media: DgMediaEnvironment,
) -> Option<Rect> {
    rich_tooltip_target(tree, layout, state)
        .map(|(_, rect)| rect)
        .or_else(|| {
            tooltip_target(tree, layout, theme, state, sf, stylesheets, media).map(|(_, rect)| rect)
        })
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

pub(crate) fn find_node<'a>(node: &'a WidgetNode, id: &str) -> Option<&'a WidgetNode> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_node(child, id))
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
        | WidgetKind::Toast
        | WidgetKind::Unknown => None,
        WidgetKind::Panel
        | WidgetKind::Sidebar
        | WidgetKind::StatusBar
        | WidgetKind::MenuBar
        | WidgetKind::Modal => Some(0.05),
        WidgetKind::Scatter3D
        | WidgetKind::DataFrameTable
        | WidgetKind::HtmlReport
        | WidgetKind::Image => Some(0.35),
        _ => Some(1.0),
    }
}

fn intersection_area(a: Rect, b: Rect) -> f32 {
    let x = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let y = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    x.max(0.0) * y.max(0.0)
}
