use crate::document::WidgetNode;
use crate::events::{TableSortColumn, TableState};
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
    Header(TableSortColumn),
    Cell { row: usize, col: usize },
}

pub fn metrics(theme: &Theme, sf: f32) -> TableMetrics {
    TableMetrics {
        header_h: theme.control_height() * sf,
        row_h: (theme.font_size + theme.spacing + 3.0).max(20.0) * sf,
        index_w: 48.0 * sf,
        col_w: 116.0 * sf,
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
    if let Some(index_w) = node.style.widget.table_index_width {
        metrics.index_w = (index_w * sf).max(1.0);
    }
    if let Some(col_w) = node.style.widget.table_column_width {
        metrics.col_w = (col_w * sf).max(1.0);
    }
    metrics
}

pub fn min_column_width(metrics: TableMetrics) -> f32 {
    metrics.col_w.mul_add(0.35, 0.0).clamp(44.0, 96.0)
}

pub fn column_width(table: &TableState, metrics: TableMetrics, col: usize) -> f32 {
    table
        .column_widths
        .get(col)
        .copied()
        .filter(|width| width.is_finite() && *width > 0.0)
        .unwrap_or(metrics.col_w)
        .max(1.0)
}

pub fn total_column_width(table: &TableState, metrics: TableMetrics) -> f32 {
    (0..table.columns.len())
        .map(|col| column_width(table, metrics, col))
        .sum()
}

pub fn visible(table: &TableState, rect: &Rect, metrics: TableMetrics) -> VisibleTable {
    let body_h = (rect.h - metrics.header_h).max(0.0);
    let row_capacity = ((body_h / metrics.row_h).ceil() as usize + 1)
        .max(1)
        .min(table.page_size.saturating_add(2));
    let body_w = (rect.w - metrics.index_w).max(0.0);
    let first_row = table.scroll_row.min(table.rows.saturating_sub(1));
    let first_col = table.scroll_col.min(table.columns.len().saturating_sub(1));
    let mut col_count = 0usize;
    let mut used_w = 0.0;
    let mut col = first_col;
    while col < table.columns.len() && (col_count == 0 || used_w < body_w) {
        used_w += column_width(table, metrics, col);
        col_count += 1;
        col += 1;
    }
    VisibleTable {
        first_row,
        row_count: row_capacity.min(table.rows.saturating_sub(first_row)),
        first_col,
        col_count: col_count.min(table.columns.len().saturating_sub(first_col)),
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

pub fn column_bounds(
    table: &TableState,
    rect: &Rect,
    metrics: TableMetrics,
    col_offset: usize,
) -> Option<(f32, f32)> {
    let first_col = table.scroll_col.min(table.columns.len().saturating_sub(1));
    let col = first_col.checked_add(col_offset)?;
    if col >= table.columns.len() {
        return None;
    }
    let mut left = rect.x + metrics.index_w;
    for prev in first_col..col {
        left += column_width(table, metrics, prev);
    }
    let right = (left + column_width(table, metrics, col)).min(rect.x + rect.w);
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
        if pos[1] < rect.y + metrics.header_h {
            return Some(TableHit::Header(TableSortColumn::Index));
        }
        return None;
    }

    let visible = visible(table, rect, metrics);
    let mut col = visible.first_col;
    let mut left = rect.x + metrics.index_w;
    let mut hit_col = None;
    for _ in 0..visible.col_count {
        let right = left + column_width(table, metrics, col);
        if pos[0] < right {
            hit_col = Some(col);
            break;
        }
        left = right;
        col += 1;
    }
    let col = hit_col?;
    if pos[1] < rect.y + metrics.header_h {
        return Some(TableHit::Header(TableSortColumn::Data(col)));
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
    use crate::events::{SortDirection, TableSortColumn};

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
        let table = TableState {
            columns: vec!["a".to_string(), "b".to_string()],
            dtypes: vec![],
            rows: 3,
            resource_id: None,
            page_size: 10,
            scroll_row: 0,
            scroll_col: 0,
            selected: None,
            sort: None,
            row_order: None,
            column_widths: Vec::new(),
        };

        assert_eq!(
            column_bounds(&table, &rect, metrics, 0),
            Some((74.0, 194.0))
        );
        assert_eq!(
            column_bounds(&table, &rect, metrics, 1),
            Some((194.0, 260.0))
        );
        assert_eq!(column_bounds(&table, &rect, metrics, 2), None);
    }

    #[test]
    fn variable_column_widths_drive_visibility_bounds_and_hit_testing() {
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            w: 260.0,
            h: 120.0,
        };
        let metrics = TableMetrics {
            header_h: 30.0,
            row_h: 24.0,
            index_w: 40.0,
            col_w: 100.0,
        };
        let table = TableState {
            columns: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            dtypes: vec![],
            rows: 3,
            resource_id: None,
            page_size: 10,
            scroll_row: 0,
            scroll_col: 0,
            selected: None,
            sort: None,
            row_order: None,
            column_widths: vec![60.0, 140.0, 80.0],
        };

        let visible = visible(&table, &rect, metrics);

        assert_eq!(visible.col_count, 3);
        assert_eq!(
            column_bounds(&table, &rect, metrics, 1),
            Some((100.0, 240.0))
        );
        assert_eq!(
            hit(&table, &rect, metrics, [120.0, 40.0]),
            Some(TableHit::Cell { row: 0, col: 1 })
        );
    }

    #[test]
    fn metrics_for_node_uses_css_table_widths() {
        let mut node = crate::document::parse_widget_node(&serde_json::json!({
            "id": "table",
            "type": "dataframe_table"
        }))
        .unwrap();
        node.style.widget.table_header_height = Some(32.0);
        node.style.widget.table_row_height = Some(28.0);
        node.style.widget.table_index_width = Some(72.0);
        node.style.widget.table_column_width = Some(180.0);

        let metrics = metrics_for_node(&node, &Theme::dark(), 1.5);

        assert_eq!(metrics.header_h, 48.0);
        assert_eq!(metrics.row_h, 42.0);
        assert_eq!(metrics.index_w, 108.0);
        assert_eq!(metrics.col_w, 270.0);
    }

    #[test]
    fn default_metrics_are_compact_for_dense_data() {
        let metrics = metrics(&Theme::dark(), 1.0);

        assert_eq!(metrics.header_h, 25.0);
        assert_eq!(metrics.row_h, 21.0);
        assert_eq!(metrics.index_w, 48.0);
        assert_eq!(metrics.col_w, 116.0);
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
            sort: Some((TableSortColumn::Data(0), SortDirection::Asc)),
            row_order: Some(vec![2, 0, 1]),
            column_widths: Vec::new(),
        };
        let resources = ResourceRegistry::default();

        assert_eq!(source_row(&table, 0), 2);
        assert_eq!(source_row(&table, 3), 3);
        assert_eq!(cell_text(&table, &resources, 0, 0), "f32 #2");
    }

    #[test]
    fn hit_index_header_returns_index_sort_target() {
        let table = TableState {
            columns: vec!["a".to_string(), "b".to_string()],
            dtypes: vec![],
            rows: 3,
            resource_id: None,
            page_size: 10,
            scroll_row: 0,
            scroll_col: 0,
            selected: None,
            sort: None,
            row_order: None,
            column_widths: Vec::new(),
        };
        let rect = Rect {
            x: 10.0,
            y: 20.0,
            w: 300.0,
            h: 120.0,
        };
        let metrics = TableMetrics {
            header_h: 30.0,
            row_h: 24.0,
            index_w: 64.0,
            col_w: 100.0,
        };

        assert_eq!(
            hit(&table, &rect, metrics, [20.0, 25.0]),
            Some(TableHit::Header(TableSortColumn::Index))
        );
        assert_eq!(hit(&table, &rect, metrics, [20.0, 60.0]), None);
    }
}
