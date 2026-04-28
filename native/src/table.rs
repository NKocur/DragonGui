use crate::document::WidgetNode;
use crate::events::{SortDirection, TableState};
use crate::layout::Rect;
use crate::resources::ResourceRegistry;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy)]
pub struct TableMetrics {
    pub header_h: f32,
    pub row_h: f32,
    pub index_w: f32,
    pub col_w: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct VisibleTable {
    pub first_row: usize,
    pub row_count: usize,
    pub first_col: usize,
    pub col_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableHit {
    Header(usize),
    Cell { row: usize, col: usize },
}

pub fn metrics(theme: &Theme, sf: f32) -> TableMetrics {
    TableMetrics {
        header_h: theme.control_height() * sf,
        row_h: (theme.font_size + theme.spacing + 8.0).max(24.0) * sf,
        index_w: 64.0 * sf,
        col_w: 140.0 * sf,
    }
}

pub fn metrics_for_node(node: &WidgetNode, theme: &Theme, sf: f32) -> TableMetrics {
    let mut metrics = metrics(theme, sf);
    if let Some(header_h) = node.style.widget.table_header_height {
        metrics.header_h = (header_h * sf).max(1.0);
    }
    if let Some(row_h) = node.style.widget.table_row_height {
        metrics.row_h = (row_h * sf).max(1.0);
    }
    metrics
}

pub fn visible(table: &TableState, rect: &Rect, metrics: TableMetrics) -> VisibleTable {
    let body_h = (rect.h - metrics.header_h).max(0.0);
    let row_capacity = ((body_h / metrics.row_h).ceil() as usize + 1)
        .max(1)
        .min(table.page_size.saturating_add(2));
    let body_w = (rect.w - metrics.index_w).max(0.0);
    let col_capacity = ((body_w / metrics.col_w).ceil() as usize + 1).max(1);
    let first_row = table.scroll_row.min(table.rows.saturating_sub(1));
    let first_col = table.scroll_col.min(table.columns.len().saturating_sub(1));
    VisibleTable {
        first_row,
        row_count: row_capacity.min(table.rows.saturating_sub(first_row)),
        first_col,
        col_count: col_capacity.min(table.columns.len().saturating_sub(first_col)),
    }
}

pub fn row_bounds(rect: &Rect, metrics: TableMetrics, row_offset: usize) -> Option<(f32, f32)> {
    let top = rect.y + metrics.header_h + row_offset as f32 * metrics.row_h;
    let bottom = (top + metrics.row_h).min(rect.y + rect.h);
    if bottom <= top {
        None
    } else {
        Some((top, bottom))
    }
}

pub fn column_bounds(rect: &Rect, metrics: TableMetrics, col_offset: usize) -> Option<(f32, f32)> {
    let left = rect.x + metrics.index_w + col_offset as f32 * metrics.col_w;
    let right = (left + metrics.col_w).min(rect.x + rect.w);
    if right <= left {
        None
    } else {
        Some((left, right))
    }
}

pub fn hit(
    table: &TableState,
    rect: &Rect,
    metrics: TableMetrics,
    pos: [f32; 2],
) -> Option<TableHit> {
    if pos[0] < rect.x || pos[0] >= rect.x + rect.w || pos[1] < rect.y || pos[1] >= rect.y + rect.h
    {
        return None;
    }
    if pos[0] < rect.x + metrics.index_w {
        return None;
    }

    let visible = visible(table, rect, metrics);
    let col_offset = ((pos[0] - rect.x - metrics.index_w) / metrics.col_w).floor() as usize;
    if col_offset >= visible.col_count {
        return None;
    }
    let col = visible.first_col + col_offset;
    if pos[1] < rect.y + metrics.header_h {
        return Some(TableHit::Header(col));
    }

    let row_offset = ((pos[1] - rect.y - metrics.header_h) / metrics.row_h).floor() as usize;
    if row_offset >= visible.row_count {
        return None;
    }
    Some(TableHit::Cell {
        row: visible.first_row + row_offset,
        col,
    })
}

pub fn sort_suffix(table: &TableState, col: usize) -> &'static str {
    match table.sort {
        Some((sort_col, SortDirection::Asc)) if sort_col == col => " ^",
        Some((sort_col, SortDirection::Desc)) if sort_col == col => " v",
        _ => "",
    }
}

pub fn placeholder_cell(table: &TableState, row: usize, col: usize) -> String {
    let dtype = table.dtypes.get(col).map(String::as_str).unwrap_or("");
    if dtype.is_empty() {
        format!("row {row}")
    } else {
        format!("{dtype} #{row}")
    }
}

pub fn source_row(table: &TableState, display_row: usize) -> usize {
    table
        .row_order
        .as_ref()
        .and_then(|rows| rows.get(display_row).copied())
        .unwrap_or(display_row)
}

pub fn cell_text(
    table: &TableState,
    resources: &ResourceRegistry,
    row: usize,
    col: usize,
) -> String {
    let row = source_row(table, row);
    resources
        .table_cell_text(table.resource_id.as_deref(), row, col)
        .unwrap_or_else(|| placeholder_cell(table, row, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_bounds_clamp_partial_rows_to_table_rect() {
        let rect = Rect {
            x: 10.0,
            y: 20.0,
            w: 300.0,
            h: 72.0,
        };
        let metrics = TableMetrics {
            header_h: 30.0,
            row_h: 24.0,
            index_w: 64.0,
            col_w: 120.0,
        };

        assert_eq!(row_bounds(&rect, metrics, 0), Some((50.0, 74.0)));
        assert_eq!(row_bounds(&rect, metrics, 1), Some((74.0, 92.0)));
        assert_eq!(row_bounds(&rect, metrics, 2), None);
    }

    #[test]
    fn column_bounds_clamp_partial_columns_to_table_rect() {
        let rect = Rect {
            x: 10.0,
            y: 20.0,
            w: 250.0,
            h: 200.0,
        };
        let metrics = TableMetrics {
            header_h: 30.0,
            row_h: 24.0,
            index_w: 64.0,
            col_w: 120.0,
        };

        assert_eq!(column_bounds(&rect, metrics, 0), Some((74.0, 194.0)));
        assert_eq!(column_bounds(&rect, metrics, 1), Some((194.0, 260.0)));
        assert_eq!(column_bounds(&rect, metrics, 2), None);
    }

    #[test]
    fn cell_text_uses_sorted_source_row_mapping() {
        let table = TableState {
            columns: vec!["a".to_string()],
            dtypes: vec!["f32".to_string()],
            rows: 3,
            resource_id: None,
            page_size: 10,
            scroll_row: 0,
            scroll_col: 0,
            selected: None,
            sort: Some((0, SortDirection::Asc)),
            row_order: Some(vec![2, 0, 1]),
        };
        let resources = ResourceRegistry::default();

        assert_eq!(source_row(&table, 0), 2);
        assert_eq!(source_row(&table, 3), 3);
        assert_eq!(cell_text(&table, &resources, 0, 0), "f32 #2");
    }
}
