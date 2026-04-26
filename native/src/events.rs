use std::collections::{HashMap, HashSet};

use crate::document::{WidgetKind, WidgetNode};
use crate::layout::{LayoutResult, Rect};
use crate::style::SLIDER_TRACK_MARGIN_LP;

// ---------------------------------------------------------------------------
// ChangeValue — argument passed to on_change callbacks
// ---------------------------------------------------------------------------

/// Value carried by a widget change event.
#[derive(Debug, Clone)]
pub enum ChangeValue {
    Bool(bool),
    Float(f32),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TableState {
    pub columns: Vec<String>,
    pub dtypes: Vec<String>,
    pub rows: usize,
    pub resource_id: Option<String>,
    pub page_size: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
    pub selected: Option<(usize, usize)>,
    pub sort: Option<(usize, SortDirection)>,
}

impl TableState {
    fn new(node: &WidgetNode) -> Self {
        let rows = node.props.table_rows.unwrap_or(0);
        let columns = node.props.table_columns.clone();
        let mut dtypes = node.props.table_dtypes.clone();
        if dtypes.len() < columns.len() {
            dtypes.resize(columns.len(), String::new());
        }
        Self {
            columns,
            dtypes,
            rows,
            resource_id: node.props.table_resource_id.clone(),
            page_size: node.props.page_size.unwrap_or(100).max(1),
            scroll_row: 0,
            scroll_col: 0,
            selected: None,
            sort: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NavigationItem {
    pub id: String,
    pub value: String,
    pub disabled: bool,
}

// ---------------------------------------------------------------------------
// WidgetState — mutable interactive state tracked entirely in Rust
// ---------------------------------------------------------------------------

/// Per-widget mutable state for interactive controls.
///
/// Initialised from the parsed widget tree at startup; updated in-place as
/// the user interacts with the UI.
#[derive(Debug, Clone, Default)]
pub struct WidgetState {
    /// Checkbox checked/unchecked state keyed by widget id.
    pub checked: HashMap<String, bool>,
    /// Slider current value keyed by widget id.
    pub float_val: HashMap<String, f32>,
    /// Slider (min, max) range keyed by widget id.
    pub float_range: HashMap<String, (f32, f32)>,
    /// Slider keyboard step keyed by widget id.
    pub float_step: HashMap<String, f32>,
    /// TextInput current value keyed by widget id.
    pub text_val: HashMap<String, String>,
    /// TextInput placeholder keyed by widget id.
    pub text_placeholder: HashMap<String, String>,
    /// TextInput cursor byte offset keyed by widget id.
    pub text_cursor: HashMap<String, usize>,
    /// Dropdown items keyed by widget id.
    pub dropdown_items: HashMap<String, Vec<String>>,
    /// Dropdown selected item index keyed by widget id.
    pub dropdown_index: HashMap<String, usize>,
    /// Disabled widgets keyed by widget id.
    pub disabled: HashSet<String>,
    /// Keyboard focus traversal order.
    pub focus_order: Vec<String>,
    /// Keyboard-focused widget id.
    pub focused: Option<String>,
    /// Widget currently under the cursor (interactive widgets only).
    pub hovered: Option<String>,
    /// Widget that received a pointer-down (not yet released).
    pub pressed: Option<String>,
    /// Dropdown whose menu is currently open.
    pub open_dropdown: Option<String>,
    /// DataFrame table state keyed by widget id.
    pub tables: HashMap<String, TableState>,
    /// Tab child ids and route values keyed by Tabs widget id.
    pub tabs: HashMap<String, Vec<NavigationItem>>,
    /// Parent Tabs id keyed by Tab id.
    pub tab_parent: HashMap<String, String>,
    /// Route value keyed by Tab id.
    pub tab_values: HashMap<String, String>,
    /// Active Tab route value keyed by Tabs widget id.
    pub active_tabs: HashMap<String, String>,
    /// Page child ids and route values keyed by Pages widget id.
    pub pages: HashMap<String, Vec<NavigationItem>>,
    /// Parent Pages id keyed by Page id.
    pub page_parent: HashMap<String, String>,
    /// Route value keyed by Page id.
    pub page_values: HashMap<String, String>,
    /// Active Page route value keyed by Pages widget id.
    pub active_pages: HashMap<String, String>,
    /// Target Page route value keyed by NavItem id.
    pub nav_targets: HashMap<String, String>,
    /// First Pages widget that owns each page route value.
    pub page_owner: HashMap<String, String>,
}

impl WidgetState {
    /// Build initial state from the parsed widget tree.
    pub fn from_tree(tree: &WidgetNode) -> Self {
        let mut s = Self::default();
        collect_state(tree, &mut s, None);
        s
    }

    /// Toggle a checkbox and return the new value.
    pub fn toggle_checkbox(&mut self, id: &str) -> bool {
        let entry = self.checked.entry(id.to_string()).or_insert(false);
        *entry = !*entry;
        *entry
    }

    /// Set a checkbox from a live native command.
    pub fn set_checked(&mut self, id: &str, checked: bool) -> Option<bool> {
        if !self.checked.contains_key(id) {
            return None;
        }
        self.checked.insert(id.to_string(), checked);
        Some(checked)
    }

    /// Update a slider value.
    pub fn set_float(&mut self, id: &str, v: f32) -> f32 {
        let (min, max) = self.float_range.get(id).copied().unwrap_or((0.0, 1.0));
        let clamped = v.clamp(min.min(max), min.max(max));
        self.float_val.insert(id.to_string(), clamped);
        clamped
    }

    /// Set a slider value from a live native command.
    pub fn try_set_float(&mut self, id: &str, v: f32) -> Option<f32> {
        if !self.float_val.contains_key(id) && !self.float_range.contains_key(id) {
            return None;
        }
        Some(self.set_float(id, v))
    }

    pub fn adjust_float(&mut self, id: &str, direction: f32) -> Option<f32> {
        let current = self.float_val.get(id).copied()?;
        let step = self.float_step.get(id).copied().unwrap_or(0.01).abs();
        Some(self.set_float(id, current + step * direction))
    }

    /// Return the slider normalized position `t ∈ [0, 1]`.
    pub fn slider_t(&self, id: &str) -> f32 {
        let v = self.float_val.get(id).copied().unwrap_or(0.0);
        let (min, max) = self.float_range.get(id).copied().unwrap_or((0.0, 1.0));
        if max > min {
            ((v - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn is_disabled(&self, id: &str) -> bool {
        self.disabled.contains(id)
    }

    pub fn focus_widget(&mut self, id: Option<String>) {
        self.focused = id.filter(|id| {
            self.focus_order.iter().any(|focus_id| focus_id == id) && !self.is_disabled(id)
        });
        if !self
            .focused
            .as_deref()
            .is_some_and(|id| self.open_dropdown.as_deref() == Some(id))
        {
            self.open_dropdown = None;
        }
    }

    pub fn focus_next_visible(&mut self, layout: &LayoutResult, reverse: bool) {
        if self.focus_order.is_empty() {
            self.focused = None;
            return;
        }
        let visible: Vec<String> = self
            .focus_order
            .iter()
            .filter(|id| layout.rects.contains_key(*id) && !self.is_disabled(id))
            .cloned()
            .collect();
        if visible.is_empty() {
            self.focused = None;
            return;
        }
        let len = visible.len();
        let start = self
            .focused
            .as_ref()
            .and_then(|id| visible.iter().position(|candidate| candidate == id));
        for offset in 1..=len {
            let idx = match (start, reverse) {
                (Some(i), true) => (i + len - offset) % len,
                (Some(i), false) => (i + offset) % len,
                (None, true) => len - offset,
                (None, false) => offset - 1,
            };
            self.focused = Some(visible[idx].clone());
            self.open_dropdown = None;
            return;
        }
    }

    pub fn text_for(&self, id: &str) -> Option<&str> {
        self.text_val.get(id).map(String::as_str)
    }

    pub fn set_text_value(&mut self, id: &str, value: String) -> Option<String> {
        if !self.text_val.contains_key(id) {
            return None;
        }
        self.text_cursor.insert(id.to_string(), value.len());
        self.text_val.insert(id.to_string(), value.clone());
        self.open_dropdown = None;
        Some(value)
    }

    pub fn placeholder_for(&self, id: &str) -> Option<&str> {
        self.text_placeholder.get(id).map(String::as_str)
    }

    pub fn dropdown_value(&self, id: &str) -> Option<&str> {
        let items = self.dropdown_items.get(id)?;
        let idx = self.dropdown_index.get(id).copied().unwrap_or(0);
        items.get(idx).map(String::as_str)
    }

    pub fn set_dropdown_value(&mut self, id: &str, value: &str) -> Option<String> {
        let items = self.dropdown_items.get(id)?;
        let idx = items.iter().position(|item| item == value)?;
        self.dropdown_index.insert(id.to_string(), idx);
        self.open_dropdown = None;
        Some(value.to_string())
    }

    pub fn insert_text(&mut self, id: &str, text: &str) -> Option<String> {
        if text.is_empty() {
            return self.text_val.get(id).cloned();
        }
        let value = self.text_val.get_mut(id)?;
        let cursor = self
            .text_cursor
            .entry(id.to_string())
            .or_insert(value.len());
        *cursor = clamp_boundary(value, *cursor);
        value.insert_str(*cursor, text);
        *cursor += text.len();
        Some(value.clone())
    }

    pub fn backspace_text(&mut self, id: &str) -> Option<String> {
        let value = self.text_val.get_mut(id)?;
        let cursor = self
            .text_cursor
            .entry(id.to_string())
            .or_insert(value.len());
        *cursor = clamp_boundary(value, *cursor);
        if *cursor > 0 {
            let prev = prev_boundary(value, *cursor);
            value.drain(prev..*cursor);
            *cursor = prev;
        }
        Some(value.clone())
    }

    pub fn delete_text(&mut self, id: &str) -> Option<String> {
        let value = self.text_val.get_mut(id)?;
        let cursor = self
            .text_cursor
            .entry(id.to_string())
            .or_insert(value.len());
        *cursor = clamp_boundary(value, *cursor);
        if *cursor < value.len() {
            let next = next_boundary(value, *cursor);
            value.drain(*cursor..next);
        }
        Some(value.clone())
    }

    pub fn move_text_cursor(&mut self, id: &str, direction: i32) {
        if let Some(value) = self.text_val.get(id) {
            let cursor = self
                .text_cursor
                .entry(id.to_string())
                .or_insert(value.len());
            *cursor = clamp_boundary(value, *cursor);
            *cursor = match direction.cmp(&0) {
                std::cmp::Ordering::Less => prev_boundary(value, *cursor),
                std::cmp::Ordering::Greater => next_boundary(value, *cursor),
                std::cmp::Ordering::Equal => *cursor,
            };
        }
    }

    pub fn move_text_cursor_home_end(&mut self, id: &str, end: bool) {
        if let Some(value) = self.text_val.get(id) {
            self.text_cursor
                .insert(id.to_string(), if end { value.len() } else { 0 });
        }
    }

    pub fn caret_t(&self, id: &str) -> f32 {
        let Some(value) = self.text_val.get(id) else {
            return 0.0;
        };
        let cursor = self.text_cursor.get(id).copied().unwrap_or(value.len());
        if value.is_empty() {
            return 0.0;
        }
        let cursor_chars = value[..clamp_boundary(value, cursor)].chars().count() as f32;
        (cursor_chars / value.chars().count() as f32).clamp(0.0, 1.0)
    }

    pub fn set_dropdown_open(&mut self, id: Option<String>) {
        self.open_dropdown = id.filter(|id| self.dropdown_items.contains_key(id));
    }

    pub fn toggle_dropdown(&mut self, id: &str) {
        if self.open_dropdown.as_deref() == Some(id) {
            self.open_dropdown = None;
        } else if self.dropdown_items.contains_key(id) {
            self.open_dropdown = Some(id.to_string());
        }
    }

    pub fn select_dropdown_index(&mut self, id: &str, idx: usize) -> Option<String> {
        let items = self.dropdown_items.get(id)?;
        let value = items.get(idx)?.clone();
        self.dropdown_index.insert(id.to_string(), idx);
        self.open_dropdown = None;
        Some(value)
    }

    pub fn move_dropdown_index(&mut self, id: &str, direction: i32) -> Option<String> {
        let items = self.dropdown_items.get(id)?;
        if items.is_empty() {
            return None;
        }
        let current = self.dropdown_index.get(id).copied().unwrap_or(0);
        let len = items.len();
        let next = if direction < 0 {
            current.saturating_sub(1)
        } else {
            (current + 1).min(len - 1)
        };
        let value = items.get(next)?.clone();
        self.dropdown_index.insert(id.to_string(), next);
        Some(value)
    }

    pub fn table(&self, id: &str) -> Option<&TableState> {
        self.tables.get(id)
    }

    pub fn active_tab(&self, id: &str) -> Option<&str> {
        self.active_tabs.get(id).map(String::as_str)
    }

    pub fn active_page(&self, id: &str) -> Option<&str> {
        self.active_pages.get(id).map(String::as_str)
    }

    pub fn is_active_tab(&self, id: &str) -> bool {
        let Some(parent) = self.tab_parent.get(id) else {
            return false;
        };
        let Some(value) = self.tab_values.get(id) else {
            return false;
        };
        self.active_tabs
            .get(parent)
            .is_some_and(|active| active == value)
    }

    pub fn is_active_nav_item(&self, id: &str) -> bool {
        let Some(target) = self.nav_targets.get(id) else {
            return false;
        };
        self.page_owner
            .get(target)
            .and_then(|pages_id| self.active_pages.get(pages_id))
            .is_some_and(|active| active == target)
    }

    pub fn activate_tab(&mut self, tab_id: &str) -> Option<(String, String)> {
        if self.is_disabled(tab_id) {
            return None;
        }
        let parent = self.tab_parent.get(tab_id)?.clone();
        let value = self.tab_values.get(tab_id)?.clone();
        self.active_tabs.insert(parent.clone(), value.clone());
        self.open_dropdown = None;
        Some((parent, value))
    }

    pub fn activate_nav_item(&mut self, nav_id: &str) -> Option<(String, String)> {
        if self.is_disabled(nav_id) {
            return None;
        }
        let value = self.nav_targets.get(nav_id)?.clone();
        let pages_id = self.page_owner.get(&value)?.clone();
        self.active_pages.insert(pages_id.clone(), value.clone());
        self.open_dropdown = None;
        Some((pages_id, value))
    }

    pub fn move_tab(&mut self, tab_id: &str, direction: i32) -> Option<(String, String, String)> {
        let parent = self.tab_parent.get(tab_id)?.clone();
        let current_value = self.tab_values.get(tab_id)?;
        let items = self.tabs.get(&parent)?;
        let current = items.iter().position(|item| &item.value == current_value)?;
        let next = enabled_neighbor(items, current, direction, &self.disabled)?;
        let next_id = items[next].id.clone();
        let value = items[next].value.clone();
        self.active_tabs.insert(parent.clone(), value.clone());
        self.focused = Some(next_id.clone());
        self.open_dropdown = None;
        Some((parent, value, next_id))
    }

    pub fn move_tab_edge(&mut self, tab_id: &str, end: bool) -> Option<(String, String, String)> {
        let parent = self.tab_parent.get(tab_id)?.clone();
        let items = self.tabs.get(&parent)?;
        let item = if end {
            items
                .iter()
                .rev()
                .find(|item| !item.disabled && !self.is_disabled(&item.id))?
        } else {
            items
                .iter()
                .find(|item| !item.disabled && !self.is_disabled(&item.id))?
        };
        let next_id = item.id.clone();
        let value = item.value.clone();
        self.active_tabs.insert(parent.clone(), value.clone());
        self.focused = Some(next_id.clone());
        self.open_dropdown = None;
        Some((parent, value, next_id))
    }

    pub fn move_nav_item(
        &mut self,
        nav_id: &str,
        direction: i32,
    ) -> Option<(String, String, String)> {
        let current_page = self.nav_targets.get(nav_id)?;
        let pages_id = self.page_owner.get(current_page)?.clone();
        let items = self.pages.get(&pages_id)?;
        let current = items.iter().position(|item| &item.value == current_page)?;
        let next = enabled_neighbor(items, current, direction, &self.disabled)?;
        let value = items[next].value.clone();
        let focus_id = self
            .nav_targets
            .iter()
            .find_map(|(id, page)| (page == &value && !self.is_disabled(id)).then(|| id.clone()))
            .unwrap_or_else(|| nav_id.to_string());
        self.active_pages.insert(pages_id.clone(), value.clone());
        self.focused = Some(focus_id.clone());
        self.open_dropdown = None;
        Some((pages_id, value, focus_id))
    }

    pub fn move_nav_item_edge(
        &mut self,
        nav_id: &str,
        end: bool,
    ) -> Option<(String, String, String)> {
        let current_page = self.nav_targets.get(nav_id)?;
        let pages_id = self.page_owner.get(current_page)?.clone();
        let items = self.pages.get(&pages_id)?;
        let item = if end {
            items
                .iter()
                .rev()
                .find(|item| !item.disabled && !self.is_disabled(&item.id))?
        } else {
            items
                .iter()
                .find(|item| !item.disabled && !self.is_disabled(&item.id))?
        };
        let value = item.value.clone();
        let focus_id = self
            .nav_targets
            .iter()
            .find_map(|(id, page)| (page == &value && !self.is_disabled(id)).then(|| id.clone()))
            .unwrap_or_else(|| nav_id.to_string());
        self.active_pages.insert(pages_id.clone(), value.clone());
        self.focused = Some(focus_id.clone());
        self.open_dropdown = None;
        Some((pages_id, value, focus_id))
    }

    pub fn scroll_table(&mut self, id: &str, row_delta: isize, col_delta: isize) -> bool {
        let Some(table) = self.tables.get_mut(id) else {
            return false;
        };
        let old = (table.scroll_row, table.scroll_col);
        let max_row = table.rows.saturating_sub(1);
        let max_col = table.columns.len().saturating_sub(1);
        table.scroll_row = apply_delta(table.scroll_row, row_delta, max_row);
        table.scroll_col = apply_delta(table.scroll_col, col_delta, max_col);
        old != (table.scroll_row, table.scroll_col)
    }

    pub fn select_table_cell(&mut self, id: &str, row: usize, col: usize) -> bool {
        let Some(table) = self.tables.get_mut(id) else {
            return false;
        };
        if row >= table.rows || col >= table.columns.len() {
            return false;
        }
        let old = table.selected;
        table.selected = Some((row, col));
        old != table.selected
    }

    pub fn toggle_table_sort(&mut self, id: &str, col: usize) -> bool {
        let Some(table) = self.tables.get_mut(id) else {
            return false;
        };
        if col >= table.columns.len() {
            return false;
        }
        table.sort = Some(match table.sort {
            Some((current_col, direction)) if current_col == col => (col, direction.toggle()),
            _ => (col, SortDirection::Asc),
        });
        true
    }

    pub fn move_table_selection(
        &mut self,
        id: &str,
        row_delta: isize,
        col_delta: isize,
        visible_rows: usize,
        visible_cols: usize,
    ) -> bool {
        let Some(table) = self.tables.get_mut(id) else {
            return false;
        };
        if table.rows == 0 || table.columns.is_empty() {
            return false;
        }
        let (row, col) = table
            .selected
            .unwrap_or((table.scroll_row, table.scroll_col));
        let next_row = apply_delta(row, row_delta, table.rows.saturating_sub(1));
        let next_col = apply_delta(col, col_delta, table.columns.len().saturating_sub(1));
        let old = (table.selected, table.scroll_row, table.scroll_col);
        table.selected = Some((next_row, next_col));
        table.scroll_row = keep_visible(table.scroll_row, next_row, visible_rows, table.rows);
        table.scroll_col = keep_visible(
            table.scroll_col,
            next_col,
            visible_cols,
            table.columns.len(),
        );
        old != (table.selected, table.scroll_row, table.scroll_col)
    }
}

fn collect_state(node: &WidgetNode, s: &mut WidgetState, parent: Option<&WidgetNode>) {
    match node.kind {
        WidgetKind::Checkbox => {
            s.checked
                .insert(node.id.clone(), node.props.checked.unwrap_or(false));
        }
        WidgetKind::Slider => {
            s.float_val
                .insert(node.id.clone(), node.props.value.unwrap_or(0.0));
            s.float_range.insert(
                node.id.clone(),
                (node.props.min.unwrap_or(0.0), node.props.max.unwrap_or(1.0)),
            );
            s.float_step
                .insert(node.id.clone(), node.props.step.unwrap_or(0.01));
        }
        WidgetKind::TextInput => {
            let value = node.props.text.clone().unwrap_or_default();
            s.text_cursor.insert(node.id.clone(), value.len());
            s.text_val.insert(node.id.clone(), value);
            if let Some(placeholder) = &node.props.placeholder {
                s.text_placeholder
                    .insert(node.id.clone(), placeholder.to_string());
            }
        }
        WidgetKind::Dropdown => {
            let selected = node.props.text.as_deref();
            let idx = selected
                .and_then(|v| node.props.items.iter().position(|item| item == v))
                .unwrap_or(0);
            s.dropdown_items
                .insert(node.id.clone(), node.props.items.clone());
            s.dropdown_index.insert(node.id.clone(), idx);
        }
        WidgetKind::DataFrameTable => {
            s.tables.insert(node.id.clone(), TableState::new(node));
        }
        WidgetKind::Tabs => {
            let items: Vec<NavigationItem> = node
                .children
                .iter()
                .filter(|child| child.kind == WidgetKind::Tab)
                .filter_map(|child| {
                    child
                        .props
                        .route_value
                        .as_ref()
                        .map(|value| NavigationItem {
                            id: child.id.clone(),
                            value: value.clone(),
                            disabled: child.props.disabled,
                        })
                })
                .collect();
            if let Some(active) = active_value(node.props.route_value.as_deref(), &items, s) {
                s.active_tabs.insert(node.id.clone(), active);
            }
            s.tabs.insert(node.id.clone(), items);
        }
        WidgetKind::Tab => {
            if let Some(parent) = parent.filter(|p| p.kind == WidgetKind::Tabs) {
                s.tab_parent.insert(node.id.clone(), parent.id.clone());
            }
            if let Some(value) = &node.props.route_value {
                s.tab_values.insert(node.id.clone(), value.clone());
            }
        }
        WidgetKind::Pages => {
            let items: Vec<NavigationItem> = node
                .children
                .iter()
                .filter(|child| child.kind == WidgetKind::Page)
                .filter_map(|child| {
                    child
                        .props
                        .route_value
                        .as_ref()
                        .map(|value| NavigationItem {
                            id: child.id.clone(),
                            value: value.clone(),
                            disabled: child.props.disabled,
                        })
                })
                .collect();
            if let Some(active) = active_value(node.props.route_value.as_deref(), &items, s) {
                s.active_pages.insert(node.id.clone(), active);
            }
            for item in &items {
                s.page_owner
                    .entry(item.value.clone())
                    .or_insert_with(|| node.id.clone());
            }
            s.pages.insert(node.id.clone(), items);
        }
        WidgetKind::Page => {
            if let Some(parent) = parent.filter(|p| p.kind == WidgetKind::Pages) {
                s.page_parent.insert(node.id.clone(), parent.id.clone());
            }
            if let Some(value) = &node.props.route_value {
                s.page_values.insert(node.id.clone(), value.clone());
            }
        }
        WidgetKind::NavItem => {
            if let Some(page) = &node.props.page {
                s.nav_targets.insert(node.id.clone(), page.clone());
            }
        }
        _ => {}
    }
    if node.props.disabled {
        s.disabled.insert(node.id.clone());
    }
    if is_interactive(&node.kind) && !node.props.disabled {
        s.focus_order.push(node.id.clone());
    }
    for child in &node.children {
        collect_state(child, s, Some(node));
    }
}

fn active_value(
    requested: Option<&str>,
    items: &[NavigationItem],
    state: &WidgetState,
) -> Option<String> {
    requested
        .and_then(|value| {
            items
                .iter()
                .any(|item| item.value == value && !item.disabled && !state.is_disabled(&item.id))
                .then(|| value.to_string())
        })
        .or_else(|| {
            items
                .iter()
                .find(|item| !item.disabled && !state.is_disabled(&item.id))
                .or_else(|| items.first())
                .map(|item| item.value.clone())
        })
}

fn enabled_neighbor(
    items: &[NavigationItem],
    current: usize,
    direction: i32,
    disabled: &HashSet<String>,
) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let len = items.len();
    for step in 1..=len {
        let idx = if direction < 0 {
            (current + len - step) % len
        } else if direction == 0 {
            current
        } else {
            (current + step) % len
        };
        if !items[idx].disabled && !disabled.contains(&items[idx].id) {
            return Some(idx);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Hit testing
// ---------------------------------------------------------------------------

/// Return the id and kind of the topmost interactive widget at physical pixel
/// position `pos`, or `None` if no interactive widget is there.
///
/// Children take priority over parents; the last child in document order is
/// considered topmost (highest paint order).
pub fn hit_test(
    tree: &WidgetNode,
    layout: &LayoutResult,
    pos: [f32; 2],
) -> Option<(String, WidgetKind)> {
    // Check children last-to-first (highest z-order first).
    for child in tree.children.iter().rev() {
        if let Some(h) = hit_test(child, layout, pos) {
            return Some(h);
        }
    }
    if is_interactive(&tree.kind) {
        if let Some(r) = layout.rects.get(&tree.id) {
            if rect_contains(r, pos) {
                return Some((tree.id.clone(), tree.kind.clone()));
            }
        }
    }
    None
}

fn is_interactive(kind: &WidgetKind) -> bool {
    matches!(
        kind,
        WidgetKind::Button
            | WidgetKind::Checkbox
            | WidgetKind::Dropdown
            | WidgetKind::Slider
            | WidgetKind::TextInput
            | WidgetKind::DataFrameTable
            | WidgetKind::Tab
            | WidgetKind::NavItem
    )
}

fn rect_contains(r: &Rect, pos: [f32; 2]) -> bool {
    pos[0] >= r.x && pos[0] < r.x + r.w && pos[1] >= r.y && pos[1] < r.y + r.h
}

fn apply_delta(value: usize, delta: isize, max_value: usize) -> usize {
    if delta < 0 {
        value.saturating_sub(delta.unsigned_abs())
    } else {
        value.saturating_add(delta as usize).min(max_value)
    }
}

fn keep_visible(scroll: usize, selected: usize, visible_count: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let visible_count = visible_count.max(1).min(total);
    let max_scroll = total.saturating_sub(visible_count);
    if selected < scroll {
        selected.min(max_scroll)
    } else if selected >= scroll.saturating_add(visible_count) {
        selected
            .saturating_add(1)
            .saturating_sub(visible_count)
            .min(max_scroll)
    } else {
        scroll.min(max_scroll)
    }
}

fn clamp_boundary(s: &str, idx: usize) -> usize {
    let mut clamped = idx.min(s.len());
    while clamped > 0 && !s.is_char_boundary(clamped) {
        clamped -= 1;
    }
    clamped
}

fn prev_boundary(s: &str, idx: usize) -> usize {
    let idx = clamp_boundary(s, idx);
    if idx == 0 {
        return 0;
    }
    s[..idx].char_indices().last().map(|(i, _)| i).unwrap_or(0)
}

fn next_boundary(s: &str, idx: usize) -> usize {
    let idx = clamp_boundary(s, idx);
    if idx >= s.len() {
        return s.len();
    }
    s[idx..]
        .char_indices()
        .nth(1)
        .map(|(i, _)| idx + i)
        .unwrap_or(s.len())
}

// ---------------------------------------------------------------------------
// SliderDrag — drag state captured when a pointer-down hits a Slider
// ---------------------------------------------------------------------------

/// Slider drag geometry captured when a pointer-down hits a slider.
pub struct SliderDrag {
    pub widget_id: String,
    /// Physical-pixel x where the usable track starts.
    track_x: f32,
    /// Physical-pixel width of the usable track area.
    track_w: f32,
    min: f32,
    max: f32,
}

impl SliderDrag {
    pub fn new(widget_id: String, rect: &Rect, min: f32, max: f32, sf: f32) -> Self {
        let margin = SLIDER_TRACK_MARGIN_LP * sf;
        Self {
            widget_id,
            track_x: rect.x + margin,
            track_w: (rect.w - 2.0 * margin).max(1.0),
            min,
            max,
        }
    }

    /// Map a cursor x position to a value within `[min, max]`.
    pub fn compute_value(&self, mouse_x: f32) -> f32 {
        let t = ((mouse_x - self.track_x) / self.track_w).clamp(0.0, 1.0);
        self.min + t * (self.max - self.min)
    }
}
