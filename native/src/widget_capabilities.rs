use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::document::{WidgetKind, WidgetNode};

#[cfg(test)]
use std::collections::BTreeSet;

const REGISTRY_JSON: &str = include_str!("../../python/dragongui/widget_css_capabilities.json");

#[derive(Debug, Deserialize)]
struct CapabilityDocument {
    schema_version: u32,
    generated_content: GeneratedContentRecord,
    widgets: Vec<WidgetCapabilityRecord>,
}

#[derive(Debug, Deserialize)]
struct GeneratedContentRecord {
    parts: Vec<String>,
    renderer: String,
    excluded_native_kinds: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WidgetCapabilityRecord {
    public_type: String,
    native_kind: String,
    #[serde(default)]
    semantic_only: bool,
    parts: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
struct WidgetCapability {
    parts: BTreeMap<String, String>,
}

#[derive(Debug)]
struct CapabilityRegistry {
    widgets: HashMap<WidgetKind, WidgetCapability>,
    public_types: HashMap<String, WidgetCapability>,
    generated_parts: BTreeMap<String, String>,
    generated_exclusions: HashSet<WidgetKind>,
}

fn registry() -> &'static CapabilityRegistry {
    static REGISTRY: OnceLock<CapabilityRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let document: CapabilityDocument = serde_json::from_str(REGISTRY_JSON)
            .expect("packaged widget CSS capability registry must be valid JSON");
        assert_eq!(
            document.schema_version, 1,
            "unsupported widget CSS capability schema"
        );

        assert_eq!(
            document.generated_content.renderer, "text",
            "generated CSS parts must use the text renderer"
        );
        let generated_parts = document
            .generated_content
            .parts
            .into_iter()
            .map(|part| (part, document.generated_content.renderer.clone()))
            .collect();
        let generated_exclusions = document
            .generated_content
            .excluded_native_kinds
            .into_iter()
            .map(|kind| WidgetKind::from_str(&kind))
            .collect();

        let mut widgets = HashMap::with_capacity(document.widgets.len());
        let mut public_types = HashMap::with_capacity(document.widgets.len());
        for record in document.widgets {
            let kind = WidgetKind::from_str(&record.native_kind);
            assert!(
                kind != WidgetKind::Unknown || record.native_kind == "unknown",
                "unknown native widget kind {:?} in CSS capability registry",
                record.native_kind
            );
            let mut parts = BTreeMap::new();
            for (renderer, renderer_parts) in record.parts {
                assert!(
                    matches!(
                        renderer.as_str(),
                        "paint" | "text" | "structural" | "forwarded"
                    ),
                    "unknown renderer status {renderer:?} in CSS capability registry"
                );
                for part in renderer_parts {
                    assert!(
                        parts.insert(part.clone(), renderer.clone()).is_none(),
                        "duplicate CSS part {part:?} for native kind {:?}",
                        record.native_kind
                    );
                }
            }
            let capability = WidgetCapability { parts };
            assert!(
                public_types
                    .insert(
                        record.public_type.clone(),
                        WidgetCapability {
                            parts: capability.parts.clone(),
                        },
                    )
                    .is_none(),
                "duplicate public type {:?} in CSS capability registry",
                record.public_type
            );
            if !record.semantic_only {
                assert!(
                    widgets.insert(kind, capability).is_none(),
                    "duplicate native kind {:?} in CSS capability registry",
                    record.native_kind
                );
            }
        }
        CapabilityRegistry {
            widgets,
            public_types,
            generated_parts,
            generated_exclusions,
        }
    })
}

pub(crate) fn widget_supports_part(node: &WidgetNode, part: &str) -> bool {
    node.css_types.iter().any(|public_type| {
        registry()
            .public_types
            .get(public_type)
            .is_some_and(|capability| capability.parts.contains_key(part))
    }) || widget_kind_supports_part(node.kind, part)
}

