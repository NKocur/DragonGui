//! DragonGUI-owned CSS style IR.
//!
//! Parser dependencies such as `lightningcss` must lower into these types
//! immediately. Selector matching, cascade resolution, computed styles, and
//! renderer integration should not depend on parser-specific AST types.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use lightningcss::properties::Property;
use lightningcss::rules::CssRule;
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::ToCss;

use crate::document::{WidgetKind, WidgetNode};
use crate::style::{
    ColorRef, DisplayStyle, FlexDirectionStyle, FontFamily, LayoutStyle, NodePartStyles, NodeStyle,
    PartLayoutStyle, PartStyle, TextAlign, TextStyle, VisualStyle,
};
use crate::theme::{parse_hex_color, Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StylesheetOrigin {
    Framework,
    Theme,
    User,
    Inline,
}

impl StylesheetOrigin {
    fn label(self) -> &'static str {
        match self {
            StylesheetOrigin::Framework => "framework",
            StylesheetOrigin::Theme => "theme",
            StylesheetOrigin::User => "user",
            StylesheetOrigin::Inline => "inline",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Specificity {
    pub ids: u16,
    pub classes: u16,
    pub types: u16,
}

impl Specificity {
    pub const ZERO: Self = Self {
        ids: 0,
        classes: 0,
        types: 0,
    };

    pub fn new(ids: u16, classes: u16, types: u16) -> Self {
        Self {
            ids,
            classes,
            types,
        }
    }

    pub fn add(self, other: Self) -> Self {
        Self {
            ids: self.ids.saturating_add(other.ids),
            classes: self.classes.saturating_add(other.classes),
            types: self.types.saturating_add(other.types),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CascadeKey {
    pub important: bool,
    pub origin: StylesheetOrigin,
    pub specificity: Specificity,
    pub source_order: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgStyleRule {
    pub selector: DgSelector,
    pub declarations: Vec<DgStyleDeclaration>,
    pub specificity: Specificity,
    pub origin: StylesheetOrigin,
    pub source_order: u32,
}

impl DgStyleRule {
    pub fn new(
        selector: DgSelector,
        declarations: Vec<DgStyleDeclaration>,
        origin: StylesheetOrigin,
        source_order: u32,
    ) -> Self {
        let specificity = selector.specificity();
        Self {
            selector,
            declarations,
            specificity,
            origin,
            source_order,
        }
    }

    pub fn cascade_key(&self, declaration: &DgStyleDeclaration) -> CascadeKey {
        CascadeKey {
            important: declaration.important,
            origin: self.origin,
            specificity: self.specificity,
            source_order: self.source_order,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgStyleDeclaration {
    pub property: DgStyleProperty,
    pub important: bool,
}

impl DgStyleDeclaration {
    pub fn normal(property: DgStyleProperty) -> Self {
        Self {
            property,
            important: false,
        }
    }

    pub fn important(property: DgStyleProperty) -> Self {
        Self {
            property,
            important: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgStyleProperty {
    Layout(DgLayoutDeclaration),
    Visual(DgVisualDeclaration),
    Text(DgTextDeclaration),
    Widget(DgWidgetDeclaration),
    CustomProperty { name: String, value: DgCssValue },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgLayoutDeclaration {
    Display(DgCssKeyword),
    FlexDirection(DgCssKeyword),
    Flex(DgCssNumber),
    FlexGrow(DgCssNumber),
    FlexShrink(DgCssNumber),
    Width(DgCssLength),
    Height(DgCssLength),
    MinWidth(DgCssLength),
    MinHeight(DgCssLength),
    MaxWidth(DgCssLength),
    MaxHeight(DgCssLength),
    Padding(DgBoxEdges<DgCssLength>),
    PaddingLeft(DgCssLength),
    PaddingRight(DgCssLength),
    PaddingTop(DgCssLength),
    PaddingBottom(DgCssLength),
    Margin(DgBoxEdges<DgCssLength>),
    Gap(DgCssLength),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgVisualDeclaration {
    Background(DgCssColor),
    Foreground(DgCssColor),
    BorderColor(DgCssColor),
    BorderWidth(DgCssLength),
    BorderRadius(DgCssLength),
    BorderTopLeftRadius(DgCssLength),
    BorderTopRightRadius(DgCssLength),
    BorderBottomRightRadius(DgCssLength),
    BorderBottomLeftRadius(DgCssLength),
    Border(DgBorder),
    Opacity(DgCssNumber),
    Accent(DgCssColor),
    TrackColor(DgCssColor),
    ThumbColor(DgCssColor),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgTextDeclaration {
    FontSize(DgCssLength),
    FontFamily(String),
    FontWeight(u16),
    Color(DgCssColor),
    TextAlign(DgCssKeyword),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgWidgetDeclaration {
    TableRowHeight(DgCssLength),
    TableHeaderHeight(DgCssLength),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgCssValue {
    Number(DgCssNumber),
    Length(DgCssLength),
    Color(DgCssColor),
    Keyword(DgCssKeyword),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DgCssNumber(pub f32);

#[derive(Debug, Clone, PartialEq)]
pub enum DgCssLength {
    LogicalPx(f32),
    Percent(f32),
    Auto,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgCssColor {
    Rgba(Color),
    Token(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DgCssKeyword(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct DgBoxEdges<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgBorder {
    pub width: DgCssLength,
    pub style: DgBorderStyle,
    pub color: DgCssColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgBorderStyle {
    Solid,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DgSelector {
    Root,
    Compound(DgCompoundSelector),
    Child {
        parent: Box<DgSelector>,
        child: DgCompoundSelector,
    },
}

impl DgSelector {
    pub fn specificity(&self) -> Specificity {
        match self {
            DgSelector::Root => Specificity::ZERO,
            DgSelector::Compound(selector) => selector.specificity(),
            DgSelector::Child { parent, child } => parent.specificity().add(child.specificity()),
        }
    }

    pub fn matches(&self, element: &StyleElement<'_>) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.matches_element(element),
            DgSelector::Child { parent, child } => {
                child.matches_element(element)
                    && element
                        .ancestors
                        .first()
                        .is_some_and(|ancestor| parent.matches_ancestor(ancestor))
            }
        }
    }

    fn matches_ancestor(&self, ancestor: &StyleAncestor<'_>) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.matches_ancestor(ancestor),
            DgSelector::Child { .. } => false,
        }
    }

    fn target_pseudo_classes(&self) -> &[DgPseudoClass] {
        match self {
            DgSelector::Root => &[],
            DgSelector::Compound(selector) => &selector.pseudo,
            DgSelector::Child { child, .. } => &child.pseudo,
        }
    }

    fn target_part(&self) -> Option<&str> {
        match self {
            DgSelector::Root => None,
            DgSelector::Compound(selector) => selector.part.as_deref(),
            DgSelector::Child { child, .. } => child.part.as_deref(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            DgSelector::Root => ":root".to_string(),
            DgSelector::Compound(selector) => selector.label(),
            DgSelector::Child { parent, child } => {
                format!("{} > {}", parent.label(), child.label())
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DgCompoundSelector {
    pub type_selector: Option<WidgetKind>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub pseudo: Vec<DgPseudoClass>,
    pub part: Option<String>,
}

impl DgCompoundSelector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type(mut self, kind: WidgetKind) -> Self {
        self.type_selector = Some(kind);
        self
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.classes.push(class.into());
        self
    }

    pub fn with_pseudo(mut self, pseudo: DgPseudoClass) -> Self {
        self.pseudo.push(pseudo);
        self
    }

    pub fn with_part(mut self, part: impl Into<String>) -> Self {
        self.part = Some(part.into());
        self
    }

    pub fn specificity(&self) -> Specificity {
        Specificity {
            ids: u16::from(self.id.is_some()),
            classes: (self.classes.len() + self.pseudo.len()).min(u16::MAX as usize) as u16,
            types: u16::from(self.type_selector.is_some()),
        }
    }

    fn matches_element(&self, element: &StyleElement<'_>) -> bool {
        self.matches_identity(element.id, element.classes, element.kind)
            && self
                .pseudo
                .iter()
                .all(|pseudo| element.pseudo.contains(pseudo))
    }

    fn matches_ancestor(&self, ancestor: &StyleAncestor<'_>) -> bool {
        self.pseudo.is_empty()
            && self.matches_identity(ancestor.id, ancestor.classes, ancestor.kind)
    }

    fn matches_identity(&self, id: &str, classes: &[&str], kind: WidgetKind) -> bool {
        if self.type_selector.is_some_and(|expected| expected != kind) {
            return false;
        }
        if self.id.as_deref().is_some_and(|expected| expected != id) {
            return false;
        }
        self.classes
            .iter()
            .all(|expected| classes.iter().any(|class| class == expected))
    }

    fn label(&self) -> String {
        let mut label = String::new();
        if let Some(kind) = self.type_selector {
            label.push_str(css_type_name(kind).unwrap_or("Unknown"));
        }
        if let Some(id) = &self.id {
            label.push('#');
            label.push_str(id);
        }
        for class in &self.classes {
            label.push('.');
            label.push_str(class);
        }
        for pseudo in &self.pseudo {
            label.push(':');
            label.push_str(pseudo.css_name());
        }
        if let Some(part) = &self.part {
            label.push_str("::");
            label.push_str(part);
        }
        if label.is_empty() {
            "*".to_string()
        } else {
            label
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgPseudoClass {
    Hover,
    Active,
    Focus,
    Disabled,
    Checked,
}

impl DgPseudoClass {
    fn css_name(self) -> &'static str {
        match self {
            DgPseudoClass::Hover => "hover",
            DgPseudoClass::Active => "active",
            DgPseudoClass::Focus => "focus",
            DgPseudoClass::Disabled => "disabled",
            DgPseudoClass::Checked => "checked",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StyleElement<'a> {
    pub id: &'a str,
    pub key: Option<&'a str>,
    pub classes: &'a [&'a str],
    pub kind: WidgetKind,
    pub ancestors: &'a [StyleAncestor<'a>],
    pub pseudo: &'a [DgPseudoClass],
}

#[derive(Debug, Clone, Copy)]
pub struct StyleAncestor<'a> {
    pub id: &'a str,
    pub key: Option<&'a str>,
    pub classes: &'a [&'a str],
    pub kind: WidgetKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DgStylePropertyName {
    Layout(DgLayoutPropertyName),
    Visual(DgVisualPropertyName),
    Text(DgTextPropertyName),
    Widget(DgWidgetPropertyName),
    BorderShorthand,
    CustomProperty(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgLayoutPropertyName {
    Display,
    FlexDirection,
    Flex,
    FlexGrow,
    FlexShrink,
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    Padding,
    PaddingLeft,
    PaddingRight,
    PaddingTop,
    PaddingBottom,
    Margin,
    Gap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgVisualPropertyName {
    Background,
    Foreground,
    BorderColor,
    BorderWidth,
    BorderRadius,
    BorderTopLeftRadius,
    BorderTopRightRadius,
    BorderBottomRightRadius,
    BorderBottomLeftRadius,
    Opacity,
    Accent,
    TrackColor,
    ThumbColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgTextPropertyName {
    FontSize,
    FontFamily,
    FontWeight,
    Color,
    TextAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgWidgetPropertyName {
    TableRowHeight,
    TableHeaderHeight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DgStyleWarning {
    pub property: String,
    pub message: String,
}

impl DgStyleWarning {
    fn unsupported_property(name: &str) -> Self {
        Self {
            property: name.to_string(),
            message: format!("unsupported DragonGUI CSS property {name:?}"),
        }
    }
}

impl DgStylePropertyName {
    /// Supported DragonGUI CSS property matrix.
    ///
    /// Layout: display, flex-direction, flex, flex-grow, flex-shrink, width,
    /// height, min-width, min-height, max-width, max-height, padding,
    /// padding-left, padding-right, padding-top, padding-bottom, margin, gap.
    ///
    /// Visual: background, background-color, foreground, border-color,
    /// border-width, border-radius, border-top-left-radius,
    /// border-top-right-radius, border-bottom-right-radius,
    /// border-bottom-left-radius, border, opacity, accent, track-color,
    /// thumb-color.
    ///
    /// Text: color, font-size, font-family, font-weight, text-align.
    ///
    /// Widget: table-row-height, table-header-height.
    pub fn from_css_name(name: &str) -> Result<Self, DgStyleWarning> {
        let normalized = name.trim().to_ascii_lowercase();
        if normalized.starts_with("--") && normalized.len() > 2 {
            return Ok(Self::CustomProperty(normalized));
        }
        match normalized.as_str() {
            "display" => Ok(Self::Layout(DgLayoutPropertyName::Display)),
            "flex-direction" => Ok(Self::Layout(DgLayoutPropertyName::FlexDirection)),
            "flex" => Ok(Self::Layout(DgLayoutPropertyName::Flex)),
            "flex-grow" => Ok(Self::Layout(DgLayoutPropertyName::FlexGrow)),
            "flex-shrink" => Ok(Self::Layout(DgLayoutPropertyName::FlexShrink)),
            "width" => Ok(Self::Layout(DgLayoutPropertyName::Width)),
            "height" => Ok(Self::Layout(DgLayoutPropertyName::Height)),
            "min-width" => Ok(Self::Layout(DgLayoutPropertyName::MinWidth)),
            "min-height" => Ok(Self::Layout(DgLayoutPropertyName::MinHeight)),
            "max-width" => Ok(Self::Layout(DgLayoutPropertyName::MaxWidth)),
            "max-height" => Ok(Self::Layout(DgLayoutPropertyName::MaxHeight)),
            "padding" => Ok(Self::Layout(DgLayoutPropertyName::Padding)),
            "padding-left" => Ok(Self::Layout(DgLayoutPropertyName::PaddingLeft)),
            "padding-right" => Ok(Self::Layout(DgLayoutPropertyName::PaddingRight)),
            "padding-top" => Ok(Self::Layout(DgLayoutPropertyName::PaddingTop)),
            "padding-bottom" => Ok(Self::Layout(DgLayoutPropertyName::PaddingBottom)),
            "margin" => Ok(Self::Layout(DgLayoutPropertyName::Margin)),
            "gap" => Ok(Self::Layout(DgLayoutPropertyName::Gap)),
            "background" | "background-color" => Ok(Self::Visual(DgVisualPropertyName::Background)),
            "foreground" => Ok(Self::Visual(DgVisualPropertyName::Foreground)),
            "border-color" => Ok(Self::Visual(DgVisualPropertyName::BorderColor)),
            "border-width" => Ok(Self::Visual(DgVisualPropertyName::BorderWidth)),
            "border-radius" => Ok(Self::Visual(DgVisualPropertyName::BorderRadius)),
            "border-top-left-radius" => Ok(Self::Visual(DgVisualPropertyName::BorderTopLeftRadius)),
            "border-top-right-radius" => {
                Ok(Self::Visual(DgVisualPropertyName::BorderTopRightRadius))
            }
            "border-bottom-right-radius" => {
                Ok(Self::Visual(DgVisualPropertyName::BorderBottomRightRadius))
            }
            "border-bottom-left-radius" => {
                Ok(Self::Visual(DgVisualPropertyName::BorderBottomLeftRadius))
            }
            "border" => Ok(Self::BorderShorthand),
            "opacity" => Ok(Self::Visual(DgVisualPropertyName::Opacity)),
            "accent" => Ok(Self::Visual(DgVisualPropertyName::Accent)),
            "track-color" => Ok(Self::Visual(DgVisualPropertyName::TrackColor)),
            "thumb-color" => Ok(Self::Visual(DgVisualPropertyName::ThumbColor)),
            "color" => Ok(Self::Text(DgTextPropertyName::Color)),
            "font-size" => Ok(Self::Text(DgTextPropertyName::FontSize)),
            "font-family" => Ok(Self::Text(DgTextPropertyName::FontFamily)),
            "font-weight" => Ok(Self::Text(DgTextPropertyName::FontWeight)),
            "text-align" => Ok(Self::Text(DgTextPropertyName::TextAlign)),
            "table-row-height" => Ok(Self::Widget(DgWidgetPropertyName::TableRowHeight)),
            "table-header-height" => Ok(Self::Widget(DgWidgetPropertyName::TableHeaderHeight)),
            _ => Err(DgStyleWarning::unsupported_property(name)),
        }
    }
}

pub fn widget_kind_from_css_type(name: &str) -> Option<WidgetKind> {
    match name.trim() {
        "Window" => Some(WidgetKind::Window),
        "HLayout" => Some(WidgetKind::HLayout),
        "VLayout" => Some(WidgetKind::VLayout),
        "Panel" => Some(WidgetKind::Panel),
        "Collapsible" => Some(WidgetKind::Collapsible),
        "Modal" => Some(WidgetKind::Modal),
        "MenuBar" => Some(WidgetKind::MenuBar),
        "Menu" => Some(WidgetKind::Menu),
        "MenuItem" => Some(WidgetKind::MenuItem),
        "ContextMenu" => Some(WidgetKind::ContextMenu),
        "Tooltip" => Some(WidgetKind::Tooltip),
        "Sidebar" => Some(WidgetKind::Sidebar),
        "StatusBar" => Some(WidgetKind::StatusBar),
        "Tabs" => Some(WidgetKind::Tabs),
        "Tab" => Some(WidgetKind::Tab),
        "Pages" => Some(WidgetKind::Pages),
        "Page" => Some(WidgetKind::Page),
        "NavItem" => Some(WidgetKind::NavItem),
        "Label" => Some(WidgetKind::Label),
        "Button" => Some(WidgetKind::Button),
        "TextInput" => Some(WidgetKind::TextInput),
        "TextArea" => Some(WidgetKind::TextArea),
        "NumberInput" => Some(WidgetKind::NumberInput),
        "Slider" => Some(WidgetKind::Slider),
        "ProgressBar" => Some(WidgetKind::ProgressBar),
        "Dropdown" => Some(WidgetKind::Dropdown),
        "Checkbox" => Some(WidgetKind::Checkbox),
        "Separator" => Some(WidgetKind::Separator),
        "Spacer" => Some(WidgetKind::Spacer),
        "Scatter3D" => Some(WidgetKind::Scatter3D),
        "DataFrameTable" => Some(WidgetKind::DataFrameTable),
        "Image" => Some(WidgetKind::Image),
        _ => None,
    }
}

pub fn css_type_name(kind: WidgetKind) -> Option<&'static str> {
    match kind {
        WidgetKind::Window => Some("Window"),
        WidgetKind::HLayout => Some("HLayout"),
        WidgetKind::VLayout => Some("VLayout"),
        WidgetKind::Panel => Some("Panel"),
        WidgetKind::Collapsible => Some("Collapsible"),
        WidgetKind::Modal => Some("Modal"),
        WidgetKind::MenuBar => Some("MenuBar"),
        WidgetKind::Menu => Some("Menu"),
        WidgetKind::MenuItem => Some("MenuItem"),
        WidgetKind::ContextMenu => Some("ContextMenu"),
        WidgetKind::Tooltip => Some("Tooltip"),
        WidgetKind::Sidebar => Some("Sidebar"),
        WidgetKind::StatusBar => Some("StatusBar"),
        WidgetKind::Tabs => Some("Tabs"),
        WidgetKind::Tab => Some("Tab"),
        WidgetKind::Pages => Some("Pages"),
        WidgetKind::Page => Some("Page"),
        WidgetKind::NavItem => Some("NavItem"),
        WidgetKind::Label => Some("Label"),
        WidgetKind::Button => Some("Button"),
        WidgetKind::TextInput => Some("TextInput"),
        WidgetKind::TextArea => Some("TextArea"),
        WidgetKind::NumberInput => Some("NumberInput"),
        WidgetKind::Slider => Some("Slider"),
        WidgetKind::ProgressBar => Some("ProgressBar"),
        WidgetKind::Dropdown => Some("Dropdown"),
        WidgetKind::Checkbox => Some("Checkbox"),
        WidgetKind::Separator => Some("Separator"),
        WidgetKind::Spacer => Some("Spacer"),
        WidgetKind::Scatter3D => Some("Scatter3D"),
        WidgetKind::DataFrameTable => Some("DataFrameTable"),
        WidgetKind::Image => Some("Image"),
        WidgetKind::Unknown => None,
    }
}

pub fn split_classes(class_name: Option<&str>) -> Vec<&str> {
    class_name
        .into_iter()
        .flat_map(str::split_whitespace)
        .filter(|class| !class.is_empty())
        .collect()
}

const STATIC_PSEUDO_CLASSES: [DgPseudoClass; 5] = [
    DgPseudoClass::Hover,
    DgPseudoClass::Active,
    DgPseudoClass::Focus,
    DgPseudoClass::Disabled,
    DgPseudoClass::Checked,
];

const FRAMEWORK_STYLESHEET: &str = include_str!("framework.dg.css");

pub fn framework_stylesheet_for_theme(theme: &Theme) -> String {
    format!(
        "{FRAMEWORK_STYLESHEET}\n:root {{ --dg-radius: {:.3}px; --dg-font-size: {:.3}px; }}\n",
        theme.radius, theme.font_size
    )
}

#[derive(Debug, Clone)]
struct AncestorSnapshot {
    id: String,
    key: Option<String>,
    classes: Vec<String>,
    kind: WidgetKind,
}

impl AncestorSnapshot {
    fn from_node(node: &WidgetNode) -> Self {
        Self {
            id: node.id.clone(),
            key: node.key.clone(),
            classes: split_classes(node.class_name.as_deref())
                .into_iter()
                .map(str::to_string)
                .collect(),
            kind: node.kind,
        }
    }
}

pub fn apply_stylesheets_to_tree(root: &mut WidgetNode, store: &mut StylesheetStore) {
    let mut ancestors = Vec::new();
    let mut validation_warnings = Vec::new();
    let mut seen_validation_warnings = BTreeSet::new();
    {
        let rules = store.all_rules();
        apply_stylesheets_to_node(
            root,
            &rules,
            &mut ancestors,
            None,
            &mut validation_warnings,
            &mut seen_validation_warnings,
        );
    }
    store.validation_warnings = validation_warnings;
}

pub fn matched_rule_labels_for_tree(
    root: &WidgetNode,
    store: &StylesheetStore,
) -> BTreeMap<String, Vec<String>> {
    let rules = store.all_rules();
    let mut ancestors = Vec::new();
    let mut out = BTreeMap::new();
    collect_matched_rule_labels(root, &rules, &mut ancestors, &mut out);
    out
}

pub fn matched_part_rule_labels_for_tree(
    root: &WidgetNode,
    store: &StylesheetStore,
) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    let rules = store.all_rules();
    let mut ancestors = Vec::new();
    let mut out = BTreeMap::new();
    collect_matched_part_rule_labels(root, &rules, &mut ancestors, &mut out);
    out
}

fn collect_matched_part_rule_labels(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &mut Vec<AncestorSnapshot>,
    out: &mut BTreeMap<String, BTreeMap<String, Vec<String>>>,
) {
    let labels = matched_part_rule_labels_for_node(node, rules, ancestors);
    if !labels.is_empty() {
        out.insert(node.id.clone(), labels);
    }
    ancestors.push(AncestorSnapshot::from_node(node));
    for child in &node.children {
        collect_matched_part_rule_labels(child, rules, ancestors, out);
    }
    ancestors.pop();
}

fn matched_part_rule_labels_for_node(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &[AncestorSnapshot],
) -> BTreeMap<String, Vec<String>> {
    let classes = split_classes(node.class_name.as_deref());
    let ancestor_classes: Vec<Vec<&str>> = ancestors
        .iter()
        .map(|ancestor| ancestor.classes.iter().map(String::as_str).collect())
        .collect();
    let style_ancestors: Vec<StyleAncestor<'_>> = ancestors
        .iter()
        .zip(ancestor_classes.iter())
        .rev()
        .map(|(ancestor, classes)| StyleAncestor {
            id: ancestor.id.as_str(),
            key: ancestor.key.as_deref(),
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let element = StyleElement {
        id: node.id.as_str(),
        key: node.key.as_deref(),
        classes: &classes,
        kind: node.kind,
        ancestors: &style_ancestors,
        pseudo: &STATIC_PSEUDO_CLASSES,
    };
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rule in rules.iter().filter(|rule| rule.selector.matches(&element)) {
        let Some(part) = rule.selector.target_part() else {
            continue;
        };
        if !widget_kind_supports_part(node.kind, part) {
            continue;
        }
        out.entry(part.to_string()).or_default().push(format!(
            "{}: {}",
            rule.origin.label(),
            rule.selector.label()
        ));
    }
    out
}

fn collect_matched_rule_labels(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &mut Vec<AncestorSnapshot>,
    out: &mut BTreeMap<String, Vec<String>>,
) {
    let labels = matched_rule_labels_for_node(node, rules, ancestors);
    if !labels.is_empty() {
        out.insert(node.id.clone(), labels);
    }
    ancestors.push(AncestorSnapshot::from_node(node));
    for child in &node.children {
        collect_matched_rule_labels(child, rules, ancestors, out);
    }
    ancestors.pop();
}

fn matched_rule_labels_for_node(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &[AncestorSnapshot],
) -> Vec<String> {
    let classes = split_classes(node.class_name.as_deref());
    let ancestor_classes: Vec<Vec<&str>> = ancestors
        .iter()
        .map(|ancestor| ancestor.classes.iter().map(String::as_str).collect())
        .collect();
    let style_ancestors: Vec<StyleAncestor<'_>> = ancestors
        .iter()
        .zip(ancestor_classes.iter())
        .rev()
        .map(|(ancestor, classes)| StyleAncestor {
            id: ancestor.id.as_str(),
            key: ancestor.key.as_deref(),
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let element = StyleElement {
        id: node.id.as_str(),
        key: node.key.as_deref(),
        classes: &classes,
        kind: node.kind,
        ancestors: &style_ancestors,
        pseudo: &STATIC_PSEUDO_CLASSES,
    };
    rules
        .iter()
        .filter(|rule| rule.selector.matches(&element))
        .map(|rule| format!("{}: {}", rule.origin.label(), rule.selector.label()))
        .collect()
}

fn apply_stylesheets_to_node(
    node: &mut WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &mut Vec<AncestorSnapshot>,
    inherited_text: Option<&TextStyle>,
    validation_warnings: &mut Vec<DgStyleWarning>,
    seen_validation_warnings: &mut BTreeSet<String>,
) {
    let classes = split_classes(node.class_name.as_deref());
    let ancestor_classes: Vec<Vec<&str>> = ancestors
        .iter()
        .map(|ancestor| ancestor.classes.iter().map(String::as_str).collect())
        .collect();
    let style_ancestors: Vec<StyleAncestor<'_>> = ancestors
        .iter()
        .zip(ancestor_classes.iter())
        .rev()
        .map(|(ancestor, classes)| StyleAncestor {
            id: ancestor.id.as_str(),
            key: ancestor.key.as_deref(),
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let element = StyleElement {
        id: node.id.as_str(),
        key: node.key.as_deref(),
        classes: &classes,
        kind: node.kind,
        ancestors: &style_ancestors,
        pseudo: &STATIC_PSEUDO_CLASSES,
    };
    // Pseudo-state selectors are intentionally matched against a static pseudo
    // set here. Their declarations are precomputed into hover/active/focus/
    // disabled style slots, and live widget state decides which slot is active.
    let mut matched = Vec::new();
    for rule in rules.iter() {
        if rule.selector.matches(&element) {
            if let Some(part) = rule.selector.target_part() {
                if !widget_kind_supports_part(node.kind, part) {
                    record_unsupported_part_warning(
                        validation_warnings,
                        seen_validation_warnings,
                        rule,
                        node.kind,
                        part,
                    );
                    continue;
                }
                record_stateful_part_layout_warnings(
                    validation_warnings,
                    seen_validation_warnings,
                    rule,
                    part,
                );
            }
            matched.extend(rule.declarations.iter().map(|declaration| {
                (
                    rule.cascade_key(declaration),
                    rule.selector.target_pseudo_classes(),
                    rule.selector.target_part(),
                    &declaration.property,
                )
            }));
        }
    }
    matched.sort_by_key(|(key, _, _, _)| *key);

    let mut computed = NodeStyle::default();
    for (_, pseudo_classes, part, property) in matched {
        if let Some(part) = part {
            apply_property_to_part_style(&mut computed, part, pseudo_classes, property);
        } else if pseudo_classes.is_empty() {
            apply_property_to_style(&mut computed, property);
        } else {
            for pseudo in pseudo_classes {
                apply_property_to_pseudo_style(&mut computed, *pseudo, property);
            }
        }
    }
    merge_node_style(&mut computed, &node.inline_style);
    retain_supported_inline_parts(
        node.kind,
        &mut computed.parts,
        validation_warnings,
        seen_validation_warnings,
    );
    if let Some(inherited_text) = inherited_text {
        inherit_text_style(&mut computed.text, inherited_text);
    }
    node.style = computed;

    ancestors.push(AncestorSnapshot::from_node(node));
    let child_text = node.style.text.clone();
    for child in &mut node.children {
        apply_stylesheets_to_node(
            child,
            rules,
            ancestors,
            Some(&child_text),
            validation_warnings,
            seen_validation_warnings,
        );
    }
    ancestors.pop();
}

fn record_stateful_part_layout_warnings(
    warnings: &mut Vec<DgStyleWarning>,
    seen: &mut BTreeSet<String>,
    rule: &DgStyleRule,
    part: &str,
) {
    if rule.selector.target_pseudo_classes().is_empty() {
        return;
    }

    for declaration in &rule.declarations {
        let Some(property) = stateful_part_layout_property_name(&declaration.property) else {
            continue;
        };
        let key = format!(
            "{}|{}|{}|stateful-part-layout",
            rule.selector.label(),
            part,
            property
        );
        if seen.insert(key) {
            warnings.push(DgStyleWarning {
                property: rule.selector.label(),
                message: format!(
                    "{property} on {selector} is ignored because part layout fields cannot vary by pseudo-state",
                    selector = rule.selector.label()
                ),
            });
        }
    }
}

fn stateful_part_layout_property_name(property: &DgStyleProperty) -> Option<&'static str> {
    match property {
        DgStyleProperty::Layout(DgLayoutDeclaration::Width(_)) => Some("width"),
        DgStyleProperty::Layout(DgLayoutDeclaration::Height(_)) => Some("height"),
        DgStyleProperty::Layout(DgLayoutDeclaration::Padding(_)) => Some("padding"),
        DgStyleProperty::Layout(DgLayoutDeclaration::Gap(_)) => Some("gap"),
        _ => None,
    }
}

fn record_unsupported_part_warning(
    warnings: &mut Vec<DgStyleWarning>,
    seen: &mut BTreeSet<String>,
    rule: &DgStyleRule,
    kind: WidgetKind,
    part: &str,
) {
    let key = format!("{}|{:?}|{}", rule.selector.label(), kind, part);
    if seen.insert(key) {
        let widget = css_type_name(kind).unwrap_or("Unknown");
        warnings.push(DgStyleWarning {
            property: rule.selector.label(),
            message: format!("{widget} has no CSS part {part:?}; rule ignored for this widget"),
        });
    }
}

fn retain_supported_inline_parts(
    kind: WidgetKind,
    parts: &mut NodePartStyles,
    warnings: &mut Vec<DgStyleWarning>,
    seen: &mut BTreeSet<String>,
) {
    retain_supported_inline_part_map(kind, &mut parts.parts, "base", warnings, seen);
    retain_supported_inline_part_map(kind, &mut parts.hover, "hover", warnings, seen);
    retain_supported_inline_part_map(kind, &mut parts.active, "active", warnings, seen);
    retain_supported_inline_part_map(kind, &mut parts.focus, "focus", warnings, seen);
    retain_supported_inline_part_map(kind, &mut parts.disabled, "disabled", warnings, seen);
    retain_supported_inline_part_map(kind, &mut parts.checked, "checked", warnings, seen);
}

fn retain_supported_inline_part_map(
    kind: WidgetKind,
    map: &mut BTreeMap<String, PartStyle>,
    state: &str,
    warnings: &mut Vec<DgStyleWarning>,
    seen: &mut BTreeSet<String>,
) {
    map.retain(|part, _| {
        if widget_kind_supports_part(kind, part) {
            true
        } else {
            record_unsupported_inline_part_warning(warnings, seen, kind, part, state);
            false
        }
    });
}

fn record_unsupported_inline_part_warning(
    warnings: &mut Vec<DgStyleWarning>,
    seen: &mut BTreeSet<String>,
    kind: WidgetKind,
    part: &str,
    state: &str,
) {
    let key = format!("inline|{:?}|{}|{}", kind, state, part);
    if seen.insert(key) {
        let widget = css_type_name(kind).unwrap_or("Unknown");
        warnings.push(DgStyleWarning {
            property: format!("inline parts.{part}"),
            message: format!(
                "{widget} has no CSS part {part:?}; inline {state} part style ignored"
            ),
        });
    }
}

fn widget_kind_supports_part(kind: WidgetKind, part: &str) -> bool {
    match kind {
        WidgetKind::Panel => matches!(part, "accent"),
        WidgetKind::Collapsible => matches!(part, "header" | "indicator" | "body"),
        WidgetKind::Button => matches!(part, "badge"),
        WidgetKind::NumberInput => matches!(
            part,
            "field"
                | "stepper"
                | "stepper-up"
                | "stepper-down"
                | "stepper-divider"
                | "divider"
                | "caret"
        ),
        WidgetKind::Dropdown => matches!(
            part,
            "field" | "chevron" | "menu" | "item" | "item-selected" | "item-hover"
        ),
        WidgetKind::Checkbox => matches!(part, "row" | "box" | "indicator" | "label"),
        WidgetKind::Slider => matches!(part, "track" | "fill" | "thumb"),
        WidgetKind::ProgressBar => matches!(part, "track" | "fill" | "label"),
        WidgetKind::Tabs => matches!(part, "header"),
        WidgetKind::Tab => matches!(part, "tab" | "accent" | "badge"),
        WidgetKind::NavItem => matches!(part, "item" | "accent" | "badge"),
        WidgetKind::DataFrameTable => {
            matches!(part, "header" | "row" | "row-selected" | "grid-line")
        }
        _ => false,
    }
}

fn merge_node_style(base: &mut NodeStyle, overlay: &NodeStyle) {
    merge_layout_style(&mut base.layout, &overlay.layout);
    merge_visual_style(&mut base.visual, &overlay.visual);
    merge_text_style(&mut base.text, &overlay.text);
    merge_widget_style(&mut base.widget, &overlay.widget);
    merge_node_part_styles(&mut base.parts, &overlay.parts);
    merge_visual_style(&mut base.hover, &overlay.hover);
    merge_visual_style(&mut base.active, &overlay.active);
    merge_visual_style(&mut base.focus, &overlay.focus);
    merge_visual_style(&mut base.disabled, &overlay.disabled);
    merge_visual_style(&mut base.checked, &overlay.checked);
}

fn merge_layout_style(base: &mut LayoutStyle, overlay: &LayoutStyle) {
    base.display = overlay.display.or(base.display);
    base.flex_direction = overlay.flex_direction.or(base.flex_direction);
    base.width = overlay.width.or(base.width);
    base.height = overlay.height.or(base.height);
    base.min_width = overlay.min_width.or(base.min_width);
    base.min_height = overlay.min_height.or(base.min_height);
    base.max_width = overlay.max_width.or(base.max_width);
    base.max_height = overlay.max_height.or(base.max_height);
    base.padding = overlay.padding.or(base.padding);
    base.padding_left = overlay.padding_left.or(base.padding_left);
    base.padding_right = overlay.padding_right.or(base.padding_right);
    base.padding_top = overlay.padding_top.or(base.padding_top);
    base.padding_bottom = overlay.padding_bottom.or(base.padding_bottom);
    base.margin = overlay.margin.or(base.margin);
    base.gap = overlay.gap.or(base.gap);
    base.flex_grow = overlay.flex_grow.or(base.flex_grow);
    base.flex_shrink = overlay.flex_shrink.or(base.flex_shrink);
}

fn merge_visual_style(base: &mut VisualStyle, overlay: &VisualStyle) {
    *base = base.merged(overlay);
}

fn merge_text_style(base: &mut TextStyle, overlay: &TextStyle) {
    base.font_size = overlay.font_size.or(base.font_size);
    base.font_family = overlay
        .font_family
        .clone()
        .or_else(|| base.font_family.clone());
    base.font_weight = overlay.font_weight.or(base.font_weight);
    base.color = overlay.color.clone().or_else(|| base.color.clone());
    base.text_align = overlay.text_align.or(base.text_align);
}

fn merge_widget_style(base: &mut crate::style::WidgetStyle, overlay: &crate::style::WidgetStyle) {
    base.table_row_height = overlay.table_row_height.or(base.table_row_height);
    base.table_header_height = overlay.table_header_height.or(base.table_header_height);
}

fn merge_node_part_styles(base: &mut NodePartStyles, overlay: &NodePartStyles) {
    merge_part_style_map(&mut base.parts, &overlay.parts);
    merge_part_style_map(&mut base.hover, &overlay.hover);
    merge_part_style_map(&mut base.active, &overlay.active);
    merge_part_style_map(&mut base.focus, &overlay.focus);
    merge_part_style_map(&mut base.disabled, &overlay.disabled);
    merge_part_style_map(&mut base.checked, &overlay.checked);
}

fn merge_part_style_map(
    base: &mut BTreeMap<String, PartStyle>,
    overlay: &BTreeMap<String, PartStyle>,
) {
    for (name, overlay_style) in overlay {
        let base_style = base.entry(name.clone()).or_default();
        merge_part_style(base_style, overlay_style);
    }
}

fn merge_part_style(base: &mut PartStyle, overlay: &PartStyle) {
    merge_part_layout_style(&mut base.layout, &overlay.layout);
    merge_visual_style(&mut base.visual, &overlay.visual);
    merge_text_style(&mut base.text, &overlay.text);
}

fn merge_part_layout_style(base: &mut PartLayoutStyle, overlay: &PartLayoutStyle) {
    base.width = overlay.width.or(base.width);
    base.height = overlay.height.or(base.height);
    base.padding = overlay.padding.or(base.padding);
    base.gap = overlay.gap.or(base.gap);
}

fn inherit_text_style(target: &mut TextStyle, inherited: &TextStyle) {
    target.font_size = target.font_size.or(inherited.font_size);
    target.font_family = target
        .font_family
        .clone()
        .or_else(|| inherited.font_family.clone());
    target.font_weight = target.font_weight.or(inherited.font_weight);
    target.color = target.color.clone().or_else(|| inherited.color.clone());
    target.text_align = target.text_align.or(inherited.text_align);
}

fn apply_property_to_style(style: &mut NodeStyle, property: &DgStyleProperty) {
    match property {
        DgStyleProperty::Layout(declaration) => {
            apply_layout_declaration(&mut style.layout, declaration)
        }
        DgStyleProperty::Visual(declaration) => {
            apply_visual_declaration(&mut style.visual, declaration)
        }
        DgStyleProperty::Text(declaration) => apply_text_declaration(&mut style.text, declaration),
        DgStyleProperty::Widget(declaration) => {
            apply_widget_declaration(&mut style.widget, declaration)
        }
        DgStyleProperty::CustomProperty { .. } => {}
    }
}

fn apply_property_to_pseudo_style(
    style: &mut NodeStyle,
    pseudo: DgPseudoClass,
    property: &DgStyleProperty,
) {
    let target = match pseudo {
        DgPseudoClass::Hover => &mut style.hover,
        DgPseudoClass::Active => &mut style.active,
        DgPseudoClass::Focus => &mut style.focus,
        DgPseudoClass::Disabled => &mut style.disabled,
        DgPseudoClass::Checked => &mut style.checked,
    };
    match property {
        DgStyleProperty::Visual(declaration) => apply_visual_declaration(target, declaration),
        DgStyleProperty::Text(DgTextDeclaration::Color(color)) => {
            target.foreground = Some(color_ref_from_css(color));
        }
        DgStyleProperty::Layout(_)
        | DgStyleProperty::Text(_)
        | DgStyleProperty::Widget(_)
        | DgStyleProperty::CustomProperty { .. } => {}
    }
}

fn apply_property_to_part_style(
    style: &mut NodeStyle,
    part: &str,
    pseudo_classes: &[DgPseudoClass],
    property: &DgStyleProperty,
) {
    if pseudo_classes.is_empty() {
        let target = style.parts.parts.entry(part.to_string()).or_default();
        apply_property_to_part(target, property, false);
        return;
    }

    for pseudo in pseudo_classes {
        let target_map = match pseudo {
            DgPseudoClass::Hover => &mut style.parts.hover,
            DgPseudoClass::Active => &mut style.parts.active,
            DgPseudoClass::Focus => &mut style.parts.focus,
            DgPseudoClass::Disabled => &mut style.parts.disabled,
            DgPseudoClass::Checked => &mut style.parts.checked,
        };
        let target = target_map.entry(part.to_string()).or_default();
        apply_property_to_part(target, property, true);
    }
}

fn apply_property_to_part(style: &mut PartStyle, property: &DgStyleProperty, stateful: bool) {
    match property {
        DgStyleProperty::Layout(declaration) if !stateful => {
            apply_part_layout_declaration(&mut style.layout, declaration)
        }
        DgStyleProperty::Visual(declaration) => {
            apply_visual_declaration(&mut style.visual, declaration)
        }
        DgStyleProperty::Text(declaration) => apply_text_declaration(&mut style.text, declaration),
        DgStyleProperty::Layout(_)
        | DgStyleProperty::Widget(_)
        | DgStyleProperty::CustomProperty { .. } => {}
    }
}

fn apply_part_layout_declaration(style: &mut PartLayoutStyle, declaration: &DgLayoutDeclaration) {
    match declaration {
        DgLayoutDeclaration::Width(value) => style.width = length_px(value),
        DgLayoutDeclaration::Height(value) => style.height = length_px(value),
        DgLayoutDeclaration::Padding(edges) => {
            if edges.top == edges.right && edges.right == edges.bottom && edges.bottom == edges.left
            {
                style.padding = length_px(&edges.top);
            }
        }
        DgLayoutDeclaration::Gap(value) => style.gap = length_px(value),
        _ => {}
    }
}

fn apply_layout_declaration(style: &mut LayoutStyle, declaration: &DgLayoutDeclaration) {
    match declaration {
        DgLayoutDeclaration::Display(value) => style.display = display_from_keyword(value),
        DgLayoutDeclaration::FlexDirection(value) => {
            style.flex_direction = flex_direction_from_keyword(value);
        }
        DgLayoutDeclaration::Flex(value) => style.flex_grow = Some(value.0.max(0.0)),
        DgLayoutDeclaration::FlexGrow(value) => style.flex_grow = Some(value.0.max(0.0)),
        DgLayoutDeclaration::FlexShrink(value) => style.flex_shrink = Some(value.0.max(0.0)),
        DgLayoutDeclaration::Width(value) => style.width = length_px(value),
        DgLayoutDeclaration::Height(value) => style.height = length_px(value),
        DgLayoutDeclaration::MinWidth(value) => style.min_width = length_px(value),
        DgLayoutDeclaration::MinHeight(value) => style.min_height = length_px(value),
        DgLayoutDeclaration::MaxWidth(value) => style.max_width = length_px(value),
        DgLayoutDeclaration::MaxHeight(value) => style.max_height = length_px(value),
        DgLayoutDeclaration::Padding(edges) => {
            style.padding_top = length_px(&edges.top);
            style.padding_right = length_px(&edges.right);
            style.padding_bottom = length_px(&edges.bottom);
            style.padding_left = length_px(&edges.left);
        }
        DgLayoutDeclaration::PaddingLeft(value) => style.padding_left = length_px(value),
        DgLayoutDeclaration::PaddingRight(value) => style.padding_right = length_px(value),
        DgLayoutDeclaration::PaddingTop(value) => style.padding_top = length_px(value),
        DgLayoutDeclaration::PaddingBottom(value) => style.padding_bottom = length_px(value),
        DgLayoutDeclaration::Margin(edges) => {
            if edges.top == edges.right && edges.right == edges.bottom && edges.bottom == edges.left
            {
                style.margin = length_px(&edges.top);
            }
        }
        DgLayoutDeclaration::Gap(value) => style.gap = length_px(value),
    }
}

fn apply_visual_declaration(style: &mut VisualStyle, declaration: &DgVisualDeclaration) {
    match declaration {
        DgVisualDeclaration::Background(value) => {
            style.background = Some(color_ref_from_css(value))
        }
        DgVisualDeclaration::Foreground(value) => {
            style.foreground = Some(color_ref_from_css(value))
        }
        DgVisualDeclaration::BorderColor(value) => {
            style.border_color = Some(color_ref_from_css(value));
        }
        DgVisualDeclaration::BorderWidth(value) => style.border_width = length_px(value),
        DgVisualDeclaration::BorderRadius(value) => style.border_radius = length_px(value),
        DgVisualDeclaration::BorderTopLeftRadius(value) => {
            style.corner_radii.top_left = length_px(value)
        }
        DgVisualDeclaration::BorderTopRightRadius(value) => {
            style.corner_radii.top_right = length_px(value)
        }
        DgVisualDeclaration::BorderBottomRightRadius(value) => {
            style.corner_radii.bottom_right = length_px(value)
        }
        DgVisualDeclaration::BorderBottomLeftRadius(value) => {
            style.corner_radii.bottom_left = length_px(value)
        }
        DgVisualDeclaration::Border(border) => {
            style.border_width = length_px(&border.width);
            style.border_color = Some(color_ref_from_css(&border.color));
        }
        DgVisualDeclaration::Opacity(value) => style.opacity = Some(value.0.clamp(0.0, 1.0)),
        DgVisualDeclaration::Accent(value) => style.accent = Some(color_ref_from_css(value)),
        DgVisualDeclaration::TrackColor(value) => {
            style.track_color = Some(color_ref_from_css(value))
        }
        DgVisualDeclaration::ThumbColor(value) => {
            style.thumb_color = Some(color_ref_from_css(value))
        }
    }
}

fn apply_text_declaration(style: &mut TextStyle, declaration: &DgTextDeclaration) {
    match declaration {
        DgTextDeclaration::FontSize(value) => style.font_size = length_px(value),
        DgTextDeclaration::FontFamily(value) => {
            style.font_family = Some(font_family_from_css(value))
        }
        DgTextDeclaration::FontWeight(value) => style.font_weight = Some((*value).clamp(100, 900)),
        DgTextDeclaration::Color(value) => style.color = Some(color_ref_from_css(value)),
        DgTextDeclaration::TextAlign(value) => style.text_align = text_align_from_keyword(value),
    }
}

fn apply_widget_declaration(
    style: &mut crate::style::WidgetStyle,
    declaration: &DgWidgetDeclaration,
) {
    match declaration {
        DgWidgetDeclaration::TableRowHeight(value) => style.table_row_height = length_px(value),
        DgWidgetDeclaration::TableHeaderHeight(value) => {
            style.table_header_height = length_px(value)
        }
    }
}

fn display_from_keyword(value: &DgCssKeyword) -> Option<DisplayStyle> {
    match value.0.trim().to_ascii_lowercase().as_str() {
        "flex" => Some(DisplayStyle::Flex),
        "block" => Some(DisplayStyle::Block),
        "none" => Some(DisplayStyle::None),
        _ => None,
    }
}

fn flex_direction_from_keyword(value: &DgCssKeyword) -> Option<FlexDirectionStyle> {
    match value.0.trim().to_ascii_lowercase().as_str() {
        "row" => Some(FlexDirectionStyle::Row),
        "column" => Some(FlexDirectionStyle::Column),
        "row-reverse" | "row_reverse" => Some(FlexDirectionStyle::RowReverse),
        "column-reverse" | "column_reverse" => Some(FlexDirectionStyle::ColumnReverse),
        _ => None,
    }
}

fn text_align_from_keyword(value: &DgCssKeyword) -> Option<TextAlign> {
    match value.0.trim().to_ascii_lowercase().as_str() {
        "left" | "start" => Some(TextAlign::Left),
        "center" | "middle" => Some(TextAlign::Center),
        "right" | "end" => Some(TextAlign::Right),
        _ => None,
    }
}

fn font_family_from_css(value: &str) -> FontFamily {
    match value.trim().to_ascii_lowercase().as_str() {
        "serif" => FontFamily::Serif,
        "sans" | "sans-serif" | "sans_serif" | "system" => FontFamily::SansSerif,
        "mono" | "monospace" => FontFamily::Monospace,
        "cursive" => FontFamily::Cursive,
        "fantasy" => FontFamily::Fantasy,
        _ => FontFamily::Name(value.trim().to_string()),
    }
}

fn color_ref_from_css(value: &DgCssColor) -> ColorRef {
    match value {
        DgCssColor::Rgba(color) => ColorRef::Rgba(*color),
        DgCssColor::Token(token) => ColorRef::Token(token.clone()),
    }
}

fn length_px(value: &DgCssLength) -> Option<f32> {
    match value {
        DgCssLength::LogicalPx(value) => Some(*value),
        DgCssLength::Percent(_) | DgCssLength::Auto => None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParsedStylesheet {
    pub rules: Vec<DgStyleRule>,
    pub variables: BTreeMap<String, DgCssValue>,
    pub warnings: Vec<DgStyleWarning>,
}

#[derive(Debug, Clone, Default)]
pub struct StylesheetStore {
    framework: ParsedStylesheet,
    theme: ParsedStylesheet,
    user: ParsedStylesheet,
    validation_warnings: Vec<DgStyleWarning>,
    pub last_error: Option<String>,
}

pub struct StylesheetRuleRefs<'a> {
    framework: &'a [DgStyleRule],
    theme: &'a [DgStyleRule],
    user: &'a [DgStyleRule],
}

impl<'a> StylesheetRuleRefs<'a> {
    fn iter(&'a self) -> impl Iterator<Item = &'a DgStyleRule> {
        self.framework
            .iter()
            .chain(self.theme.iter())
            .chain(self.user.iter())
    }

    pub fn len(&self) -> usize {
        self.framework.len() + self.theme.len() + self.user.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl StylesheetStore {
    pub fn install_framework_defaults(&mut self, theme: &Theme) {
        let css = framework_stylesheet_for_theme(theme);
        if let Ok(parsed) = parse_stylesheet(&css, StylesheetOrigin::Framework) {
            self.framework = parsed;
        }
    }

    pub fn set_stylesheet(
        &mut self,
        origin: StylesheetOrigin,
        css: &str,
    ) -> Result<(), DgCssParseError> {
        let parsed = parse_stylesheet(css, origin)?;
        match origin {
            StylesheetOrigin::Framework => self.framework = parsed,
            StylesheetOrigin::Theme => self.theme = parsed,
            StylesheetOrigin::User => self.user = parsed,
            StylesheetOrigin::Inline => {
                let error = DgCssParseError::new("inline styles are not stored as stylesheets");
                self.last_error = Some(error.message.clone());
                return Err(error);
            }
        }
        self.last_error = None;
        Ok(())
    }

    pub fn clear(&mut self, origin: StylesheetOrigin) {
        match origin {
            StylesheetOrigin::Framework => self.framework = ParsedStylesheet::default(),
            StylesheetOrigin::Theme => self.theme = ParsedStylesheet::default(),
            StylesheetOrigin::User => self.user = ParsedStylesheet::default(),
            StylesheetOrigin::Inline => {}
        }
    }

    pub fn rules(&self, origin: StylesheetOrigin) -> &[DgStyleRule] {
        match origin {
            StylesheetOrigin::Framework => &self.framework.rules,
            StylesheetOrigin::Theme => &self.theme.rules,
            StylesheetOrigin::User => &self.user.rules,
            StylesheetOrigin::Inline => &[],
        }
    }

    pub fn all_rules(&self) -> StylesheetRuleRefs<'_> {
        StylesheetRuleRefs {
            framework: &self.framework.rules,
            theme: &self.theme.rules,
            user: &self.user.rules,
        }
    }

    pub fn variables(&self) -> BTreeMap<String, DgCssValue> {
        let mut variables = BTreeMap::new();
        variables.extend(self.framework.variables.clone());
        variables.extend(self.theme.variables.clone());
        variables.extend(self.user.variables.clone());
        variables
    }

    pub fn warnings(&self) -> Vec<&DgStyleWarning> {
        self.framework
            .warnings
            .iter()
            .chain(self.theme.warnings.iter())
            .chain(self.user.warnings.iter())
            .chain(self.validation_warnings.iter())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DgCssParseError {
    pub message: String,
}

impl DgCssParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DgCssParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DgCssParseError {}

pub fn parse_stylesheet(
    css: &str,
    origin: StylesheetOrigin,
) -> Result<ParsedStylesheet, DgCssParseError> {
    let sheet = StyleSheet::parse(css, ParserOptions::default()).map_err(|error| {
        DgCssParseError::new(format!("failed to parse DragonGUI stylesheet: {error}"))
    })?;

    let mut variables = BTreeMap::new();
    let mut warnings = Vec::new();
    collect_root_variables(&sheet, &mut variables, &mut warnings)?;

    let mut rules = Vec::new();
    let mut source_order = 0;
    for rule in sheet.rules.0.iter() {
        let CssRule::Style(style_rule) = rule else {
            continue;
        };
        let selectors = selector_strings(&style_rule.selectors)?;
        let declaration_specs = lower_declarations(
            &style_rule.declarations,
            &variables,
            &mut warnings,
            selectors.first().map(String::as_str),
        )?;
        if declaration_specs.is_empty() {
            continue;
        }
        for selector_text in selectors {
            if selector_text == ":root" {
                continue;
            }
            let Some(selector) = parse_selector(&selector_text, &mut warnings) else {
                continue;
            };
            let declarations = declaration_specs
                .iter()
                .cloned()
                .map(|(property, important)| DgStyleDeclaration {
                    property,
                    important,
                })
                .collect();
            rules.push(DgStyleRule::new(
                selector,
                declarations,
                origin,
                source_order,
            ));
            source_order += 1;
        }
    }

    Ok(ParsedStylesheet {
        rules,
        variables,
        warnings,
    })
}

fn collect_root_variables(
    sheet: &StyleSheet<'_, '_>,
    variables: &mut BTreeMap<String, DgCssValue>,
    warnings: &mut Vec<DgStyleWarning>,
) -> Result<(), DgCssParseError> {
    for rule in sheet.rules.0.iter() {
        let CssRule::Style(style_rule) = rule else {
            continue;
        };
        let selectors = selector_strings(&style_rule.selectors)?;
        if !selectors.iter().any(|selector| selector == ":root") {
            continue;
        }
        for (declaration, important) in style_rule.declarations.iter() {
            let declaration_text = declaration_to_css(declaration, important)?;
            let Some((name, value)) = split_declaration(&declaration_text) else {
                continue;
            };
            if !name.starts_with("--") {
                continue;
            }
            if let Some(value) = parse_css_value(value, variables) {
                variables.insert(name.to_string(), value);
            } else {
                warnings.push(DgStyleWarning {
                    property: name.to_string(),
                    message: format!("could not parse custom property value {value:?}"),
                });
            }
        }
    }
    Ok(())
}

fn selector_strings(selectors: &impl ToCss) -> Result<Vec<String>, DgCssParseError> {
    let css = selectors
        .to_css_string(PrinterOptions::default())
        .map_err(|error| DgCssParseError::new(format!("failed to serialize selector: {error}")))?;
    Ok(split_selector_list(&css))
}

fn declaration_to_css(
    declaration: &Property<'_>,
    important: bool,
) -> Result<String, DgCssParseError> {
    declaration
        .to_css_string(important, PrinterOptions::default())
        .map_err(|error| DgCssParseError::new(format!("failed to serialize declaration: {error}")))
}

fn lower_declarations(
    block: &lightningcss::declaration::DeclarationBlock<'_>,
    variables: &BTreeMap<String, DgCssValue>,
    warnings: &mut Vec<DgStyleWarning>,
    selector: Option<&str>,
) -> Result<Vec<(DgStyleProperty, bool)>, DgCssParseError> {
    let mut declarations = Vec::new();
    for (declaration, important) in block.iter() {
        let declaration_text = declaration_to_css(declaration, important)?;
        let Some((name, value)) = split_declaration(&declaration_text) else {
            continue;
        };
        match lower_declaration(name, value, variables) {
            Ok(Some(property)) => declarations.push((property, important)),
            Ok(None) => {}
            Err(mut warning) => {
                if let Some(selector) = selector {
                    warning.message = format!("{} in selector {selector:?}", warning.message);
                }
                warnings.push(warning);
            }
        }
    }
    Ok(declarations)
}

fn lower_declaration(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgStyleProperty>, DgStyleWarning> {
    match DgStylePropertyName::from_css_name(name)? {
        DgStylePropertyName::CustomProperty(name) => {
            let Some(value) = parse_css_value(value, variables) else {
                return Err(DgStyleWarning {
                    property: name,
                    message: format!("could not parse custom property value {value:?}"),
                });
            };
            Ok(Some(DgStyleProperty::CustomProperty { name, value }))
        }
        DgStylePropertyName::BorderShorthand => {
            let border = parse_border(value, variables).ok_or_else(|| DgStyleWarning {
                property: name.to_string(),
                message: "only `border: <width> solid <color>` is supported".to_string(),
            })?;
            Ok(Some(DgStyleProperty::Visual(DgVisualDeclaration::Border(
                border,
            ))))
        }
        DgStylePropertyName::Layout(property) => lower_layout(name, property, value, variables),
        DgStylePropertyName::Visual(property) => lower_visual(name, property, value, variables),
        DgStylePropertyName::Text(property) => lower_text(name, property, value, variables),
        DgStylePropertyName::Widget(property) => lower_widget(name, property, value, variables),
    }
}

fn lower_layout(
    name: &str,
    property: DgLayoutPropertyName,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgStyleProperty>, DgStyleWarning> {
    let declaration = match property {
        DgLayoutPropertyName::Display => {
            DgLayoutDeclaration::Display(DgCssKeyword(resolve_keyword(value, variables)))
        }
        DgLayoutPropertyName::FlexDirection => {
            DgLayoutDeclaration::FlexDirection(DgCssKeyword(resolve_keyword(value, variables)))
        }
        DgLayoutPropertyName::Flex => {
            DgLayoutDeclaration::Flex(parse_number_value(name, value, variables)?)
        }
        DgLayoutPropertyName::FlexGrow => {
            DgLayoutDeclaration::FlexGrow(parse_number_value(name, value, variables)?)
        }
        DgLayoutPropertyName::FlexShrink => {
            DgLayoutDeclaration::FlexShrink(parse_number_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Width => {
            DgLayoutDeclaration::Width(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Height => {
            DgLayoutDeclaration::Height(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MinWidth => {
            DgLayoutDeclaration::MinWidth(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MinHeight => {
            DgLayoutDeclaration::MinHeight(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MaxWidth => {
            DgLayoutDeclaration::MaxWidth(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MaxHeight => {
            DgLayoutDeclaration::MaxHeight(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Padding => {
            DgLayoutDeclaration::Padding(parse_px_box_edges(name, value, variables)?)
        }
        DgLayoutPropertyName::PaddingLeft => {
            DgLayoutDeclaration::PaddingLeft(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::PaddingRight => {
            DgLayoutDeclaration::PaddingRight(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::PaddingTop => {
            DgLayoutDeclaration::PaddingTop(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::PaddingBottom => {
            DgLayoutDeclaration::PaddingBottom(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Margin => {
            let edges = parse_px_box_edges(name, value, variables)?;
            if edges.top != edges.right || edges.right != edges.bottom || edges.bottom != edges.left
            {
                return Err(DgStyleWarning {
                    property: name.to_string(),
                    message: "only uniform margin values are supported in DragonGUI CSS V1"
                        .to_string(),
                });
            }
            DgLayoutDeclaration::Margin(edges)
        }
        DgLayoutPropertyName::Gap => {
            DgLayoutDeclaration::Gap(parse_px_length_value(name, value, variables)?)
        }
    };
    Ok(Some(DgStyleProperty::Layout(declaration)))
}

fn lower_visual(
    name: &str,
    property: DgVisualPropertyName,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgStyleProperty>, DgStyleWarning> {
    let declaration = match property {
        DgVisualPropertyName::Background => {
            DgVisualDeclaration::Background(parse_color_value(name, value, variables)?)
        }
        DgVisualPropertyName::Foreground => {
            DgVisualDeclaration::Foreground(parse_color_value(name, value, variables)?)
        }
        DgVisualPropertyName::BorderColor => {
            DgVisualDeclaration::BorderColor(parse_color_value(name, value, variables)?)
        }
        DgVisualPropertyName::BorderWidth => {
            DgVisualDeclaration::BorderWidth(parse_px_length_value(name, value, variables)?)
        }
        DgVisualPropertyName::BorderRadius => {
            DgVisualDeclaration::BorderRadius(parse_px_length_value(name, value, variables)?)
        }
        DgVisualPropertyName::BorderTopLeftRadius => {
            DgVisualDeclaration::BorderTopLeftRadius(parse_px_length_value(name, value, variables)?)
        }
        DgVisualPropertyName::BorderTopRightRadius => DgVisualDeclaration::BorderTopRightRadius(
            parse_px_length_value(name, value, variables)?,
        ),
        DgVisualPropertyName::BorderBottomRightRadius => {
            DgVisualDeclaration::BorderBottomRightRadius(parse_px_length_value(
                name, value, variables,
            )?)
        }
        DgVisualPropertyName::BorderBottomLeftRadius => {
            DgVisualDeclaration::BorderBottomLeftRadius(parse_px_length_value(
                name, value, variables,
            )?)
        }
        DgVisualPropertyName::Opacity => {
            DgVisualDeclaration::Opacity(parse_number_value(name, value, variables)?)
        }
        DgVisualPropertyName::Accent => {
            DgVisualDeclaration::Accent(parse_color_value(name, value, variables)?)
        }
        DgVisualPropertyName::TrackColor => {
            DgVisualDeclaration::TrackColor(parse_color_value(name, value, variables)?)
        }
        DgVisualPropertyName::ThumbColor => {
            DgVisualDeclaration::ThumbColor(parse_color_value(name, value, variables)?)
        }
    };
    Ok(Some(DgStyleProperty::Visual(declaration)))
}

fn lower_text(
    name: &str,
    property: DgTextPropertyName,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgStyleProperty>, DgStyleWarning> {
    let declaration = match property {
        DgTextPropertyName::FontSize => {
            DgTextDeclaration::FontSize(parse_px_length_value(name, value, variables)?)
        }
        DgTextPropertyName::FontFamily => {
            DgTextDeclaration::FontFamily(unquote(resolve_keyword(value, variables).as_str()))
        }
        DgTextPropertyName::FontWeight => {
            DgTextDeclaration::FontWeight(parse_font_weight_value(name, value, variables)?)
        }
        DgTextPropertyName::Color => {
            DgTextDeclaration::Color(parse_color_value(name, value, variables)?)
        }
        DgTextPropertyName::TextAlign => {
            DgTextDeclaration::TextAlign(DgCssKeyword(resolve_keyword(value, variables)))
        }
    };
    Ok(Some(DgStyleProperty::Text(declaration)))
}

fn lower_widget(
    name: &str,
    property: DgWidgetPropertyName,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgStyleProperty>, DgStyleWarning> {
    let declaration = match property {
        DgWidgetPropertyName::TableRowHeight => {
            DgWidgetDeclaration::TableRowHeight(parse_px_length_value(name, value, variables)?)
        }
        DgWidgetPropertyName::TableHeaderHeight => {
            DgWidgetDeclaration::TableHeaderHeight(parse_px_length_value(name, value, variables)?)
        }
    };
    Ok(Some(DgStyleProperty::Widget(declaration)))
}

fn parse_selector(selector: &str, warnings: &mut Vec<DgStyleWarning>) -> Option<DgSelector> {
    let selector = selector.trim();
    if selector == ":root" {
        return Some(DgSelector::Root);
    }
    let parts: Vec<&str> = selector.split('>').map(str::trim).collect();
    if parts.len() == 1 {
        parse_compound_selector(parts[0])
            .map(DgSelector::Compound)
            .or_else(|| {
                warnings.push(DgStyleWarning {
                    property: selector.to_string(),
                    message: "unsupported selector for DragonGUI CSS subset".to_string(),
                });
                None
            })
    } else if parts.len() == 2 {
        let Some(parent) = parse_compound_selector(parts[0]) else {
            warnings.push(DgStyleWarning {
                property: selector.to_string(),
                message: "unsupported parent selector for DragonGUI CSS subset".to_string(),
            });
            return None;
        };
        if parent.part.is_some() {
            warnings.push(DgStyleWarning {
                property: selector.to_string(),
                message: "part selectors are only supported on the target widget".to_string(),
            });
            return None;
        }
        let Some(child) = parse_compound_selector(parts[1]) else {
            warnings.push(DgStyleWarning {
                property: selector.to_string(),
                message: "unsupported child selector for DragonGUI CSS subset".to_string(),
            });
            return None;
        };
        Some(DgSelector::Child {
            parent: Box::new(DgSelector::Compound(parent)),
            child,
        })
    } else {
        warnings.push(DgStyleWarning {
            property: selector.to_string(),
            message: "only direct child selectors are supported".to_string(),
        });
        None
    }
}

fn parse_compound_selector(selector: &str) -> Option<DgCompoundSelector> {
    let selector = selector.trim();
    if selector.is_empty() || selector.contains(' ') || selector.contains('[') {
        return None;
    }
    let (selector, part) = match selector.split_once("::") {
        Some((target, part)) => {
            if target.is_empty() || part.contains("::") || !is_part_name(part) {
                return None;
            }
            (target, Some(part.to_string()))
        }
        None => (selector, None),
    };
    let mut compound = DgCompoundSelector::new();
    compound.part = part;
    let mut rest = selector;

    if let Some(type_len) = rest
        .find(['.', '#', ':'])
        .or_else(|| (!rest.is_empty()).then_some(rest.len()))
    {
        if type_len > 0 {
            let type_name = &rest[..type_len];
            compound.type_selector = Some(widget_kind_from_css_type(type_name)?);
            rest = &rest[type_len..];
        }
    }

    while !rest.is_empty() {
        let (prefix, tail) = rest.split_at(1);
        let next = tail
            .find(['.', '#', ':'])
            .map(|idx| idx + 1)
            .unwrap_or(rest.len());
        let value = &rest[1..next];
        if value.is_empty() {
            return None;
        }
        match prefix {
            "." => compound.classes.push(value.to_string()),
            "#" => compound.id = Some(value.to_string()),
            ":" => compound.pseudo.push(parse_pseudo(value)?),
            _ => return None,
        }
        rest = &rest[next..];
    }
    Some(compound)
}

fn is_part_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn parse_pseudo(value: &str) -> Option<DgPseudoClass> {
    match value {
        "hover" => Some(DgPseudoClass::Hover),
        "active" => Some(DgPseudoClass::Active),
        "focus" => Some(DgPseudoClass::Focus),
        "disabled" => Some(DgPseudoClass::Disabled),
        "checked" => Some(DgPseudoClass::Checked),
        _ => None,
    }
}

fn split_selector_list(selector: &str) -> Vec<String> {
    selector
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn split_declaration(declaration: &str) -> Option<(&str, &str)> {
    let declaration = declaration.trim().trim_end_matches(';').trim();
    let (name, value) = declaration.split_once(':')?;
    let value = value.trim().trim_end_matches("!important").trim();
    Some((name.trim(), value))
}

fn parse_css_value(value: &str, variables: &BTreeMap<String, DgCssValue>) -> Option<DgCssValue> {
    if let Some(resolved) = resolve_variable(value, variables) {
        return Some(resolved);
    }
    if let Ok(number) = value.trim().parse::<f32>() {
        return Some(DgCssValue::Number(DgCssNumber(number)));
    }
    if let Some(length) = parse_length(value) {
        return Some(DgCssValue::Length(length));
    }
    if let Some(color) = parse_color(value) {
        return Some(DgCssValue::Color(color));
    }
    let value = value.trim();
    if value.is_empty() {
        None
    } else if is_quoted(value) {
        Some(DgCssValue::String(unquote(value)))
    } else {
        Some(DgCssValue::Keyword(DgCssKeyword(value.to_string())))
    }
}

fn resolve_variable(value: &str, variables: &BTreeMap<String, DgCssValue>) -> Option<DgCssValue> {
    let value = value.trim();
    let inner = value.strip_prefix("var(")?.strip_suffix(')')?.trim();
    variables.get(inner).cloned()
}

fn resolve_keyword(value: &str, variables: &BTreeMap<String, DgCssValue>) -> String {
    match resolve_variable(value, variables) {
        Some(DgCssValue::Keyword(keyword)) => keyword.0,
        Some(DgCssValue::String(value)) => value,
        Some(DgCssValue::Color(DgCssColor::Token(token))) => token,
        _ => value.trim().to_string(),
    }
}

fn parse_number_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssNumber, DgStyleWarning> {
    match resolve_variable(value, variables) {
        Some(DgCssValue::Number(number)) => return Ok(number),
        Some(DgCssValue::Length(DgCssLength::LogicalPx(value))) => return Ok(DgCssNumber(value)),
        Some(_) => return Err(parse_warning(name, value, "number")),
        None => {}
    }
    value
        .trim()
        .parse::<f32>()
        .map(DgCssNumber)
        .map_err(|_| parse_warning(name, value, "number"))
}

fn parse_font_weight_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<u16, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    match value.trim() {
        "normal" => Ok(400),
        "bold" => Ok(700),
        value => value
            .parse::<u16>()
            .map_err(|_| parse_warning(name, value, "font weight")),
    }
}

fn parse_length_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssLength, DgStyleWarning> {
    match resolve_variable(value, variables) {
        Some(DgCssValue::Length(length)) => return Ok(length),
        Some(DgCssValue::Number(number)) => return Ok(DgCssLength::LogicalPx(number.0)),
        Some(_) => return Err(parse_warning(name, value, "length")),
        None => {}
    }
    parse_length(value).ok_or_else(|| parse_warning(name, value, "length"))
}

fn parse_px_length_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssLength, DgStyleWarning> {
    let length = parse_length_value(name, value, variables)?;
    require_logical_px(name, value, length)
}

fn require_logical_px(
    name: &str,
    source: &str,
    length: DgCssLength,
) -> Result<DgCssLength, DgStyleWarning> {
    match length {
        DgCssLength::LogicalPx(_) => Ok(length),
        DgCssLength::Percent(_) => Err(DgStyleWarning {
            property: name.to_string(),
            message: format!(
                "percentage lengths are not supported for {name:?} in DragonGUI CSS V1: {source:?}"
            ),
        }),
        DgCssLength::Auto => Err(DgStyleWarning {
            property: name.to_string(),
            message: format!("`auto` lengths are not supported for {name:?} in DragonGUI CSS V1"),
        }),
    }
}

fn parse_box_edges(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgBoxEdges<DgCssLength>, DgStyleWarning> {
    if let Some(DgCssValue::Length(length)) = resolve_variable(value, variables) {
        return Ok(DgBoxEdges {
            top: length.clone(),
            right: length.clone(),
            bottom: length.clone(),
            left: length,
        });
    }
    let values = split_value_tokens(value);
    if values.is_empty() || values.len() > 4 {
        return Err(parse_warning(name, value, "box shorthand"));
    }
    let parsed: Result<Vec<_>, _> = values
        .iter()
        .map(|part| parse_length_value(name, part, variables))
        .collect();
    let parsed = parsed?;
    let top = parsed[0].clone();
    let right = parsed.get(1).cloned().unwrap_or_else(|| top.clone());
    let bottom = parsed.get(2).cloned().unwrap_or_else(|| top.clone());
    let left = parsed.get(3).cloned().unwrap_or_else(|| right.clone());
    Ok(DgBoxEdges {
        top,
        right,
        bottom,
        left,
    })
}

fn parse_px_box_edges(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgBoxEdges<DgCssLength>, DgStyleWarning> {
    let edges = parse_box_edges(name, value, variables)?;
    Ok(DgBoxEdges {
        top: require_logical_px(name, value, edges.top)?,
        right: require_logical_px(name, value, edges.right)?,
        bottom: require_logical_px(name, value, edges.bottom)?,
        left: require_logical_px(name, value, edges.left)?,
    })
}

fn parse_color_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssColor, DgStyleWarning> {
    match resolve_variable(value, variables) {
        Some(DgCssValue::Color(color)) => return Ok(color),
        Some(DgCssValue::Keyword(keyword)) => return Ok(DgCssColor::Token(keyword.0)),
        Some(DgCssValue::String(value)) => return Ok(DgCssColor::Token(value)),
        Some(_) => return Err(parse_warning(name, value, "color")),
        None => {}
    }
    parse_color(value).ok_or_else(|| parse_warning(name, value, "color"))
}

fn parse_color(value: &str) -> Option<DgCssColor> {
    let value = value.trim();
    if value.starts_with('#') {
        parse_css_hex_color(value).map(DgCssColor::Rgba)
    } else if is_identifier_like(value) {
        Some(DgCssColor::Token(value.to_string()))
    } else {
        None
    }
}

fn parse_css_hex_color(value: &str) -> Option<Color> {
    let hex = value.trim_start_matches('#');
    if hex.len() == 3 {
        let mut expanded = String::with_capacity(7);
        expanded.push('#');
        for ch in hex.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        return parse_hex_color(&expanded);
    }
    if hex.len() == 4 {
        let mut expanded = String::with_capacity(9);
        expanded.push('#');
        for ch in hex.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        return parse_css_hex_color(&expanded);
    }
    if hex.len() == 8 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
        return Some([
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        ]);
    }
    parse_hex_color(value)
}

fn parse_length(value: &str) -> Option<DgCssLength> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") {
        return Some(DgCssLength::Auto);
    }
    if let Some(px) = value.strip_suffix("px") {
        return px.trim().parse::<f32>().ok().map(DgCssLength::LogicalPx);
    }
    if let Some(percent) = value.strip_suffix('%') {
        return percent.trim().parse::<f32>().ok().map(DgCssLength::Percent);
    }
    value.parse::<f32>().ok().map(DgCssLength::LogicalPx)
}

fn parse_border(value: &str, variables: &BTreeMap<String, DgCssValue>) -> Option<DgBorder> {
    let parts = split_value_tokens(value);
    if parts.len() != 3 || !parts[1].eq_ignore_ascii_case("solid") {
        return None;
    }
    Some(DgBorder {
        width: parse_px_length_value("border", parts[0], variables).ok()?,
        style: DgBorderStyle::Solid,
        color: parse_color_value("border", parts[2], variables).ok()?,
    })
}

fn split_value_tokens(value: &str) -> Vec<&str> {
    value
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect()
}

fn parse_warning(name: &str, value: &str, expected: &str) -> DgStyleWarning {
    DgStyleWarning {
        property: name.to_string(),
        message: format!("expected {expected} value for {name:?}, got {value:?}"),
    }
}

fn is_identifier_like(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_quoted(value: &str) -> bool {
    (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_matrix_maps_supported_names() {
        let cases = [
            (
                "flex-direction",
                DgStylePropertyName::Layout(DgLayoutPropertyName::FlexDirection),
            ),
            (
                "background-color",
                DgStylePropertyName::Visual(DgVisualPropertyName::Background),
            ),
            (
                "border-top-right-radius",
                DgStylePropertyName::Visual(DgVisualPropertyName::BorderTopRightRadius),
            ),
            ("border", DgStylePropertyName::BorderShorthand),
            (
                "color",
                DgStylePropertyName::Text(DgTextPropertyName::Color),
            ),
            (
                "table-row-height",
                DgStylePropertyName::Widget(DgWidgetPropertyName::TableRowHeight),
            ),
            (
                "--accent",
                DgStylePropertyName::CustomProperty("--accent".to_string()),
            ),
        ];

        for (name, expected) in cases {
            assert_eq!(DgStylePropertyName::from_css_name(name), Ok(expected));
        }
    }

    #[test]
    fn unsupported_property_returns_warning() {
        let warning = DgStylePropertyName::from_css_name("box-shadow").unwrap_err();
        assert_eq!(warning.property, "box-shadow");
        assert!(warning.message.contains("unsupported"));
    }

    #[test]
    fn selector_specificity_counts_type_class_id_and_pseudo() {
        let selector = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_id("run")
                .with_class("danger")
                .with_class("primary")
                .with_pseudo(DgPseudoClass::Hover),
        );

        assert_eq!(selector.specificity(), Specificity::new(1, 3, 1));
    }

    #[test]
    fn child_selector_specificity_accumulates_parent_and_child() {
        let selector = DgSelector::Child {
            parent: Box::new(DgSelector::Compound(
                DgCompoundSelector::new()
                    .with_type(WidgetKind::Panel)
                    .with_class("controls"),
            )),
            child: DgCompoundSelector::new().with_type(WidgetKind::Button),
        };

        assert_eq!(selector.specificity(), Specificity::new(0, 1, 2));
    }

    #[test]
    fn cascade_key_orders_important_origin_specificity_and_source_order() {
        let low = CascadeKey {
            important: false,
            origin: StylesheetOrigin::Framework,
            specificity: Specificity::new(0, 0, 1),
            source_order: 10,
        };
        let user = CascadeKey {
            origin: StylesheetOrigin::User,
            ..low
        };
        let class_rule = CascadeKey {
            specificity: Specificity::new(0, 1, 0),
            ..user
        };
        let later = CascadeKey {
            source_order: 11,
            ..class_rule
        };
        let important = CascadeKey {
            important: true,
            origin: StylesheetOrigin::Framework,
            specificity: Specificity::ZERO,
            source_order: 0,
        };

        assert!(user > low);
        assert!(class_rule > user);
        assert!(later > class_rule);
        assert!(important > later);
    }

    #[test]
    fn can_construct_rule_without_parser_types() {
        let declaration = DgStyleDeclaration::normal(DgStyleProperty::Visual(
            DgVisualDeclaration::Border(DgBorder {
                width: DgCssLength::LogicalPx(1.0),
                style: DgBorderStyle::Solid,
                color: DgCssColor::Token("border".to_string()),
            }),
        ));
        let selector = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_class("ghost"),
        );
        let rule = DgStyleRule::new(selector, vec![declaration], StylesheetOrigin::User, 7);

        assert_eq!(rule.specificity, Specificity::new(0, 1, 1));
        assert_eq!(rule.declarations.len(), 1);
        assert_eq!(rule.cascade_key(&rule.declarations[0]).source_order, 7);
    }

    #[test]
    fn css_widget_type_names_round_trip_known_widgets() {
        let kinds = [
            WidgetKind::Button,
            WidgetKind::Panel,
            WidgetKind::DataFrameTable,
            WidgetKind::Scatter3D,
            WidgetKind::Image,
        ];

        for kind in kinds {
            let name = css_type_name(kind).expect("known widget should have a CSS name");
            assert_eq!(widget_kind_from_css_type(name), Some(kind));
        }
        assert_eq!(widget_kind_from_css_type("button"), None);
        assert_eq!(css_type_name(WidgetKind::Unknown), None);
    }

    #[test]
    fn split_classes_uses_css_whitespace_model() {
        assert_eq!(
            split_classes(Some("controls primary\tcompact")),
            vec!["controls", "primary", "compact"]
        );
        assert!(split_classes(None).is_empty());
    }

    #[test]
    fn selector_matching_supports_type_class_id_and_pseudo() {
        let classes = ["primary", "danger"];
        let pseudos = [DgPseudoClass::Hover];
        let element = StyleElement {
            id: "run",
            key: None,
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &[],
            pseudo: &pseudos,
        };
        let selector = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_id("run")
                .with_class("primary")
                .with_pseudo(DgPseudoClass::Hover),
        );
        let missing_class = DgSelector::Compound(DgCompoundSelector::new().with_class("secondary"));

        assert!(selector.matches(&element));
        assert!(!missing_class.matches(&element));
    }

    #[test]
    fn selector_matching_supports_direct_child_ancestors() {
        let classes = ["primary"];
        let parent_classes = ["controls"];
        let ancestors = [StyleAncestor {
            id: "controls-panel",
            key: None,
            classes: &parent_classes,
            kind: WidgetKind::Panel,
        }];
        let element = StyleElement {
            id: "run",
            key: None,
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &ancestors,
            pseudo: &[],
        };
        let selector = DgSelector::Child {
            parent: Box::new(DgSelector::Compound(
                DgCompoundSelector::new()
                    .with_type(WidgetKind::Panel)
                    .with_class("controls"),
            )),
            child: DgCompoundSelector::new().with_type(WidgetKind::Button),
        };
        let wrong_parent = DgSelector::Child {
            parent: Box::new(DgSelector::Compound(
                DgCompoundSelector::new().with_type(WidgetKind::Sidebar),
            )),
            child: DgCompoundSelector::new().with_type(WidgetKind::Button),
        };

        assert!(selector.matches(&element));
        assert!(!wrong_parent.matches(&element));
    }

    #[test]
    fn matched_rule_labels_reports_selectors_per_widget() {
        let tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "panel",
                "type": "panel",
                "class": "controls",
                "children": [{
                    "id": "run",
                    "type": "button",
                    "class": "primary",
                    "props": {"text": "Run"}
                }]
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { border-radius: 4px; }
                .primary { background: accent; }
                Panel.controls > Button { border-width: 2px; }
                Button:hover { background: accent_mix_20; }
                "#,
            )
            .unwrap();

        let labels = matched_rule_labels_for_tree(&tree, &store);
        let button_labels = labels.get("run").expect("button should have CSS matches");

        assert!(button_labels.contains(&"user: Button".to_string()));
        assert!(button_labels.contains(&"user: .primary".to_string()));
        assert!(button_labels.contains(&"user: Panel.controls > Button".to_string()));
        assert!(button_labels.contains(&"user: Button:hover".to_string()));
    }

    #[test]
    fn parses_valid_dragongui_css_into_internal_rules() {
        let parsed = parse_stylesheet(
            "Button { border-radius: 4px; padding: 6px 10px; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert_eq!(parsed.rules.len(), 1);
        assert_eq!(
            parsed.rules[0].selector,
            DgSelector::Compound(DgCompoundSelector::new().with_type(WidgetKind::Button))
        );
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderRadius(DgCssLength::LogicalPx(
                    4.0
                )))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Padding(_))
            )
        }));
    }

    #[test]
    fn parses_per_corner_radius_declarations() {
        let parsed = parse_stylesheet(
            "NumberInput { border-radius: 8px; border-top-right-radius: 12px; border-bottom-right-radius: 4px; }",
            StylesheetOrigin::User,
        )
        .unwrap();
        let declarations = &parsed.rules[0].declarations;

        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderTopRightRadius(
                    DgCssLength::LogicalPx(12.0)
                ))
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderBottomRightRadius(
                    DgCssLength::LogicalPx(4.0)
                ))
            )
        }));
    }

    #[test]
    fn parses_widget_part_selectors() {
        let parsed = parse_stylesheet(
            r#"
            NumberInput::stepper { width: 34px; background: surface_alt; }
            Panel.controls > NumberInput:hover::stepper-up { color: accent; }
            .numeric::stepper-down { border-bottom-right-radius: 10px; }
            Checkbox:checked::indicator { background: accent; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert_eq!(
            parsed.rules[0].selector,
            DgSelector::Compound(
                DgCompoundSelector::new()
                    .with_type(WidgetKind::NumberInput)
                    .with_part("stepper")
            )
        );
        assert_eq!(parsed.rules[0].selector.target_part(), Some("stepper"));
        assert_eq!(parsed.rules[0].specificity, Specificity::new(0, 0, 1));
        assert_eq!(parsed.rules[1].selector.target_part(), Some("stepper-up"));
        assert_eq!(parsed.rules[1].specificity, Specificity::new(0, 2, 2));
        assert_eq!(parsed.rules[2].selector.target_part(), Some("stepper-down"));
        assert_eq!(parsed.rules[3].selector.target_part(), Some("indicator"));
        assert_eq!(
            parsed.rules[3].selector.target_pseudo_classes(),
            &[DgPseudoClass::Checked]
        );
    }

    #[test]
    fn rejects_invalid_css_with_useful_error() {
        let error = parse_stylesheet("Button > > Label { color: red; }", StylesheetOrigin::User)
            .unwrap_err();
        assert!(error.message.contains("failed to parse"));
    }

    #[test]
    fn root_variables_resolve_before_normal_declarations() {
        let parsed = parse_stylesheet(
            r#"
            :root { --radius: 4px; --brand: #ff6b35; }
            Button { border-radius: var(--radius); background: var(--brand); }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert_eq!(
            parsed.variables.get("--radius"),
            Some(&DgCssValue::Length(DgCssLength::LogicalPx(4.0)))
        );
        let declarations = &parsed.rules[0].declarations;
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderRadius(DgCssLength::LogicalPx(
                    4.0
                )))
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::Background(DgCssColor::Rgba(_)))
            )
        }));
    }

    #[test]
    fn cascade_applies_table_widget_declarations() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "table",
                "type": "dataframe_table",
                "props": {
                    "frame": {"columns": ["x"], "dtypes": ["f32"], "rows": 2},
                    "page_size": 20
                }
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                "DataFrameTable { table-row-height: 22px; table-header-height: 26px; }",
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let table = &tree.children[0];

        assert_eq!(table.style.widget.table_row_height, Some(22.0));
        assert_eq!(table.style.widget.table_header_height, Some(26.0));
    }

    #[test]
    fn stylesheet_cascade_applies_table_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "table",
                "type": "dataframe_table",
                "props": {
                    "frame": {"columns": ["x"], "dtypes": ["f32"], "rows": 2},
                    "page_size": 20
                }
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                DataFrameTable::header {
                    background: surface_alt;
                    color: text;
                    font-weight: 700;
                }
                DataFrameTable::row {
                    background: surface;
                    color: muted_text;
                }
                DataFrameTable::row-selected {
                    background: accent;
                    color: text;
                }
                DataFrameTable::grid-line {
                    background: border;
                    width: 2px;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let table = &tree.children[0];
        let header = table.style.parts.parts.get("header").unwrap();
        let row = table.style.parts.parts.get("row").unwrap();
        let selected = table.style.parts.parts.get("row-selected").unwrap();
        let grid = table.style.parts.parts.get("grid-line").unwrap();

        assert_eq!(
            header.visual.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(header.text.color, Some(ColorRef::Token("text".to_string())));
        assert_eq!(header.text.font_weight, Some(700));
        assert_eq!(
            row.text.color,
            Some(ColorRef::Token("muted_text".to_string()))
        );
        assert_eq!(
            selected.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            selected.text.color,
            Some(ColorRef::Token("text".to_string()))
        );
        assert_eq!(
            grid.visual.background,
            Some(ColorRef::Token("border".to_string()))
        );
        assert_eq!(grid.layout.width, Some(2.0));
    }

    #[test]
    fn border_shorthand_accepts_theme_token_color() {
        let parsed = parse_stylesheet(
            "Button { border: 1px solid border; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::Border(DgBorder {
                    width: DgCssLength::LogicalPx(1.0),
                    style: DgBorderStyle::Solid,
                    color: DgCssColor::Token(token),
                })) if token == "border"
            )
        }));
    }

    #[test]
    fn unsupported_properties_are_reported_as_warnings() {
        let parsed = parse_stylesheet(
            "Button { box-shadow: 0 0 4px red; border-radius: 4px; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "box-shadow"));
        assert_eq!(parsed.rules.len(), 1);
    }

    #[test]
    fn unsupported_css_lengths_are_reported_as_warnings() {
        let parsed = parse_stylesheet(
            "Button { width: 50%; height: auto; border-radius: 4px; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "width"
                && warning.message.contains("percentage lengths")));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "height" && warning.message.contains("auto")));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderRadius(DgCssLength::LogicalPx(
                    4.0
                )))
            )
        }));
    }

    #[test]
    fn asymmetric_margin_is_reported_as_warning() {
        let parsed = parse_stylesheet(
            "Panel { margin: 10px 20px; padding: 4px 8px; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "margin"
                && warning.message.contains("uniform margin")));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Padding(_))
            )
        }));
    }

    #[test]
    fn unsupported_child_selector_parts_are_reported() {
        let parsed = parse_stylesheet(
            "Panel > [key=\"run\"] { color: text; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.rules.is_empty());
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "Panel > [key=\"run\"]"
                && warning.message.contains("unsupported child selector")));
    }

    #[test]
    fn css_hex_alpha_forms_parse_to_rgba() {
        let parsed = parse_stylesheet(
            "Button { background: #ff6b3580; border-color: #0f08; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::Background(DgCssColor::Rgba(color)))
                    if (color[3] - (0x80 as f32 / 255.0)).abs() < 0.001
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderColor(DgCssColor::Rgba(color)))
                    if (color[3] - (0x88 as f32 / 255.0)).abs() < 0.001
            )
        }));
    }

    #[test]
    fn stylesheet_store_layers_and_clears_origins() {
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::Framework,
                ":root { --radius: 2px; } Button { border-radius: var(--radius); }",
            )
            .unwrap();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                ":root { --radius: 6px; } .primary { background: accent; }",
            )
            .unwrap();

        assert_eq!(store.rules(StylesheetOrigin::Framework).len(), 1);
        assert_eq!(store.rules(StylesheetOrigin::User).len(), 1);
        assert_eq!(store.all_rules().len(), 2);
        assert_eq!(
            store.variables().get("--radius"),
            Some(&DgCssValue::Length(DgCssLength::LogicalPx(6.0)))
        );

        store.clear(StylesheetOrigin::User);
        assert!(store.rules(StylesheetOrigin::User).is_empty());
        assert_eq!(
            store.variables().get("--radius"),
            Some(&DgCssValue::Length(DgCssLength::LogicalPx(2.0)))
        );
    }

    #[test]
    fn framework_defaults_install_and_remain_lower_precedence_than_user_css() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "run",
                "type": "button",
                "props": {"text": "Run"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store.install_framework_defaults(&Theme::dark());

        assert!(!store.rules(StylesheetOrigin::Framework).is_empty());
        assert!(store.warnings().is_empty());

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let button = &tree.children[0];
        assert_eq!(
            button.style.visual.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(
            button.style.visual.border_radius,
            Some(Theme::dark().radius)
        );

        store
            .set_stylesheet(
                StylesheetOrigin::User,
                "Button { border-radius: 0px; background: danger; }",
            )
            .unwrap();
        apply_stylesheets_to_tree(&mut tree, &mut store);
        let button = &tree.children[0];
        assert_eq!(button.style.visual.border_radius, Some(0.0));
        assert_eq!(
            button.style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn stylesheet_cascade_applies_to_widget_tree_with_inline_override() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "props": {"title": "CSS", "width": 320, "height": 240},
            "children": [{
                "id": "panel",
                "type": "panel",
                "class": "controls",
                "children": [{
                    "id": "run",
                    "type": "button",
                    "class": "primary",
                    "props": {"text": "Run"},
                    "style": {"border_radius": 3}
                }]
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { width: 120px; border-radius: 4px; }
                .primary { background: #336699; border-radius: 6px; }
                Panel.controls > Button { border-width: 2px; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let button = &tree.children[0].children[0];

        assert_eq!(button.style.layout.width, Some(120.0));
        assert_eq!(button.style.visual.border_width, Some(2.0));
        assert_eq!(
            button.style.visual.background,
            Some(ColorRef::Rgba([
                0x33 as f32 / 255.0,
                0x66 as f32 / 255.0,
                0x99 as f32 / 255.0,
                1.0
            ]))
        );
        assert_eq!(button.style.visual.border_radius, Some(3.0));
    }

    #[test]
    fn stylesheet_cascade_applies_per_corner_radius() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "number",
                "type": "number_input"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                NumberInput {
                    border-radius: 8px;
                    border-top-right-radius: 12px;
                    border-bottom-right-radius: 4px;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let number = &tree.children[0];

        assert_eq!(number.style.visual.border_radius, Some(8.0));
        assert_eq!(number.style.visual.corner_radii.top_left, None);
        assert_eq!(number.style.visual.corner_radii.top_right, Some(12.0));
        assert_eq!(number.style.visual.corner_radii.bottom_right, Some(4.0));
        assert_eq!(
            number.style.visual.corner_radii.resolve(8.0),
            [8.0, 12.0, 4.0, 8.0]
        );
    }

    #[test]
    fn stylesheet_cascade_applies_widget_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "number",
                "type": "number_input",
                "class": "numeric"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                NumberInput::stepper {
                    width: 34px;
                    background: surface_alt;
                    border-top-right-radius: 10px;
                }
                .numeric:hover::stepper-up {
                    color: accent;
                    width: 99px;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let number = &tree.children[0];
        let stepper = number
            .style
            .parts
            .parts
            .get("stepper")
            .expect("base stepper part should be styled");
        let hover_up = number
            .style
            .parts
            .hover
            .get("stepper-up")
            .expect("hover stepper-up part should be styled");

        assert_eq!(number.style.visual.background, None);
        assert_eq!(stepper.layout.width, Some(34.0));
        assert_eq!(
            stepper.visual.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(stepper.visual.corner_radii.top_right, Some(10.0));
        assert_eq!(
            hover_up.text.color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            hover_up.layout.width, None,
            "stateful part layout changes are ignored"
        );
        let warnings = store.warnings();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].property, ".numeric:hover::stepper-up");
        assert!(warnings[0]
            .message
            .contains("width on .numeric:hover::stepper-up is ignored"));
    }

    #[test]
    fn stylesheet_cascade_applies_checked_checkbox_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "accept",
                "type": "checkbox"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Checkbox::box {
                    border-radius: 2px;
                    background: surface_alt;
                }
                Checkbox:checked::indicator {
                    background: accent;
                    width: 9px;
                }
                Checkbox::label {
                    color: muted_text;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let checkbox = &tree.children[0];
        let box_style = checkbox.style.parts.parts.get("box").unwrap();
        let indicator_style = checkbox.style.parts.checked.get("indicator").unwrap();
        let label_style = checkbox.style.parts.parts.get("label").unwrap();

        assert_eq!(box_style.visual.border_radius, Some(2.0));
        assert_eq!(
            box_style.visual.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(
            indicator_style.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(indicator_style.layout.width, None);
        assert_eq!(
            label_style.text.color,
            Some(ColorRef::Token("muted_text".to_string()))
        );
    }

    #[test]
    fn stylesheet_cascade_applies_slider_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "gain",
                "type": "slider"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Slider::track {
                    height: 7px;
                    background: border;
                }
                Slider::fill {
                    background: accent;
                }
                Slider::thumb {
                    width: 20px;
                    height: 18px;
                    border-color: surface;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let slider = &tree.children[0];
        let track = slider.style.parts.parts.get("track").unwrap();
        let fill = slider.style.parts.parts.get("fill").unwrap();
        let thumb = slider.style.parts.parts.get("thumb").unwrap();

        assert_eq!(track.layout.height, Some(7.0));
        assert_eq!(
            track.visual.background,
            Some(ColorRef::Token("border".to_string()))
        );
        assert_eq!(
            fill.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(thumb.layout.width, Some(20.0));
        assert_eq!(thumb.layout.height, Some(18.0));
        assert_eq!(
            thumb.visual.border_color,
            Some(ColorRef::Token("surface".to_string()))
        );
    }

    #[test]
    fn stylesheet_cascade_applies_progress_bar_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "progress",
                "type": "progress_bar"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                ProgressBar::track {
                    background: surface_alt;
                    border-radius: 4px;
                }
                ProgressBar::fill {
                    background: success;
                    height: 8px;
                }
                ProgressBar::label {
                    color: text;
                    font-weight: 700;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let progress = &tree.children[0];
        let track = progress.style.parts.parts.get("track").unwrap();
        let fill = progress.style.parts.parts.get("fill").unwrap();
        let label = progress.style.parts.parts.get("label").unwrap();

        assert_eq!(
            track.visual.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(track.visual.border_radius, Some(4.0));
        assert_eq!(
            fill.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(fill.layout.height, Some(8.0));
        assert_eq!(label.text.color, Some(ColorRef::Token("text".to_string())));
        assert_eq!(label.text.font_weight, Some(700));
    }

    #[test]
    fn stylesheet_cascade_applies_tabs_and_navigation_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "tabs",
                "type": "tabs",
                "children": [{
                    "id": "tab-a",
                    "type": "tab",
                    "props": {"label": "Alpha", "value": "alpha"}
                }]
            }, {
                "id": "nav-a",
                "type": "nav_item",
                "props": {"label": "Alpha", "page": "alpha"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Tabs::header {
                    background: surface;
                    border-color: accent;
                    height: 42px;
                }
                Tab::tab {
                    background: surface_alt;
                    color: text;
                    padding: 14px;
                }
                Tab::accent {
                    background: warning;
                    height: 5px;
                    border-radius: 2px;
                }
                NavItem::item {
                    background: surface_alt;
                    color: muted_text;
                    padding: 18px;
                }
                NavItem::accent {
                    background: success;
                    width: 6px;
                    border-radius: 3px;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let tabs = &tree.children[0];
        let tab = &tabs.children[0];
        let nav = &tree.children[1];
        let header = tabs.style.parts.parts.get("header").unwrap();
        let tab_body = tab.style.parts.parts.get("tab").unwrap();
        let tab_accent = tab.style.parts.parts.get("accent").unwrap();
        let nav_item = nav.style.parts.parts.get("item").unwrap();
        let nav_accent = nav.style.parts.parts.get("accent").unwrap();

        assert_eq!(
            header.visual.background,
            Some(ColorRef::Token("surface".to_string()))
        );
        assert_eq!(header.layout.height, Some(42.0));
        assert_eq!(
            tab_body.visual.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(
            tab_body.text.color,
            Some(ColorRef::Token("text".to_string()))
        );
        assert_eq!(tab_body.layout.padding, Some(14.0));
        assert_eq!(
            tab_accent.visual.background,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_eq!(tab_accent.layout.height, Some(5.0));
        assert_eq!(
            nav_item.text.color,
            Some(ColorRef::Token("muted_text".to_string()))
        );
        assert_eq!(nav_item.layout.padding, Some(18.0));
        assert_eq!(
            nav_accent.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(nav_accent.layout.width, Some(6.0));
        assert_eq!(nav_accent.visual.border_radius, Some(3.0));
    }

    #[test]
    fn stylesheet_cascade_applies_panel_accent_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "class": "quiet"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel::accent {
                    width: 6px;
                    background: border;
                    border-radius: 2px;
                }
                Panel.quiet::accent {
                    width: 0px;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);

        let accent = tree.children[0].style.parts.parts.get("accent").unwrap();
        assert_eq!(accent.layout.width, Some(0.0));
        assert_eq!(
            accent.visual.background,
            Some(ColorRef::Token("border".to_string()))
        );
        assert_eq!(accent.visual.border_radius, Some(2.0));
        assert!(store.warnings().is_empty());
    }

    #[test]
    fn unsupported_widget_parts_warn_at_cascade_time() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "run",
                "type": "button",
                "class": "numeric"
            }, {
                "id": "amount",
                "type": "number_input",
                "class": "numeric"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                ".numeric::stepper { background: accent; }",
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);

        let warnings = store.warnings();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("Button has no CSS part"));
        assert!(tree.children[0].style.parts.parts.get("stepper").is_none());
        assert_eq!(
            tree.children[1]
                .style
                .parts
                .parts
                .get("stepper")
                .and_then(|style| style.visual.background.clone()),
            Some(ColorRef::Token("accent".to_string()))
        );
    }

    #[test]
    fn inline_style_cache_survives_stylesheet_reapply() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "run",
                "type": "button",
                "props": {"text": "Run"},
                "style": {"border_radius": 3}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                "Button { border-radius: 9px; background: accent; }",
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        assert_eq!(tree.children[0].style.visual.border_radius, Some(3.0));

        store
            .set_stylesheet(
                StylesheetOrigin::User,
                "Button { border-radius: 12px; background: danger; }",
            )
            .unwrap();
        apply_stylesheets_to_tree(&mut tree, &mut store);
        let button = &tree.children[0];

        assert_eq!(button.inline_style.visual.border_radius, Some(3.0));
        assert_eq!(button.style.visual.border_radius, Some(3.0));
        assert_eq!(
            button.style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn inline_part_style_overrides_stylesheet_part_rules() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "amount",
                "type": "number_input",
                "style": {
                    "parts": {
                        "stepper": {
                            "background": "danger",
                            "width": 40
                        },
                        "stepper_up": {
                            "color": "success"
                        }
                    }
                }
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                NumberInput::stepper {
                    background: surface_alt;
                    width: 28px;
                }
                NumberInput::stepper-up {
                    color: accent;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let number = &tree.children[0];
        let stepper = number.style.parts.parts.get("stepper").unwrap();
        let stepper_up = number.style.parts.parts.get("stepper-up").unwrap();

        assert_eq!(
            stepper.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
        assert_eq!(stepper.layout.width, Some(40.0));
        assert_eq!(
            stepper_up.text.color,
            Some(ColorRef::Token("success".to_string()))
        );
    }

    #[test]
    fn unsupported_inline_part_styles_warn_and_are_ignored() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "run",
                "type": "button",
                "style": {
                    "parts": {
                        "stepper": {"background": "danger"}
                    }
                }
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();

        apply_stylesheets_to_tree(&mut tree, &mut store);

        let warnings = store.warnings();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("Button has no CSS part"));
        assert!(warnings[0]
            .message
            .contains("inline base part style ignored"));
        assert!(tree.children[0].style.parts.parts.get("stepper").is_none());
    }

    #[test]
    fn stylesheet_cascade_applies_pseudo_state_rules_to_visual_slots() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "props": {"title": "CSS", "width": 320, "height": 240},
            "children": [{
                "id": "run",
                "type": "button",
                "props": {"text": "Run"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                "Button:hover { background: accent; color: text; }",
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let button = &tree.children[0];

        assert_eq!(
            button.style.hover.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            button.style.hover.foreground,
            Some(ColorRef::Token("text".to_string()))
        );
    }

    #[test]
    fn stylesheet_cascade_inherits_text_properties_to_children() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "props": {"title": "CSS", "width": 320, "height": 240},
            "children": [{
                "id": "panel",
                "type": "panel",
                "children": [{
                    "id": "label",
                    "type": "label",
                    "props": {"text": "Inherited"}
                }, {
                    "id": "button",
                    "type": "button",
                    "props": {"text": "Override"}
                }]
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                "Panel { color: muted_text; font-size: 18px; } Button { color: text; }",
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let panel = &tree.children[0];
        let label = &panel.children[0];
        let button = &panel.children[1];

        assert_eq!(
            label.style.text.color,
            Some(ColorRef::Token("muted_text".to_string()))
        );
        assert_eq!(label.style.text.font_size, Some(18.0));
        assert_eq!(
            button.style.text.color,
            Some(ColorRef::Token("text".to_string()))
        );
        assert_eq!(button.style.text.font_size, Some(18.0));
    }
}
