//! DragonGUI-owned CSS style IR.
//!
//! Parser dependencies such as `lightningcss` must lower into these types
//! immediately. Selector matching, cascade resolution, computed styles, and
//! renderer integration should not depend on parser-specific AST types.

use crate::document::WidgetKind;
use crate::theme::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StylesheetOrigin {
    Framework,
    Theme,
    User,
    Inline,
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DgCompoundSelector {
    pub type_selector: Option<WidgetKind>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub pseudo: Vec<DgPseudoClass>,
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

    pub fn specificity(&self) -> Specificity {
        Specificity {
            ids: u16::from(self.id.is_some()),
            classes: (self.classes.len() + self.pseudo.len()).min(u16::MAX as usize) as u16,
            types: u16::from(self.type_selector.is_some()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgPseudoClass {
    Hover,
    Active,
    Focus,
    Disabled,
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
    /// border-width, border-radius, border, opacity, accent, track-color,
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
        "Modal" => Some(WidgetKind::Modal),
        "MenuBar" => Some(WidgetKind::MenuBar),
        "Menu" => Some(WidgetKind::Menu),
        "MenuItem" => Some(WidgetKind::MenuItem),
        "ContextMenu" => Some(WidgetKind::ContextMenu),
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
        WidgetKind::Modal => Some("Modal"),
        WidgetKind::MenuBar => Some("MenuBar"),
        WidgetKind::Menu => Some("Menu"),
        WidgetKind::MenuItem => Some("MenuItem"),
        WidgetKind::ContextMenu => Some("ContextMenu"),
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
}
