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
    BackgroundPaint, BoxShadow, ColorRef, DisplayStyle, FlexDirectionStyle, FontFamily, FontStyle,
    FontVariantNumeric, GradientStop, LayoutStyle, LineHeight, LinearGradient, NodePartStyles,
    NodeStyle, PartLayoutStyle, PartStyle, RadialGradient, TextAlign, TextOverflow, TextSpacing,
    TextStyle, TextTransform, VisualStyle,
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
    BackgroundPaint(DgBackgroundPaint),
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
    BoxShadow(Vec<DgBoxShadow>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgTextDeclaration {
    FontSize(DgCssLength),
    FontFamily(String),
    FontWeight(u16),
    Color(DgCssColor),
    TextAlign(DgCssKeyword),
    TextTransform(TextTransform),
    LetterSpacing(DgCssLength),
    LineHeight(DgLineHeight),
    FontStyle(FontStyle),
    FontVariantNumeric(FontVariantNumeric),
    TextOverflow(TextOverflow),
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
    Em(f32),
    Percent(f32),
    Auto,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgLineHeight {
    Multiplier(f32),
    Length(DgCssLength),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgCssColor {
    Rgba(Color),
    Token(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgBackgroundPaint {
    LinearGradient(DgLinearGradient),
    RadialGradient(DgRadialGradient),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgLinearGradient {
    pub angle_deg: f32,
    pub stops: Vec<DgGradientStop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgRadialGradient {
    pub stops: Vec<DgGradientStop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgGradientStop {
    pub color: DgCssColor,
    pub position: Option<f32>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct DgBoxShadow {
    pub offset_x: DgCssLength,
    pub offset_y: DgCssLength,
    pub blur: DgCssLength,
    pub spread: DgCssLength,
    pub color: DgCssColor,
    pub inset: bool,
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
    Chain(DgSelectorChain),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DgSelectorChain {
    /// Ancestors nearest-to-farthest, with the combinator between that ancestor
    /// and the selector to its right.
    pub ancestors: Vec<(DgCombinator, DgCompoundSelector)>,
    pub target: DgCompoundSelector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgCombinator {
    Descendant,
    Child,
}

impl DgSelector {
    pub fn specificity(&self) -> Specificity {
        match self {
            DgSelector::Root => Specificity::ZERO,
            DgSelector::Compound(selector) => selector.specificity(),
            DgSelector::Child { parent, child } => parent.specificity().add(child.specificity()),
            DgSelector::Chain(chain) => chain.specificity(),
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
            DgSelector::Chain(chain) => chain.matches(element),
        }
    }

    fn matches_ancestor(&self, ancestor: &StyleAncestor<'_>) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.matches_ancestor(ancestor),
            DgSelector::Child { .. } => false,
            DgSelector::Chain(_) => false,
        }
    }

    fn target_pseudo_classes(&self) -> &[DgPseudoClass] {
        match self {
            DgSelector::Root => &[],
            DgSelector::Compound(selector) => &selector.pseudo,
            DgSelector::Child { child, .. } => &child.pseudo,
            DgSelector::Chain(chain) => &chain.target.pseudo,
        }
    }

    fn target_part(&self) -> Option<&str> {
        match self {
            DgSelector::Root => None,
            DgSelector::Compound(selector) => selector.part.as_deref(),
            DgSelector::Child { child, .. } => child.part.as_deref(),
            DgSelector::Chain(chain) => chain.target.part.as_deref(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            DgSelector::Root => ":root".to_string(),
            DgSelector::Compound(selector) => selector.label(),
            DgSelector::Child { parent, child } => {
                format!("{} > {}", parent.label(), child.label())
            }
            DgSelector::Chain(chain) => chain.label(),
        }
    }
}

impl DgSelectorChain {
    fn specificity(&self) -> Specificity {
        self.ancestors
            .iter()
            .fold(self.target.specificity(), |specificity, (_, selector)| {
                specificity.add(selector.specificity())
            })
    }

    fn matches(&self, element: &StyleElement<'_>) -> bool {
        if !self.target.matches_element(element) {
            return false;
        }

        let mut ancestor_idx = 0;
        for (combinator, selector) in &self.ancestors {
            match combinator {
                DgCombinator::Child => {
                    let Some(ancestor) = element.ancestors.get(ancestor_idx) else {
                        return false;
                    };
                    if !selector.matches_ancestor(ancestor) {
                        return false;
                    }
                    ancestor_idx += 1;
                }
                DgCombinator::Descendant => {
                    let Some(found_idx) = element.ancestors[ancestor_idx..]
                        .iter()
                        .position(|ancestor| selector.matches_ancestor(ancestor))
                    else {
                        return false;
                    };
                    ancestor_idx += found_idx + 1;
                }
            }
        }
        true
    }

    fn label(&self) -> String {
        let mut label = String::new();
        for (combinator, selector) in self.ancestors.iter().rev() {
            label.push_str(&selector.label());
            label.push_str(match combinator {
                DgCombinator::Descendant => " ",
                DgCombinator::Child => " > ",
            });
        }
        label.push_str(&self.target.label());
        label
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DgCompoundSelector {
    pub type_selector: Option<WidgetKind>,
    pub id: Option<String>,
    pub key: Option<String>,
    pub classes: Vec<String>,
    pub pseudo: Vec<DgPseudoClass>,
    pub structural: Vec<DgStructuralPseudo>,
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

    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
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

    pub fn with_structural(mut self, structural: DgStructuralPseudo) -> Self {
        self.structural.push(structural);
        self
    }

    pub fn with_part(mut self, part: impl Into<String>) -> Self {
        self.part = Some(part.into());
        self
    }

    pub fn specificity(&self) -> Specificity {
        Specificity {
            ids: u16::from(self.id.is_some()),
            classes: (self.classes.len()
                + self.pseudo.len()
                + self.structural.len()
                + usize::from(self.key.is_some()))
            .min(u16::MAX as usize) as u16,
            types: u16::from(self.type_selector.is_some()),
        }
    }

    fn matches_element(&self, element: &StyleElement<'_>) -> bool {
        self.matches_identity(element.id, element.key, element.classes, element.kind)
            && self
                .pseudo
                .iter()
                .all(|pseudo| element.pseudo.contains(pseudo))
            && self
                .structural
                .iter()
                .all(|pseudo| pseudo.matches_element(element))
    }

    fn matches_ancestor(&self, ancestor: &StyleAncestor<'_>) -> bool {
        self.pseudo.is_empty()
            && self.structural.is_empty()
            && self.matches_identity(ancestor.id, ancestor.key, ancestor.classes, ancestor.kind)
    }

    fn matches_identity(
        &self,
        id: &str,
        key: Option<&str>,
        classes: &[&str],
        kind: WidgetKind,
    ) -> bool {
        if self.type_selector.is_some_and(|expected| expected != kind) {
            return false;
        }
        if self.id.as_deref().is_some_and(|expected| expected != id) {
            return false;
        }
        if self
            .key
            .as_deref()
            .is_some_and(|expected| key != Some(expected))
        {
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
        if let Some(key) = &self.key {
            label.push_str("[key=\"");
            label.push_str(key);
            label.push_str("\"]");
        }
        for class in &self.classes {
            label.push('.');
            label.push_str(class);
        }
        for pseudo in &self.pseudo {
            label.push(':');
            label.push_str(pseudo.css_name());
        }
        for structural in &self.structural {
            label.push(':');
            label.push_str(&structural.label());
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
    Open,
    Expanded,
    Collapsed,
    Selected,
}

impl DgPseudoClass {
    fn css_name(self) -> &'static str {
        match self {
            DgPseudoClass::Hover => "hover",
            DgPseudoClass::Active => "active",
            DgPseudoClass::Focus => "focus",
            DgPseudoClass::Disabled => "disabled",
            DgPseudoClass::Checked => "checked",
            DgPseudoClass::Open => "open",
            DgPseudoClass::Expanded => "expanded",
            DgPseudoClass::Collapsed => "collapsed",
            DgPseudoClass::Selected => "selected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgStructuralPseudo {
    FirstChild,
    LastChild,
    NthChild(DgNthChild),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgNthChild {
    Odd,
    Even,
    Exact(usize),
}

impl DgStructuralPseudo {
    fn matches_element(self, element: &StyleElement<'_>) -> bool {
        let (Some(index), Some(count)) = (element.sibling_index, element.sibling_count) else {
            return false;
        };
        if count == 0 || index >= count {
            return false;
        }
        let one_based = index + 1;
        match self {
            DgStructuralPseudo::FirstChild => index == 0,
            DgStructuralPseudo::LastChild => one_based == count,
            DgStructuralPseudo::NthChild(DgNthChild::Odd) => one_based % 2 == 1,
            DgStructuralPseudo::NthChild(DgNthChild::Even) => one_based % 2 == 0,
            DgStructuralPseudo::NthChild(DgNthChild::Exact(expected)) => expected == one_based,
        }
    }

    fn label(self) -> String {
        match self {
            DgStructuralPseudo::FirstChild => "first-child".to_string(),
            DgStructuralPseudo::LastChild => "last-child".to_string(),
            DgStructuralPseudo::NthChild(DgNthChild::Odd) => "nth-child(odd)".to_string(),
            DgStructuralPseudo::NthChild(DgNthChild::Even) => "nth-child(even)".to_string(),
            DgStructuralPseudo::NthChild(DgNthChild::Exact(index)) => {
                format!("nth-child({index})")
            }
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
    pub sibling_index: Option<usize>,
    pub sibling_count: Option<usize>,
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
    BoxShadow,
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
    TextTransform,
    LetterSpacing,
    LineHeight,
    FontStyle,
    FontVariantNumeric,
    TextOverflow,
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
    /// border-bottom-left-radius, border, box-shadow, opacity, accent, track-color,
    /// thumb-color.
    ///
    /// Text: color, font-size, font-family, font-weight, text-align,
    /// text-transform, letter-spacing, line-height, font-style,
    /// font-variant-numeric, text-overflow.
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
            "box-shadow" => Ok(Self::Visual(DgVisualPropertyName::BoxShadow)),
            "opacity" => Ok(Self::Visual(DgVisualPropertyName::Opacity)),
            "accent" => Ok(Self::Visual(DgVisualPropertyName::Accent)),
            "track-color" => Ok(Self::Visual(DgVisualPropertyName::TrackColor)),
            "thumb-color" => Ok(Self::Visual(DgVisualPropertyName::ThumbColor)),
            "color" => Ok(Self::Text(DgTextPropertyName::Color)),
            "font-size" => Ok(Self::Text(DgTextPropertyName::FontSize)),
            "font-family" => Ok(Self::Text(DgTextPropertyName::FontFamily)),
            "font-weight" => Ok(Self::Text(DgTextPropertyName::FontWeight)),
            "text-align" => Ok(Self::Text(DgTextPropertyName::TextAlign)),
            "text-transform" => Ok(Self::Text(DgTextPropertyName::TextTransform)),
            "letter-spacing" => Ok(Self::Text(DgTextPropertyName::LetterSpacing)),
            "line-height" => Ok(Self::Text(DgTextPropertyName::LineHeight)),
            "font-style" => Ok(Self::Text(DgTextPropertyName::FontStyle)),
            "font-variant-numeric" => Ok(Self::Text(DgTextPropertyName::FontVariantNumeric)),
            "text-overflow" => Ok(Self::Text(DgTextPropertyName::TextOverflow)),
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
        "Badge" => Some(WidgetKind::Badge),
        "Tag" => Some(WidgetKind::Tag),
        "MenuBar" => Some(WidgetKind::MenuBar),
        "Menu" => Some(WidgetKind::Menu),
        "MenuItem" => Some(WidgetKind::MenuItem),
        "ContextMenu" => Some(WidgetKind::ContextMenu),
        "Tooltip" => Some(WidgetKind::Tooltip),
        "Toast" => Some(WidgetKind::Toast),
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
        WidgetKind::Badge => Some("Badge"),
        WidgetKind::Tag => Some("Tag"),
        WidgetKind::MenuBar => Some("MenuBar"),
        WidgetKind::Menu => Some("Menu"),
        WidgetKind::MenuItem => Some("MenuItem"),
        WidgetKind::ContextMenu => Some("ContextMenu"),
        WidgetKind::Tooltip => Some("Tooltip"),
        WidgetKind::Toast => Some("Toast"),
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

const STATIC_PSEUDO_CLASSES: [DgPseudoClass; 9] = [
    DgPseudoClass::Hover,
    DgPseudoClass::Active,
    DgPseudoClass::Focus,
    DgPseudoClass::Disabled,
    DgPseudoClass::Checked,
    DgPseudoClass::Open,
    DgPseudoClass::Expanded,
    DgPseudoClass::Collapsed,
    DgPseudoClass::Selected,
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
            classes: node_css_classes(node)
                .into_iter()
                .map(str::to_string)
                .collect(),
            kind: node.kind,
        }
    }
}

fn node_css_classes(node: &WidgetNode) -> Vec<&str> {
    let mut classes = split_classes(node.class_name.as_deref());
    if matches!(node.kind, WidgetKind::Badge | WidgetKind::Tag) {
        if let Some(level) = node
            .props
            .level
            .as_deref()
            .filter(|level| !level.is_empty())
        {
            if !classes.iter().any(|class| *class == level) {
                classes.push(level);
            }
        }
    }
    classes
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
            None,
            None,
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
    collect_matched_rule_labels(root, &rules, &mut ancestors, &mut out, None, None);
    out
}

pub fn matched_part_rule_labels_for_tree(
    root: &WidgetNode,
    store: &StylesheetStore,
) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    let rules = store.all_rules();
    let mut ancestors = Vec::new();
    let mut out = BTreeMap::new();
    collect_matched_part_rule_labels(root, &rules, &mut ancestors, &mut out, None, None);
    out
}

pub fn computed_style_for_virtual_element(
    kind: WidgetKind,
    id: &str,
    classes: &[&str],
    store: &StylesheetStore,
) -> NodeStyle {
    let rules = store.all_rules();
    let element = StyleElement {
        id,
        key: None,
        classes,
        kind,
        ancestors: &[],
        pseudo: &STATIC_PSEUDO_CLASSES,
        sibling_index: None,
        sibling_count: None,
    };
    let mut matched = Vec::new();
    for rule in rules.iter() {
        if rule.selector.matches(&element) && rule.selector.target_part().is_none() {
            matched.extend(rule.declarations.iter().map(|declaration| {
                (
                    rule.cascade_key(declaration),
                    rule.selector.target_pseudo_classes(),
                    &declaration.property,
                )
            }));
        }
    }
    matched.sort_by_key(|(key, _, _)| *key);

    let mut computed = NodeStyle::default();
    for (_, pseudo_classes, property) in matched {
        if pseudo_classes.is_empty() {
            apply_property_to_style(&mut computed, property);
        } else {
            for pseudo in pseudo_classes {
                apply_property_to_pseudo_style(&mut computed, *pseudo, property);
            }
        }
    }
    computed
}

fn collect_matched_part_rule_labels(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &mut Vec<AncestorSnapshot>,
    out: &mut BTreeMap<String, BTreeMap<String, Vec<String>>>,
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
) {
    let labels =
        matched_part_rule_labels_for_node(node, rules, ancestors, sibling_index, sibling_count);
    if !labels.is_empty() {
        out.insert(node.id.clone(), labels);
    }
    ancestors.push(AncestorSnapshot::from_node(node));
    let child_count = node.children.len();
    for (index, child) in node.children.iter().enumerate() {
        collect_matched_part_rule_labels(
            child,
            rules,
            ancestors,
            out,
            Some(index),
            Some(child_count),
        );
    }
    ancestors.pop();
}

fn matched_part_rule_labels_for_node(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &[AncestorSnapshot],
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
) -> BTreeMap<String, Vec<String>> {
    let classes = node_css_classes(node);
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
        sibling_index,
        sibling_count,
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
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
) {
    let labels = matched_rule_labels_for_node(node, rules, ancestors, sibling_index, sibling_count);
    if !labels.is_empty() {
        out.insert(node.id.clone(), labels);
    }
    ancestors.push(AncestorSnapshot::from_node(node));
    let child_count = node.children.len();
    for (index, child) in node.children.iter().enumerate() {
        collect_matched_rule_labels(child, rules, ancestors, out, Some(index), Some(child_count));
    }
    ancestors.pop();
}

fn matched_rule_labels_for_node(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &[AncestorSnapshot],
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
) -> Vec<String> {
    let classes = node_css_classes(node);
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
        sibling_index,
        sibling_count,
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
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
) {
    let classes = node_css_classes(node);
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
        sibling_index,
        sibling_count,
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
    let child_count = node.children.len();
    for (index, child) in node.children.iter_mut().enumerate() {
        apply_stylesheets_to_node(
            child,
            rules,
            ancestors,
            Some(&child_text),
            validation_warnings,
            seen_validation_warnings,
            Some(index),
            Some(child_count),
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
    retain_supported_inline_part_map(kind, &mut parts.open, "open", warnings, seen);
    retain_supported_inline_part_map(kind, &mut parts.expanded, "expanded", warnings, seen);
    retain_supported_inline_part_map(kind, &mut parts.collapsed, "collapsed", warnings, seen);
    retain_supported_inline_part_map(kind, &mut parts.selected, "selected", warnings, seen);
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
    merge_visual_style(&mut base.open, &overlay.open);
    merge_visual_style(&mut base.expanded, &overlay.expanded);
    merge_visual_style(&mut base.collapsed, &overlay.collapsed);
    merge_visual_style(&mut base.selected, &overlay.selected);
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
    base.text_transform = overlay.text_transform.or(base.text_transform);
    base.letter_spacing = overlay.letter_spacing.or(base.letter_spacing);
    base.line_height = overlay.line_height.or(base.line_height);
    base.font_style = overlay.font_style.or(base.font_style);
    base.font_variant_numeric = overlay.font_variant_numeric.or(base.font_variant_numeric);
    base.text_overflow = overlay.text_overflow.or(base.text_overflow);
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
    merge_part_style_map(&mut base.open, &overlay.open);
    merge_part_style_map(&mut base.expanded, &overlay.expanded);
    merge_part_style_map(&mut base.collapsed, &overlay.collapsed);
    merge_part_style_map(&mut base.selected, &overlay.selected);
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
    target.text_transform = target.text_transform.or(inherited.text_transform);
    target.letter_spacing = target.letter_spacing.or(inherited.letter_spacing);
    target.line_height = target.line_height.or(inherited.line_height);
    target.font_style = target.font_style.or(inherited.font_style);
    target.font_variant_numeric = target
        .font_variant_numeric
        .or(inherited.font_variant_numeric);
    target.text_overflow = target.text_overflow.or(inherited.text_overflow);
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
        DgPseudoClass::Open => &mut style.open,
        DgPseudoClass::Expanded => &mut style.expanded,
        DgPseudoClass::Collapsed => &mut style.collapsed,
        DgPseudoClass::Selected => &mut style.selected,
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
            DgPseudoClass::Open => &mut style.parts.open,
            DgPseudoClass::Expanded => &mut style.parts.expanded,
            DgPseudoClass::Collapsed => &mut style.parts.collapsed,
            DgPseudoClass::Selected => &mut style.parts.selected,
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
            let color = color_ref_from_css(value);
            style.background = Some(color.clone());
            style.background_paint = Some(BackgroundPaint::Color(color));
        }
        DgVisualDeclaration::BackgroundPaint(value) => {
            style.background = None;
            style.background_paint = Some(background_paint_from_css(value));
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
        DgVisualDeclaration::BoxShadow(value) => {
            style.box_shadows = Some(value.iter().filter_map(box_shadow_from_css).collect());
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
        DgTextDeclaration::TextTransform(value) => style.text_transform = Some(*value),
        DgTextDeclaration::LetterSpacing(value) => {
            style.letter_spacing = text_spacing_from_css(value)
        }
        DgTextDeclaration::LineHeight(value) => style.line_height = line_height_from_css(value),
        DgTextDeclaration::FontStyle(value) => style.font_style = Some(*value),
        DgTextDeclaration::FontVariantNumeric(value) => {
            style.font_variant_numeric = Some(*value);
        }
        DgTextDeclaration::TextOverflow(value) => style.text_overflow = Some(*value),
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

fn text_transform_from_keyword(value: &str) -> Option<TextTransform> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Some(TextTransform::None),
        "uppercase" => Some(TextTransform::Uppercase),
        "lowercase" => Some(TextTransform::Lowercase),
        "capitalize" => Some(TextTransform::Capitalize),
        _ => None,
    }
}

fn font_style_from_keyword(value: &str) -> Option<FontStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(FontStyle::Normal),
        "italic" => Some(FontStyle::Italic),
        _ => None,
    }
}

fn font_variant_numeric_from_keyword(value: &str) -> Option<FontVariantNumeric> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(FontVariantNumeric::Normal),
        "tabular-nums" | "tabular_nums" => Some(FontVariantNumeric::TabularNums),
        _ => None,
    }
}

fn text_overflow_from_keyword(value: &str) -> Option<TextOverflow> {
    match value.trim().to_ascii_lowercase().as_str() {
        "clip" => Some(TextOverflow::Clip),
        "ellipsis" => Some(TextOverflow::Ellipsis),
        _ => None,
    }
}

fn text_spacing_from_css(value: &DgCssLength) -> Option<TextSpacing> {
    match value {
        DgCssLength::LogicalPx(value) => Some(TextSpacing::LogicalPx(*value)),
        DgCssLength::Em(value) => Some(TextSpacing::Em(*value)),
        DgCssLength::Percent(_) | DgCssLength::Auto => None,
    }
}

fn line_height_from_css(value: &DgLineHeight) -> Option<LineHeight> {
    match value {
        DgLineHeight::Multiplier(value) => Some(LineHeight::Multiplier(value.max(0.0))),
        DgLineHeight::Length(DgCssLength::LogicalPx(value)) => {
            Some(LineHeight::LogicalPx(value.max(0.0)))
        }
        DgLineHeight::Length(DgCssLength::Em(value)) => Some(LineHeight::Multiplier(*value)),
        DgLineHeight::Length(DgCssLength::Percent(_)) | DgLineHeight::Length(DgCssLength::Auto) => {
            None
        }
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

fn background_paint_from_css(value: &DgBackgroundPaint) -> BackgroundPaint {
    match value {
        DgBackgroundPaint::LinearGradient(gradient) => {
            BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: gradient.angle_deg,
                stops: gradient
                    .stops
                    .iter()
                    .map(|stop| GradientStop {
                        color: color_ref_from_css(&stop.color),
                        position: stop.position,
                    })
                    .collect(),
            })
        }
        DgBackgroundPaint::RadialGradient(gradient) => {
            BackgroundPaint::RadialGradient(RadialGradient {
                stops: gradient
                    .stops
                    .iter()
                    .map(|stop| GradientStop {
                        color: color_ref_from_css(&stop.color),
                        position: stop.position,
                    })
                    .collect(),
            })
        }
    }
}

fn box_shadow_from_css(value: &DgBoxShadow) -> Option<BoxShadow> {
    Some(BoxShadow {
        offset_x: length_px(&value.offset_x)?,
        offset_y: length_px(&value.offset_y)?,
        blur: length_px(&value.blur)?.max(0.0),
        spread: length_px(&value.spread)?,
        color: color_ref_from_css(&value.color),
        inset: value.inset,
    })
}

fn length_px(value: &DgCssLength) -> Option<f32> {
    match value {
        DgCssLength::LogicalPx(value) => Some(*value),
        DgCssLength::Em(_) | DgCssLength::Percent(_) | DgCssLength::Auto => None,
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
        DgVisualPropertyName::Background => match parse_background_paint_value(value, variables)? {
            Some(paint) => DgVisualDeclaration::BackgroundPaint(paint),
            None => DgVisualDeclaration::Background(parse_color_value(name, value, variables)?),
        },
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
        DgVisualPropertyName::BoxShadow => {
            DgVisualDeclaration::BoxShadow(parse_box_shadow_value(name, value, variables)?)
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
        DgTextPropertyName::TextTransform => {
            let keyword = resolve_keyword(value, variables);
            let transform = text_transform_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "text-transform"))?;
            DgTextDeclaration::TextTransform(transform)
        }
        DgTextPropertyName::LetterSpacing => {
            DgTextDeclaration::LetterSpacing(parse_letter_spacing_value(name, value, variables)?)
        }
        DgTextPropertyName::LineHeight => {
            DgTextDeclaration::LineHeight(parse_line_height_value(name, value, variables)?)
        }
        DgTextPropertyName::FontStyle => {
            let keyword = resolve_keyword(value, variables);
            let font_style = font_style_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "font style"))?;
            DgTextDeclaration::FontStyle(font_style)
        }
        DgTextPropertyName::FontVariantNumeric => {
            let keyword = resolve_keyword(value, variables);
            let variant = font_variant_numeric_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "font variant numeric"))?;
            DgTextDeclaration::FontVariantNumeric(variant)
        }
        DgTextPropertyName::TextOverflow => {
            let keyword = resolve_keyword(value, variables);
            let overflow = text_overflow_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "text overflow"))?;
            DgTextDeclaration::TextOverflow(overflow)
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

    let Some((parts, combinators)) = split_selector_chain(selector) else {
        warnings.push(DgStyleWarning {
            property: selector.to_string(),
            message: "unsupported selector for DragonGUI CSS subset".to_string(),
        });
        return None;
    };

    if parts.len() == 1 {
        return parse_compound_selector(&parts[0])
            .map(DgSelector::Compound)
            .or_else(|| {
                warnings.push(DgStyleWarning {
                    property: selector.to_string(),
                    message: "unsupported selector for DragonGUI CSS subset".to_string(),
                });
                None
            });
    }

    let mut compounds = Vec::with_capacity(parts.len());
    for (idx, part) in parts.iter().enumerate() {
        let Some(compound) = parse_compound_selector(part) else {
            warnings.push(DgStyleWarning {
                property: selector.to_string(),
                message: if idx + 1 == parts.len() {
                    "unsupported target selector for DragonGUI CSS subset".to_string()
                } else {
                    "unsupported ancestor selector for DragonGUI CSS subset".to_string()
                },
            });
            return None;
        };
        if idx + 1 != parts.len() && compound.part.is_some() {
            warnings.push(DgStyleWarning {
                property: selector.to_string(),
                message: "part selectors are only supported on the target widget".to_string(),
            });
            return None;
        }
        if idx + 1 != parts.len()
            && (!compound.pseudo.is_empty() || !compound.structural.is_empty())
        {
            warnings.push(DgStyleWarning {
                property: selector.to_string(),
                message: "pseudo selectors are only supported on the target widget".to_string(),
            });
            return None;
        }
        compounds.push(compound);
    }

    let target = compounds
        .pop()
        .expect("selector chain has at least two compounds");
    let ancestors = compounds
        .into_iter()
        .zip(combinators)
        .rev()
        .map(|(compound, combinator)| (combinator, compound))
        .collect();
    Some(DgSelector::Chain(DgSelectorChain { ancestors, target }))
}

fn split_selector_chain(selector: &str) -> Option<(Vec<String>, Vec<DgCombinator>)> {
    let mut parts = Vec::new();
    let mut combinators = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;
    let mut chars = selector.char_indices().peekable();

    while let Some((_, ch)) = chars.next() {
        if let Some(quote_ch) = quote {
            current.push(ch);
            if ch == quote_ch {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            '[' => {
                bracket_depth += 1;
                current.push(ch);
            }
            ']' => {
                bracket_depth = bracket_depth.checked_sub(1)?;
                current.push(ch);
            }
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.checked_sub(1)?;
                current.push(ch);
            }
            '>' if bracket_depth == 0 && paren_depth == 0 => {
                push_selector_chain_part(&mut parts, &mut current)?;
                combinators.push(DgCombinator::Child);
                while chars
                    .peek()
                    .is_some_and(|(_, next)| next.is_ascii_whitespace())
                {
                    chars.next();
                }
            }
            _ if ch.is_ascii_whitespace() && bracket_depth == 0 && paren_depth == 0 => {
                while chars
                    .peek()
                    .is_some_and(|(_, next)| next.is_ascii_whitespace())
                {
                    chars.next();
                }
                if chars.peek().is_some_and(|(_, next)| *next == '>') {
                    continue;
                }
                if current.trim().is_empty() {
                    continue;
                }
                push_selector_chain_part(&mut parts, &mut current)?;
                combinators.push(DgCombinator::Descendant);
            }
            _ => current.push(ch),
        }
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return None;
    }
    push_selector_chain_part(&mut parts, &mut current)?;
    if parts.is_empty() || combinators.len() + 1 != parts.len() {
        return None;
    }
    Some((parts, combinators))
}

fn push_selector_chain_part(parts: &mut Vec<String>, current: &mut String) -> Option<()> {
    let part = current.trim();
    if part.is_empty() {
        return None;
    }
    parts.push(part.to_string());
    current.clear();
    Some(())
}

fn parse_compound_selector(selector: &str) -> Option<DgCompoundSelector> {
    let selector = selector.trim();
    if selector.is_empty() || selector.contains(' ') {
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
        .find(['.', '#', ':', '['])
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
        if prefix == "[" {
            let close = rest.find(']')?;
            let value = &rest[1..close];
            parse_attribute_selector(value, &mut compound)?;
            rest = &rest[close + 1..];
        } else {
            let next = tail
                .find(['.', '#', ':', '['])
                .map(|idx| idx + 1)
                .unwrap_or(rest.len());
            let value = &rest[1..next];
            if value.is_empty() {
                return None;
            }
            match prefix {
                "." => compound.classes.push(value.to_string()),
                "#" => compound.id = Some(value.to_string()),
                ":" => parse_pseudo_selector(value, &mut compound)?,
                _ => return None,
            }
            rest = &rest[next..];
        }
    }
    Some(compound)
}

fn parse_attribute_selector(value: &str, compound: &mut DgCompoundSelector) -> Option<()> {
    let (name, raw_value) = value.split_once('=')?;
    if name.trim() != "key" {
        return None;
    }
    let key = raw_value.trim();
    if key.is_empty() {
        return None;
    }
    let key = key
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            key.strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(key);
    if key.is_empty() {
        return None;
    }
    compound.key = Some(key.to_string());
    Some(())
}

fn is_part_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn parse_pseudo_selector(value: &str, compound: &mut DgCompoundSelector) -> Option<()> {
    if let Some(pseudo) = parse_pseudo(value) {
        compound.pseudo.push(pseudo);
        return Some(());
    }
    if let Some(structural) = parse_structural_pseudo(value) {
        compound.structural.push(structural);
        return Some(());
    }
    None
}

fn parse_pseudo(value: &str) -> Option<DgPseudoClass> {
    match value {
        "hover" => Some(DgPseudoClass::Hover),
        "active" => Some(DgPseudoClass::Active),
        "focus" => Some(DgPseudoClass::Focus),
        "disabled" => Some(DgPseudoClass::Disabled),
        "checked" => Some(DgPseudoClass::Checked),
        "open" => Some(DgPseudoClass::Open),
        "expanded" => Some(DgPseudoClass::Expanded),
        "collapsed" => Some(DgPseudoClass::Collapsed),
        "selected" => Some(DgPseudoClass::Selected),
        _ => None,
    }
}

fn parse_structural_pseudo(value: &str) -> Option<DgStructuralPseudo> {
    match value {
        "first-child" => Some(DgStructuralPseudo::FirstChild),
        "last-child" => Some(DgStructuralPseudo::LastChild),
        _ => {
            let inner = value
                .strip_prefix("nth-child(")
                .and_then(|value| value.strip_suffix(')'))?
                .trim();
            match inner {
                "odd" => Some(DgStructuralPseudo::NthChild(DgNthChild::Odd)),
                "even" => Some(DgStructuralPseudo::NthChild(DgNthChild::Even)),
                "2n" | "2n+0" => Some(DgStructuralPseudo::NthChild(DgNthChild::Even)),
                "2n+1" => Some(DgStructuralPseudo::NthChild(DgNthChild::Odd)),
                _ => inner
                    .parse::<usize>()
                    .ok()
                    .filter(|index| *index > 0)
                    .map(|index| DgStructuralPseudo::NthChild(DgNthChild::Exact(index))),
            }
        }
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
    let (name, fallback) = split_var_name_and_fallback(inner);
    variables
        .get(name)
        .cloned()
        .or_else(|| fallback.and_then(|fallback| parse_css_value(fallback, variables)))
}

fn split_var_name_and_fallback(value: &str) -> (&str, Option<&str>) {
    let mut depth = 0usize;
    for (idx, ch) in value.char_indices() {
        match ch {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let name = value[..idx].trim();
                let fallback = value[idx + 1..].trim();
                return (name, (!fallback.is_empty()).then_some(fallback));
            }
            _ => {}
        }
    }
    (value.trim(), None)
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

fn parse_letter_spacing_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssLength, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.eq_ignore_ascii_case("normal") {
        return Ok(DgCssLength::LogicalPx(0.0));
    }
    let length = parse_length(&value).ok_or_else(|| parse_warning(name, &value, "length"))?;
    match length {
        DgCssLength::LogicalPx(_) | DgCssLength::Em(_) => Ok(length),
        DgCssLength::Percent(_) | DgCssLength::Auto => {
            Err(parse_warning(name, &value, "px or em length"))
        }
    }
}

fn parse_line_height_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgLineHeight, DgStyleWarning> {
    match resolve_variable(value, variables) {
        Some(DgCssValue::Number(number)) => return Ok(DgLineHeight::Multiplier(number.0)),
        Some(DgCssValue::Length(length)) => {
            return match length {
                DgCssLength::LogicalPx(_) | DgCssLength::Em(_) => Ok(DgLineHeight::Length(length)),
                DgCssLength::Percent(_) | DgCssLength::Auto => {
                    Err(parse_warning(name, value, "number or px length"))
                }
            };
        }
        Some(_) => return Err(parse_warning(name, value, "line height")),
        None => {}
    }
    let value = value.trim();
    if let Ok(number) = value.parse::<f32>() {
        return Ok(DgLineHeight::Multiplier(number));
    }
    let length = parse_length(value).ok_or_else(|| parse_warning(name, value, "line height"))?;
    match length {
        DgCssLength::LogicalPx(_) | DgCssLength::Em(_) => Ok(DgLineHeight::Length(length)),
        DgCssLength::Percent(_) | DgCssLength::Auto => {
            Err(parse_warning(name, value, "number or px length"))
        }
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
        DgCssLength::Em(_) => Err(DgStyleWarning {
            property: name.to_string(),
            message: format!(
                "`em` lengths are only supported for text spacing in DragonGUI CSS: {source:?}"
            ),
        }),
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
    if matches!(name, "background" | "background-color")
        && value.trim().eq_ignore_ascii_case("none")
    {
        return Ok(DgCssColor::Rgba([0.0, 0.0, 0.0, 0.0]));
    }
    match resolve_variable(value, variables) {
        Some(DgCssValue::Color(color)) => return Ok(color),
        Some(DgCssValue::Keyword(keyword)) => return Ok(DgCssColor::Token(keyword.0)),
        Some(DgCssValue::String(value)) => return Ok(DgCssColor::Token(value)),
        Some(_) => return Err(parse_warning(name, value, "color")),
        None => {}
    }
    parse_color(value).ok_or_else(|| parse_warning(name, value, "color"))
}

fn parse_background_paint_value(
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgBackgroundPaint>, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if let Some(args) = function_args(&value, "linear-gradient") {
        return Ok(Some(DgBackgroundPaint::LinearGradient(
            parse_linear_gradient(args, variables)?,
        )));
    }
    if let Some(args) = function_args(&value, "radial-gradient") {
        return Ok(Some(DgBackgroundPaint::RadialGradient(
            parse_radial_gradient(args, variables)?,
        )));
    }
    Ok(None)
}

fn parse_linear_gradient(
    args: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgLinearGradient, DgStyleWarning> {
    let parts = split_top_level_commas(args);
    if parts.len() < 2 {
        return Err(parse_warning(
            "background",
            args,
            "linear-gradient with at least two color stops",
        ));
    }
    let (angle_deg, stop_parts) = if let Some(angle) = parse_linear_gradient_direction(parts[0]) {
        (angle, &parts[1..])
    } else {
        (180.0, &parts[..])
    };
    if stop_parts.len() < 2 {
        return Err(parse_warning(
            "background",
            args,
            "linear-gradient with at least two color stops",
        ));
    }
    let mut stops = Vec::with_capacity(stop_parts.len());
    for part in stop_parts {
        stops.push(parse_gradient_stop(part, variables)?);
    }
    Ok(DgLinearGradient { angle_deg, stops })
}

fn parse_radial_gradient(
    args: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgRadialGradient, DgStyleWarning> {
    let parts = split_top_level_commas(args);
    if parts.len() < 2 {
        return Err(parse_warning(
            "background",
            args,
            "radial-gradient with at least two color stops",
        ));
    }
    let stop_parts = if is_supported_radial_gradient_shape(parts[0]) {
        &parts[1..]
    } else {
        &parts[..]
    };
    if stop_parts.len() < 2 {
        return Err(parse_warning(
            "background",
            args,
            "radial-gradient with at least two color stops",
        ));
    }
    let mut stops = Vec::with_capacity(stop_parts.len());
    for part in stop_parts {
        stops.push(parse_gradient_stop(part, variables)?);
    }
    Ok(DgRadialGradient { stops })
}

fn is_supported_radial_gradient_shape(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "circle" | "circle at center"
    )
}

fn parse_linear_gradient_direction(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(deg) = value.strip_suffix("deg") {
        return deg
            .trim()
            .parse::<f32>()
            .ok()
            .map(|deg| deg.rem_euclid(360.0));
    }
    let direction = value.strip_prefix("to ")?.trim();
    match direction {
        "top" => Some(0.0),
        "top right" | "right top" => Some(45.0),
        "right" => Some(90.0),
        "bottom right" | "right bottom" => Some(135.0),
        "bottom" => Some(180.0),
        "bottom left" | "left bottom" => Some(225.0),
        "left" => Some(270.0),
        "top left" | "left top" => Some(315.0),
        _ => None,
    }
}

fn parse_gradient_stop(
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgGradientStop, DgStyleWarning> {
    let tokens = split_value_tokens(value);
    if tokens.is_empty() || tokens.len() > 2 {
        return Err(parse_warning("background", value, "gradient color stop"));
    }
    let color = parse_color_value("background", tokens[0], variables)?;
    let position = tokens
        .get(1)
        .map(|value| parse_gradient_stop_position(value))
        .transpose()
        .map_err(|_| parse_warning("background", value, "gradient stop position"))?;
    Ok(DgGradientStop { color, position })
}

fn parse_gradient_stop_position(value: &str) -> Result<f32, ()> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .map(|value| (value / 100.0).clamp(0.0, 1.0))
            .map_err(|_| ());
    }
    value
        .parse::<f32>()
        .map(|value| value.clamp(0.0, 1.0))
        .map_err(|_| ())
}

fn parse_color(value: &str) -> Option<DgCssColor> {
    let value = value.trim();
    if value.starts_with('#') {
        parse_css_hex_color(value).map(DgCssColor::Rgba)
    } else if let Some(color) = parse_named_css_color(value) {
        Some(DgCssColor::Rgba(color))
    } else if let Some(color) = parse_functional_color(value) {
        Some(DgCssColor::Rgba(color))
    } else if is_identifier_like(value) {
        Some(DgCssColor::Token(value.to_string()))
    } else {
        None
    }
}

fn parse_named_css_color(value: &str) -> Option<Color> {
    match value.trim().to_ascii_lowercase().as_str() {
        "transparent" => Some([0.0, 0.0, 0.0, 0.0]),
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        "white" => Some([1.0, 1.0, 1.0, 1.0]),
        "red" => Some([1.0, 0.0, 0.0, 1.0]),
        "green" => Some([0.0, 0.5019608, 0.0, 1.0]),
        "blue" => Some([0.0, 0.0, 1.0, 1.0]),
        "gray" | "grey" => Some([0.5019608, 0.5019608, 0.5019608, 1.0]),
        _ => None,
    }
}

fn parse_functional_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if let Some(args) = function_args(value, "rgb").or_else(|| function_args(value, "rgba")) {
        return parse_rgb_function(args);
    }
    if let Some(args) = function_args(value, "hsl").or_else(|| function_args(value, "hsla")) {
        return parse_hsl_function(args);
    }
    None
}

fn function_args<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let value = value.trim();
    let prefix = format!("{name}(");
    value
        .strip_prefix(&prefix)
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::trim)
}

fn color_function_tokens(args: &str) -> Vec<&str> {
    if args.contains(',') {
        args.split(',')
            .flat_map(|part| part.split('/'))
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect()
    } else {
        args.split_whitespace()
            .filter(|part| *part != "/")
            .collect()
    }
}

fn parse_rgb_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let r = parse_rgb_channel(tokens[0])?;
    let g = parse_rgb_channel(tokens[1])?;
    let b = parse_rgb_channel(tokens[2])?;
    let a = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    Some([r, g, b, a])
}

fn parse_hsl_function(args: &str) -> Option<Color> {
    let tokens = color_function_tokens(args);
    if tokens.len() != 3 && tokens.len() != 4 {
        return None;
    }
    let hue = parse_hue_degrees(tokens[0])?;
    let saturation = parse_percent_channel(tokens[1])?;
    let lightness = parse_percent_channel(tokens[2])?;
    let alpha = tokens
        .get(3)
        .and_then(|value| parse_alpha_channel(value))
        .unwrap_or(1.0);
    let [r, g, b] = hsl_to_rgb(hue, saturation, lightness);
    Some([r, g, b, alpha])
}

fn parse_rgb_channel(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    value
        .trim()
        .parse::<f32>()
        .ok()
        .map(|v| (v / 255.0).clamp(0.0, 1.0))
}

fn parse_percent_channel(value: &str) -> Option<f32> {
    value
        .trim()
        .strip_suffix('%')?
        .trim()
        .parse::<f32>()
        .ok()
        .map(|v| (v / 100.0).clamp(0.0, 1.0))
}

fn parse_alpha_channel(value: &str) -> Option<f32> {
    if let Some(percent) = value.trim().strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v / 100.0).clamp(0.0, 1.0));
    }
    value.trim().parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
}

fn parse_hue_degrees(value: &str) -> Option<f32> {
    let value = value.trim();
    let number = value
        .strip_suffix("deg")
        .unwrap_or(value)
        .trim()
        .parse::<f32>()
        .ok()?;
    Some(number.rem_euclid(360.0))
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> [f32; 3] {
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let h = hue / 60.0;
    let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = if h < 1.0 {
        (c, x, 0.0)
    } else if h < 2.0 {
        (x, c, 0.0)
    } else if h < 3.0 {
        (0.0, c, x)
    } else if h < 4.0 {
        (0.0, x, c)
    } else if h < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = lightness - c * 0.5;
    [
        (r1 + m).clamp(0.0, 1.0),
        (g1 + m).clamp(0.0, 1.0),
        (b1 + m).clamp(0.0, 1.0),
    ]
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
    if let Some(em) = value.strip_suffix("em") {
        return em.trim().parse::<f32>().ok().map(DgCssLength::Em);
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

fn parse_box_shadow_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Vec<DgBoxShadow>, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let shadows = split_top_level_commas(&value);
    if shadows.len() != 1 {
        return Err(DgStyleWarning {
            property: name.to_string(),
            message: "only a single non-inset box-shadow is supported in DragonGUI CSS V1"
                .to_string(),
        });
    }
    parse_single_box_shadow(name, shadows[0], variables).map(|shadow| vec![shadow])
}

fn parse_single_box_shadow(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgBoxShadow, DgStyleWarning> {
    let mut lengths = Vec::new();
    let mut color = None;
    let mut inset = false;
    for token in split_value_tokens(value) {
        if token.eq_ignore_ascii_case("inset") {
            inset = true;
            continue;
        }
        if color.is_none() {
            if let Ok(parsed) = parse_color_value(name, token, variables) {
                color = Some(parsed);
                continue;
            }
        }
        lengths.push(parse_px_length_value(name, token, variables)?);
    }
    if inset {
        return Err(DgStyleWarning {
            property: name.to_string(),
            message: "inset box-shadow is not supported in DragonGUI CSS V1".to_string(),
        });
    }
    if lengths.len() < 2 || lengths.len() > 4 {
        return Err(parse_warning(
            name,
            value,
            "box-shadow: <offset-x> <offset-y> <blur?> <spread?> <color>",
        ));
    }
    let Some(color) = color else {
        return Err(parse_warning(name, value, "box-shadow color"));
    };
    Ok(DgBoxShadow {
        offset_x: lengths[0].clone(),
        offset_y: lengths[1].clone(),
        blur: lengths
            .get(2)
            .cloned()
            .unwrap_or(DgCssLength::LogicalPx(0.0)),
        spread: lengths
            .get(3)
            .cloned()
            .unwrap_or(DgCssLength::LogicalPx(0.0)),
        color,
        inset: false,
    })
}

fn split_value_tokens(value: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    let mut quote = None;
    for (idx, ch) in value.char_indices() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                start.get_or_insert(idx);
            }
            '(' => {
                depth = depth.saturating_add(1);
                start.get_or_insert(idx);
            }
            ')' => {
                depth = depth.saturating_sub(1);
            }
            ch if ch.is_whitespace() && depth == 0 => {
                if let Some(token_start) = start.take() {
                    let token = value[token_start..idx].trim();
                    if !token.is_empty() {
                        tokens.push(token);
                    }
                }
            }
            _ => {
                start.get_or_insert(idx);
            }
        }
    }
    if let Some(token_start) = start {
        let token = value[token_start..].trim();
        if !token.is_empty() {
            tokens.push(token);
        }
    }
    tokens
}

fn split_top_level_commas(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut quote = None;
    let mut start = 0usize;
    for (idx, ch) in value.char_indices() {
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let part = value[start..idx].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    let part = value[start..].trim();
    if !part.is_empty() {
        parts.push(part);
    }
    parts
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
            (
                "box-shadow",
                DgStylePropertyName::Visual(DgVisualPropertyName::BoxShadow),
            ),
            ("border", DgStylePropertyName::BorderShorthand),
            (
                "color",
                DgStylePropertyName::Text(DgTextPropertyName::Color),
            ),
            (
                "text-transform",
                DgStylePropertyName::Text(DgTextPropertyName::TextTransform),
            ),
            (
                "letter-spacing",
                DgStylePropertyName::Text(DgTextPropertyName::LetterSpacing),
            ),
            (
                "line-height",
                DgStylePropertyName::Text(DgTextPropertyName::LineHeight),
            ),
            (
                "font-style",
                DgStylePropertyName::Text(DgTextPropertyName::FontStyle),
            ),
            (
                "font-variant-numeric",
                DgStylePropertyName::Text(DgTextPropertyName::FontVariantNumeric),
            ),
            (
                "text-overflow",
                DgStylePropertyName::Text(DgTextPropertyName::TextOverflow),
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
        let warning = DgStylePropertyName::from_css_name("box-shadow-color").unwrap_err();
        assert_eq!(warning.property, "box-shadow-color");
        assert!(warning.message.contains("unsupported"));
    }

    #[test]
    fn selector_specificity_counts_type_class_id_and_pseudo() {
        let selector = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_id("run")
                .with_key("primary-action")
                .with_class("danger")
                .with_class("primary")
                .with_pseudo(DgPseudoClass::Hover),
        );

        assert_eq!(selector.specificity(), Specificity::new(1, 4, 1));
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
            WidgetKind::Badge,
            WidgetKind::Tag,
            WidgetKind::Panel,
            WidgetKind::DataFrameTable,
            WidgetKind::Scatter3D,
            WidgetKind::Image,
            WidgetKind::Toast,
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
            key: Some("primary-action"),
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &[],
            pseudo: &pseudos,
            sibling_index: Some(0),
            sibling_count: Some(2),
        };
        let selector = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_id("run")
                .with_key("primary-action")
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
            sibling_index: Some(0),
            sibling_count: Some(1),
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
    fn selector_matching_supports_descendant_and_deep_child_chains() {
        let button_classes = ["primary"];
        let h_layout_classes = ["toolbar"];
        let panel_classes = ["controls"];
        let window_classes: [&str; 0] = [];
        let ancestors = [
            StyleAncestor {
                id: "toolbar",
                key: Some("main-toolbar"),
                classes: &h_layout_classes,
                kind: WidgetKind::HLayout,
            },
            StyleAncestor {
                id: "controls-panel",
                key: None,
                classes: &panel_classes,
                kind: WidgetKind::Panel,
            },
            StyleAncestor {
                id: "root",
                key: None,
                classes: &window_classes,
                kind: WidgetKind::Window,
            },
        ];
        let element = StyleElement {
            id: "run",
            key: Some("primary-action"),
            classes: &button_classes,
            kind: WidgetKind::Button,
            ancestors: &ancestors,
            pseudo: &[],
            sibling_index: Some(1),
            sibling_count: Some(3),
        };
        let descendant = DgSelector::Chain(DgSelectorChain {
            ancestors: vec![(
                DgCombinator::Descendant,
                DgCompoundSelector::new()
                    .with_type(WidgetKind::Panel)
                    .with_class("controls"),
            )],
            target: DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_key("primary-action"),
        });
        let deep_child = DgSelector::Chain(DgSelectorChain {
            ancestors: vec![
                (
                    DgCombinator::Child,
                    DgCompoundSelector::new()
                        .with_type(WidgetKind::HLayout)
                        .with_key("main-toolbar"),
                ),
                (
                    DgCombinator::Child,
                    DgCompoundSelector::new()
                        .with_type(WidgetKind::Panel)
                        .with_class("controls"),
                ),
            ],
            target: DgCompoundSelector::new().with_type(WidgetKind::Button),
        });
        let wrong_direct_parent = DgSelector::Chain(DgSelectorChain {
            ancestors: vec![(
                DgCombinator::Child,
                DgCompoundSelector::new()
                    .with_type(WidgetKind::Panel)
                    .with_class("controls"),
            )],
            target: DgCompoundSelector::new().with_type(WidgetKind::Button),
        });

        assert!(descendant.matches(&element));
        assert!(deep_child.matches(&element));
        assert!(!wrong_direct_parent.matches(&element));
    }

    #[test]
    fn selector_matching_supports_structural_pseudo_classes() {
        let classes = ["primary"];
        let element = StyleElement {
            id: "second",
            key: None,
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &[],
            pseudo: &[],
            sibling_index: Some(1),
            sibling_count: Some(3),
        };
        let second_child = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::NthChild(DgNthChild::Exact(2))),
        );
        let even_child = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::NthChild(DgNthChild::Even)),
        );
        let first_child = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::FirstChild),
        );
        let last_child = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::LastChild),
        );

        assert!(second_child.matches(&element));
        assert!(even_child.matches(&element));
        assert!(!first_child.matches(&element));
        assert!(!last_child.matches(&element));
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
                    "key": "primary-action",
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
                Panel Button[key="primary-action"] { color: white; }
                Panel.controls > Button { border-width: 2px; }
                Button:hover { background: accent_mix_20; }
                "#,
            )
            .unwrap();

        let labels = matched_rule_labels_for_tree(&tree, &store);
        let button_labels = labels.get("run").expect("button should have CSS matches");

        assert!(button_labels.contains(&"user: Button".to_string()));
        assert!(button_labels.contains(&"user: .primary".to_string()));
        assert!(button_labels.contains(&"user: Panel Button[key=\"primary-action\"]".to_string()));
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
    fn parses_descendant_child_chain_and_key_selectors() {
        let parsed = parse_stylesheet(
            r#"
            Panel Button[key="primary-action"] { color: white; }
            Window > Panel.controls > HLayout.toolbar > Button.primary { background: accent; }
            Panel > Button:nth-child(even) { border-color: accent; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert_eq!(parsed.rules.len(), 3);
        assert_eq!(
            parsed.rules[0].selector.label(),
            "Panel Button[key=\"primary-action\"]"
        );
        assert_eq!(
            parsed.rules[0].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(
            parsed.rules[1].selector.label(),
            "Window > Panel.controls > HLayout.toolbar > Button.primary"
        );
        assert_eq!(
            parsed.rules[1].selector.specificity(),
            Specificity::new(0, 3, 4)
        );
        assert_eq!(
            parsed.rules[2].selector.label(),
            "Panel > Button:nth-child(even)"
        );
        assert_eq!(
            parsed.rules[2].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
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
    fn stylesheet_cascade_applies_structural_selectors() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "panel",
                "type": "panel",
                "children": [
                    {"id": "first", "type": "button", "props": {"text": "One"}},
                    {"id": "second", "type": "button", "props": {"text": "Two"}},
                    {"id": "caption", "type": "label", "props": {"text": "End"}}
                ]
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel > Button:first-child { background: accent; }
                Panel > Button:nth-child(2) { border-width: 3px; }
                Panel > Button:last-child { background: danger; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let panel = &tree.children[0];
        let first = &panel.children[0];
        let second = &panel.children[1];

        assert_eq!(
            first.style.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(second.style.visual.border_width, Some(3.0));
        assert_ne!(
            second.style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn parses_widget_part_selectors() {
        let parsed = parse_stylesheet(
            r#"
            NumberInput::stepper { width: 34px; background: surface_alt; }
            Panel.controls > NumberInput:hover::stepper-up { color: accent; }
            .numeric::stepper-down { border-bottom-right-radius: 10px; }
            Checkbox:checked::indicator { background: accent; }
            Collapsible:collapsed::indicator { color: muted_text; }
            Dropdown:open { border-color: accent; }
            Tab:selected::accent { background: accent; }
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
        assert_eq!(
            parsed.rules[4].selector.target_pseudo_classes(),
            &[DgPseudoClass::Collapsed]
        );
        assert_eq!(
            parsed.rules[5].selector.target_pseudo_classes(),
            &[DgPseudoClass::Open]
        );
        assert_eq!(
            parsed.rules[6].selector.target_pseudo_classes(),
            &[DgPseudoClass::Selected]
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
    fn stylesheet_cascade_applies_semantic_pseudo_state_slots() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "advanced",
                "type": "collapsible",
                "props": {"text": "Advanced", "expanded": false}
            }, {
                "id": "mode",
                "type": "dropdown",
                "props": {"items": ["A", "B"], "value": "A"}
            }, {
                "id": "tabs",
                "type": "tabs",
                "children": [{
                    "id": "tab-a",
                    "type": "tab",
                    "props": {"label": "A", "value": "a"}
                }]
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Collapsible:collapsed { background: surface_alt; }
                Collapsible:expanded::header { color: success; }
                Dropdown:open { border-color: accent; }
                Tab:selected { background: accent_mix_20; }
                Tab:selected::accent { background: accent; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let collapsible = &tree.children[0];
        let dropdown = &tree.children[1];
        let tab = &tree.children[2].children[0];

        assert_eq!(
            collapsible.style.collapsed.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(
            collapsible
                .style
                .parts
                .expanded
                .get("header")
                .and_then(|style| style.text.color.clone()),
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(
            dropdown.style.open.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            tab.style.selected.background,
            Some(ColorRef::Token("accent_mix_20".to_string()))
        );
        assert_eq!(
            tab.style
                .parts
                .selected
                .get("accent")
                .and_then(|style| style.visual.background.clone()),
            Some(ColorRef::Token("accent".to_string()))
        );
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
            "Button { filter: blur(2px); border-radius: 4px; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "filter"));
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
            "Panel::accent > Button { color: text; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.rules.is_empty());
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "Panel::accent > Button"
                && warning
                    .message
                    .contains("part selectors are only supported on the target widget")));
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
    fn web_color_syntax_parses_to_rgba() {
        let parsed = parse_stylesheet(
            r#"
            Button {
                background: transparent;
                border-color: rgba(255, 128, 0, 0.25);
                foreground: hsl(120, 100%, 25%);
                accent: white;
            }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();
        let declarations = &parsed.rules[0].declarations;
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::Background(DgCssColor::Rgba(color)))
                    if color == [0.0, 0.0, 0.0, 0.0]
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderColor(DgCssColor::Rgba(color)))
                    if (color[0] - 1.0).abs() < 0.001
                        && (color[1] - (128.0 / 255.0)).abs() < 0.001
                        && color[2].abs() < 0.001
                        && (color[3] - 0.25).abs() < 0.003
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::Foreground(DgCssColor::Rgba(color)))
                    if color[1] > 0.49 && color[0] < 0.01 && color[2] < 0.01
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::Accent(DgCssColor::Rgba(color)))
                    if color == [1.0, 1.0, 1.0, 1.0]
            )
        }));
    }

    #[test]
    fn box_shadow_parses_to_visual_style() {
        let parsed = parse_stylesheet(
            "Panel.card { box-shadow: 0 10px 30px 2px rgba(0, 0, 0, 0.28); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        let declaration = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BoxShadow(shadows)) => {
                    Some(&shadows[0])
                }
                _ => None,
            })
            .expect("box-shadow declaration");
        assert_eq!(declaration.offset_x, DgCssLength::LogicalPx(0.0));
        assert_eq!(declaration.offset_y, DgCssLength::LogicalPx(10.0));
        assert_eq!(declaration.blur, DgCssLength::LogicalPx(30.0));
        assert_eq!(declaration.spread, DgCssLength::LogicalPx(2.0));
        assert!(!declaration.inset);
        assert!(matches!(
            declaration.color,
            DgCssColor::Rgba(color) if (color[3] - 0.28).abs() < 0.003
        ));

        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);
        let shadows = style.visual.box_shadows.as_ref().expect("computed shadow");
        assert_eq!(shadows.len(), 1);
        assert_eq!(shadows[0].offset_y, 10.0);
        assert_eq!(shadows[0].blur, 30.0);
        assert_eq!(shadows[0].spread, 2.0);
    }

    #[test]
    fn box_shadow_none_overrides_to_empty_shadow_list() {
        let parsed =
            parse_stylesheet("Panel { box-shadow: none; }", StylesheetOrigin::User).unwrap();
        assert!(matches!(
            &parsed.rules[0].declarations[0].property,
            DgStyleProperty::Visual(DgVisualDeclaration::BoxShadow(shadows))
                if shadows.is_empty()
        ));
    }

    #[test]
    fn linear_gradient_background_parses_to_background_paint() {
        let parsed = parse_stylesheet(
            "Panel.hero { background: linear-gradient(180deg, #ff7a18, rgba(175, 0, 45, 0.8)); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        let gradient = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundPaint(
                    DgBackgroundPaint::LinearGradient(gradient),
                )) => Some(gradient),
                _ => None,
            })
            .expect("linear gradient background paint");
        assert_eq!(gradient.angle_deg, 180.0);
        assert_eq!(gradient.stops.len(), 2);
        assert!(matches!(gradient.stops[0].color, DgCssColor::Rgba(_)));
        assert!(matches!(
            gradient.stops[1].color,
            DgCssColor::Rgba(color) if (color[3] - 0.8).abs() < 0.003
        ));

        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);
        assert!(matches!(
            style.visual.background_paint,
            Some(BackgroundPaint::LinearGradient(_))
        ));
    }

    #[test]
    fn linear_gradient_background_can_use_root_variable() {
        let parsed = parse_stylesheet(
            r#"
            :root {
                --hero-bg: linear-gradient(to right, rgba(255, 255, 255, 0.2), transparent);
            }

            Panel.hero {
                background: var(--hero-bg);
            }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        let gradient = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundPaint(
                    DgBackgroundPaint::LinearGradient(gradient),
                )) => Some(gradient),
                _ => None,
            })
            .expect("linear gradient background paint from variable");
        assert_eq!(gradient.angle_deg, 90.0);
        assert_eq!(gradient.stops.len(), 2);
    }

    #[test]
    fn radial_gradient_background_parses_to_background_paint() {
        let parsed = parse_stylesheet(
            "Panel.hero { background: radial-gradient(circle, rgba(255, 255, 255, 0.25), transparent); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        let gradient = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundPaint(
                    DgBackgroundPaint::RadialGradient(gradient),
                )) => Some(gradient),
                _ => None,
            })
            .expect("radial gradient background paint");
        assert_eq!(gradient.stops.len(), 2);
        assert!(matches!(
            gradient.stops[0].color,
            DgCssColor::Rgba(color) if (color[3] - 0.25).abs() < 0.003
        ));
        assert!(matches!(
            gradient.stops[1].color,
            DgCssColor::Rgba(color) if color[3].abs() < 0.001
        ));
    }

    #[test]
    fn variable_fallbacks_parse_when_variable_is_missing() {
        let parsed = parse_stylesheet(
            r#"
            Button {
                background: var(--missing-bg, rgba(255, 0, 0, 0.5));
                border-radius: var(--missing-radius, 9px);
            }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::Background(DgCssColor::Rgba(color)))
                    if (color[0] - 1.0).abs() < 0.001
                        && color[1].abs() < 0.001
                        && color[2].abs() < 0.001
                        && (color[3] - 0.5).abs() < 0.003
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderRadius(DgCssLength::LogicalPx(
                    9.0
                )))
            )
        }));
    }

    #[test]
    fn typography_properties_cascade_to_text_style() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "label",
                "type": "label",
                "props": {"text": "Latency 123"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Label {
                    text-transform: uppercase;
                    letter-spacing: 0.08em;
                    line-height: 1.15;
                    font-style: italic;
                    font-variant-numeric: tabular-nums;
                    text-overflow: ellipsis;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let label = &tree.children[0];

        assert_eq!(
            label.style.text.text_transform,
            Some(TextTransform::Uppercase)
        );
        assert_eq!(label.style.text.letter_spacing, Some(TextSpacing::Em(0.08)));
        assert_eq!(
            label.style.text.line_height,
            Some(LineHeight::Multiplier(1.15))
        );
        assert_eq!(label.style.text.font_style, Some(FontStyle::Italic));
        assert_eq!(
            label.style.text.font_variant_numeric,
            Some(FontVariantNumeric::TabularNums)
        );
        assert_eq!(label.style.text.text_overflow, Some(TextOverflow::Ellipsis));
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
    fn badge_and_tag_levels_are_css_classes() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [
                {
                    "id": "badge",
                    "type": "badge",
                    "props": {"text": "3", "level": "info"}
                },
                {
                    "id": "tag",
                    "type": "tag",
                    "class": "pill",
                    "props": {"text": "Ready", "level": "neutral"}
                }
            ]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Badge.info { background: success; }
                Tag.neutral.pill { border-color: accent; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let badge = &tree.children[0];
        let tag = &tree.children[1];

        assert_eq!(
            badge.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(
            tag.style.visual.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
    }

    #[test]
    fn stylesheet_cascade_applies_to_virtual_overlay_elements() {
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Tooltip {
                    background: #101820;
                    color: #f6f7f8;
                    padding: 10px;
                }

                Toast.error {
                    background: danger;
                    border-radius: 12px;
                    font-weight: 700;
                }
                "#,
            )
            .unwrap();

        let tooltip = computed_style_for_virtual_element(
            WidgetKind::Tooltip,
            "__dg_static_tooltip",
            &["static"],
            &store,
        );
        assert_eq!(
            tooltip.visual.background,
            Some(ColorRef::Rgba([
                16.0 / 255.0,
                24.0 / 255.0,
                32.0 / 255.0,
                1.0
            ]))
        );
        assert_eq!(tooltip.layout.padding_top, Some(10.0));
        assert_eq!(tooltip.layout.padding_right, Some(10.0));

        let toast =
            computed_style_for_virtual_element(WidgetKind::Toast, "toast-1", &["error"], &store);
        assert_eq!(
            toast.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
        assert_eq!(toast.visual.border_radius, Some(12.0));
        assert_eq!(toast.text.font_weight, Some(700));
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
