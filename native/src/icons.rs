#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinIconResolution {
    pub(crate) resolved: &'static str,
    pub(crate) recognized: bool,
    pub(crate) alias: bool,
}

pub(crate) fn resolve_builtin_icon(name: &str) -> BuiltinIconResolution {
    let resolved = match name {
        "add" | "plus" | "new" => "add",
        "minus" | "remove" | "subtract" => "minus",
        "close" | "x" | "delete" | "clear" => "close",
        "check" | "ok" | "done" => "check",
        "edit" | "pencil" => "edit",
        "copy" | "duplicate" => "copy",
        "file" | "document" => "file",
        "folder" | "open" | "folder-open" => "folder",
        "upload" | "import" => "upload",
        "download" | "export" => "download",
        "refresh" | "reload" | "sync" => "refresh",
        "settings" | "gear" => "settings",
        "home" => "home",
        "info" => "info",
        "help" | "question" => "help",
        "warning" | "alert" => "warning",
        "lock" => "lock",
        "unlock" => "unlock",
        "eye" | "show" | "visible" => "eye",
        "eye-off" | "hide" | "hidden" => "eye-off",
        "menu" | "hamburger" => "menu",
        "list" | "workflow" => "list",
        "filter" | "funnel" => "filter",
        "sort" => "sort",
        "undo" => "undo",
        "redo" => "redo",
        "play" | "run" => "play",
        "pause" => "pause",
        "stop" | "square" => "stop",
        "save" => "save",
        "search" | "zoom" => "search",
        "fit" => "fit",
        "pan" | "move" => "pan",
        "grid" => "grid",
        "axes" => "axes",
        "more" => "more",
        _ => {
            return BuiltinIconResolution {
                resolved: "more",
                recognized: false,
                alias: false,
            };
        }
    };
    BuiltinIconResolution {
        resolved,
        recognized: true,
        alias: name != resolved,
    }
}

fn normalized_icon_name(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
    if normalized.is_empty() {
        Err("icon names must be non-empty".to_string())
    } else {
        Ok(normalized)
    }
}

fn finite_number(value: &Value) -> Option<f32> {
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .map(|number| number as f32)
}

fn validate_icon_resource(value: &Value) -> Result<(), String> {
    let resource = value
        .as_object()
        .ok_or_else(|| "icon resources must be objects".to_string())?;
    if resource.get("type").and_then(Value::as_str) != Some("stroke") {
        return Err("icon resource type must be 'stroke'".to_string());
    }
    let view_box = resource
        .get("view_box")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 4)
        .ok_or_else(|| "icon resource view_box must contain four numbers".to_string())?;
    let box_values = view_box
        .iter()
        .map(finite_number)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "icon resource view_box values must be finite".to_string())?;
    if box_values[2] <= 0.0 || box_values[3] <= 0.0 {
        return Err("icon resource view_box width and height must be positive".to_string());
    }
    let stroke_width = resource
        .get("stroke_width")
        .and_then(finite_number)
        .filter(|width| *width > 0.0 && *width <= box_values[2].min(box_values[3]))
        .ok_or_else(|| "icon resource stroke_width is outside its view_box".to_string())?;
    let _ = stroke_width;
    let strokes = resource
        .get("strokes")
        .and_then(Value::as_array)
        .filter(|strokes| !strokes.is_empty() && strokes.len() <= 64)
        .ok_or_else(|| "icon resource requires between 1 and 64 strokes".to_string())?;
    let mut point_count = 0usize;
    for stroke in strokes {
        let points = stroke
            .get("points")
            .and_then(Value::as_array)
            .filter(|points| points.len() >= 2)
            .ok_or_else(|| "each icon stroke requires at least two points".to_string())?;
        point_count += points.len();
        if point_count > 256 {
            return Err("icon resource cannot contain more than 256 points".to_string());
        }
        for point in points {
            let pair = point
                .as_array()
                .filter(|pair| pair.len() == 2)
                .ok_or_else(|| "icon stroke points must be coordinate pairs".to_string())?;
            if pair
                .iter()
                .any(|coordinate| finite_number(coordinate).is_none())
            {
                return Err("icon stroke coordinates must be finite".to_string());
            }
        }
        if stroke
            .get("closed")
            .is_some_and(|closed| !closed.is_boolean())
        {
            return Err("icon stroke closed must be a boolean".to_string());
        }
    }
    Ok(())
}

