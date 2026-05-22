use std::collections::{HashMap, HashSet};

use crate::document::{WidgetKind, WidgetNode};
use crate::layout::{is_scroll_container_node, LayoutResult, Rect};
use crate::style::{VisualStyle, SLIDER_TRACK_MARGIN_LP};

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
    pub row_order: Option<Vec<usize>>,
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
            row_order: None,
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
    /// TextArea vertical scroll offset in physical pixels keyed by widget id.
    pub text_scroll_y: HashMap<String, f32>,
    /// Scrollable container vertical offset in physical pixels keyed by widget id.
    pub container_scroll_y: HashMap<String, f32>,
    /// Scrollable container horizontal offset in physical pixels keyed by widget id.
    pub container_scroll_x: HashMap<String, f32>,
    /// NumberInput widgets whose edited text cannot currently be parsed.
    pub invalid_numbers: HashSet<String>,
    /// Dropdown items keyed by widget id.
    pub dropdown_items: HashMap<String, Vec<String>>,
    /// Dropdown selected item index keyed by widget id.
    pub dropdown_index: HashMap<String, usize>,
    /// Disabled widgets keyed by widget id.
    pub disabled: HashSet<String>,
    /// Collapsible expanded state keyed by widget id.
    pub expanded: HashMap<String, bool>,
    /// Keyboard focus traversal order.
    pub focus_order: Vec<String>,
    /// Keyboard-focused widget id.
    pub focused: Option<String>,
    /// Focus animation progress keyed by widget id, where 0.0 is unfocused and 1.0 is focused.
    pub focus_t: HashMap<String, f32>,
    /// Widget currently under the cursor (interactive widgets only).
    pub hovered: Option<String>,
    /// Hover animation progress keyed by widget id, where 0.0 is base and 1.0 is hover.
    pub hover_t: HashMap<String, f32>,
    /// Checked animation progress keyed by widget id, where 0.0 is unchecked and 1.0 is checked.
    pub checked_t: HashMap<String, f32>,
    /// Active animation progress keyed by widget id, where 0.0 is base and 1.0 is active.
    pub active_t: HashMap<String, f32>,
    /// Open animation progress keyed by widget id, where 0.0 is closed and 1.0 is open.
    pub open_t: HashMap<String, f32>,
    /// Selected animation progress keyed by widget id, where 0.0 is unselected and 1.0 is selected.
    pub selected_t: HashMap<String, f32>,
    /// Expansion animation progress keyed by widget id, where 0.0 is collapsed and 1.0 is expanded.
    pub expanded_t: HashMap<String, f32>,
    /// Continuous CSS animation visual overrides keyed by widget id.
    pub animation_visuals: HashMap<String, VisualStyle>,
    /// Widget that received a pointer-down (not yet released).
    pub pressed: Option<String>,
    /// Dropdown whose menu is currently open.
    pub open_dropdown: Option<String>,
    /// Open dropdown item currently under the cursor, as (dropdown id, item index).
    pub dropdown_hover: Option<(String, usize)>,
    /// Menu items keyed by Menu or ContextMenu widget id.
    pub menu_items: HashMap<String, Vec<NavigationItem>>,
    /// Top-level menu whose popup is currently open.
    pub open_menu: Option<String>,
    /// Context menu target widget id to ContextMenu widget id.
    pub context_targets: HashMap<String, String>,
    /// Context menu whose popup is currently open.
    pub open_context_menu: Option<String>,
    /// Physical pixel position for the current context menu popup.
    pub context_menu_pos: Option<[f32; 2]>,
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
    pub fn set_number_value(&mut self, id: &str, v: f32) -> Option<f32> {
        if !self.float_val.contains_key(id) && !self.float_range.contains_key(id) {
            return None;
        }
        let value = self.set_float(id, v);
        let text = format_number(value);
        self.text_cursor.insert(id.to_string(), text.len());
        self.text_val.insert(id.to_string(), text);
        self.invalid_numbers.remove(id);
        Some(value)
    }

    pub fn adjust_number(&mut self, id: &str, direction: f32) -> Option<f32> {
        let current = self.float_val.get(id).copied()?;
        let step = self.float_step.get(id).copied().unwrap_or(1.0).abs();
        self.set_number_value(id, current + step * direction)
    }

    pub fn validate_number_text(&mut self, id: &str) -> Option<Option<f32>> {
        let text = self.text_val.get(id)?.trim();
        match text.parse::<f32>() {
            Ok(value) if value.is_finite() => {
                let value = self.set_float(id, value);
                self.invalid_numbers.remove(id);
                Some(Some(value))
            }
            _ => {
                self.invalid_numbers.insert(id.to_string());
                Some(None)
            }
        }
    }

    pub fn number_is_invalid(&self, id: &str) -> bool {
        self.invalid_numbers.contains(id)
    }

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

    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.get(id).copied().unwrap_or(true)
    }

    pub fn is_expanded_widget(&self, id: &str) -> bool {
        self.expanded.get(id).copied() == Some(true)
    }

    pub fn is_collapsed_widget(&self, id: &str) -> bool {
        self.expanded.get(id).copied() == Some(false)
    }

    pub fn is_open_widget(&self, id: &str) -> bool {
        self.open_dropdown.as_deref() == Some(id)
            || self.open_menu.as_deref() == Some(id)
            || self.open_context_menu.as_deref() == Some(id)
    }

    pub fn set_expanded(&mut self, id: &str, expanded: bool) -> Option<bool> {
        if !self.expanded.contains_key(id) {
            return None;
        }
        self.expanded.insert(id.to_string(), expanded);
        self.close_popups();
        Some(expanded)
    }

    pub fn toggle_expanded(&mut self, id: &str) -> Option<bool> {
        if self.is_disabled(id) {
            return None;
        }
        let current = self.expanded.get(id).copied()?;
        self.set_expanded(id, !current)
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
            self.dropdown_hover = None;
        }
        if !self
            .focused
            .as_deref()
            .is_some_and(|id| self.open_menu.as_deref() == Some(id))
        {
            self.open_menu = None;
        }
        self.open_context_menu = None;
        self.context_menu_pos = None;
    }

    pub fn focus_next_visible(&mut self, layout: &LayoutResult, reverse: bool) {
        if self.focus_order.is_empty() {
            self.focused = None;
            return;
        }
        let visible: Vec<String> = self
            .focus_order
            .iter()
            .filter(|id| {
                layout
                    .rects
                    .get(*id)
                    .is_some_and(|r| r.w > 0.0 && r.h > 0.0)
                    && !self.is_disabled(id)
            })
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
            self.close_popups();
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
        self.text_scroll_y.insert(id.to_string(), 0.0);
        self.close_popups();
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
        self.close_popups();
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

    pub fn move_text_cursor_vertical(&mut self, id: &str, direction: i32) {
        if direction == 0 {
            return;
        }
        let Some(value) = self.text_val.get(id) else {
            return;
        };
        let current = self.text_cursor.get(id).copied().unwrap_or(value.len());
        let current = clamp_boundary(value, current);
        let (line, column) = line_column_for_cursor(value, current);
        let target_line = if direction < 0 {
            line.saturating_sub(1)
        } else {
            (line + 1).min(line_count(value).saturating_sub(1))
        };
        let target = cursor_for_line_column(value, target_line, column);
        self.text_cursor.insert(id.to_string(), target);
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

    pub fn text_area_scroll_y(&self, id: &str, visible_h: f32, line_h: f32) -> f32 {
        let Some(value) = self.text_val.get(id) else {
            return 0.0;
        };
        let max_scroll = text_area_max_scroll(value, visible_h, line_h);
        self.text_scroll_y
            .get(id)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, max_scroll)
    }

    pub fn scroll_text_area(
        &mut self,
        id: &str,
        delta_y: f32,
        visible_h: f32,
        line_h: f32,
    ) -> bool {
        let Some(value) = self.text_val.get(id) else {
            return false;
        };
        let max_scroll = text_area_max_scroll(value, visible_h, line_h);
        let current = self.text_scroll_y.get(id).copied().unwrap_or(0.0);
        let next = (current + delta_y).clamp(0.0, max_scroll);
        if (next - current).abs() <= f32::EPSILON {
            self.text_scroll_y.insert(id.to_string(), next);
            return false;
        }
        self.text_scroll_y.insert(id.to_string(), next);
        true
    }

    pub fn container_scroll_y(&self, id: &str, max_scroll: f32) -> f32 {
        self.container_scroll_y
            .get(id)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, max_scroll.max(0.0))
    }

    pub fn container_scroll_x(&self, id: &str, max_scroll: f32) -> f32 {
        self.container_scroll_x
            .get(id)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, max_scroll.max(0.0))
    }

    pub fn scroll_container(
        &mut self,
        id: &str,
        delta_x: f32,
        delta_y: f32,
        max_scroll_x: f32,
        max_scroll_y: f32,
    ) -> bool {
        let next_x = scroll_axis_next(&self.container_scroll_x, id, delta_x, max_scroll_x);
        let next_y = scroll_axis_next(&self.container_scroll_y, id, delta_y, max_scroll_y);
        let current_x = self.container_scroll_x(id, max_scroll_x);
        let current_y = self.container_scroll_y(id, max_scroll_y);
        let changed =
            (next_x - current_x).abs() > f32::EPSILON || (next_y - current_y).abs() > f32::EPSILON;
        self.container_scroll_x.insert(id.to_string(), next_x);
        self.container_scroll_y.insert(id.to_string(), next_y);
        if changed {
            self.close_popups();
        }
        changed
    }

    pub fn ensure_text_area_cursor_visible(
        &mut self,
        id: &str,
        visible_h: f32,
        line_h: f32,
    ) -> bool {
        let Some(value) = self.text_val.get(id) else {
            return false;
        };
        if visible_h <= 0.0 || line_h <= 0.0 {
            self.text_scroll_y.insert(id.to_string(), 0.0);
            return false;
        }
        let cursor = self.text_cursor.get(id).copied().unwrap_or(value.len());
        let cursor = clamp_boundary(value, cursor);
        let (line, _) = line_column_for_cursor(value, cursor);
        let max_scroll = text_area_max_scroll(value, visible_h, line_h);
        let current = self
            .text_scroll_y
            .get(id)
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, max_scroll);
        let caret_top = line as f32 * line_h;
        let caret_bottom = caret_top + line_h;
        let mut next = current;
        if caret_top < next {
            next = caret_top;
        } else if caret_bottom > next + visible_h {
            next = caret_bottom - visible_h;
        }
        next = next.clamp(0.0, max_scroll);
        self.text_scroll_y.insert(id.to_string(), next);
        (next - current).abs() > f32::EPSILON
    }

    pub fn set_dropdown_open(&mut self, id: Option<String>) {
        self.dropdown_hover = None;
        self.open_dropdown = id.filter(|id| self.dropdown_items.contains_key(id));
        if self.open_dropdown.is_some() {
            self.open_menu = None;
            self.open_context_menu = None;
            self.context_menu_pos = None;
        }
    }

    pub fn toggle_dropdown(&mut self, id: &str) {
        self.dropdown_hover = None;
        if self.open_dropdown.as_deref() == Some(id) {
            self.open_dropdown = None;
        } else if self.dropdown_items.contains_key(id) {
            self.open_dropdown = Some(id.to_string());
            self.open_menu = None;
            self.open_context_menu = None;
            self.context_menu_pos = None;
        }
    }

    pub fn select_dropdown_index(&mut self, id: &str, idx: usize) -> Option<String> {
        let items = self.dropdown_items.get(id)?;
        let value = items.get(idx)?.clone();
        self.dropdown_index.insert(id.to_string(), idx);
        self.close_popups();
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

    pub fn close_popups(&mut self) {
        self.open_dropdown = None;
        self.dropdown_hover = None;
        self.open_menu = None;
        self.open_context_menu = None;
        self.context_menu_pos = None;
    }

    pub fn toggle_menu(&mut self, id: &str) {
        if self.open_menu.as_deref() == Some(id) {
            self.open_menu = None;
        } else if self.menu_items.contains_key(id) {
            self.open_menu = Some(id.to_string());
        }
        self.open_dropdown = None;
        self.dropdown_hover = None;
        self.open_context_menu = None;
        self.context_menu_pos = None;
    }

    pub fn open_context_menu(&mut self, id: &str, pos: [f32; 2]) -> bool {
        if !self.menu_items.contains_key(id) {
            return false;
        }
        self.open_dropdown = None;
        self.dropdown_hover = None;
        self.open_menu = None;
        self.open_context_menu = Some(id.to_string());
        self.context_menu_pos = Some(pos);
        true
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

    pub fn is_active_page_child(&self, id: &str) -> bool {
        let Some(parent) = self.page_parent.get(id) else {
            return false;
        };
        let Some(value) = self.page_values.get(id) else {
            return false;
        };
        self.active_pages
            .get(parent)
            .is_some_and(|active| active == value)
    }

    pub fn is_selected_widget(&self, id: &str) -> bool {
        self.is_active_tab(id)
            || self.is_active_nav_item(id)
            || self.is_active_page_child(id)
            || self
                .tables
                .get(id)
                .is_some_and(|table| table.selected.is_some())
    }

    pub fn activate_tab(&mut self, tab_id: &str) -> Option<(String, String)> {
        if self.is_disabled(tab_id) {
            return None;
        }
        let parent = self.tab_parent.get(tab_id)?.clone();
        let value = self.tab_values.get(tab_id)?.clone();
        self.active_tabs.insert(parent.clone(), value.clone());
        self.close_popups();
        Some((parent, value))
    }

    pub fn set_active_tab_value(&mut self, tabs_id: &str, value: &str) -> Option<()> {
        let valid =
            self.tabs.get(tabs_id)?.iter().any(|item| {
                item.value == value && !item.disabled && !self.disabled.contains(&item.id)
            });
        if !valid {
            return None;
        }
        self.active_tabs
            .insert(tabs_id.to_string(), value.to_string());
        self.close_popups();
        Some(())
    }

    pub fn activate_nav_item(&mut self, nav_id: &str) -> Option<(String, String)> {
        if self.is_disabled(nav_id) {
            return None;
        }
        let value = self.nav_targets.get(nav_id)?.clone();
        let pages_id = self.page_owner.get(&value)?.clone();
        self.active_pages.insert(pages_id.clone(), value.clone());
        self.close_popups();
        Some((pages_id, value))
    }

    pub fn set_active_page_value(&mut self, pages_id: &str, value: &str) -> Option<()> {
        let valid =
            self.pages.get(pages_id)?.iter().any(|item| {
                item.value == value && !item.disabled && !self.disabled.contains(&item.id)
            });
        if !valid {
            return None;
        }
        self.active_pages
            .insert(pages_id.to_string(), value.to_string());
        self.close_popups();
        Some(())
    }

    pub fn preserve_rebuild_state_from(&mut self, previous: &WidgetState) {
        for (id, value) in &previous.checked {
            if self.checked.contains_key(id) {
                self.checked.insert(id.clone(), *value);
            }
        }

        for (id, value) in &previous.float_val {
            if self.float_val.contains_key(id) {
                let (min, max) = self
                    .float_range
                    .get(id)
                    .copied()
                    .unwrap_or((f32::NEG_INFINITY, f32::INFINITY));
                self.float_val
                    .insert(id.clone(), value.clamp(min.min(max), min.max(max)));
            }
        }

        for (id, value) in &previous.text_val {
            if self.text_val.contains_key(id) {
                self.text_val.insert(id.clone(), value.clone());
                let cursor = previous.text_cursor.get(id).copied().unwrap_or(value.len());
                self.text_cursor
                    .insert(id.clone(), clamp_boundary(value, cursor));
            }
        }
        self.invalid_numbers = previous
            .invalid_numbers
            .iter()
            .filter(|id| self.text_val.contains_key(*id))
            .cloned()
            .collect();

        for (id, scroll) in &previous.text_scroll_y {
            if self.text_scroll_y.contains_key(id) {
                self.text_scroll_y.insert(id.clone(), scroll.max(0.0));
            }
        }
        for (id, scroll) in &previous.container_scroll_x {
            if self.container_scroll_x.contains_key(id) {
                self.container_scroll_x.insert(id.clone(), scroll.max(0.0));
            }
        }
        for (id, scroll) in &previous.container_scroll_y {
            if self.container_scroll_y.contains_key(id) {
                self.container_scroll_y.insert(id.clone(), scroll.max(0.0));
            }
        }

        for (id, index) in &previous.dropdown_index {
            if let Some(items) = self.dropdown_items.get(id) {
                if !items.is_empty() {
                    self.dropdown_index
                        .insert(id.clone(), (*index).min(items.len().saturating_sub(1)));
                }
            }
        }

        for (id, expanded) in &previous.expanded {
            if self.expanded.contains_key(id) {
                self.expanded.insert(id.clone(), *expanded);
            }
        }

        for (id, previous_table) in &previous.tables {
            let Some(table) = self.tables.get_mut(id) else {
                continue;
            };
            let max_row = table.rows.saturating_sub(1);
            let max_col = table.columns.len().saturating_sub(1);
            table.scroll_row = previous_table.scroll_row.min(max_row);
            table.scroll_col = previous_table.scroll_col.min(max_col);
            table.selected = previous_table.selected.and_then(|(row, col)| {
                (row < table.rows && col < table.columns.len()).then_some((row, col))
            });
            table.sort = previous_table
                .sort
                .filter(|(column, _)| *column < table.columns.len());
            table.row_order = previous_table
                .row_order
                .clone()
                .filter(|rows| rows.len() == table.rows);
        }

        let active_tabs: Vec<(String, String)> = previous
            .active_tabs
            .iter()
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect();
        let active_pages: Vec<(String, String)> = previous
            .active_pages
            .iter()
            .map(|(id, value)| (id.clone(), value.clone()))
            .collect();
        for (id, value) in active_tabs {
            let _ = self.set_active_tab_value(&id, &value);
        }
        for (id, value) in active_pages {
            let _ = self.set_active_page_value(&id, &value);
        }

        if let Some(focused) = previous.focused.as_ref() {
            if self.focus_order.iter().any(|id| id == focused) && !self.is_disabled(focused) {
                self.focused = Some(focused.clone());
            }
        }
        self.pressed = previous
            .pressed
            .as_ref()
            .filter(|id| self.focus_order.iter().any(|focus_id| focus_id == *id))
            .cloned();
        self.open_dropdown = previous
            .open_dropdown
            .as_ref()
            .filter(|id| self.dropdown_items.contains_key(*id) && !self.is_disabled(id))
            .cloned();
        self.dropdown_hover = previous
            .dropdown_hover
            .as_ref()
            .filter(|(id, index)| {
                self.open_dropdown.as_deref() == Some(id.as_str())
                    && self
                        .dropdown_items
                        .get(id)
                        .is_some_and(|items| *index < items.len())
            })
            .cloned();
        self.open_menu = previous
            .open_menu
            .as_ref()
            .filter(|id| self.menu_items.contains_key(*id) && !self.is_disabled(id))
            .cloned();
        self.open_context_menu = previous
            .open_context_menu
            .as_ref()
            .filter(|id| self.menu_items.contains_key(*id))
            .cloned();
        self.context_menu_pos = self
            .open_context_menu
            .as_ref()
            .and(previous.context_menu_pos);
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
        self.close_popups();
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
        self.close_popups();
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
        self.close_popups();
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
        self.close_popups();
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
        table.scroll_row = 0;
        table.selected = None;
        true
    }

    pub fn set_table_row_order(&mut self, id: &str, row_order: Option<Vec<usize>>) -> bool {
        let Some(table) = self.tables.get_mut(id) else {
            return false;
        };
        table.row_order = row_order.filter(|rows| rows.len() == table.rows);
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

    pub fn move_table_selection_to_col_edge(&mut self, id: &str, end: bool) -> bool {
        let Some(table) = self.tables.get_mut(id) else {
            return false;
        };
        if table.rows == 0 || table.columns.is_empty() {
            return false;
        }
        let (row, _) = table
            .selected
            .unwrap_or((table.scroll_row, table.scroll_col));
        let col = if end {
            table.columns.len().saturating_sub(1)
        } else {
            0
        };
        let old = (table.selected, table.scroll_col);
        table.selected = Some((row.min(table.rows.saturating_sub(1)), col));
        table.scroll_col = keep_visible(table.scroll_col, col, 1, table.columns.len());
        old != (table.selected, table.scroll_col)
    }

    pub fn current_table_selection(&self, id: &str) -> Option<(usize, usize)> {
        let table = self.tables.get(id)?;
        if table.rows == 0 || table.columns.is_empty() {
            return None;
        }
        let (row, col) = table
            .selected
            .unwrap_or((table.scroll_row, table.scroll_col));
        Some((
            row.min(table.rows.saturating_sub(1)),
            col.min(table.columns.len().saturating_sub(1)),
        ))
    }
}

fn collect_state(node: &WidgetNode, s: &mut WidgetState, parent: Option<&WidgetNode>) {
    if matches!(node.kind, WidgetKind::Tooltip | WidgetKind::Toast) {
        return;
    }
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
        WidgetKind::NumberInput => {
            let value = node.props.value.unwrap_or(0.0);
            s.float_val.insert(node.id.clone(), value);
            s.float_range.insert(
                node.id.clone(),
                (
                    node.props.min.unwrap_or(f32::NEG_INFINITY),
                    node.props.max.unwrap_or(f32::INFINITY),
                ),
            );
            s.float_step
                .insert(node.id.clone(), node.props.step.unwrap_or(1.0));
            let text = node
                .props
                .text
                .clone()
                .unwrap_or_else(|| format_number(value));
            s.text_cursor.insert(node.id.clone(), text.len());
            s.text_val.insert(node.id.clone(), text);
        }
        WidgetKind::ProgressBar => {
            s.float_val
                .insert(node.id.clone(), node.props.value.unwrap_or(0.0));
            let min = node.props.min.unwrap_or(0.0);
            let max = node.props.max.unwrap_or(1.0);
            if min != 0.0 || max != 1.0 {
                s.float_range.insert(node.id.clone(), (min, max));
            }
        }
        WidgetKind::TextInput | WidgetKind::TextArea => {
            let value = node.props.text.clone().unwrap_or_default();
            s.text_cursor.insert(node.id.clone(), value.len());
            s.text_val.insert(node.id.clone(), value);
            if node.kind == WidgetKind::TextArea {
                s.text_scroll_y.insert(node.id.clone(), 0.0);
            }
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
        WidgetKind::Collapsible => {
            s.expanded
                .insert(node.id.clone(), node.props.expanded.unwrap_or(true));
        }
        WidgetKind::Menu | WidgetKind::ContextMenu => {
            let items: Vec<NavigationItem> = node
                .children
                .iter()
                .filter(|child| child.kind == WidgetKind::MenuItem)
                .filter_map(|child| {
                    child.props.text.as_ref().map(|value| NavigationItem {
                        id: child.id.clone(),
                        value: value.clone(),
                        disabled: child.props.disabled,
                    })
                })
                .collect();
            if !items.is_empty() {
                s.menu_items.insert(node.id.clone(), items);
            }
            if node.kind == WidgetKind::ContextMenu {
                if let Some(target) = &node.props.target {
                    s.context_targets.insert(target.clone(), node.id.clone());
                }
            }
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
    if is_scroll_container_node(node) {
        s.container_scroll_x.entry(node.id.clone()).or_insert(0.0);
        s.container_scroll_y.entry(node.id.clone()).or_insert(0.0);
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
    if let Some(modal) = active_modal(tree) {
        let r = layout.rects.get(&modal.id)?;
        if !rect_contains(r, pos) {
            return None;
        }
        return hit_test_with(modal, layout, pos, |node| is_interactive(&node.kind));
    }
    hit_test_with(tree, layout, pos, |node| is_interactive(&node.kind))
}

pub fn hit_test_hover(
    tree: &WidgetNode,
    layout: &LayoutResult,
    pos: [f32; 2],
) -> Option<(String, WidgetKind)> {
    if let Some(modal) = active_modal(tree) {
        let r = layout.rects.get(&modal.id)?;
        if !rect_contains(r, pos) {
            return None;
        }
        return hit_test_with(modal, layout, pos, |node| {
            is_interactive(&node.kind)
                || node.props.tooltip.is_some()
                || has_rich_tooltip_target(tree, &node.id)
                || has_hover_visual(&node.style.hover)
        });
    }
    hit_test_with(tree, layout, pos, |node| {
        is_interactive(&node.kind)
            || node.props.tooltip.is_some()
            || has_rich_tooltip_target(tree, &node.id)
            || has_hover_visual(&node.style.hover)
    })
}

pub fn modal_blocks_point(tree: &WidgetNode, layout: &LayoutResult, pos: [f32; 2]) -> bool {
    let Some(modal) = active_modal(tree) else {
        return false;
    };
    match layout.rects.get(&modal.id) {
        Some(r) => !rect_contains(r, pos),
        None => true,
    }
}

pub fn has_active_modal(tree: &WidgetNode) -> bool {
    active_modal(tree).is_some()
}

fn active_modal(node: &WidgetNode) -> Option<&WidgetNode> {
    for child in node.children.iter().rev() {
        if let Some(modal) = active_modal(child) {
            return Some(modal);
        }
    }
    (node.kind == WidgetKind::Modal && node.props.open.unwrap_or(false)).then_some(node)
}

fn has_rich_tooltip_target(node: &WidgetNode, target: &str) -> bool {
    (node.kind == WidgetKind::Tooltip && node.props.target.as_deref() == Some(target))
        || node
            .children
            .iter()
            .any(|child| has_rich_tooltip_target(child, target))
}

fn has_hover_visual(visual: &VisualStyle) -> bool {
    visual.background.is_some()
        || visual.background_paint.is_some()
        || visual.gradient_interpolation.is_some()
        || visual.backdrop_filter.is_some()
        || visual.foreground.is_some()
        || visual.border_color.is_some()
        || visual.border_width.is_some()
        || visual.outline_color.is_some()
        || visual.outline_width.is_some()
        || visual.outline_offset.is_some()
        || visual.border_radius.is_some()
        || !visual.corner_radii.is_empty()
        || visual.accent.is_some()
        || visual.track_color.is_some()
        || visual.thumb_color.is_some()
        || visual.opacity.is_some()
        || visual.background_noise.is_some()
        || visual.box_shadows.is_some()
        || visual.transform.is_some()
}

fn hit_test_with<F>(
    tree: &WidgetNode,
    layout: &LayoutResult,
    pos: [f32; 2],
    accepts_node: F,
) -> Option<(String, WidgetKind)>
where
    F: Fn(&WidgetNode) -> bool + Copy,
{
    if tree.kind == WidgetKind::Tooltip {
        return None;
    }
    // Check children last-to-first (highest z-order first).
    for child in tree.children.iter().rev() {
        if let Some(h) = hit_test_with(child, layout, pos, accepts_node) {
            return Some(h);
        }
    }
    if accepts_node(tree) {
        if let Some(r) = layout.visible_rect(&tree.id) {
            if rect_contains(&r, pos) {
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
            | WidgetKind::Menu
            | WidgetKind::Slider
            | WidgetKind::NumberInput
            | WidgetKind::TextInput
            | WidgetKind::TextArea
            | WidgetKind::DataFrameTable
            | WidgetKind::Histogram
            | WidgetKind::LinePlot
            | WidgetKind::Collapsible
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

fn line_count(text: &str) -> usize {
    text.chars().filter(|ch| *ch == '\n').count() + 1
}

fn text_area_max_scroll(text: &str, visible_h: f32, line_h: f32) -> f32 {
    let content_h = line_count(text) as f32 * line_h.max(1.0);
    (content_h - visible_h.max(1.0)).max(0.0)
}

fn scroll_axis_next(scrolls: &HashMap<String, f32>, id: &str, delta: f32, max_scroll: f32) -> f32 {
    let max_scroll = max_scroll.max(0.0);
    let current = scrolls
        .get(id)
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, max_scroll);
    (current + delta).clamp(0.0, max_scroll)
}

fn line_column_for_cursor(text: &str, cursor: usize) -> (usize, usize) {
    let cursor = clamp_boundary(text, cursor);
    let mut line = 0;
    let mut col = 0;
    for (idx, ch) in text.char_indices() {
        if idx >= cursor {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn cursor_for_line_column(text: &str, target_line: usize, target_col: usize) -> usize {
    let mut line = 0;
    let mut col = 0;
    for (idx, ch) in text.char_indices() {
        if line == target_line {
            if ch == '\n' || col >= target_col {
                return idx;
            }
            col += 1;
            continue;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    text.len()
}

fn format_number(value: f32) -> String {
    if value.fract().abs() < f32::EPSILON && value.abs() < 1.0e7 {
        format!("{value:.0}")
    } else {
        let mut text = format!("{value:.6}");
        while text.contains('.') && text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        if text == "-0" {
            "0".to_string()
        } else {
            text
        }
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
            inline_style: Default::default(),
            style: Default::default(),
            children,
        }
    }

    #[test]
    fn hover_hit_test_accepts_noninteractive_rich_tooltip_targets() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![
                node(
                    "progress",
                    WidgetKind::ProgressBar,
                    NodeProps::default(),
                    vec![],
                ),
                node(
                    "tip",
                    WidgetKind::Tooltip,
                    NodeProps {
                        target: Some("progress".to_string()),
                        ..NodeProps::default()
                    },
                    vec![node(
                        "label",
                        WidgetKind::Label,
                        NodeProps::default(),
                        vec![],
                    )],
                ),
            ],
        );
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 200.0,
            },
        );
        layout.rects.insert(
            "progress".to_string(),
            Rect {
                x: 20.0,
                y: 20.0,
                w: 160.0,
                h: 34.0,
            },
        );
        layout.rects.insert("tip".to_string(), Rect::default());

        let hit = hit_test_hover(&root, &layout, [30.0, 30.0]);

        assert_eq!(hit, Some(("progress".to_string(), WidgetKind::ProgressBar)));
    }

    #[test]
    fn hover_hit_test_accepts_noninteractive_widgets_with_hover_style() {
        let mut panel = node("card", WidgetKind::Panel, NodeProps::default(), vec![]);
        panel.style.hover.background = Some(crate::style::ColorRef::Rgba([0.1, 0.2, 0.3, 1.0]));
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![panel],
        );
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 400.0,
                h: 200.0,
            },
        );
        layout.rects.insert(
            "card".to_string(),
            Rect {
                x: 20.0,
                y: 20.0,
                w: 160.0,
                h: 80.0,
            },
        );

        let hit = hit_test_hover(&root, &layout, [30.0, 30.0]);

        assert_eq!(hit, Some(("card".to_string(), WidgetKind::Panel)));
    }

    #[test]
    fn hit_test_ignores_clipped_widget_area() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "button",
                WidgetKind::Button,
                NodeProps::default(),
                vec![],
            )],
        );
        let mut layout = LayoutResult::default();
        layout.rects.insert(
            "window".to_string(),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 80.0,
            },
        );
        layout.rects.insert(
            "button".to_string(),
            Rect {
                x: 0.0,
                y: 60.0,
                w: 120.0,
                h: 40.0,
            },
        );
        layout.clips.insert(
            "button".to_string(),
            Rect {
                x: 0.0,
                y: 60.0,
                w: 120.0,
                h: 20.0,
            },
        );

        assert_eq!(
            hit_test(&root, &layout, [20.0, 70.0]),
            Some(("button".to_string(), WidgetKind::Button))
        );
        assert_eq!(hit_test(&root, &layout, [20.0, 90.0]), None);
    }

    #[test]
    fn text_area_cursor_visibility_updates_scroll_offset() {
        let mut state = WidgetState::default();
        state
            .text_val
            .insert("notes".to_string(), "one\ntwo\nthree\nfour".to_string());
        state
            .text_cursor
            .insert("notes".to_string(), "one\ntwo\nthree\nfour".len());
        state.text_scroll_y.insert("notes".to_string(), 0.0);

        assert!(state.ensure_text_area_cursor_visible("notes", 20.0, 10.0));
        assert_eq!(state.text_area_scroll_y("notes", 20.0, 10.0), 20.0);
    }

    #[test]
    fn text_area_scroll_clamps_to_content_height() {
        let mut state = WidgetState::default();
        state
            .text_val
            .insert("notes".to_string(), "one\ntwo\nthree".to_string());

        assert!(state.scroll_text_area("notes", 999.0, 20.0, 10.0));
        assert_eq!(state.text_area_scroll_y("notes", 20.0, 10.0), 10.0);
        assert!(state.scroll_text_area("notes", -999.0, 20.0, 10.0));
        assert_eq!(state.text_area_scroll_y("notes", 20.0, 10.0), 0.0);
    }

    #[test]
    fn container_scroll_clamps_both_axes() {
        let mut state = WidgetState::default();

        assert!(state.scroll_container("panel", 25.0, 40.0, 20.0, 30.0));
        assert_eq!(state.container_scroll_x("panel", 20.0), 20.0);
        assert_eq!(state.container_scroll_y("panel", 30.0), 30.0);

        assert!(state.scroll_container("panel", -999.0, -999.0, 20.0, 30.0));
        assert_eq!(state.container_scroll_x("panel", 20.0), 0.0);
        assert_eq!(state.container_scroll_y("panel", 30.0), 0.0);
    }

    #[test]
    fn text_area_vertical_cursor_movement_preserves_column() {
        let mut state = WidgetState::default();
        let text = "alpha\nbeta\ngamma";
        state.text_val.insert("notes".to_string(), text.to_string());
        state
            .text_cursor
            .insert("notes".to_string(), "alpha\nbe".len());

        state.move_text_cursor_vertical("notes", 1);

        let cursor = state.text_cursor["notes"];
        assert_eq!(line_column_for_cursor(text, cursor), (2, 2));
    }

    #[test]
    fn table_selection_navigation_keeps_cell_visible() {
        let mut state = WidgetState::default();
        state.tables.insert(
            "table".to_string(),
            TableState {
                columns: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                dtypes: vec![],
                rows: 20,
                resource_id: None,
                page_size: 10,
                scroll_row: 0,
                scroll_col: 0,
                selected: Some((0, 0)),
                sort: None,
                row_order: None,
            },
        );

        assert!(state.move_table_selection("table", 6, 2, 4, 2));
        let table = state.table("table").unwrap();
        assert_eq!(table.selected, Some((6, 2)));
        assert_eq!(table.scroll_row, 3);
        assert_eq!(table.scroll_col, 1);
    }

    #[test]
    fn table_selection_edge_moves_within_current_row() {
        let mut state = WidgetState::default();
        state.tables.insert(
            "table".to_string(),
            TableState {
                columns: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                dtypes: vec![],
                rows: 5,
                resource_id: None,
                page_size: 10,
                scroll_row: 0,
                scroll_col: 0,
                selected: Some((2, 1)),
                sort: None,
                row_order: None,
            },
        );

        assert!(state.move_table_selection_to_col_edge("table", true));
        assert_eq!(state.current_table_selection("table"), Some((2, 2)));
        assert!(state.move_table_selection_to_col_edge("table", false));
        assert_eq!(state.current_table_selection("table"), Some((2, 0)));
    }

    #[test]
    fn table_sort_resets_view_selection_and_stores_row_order() {
        let mut state = WidgetState::default();
        state.tables.insert(
            "table".to_string(),
            TableState {
                columns: vec!["a".to_string(), "b".to_string()],
                dtypes: vec![],
                rows: 3,
                resource_id: None,
                page_size: 10,
                scroll_row: 2,
                scroll_col: 0,
                selected: Some((2, 1)),
                sort: None,
                row_order: None,
            },
        );

        assert!(state.toggle_table_sort("table", 0));
        let table = state.table("table").unwrap();
        assert_eq!(table.sort, Some((0, SortDirection::Asc)));
        assert_eq!(table.scroll_row, 0);
        assert_eq!(table.selected, None);

        assert!(state.set_table_row_order("table", Some(vec![1, 0, 2])));
        assert_eq!(
            state.table("table").unwrap().row_order.as_deref(),
            Some([1, 0, 2].as_slice())
        );

        assert!(state.toggle_table_sort("table", 0));
        assert_eq!(
            state.table("table").unwrap().sort,
            Some((0, SortDirection::Desc))
        );
    }

    #[test]
    fn programmatic_navigation_value_updates_active_routes() {
        let mut state = WidgetState::default();
        state.tabs.insert(
            "tabs".to_string(),
            vec![
                NavigationItem {
                    id: "tab-a".to_string(),
                    value: "a".to_string(),
                    disabled: false,
                },
                NavigationItem {
                    id: "tab-b".to_string(),
                    value: "b".to_string(),
                    disabled: false,
                },
            ],
        );
        state.pages.insert(
            "pages".to_string(),
            vec![
                NavigationItem {
                    id: "page-a".to_string(),
                    value: "a".to_string(),
                    disabled: false,
                },
                NavigationItem {
                    id: "page-b".to_string(),
                    value: "b".to_string(),
                    disabled: false,
                },
            ],
        );

        assert_eq!(state.set_active_tab_value("tabs", "b"), Some(()));
        assert_eq!(state.set_active_page_value("pages", "b"), Some(()));
        assert_eq!(state.active_tab("tabs"), Some("b"));
        assert_eq!(state.active_page("pages"), Some("b"));

        assert_eq!(state.set_active_tab_value("tabs", "missing"), None);
        assert_eq!(state.active_tab("tabs"), Some("b"));
    }

    #[test]
    fn collected_state_tracks_scrollable_container_offsets() {
        let root = node(
            "window",
            WidgetKind::Window,
            NodeProps::default(),
            vec![node(
                "scroll-panel",
                WidgetKind::Panel,
                NodeProps::default(),
                vec![node(
                    "label",
                    WidgetKind::Label,
                    NodeProps::default(),
                    vec![],
                )],
            )],
        );

        let state = WidgetState::from_tree(&root);

        assert_eq!(
            state.container_scroll_x.get("scroll-panel").copied(),
            Some(0.0)
        );
        assert_eq!(
            state.container_scroll_y.get("scroll-panel").copied(),
            Some(0.0)
        );
        assert_eq!(state.container_scroll_y.get("window").copied(), None);
    }

    #[test]
    fn rebuild_state_preserves_navigation_and_scroll_offsets() {
        let mut previous = WidgetState::default();
        previous.tabs.insert(
            "tabs".to_string(),
            vec![
                NavigationItem {
                    id: "tab-a".to_string(),
                    value: "a".to_string(),
                    disabled: false,
                },
                NavigationItem {
                    id: "tab-b".to_string(),
                    value: "b".to_string(),
                    disabled: false,
                },
            ],
        );
        previous.pages.insert(
            "pages".to_string(),
            vec![
                NavigationItem {
                    id: "page-overview".to_string(),
                    value: "overview".to_string(),
                    disabled: false,
                },
                NavigationItem {
                    id: "page-debug".to_string(),
                    value: "debug".to_string(),
                    disabled: false,
                },
            ],
        );
        let _ = previous.set_active_tab_value("tabs", "b");
        let _ = previous.set_active_page_value("pages", "debug");
        previous
            .container_scroll_x
            .insert("debug-scroll".to_string(), 18.0);
        previous
            .container_scroll_y
            .insert("debug-scroll".to_string(), 240.0);
        previous
            .container_scroll_y
            .insert("removed-scroll".to_string(), 80.0);
        previous
            .text_val
            .insert("notes".to_string(), "queue lag".to_string());
        previous.text_cursor.insert("notes".to_string(), 5);
        previous.text_scroll_y.insert("notes".to_string(), 12.0);

        let mut rebuilt = WidgetState::default();
        rebuilt.tabs.insert(
            "tabs".to_string(),
            vec![
                NavigationItem {
                    id: "tab-a".to_string(),
                    value: "a".to_string(),
                    disabled: false,
                },
                NavigationItem {
                    id: "tab-b".to_string(),
                    value: "b".to_string(),
                    disabled: false,
                },
            ],
        );
        rebuilt
            .active_tabs
            .insert("tabs".to_string(), "a".to_string());
        rebuilt.pages.insert(
            "pages".to_string(),
            vec![
                NavigationItem {
                    id: "page-overview".to_string(),
                    value: "overview".to_string(),
                    disabled: false,
                },
                NavigationItem {
                    id: "page-debug".to_string(),
                    value: "debug".to_string(),
                    disabled: false,
                },
            ],
        );
        rebuilt
            .active_pages
            .insert("pages".to_string(), "overview".to_string());
        rebuilt
            .container_scroll_x
            .insert("debug-scroll".to_string(), 0.0);
        rebuilt
            .container_scroll_y
            .insert("debug-scroll".to_string(), 0.0);
        rebuilt.text_val.insert("notes".to_string(), String::new());
        rebuilt.text_cursor.insert("notes".to_string(), 0);
        rebuilt.text_scroll_y.insert("notes".to_string(), 0.0);

        rebuilt.preserve_rebuild_state_from(&previous);

        assert_eq!(rebuilt.active_tab("tabs"), Some("b"));
        assert_eq!(rebuilt.active_page("pages"), Some("debug"));
        assert_eq!(
            rebuilt.container_scroll_x.get("debug-scroll").copied(),
            Some(18.0)
        );
        assert_eq!(
            rebuilt.container_scroll_y.get("debug-scroll").copied(),
            Some(240.0)
        );
        assert_eq!(
            rebuilt.container_scroll_y.get("removed-scroll").copied(),
            None
        );
        assert_eq!(
            rebuilt.text_val.get("notes").map(String::as_str),
            Some("queue lag")
        );
        assert_eq!(rebuilt.text_cursor.get("notes").copied(), Some(5));
        assert_eq!(rebuilt.text_scroll_y.get("notes").copied(), Some(12.0));
    }
}
