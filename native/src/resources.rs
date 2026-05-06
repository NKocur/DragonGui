use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::str;

use serde_json::{json, Map, Value};

use crate::commands::TableColumnPacket;
use crate::document::{NodeProps, WidgetKind, WidgetNode};
use crate::events::SortDirection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableColumnData {
    F32(Vec<u8>),
    F64(Vec<u8>),
    I32(Vec<u8>),
    I64(Vec<u8>),
    U32(Vec<u8>),
    U64(Vec<u8>),
    Bool(Vec<u8>),
    Utf8 { offsets: Vec<usize>, data: Vec<u8> },
}

impl TableColumnData {
    fn from_packet(packet: TableColumnPacket) -> Option<Self> {
        match packet.dtype.as_str() {
            "f32" if packet.bytes.len() % 4 == 0 => Some(Self::F32(packet.bytes)),
            "f64" if packet.bytes.len() % 8 == 0 => Some(Self::F64(packet.bytes)),
            "i32" if packet.bytes.len() % 4 == 0 => Some(Self::I32(packet.bytes)),
            "i64" if packet.bytes.len() % 8 == 0 => Some(Self::I64(packet.bytes)),
            "u32" if packet.bytes.len() % 4 == 0 => Some(Self::U32(packet.bytes)),
            "u64" if packet.bytes.len() % 8 == 0 => Some(Self::U64(packet.bytes)),
            "bool" => Some(Self::Bool(packet.bytes)),
            "utf8" => decode_utf8_packet(packet.bytes),
            _ => None,
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::F32(bytes) | Self::I32(bytes) | Self::U32(bytes) => bytes.len() / 4,
            Self::F64(bytes) | Self::I64(bytes) | Self::U64(bytes) => bytes.len() / 8,
            Self::Bool(bytes) => bytes.len(),
            Self::Utf8 { offsets, .. } => offsets.len().saturating_sub(1),
        }
    }

    fn bytes_len(&self) -> usize {
        match self {
            Self::F32(bytes)
            | Self::F64(bytes)
            | Self::I32(bytes)
            | Self::I64(bytes)
            | Self::U32(bytes)
            | Self::U64(bytes)
            | Self::Bool(bytes) => bytes.len(),
            Self::Utf8 { offsets, data } => offsets.len() * 8 + data.len(),
        }
    }

    fn value_text(&self, row: usize) -> Option<String> {
        match self {
            Self::F32(bytes) => {
                read_array::<4>(bytes, row).map(|b| format_float(f32::from_le_bytes(b) as f64))
            }
            Self::F64(bytes) => {
                read_array::<8>(bytes, row).map(|b| format_float(f64::from_le_bytes(b)))
            }
            Self::I32(bytes) => {
                read_array::<4>(bytes, row).map(|b| i32::from_le_bytes(b).to_string())
            }
            Self::I64(bytes) => {
                read_array::<8>(bytes, row).map(|b| i64::from_le_bytes(b).to_string())
            }
            Self::U32(bytes) => {
                read_array::<4>(bytes, row).map(|b| u32::from_le_bytes(b).to_string())
            }
            Self::U64(bytes) => {
                read_array::<8>(bytes, row).map(|b| u64::from_le_bytes(b).to_string())
            }
            Self::Bool(bytes) => bytes
                .get(row)
                .map(|v| if *v == 0 { "false" } else { "true" }.to_string()),
            Self::Utf8 { offsets, data } => {
                let start = *offsets.get(row)?;
                let end = *offsets.get(row + 1)?;
                str::from_utf8(data.get(start..end)?)
                    .ok()
                    .map(str::to_string)
            }
        }
    }