pub(crate) fn widget_kind_supports_part(kind: WidgetKind, part: &str) -> bool {
    registry()
        .widgets
        .get(&kind)
        .is_some_and(|capability| capability.parts.contains_key(part))
        || (!registry().generated_exclusions.contains(&kind)
            && registry().generated_parts.contains_key(part))
}

#[cfg(test)]
pub(crate) fn widget_kind_registered_parts(kind: WidgetKind) -> BTreeSet<&'static str> {
    registry()
        .widgets
        .get(&kind)
        .into_iter()
        .flat_map(|capability| capability.parts.keys().map(String::as_str))
        .collect()
}

#[cfg(test)]
pub(crate) fn widget_part_renderer_status(kind: WidgetKind, part: &str) -> Option<&'static str> {
    registry()
        .widgets
        .get(&kind)
        .and_then(|capability| capability.parts.get(part))
        .or_else(|| {
            if registry().generated_exclusions.contains(&kind) {
                None
            } else {
                registry().generated_parts.get(part)
            }
        })
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ALL_WIDGET_KINDS;
    use crate::paint::native_widget_paint_fallback_parts;

    #[test]
    fn registry_publishes_scroll_area_and_radio_button_parts() {
        assert_eq!(
            widget_kind_registered_parts(WidgetKind::Window),
            BTreeSet::from([
                "close",
                "maximize",
                "minimize",
                "resize-border",
                "title",
                "titlebar"
            ])
        );
        assert_eq!(
            widget_part_renderer_status(WidgetKind::Window, "titlebar"),
            Some("forwarded")
        );
        assert_eq!(
            widget_part_renderer_status(WidgetKind::Window, "resize-border"),
            Some("structural")
        );
        assert!(widget_kind_supports_part(
            WidgetKind::ScrollArea,
            "scrollbar-track"
        ));
        assert!(widget_kind_supports_part(
            WidgetKind::ScrollArea,
            "scrollbar-thumb"
        ));
        assert_eq!(
            widget_kind_registered_parts(WidgetKind::RadioButton),
            BTreeSet::from(["dot", "indicator", "label"])
        );
        assert!(
            !widget_kind_supports_part(WidgetKind::ImageButton, "image"),
            "ImageButton::image must remain rejected until its renderer consumes a distinct image part"
        );
    }

    #[test]
    fn semantic_composite_parts_require_the_public_widget_type() {
        let search = crate::document::parse_widget_node(&serde_json::json!({
            "id": "search",
            "type": "h_layout",
            "css_types": ["SearchBox", "HLayout", "Container", "Widget"],
            "props": {},
            "children": []
        }))
        .unwrap();
        let layout = crate::document::parse_widget_node(&serde_json::json!({
            "id": "layout",
            "type": "h_layout",
            "css_types": ["HLayout", "Container", "Widget"],
            "props": {},
            "children": []
        }))
        .unwrap();

        assert!(widget_supports_part(&search, "field"));
        assert!(widget_supports_part(&search, "scrollbar-thumb"));
        assert!(!widget_supports_part(&layout, "field"));
        assert!(!widget_kind_supports_part(WidgetKind::HLayout, "field"));
    }

    #[test]
    fn registry_owns_generated_content_parts_and_exclusions() {
        assert!(widget_kind_supports_part(WidgetKind::Button, "before"));
        assert!(widget_kind_supports_part(WidgetKind::Label, "after"));
        assert!(!widget_kind_supports_part(WidgetKind::Window, "before"));
        assert!(!widget_kind_supports_part(WidgetKind::Spacer, "after"));
        assert!(!widget_kind_supports_part(WidgetKind::Unknown, "before"));
        assert_eq!(
            widget_part_renderer_status(WidgetKind::Button, "before"),
            Some("text")
        );
    }

    #[test]
    fn every_native_paint_fallback_part_is_registered_as_painted() {
        for kind in ALL_WIDGET_KINDS {
            for part in native_widget_paint_fallback_parts(*kind) {
                assert_eq!(
                    widget_part_renderer_status(*kind, part),
                    Some("paint"),
                    "{kind:?}::{part} has native paint fallback metadata but is not registered as painted"
                );
            }
        }
    }
}