fn resolve_theme_value(
    name: &str,
    theme: &HashMap<String, Value>,
    visiting: &mut HashSet<String>,
) -> Result<Option<Value>, String> {
    let Some(value) = theme.get(name) else {
        return Ok(None);
    };
    if !visiting.insert(name.to_string()) {
        return Err(format!("icon theme alias cycle contains {name:?}"));
    }
    let resolved = if let Some(alias) = value.as_str() {
        let alias = normalized_icon_name(alias)?;
        if theme.contains_key(&alias) {
            resolve_theme_value(&alias, theme, visiting)?
        } else if resolve_builtin_icon(&alias).recognized {
            Some(Value::String(alias))
        } else {
            return Err(format!(
                "icon theme alias {name:?} targets unknown icon {alias:?}"
            ));
        }
    } else {
        validate_icon_resource(value)?;
        Some(value.clone())
    };
    visiting.remove(name);
    Ok(resolved)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct IconThemeRegistry {
    overrides: HashMap<String, Value>,
}

impl IconThemeRegistry {
    pub(crate) fn from_value(raw_theme: Option<&Value>) -> Result<Self, String> {
        let Some(raw_theme) = raw_theme else {
            return Ok(Self::default());
        };
        let object = raw_theme
            .as_object()
            .ok_or_else(|| "icon_theme must be an object".to_string())?;
        let mut overrides = HashMap::with_capacity(object.len());
        for (raw_name, value) in object {
            let name = normalized_icon_name(raw_name)?;
            if overrides.insert(name.clone(), value.clone()).is_some() {
                return Err(format!("duplicate normalized icon override {name:?}"));
            }
        }
        for name in overrides.keys() {
            resolve_theme_value(name, &overrides, &mut HashSet::new())?;
        }
        Ok(Self { overrides })
    }

    pub(crate) fn len(&self) -> usize {
        self.overrides.len()
    }

    pub(crate) fn apply_to_tree(&self, tree: &mut WidgetNode) -> Result<(), String> {
        apply_icon_theme_to_node(tree, &self.overrides)
    }

    pub(crate) fn apply_to_subtree(&self, node: &mut WidgetNode) -> Result<(), String> {
        apply_icon_theme_to_node(node, &self.overrides)
    }
}

fn apply_icon_theme_to_node(
    node: &mut WidgetNode,
    theme: &HashMap<String, Value>,
) -> Result<(), String> {
    if matches!(node.kind, WidgetKind::IconButton | WidgetKind::NavItem) {
        node.props.raw_props.remove("icon_override_name");
        node.props.raw_props.remove("icon_override_resource");
        node.props.raw_props.remove("icon_override_key");
        if let Some(requested) = node.props.raw_props.get("icon").and_then(Value::as_str) {
            let requested = normalized_icon_name(requested)?;
            let canonical = resolve_builtin_icon(&requested).resolved;
            let override_name = if theme.contains_key(&requested) {
                Some(requested.as_str())
            } else if theme.contains_key(canonical) {
                Some(canonical)
            } else {
                None
            };
            if let Some(override_name) = override_name {
                if let Some(value) = resolve_theme_value(override_name, theme, &mut HashSet::new())?
                {
                    match value {
                        Value::String(name) => {
                            node.props
                                .raw_props
                                .insert("icon_override_name".to_string(), Value::String(name));
                        }
                        resource => {
                            node.props
                                .raw_props
                                .insert("icon_override_resource".to_string(), resource);
                            node.props.raw_props.insert(
                                "icon_override_key".to_string(),
                                Value::String(override_name.to_string()),
                            );
                        }
                    }
                }
            }
        }
    }
    for child in &mut node.children {
        apply_icon_theme_to_node(child, theme)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn apply_icon_theme_to_tree(
    tree: &mut WidgetNode,
    raw_theme: Option<&Value>,
) -> Result<(), String> {
    IconThemeRegistry::from_value(raw_theme)?.apply_to_tree(tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_canonical_alias_and_fallback_identities() {
        assert_eq!(
            resolve_builtin_icon("search"),
            BuiltinIconResolution {
                resolved: "search",
                recognized: true,
                alias: false,
            }
        );
        assert_eq!(
            resolve_builtin_icon("zoom"),
            BuiltinIconResolution {
                resolved: "search",
                recognized: true,
                alias: true,
            }
        );
        assert_eq!(
            resolve_builtin_icon("folder-open"),
            BuiltinIconResolution {
                resolved: "folder",
                recognized: true,
                alias: true,
            }
        );
        assert_eq!(
            resolve_builtin_icon("not-registered"),
            BuiltinIconResolution {
                resolved: "more",
                recognized: false,
                alias: false,
            }
        );
    }

    #[test]
    fn validates_and_applies_custom_icon_resources_and_aliases() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [
                {"id": "search", "type": "icon_button", "props": {"icon": "zoom"}},
                {"id": "save", "type": "icon_button", "props": {"icon": "save"}}
            ]
        }))
        .unwrap();
        let theme = serde_json::json!({
            "search": {
                "type": "stroke",
                "view_box": [0, 0, 24, 24],
                "stroke_width": 2,
                "strokes": [{"points": [[3, 12], [21, 12]]}]
            },
            "save": "check"
        });

        apply_icon_theme_to_tree(&mut tree, Some(&theme)).unwrap();

        assert!(tree.children[0]
            .props
            .raw_props
            .contains_key("icon_override_resource"));
        assert_eq!(
            tree.children[0].props.raw_props["icon_override_key"],
            "search"
        );
        assert_eq!(
            tree.children[1].props.raw_props["icon_override_name"],
            "check"
        );
    }

    #[test]
    fn rejects_icon_theme_cycles_and_excessive_resources() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window"
        }))
        .unwrap();
        assert!(apply_icon_theme_to_tree(
            &mut tree,
            Some(&serde_json::json!({"search": "find", "find": "search"}))
        )
        .unwrap_err()
        .contains("cycle"));
        assert!(validate_icon_resource(&serde_json::json!({
            "type": "stroke",
            "view_box": [0, 0, 24, 24],
            "stroke_width": 30,
            "strokes": [{"points": [[0, 0], [1, 1]]}]
        }))
        .is_err());
    }

    #[test]
    fn replacing_registry_clears_stale_node_overrides() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "search",
            "type": "icon_button",
            "props": {"icon": "search"}
        }))
        .unwrap();
        let populated = IconThemeRegistry::from_value(Some(&serde_json::json!({
            "search": {
                "type": "stroke",
                "view_box": [0, 0, 24, 24],
                "stroke_width": 2,
                "strokes": [{"points": [[3, 3], [21, 21]]}]
            }
        })))
        .unwrap();
        populated.apply_to_tree(&mut tree).unwrap();
        assert!(tree.props.raw_props.contains_key("icon_override_resource"));

        IconThemeRegistry::default()
            .apply_to_tree(&mut tree)
            .unwrap();
        assert!(!tree.props.raw_props.contains_key("icon_override_resource"));
        assert!(!tree.props.raw_props.contains_key("icon_override_key"));
    }
}
use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::document::{WidgetKind, WidgetNode};