    fn compare_rows(&self, left: usize, right: usize) -> Ordering {
        match self {
            Self::F32(bytes) => compare_float_values(
                read_array::<4>(bytes, left)
                    .map(f32::from_le_bytes)
                    .map(f64::from),
                read_array::<4>(bytes, right)
                    .map(f32::from_le_bytes)
                    .map(f64::from),
            ),
            Self::F64(bytes) => compare_float_values(
                read_array::<8>(bytes, left).map(f64::from_le_bytes),
                read_array::<8>(bytes, right).map(f64::from_le_bytes),
            ),
            Self::I32(bytes) => compare_option_values(
                read_array::<4>(bytes, left).map(i32::from_le_bytes),
                read_array::<4>(bytes, right).map(i32::from_le_bytes),
            ),
            Self::I64(bytes) => compare_option_values(
                read_array::<8>(bytes, left).map(i64::from_le_bytes),
                read_array::<8>(bytes, right).map(i64::from_le_bytes),
            ),
            Self::U32(bytes) => compare_option_values(
                read_array::<4>(bytes, left).map(u32::from_le_bytes),
                read_array::<4>(bytes, right).map(u32::from_le_bytes),
            ),
            Self::U64(bytes) => compare_option_values(
                read_array::<8>(bytes, left).map(u64::from_le_bytes),
                read_array::<8>(bytes, right).map(u64::from_le_bytes),
            ),
            Self::Bool(bytes) => {
                compare_option_values(bytes.get(left).copied(), bytes.get(right).copied())
            }
            Self::Utf8 { offsets, data } => {
                let left = utf8_value(offsets, data, left);
                let right = utf8_value(offsets, data, right);
                compare_option_by(left, right, |left, right| left.cmp(right))
            }
        }
    }
}

fn decode_utf8_packet(bytes: Vec<u8>) -> Option<TableColumnData> {
    let row_count = u64::from_le_bytes(bytes.get(0..8)?.try_into().ok()?) as usize;
    let offsets_start: usize = 8;
    let offsets_len = row_count.checked_add(1)?.checked_mul(8)?;
    let data_start = offsets_start.checked_add(offsets_len)?;
    if bytes.len() < data_start {
        return None;
    }
    let mut offsets = Vec::with_capacity(row_count + 1);
    for chunk in bytes.get(offsets_start..data_start)?.chunks_exact(8) {
        offsets.push(u64::from_le_bytes(chunk.try_into().ok()?) as usize);
    }
    let data = bytes.get(data_start..)?.to_vec();
    if offsets.first().copied() != Some(0)
        || offsets.windows(2).any(|pair| pair[0] > pair[1])
        || offsets.last().copied().unwrap_or(0) > data.len()
    {
        return None;
    }
    Some(TableColumnData::Utf8 { offsets, data })
}

fn read_array<const N: usize>(bytes: &[u8], row: usize) -> Option<[u8; N]> {
    let offset = row.checked_mul(N)?;
    let end = offset.checked_add(N)?;
    bytes.get(offset..end)?.try_into().ok()
}

