//! DragonGUI-owned CSS style IR.
//!
//! Parser dependencies such as `lightningcss` must lower into these types
//! immediately. Selector matching, cascade resolution, computed styles, and
//! renderer integration should not depend on parser-specific AST types.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use lightningcss::media_query::{
    MediaCondition as LightningMediaCondition, MediaFeatureComparison, MediaFeatureId,
    MediaFeatureName, MediaFeatureValue, MediaList, MediaQuery, MediaType, Operator, QueryFeature,
};
use lightningcss::properties::Property;
use lightningcss::rules::{supports::SupportsCondition, CssRule, CssRuleList};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::ToCss;

use crate::document::{WidgetKind, WidgetNode};
use crate::style::{
    BackgroundPaint, BoxShadow, CalcLength, ColorRef, DisplayStyle, FlexDirectionStyle, FontFamily,
    FontStyle, FontVariantNumeric, GradientStop, GridLineStyle, GridPlacementStyle,
    GridTrackFitContentSize, GridTrackMaxSize, GridTrackMinSize, GridTrackRepeatKind,
    GridTrackSize, LayoutLength, LayoutStyle, LineHeight, LinearGradient, NodePartStyles,
    NodeStyle, OverflowStyle, PartLayoutStyle, PartStyle, PositionStyle, RadialGradient, TextAlign,
    TextOverflow, TextSpacing, TextStyle, TextTransform, TransformStyle, TransitionProperty,
    TransitionStyle, TransitionTimingFunction, VisualStyle,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DgMediaEnvironment {
    pub width: f32,
    pub height: f32,
}

impl DgMediaEnvironment {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }

    pub fn from_physical_size(width: f32, height: f32, scale_factor: f32) -> Self {
        let scale_factor = scale_factor.max(0.001);
        Self::new(width / scale_factor, height / scale_factor)
    }

    fn orientation(self) -> DgMediaOrientation {
        if self.width > self.height {
            DgMediaOrientation::Landscape
        } else {
            DgMediaOrientation::Portrait
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgMediaCondition {
    expression: DgMediaExpression,
}

impl DgMediaCondition {
    fn new(expression: DgMediaExpression) -> Self {
        Self {
            expression: expression.simplified(),
        }
    }

    fn always() -> Self {
        Self::new(DgMediaExpression::Always)
    }

    fn and(self, other: Self) -> Self {
        Self::new(DgMediaExpression::And(vec![
            self.expression,
            other.expression,
        ]))
    }

    pub fn matches(&self, environment: Option<DgMediaEnvironment>) -> bool {
        self.expression.matches(environment)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum DgMediaExpression {
    Always,
    Never,
    Constraint(DgMediaConstraint),
    Orientation(DgMediaOrientation),
    And(Vec<DgMediaExpression>),
    Or(Vec<DgMediaExpression>),
    Not(Box<DgMediaExpression>),
}

impl DgMediaExpression {
    fn and(expressions: Vec<DgMediaExpression>) -> Self {
        Self::And(expressions).simplified()
    }

    fn or(expressions: Vec<DgMediaExpression>) -> Self {
        Self::Or(expressions).simplified()
    }

    fn simplified(self) -> Self {
        match self {
            DgMediaExpression::And(expressions) => {
                let mut out = Vec::new();
                for expression in expressions {
                    match expression.simplified() {
                        DgMediaExpression::Always => {}
                        DgMediaExpression::Never => return DgMediaExpression::Never,
                        DgMediaExpression::And(nested) => out.extend(nested),
                        expression => out.push(expression),
                    }
                }
                match out.len() {
                    0 => DgMediaExpression::Always,
                    1 => out.remove(0),
                    _ => DgMediaExpression::And(out),
                }
            }
            DgMediaExpression::Or(expressions) => {
                let mut out = Vec::new();
                for expression in expressions {
                    match expression.simplified() {
                        DgMediaExpression::Never => {}
                        DgMediaExpression::Always => return DgMediaExpression::Always,
                        DgMediaExpression::Or(nested) => out.extend(nested),
                        expression => out.push(expression),
                    }
                }
                match out.len() {
                    0 => DgMediaExpression::Never,
                    1 => out.remove(0),
                    _ => DgMediaExpression::Or(out),
                }
            }
            DgMediaExpression::Not(expression) => match expression.simplified() {
                DgMediaExpression::Always => DgMediaExpression::Never,
                DgMediaExpression::Never => DgMediaExpression::Always,
                DgMediaExpression::Not(nested) => *nested,
                expression => DgMediaExpression::Not(Box::new(expression)),
            },
            expression => expression,
        }
    }

    fn matches(&self, environment: Option<DgMediaEnvironment>) -> bool {
        match self {
            DgMediaExpression::Always => true,
            DgMediaExpression::Never => false,
            DgMediaExpression::Constraint(constraint) => constraint.matches(environment),
            DgMediaExpression::Orientation(expected) => {
                environment.is_some_and(|environment| environment.orientation() == *expected)
            }
            DgMediaExpression::And(expressions) => expressions
                .iter()
                .all(|expression| expression.matches(environment)),
            DgMediaExpression::Or(expressions) => expressions
                .iter()
                .any(|expression| expression.matches(environment)),
            DgMediaExpression::Not(expression) => !expression.matches(environment),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DgMediaConstraint {
    feature: DgMediaFeature,
    comparison: DgMediaComparison,
    value: f32,
}

impl DgMediaConstraint {
    fn matches(self, environment: Option<DgMediaEnvironment>) -> bool {
        let Some(environment) = environment else {
            return false;
        };
        let actual = match self.feature {
            DgMediaFeature::Width => environment.width,
            DgMediaFeature::Height => environment.height,
        };
        match self.comparison {
            DgMediaComparison::Equal => (actual - self.value).abs() <= f32::EPSILON,
            DgMediaComparison::GreaterThan => actual > self.value,
            DgMediaComparison::GreaterThanEqual => actual >= self.value,
            DgMediaComparison::LessThan => actual < self.value,
            DgMediaComparison::LessThanEqual => actual <= self.value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DgMediaFeature {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DgMediaOrientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DgMediaComparison {
    Equal,
    GreaterThan,
    GreaterThanEqual,
    LessThan,
    LessThanEqual,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgStyleRule {
    pub selector: DgSelector,
    pub declarations: Vec<DgStyleDeclaration>,
    pub specificity: Specificity,
    pub origin: StylesheetOrigin,
    pub source_order: u32,
    pub media: Option<DgMediaCondition>,
}

impl DgStyleRule {
    pub fn new(
        selector: DgSelector,
        declarations: Vec<DgStyleDeclaration>,
        origin: StylesheetOrigin,
        source_order: u32,
    ) -> Self {
        Self::with_media(selector, declarations, origin, source_order, None)
    }

    pub fn with_media(
        selector: DgSelector,
        declarations: Vec<DgStyleDeclaration>,
        origin: StylesheetOrigin,
        source_order: u32,
        media: Option<DgMediaCondition>,
    ) -> Self {
        let specificity = selector.specificity();
        Self {
            selector,
            declarations,
            specificity,
            origin,
            source_order,
            media,
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
    Transition(DgTransitionDeclaration),
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
    RowGap(DgCssLength),
    ColumnGap(DgCssLength),
    GridTemplateColumns(Vec<DgGridTrackSize>),
    GridTemplateRows(Vec<DgGridTrackSize>),
    GridColumn(DgGridPlacement),
    GridRow(DgGridPlacement),
    Overflow(DgCssKeyword),
    OverflowX(DgCssKeyword),
    OverflowY(DgCssKeyword),
    Position(DgCssKeyword),
    Top(DgCssLength),
    Right(DgCssLength),
    Bottom(DgCssLength),
    Left(DgCssLength),
    ZIndex(DgCssNumber),
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
    BackgroundNoise(DgCssNumber),
    BoxShadow(Vec<DgBoxShadow>),
    Transform(TransformStyle),
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32),
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
pub enum DgTransitionDeclaration {
    Property(Vec<TransitionProperty>),
    Duration(u64),
    Delay(u64),
    TimingFunction(TransitionTimingFunction),
    Shorthand(TransitionStyle),
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
    Calc(CalcLength),
    Auto,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgGridTrackSize {
    LogicalPx(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
    FitContent(DgGridTrackFitContentSize),
    MinMax {
        min: DgGridTrackMinSize,
        max: DgGridTrackMaxSize,
    },
    Repeat {
        kind: DgGridTrackRepeatKind,
        tracks: Vec<DgGridTrackSize>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgGridTrackRepeatKind {
    AutoFit,
    AutoFill,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DgGridTrackMinSize {
    LogicalPx(f32),
    Percent(f32),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DgGridTrackMaxSize {
    LogicalPx(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DgGridTrackFitContentSize {
    LogicalPx(f32),
    Percent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgGridLine {
    Auto,
    Line(i16),
    Span(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DgGridPlacement {
    pub start: DgGridLine,
    pub end: DgGridLine,
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
    Color(DgCssColor),
    LinearGradient(DgLinearGradient),
    RadialGradient(DgRadialGradient),
    Layers(Vec<DgBackgroundPaint>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgLinearGradient {
    pub angle_deg: f32,
    pub stops: Vec<DgGradientStop>,
    pub repeating: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgRadialGradient {
    pub stops: Vec<DgGradientStop>,
    pub repeating: bool,
    pub center: [f32; 2],
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
    None,
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

    #[cfg(test)]
    fn target_pseudo_classes(&self) -> &[DgPseudoClass] {
        match self {
            DgSelector::Root => &[],
            DgSelector::Compound(selector) => &selector.pseudo,
            DgSelector::Child { child, .. } => &child.pseudo,
            DgSelector::Chain(chain) => &chain.target.pseudo,
        }
    }

    fn target_contains_state_pseudo(&self) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.contains_state_pseudo(),
            DgSelector::Child { child, .. } => child.contains_state_pseudo(),
            DgSelector::Chain(chain) => chain.target.contains_state_pseudo(),
        }
    }

    fn target_contains_structural_pseudo(&self) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.contains_structural_pseudo(),
            DgSelector::Child { child, .. } => child.contains_structural_pseudo(),
            DgSelector::Chain(chain) => chain.target.contains_structural_pseudo(),
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
    pub attributes: Vec<DgAttributeSelector>,
    pub classes: Vec<String>,
    pub pseudo: Vec<DgPseudoClass>,
    pub structural: Vec<DgStructuralPseudo>,
    pub functions: Vec<DgSelectorFunction>,
    pub part: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DgAttributeSelector {
    pub name: String,
    pub operator: DgAttributeOperator,
    pub value: Option<String>,
    pub case_sensitivity: DgAttributeCaseSensitivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgAttributeOperator {
    Exists,
    Equals,
    Includes,
    Prefix,
    Suffix,
    Substring,
    DashMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgAttributeCaseSensitivity {
    Default,
    CaseSensitive,
    CaseInsensitive,
}

impl DgAttributeSelector {
    fn new(name: impl Into<String>, operator: DgAttributeOperator, value: Option<String>) -> Self {
        Self::new_with_case(name, operator, value, DgAttributeCaseSensitivity::Default)
    }

    fn new_with_case(
        name: impl Into<String>,
        operator: DgAttributeOperator,
        value: Option<String>,
        case_sensitivity: DgAttributeCaseSensitivity,
    ) -> Self {
        Self {
            name: name.into(),
            operator,
            value,
            case_sensitivity,
        }
    }

    fn matches(&self, attributes: &[StyleAttribute]) -> bool {
        let actual = attribute_value(attributes, &self.name);
        match self.operator {
            DgAttributeOperator::Exists => actual.is_some(),
            DgAttributeOperator::Equals => self
                .value
                .as_deref()
                .is_some_and(|expected| actual.is_some_and(|actual| self.eq(actual, expected))),
            DgAttributeOperator::Includes => self.value.as_deref().is_some_and(|expected| {
                actual.is_some_and(|actual| {
                    actual
                        .split_whitespace()
                        .any(|part| self.eq(part, expected))
                })
            }),
            DgAttributeOperator::Prefix => self.value.as_deref().is_some_and(|expected| {
                actual.is_some_and(|actual| self.starts_with(actual, expected))
            }),
            DgAttributeOperator::Suffix => self.value.as_deref().is_some_and(|expected| {
                actual.is_some_and(|actual| self.ends_with(actual, expected))
            }),
            DgAttributeOperator::Substring => self.value.as_deref().is_some_and(|expected| {
                actual.is_some_and(|actual| self.contains(actual, expected))
            }),
            DgAttributeOperator::DashMatch => self.value.as_deref().is_some_and(|expected| {
                actual.is_some_and(|actual| {
                    self.eq(actual, expected)
                        || self.starts_with(actual, expected)
                            && actual
                                .get(expected.len()..)
                                .is_some_and(|suffix| suffix.starts_with('-'))
                })
            }),
        }
    }

    fn eq(&self, actual: &str, expected: &str) -> bool {
        match self.case_sensitivity {
            DgAttributeCaseSensitivity::CaseInsensitive => actual.eq_ignore_ascii_case(expected),
            DgAttributeCaseSensitivity::Default | DgAttributeCaseSensitivity::CaseSensitive => {
                actual == expected
            }
        }
    }

    fn starts_with(&self, actual: &str, expected: &str) -> bool {
        match self.case_sensitivity {
            DgAttributeCaseSensitivity::CaseInsensitive => actual
                .get(..expected.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(expected)),
            DgAttributeCaseSensitivity::Default | DgAttributeCaseSensitivity::CaseSensitive => {
                actual.starts_with(expected)
            }
        }
    }

    fn ends_with(&self, actual: &str, expected: &str) -> bool {
        match self.case_sensitivity {
            DgAttributeCaseSensitivity::CaseInsensitive => actual
                .len()
                .checked_sub(expected.len())
                .and_then(|start| actual.get(start..))
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(expected)),
            DgAttributeCaseSensitivity::Default | DgAttributeCaseSensitivity::CaseSensitive => {
                actual.ends_with(expected)
            }
        }
    }

    fn contains(&self, actual: &str, expected: &str) -> bool {
        match self.case_sensitivity {
            DgAttributeCaseSensitivity::CaseInsensitive => actual
                .to_ascii_lowercase()
                .contains(&expected.to_ascii_lowercase()),
            DgAttributeCaseSensitivity::Default | DgAttributeCaseSensitivity::CaseSensitive => {
                actual.contains(expected)
            }
        }
    }

    fn label(&self) -> String {
        let mut label = String::new();
        label.push('[');
        label.push_str(&self.name);
        if let Some(value) = &self.value {
            label.push_str(match self.operator {
                DgAttributeOperator::Exists => "",
                DgAttributeOperator::Equals => "=\"",
                DgAttributeOperator::Includes => "~=\"",
                DgAttributeOperator::Prefix => "^=\"",
                DgAttributeOperator::Suffix => "$=\"",
                DgAttributeOperator::Substring => "*=\"",
                DgAttributeOperator::DashMatch => "|=\"",
            });
            if self.operator != DgAttributeOperator::Exists {
                label.push_str(value);
                label.push('"');
                match self.case_sensitivity {
                    DgAttributeCaseSensitivity::Default => {}
                    DgAttributeCaseSensitivity::CaseSensitive => label.push_str(" s"),
                    DgAttributeCaseSensitivity::CaseInsensitive => label.push_str(" i"),
                }
            }
        }
        label.push(']');
        label
    }
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

    pub fn with_attribute(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.push(DgAttributeSelector::new(
            name,
            DgAttributeOperator::Equals,
            Some(value.into()),
        ));
        self
    }

    pub fn with_attribute_operator(
        mut self,
        name: impl Into<String>,
        operator: DgAttributeOperator,
        value: Option<String>,
    ) -> Self {
        self.attributes
            .push(DgAttributeSelector::new(name, operator, value));
        self
    }

    pub fn with_attribute_case(
        mut self,
        name: impl Into<String>,
        operator: DgAttributeOperator,
        value: Option<String>,
        case_sensitivity: DgAttributeCaseSensitivity,
    ) -> Self {
        self.attributes.push(DgAttributeSelector::new_with_case(
            name,
            operator,
            value,
            case_sensitivity,
        ));
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

    pub fn with_function(mut self, function: DgSelectorFunction) -> Self {
        self.functions.push(function);
        self
    }

    pub fn with_part(mut self, part: impl Into<String>) -> Self {
        self.part = Some(part.into());
        self
    }

    pub fn specificity(&self) -> Specificity {
        let structural_specificity = self
            .structural
            .iter()
            .fold(Specificity::ZERO, |specificity, structural| {
                specificity.add(structural.specificity_extra())
            });
        let function_specificity = self
            .functions
            .iter()
            .fold(Specificity::ZERO, |specificity, function| {
                specificity.add(function.specificity())
            });
        Specificity {
            ids: u16::from(self.id.is_some()),
            classes: (self.classes.len()
                + self.pseudo.len()
                + self.structural.len()
                + usize::from(self.key.is_some())
                + self.attributes.len())
            .min(u16::MAX as usize) as u16,
            types: u16::from(self.type_selector.is_some()),
        }
        .add(structural_specificity)
        .add(function_specificity)
    }

    fn matches_element(&self, element: &StyleElement<'_>) -> bool {
        self.matches_identity(
            element.id,
            element.key,
            element.attributes,
            element.classes,
            element.kind,
        ) && self
            .pseudo
            .iter()
            .all(|pseudo| element.pseudo.contains(pseudo))
            && self
                .structural
                .iter()
                .all(|pseudo| pseudo.matches_element(element))
            && self
                .functions
                .iter()
                .all(|function| function.matches_element(element))
    }

    fn matches_ancestor(&self, ancestor: &StyleAncestor<'_>) -> bool {
        self.pseudo.is_empty()
            && self.structural.is_empty()
            && self.matches_identity(
                ancestor.id,
                ancestor.key,
                ancestor.attributes,
                ancestor.classes,
                ancestor.kind,
            )
            && self
                .functions
                .iter()
                .all(|function| function.matches_ancestor(ancestor))
    }

    fn matches_identity(
        &self,
        id: &str,
        key: Option<&str>,
        attributes: &[StyleAttribute],
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
        if !self
            .attributes
            .iter()
            .all(|expected| expected.matches(attributes))
        {
            return false;
        }
        self.classes
            .iter()
            .all(|expected| classes.iter().any(|class| class == expected))
    }

    fn contains_state_pseudo(&self) -> bool {
        !self.pseudo.is_empty()
            || self
                .functions
                .iter()
                .any(DgSelectorFunction::contains_state_pseudo)
    }

    fn contains_structural_pseudo(&self) -> bool {
        !self.structural.is_empty()
            || self
                .functions
                .iter()
                .any(DgSelectorFunction::contains_structural_pseudo)
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
        for attribute in &self.attributes {
            label.push_str(&attribute.label());
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
        for function in &self.functions {
            label.push(':');
            label.push_str(&function.label());
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DgSelectorFunction {
    pub kind: DgSelectorFunctionKind,
    pub selectors: Vec<DgCompoundSelector>,
}

impl DgSelectorFunction {
    fn specificity(&self) -> Specificity {
        match self.kind {
            DgSelectorFunctionKind::Where => Specificity::ZERO,
            DgSelectorFunctionKind::Not | DgSelectorFunctionKind::Is => self
                .selectors
                .iter()
                .map(DgCompoundSelector::specificity)
                .max()
                .unwrap_or(Specificity::ZERO),
        }
    }

    fn matches_element(&self, element: &StyleElement<'_>) -> bool {
        match self.kind {
            DgSelectorFunctionKind::Not => !self
                .selectors
                .iter()
                .any(|selector| selector.matches_element(element)),
            DgSelectorFunctionKind::Is | DgSelectorFunctionKind::Where => self
                .selectors
                .iter()
                .any(|selector| selector.matches_element(element)),
        }
    }

    fn matches_ancestor(&self, ancestor: &StyleAncestor<'_>) -> bool {
        match self.kind {
            DgSelectorFunctionKind::Not => !self
                .selectors
                .iter()
                .any(|selector| selector.matches_ancestor(ancestor)),
            DgSelectorFunctionKind::Is | DgSelectorFunctionKind::Where => self
                .selectors
                .iter()
                .any(|selector| selector.matches_ancestor(ancestor)),
        }
    }

    fn contains_state_pseudo(&self) -> bool {
        self.selectors
            .iter()
            .any(DgCompoundSelector::contains_state_pseudo)
    }

    fn contains_structural_pseudo(&self) -> bool {
        self.selectors
            .iter()
            .any(DgCompoundSelector::contains_structural_pseudo)
    }

    fn label(&self) -> String {
        let name = match self.kind {
            DgSelectorFunctionKind::Not => "not",
            DgSelectorFunctionKind::Is => "is",
            DgSelectorFunctionKind::Where => "where",
        };
        let selectors = self
            .selectors
            .iter()
            .map(DgCompoundSelector::label)
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name}({selectors})")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgSelectorFunctionKind {
    Not,
    Is,
    Where,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DgStructuralPseudo {
    FirstChild,
    LastChild,
    NthChild(DgNthChild),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DgNthChild {
    Odd,
    Even,
    Exact(usize),
    Formula {
        step: i64,
        offset: i64,
    },
    Of {
        pattern: Box<DgNthChild>,
        selectors: Vec<DgSelector>,
    },
}

impl DgStructuralPseudo {
    fn specificity_extra(&self) -> Specificity {
        match self {
            DgStructuralPseudo::NthChild(child) => child.specificity_extra(),
            _ => Specificity::ZERO,
        }
    }

    fn matches_element(&self, element: &StyleElement<'_>) -> bool {
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
            DgStructuralPseudo::NthChild(child) => child.matches_element(element, one_based),
        }
    }

    fn label(&self) -> String {
        match self {
            DgStructuralPseudo::FirstChild => "first-child".to_string(),
            DgStructuralPseudo::LastChild => "last-child".to_string(),
            DgStructuralPseudo::NthChild(child) => format!("nth-child({})", child.label()),
        }
    }
}

impl DgNthChild {
    fn specificity_extra(&self) -> Specificity {
        match self {
            DgNthChild::Of { selectors, .. } => selectors
                .iter()
                .map(DgSelector::specificity)
                .max()
                .unwrap_or(Specificity::ZERO),
            _ => Specificity::ZERO,
        }
    }

    fn matches_element(&self, element: &StyleElement<'_>, one_based: usize) -> bool {
        match self {
            DgNthChild::Odd => one_based % 2 == 1,
            DgNthChild::Even => one_based % 2 == 0,
            DgNthChild::Exact(expected) => *expected == one_based,
            DgNthChild::Formula { step, offset } => {
                nth_child_formula_matches(one_based as i64, *step, *offset)
            }
            DgNthChild::Of { pattern, selectors } => {
                nth_child_of_matches(pattern, selectors, element)
            }
        }
    }

    fn label(&self) -> String {
        match self {
            DgNthChild::Odd => "odd".to_string(),
            DgNthChild::Even => "even".to_string(),
            DgNthChild::Exact(index) => index.to_string(),
            DgNthChild::Formula { step, offset } => nth_child_formula_label(*step, *offset),
            DgNthChild::Of { pattern, selectors } => {
                let selectors = selectors
                    .iter()
                    .map(DgSelector::label)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{} of {selectors}", pattern.label())
            }
        }
    }
}

fn nth_child_of_matches(
    pattern: &DgNthChild,
    selectors: &[DgSelector],
    element: &StyleElement<'_>,
) -> bool {
    let (Some(siblings), Some(index)) = (element.siblings, element.sibling_index) else {
        return false;
    };
    if index >= siblings.len() {
        return false;
    }

    let mut filtered_index = 0usize;
    let mut current_matches = false;
    for (sibling_index, sibling) in siblings.iter().enumerate() {
        if sibling_matches_nth_child_filter(
            sibling,
            sibling_index,
            siblings.len(),
            element,
            selectors,
        ) {
            filtered_index += 1;
            if sibling_index == index {
                current_matches = true;
                break;
            }
        }
    }

    current_matches && pattern.matches_element(element, filtered_index)
}

fn sibling_matches_nth_child_filter(
    sibling: &StyleSibling,
    sibling_index: usize,
    sibling_count: usize,
    base: &StyleElement<'_>,
    selectors: &[DgSelector],
) -> bool {
    let classes: Vec<&str> = sibling.classes.iter().map(String::as_str).collect();
    let sibling_element = StyleElement {
        id: sibling.id.as_str(),
        key: sibling.key.as_deref(),
        attributes: &sibling.attributes,
        classes: &classes,
        kind: sibling.kind,
        ancestors: base.ancestors,
        pseudo: &[],
        sibling_index: Some(sibling_index),
        sibling_count: Some(sibling_count),
        siblings: base.siblings,
    };
    selectors
        .iter()
        .any(|selector| selector.matches(&sibling_element))
}

fn nth_child_formula_matches(index: i64, step: i64, offset: i64) -> bool {
    if index < 1 {
        return false;
    }
    if step == 0 {
        return index == offset;
    }
    if step > 0 {
        index >= offset && (index - offset) % step == 0
    } else {
        let span = step.checked_abs().unwrap_or(i64::MAX);
        index <= offset && (offset - index) % span == 0
    }
}

fn nth_child_formula_label(step: i64, offset: i64) -> String {
    if step == 0 {
        return offset.to_string();
    }

    let mut label = match step {
        1 => "n".to_string(),
        -1 => "-n".to_string(),
        _ => format!("{step}n"),
    };
    if offset > 0 {
        label.push('+');
        label.push_str(&offset.to_string());
    } else if offset < 0 {
        label.push_str(&offset.to_string());
    }
    label
}

#[derive(Debug, Clone, Copy)]
pub struct StyleElement<'a> {
    pub id: &'a str,
    pub key: Option<&'a str>,
    pub attributes: &'a [StyleAttribute],
    pub classes: &'a [&'a str],
    pub kind: WidgetKind,
    pub ancestors: &'a [StyleAncestor<'a>],
    pub pseudo: &'a [DgPseudoClass],
    pub sibling_index: Option<usize>,
    pub sibling_count: Option<usize>,
    pub siblings: Option<&'a [StyleSibling]>,
}

#[derive(Debug, Clone, Copy)]
pub struct StyleAncestor<'a> {
    pub id: &'a str,
    pub key: Option<&'a str>,
    pub attributes: &'a [StyleAttribute],
    pub classes: &'a [&'a str],
    pub kind: WidgetKind,
}

#[derive(Debug, Clone)]
pub struct StyleSibling {
    pub id: String,
    pub key: Option<String>,
    pub attributes: Vec<StyleAttribute>,
    pub classes: Vec<String>,
    pub kind: WidgetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StyleAttribute {
    pub name: String,
    pub value: String,
}

fn attribute_value<'a>(attributes: &'a [StyleAttribute], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.name == name)
        .map(|attribute| attribute.value.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DgStylePropertyName {
    Layout(DgLayoutPropertyName),
    Visual(DgVisualPropertyName),
    Text(DgTextPropertyName),
    Widget(DgWidgetPropertyName),
    Transition(DgTransitionPropertyName),
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
    RowGap,
    ColumnGap,
    GridTemplateColumns,
    GridTemplateRows,
    GridColumn,
    GridRow,
    Overflow,
    OverflowX,
    OverflowY,
    Position,
    Top,
    Right,
    Bottom,
    Left,
    ZIndex,
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
    BackgroundNoise,
    Transform,
    Translate,
    Scale,
    Rotate,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgTransitionPropertyName {
    Transition,
    Property,
    Duration,
    TimingFunction,
    Delay,
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
    /// padding-left, padding-right, padding-top, padding-bottom, margin, gap,
    /// overflow, position, top, right, bottom, left.
    ///
    /// Visual: background, background-color, foreground, border-color,
    /// border-width, border-radius, border-top-left-radius,
    /// border-top-right-radius, border-bottom-right-radius,
    /// border-bottom-left-radius, border, box-shadow, opacity, accent, track-color,
    /// thumb-color, transform, translate, scale, rotate.
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
            "row-gap" => Ok(Self::Layout(DgLayoutPropertyName::RowGap)),
            "column-gap" => Ok(Self::Layout(DgLayoutPropertyName::ColumnGap)),
            "grid-template-columns" => Ok(Self::Layout(DgLayoutPropertyName::GridTemplateColumns)),
            "grid-template-rows" => Ok(Self::Layout(DgLayoutPropertyName::GridTemplateRows)),
            "grid-column" => Ok(Self::Layout(DgLayoutPropertyName::GridColumn)),
            "grid-row" => Ok(Self::Layout(DgLayoutPropertyName::GridRow)),
            "overflow" => Ok(Self::Layout(DgLayoutPropertyName::Overflow)),
            "overflow-x" => Ok(Self::Layout(DgLayoutPropertyName::OverflowX)),
            "overflow-y" => Ok(Self::Layout(DgLayoutPropertyName::OverflowY)),
            "position" => Ok(Self::Layout(DgLayoutPropertyName::Position)),
            "top" => Ok(Self::Layout(DgLayoutPropertyName::Top)),
            "right" => Ok(Self::Layout(DgLayoutPropertyName::Right)),
            "bottom" => Ok(Self::Layout(DgLayoutPropertyName::Bottom)),
            "left" => Ok(Self::Layout(DgLayoutPropertyName::Left)),
            "z-index" => Ok(Self::Layout(DgLayoutPropertyName::ZIndex)),
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
            "background-noise" => Ok(Self::Visual(DgVisualPropertyName::BackgroundNoise)),
            "transform" => Ok(Self::Visual(DgVisualPropertyName::Transform)),
            "translate" => Ok(Self::Visual(DgVisualPropertyName::Translate)),
            "scale" => Ok(Self::Visual(DgVisualPropertyName::Scale)),
            "rotate" => Ok(Self::Visual(DgVisualPropertyName::Rotate)),
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
            "transition" => Ok(Self::Transition(DgTransitionPropertyName::Transition)),
            "transition-property" => Ok(Self::Transition(DgTransitionPropertyName::Property)),
            "transition-duration" => Ok(Self::Transition(DgTransitionPropertyName::Duration)),
            "transition-timing-function" => {
                Ok(Self::Transition(DgTransitionPropertyName::TimingFunction))
            }
            "transition-delay" => Ok(Self::Transition(DgTransitionPropertyName::Delay)),
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
    attributes: Vec<StyleAttribute>,
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
            attributes: node_style_attributes(node),
            kind: node.kind,
        }
    }
}

impl StyleSibling {
    fn from_node(node: &WidgetNode) -> Self {
        Self {
            id: node.id.clone(),
            key: node.key.clone(),
            classes: node_css_classes(node)
                .into_iter()
                .map(str::to_string)
                .collect(),
            attributes: node_style_attributes(node),
            kind: node.kind,
        }
    }
}

fn node_style_attributes(node: &WidgetNode) -> Vec<StyleAttribute> {
    let mut attributes = Vec::new();
    push_attr(&mut attributes, "id", &node.id);
    if let Some(kind) = css_type_name(node.kind) {
        push_attr(&mut attributes, "type", kind);
    }
    if let Some(key) = node.key.as_deref().filter(|value| !value.is_empty()) {
        push_attr(&mut attributes, "key", key);
    }
    if let Some(class_name) = node.class_name.as_deref().filter(|value| !value.is_empty()) {
        push_attr(&mut attributes, "class", class_name);
    }

    let props = &node.props;
    push_attr_opt(&mut attributes, "text", props.text.as_deref());
    push_attr_opt(&mut attributes, "badge", props.badge.as_deref());
    push_attr_opt(&mut attributes, "level", props.level.as_deref());
    push_attr_opt(&mut attributes, "placeholder", props.placeholder.as_deref());
    if let Some(value) = props.route_value.as_deref() {
        push_attr(&mut attributes, "value", value);
    } else {
        push_attr_number_opt(&mut attributes, "value", props.value);
    }
    push_attr_opt(&mut attributes, "page", props.page.as_deref());
    push_attr_opt(&mut attributes, "orientation", props.orientation.as_deref());
    push_attr_opt(&mut attributes, "target", props.target.as_deref());
    push_attr_opt(&mut attributes, "tooltip", props.tooltip.as_deref());
    push_attr_opt(&mut attributes, "path", props.image_path.as_deref());
    push_attr_opt(&mut attributes, "fit", props.image_fit.as_deref());
    push_attr_number_opt(&mut attributes, "width", props.fixed_width);
    push_attr_number_opt(&mut attributes, "height", props.fixed_height);
    push_attr_number_opt(&mut attributes, "min", props.min);
    push_attr_number_opt(&mut attributes, "max", props.max);
    push_attr_number_opt(&mut attributes, "step", props.step);
    push_attr_bool_if_true(&mut attributes, "disabled", props.disabled);
    push_attr_bool_opt(&mut attributes, "checked", props.checked);
    push_attr_bool_opt(&mut attributes, "expanded", props.expanded);
    push_attr_bool_opt(&mut attributes, "open", props.open);
    push_attr_bool_opt(&mut attributes, "wrap", props.wrap);
    if let Some(rows) = props.rows {
        push_attr(&mut attributes, "rows", &rows.to_string());
    }
    if let Some(page_size) = props.page_size {
        push_attr(&mut attributes, "page-size", &page_size.to_string());
    }
    if let Some(table_rows) = props.table_rows {
        push_attr(&mut attributes, "table-rows", &table_rows.to_string());
    }
    if !props.items.is_empty() {
        push_attr(
            &mut attributes,
            "items-count",
            &props.items.len().to_string(),
        );
    }
    attributes
}

fn push_attr(attributes: &mut Vec<StyleAttribute>, name: &str, value: &str) {
    attributes.push(StyleAttribute {
        name: name.to_string(),
        value: value.to_string(),
    });
}

fn push_attr_opt(attributes: &mut Vec<StyleAttribute>, name: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        push_attr(attributes, name, value);
    }
}

fn push_attr_number_opt(attributes: &mut Vec<StyleAttribute>, name: &str, value: Option<f32>) {
    if let Some(value) = value {
        push_attr(attributes, name, &value.to_string());
    }
}

fn push_attr_bool_if_true(attributes: &mut Vec<StyleAttribute>, name: &str, value: bool) {
    if value {
        push_attr(attributes, name, "true");
    }
}

fn push_attr_bool_opt(attributes: &mut Vec<StyleAttribute>, name: &str, value: Option<bool>) {
    if value == Some(true) {
        push_attr(attributes, name, "true");
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

fn selector_match_slots(
    selector: &DgSelector,
    base_element: &StyleElement<'_>,
) -> Vec<Option<DgPseudoClass>> {
    if !selector.target_contains_state_pseudo() {
        return selector
            .matches(base_element)
            .then_some(vec![None])
            .unwrap_or_default();
    }

    let mut slots = Vec::new();
    let base = StyleElement {
        pseudo: &[],
        ..*base_element
    };
    if selector.matches(&base) {
        slots.push(None);
    }
    for pseudo in STATIC_PSEUDO_CLASSES {
        let pseudos = [pseudo];
        let state = StyleElement {
            pseudo: &pseudos,
            ..*base_element
        };
        if selector.matches(&state) {
            slots.push(Some(pseudo));
        }
    }
    slots
}

pub fn apply_stylesheets_to_tree(root: &mut WidgetNode, store: &mut StylesheetStore) {
    apply_stylesheets_to_tree_with_media(root, store, None);
}

pub fn apply_stylesheets_to_tree_for_media(
    root: &mut WidgetNode,
    store: &mut StylesheetStore,
    media: DgMediaEnvironment,
) {
    apply_stylesheets_to_tree_with_media(root, store, Some(media));
}

fn apply_stylesheets_to_tree_with_media(
    root: &mut WidgetNode,
    store: &mut StylesheetStore,
    media: Option<DgMediaEnvironment>,
) {
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
            None,
            media,
        );
    }
    store.validation_warnings = validation_warnings;
}

pub fn matched_rule_labels_for_tree(
    root: &WidgetNode,
    store: &StylesheetStore,
) -> BTreeMap<String, Vec<String>> {
    matched_rule_labels_for_tree_with_media(root, store, None)
}

pub fn matched_rule_labels_for_tree_with_media(
    root: &WidgetNode,
    store: &StylesheetStore,
    media: Option<DgMediaEnvironment>,
) -> BTreeMap<String, Vec<String>> {
    let rules = store.all_rules();
    let mut ancestors = Vec::new();
    let mut out = BTreeMap::new();
    collect_matched_rule_labels(
        root,
        &rules,
        &mut ancestors,
        &mut out,
        None,
        None,
        None,
        media,
    );
    out
}

pub fn matched_part_rule_labels_for_tree(
    root: &WidgetNode,
    store: &StylesheetStore,
) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    matched_part_rule_labels_for_tree_with_media(root, store, None)
}

pub fn matched_part_rule_labels_for_tree_with_media(
    root: &WidgetNode,
    store: &StylesheetStore,
    media: Option<DgMediaEnvironment>,
) -> BTreeMap<String, BTreeMap<String, Vec<String>>> {
    let rules = store.all_rules();
    let mut ancestors = Vec::new();
    let mut out = BTreeMap::new();
    collect_matched_part_rule_labels(
        root,
        &rules,
        &mut ancestors,
        &mut out,
        None,
        None,
        None,
        media,
    );
    out
}

pub fn computed_style_for_virtual_element(
    kind: WidgetKind,
    id: &str,
    classes: &[&str],
    store: &StylesheetStore,
) -> NodeStyle {
    computed_style_for_virtual_element_with_media(kind, id, classes, store, None)
}

pub fn computed_style_for_virtual_element_with_media(
    kind: WidgetKind,
    id: &str,
    classes: &[&str],
    store: &StylesheetStore,
    media: Option<DgMediaEnvironment>,
) -> NodeStyle {
    let rules = store.all_rules();
    let element = StyleElement {
        id,
        key: None,
        attributes: &[],
        classes,
        kind,
        ancestors: &[],
        pseudo: &[],
        sibling_index: None,
        sibling_count: None,
        siblings: None,
    };
    let mut matched = Vec::new();
    for rule in rules.iter() {
        if !rule_matches_media(rule, media) {
            continue;
        }
        let slots = selector_match_slots(&rule.selector, &element);
        if !slots.is_empty() && rule.selector.target_part().is_none() {
            for slot in slots {
                matched.extend(rule.declarations.iter().map(|declaration| {
                    (rule.cascade_key(declaration), slot, &declaration.property)
                }));
            }
        }
    }
    matched.sort_by_key(|(key, _, _)| *key);

    let mut computed = NodeStyle::default();
    for (_, slot, property) in matched {
        match slot {
            Some(pseudo) => apply_property_to_pseudo_style(&mut computed, pseudo, property),
            None => apply_property_to_style(&mut computed, property),
        }
    }
    computed
}

fn rule_matches_media(rule: &DgStyleRule, media: Option<DgMediaEnvironment>) -> bool {
    rule.media
        .as_ref()
        .map(|condition| condition.matches(media))
        .unwrap_or(true)
}

fn collect_matched_part_rule_labels(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &mut Vec<AncestorSnapshot>,
    out: &mut BTreeMap<String, BTreeMap<String, Vec<String>>>,
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
) {
    let labels = matched_part_rule_labels_for_node(
        node,
        rules,
        ancestors,
        sibling_index,
        sibling_count,
        siblings,
        media,
    );
    if !labels.is_empty() {
        out.insert(node.id.clone(), labels);
    }
    ancestors.push(AncestorSnapshot::from_node(node));
    let child_count = node.children.len();
    let child_siblings: Vec<StyleSibling> =
        node.children.iter().map(StyleSibling::from_node).collect();
    for (index, child) in node.children.iter().enumerate() {
        collect_matched_part_rule_labels(
            child,
            rules,
            ancestors,
            out,
            Some(index),
            Some(child_count),
            Some(&child_siblings),
            media,
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
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
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
            attributes: &ancestor.attributes,
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let attributes = node_style_attributes(node);
    let element = StyleElement {
        id: node.id.as_str(),
        key: node.key.as_deref(),
        attributes: &attributes,
        classes: &classes,
        kind: node.kind,
        ancestors: &style_ancestors,
        pseudo: &[],
        sibling_index,
        sibling_count,
        siblings,
    };
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rule in rules
        .iter()
        .filter(|rule| rule_matches_media(rule, media))
        .filter(|rule| !selector_match_slots(&rule.selector, &element).is_empty())
    {
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
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
) {
    let labels = matched_rule_labels_for_node(
        node,
        rules,
        ancestors,
        sibling_index,
        sibling_count,
        siblings,
        media,
    );
    if !labels.is_empty() {
        out.insert(node.id.clone(), labels);
    }
    ancestors.push(AncestorSnapshot::from_node(node));
    let child_count = node.children.len();
    let child_siblings: Vec<StyleSibling> =
        node.children.iter().map(StyleSibling::from_node).collect();
    for (index, child) in node.children.iter().enumerate() {
        collect_matched_rule_labels(
            child,
            rules,
            ancestors,
            out,
            Some(index),
            Some(child_count),
            Some(&child_siblings),
            media,
        );
    }
    ancestors.pop();
}

fn matched_rule_labels_for_node(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    ancestors: &[AncestorSnapshot],
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
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
            attributes: &ancestor.attributes,
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let attributes = node_style_attributes(node);
    let element = StyleElement {
        id: node.id.as_str(),
        key: node.key.as_deref(),
        attributes: &attributes,
        classes: &classes,
        kind: node.kind,
        ancestors: &style_ancestors,
        pseudo: &[],
        sibling_index,
        sibling_count,
        siblings,
    };
    rules
        .iter()
        .filter(|rule| rule_matches_media(rule, media))
        .filter(|rule| !selector_match_slots(&rule.selector, &element).is_empty())
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
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
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
            attributes: &ancestor.attributes,
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let attributes = node_style_attributes(node);
    let element = StyleElement {
        id: node.id.as_str(),
        key: node.key.as_deref(),
        attributes: &attributes,
        classes: &classes,
        kind: node.kind,
        ancestors: &style_ancestors,
        pseudo: &[],
        sibling_index,
        sibling_count,
        siblings,
    };
    // Pseudo-state selectors are matched against base and single-state contexts
    // here. Their declarations are precomputed into hover/active/focus/disabled
    // style slots, and live widget state decides which slot is active.
    let mut matched = Vec::new();
    for rule in rules.iter() {
        if !rule_matches_media(rule, media) {
            continue;
        }
        let slots = selector_match_slots(&rule.selector, &element);
        if !slots.is_empty() {
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
            for slot in slots {
                matched.extend(rule.declarations.iter().map(|declaration| {
                    (
                        rule.cascade_key(declaration),
                        slot,
                        rule.selector.target_part(),
                        &declaration.property,
                    )
                }));
            }
        }
    }
    matched.sort_by_key(|(key, _, _, _)| *key);

    let mut computed = NodeStyle::default();
    for (_, slot, part, property) in matched {
        if let Some(part) = part {
            match slot {
                Some(pseudo) => {
                    apply_property_to_part_style(&mut computed, part, &[pseudo], property)
                }
                None => apply_property_to_part_style(&mut computed, part, &[], property),
            }
        } else if let Some(pseudo) = slot {
            apply_property_to_pseudo_style(&mut computed, pseudo, property);
        } else {
            apply_property_to_style(&mut computed, property);
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
    let child_siblings: Vec<StyleSibling> =
        node.children.iter().map(StyleSibling::from_node).collect();
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
            Some(&child_siblings),
            media,
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
    if !rule.selector.target_contains_state_pseudo() {
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
        WidgetKind::Panel => matches!(part, "accent" | "scrollbar-track" | "scrollbar-thumb"),
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
    merge_transition_style(&mut base.transition, &overlay.transition);
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
    base.width_value = overlay.width_value.or(base.width_value);
    base.height_value = overlay.height_value.or(base.height_value);
    base.min_width_value = overlay.min_width_value.or(base.min_width_value);
    base.min_height_value = overlay.min_height_value.or(base.min_height_value);
    base.max_width_value = overlay.max_width_value.or(base.max_width_value);
    base.max_height_value = overlay.max_height_value.or(base.max_height_value);
    base.padding = overlay.padding.or(base.padding);
    base.padding_left = overlay.padding_left.or(base.padding_left);
    base.padding_right = overlay.padding_right.or(base.padding_right);
    base.padding_top = overlay.padding_top.or(base.padding_top);
    base.padding_bottom = overlay.padding_bottom.or(base.padding_bottom);
    base.padding_value = overlay.padding_value.or(base.padding_value);
    base.padding_left_value = overlay.padding_left_value.or(base.padding_left_value);
    base.padding_right_value = overlay.padding_right_value.or(base.padding_right_value);
    base.padding_top_value = overlay.padding_top_value.or(base.padding_top_value);
    base.padding_bottom_value = overlay.padding_bottom_value.or(base.padding_bottom_value);
    base.margin = overlay.margin.or(base.margin);
    base.margin_value = overlay.margin_value.or(base.margin_value);
    base.gap = overlay.gap.or(base.gap);
    base.row_gap = overlay.row_gap.or(base.row_gap);
    base.column_gap = overlay.column_gap.or(base.column_gap);
    base.gap_value = overlay.gap_value.or(base.gap_value);
    base.row_gap_value = overlay.row_gap_value.or(base.row_gap_value);
    base.column_gap_value = overlay.column_gap_value.or(base.column_gap_value);
    base.overflow = overlay.overflow.or(base.overflow);
    base.overflow_x = overlay.overflow_x.or(base.overflow_x);
    base.overflow_y = overlay.overflow_y.or(base.overflow_y);
    base.position = overlay.position.or(base.position);
    base.top = overlay.top.or(base.top);
    base.right = overlay.right.or(base.right);
    base.bottom = overlay.bottom.or(base.bottom);
    base.left = overlay.left.or(base.left);
    base.z_index = overlay.z_index.or(base.z_index);
    base.flex_grow = overlay.flex_grow.or(base.flex_grow);
    base.flex_shrink = overlay.flex_shrink.or(base.flex_shrink);
    base.grid_template_columns = overlay
        .grid_template_columns
        .clone()
        .or_else(|| base.grid_template_columns.clone());
    base.grid_template_rows = overlay
        .grid_template_rows
        .clone()
        .or_else(|| base.grid_template_rows.clone());
    base.grid_column = overlay.grid_column.or(base.grid_column);
    base.grid_row = overlay.grid_row.or(base.grid_row);
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

fn merge_transition_style(
    base: &mut crate::style::TransitionStyle,
    overlay: &crate::style::TransitionStyle,
) {
    base.properties = overlay
        .properties
        .clone()
        .or_else(|| base.properties.clone());
    base.duration_ms = overlay.duration_ms.or(base.duration_ms);
    base.delay_ms = overlay.delay_ms.or(base.delay_ms);
    base.timing_function = overlay.timing_function.or(base.timing_function);
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
        DgStyleProperty::Transition(declaration) => {
            apply_transition_declaration(&mut style.transition, declaration)
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
        | DgStyleProperty::Transition(_)
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
        | DgStyleProperty::Transition(_)
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
        DgLayoutDeclaration::Width(value) => {
            style.width = length_px(value);
            style.width_value = layout_length(value);
        }
        DgLayoutDeclaration::Height(value) => {
            style.height = length_px(value);
            style.height_value = layout_length(value);
        }
        DgLayoutDeclaration::MinWidth(value) => {
            style.min_width = length_px(value);
            style.min_width_value = layout_length(value);
        }
        DgLayoutDeclaration::MinHeight(value) => {
            style.min_height = length_px(value);
            style.min_height_value = layout_length(value);
        }
        DgLayoutDeclaration::MaxWidth(value) => {
            style.max_width = length_px(value);
            style.max_width_value = layout_length(value);
        }
        DgLayoutDeclaration::MaxHeight(value) => {
            style.max_height = length_px(value);
            style.max_height_value = layout_length(value);
        }
        DgLayoutDeclaration::Padding(edges) => {
            style.padding_top = length_px(&edges.top);
            style.padding_right = length_px(&edges.right);
            style.padding_bottom = length_px(&edges.bottom);
            style.padding_left = length_px(&edges.left);
            style.padding_top_value = layout_length(&edges.top);
            style.padding_right_value = layout_length(&edges.right);
            style.padding_bottom_value = layout_length(&edges.bottom);
            style.padding_left_value = layout_length(&edges.left);
            if edges.top == edges.right && edges.right == edges.bottom && edges.bottom == edges.left
            {
                style.padding = length_px(&edges.top);
                style.padding_value = layout_length(&edges.top);
            } else {
                style.padding = None;
                style.padding_value = None;
            }
        }
        DgLayoutDeclaration::PaddingLeft(value) => {
            style.padding_left = length_px(value);
            style.padding_left_value = layout_length(value);
        }
        DgLayoutDeclaration::PaddingRight(value) => {
            style.padding_right = length_px(value);
            style.padding_right_value = layout_length(value);
        }
        DgLayoutDeclaration::PaddingTop(value) => {
            style.padding_top = length_px(value);
            style.padding_top_value = layout_length(value);
        }
        DgLayoutDeclaration::PaddingBottom(value) => {
            style.padding_bottom = length_px(value);
            style.padding_bottom_value = layout_length(value);
        }
        DgLayoutDeclaration::Margin(edges) => {
            if edges.top == edges.right && edges.right == edges.bottom && edges.bottom == edges.left
            {
                style.margin = length_px(&edges.top);
                style.margin_value = layout_length(&edges.top);
            }
        }
        DgLayoutDeclaration::Gap(value) => {
            style.gap = length_px(value);
            style.gap_value = layout_length(value);
        }
        DgLayoutDeclaration::RowGap(value) => {
            style.row_gap = length_px(value);
            style.row_gap_value = layout_length(value);
        }
        DgLayoutDeclaration::ColumnGap(value) => {
            style.column_gap = length_px(value);
            style.column_gap_value = layout_length(value);
        }
        DgLayoutDeclaration::GridTemplateColumns(value) => {
            style.grid_template_columns = Some(value.iter().map(grid_track_from_css).collect())
        }
        DgLayoutDeclaration::GridTemplateRows(value) => {
            style.grid_template_rows = Some(value.iter().map(grid_track_from_css).collect())
        }
        DgLayoutDeclaration::GridColumn(value) => {
            style.grid_column = Some(grid_placement_from_css(value))
        }
        DgLayoutDeclaration::GridRow(value) => {
            style.grid_row = Some(grid_placement_from_css(value))
        }
        DgLayoutDeclaration::Overflow(value) => style.overflow = overflow_from_keyword(value),
        DgLayoutDeclaration::OverflowX(value) => style.overflow_x = overflow_from_keyword(value),
        DgLayoutDeclaration::OverflowY(value) => style.overflow_y = overflow_from_keyword(value),
        DgLayoutDeclaration::Position(value) => style.position = position_from_keyword(value),
        DgLayoutDeclaration::Top(value) => style.top = length_px(value),
        DgLayoutDeclaration::Right(value) => style.right = length_px(value),
        DgLayoutDeclaration::Bottom(value) => style.bottom = length_px(value),
        DgLayoutDeclaration::Left(value) => style.left = length_px(value),
        DgLayoutDeclaration::ZIndex(value) => style.z_index = Some(value.0.round() as i32),
    }
}

fn grid_track_from_css(value: &DgGridTrackSize) -> GridTrackSize {
    match value {
        DgGridTrackSize::LogicalPx(value) => GridTrackSize::LogicalPx(*value),
        DgGridTrackSize::Percent(value) => GridTrackSize::Percent(*value),
        DgGridTrackSize::Fraction(value) => GridTrackSize::Fraction(*value),
        DgGridTrackSize::Auto => GridTrackSize::Auto,
        DgGridTrackSize::FitContent(value) => {
            GridTrackSize::FitContent(grid_track_fit_content_from_css(*value))
        }
        DgGridTrackSize::MinMax { min, max } => GridTrackSize::MinMax {
            min: grid_track_min_from_css(*min),
            max: grid_track_max_from_css(*max),
        },
        DgGridTrackSize::Repeat { kind, tracks } => GridTrackSize::Repeat {
            kind: grid_track_repeat_kind_from_css(*kind),
            tracks: tracks.iter().map(grid_track_from_css).collect(),
        },
    }
}

fn grid_track_repeat_kind_from_css(value: DgGridTrackRepeatKind) -> GridTrackRepeatKind {
    match value {
        DgGridTrackRepeatKind::AutoFit => GridTrackRepeatKind::AutoFit,
        DgGridTrackRepeatKind::AutoFill => GridTrackRepeatKind::AutoFill,
    }
}

fn grid_track_min_from_css(value: DgGridTrackMinSize) -> GridTrackMinSize {
    match value {
        DgGridTrackMinSize::LogicalPx(value) => GridTrackMinSize::LogicalPx(value),
        DgGridTrackMinSize::Percent(value) => GridTrackMinSize::Percent(value),
        DgGridTrackMinSize::Auto => GridTrackMinSize::Auto,
    }
}

fn grid_track_max_from_css(value: DgGridTrackMaxSize) -> GridTrackMaxSize {
    match value {
        DgGridTrackMaxSize::LogicalPx(value) => GridTrackMaxSize::LogicalPx(value),
        DgGridTrackMaxSize::Percent(value) => GridTrackMaxSize::Percent(value),
        DgGridTrackMaxSize::Fraction(value) => GridTrackMaxSize::Fraction(value),
        DgGridTrackMaxSize::Auto => GridTrackMaxSize::Auto,
    }
}

fn grid_track_fit_content_from_css(value: DgGridTrackFitContentSize) -> GridTrackFitContentSize {
    match value {
        DgGridTrackFitContentSize::LogicalPx(value) => GridTrackFitContentSize::LogicalPx(value),
        DgGridTrackFitContentSize::Percent(value) => GridTrackFitContentSize::Percent(value),
    }
}

fn grid_placement_from_css(value: &DgGridPlacement) -> GridPlacementStyle {
    GridPlacementStyle {
        start: grid_line_from_css(value.start),
        end: grid_line_from_css(value.end),
    }
}

fn grid_line_from_css(value: DgGridLine) -> GridLineStyle {
    match value {
        DgGridLine::Auto => GridLineStyle::Auto,
        DgGridLine::Line(value) => GridLineStyle::Line(value),
        DgGridLine::Span(value) => GridLineStyle::Span(value),
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
        DgVisualDeclaration::Border(border) => match border.style {
            DgBorderStyle::None => {
                style.border_width = Some(0.0);
                style.border_color = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]));
            }
            DgBorderStyle::Solid => {
                style.border_width = length_px(&border.width);
                style.border_color = Some(color_ref_from_css(&border.color));
            }
        },
        DgVisualDeclaration::Opacity(value) => style.opacity = Some(value.0.clamp(0.0, 1.0)),
        DgVisualDeclaration::Accent(value) => style.accent = Some(color_ref_from_css(value)),
        DgVisualDeclaration::TrackColor(value) => {
            style.track_color = Some(color_ref_from_css(value))
        }
        DgVisualDeclaration::ThumbColor(value) => {
            style.thumb_color = Some(color_ref_from_css(value))
        }
        DgVisualDeclaration::BackgroundNoise(value) => {
            style.background_noise = Some(value.0.clamp(0.0, 0.25))
        }
        DgVisualDeclaration::BoxShadow(value) => {
            style.box_shadows = Some(value.iter().filter_map(box_shadow_from_css).collect());
        }
        DgVisualDeclaration::Transform(value) => style.transform = Some(*value),
        DgVisualDeclaration::Translate(x, y) => {
            let transform = style.transform.get_or_insert_with(TransformStyle::default);
            transform.translate_x = *x;
            transform.translate_y = *y;
        }
        DgVisualDeclaration::Scale(x, y) => {
            let transform = style.transform.get_or_insert_with(TransformStyle::default);
            transform.scale_x = *x;
            transform.scale_y = *y;
        }
        DgVisualDeclaration::Rotate(value) => {
            let transform = style.transform.get_or_insert_with(TransformStyle::default);
            transform.rotate_deg = *value;
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

fn apply_transition_declaration(
    style: &mut TransitionStyle,
    declaration: &DgTransitionDeclaration,
) {
    match declaration {
        DgTransitionDeclaration::Property(value) => style.properties = Some(value.clone()),
        DgTransitionDeclaration::Duration(value) => style.duration_ms = Some(*value),
        DgTransitionDeclaration::Delay(value) => style.delay_ms = Some(*value),
        DgTransitionDeclaration::TimingFunction(value) => style.timing_function = Some(*value),
        DgTransitionDeclaration::Shorthand(value) => {
            if value.properties.is_some() {
                style.properties = value.properties.clone();
            }
            if value.duration_ms.is_some() {
                style.duration_ms = value.duration_ms;
            }
            if value.delay_ms.is_some() {
                style.delay_ms = value.delay_ms;
            }
            if value.timing_function.is_some() {
                style.timing_function = value.timing_function;
            }
        }
    }
}

fn display_from_keyword(value: &DgCssKeyword) -> Option<DisplayStyle> {
    match value.0.trim().to_ascii_lowercase().as_str() {
        "flex" => Some(DisplayStyle::Flex),
        "grid" => Some(DisplayStyle::Grid),
        "block" => Some(DisplayStyle::Block),
        "none" => Some(DisplayStyle::None),
        _ => None,
    }
}

fn parse_display_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssKeyword, DgStyleWarning> {
    let keyword = DgCssKeyword(resolve_keyword(value, variables));
    display_from_keyword(&keyword)
        .map(|_| keyword)
        .ok_or_else(|| parse_warning(name, value, "display value"))
}

fn overflow_from_keyword(value: &DgCssKeyword) -> Option<OverflowStyle> {
    match value.0.trim().to_ascii_lowercase().as_str() {
        "visible" => Some(OverflowStyle::Visible),
        "hidden" | "clip" => Some(OverflowStyle::Hidden),
        "scroll" => Some(OverflowStyle::Scroll),
        "auto" => Some(OverflowStyle::Auto),
        _ => None,
    }
}

fn parse_overflow_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssKeyword, DgStyleWarning> {
    let keyword = DgCssKeyword(resolve_keyword(value, variables));
    overflow_from_keyword(&keyword)
        .map(|_| keyword)
        .ok_or_else(|| parse_warning(name, value, "overflow value"))
}

fn position_from_keyword(value: &DgCssKeyword) -> Option<PositionStyle> {
    match value.0.trim().to_ascii_lowercase().as_str() {
        "static" => Some(PositionStyle::Static),
        "relative" => Some(PositionStyle::Relative),
        "absolute" => Some(PositionStyle::Absolute),
        "fixed" => Some(PositionStyle::Fixed),
        _ => None,
    }
}

fn parse_position_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssKeyword, DgStyleWarning> {
    let keyword = DgCssKeyword(resolve_keyword(value, variables));
    position_from_keyword(&keyword)
        .map(|_| keyword)
        .ok_or_else(|| parse_warning(name, value, "position value"))
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

fn parse_flex_direction_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssKeyword, DgStyleWarning> {
    let keyword = DgCssKeyword(resolve_keyword(value, variables));
    flex_direction_from_keyword(&keyword)
        .map(|_| keyword)
        .ok_or_else(|| parse_warning(name, value, "flex direction value"))
}

fn text_align_from_keyword(value: &DgCssKeyword) -> Option<TextAlign> {
    match value.0.trim().to_ascii_lowercase().as_str() {
        "left" | "start" => Some(TextAlign::Left),
        "center" | "middle" => Some(TextAlign::Center),
        "right" | "end" => Some(TextAlign::Right),
        _ => None,
    }
}

fn parse_text_align_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssKeyword, DgStyleWarning> {
    let keyword = DgCssKeyword(resolve_keyword(value, variables));
    text_align_from_keyword(&keyword)
        .map(|_| keyword)
        .ok_or_else(|| parse_warning(name, value, "text align value"))
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
        DgCssLength::Percent(_) | DgCssLength::Calc(_) | DgCssLength::Auto => None,
    }
}

fn line_height_from_css(value: &DgLineHeight) -> Option<LineHeight> {
    match value {
        DgLineHeight::Multiplier(value) => Some(LineHeight::Multiplier(value.max(0.0))),
        DgLineHeight::Length(DgCssLength::LogicalPx(value)) => {
            Some(LineHeight::LogicalPx(value.max(0.0)))
        }
        DgLineHeight::Length(DgCssLength::Em(value)) => Some(LineHeight::Multiplier(*value)),
        DgLineHeight::Length(DgCssLength::Percent(_))
        | DgLineHeight::Length(DgCssLength::Calc(_))
        | DgLineHeight::Length(DgCssLength::Auto) => None,
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
        DgBackgroundPaint::Color(color) => BackgroundPaint::Color(color_ref_from_css(color)),
        DgBackgroundPaint::Layers(layers) => BackgroundPaint::Layers(
            layers
                .iter()
                .map(background_paint_from_css)
                .collect::<Vec<_>>(),
        ),
        DgBackgroundPaint::LinearGradient(gradient) => {
            BackgroundPaint::LinearGradient(LinearGradient {
                angle_deg: gradient.angle_deg,
                repeating: gradient.repeating,
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
                repeating: gradient.repeating,
                center: gradient.center,
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
        DgCssLength::Em(_) | DgCssLength::Percent(_) | DgCssLength::Calc(_) | DgCssLength::Auto => {
            None
        }
    }
}

fn layout_length(value: &DgCssLength) -> Option<LayoutLength> {
    match value {
        DgCssLength::LogicalPx(value) => Some(LayoutLength::LogicalPx(*value)),
        DgCssLength::Percent(value) => Some(LayoutLength::Percent(*value)),
        DgCssLength::Calc(value) => Some(LayoutLength::Calc(*value)),
        DgCssLength::Auto => Some(LayoutLength::Auto),
        DgCssLength::Em(_) => None,
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
    collect_style_rules(
        &sheet.rules,
        origin,
        &variables,
        &mut warnings,
        &mut rules,
        &mut source_order,
        None,
    )?;

    Ok(ParsedStylesheet {
        rules,
        variables,
        warnings,
    })
}

fn collect_style_rules<R>(
    rules_list: &CssRuleList<'_, R>,
    origin: StylesheetOrigin,
    variables: &BTreeMap<String, DgCssValue>,
    warnings: &mut Vec<DgStyleWarning>,
    rules: &mut Vec<DgStyleRule>,
    source_order: &mut u32,
    media: Option<DgMediaCondition>,
) -> Result<(), DgCssParseError> {
    for rule in rules_list.0.iter() {
        match rule {
            CssRule::Style(style_rule) => {
                let selectors = selector_strings(&style_rule.selectors)?;
                let declaration_specs = lower_declarations(
                    &style_rule.declarations,
                    variables,
                    warnings,
                    selectors.first().map(String::as_str),
                )?;
                if declaration_specs.is_empty() {
                    continue;
                }
                for selector_text in selectors {
                    if selector_text == ":root" {
                        continue;
                    }
                    let Some(selector) = parse_selector(&selector_text, warnings) else {
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
                    rules.push(DgStyleRule::with_media(
                        selector,
                        declarations,
                        origin,
                        *source_order,
                        media.clone(),
                    ));
                    *source_order += 1;
                }
            }
            CssRule::Media(media_rule) => {
                let nested_media = match media_condition_from_list(&media_rule.query) {
                    Ok(condition) => media
                        .clone()
                        .map(|parent| parent.and(condition.clone()))
                        .unwrap_or(condition),
                    Err(message) => {
                        warnings.push(DgStyleWarning {
                            property: "@media".to_string(),
                            message,
                        });
                        continue;
                    }
                };
                collect_style_rules(
                    &media_rule.rules,
                    origin,
                    variables,
                    warnings,
                    rules,
                    source_order,
                    Some(nested_media),
                )?;
            }
            CssRule::Supports(supports_rule) => {
                if supports_condition_matches(&supports_rule.condition, variables) {
                    collect_style_rules(
                        &supports_rule.rules,
                        origin,
                        variables,
                        warnings,
                        rules,
                        source_order,
                        media.clone(),
                    )?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn supports_condition_matches(
    condition: &SupportsCondition<'_>,
    variables: &BTreeMap<String, DgCssValue>,
) -> bool {
    match condition {
        SupportsCondition::Not(condition) => !supports_condition_matches(condition, variables),
        SupportsCondition::And(conditions) => conditions
            .iter()
            .all(|condition| supports_condition_matches(condition, variables)),
        SupportsCondition::Or(conditions) => conditions
            .iter()
            .any(|condition| supports_condition_matches(condition, variables)),
        SupportsCondition::Declaration { property_id, value } => {
            supports_declaration_matches(property_id.name(), value.as_ref(), variables)
        }
        SupportsCondition::Selector(selector) => supports_selector_matches(selector.as_ref()),
        SupportsCondition::Unknown(_) => false,
    }
}

fn supports_declaration_matches(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> bool {
    lower_declaration(name, value, variables).is_ok_and(|property| property.is_some())
}

fn supports_selector_matches(selector: &str) -> bool {
    let mut warnings = Vec::new();
    parse_selector(selector, &mut warnings).is_some()
}

fn media_condition_from_list(media_list: &MediaList<'_>) -> Result<DgMediaCondition, String> {
    if media_list.media_queries.is_empty() {
        return Ok(DgMediaCondition::always());
    }
    let expressions = media_list
        .media_queries
        .iter()
        .map(media_query_expression)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DgMediaCondition::new(DgMediaExpression::or(expressions)))
}

fn media_query_expression(query: &MediaQuery<'_>) -> Result<DgMediaExpression, String> {
    let media_type = match &query.media_type {
        MediaType::All | MediaType::Screen => DgMediaExpression::Always,
        MediaType::Print => DgMediaExpression::Never,
        MediaType::Custom(media_type) => {
            return Err(format!(
                "unsupported @media type {media_type:?}; only screen/all width, height, and orientation queries are supported"
            ));
        }
    };
    let condition = query
        .condition
        .as_ref()
        .map(media_condition_expression)
        .transpose()?
        .unwrap_or(DgMediaExpression::Always);
    let expression = DgMediaExpression::and(vec![media_type, condition]);
    Ok(match query.qualifier {
        Some(lightningcss::media_query::Qualifier::Not) => {
            DgMediaExpression::Not(Box::new(expression))
        }
        Some(lightningcss::media_query::Qualifier::Only) | None => expression,
    })
}

fn media_condition_expression(
    condition: &LightningMediaCondition<'_>,
) -> Result<DgMediaExpression, String> {
    match condition {
        LightningMediaCondition::Feature(feature) => media_feature_expression(feature),
        LightningMediaCondition::Not(condition) => Ok(DgMediaExpression::Not(Box::new(
            media_condition_expression(condition)?,
        ))),
        LightningMediaCondition::Operation {
            conditions,
            operator,
        } => {
            let expressions = conditions
                .iter()
                .map(media_condition_expression)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(match operator {
                Operator::And => DgMediaExpression::and(expressions),
                Operator::Or => DgMediaExpression::or(expressions),
            })
        }
        LightningMediaCondition::Unknown(_) => Err(
            "unsupported @media condition; only width, height, and orientation queries are supported"
                .to_string(),
        ),
    }
}

fn media_feature_expression(
    feature: &lightningcss::media_query::MediaFeature<'_>,
) -> Result<DgMediaExpression, String> {
    match feature {
        QueryFeature::Plain { name, value } => media_plain_feature_expression(name, value),
        QueryFeature::Range {
            name,
            operator,
            value,
        } => media_constraint_expression(name, media_comparison(*operator), value),
        QueryFeature::Interval {
            name,
            start,
            start_operator,
            end,
            end_operator,
        } => Ok(DgMediaExpression::and(vec![
            media_constraint_expression(
                name,
                media_comparison_for_interval_start(*start_operator),
                start,
            )?,
            media_constraint_expression(name, media_comparison(*end_operator), end)?,
        ])),
        QueryFeature::Boolean { name } => Err(format!(
            "unsupported @media feature {feature}; only width, height, and orientation queries are supported",
            feature = media_feature_name_label(name)
        )),
    }
}

fn media_plain_feature_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::Orientation)
    ) {
        return media_orientation_expression(name, value);
    }
    media_constraint_expression(name, DgMediaComparison::Equal, value)
}

fn media_orientation_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only portrait and landscape are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let orientation = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("portrait") => DgMediaOrientation::Portrait,
        value if value.eq_ignore_ascii_case("landscape") => DgMediaOrientation::Landscape,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only portrait and landscape are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::Orientation(orientation))
}

fn media_constraint_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    comparison: DgMediaComparison,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let feature = media_feature_name(name).ok_or_else(|| {
        format!(
            "unsupported @media feature {feature}; only width, height, and orientation queries are supported",
            feature = media_feature_name_label(name)
        )
    })?;
    let value = media_feature_length_px(value).ok_or_else(|| {
        format!(
            "unsupported @media value for {feature}; only absolute length values are supported",
            feature = media_feature_name_label(name)
        )
    })?;
    Ok(DgMediaExpression::Constraint(DgMediaConstraint {
        feature,
        comparison,
        value,
    }))
}

fn media_feature_name(name: &MediaFeatureName<'_, MediaFeatureId>) -> Option<DgMediaFeature> {
    match name {
        MediaFeatureName::Standard(MediaFeatureId::Width) => Some(DgMediaFeature::Width),
        MediaFeatureName::Standard(MediaFeatureId::Height) => Some(DgMediaFeature::Height),
        _ => None,
    }
}

fn media_feature_name_label(name: &MediaFeatureName<'_, MediaFeatureId>) -> String {
    match name {
        MediaFeatureName::Standard(MediaFeatureId::Width) => "width".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Height) => "height".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Orientation) => "orientation".to_string(),
        other => format!("{other:?}"),
    }
}

fn media_feature_length_px(value: &MediaFeatureValue<'_>) -> Option<f32> {
    match value {
        MediaFeatureValue::Length(length) => length.to_px(),
        _ => None,
    }
}

fn media_comparison(comparison: MediaFeatureComparison) -> DgMediaComparison {
    match comparison {
        MediaFeatureComparison::Equal => DgMediaComparison::Equal,
        MediaFeatureComparison::GreaterThan => DgMediaComparison::GreaterThan,
        MediaFeatureComparison::GreaterThanEqual => DgMediaComparison::GreaterThanEqual,
        MediaFeatureComparison::LessThan => DgMediaComparison::LessThan,
        MediaFeatureComparison::LessThanEqual => DgMediaComparison::LessThanEqual,
    }
}

fn media_comparison_for_interval_start(comparison: MediaFeatureComparison) -> DgMediaComparison {
    match comparison {
        MediaFeatureComparison::Equal => DgMediaComparison::Equal,
        MediaFeatureComparison::GreaterThan => DgMediaComparison::LessThan,
        MediaFeatureComparison::GreaterThanEqual => DgMediaComparison::LessThanEqual,
        MediaFeatureComparison::LessThan => DgMediaComparison::GreaterThan,
        MediaFeatureComparison::LessThanEqual => DgMediaComparison::GreaterThanEqual,
    }
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
                message: "only `border: none` or `border: <width> solid <color>` is supported"
                    .to_string(),
            })?;
            Ok(Some(DgStyleProperty::Visual(DgVisualDeclaration::Border(
                border,
            ))))
        }
        DgStylePropertyName::Layout(property) => lower_layout(name, property, value, variables),
        DgStylePropertyName::Visual(property) => lower_visual(name, property, value, variables),
        DgStylePropertyName::Text(property) => lower_text(name, property, value, variables),
        DgStylePropertyName::Widget(property) => lower_widget(name, property, value, variables),
        DgStylePropertyName::Transition(property) => {
            lower_transition(name, property, value, variables)
        }
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
            DgLayoutDeclaration::Display(parse_display_value(name, value, variables)?)
        }
        DgLayoutPropertyName::FlexDirection => {
            DgLayoutDeclaration::FlexDirection(parse_flex_direction_value(name, value, variables)?)
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
            DgLayoutDeclaration::Width(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Height => {
            DgLayoutDeclaration::Height(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MinWidth => {
            DgLayoutDeclaration::MinWidth(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MinHeight => {
            DgLayoutDeclaration::MinHeight(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MaxWidth => {
            DgLayoutDeclaration::MaxWidth(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MaxHeight => {
            DgLayoutDeclaration::MaxHeight(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Padding => {
            DgLayoutDeclaration::Padding(parse_spacing_box_edges(name, value, variables)?)
        }
        DgLayoutPropertyName::PaddingLeft => {
            DgLayoutDeclaration::PaddingLeft(parse_spacing_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::PaddingRight => {
            DgLayoutDeclaration::PaddingRight(parse_spacing_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::PaddingTop => {
            DgLayoutDeclaration::PaddingTop(parse_spacing_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::PaddingBottom => {
            DgLayoutDeclaration::PaddingBottom(parse_spacing_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Margin => {
            let edges = parse_layout_box_edges(name, value, variables)?;
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
            DgLayoutDeclaration::Gap(parse_spacing_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::RowGap => {
            DgLayoutDeclaration::RowGap(parse_spacing_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::ColumnGap => {
            DgLayoutDeclaration::ColumnGap(parse_spacing_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::GridTemplateColumns => DgLayoutDeclaration::GridTemplateColumns(
            parse_grid_template_value(name, value, variables)?,
        ),
        DgLayoutPropertyName::GridTemplateRows => DgLayoutDeclaration::GridTemplateRows(
            parse_grid_template_value(name, value, variables)?,
        ),
        DgLayoutPropertyName::GridColumn => {
            DgLayoutDeclaration::GridColumn(parse_grid_placement_value(name, value)?)
        }
        DgLayoutPropertyName::GridRow => {
            DgLayoutDeclaration::GridRow(parse_grid_placement_value(name, value)?)
        }
        DgLayoutPropertyName::Overflow => {
            DgLayoutDeclaration::Overflow(parse_overflow_value(name, value, variables)?)
        }
        DgLayoutPropertyName::OverflowX => {
            DgLayoutDeclaration::OverflowX(parse_overflow_value(name, value, variables)?)
        }
        DgLayoutPropertyName::OverflowY => {
            DgLayoutDeclaration::OverflowY(parse_overflow_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Position => {
            DgLayoutDeclaration::Position(parse_position_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Top => {
            DgLayoutDeclaration::Top(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Right => {
            DgLayoutDeclaration::Right(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Bottom => {
            DgLayoutDeclaration::Bottom(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::Left => {
            DgLayoutDeclaration::Left(parse_px_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::ZIndex => {
            DgLayoutDeclaration::ZIndex(parse_number_value(name, value, variables)?)
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
        DgVisualPropertyName::BackgroundNoise => {
            DgVisualDeclaration::BackgroundNoise(parse_number_value(name, value, variables)?)
        }
        DgVisualPropertyName::Transform => {
            DgVisualDeclaration::Transform(parse_transform_value(name, value, variables)?)
        }
        DgVisualPropertyName::Translate => {
            let (x, y) = parse_translate_value(name, value, variables)?;
            DgVisualDeclaration::Translate(x, y)
        }
        DgVisualPropertyName::Scale => {
            let (x, y) = parse_scale_value(name, value, variables)?;
            DgVisualDeclaration::Scale(x, y)
        }
        DgVisualPropertyName::Rotate => {
            DgVisualDeclaration::Rotate(parse_rotate_value(name, value, variables)?)
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
            DgTextDeclaration::TextAlign(parse_text_align_value(name, value, variables)?)
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

fn lower_transition(
    name: &str,
    property: DgTransitionPropertyName,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgStyleProperty>, DgStyleWarning> {
    let declaration = match property {
        DgTransitionPropertyName::Transition => {
            DgTransitionDeclaration::Shorthand(parse_transition_shorthand(name, value, variables)?)
        }
        DgTransitionPropertyName::Property => DgTransitionDeclaration::Property(
            parse_transition_property_list(name, value, variables)?,
        ),
        DgTransitionPropertyName::Duration => {
            DgTransitionDeclaration::Duration(parse_time_ms_value(name, value, variables)?)
        }
        DgTransitionPropertyName::Delay => {
            DgTransitionDeclaration::Delay(parse_time_ms_value(name, value, variables)?)
        }
        DgTransitionPropertyName::TimingFunction => {
            let keyword = resolve_keyword(value, variables);
            let timing = transition_timing_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "transition timing function"))?;
            DgTransitionDeclaration::TimingFunction(timing)
        }
    };
    Ok(Some(DgStyleProperty::Transition(declaration)))
}

fn parse_transition_shorthand(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<TransitionStyle, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    let first = split_selector_list(&value)
        .into_iter()
        .next()
        .unwrap_or_else(|| value.clone())
        .trim()
        .to_string();
    if first.eq_ignore_ascii_case("none") {
        return Ok(TransitionStyle {
            properties: Some(Vec::new()),
            duration_ms: Some(0),
            delay_ms: Some(0),
            timing_function: None,
        });
    }

    let mut style = TransitionStyle::default();
    let mut saw_duration = false;
    let tokens = split_css_whitespace_tokens(&first)
        .ok_or_else(|| parse_warning(name, value.as_str(), "transition shorthand"))?;
    for token in tokens {
        if let Some(timing) = transition_timing_from_keyword(&token) {
            style.timing_function = Some(timing);
            continue;
        }
        if let Some(time) = parse_time_ms(&token) {
            if !saw_duration {
                style.duration_ms = Some(time);
                saw_duration = true;
            } else {
                style.delay_ms = Some(time);
            }
            continue;
        }
        if let Some(property) = transition_property_from_keyword(&token) {
            style.properties.get_or_insert_with(Vec::new).push(property);
            continue;
        }
        return Err(parse_warning(name, value.as_str(), "transition shorthand"));
    }
    if style.duration_ms.is_none() {
        return Err(parse_warning(name, value.as_str(), "transition duration"));
    }
    Ok(style)
}

fn parse_transition_property_list(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Vec<TransitionProperty>, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let mut properties = Vec::new();
    for property in split_selector_list(&value) {
        let Some(property) = transition_property_from_keyword(&property) else {
            return Err(parse_warning(
                name,
                value.as_str(),
                "transition property list",
            ));
        };
        if !properties.contains(&property) {
            properties.push(property);
        }
    }
    if properties.is_empty() {
        Err(parse_warning(
            name,
            value.as_str(),
            "transition property list",
        ))
    } else {
        Ok(properties)
    }
}

fn parse_time_ms_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<u64, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    let first = value.split(',').next().unwrap_or(value.as_str()).trim();
    parse_time_ms(first).ok_or_else(|| parse_warning(name, value.as_str(), "time"))
}

fn parse_time_ms(value: &str) -> Option<u64> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(ms) = value.strip_suffix("ms") {
        return ms
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| v.max(0.0).round() as u64);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds
            .trim()
            .parse::<f32>()
            .ok()
            .map(|v| (v.max(0.0) * 1000.0).round() as u64);
    }
    None
}

fn transition_timing_from_keyword(value: &str) -> Option<TransitionTimingFunction> {
    TransitionTimingFunction::parse(value)
}

fn transition_property_from_keyword(value: &str) -> Option<TransitionProperty> {
    match value.trim().to_ascii_lowercase().as_str() {
        "all" => Some(TransitionProperty::All),
        "background" | "background-color" => Some(TransitionProperty::Background),
        "foreground" => Some(TransitionProperty::Foreground),
        "border-color" => Some(TransitionProperty::BorderColor),
        "border-width" => Some(TransitionProperty::BorderWidth),
        "border-radius" => Some(TransitionProperty::BorderRadius),
        "opacity" => Some(TransitionProperty::Opacity),
        "color" => Some(TransitionProperty::Color),
        "accent" => Some(TransitionProperty::Accent),
        "track-color" => Some(TransitionProperty::TrackColor),
        "thumb-color" => Some(TransitionProperty::ThumbColor),
        "box-shadow" => Some(TransitionProperty::BoxShadow),
        "transform" | "translate" | "scale" | "rotate" => Some(TransitionProperty::Transform),
        _ => None,
    }
}

fn parse_transform_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<TransformStyle, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(TransformStyle::default());
    }
    let transform = parse_transform_functions(&value)
        .ok_or_else(|| parse_warning(name, value.as_str(), "transform"))?;
    Ok(transform)
}

fn parse_translate_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<(f32, f32), DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok((0.0, 0.0));
    }
    let args = split_transform_args(&value);
    if args.is_empty() || args.len() > 2 {
        return Err(parse_warning(name, value.as_str(), "translate"));
    }
    let x = parse_transform_length(args[0])
        .ok_or_else(|| parse_warning(name, value.as_str(), "translate"))?;
    let y = args
        .get(1)
        .and_then(|arg| parse_transform_length(arg))
        .unwrap_or(0.0);
    Ok((x, y))
}

fn parse_scale_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<(f32, f32), DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok((1.0, 1.0));
    }
    let args = split_transform_args(&value);
    if args.is_empty() || args.len() > 2 {
        return Err(parse_warning(name, value.as_str(), "scale"));
    }
    let x = parse_transform_number(args[0])
        .ok_or_else(|| parse_warning(name, value.as_str(), "scale"))?;
    let y = args
        .get(1)
        .and_then(|arg| parse_transform_number(arg))
        .unwrap_or(x);
    Ok((x, y))
}

fn parse_rotate_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<f32, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(0.0);
    }
    parse_transform_angle(&value).ok_or_else(|| parse_warning(name, value.as_str(), "rotate"))
}

fn parse_transform_functions(value: &str) -> Option<TransformStyle> {
    let mut rest = value.trim();
    let mut transform = TransformStyle::default();
    while !rest.is_empty() {
        rest = rest.trim_start();
        let open = rest.find('(')?;
        let name = rest[..open].trim().to_ascii_lowercase();
        let after_open = &rest[open + 1..];
        let close = after_open.find(')')?;
        let args = &after_open[..close];
        apply_transform_function(&mut transform, &name, args)?;
        rest = after_open[close + 1..].trim_start();
    }
    Some(transform)
}

fn apply_transform_function(transform: &mut TransformStyle, name: &str, args: &str) -> Option<()> {
    match name {
        "translate" => {
            let args = split_transform_args(args);
            let x = parse_transform_length(args.first()?)?;
            let y = args
                .get(1)
                .and_then(|value| parse_transform_length(value))
                .unwrap_or(0.0);
            transform.translate_x += x;
            transform.translate_y += y;
        }
        "translatex" => {
            transform.translate_x += parse_transform_length(args)?;
        }
        "translatey" => {
            transform.translate_y += parse_transform_length(args)?;
        }
        "scale" => {
            let args = split_transform_args(args);
            let x = parse_transform_number(args.first()?)?;
            let y = args
                .get(1)
                .and_then(|value| parse_transform_number(value))
                .unwrap_or(x);
            transform.scale_x *= x;
            transform.scale_y *= y;
        }
        "scalex" => {
            transform.scale_x *= parse_transform_number(args)?;
        }
        "scaley" => {
            transform.scale_y *= parse_transform_number(args)?;
        }
        "rotate" => {
            transform.rotate_deg += parse_transform_angle(args)?;
        }
        _ => return None,
    }
    Some(())
}

fn split_transform_args(args: &str) -> Vec<&str> {
    if args.contains(',') {
        args.split(',')
            .map(str::trim)
            .filter(|arg| !arg.is_empty())
            .collect()
    } else {
        args.split_whitespace().collect()
    }
}

fn parse_transform_length(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(px) = value.strip_suffix("px") {
        return px.trim().parse().ok();
    }
    value.parse().ok()
}

fn parse_transform_number(value: &str) -> Option<f32> {
    value.trim().parse().ok()
}

fn parse_transform_angle(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(deg) = value.strip_suffix("deg") {
        return deg.trim().parse().ok();
    }
    if let Some(rad) = value.strip_suffix("rad") {
        return rad
            .trim()
            .parse::<f32>()
            .ok()
            .map(|radians| radians.to_degrees());
    }
    if let Some(turn) = value.strip_suffix("turn") {
        return turn.trim().parse::<f32>().ok().map(|turns| turns * 360.0);
    }
    value.parse().ok()
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
            && (!compound.pseudo.is_empty()
                || !compound.structural.is_empty()
                || compound.contains_state_pseudo())
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
    if selector.is_empty() || contains_top_level_whitespace(selector) {
        return None;
    }
    let (selector, part) = match split_target_part(selector)? {
        Some((target, part)) => {
            if target.is_empty() || !is_part_name(part) {
                return None;
            }
            (target, Some(part))
        }
        None => (selector, None),
    };
    let mut compound = DgCompoundSelector::new();
    compound.part = part.map(str::to_string);
    let mut rest = selector;
    if let Some(tail) = rest.strip_prefix('*') {
        if tail
            .chars()
            .next()
            .is_some_and(|ch| !matches!(ch, '.' | '#' | ':' | '['))
        {
            return None;
        }
        rest = tail;
    }

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
        } else if prefix == ":" {
            let next = pseudo_selector_len(rest)?;
            let value = &rest[1..next];
            parse_pseudo_selector(value, &mut compound)?;
            rest = &rest[next..];
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
                _ => return None,
            }
            rest = &rest[next..];
        }
    }
    Some(compound)
}

fn contains_top_level_whitespace(value: &str) -> bool {
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;
    for ch in value.chars() {
        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            _ if ch.is_ascii_whitespace() && bracket_depth == 0 && paren_depth == 0 => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn split_target_part(selector: &str) -> Option<Option<(&str, &str)>> {
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;
    let mut chars = selector.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.checked_sub(1)?,
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.checked_sub(1)?,
            ':' if bracket_depth == 0 && paren_depth == 0 => {
                if chars.peek().is_some_and(|(_, next)| *next == ':') {
                    chars.next();
                    let target = &selector[..idx];
                    let part = &selector[idx + 2..];
                    if part.contains("::") {
                        return None;
                    }
                    return Some(Some((target, part)));
                }
            }
            _ => {}
        }
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return None;
    }
    Some(None)
}

fn pseudo_selector_len(rest: &str) -> Option<usize> {
    let tail = rest.strip_prefix(':')?;
    let name_len = tail
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_alphanumeric() || *ch == '-')
        .map(|(idx, ch)| idx + ch.len_utf8())
        .last()?;
    let after_name = &tail[name_len..];
    if !after_name.starts_with('(') {
        return Some(1 + name_len);
    }

    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    for (idx, ch) in after_name.char_indices() {
        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(1 + name_len + idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_attribute_selector(value: &str, compound: &mut DgCompoundSelector) -> Option<()> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let (name, operator, raw_value) = parse_attribute_operator(value)?;
    let name = name.trim().to_ascii_lowercase();
    if !is_attribute_name(&name) {
        return None;
    }

    let (parsed_value, case_sensitivity) = if operator == DgAttributeOperator::Exists {
        if raw_value.is_some() {
            return None;
        }
        (None, DgAttributeCaseSensitivity::Default)
    } else {
        let (value, case_sensitivity) = parse_attribute_value(raw_value?)?;
        (Some(value), case_sensitivity)
    };

    if name == "key"
        && operator == DgAttributeOperator::Equals
        && case_sensitivity == DgAttributeCaseSensitivity::Default
    {
        compound.key = parsed_value;
    } else {
        compound.attributes.push(DgAttributeSelector::new_with_case(
            name,
            operator,
            parsed_value,
            case_sensitivity,
        ));
    }
    Some(())
}

fn parse_attribute_value(value: &str) -> Option<(String, DgAttributeCaseSensitivity)> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('"') || value.starts_with('\'') {
        let quote = value.chars().next()?;
        let close = value[quote.len_utf8()..].find(quote)? + quote.len_utf8();
        let parsed = &value[quote.len_utf8()..close];
        if parsed.is_empty() {
            return None;
        }
        let case_sensitivity = parse_attribute_case_flag(&value[close + quote.len_utf8()..])?;
        return Some((parsed.to_string(), case_sensitivity));
    }
    if value.chars().any(|ch| ch == '"' || ch == '\'') {
        return None;
    }
    let mut parts = value.split_ascii_whitespace();
    let parsed = parts.next()?;
    if parsed.is_empty() {
        return None;
    }
    let case_sensitivity = match (parts.next(), parts.next()) {
        (None, None) => DgAttributeCaseSensitivity::Default,
        (Some(flag), None) => parse_attribute_case_flag(flag)?,
        _ => return None,
    };
    Some((parsed.to_string(), case_sensitivity))
}

fn parse_attribute_case_flag(value: &str) -> Option<DgAttributeCaseSensitivity> {
    match value.trim() {
        "" => Some(DgAttributeCaseSensitivity::Default),
        "i" | "I" => Some(DgAttributeCaseSensitivity::CaseInsensitive),
        "s" | "S" => Some(DgAttributeCaseSensitivity::CaseSensitive),
        _ => None,
    }
}

fn parse_attribute_operator(value: &str) -> Option<(&str, DgAttributeOperator, Option<&str>)> {
    const OPERATORS: [(&str, DgAttributeOperator); 6] = [
        ("~=", DgAttributeOperator::Includes),
        ("^=", DgAttributeOperator::Prefix),
        ("$=", DgAttributeOperator::Suffix),
        ("*=", DgAttributeOperator::Substring),
        ("|=", DgAttributeOperator::DashMatch),
        ("=", DgAttributeOperator::Equals),
    ];

    if let Some((idx, token, operator)) = OPERATORS
        .iter()
        .filter_map(|(token, operator)| value.find(token).map(|idx| (idx, *token, *operator)))
        .min_by_key(|(idx, _, _)| *idx)
    {
        let name = &value[..idx];
        let raw_value = &value[idx + token.len()..];
        return Some((name, operator, Some(raw_value)));
    }

    Some((value, DgAttributeOperator::Exists, None))
}

fn is_attribute_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
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
    if let Some(function) = parse_selector_function(value) {
        compound.functions.push(function);
        return Some(());
    }
    None
}

fn parse_selector_function(value: &str) -> Option<DgSelectorFunction> {
    let (kind, inner) = if let Some(inner) = value.strip_prefix("not(") {
        (DgSelectorFunctionKind::Not, inner.strip_suffix(')')?)
    } else if let Some(inner) = value.strip_prefix("is(") {
        (DgSelectorFunctionKind::Is, inner.strip_suffix(')')?)
    } else if let Some(inner) = value.strip_prefix("where(") {
        (DgSelectorFunctionKind::Where, inner.strip_suffix(')')?)
    } else {
        return None;
    };
    let selectors = split_selector_list(inner)
        .into_iter()
        .map(|selector| parse_compound_selector(&selector))
        .collect::<Option<Vec<_>>>()?;
    if selectors.is_empty() || selectors.iter().any(|selector| selector.part.is_some()) {
        return None;
    }
    Some(DgSelectorFunction { kind, selectors })
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
            parse_nth_child(inner).map(DgStructuralPseudo::NthChild)
        }
    }
}

fn parse_nth_child(value: &str) -> Option<DgNthChild> {
    let (pattern, filter) = split_nth_child_pattern_and_filter(value)?;
    let pattern = parse_nth_child_pattern(pattern)?;
    let Some(filter) = filter else {
        return Some(pattern);
    };

    let selectors = split_selector_list(filter)
        .into_iter()
        .map(|selector| {
            let mut warnings = Vec::new();
            let selector = parse_selector(&selector, &mut warnings)?;
            nth_child_filter_selector_is_supported(&selector).then_some(selector)
        })
        .collect::<Option<Vec<_>>>()?;
    (!selectors.is_empty()).then_some(DgNthChild::Of {
        pattern: Box::new(pattern),
        selectors,
    })
}

fn parse_nth_child_pattern(value: &str) -> Option<DgNthChild> {
    let compact = value
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    match compact.as_str() {
        "odd" => return Some(DgNthChild::Odd),
        "even" | "2n" | "2n+0" => return Some(DgNthChild::Even),
        "2n+1" => return Some(DgNthChild::Odd),
        _ => {}
    }

    let Some(n_idx) = compact.find('n') else {
        let offset = compact.parse::<i64>().ok()?;
        return Some(if offset > 0 {
            DgNthChild::Exact(usize::try_from(offset).ok()?)
        } else {
            DgNthChild::Formula { step: 0, offset }
        });
    };
    if compact[n_idx + 1..].contains('n') {
        return None;
    }

    let step = match &compact[..n_idx] {
        "" | "+" => 1,
        "-" => -1,
        value => value.parse::<i64>().ok()?,
    };
    let offset = match &compact[n_idx + 1..] {
        "" => 0,
        value if value.starts_with('+') || value.starts_with('-') => value.parse::<i64>().ok()?,
        _ => return None,
    };
    Some(DgNthChild::Formula { step, offset })
}

fn split_nth_child_pattern_and_filter(value: &str) -> Option<(&str, Option<&str>)> {
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;
    let mut chars = value.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.checked_sub(1)?,
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.checked_sub(1)?,
            _ if ch.is_ascii_whitespace() && bracket_depth == 0 && paren_depth == 0 => {
                while chars
                    .peek()
                    .is_some_and(|(_, next)| next.is_ascii_whitespace())
                {
                    chars.next();
                }
                let Some((token_idx, _)) = chars.peek().copied() else {
                    break;
                };
                let rest = &value[token_idx..];
                if rest.len() < 2 || !rest[..2].eq_ignore_ascii_case("of") {
                    continue;
                }
                let after_of = &rest[2..];
                if !after_of
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_whitespace())
                {
                    continue;
                }
                let pattern = value[..idx].trim();
                let filter = after_of.trim();
                return (!pattern.is_empty() && !filter.is_empty())
                    .then_some((pattern, Some(filter)));
            }
            _ => {}
        }
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return None;
    }
    let pattern = value.trim();
    (!pattern.is_empty()).then_some((pattern, None))
}

fn nth_child_filter_selector_is_supported(selector: &DgSelector) -> bool {
    !matches!(selector, DgSelector::Root)
        && selector.target_part().is_none()
        && !selector.target_contains_state_pseudo()
        && !selector.target_contains_structural_pseudo()
}

fn split_selector_list(selector: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;

    for ch in selector.chars() {
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
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if bracket_depth == 0 && paren_depth == 0 => {
                let part = current.trim();
                if !part.is_empty() {
                    parts.push(part.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let part = current.trim();
    if !part.is_empty() {
        parts.push(part.to_string());
    }
    parts
}

fn split_css_whitespace_tokens(value: &str) -> Option<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0usize;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;

    for ch in value.chars() {
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
            _ if ch.is_ascii_whitespace() && bracket_depth == 0 && paren_depth == 0 => {
                let token = current.trim();
                if !token.is_empty() {
                    tokens.push(token.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if quote.is_some() || bracket_depth != 0 || paren_depth != 0 {
        return None;
    }
    let token = current.trim();
    if !token.is_empty() {
        tokens.push(token.to_string());
    }
    Some(tokens)
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
    let resolved =
        resolve_vars_in_value(value, variables).unwrap_or_else(|| value.trim().to_string());
    let value = resolved.as_str();
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
        Some(value) => css_value_text(&value),
        None => resolve_vars_in_value(value, variables).unwrap_or_else(|| value.trim().to_string()),
    }
}

fn resolve_vars_in_value(value: &str, variables: &BTreeMap<String, DgCssValue>) -> Option<String> {
    resolve_vars_in_value_inner(value, variables, 0)
}

fn resolve_vars_in_value_inner(
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
    depth: usize,
) -> Option<String> {
    const MAX_VAR_DEPTH: usize = 16;
    if depth > MAX_VAR_DEPTH {
        return None;
    }

    let value = value.trim();
    let mut output = String::new();
    let mut index = 0usize;
    let mut changed = false;
    while index < value.len() {
        let Some(var_start) = find_next_var_function(value, index) else {
            output.push_str(&value[index..]);
            break;
        };
        output.push_str(&value[index..var_start]);
        let (inner, end) = var_function_inner(value, var_start)?;
        let (name, fallback) = split_var_name_and_fallback(inner);
        let replacement = if let Some(value) = variables.get(name.trim()) {
            let text = css_value_text(value);
            resolve_vars_in_value_inner(&text, variables, depth + 1)?
        } else {
            let fallback = fallback?;
            resolve_vars_in_value_inner(fallback, variables, depth + 1)?
        };
        output.push_str(&replacement);
        index = end;
        changed = true;
    }

    Some(if changed { output } else { value.to_string() })
}

fn find_next_var_function(value: &str, start: usize) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut index = start;
    while index < value.len() {
        let ch = value[index..].chars().next()?;
        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            }
            index += ch.len_utf8();
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                index += ch.len_utf8();
            }
            _ if value[index..]
                .get(..4)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("var(")) =>
            {
                return Some(index);
            }
            _ => index += ch.len_utf8(),
        }
    }
    None
}

fn var_function_inner(value: &str, start: usize) -> Option<(&str, usize)> {
    let open = start + 3;
    if value.as_bytes().get(open).copied()? != b'(' {
        return None;
    }
    let inner_start = open + 1;
    let mut depth = 1usize;
    let mut quote: Option<char> = None;
    let mut index = inner_start;
    while index < value.len() {
        let ch = value[index..].chars().next()?;
        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            }
            index += ch.len_utf8();
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some((&value[inner_start..index], index + ch.len_utf8()));
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

fn css_value_text(value: &DgCssValue) -> String {
    match value {
        DgCssValue::Number(number) => format_css_number(number.0),
        DgCssValue::Length(length) => css_length_text(length),
        DgCssValue::Color(color) => css_color_text(color),
        DgCssValue::Keyword(keyword) => keyword.0.clone(),
        DgCssValue::String(value) => value.clone(),
    }
}

fn css_length_text(value: &DgCssLength) -> String {
    match value {
        DgCssLength::LogicalPx(value) => format!("{}px", format_css_number(*value)),
        DgCssLength::Em(value) => format!("{}em", format_css_number(*value)),
        DgCssLength::Percent(value) => format!("{}%", format_css_number(*value)),
        DgCssLength::Calc(value) => {
            let px = value.px;
            let percent = value.percent;
            if px == 0.0 {
                format!("calc({}%)", format_css_number(percent))
            } else if percent == 0.0 {
                format!("calc({}px)", format_css_number(px))
            } else if px < 0.0 {
                format!(
                    "calc({}% - {}px)",
                    format_css_number(percent),
                    format_css_number(px.abs())
                )
            } else {
                format!(
                    "calc({}% + {}px)",
                    format_css_number(percent),
                    format_css_number(px)
                )
            }
        }
        DgCssLength::Auto => "auto".to_string(),
    }
}

fn css_color_text(value: &DgCssColor) -> String {
    match value {
        DgCssColor::Token(token) => token.clone(),
        DgCssColor::Rgba(color) => format!(
            "rgba({}, {}, {}, {})",
            format_css_number((color[0].clamp(0.0, 1.0) * 255.0).round()),
            format_css_number((color[1].clamp(0.0, 1.0) * 255.0).round()),
            format_css_number((color[2].clamp(0.0, 1.0) * 255.0).round()),
            format_css_number(color[3].clamp(0.0, 1.0))
        ),
    }
}

fn format_css_number(value: f32) -> String {
    let mut text = format!("{value:.3}");
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
        DgCssLength::Percent(_) | DgCssLength::Calc(_) | DgCssLength::Auto => {
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
                DgCssLength::Percent(_) | DgCssLength::Calc(_) | DgCssLength::Auto => {
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
        DgCssLength::Percent(_) | DgCssLength::Calc(_) | DgCssLength::Auto => {
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
    parse_length_with_variables(value, Some(variables))
        .ok_or_else(|| parse_warning(name, value, "length"))
}

fn parse_px_length_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssLength, DgStyleWarning> {
    let length = parse_length_value(name, value, variables)?;
    require_logical_px(name, value, length)
}

fn parse_grid_template_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Vec<DgGridTrackSize>, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let mut tracks = Vec::new();
    for token in split_value_tokens(&value) {
        if let Some(repeated) = parse_grid_repeat(name, token) {
            tracks.extend(repeated?);
        } else {
            tracks.push(parse_grid_track_size(name, token)?);
        }
    }
    if tracks.is_empty() {
        return Err(parse_warning(name, &value, "grid track list"));
    }
    Ok(tracks)
}

fn parse_grid_repeat(
    name: &str,
    token: &str,
) -> Option<Result<Vec<DgGridTrackSize>, DgStyleWarning>> {
    let token = token.trim();
    if !token
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("repeat("))
        || !token.ends_with(')')
    {
        return None;
    }
    let inner = &token[7..token.len() - 1];
    let parts = split_top_level_commas(inner);
    if parts.len() != 2 {
        return Some(Err(parse_warning(name, token, "repeat(count, tracks)")));
    }
    let repeat_kind = parts[0].trim();
    let track_tokens = split_value_tokens(parts[1]);
    if track_tokens.is_empty() {
        return Some(Err(parse_warning(name, token, "repeat track list")));
    }
    let mut parsed = Vec::with_capacity(track_tokens.len());
    for track in track_tokens {
        if parse_grid_repeat(name, track).is_some() {
            return Some(Err(parse_warning(
                name,
                token,
                "non-nested repeat track list",
            )));
        }
        match parse_grid_track_size(name, track) {
            Ok(track) => parsed.push(track),
            Err(warning) => return Some(Err(warning)),
        }
    }
    match repeat_kind.parse::<usize>() {
        Ok(count) if count > 0 && count <= 32 => Some(Ok({
            let mut repeated = Vec::with_capacity(parsed.len() * count);
            for _ in 0..count {
                repeated.extend(parsed.iter().cloned());
            }
            repeated
        })),
        Ok(_) => Some(Err(parse_warning(name, token, "positive repeat count"))),
        Err(_) => {
            let kind = match repeat_kind.to_ascii_lowercase().as_str() {
                "auto-fit" => DgGridTrackRepeatKind::AutoFit,
                "auto-fill" => DgGridTrackRepeatKind::AutoFill,
                _ => {
                    return Some(Err(parse_warning(
                        name,
                        token,
                        "repeat count or auto-repeat",
                    )))
                }
            };
            Some(Ok(vec![DgGridTrackSize::Repeat {
                kind,
                tracks: parsed,
            }]))
        }
    }
}

fn parse_grid_track_size(name: &str, value: &str) -> Result<DgGridTrackSize, DgStyleWarning> {
    let value = value.trim();
    if let Some(minmax) = parse_grid_minmax(name, value) {
        return minmax;
    }
    if let Some(fit_content) = parse_grid_fit_content(name, value) {
        return fit_content;
    }
    if value.eq_ignore_ascii_case("auto") {
        return Ok(DgGridTrackSize::Auto);
    }
    if let Some(fr) = value.strip_suffix("fr") {
        return fr
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(DgGridTrackSize::Fraction)
            .ok_or_else(|| parse_warning(name, value, "positive fr track"));
    }
    match parse_length(value) {
        Some(DgCssLength::LogicalPx(value)) => Ok(DgGridTrackSize::LogicalPx(value)),
        Some(DgCssLength::Percent(value)) => Ok(DgGridTrackSize::Percent(value)),
        Some(DgCssLength::Calc(calc)) if calc.percent == 0.0 => {
            Ok(DgGridTrackSize::LogicalPx(calc.px))
        }
        Some(DgCssLength::Calc(calc)) if calc.px == 0.0 => {
            Ok(DgGridTrackSize::Percent(calc.percent))
        }
        _ => Err(parse_warning(
            name,
            value,
            "px, percent, fr, auto grid track",
        )),
    }
}

fn parse_grid_fit_content(
    name: &str,
    value: &str,
) -> Option<Result<DgGridTrackSize, DgStyleWarning>> {
    let value = value.trim();
    if !value
        .get(..12)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("fit-content("))
        || !value.ends_with(')')
    {
        return None;
    }
    let inner = value[12..value.len() - 1].trim();
    Some(parse_grid_fit_content_size(name, inner).map(DgGridTrackSize::FitContent))
}

fn parse_grid_fit_content_size(
    name: &str,
    value: &str,
) -> Result<DgGridTrackFitContentSize, DgStyleWarning> {
    match parse_length(value) {
        Some(DgCssLength::LogicalPx(value)) => Ok(DgGridTrackFitContentSize::LogicalPx(value)),
        Some(DgCssLength::Percent(value)) => Ok(DgGridTrackFitContentSize::Percent(value)),
        Some(DgCssLength::Calc(calc)) if calc.percent == 0.0 => {
            Ok(DgGridTrackFitContentSize::LogicalPx(calc.px))
        }
        Some(DgCssLength::Calc(calc)) if calc.px == 0.0 => {
            Ok(DgGridTrackFitContentSize::Percent(calc.percent))
        }
        _ => Err(parse_warning(
            name,
            value,
            "px or percent fit-content track",
        )),
    }
}

fn parse_grid_minmax(name: &str, value: &str) -> Option<Result<DgGridTrackSize, DgStyleWarning>> {
    let value = value.trim();
    if !value
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("minmax("))
        || !value.ends_with(')')
    {
        return None;
    }
    let inner = &value[7..value.len() - 1];
    let parts = split_top_level_commas(inner);
    if parts.len() != 2 {
        return Some(Err(parse_warning(name, value, "minmax(min, max)")));
    }
    let min = match parse_grid_track_min_size(name, parts[0].trim()) {
        Ok(value) => value,
        Err(warning) => return Some(Err(warning)),
    };
    let max = match parse_grid_track_max_size(name, parts[1].trim()) {
        Ok(value) => value,
        Err(warning) => return Some(Err(warning)),
    };
    Some(Ok(DgGridTrackSize::MinMax { min, max }))
}

fn parse_grid_track_min_size(
    name: &str,
    value: &str,
) -> Result<DgGridTrackMinSize, DgStyleWarning> {
    if value.eq_ignore_ascii_case("auto") {
        return Ok(DgGridTrackMinSize::Auto);
    }
    match parse_length(value) {
        Some(DgCssLength::LogicalPx(value)) => Ok(DgGridTrackMinSize::LogicalPx(value)),
        Some(DgCssLength::Percent(value)) => Ok(DgGridTrackMinSize::Percent(value)),
        Some(DgCssLength::Calc(calc)) if calc.percent == 0.0 => {
            Ok(DgGridTrackMinSize::LogicalPx(calc.px))
        }
        Some(DgCssLength::Calc(calc)) if calc.px == 0.0 => {
            Ok(DgGridTrackMinSize::Percent(calc.percent))
        }
        _ => Err(parse_warning(name, value, "px, percent, or auto min track")),
    }
}

fn parse_grid_track_max_size(
    name: &str,
    value: &str,
) -> Result<DgGridTrackMaxSize, DgStyleWarning> {
    if value.eq_ignore_ascii_case("auto") {
        return Ok(DgGridTrackMaxSize::Auto);
    }
    if let Some(fr) = value.strip_suffix("fr") {
        return fr
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite() && *value > 0.0)
            .map(DgGridTrackMaxSize::Fraction)
            .ok_or_else(|| parse_warning(name, value, "positive fr max track"));
    }
    match parse_length(value) {
        Some(DgCssLength::LogicalPx(value)) => Ok(DgGridTrackMaxSize::LogicalPx(value)),
        Some(DgCssLength::Percent(value)) => Ok(DgGridTrackMaxSize::Percent(value)),
        Some(DgCssLength::Calc(calc)) if calc.percent == 0.0 => {
            Ok(DgGridTrackMaxSize::LogicalPx(calc.px))
        }
        Some(DgCssLength::Calc(calc)) if calc.px == 0.0 => {
            Ok(DgGridTrackMaxSize::Percent(calc.percent))
        }
        _ => Err(parse_warning(
            name,
            value,
            "px, percent, fr, or auto max track",
        )),
    }
}

fn parse_grid_placement_value(name: &str, value: &str) -> Result<DgGridPlacement, DgStyleWarning> {
    let parts: Vec<_> = value.split('/').map(str::trim).collect();
    match parts.as_slice() {
        [single] => {
            let start = parse_grid_line(name, single)?;
            Ok(DgGridPlacement {
                start,
                end: DgGridLine::Auto,
            })
        }
        [start, end] => Ok(DgGridPlacement {
            start: parse_grid_line(name, start)?,
            end: parse_grid_line(name, end)?,
        }),
        _ => Err(parse_warning(name, value, "grid placement")),
    }
}

fn parse_grid_line(name: &str, value: &str) -> Result<DgGridLine, DgStyleWarning> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("auto") {
        return Ok(DgGridLine::Auto);
    }
    let lower = value.to_ascii_lowercase();
    if let Some(span) = lower.strip_prefix("span ") {
        return span
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|value| *value > 0)
            .map(DgGridLine::Span)
            .ok_or_else(|| parse_warning(name, value, "positive grid span"));
    }
    value
        .parse::<i16>()
        .ok()
        .filter(|value| *value != 0)
        .map(DgGridLine::Line)
        .ok_or_else(|| parse_warning(name, value, "grid line number, span, or auto"))
}

fn parse_layout_length_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssLength, DgStyleWarning> {
    let length = parse_length_value(name, value, variables)?;
    match length {
        DgCssLength::LogicalPx(_)
        | DgCssLength::Percent(_)
        | DgCssLength::Calc(_)
        | DgCssLength::Auto => Ok(length),
        DgCssLength::Em(_) => Err(DgStyleWarning {
            property: name.to_string(),
            message: format!(
                "`em` lengths are only supported for text spacing in DragonGUI CSS: {value:?}"
            ),
        }),
    }
}

fn parse_spacing_length_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssLength, DgStyleWarning> {
    let length = parse_layout_length_value(name, value, variables)?;
    match length {
        DgCssLength::LogicalPx(_) | DgCssLength::Percent(_) | DgCssLength::Calc(_) => Ok(length),
        DgCssLength::Auto => Err(DgStyleWarning {
            property: name.to_string(),
            message: format!("`auto` lengths are not supported for {name:?} in DragonGUI CSS V1"),
        }),
        DgCssLength::Em(_) => unreachable!("parse_layout_length_value rejects em lengths"),
    }
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
        DgCssLength::Calc(_) => Err(DgStyleWarning {
            property: name.to_string(),
            message: format!(
                "calc() lengths are only supported for width/height sizing properties in DragonGUI CSS: {source:?}"
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

fn parse_layout_box_edges(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgBoxEdges<DgCssLength>, DgStyleWarning> {
    let edges = parse_box_edges(name, value, variables)?;
    Ok(DgBoxEdges {
        top: parse_layout_box_edge(name, value, edges.top)?,
        right: parse_layout_box_edge(name, value, edges.right)?,
        bottom: parse_layout_box_edge(name, value, edges.bottom)?,
        left: parse_layout_box_edge(name, value, edges.left)?,
    })
}

fn parse_layout_box_edge(
    name: &str,
    source: &str,
    length: DgCssLength,
) -> Result<DgCssLength, DgStyleWarning> {
    match length {
        DgCssLength::LogicalPx(_)
        | DgCssLength::Percent(_)
        | DgCssLength::Calc(_)
        | DgCssLength::Auto => Ok(length),
        DgCssLength::Em(_) => Err(DgStyleWarning {
            property: name.to_string(),
            message: format!(
                "`em` lengths are only supported for text spacing in DragonGUI CSS: {source:?}"
            ),
        }),
    }
}

fn parse_spacing_box_edges(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgBoxEdges<DgCssLength>, DgStyleWarning> {
    let edges = parse_layout_box_edges(name, value, variables)?;
    Ok(DgBoxEdges {
        top: reject_auto_spacing(name, value, edges.top)?,
        right: reject_auto_spacing(name, value, edges.right)?,
        bottom: reject_auto_spacing(name, value, edges.bottom)?,
        left: reject_auto_spacing(name, value, edges.left)?,
    })
}

fn reject_auto_spacing(
    name: &str,
    source: &str,
    length: DgCssLength,
) -> Result<DgCssLength, DgStyleWarning> {
    match length {
        DgCssLength::Auto => Err(DgStyleWarning {
            property: name.to_string(),
            message: format!(
                "`auto` lengths are not supported for {name:?} in DragonGUI CSS V1: {source:?}"
            ),
        }),
        _ => Ok(length),
    }
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
    let layers = split_top_level_commas(&value);
    if layers.len() > 1 {
        let mut paints = Vec::with_capacity(layers.len());
        for layer in layers {
            let paint = match parse_single_background_paint(layer, variables)? {
                Some(paint) => paint,
                None => {
                    DgBackgroundPaint::Color(parse_color_value("background", layer, variables)?)
                }
            };
            paints.push(paint);
        }
        return Ok(Some(DgBackgroundPaint::Layers(paints)));
    }
    parse_single_background_paint(&value, variables)
}

fn parse_single_background_paint(
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgBackgroundPaint>, DgStyleWarning> {
    if let Some(args) = function_args(&value, "linear-gradient") {
        return Ok(Some(DgBackgroundPaint::LinearGradient(
            parse_linear_gradient(args, variables, false)?,
        )));
    }
    if let Some(args) = function_args(&value, "repeating-linear-gradient") {
        return Ok(Some(DgBackgroundPaint::LinearGradient(
            parse_linear_gradient(args, variables, true)?,
        )));
    }
    if let Some(args) = function_args(&value, "radial-gradient") {
        return Ok(Some(DgBackgroundPaint::RadialGradient(
            parse_radial_gradient(args, variables, false)?,
        )));
    }
    if let Some(args) = function_args(&value, "repeating-radial-gradient") {
        return Ok(Some(DgBackgroundPaint::RadialGradient(
            parse_radial_gradient(args, variables, true)?,
        )));
    }
    Ok(None)
}

fn parse_linear_gradient(
    args: &str,
    variables: &BTreeMap<String, DgCssValue>,
    repeating: bool,
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
        stops.extend(parse_gradient_stops(part, variables)?);
    }
    Ok(DgLinearGradient {
        angle_deg,
        stops,
        repeating,
    })
}

fn parse_radial_gradient(
    args: &str,
    variables: &BTreeMap<String, DgCssValue>,
    repeating: bool,
) -> Result<DgRadialGradient, DgStyleWarning> {
    let parts = split_top_level_commas(args);
    if parts.len() < 2 {
        return Err(parse_warning(
            "background",
            args,
            "radial-gradient with at least two color stops",
        ));
    }
    let (center, stop_parts) = if let Some(center) = parse_radial_gradient_shape(parts[0]) {
        (center, &parts[1..])
    } else {
        ([0.5, 0.5], &parts[..])
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
        stops.extend(parse_gradient_stops(part, variables)?);
    }
    Ok(DgRadialGradient {
        stops,
        repeating,
        center,
    })
}

fn parse_radial_gradient_shape(value: &str) -> Option<[f32; 2]> {
    let value = value.trim().to_ascii_lowercase();
    if value == "circle" {
        return Some([0.5, 0.5]);
    }
    if let Some(position) = value.strip_prefix("circle at ") {
        return parse_radial_gradient_center(position);
    }
    if let Some(position) = value.strip_prefix("at ") {
        return parse_radial_gradient_center(position);
    }
    None
}

fn parse_radial_gradient_center(value: &str) -> Option<[f32; 2]> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("center") {
        return Some([0.5, 0.5]);
    }
    let tokens = split_value_tokens(value);
    match tokens.as_slice() {
        [single] => parse_radial_center_axis(single).map(|axis| [axis, 0.5]),
        [x, y] => Some([parse_radial_center_axis(x)?, parse_radial_center_axis(y)?]),
        _ => None,
    }
}

fn parse_radial_center_axis(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "left" | "top" => Some(0.0),
        "center" => Some(0.5),
        "right" | "bottom" => Some(1.0),
        _ => parse_gradient_stop_position(&value).ok(),
    }
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

fn parse_gradient_stops(
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Vec<DgGradientStop>, DgStyleWarning> {
    let tokens = split_value_tokens(value);
    if tokens.is_empty() || tokens.len() > 3 {
        return Err(parse_warning("background", value, "gradient color stop"));
    }
    let color = parse_color_value("background", tokens[0], variables)?;
    let first_position = tokens
        .get(1)
        .map(|value| parse_gradient_stop_position(value))
        .transpose()
        .map_err(|_| parse_warning("background", value, "gradient stop position"))?;
    let mut stops = vec![DgGradientStop {
        color: color.clone(),
        position: first_position,
    }];
    if let Some(second) = tokens.get(2) {
        let position = parse_gradient_stop_position(second)
            .map_err(|_| parse_warning("background", value, "gradient stop position"))?;
        stops.push(DgGradientStop {
            color,
            position: Some(position),
        });
    }
    Ok(stops)
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct ParsedCalcTerm {
    percent: f32,
    px: f32,
}

fn parse_calc_length(
    value: &str,
    variables: Option<&BTreeMap<String, DgCssValue>>,
) -> Option<DgCssLength> {
    let value = value.trim();
    let prefix = value.get(..5)?;
    if !prefix.eq_ignore_ascii_case("calc(") || !value.ends_with(')') {
        return None;
    }
    let inner = value.get(5..value.len() - 1)?.trim();
    if inner.is_empty() {
        return None;
    }

    let mut percent = 0.0;
    let mut px = 0.0;
    let mut index = 0;
    let mut parsed_any = false;

    while index < inner.len() {
        index = skip_ascii_whitespace(inner, index);
        let mut sign = 1.0;
        if let Some((next_index, ch)) = char_at(inner, index) {
            if ch == '+' || ch == '-' {
                sign = if ch == '-' { -1.0 } else { 1.0 };
                index = next_index;
            }
        }

        let start = skip_ascii_whitespace(inner, index);
        index = start;
        index = calc_term_end(inner, index)?;

        let term = inner.get(start..index)?.trim();
        if term.is_empty() {
            return None;
        }
        let parsed = parse_calc_term(term, variables)?;
        percent += sign * parsed.percent;
        px += sign * parsed.px;
        parsed_any = true;
    }

    parsed_any.then_some(DgCssLength::Calc(CalcLength { percent, px }))
}

fn parse_calc_term(
    value: &str,
    variables: Option<&BTreeMap<String, DgCssValue>>,
) -> Option<ParsedCalcTerm> {
    if let Some(index) = find_top_level_operator(value, '*') {
        let left = value.get(..index)?.trim();
        let right = value.get(index + 1..)?.trim();
        let left_length = parse_calc_factor_length(left, variables);
        let right_length = parse_calc_factor_length(right, variables);
        let left_number = parse_calc_factor_number(left, variables);
        let right_number = parse_calc_factor_number(right, variables);
        return match (left_length, right_length, left_number, right_number) {
            (Some(length), _, None, Some(number)) => Some(scale_calc_term(length, number)),
            (_, Some(length), Some(number), None) => Some(scale_calc_term(length, number)),
            _ => None,
        };
    }

    if let Some(index) = find_top_level_operator(value, '/') {
        let left = value.get(..index)?.trim();
        let right = value.get(index + 1..)?.trim();
        let length = parse_calc_factor_length(left, variables)?;
        let number = parse_calc_factor_number(right, variables)?;
        if number == 0.0 {
            return None;
        }
        return Some(scale_calc_term(length, 1.0 / number));
    }

    parse_calc_factor_length(value, variables)
}

fn parse_calc_factor_length(
    value: &str,
    variables: Option<&BTreeMap<String, DgCssValue>>,
) -> Option<ParsedCalcTerm> {
    let length = if let Some(variables) = variables {
        match resolve_variable(value, variables) {
            Some(DgCssValue::Length(length)) => Some(length),
            Some(DgCssValue::Number(number)) => Some(DgCssLength::LogicalPx(number.0)),
            Some(DgCssValue::Keyword(keyword)) => parse_simple_length(&keyword.0),
            Some(DgCssValue::String(value)) => parse_simple_length(&value),
            Some(DgCssValue::Color(_)) => None,
            None => parse_simple_length(value),
        }
    } else {
        parse_simple_length(value)
    }?;

    match length {
        DgCssLength::LogicalPx(value) => Some(ParsedCalcTerm {
            percent: 0.0,
            px: value,
        }),
        DgCssLength::Percent(value) => Some(ParsedCalcTerm {
            percent: value,
            px: 0.0,
        }),
        DgCssLength::Em(_) | DgCssLength::Calc(_) | DgCssLength::Auto => None,
    }
}

fn parse_calc_factor_number(
    value: &str,
    variables: Option<&BTreeMap<String, DgCssValue>>,
) -> Option<f32> {
    if let Some(variables) = variables {
        match resolve_variable(value, variables) {
            Some(DgCssValue::Number(number)) => return Some(number.0),
            Some(DgCssValue::Keyword(keyword)) => return keyword.0.trim().parse().ok(),
            Some(DgCssValue::String(value)) => return value.trim().parse().ok(),
            Some(DgCssValue::Length(_) | DgCssValue::Color(_)) => return None,
            None => {}
        }
    }
    value.trim().parse().ok()
}

fn scale_calc_term(value: ParsedCalcTerm, scale: f32) -> ParsedCalcTerm {
    ParsedCalcTerm {
        percent: value.percent * scale,
        px: value.px * scale,
    }
}

fn find_top_level_operator(value: &str, operator: char) -> Option<usize> {
    let mut previous_was_exponent = false;
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;
    for (index, ch) in value.char_indices() {
        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            }
            previous_was_exponent = false;
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.saturating_sub(1),
            _ => {
                if ch == operator && paren_depth == 0 && !previous_was_exponent {
                    return Some(index);
                }
            }
        }
        previous_was_exponent = ch == 'e' || ch == 'E';
    }
    None
}

fn calc_term_end(value: &str, mut index: usize) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut quote: Option<char> = None;
    while let Some((next_index, ch)) = char_at(value, index) {
        if let Some(quote_ch) = quote {
            if ch == quote_ch {
                quote = None;
            }
            index = next_index;
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => paren_depth = paren_depth.saturating_add(1),
            ')' => paren_depth = paren_depth.checked_sub(1)?,
            '+' | '-' if paren_depth == 0 && !is_exponent_sign(value, index) => break,
            _ => {}
        }
        index = next_index;
    }
    (quote.is_none() && paren_depth == 0).then_some(index)
}

fn skip_ascii_whitespace(value: &str, mut index: usize) -> usize {
    while let Some((next_index, ch)) = char_at(value, index) {
        if !ch.is_ascii_whitespace() {
            break;
        }
        index = next_index;
    }
    index
}

fn char_at(value: &str, index: usize) -> Option<(usize, char)> {
    let ch = value.get(index..)?.chars().next()?;
    Some((index + ch.len_utf8(), ch))
}

fn is_exponent_sign(value: &str, index: usize) -> bool {
    value
        .get(..index)
        .and_then(|prefix| prefix.chars().next_back())
        .is_some_and(|ch| ch == 'e' || ch == 'E')
}

fn parse_length(value: &str) -> Option<DgCssLength> {
    parse_length_with_variables(value, None)
}

fn parse_length_with_variables(
    value: &str,
    variables: Option<&BTreeMap<String, DgCssValue>>,
) -> Option<DgCssLength> {
    let value = value.trim();
    if let Some(calc) = parse_calc_length(value, variables) {
        return Some(calc);
    }
    parse_simple_length(value)
}

fn parse_simple_length(value: &str) -> Option<DgCssLength> {
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
    let value = resolve_keyword(value, variables);
    let parts = split_value_tokens(&value);
    if parts.is_empty() {
        return None;
    }

    let mut width = None;
    let mut style = None;
    let mut color = None;
    for part in parts {
        if part.eq_ignore_ascii_case("none") {
            style = Some(DgBorderStyle::None);
            continue;
        }
        if part.eq_ignore_ascii_case("solid") {
            style = Some(DgBorderStyle::Solid);
            continue;
        }
        if width.is_none() {
            if let Ok(parsed) = parse_px_length_value("border", part, variables) {
                width = Some(parsed);
                continue;
            }
        }
        if color.is_none() {
            if let Ok(parsed) = parse_color_value("border", part, variables) {
                color = Some(parsed);
                continue;
            }
        }
        return None;
    }

    match style {
        Some(DgBorderStyle::Solid) => Some(DgBorder {
            width: width?,
            style: DgBorderStyle::Solid,
            color: color?,
        }),
        Some(DgBorderStyle::None) => Some(DgBorder {
            width: width.unwrap_or(DgCssLength::LogicalPx(0.0)),
            style: DgBorderStyle::None,
            color: color.unwrap_or(DgCssColor::Rgba([0.0, 0.0, 0.0, 0.0])),
        }),
        None if width.as_ref().is_some_and(css_length_is_zero) => Some(DgBorder {
            width: width.unwrap_or(DgCssLength::LogicalPx(0.0)),
            style: DgBorderStyle::None,
            color: color.unwrap_or(DgCssColor::Rgba([0.0, 0.0, 0.0, 0.0])),
        }),
        None => None,
    }
}

fn css_length_is_zero(value: &DgCssLength) -> bool {
    match value {
        DgCssLength::LogicalPx(value) | DgCssLength::Em(value) | DgCssLength::Percent(value) => {
            value.abs() <= f32::EPSILON
        }
        DgCssLength::Calc(value) => {
            value.px.abs() <= f32::EPSILON && value.percent.abs() <= f32::EPSILON
        }
        DgCssLength::Auto => false,
    }
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
    if shadows.is_empty() {
        return Err(parse_warning(name, &value, "box-shadow"));
    }
    shadows
        .iter()
        .map(|shadow| parse_single_box_shadow(name, shadow, variables))
        .collect()
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
    if lengths.len() < 2 || lengths.len() > 4 {
        return Err(parse_warning(
            name,
            value,
            "box-shadow: inset? <offset-x> <offset-y> <blur?> <spread?> <color>",
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
        inset,
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
            (
                "transform",
                DgStylePropertyName::Visual(DgVisualPropertyName::Transform),
            ),
            (
                "translate",
                DgStylePropertyName::Visual(DgVisualPropertyName::Translate),
            ),
            (
                "scale",
                DgStylePropertyName::Visual(DgVisualPropertyName::Scale),
            ),
            (
                "rotate",
                DgStylePropertyName::Visual(DgVisualPropertyName::Rotate),
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
                "transition",
                DgStylePropertyName::Transition(DgTransitionPropertyName::Transition),
            ),
            (
                "transition-property",
                DgStylePropertyName::Transition(DgTransitionPropertyName::Property),
            ),
            (
                "transition-duration",
                DgStylePropertyName::Transition(DgTransitionPropertyName::Duration),
            ),
            (
                "transition-timing-function",
                DgStylePropertyName::Transition(DgTransitionPropertyName::TimingFunction),
            ),
            (
                "transition-delay",
                DgStylePropertyName::Transition(DgTransitionPropertyName::Delay),
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
            attributes: &[],
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &[],
            pseudo: &pseudos,
            sibling_index: Some(0),
            sibling_count: Some(2),
            siblings: None,
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
    fn selector_matching_supports_exact_attribute_selectors() {
        let attributes = [
            StyleAttribute {
                name: "level".to_string(),
                value: "info".to_string(),
            },
            StyleAttribute {
                name: "disabled".to_string(),
                value: "true".to_string(),
            },
        ];
        let element = StyleElement {
            id: "status",
            key: None,
            attributes: &attributes,
            classes: &[],
            kind: WidgetKind::Badge,
            ancestors: &[],
            pseudo: &[],
            sibling_index: None,
            sibling_count: None,
            siblings: None,
        };
        let selector = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Badge)
                .with_attribute("level", "info")
                .with_attribute("disabled", "true"),
        );
        let wrong_level =
            DgSelector::Compound(DgCompoundSelector::new().with_attribute("level", "warning"));

        assert!(selector.matches(&element));
        assert!(!wrong_level.matches(&element));
        assert_eq!(selector.specificity(), Specificity::new(0, 2, 1));
    }

    #[test]
    fn selector_matching_supports_attribute_presence_and_string_operators() {
        let attributes = [
            StyleAttribute {
                name: "class".to_string(),
                value: "callout pill".to_string(),
            },
            StyleAttribute {
                name: "text".to_string(),
                value: "Run report".to_string(),
            },
            StyleAttribute {
                name: "path".to_string(),
                value: "icons/run.png".to_string(),
            },
            StyleAttribute {
                name: "level".to_string(),
                value: "info-primary".to_string(),
            },
            StyleAttribute {
                name: "disabled".to_string(),
                value: "true".to_string(),
            },
        ];
        let element = StyleElement {
            id: "run",
            key: None,
            attributes: &attributes,
            classes: &[],
            kind: WidgetKind::Button,
            ancestors: &[],
            pseudo: &[],
            sibling_index: None,
            sibling_count: None,
            siblings: None,
        };
        let selector = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_attribute_operator("disabled", DgAttributeOperator::Exists, None)
                .with_attribute_operator(
                    "class",
                    DgAttributeOperator::Includes,
                    Some("pill".to_string()),
                )
                .with_attribute_operator(
                    "text",
                    DgAttributeOperator::Prefix,
                    Some("Run".to_string()),
                )
                .with_attribute_operator(
                    "path",
                    DgAttributeOperator::Suffix,
                    Some(".png".to_string()),
                )
                .with_attribute_operator(
                    "text",
                    DgAttributeOperator::Substring,
                    Some("report".to_string()),
                )
                .with_attribute_operator(
                    "level",
                    DgAttributeOperator::DashMatch,
                    Some("info".to_string()),
                )
                .with_attribute_case(
                    "text",
                    DgAttributeOperator::Prefix,
                    Some("run".to_string()),
                    DgAttributeCaseSensitivity::CaseInsensitive,
                ),
        );
        let missing_presence =
            DgSelector::Compound(DgCompoundSelector::new().with_attribute_operator(
                "open",
                DgAttributeOperator::Exists,
                None,
            ));
        let wrong_word = DgSelector::Compound(DgCompoundSelector::new().with_attribute_operator(
            "class",
            DgAttributeOperator::Includes,
            Some("pi".to_string()),
        ));

        assert!(selector.matches(&element));
        assert!(!missing_presence.matches(&element));
        assert!(!wrong_word.matches(&element));
        assert_eq!(selector.specificity(), Specificity::new(0, 7, 1));
    }

    #[test]
    fn selector_matching_supports_direct_child_ancestors() {
        let classes = ["primary"];
        let parent_classes = ["controls"];
        let ancestors = [StyleAncestor {
            id: "controls-panel",
            key: None,
            attributes: &[],
            classes: &parent_classes,
            kind: WidgetKind::Panel,
        }];
        let element = StyleElement {
            id: "run",
            key: None,
            attributes: &[],
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &ancestors,
            pseudo: &[],
            sibling_index: Some(0),
            sibling_count: Some(1),
            siblings: None,
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
                attributes: &[],
                classes: &h_layout_classes,
                kind: WidgetKind::HLayout,
            },
            StyleAncestor {
                id: "controls-panel",
                key: None,
                attributes: &[],
                classes: &panel_classes,
                kind: WidgetKind::Panel,
            },
            StyleAncestor {
                id: "root",
                key: None,
                attributes: &[],
                classes: &window_classes,
                kind: WidgetKind::Window,
            },
        ];
        let element = StyleElement {
            id: "run",
            key: Some("primary-action"),
            attributes: &[],
            classes: &button_classes,
            kind: WidgetKind::Button,
            ancestors: &ancestors,
            pseudo: &[],
            sibling_index: Some(1),
            sibling_count: Some(3),
            siblings: None,
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
            attributes: &[],
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &[],
            pseudo: &[],
            sibling_index: Some(1),
            sibling_count: Some(3),
            siblings: None,
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
        let every_third_offset = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::NthChild(DgNthChild::Formula {
                    step: 3,
                    offset: -1,
                })),
        );
        let first_three = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::NthChild(DgNthChild::Formula {
                    step: -1,
                    offset: 3,
                })),
        );
        let after_third = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::NthChild(DgNthChild::Formula {
                    step: 1,
                    offset: 4,
                })),
        );

        assert!(second_child.matches(&element));
        assert!(even_child.matches(&element));
        assert!(!first_child.matches(&element));
        assert!(!last_child.matches(&element));
        assert!(every_third_offset.matches(&element));
        assert!(first_three.matches(&element));
        assert!(!after_third.matches(&element));
    }

    #[test]
    fn selector_matching_supports_not_is_and_where_functions() {
        let classes = ["primary"];
        let element = StyleElement {
            id: "run",
            key: None,
            attributes: &[],
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &[],
            pseudo: &[],
            sibling_index: Some(1),
            sibling_count: Some(3),
            siblings: None,
        };
        let not_ghost = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Not,
                    selectors: vec![DgCompoundSelector::new().with_class("ghost")],
                }),
        );
        let is_button_or_label =
            DgSelector::Compound(DgCompoundSelector::new().with_function(DgSelectorFunction {
                kind: DgSelectorFunctionKind::Is,
                selectors: vec![
                    DgCompoundSelector::new().with_type(WidgetKind::Button),
                    DgCompoundSelector::new().with_type(WidgetKind::Label),
                ],
            }));
        let where_primary =
            DgSelector::Compound(DgCompoundSelector::new().with_function(DgSelectorFunction {
                kind: DgSelectorFunctionKind::Where,
                selectors: vec![DgCompoundSelector::new().with_class("primary")],
            }));

        assert!(not_ghost.matches(&element));
        assert!(is_button_or_label.matches(&element));
        assert!(where_primary.matches(&element));
        assert_eq!(not_ghost.specificity(), Specificity::new(0, 1, 1));
        assert_eq!(is_button_or_label.specificity(), Specificity::new(0, 0, 1));
        assert_eq!(where_primary.specificity(), Specificity::ZERO);
    }

    #[test]
    fn selector_matching_supports_dynamic_pseudos_in_functions() {
        let classes = ["primary"];
        let base = StyleElement {
            id: "run",
            key: None,
            attributes: &[],
            classes: &classes,
            kind: WidgetKind::Button,
            ancestors: &[],
            pseudo: &[],
            sibling_index: None,
            sibling_count: None,
            siblings: None,
        };
        let hover_pseudos = [DgPseudoClass::Hover];
        let hover = StyleElement {
            pseudo: &hover_pseudos,
            ..base
        };
        let disabled_pseudos = [DgPseudoClass::Disabled];
        let disabled = StyleElement {
            pseudo: &disabled_pseudos,
            ..base
        };
        let not_disabled = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Not,
                    selectors: vec![DgCompoundSelector::new().with_pseudo(DgPseudoClass::Disabled)],
                }),
        );
        let is_hover_or_focus =
            DgSelector::Compound(DgCompoundSelector::new().with_function(DgSelectorFunction {
                kind: DgSelectorFunctionKind::Is,
                selectors: vec![
                    DgCompoundSelector::new().with_pseudo(DgPseudoClass::Hover),
                    DgCompoundSelector::new().with_pseudo(DgPseudoClass::Focus),
                ],
            }));

        assert!(not_disabled.matches(&base));
        assert!(not_disabled.matches(&hover));
        assert!(!not_disabled.matches(&disabled));
        assert!(!is_hover_or_focus.matches(&base));
        assert!(is_hover_or_focus.matches(&hover));
        assert_eq!(
            selector_match_slots(&is_hover_or_focus, &base),
            vec![Some(DgPseudoClass::Hover), Some(DgPseudoClass::Focus)]
        );
    }

    #[test]
    fn universal_selectors_parse_and_match() {
        let mut warnings = Vec::new();
        let universal = parse_selector("*", &mut warnings).expect("universal selector");
        let universal_quiet =
            parse_selector("*.quiet", &mut warnings).expect("universal class selector");
        let chain =
            parse_selector("Panel > *:nth-child(2)", &mut warnings).expect("universal child");

        let classes = ["quiet"];
        let ancestors = [StyleAncestor {
            id: "panel",
            key: None,
            attributes: &[],
            classes: &[],
            kind: WidgetKind::Panel,
        }];
        let element = StyleElement {
            id: "note",
            key: None,
            attributes: &[],
            classes: &classes,
            kind: WidgetKind::Label,
            ancestors: &ancestors,
            pseudo: &[],
            sibling_index: Some(1),
            sibling_count: Some(3),
            siblings: None,
        };

        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(universal.matches(&element));
        assert_eq!(universal.specificity(), Specificity::ZERO);
        assert_eq!(universal.label(), "*");
        assert!(universal_quiet.matches(&element));
        assert_eq!(universal_quiet.specificity(), Specificity::new(0, 1, 0));
        assert!(chain.matches(&element));
        assert_eq!(chain.specificity(), Specificity::new(0, 1, 1));
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
            Panel > :is(Button, Label).callout:not(.muted) { color: accent; }
            :where(.quiet, [key="secondary-action"]) { opacity: 0.8; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert_eq!(parsed.rules.len(), 5);
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
        assert_eq!(
            parsed.rules[3].selector.label(),
            "Panel > .callout:is(Button, Label):not(.muted)"
        );
        assert_eq!(
            parsed.rules[3].selector.specificity(),
            Specificity::new(0, 2, 2)
        );
        assert_eq!(
            parsed.rules[4].selector.label(),
            ":where(.quiet, [key=\"secondary-action\"])"
        );
        assert_eq!(parsed.rules[4].selector.specificity(), Specificity::ZERO);
    }

    #[test]
    fn parses_an_plus_b_nth_child_selectors() {
        let parsed = parse_stylesheet(
            r#"
            Panel > Button:nth-child(3n + 1) { border-width: 2px; }
            Panel > Label:nth-child(-n + 3) { color: accent; }
            Panel > *:nth-child(n + 2) { opacity: 0.8; }
            Panel > *:nth-child(2 of Button.primary, Badge[level="info"]) { border-color: accent; }
            Panel > Button:nth-child(2 of Window > Panel > Button.metric) { opacity: 0.4; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.rules.len(), 5);
        assert_eq!(
            parsed.rules[0].selector.label(),
            "Panel > Button:nth-child(3n+1)"
        );
        assert_eq!(
            parsed.rules[1].selector.label(),
            "Panel > Label:nth-child(-n+3)"
        );
        assert_eq!(parsed.rules[2].selector.label(), "Panel > :nth-child(n+2)");
        assert_eq!(
            parsed.rules[2].selector.specificity(),
            Specificity::new(0, 1, 1)
        );
        assert_eq!(
            parsed.rules[3].selector.label(),
            "Panel > :nth-child(2 of Button.primary, Badge[level=\"info\"])"
        );
        assert_eq!(
            parsed.rules[3].selector.specificity(),
            Specificity::new(0, 2, 2)
        );
        assert_eq!(
            parsed.rules[4].selector.label(),
            "Panel > Button:nth-child(2 of Window > Panel > Button.metric)"
        );
        assert_eq!(
            parsed.rules[4].selector.specificity(),
            Specificity::new(0, 2, 5)
        );
    }

    #[test]
    fn parses_exact_attribute_selectors() {
        let parsed = parse_stylesheet(
            r#"
            Badge[level="info"] { border-width: 2px; }
            Dropdown[value="Finance"] { border-color: accent; }
            Button[disabled="true"] { opacity: 0.5; }
            Button[disabled] { opacity: 0.6; }
            Badge[class~="pill"] { border-radius: 8px; }
            Label[text^="Run"] { color: accent; }
            Image[path$=".png"] { opacity: 0.9; }
            Panel[text*="Status"] { border-color: accent; }
            Badge[level|="info"] { border-width: 1px; }
            Label[text="run" i] { color: white; }
            Button[text=RUN s] { opacity: 0.7; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.rules.len(), 11);
        assert_eq!(parsed.rules[0].selector.label(), "Badge[level=\"info\"]");
        assert_eq!(
            parsed.rules[0].selector.specificity(),
            Specificity::new(0, 1, 1)
        );
        assert_eq!(
            parsed.rules[1].selector.label(),
            "Dropdown[value=\"Finance\"]"
        );
        assert_eq!(
            parsed.rules[2].selector.label(),
            "Button[disabled=\"true\"]"
        );
        assert_eq!(parsed.rules[3].selector.label(), "Button[disabled]");
        assert_eq!(parsed.rules[4].selector.label(), "Badge[class~=\"pill\"]");
        assert_eq!(parsed.rules[5].selector.label(), "Label[text^=\"Run\"]");
        assert_eq!(parsed.rules[6].selector.label(), "Image[path$=\".png\"]");
        assert_eq!(parsed.rules[7].selector.label(), "Panel[text*=\"Status\"]");
        assert_eq!(parsed.rules[8].selector.label(), "Badge[level|=\"info\"]");
        assert_eq!(parsed.rules[9].selector.label(), "Label[text=\"run\" i]");
        assert_eq!(parsed.rules[10].selector.label(), "Button[text=\"RUN\" s]");
    }

    #[test]
    fn invalid_attribute_selector_flags_are_parse_errors() {
        let err = parse_stylesheet(
            r#"
            Label[text="run" q] { color: white; }
            Label[text=run q] { color: white; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap_err();

        assert!(err.message.contains("failed to parse DragonGUI stylesheet"));
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
                    {"id": "first", "type": "button", "class": "metric", "props": {"text": "One"}},
                    {"id": "caption", "type": "label", "class": "metric", "props": {"text": "Middle"}},
                    {"id": "second", "type": "button", "class": "metric", "props": {"text": "Two"}},
                    {"id": "third", "type": "button", "props": {"text": "Three"}}
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
                Panel > *:nth-child(-n + 2) { color: white; }
                Panel > *:nth-child(3n) { opacity: 0.6; }
                Panel > Button:last-child { background: danger; }
                Panel > *:nth-child(2 of Button) { border-color: accent; }
                Panel > Button:nth-child(3 of .metric) { background: success; }
                Panel > *:nth-child(odd of .metric) { border-radius: 10px; }
                Panel > Button:nth-child(2 of Window > Panel > Button.metric) { opacity: 0.4; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let panel = &tree.children[0];
        let first = &panel.children[0];
        let caption = &panel.children[1];
        let second = &panel.children[2];
        let third = &panel.children[3];

        assert_eq!(
            first.style.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            first.style.text.color,
            Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]))
        );
        assert_eq!(first.style.visual.border_radius, Some(10.0));
        assert_ne!(caption.style.visual.border_radius, Some(10.0));
        assert_eq!(second.style.visual.opacity, Some(0.4));
        assert_eq!(second.style.visual.border_radius, Some(10.0));
        assert_eq!(
            second.style.visual.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            second.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_ne!(
            third.style.visual.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            third.style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn stylesheet_cascade_applies_selector_functions() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "panel",
                "type": "panel",
                "children": [
                    {"id": "run", "type": "button", "class": "callout", "props": {"text": "Run"}},
                    {"id": "ghost", "type": "button", "class": "ghost callout", "props": {"text": "Ghost"}},
                    {"id": "note", "type": "label", "class": "callout quiet", "props": {"text": "Note"}}
                ]
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button:not(.ghost) { background: accent; }
                :is(Button, Label).callout { color: white; }
                :where(.quiet) { border-radius: 9px; }
                Panel > *:nth-child(3) { opacity: 0.7; }
                Button:is(:hover, :focus) { border-color: accent; }
                Button:not(:disabled) { border-width: 2px; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let panel = &tree.children[0];
        let run = &panel.children[0];
        let ghost = &panel.children[1];
        let note = &panel.children[2];

        assert_eq!(
            run.style.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_ne!(
            ghost.style.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            run.style.text.color,
            Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]))
        );
        assert_eq!(
            note.style.text.color,
            Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]))
        );
        assert_eq!(note.style.visual.border_radius, Some(9.0));
        assert_eq!(note.style.visual.opacity, Some(0.7));
        assert_eq!(
            run.style.hover.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            run.style.focus.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(run.style.visual.border_width, Some(2.0));
        assert_ne!(run.style.disabled.border_width, Some(2.0));
    }

    #[test]
    fn stylesheet_cascade_applies_exact_attribute_selectors() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [
                {"id": "status", "type": "badge", "class": "callout pill", "props": {"text": "Ready", "level": "info-primary"}},
                {"id": "team", "type": "dropdown", "props": {"items": ["Operations", "Finance"], "value": "Finance"}},
                {"id": "run", "type": "button", "props": {"text": "Run", "disabled": true}}
            ]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Badge[level] { border-width: 3px; }
                Badge[level|="info"] { border-radius: 8px; }
                Badge[class~="pill"] { opacity: 0.8; }
                Badge[text*="AD" i] { color: white; }
                Dropdown[value$="ance"] { border-color: accent; }
                Button[disabled] { opacity: 0.5; }
                Button[text^="Run"] { background: accent; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);

        assert_eq!(tree.children[0].style.visual.border_width, Some(3.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(8.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_eq!(
            tree.children[0].style.text.color,
            Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]))
        );
        assert_eq!(
            tree.children[1].style.visual.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(tree.children[2].style.visual.opacity, Some(0.5));
        assert_eq!(
            tree.children[2].style.visual.background,
            Some(ColorRef::Token("accent".to_string()))
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
    fn border_shorthand_accepts_none_reset() {
        let parsed = parse_stylesheet("Button { border: none; }", StylesheetOrigin::User).unwrap();
        let declaration = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::Border(border)) => Some(border),
                _ => None,
            })
            .expect("border shorthand should lower");

        assert_eq!(declaration.style, DgBorderStyle::None);
        assert_eq!(declaration.width, DgCssLength::LogicalPx(0.0));

        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);
        assert_eq!(style.visual.border_width, Some(0.0));
        assert_eq!(
            style.visual.border_color,
            Some(ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]))
        );
    }

    #[test]
    fn border_shorthand_zero_resets_border() {
        let parsed = parse_stylesheet("Button { border: 0; }", StylesheetOrigin::User).unwrap();

        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);

        assert_eq!(style.visual.border_width, Some(0.0));
        assert_eq!(
            style.visual.border_color,
            Some(ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]))
        );
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
            "Button { letter-spacing: 50%; border-radius: 50%; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "letter-spacing"
                && warning.message.contains("px or em length")));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "border-radius"
                && warning.message.contains("percentage lengths")));
    }

    #[test]
    fn layout_percent_and_auto_lengths_parse_without_warning() {
        let parsed = parse_stylesheet(
            "Panel { width: 50%; height: auto; min-width: 25%; max-height: 100%; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Width(DgCssLength::Percent(50.0)))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Height(DgCssLength::Auto))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::MaxHeight(DgCssLength::Percent(
                    100.0
                )))
            )
        }));
    }

    #[test]
    fn layout_calc_lengths_parse_for_single_unit_expressions() {
        let parsed = parse_stylesheet(
            "Panel { width: calc(20% + 30%); min-width: calc(220px + 40px); height: calc(80px * 2); max-height: calc(100% / 2); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Width(DgCssLength::Calc(
                    CalcLength {
                        percent: 50.0,
                        px: 0.0
                    }
                ))) | DgStyleProperty::Layout(DgLayoutDeclaration::Width(DgCssLength::Percent(
                    50.0
                )))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::MinWidth(DgCssLength::Calc(
                    CalcLength {
                        percent: 0.0,
                        px: 260.0
                    }
                ))) | DgStyleProperty::Layout(DgLayoutDeclaration::MinWidth(
                    DgCssLength::LogicalPx(260.0)
                ))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Height(DgCssLength::Calc(
                    CalcLength {
                        percent: 0.0,
                        px: 160.0
                    }
                ))) | DgStyleProperty::Layout(DgLayoutDeclaration::Height(DgCssLength::LogicalPx(
                    160.0
                )))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::MaxHeight(DgCssLength::Calc(
                    CalcLength {
                        percent: 50.0,
                        px: 0.0
                    }
                ))) | DgStyleProperty::Layout(DgLayoutDeclaration::MaxHeight(
                    DgCssLength::Percent(50.0)
                ))
            )
        }));
    }

    #[test]
    fn layout_calc_lengths_can_use_variables() {
        let parsed = parse_stylesheet(
            ":root { --sidebar: 240px; --scale: 2; --fallback-side: 32px; } Panel { width: calc(100% - var(--sidebar)); min-width: calc(var(--missing, 220px) + 40px); height: calc(var(--fallback-side) * var(--scale)); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Width(DgCssLength::Calc(
                    CalcLength {
                        percent: 100.0,
                        px: -240.0
                    }
                )))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::MinWidth(DgCssLength::Calc(
                    CalcLength {
                        percent: 0.0,
                        px: 260.0
                    }
                ))) | DgStyleProperty::Layout(DgLayoutDeclaration::MinWidth(
                    DgCssLength::LogicalPx(260.0)
                ))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Height(DgCssLength::Calc(
                    CalcLength {
                        percent: 0.0,
                        px: 64.0
                    }
                ))) | DgStyleProperty::Layout(DgLayoutDeclaration::Height(DgCssLength::LogicalPx(
                    64.0
                )))
            )
        }));
    }

    #[test]
    fn mixed_calc_layout_lengths_parse_for_sizing_properties() {
        let parsed = parse_stylesheet(
            "Panel { width: calc(100% - 240px); border-radius: calc(50% + 2px); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(!parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "width"));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Width(DgCssLength::Calc(
                    CalcLength {
                        percent: 100.0,
                        px: -240.0
                    }
                )))
            )
        }));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "border-radius"
                && warning
                    .message
                    .contains("calc() lengths are only supported")));
    }

    #[test]
    fn layout_spacing_percent_calc_and_auto_lengths_parse() {
        let parsed = parse_stylesheet(
            "Panel { padding: 4% calc(10px + 2%); margin: auto; gap: calc(2% + 8px); row-gap: 4%; column-gap: calc(16px + 2%); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Padding(edges))
                    if edges.top == DgCssLength::Percent(4.0)
                        && edges.right == DgCssLength::Calc(CalcLength { percent: 2.0, px: 10.0 })
                        && edges.bottom == DgCssLength::Percent(4.0)
                        && edges.left == DgCssLength::Calc(CalcLength { percent: 2.0, px: 10.0 })
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Margin(edges))
                    if edges.top == DgCssLength::Auto
                        && edges.right == DgCssLength::Auto
                        && edges.bottom == DgCssLength::Auto
                        && edges.left == DgCssLength::Auto
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Gap(DgCssLength::Calc(CalcLength {
                    percent: 2.0,
                    px: 8.0
                })))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::RowGap(DgCssLength::Percent(4.0)))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::ColumnGap(DgCssLength::Calc(
                    CalcLength {
                        percent: 2.0,
                        px: 16.0
                    }
                )))
            )
        }));
    }

    #[test]
    fn padding_and_gap_auto_lengths_warn() {
        let parsed = parse_stylesheet(
            "Panel { padding: auto; gap: auto; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "padding" && warning.message.contains("auto")));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "gap" && warning.message.contains("auto")));
    }

    #[test]
    fn grid_layout_properties_parse() {
        let parsed = parse_stylesheet(
            "Panel.dashboard { display: grid; grid-template-columns: 180px 1fr 2fr; grid-template-rows: auto 48px; column-gap: 10px; row-gap: 12px; } Panel.sidebar { grid-column: 1; grid-row: 1 / span 2; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty());
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Display(DgCssKeyword(ref value)))
                    if value == "grid"
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateColumns(ref tracks))
                    if tracks == &vec![
                        DgGridTrackSize::LogicalPx(180.0),
                        DgGridTrackSize::Fraction(1.0),
                        DgGridTrackSize::Fraction(2.0),
                    ]
            )
        }));
        assert!(parsed.rules[1].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridRow(DgGridPlacement {
                    start: DgGridLine::Line(1),
                    end: DgGridLine::Span(2),
                }))
            )
        }));
    }

    #[test]
    fn grid_repeat_expands_small_count() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: repeat(2, 1fr 80px); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty());
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateColumns(ref tracks))
                    if tracks == &vec![
                        DgGridTrackSize::Fraction(1.0),
                        DgGridTrackSize::LogicalPx(80.0),
                        DgGridTrackSize::Fraction(1.0),
                        DgGridTrackSize::LogicalPx(80.0),
                    ]
            )
        }));
    }

    #[test]
    fn grid_auto_repeat_tracks_parse() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: fit-content(180px) repeat(auto-fit, minmax(120px, 1fr)); grid-template-rows: repeat(auto-fill, 40px); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateColumns(tracks))
                    if tracks == &vec![
                        DgGridTrackSize::FitContent(DgGridTrackFitContentSize::LogicalPx(180.0)),
                        DgGridTrackSize::Repeat {
                            kind: DgGridTrackRepeatKind::AutoFit,
                            tracks: vec![DgGridTrackSize::MinMax {
                                min: DgGridTrackMinSize::LogicalPx(120.0),
                                max: DgGridTrackMaxSize::Fraction(1.0),
                            }],
                        },
                    ]
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateRows(tracks))
                    if tracks == &vec![DgGridTrackSize::Repeat {
                        kind: DgGridTrackRepeatKind::AutoFill,
                        tracks: vec![DgGridTrackSize::LogicalPx(40.0)],
                    }]
            )
        }));
    }

    #[test]
    fn grid_repeat_rejects_nested_repeat() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: repeat(auto-fit, repeat(2, 120px)); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "grid-template-columns"
                && warning.message.contains("non-nested")));
    }

    #[test]
    fn grid_minmax_tracks_parse() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: minmax(160px, 1fr) minmax(25%, auto); grid-template-rows: minmax(calc(40px + 20px), 120px); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateColumns(tracks))
                    if tracks == &vec![
                        DgGridTrackSize::MinMax {
                            min: DgGridTrackMinSize::LogicalPx(160.0),
                            max: DgGridTrackMaxSize::Fraction(1.0),
                        },
                        DgGridTrackSize::MinMax {
                            min: DgGridTrackMinSize::Percent(25.0),
                            max: DgGridTrackMaxSize::Auto,
                        },
                    ]
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateRows(tracks))
                    if tracks == &vec![DgGridTrackSize::MinMax {
                        min: DgGridTrackMinSize::LogicalPx(60.0),
                        max: DgGridTrackMaxSize::LogicalPx(120.0),
                    }]
            )
        }));
    }

    #[test]
    fn grid_minmax_rejects_flexible_min_track() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: minmax(1fr, 2fr); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "grid-template-columns"
                && warning.message.contains("min track")));
    }

    #[test]
    fn grid_fit_content_tracks_parse() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: fit-content(220px) fit-content(40%); grid-template-rows: fit-content(calc(20px + 30px)); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateColumns(tracks))
                    if tracks == &vec![
                        DgGridTrackSize::FitContent(DgGridTrackFitContentSize::LogicalPx(220.0)),
                        DgGridTrackSize::FitContent(DgGridTrackFitContentSize::Percent(40.0)),
                    ]
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateRows(tracks))
                    if tracks == &vec![DgGridTrackSize::FitContent(
                        DgGridTrackFitContentSize::LogicalPx(50.0)
                    )]
            )
        }));
    }

    #[test]
    fn grid_fit_content_rejects_fr_track() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: fit-content(1fr); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "grid-template-columns"
                && warning.message.contains("fit-content")));
    }

    #[test]
    fn overflow_properties_parse_and_validate() {
        let parsed = parse_stylesheet(
            "Panel { overflow: auto; overflow-x: visible; overflow-y: hidden; } Panel.bad { overflow: banana; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Overflow(DgCssKeyword(ref value)))
                    if value == "auto"
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::OverflowX(DgCssKeyword(ref value)))
                    if value == "visible"
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::OverflowY(DgCssKeyword(ref value)))
                    if value == "hidden"
            )
        }));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "overflow"
                && warning.message.contains("overflow value")));
    }

    #[test]
    fn position_properties_parse_and_validate() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [
                {"id": "badge", "type": "badge", "class": "float", "props": {"text": "Offset"}},
                {"id": "pin", "type": "badge", "class": "pin", "props": {"text": "Pinned"}},
                {"id": "dock", "type": "badge", "class": "dock", "props": {"text": "Docked"}}
            ]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
            "Badge.float { position: relative; top: -6px; left: 8px; z-index: 3; } Badge.pin { position: absolute; top: 8px; right: 10px; } Badge.dock { position: fixed; bottom: 12px; left: 16px; } Badge.bad { position: sticky; }",
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let badge = &tree.children[0];
        assert_eq!(badge.style.layout.position, Some(PositionStyle::Relative));
        assert_eq!(badge.style.layout.top, Some(-6.0));
        assert_eq!(badge.style.layout.left, Some(8.0));
        assert_eq!(badge.style.layout.z_index, Some(3));
        let pin = &tree.children[1];
        assert_eq!(pin.style.layout.position, Some(PositionStyle::Absolute));
        assert_eq!(pin.style.layout.top, Some(8.0));
        assert_eq!(pin.style.layout.right, Some(10.0));
        let dock = &tree.children[2];
        assert_eq!(dock.style.layout.position, Some(PositionStyle::Fixed));
        assert_eq!(dock.style.layout.bottom, Some(12.0));
        assert_eq!(dock.style.layout.left, Some(16.0));
        assert!(store
            .warnings()
            .iter()
            .any(|warning| warning.property == "position"
                && warning.message.contains("position value")));
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
    fn box_shadow_parses_multiple_non_inset_layers() {
        let parsed = parse_stylesheet(
            "Panel.card { box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18), 0 16px 40px 4px rgba(0, 0, 0, 0.24); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        let shadows = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BoxShadow(shadows)) => Some(shadows),
                _ => None,
            })
            .expect("box-shadow declaration");
        assert_eq!(shadows.len(), 2);
        assert_eq!(shadows[0].offset_y, DgCssLength::LogicalPx(2.0));
        assert_eq!(shadows[0].blur, DgCssLength::LogicalPx(8.0));
        assert_eq!(shadows[1].offset_y, DgCssLength::LogicalPx(16.0));
        assert_eq!(shadows[1].blur, DgCssLength::LogicalPx(40.0));
        assert_eq!(shadows[1].spread, DgCssLength::LogicalPx(4.0));

        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);
        let shadows = style.visual.box_shadows.as_ref().expect("computed shadows");
        assert_eq!(shadows.len(), 2);
        assert_eq!(shadows[0].offset_y, 2.0);
        assert_eq!(shadows[1].offset_y, 16.0);
    }

    #[test]
    fn box_shadow_parses_inset_layers() {
        let parsed = parse_stylesheet(
            "Panel.card { box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.18), inset 0 -14px 30px rgba(0, 0, 0, 0.24); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        let shadows = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BoxShadow(shadows)) => Some(shadows),
                _ => None,
            })
            .expect("box-shadow declaration");
        assert_eq!(shadows.len(), 2);
        assert!(shadows[0].inset);
        assert_eq!(shadows[0].offset_y, DgCssLength::LogicalPx(1.0));
        assert_eq!(shadows[0].blur, DgCssLength::LogicalPx(0.0));
        assert!(shadows[1].inset);
        assert_eq!(shadows[1].offset_y, DgCssLength::LogicalPx(-14.0));
        assert_eq!(shadows[1].blur, DgCssLength::LogicalPx(30.0));

        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);
        let shadows = style.visual.box_shadows.as_ref().expect("computed shadows");
        assert_eq!(shadows.len(), 2);
        assert!(shadows.iter().all(|shadow| shadow.inset));
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
            "Panel.hero { background: radial-gradient(circle at 20% 35%, rgba(255, 255, 255, 0.25), transparent); }",
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
        assert_eq!(gradient.center, [0.2, 0.35]);
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
    fn repeating_gradients_parse_to_background_paint() {
        let parsed = parse_stylesheet(
            "Panel.a { background: repeating-linear-gradient(90deg, #ff0000 0%, #ff0000 8%, transparent 8%, transparent 16%); } Panel.b { background: repeating-radial-gradient(circle, rgba(255,255,255,0.2) 0%, transparent 20%); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        let linear = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundPaint(
                    DgBackgroundPaint::LinearGradient(gradient),
                )) => Some(gradient),
                _ => None,
            })
            .expect("repeating linear gradient");
        assert!(linear.repeating);
        assert_eq!(linear.stops.len(), 4);

        let radial = parsed.rules[1]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundPaint(
                    DgBackgroundPaint::RadialGradient(gradient),
                )) => Some(gradient),
                _ => None,
            })
            .expect("repeating radial gradient");
        assert!(radial.repeating);
        assert_eq!(radial.stops.len(), 2);
    }

    #[test]
    fn layered_gradient_background_parses_to_background_paint() {
        let parsed = parse_stylesheet(
            "Panel.hero { background: radial-gradient(circle at 22% 18%, rgba(255,255,255,0.18) 0%, transparent 55%), linear-gradient(135deg, #172235 0%, #0f1724 100%); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        let layers = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundPaint(
                    DgBackgroundPaint::Layers(layers),
                )) => Some(layers),
                _ => None,
            })
            .expect("layered gradient background");
        assert_eq!(layers.len(), 2);
        assert!(matches!(
            layers[0],
            DgBackgroundPaint::RadialGradient(DgRadialGradient {
                center: [0.22, 0.18],
                ..
            })
        ));
        assert!(matches!(layers[1], DgBackgroundPaint::LinearGradient(_)));
    }

    #[test]
    fn background_noise_parses_and_clamps_to_visual_style() {
        let parsed = parse_stylesheet(
            "Panel.hero { background-noise: 0.035; } Panel.loud { background-noise: 1; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);

        let mut hero = NodeStyle::default();
        apply_property_to_style(&mut hero, &parsed.rules[0].declarations[0].property);
        assert_eq!(hero.visual.background_noise, Some(0.035));

        let mut loud = NodeStyle::default();
        apply_property_to_style(&mut loud, &parsed.rules[1].declarations[0].property);
        assert_eq!(loud.visual.background_noise, Some(0.25));
    }

    #[test]
    fn transition_declarations_parse_to_transition_style() {
        let parsed = parse_stylesheet(
            r#"
            Button {
                transition-property: background, border-color;
                transition-duration: 180ms;
                transition-timing-function: ease-out;
                transition-delay: 25ms;
            }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();
        let mut style = NodeStyle::default();
        for declaration in &parsed.rules[0].declarations {
            apply_property_to_style(&mut style, &declaration.property);
        }

        assert_eq!(
            style.transition.properties,
            Some(vec![
                TransitionProperty::Background,
                TransitionProperty::BorderColor,
            ])
        );
        assert_eq!(style.transition.duration_ms, Some(180));
        assert_eq!(style.transition.delay_ms, Some(25));
        assert_eq!(
            style.transition.timing_function,
            Some(TransitionTimingFunction::EaseOut)
        );
    }

    #[test]
    fn transition_shorthand_parses_first_transition_item() {
        let parsed = parse_stylesheet(
            "Button { transition: background 0.2s ease-in-out 50ms; }",
            StylesheetOrigin::User,
        )
        .unwrap();
        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);

        assert_eq!(
            style.transition.properties,
            Some(vec![TransitionProperty::Background])
        );
        assert_eq!(style.transition.duration_ms, Some(200));
        assert_eq!(style.transition.delay_ms, Some(50));
        assert_eq!(
            style.transition.timing_function,
            Some(TransitionTimingFunction::EaseInOut)
        );
    }

    #[test]
    fn transition_timing_parses_cubic_bezier() {
        let parsed = parse_stylesheet(
            "Button { transition-timing-function: cubic-bezier(0.16, 1, 0.3, 1); }",
            StylesheetOrigin::User,
        )
        .unwrap();
        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);

        assert_eq!(
            style.transition.timing_function,
            Some(TransitionTimingFunction::CubicBezier {
                x1: 0.16,
                y1: 1.0,
                x2: 0.3,
                y2: 1.0
            })
        );
    }

    #[test]
    fn transition_shorthand_keeps_cubic_bezier_function_token() {
        let parsed = parse_stylesheet(
            "Button { transition: background 220ms cubic-bezier(0.16, 1, 0.3, 1) 40ms; }",
            StylesheetOrigin::User,
        )
        .unwrap();
        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);

        assert_eq!(
            style.transition.properties,
            Some(vec![TransitionProperty::Background])
        );
        assert_eq!(style.transition.duration_ms, Some(220));
        assert_eq!(style.transition.delay_ms, Some(40));
        assert_eq!(
            style.transition.timing_function,
            Some(TransitionTimingFunction::CubicBezier {
                x1: 0.16,
                y1: 1.0,
                x2: 0.3,
                y2: 1.0
            })
        );
    }

    #[test]
    fn transition_timing_rejects_invalid_cubic_bezier_x_control_points() {
        let parsed = parse_stylesheet(
            "Button { transition-timing-function: cubic-bezier(1.2, 0, 0.3, 1); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .rules
            .first()
            .is_none_or(|rule| rule.declarations.is_empty()));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "transition-timing-function"));
    }

    #[test]
    fn transition_property_rejects_unknown_names() {
        let parsed = parse_stylesheet(
            "Button { transition-property: background, grid-template-columns; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed
            .rules
            .first()
            .is_none_or(|rule| rule.declarations.is_empty()));
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "transition-property"));
    }

    #[test]
    fn transform_declaration_parses_to_visual_style() {
        let parsed = parse_stylesheet(
            "Button:hover { transform: translateY(-2px) scale(1.02) rotate(1deg); }",
            StylesheetOrigin::User,
        )
        .unwrap();
        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);

        let transform = style.visual.transform.expect("transform declaration");
        assert_eq!(transform.translate_y, -2.0);
        assert_eq!(transform.scale_x, 1.02);
        assert_eq!(transform.scale_y, 1.02);
        assert_eq!(transform.rotate_deg, 1.0);
    }

    #[test]
    fn transform_longhands_merge_into_visual_transform() {
        let parsed = parse_stylesheet(
            "Button:hover { transform: translateX(2px); translate: 6px -3px; scale: 1.04 0.98; rotate: 0.25turn; transition-property: translate, rotate; }",
            StylesheetOrigin::User,
        )
        .unwrap();
        let mut style = NodeStyle::default();
        for declaration in &parsed.rules[0].declarations {
            apply_property_to_style(&mut style, &declaration.property);
        }

        let transform = style.visual.transform.expect("merged transform");
        assert_eq!(transform.translate_x, 6.0);
        assert_eq!(transform.translate_y, -3.0);
        assert_eq!(transform.scale_x, 1.04);
        assert_eq!(transform.scale_y, 0.98);
        assert_eq!(transform.rotate_deg, 90.0);
        assert_eq!(
            style.transition.properties,
            Some(vec![TransitionProperty::Transform])
        );
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
    fn variables_resolve_inside_larger_property_values() {
        let parsed = parse_stylesheet(
            r#"
            :root {
                --line-color: rgba(255, 255, 255, 0.25);
                --shadow-color: rgba(0, 0, 0, 0.35);
                --brand-stop: #5aa9ff;
                --border-width: 2px;
                --fast: 160ms;
            }

            Button {
                border: var(--border-width) solid var(--line-color);
                box-shadow: 0 2px 8px var(--shadow-color);
                background: linear-gradient(180deg, var(--brand-stop), var(--missing-stop, transparent));
                transition: background var(--fast) ease-out;
            }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);

        let mut style = NodeStyle::default();
        for declaration in &parsed.rules[0].declarations {
            apply_property_to_style(&mut style, &declaration.property);
        }

        assert_eq!(style.visual.border_width, Some(2.0));
        assert!(matches!(
            style.visual.border_color,
            Some(ColorRef::Rgba([r, g, b, a]))
                if (r - 1.0).abs() < 0.001
                    && (g - 1.0).abs() < 0.001
                    && (b - 1.0).abs() < 0.001
                    && (a - 0.25).abs() < 0.003
        ));
        let shadows = style.visual.box_shadows.as_ref().expect("box shadow");
        assert_eq!(shadows.len(), 1);
        assert!(matches!(
            shadows[0].color,
            ColorRef::Rgba([r, g, b, a])
                if r.abs() < 0.001
                    && g.abs() < 0.001
                    && b.abs() < 0.001
                    && (a - 0.35).abs() < 0.003
        ));
        assert!(matches!(
            style.visual.background_paint,
            Some(BackgroundPaint::LinearGradient(_))
        ));
        assert_eq!(style.transition.duration_ms, Some(160));
        assert_eq!(
            style.transition.properties,
            Some(vec![TransitionProperty::Background])
        );
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
    fn media_rules_apply_against_logical_viewport_size() {
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
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { width: 180px; background: neutral; }
                @media (max-width: 500px) {
                    Button { width: 120px; background: danger; }
                }
                @media (min-width: 900px) {
                    Button { width: 240px; background: success; }
                }
                "#,
            )
            .unwrap();

        assert_eq!(store.rules(StylesheetOrigin::User).len(), 3);
        assert!(store.rules(StylesheetOrigin::User)[1].media.is_some());

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(420.0, 700.0),
        );
        let button = &tree.children[0];
        assert_eq!(button.style.layout.width, Some(120.0));
        assert_eq!(
            button.style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(700.0, 700.0),
        );
        let button = &tree.children[0];
        assert_eq!(button.style.layout.width, Some(180.0));
        assert_eq!(
            button.style.visual.background,
            Some(ColorRef::Token("neutral".to_string()))
        );

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(960.0, 700.0),
        );
        let button = &tree.children[0];
        assert_eq!(button.style.layout.width, Some(240.0));
        assert_eq!(
            button.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
    }

    #[test]
    fn media_rules_support_height_ranges_and_or_lists() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "status",
                "type": "label",
                "props": {"text": "Status"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Label { font-size: 12px; }
                @media (height >= 600px) and (height <= 900px), (max-width: 420px) {
                    Label { font-size: 18px; }
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(800.0, 700.0),
        );
        assert_eq!(tree.children[0].style.text.font_size, Some(18.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(400.0, 300.0),
        );
        assert_eq!(tree.children[0].style.text.font_size, Some(18.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(800.0, 300.0),
        );
        assert_eq!(tree.children[0].style.text.font_size, Some(12.0));
    }

    #[test]
    fn media_rules_support_viewport_orientation() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "status",
                "type": "label",
                "props": {"text": "Status"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Label { font-size: 12px; color: white; }
                @media (orientation: landscape) {
                    Label { font-size: 18px; }
                }
                @media screen and (orientation: portrait) {
                    Label { color: accent; }
                }
                @media (orientation: square) {
                    Label { border-width: 4px; }
                }
                "#,
            )
            .unwrap();

        assert!(store
            .warnings()
            .iter()
            .any(|warning| warning.property == "@media"
                && warning.message.contains("portrait and landscape")));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(900.0, 600.0),
        );
        assert_eq!(tree.children[0].style.text.font_size, Some(18.0));
        assert_eq!(
            tree.children[0].style.text.color,
            Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]))
        );
        assert_ne!(tree.children[0].style.visual.border_width, Some(4.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(600.0, 900.0),
        );
        assert_eq!(tree.children[0].style.text.font_size, Some(12.0));
        assert_eq!(
            tree.children[0].style.text.color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_ne!(tree.children[0].style.visual.border_width, Some(4.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(700.0, 700.0),
        );
        assert_eq!(
            tree.children[0].style.text.color,
            Some(ColorRef::Token("accent".to_string()))
        );
    }

    #[test]
    fn supports_rules_gate_declaration_and_selector_queries() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "run",
                "type": "button",
                "class": "primary",
                "props": {"text": "Run"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { background: neutral; border-radius: 4px; }
                @supports (display: grid) and (selector(Button.primary)) {
                    Button.primary { background: success; }
                }
                @supports not (backdrop-filter: blur(8px)) {
                    Button.primary { border-radius: 12px; }
                }
                @supports (display: inline-grid) or (selector(Widget.unknown)) {
                    Button.primary { border-width: 7px; }
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let button = &tree.children[0];
        assert_eq!(
            button.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(button.style.visual.border_radius, Some(12.0));
        assert_ne!(button.style.visual.border_width, Some(7.0));
    }

    #[test]
    fn supports_rules_compose_with_media_rules() {
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
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { width: 160px; }
                @media (min-width: 900px) {
                    @supports (width: calc(100% - 40px)) {
                        Button { width: 240px; }
                    }
                    @supports (display: inline-grid) {
                        Button { width: 80px; }
                    }
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(700.0, 700.0),
        );
        assert_eq!(tree.children[0].style.layout.width, Some(160.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(960.0, 700.0),
        );
        assert_eq!(tree.children[0].style.layout.width, Some(240.0));
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
        assert_eq!(
            button.style.layout.width_value,
            Some(LayoutLength::LogicalPx(120.0))
        );
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
    fn stylesheet_cascade_applies_panel_scrollbar_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "panel",
                "type": "panel"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel::scrollbar-track {
                    width: 6px;
                    padding: 14px;
                    background: rgba(255, 255, 255, 0.12);
                    border-radius: 999px;
                }

                Panel::scrollbar-thumb {
                    width: 8px;
                    background: accent;
                    border-radius: 999px;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let panel = &tree.children[0];
        let track = panel.style.parts.parts.get("scrollbar-track").unwrap();
        let thumb = panel.style.parts.parts.get("scrollbar-thumb").unwrap();

        assert_eq!(track.layout.width, Some(6.0));
        assert_eq!(track.layout.padding, Some(14.0));
        assert_eq!(track.visual.border_radius, Some(999.0));
        assert!(matches!(
            track.visual.background,
            Some(ColorRef::Rgba([_, _, _, alpha])) if (alpha - 0.12).abs() < 0.003
        ));
        assert_eq!(thumb.layout.width, Some(8.0));
        assert_eq!(
            thumb.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(thumb.visual.border_radius, Some(999.0));
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