fn format_float(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else if value.abs() >= 1.0e6 || value.abs() < 1.0e-4 {
        format!("{value:.6e}")
    } else {
        format!("{value:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn utf8_value<'a>(offsets: &[usize], data: &'a [u8], row: usize) -> Option<&'a str> {
    let start = *offsets.get(row)?;
    let end = *offsets.get(row + 1)?;
    str::from_utf8(data.get(start..end)?).ok()
}

fn compare_option_values<T: Ord>(left: Option<T>, right: Option<T>) -> Ordering {
    compare_option_by(left, right, Ord::cmp)
}

fn compare_option_by<T>(
    left: Option<T>,
    right: Option<T>,
    compare: impl FnOnce(&T, &T) -> Ordering,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare(&left, &right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_float_values(left: Option<f64>, right: Option<f64>) -> Ordering {
    compare_option_by(left, right, |left, right| {
        match (left.is_nan(), right.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => left.partial_cmp(right).unwrap_or(Ordering::Equal),
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnBuffer {
    pub name: String,
    pub data: TableColumnData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableResource {
    pub id: String,
    pub columns: Vec<String>,
    pub dtypes: Vec<String>,
    pub rows: usize,
    pub sample_rows: usize,
    pub cells: Vec<Vec<String>>,
    pub column_buffers: Vec<TableColumnBuffer>,
    pub version: u64,
}

impl TableResource {
    fn props_sample_rows(props: &NodeProps) -> usize {
        let rows = props.table_rows.unwrap_or(0);
        props
            .table_sample_rows
            .unwrap_or(props.table_cells.len())
            .min(rows)
    }

    fn from_props(id: String, props: &NodeProps, previous_version: u64) -> Self {
        let rows = props.table_rows.unwrap_or(0);
        let sample_rows = Self::props_sample_rows(props);
        Self {
            id,
            columns: props.table_columns.clone(),
            dtypes: props.table_dtypes.clone(),
            rows,
            sample_rows,
            cells: props.table_cells.clone(),
            column_buffers: Vec::new(),
            version: previous_version,
        }
    }

    fn same_payload(&self, props: &NodeProps) -> bool {
        self.columns == props.table_columns
            && self.dtypes == props.table_dtypes
            && Some(self.rows) == props.table_rows
            && self.sample_rows == Self::props_sample_rows(props)
            && self.cells == props.table_cells
    }

    pub fn cell(&self, row: usize, col: usize) -> Option<&str> {
        self.cells
            .get(row)
            .and_then(|cells| cells.get(col))
            .map(String::as_str)
    }

    pub fn cell_text(&self, row: usize, col: usize) -> Option<String> {
        if let Some(text) = self.cell(row, col) {
            return Some(text.to_string());
        }
        let column_name = self.columns.get(col)?;
        self.column_buffers
            .iter()
            .find(|buffer| &buffer.name == column_name)
            .and_then(|buffer| buffer.data.value_text(row))
    }

    pub fn sorted_rows(&self, col: usize, direction: SortDirection) -> Option<Vec<usize>> {
        if col >= self.columns.len() {
            return None;
        }
        let column_name = self.columns.get(col)?;
        let column_buffer = self
            .column_buffers
            .iter()
            .find(|buffer| &buffer.name == column_name);
        let mut rows: Vec<usize> = (0..self.rows).collect();
        rows.sort_by(|left, right| {
            let ordering = if let Some(buffer) = column_buffer {
                buffer.data.compare_rows(*left, *right)
            } else {
                compare_option_values(self.cell_text(*left, col), self.cell_text(*right, col))
            };
            match direction {
                SortDirection::Asc => ordering,
                SortDirection::Desc => ordering.reverse(),
            }
            .then_with(|| left.cmp(right))
        });
        Some(rows)
    }

    fn snapshot(&self) -> Value {
        json!({
            "columns": self.columns.len(),
            "rows": self.rows,
            "sample_rows": self.sample_rows,
            "sampled_cells": self.cells.iter().map(Vec::len).sum::<usize>(),
            "buffer_columns": self.column_buffers.len(),
            "buffer_bytes": self.column_buffers.iter().map(|buffer| buffer.data.bytes_len()).sum::<usize>(),
            "version": self.version,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferResource {
    pub id: String,
    pub kind: String,
    pub bytes: Vec<u8>,
    pub version: u64,
    pub owner_id: Option<String>,
}

impl BufferResource {
    fn snapshot(&self) -> Value {
        json!({
            "kind": self.kind,
            "bytes": self.bytes.len(),
            "version": self.version,
            "owner_id": self.owner_id,
        })
    }
}

#[derive(Debug, Default)]
pub struct ResourceRegistry {
    tables: HashMap<String, TableResource>,
    buffers: HashMap<String, BufferResource>,
}

impl ResourceRegistry {
    pub fn sync_from_tree(&mut self, root: &WidgetNode) {
        let mut active_tables = HashSet::new();
        let mut active_widgets = HashSet::new();
        self.sync_node(root, &mut active_tables, &mut active_widgets);
        self.tables.retain(|id, _| active_tables.contains(id));
        self.buffers.retain(|_, resource| {
            resource
                .owner_id
                .as_ref()
                .map(|owner_id| active_widgets.contains(owner_id))
                .unwrap_or(true)
        });
    }

    pub fn table(&self, id: &str) -> Option<&TableResource> {
        self.tables.get(id)
    }

    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    #[cfg(test)]
    fn buffer(&self, id: &str) -> Option<&BufferResource> {
        self.buffers.get(id)
    }

    pub fn table_cell_text(
        &self,
        resource_id: Option<&str>,
        row: usize,
        col: usize,
    ) -> Option<String> {
        let resource_id = resource_id?;
        self.table(resource_id)?.cell_text(row, col)
    }

    pub fn sorted_table_rows(
        &self,
        resource_id: Option<&str>,
        col: usize,
        rows: usize,
        direction: SortDirection,
    ) -> Option<Vec<usize>> {
        let resource_id = resource_id?;
        let mut row_order = self.table(resource_id)?.sorted_rows(col, direction)?;
        row_order.truncate(rows);
        (row_order.len() == rows).then_some(row_order)
    }

    pub fn update_table_columns(
        &mut self,
        resource_id: &str,
        props: &NodeProps,
        columns: Vec<TableColumnPacket>,
    ) {
        let previous_version = self
            .tables
            .get(resource_id)
            .map(|resource| resource.version)
            .unwrap_or(0);
        self.upsert_table(resource_id.to_string(), props);
        let Some(resource) = self.tables.get_mut(resource_id) else {
            return;
        };
        resource.column_buffers = columns
            .into_iter()
            .filter_map(|packet| {
                let name = packet.name.clone();
                TableColumnData::from_packet(packet).and_then(|data| {
                    (data.len() > 0 && data.len() <= resource.rows)
                        .then_some(TableColumnBuffer { name, data })
                })
            })
            .collect();
        if resource.version == previous_version {
            resource.version += 1;
        }
    }

    pub fn update_buffer(
        &mut self,
        id: String,
        kind: String,
        bytes: Vec<u8>,
        owner_id: Option<String>,
    ) {
        let existing = self.buffers.get(&id);
        let version = existing.map(|resource| resource.version + 1).unwrap_or(1);
        let owner_id = owner_id.or_else(|| existing.and_then(|resource| resource.owner_id.clone()));
        self.buffers.insert(
            id.clone(),
            BufferResource {
                id,
                kind,
                bytes,
                version,
                owner_id,
            },
        );
    }

    pub fn release(&mut self, id: &str) -> bool {
        let table_released = self.tables.remove(id).is_some();
        let buffer_released = self.buffers.remove(id).is_some();
        table_released || buffer_released
    }

    pub fn snapshot(&self) -> Value {
        let mut tables = Map::new();
        for (id, resource) in &self.tables {
            tables.insert(id.clone(), resource.snapshot());
        }
        let mut buffers = Map::new();
        for (id, resource) in &self.buffers {
            buffers.insert(id.clone(), resource.snapshot());
        }
        json!({
            "tables": {
                "count": self.tables.len(),
                "items": tables,
            },
            "buffers": {
                "count": self.buffers.len(),
                "items": buffers,
            }
        })
    }

    fn sync_node(
        &mut self,
        node: &WidgetNode,
        active_tables: &mut HashSet<String>,
        active_widgets: &mut HashSet<String>,
    ) {
        active_widgets.insert(node.id.clone());
        if node.kind == WidgetKind::DataFrameTable {
            if let Some(resource_id) = node.props.table_resource_id.clone() {
                active_tables.insert(resource_id.clone());
                self.upsert_table(resource_id, &node.props);
            }
        }
        for child in &node.children {
            self.sync_node(child, active_tables, active_widgets);
        }
    }

    fn upsert_table(&mut self, resource_id: String, props: &NodeProps) {
        if self
            .tables
            .get(&resource_id)
            .is_some_and(|resource| resource.same_payload(props))
        {
            return;
        }
        let version = self
            .tables
            .get(&resource_id)
            .map(|resource| resource.version + 1)
            .unwrap_or(1);
        let previous_buffers = self
            .tables
            .get(&resource_id)
            .map(|resource| resource.column_buffers.clone())
            .unwrap_or_default();
        let mut resource = TableResource::from_props(resource_id.clone(), props, version);
        resource.column_buffers = previous_buffers;
        self.tables.insert(resource_id, resource);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_syncs_table_resources_and_releases_stale_entries() {
        let root = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "props": {},
            "children": [{
                "id": "table",
                "type": "dataframe_table",
                "props": {
                    "resource_id": "table-resource",
                    "frame": {
                        "columns": ["a"],
                        "dtypes": ["int"],
                        "rows": 2
                    },
                    "sample_rows": 2,
                    "cells": [["1"], ["2"]]
                }
            }]
        }))
        .unwrap();
        let empty = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "props": {},
            "children": []
        }))
        .unwrap();

        let mut registry = ResourceRegistry::default();
        registry.sync_from_tree(&root);

        assert_eq!(registry.table_count(), 1);
        assert_eq!(
            registry.table_cell_text(Some("table-resource"), 1, 0),
            Some("2".to_string())
        );
        assert_eq!(registry.table("table-resource").unwrap().version, 1);

        registry.sync_from_tree(&root);
        assert_eq!(registry.table("table-resource").unwrap().version, 1);

        registry.sync_from_tree(&empty);
        assert_eq!(registry.table_count(), 0);
    }

    #[test]
    fn registry_formats_buffer_backed_cells_beyond_sample() {
        let root = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "props": {},
            "children": [{
                "id": "table",
                "type": "dataframe_table",
                "props": {
                    "resource_id": "table-resource",
                    "frame": {
                        "columns": ["x", "flag", "label"],
                        "dtypes": ["float32", "bool", "str"],
                        "rows": 3
                    },
                    "sample_rows": 1,
                    "cells": [["1"]]
                }
            }]
        }))
        .unwrap();
        let props = &root.children[0].props;
        let mut registry = ResourceRegistry::default();
        registry.sync_from_tree(&root);
        registry.update_table_columns(
            "table-resource",
            props,
            vec![
                TableColumnPacket {
                    name: "x".to_string(),
                    dtype: "f32".to_string(),
                    bytes: [1.0_f32, 2.5, 3.75]
                        .into_iter()
                        .flat_map(f32::to_le_bytes)
                        .collect(),
                },
                TableColumnPacket {
                    name: "flag".to_string(),
                    dtype: "bool".to_string(),
                    bytes: vec![1, 0, 1],
                },
                TableColumnPacket {
                    name: "label".to_string(),
                    dtype: "utf8".to_string(),
                    bytes: utf8_packet(["alpha", "beta", "gamma"]),
                },
            ],
        );

        assert_eq!(
            registry.table_cell_text(Some("table-resource"), 0, 0),
            Some("1".to_string())
        );
        assert_eq!(
            registry.table_cell_text(Some("table-resource"), 1, 0),
            Some("2.5".to_string())
        );
        assert_eq!(
            registry.table_cell_text(Some("table-resource"), 1, 1),
            Some("false".to_string())
        );
        assert_eq!(
            registry.table_cell_text(Some("table-resource"), 2, 2),
            Some("gamma".to_string())
        );
    }

    #[test]
    fn registry_accepts_partial_table_column_buffers() {
        let root = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "props": {},
            "children": [{
                "id": "table",
                "type": "dataframe_table",
                "props": {
                    "resource_id": "table-resource",
                    "frame": {
                        "columns": ["x"],
                        "dtypes": ["float32"],
                        "rows": 3
                    },
                    "sample_rows": 0,
                    "cells": []
                }
            }]
        }))
        .unwrap();
        let props = &root.children[0].props;
        let mut registry = ResourceRegistry::default();
        registry.sync_from_tree(&root);
        registry.update_table_columns(
            "table-resource",
            props,
            vec![TableColumnPacket {
                name: "x".to_string(),
                dtype: "f32".to_string(),
                bytes: [1.0_f32, 2.5]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            }],
        );

        assert_eq!(
            registry.table_cell_text(Some("table-resource"), 1, 0),
            Some("2.5".to_string())
        );
        assert_eq!(registry.table_cell_text(Some("table-resource"), 2, 0), None);
    }

    #[test]
    fn registry_sorts_buffer_backed_rows_by_typed_values() {
        let root = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "props": {},
            "children": [{
                "id": "table",
                "type": "dataframe_table",
                "props": {
                    "resource_id": "table-resource",
                    "frame": {
                        "columns": ["x", "label"],
                        "dtypes": ["float32", "str"],
                        "rows": 3
                    },
                    "sample_rows": 0,
                    "cells": []
                }
            }]
        }))
        .unwrap();
        let props = &root.children[0].props;
        let mut registry = ResourceRegistry::default();
        registry.sync_from_tree(&root);
        registry.update_table_columns(
            "table-resource",
            props,
            vec![
                TableColumnPacket {
                    name: "x".to_string(),
                    dtype: "f32".to_string(),
                    bytes: [10.0_f32, 2.0, 30.0]
                        .into_iter()
                        .flat_map(f32::to_le_bytes)
                        .collect(),
                },
                TableColumnPacket {
                    name: "label".to_string(),
                    dtype: "utf8".to_string(),
                    bytes: utf8_packet(["beta", "alpha", "gamma"]),
                },
            ],
        );

        assert_eq!(
            registry.sorted_table_rows(Some("table-resource"), 0, 3, SortDirection::Asc),
            Some(vec![1, 0, 2])
        );
        assert_eq!(
            registry.sorted_table_rows(Some("table-resource"), 0, 3, SortDirection::Desc),
            Some(vec![2, 0, 1])
        );
        assert_eq!(
            registry.sorted_table_rows(Some("table-resource"), 1, 3, SortDirection::Asc),
            Some(vec![1, 0, 2])
        );
    }

    #[test]
    fn registry_updates_and_releases_generic_buffers() {
        let mut registry = ResourceRegistry::default();
        registry.update_buffer(
            "image-1".to_string(),
            "rgba8".to_string(),
            vec![1, 2, 3, 4],
            None,
        );

        let buffer = registry.buffer("image-1").unwrap();
        assert_eq!(buffer.kind, "rgba8");
        assert_eq!(buffer.bytes, vec![1, 2, 3, 4]);
        assert_eq!(buffer.version, 1);
        assert_eq!(registry.buffer_count(), 1);

        registry.update_buffer("image-1".to_string(), "rgba8".to_string(), vec![5, 6], None);
        let buffer = registry.buffer("image-1").unwrap();
        assert_eq!(buffer.bytes, vec![5, 6]);
        assert_eq!(buffer.version, 2);

        assert!(registry.release("image-1"));
        assert_eq!(registry.buffer_count(), 0);
        assert!(!registry.release("image-1"));
    }

    #[test]
    fn registry_auto_purges_widget_owned_buffers_only() {
        let root = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "props": {},
            "children": [{
                "id": "owner",
                "type": "label",
                "props": {"text": "owns buffer"}
            }]
        }))
        .unwrap();
        let empty = crate::document::parse_widget_node(&json!({
            "id": "root",
            "type": "window",
            "props": {},
            "children": []
        }))
        .unwrap();

        let mut registry = ResourceRegistry::default();
        registry.update_buffer(
            "owned".to_string(),
            "bytes".to_string(),
            vec![1],
            Some("owner".to_string()),
        );
        registry.update_buffer("app".to_string(), "bytes".to_string(), vec![2], None);

        registry.sync_from_tree(&root);
        assert_eq!(registry.buffer_count(), 2);

        registry.sync_from_tree(&empty);
        assert!(registry.buffer("owned").is_none());
        assert!(registry.buffer("app").is_some());
        assert_eq!(registry.buffer_count(), 1);
    }

    fn utf8_packet<const N: usize>(values: [&str; N]) -> Vec<u8> {
        let mut data = Vec::new();
        let mut offsets = vec![0_usize];
        for value in values {
            data.extend_from_slice(value.as_bytes());
            offsets.push(data.len());
        }
        let mut out = Vec::new();
        out.extend_from_slice(&(N as u64).to_le_bytes());
        for offset in offsets {
            out.extend_from_slice(&(offset as u64).to_le_bytes());
        }
        out.extend_from_slice(&data);
        out
    }
}
