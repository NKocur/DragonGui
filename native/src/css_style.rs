//! DragonGUI-owned CSS style IR.
//!
//! Parser dependencies such as `lightningcss` must lower into these types
//! immediately. Selector matching, cascade resolution, computed styles, and
//! renderer integration should not depend on parser-specific AST types.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use lightningcss::media_query::{
    MediaCondition as LightningMediaCondition, MediaFeatureComparison, MediaFeatureId,
    MediaFeatureName, MediaFeatureValue, MediaList, MediaQuery, MediaType, Operator, QueryFeature,
};
use lightningcss::properties::font::FontFamily as CssFontFamily;
use lightningcss::properties::Property;
use lightningcss::rules::container::{
    ContainerCondition, ContainerRule, ContainerSizeFeature, ContainerSizeFeatureId,
};
use lightningcss::rules::font_face::{FontFaceProperty, FontFaceRule, Source as FontFaceSource};
use lightningcss::rules::keyframes::{KeyframeSelector, KeyframesName, KeyframesRule};
use lightningcss::rules::{supports::SupportsCondition, CssRule, CssRuleList};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::ToCss;
use lightningcss::values::resolution::Resolution as CssResolution;

use crate::document::{WidgetKind, WidgetNode};
use crate::style::{
    visual_style_is_empty, AnimationDirection, AnimationFillMode, AnimationIterationCount,
    AnimationPlayState, AnimationStyle, BackdropFilterStyle, BackgroundPaint, BlobGradient,
    BlobGradientStop, BoxShadow, CalcLength, ColorRef, ContainerTypeStyle, DisplayStyle,
    FlexDirectionStyle, FontFamily, FontStyle, FontVariantNumeric, GeneratedContent,
    GradientInterpolation, GradientStop, GridAutoFlowStyle, GridLineStyle, GridPlacementStyle,
    GridTemplateArea, GridTemplateAreas, GridTrackFitContentSize, GridTrackMaxSize,
    GridTrackMinSize, GridTrackRepeatKind, GridTrackSize, LayoutLength, LayoutStyle, LineHeight,
    LinearGradient, MeshGradient, NodePartStyles, NodeStyle, OverflowStyle, PartLayoutStyle,
    PartStyle, PositionStyle, RadialGradient, TextAlign, TextOverflow, TextSpacing, TextStyle,
    TextTransform, TransformStyle, TransitionProperty, TransitionStyle, TransitionTimingFunction,
    VisualStyle,
};
use crate::theme::{parse_web_color, Color, Theme};

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
    pub resolution_dppx: f32,
    pub device_width: f32,
    pub device_height: f32,
    pub color_gamut: DgMediaColorGamut,
    pub video_color_gamut: DgMediaColorGamut,
    pub pointer: DgMediaPointer,
    pub any_pointer: DgMediaPointer,
    pub hover: DgMediaHover,
    pub any_hover: DgMediaHover,
    pub prefers_reduced_motion: bool,
    pub prefers_color_scheme: DgMediaColorScheme,
    pub update: DgMediaUpdate,
    pub scripting: DgMediaScripting,
    pub forced_colors: DgMediaForcedColors,
    pub prefers_contrast: DgMediaContrast,
    pub inverted_colors: DgMediaInvertedColors,
    pub dynamic_range: DgMediaDynamicRange,
    pub video_dynamic_range: DgMediaDynamicRange,
    pub prefers_reduced_transparency: bool,
    pub prefers_reduced_data: bool,
    pub display_mode: DgMediaDisplayMode,
    pub overflow_block: DgMediaOverflow,
    pub overflow_inline: DgMediaOverflow,
    pub color_bits: f32,
    pub color_index: f32,
    pub monochrome_bits: f32,
    pub horizontal_viewport_segments: f32,
    pub vertical_viewport_segments: f32,
    pub scan: DgMediaScan,
    pub grid: bool,
    pub environment_blending: DgMediaEnvironmentBlending,
    pub nav_controls: DgMediaNavControls,
}

impl DgMediaEnvironment {
    pub fn new(width: f32, height: f32) -> Self {
        Self::with_resolution(width, height, 1.0)
    }

    pub fn with_resolution(width: f32, height: f32, resolution_dppx: f32) -> Self {
        Self::with_preferences(width, height, resolution_dppx, false)
    }

    pub fn with_preferences(
        width: f32,
        height: f32,
        resolution_dppx: f32,
        prefers_reduced_motion: bool,
    ) -> Self {
        Self::with_color_scheme(
            width,
            height,
            resolution_dppx,
            DgMediaColorGamut::Srgb,
            DgMediaPointer::Fine,
            DgMediaPointer::Fine,
            DgMediaHover::Hover,
            DgMediaHover::Hover,
            prefers_reduced_motion,
            DgMediaColorScheme::Dark,
        )
    }

    pub fn with_color_scheme(
        width: f32,
        height: f32,
        resolution_dppx: f32,
        color_gamut: DgMediaColorGamut,
        pointer: DgMediaPointer,
        any_pointer: DgMediaPointer,
        hover: DgMediaHover,
        any_hover: DgMediaHover,
        prefers_reduced_motion: bool,
        prefers_color_scheme: DgMediaColorScheme,
    ) -> Self {
        Self {
            width: width.max(0.0),
            height: height.max(0.0),
            resolution_dppx: resolution_dppx.max(0.001),
            device_width: width.max(0.0),
            device_height: height.max(0.0),
            color_gamut,
            video_color_gamut: color_gamut,
            pointer,
            any_pointer,
            hover,
            any_hover,
            prefers_reduced_motion,
            prefers_color_scheme,
            update: DgMediaUpdate::Fast,
            scripting: DgMediaScripting::None,
            forced_colors: DgMediaForcedColors::None,
            prefers_contrast: DgMediaContrast::NoPreference,
            inverted_colors: DgMediaInvertedColors::None,
            dynamic_range: DgMediaDynamicRange::Standard,
            video_dynamic_range: DgMediaDynamicRange::Standard,
            prefers_reduced_transparency: false,
            prefers_reduced_data: false,
            display_mode: DgMediaDisplayMode::Standalone,
            overflow_block: DgMediaOverflow::Scroll,
            overflow_inline: DgMediaOverflow::Scroll,
            color_bits: 8.0,
            color_index: 0.0,
            monochrome_bits: 0.0,
            horizontal_viewport_segments: 1.0,
            vertical_viewport_segments: 1.0,
            scan: DgMediaScan::Progressive,
            grid: false,
            environment_blending: DgMediaEnvironmentBlending::Opaque,
            nav_controls: DgMediaNavControls::None,
        }
    }

    pub fn from_physical_size(width: f32, height: f32, scale_factor: f32) -> Self {
        let scale_factor = scale_factor.max(0.001);
        Self::with_resolution(width / scale_factor, height / scale_factor, scale_factor)
    }

    fn orientation(self) -> DgMediaOrientation {
        if self.width > self.height {
            DgMediaOrientation::Landscape
        } else {
            DgMediaOrientation::Portrait
        }
    }

    fn aspect_ratio(self) -> f32 {
        if self.height <= f32::EPSILON {
            0.0
        } else {
            self.width / self.height
        }
    }

    fn device_aspect_ratio(self) -> f32 {
        if self.device_height <= f32::EPSILON {
            0.0
        } else {
            self.device_width / self.device_height
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
    ColorGamut(DgMediaColorGamut),
    VideoColorGamut(DgMediaColorGamut),
    Pointer(DgMediaPointer),
    AnyPointer(DgMediaPointer),
    Hover(DgMediaHover),
    AnyHover(DgMediaHover),
    PrefersReducedMotion(bool),
    PrefersColorScheme(DgMediaColorScheme),
    Update(DgMediaUpdate),
    Scripting(DgMediaScripting),
    ForcedColors(DgMediaForcedColors),
    PrefersContrast(DgMediaContrast),
    InvertedColors(DgMediaInvertedColors),
    DynamicRange(DgMediaDynamicRange),
    VideoDynamicRange(DgMediaDynamicRange),
    PrefersReducedTransparency(bool),
    PrefersReducedData(bool),
    DisplayMode(DgMediaDisplayMode),
    OverflowBlock(DgMediaOverflow),
    OverflowInline(DgMediaOverflow),
    Scan(DgMediaScan),
    Grid(bool),
    EnvironmentBlending(DgMediaEnvironmentBlending),
    NavControls(DgMediaNavControls),
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
            DgMediaExpression::ColorGamut(expected) => {
                environment.is_some_and(|environment| environment.color_gamut.supports(*expected))
            }
            DgMediaExpression::VideoColorGamut(expected) => environment
                .is_some_and(|environment| environment.video_color_gamut.supports(*expected)),
            DgMediaExpression::Pointer(expected) => {
                environment.is_some_and(|environment| environment.pointer == *expected)
            }
            DgMediaExpression::AnyPointer(expected) => {
                environment.is_some_and(|environment| environment.any_pointer == *expected)
            }
            DgMediaExpression::Hover(expected) => {
                environment.is_some_and(|environment| environment.hover == *expected)
            }
            DgMediaExpression::AnyHover(expected) => {
                environment.is_some_and(|environment| environment.any_hover == *expected)
            }
            DgMediaExpression::PrefersReducedMotion(expected) => environment
                .is_some_and(|environment| environment.prefers_reduced_motion == *expected),
            DgMediaExpression::PrefersColorScheme(expected) => {
                environment.is_some_and(|environment| environment.prefers_color_scheme == *expected)
            }
            DgMediaExpression::Update(expected) => {
                environment.is_some_and(|environment| environment.update == *expected)
            }
            DgMediaExpression::Scripting(expected) => {
                environment.is_some_and(|environment| environment.scripting == *expected)
            }
            DgMediaExpression::ForcedColors(expected) => {
                environment.is_some_and(|environment| environment.forced_colors == *expected)
            }
            DgMediaExpression::PrefersContrast(expected) => {
                environment.is_some_and(|environment| environment.prefers_contrast == *expected)
            }
            DgMediaExpression::InvertedColors(expected) => {
                environment.is_some_and(|environment| environment.inverted_colors == *expected)
            }
            DgMediaExpression::DynamicRange(expected) => {
                environment.is_some_and(|environment| environment.dynamic_range.supports(*expected))
            }
            DgMediaExpression::VideoDynamicRange(expected) => environment
                .is_some_and(|environment| environment.video_dynamic_range.supports(*expected)),
            DgMediaExpression::PrefersReducedTransparency(expected) => environment
                .is_some_and(|environment| environment.prefers_reduced_transparency == *expected),
            DgMediaExpression::PrefersReducedData(expected) => {
                environment.is_some_and(|environment| environment.prefers_reduced_data == *expected)
            }
            DgMediaExpression::DisplayMode(expected) => {
                environment.is_some_and(|environment| environment.display_mode == *expected)
            }
            DgMediaExpression::OverflowBlock(expected) => {
                environment.is_some_and(|environment| environment.overflow_block == *expected)
            }
            DgMediaExpression::OverflowInline(expected) => {
                environment.is_some_and(|environment| environment.overflow_inline == *expected)
            }
            DgMediaExpression::Scan(expected) => {
                environment.is_some_and(|environment| environment.scan == *expected)
            }
            DgMediaExpression::Grid(expected) => {
                environment.is_some_and(|environment| environment.grid == *expected)
            }
            DgMediaExpression::EnvironmentBlending(expected) => {
                environment.is_some_and(|environment| environment.environment_blending == *expected)
            }
            DgMediaExpression::NavControls(expected) => {
                environment.is_some_and(|environment| environment.nav_controls == *expected)
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
            DgMediaFeature::AspectRatio => environment.aspect_ratio(),
            DgMediaFeature::Resolution => environment.resolution_dppx,
            DgMediaFeature::DevicePixelRatio => environment.resolution_dppx,
            DgMediaFeature::DeviceWidth => environment.device_width,
            DgMediaFeature::DeviceHeight => environment.device_height,
            DgMediaFeature::DeviceAspectRatio => environment.device_aspect_ratio(),
            DgMediaFeature::Color => environment.color_bits,
            DgMediaFeature::ColorIndex => environment.color_index,
            DgMediaFeature::Monochrome => environment.monochrome_bits,
            DgMediaFeature::HorizontalViewportSegments => environment.horizontal_viewport_segments,
            DgMediaFeature::VerticalViewportSegments => environment.vertical_viewport_segments,
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
    AspectRatio,
    Resolution,
    DevicePixelRatio,
    DeviceWidth,
    DeviceHeight,
    DeviceAspectRatio,
    Color,
    ColorIndex,
    Monochrome,
    HorizontalViewportSegments,
    VerticalViewportSegments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DgMediaOrientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DgMediaColorGamut {
    Srgb,
    P3,
    Rec2020,
}

impl DgMediaColorGamut {
    fn supports(self, expected: Self) -> bool {
        self >= expected
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaPointer {
    None,
    Coarse,
    Fine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaHover {
    None,
    Hover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaUpdate {
    None,
    Slow,
    Fast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaScripting {
    None,
    InitialOnly,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaForcedColors {
    None,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaContrast {
    NoPreference,
    More,
    Less,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaInvertedColors {
    None,
    Inverted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DgMediaDynamicRange {
    Standard,
    High,
}

impl DgMediaDynamicRange {
    fn supports(self, expected: Self) -> bool {
        self >= expected
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaDisplayMode {
    Browser,
    MinimalUi,
    Standalone,
    Fullscreen,
    WindowControlsOverlay,
    PictureInPicture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaOverflow {
    None,
    Scroll,
    OptionalPaged,
    Paged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaScan {
    Interlace,
    Progressive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaEnvironmentBlending {
    Opaque,
    Additive,
    Subtractive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaNavControls {
    None,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgMediaComparison {
    Equal,
    GreaterThan,
    GreaterThanEqual,
    LessThan,
    LessThanEqual,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgContainerRuleCondition {
    pub name: Option<String>,
    pub expression: DgContainerExpression,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgContainerExpression {
    Width(DgMediaComparison, f32),
    And(Vec<DgContainerExpression>),
    Or(Vec<DgContainerExpression>),
    Not(Box<DgContainerExpression>),
}

#[derive(Debug, Clone, Default)]
pub struct DgContainerQueryContext {
    widths: BTreeMap<String, f32>,
}

impl DgContainerQueryContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_width(&mut self, id: impl Into<String>, width: f32) {
        if width.is_finite() {
            self.widths.insert(id.into(), width.max(0.0));
        }
    }

    fn width(&self, id: &str) -> Option<f32> {
        self.widths.get(id).copied()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgStyleRule {
    pub selector: DgSelector,
    pub declarations: Vec<DgStyleDeclaration>,
    pub specificity: Specificity,
    pub origin: StylesheetOrigin,
    pub source_order: u32,
    pub media: Option<DgMediaCondition>,
    pub container: Option<DgContainerRuleCondition>,
    target_filter: DgSelectorTargetFilter,
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
        Self::with_conditions(selector, declarations, origin, source_order, media, None)
    }

    pub fn with_conditions(
        selector: DgSelector,
        declarations: Vec<DgStyleDeclaration>,
        origin: StylesheetOrigin,
        source_order: u32,
        media: Option<DgMediaCondition>,
        container: Option<DgContainerRuleCondition>,
    ) -> Self {
        let specificity = selector.specificity();
        let target_filter = DgSelectorTargetFilter::from_selector(&selector);
        Self {
            selector,
            declarations,
            specificity,
            origin,
            source_order,
            media,
            container,
            target_filter,
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

    fn target_may_match(&self, element: &StyleElement<'_>) -> bool {
        self.target_filter.matches(element)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DgSelectorTargetFilter {
    kind: Option<WidgetKind>,
    id: Option<String>,
    key: Option<String>,
    classes: Vec<String>,
}

impl DgSelectorTargetFilter {
    fn from_selector(selector: &DgSelector) -> Self {
        let Some(target) = selector.target_compound() else {
            return Self::default();
        };
        Self {
            kind: target.type_selector,
            id: target.id.clone(),
            key: target.key.clone(),
            classes: target.classes.clone(),
        }
    }

    fn matches(&self, element: &StyleElement<'_>) -> bool {
        if self.kind.is_some_and(|expected| expected != element.kind) {
            return false;
        }
        if self
            .id
            .as_deref()
            .is_some_and(|expected| expected != element.id)
        {
            return false;
        }
        if self
            .key
            .as_deref()
            .is_some_and(|expected| element.key != Some(expected))
        {
            return false;
        }
        self.classes
            .iter()
            .all(|expected| element.classes.iter().any(|class| class == expected))
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
    Animation(DgAnimationDeclaration),
    Generated(DgGeneratedDeclaration),
    CustomProperty { name: String, value: DgCssValue },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgLayoutDeclaration {
    Display(DgCssKeyword),
    FlexDirection(DgCssKeyword),
    Flex(DgCssNumber),
    FlexGrow(DgCssNumber),
    FlexShrink(DgCssNumber),
    FlexBasis(DgCssLength),
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
    MarginLeft(DgCssLength),
    MarginRight(DgCssLength),
    MarginTop(DgCssLength),
    MarginBottom(DgCssLength),
    Gap(DgCssLength),
    RowGap(DgCssLength),
    ColumnGap(DgCssLength),
    GridTemplateColumns(Vec<DgGridTrackSize>),
    GridTemplateRows(Vec<DgGridTrackSize>),
    GridTemplateAreas(DgGridTemplateAreas),
    GridAutoFlow(DgGridAutoFlow),
    GridArea(String),
    GridColumn(DgGridPlacement),
    GridRow(DgGridPlacement),
    ContainerName(Vec<String>),
    ContainerType(ContainerTypeStyle),
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
    BackgroundImage(Option<DgBackgroundPaint>),
    BackdropFilter(Option<BackdropFilterStyle>),
    Foreground(DgCssColor),
    BorderColor(DgCssColor),
    BorderWidth(DgCssLength),
    BorderStyle(DgBorderStyle),
    OutlineColor(DgCssColor),
    OutlineWidth(DgCssLength),
    OutlineStyle(DgBorderStyle),
    OutlineOffset(DgCssLength),
    BorderRadius(DgCssLength),
    BorderTopLeftRadius(DgCssLength),
    BorderTopRightRadius(DgCssLength),
    BorderBottomRightRadius(DgCssLength),
    BorderBottomLeftRadius(DgCssLength),
    Border(DgBorder),
    Outline(DgBorder),
    Opacity(DgCssNumber),
    Accent(DgCssColor),
    TrackColor(DgCssColor),
    ThumbColor(DgCssColor),
    BackgroundNoise(DgCssNumber),
    GradientInterpolation(GradientInterpolation),
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
    TextAreaRows(DgCssNumber),
    ScatterPointSize(DgCssLength),
    ScatterPointStyle(DgCssKeyword),
    ScatterGridVisible(bool),
    ScatterGridPlanes(bool, bool),
    ScatterLegendPosition(DgCssKeyword),
    ScatterOrientationAxes(bool),
    TableRowHeight(DgCssLength),
    TableHeaderHeight(DgCssLength),
    TableColumnWidth(DgCssLength),
    TableIndexWidth(DgCssLength),
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
pub enum DgAnimationDeclaration {
    Shorthand(AnimationStyle),
    Name(Option<String>),
    Duration(u64),
    Delay(i64),
    TimingFunction(TransitionTimingFunction),
    IterationCount(AnimationIterationCount),
    Direction(AnimationDirection),
    FillMode(AnimationFillMode),
    PlayState(AnimationPlayState),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DgGeneratedDeclaration {
    Content(Option<GeneratedContent>),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgGridAutoFlow {
    Row,
    Column,
    RowDense,
    ColumnDense,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DgGridTemplateAreas {
    pub columns: u16,
    pub rows: u16,
    pub areas: Vec<DgGridTemplateArea>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DgGridTemplateArea {
    pub name: String,
    pub row_start: u16,
    pub row_end: u16,
    pub column_start: u16,
    pub column_end: u16,
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
    BlobGradient(DgBlobGradient),
    MeshGradient(DgMeshGradient),
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

#[derive(Debug, Clone, PartialEq)]
pub struct DgBlobGradient {
    pub blobs: Vec<DgBlobGradientStop>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgBlobGradientStop {
    pub center: [f32; 2],
    pub radius: f32,
    pub color: DgCssColor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DgMeshGradient {
    pub top_left: DgCssColor,
    pub top_right: DgCssColor,
    pub bottom_left: DgCssColor,
    pub bottom_right: DgCssColor,
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

    fn target_contains_style_slot_pseudo(&self) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.contains_style_slot_pseudo(),
            DgSelector::Child { child, .. } => child.contains_style_slot_pseudo(),
            DgSelector::Chain(chain) => chain.target.contains_style_slot_pseudo(),
        }
    }

    fn target_state_pseudos_are_snapshot_matchable(&self) -> bool {
        match self {
            DgSelector::Root => true,
            DgSelector::Compound(selector) => selector.state_pseudos_are_snapshot_matchable(),
            DgSelector::Child { child, .. } => child.state_pseudos_are_snapshot_matchable(),
            DgSelector::Chain(chain) => chain.target.state_pseudos_are_snapshot_matchable(),
        }
    }

    fn target_contains_has_function(&self) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.contains_has_function(),
            DgSelector::Child { child, .. } => child.contains_has_function(),
            DgSelector::Chain(chain) => chain.target.contains_has_function(),
        }
    }

    fn contains_has_sibling_relation(&self) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.contains_has_sibling_relation(),
            DgSelector::Child { parent, child } => {
                parent.contains_has_sibling_relation() || child.contains_has_sibling_relation()
            }
            DgSelector::Chain(chain) => {
                chain
                    .ancestors
                    .iter()
                    .any(|(_, selector)| selector.contains_has_sibling_relation())
                    || chain.target.contains_has_sibling_relation()
            }
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

    fn target_compound(&self) -> Option<&DgCompoundSelector> {
        match self {
            DgSelector::Root => None,
            DgSelector::Compound(selector) => Some(selector),
            DgSelector::Child { child, .. } => Some(child),
            DgSelector::Chain(chain) => Some(&chain.target),
        }
    }

    fn contains_attribute_selector(&self) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.contains_attribute_selector(),
            DgSelector::Child { parent, child } => {
                parent.contains_attribute_selector() || child.contains_attribute_selector()
            }
            DgSelector::Chain(chain) => {
                chain
                    .ancestors
                    .iter()
                    .any(|(_, selector)| selector.contains_attribute_selector())
                    || chain.target.contains_attribute_selector()
            }
        }
    }

    fn requires_sibling_snapshots(&self) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.requires_sibling_snapshots(),
            DgSelector::Child { parent, child } => {
                parent.requires_sibling_snapshots() || child.requires_sibling_snapshots()
            }
            DgSelector::Chain(chain) => {
                chain
                    .ancestors
                    .iter()
                    .any(|(_, selector)| selector.requires_sibling_snapshots())
                    || chain.target.requires_sibling_snapshots()
            }
        }
    }

    fn requires_ancestor_matching(&self) -> bool {
        match self {
            DgSelector::Root => false,
            DgSelector::Compound(selector) => selector.requires_ancestor_matching(),
            DgSelector::Child { .. } | DgSelector::Chain(_) => true,
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
            && !self.contains_has_function()
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

    fn contains_style_slot_pseudo(&self) -> bool {
        !self.pseudo.is_empty()
            || self
                .functions
                .iter()
                .any(DgSelectorFunction::contains_style_slot_pseudo)
    }

    fn state_pseudos_are_snapshot_matchable(&self) -> bool {
        self.pseudo
            .iter()
            .all(|pseudo| pseudo.is_snapshot_matchable())
            && self
                .functions
                .iter()
                .all(DgSelectorFunction::state_pseudos_are_snapshot_matchable)
    }

    fn contains_has_function(&self) -> bool {
        self.functions
            .iter()
            .any(DgSelectorFunction::contains_has_function)
    }

    fn contains_has_sibling_relation(&self) -> bool {
        self.functions
            .iter()
            .any(DgSelectorFunction::contains_has_sibling_relation)
    }

    fn contains_attribute_selector(&self) -> bool {
        !self.attributes.is_empty()
            || self
                .functions
                .iter()
                .any(DgSelectorFunction::contains_attribute_selector)
    }

    fn requires_sibling_snapshots(&self) -> bool {
        !self.structural.is_empty()
            || self.functions.iter().any(|function| {
                function.kind == DgSelectorFunctionKind::Has
                    || function.requires_sibling_snapshots()
            })
    }

    fn requires_ancestor_matching(&self) -> bool {
        self.functions
            .iter()
            .any(DgSelectorFunction::requires_ancestor_matching)
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
    pub selectors: Vec<DgSelectorFunctionArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DgSelectorFunctionArgument {
    pub selector: DgSelector,
    pub relation: DgSelectorFunctionRelation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DgSelectorFunctionRelation {
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
}

impl DgSelectorFunctionArgument {
    fn new(selector: DgSelector) -> Self {
        Self {
            selector,
            relation: DgSelectorFunctionRelation::Descendant,
        }
    }

    fn direct_child(selector: DgSelector) -> Self {
        Self {
            selector,
            relation: DgSelectorFunctionRelation::Child,
        }
    }

    fn next_sibling(selector: DgSelector) -> Self {
        Self {
            selector,
            relation: DgSelectorFunctionRelation::NextSibling,
        }
    }

    fn subsequent_sibling(selector: DgSelector) -> Self {
        Self {
            selector,
            relation: DgSelectorFunctionRelation::SubsequentSibling,
        }
    }

    fn compound(selector: DgCompoundSelector) -> Self {
        Self::new(DgSelector::Compound(selector))
    }
}

impl DgSelectorFunction {
    fn specificity(&self) -> Specificity {
        match self.kind {
            DgSelectorFunctionKind::Where => Specificity::ZERO,
            DgSelectorFunctionKind::Not
            | DgSelectorFunctionKind::Is
            | DgSelectorFunctionKind::Has => self
                .selectors
                .iter()
                .map(|selector| selector.selector.specificity())
                .max()
                .unwrap_or(Specificity::ZERO),
        }
    }

    fn matches_element(&self, element: &StyleElement<'_>) -> bool {
        match self.kind {
            DgSelectorFunctionKind::Not => !self
                .selectors
                .iter()
                .any(|selector| selector.selector.matches(element)),
            DgSelectorFunctionKind::Is | DgSelectorFunctionKind::Where => self
                .selectors
                .iter()
                .any(|selector| selector.selector.matches(element)),
            DgSelectorFunctionKind::Has => self
                .selectors
                .iter()
                .any(|selector| has_selector_matches_element(element, selector)),
        }
    }

    fn matches_ancestor(&self, ancestor: &StyleAncestor<'_>) -> bool {
        match self.kind {
            DgSelectorFunctionKind::Not => !self
                .selectors
                .iter()
                .any(|selector| selector.selector.matches_ancestor(ancestor)),
            DgSelectorFunctionKind::Is | DgSelectorFunctionKind::Where => self
                .selectors
                .iter()
                .any(|selector| selector.selector.matches_ancestor(ancestor)),
            DgSelectorFunctionKind::Has => false,
        }
    }

    fn contains_state_pseudo(&self) -> bool {
        self.selectors
            .iter()
            .any(|selector| selector.selector.target_contains_state_pseudo())
    }

    fn contains_style_slot_pseudo(&self) -> bool {
        if self.kind == DgSelectorFunctionKind::Has {
            return false;
        }
        self.selectors
            .iter()
            .any(|selector| selector.selector.target_contains_style_slot_pseudo())
    }

    fn state_pseudos_are_snapshot_matchable(&self) -> bool {
        self.selectors.iter().all(|selector| {
            selector
                .selector
                .target_state_pseudos_are_snapshot_matchable()
        })
    }

    fn contains_has_function(&self) -> bool {
        self.kind == DgSelectorFunctionKind::Has
            || self
                .selectors
                .iter()
                .any(|selector| selector.selector.target_contains_has_function())
    }

    fn contains_has_sibling_relation(&self) -> bool {
        (self.kind == DgSelectorFunctionKind::Has
            && self.selectors.iter().any(|selector| {
                matches!(
                    selector.relation,
                    DgSelectorFunctionRelation::NextSibling
                        | DgSelectorFunctionRelation::SubsequentSibling
                )
            }))
            || self
                .selectors
                .iter()
                .any(|selector| selector.selector.contains_has_sibling_relation())
    }

    fn contains_attribute_selector(&self) -> bool {
        self.selectors
            .iter()
            .any(|selector| selector.selector.contains_attribute_selector())
    }

    fn requires_sibling_snapshots(&self) -> bool {
        self.selectors
            .iter()
            .any(|selector| selector.selector.requires_sibling_snapshots())
    }

    fn requires_ancestor_matching(&self) -> bool {
        self.kind != DgSelectorFunctionKind::Has
            && self
                .selectors
                .iter()
                .any(|selector| selector.selector.requires_ancestor_matching())
    }

    fn label(&self) -> String {
        let name = match self.kind {
            DgSelectorFunctionKind::Not => "not",
            DgSelectorFunctionKind::Is => "is",
            DgSelectorFunctionKind::Where => "where",
            DgSelectorFunctionKind::Has => "has",
        };
        let selectors = self
            .selectors
            .iter()
            .map(|selector| match (self.kind, selector.relation) {
                (DgSelectorFunctionKind::Has, DgSelectorFunctionRelation::Child) => {
                    format!("> {}", selector.selector.label())
                }
                (DgSelectorFunctionKind::Has, DgSelectorFunctionRelation::NextSibling) => {
                    format!("+ {}", selector.selector.label())
                }
                (DgSelectorFunctionKind::Has, DgSelectorFunctionRelation::SubsequentSibling) => {
                    format!("~ {}", selector.selector.label())
                }
                _ => selector.selector.label(),
            })
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
    Has,
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

    fn is_snapshot_matchable(self) -> bool {
        matches!(
            self,
            DgPseudoClass::Disabled
                | DgPseudoClass::Checked
                | DgPseudoClass::Open
                | DgPseudoClass::Expanded
                | DgPseudoClass::Collapsed
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DgStructuralPseudo {
    FirstChild,
    LastChild,
    OnlyChild,
    Empty,
    NthChild(DgNthChild),
    NthLastChild(DgNthChild),
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
            DgStructuralPseudo::NthChild(child) | DgStructuralPseudo::NthLastChild(child) => {
                child.specificity_extra()
            }
            _ => Specificity::ZERO,
        }
    }

    fn matches_element(&self, element: &StyleElement<'_>) -> bool {
        if matches!(self, DgStructuralPseudo::Empty) {
            return element.children.is_some_and(|children| children.is_empty());
        }

        let (Some(index), Some(count)) = (element.sibling_index, element.sibling_count) else {
            return false;
        };
        if count == 0 || index >= count {
            return false;
        }
        let one_based = index + 1;
        let reverse_one_based = count - index;
        match self {
            DgStructuralPseudo::FirstChild => index == 0,
            DgStructuralPseudo::LastChild => one_based == count,
            DgStructuralPseudo::OnlyChild => index == 0 && count == 1,
            DgStructuralPseudo::Empty => false,
            DgStructuralPseudo::NthChild(child) => child.matches_element(element, one_based),
            DgStructuralPseudo::NthLastChild(child) => {
                child.matches_element_from_end(element, reverse_one_based)
            }
        }
    }

    fn label(&self) -> String {
        match self {
            DgStructuralPseudo::FirstChild => "first-child".to_string(),
            DgStructuralPseudo::LastChild => "last-child".to_string(),
            DgStructuralPseudo::OnlyChild => "only-child".to_string(),
            DgStructuralPseudo::Empty => "empty".to_string(),
            DgStructuralPseudo::NthChild(child) => format!("nth-child({})", child.label()),
            DgStructuralPseudo::NthLastChild(child) => {
                format!("nth-last-child({})", child.label())
            }
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

    fn matches_element_from_end(
        &self,
        element: &StyleElement<'_>,
        reverse_one_based: usize,
    ) -> bool {
        match self {
            DgNthChild::Odd => reverse_one_based % 2 == 1,
            DgNthChild::Even => reverse_one_based % 2 == 0,
            DgNthChild::Exact(expected) => *expected == reverse_one_based,
            DgNthChild::Formula { step, offset } => {
                nth_child_formula_matches(reverse_one_based as i64, *step, *offset)
            }
            DgNthChild::Of { pattern, selectors } => {
                nth_last_child_of_matches(pattern, selectors, element)
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

fn nth_last_child_of_matches(
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
    for sibling_index in (0..siblings.len()).rev() {
        let sibling = &siblings[sibling_index];
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

    current_matches && pattern.matches_element_from_end(element, filtered_index)
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
        pseudo: &sibling.pseudo,
        sibling_index: Some(sibling_index),
        sibling_count: Some(sibling_count),
        siblings: base.siblings,
        children: Some(&sibling.children),
    };
    selectors
        .iter()
        .any(|selector| selector.matches(&sibling_element))
}

fn has_selector_matches_element(
    element: &StyleElement<'_>,
    selector: &DgSelectorFunctionArgument,
) -> bool {
    match selector.relation {
        DgSelectorFunctionRelation::Descendant | DgSelectorFunctionRelation::Child => element
            .children
            .is_some_and(|children| has_selector_matches_children(children, selector)),
        DgSelectorFunctionRelation::NextSibling => {
            let (Some(siblings), Some(index)) = (element.siblings, element.sibling_index) else {
                return false;
            };
            siblings.get(index + 1).is_some_and(|sibling| {
                has_selector_matches_relative_sibling(
                    sibling,
                    index + 1,
                    siblings.len(),
                    siblings,
                    selector,
                )
            })
        }
        DgSelectorFunctionRelation::SubsequentSibling => {
            let (Some(siblings), Some(index)) = (element.siblings, element.sibling_index) else {
                return false;
            };
            siblings
                .iter()
                .enumerate()
                .skip(index + 1)
                .any(|(sibling_index, sibling)| {
                    has_selector_matches_relative_sibling(
                        sibling,
                        sibling_index,
                        siblings.len(),
                        siblings,
                        selector,
                    )
                })
        }
    }
}

fn has_selector_matches_children(
    children: &[StyleSibling],
    selector: &DgSelectorFunctionArgument,
) -> bool {
    children.iter().enumerate().any(|(index, child)| {
        style_sibling_matches_has_selector(child, index, children.len(), children, selector, &[])
            || (selector.relation == DgSelectorFunctionRelation::Descendant
                && has_selector_matches_descendants(child, selector, &[child]))
    })
}

fn has_selector_matches_relative_sibling(
    sibling: &StyleSibling,
    sibling_index: usize,
    sibling_count: usize,
    siblings: &[StyleSibling],
    selector: &DgSelectorFunctionArgument,
) -> bool {
    style_sibling_matches_has_selector(
        sibling,
        sibling_index,
        sibling_count,
        siblings,
        selector,
        &[],
    ) || (matches!(
        selector.selector,
        DgSelector::Child { .. } | DgSelector::Chain(_)
    ) && has_selector_matches_descendants(sibling, selector, &[sibling]))
}

fn has_selector_matches_descendants(
    parent: &StyleSibling,
    selector: &DgSelectorFunctionArgument,
    ancestor_path: &[&StyleSibling],
) -> bool {
    parent.children.iter().enumerate().any(|(index, child)| {
        style_sibling_matches_has_selector(
            child,
            index,
            parent.children.len(),
            &parent.children,
            selector,
            ancestor_path,
        ) || {
            let mut child_path = Vec::with_capacity(ancestor_path.len() + 1);
            child_path.push(child);
            child_path.extend_from_slice(ancestor_path);
            has_selector_matches_descendants(child, selector, &child_path)
        }
    })
}

fn style_sibling_matches_has_selector(
    child: &StyleSibling,
    child_index: usize,
    child_count: usize,
    siblings: &[StyleSibling],
    selector: &DgSelectorFunctionArgument,
    ancestor_path: &[&StyleSibling],
) -> bool {
    let classes: Vec<&str> = child.classes.iter().map(String::as_str).collect();
    let ancestor_classes: Vec<Vec<&str>> = ancestor_path
        .iter()
        .map(|ancestor| ancestor.classes.iter().map(String::as_str).collect())
        .collect();
    let style_ancestors: Vec<StyleAncestor<'_>> = ancestor_path
        .iter()
        .zip(ancestor_classes.iter())
        .map(|(ancestor, classes)| StyleAncestor {
            id: ancestor.id.as_str(),
            key: ancestor.key.as_deref(),
            attributes: &ancestor.attributes,
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let child_element = StyleElement {
        id: child.id.as_str(),
        key: child.key.as_deref(),
        attributes: &child.attributes,
        classes: &classes,
        kind: child.kind,
        ancestors: &style_ancestors,
        pseudo: &child.pseudo,
        sibling_index: Some(child_index),
        sibling_count: Some(child_count),
        siblings: Some(siblings),
        children: Some(&child.children),
    };
    has_argument_selector_matches(&selector.selector, &child_element, ancestor_path)
}

fn has_argument_selector_matches(
    selector: &DgSelector,
    element: &StyleElement<'_>,
    ancestor_path: &[&StyleSibling],
) -> bool {
    match selector {
        DgSelector::Root => false,
        DgSelector::Compound(selector) => selector.matches_element(element),
        DgSelector::Child { parent, child } => {
            child.matches_element(element)
                && ancestor_path.first().is_some_and(|ancestor| {
                    has_argument_selector_matches_sibling(parent, ancestor, &ancestor_path[1..])
                })
        }
        DgSelector::Chain(chain) => has_argument_chain_matches(chain, element, ancestor_path),
    }
}

fn has_argument_chain_matches(
    chain: &DgSelectorChain,
    element: &StyleElement<'_>,
    ancestor_path: &[&StyleSibling],
) -> bool {
    if !chain.target.matches_element(element) {
        return false;
    }

    let mut ancestor_idx = 0;
    for (combinator, selector) in &chain.ancestors {
        match combinator {
            DgCombinator::Child => {
                let Some(ancestor) = ancestor_path.get(ancestor_idx) else {
                    return false;
                };
                if !has_argument_compound_matches_sibling(
                    selector,
                    ancestor,
                    &ancestor_path[ancestor_idx + 1..],
                ) {
                    return false;
                }
                ancestor_idx += 1;
            }
            DgCombinator::Descendant => {
                let Some(found_idx) = ancestor_path[ancestor_idx..].iter().enumerate().position(
                    |(offset, ancestor)| {
                        has_argument_compound_matches_sibling(
                            selector,
                            ancestor,
                            &ancestor_path[ancestor_idx + offset + 1..],
                        )
                    },
                ) else {
                    return false;
                };
                ancestor_idx += found_idx + 1;
            }
        }
    }
    true
}

fn has_argument_selector_matches_sibling(
    selector: &DgSelector,
    sibling: &StyleSibling,
    ancestor_path: &[&StyleSibling],
) -> bool {
    let classes: Vec<&str> = sibling.classes.iter().map(String::as_str).collect();
    let ancestor_classes: Vec<Vec<&str>> = ancestor_path
        .iter()
        .map(|ancestor| ancestor.classes.iter().map(String::as_str).collect())
        .collect();
    let style_ancestors: Vec<StyleAncestor<'_>> = ancestor_path
        .iter()
        .zip(ancestor_classes.iter())
        .map(|(ancestor, classes)| StyleAncestor {
            id: ancestor.id.as_str(),
            key: ancestor.key.as_deref(),
            attributes: &ancestor.attributes,
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let element = StyleElement {
        id: sibling.id.as_str(),
        key: sibling.key.as_deref(),
        attributes: &sibling.attributes,
        classes: &classes,
        kind: sibling.kind,
        ancestors: &style_ancestors,
        pseudo: &sibling.pseudo,
        sibling_index: None,
        sibling_count: None,
        siblings: None,
        children: Some(&sibling.children),
    };
    has_argument_selector_matches(selector, &element, ancestor_path)
}

fn has_argument_compound_matches_sibling(
    selector: &DgCompoundSelector,
    sibling: &StyleSibling,
    ancestor_path: &[&StyleSibling],
) -> bool {
    let classes: Vec<&str> = sibling.classes.iter().map(String::as_str).collect();
    let ancestor_classes: Vec<Vec<&str>> = ancestor_path
        .iter()
        .map(|ancestor| ancestor.classes.iter().map(String::as_str).collect())
        .collect();
    let style_ancestors: Vec<StyleAncestor<'_>> = ancestor_path
        .iter()
        .zip(ancestor_classes.iter())
        .map(|(ancestor, classes)| StyleAncestor {
            id: ancestor.id.as_str(),
            key: ancestor.key.as_deref(),
            attributes: &ancestor.attributes,
            classes,
            kind: ancestor.kind,
        })
        .collect();
    let element = StyleElement {
        id: sibling.id.as_str(),
        key: sibling.key.as_deref(),
        attributes: &sibling.attributes,
        classes: &classes,
        kind: sibling.kind,
        ancestors: &style_ancestors,
        pseudo: &sibling.pseudo,
        sibling_index: None,
        sibling_count: None,
        siblings: None,
        children: Some(&sibling.children),
    };
    selector.matches_element(&element)
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
    pub children: Option<&'a [StyleSibling]>,
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
    pub pseudo: Vec<DgPseudoClass>,
    pub kind: WidgetKind,
    pub children: Vec<StyleSibling>,
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
    Animation(DgAnimationPropertyName),
    Generated(DgGeneratedPropertyName),
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
    FlexBasis,
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
    MarginLeft,
    MarginRight,
    MarginTop,
    MarginBottom,
    Gap,
    RowGap,
    ColumnGap,
    GridTemplateColumns,
    GridTemplateRows,
    GridTemplateAreas,
    GridAutoFlow,
    GridArea,
    GridColumn,
    GridRow,
    ContainerName,
    ContainerType,
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
    BackgroundImage,
    BackdropFilter,
    Foreground,
    BorderColor,
    BorderWidth,
    BorderStyle,
    OutlineColor,
    OutlineWidth,
    OutlineStyle,
    OutlineOffset,
    Outline,
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
    GradientInterpolation,
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
    TextAreaRows,
    ScatterPointSize,
    ScatterPointStyle,
    ScatterGridVisible,
    ScatterGridPlanes,
    ScatterLegendPosition,
    ScatterOrientationAxes,
    TableRowHeight,
    TableHeaderHeight,
    TableColumnWidth,
    TableIndexWidth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgTransitionPropertyName {
    Transition,
    Property,
    Duration,
    TimingFunction,
    Delay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgAnimationPropertyName {
    Animation,
    Name,
    Duration,
    TimingFunction,
    Delay,
    IterationCount,
    Direction,
    FillMode,
    PlayState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgGeneratedPropertyName {
    Content,
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
    /// Layout: display, flex-direction, flex, flex-grow, flex-shrink, flex-basis, width,
    /// height, min-width, min-height, max-width, max-height, padding,
    /// padding-left, padding-right, padding-top, padding-bottom, margin,
    /// margin-left, margin-right, margin-top, margin-bottom, gap,
    /// grid-template-columns, grid-template-rows, grid-template-areas,
    /// grid-auto-flow, grid-area, grid-column, grid-row, container-name,
    /// container-type, overflow, position, top, right, bottom, left.
    ///
    /// Visual: background, background-color, background-image, foreground,
    /// border-color, border-width, border-style, outline, outline-color,
    /// outline-width, outline-style, outline-offset, border-radius, border-top-left-radius,
    /// border-top-right-radius, border-bottom-right-radius,
    /// border-bottom-left-radius, border, box-shadow, opacity, accent, track-color,
    /// thumb-color, transform, translate, scale, rotate.
    ///
    /// Text: color, font-size, font-family, font-weight, text-align,
    /// text-transform, letter-spacing, line-height, font-style,
    /// font-variant-numeric, text-overflow.
    ///
    /// Widget: text-area-rows, scatter-point-size, scatter-point-style,
    /// scatter-grid-visible, scatter-grid-planes, scatter-legend-position,
    /// scatter-orientation-axes, table-row-height, table-header-height,
    /// table-column-width, table-index-width.
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
            "flex-basis" => Ok(Self::Layout(DgLayoutPropertyName::FlexBasis)),
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
            "margin-left" => Ok(Self::Layout(DgLayoutPropertyName::MarginLeft)),
            "margin-right" => Ok(Self::Layout(DgLayoutPropertyName::MarginRight)),
            "margin-top" => Ok(Self::Layout(DgLayoutPropertyName::MarginTop)),
            "margin-bottom" => Ok(Self::Layout(DgLayoutPropertyName::MarginBottom)),
            "gap" => Ok(Self::Layout(DgLayoutPropertyName::Gap)),
            "row-gap" => Ok(Self::Layout(DgLayoutPropertyName::RowGap)),
            "column-gap" => Ok(Self::Layout(DgLayoutPropertyName::ColumnGap)),
            "grid-template-columns" => Ok(Self::Layout(DgLayoutPropertyName::GridTemplateColumns)),
            "grid-template-rows" => Ok(Self::Layout(DgLayoutPropertyName::GridTemplateRows)),
            "grid-template-areas" => Ok(Self::Layout(DgLayoutPropertyName::GridTemplateAreas)),
            "grid-auto-flow" => Ok(Self::Layout(DgLayoutPropertyName::GridAutoFlow)),
            "grid-area" => Ok(Self::Layout(DgLayoutPropertyName::GridArea)),
            "grid-column" => Ok(Self::Layout(DgLayoutPropertyName::GridColumn)),
            "grid-row" => Ok(Self::Layout(DgLayoutPropertyName::GridRow)),
            "container-name" => Ok(Self::Layout(DgLayoutPropertyName::ContainerName)),
            "container-type" => Ok(Self::Layout(DgLayoutPropertyName::ContainerType)),
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
            "background-image" => Ok(Self::Visual(DgVisualPropertyName::BackgroundImage)),
            "backdrop-filter" => Ok(Self::Visual(DgVisualPropertyName::BackdropFilter)),
            "foreground" => Ok(Self::Visual(DgVisualPropertyName::Foreground)),
            "border-color" => Ok(Self::Visual(DgVisualPropertyName::BorderColor)),
            "border-width" => Ok(Self::Visual(DgVisualPropertyName::BorderWidth)),
            "border-style" => Ok(Self::Visual(DgVisualPropertyName::BorderStyle)),
            "outline-color" => Ok(Self::Visual(DgVisualPropertyName::OutlineColor)),
            "outline-width" => Ok(Self::Visual(DgVisualPropertyName::OutlineWidth)),
            "outline-style" => Ok(Self::Visual(DgVisualPropertyName::OutlineStyle)),
            "outline-offset" => Ok(Self::Visual(DgVisualPropertyName::OutlineOffset)),
            "outline" => Ok(Self::Visual(DgVisualPropertyName::Outline)),
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
            "gradient-interpolation" => {
                Ok(Self::Visual(DgVisualPropertyName::GradientInterpolation))
            }
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
            "animation" => Ok(Self::Animation(DgAnimationPropertyName::Animation)),
            "animation-name" => Ok(Self::Animation(DgAnimationPropertyName::Name)),
            "animation-duration" => Ok(Self::Animation(DgAnimationPropertyName::Duration)),
            "animation-timing-function" => {
                Ok(Self::Animation(DgAnimationPropertyName::TimingFunction))
            }
            "animation-delay" => Ok(Self::Animation(DgAnimationPropertyName::Delay)),
            "animation-iteration-count" => {
                Ok(Self::Animation(DgAnimationPropertyName::IterationCount))
            }
            "animation-direction" => Ok(Self::Animation(DgAnimationPropertyName::Direction)),
            "animation-fill-mode" => Ok(Self::Animation(DgAnimationPropertyName::FillMode)),
            "animation-play-state" => Ok(Self::Animation(DgAnimationPropertyName::PlayState)),
            "content" => Ok(Self::Generated(DgGeneratedPropertyName::Content)),
            "text-area-rows" => Ok(Self::Widget(DgWidgetPropertyName::TextAreaRows)),
            "scatter-point-size" => Ok(Self::Widget(DgWidgetPropertyName::ScatterPointSize)),
            "scatter-point-style" => Ok(Self::Widget(DgWidgetPropertyName::ScatterPointStyle)),
            "scatter-grid-visible" => Ok(Self::Widget(DgWidgetPropertyName::ScatterGridVisible)),
            "scatter-grid-planes" => Ok(Self::Widget(DgWidgetPropertyName::ScatterGridPlanes)),
            "scatter-legend-position" => {
                Ok(Self::Widget(DgWidgetPropertyName::ScatterLegendPosition))
            }
            "scatter-orientation-axes" => {
                Ok(Self::Widget(DgWidgetPropertyName::ScatterOrientationAxes))
            }
            "table-row-height" => Ok(Self::Widget(DgWidgetPropertyName::TableRowHeight)),
            "table-header-height" => Ok(Self::Widget(DgWidgetPropertyName::TableHeaderHeight)),
            "table-column-width" => Ok(Self::Widget(DgWidgetPropertyName::TableColumnWidth)),
            "table-index-width" => Ok(Self::Widget(DgWidgetPropertyName::TableIndexWidth)),
            _ => Err(DgStyleWarning::unsupported_property(name)),
        }
    }
}

pub fn widget_kind_from_css_type(name: &str) -> Option<WidgetKind> {
    match name.trim() {
        "Window" => Some(WidgetKind::Window),
        "HLayout" => Some(WidgetKind::HLayout),
        "VLayout" => Some(WidgetKind::VLayout),
        "ScrollArea" => Some(WidgetKind::ScrollArea),
        "GridLayout" => Some(WidgetKind::GridLayout),
        "FlowLayout" => Some(WidgetKind::FlowLayout),
        "Splitter" => Some(WidgetKind::Splitter),
        "Pane" => Some(WidgetKind::Pane),
        "Panel" => Some(WidgetKind::Panel),
        "Collapsible" => Some(WidgetKind::Collapsible),
        "Modal" => Some(WidgetKind::Modal),
        "Badge" => Some(WidgetKind::Badge),
        "Tag" => Some(WidgetKind::Tag),
        "LED" | "Led" => Some(WidgetKind::Led),
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
        "SmallButton" => Some(WidgetKind::SmallButton),
        "IconButton" => Some(WidgetKind::IconButton),
        "ImageButton" => Some(WidgetKind::ImageButton),
        "ArrowButton" => Some(WidgetKind::ArrowButton),
        "Selectable" => Some(WidgetKind::Selectable),
        "RadioButton" => Some(WidgetKind::RadioButton),
        "TreeView" => Some(WidgetKind::TreeView),
        "TreeNode" => Some(WidgetKind::TreeNode),
        "DragSource" => Some(WidgetKind::DragSource),
        "DropTarget" => Some(WidgetKind::DropTarget),
        "TextInput" => Some(WidgetKind::TextInput),
        "TextArea" => Some(WidgetKind::TextArea),
        "CodeEditor" => Some(WidgetKind::CodeEditor),
        "LogView" => Some(WidgetKind::LogView),
        "NumberInput" => Some(WidgetKind::NumberInput),
        "DragNumber" => Some(WidgetKind::DragNumber),
        "Slider" => Some(WidgetKind::Slider),
        "RangeSlider" => Some(WidgetKind::RangeSlider),
        "ProgressBar" => Some(WidgetKind::ProgressBar),
        "LoadingSpinner" => Some(WidgetKind::LoadingSpinner),
        "Dropdown" => Some(WidgetKind::Dropdown),
        "Checkbox" => Some(WidgetKind::Checkbox),
        "ToggleSwitch" => Some(WidgetKind::ToggleSwitch),
        "Separator" => Some(WidgetKind::Separator),
        "Spacer" => Some(WidgetKind::Spacer),
        "PieChart" => Some(WidgetKind::PieChart),
        "Histogram" => Some(WidgetKind::Histogram),
        "BarChart" => Some(WidgetKind::BarChart),
        "Heatmap" => Some(WidgetKind::Heatmap),
        "LinePlot" => Some(WidgetKind::LinePlot),
        "Scatter3D" => Some(WidgetKind::Scatter3D),
        "DataFrameTable" => Some(WidgetKind::DataFrameTable),
        "HtmlReport" => Some(WidgetKind::HtmlReport),
        "Image" => Some(WidgetKind::Image),
        "ExtensionWidget" | "Extension" | "PaintWidget" => Some(WidgetKind::Extension),
        _ => None,
    }
}

pub fn css_type_name(kind: WidgetKind) -> Option<&'static str> {
    match kind {
        WidgetKind::Window => Some("Window"),
        WidgetKind::HLayout => Some("HLayout"),
        WidgetKind::VLayout => Some("VLayout"),
        WidgetKind::ScrollArea => Some("ScrollArea"),
        WidgetKind::GridLayout => Some("GridLayout"),
        WidgetKind::FlowLayout => Some("FlowLayout"),
        WidgetKind::Splitter => Some("Splitter"),
        WidgetKind::Pane => Some("Pane"),
        WidgetKind::Panel => Some("Panel"),
        WidgetKind::Collapsible => Some("Collapsible"),
        WidgetKind::Modal => Some("Modal"),
        WidgetKind::Badge => Some("Badge"),
        WidgetKind::Tag => Some("Tag"),
        WidgetKind::Led => Some("LED"),
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
        WidgetKind::SmallButton => Some("SmallButton"),
        WidgetKind::IconButton => Some("IconButton"),
        WidgetKind::ImageButton => Some("ImageButton"),
        WidgetKind::ArrowButton => Some("ArrowButton"),
        WidgetKind::Selectable => Some("Selectable"),
        WidgetKind::RadioButton => Some("RadioButton"),
        WidgetKind::TreeView => Some("TreeView"),
        WidgetKind::TreeNode => Some("TreeNode"),
        WidgetKind::DragSource => Some("DragSource"),
        WidgetKind::DropTarget => Some("DropTarget"),
        WidgetKind::TextInput => Some("TextInput"),
        WidgetKind::TextArea => Some("TextArea"),
        WidgetKind::CodeEditor => Some("CodeEditor"),
        WidgetKind::LogView => Some("LogView"),
        WidgetKind::NumberInput => Some("NumberInput"),
        WidgetKind::DragNumber => Some("DragNumber"),
        WidgetKind::Slider => Some("Slider"),
        WidgetKind::RangeSlider => Some("RangeSlider"),
        WidgetKind::ProgressBar => Some("ProgressBar"),
        WidgetKind::LoadingSpinner => Some("LoadingSpinner"),
        WidgetKind::Dropdown => Some("Dropdown"),
        WidgetKind::Checkbox => Some("Checkbox"),
        WidgetKind::ToggleSwitch => Some("ToggleSwitch"),
        WidgetKind::Separator => Some("Separator"),
        WidgetKind::Spacer => Some("Spacer"),
        WidgetKind::PieChart => Some("PieChart"),
        WidgetKind::Histogram => Some("Histogram"),
        WidgetKind::BarChart => Some("BarChart"),
        WidgetKind::Heatmap => Some("Heatmap"),
        WidgetKind::LinePlot => Some("LinePlot"),
        WidgetKind::Scatter3D => Some("Scatter3D"),
        WidgetKind::DataFrameTable => Some("DataFrameTable"),
        WidgetKind::HtmlReport => Some("HtmlReport"),
        WidgetKind::Image => Some("Image"),
        WidgetKind::Extension => Some("ExtensionWidget"),
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
    container_names: Vec<String>,
    container_type: Option<ContainerTypeStyle>,
}

impl AncestorSnapshot {
    fn from_node(node: &WidgetNode, features: StylesheetMatchFeatures) -> Self {
        Self {
            id: node.id.clone(),
            key: node.key.clone(),
            classes: if features.ancestor_selectors {
                node_css_classes(node)
                    .into_iter()
                    .map(str::to_string)
                    .collect()
            } else {
                Vec::new()
            },
            attributes: if features.ancestor_selectors && features.attributes {
                node_style_attributes(node)
            } else {
                Vec::new()
            },
            kind: node.kind,
            container_names: node
                .style
                .layout
                .container_names
                .clone()
                .unwrap_or_default(),
            container_type: node.style.layout.container_type,
        }
    }
}

impl StyleSibling {
    fn from_node(node: &WidgetNode, features: StylesheetMatchFeatures) -> Self {
        Self {
            id: node.id.clone(),
            key: node.key.clone(),
            classes: node_css_classes(node)
                .into_iter()
                .map(str::to_string)
                .collect(),
            attributes: if features.attributes {
                node_style_attributes(node)
            } else {
                Vec::new()
            },
            pseudo: node_snapshot_pseudo_classes(node),
            kind: node.kind,
            children: if features.sibling_snapshots {
                node.children
                    .iter()
                    .map(|child| StyleSibling::from_node(child, features))
                    .collect()
            } else {
                Vec::new()
            },
        }
    }
}

fn node_snapshot_pseudo_classes(node: &WidgetNode) -> Vec<DgPseudoClass> {
    let mut pseudos = Vec::new();
    if node.props.disabled {
        pseudos.push(DgPseudoClass::Disabled);
    }
    if matches!(node.kind, WidgetKind::Checkbox | WidgetKind::ToggleSwitch)
        && node.props.checked.unwrap_or(false)
    {
        pseudos.push(DgPseudoClass::Checked);
    }
    if node.kind == WidgetKind::Selectable && node.props.checked.unwrap_or(false) {
        pseudos.push(DgPseudoClass::Selected);
    }
    if node.kind == WidgetKind::RadioButton && node.props.checked.unwrap_or(false) {
        pseudos.push(DgPseudoClass::Checked);
        pseudos.push(DgPseudoClass::Selected);
    }
    if node.kind == WidgetKind::TreeNode {
        if node.props.checked.unwrap_or(false) {
            pseudos.push(DgPseudoClass::Selected);
        }
        if node.props.expanded.unwrap_or(false) {
            pseudos.push(DgPseudoClass::Expanded);
        } else {
            pseudos.push(DgPseudoClass::Collapsed);
        }
    }
    if node.kind == WidgetKind::Modal && node.props.open == Some(true) {
        pseudos.push(DgPseudoClass::Open);
    }
    if node.kind == WidgetKind::Collapsible {
        if node.props.expanded.unwrap_or(true) {
            pseudos.push(DgPseudoClass::Expanded);
        } else {
            pseudos.push(DgPseudoClass::Collapsed);
        }
    }
    pseudos
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
    push_attr_opt(
        &mut attributes,
        "html-report-path",
        props.html_report_path.as_deref(),
    );
    push_attr_opt(
        &mut attributes,
        "html-report-base-dir",
        props.html_report_base_dir.as_deref(),
    );
    push_attr_bool_if_true(
        &mut attributes,
        "allow-remote",
        props.html_report_allow_remote,
    );
    push_attr_bool_if_true(
        &mut attributes,
        "allow-scripts",
        props.html_report_allow_scripts,
    );
    push_attr_bool_if_true(
        &mut attributes,
        "external-fallback",
        props.html_report_external_fallback,
    );
    push_attr_opt(&mut attributes, "state", props.led_state.as_deref());
    push_attr_number_opt(&mut attributes, "width", props.fixed_width);
    push_attr_number_opt(&mut attributes, "height", props.fixed_height);
    push_attr_number_opt(&mut attributes, "size", props.led_size);
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
    if node.kind == WidgetKind::Led {
        if let Some(state) = node
            .props
            .led_state
            .as_deref()
            .filter(|state| !state.is_empty())
        {
            if !classes.iter().any(|class| *class == state) {
                classes.push(state);
            }
        }
    }
    classes
}

fn selector_match_slots(
    selector: &DgSelector,
    base_element: &StyleElement<'_>,
) -> Vec<Option<DgPseudoClass>> {
    if !selector.target_contains_style_slot_pseudo() {
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

pub fn apply_stylesheets_to_tree_for_media_and_containers(
    root: &mut WidgetNode,
    store: &mut StylesheetStore,
    media: DgMediaEnvironment,
    containers: Option<&DgContainerQueryContext>,
) {
    apply_stylesheets_to_tree_with_media_and_containers(root, store, Some(media), containers);
}

fn apply_stylesheets_to_tree_with_media(
    root: &mut WidgetNode,
    store: &mut StylesheetStore,
    media: Option<DgMediaEnvironment>,
) {
    apply_stylesheets_to_tree_with_media_and_containers(root, store, media, None);
}

fn apply_stylesheets_to_tree_with_media_and_containers(
    root: &mut WidgetNode,
    store: &mut StylesheetStore,
    media: Option<DgMediaEnvironment>,
    containers: Option<&DgContainerQueryContext>,
) {
    let mut ancestors = Vec::new();
    let mut validation_warnings = Vec::new();
    let mut seen_validation_warnings = BTreeSet::new();
    {
        let rules = store.all_rules();
        let features = rules.match_features();
        let mut matched_scratch = Vec::new();
        let mut candidate_scratch = Vec::new();
        apply_stylesheets_to_node(
            root,
            &rules,
            features,
            &mut ancestors,
            None,
            &mut validation_warnings,
            &mut seen_validation_warnings,
            &mut matched_scratch,
            &mut candidate_scratch,
            None,
            None,
            None,
            media,
            containers,
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
    let features = rules.match_features();
    let mut ancestors = Vec::new();
    let mut out = BTreeMap::new();
    collect_matched_rule_labels(
        root,
        &rules,
        features,
        &mut ancestors,
        &mut out,
        None,
        None,
        None,
        media,
        None,
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
    let features = rules.match_features();
    let mut ancestors = Vec::new();
    let mut out = BTreeMap::new();
    collect_matched_part_rule_labels(
        root,
        &rules,
        features,
        &mut ancestors,
        &mut out,
        None,
        None,
        None,
        media,
        None,
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
        children: None,
    };
    let mut matched = Vec::new();
    rules.for_each_candidate_rule(&element, |rule| {
        if !rule.target_may_match(&element) {
            return;
        }
        if !rule_matches_conditions(rule, media, &[], None) {
            return;
        }
        if rule.selector.target_part().is_some() {
            return;
        }
        if rule.selector.target_contains_style_slot_pseudo() {
            let slots = selector_match_slots(&rule.selector, &element);
            for slot in slots {
                matched.extend(rule.declarations.iter().map(|declaration| {
                    (rule.cascade_key(declaration), slot, &declaration.property)
                }));
            }
        } else if rule.selector.matches(&element) {
            matched.extend(
                rule.declarations.iter().map(|declaration| {
                    (rule.cascade_key(declaration), None, &declaration.property)
                }),
            );
        }
    });
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

fn rule_matches_conditions(
    rule: &DgStyleRule,
    media: Option<DgMediaEnvironment>,
    ancestors: &[AncestorSnapshot],
    containers: Option<&DgContainerQueryContext>,
) -> bool {
    rule_matches_media(rule, media) && rule_matches_container(rule, ancestors, containers)
}

fn rule_matches_container(
    rule: &DgStyleRule,
    ancestors: &[AncestorSnapshot],
    containers: Option<&DgContainerQueryContext>,
) -> bool {
    let Some(condition) = &rule.container else {
        return true;
    };
    let Some(containers) = containers else {
        return false;
    };
    let Some(width) = nearest_container_width(condition, ancestors, containers) else {
        return false;
    };
    container_expression_matches(&condition.expression, width)
}

fn nearest_container_width(
    condition: &DgContainerRuleCondition,
    ancestors: &[AncestorSnapshot],
    containers: &DgContainerQueryContext,
) -> Option<f32> {
    ancestors
        .iter()
        .rev()
        .find(|ancestor| {
            ancestor.container_type == Some(ContainerTypeStyle::InlineSize)
                && condition.name.as_ref().is_none_or(|name| {
                    ancestor
                        .container_names
                        .iter()
                        .any(|container_name| container_name == name)
                })
        })
        .and_then(|ancestor| containers.width(&ancestor.id))
}

fn container_expression_matches(expression: &DgContainerExpression, width: f32) -> bool {
    match expression {
        DgContainerExpression::Width(comparison, value) => match comparison {
            DgMediaComparison::Equal => (width - value).abs() <= f32::EPSILON,
            DgMediaComparison::GreaterThan => width > *value,
            DgMediaComparison::GreaterThanEqual => width >= *value,
            DgMediaComparison::LessThan => width < *value,
            DgMediaComparison::LessThanEqual => width <= *value,
        },
        DgContainerExpression::And(expressions) => expressions
            .iter()
            .all(|expression| container_expression_matches(expression, width)),
        DgContainerExpression::Or(expressions) => expressions
            .iter()
            .any(|expression| container_expression_matches(expression, width)),
        DgContainerExpression::Not(expression) => !container_expression_matches(expression, width),
    }
}

fn collect_matched_part_rule_labels(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    features: StylesheetMatchFeatures,
    ancestors: &mut Vec<AncestorSnapshot>,
    out: &mut BTreeMap<String, BTreeMap<String, Vec<String>>>,
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
    containers: Option<&DgContainerQueryContext>,
) {
    let labels = matched_part_rule_labels_for_node(
        node,
        rules,
        features,
        ancestors,
        sibling_index,
        sibling_count,
        siblings,
        media,
        containers,
    );
    if !labels.is_empty() {
        out.insert(node.id.clone(), labels);
    }
    ancestors.push(AncestorSnapshot::from_node(node, features));
    let child_count = node.children.len();
    let child_siblings: Vec<StyleSibling> = if features.sibling_snapshots {
        node.children
            .iter()
            .map(|child| StyleSibling::from_node(child, features))
            .collect()
    } else {
        Vec::new()
    };
    for (index, child) in node.children.iter().enumerate() {
        collect_matched_part_rule_labels(
            child,
            rules,
            features,
            ancestors,
            out,
            Some(index),
            Some(child_count),
            features
                .sibling_snapshots
                .then_some(child_siblings.as_slice()),
            media,
            containers,
        );
    }
    ancestors.pop();
}

fn matched_part_rule_labels_for_node(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    features: StylesheetMatchFeatures,
    ancestors: &[AncestorSnapshot],
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
    containers: Option<&DgContainerQueryContext>,
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
    let attributes = if features.attributes {
        node_style_attributes(node)
    } else {
        Vec::new()
    };
    let child_siblings: Vec<StyleSibling> = if features.sibling_snapshots {
        node.children
            .iter()
            .map(|child| StyleSibling::from_node(child, features))
            .collect()
    } else {
        Vec::new()
    };
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
        children: features
            .sibling_snapshots
            .then_some(child_siblings.as_slice()),
    };
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    rules.for_each_candidate_rule(&element, |rule| {
        if !rule.target_may_match(&element)
            || !rule_matches_conditions(rule, media, ancestors, containers)
            || !rule_selector_matches_any_slot(rule, &element)
        {
            return;
        }
        let Some(part) = rule.selector.target_part() else {
            return;
        };
        if !widget_kind_supports_part(node.kind, part) {
            return;
        }
        out.entry(part.to_string()).or_default().push(format!(
            "{}: {}",
            rule.origin.label(),
            rule.selector.label()
        ));
    });
    out
}

fn collect_matched_rule_labels(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    features: StylesheetMatchFeatures,
    ancestors: &mut Vec<AncestorSnapshot>,
    out: &mut BTreeMap<String, Vec<String>>,
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
    containers: Option<&DgContainerQueryContext>,
) {
    let labels = matched_rule_labels_for_node(
        node,
        rules,
        features,
        ancestors,
        sibling_index,
        sibling_count,
        siblings,
        media,
        containers,
    );
    if !labels.is_empty() {
        out.insert(node.id.clone(), labels);
    }
    ancestors.push(AncestorSnapshot::from_node(node, features));
    let child_count = node.children.len();
    let child_siblings: Vec<StyleSibling> = if features.sibling_snapshots {
        node.children
            .iter()
            .map(|child| StyleSibling::from_node(child, features))
            .collect()
    } else {
        Vec::new()
    };
    for (index, child) in node.children.iter().enumerate() {
        collect_matched_rule_labels(
            child,
            rules,
            features,
            ancestors,
            out,
            Some(index),
            Some(child_count),
            features
                .sibling_snapshots
                .then_some(child_siblings.as_slice()),
            media,
            containers,
        );
    }
    ancestors.pop();
}

fn matched_rule_labels_for_node(
    node: &WidgetNode,
    rules: &StylesheetRuleRefs<'_>,
    features: StylesheetMatchFeatures,
    ancestors: &[AncestorSnapshot],
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
    containers: Option<&DgContainerQueryContext>,
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
    let attributes = if features.attributes {
        node_style_attributes(node)
    } else {
        Vec::new()
    };
    let child_siblings: Vec<StyleSibling> = if features.sibling_snapshots {
        node.children
            .iter()
            .map(|child| StyleSibling::from_node(child, features))
            .collect()
    } else {
        Vec::new()
    };
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
        children: features
            .sibling_snapshots
            .then_some(child_siblings.as_slice()),
    };
    let mut out = Vec::new();
    rules.for_each_candidate_rule(&element, |rule| {
        if rule.target_may_match(&element)
            && rule_matches_conditions(rule, media, ancestors, containers)
            && rule_selector_matches_any_slot(rule, &element)
        {
            out.push(format!(
                "{}: {}",
                rule.origin.label(),
                rule.selector.label()
            ));
        }
    });
    out
}

fn rule_selector_matches_any_slot(rule: &DgStyleRule, element: &StyleElement<'_>) -> bool {
    if rule.selector.target_contains_style_slot_pseudo() {
        !selector_match_slots(&rule.selector, element).is_empty()
    } else {
        rule.selector.matches(element)
    }
}

type MatchedStyleDeclaration<'a> = (
    CascadeKey,
    Option<DgPseudoClass>,
    Option<&'a str>,
    &'a DgStyleProperty,
);

fn collect_stylesheet_node_rule_matches<'a>(
    node: &WidgetNode,
    rule: &'a DgStyleRule,
    element: &StyleElement<'_>,
    ancestors: &[AncestorSnapshot],
    validation_warnings: &mut Vec<DgStyleWarning>,
    seen_validation_warnings: &mut BTreeSet<String>,
    media: Option<DgMediaEnvironment>,
    containers: Option<&DgContainerQueryContext>,
    matched: &mut Vec<MatchedStyleDeclaration<'a>>,
) {
    if !rule.target_may_match(element) {
        return;
    }
    if !rule_matches_conditions(rule, media, ancestors, containers) {
        return;
    }
    if rule.selector.target_contains_style_slot_pseudo() {
        let slots = selector_match_slots(&rule.selector, element);
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
                    return;
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
    } else if rule.selector.matches(element) {
        if let Some(part) = rule.selector.target_part() {
            if !widget_kind_supports_part(node.kind, part) {
                record_unsupported_part_warning(
                    validation_warnings,
                    seen_validation_warnings,
                    rule,
                    node.kind,
                    part,
                );
                return;
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
                None,
                rule.selector.target_part(),
                &declaration.property,
            )
        }));
    }
}

fn compute_stylesheet_node_style<'a>(
    node: &WidgetNode,
    rules: &'a StylesheetRuleRefs<'a>,
    element: &StyleElement<'_>,
    ancestors: &[AncestorSnapshot],
    validation_warnings: &mut Vec<DgStyleWarning>,
    seen_validation_warnings: &mut BTreeSet<String>,
    media: Option<DgMediaEnvironment>,
    containers: Option<&DgContainerQueryContext>,
    matched: &mut Vec<MatchedStyleDeclaration<'a>>,
    candidate_scratch: &mut Vec<usize>,
) -> NodeStyle {
    // Pseudo-state selectors are matched against base and single-state contexts
    // here. Their declarations are precomputed into hover/active/focus/disabled
    // style slots, and live widget state decides which slot is active.
    matched.clear();
    if rules.uses_linear_candidates() {
        for rule in rules.iter() {
            collect_stylesheet_node_rule_matches(
                node,
                rule,
                element,
                ancestors,
                validation_warnings,
                seen_validation_warnings,
                media,
                containers,
                matched,
            );
        }
    } else {
        rules.for_each_candidate_rule_with_scratch(element, candidate_scratch, |rule| {
            collect_stylesheet_node_rule_matches(
                node,
                rule,
                element,
                ancestors,
                validation_warnings,
                seen_validation_warnings,
                media,
                containers,
                matched,
            );
        });
    }
    matched.sort_by_key(|(key, _, _, _)| *key);

    let mut computed = NodeStyle::default();
    for (_, slot, part, property) in matched.drain(..) {
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
    computed
}

fn apply_stylesheets_to_node<'a>(
    node: &mut WidgetNode,
    rules: &'a StylesheetRuleRefs<'a>,
    features: StylesheetMatchFeatures,
    ancestors: &mut Vec<AncestorSnapshot>,
    inherited_text: Option<&TextStyle>,
    validation_warnings: &mut Vec<DgStyleWarning>,
    seen_validation_warnings: &mut BTreeSet<String>,
    matched_scratch: &mut Vec<MatchedStyleDeclaration<'a>>,
    candidate_scratch: &mut Vec<usize>,
    sibling_index: Option<usize>,
    sibling_count: Option<usize>,
    siblings: Option<&[StyleSibling]>,
    media: Option<DgMediaEnvironment>,
    containers: Option<&DgContainerQueryContext>,
) {
    let classes = node_css_classes(node);
    let ancestor_classes: Vec<Vec<&str>>;
    let style_ancestors: Vec<StyleAncestor<'_>>;
    if features.ancestor_selectors {
        ancestor_classes = ancestors
            .iter()
            .map(|ancestor| ancestor.classes.iter().map(String::as_str).collect())
            .collect();
        style_ancestors = ancestors
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
    } else {
        style_ancestors = Vec::new();
    }
    let attributes = if features.attributes {
        node_style_attributes(node)
    } else {
        Vec::new()
    };
    let child_count = node.children.len();
    let child_siblings: Vec<StyleSibling> = if features.sibling_snapshots {
        node.children
            .iter()
            .map(|child| StyleSibling::from_node(child, features))
            .collect()
    } else {
        Vec::new()
    };
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
        children: features
            .sibling_snapshots
            .then_some(child_siblings.as_slice()),
    };
    let mut computed = compute_stylesheet_node_style(
        node,
        rules,
        &element,
        ancestors,
        validation_warnings,
        seen_validation_warnings,
        media,
        containers,
        matched_scratch,
        candidate_scratch,
    );
    if let Some(inherited_text) = inherited_text {
        inherit_text_style(&mut computed.text, inherited_text);
    }
    node.style = computed;

    let pushed_ancestor = features.needs_ancestor_snapshots();
    if pushed_ancestor {
        ancestors.push(AncestorSnapshot::from_node(node, features));
    }
    let child_text = node.style.text.clone();
    for (index, child) in node.children.iter_mut().enumerate() {
        apply_stylesheets_to_node(
            child,
            rules,
            features,
            ancestors,
            Some(&child_text),
            validation_warnings,
            seen_validation_warnings,
            matched_scratch,
            candidate_scratch,
            Some(index),
            Some(child_count),
            features
                .sibling_snapshots
                .then_some(child_siblings.as_slice()),
            media,
            containers,
        );
    }
    if pushed_ancestor {
        ancestors.pop();
    }
}

fn record_stateful_part_layout_warnings(
    warnings: &mut Vec<DgStyleWarning>,
    seen: &mut BTreeSet<String>,
    rule: &DgStyleRule,
    part: &str,
) {
    if !rule.selector.target_contains_style_slot_pseudo() {
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
    if matches!(part, "before" | "after") {
        return !matches!(
            kind,
            WidgetKind::Window | WidgetKind::Spacer | WidgetKind::Unknown
        );
    }
    if matches!(part, "scrollbar-track" | "scrollbar-thumb")
        && widget_kind_supports_scrollbar_part(kind)
    {
        return true;
    }
    match kind {
        WidgetKind::Panel => matches!(part, "accent"),
        WidgetKind::Collapsible => matches!(part, "header" | "indicator" | "body"),
        WidgetKind::Modal => matches!(part, "scrim"),
        WidgetKind::Menu | WidgetKind::ContextMenu => {
            matches!(part, "menu" | "item" | "item-hover" | "item-disabled")
        }
        WidgetKind::Splitter => matches!(part, "gutter"),
        WidgetKind::Pane => matches!(part, "pane"),
        WidgetKind::Button | WidgetKind::SmallButton => matches!(part, "badge"),
        WidgetKind::IconButton | WidgetKind::ArrowButton => matches!(part, "icon"),
        WidgetKind::ImageButton => matches!(part, "image"),
        WidgetKind::Selectable => matches!(part, "row" | "indicator" | "label"),
        WidgetKind::RadioButton => matches!(part, "indicator" | "dot" | "label"),
        WidgetKind::TreeNode => matches!(part, "row" | "indicator" | "label" | "guide"),
        WidgetKind::CodeEditor => matches!(part, "field" | "gutter" | "line-number" | "caret"),
        WidgetKind::LogView => {
            matches!(part, "line" | "debug" | "info" | "warning" | "error")
        }
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
        WidgetKind::DragNumber => matches!(part, "field" | "value" | "grip"),
        WidgetKind::Dropdown => matches!(
            part,
            "field" | "chevron" | "menu" | "item" | "item-selected" | "item-hover"
        ),
        WidgetKind::Checkbox => matches!(part, "row" | "box" | "indicator" | "label"),
        WidgetKind::ToggleSwitch => matches!(part, "row" | "track" | "thumb" | "label"),
        WidgetKind::Led => matches!(part, "dot" | "glow" | "highlight"),
        WidgetKind::Slider => matches!(part, "track" | "fill" | "thumb"),
        WidgetKind::RangeSlider => {
            matches!(
                part,
                "track" | "range" | "thumb-min" | "thumb-max" | "label"
            )
        }
        WidgetKind::ProgressBar => matches!(part, "track" | "fill" | "label"),
        WidgetKind::LoadingSpinner => matches!(part, "track" | "arc" | "label"),
        WidgetKind::Heatmap => matches!(part, "cell" | "grid" | "hover" | "scalar-bar" | "label"),
        WidgetKind::BarChart => matches!(part, "label" | "value-label"),
        WidgetKind::Extension => false,
        WidgetKind::Tabs => matches!(part, "header"),
        WidgetKind::Tab => matches!(part, "tab" | "accent" | "badge"),
        WidgetKind::NavItem => matches!(part, "item" | "accent" | "badge"),
        WidgetKind::DataFrameTable => {
            matches!(
                part,
                "header"
                    | "row"
                    | "row-selected"
                    | "grid-line"
                    | "scrollbar-track"
                    | "scrollbar-thumb"
            )
        }
        _ => false,
    }
}

fn widget_kind_supports_scrollbar_part(kind: WidgetKind) -> bool {
    matches!(
        kind,
        WidgetKind::HLayout
            | WidgetKind::VLayout
            | WidgetKind::Pages
            | WidgetKind::Page
            | WidgetKind::Sidebar
            | WidgetKind::Panel
            | WidgetKind::Collapsible
            | WidgetKind::Modal
            | WidgetKind::DataFrameTable
    )
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
    base.align_items = overlay.align_items.or(base.align_items);
    base.align_self = overlay.align_self.or(base.align_self);
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
    base.margin_left = overlay.margin_left.or(base.margin_left);
    base.margin_right = overlay.margin_right.or(base.margin_right);
    base.margin_top = overlay.margin_top.or(base.margin_top);
    base.margin_bottom = overlay.margin_bottom.or(base.margin_bottom);
    base.margin_value = overlay.margin_value.or(base.margin_value);
    base.margin_left_value = overlay.margin_left_value.or(base.margin_left_value);
    base.margin_right_value = overlay.margin_right_value.or(base.margin_right_value);
    base.margin_top_value = overlay.margin_top_value.or(base.margin_top_value);
    base.margin_bottom_value = overlay.margin_bottom_value.or(base.margin_bottom_value);
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
    base.flex_basis = overlay.flex_basis.or(base.flex_basis);
    base.flex_basis_value = overlay.flex_basis_value.or(base.flex_basis_value);
    base.grid_template_columns = overlay
        .grid_template_columns
        .clone()
        .or_else(|| base.grid_template_columns.clone());
    base.grid_template_rows = overlay
        .grid_template_rows
        .clone()
        .or_else(|| base.grid_template_rows.clone());
    base.grid_template_areas = overlay
        .grid_template_areas
        .clone()
        .or_else(|| base.grid_template_areas.clone());
    base.grid_auto_flow = overlay.grid_auto_flow.or(base.grid_auto_flow);
    base.grid_area = overlay.grid_area.clone().or_else(|| base.grid_area.clone());
    base.grid_column = overlay.grid_column.or(base.grid_column);
    base.grid_row = overlay.grid_row.or(base.grid_row);
    base.container_names = overlay
        .container_names
        .clone()
        .or_else(|| base.container_names.clone());
    base.container_type = overlay.container_type.or(base.container_type);
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
    base.text_area_rows = overlay.text_area_rows.or(base.text_area_rows);
    base.scatter_point_size = overlay.scatter_point_size.or(base.scatter_point_size);
    base.scatter_point_style = overlay
        .scatter_point_style
        .clone()
        .or_else(|| base.scatter_point_style.clone());
    base.scatter_grid_visible = overlay.scatter_grid_visible.or(base.scatter_grid_visible);
    base.scatter_grid_planes = overlay.scatter_grid_planes.or(base.scatter_grid_planes);
    base.scatter_legend_position = overlay
        .scatter_legend_position
        .clone()
        .or_else(|| base.scatter_legend_position.clone());
    base.scatter_orientation_axes_visible = overlay
        .scatter_orientation_axes_visible
        .or(base.scatter_orientation_axes_visible);
    base.table_row_height = overlay.table_row_height.or(base.table_row_height);
    base.table_header_height = overlay.table_header_height.or(base.table_header_height);
    base.table_column_width = overlay.table_column_width.or(base.table_column_width);
    base.table_index_width = overlay.table_index_width.or(base.table_index_width);
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
    base.content = overlay.content.clone().or_else(|| base.content.clone());
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
        DgStyleProperty::Animation(declaration) => {
            apply_animation_declaration(&mut style.animation, declaration)
        }
        DgStyleProperty::Generated(_) => {}
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
        | DgStyleProperty::Animation(_)
        | DgStyleProperty::Generated(_)
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
        DgStyleProperty::Generated(DgGeneratedDeclaration::Content(value)) => {
            style.content = value.clone()
        }
        DgStyleProperty::Layout(_)
        | DgStyleProperty::Widget(_)
        | DgStyleProperty::Transition(_)
        | DgStyleProperty::Animation(_)
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
        DgLayoutDeclaration::Flex(value) => {
            style.flex_grow = Some(value.0.max(0.0));
            style.flex_shrink = Some(1.0);
            style.flex_basis = Some(0.0);
            style.flex_basis_value = Some(LayoutLength::LogicalPx(0.0));
        }
        DgLayoutDeclaration::FlexGrow(value) => style.flex_grow = Some(value.0.max(0.0)),
        DgLayoutDeclaration::FlexShrink(value) => style.flex_shrink = Some(value.0.max(0.0)),
        DgLayoutDeclaration::FlexBasis(value) => {
            style.flex_basis = length_px(value);
            style.flex_basis_value = layout_length(value);
        }
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
            style.margin_top = length_px(&edges.top);
            style.margin_right = length_px(&edges.right);
            style.margin_bottom = length_px(&edges.bottom);
            style.margin_left = length_px(&edges.left);
            style.margin_top_value = layout_length(&edges.top);
            style.margin_right_value = layout_length(&edges.right);
            style.margin_bottom_value = layout_length(&edges.bottom);
            style.margin_left_value = layout_length(&edges.left);
            if edges.top == edges.right && edges.right == edges.bottom && edges.bottom == edges.left
            {
                style.margin = length_px(&edges.top);
                style.margin_value = layout_length(&edges.top);
            } else {
                style.margin = None;
                style.margin_value = None;
            }
        }
        DgLayoutDeclaration::MarginLeft(value) => {
            style.margin_left = length_px(value);
            style.margin_left_value = layout_length(value);
        }
        DgLayoutDeclaration::MarginRight(value) => {
            style.margin_right = length_px(value);
            style.margin_right_value = layout_length(value);
        }
        DgLayoutDeclaration::MarginTop(value) => {
            style.margin_top = length_px(value);
            style.margin_top_value = layout_length(value);
        }
        DgLayoutDeclaration::MarginBottom(value) => {
            style.margin_bottom = length_px(value);
            style.margin_bottom_value = layout_length(value);
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
        DgLayoutDeclaration::GridTemplateAreas(value) => {
            style.grid_template_areas = Some(grid_template_areas_from_css(value))
        }
        DgLayoutDeclaration::GridAutoFlow(value) => {
            style.grid_auto_flow = Some(grid_auto_flow_from_css(*value))
        }
        DgLayoutDeclaration::GridArea(value) => style.grid_area = Some(value.clone()),
        DgLayoutDeclaration::GridColumn(value) => {
            style.grid_column = Some(grid_placement_from_css(value))
        }
        DgLayoutDeclaration::GridRow(value) => {
            style.grid_row = Some(grid_placement_from_css(value))
        }
        DgLayoutDeclaration::ContainerName(value) => style.container_names = Some(value.clone()),
        DgLayoutDeclaration::ContainerType(value) => style.container_type = Some(*value),
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

fn grid_auto_flow_from_css(value: DgGridAutoFlow) -> GridAutoFlowStyle {
    match value {
        DgGridAutoFlow::Row => GridAutoFlowStyle::Row,
        DgGridAutoFlow::Column => GridAutoFlowStyle::Column,
        DgGridAutoFlow::RowDense => GridAutoFlowStyle::RowDense,
        DgGridAutoFlow::ColumnDense => GridAutoFlowStyle::ColumnDense,
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

fn grid_template_areas_from_css(value: &DgGridTemplateAreas) -> GridTemplateAreas {
    GridTemplateAreas {
        columns: value.columns,
        rows: value.rows,
        areas: value
            .areas
            .iter()
            .map(|area| GridTemplateArea {
                name: area.name.clone(),
                row_start: area.row_start,
                row_end: area.row_end,
                column_start: area.column_start,
                column_end: area.column_end,
            })
            .collect(),
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
        DgVisualDeclaration::BackgroundImage(value) => {
            apply_background_image_declaration(style, value);
        }
        DgVisualDeclaration::BackdropFilter(value) => style.backdrop_filter = *value,
        DgVisualDeclaration::Foreground(value) => {
            style.foreground = Some(color_ref_from_css(value))
        }
        DgVisualDeclaration::BorderColor(value) => {
            style.border_color = Some(color_ref_from_css(value));
        }
        DgVisualDeclaration::BorderWidth(value) => style.border_width = length_px(value),
        DgVisualDeclaration::BorderStyle(value) => {
            if matches!(value, DgBorderStyle::None) {
                style.border_width = Some(0.0);
                style.border_color = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]));
            }
        }
        DgVisualDeclaration::OutlineColor(value) => {
            style.outline_color = Some(color_ref_from_css(value));
        }
        DgVisualDeclaration::OutlineWidth(value) => style.outline_width = length_px(value),
        DgVisualDeclaration::OutlineStyle(value) => {
            if matches!(value, DgBorderStyle::None) {
                style.outline_width = Some(0.0);
                style.outline_color = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]));
            }
        }
        DgVisualDeclaration::OutlineOffset(value) => style.outline_offset = length_px(value),
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
        DgVisualDeclaration::Outline(outline) => match outline.style {
            DgBorderStyle::None => {
                style.outline_width = Some(0.0);
                style.outline_color = Some(ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]));
            }
            DgBorderStyle::Solid => {
                style.outline_width = length_px(&outline.width);
                style.outline_color = Some(color_ref_from_css(&outline.color));
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
        DgVisualDeclaration::GradientInterpolation(value) => {
            style.gradient_interpolation = Some(*value)
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

fn apply_background_image_declaration(style: &mut VisualStyle, value: &Option<DgBackgroundPaint>) {
    style.background_paint = match value {
        Some(value) => {
            let paint = background_paint_from_css(value);
            Some(match style.background.clone() {
                Some(background) => background_image_over_color(paint, background),
                None => paint,
            })
        }
        None => style.background.clone().map(BackgroundPaint::Color),
    };
}

fn background_image_over_color(paint: BackgroundPaint, color: ColorRef) -> BackgroundPaint {
    match paint {
        BackgroundPaint::Layers(mut layers) => {
            layers.push(BackgroundPaint::Color(color));
            BackgroundPaint::Layers(layers)
        }
        paint => BackgroundPaint::Layers(vec![paint, BackgroundPaint::Color(color)]),
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
        DgWidgetDeclaration::TextAreaRows(value) => {
            style.text_area_rows = Some(value.0.round().max(1.0))
        }
        DgWidgetDeclaration::ScatterPointSize(value) => {
            style.scatter_point_size = length_px(value).map(|size| size.max(0.0))
        }
        DgWidgetDeclaration::ScatterPointStyle(kw) => {
            if matches!(kw.0.as_str(), "circle" | "square" | "gaussian") {
                style.scatter_point_style = Some(kw.0.clone());
            }
        }
        DgWidgetDeclaration::ScatterGridVisible(value) => {
            style.scatter_grid_visible = Some(*value);
        }
        DgWidgetDeclaration::ScatterGridPlanes(major, minor) => {
            style.scatter_grid_planes = Some((*major, *minor));
        }
        DgWidgetDeclaration::ScatterLegendPosition(kw) => {
            style.scatter_legend_position = Some(kw.0.clone());
        }
        DgWidgetDeclaration::ScatterOrientationAxes(value) => {
            style.scatter_orientation_axes_visible = Some(*value);
        }
        DgWidgetDeclaration::TableRowHeight(value) => style.table_row_height = length_px(value),
        DgWidgetDeclaration::TableHeaderHeight(value) => {
            style.table_header_height = length_px(value)
        }
        DgWidgetDeclaration::TableColumnWidth(value) => style.table_column_width = length_px(value),
        DgWidgetDeclaration::TableIndexWidth(value) => style.table_index_width = length_px(value),
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

fn parse_container_name_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Vec<String>, DgStyleWarning> {
    let resolved = resolve_keyword(value, variables);
    let trimmed = resolved.trim();
    if trimmed.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let names: Vec<String> = trimmed
        .split_whitespace()
        .filter(|name| !container_name_is_reserved(name))
        .map(str::to_string)
        .collect();
    if names.is_empty() || names.len() != trimmed.split_whitespace().count() {
        return Err(parse_warning(
            name,
            value,
            "one or more container identifiers, or none",
        ));
    }
    Ok(names)
}

fn parse_container_type_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<ContainerTypeStyle, DgStyleWarning> {
    let resolved = resolve_keyword(value, variables);
    match resolved
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .as_str()
    {
        "normal" => Ok(ContainerTypeStyle::Normal),
        "inline-size" => Ok(ContainerTypeStyle::InlineSize),
        _ => Err(parse_warning(name, value, "normal or inline-size")),
    }
}

fn container_name_is_reserved(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "none" | "and" | "or" | "not"
    )
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

fn parse_gradient_interpolation_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<GradientInterpolation, DgStyleWarning> {
    match resolve_keyword(value, variables)
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
        .as_str()
    {
        "srgb" => Ok(GradientInterpolation::Srgb),
        "linear-srgb" => Ok(GradientInterpolation::LinearSrgb),
        "oklab" => Ok(GradientInterpolation::Oklab),
        _ => Err(parse_warning(name, value, "srgb, linear-srgb, or oklab")),
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
        DgBackgroundPaint::BlobGradient(gradient) => BackgroundPaint::BlobGradient(BlobGradient {
            blobs: gradient
                .blobs
                .iter()
                .map(|blob| BlobGradientStop {
                    center: blob.center,
                    radius: blob.radius,
                    color: color_ref_from_css(&blob.color),
                })
                .collect(),
        }),
        DgBackgroundPaint::MeshGradient(gradient) => BackgroundPaint::MeshGradient(MeshGradient {
            top_left: color_ref_from_css(&gradient.top_left),
            top_right: color_ref_from_css(&gradient.top_right),
            bottom_left: color_ref_from_css(&gradient.bottom_left),
            bottom_right: color_ref_from_css(&gradient.bottom_right),
        }),
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
    buckets: StylesheetRuleBuckets,
    pub variables: BTreeMap<String, DgCssValue>,
    pub keyframes: BTreeMap<String, DgKeyframes>,
    pub font_faces: Vec<DgFontFace>,
    pub warnings: Vec<DgStyleWarning>,
}

impl ParsedStylesheet {
    fn with_rules(
        rules: Vec<DgStyleRule>,
        variables: BTreeMap<String, DgCssValue>,
        keyframes: BTreeMap<String, DgKeyframes>,
        font_faces: Vec<DgFontFace>,
        warnings: Vec<DgStyleWarning>,
    ) -> Self {
        let buckets = StylesheetRuleBuckets::build(&rules);
        Self {
            rules,
            buckets,
            variables,
            keyframes,
            font_faces,
            warnings,
        }
    }

    fn for_each_candidate_rule<'a, F>(
        &'a self,
        element: &StyleElement<'_>,
        scratch: &mut Vec<usize>,
        f: &mut F,
    ) where
        F: FnMut(&'a DgStyleRule),
    {
        scratch.clear();
        self.buckets
            .candidate_indices(self.rules.len(), element, scratch);
        for index in scratch.iter().copied() {
            if let Some(rule) = self.rules.get(index) {
                f(rule);
            }
        }
    }

    fn uses_linear_candidates(&self) -> bool {
        self.rules.len() <= StylesheetRuleBuckets::LINEAR_SCAN_RULE_LIMIT
    }
}

#[derive(Debug, Clone, Default)]
struct StylesheetRuleBuckets {
    universal: Vec<usize>,
    by_kind: HashMap<WidgetKind, Vec<usize>>,
    by_id: HashMap<String, Vec<usize>>,
    by_key: HashMap<String, Vec<usize>>,
    by_class: HashMap<String, Vec<usize>>,
}

impl StylesheetRuleBuckets {
    const LINEAR_SCAN_RULE_LIMIT: usize = 32;

    fn build(rules: &[DgStyleRule]) -> Self {
        let mut buckets = Self::default();
        for (index, rule) in rules.iter().enumerate() {
            let filter = &rule.target_filter;
            if let Some(id) = &filter.id {
                buckets.by_id.entry(id.clone()).or_default().push(index);
            } else if let Some(key) = &filter.key {
                buckets.by_key.entry(key.clone()).or_default().push(index);
            } else if let Some(class) = filter.classes.first() {
                buckets
                    .by_class
                    .entry(class.clone())
                    .or_default()
                    .push(index);
            } else if let Some(kind) = filter.kind {
                buckets.by_kind.entry(kind).or_default().push(index);
            } else {
                buckets.universal.push(index);
            }
        }
        buckets
    }

    fn candidate_indices(
        &self,
        rule_count: usize,
        element: &StyleElement<'_>,
        out: &mut Vec<usize>,
    ) {
        if rule_count <= Self::LINEAR_SCAN_RULE_LIMIT {
            out.extend(0..rule_count);
            return;
        }

        out.extend(self.universal.iter().copied());
        if let Some(indices) = self.by_id.get(element.id) {
            out.extend(indices.iter().copied());
        }
        if let Some(key) = element.key {
            if let Some(indices) = self.by_key.get(key) {
                out.extend(indices.iter().copied());
            }
        }
        if let Some(indices) = self.by_kind.get(&element.kind) {
            out.extend(indices.iter().copied());
        }
        for class in element.classes {
            if let Some(indices) = self.by_class.get(*class) {
                out.extend(indices.iter().copied());
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DgFontFace {
    pub family: String,
    pub sources: Vec<DgFontFaceSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DgFontFaceSource {
    pub kind: DgFontFaceSourceKind,
    pub url: String,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DgFontFaceSourceKind {
    Url,
    Local,
}

#[derive(Debug, Clone)]
pub struct DgKeyframes {
    pub name: String,
    pub frames: Vec<DgKeyframe>,
}

#[derive(Debug, Clone)]
pub struct DgKeyframe {
    pub offset: f32,
    pub visual: VisualStyle,
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
    framework: &'a ParsedStylesheet,
    theme: &'a ParsedStylesheet,
    user: &'a ParsedStylesheet,
}

#[derive(Debug, Clone, Copy, Default)]
struct StylesheetMatchFeatures {
    attributes: bool,
    ancestor_selectors: bool,
    container_queries: bool,
    sibling_snapshots: bool,
}

impl StylesheetMatchFeatures {
    fn needs_ancestor_snapshots(self) -> bool {
        self.ancestor_selectors || self.container_queries
    }
}

impl<'a> StylesheetRuleRefs<'a> {
    fn uses_linear_candidates(&self) -> bool {
        self.framework.uses_linear_candidates()
            && self.theme.uses_linear_candidates()
            && self.user.uses_linear_candidates()
    }

    fn iter(&'a self) -> impl Iterator<Item = &'a DgStyleRule> {
        self.framework
            .rules
            .iter()
            .chain(self.theme.rules.iter())
            .chain(self.user.rules.iter())
    }

    pub fn len(&self) -> usize {
        self.framework.rules.len() + self.theme.rules.len() + self.user.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn match_features(&self) -> StylesheetMatchFeatures {
        let mut features = StylesheetMatchFeatures::default();
        for rule in self.iter() {
            features.attributes |= rule.selector.contains_attribute_selector();
            features.ancestor_selectors |= rule.selector.requires_ancestor_matching();
            features.container_queries |= rule.container.is_some();
            features.sibling_snapshots |= rule.selector.requires_sibling_snapshots();
            if features.attributes
                && features.ancestor_selectors
                && features.container_queries
                && features.sibling_snapshots
            {
                break;
            }
        }
        features
    }

    fn for_each_candidate_rule<F>(&'a self, element: &StyleElement<'_>, f: F)
    where
        F: FnMut(&'a DgStyleRule),
    {
        let mut scratch = Vec::new();
        self.for_each_candidate_rule_with_scratch(element, &mut scratch, f);
    }

    fn for_each_candidate_rule_with_scratch<F>(
        &'a self,
        element: &StyleElement<'_>,
        scratch: &mut Vec<usize>,
        mut f: F,
    ) where
        F: FnMut(&'a DgStyleRule),
    {
        self.framework
            .for_each_candidate_rule(element, scratch, &mut f);
        self.theme.for_each_candidate_rule(element, scratch, &mut f);
        self.user.for_each_candidate_rule(element, scratch, &mut f);
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
            framework: &self.framework,
            theme: &self.theme,
            user: &self.user,
        }
    }

    pub fn has_container_rules(&self) -> bool {
        self.all_rules().iter().any(|rule| rule.container.is_some())
    }

    pub fn variables(&self) -> BTreeMap<String, DgCssValue> {
        let mut variables = BTreeMap::new();
        variables.extend(self.framework.variables.clone());
        variables.extend(self.theme.variables.clone());
        variables.extend(self.user.variables.clone());
        variables
    }

    pub fn keyframes(&self) -> BTreeMap<String, DgKeyframes> {
        let mut keyframes = BTreeMap::new();
        keyframes.extend(self.framework.keyframes.clone());
        keyframes.extend(self.theme.keyframes.clone());
        keyframes.extend(self.user.keyframes.clone());
        keyframes
    }

    pub fn font_faces(&self) -> Vec<DgFontFace> {
        let mut font_faces = Vec::new();
        font_faces.extend(self.framework.font_faces.clone());
        font_faces.extend(self.theme.font_faces.clone());
        font_faces.extend(self.user.font_faces.clone());
        font_faces
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
    let mut keyframes = BTreeMap::new();
    let mut font_faces = Vec::new();
    let mut source_order = 0;
    collect_style_rules(
        &sheet.rules,
        origin,
        &variables,
        &mut warnings,
        &mut rules,
        &mut keyframes,
        &mut font_faces,
        &mut source_order,
        None,
        None,
    )?;

    Ok(ParsedStylesheet::with_rules(
        rules, variables, keyframes, font_faces, warnings,
    ))
}

fn collect_style_rules<R>(
    rules_list: &CssRuleList<'_, R>,
    origin: StylesheetOrigin,
    variables: &BTreeMap<String, DgCssValue>,
    warnings: &mut Vec<DgStyleWarning>,
    rules: &mut Vec<DgStyleRule>,
    keyframes: &mut BTreeMap<String, DgKeyframes>,
    font_faces: &mut Vec<DgFontFace>,
    source_order: &mut u32,
    media: Option<DgMediaCondition>,
    container: Option<DgContainerRuleCondition>,
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
                    rules.push(DgStyleRule::with_conditions(
                        selector,
                        declarations,
                        origin,
                        *source_order,
                        media.clone(),
                        container.clone(),
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
                let mut scoped_variables = variables.clone();
                collect_immediate_root_variables(
                    &media_rule.rules,
                    &mut scoped_variables,
                    warnings,
                )?;
                collect_style_rules(
                    &media_rule.rules,
                    origin,
                    &scoped_variables,
                    warnings,
                    rules,
                    keyframes,
                    font_faces,
                    source_order,
                    Some(nested_media),
                    container.clone(),
                )?;
            }
            CssRule::Container(container_rule) => {
                if container.is_some() {
                    warnings.push(DgStyleWarning {
                        property: "@container".to_string(),
                        message: "nested @container rules are not supported by DragonGUI yet"
                            .to_string(),
                    });
                    continue;
                }
                let nested_container = match container_rule_condition(container_rule) {
                    Ok(condition) => condition,
                    Err(message) => {
                        warnings.push(DgStyleWarning {
                            property: "@container".to_string(),
                            message,
                        });
                        continue;
                    }
                };
                let mut scoped_variables = variables.clone();
                collect_immediate_root_variables(
                    &container_rule.rules,
                    &mut scoped_variables,
                    warnings,
                )?;
                collect_style_rules(
                    &container_rule.rules,
                    origin,
                    &scoped_variables,
                    warnings,
                    rules,
                    keyframes,
                    font_faces,
                    source_order,
                    media.clone(),
                    Some(nested_container),
                )?;
            }
            CssRule::Supports(supports_rule) => {
                if supports_condition_matches(&supports_rule.condition, variables) {
                    let mut scoped_variables = variables.clone();
                    collect_immediate_root_variables(
                        &supports_rule.rules,
                        &mut scoped_variables,
                        warnings,
                    )?;
                    collect_style_rules(
                        &supports_rule.rules,
                        origin,
                        &scoped_variables,
                        warnings,
                        rules,
                        keyframes,
                        font_faces,
                        source_order,
                        media.clone(),
                        container.clone(),
                    )?;
                }
            }
            CssRule::Keyframes(rule) => {
                if let Some(parsed) = lower_keyframes_rule(rule, variables, warnings)? {
                    keyframes.insert(parsed.name.clone(), parsed);
                }
            }
            CssRule::FontFace(rule) => {
                if let Some(font_face) = lower_font_face_rule(rule, warnings)? {
                    font_faces.push(font_face);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn lower_font_face_rule(
    rule: &FontFaceRule<'_>,
    warnings: &mut Vec<DgStyleWarning>,
) -> Result<Option<DgFontFace>, DgCssParseError> {
    let mut family = None;
    let mut sources = Vec::new();
    for property in &rule.properties {
        match property {
            FontFaceProperty::FontFamily(value) => {
                family = Some(font_face_family_to_string(value)?);
            }
            FontFaceProperty::Source(values) => {
                for source in values {
                    match source {
                        FontFaceSource::Url(url_source) => {
                            let format = url_source
                                .format
                                .as_ref()
                                .map(|format| {
                                    format
                                        .to_css_string(PrinterOptions::default())
                                        .map(|value| unquote(&value))
                                })
                                .transpose()
                                .map_err(|error| {
                                    DgCssParseError::new(format!(
                                        "failed to serialize @font-face format: {error}"
                                    ))
                                })?;
                            sources.push(DgFontFaceSource {
                                kind: DgFontFaceSourceKind::Url,
                                url: url_source.url.url.as_ref().to_string(),
                                format,
                            });
                        }
                        FontFaceSource::Local(local) => {
                            let family = font_face_family_to_string(local)?;
                            if family.trim().is_empty() {
                                continue;
                            }
                            sources.push(DgFontFaceSource {
                                kind: DgFontFaceSourceKind::Local,
                                url: family,
                                format: None,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let Some(family) = family.filter(|value| !value.trim().is_empty()) else {
        warnings.push(DgStyleWarning {
            property: "@font-face".to_string(),
            message: "@font-face rule is missing a supported font-family descriptor".to_string(),
        });
        return Ok(None);
    };
    if sources.is_empty() {
        warnings.push(DgStyleWarning {
            property: format!("@font-face {family}"),
            message: "@font-face rule is missing a supported local(...) or url(...) src descriptor"
                .to_string(),
        });
        return Ok(None);
    }
    Ok(Some(DgFontFace { family, sources }))
}

fn font_face_family_to_string(value: &CssFontFamily<'_>) -> Result<String, DgCssParseError> {
    let css = value
        .to_css_string(PrinterOptions::default())
        .map_err(|error| {
            DgCssParseError::new(format!("failed to serialize @font-face family: {error}"))
        })?;
    Ok(unquote(&css))
}

fn lower_keyframes_rule(
    rule: &KeyframesRule<'_>,
    variables: &BTreeMap<String, DgCssValue>,
    warnings: &mut Vec<DgStyleWarning>,
) -> Result<Option<DgKeyframes>, DgCssParseError> {
    let name = keyframes_name(rule.name.clone());
    let mut frames = Vec::new();
    for keyframe in &rule.keyframes {
        let declaration_specs = lower_declarations(
            &keyframe.declarations,
            variables,
            warnings,
            Some("@keyframes"),
        )?;
        let mut visual = VisualStyle::default();
        for (property, _) in declaration_specs {
            if let DgStyleProperty::Visual(declaration) = property {
                apply_visual_declaration(&mut visual, &declaration);
            }
        }
        if visual_style_is_empty(&visual) {
            continue;
        }
        for selector in &keyframe.selectors {
            if let Some(offset) = keyframe_selector_offset(selector) {
                frames.push(DgKeyframe {
                    offset,
                    visual: visual.clone(),
                });
            }
        }
    }
    if frames.is_empty() {
        warnings.push(DgStyleWarning {
            property: format!("@keyframes {name}"),
            message: "keyframes rule has no supported visual declarations".to_string(),
        });
        return Ok(None);
    }
    frames.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    Ok(Some(DgKeyframes { name, frames }))
}

fn keyframes_name(name: KeyframesName<'_>) -> String {
    match name {
        KeyframesName::Ident(ident) => ident.0.as_ref().to_string(),
        KeyframesName::Custom(name) => name.as_ref().to_string(),
    }
}

fn keyframe_selector_offset(selector: &KeyframeSelector) -> Option<f32> {
    match selector {
        KeyframeSelector::From => Some(0.0),
        KeyframeSelector::To => Some(1.0),
        KeyframeSelector::Percentage(value) => Some(value.0.clamp(0.0, 1.0)),
        KeyframeSelector::TimelineRangePercentage(_) => None,
    }
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
        SupportsCondition::Unknown(value) => supports_unknown_condition_matches(value.as_ref()),
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

fn supports_unknown_condition_matches(value: &str) -> bool {
    if let Some(argument) = supports_function_argument(value, "font-format") {
        let format = unquote(argument.trim()).to_ascii_lowercase();
        return matches!(
            format.as_str(),
            "truetype" | "ttf" | "opentype" | "otf" | "collection" | "ttc" | "woff"
        );
    }
    if let Some(argument) = supports_function_argument(value, "at-rule") {
        let at_rule = argument.trim().trim_start_matches('@').to_ascii_lowercase();
        return matches!(
            at_rule.as_str(),
            "media" | "supports" | "container" | "keyframes" | "font-face"
        );
    }
    if let Some(argument) = supports_function_argument(value, "font-tech") {
        let tech = unquote(argument.trim()).to_ascii_lowercase();
        return matches!(tech.as_str(), "features-opentype");
    }
    false
}

fn supports_function_argument<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let value = value.trim();
    let prefix_len = name.len();
    if value.len() <= prefix_len + 2
        || !value[..prefix_len].eq_ignore_ascii_case(name)
        || !value[prefix_len..].starts_with('(')
        || !value.ends_with(')')
    {
        return None;
    }
    Some(&value[prefix_len + 1..value.len() - 1])
}

fn container_rule_condition<R>(
    rule: &ContainerRule<'_, R>,
) -> Result<DgContainerRuleCondition, String> {
    let Some(condition) = &rule.condition else {
        return Err("unsupported @container rule without a size condition".to_string());
    };
    let expression = container_condition_expression(condition)?;
    let name = rule.name.as_ref().map(|name| name.0 .0.to_string());
    Ok(DgContainerRuleCondition { name, expression })
}

fn container_condition_expression(
    condition: &ContainerCondition<'_>,
) -> Result<DgContainerExpression, String> {
    match condition {
        ContainerCondition::Feature(feature) => container_feature_expression(feature),
        ContainerCondition::Not(condition) => Ok(DgContainerExpression::Not(Box::new(
            container_condition_expression(condition)?,
        ))),
        ContainerCondition::Operation {
            operator,
            conditions,
        } => {
            let expressions = conditions
                .iter()
                .map(container_condition_expression)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(match operator {
                Operator::And => DgContainerExpression::And(expressions),
                Operator::Or => DgContainerExpression::Or(expressions),
            })
        }
        ContainerCondition::Style(_)
        | ContainerCondition::ScrollState(_)
        | ContainerCondition::Unknown(_) => Err(
            "unsupported @container condition; only width and inline-size length queries are supported"
                .to_string(),
        ),
    }
}

fn container_feature_expression(
    feature: &ContainerSizeFeature<'_>,
) -> Result<DgContainerExpression, String> {
    match feature {
        QueryFeature::Plain { name, value } => container_width_constraint(
            name,
            DgMediaComparison::Equal,
            value,
            "unsupported @container value; only width and inline-size length values are supported",
        ),
        QueryFeature::Range {
            name,
            operator,
            value,
        } => container_width_constraint(
            name,
            media_comparison(*operator),
            value,
            "unsupported @container range; only width and inline-size length ranges are supported",
        ),
        QueryFeature::Interval {
            name,
            start,
            start_operator,
            end,
            end_operator,
        } => Ok(DgContainerExpression::And(vec![
            container_width_constraint(
                name,
                media_comparison_for_interval_start(*start_operator),
                start,
                "unsupported @container interval; only width and inline-size length intervals are supported",
            )?,
            container_width_constraint(
                name,
                media_comparison(*end_operator),
                end,
                "unsupported @container interval; only width and inline-size length intervals are supported",
            )?,
        ])),
        QueryFeature::Boolean { .. } => Err(
            "unsupported @container boolean feature; use width or inline-size length comparisons"
                .to_string(),
        ),
    }
}

fn container_width_constraint(
    name: &MediaFeatureName<'_, ContainerSizeFeatureId>,
    comparison: DgMediaComparison,
    value: &MediaFeatureValue<'_>,
    message: &str,
) -> Result<DgContainerExpression, String> {
    if !matches!(
        name,
        MediaFeatureName::Standard(ContainerSizeFeatureId::Width)
            | MediaFeatureName::Standard(ContainerSizeFeatureId::InlineSize)
    ) {
        return Err(message.to_string());
    }
    let Some(width) = media_feature_length_px(value) else {
        return Err(message.to_string());
    };
    Ok(DgContainerExpression::Width(comparison, width))
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
                "unsupported @media type {media_type:?}; only screen/all width, height, aspect-ratio, resolution, -webkit-device-pixel-ratio, -moz-device-pixel-ratio, device-width, device-height, device-aspect-ratio, horizontal-viewport-segments, vertical-viewport-segments, color, color-index, monochrome, color-gamut, video-color-gamut, orientation, pointer, hover, nav-controls, overflow-block, overflow-inline, scan, grid, update, environment-blending, scripting, forced-colors, prefers-contrast, inverted-colors, dynamic-range, video-dynamic-range, display-mode, prefers-reduced-motion, prefers-reduced-transparency, prefers-reduced-data, and prefers-color-scheme queries are supported"
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
            "unsupported @media condition; only width, height, aspect-ratio, resolution, -webkit-device-pixel-ratio, -moz-device-pixel-ratio, device-width, device-height, device-aspect-ratio, horizontal-viewport-segments, vertical-viewport-segments, color, color-index, monochrome, color-gamut, video-color-gamut, orientation, pointer, hover, nav-controls, overflow-block, overflow-inline, scan, grid, update, environment-blending, scripting, forced-colors, prefers-contrast, inverted-colors, dynamic-range, video-dynamic-range, display-mode, prefers-reduced-motion, prefers-reduced-transparency, prefers-reduced-data, and prefers-color-scheme queries are supported"
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
        QueryFeature::Boolean { name } => media_boolean_feature_expression(name),
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
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::ColorGamut)) {
        return media_color_gamut_expression(name, value, false);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::VideoColorGamut)
    ) {
        return media_color_gamut_expression(name, value, true);
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Pointer)) {
        return media_pointer_expression(name, value, false);
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::AnyPointer)) {
        return media_pointer_expression(name, value, true);
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Hover)) {
        return media_hover_expression(name, value, false);
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::AnyHover)) {
        return media_hover_expression(name, value, true);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::NavControls)
    ) {
        return media_nav_controls_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedMotion)
    ) {
        return media_reduced_motion_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedTransparency)
    ) {
        return media_reduced_transparency_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedData)
    ) {
        return media_reduced_data_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersColorScheme)
    ) {
        return media_color_scheme_expression(name, value);
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Update)) {
        return media_update_expression(name, value);
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Scripting)) {
        return media_scripting_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::ForcedColors)
    ) {
        return media_forced_colors_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersContrast)
    ) {
        return media_contrast_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::InvertedColors)
    ) {
        return media_inverted_colors_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::DynamicRange)
    ) {
        return media_dynamic_range_expression(name, value, false);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::VideoDynamicRange)
    ) {
        return media_dynamic_range_expression(name, value, true);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::DisplayMode)
    ) {
        return media_display_mode_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::OverflowBlock)
    ) {
        return media_overflow_expression(name, value, false);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::OverflowInline)
    ) {
        return media_overflow_expression(name, value, true);
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Scan)) {
        return media_scan_expression(name, value);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::EnvironmentBlending)
    ) {
        return media_environment_blending_expression(name, value);
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Grid)) {
        return media_grid_expression(name, value);
    }
    media_constraint_expression(name, DgMediaComparison::Equal, value)
}

fn media_boolean_feature_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
) -> Result<DgMediaExpression, String> {
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedMotion)
    ) {
        return Ok(DgMediaExpression::PrefersReducedMotion(true));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedTransparency)
    ) {
        return Ok(DgMediaExpression::PrefersReducedTransparency(true));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedData)
    ) {
        return Ok(DgMediaExpression::PrefersReducedData(true));
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Pointer)) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::Pointer(DgMediaPointer::None),
        )));
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::AnyPointer)) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::AnyPointer(DgMediaPointer::None),
        )));
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Hover)) {
        return Ok(DgMediaExpression::Hover(DgMediaHover::Hover));
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::AnyHover)) {
        return Ok(DgMediaExpression::AnyHover(DgMediaHover::Hover));
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Update)) {
        return Ok(DgMediaExpression::Not(Box::new(DgMediaExpression::Update(
            DgMediaUpdate::None,
        ))));
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Scripting)) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::Scripting(DgMediaScripting::None),
        )));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::ForcedColors)
    ) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::ForcedColors(DgMediaForcedColors::None),
        )));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::PrefersContrast)
    ) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::PrefersContrast(DgMediaContrast::NoPreference),
        )));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::InvertedColors)
    ) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::InvertedColors(DgMediaInvertedColors::None),
        )));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::DynamicRange)
    ) {
        return Ok(DgMediaExpression::DynamicRange(
            DgMediaDynamicRange::Standard,
        ));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::VideoDynamicRange)
    ) {
        return Ok(DgMediaExpression::VideoDynamicRange(
            DgMediaDynamicRange::Standard,
        ));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::DisplayMode)
    ) {
        return Ok(DgMediaExpression::Always);
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::OverflowBlock)
    ) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::OverflowBlock(DgMediaOverflow::None),
        )));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::OverflowInline)
    ) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::OverflowInline(DgMediaOverflow::None),
        )));
    }
    if let Some(feature) = media_integer_feature_name(name) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::Constraint(DgMediaConstraint {
                feature,
                comparison: DgMediaComparison::Equal,
                value: 0.0,
            }),
        )));
    }
    if matches!(name, MediaFeatureName::Standard(MediaFeatureId::Grid)) {
        return Ok(DgMediaExpression::Grid(true));
    }
    if matches!(
        name,
        MediaFeatureName::Standard(MediaFeatureId::NavControls)
    ) {
        return Ok(DgMediaExpression::Not(Box::new(
            DgMediaExpression::NavControls(DgMediaNavControls::None),
        )));
    }
    Err(format!(
        "unsupported @media feature {feature}; only width, height, aspect-ratio, resolution, -webkit-device-pixel-ratio, -moz-device-pixel-ratio, device-width, device-height, device-aspect-ratio, horizontal-viewport-segments, vertical-viewport-segments, color, color-index, monochrome, color-gamut, video-color-gamut, orientation, pointer, hover, nav-controls, overflow-block, overflow-inline, scan, grid, update, environment-blending, scripting, forced-colors, prefers-contrast, inverted-colors, dynamic-range, video-dynamic-range, display-mode, prefers-reduced-motion, prefers-reduced-transparency, prefers-reduced-data, and prefers-color-scheme queries are supported",
        feature = media_feature_name_label(name)
    ))
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

fn media_color_gamut_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
    video: bool,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only srgb, p3, and rec2020 are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let gamut = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("srgb") => DgMediaColorGamut::Srgb,
        value if value.eq_ignore_ascii_case("p3") => DgMediaColorGamut::P3,
        value if value.eq_ignore_ascii_case("rec2020") => DgMediaColorGamut::Rec2020,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only srgb, p3, and rec2020 are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(if video {
        DgMediaExpression::VideoColorGamut(gamut)
    } else {
        DgMediaExpression::ColorGamut(gamut)
    })
}

fn media_pointer_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
    any: bool,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only none, coarse, and fine are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let pointer = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("none") => DgMediaPointer::None,
        value if value.eq_ignore_ascii_case("coarse") => DgMediaPointer::Coarse,
        value if value.eq_ignore_ascii_case("fine") => DgMediaPointer::Fine,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only none, coarse, and fine are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(if any {
        DgMediaExpression::AnyPointer(pointer)
    } else {
        DgMediaExpression::Pointer(pointer)
    })
}

fn media_hover_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
    any: bool,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only none and hover are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let hover = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("none") => DgMediaHover::None,
        value if value.eq_ignore_ascii_case("hover") => DgMediaHover::Hover,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only none and hover are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(if any {
        DgMediaExpression::AnyHover(hover)
    } else {
        DgMediaExpression::Hover(hover)
    })
}

fn media_nav_controls_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only none and back are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let nav_controls = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("none") => DgMediaNavControls::None,
        value if value.eq_ignore_ascii_case("back") => DgMediaNavControls::Back,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only none and back are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::NavControls(nav_controls))
}

fn media_reduced_motion_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    Ok(DgMediaExpression::PrefersReducedMotion(
        media_reduced_preference_value(name, value)?,
    ))
}

fn media_reduced_transparency_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    Ok(DgMediaExpression::PrefersReducedTransparency(
        media_reduced_preference_value(name, value)?,
    ))
}

fn media_reduced_data_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    Ok(DgMediaExpression::PrefersReducedData(
        media_reduced_preference_value(name, value)?,
    ))
}

fn media_reduced_preference_value(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<bool, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only reduce and no-preference are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let reduce = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("reduce") => true,
        value if value.eq_ignore_ascii_case("no-preference") => false,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only reduce and no-preference are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(reduce)
}

fn media_color_scheme_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only dark and light are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let scheme = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("dark") => DgMediaColorScheme::Dark,
        value if value.eq_ignore_ascii_case("light") => DgMediaColorScheme::Light,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only dark and light are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::PrefersColorScheme(scheme))
}

fn media_update_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only none, slow, and fast are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let update = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("none") => DgMediaUpdate::None,
        value if value.eq_ignore_ascii_case("slow") => DgMediaUpdate::Slow,
        value if value.eq_ignore_ascii_case("fast") => DgMediaUpdate::Fast,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only none, slow, and fast are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::Update(update))
}

fn media_scripting_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only none, initial-only, and enabled are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let scripting = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("none") => DgMediaScripting::None,
        value if value.eq_ignore_ascii_case("initial-only") => DgMediaScripting::InitialOnly,
        value if value.eq_ignore_ascii_case("enabled") => DgMediaScripting::Enabled,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only none, initial-only, and enabled are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::Scripting(scripting))
}

fn media_forced_colors_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only none and active are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let forced_colors = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("none") => DgMediaForcedColors::None,
        value if value.eq_ignore_ascii_case("active") => DgMediaForcedColors::Active,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only none and active are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::ForcedColors(forced_colors))
}

fn media_contrast_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only no-preference, more, less, and custom are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let contrast = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("no-preference") => DgMediaContrast::NoPreference,
        value if value.eq_ignore_ascii_case("more") => DgMediaContrast::More,
        value if value.eq_ignore_ascii_case("less") => DgMediaContrast::Less,
        value if value.eq_ignore_ascii_case("custom") => DgMediaContrast::Custom,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only no-preference, more, less, and custom are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::PrefersContrast(contrast))
}

fn media_inverted_colors_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only none and inverted are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let inverted_colors = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("none") => DgMediaInvertedColors::None,
        value if value.eq_ignore_ascii_case("inverted") => DgMediaInvertedColors::Inverted,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only none and inverted are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::InvertedColors(inverted_colors))
}

fn media_dynamic_range_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
    video: bool,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only standard and high are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let dynamic_range = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("standard") => DgMediaDynamicRange::Standard,
        value if value.eq_ignore_ascii_case("high") => DgMediaDynamicRange::High,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only standard and high are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(if video {
        DgMediaExpression::VideoDynamicRange(dynamic_range)
    } else {
        DgMediaExpression::DynamicRange(dynamic_range)
    })
}

fn media_display_mode_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only browser, minimal-ui, standalone, fullscreen, window-controls-overlay, and picture-in-picture are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let display_mode = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("browser") => DgMediaDisplayMode::Browser,
        value if value.eq_ignore_ascii_case("minimal-ui") => DgMediaDisplayMode::MinimalUi,
        value if value.eq_ignore_ascii_case("standalone") => DgMediaDisplayMode::Standalone,
        value if value.eq_ignore_ascii_case("fullscreen") => DgMediaDisplayMode::Fullscreen,
        value if value.eq_ignore_ascii_case("window-controls-overlay") => {
            DgMediaDisplayMode::WindowControlsOverlay
        }
        value if value.eq_ignore_ascii_case("picture-in-picture") => {
            DgMediaDisplayMode::PictureInPicture
        }
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only browser, minimal-ui, standalone, fullscreen, window-controls-overlay, and picture-in-picture are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::DisplayMode(display_mode))
}

fn media_overflow_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
    inline: bool,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only {supported} are supported",
            feature = media_feature_name_label(name),
            supported = if inline {
                "none and scroll"
            } else {
                "none, scroll, optional-paged, and paged"
            }
        ));
    };
    let overflow = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("none") => DgMediaOverflow::None,
        value if value.eq_ignore_ascii_case("scroll") => DgMediaOverflow::Scroll,
        value if !inline && value.eq_ignore_ascii_case("optional-paged") => {
            DgMediaOverflow::OptionalPaged
        }
        value if !inline && value.eq_ignore_ascii_case("paged") => DgMediaOverflow::Paged,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only {supported} are supported",
                feature = media_feature_name_label(name),
                supported = if inline {
                    "none and scroll"
                } else {
                    "none, scroll, optional-paged, and paged"
                }
            ));
        }
    };
    Ok(if inline {
        DgMediaExpression::OverflowInline(overflow)
    } else {
        DgMediaExpression::OverflowBlock(overflow)
    })
}

fn media_scan_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only interlace and progressive are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let scan = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("interlace") => DgMediaScan::Interlace,
        value if value.eq_ignore_ascii_case("progressive") => DgMediaScan::Progressive,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only interlace and progressive are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::Scan(scan))
}

fn media_environment_blending_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let MediaFeatureValue::Ident(ident) = value else {
        return Err(format!(
            "unsupported @media value for {feature}; only opaque, additive, and subtractive are supported",
            feature = media_feature_name_label(name)
        ));
    };
    let environment_blending = match ident.as_ref() {
        value if value.eq_ignore_ascii_case("opaque") => DgMediaEnvironmentBlending::Opaque,
        value if value.eq_ignore_ascii_case("additive") => DgMediaEnvironmentBlending::Additive,
        value if value.eq_ignore_ascii_case("subtractive") => {
            DgMediaEnvironmentBlending::Subtractive
        }
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only opaque, additive, and subtractive are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::EnvironmentBlending(environment_blending))
}

fn media_grid_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let grid = match value {
        MediaFeatureValue::Boolean(value) => *value,
        MediaFeatureValue::Integer(0) => false,
        MediaFeatureValue::Integer(1) => true,
        _ => {
            return Err(format!(
                "unsupported @media value for {feature}; only 0 and 1 are supported",
                feature = media_feature_name_label(name)
            ));
        }
    };
    Ok(DgMediaExpression::Grid(grid))
}

fn media_constraint_expression(
    name: &MediaFeatureName<'_, MediaFeatureId>,
    comparison: DgMediaComparison,
    value: &MediaFeatureValue<'_>,
) -> Result<DgMediaExpression, String> {
    let feature = media_feature_name(name).ok_or_else(|| {
        format!(
        "unsupported @media feature {feature}; only width, height, aspect-ratio, resolution, -webkit-device-pixel-ratio, -moz-device-pixel-ratio, device-width, device-height, device-aspect-ratio, horizontal-viewport-segments, vertical-viewport-segments, color, color-index, monochrome, color-gamut, video-color-gamut, orientation, pointer, hover, nav-controls, overflow-block, overflow-inline, scan, grid, update, environment-blending, scripting, forced-colors, prefers-contrast, inverted-colors, dynamic-range, video-dynamic-range, display-mode, prefers-reduced-motion, prefers-reduced-transparency, prefers-reduced-data, and prefers-color-scheme queries are supported",
            feature = media_feature_name_label(name)
        )
    })?;
    let value = media_feature_constraint_value(feature, value).ok_or_else(|| {
        format!(
            "unsupported @media value for {feature}; only absolute length, integer, aspect-ratio ratio, and resolution values are supported",
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
        MediaFeatureName::Standard(MediaFeatureId::AspectRatio) => {
            Some(DgMediaFeature::AspectRatio)
        }
        MediaFeatureName::Standard(MediaFeatureId::Resolution) => Some(DgMediaFeature::Resolution),
        MediaFeatureName::Standard(MediaFeatureId::WebKitDevicePixelRatio)
        | MediaFeatureName::Standard(MediaFeatureId::MozDevicePixelRatio) => {
            Some(DgMediaFeature::DevicePixelRatio)
        }
        MediaFeatureName::Standard(MediaFeatureId::DeviceWidth) => {
            Some(DgMediaFeature::DeviceWidth)
        }
        MediaFeatureName::Standard(MediaFeatureId::DeviceHeight) => {
            Some(DgMediaFeature::DeviceHeight)
        }
        MediaFeatureName::Standard(MediaFeatureId::DeviceAspectRatio) => {
            Some(DgMediaFeature::DeviceAspectRatio)
        }
        MediaFeatureName::Standard(MediaFeatureId::Color) => Some(DgMediaFeature::Color),
        MediaFeatureName::Standard(MediaFeatureId::ColorIndex) => Some(DgMediaFeature::ColorIndex),
        MediaFeatureName::Standard(MediaFeatureId::Monochrome) => Some(DgMediaFeature::Monochrome),
        MediaFeatureName::Standard(MediaFeatureId::HorizontalViewportSegments) => {
            Some(DgMediaFeature::HorizontalViewportSegments)
        }
        MediaFeatureName::Standard(MediaFeatureId::VerticalViewportSegments) => {
            Some(DgMediaFeature::VerticalViewportSegments)
        }
        _ => None,
    }
}

fn media_integer_feature_name(
    name: &MediaFeatureName<'_, MediaFeatureId>,
) -> Option<DgMediaFeature> {
    match name {
        MediaFeatureName::Standard(MediaFeatureId::Color) => Some(DgMediaFeature::Color),
        MediaFeatureName::Standard(MediaFeatureId::ColorIndex) => Some(DgMediaFeature::ColorIndex),
        MediaFeatureName::Standard(MediaFeatureId::Monochrome) => Some(DgMediaFeature::Monochrome),
        MediaFeatureName::Standard(MediaFeatureId::HorizontalViewportSegments) => {
            Some(DgMediaFeature::HorizontalViewportSegments)
        }
        MediaFeatureName::Standard(MediaFeatureId::VerticalViewportSegments) => {
            Some(DgMediaFeature::VerticalViewportSegments)
        }
        _ => None,
    }
}

fn media_feature_name_label(name: &MediaFeatureName<'_, MediaFeatureId>) -> String {
    match name {
        MediaFeatureName::Standard(MediaFeatureId::Width) => "width".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Height) => "height".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::AspectRatio) => "aspect-ratio".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Resolution) => "resolution".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::WebKitDevicePixelRatio) => {
            "-webkit-device-pixel-ratio".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::MozDevicePixelRatio) => {
            "-moz-device-pixel-ratio".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::DeviceWidth) => "device-width".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::DeviceHeight) => "device-height".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::DeviceAspectRatio) => {
            "device-aspect-ratio".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::Color) => "color".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::ColorIndex) => "color-index".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Monochrome) => "monochrome".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::HorizontalViewportSegments) => {
            "horizontal-viewport-segments".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::VerticalViewportSegments) => {
            "vertical-viewport-segments".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::ColorGamut) => "color-gamut".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::VideoColorGamut) => {
            "video-color-gamut".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::Orientation) => "orientation".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Scan) => "scan".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Grid) => "grid".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::EnvironmentBlending) => {
            "environment-blending".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::Pointer) => "pointer".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::AnyPointer) => "any-pointer".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Hover) => "hover".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::AnyHover) => "any-hover".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::NavControls) => "nav-controls".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Update) => "update".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::Scripting) => "scripting".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::ForcedColors) => "forced-colors".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::PrefersContrast) => {
            "prefers-contrast".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::InvertedColors) => "inverted-colors".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::DynamicRange) => "dynamic-range".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::VideoDynamicRange) => {
            "video-dynamic-range".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::DisplayMode) => "display-mode".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::OverflowBlock) => "overflow-block".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::OverflowInline) => "overflow-inline".to_string(),
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedMotion) => {
            "prefers-reduced-motion".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedTransparency) => {
            "prefers-reduced-transparency".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::PrefersReducedData) => {
            "prefers-reduced-data".to_string()
        }
        MediaFeatureName::Standard(MediaFeatureId::PrefersColorScheme) => {
            "prefers-color-scheme".to_string()
        }
        other => format!("{other:?}"),
    }
}

fn media_feature_constraint_value(
    feature: DgMediaFeature,
    value: &MediaFeatureValue<'_>,
) -> Option<f32> {
    match feature {
        DgMediaFeature::Width
        | DgMediaFeature::Height
        | DgMediaFeature::DeviceWidth
        | DgMediaFeature::DeviceHeight => media_feature_length_px(value),
        DgMediaFeature::AspectRatio | DgMediaFeature::DeviceAspectRatio => {
            media_feature_ratio(value)
        }
        DgMediaFeature::Resolution => media_feature_resolution_dppx(value),
        DgMediaFeature::DevicePixelRatio => media_feature_number(value),
        DgMediaFeature::Color
        | DgMediaFeature::ColorIndex
        | DgMediaFeature::Monochrome
        | DgMediaFeature::HorizontalViewportSegments
        | DgMediaFeature::VerticalViewportSegments => media_feature_integer(value),
    }
}

fn media_feature_integer(value: &MediaFeatureValue<'_>) -> Option<f32> {
    match value {
        MediaFeatureValue::Integer(value) if *value >= 0 => Some(*value as f32),
        _ => None,
    }
}

fn media_feature_number(value: &MediaFeatureValue<'_>) -> Option<f32> {
    match value {
        MediaFeatureValue::Number(value) if value.is_finite() && *value >= 0.0 => Some(*value),
        _ => None,
    }
}

fn media_feature_length_px(value: &MediaFeatureValue<'_>) -> Option<f32> {
    match value {
        MediaFeatureValue::Length(length) => length.to_px(),
        _ => None,
    }
}

fn media_feature_ratio(value: &MediaFeatureValue<'_>) -> Option<f32> {
    match value {
        MediaFeatureValue::Ratio(ratio)
            if ratio.0.is_finite() && ratio.1.is_finite() && ratio.1 > 0.0 =>
        {
            Some(ratio.0 / ratio.1)
        }
        _ => None,
    }
}

fn media_feature_resolution_dppx(value: &MediaFeatureValue<'_>) -> Option<f32> {
    match value {
        MediaFeatureValue::Resolution(CssResolution::Dppx(value)) if value.is_finite() => {
            Some(*value)
        }
        MediaFeatureValue::Resolution(CssResolution::Dpi(value)) if value.is_finite() => {
            Some(*value / 96.0)
        }
        MediaFeatureValue::Resolution(CssResolution::Dpcm(value)) if value.is_finite() => {
            Some(*value * 2.54 / 96.0)
        }
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
    collect_immediate_root_variables(&sheet.rules, variables, warnings)
}

fn collect_immediate_root_variables<R>(
    rules_list: &CssRuleList<'_, R>,
    variables: &mut BTreeMap<String, DgCssValue>,
    warnings: &mut Vec<DgStyleWarning>,
) -> Result<(), DgCssParseError> {
    for rule in rules_list.0.iter() {
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
    let mut scoped_variables = variables.clone();
    for (declaration, important) in block.iter() {
        let declaration_text = declaration_to_css(declaration, important)?;
        let Some((name, value)) = split_declaration(&declaration_text) else {
            continue;
        };
        if !name.starts_with("--") {
            continue;
        }
        if let Some(value) = parse_css_value(value, &scoped_variables) {
            scoped_variables.insert(name.to_string(), value);
        } else {
            let mut warning = DgStyleWarning {
                property: name.to_string(),
                message: format!("could not parse custom property value {value:?}"),
            };
            if let Some(selector) = selector {
                warning.message = format!("{} in selector {selector:?}", warning.message);
            }
            warnings.push(warning);
        }
    }

    let mut declarations = Vec::new();
    for (declaration, important) in block.iter() {
        let declaration_text = declaration_to_css(declaration, important)?;
        let Some((name, value)) = split_declaration(&declaration_text) else {
            continue;
        };
        if name.starts_with("--") {
            continue;
        }
        match lower_declaration(name, value, &scoped_variables) {
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
        DgStylePropertyName::Animation(property) => {
            lower_animation(name, property, value, variables)
        }
        DgStylePropertyName::Generated(property) => {
            lower_generated(name, property, value, variables)
        }
    }
}

fn lower_generated(
    name: &str,
    property: DgGeneratedPropertyName,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgStyleProperty>, DgStyleWarning> {
    let declaration = match property {
        DgGeneratedPropertyName::Content => {
            let value = resolve_keyword(value, variables);
            let trimmed = value.trim();
            if trimmed.eq_ignore_ascii_case("none") || trimmed.eq_ignore_ascii_case("normal") {
                DgGeneratedDeclaration::Content(None)
            } else if let Some(attr) = parse_generated_attr(trimmed) {
                DgGeneratedDeclaration::Content(Some(GeneratedContent::Attr(attr)))
            } else if is_quoted(trimmed) {
                DgGeneratedDeclaration::Content(Some(GeneratedContent::Text(unquote(trimmed))))
            } else {
                return Err(parse_warning(
                    name,
                    value.as_str(),
                    "quoted string, attr(name), none, or normal",
                ));
            }
        }
    };
    Ok(Some(DgStyleProperty::Generated(declaration)))
}

fn apply_animation_declaration(style: &mut AnimationStyle, declaration: &DgAnimationDeclaration) {
    match declaration {
        DgAnimationDeclaration::Shorthand(value) => *style = value.clone(),
        DgAnimationDeclaration::Name(value) => style.name = value.clone(),
        DgAnimationDeclaration::Duration(value) => style.duration_ms = Some(*value),
        DgAnimationDeclaration::Delay(value) => style.delay_ms = Some(*value),
        DgAnimationDeclaration::TimingFunction(value) => style.timing_function = Some(*value),
        DgAnimationDeclaration::IterationCount(value) => style.iteration_count = Some(*value),
        DgAnimationDeclaration::Direction(value) => style.direction = Some(*value),
        DgAnimationDeclaration::FillMode(value) => style.fill_mode = Some(*value),
        DgAnimationDeclaration::PlayState(value) => style.play_state = Some(*value),
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
        DgLayoutPropertyName::FlexBasis => {
            DgLayoutDeclaration::FlexBasis(parse_layout_length_value(name, value, variables)?)
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
            DgLayoutDeclaration::Margin(parse_layout_box_edges(name, value, variables)?)
        }
        DgLayoutPropertyName::MarginLeft => {
            DgLayoutDeclaration::MarginLeft(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MarginRight => {
            DgLayoutDeclaration::MarginRight(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MarginTop => {
            DgLayoutDeclaration::MarginTop(parse_layout_length_value(name, value, variables)?)
        }
        DgLayoutPropertyName::MarginBottom => {
            DgLayoutDeclaration::MarginBottom(parse_layout_length_value(name, value, variables)?)
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
        DgLayoutPropertyName::GridTemplateAreas => DgLayoutDeclaration::GridTemplateAreas(
            parse_grid_template_areas_value(name, value, variables)?,
        ),
        DgLayoutPropertyName::GridAutoFlow => {
            DgLayoutDeclaration::GridAutoFlow(parse_grid_auto_flow_value(name, value, variables)?)
        }
        DgLayoutPropertyName::GridArea => {
            DgLayoutDeclaration::GridArea(parse_grid_area_value(name, value, variables)?)
        }
        DgLayoutPropertyName::GridColumn => {
            DgLayoutDeclaration::GridColumn(parse_grid_placement_value(name, value)?)
        }
        DgLayoutPropertyName::GridRow => {
            DgLayoutDeclaration::GridRow(parse_grid_placement_value(name, value)?)
        }
        DgLayoutPropertyName::ContainerName => {
            DgLayoutDeclaration::ContainerName(parse_container_name_value(name, value, variables)?)
        }
        DgLayoutPropertyName::ContainerType => {
            DgLayoutDeclaration::ContainerType(parse_container_type_value(name, value, variables)?)
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
        DgVisualPropertyName::BackgroundImage => DgVisualDeclaration::BackgroundImage(
            parse_background_image_paint_value(name, value, variables)?,
        ),
        DgVisualPropertyName::BackdropFilter => DgVisualDeclaration::BackdropFilter(
            parse_backdrop_filter_value(name, value, variables)?,
        ),
        DgVisualPropertyName::Foreground => {
            DgVisualDeclaration::Foreground(parse_color_value(name, value, variables)?)
        }
        DgVisualPropertyName::BorderColor => {
            DgVisualDeclaration::BorderColor(parse_color_value(name, value, variables)?)
        }
        DgVisualPropertyName::BorderWidth => {
            DgVisualDeclaration::BorderWidth(parse_px_length_value(name, value, variables)?)
        }
        DgVisualPropertyName::BorderStyle => {
            DgVisualDeclaration::BorderStyle(parse_border_style_value(name, value, variables)?)
        }
        DgVisualPropertyName::OutlineColor => {
            DgVisualDeclaration::OutlineColor(parse_color_value(name, value, variables)?)
        }
        DgVisualPropertyName::OutlineWidth => {
            DgVisualDeclaration::OutlineWidth(parse_px_length_value(name, value, variables)?)
        }
        DgVisualPropertyName::OutlineStyle => {
            DgVisualDeclaration::OutlineStyle(parse_border_style_value(name, value, variables)?)
        }
        DgVisualPropertyName::OutlineOffset => {
            DgVisualDeclaration::OutlineOffset(parse_px_length_value(name, value, variables)?)
        }
        DgVisualPropertyName::Outline => DgVisualDeclaration::Outline(
            parse_border(value, variables)
                .ok_or_else(|| parse_warning(name, value, "<width> solid <color> or none"))?,
        ),
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
        DgVisualPropertyName::GradientInterpolation => DgVisualDeclaration::GradientInterpolation(
            parse_gradient_interpolation_value(name, value, variables)?,
        ),
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
        DgWidgetPropertyName::TextAreaRows => {
            DgWidgetDeclaration::TextAreaRows(parse_number_value(name, value, variables)?)
        }
        DgWidgetPropertyName::ScatterPointSize => {
            DgWidgetDeclaration::ScatterPointSize(parse_px_length_value(name, value, variables)?)
        }
        DgWidgetPropertyName::ScatterPointStyle => DgWidgetDeclaration::ScatterPointStyle(
            parse_scatter_point_style_value(name, value, variables)?,
        ),
        DgWidgetPropertyName::ScatterGridVisible => DgWidgetDeclaration::ScatterGridVisible(
            parse_bool_keyword_value(name, value, variables)?,
        ),
        DgWidgetPropertyName::ScatterGridPlanes => {
            let (major, minor) = parse_scatter_grid_planes_value(name, value, variables)?;
            DgWidgetDeclaration::ScatterGridPlanes(major, minor)
        }
        DgWidgetPropertyName::ScatterLegendPosition => DgWidgetDeclaration::ScatterLegendPosition(
            parse_scatter_legend_position_value(name, value, variables)?,
        ),
        DgWidgetPropertyName::ScatterOrientationAxes => {
            DgWidgetDeclaration::ScatterOrientationAxes(parse_bool_keyword_value(
                name, value, variables,
            )?)
        }
        DgWidgetPropertyName::TableRowHeight => {
            DgWidgetDeclaration::TableRowHeight(parse_px_length_value(name, value, variables)?)
        }
        DgWidgetPropertyName::TableHeaderHeight => {
            DgWidgetDeclaration::TableHeaderHeight(parse_px_length_value(name, value, variables)?)
        }
        DgWidgetPropertyName::TableColumnWidth => {
            DgWidgetDeclaration::TableColumnWidth(parse_px_length_value(name, value, variables)?)
        }
        DgWidgetPropertyName::TableIndexWidth => {
            DgWidgetDeclaration::TableIndexWidth(parse_px_length_value(name, value, variables)?)
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

fn lower_animation(
    name: &str,
    property: DgAnimationPropertyName,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgStyleProperty>, DgStyleWarning> {
    let declaration = match property {
        DgAnimationPropertyName::Animation => {
            DgAnimationDeclaration::Shorthand(parse_animation_shorthand(name, value, variables)?)
        }
        DgAnimationPropertyName::Name => {
            let name = first_animation_list_value(value, variables);
            let name = name.trim();
            if name.eq_ignore_ascii_case("none") {
                DgAnimationDeclaration::Name(None)
            } else if name.is_empty() {
                return Err(parse_warning("animation-name", value, "animation name"));
            } else {
                DgAnimationDeclaration::Name(Some(unquote(name)))
            }
        }
        DgAnimationPropertyName::Duration => {
            let value = first_animation_list_value(value, variables);
            DgAnimationDeclaration::Duration(parse_time_ms_value(name, &value, variables)?)
        }
        DgAnimationPropertyName::Delay => {
            let value = first_animation_list_value(value, variables);
            DgAnimationDeclaration::Delay(parse_signed_time_ms_value(name, &value, variables)?)
        }
        DgAnimationPropertyName::TimingFunction => {
            let keyword = first_animation_list_value(value, variables);
            let timing = transition_timing_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "animation timing function"))?;
            DgAnimationDeclaration::TimingFunction(timing)
        }
        DgAnimationPropertyName::IterationCount => {
            DgAnimationDeclaration::IterationCount(parse_animation_iteration_count_value(
                name,
                &first_animation_list_value(value, variables),
                variables,
            )?)
        }
        DgAnimationPropertyName::Direction => {
            let keyword = first_animation_list_value(value, variables);
            let direction = animation_direction_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "animation direction"))?;
            DgAnimationDeclaration::Direction(direction)
        }
        DgAnimationPropertyName::FillMode => {
            let keyword = first_animation_list_value(value, variables);
            let fill_mode = animation_fill_mode_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "animation fill mode"))?;
            DgAnimationDeclaration::FillMode(fill_mode)
        }
        DgAnimationPropertyName::PlayState => {
            let keyword = first_animation_list_value(value, variables);
            let play_state = animation_play_state_from_keyword(&keyword)
                .ok_or_else(|| parse_warning(name, value, "animation play state"))?;
            DgAnimationDeclaration::PlayState(play_state)
        }
    };
    Ok(Some(DgStyleProperty::Animation(declaration)))
}

fn first_animation_list_value(value: &str, variables: &BTreeMap<String, DgCssValue>) -> String {
    let value = resolve_keyword(value, variables);
    split_selector_list(&value)
        .into_iter()
        .next()
        .unwrap_or_else(|| value.clone())
        .trim()
        .to_string()
}

fn parse_animation_shorthand(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<AnimationStyle, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    let first = split_selector_list(&value)
        .into_iter()
        .next()
        .unwrap_or_else(|| value.clone())
        .trim()
        .to_string();
    if first.eq_ignore_ascii_case("none") {
        return Ok(AnimationStyle {
            name: None,
            duration_ms: Some(0),
            delay_ms: Some(0),
            timing_function: Some(TransitionTimingFunction::Ease),
            iteration_count: Some(AnimationIterationCount::One),
            direction: Some(AnimationDirection::Normal),
            fill_mode: Some(AnimationFillMode::None),
            play_state: Some(AnimationPlayState::Running),
        });
    }

    let mut style = AnimationStyle {
        name: None,
        duration_ms: Some(0),
        delay_ms: Some(0),
        timing_function: Some(TransitionTimingFunction::Ease),
        iteration_count: Some(AnimationIterationCount::One),
        direction: Some(AnimationDirection::Normal),
        fill_mode: Some(AnimationFillMode::None),
        play_state: Some(AnimationPlayState::Running),
    };
    let mut saw_duration = false;
    let mut saw_name = false;
    let tokens = split_css_whitespace_tokens(&first)
        .ok_or_else(|| parse_warning(name, value.as_str(), "animation shorthand"))?;
    for token in tokens {
        if let Some(timing) = transition_timing_from_keyword(&token) {
            style.timing_function = Some(timing);
            continue;
        }
        if let Some(time) = parse_time_ms(&token) {
            if !saw_duration {
                style.duration_ms = Some(time);
                saw_duration = true;
                continue;
            }
        }
        if saw_duration {
            if let Some(delay) = parse_signed_time_ms(&token) {
                style.delay_ms = Some(delay);
                continue;
            }
        } else if is_signed_time_token(&token) {
            return Err(parse_warning(name, value.as_str(), "animation duration"));
        }
        if parse_time_ms(&token).is_some() {
            continue;
        }
        if let Ok(count) = parse_animation_iteration_count_value(name, &token, variables) {
            style.iteration_count = Some(count);
            continue;
        }
        if let Some(direction) = animation_direction_from_keyword(&token) {
            style.direction = Some(direction);
            continue;
        }
        if let Some(fill_mode) = animation_fill_mode_from_keyword(&token) {
            style.fill_mode = Some(fill_mode);
            continue;
        }
        if let Some(play_state) = animation_play_state_from_keyword(&token) {
            style.play_state = Some(play_state);
            continue;
        }

        let token_lower = token.trim().to_ascii_lowercase();
        if token_lower == "auto"
            || token_lower.starts_with("scroll(")
            || token_lower.starts_with("view(")
        {
            return Err(DgStyleWarning {
                property: name.to_string(),
                message:
                    "animation shorthand supports name, duration, timing, delay, iteration count, direction, fill mode, and play state only"
                        .to_string(),
            });
        }
        if saw_name {
            return Err(parse_warning(
                name,
                value.as_str(),
                "single animation shorthand",
            ));
        }
        let animation_name = unquote(token.trim());
        if animation_name.is_empty() {
            return Err(parse_warning(name, value.as_str(), "animation name"));
        }
        style.name = Some(animation_name);
        saw_name = true;
    }
    Ok(style)
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

fn parse_signed_time_ms_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<i64, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    let first = value.split(',').next().unwrap_or(value.as_str()).trim();
    parse_signed_time_ms(first).ok_or_else(|| parse_warning(name, value.as_str(), "time"))
}

fn parse_time_ms(value: &str) -> Option<u64> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(ms) = value.strip_suffix("ms") {
        return ms
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| value.round() as u64);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| (value * 1000.0).round() as u64);
    }
    None
}

fn parse_signed_time_ms(value: &str) -> Option<i64> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(ms) = value.strip_suffix("ms") {
        return ms
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| value.round() as i64);
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| (value * 1000.0).round() as i64);
    }
    None
}

fn is_signed_time_token(value: &str) -> bool {
    let value = value.trim();
    value.starts_with('-') && parse_signed_time_ms(value).is_some()
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
        "outline" | "outline-style" => Some(TransitionProperty::Outline),
        "outline-color" => Some(TransitionProperty::OutlineColor),
        "outline-width" => Some(TransitionProperty::OutlineWidth),
        "outline-offset" => Some(TransitionProperty::OutlineOffset),
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

fn parse_animation_iteration_count_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<AnimationIterationCount, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    let value = value.trim();
    if value.eq_ignore_ascii_case("infinite") {
        return Ok(AnimationIterationCount::Infinite);
    }
    let count = value
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| parse_warning(name, value, "animation iteration count"))?;
    Ok(AnimationIterationCount::Count(count))
}

fn animation_direction_from_keyword(value: &str) -> Option<AnimationDirection> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(AnimationDirection::Normal),
        "reverse" => Some(AnimationDirection::Reverse),
        "alternate" => Some(AnimationDirection::Alternate),
        "alternate-reverse" => Some(AnimationDirection::AlternateReverse),
        _ => None,
    }
}

fn animation_fill_mode_from_keyword(value: &str) -> Option<AnimationFillMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Some(AnimationFillMode::None),
        "forwards" => Some(AnimationFillMode::Forwards),
        "backwards" => Some(AnimationFillMode::Backwards),
        "both" => Some(AnimationFillMode::Both),
        _ => None,
    }
}

fn animation_play_state_from_keyword(value: &str) -> Option<AnimationPlayState> {
    match value.trim().to_ascii_lowercase().as_str() {
        "running" => Some(AnimationPlayState::Running),
        "paused" => Some(AnimationPlayState::Paused),
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

fn parse_backdrop_filter_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<BackdropFilterStyle>, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let filter = parse_backdrop_filter_functions(name, trimmed, variables)
        .ok_or_else(|| parse_warning(name, value.as_str(), "backdrop filter"))?;
    Ok(Some(filter))
}

fn parse_backdrop_filter_functions(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Option<BackdropFilterStyle> {
    let mut rest = value;
    let mut filter = BackdropFilterStyle::default();
    let mut parsed_any = false;
    while !rest.trim().is_empty() {
        rest = rest.trim_start();
        let open = rest.find('(')?;
        let function_name = rest[..open].trim().to_ascii_lowercase();
        let after_open = &rest[open + 1..];
        let close = find_closing_paren(after_open)?;
        let args = after_open[..close].trim();
        match function_name.as_str() {
            "blur" => {
                let blur = length_px(&parse_px_length_value(name, args, variables).ok()?)
                    .unwrap_or(0.0)
                    .max(0.0);
                filter.blur += blur;
            }
            "brightness" => {
                filter.brightness *= parse_filter_factor_value(args)?;
            }
            "saturate" => {
                filter.saturate *= parse_filter_factor_value(args)?;
            }
            _ => return None,
        }
        parsed_any = true;
        rest = &after_open[close + 1..];
    }
    parsed_any.then_some(filter)
}

fn find_closing_paren(value: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
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
            ')' if depth == 0 => return Some(idx),
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn parse_filter_factor_value(value: &str) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    let factor = if let Some(percent) = value.strip_suffix('%') {
        percent.trim().parse::<f32>().ok()? / 100.0
    } else {
        value.parse::<f32>().ok()?
    };
    factor.is_finite().then_some(factor.max(0.0))
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
    parse_selector_with_options(selector, warnings, false)
}

fn parse_has_argument_selector(
    selector: &str,
    warnings: &mut Vec<DgStyleWarning>,
) -> Option<DgSelector> {
    parse_selector_with_options(selector, warnings, true)
}

fn parse_selector_with_options(
    selector: &str,
    warnings: &mut Vec<DgStyleWarning>,
    allow_has_in_chain_ancestors: bool,
) -> Option<DgSelector> {
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
                || compound.contains_state_pseudo()
                || (compound.contains_has_function() && !allow_has_in_chain_ancestors)
                || (allow_has_in_chain_ancestors && compound.contains_has_sibling_relation()))
        {
            warnings.push(DgStyleWarning {
                property: selector.to_string(),
                message: if allow_has_in_chain_ancestors && compound.contains_has_sibling_relation()
                {
                    "sibling-relative :has() is only supported on target selectors".to_string()
                } else {
                    "pseudo selectors are only supported on the target widget".to_string()
                },
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
    if matches!(value, "before" | "after") && compound.part.is_none() {
        compound.part = Some(value.to_string());
        return Some(());
    }
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
    } else if let Some(inner) = value.strip_prefix("has(") {
        (DgSelectorFunctionKind::Has, inner.strip_suffix(')')?)
    } else {
        return None;
    };
    let selectors = split_selector_list(inner)
        .into_iter()
        .map(|selector| parse_selector_function_argument(kind, &selector))
        .collect::<Option<Vec<_>>>()?;
    if selectors.is_empty()
        || selectors.iter().any(|selector| {
            matches!(selector.selector, DgSelector::Root)
                || selector.selector.target_part().is_some()
        })
    {
        return None;
    }
    if kind == DgSelectorFunctionKind::Has
        && selectors.iter().any(|selector| {
            !selector
                .selector
                .target_state_pseudos_are_snapshot_matchable()
        })
    {
        return None;
    }
    Some(DgSelectorFunction { kind, selectors })
}

fn parse_selector_function_argument(
    kind: DgSelectorFunctionKind,
    selector: &str,
) -> Option<DgSelectorFunctionArgument> {
    if kind == DgSelectorFunctionKind::Has {
        let selector = selector.trim();
        let (relation, selector) = if let Some(selector) = selector.strip_prefix('>') {
            (DgSelectorFunctionRelation::Child, selector.trim())
        } else if let Some(selector) = selector.strip_prefix('+') {
            (DgSelectorFunctionRelation::NextSibling, selector.trim())
        } else if let Some(selector) = selector.strip_prefix('~') {
            (
                DgSelectorFunctionRelation::SubsequentSibling,
                selector.trim(),
            )
        } else {
            (DgSelectorFunctionRelation::Descendant, selector)
        };
        if selector.is_empty() {
            return None;
        }
        let mut warnings = Vec::new();
        let selector = parse_has_argument_selector(selector, &mut warnings)?;
        return Some(match relation {
            DgSelectorFunctionRelation::Descendant => DgSelectorFunctionArgument::new(selector),
            DgSelectorFunctionRelation::Child => DgSelectorFunctionArgument::direct_child(selector),
            DgSelectorFunctionRelation::NextSibling => {
                DgSelectorFunctionArgument::next_sibling(selector)
            }
            DgSelectorFunctionRelation::SubsequentSibling => {
                DgSelectorFunctionArgument::subsequent_sibling(selector)
            }
        });
    }

    parse_compound_selector(selector).map(DgSelectorFunctionArgument::compound)
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
        "only-child" => Some(DgStructuralPseudo::OnlyChild),
        "empty" => Some(DgStructuralPseudo::Empty),
        _ => {
            if let Some(inner) = value
                .strip_prefix("nth-last-child(")
                .and_then(|value| value.strip_suffix(')'))
                .map(str::trim)
            {
                return parse_nth_child(inner).map(DgStructuralPseudo::NthLastChild);
            }
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
        && selector.target_state_pseudos_are_snapshot_matchable()
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

fn parse_scatter_point_style_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssKeyword, DgStyleWarning> {
    let kw = DgCssKeyword(resolve_keyword(value, variables));
    if matches!(kw.0.as_str(), "circle" | "square" | "gaussian") {
        Ok(kw)
    } else {
        Err(parse_warning(
            name,
            value,
            "scatter-point-style (circle | square | gaussian)",
        ))
    }
}

fn parse_bool_keyword_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<bool, DgStyleWarning> {
    match resolve_keyword(value, variables).as_str() {
        "true" | "yes" | "on" => Ok(true),
        "false" | "no" | "off" => Ok(false),
        _ => Err(parse_warning(name, value, "true | false")),
    }
}

fn parse_scatter_grid_planes_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<(bool, bool), DgStyleWarning> {
    match resolve_keyword(value, variables).as_str() {
        "none" => Ok((false, false)),
        "major" => Ok((true, false)),
        "minor" => Ok((false, true)),
        "all" | "both" => Ok((true, true)),
        _ => Err(parse_warning(
            name,
            value,
            "scatter-grid-planes (none | major | minor | all)",
        )),
    }
}

fn parse_scatter_legend_position_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgCssKeyword, DgStyleWarning> {
    let kw = DgCssKeyword(resolve_keyword(value, variables));
    if matches!(
        kw.0.as_str(),
        "top-right" | "top-left" | "bottom-right" | "bottom-left"
    ) {
        Ok(kw)
    } else {
        Err(parse_warning(
            name,
            value,
            "scatter-legend-position (top-right | top-left | bottom-right | bottom-left)",
        ))
    }
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

fn parse_grid_template_areas_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgGridTemplateAreas, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(DgGridTemplateAreas {
            columns: 0,
            rows: 0,
            areas: Vec::new(),
        });
    }
    let rows = parse_grid_area_rows(&value)
        .ok_or_else(|| parse_warning(name, &value, "quoted grid area rows"))?;
    if rows.is_empty() {
        return Err(parse_warning(name, &value, "quoted grid area rows"));
    }
    let first_columns = rows[0].len();
    if first_columns == 0 || first_columns > u16::MAX as usize || rows.len() > u16::MAX as usize {
        return Err(parse_warning(name, &value, "non-empty grid area rows"));
    }
    if rows.iter().any(|row| row.len() != first_columns) {
        return Err(parse_warning(name, &value, "equal-width grid area rows"));
    }

    let mut bounds: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    for (row_index, row) in rows.iter().enumerate() {
        for (column_index, cell) in row.iter().enumerate() {
            if is_grid_null_cell(cell) {
                continue;
            }
            if !is_grid_area_name(cell) {
                return Err(parse_warning(name, cell, "grid area name"));
            }
            bounds
                .entry(cell.clone())
                .and_modify(|(_, row_end, column_start, column_end)| {
                    *row_end = row_index + 1;
                    *column_start = (*column_start).min(column_index);
                    *column_end = (*column_end).max(column_index + 1);
                })
                .or_insert((row_index, row_index + 1, column_index, column_index + 1));
        }
    }

    let mut areas = Vec::with_capacity(bounds.len());
    for (area_name, (row_start, row_end, column_start, column_end)) in bounds {
        for row in rows.iter().take(row_end).skip(row_start) {
            for cell in row.iter().take(column_end).skip(column_start) {
                if cell != &area_name {
                    return Err(parse_warning(name, &area_name, "rectangular grid area"));
                }
            }
        }
        areas.push(DgGridTemplateArea {
            name: area_name,
            row_start: (row_start + 1) as u16,
            row_end: (row_end + 1) as u16,
            column_start: (column_start + 1) as u16,
            column_end: (column_end + 1) as u16,
        });
    }

    Ok(DgGridTemplateAreas {
        columns: first_columns as u16,
        rows: rows.len() as u16,
        areas,
    })
}

fn parse_grid_auto_flow_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgGridAutoFlow, DgStyleWarning> {
    let resolved = resolve_keyword(value, variables);
    let mut direction: Option<DgGridAutoFlow> = None;
    let mut dense = false;
    let mut saw_token = false;
    for token in resolved.split_whitespace() {
        saw_token = true;
        match token.to_ascii_lowercase().as_str() {
            "row" if direction.is_none() => direction = Some(DgGridAutoFlow::Row),
            "column" if direction.is_none() => direction = Some(DgGridAutoFlow::Column),
            "dense" if !dense => dense = true,
            _ => return Err(parse_warning(name, &resolved, "grid-auto-flow value")),
        }
    }
    if !saw_token {
        return Err(parse_warning(name, value, "grid-auto-flow value"));
    }
    Ok(match (direction.unwrap_or(DgGridAutoFlow::Row), dense) {
        (DgGridAutoFlow::Row, false) => DgGridAutoFlow::Row,
        (DgGridAutoFlow::Row, true) => DgGridAutoFlow::RowDense,
        (DgGridAutoFlow::Column, false) => DgGridAutoFlow::Column,
        (DgGridAutoFlow::Column, true) => DgGridAutoFlow::ColumnDense,
        (flow, _) => flow,
    })
}

fn parse_grid_area_rows(value: &str) -> Option<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    let mut rest = value.trim();
    while !rest.is_empty() {
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let mut row = String::new();
        let mut escaped = false;
        let mut end_index = None;
        for (offset, ch) in rest[quote.len_utf8()..].char_indices() {
            if escaped {
                row.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                end_index = Some(quote.len_utf8() + offset + ch.len_utf8());
                break;
            }
            row.push(ch);
        }
        let end = end_index?;
        let cells: Vec<String> = row.split_whitespace().map(str::to_string).collect();
        if cells.is_empty() {
            return None;
        }
        rows.push(cells);
        rest = rest[end..].trim_start();
    }
    Some(rows)
}

fn is_grid_null_cell(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch == '.')
}

fn is_grid_area_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
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
        if let Some(nested) = parse_grid_repeat(name, track) {
            let nested = match nested {
                Ok(nested) => nested,
                Err(warning) => return Some(Err(warning)),
            };
            if nested
                .iter()
                .any(|track| matches!(track, DgGridTrackSize::Repeat { .. }))
            {
                return Some(Err(parse_warning(
                    name,
                    token,
                    "non-nested auto-repeat track list",
                )));
            }
            parsed.extend(nested);
            continue;
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

fn parse_grid_area_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<String, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    let value = value.trim();
    if is_grid_area_name(value) && !value.eq_ignore_ascii_case("auto") {
        Ok(value.to_string())
    } else {
        Err(parse_warning(name, value, "named grid area"))
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
            let paint = match parse_single_background_paint("background", layer, variables)? {
                Some(paint) => paint,
                None => {
                    DgBackgroundPaint::Color(parse_color_value("background", layer, variables)?)
                }
            };
            paints.push(paint);
        }
        return Ok(Some(DgBackgroundPaint::Layers(paints)));
    }
    parse_single_background_paint("background", &value, variables)
}

fn parse_background_image_paint_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgBackgroundPaint>, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    if value.trim().eq_ignore_ascii_case("none") {
        return Ok(None);
    }

    let layers = split_top_level_commas(&value);
    let mut paints = Vec::with_capacity(layers.len());
    for layer in layers {
        let Some(paint) = parse_single_background_paint(name, layer, variables)? else {
            return Err(parse_warning(
                name,
                layer,
                "linear-gradient, radial-gradient, repeating-gradient, blob-gradient, or none",
            ));
        };
        paints.push(paint);
    }

    match paints.len() {
        0 => Err(parse_warning(
            name,
            &value,
            "linear-gradient, radial-gradient, repeating-gradient, blob-gradient, or none",
        )),
        1 => Ok(paints.pop()),
        _ => Ok(Some(DgBackgroundPaint::Layers(paints))),
    }
}

fn parse_single_background_paint(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Option<DgBackgroundPaint>, DgStyleWarning> {
    if let Some(args) = function_args(&value, "linear-gradient") {
        return Ok(Some(DgBackgroundPaint::LinearGradient(
            parse_linear_gradient(name, args, variables, false)?,
        )));
    }
    if let Some(args) = function_args(&value, "repeating-linear-gradient") {
        return Ok(Some(DgBackgroundPaint::LinearGradient(
            parse_linear_gradient(name, args, variables, true)?,
        )));
    }
    if let Some(args) = function_args(&value, "radial-gradient") {
        return Ok(Some(DgBackgroundPaint::RadialGradient(
            parse_radial_gradient(name, args, variables, false)?,
        )));
    }
    if let Some(args) = function_args(&value, "repeating-radial-gradient") {
        return Ok(Some(DgBackgroundPaint::RadialGradient(
            parse_radial_gradient(name, args, variables, true)?,
        )));
    }
    if let Some(args) = function_args(&value, "blob-gradient") {
        return Ok(Some(DgBackgroundPaint::BlobGradient(parse_blob_gradient(
            name, args, variables,
        )?)));
    }
    if let Some(args) = function_args(&value, "mesh-gradient") {
        return Ok(Some(DgBackgroundPaint::MeshGradient(parse_mesh_gradient(
            name, args, variables,
        )?)));
    }
    Ok(None)
}

fn function_args<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let value = value.trim();
    let prefix = format!("{name}(");
    value
        .strip_prefix(&prefix)
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::trim)
}

fn parse_linear_gradient(
    name: &str,
    args: &str,
    variables: &BTreeMap<String, DgCssValue>,
    repeating: bool,
) -> Result<DgLinearGradient, DgStyleWarning> {
    let parts = split_top_level_commas(args);
    if parts.len() < 2 {
        return Err(parse_warning(
            name,
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
            name,
            args,
            "linear-gradient with at least two color stops",
        ));
    }
    let mut stops = Vec::with_capacity(stop_parts.len());
    for part in stop_parts {
        stops.extend(parse_gradient_stops(name, part, variables)?);
    }
    Ok(DgLinearGradient {
        angle_deg,
        stops,
        repeating,
    })
}

fn parse_radial_gradient(
    name: &str,
    args: &str,
    variables: &BTreeMap<String, DgCssValue>,
    repeating: bool,
) -> Result<DgRadialGradient, DgStyleWarning> {
    let parts = split_top_level_commas(args);
    if parts.len() < 2 {
        return Err(parse_warning(
            name,
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
            name,
            args,
            "radial-gradient with at least two color stops",
        ));
    }
    let mut stops = Vec::with_capacity(stop_parts.len());
    for part in stop_parts {
        stops.extend(parse_gradient_stops(name, part, variables)?);
    }
    Ok(DgRadialGradient {
        stops,
        repeating,
        center,
    })
}

fn parse_blob_gradient(
    name: &str,
    args: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgBlobGradient, DgStyleWarning> {
    let parts = split_top_level_commas(args);
    if parts.is_empty() {
        return Err(parse_warning(
            name,
            args,
            "blob-gradient entries like `at 20% 30% <color> 45%`",
        ));
    }
    let mut blobs = Vec::with_capacity(parts.len().min(4));
    for part in parts.iter().take(4) {
        blobs.push(parse_blob_gradient_stop(name, part, variables)?);
    }
    Ok(DgBlobGradient { blobs })
}

fn parse_blob_gradient_stop(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgBlobGradientStop, DgStyleWarning> {
    let tokens = split_value_tokens(value);
    if tokens.len() != 5 || !tokens[0].eq_ignore_ascii_case("at") {
        return Err(parse_warning(
            name,
            value,
            "blob-gradient stop `at <x> <y> <color> <radius>`",
        ));
    }
    let x = tokens[1];
    let y = tokens[2];
    let color = tokens[3];
    let radius = tokens[4];
    let center = [
        parse_radial_center_axis(x).ok_or_else(|| parse_warning(name, value, "blob x center"))?,
        parse_radial_center_axis(y).ok_or_else(|| parse_warning(name, value, "blob y center"))?,
    ];
    let radius = parse_gradient_stop_position(radius)
        .map_err(|_| parse_warning(name, value, "blob radius"))?
        .clamp(0.05, 1.2);
    let color = parse_color_value(name, color, variables)?;
    Ok(DgBlobGradientStop {
        center,
        radius,
        color,
    })
}

fn parse_mesh_gradient(
    name: &str,
    args: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgMeshGradient, DgStyleWarning> {
    let parts = split_top_level_commas(args);
    if parts.len() != 4 {
        return Err(parse_warning(
            name,
            args,
            "mesh-gradient(<top-left>, <top-right>, <bottom-left>, <bottom-right>)",
        ));
    }
    Ok(DgMeshGradient {
        top_left: parse_color_value(name, parts[0], variables)?,
        top_right: parse_color_value(name, parts[1], variables)?,
        bottom_left: parse_color_value(name, parts[2], variables)?,
        bottom_right: parse_color_value(name, parts[3], variables)?,
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
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<Vec<DgGradientStop>, DgStyleWarning> {
    let tokens = split_value_tokens(value);
    if tokens.is_empty() || tokens.len() > 3 {
        return Err(parse_warning(name, value, "gradient color stop"));
    }
    let color = parse_color_value(name, tokens[0], variables)?;
    let first_position = tokens
        .get(1)
        .map(|value| parse_gradient_stop_position(value))
        .transpose()
        .map_err(|_| parse_warning(name, value, "gradient stop position"))?;
    let mut stops = vec![DgGradientStop {
        color: color.clone(),
        position: first_position,
    }];
    if let Some(second) = tokens.get(2) {
        let position = parse_gradient_stop_position(second)
            .map_err(|_| parse_warning(name, value, "gradient stop position"))?;
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
    if let Some(color) = parse_web_color(value) {
        Some(DgCssColor::Rgba(color))
    } else if is_identifier_like(value) {
        Some(DgCssColor::Token(value.to_string()))
    } else {
        None
    }
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

fn parse_border_style_value(
    name: &str,
    value: &str,
    variables: &BTreeMap<String, DgCssValue>,
) -> Result<DgBorderStyle, DgStyleWarning> {
    let value = resolve_keyword(value, variables);
    let parts = split_value_tokens(&value);
    if parts.len() != 1 {
        return Err(parse_warning(
            name,
            value.as_str(),
            "solid, none, or hidden",
        ));
    }
    match parts[0].trim().to_ascii_lowercase().as_str() {
        "solid" => Ok(DgBorderStyle::Solid),
        "none" | "hidden" => Ok(DgBorderStyle::None),
        _ => Err(parse_warning(
            name,
            value.as_str(),
            "solid, none, or hidden",
        )),
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

fn parse_generated_attr(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    let inner = lower.strip_prefix("attr(")?.strip_suffix(')')?.trim();
    if inner.is_empty()
        || !inner
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return None;
    }
    Some(inner.to_string())
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
    use crate::document::NodeProps;
    use crate::style::AlignItemsStyle;
    use crate::style::StepPosition;

    fn env_usize(name: &str, default: usize) -> usize {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(default)
    }

    fn css_bench_node(id: &str, kind: WidgetKind, class_name: Option<String>) -> WidgetNode {
        WidgetNode {
            id: id.to_string(),
            key: None,
            class_name,
            kind,
            props: NodeProps::default(),
            style_json: Default::default(),
            inline_style: Default::default(),
            style: Default::default(),
            children: Vec::new(),
        }
    }

    fn css_bench_tree(count: usize) -> WidgetNode {
        let mut root = css_bench_node("root", WidgetKind::Window, Some("app".to_string()));
        let mut panel = css_bench_node("panel", WidgetKind::Panel, Some("content".to_string()));
        panel.children.reserve(count);
        for index in 0..count {
            let kind = match index % 8 {
                0 => WidgetKind::Label,
                1 => WidgetKind::Button,
                2 => WidgetKind::ProgressBar,
                3 => WidgetKind::Slider,
                4 => WidgetKind::DataFrameTable,
                5 => WidgetKind::TextInput,
                6 => WidgetKind::Badge,
                _ => WidgetKind::Tag,
            };
            let class_name = match index % 5 {
                0 => Some("primary dense".to_string()),
                1 => Some("secondary".to_string()),
                2 => Some("metric warning".to_string()),
                3 => Some("quiet".to_string()),
                _ => None,
            };
            panel
                .children
                .push(css_bench_node(&format!("w{index}"), kind, class_name));
        }
        root.children.push(panel);
        root
    }

    fn css_bench_store() -> StylesheetStore {
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Window { background: #101318; color: #e8edf3; }
                Panel.content { padding: 12px; gap: 8px; }
                Label, Button, Slider, ProgressBar, TextInput, Badge, Tag {
                    font-size: 13px;
                    border-radius: 6px;
                }
                Button.primary, Badge.metric, Tag.warning { background: accent; color: white; }
                .dense { width: 140px; height: 28px; }
                .quiet { opacity: 0.86; }
                ProgressBar::track { background: #1b2330; }
                ProgressBar::fill { background: #69d2a2; }
                DataFrameTable { table-row-height: 24px; table-column-width: 132px; }
                DataFrameTable::header { background: #1d2836; font-weight: 700; }
                Panel.content > Button { border-width: 1px; border-color: #34445a; }
                "#,
            )
            .unwrap();
        store
    }

    fn css_bench_simple_store() -> StylesheetStore {
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Window { background: #101318; color: #e8edf3; }
                Panel.content { padding: 12px; gap: 8px; }
                Label, Button, Slider, ProgressBar, TextInput, Badge, Tag {
                    font-size: 13px;
                    border-radius: 6px;
                }
                Button.primary, Badge.metric, Tag.warning { background: accent; color: white; }
                .dense { width: 140px; height: 28px; }
                .quiet { opacity: 0.86; }
                ProgressBar::track { background: #1b2330; }
                ProgressBar::fill { background: #69d2a2; }
                DataFrameTable { table-row-height: 24px; table-column-width: 132px; }
                DataFrameTable::header { background: #1d2836; font-weight: 700; }
                "#,
            )
            .unwrap();
        store
    }

    fn css_bench_large_rule_store(extra_rules: usize) -> StylesheetStore {
        let mut css = String::from(
            r#"
            Window { background: #101318; color: #e8edf3; }
            Panel.content { padding: 12px; gap: 8px; }
            Label, Button, Slider, ProgressBar, TextInput, Badge, Tag {
                font-size: 13px;
                border-radius: 6px;
            }
            Button.primary, Badge.metric, Tag.warning { background: accent; color: white; }
            .dense { width: 140px; height: 28px; }
            .quiet { opacity: 0.86; }
            ProgressBar::track { background: #1b2330; }
            ProgressBar::fill { background: #69d2a2; }
            DataFrameTable { table-row-height: 24px; table-column-width: 132px; }
            DataFrameTable::header { background: #1d2836; font-weight: 700; }
            Panel.content > Button { border-width: 1px; border-color: #34445a; }
            "#,
        );
        for index in 0..extra_rules {
            css.push_str(&format!(
                "Button.unused-{index} {{ width: {}px; }}\n",
                80 + (index % 120)
            ));
        }

        let mut store = StylesheetStore::default();
        store.set_stylesheet(StylesheetOrigin::User, &css).unwrap();
        store
    }

    #[test]
    #[ignore]
    fn bench_css_clone_many_widgets() {
        let count = env_usize("DRAGONGUI_BENCH_CSS_WIDGETS", 2_000);
        let iterations = env_usize("DRAGONGUI_BENCH_CSS_ITERS", 300);
        let warmup = env_usize("DRAGONGUI_BENCH_CSS_WARMUP", 20);
        let tree = css_bench_tree(count);

        for _ in 0..warmup {
            let next = tree.clone();
            std::hint::black_box(next.children.len());
        }

        let start = std::time::Instant::now();
        let mut touched = 0usize;
        for _ in 0..iterations {
            let next = tree.clone();
            touched += next
                .children
                .first()
                .map(|node| node.children.len())
                .unwrap_or(0);
            std::hint::black_box(&next);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "css clone many widgets: widgets={count} iterations={iterations} total_ms={:.3} ns_per_widget={:.1} touched_per_iter={:.1}",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_nanos() as f64 / (iterations * count) as f64,
            touched as f64 / iterations as f64
        );
    }

    #[test]
    #[ignore]
    fn bench_css_cascade_many_widgets() {
        let count = env_usize("DRAGONGUI_BENCH_CSS_WIDGETS", 2_000);
        let iterations = env_usize("DRAGONGUI_BENCH_CSS_ITERS", 300);
        let warmup = env_usize("DRAGONGUI_BENCH_CSS_WARMUP", 20);
        let tree = css_bench_tree(count);
        let mut store = css_bench_store();

        for _ in 0..warmup {
            let mut next = tree.clone();
            apply_stylesheets_to_tree(&mut next, &mut store);
            std::hint::black_box(next.children.len());
        }

        let start = std::time::Instant::now();
        let mut touched = 0usize;
        for _ in 0..iterations {
            let mut next = tree.clone();
            apply_stylesheets_to_tree(&mut next, &mut store);
            touched += next
                .children
                .first()
                .map(|node| node.children.len())
                .unwrap_or(0);
            std::hint::black_box(&next);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "css cascade many widgets: widgets={count} iterations={iterations} total_ms={:.3} ns_per_widget={:.1} touched_per_iter={:.1}",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_nanos() as f64 / (iterations * count) as f64,
            touched as f64 / iterations as f64
        );
    }

    #[test]
    #[ignore]
    fn bench_css_cascade_many_widgets_pure() {
        let count = env_usize("DRAGONGUI_BENCH_CSS_WIDGETS", 2_000);
        let iterations = env_usize("DRAGONGUI_BENCH_CSS_ITERS", 300);
        let warmup = env_usize("DRAGONGUI_BENCH_CSS_WARMUP", 20);
        let mut tree = css_bench_tree(count);
        let mut store = css_bench_store();

        for _ in 0..warmup {
            apply_stylesheets_to_tree(&mut tree, &mut store);
            std::hint::black_box(tree.children.len());
        }

        let start = std::time::Instant::now();
        let mut touched = 0usize;
        for _ in 0..iterations {
            apply_stylesheets_to_tree(&mut tree, &mut store);
            touched += tree
                .children
                .first()
                .map(|node| node.children.len())
                .unwrap_or(0);
            std::hint::black_box(&tree);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "css cascade many widgets pure: widgets={count} iterations={iterations} total_ms={:.3} ns_per_widget={:.1} touched_per_iter={:.1}",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_nanos() as f64 / (iterations * count) as f64,
            touched as f64 / iterations as f64
        );
    }

    #[test]
    #[ignore]
    fn bench_css_cascade_simple_widgets_pure() {
        let count = env_usize("DRAGONGUI_BENCH_CSS_WIDGETS", 2_000);
        let iterations = env_usize("DRAGONGUI_BENCH_CSS_ITERS", 300);
        let warmup = env_usize("DRAGONGUI_BENCH_CSS_WARMUP", 20);
        let mut tree = css_bench_tree(count);
        let mut store = css_bench_simple_store();

        for _ in 0..warmup {
            apply_stylesheets_to_tree(&mut tree, &mut store);
            std::hint::black_box(tree.children.len());
        }

        let start = std::time::Instant::now();
        let mut touched = 0usize;
        for _ in 0..iterations {
            apply_stylesheets_to_tree(&mut tree, &mut store);
            touched += tree
                .children
                .first()
                .map(|node| node.children.len())
                .unwrap_or(0);
            std::hint::black_box(&tree);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "css cascade simple widgets pure: widgets={count} iterations={iterations} total_ms={:.3} ns_per_widget={:.1} touched_per_iter={:.1}",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_nanos() as f64 / (iterations * count) as f64,
            touched as f64 / iterations as f64
        );
    }

    #[test]
    #[ignore]
    fn bench_css_cascade_many_rules_pure() {
        let count = env_usize("DRAGONGUI_BENCH_CSS_WIDGETS", 2_000);
        let iterations = env_usize("DRAGONGUI_BENCH_CSS_ITERS", 300);
        let warmup = env_usize("DRAGONGUI_BENCH_CSS_WARMUP", 20);
        let extra_rules = env_usize("DRAGONGUI_BENCH_CSS_EXTRA_RULES", 400);
        let mut tree = css_bench_tree(count);
        let mut store = css_bench_large_rule_store(extra_rules);

        for _ in 0..warmup {
            apply_stylesheets_to_tree(&mut tree, &mut store);
            std::hint::black_box(tree.children.len());
        }

        let start = std::time::Instant::now();
        let mut touched = 0usize;
        for _ in 0..iterations {
            apply_stylesheets_to_tree(&mut tree, &mut store);
            touched += tree
                .children
                .first()
                .map(|node| node.children.len())
                .unwrap_or(0);
            std::hint::black_box(&tree);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "css cascade many rules pure: widgets={count} extra_rules={extra_rules} iterations={iterations} total_ms={:.3} ns_per_widget={:.1} touched_per_iter={:.1}",
            elapsed.as_secs_f64() * 1000.0,
            elapsed.as_nanos() as f64 / (iterations * count) as f64,
            touched as f64 / iterations as f64
        );
    }

    #[test]
    fn property_matrix_maps_supported_names() {
        let cases = [
            (
                "flex-direction",
                DgStylePropertyName::Layout(DgLayoutPropertyName::FlexDirection),
            ),
            (
                "flex-basis",
                DgStylePropertyName::Layout(DgLayoutPropertyName::FlexBasis),
            ),
            (
                "background-color",
                DgStylePropertyName::Visual(DgVisualPropertyName::Background),
            ),
            (
                "margin-left",
                DgStylePropertyName::Layout(DgLayoutPropertyName::MarginLeft),
            ),
            (
                "grid-auto-flow",
                DgStylePropertyName::Layout(DgLayoutPropertyName::GridAutoFlow),
            ),
            (
                "background-image",
                DgStylePropertyName::Visual(DgVisualPropertyName::BackgroundImage),
            ),
            (
                "backdrop-filter",
                DgStylePropertyName::Visual(DgVisualPropertyName::BackdropFilter),
            ),
            (
                "border-top-right-radius",
                DgStylePropertyName::Visual(DgVisualPropertyName::BorderTopRightRadius),
            ),
            (
                "border-style",
                DgStylePropertyName::Visual(DgVisualPropertyName::BorderStyle),
            ),
            (
                "outline-color",
                DgStylePropertyName::Visual(DgVisualPropertyName::OutlineColor),
            ),
            (
                "outline",
                DgStylePropertyName::Visual(DgVisualPropertyName::Outline),
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
                "animation",
                DgStylePropertyName::Animation(DgAnimationPropertyName::Animation),
            ),
            (
                "animation-name",
                DgStylePropertyName::Animation(DgAnimationPropertyName::Name),
            ),
            (
                "animation-duration",
                DgStylePropertyName::Animation(DgAnimationPropertyName::Duration),
            ),
            (
                "animation-timing-function",
                DgStylePropertyName::Animation(DgAnimationPropertyName::TimingFunction),
            ),
            (
                "animation-delay",
                DgStylePropertyName::Animation(DgAnimationPropertyName::Delay),
            ),
            (
                "animation-iteration-count",
                DgStylePropertyName::Animation(DgAnimationPropertyName::IterationCount),
            ),
            (
                "animation-direction",
                DgStylePropertyName::Animation(DgAnimationPropertyName::Direction),
            ),
            (
                "animation-fill-mode",
                DgStylePropertyName::Animation(DgAnimationPropertyName::FillMode),
            ),
            (
                "animation-play-state",
                DgStylePropertyName::Animation(DgAnimationPropertyName::PlayState),
            ),
            (
                "content",
                DgStylePropertyName::Generated(DgGeneratedPropertyName::Content),
            ),
            (
                "text-area-rows",
                DgStylePropertyName::Widget(DgWidgetPropertyName::TextAreaRows),
            ),
            (
                "scatter-point-size",
                DgStylePropertyName::Widget(DgWidgetPropertyName::ScatterPointSize),
            ),
            (
                "scatter-point-style",
                DgStylePropertyName::Widget(DgWidgetPropertyName::ScatterPointStyle),
            ),
            (
                "scatter-grid-visible",
                DgStylePropertyName::Widget(DgWidgetPropertyName::ScatterGridVisible),
            ),
            (
                "scatter-grid-planes",
                DgStylePropertyName::Widget(DgWidgetPropertyName::ScatterGridPlanes),
            ),
            (
                "scatter-legend-position",
                DgStylePropertyName::Widget(DgWidgetPropertyName::ScatterLegendPosition),
            ),
            (
                "scatter-orientation-axes",
                DgStylePropertyName::Widget(DgWidgetPropertyName::ScatterOrientationAxes),
            ),
            (
                "table-row-height",
                DgStylePropertyName::Widget(DgWidgetPropertyName::TableRowHeight),
            ),
            (
                "table-header-height",
                DgStylePropertyName::Widget(DgWidgetPropertyName::TableHeaderHeight),
            ),
            (
                "table-column-width",
                DgStylePropertyName::Widget(DgWidgetPropertyName::TableColumnWidth),
            ),
            (
                "table-index-width",
                DgStylePropertyName::Widget(DgWidgetPropertyName::TableIndexWidth),
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
            WidgetKind::SmallButton,
            WidgetKind::IconButton,
            WidgetKind::ImageButton,
            WidgetKind::ArrowButton,
            WidgetKind::Badge,
            WidgetKind::Tag,
            WidgetKind::Panel,
            WidgetKind::DataFrameTable,
            WidgetKind::PieChart,
            WidgetKind::Histogram,
            WidgetKind::BarChart,
            WidgetKind::Heatmap,
            WidgetKind::LinePlot,
            WidgetKind::Scatter3D,
            WidgetKind::HtmlReport,
            WidgetKind::Image,
            WidgetKind::Extension,
            WidgetKind::Toast,
        ];

        for kind in kinds {
            let name = css_type_name(kind).expect("known widget should have a CSS name");
            assert_eq!(widget_kind_from_css_type(name), Some(kind));
        }
        assert_eq!(
            widget_kind_from_css_type("PaintWidget"),
            Some(WidgetKind::Extension)
        );
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
            children: None,
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
            children: None,
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
            children: None,
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
            children: None,
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
            children: None,
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
        let empty_children: [StyleSibling; 0] = [];
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
            children: Some(&empty_children),
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
        let second_from_end = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::NthLastChild(DgNthChild::Exact(2))),
        );
        let even_from_end = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::NthLastChild(DgNthChild::Even)),
        );
        let odd_from_end = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::NthLastChild(DgNthChild::Odd)),
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
        let only_child = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::OnlyChild),
        );
        let empty = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_structural(DgStructuralPseudo::Empty),
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
        assert!(second_from_end.matches(&element));
        assert!(even_from_end.matches(&element));
        assert!(!odd_from_end.matches(&element));
        assert!(!first_child.matches(&element));
        assert!(!last_child.matches(&element));
        assert!(!only_child.matches(&element));
        assert!(empty.matches(&element));
        assert!(every_third_offset.matches(&element));
        assert!(first_three.matches(&element));
        assert!(!after_third.matches(&element));

        let single = StyleElement {
            id: "only",
            sibling_index: Some(0),
            sibling_count: Some(1),
            ..element
        };
        assert!(only_child.matches(&single));
        assert_eq!(only_child.label(), "Button:only-child");

        let non_empty_children = [StyleSibling {
            id: "child".to_string(),
            key: None,
            attributes: Vec::new(),
            classes: Vec::new(),
            pseudo: Vec::new(),
            kind: WidgetKind::Label,
            children: Vec::new(),
        }];
        let non_empty = StyleElement {
            children: Some(&non_empty_children),
            ..element
        };
        assert!(!empty.matches(&non_empty));
        assert_eq!(empty.label(), "Button:empty");
        assert_eq!(second_from_end.label(), "Button:nth-last-child(2)");
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
            children: None,
        };
        let not_ghost = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Button)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Not,
                    selectors: vec![DgSelectorFunctionArgument::compound(
                        DgCompoundSelector::new().with_class("ghost"),
                    )],
                }),
        );
        let is_button_or_label =
            DgSelector::Compound(DgCompoundSelector::new().with_function(DgSelectorFunction {
                kind: DgSelectorFunctionKind::Is,
                selectors: vec![
                    DgSelectorFunctionArgument::compound(
                        DgCompoundSelector::new().with_type(WidgetKind::Button),
                    ),
                    DgSelectorFunctionArgument::compound(
                        DgCompoundSelector::new().with_type(WidgetKind::Label),
                    ),
                ],
            }));
        let where_primary =
            DgSelector::Compound(DgCompoundSelector::new().with_function(DgSelectorFunction {
                kind: DgSelectorFunctionKind::Where,
                selectors: vec![DgSelectorFunctionArgument::compound(
                    DgCompoundSelector::new().with_class("primary"),
                )],
            }));

        assert!(not_ghost.matches(&element));
        assert!(is_button_or_label.matches(&element));
        assert!(where_primary.matches(&element));
        assert_eq!(not_ghost.specificity(), Specificity::new(0, 1, 1));
        assert_eq!(is_button_or_label.specificity(), Specificity::new(0, 0, 1));
        assert_eq!(where_primary.specificity(), Specificity::ZERO);
    }

    #[test]
    fn selector_matching_supports_direct_child_has_function() {
        let button_classes = vec!["primary".to_string()];
        let button_attributes = vec![StyleAttribute {
            name: "text".to_string(),
            value: "Run".to_string(),
        }];
        let badge_attributes = vec![StyleAttribute {
            name: "level".to_string(),
            value: "warning".to_string(),
        }];
        let success_badge_attributes = vec![StyleAttribute {
            name: "level".to_string(),
            value: "success".to_string(),
        }];
        let children = [
            StyleSibling {
                id: "run".to_string(),
                key: Some("primary-action".to_string()),
                attributes: button_attributes,
                classes: button_classes,
                pseudo: Vec::new(),
                kind: WidgetKind::Button,
                children: Vec::new(),
            },
            StyleSibling {
                id: "flag".to_string(),
                key: None,
                attributes: badge_attributes,
                classes: Vec::new(),
                pseudo: Vec::new(),
                kind: WidgetKind::Badge,
                children: Vec::new(),
            },
            StyleSibling {
                id: "row".to_string(),
                key: None,
                attributes: Vec::new(),
                classes: Vec::new(),
                pseudo: Vec::new(),
                kind: WidgetKind::HLayout,
                children: vec![StyleSibling {
                    id: "ok".to_string(),
                    key: None,
                    attributes: success_badge_attributes,
                    classes: Vec::new(),
                    pseudo: Vec::new(),
                    kind: WidgetKind::Badge,
                    children: Vec::new(),
                }],
            },
        ];
        let element = StyleElement {
            id: "panel",
            key: None,
            attributes: &[],
            classes: &[],
            kind: WidgetKind::Panel,
            ancestors: &[],
            pseudo: &[],
            sibling_index: None,
            sibling_count: None,
            siblings: None,
            children: Some(&children),
        };
        let has_primary_button = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Panel)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Has,
                    selectors: vec![DgSelectorFunctionArgument::new(DgSelector::Compound(
                        DgCompoundSelector::new()
                            .with_type(WidgetKind::Button)
                            .with_class("primary"),
                    ))],
                }),
        );
        let has_warning_badge = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Panel)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Has,
                    selectors: vec![DgSelectorFunctionArgument::new(DgSelector::Compound(
                        DgCompoundSelector::new()
                            .with_type(WidgetKind::Badge)
                            .with_attribute("level", "warning"),
                    ))],
                }),
        );
        let has_first_primary_button = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Panel)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Has,
                    selectors: vec![DgSelectorFunctionArgument::direct_child(
                        DgSelector::Compound(
                            DgCompoundSelector::new()
                                .with_type(WidgetKind::Button)
                                .with_class("primary")
                                .with_structural(DgStructuralPseudo::FirstChild),
                        ),
                    )],
                }),
        );
        let has_second_badge = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Panel)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Has,
                    selectors: vec![DgSelectorFunctionArgument::direct_child(
                        DgSelector::Compound(
                            DgCompoundSelector::new()
                                .with_type(WidgetKind::Badge)
                                .with_structural(DgStructuralPseudo::NthChild(DgNthChild::Exact(
                                    2,
                                ))),
                        ),
                    )],
                }),
        );
        let has_first_badge = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Panel)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Has,
                    selectors: vec![DgSelectorFunctionArgument::direct_child(
                        DgSelector::Compound(
                            DgCompoundSelector::new()
                                .with_type(WidgetKind::Badge)
                                .with_structural(DgStructuralPseudo::FirstChild),
                        ),
                    )],
                }),
        );
        let has_success_badge_descendant = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Panel)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Has,
                    selectors: vec![DgSelectorFunctionArgument::new(DgSelector::Compound(
                        DgCompoundSelector::new()
                            .with_type(WidgetKind::Badge)
                            .with_attribute("level", "success"),
                    ))],
                }),
        );
        let has_hlayout_success_badge_chain = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Panel)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Has,
                    selectors: vec![DgSelectorFunctionArgument::new(DgSelector::Chain(
                        DgSelectorChain {
                            ancestors: vec![(
                                DgCombinator::Child,
                                DgCompoundSelector::new().with_type(WidgetKind::HLayout),
                            )],
                            target: DgCompoundSelector::new()
                                .with_type(WidgetKind::Badge)
                                .with_attribute("level", "success"),
                        },
                    ))],
                }),
        );
        let has_direct_success_badge = DgSelector::Compound(
            DgCompoundSelector::new()
                .with_type(WidgetKind::Panel)
                .with_function(DgSelectorFunction {
                    kind: DgSelectorFunctionKind::Has,
                    selectors: vec![DgSelectorFunctionArgument::direct_child(
                        DgSelector::Compound(
                            DgCompoundSelector::new()
                                .with_type(WidgetKind::Badge)
                                .with_attribute("level", "success"),
                        ),
                    )],
                }),
        );

        assert!(has_primary_button.matches(&element));
        assert!(has_warning_badge.matches(&element));
        assert!(has_first_primary_button.matches(&element));
        assert!(has_second_badge.matches(&element));
        assert!(!has_first_badge.matches(&element));
        assert!(has_success_badge_descendant.matches(&element));
        assert!(has_hlayout_success_badge_chain.matches(&element));
        assert!(!has_direct_success_badge.matches(&element));
        assert_eq!(has_primary_button.specificity(), Specificity::new(0, 1, 2));
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
            children: None,
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
                    selectors: vec![DgSelectorFunctionArgument::compound(
                        DgCompoundSelector::new().with_pseudo(DgPseudoClass::Disabled),
                    )],
                }),
        );
        let is_hover_or_focus =
            DgSelector::Compound(DgCompoundSelector::new().with_function(DgSelectorFunction {
                kind: DgSelectorFunctionKind::Is,
                selectors: vec![
                    DgSelectorFunctionArgument::compound(
                        DgCompoundSelector::new().with_pseudo(DgPseudoClass::Hover),
                    ),
                    DgSelectorFunctionArgument::compound(
                        DgCompoundSelector::new().with_pseudo(DgPseudoClass::Focus),
                    ),
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
            children: None,
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
            Panel:has(> Button.primary) { border-width: 2px; }
            Panel:has(Badge[level="warning"]) { border-color: warning; }
            Panel:has(> Button:first-child) { border-radius: 9px; }
            Panel:has(> Label:only-child) { border-radius: 10px; }
            Panel:has(> Badge:empty) { opacity: 0.55; }
            Panel:has(HLayout > Badge[level="success"]) { opacity: 0.9; }
            Panel:has(+ Button.primary) { border-width: 3px; }
            Panel:has(~ Badge[level="success"]) { background: success; }
            Panel:has(Panel:has(Button.primary)) { opacity: 0.7; }
            Panel:has(Panel:has(> Badge[level="success"]) > Button.primary) { opacity: 0.6; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert_eq!(parsed.rules.len(), 15);
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
        assert_eq!(
            parsed.rules[5].selector.label(),
            "Panel:has(> Button.primary)"
        );
        assert_eq!(
            parsed.rules[5].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(
            parsed.rules[6].selector.label(),
            "Panel:has(Badge[level=\"warning\"])"
        );
        assert_eq!(
            parsed.rules[6].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(
            parsed.rules[7].selector.label(),
            "Panel:has(> Button:first-child)"
        );
        assert_eq!(
            parsed.rules[7].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(
            parsed.rules[8].selector.label(),
            "Panel:has(> Label:only-child)"
        );
        assert_eq!(
            parsed.rules[8].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(parsed.rules[9].selector.label(), "Panel:has(> Badge:empty)");
        assert_eq!(
            parsed.rules[9].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(
            parsed.rules[10].selector.label(),
            "Panel:has(HLayout > Badge[level=\"success\"])"
        );
        assert_eq!(
            parsed.rules[10].selector.specificity(),
            Specificity::new(0, 1, 3)
        );
        assert_eq!(
            parsed.rules[11].selector.label(),
            "Panel:has(+ Button.primary)"
        );
        assert_eq!(
            parsed.rules[11].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(
            parsed.rules[12].selector.label(),
            "Panel:has(~ Badge[level=\"success\"])"
        );
        assert_eq!(
            parsed.rules[12].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(
            parsed.rules[13].selector.label(),
            "Panel:has(Panel:has(Button.primary))"
        );
        assert_eq!(
            parsed.rules[13].selector.specificity(),
            Specificity::new(0, 1, 3)
        );
        assert_eq!(
            parsed.rules[14].selector.label(),
            "Panel:has(Panel:has(> Badge[level=\"success\"]) > Button.primary)"
        );
        assert_eq!(
            parsed.rules[14].selector.specificity(),
            Specificity::new(0, 2, 4)
        );
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
            Panel > *:nth-child(1 of Button:first-child, Badge:last-child) { border-radius: 7px; }
            Panel > *:nth-child(1 of Window > Panel > Button:nth-child(2)) { border-width: 4px; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.rules.len(), 7);
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
        assert_eq!(
            parsed.rules[5].selector.label(),
            "Panel > :nth-child(1 of Button:first-child, Badge:last-child)"
        );
        assert_eq!(
            parsed.rules[5].selector.specificity(),
            Specificity::new(0, 2, 2)
        );
        assert_eq!(
            parsed.rules[6].selector.label(),
            "Panel > :nth-child(1 of Window > Panel > Button:nth-child(2))"
        );
        assert_eq!(
            parsed.rules[6].selector.specificity(),
            Specificity::new(0, 2, 4)
        );
    }

    #[test]
    fn parses_nth_last_child_selectors() {
        let parsed = parse_stylesheet(
            r#"
            Panel > Button:nth-last-child(2) { border-width: 6px; }
            Panel > *:nth-last-child(1 of Button.metric, Badge[level="info"]) { opacity: 0.35; }
            Panel > *:nth-last-child(2n + 1 of Panel > Button.primary) { border-radius: 9px; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.rules.len(), 3);
        assert_eq!(
            parsed.rules[0].selector.label(),
            "Panel > Button:nth-last-child(2)"
        );
        assert_eq!(
            parsed.rules[0].selector.specificity(),
            Specificity::new(0, 1, 2)
        );
        assert_eq!(
            parsed.rules[1].selector.label(),
            "Panel > :nth-last-child(1 of Button.metric, Badge[level=\"info\"])"
        );
        assert_eq!(
            parsed.rules[1].selector.specificity(),
            Specificity::new(0, 2, 2)
        );
        assert_eq!(
            parsed.rules[2].selector.label(),
            "Panel > :nth-last-child(odd of Panel > Button.primary)"
        );
        assert_eq!(
            parsed.rules[2].selector.specificity(),
            Specificity::new(0, 2, 3)
        );
    }

    #[test]
    fn parses_data_backed_stateful_has_and_nth_filters() {
        let parsed = parse_stylesheet(
            r#"
            Panel:has(> Checkbox:checked) { border-width: 2px; }
            Panel:has(Collapsible:collapsed) { border-color: warning; }
            Panel > *:nth-child(1 of Checkbox:checked, Collapsible:collapsed) { opacity: 0.8; }
            Panel:has(Button:hover) { background: danger; }
            Panel > *:nth-child(1 of Button:hover) { color: danger; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert_eq!(parsed.rules.len(), 3);
        assert_eq!(parsed.warnings.len(), 2);
        assert_eq!(
            parsed.rules[0].selector.label(),
            "Panel:has(> Checkbox:checked)"
        );
        assert_eq!(
            parsed.rules[1].selector.label(),
            "Panel:has(Collapsible:collapsed)"
        );
        assert_eq!(
            parsed.rules[2].selector.label(),
            "Panel > :nth-child(1 of Checkbox:checked, Collapsible:collapsed)"
        );
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property.contains("Button:hover")));
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
            }, {
                "id": "single-panel",
                "type": "panel",
                "children": [
                    {"id": "only-label", "type": "label", "props": {"text": "Only"}}
                ]
            }, {
                "id": "empty-container",
                "type": "panel",
                "children": [{
                    "id": "empty-panel",
                    "type": "panel",
                    "class": "empty-target"
                }, {
                    "id": "non-empty-panel",
                    "type": "panel",
                    "class": "empty-target",
                    "children": [
                        {"id": "inner-label", "type": "label", "props": {"text": "Inner"}}
                    ]
                }]
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
                Panel > Button:nth-last-child(2) { thumb-color: success; }
                Panel > *:nth-last-child(1 of Button.metric) { track-color: warning; }
                Panel > *:nth-child(odd of .metric) { border-radius: 10px; }
                Panel > Button:nth-child(2 of Window > Panel > Button.metric) { opacity: 0.4; }
                Panel > *:nth-child(2 of Button:first-child, Label:nth-child(2)) { border-width: 5px; }
                Panel > *:nth-child(1 of Window > Panel > Label:nth-child(2)) { background: warning; }
                Panel > Label:only-child { background: success; }
                Panel:has(> Label:only-child) { border-color: success; }
                Panel > Panel.empty-target:empty { background: warning; }
                Panel:has(> Panel.empty-target:empty) { border-width: 6px; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let panel = &tree.children[0];
        let single_panel = &tree.children[1];
        let empty_container = &tree.children[2];
        let first = &panel.children[0];
        let caption = &panel.children[1];
        let second = &panel.children[2];
        let third = &panel.children[3];
        let only_label = &single_panel.children[0];
        let empty_panel = &empty_container.children[0];
        let non_empty_panel = &empty_container.children[1];

        assert_eq!(
            first.style.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            first.style.text.color,
            Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]))
        );
        assert_eq!(first.style.visual.border_radius, Some(10.0));
        assert_ne!(first.style.visual.border_width, Some(5.0));
        assert_ne!(caption.style.visual.border_radius, Some(10.0));
        assert_eq!(caption.style.visual.border_width, Some(5.0));
        assert_eq!(
            caption.style.visual.background,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_eq!(second.style.visual.opacity, Some(0.4));
        assert_eq!(second.style.visual.border_radius, Some(10.0));
        assert_eq!(
            second.style.visual.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            second.style.visual.thumb_color,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(
            second.style.visual.track_color,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_eq!(
            second.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_ne!(
            first.style.visual.track_color,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_ne!(
            third.style.visual.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            third.style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
        assert_eq!(
            single_panel.style.visual.border_color,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(
            only_label.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(empty_container.style.visual.border_width, Some(6.0));
        assert_eq!(
            empty_panel.style.visual.background,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_ne!(
            non_empty_panel.style.visual.background,
            Some(ColorRef::Token("warning".to_string()))
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
    fn stylesheet_cascade_applies_direct_child_has_selectors() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [
                {
                    "id": "actions",
                    "type": "panel",
                    "children": [
                        {"id": "run", "type": "button", "class": "primary", "props": {"text": "Run"}}
                    ]
                },
                {
                    "id": "alerts",
                    "type": "panel",
                    "children": [
                        {"id": "warning", "type": "badge", "props": {"text": "Warn", "level": "warning"}}
                    ]
                },
                {
                    "id": "empty",
                    "type": "panel",
                    "children": [
                        {"id": "note", "type": "label", "props": {"text": "Plain"}}
                    ]
                },
                {
                    "id": "nested",
                    "type": "panel",
                    "children": [{
                        "id": "row",
                        "type": "h_layout",
                        "children": [
                            {"id": "success", "type": "badge", "props": {"text": "OK", "level": "success"}}
                        ]
                    }]
                }
            ]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel:has(> Button.primary) { border-width: 4px; }
                Panel:has(Badge[level="warning"]) { border-color: warning; }
                Panel:has(> Button:first-child) { border-radius: 9px; }
                Panel:has(> Badge:last-child) { opacity: 0.75; }
                Panel:has(HLayout > Badge[level="success"]) { background: success; }
                Panel:has(> Badge[level="success"]) { border-width: 8px; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);

        assert_eq!(tree.children[0].style.visual.border_width, Some(4.0));
        assert_ne!(tree.children[1].style.visual.border_width, Some(4.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(9.0));
        assert_ne!(tree.children[2].style.visual.border_radius, Some(9.0));
        assert_eq!(
            tree.children[1].style.visual.border_color,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_eq!(tree.children[1].style.visual.opacity, Some(0.75));
        assert_ne!(
            tree.children[2].style.visual.border_color,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_ne!(tree.children[2].style.visual.opacity, Some(0.75));
        assert_eq!(
            tree.children[3].style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_ne!(tree.children[3].style.visual.border_width, Some(8.0));
    }

    #[test]
    fn stylesheet_cascade_applies_data_backed_stateful_has_and_nth_filters() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "panel",
                "type": "panel",
                "children": [
                    {"id": "enabled-check", "type": "checkbox", "props": {"label": "Enabled", "checked": true}},
                    {"id": "advanced", "type": "collapsible", "props": {"title": "Advanced", "expanded": false}},
                    {"id": "plain-check", "type": "checkbox", "props": {"label": "Plain", "checked": false}}
                ]
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel:has(> Checkbox:checked) { border-width: 4px; }
                Panel:has(> Collapsible:collapsed) { border-color: warning; }
                Panel > *:nth-child(1 of Checkbox:checked) { background: success; }
                Panel > *:nth-child(1 of Collapsible:collapsed) { border-radius: 7px; }
                Panel > *:nth-child(2 of Checkbox:checked, Collapsible:collapsed) { opacity: 0.8; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);

        let panel = &tree.children[0];
        let checked = &panel.children[0];
        let collapsed = &panel.children[1];
        let unchecked = &panel.children[2];

        assert_eq!(panel.style.visual.border_width, Some(4.0));
        assert_eq!(
            panel.style.visual.border_color,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_eq!(
            checked.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(collapsed.style.visual.border_radius, Some(7.0));
        assert_eq!(collapsed.style.visual.opacity, Some(0.8));
        assert_ne!(
            unchecked.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
    }

    #[test]
    fn stylesheet_cascade_applies_sibling_has_selectors() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [
                {"id": "before-button", "type": "panel"},
                {"id": "primary", "type": "button", "class": "primary", "props": {"text": "Run"}},
                {"id": "before-row", "type": "panel"},
                {
                    "id": "row",
                    "type": "h_layout",
                    "children": [
                        {"id": "success-child", "type": "badge", "props": {"text": "OK", "level": "success"}}
                    ]
                },
                {"id": "before-later", "type": "panel"},
                {"id": "gap", "type": "spacer"},
                {"id": "success-sibling", "type": "badge", "props": {"text": "Later", "level": "success"}},
                {"id": "after-success", "type": "panel"}
            ]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel:has(+ Button.primary) { border-width: 4px; }
                Panel:has(+ HLayout > Badge[level="success"]) { background: success; }
                Panel:has(~ Badge[level="success"]) { border-color: success; }
                Panel:has(+ Badge[level="success"]) { opacity: 0.75; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);

        let before_button = &tree.children[0];
        let before_row = &tree.children[2];
        let before_later = &tree.children[4];
        let after_success = &tree.children[7];

        assert_eq!(before_button.style.visual.border_width, Some(4.0));
        assert_eq!(
            before_button.style.visual.border_color,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(
            before_row.style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(
            before_row.style.visual.border_color,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(
            before_later.style.visual.border_color,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_ne!(before_later.style.visual.opacity, Some(0.75));
        assert_ne!(
            after_success.style.visual.border_color,
            Some(ColorRef::Token("success".to_string()))
        );
    }

    #[test]
    fn stylesheet_cascade_applies_nested_has_target_selectors() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [
                {
                    "id": "outer",
                    "type": "panel",
                    "children": [{
                        "id": "inner-actions",
                        "type": "panel",
                        "children": [
                            {"id": "ready", "type": "badge", "props": {"text": "Ready", "level": "success"}},
                            {"id": "run", "type": "button", "class": "primary", "props": {"text": "Run"}}
                        ]
                    }]
                },
                {
                    "id": "plain",
                    "type": "panel",
                    "children": [{
                        "id": "inner-plain",
                        "type": "panel",
                        "children": [
                            {"id": "note", "type": "label", "props": {"text": "Plain"}}
                        ]
                    }]
                },
                {"id": "before-status", "type": "panel"},
                {
                    "id": "status-card",
                    "type": "panel",
                    "children": [{
                        "id": "status-row",
                        "type": "panel",
                        "children": [
                            {"id": "ok", "type": "badge", "props": {"text": "OK", "level": "success"}}
                        ]
                    }]
                }
            ]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel:has(Panel:has(Button.primary)) { border-width: 6px; }
                Panel:has(Panel:has(> Badge[level="success"]) > Button.primary) { background: success; }
                Panel:has(+ Panel:has(Badge[level="success"])) { opacity: 0.65; }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);

        assert_eq!(tree.children[0].style.visual.border_width, Some(6.0));
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_ne!(tree.children[1].style.visual.border_width, Some(6.0));
        assert_ne!(
            tree.children[1].style.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_eq!(tree.children[2].style.visual.opacity, Some(0.65));
        assert_ne!(tree.children[1].style.visual.opacity, Some(0.65));
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
    fn selector_scoped_variables_resolve_inside_same_rule_block() {
        let parsed = parse_stylesheet(
            r#"
            Panel.card {
                --card-bg: #172235;
                --card-radius: 14px;
                --card-border: rgba(116, 221, 176, 0.45);
                background: var(--card-bg);
                border-radius: var(--card-radius);
                border-color: var(--card-border);
            }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        let declarations = &parsed.rules[0].declarations;
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::Background(DgCssColor::Rgba(_)))
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderRadius(DgCssLength::LogicalPx(
                    14.0
                )))
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderColor(DgCssColor::Rgba(_)))
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
                "DataFrameTable { table-row-height: 22px; table-header-height: 26px; table-column-width: 180px; table-index-width: 72px; }",
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let table = &tree.children[0];

        assert_eq!(table.style.widget.table_row_height, Some(22.0));
        assert_eq!(table.style.widget.table_header_height, Some(26.0));
        assert_eq!(table.style.widget.table_column_width, Some(180.0));
        assert_eq!(table.style.widget.table_index_width, Some(72.0));
    }

    #[test]
    fn cascade_applies_text_area_widget_declarations() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "notes",
                "type": "text_area",
                "props": {"value": "one\ntwo", "rows": 2}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(StylesheetOrigin::User, "TextArea { text-area-rows: 5; }")
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let notes = &tree.children[0];

        assert_eq!(notes.style.widget.text_area_rows, Some(5.0));
    }

    #[test]
    fn cascade_applies_scatter_widget_declarations() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "scatter",
                "type": "scatter_3d",
                "props": {
                    "frame": {"columns": ["x", "y", "z"], "dtypes": ["f32", "f32", "f32"], "rows": 2},
                    "x": "x",
                    "y": "y",
                    "z": "z"
                }
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Scatter3D {
                    scatter-point-size: 7px;
                    scatter-point-style: square;
                    scatter-grid-visible: true;
                    scatter-grid-planes: all;
                    scatter-legend-position: bottom-left;
                    scatter-orientation-axes: true;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let scatter = &tree.children[0];

        assert_eq!(scatter.style.widget.scatter_point_size, Some(7.0));
        assert_eq!(
            scatter.style.widget.scatter_point_style.as_deref(),
            Some("square")
        );
        assert_eq!(scatter.style.widget.scatter_grid_visible, Some(true));
        assert_eq!(scatter.style.widget.scatter_grid_planes, Some((true, true)));
        assert_eq!(
            scatter.style.widget.scatter_legend_position.as_deref(),
            Some("bottom-left")
        );
        assert_eq!(
            scatter.style.widget.scatter_orientation_axes_visible,
            Some(true)
        );
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
    fn border_style_longhand_accepts_solid() {
        let parsed =
            parse_stylesheet("Button { border-style: solid; }", StylesheetOrigin::User).unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(matches!(
            parsed.rules[0].declarations[0].property,
            DgStyleProperty::Visual(DgVisualDeclaration::BorderStyle(DgBorderStyle::Solid))
        ));

        let mut style = NodeStyle::default();
        style.visual.border_width = Some(2.0);
        style.visual.border_color = Some(ColorRef::Token("accent".to_string()));
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);

        assert_eq!(style.visual.border_width, Some(2.0));
        assert_eq!(
            style.visual.border_color,
            Some(ColorRef::Token("accent".to_string()))
        );
    }

    #[test]
    fn border_style_longhand_none_and_hidden_reset_border() {
        for keyword in ["none", "hidden"] {
            let parsed = parse_stylesheet(
                format!("Button {{ border-style: {keyword}; }}").as_str(),
                StylesheetOrigin::User,
            )
            .unwrap();

            assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
            assert!(matches!(
                parsed.rules[0].declarations[0].property,
                DgStyleProperty::Visual(DgVisualDeclaration::BorderStyle(DgBorderStyle::None))
            ));

            let mut style = NodeStyle::default();
            style.visual.border_width = Some(2.0);
            style.visual.border_color = Some(ColorRef::Token("accent".to_string()));
            apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);

            assert_eq!(style.visual.border_width, Some(0.0));
            assert_eq!(
                style.visual.border_color,
                Some(ColorRef::Rgba([0.0, 0.0, 0.0, 0.0]))
            );
        }
    }

    #[test]
    fn border_style_longhand_reports_unsupported_styles() {
        let parsed =
            parse_stylesheet("Button { border-style: dashed; }", StylesheetOrigin::User).unwrap();

        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.property == "border-style"
                && warning.message.contains("solid, none, or hidden")));
        assert!(parsed
            .rules
            .first()
            .is_none_or(|rule| rule.declarations.is_empty()));
    }

    #[test]
    fn outline_properties_parse_to_visual_style() {
        let parsed = parse_stylesheet(
            "Button { outline: 2px solid accent; outline-offset: 3px; } Button.off { outline-style: none; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(matches!(
            parsed.rules[0].declarations[0].property,
            DgStyleProperty::Visual(DgVisualDeclaration::Outline(DgBorder {
                width: DgCssLength::LogicalPx(2.0),
                style: DgBorderStyle::Solid,
                color: DgCssColor::Token(ref token),
            })) if token == "accent"
        ));

        let mut style = NodeStyle::default();
        for declaration in &parsed.rules[0].declarations {
            apply_property_to_style(&mut style, &declaration.property);
        }
        assert_eq!(style.visual.outline_width, Some(2.0));
        assert_eq!(style.visual.outline_offset, Some(3.0));
        assert_eq!(
            style.visual.outline_color,
            Some(ColorRef::Token("accent".to_string()))
        );

        apply_property_to_style(&mut style, &parsed.rules[1].declarations[0].property);
        assert_eq!(style.visual.outline_width, Some(0.0));
        assert_eq!(
            style.visual.outline_color,
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
    fn backdrop_filter_blur_parses_to_visual_style() {
        let parsed = parse_stylesheet(
            "Panel.glass { backdrop-filter: blur(14px) brightness(115%) saturate(0.8); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(matches!(
            parsed.rules[0].declarations[0].property,
            DgStyleProperty::Visual(DgVisualDeclaration::BackdropFilter(Some(filter)))
                if (filter.blur - 14.0).abs() < 0.001
                    && (filter.brightness - 1.15).abs() < 0.001
                    && (filter.saturate - 0.8).abs() < 0.001
        ));

        let mut style = NodeStyle::default();
        apply_property_to_style(&mut style, &parsed.rules[0].declarations[0].property);
        let filter = style.visual.backdrop_filter.unwrap();
        assert_eq!(filter.blur, 14.0);
        assert!((filter.brightness - 1.15).abs() < 0.001);
        assert!((filter.saturate - 0.8).abs() < 0.001);
    }

    #[test]
    fn generated_before_after_content_cascades_to_part_styles() {
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
                Button::before {
                    content: ">";
                    width: 16px;
                    color: accent;
                    font-weight: 700;
                }
                Button::after {
                    content: "NEW";
                    text-align: right;
                    color: success;
                }
                Badge::after {
                    content: attr(level);
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let button = &tree.children[0];
        let before = button.style.parts.parts.get("before").unwrap();
        assert_eq!(
            before.content,
            Some(GeneratedContent::Text(">".to_string()))
        );
        assert_eq!(before.layout.width, Some(16.0));
        assert_eq!(
            before.text.color,
            Some(ColorRef::Token("accent".to_string()))
        );
        let after = button.style.parts.parts.get("after").unwrap();
        assert_eq!(
            after.content,
            Some(GeneratedContent::Text("NEW".to_string()))
        );
        assert_eq!(after.text.text_align, Some(TextAlign::Right));

        let mut badge = crate::document::parse_widget_node(&serde_json::json!({
            "id": "state",
            "type": "badge",
            "props": {"text": "Ready", "level": "success"}
        }))
        .unwrap();
        apply_stylesheets_to_tree(&mut badge, &mut store);
        let generated_level = badge.style.parts.parts.get("after").unwrap();
        assert_eq!(
            generated_level.content,
            Some(GeneratedContent::Attr("level".to_string()))
        );
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
    fn flex_shorthand_sets_shrink_and_zero_basis() {
        let parsed = parse_stylesheet(
            "HLayout > TextInput { flex: 1; flex-basis: 25%; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Flex(DgCssNumber(1.0)))
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::FlexBasis(DgCssLength::Percent(25.0)))
            )
        }));

        let mut tree = css_bench_node("window", WidgetKind::Window, None);
        let mut row = css_bench_node("row", WidgetKind::HLayout, None);
        row.children
            .push(css_bench_node("input", WidgetKind::TextInput, None));
        tree.children.push(row);
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(StylesheetOrigin::User, "TextInput { flex: 1; }")
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let input = &tree.children[0].children[0];
        assert_eq!(input.style.layout.flex_grow, Some(1.0));
        assert_eq!(input.style.layout.flex_shrink, Some(1.0));
        assert_eq!(
            input.style.layout.flex_basis_value,
            Some(LayoutLength::LogicalPx(0.0))
        );
    }

    #[test]
    fn inline_alignment_survives_stylesheet_merge() {
        let mut tree = css_bench_node("window", WidgetKind::Window, None);
        let mut row = css_bench_node("row", WidgetKind::HLayout, None);
        row.inline_style.layout.align_items = Some(AlignItemsStyle::Center);
        row.style.layout.align_items = Some(AlignItemsStyle::Center);
        tree.children.push(row);
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(StylesheetOrigin::User, "HLayout { gap: 8px; }")
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let row = &tree.children[0];

        assert_eq!(row.style.layout.align_items, Some(AlignItemsStyle::Center));
        assert_eq!(row.style.layout.gap, Some(8.0));
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
    fn grid_auto_flow_values_parse() {
        let parsed = parse_stylesheet(
            "Panel.a { grid-auto-flow: dense; } Panel.b { grid-auto-flow: column dense; } Panel.c { grid-auto-flow: dense row; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridAutoFlow(
                    DgGridAutoFlow::RowDense
                ))
            )
        }));
        assert!(parsed.rules[1].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridAutoFlow(
                    DgGridAutoFlow::ColumnDense
                ))
            )
        }));
        assert!(parsed.rules[2].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridAutoFlow(
                    DgGridAutoFlow::RowDense
                ))
            )
        }));
    }

    #[test]
    fn grid_auto_flow_invalid_values_warn() {
        let parsed = parse_stylesheet(
            "Panel { grid-auto-flow: row column; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.iter().any(|warning| {
            warning.property == "grid-auto-flow" && warning.message.contains("grid-auto-flow value")
        }));
    }

    #[test]
    fn grid_template_areas_and_named_grid_area_parse() {
        let parsed = parse_stylesheet(
            r#"Panel.dashboard { display: grid; grid-template-areas: "sidebar main main" "sidebar footer footer"; } Panel.sidebar { grid-area: sidebar; }"#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateAreas(areas))
                    if areas.columns == 3
                        && areas.rows == 2
                        && areas.areas.iter().any(|area| area.name == "sidebar"
                            && area.row_start == 1
                            && area.row_end == 3
                            && area.column_start == 1
                            && area.column_end == 2)
                        && areas.areas.iter().any(|area| area.name == "main"
                            && area.row_start == 1
                            && area.row_end == 2
                            && area.column_start == 2
                            && area.column_end == 4)
            )
        }));
        assert!(parsed.rules[1].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridArea(name))
                    if name == "sidebar"
            )
        }));
    }

    #[test]
    fn grid_template_areas_reject_non_rectangular_areas() {
        let parsed = parse_stylesheet(
            r#"Panel { grid-template-areas: "a a" "a b"; }"#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.iter().any(|warning| {
            warning.property == "grid-template-areas"
                && warning.message.contains("rectangular grid area")
        }));
    }

    #[test]
    fn grid_repeat_flattens_nested_finite_repeat() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: repeat(2, repeat(2, 120px 1fr)); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::GridTemplateColumns(tracks))
                    if tracks == &vec![
                        DgGridTrackSize::LogicalPx(120.0),
                        DgGridTrackSize::Fraction(1.0),
                        DgGridTrackSize::LogicalPx(120.0),
                        DgGridTrackSize::Fraction(1.0),
                        DgGridTrackSize::LogicalPx(120.0),
                        DgGridTrackSize::Fraction(1.0),
                        DgGridTrackSize::LogicalPx(120.0),
                        DgGridTrackSize::Fraction(1.0),
                    ]
            )
        }));
    }

    #[test]
    fn grid_repeat_rejects_nested_auto_repeat() {
        let parsed = parse_stylesheet(
            "Panel { display: grid; grid-template-columns: repeat(2, repeat(auto-fit, 120px)); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.iter().any(|warning| {
            warning.property == "grid-template-columns"
                && warning.message.contains("non-nested auto-repeat")
        }));
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
    fn asymmetric_margin_and_margin_longhands_parse() {
        let parsed = parse_stylesheet(
            "Panel { margin: 10px 20px calc(4px + 1%) auto; margin-left: 12px; padding: 4px 8px; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                &declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::Margin(edges))
                    if edges.top == DgCssLength::LogicalPx(10.0)
                        && edges.right == DgCssLength::LogicalPx(20.0)
                        && edges.bottom == DgCssLength::Calc(CalcLength { percent: 1.0, px: 4.0 })
                        && edges.left == DgCssLength::Auto
            )
        }));
        assert!(parsed.rules[0].declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Layout(DgLayoutDeclaration::MarginLeft(DgCssLength::LogicalPx(
                    12.0
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
    fn stylesheet_cascade_applies_margin_edges() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "class": "card"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel.card {
                    margin: 2px 4px 6px 8px;
                    margin-left: 12%;
                    margin-bottom: auto;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let card = &tree.children[0];

        assert_eq!(
            card.style.layout.margin_top_value,
            Some(LayoutLength::LogicalPx(2.0))
        );
        assert_eq!(
            card.style.layout.margin_right_value,
            Some(LayoutLength::LogicalPx(4.0))
        );
        assert_eq!(
            card.style.layout.margin_bottom_value,
            Some(LayoutLength::Auto)
        );
        assert_eq!(
            card.style.layout.margin_left_value,
            Some(LayoutLength::Percent(12.0))
        );
        assert!(card.style.layout.margin_value.is_none());
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
                foreground: hwb(120 0% 50%);
                accent: color(srgb 1 1 1);
                color: oklab(100% 0 0);
                track-color: oklch(62% 0.18 240deg / 0.5);
                thumb-color: lch(50% 0 0 / 60%);
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
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Text(DgTextDeclaration::Color(DgCssColor::Rgba(color)))
                    if color[0] > 0.99 && color[1] > 0.99 && color[2] > 0.99 && color[3] > 0.99
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::TrackColor(DgCssColor::Rgba(color)))
                    if color[2] > color[0] && color[2] > color[1] && (color[3] - 0.5).abs() < 0.003
            )
        }));
        assert!(declarations.iter().any(|declaration| {
            matches!(
                declaration.property,
                DgStyleProperty::Visual(DgVisualDeclaration::ThumbColor(DgCssColor::Rgba(color)))
                    if (color[0] - color[1]).abs() < 0.015
                        && (color[1] - color[2]).abs() < 0.015
                        && (color[3] - 0.6).abs() < 0.003
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
        assert_eq!(parsed.rules.len(), 1);
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
    fn background_image_gradient_layers_over_background_color() {
        let parsed = parse_stylesheet(
            r#"
            :root {
                --hero-image:
                    radial-gradient(circle at 25% 20%, rgba(255,255,255,0.16), transparent 58%),
                    linear-gradient(135deg, #172235, #0f1724);
            }

            Panel.hero {
                background-color: #101820;
                background-image: var(--hero-image);
            }

            Panel.flat {
                background-image: none;
            }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.rules.len(), 2);
        let image_layers = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundImage(Some(
                    DgBackgroundPaint::Layers(layers),
                ))) => Some(layers),
                _ => None,
            })
            .expect("background-image layered paint");
        assert_eq!(image_layers.len(), 2);
        assert!(matches!(
            image_layers[0],
            DgBackgroundPaint::RadialGradient(_)
        ));
        assert!(matches!(
            image_layers[1],
            DgBackgroundPaint::LinearGradient(_)
        ));
        assert!(matches!(
            parsed.rules[1].declarations[0].property,
            DgStyleProperty::Visual(DgVisualDeclaration::BackgroundImage(None))
        ));

        let mut style = NodeStyle::default();
        for declaration in &parsed.rules[0].declarations {
            apply_property_to_style(&mut style, &declaration.property);
        }
        let Some(BackgroundPaint::Layers(resolved_layers)) = &style.visual.background_paint else {
            panic!("background-image should compose with background-color");
        };
        assert_eq!(resolved_layers.len(), 3);
        assert!(matches!(
            resolved_layers[0],
            BackgroundPaint::RadialGradient(_)
        ));
        assert!(matches!(
            resolved_layers[1],
            BackgroundPaint::LinearGradient(_)
        ));
        assert!(matches!(resolved_layers[2], BackgroundPaint::Color(_)));
    }

    #[test]
    fn background_image_none_clears_to_background_color() {
        let parsed = parse_stylesheet(
            r#"
            Panel.hero {
                background-color: #101820;
                background-image: linear-gradient(180deg, rgba(255,255,255,0.2), transparent);
                background-image: none;
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

        assert!(matches!(
            style.visual.background_paint,
            Some(BackgroundPaint::Color(_))
        ));
    }

    #[test]
    fn background_image_rejects_url_sources() {
        let parsed = parse_stylesheet(
            r#"Panel.hero { background-image: url("hero.png"); }"#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(parsed.warnings[0].property, "background-image");
        assert!(parsed.warnings[0].message.contains("linear-gradient"));
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
    fn gradient_interpolation_parses_to_visual_style() {
        let parsed = parse_stylesheet(
            "Panel.a { gradient-interpolation: oklab; } Panel.b { gradient-interpolation: linear-srgb; }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);

        let mut oklab = NodeStyle::default();
        apply_property_to_style(&mut oklab, &parsed.rules[0].declarations[0].property);
        assert_eq!(
            oklab.visual.gradient_interpolation,
            Some(GradientInterpolation::Oklab)
        );

        let mut linear = NodeStyle::default();
        apply_property_to_style(&mut linear, &parsed.rules[1].declarations[0].property);
        assert_eq!(
            linear.visual.gradient_interpolation,
            Some(GradientInterpolation::LinearSrgb)
        );
    }

    #[test]
    fn blob_gradient_background_parses_to_background_paint() {
        let parsed = parse_stylesheet(
            "Panel.hero { background: blob-gradient(at 20% 30% rgba(90,169,255,0.5) 42%, at 82% 38% #ff6584 36%); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        let gradient = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundPaint(
                    DgBackgroundPaint::BlobGradient(gradient),
                )) => Some(gradient),
                _ => None,
            })
            .expect("blob gradient background paint");

        assert_eq!(gradient.blobs.len(), 2);
        assert_eq!(gradient.blobs[0].center, [0.2, 0.3]);
        assert_eq!(gradient.blobs[0].radius, 0.42);
        assert_eq!(gradient.blobs[1].center, [0.82, 0.38]);
        assert_eq!(gradient.blobs[1].radius, 0.36);
    }

    #[test]
    fn mesh_gradient_background_parses_to_background_paint() {
        let parsed = parse_stylesheet(
            "Panel.hero { background: mesh-gradient(#2337aa, #c24f8a, #2b9f8d, #0a0f1b); }",
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        let gradient = parsed.rules[0]
            .declarations
            .iter()
            .find_map(|declaration| match &declaration.property {
                DgStyleProperty::Visual(DgVisualDeclaration::BackgroundPaint(
                    DgBackgroundPaint::MeshGradient(gradient),
                )) => Some(gradient),
                _ => None,
            })
            .expect("mesh gradient background paint");

        assert!(matches!(gradient.top_left, DgCssColor::Rgba(_)));
        assert!(matches!(gradient.top_right, DgCssColor::Rgba(_)));
        assert!(matches!(gradient.bottom_left, DgCssColor::Rgba(_)));
        assert!(matches!(gradient.bottom_right, DgCssColor::Rgba(_)));
    }

    #[test]
    fn transition_declarations_parse_to_transition_style() {
        let parsed = parse_stylesheet(
            r#"
            Button {
                transition-property: background, border-color, outline, outline-offset;
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
                TransitionProperty::Outline,
                TransitionProperty::OutlineOffset,
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
    fn transition_and_animation_timing_parse_steps() {
        let parsed = parse_stylesheet(
            r#"
            Button {
                transition-timing-function: steps(4, start);
                animation-timing-function: step-end;
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
            style.transition.timing_function,
            Some(TransitionTimingFunction::Steps {
                count: 4,
                position: StepPosition::Start,
            })
        );
        assert_eq!(
            style.animation.timing_function,
            Some(TransitionTimingFunction::Steps {
                count: 1,
                position: StepPosition::End,
            })
        );
    }

    #[test]
    fn animation_iteration_count_preserves_fractional_values() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "pulse",
                "type": "badge",
                "class": "longhand",
                "props": {"text": "LIVE"}
            }, {
                "id": "pulse-shorthand",
                "type": "badge",
                "class": "shorthand",
                "props": {"text": "LIVE"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                "Badge { animation-iteration-count: 2.5; }",
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());
        apply_stylesheets_to_tree(&mut tree, &mut store);
        let badge = tree.children.first().unwrap();
        assert_eq!(
            badge.style.animation.iteration_count,
            Some(AnimationIterationCount::Count(2.5))
        );
    }

    #[test]
    fn animation_delay_accepts_negative_values() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "pulse",
                "type": "badge",
                "class": "longhand",
                "props": {"text": "LIVE"}
            }, {
                "id": "pulse-shorthand",
                "type": "badge",
                "class": "shorthand",
                "props": {"text": "LIVE"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Badge.longhand { animation-delay: -250ms; }
                Badge.shorthand {
                    animation: 900ms -0.45s ease-out 2.5 alternate both live-pulse;
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());
        apply_stylesheets_to_tree(&mut tree, &mut store);
        let longhand = &tree.children[0];
        let shorthand = &tree.children[1];
        assert_eq!(longhand.style.animation.delay_ms, Some(-250));
        assert_eq!(shorthand.style.animation.delay_ms, Some(-450));
        assert_eq!(shorthand.style.animation.duration_ms, Some(900));
        assert_eq!(
            shorthand.style.animation.iteration_count,
            Some(AnimationIterationCount::Count(2.5))
        );
    }

    #[test]
    fn keyframes_and_animation_longhands_parse_to_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "pulse",
                "type": "badge",
                "props": {"text": "LIVE"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                @keyframes breathe {
                    from { opacity: 0.52; transform: scale(0.98); }
                    50% { opacity: 1; }
                    to { opacity: 0.72; transform: scale(1.04); }
                }

                Badge {
                    animation-name: breathe;
                    animation-duration: 1200ms;
                    animation-timing-function: cubic-bezier(0.16, 1, 0.3, 1);
                    animation-iteration-count: infinite;
                    animation-direction: alternate;
                    animation-fill-mode: both;
                    animation-play-state: paused;
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());
        let keyframes = store.keyframes();
        let breathe = keyframes.get("breathe").expect("breathe keyframes");
        assert_eq!(breathe.frames.len(), 3);
        assert_eq!(breathe.frames[0].offset, 0.0);
        assert_eq!(breathe.frames[1].offset, 0.5);
        assert_eq!(breathe.frames[2].offset, 1.0);

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let badge = tree.children.first().unwrap();
        assert_eq!(badge.style.animation.name.as_deref(), Some("breathe"));
        assert_eq!(badge.style.animation.duration_ms, Some(1200));
        assert_eq!(
            badge.style.animation.iteration_count,
            Some(AnimationIterationCount::Infinite)
        );
        assert_eq!(
            badge.style.animation.direction,
            Some(AnimationDirection::Alternate)
        );
        assert_eq!(
            badge.style.animation.fill_mode,
            Some(AnimationFillMode::Both)
        );
        assert_eq!(
            badge.style.animation.play_state,
            Some(AnimationPlayState::Paused)
        );
    }

    #[test]
    fn animation_longhands_accept_first_comma_list_item() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "pulse",
                "type": "badge",
                "props": {"text": "LIVE"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                @keyframes breathe {
                    from { opacity: 0.52; }
                    to { opacity: 1; }
                }

                Badge {
                    animation-name: breathe, ignored;
                    animation-duration: 1200ms, 250ms;
                    animation-timing-function: ease-in, linear;
                    animation-delay: 80ms, 0s;
                    animation-iteration-count: infinite, 1;
                    animation-direction: alternate-reverse, normal;
                    animation-fill-mode: backwards, none;
                    animation-play-state: paused, running;
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());
        apply_stylesheets_to_tree(&mut tree, &mut store);
        let badge = tree.children.first().unwrap();
        assert_eq!(badge.style.animation.name.as_deref(), Some("breathe"));
        assert_eq!(badge.style.animation.duration_ms, Some(1200));
        assert_eq!(badge.style.animation.delay_ms, Some(80));
        assert_eq!(
            badge.style.animation.timing_function,
            Some(TransitionTimingFunction::EaseIn)
        );
        assert_eq!(
            badge.style.animation.iteration_count,
            Some(AnimationIterationCount::Infinite)
        );
        assert_eq!(
            badge.style.animation.direction,
            Some(AnimationDirection::AlternateReverse)
        );
        assert_eq!(
            badge.style.animation.fill_mode,
            Some(AnimationFillMode::Backwards)
        );
        assert_eq!(
            badge.style.animation.play_state,
            Some(AnimationPlayState::Paused)
        );
    }

    #[test]
    fn animation_shorthand_parses_to_animation_style() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "pulse",
                "type": "badge",
                "props": {"text": "LIVE"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                @keyframes breathe {
                    from { opacity: 0.52; }
                    to { opacity: 1; }
                }

                Badge {
                    animation: 1.4s cubic-bezier(0.16, 1, 0.3, 1) 120ms infinite alternate both paused breathe;
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());
        apply_stylesheets_to_tree(&mut tree, &mut store);
        let badge = tree.children.first().unwrap();
        assert_eq!(badge.style.animation.name.as_deref(), Some("breathe"));
        assert_eq!(badge.style.animation.duration_ms, Some(1400));
        assert_eq!(badge.style.animation.delay_ms, Some(120));
        assert_eq!(
            badge.style.animation.timing_function,
            Some(TransitionTimingFunction::CubicBezier {
                x1: 0.16,
                y1: 1.0,
                x2: 0.3,
                y2: 1.0,
            })
        );
        assert_eq!(
            badge.style.animation.iteration_count,
            Some(AnimationIterationCount::Infinite)
        );
        assert_eq!(
            badge.style.animation.direction,
            Some(AnimationDirection::Alternate)
        );
        assert_eq!(
            badge.style.animation.fill_mode,
            Some(AnimationFillMode::Both)
        );
        assert_eq!(
            badge.style.animation.play_state,
            Some(AnimationPlayState::Paused)
        );
    }

    #[test]
    fn font_face_rules_collect_local_url_sources() {
        let store = parse_stylesheet(
            r#"
            @font-face {
                font-family: "Dragon Demo UI";
                src: local("Segoe UI"),
                     url("C:/Windows/Fonts/segoeui.ttf") format("truetype"),
                     url("data:font/ttf;base64,AAEAAA==") format("truetype");
            }
            Label.title { font-family: "Dragon Demo UI"; }
            "#,
            StylesheetOrigin::User,
        )
        .unwrap();

        assert!(store.warnings.is_empty(), "{:?}", store.warnings);
        assert_eq!(store.font_faces.len(), 1);
        assert_eq!(store.font_faces[0].family, "Dragon Demo UI");
        assert_eq!(store.font_faces[0].sources.len(), 3);
        assert_eq!(
            store.font_faces[0].sources[0].kind,
            DgFontFaceSourceKind::Local
        );
        assert_eq!(store.font_faces[0].sources[0].url, "Segoe UI");
        assert_eq!(
            store.font_faces[0].sources[1].url,
            "C:/Windows/Fonts/segoeui.ttf"
        );
        assert_eq!(
            store.font_faces[0].sources[1].kind,
            DgFontFaceSourceKind::Url
        );
        assert_eq!(
            store.font_faces[0].sources[1].format.as_deref(),
            Some("truetype")
        );
        assert_eq!(
            store.font_faces[0].sources[2].url,
            "data:font/ttf;base64,AAEAAA=="
        );
        assert_eq!(
            store.font_faces[0].sources[2].kind,
            DgFontFaceSourceKind::Url
        );
        assert_eq!(
            store.font_faces[0].sources[2].format.as_deref(),
            Some("truetype")
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
    fn container_rules_match_named_inline_size_ancestor_width() {
        let css = r#"
            Panel.card {
                container-name: card;
                container-type: inline-size;
            }
            Label.item { width: 100px; }
            @container card (min-width: 300px) {
                Label.item { width: 180px; }
            }
        "#;
        let media = DgMediaEnvironment::new(800.0, 600.0);
        let mut store = StylesheetStore::default();
        store.set_stylesheet(StylesheetOrigin::User, css).unwrap();

        let build_tree = || {
            crate::document::parse_widget_node(&serde_json::json!({
                "id": "root",
                "type": "window",
                "children": [{
                    "id": "card",
                    "type": "panel",
                    "class": "card",
                    "children": [{
                        "id": "item",
                        "type": "label",
                        "class": "item",
                        "props": {"text": "Container query"}
                    }]
                }]
            }))
            .unwrap()
        };

        let mut tree = build_tree();
        apply_stylesheets_to_tree_for_media_and_containers(&mut tree, &mut store, media, None);
        assert_eq!(tree.children[0].children[0].style.layout.width, Some(100.0));

        let mut wide_context = DgContainerQueryContext::new();
        wide_context.insert_width("card", 320.0);
        let mut tree = build_tree();
        apply_stylesheets_to_tree_for_media_and_containers(
            &mut tree,
            &mut store,
            media,
            Some(&wide_context),
        );
        assert_eq!(tree.children[0].children[0].style.layout.width, Some(180.0));

        let mut narrow_context = DgContainerQueryContext::new();
        narrow_context.insert_width("card", 260.0);
        let mut tree = build_tree();
        apply_stylesheets_to_tree_for_media_and_containers(
            &mut tree,
            &mut store,
            media,
            Some(&narrow_context),
        );
        assert_eq!(tree.children[0].children[0].style.layout.width, Some(100.0));
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
    fn media_rules_support_viewport_aspect_ratio() {
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
                @media (min-aspect-ratio: 3/2) {
                    Label { font-size: 18px; }
                }
                @media (aspect-ratio <= 1/1) {
                    Label { color: accent; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

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
    }

    #[test]
    fn media_rules_support_viewport_resolution() {
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
                @media (min-resolution: 2dppx) {
                    Label { font-size: 18px; }
                }
                @media (resolution >= 192dpi) {
                    Label { color: accent; }
                }
                @media (-webkit-device-pixel-ratio: 2) {
                    Label { border-width: 2px; }
                }
                @media (-moz-device-pixel-ratio >= 2) {
                    Label { border-radius: 8px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_resolution(900.0, 600.0, 1.0),
        );
        assert_eq!(tree.children[0].style.text.font_size, Some(12.0));
        assert_eq!(
            tree.children[0].style.text.color,
            Some(ColorRef::Rgba([1.0, 1.0, 1.0, 1.0]))
        );
        assert_ne!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_ne!(tree.children[0].style.visual.border_radius, Some(8.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_resolution(900.0, 600.0, 2.0),
        );
        assert_eq!(tree.children[0].style.text.font_size, Some(18.0));
        assert_eq!(
            tree.children[0].style.text.color,
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(8.0));
    }

    #[test]
    fn media_rules_support_reduced_motion_preference() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "pulse",
                "type": "badge",
                "props": {"text": "Live"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Badge {
                    animation-name: pulse;
                    animation-play-state: running;
                }
                @media (prefers-reduced-motion: reduce) {
                    Badge { animation-play-state: paused; }
                }
                @media (prefers-reduced-motion: no-preference) {
                    Badge { border-width: 2px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_preferences(900.0, 600.0, 1.0, false),
        );
        assert_eq!(
            tree.children[0].style.animation.play_state,
            Some(AnimationPlayState::Running)
        );
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_preferences(900.0, 600.0, 1.0, true),
        );
        assert_eq!(
            tree.children[0].style.animation.play_state,
            Some(AnimationPlayState::Paused)
        );
        assert_ne!(tree.children[0].style.visual.border_width, Some(2.0));
    }

    #[test]
    fn media_rules_support_reduced_transparency_and_data_preferences() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel {
                    opacity: 0.5;
                    border-width: 1px;
                }
                @media (prefers-reduced-transparency: no-preference) {
                    Panel { opacity: 0.8; }
                }
                @media (prefers-reduced-data: no-preference) {
                    Panel { border-width: 2px; }
                }
                @media (prefers-reduced-transparency) {
                    Panel { background: danger; }
                }
                @media (prefers-reduced-data: reduce) {
                    Panel { border-radius: 12px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(900.0, 600.0),
        );
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_ne!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
        assert_ne!(tree.children[0].style.visual.border_radius, Some(12.0));

        let mut reduced = DgMediaEnvironment::new(900.0, 600.0);
        reduced.prefers_reduced_transparency = true;
        reduced.prefers_reduced_data = true;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, reduced);
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.5));
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
    }

    #[test]
    fn media_rules_support_color_scheme_preference() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel { border-width: 1px; }
                @media (prefers-color-scheme: dark) {
                    Panel { background: #101820; }
                }
                @media (prefers-color-scheme: light) {
                    Panel { background: #f5f7fb; border-width: 3px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_color_scheme(
                900.0,
                600.0,
                1.0,
                DgMediaColorGamut::Srgb,
                DgMediaPointer::Fine,
                DgMediaPointer::Fine,
                DgMediaHover::Hover,
                DgMediaHover::Hover,
                false,
                DgMediaColorScheme::Dark,
            ),
        );
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Rgba([
                0x10 as f32 / 255.0,
                0x18 as f32 / 255.0,
                0x20 as f32 / 255.0,
                1.0
            ]))
        );
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_color_scheme(
                900.0,
                600.0,
                1.0,
                DgMediaColorGamut::Srgb,
                DgMediaPointer::Fine,
                DgMediaPointer::Fine,
                DgMediaHover::Hover,
                DgMediaHover::Hover,
                false,
                DgMediaColorScheme::Light,
            ),
        );
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Rgba([
                0xf5 as f32 / 255.0,
                0xf7 as f32 / 255.0,
                0xfb as f32 / 255.0,
                1.0
            ]))
        );
        assert_eq!(tree.children[0].style.visual.border_width, Some(3.0));
    }

    #[test]
    fn media_rules_support_color_gamut() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel { border-width: 1px; }
                @media (color-gamut: srgb) {
                    Panel { border-width: 2px; }
                }
                @media (color-gamut: p3) {
                    Panel { border-radius: 12px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_color_scheme(
                900.0,
                600.0,
                1.0,
                DgMediaColorGamut::Srgb,
                DgMediaPointer::Fine,
                DgMediaPointer::Fine,
                DgMediaHover::Hover,
                DgMediaHover::Hover,
                false,
                DgMediaColorScheme::Dark,
            ),
        );
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_ne!(tree.children[0].style.visual.border_radius, Some(12.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_color_scheme(
                900.0,
                600.0,
                1.0,
                DgMediaColorGamut::P3,
                DgMediaPointer::Fine,
                DgMediaPointer::Fine,
                DgMediaHover::Hover,
                DgMediaHover::Hover,
                false,
                DgMediaColorScheme::Dark,
            ),
        );
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
    }

    #[test]
    fn media_rules_support_pointer_and_hover_capabilities() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "button",
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
                Button { border-width: 1px; }
                @media (pointer: fine) and (hover: hover) {
                    Button { border-width: 2px; }
                }
                @media (any-pointer: coarse) {
                    Button { border-radius: 14px; }
                }
                @media (any-hover: none) {
                    Button { opacity: 0.7; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_color_scheme(
                900.0,
                600.0,
                1.0,
                DgMediaColorGamut::Srgb,
                DgMediaPointer::Fine,
                DgMediaPointer::Fine,
                DgMediaHover::Hover,
                DgMediaHover::Hover,
                false,
                DgMediaColorScheme::Dark,
            ),
        );
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_ne!(tree.children[0].style.visual.border_radius, Some(14.0));
        assert_ne!(tree.children[0].style.visual.opacity, Some(0.7));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::with_color_scheme(
                900.0,
                600.0,
                1.0,
                DgMediaColorGamut::Srgb,
                DgMediaPointer::Coarse,
                DgMediaPointer::Coarse,
                DgMediaHover::None,
                DgMediaHover::None,
                false,
                DgMediaColorScheme::Dark,
            ),
        );
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(14.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.7));
    }

    #[test]
    fn media_rules_support_update_capability() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "badge",
                "type": "badge",
                "props": {"text": "Live"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Badge { border-width: 1px; opacity: 0.8; }
                @media (update: fast) {
                    Badge { border-width: 3px; }
                }
                @media (update) {
                    Badge { border-radius: 8px; }
                }
                @media (update: slow) {
                    Badge { opacity: 0.5; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut fast = DgMediaEnvironment::new(900.0, 600.0);
        fast.update = DgMediaUpdate::Fast;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, fast);
        assert_eq!(tree.children[0].style.visual.border_width, Some(3.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(8.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));

        let mut slow = DgMediaEnvironment::new(900.0, 600.0);
        slow.update = DgMediaUpdate::Slow;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, slow);
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(8.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.5));
    }

    #[test]
    fn media_rules_support_scripting_capability() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "badge",
                "type": "badge",
                "props": {"text": "Native"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Badge { border-width: 1px; opacity: 0.8; }
                @media (scripting: none) {
                    Badge { border-width: 3px; }
                }
                @media (scripting) {
                    Badge { opacity: 0.5; }
                }
                @media (scripting: enabled) {
                    Badge { border-radius: 12px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut none = DgMediaEnvironment::new(900.0, 600.0);
        none.scripting = DgMediaScripting::None;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, none);
        assert_eq!(tree.children[0].style.visual.border_width, Some(3.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_ne!(tree.children[0].style.visual.border_radius, Some(12.0));

        let mut enabled = DgMediaEnvironment::new(900.0, 600.0);
        enabled.scripting = DgMediaScripting::Enabled;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, enabled);
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.5));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
    }

    #[test]
    fn media_rules_support_forced_colors_capability() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "button",
                "type": "button",
                "props": {"text": "Export"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { border-width: 1px; opacity: 0.8; }
                @media (forced-colors: none) {
                    Button { border-width: 3px; }
                }
                @media (forced-colors) {
                    Button { opacity: 0.5; }
                }
                @media (forced-colors: active) {
                    Button { border-radius: 12px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut none = DgMediaEnvironment::new(900.0, 600.0);
        none.forced_colors = DgMediaForcedColors::None;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, none);
        assert_eq!(tree.children[0].style.visual.border_width, Some(3.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_ne!(tree.children[0].style.visual.border_radius, Some(12.0));

        let mut active = DgMediaEnvironment::new(900.0, 600.0);
        active.forced_colors = DgMediaForcedColors::Active;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, active);
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.5));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
    }

    #[test]
    fn media_rules_support_contrast_preference() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel { border-width: 1px; opacity: 0.8; }
                @media (prefers-contrast: no-preference) {
                    Panel { border-width: 2px; }
                }
                @media (prefers-contrast) {
                    Panel { opacity: 1; }
                }
                @media (prefers-contrast: more) {
                    Panel { border-radius: 12px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut no_preference = DgMediaEnvironment::new(900.0, 600.0);
        no_preference.prefers_contrast = DgMediaContrast::NoPreference;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, no_preference);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_ne!(tree.children[0].style.visual.border_radius, Some(12.0));

        let mut more = DgMediaEnvironment::new(900.0, 600.0);
        more.prefers_contrast = DgMediaContrast::More;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, more);
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(1.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
    }

    #[test]
    fn media_rules_support_inverted_colors_capability() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "button",
                "type": "button",
                "props": {"text": "Inspect"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { border-width: 1px; opacity: 0.8; }
                @media (inverted-colors: none) {
                    Button { border-width: 3px; }
                }
                @media (inverted-colors) {
                    Button { opacity: 0.5; }
                }
                @media (inverted-colors: inverted) {
                    Button { border-radius: 12px; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut none = DgMediaEnvironment::new(900.0, 600.0);
        none.inverted_colors = DgMediaInvertedColors::None;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, none);
        assert_eq!(tree.children[0].style.visual.border_width, Some(3.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_ne!(tree.children[0].style.visual.border_radius, Some(12.0));

        let mut inverted = DgMediaEnvironment::new(900.0, 600.0);
        inverted.inverted_colors = DgMediaInvertedColors::Inverted;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, inverted);
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.5));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
    }

    #[test]
    fn media_rules_support_dynamic_range_capabilities() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "button",
                "type": "button",
                "props": {"text": "Inspect"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { border-width: 1px; border-radius: 4px; opacity: 0.4; }
                @media (dynamic-range) {
                    Button { border-width: 2px; }
                }
                @media (dynamic-range: high) {
                    Button { border-radius: 12px; }
                }
                @media (video-dynamic-range: high) {
                    Button { opacity: 1; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut standard = DgMediaEnvironment::new(900.0, 600.0);
        standard.dynamic_range = DgMediaDynamicRange::Standard;
        standard.video_dynamic_range = DgMediaDynamicRange::Standard;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, standard);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(4.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.4));

        let mut high_canvas = DgMediaEnvironment::new(900.0, 600.0);
        high_canvas.dynamic_range = DgMediaDynamicRange::High;
        high_canvas.video_dynamic_range = DgMediaDynamicRange::Standard;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, high_canvas);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.4));

        let mut high_video = DgMediaEnvironment::new(900.0, 600.0);
        high_video.dynamic_range = DgMediaDynamicRange::High;
        high_video.video_dynamic_range = DgMediaDynamicRange::High;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, high_video);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(1.0));
    }

    #[test]
    fn media_rules_support_display_mode_capability() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "button",
                "type": "button",
                "props": {"text": "Inspect"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Button { border-width: 1px; border-radius: 4px; opacity: 0.4; }
                @media (display-mode) {
                    Button { border-width: 2px; }
                }
                @media (display-mode: standalone) {
                    Button { border-radius: 10px; }
                }
                @media (display-mode: fullscreen) {
                    Button { opacity: 1; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut standalone = DgMediaEnvironment::new(900.0, 600.0);
        standalone.display_mode = DgMediaDisplayMode::Standalone;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, standalone);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(10.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.4));

        let mut fullscreen = DgMediaEnvironment::new(900.0, 600.0);
        fullscreen.display_mode = DgMediaDisplayMode::Fullscreen;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, fullscreen);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(4.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(1.0));
    }

    #[test]
    fn media_rules_support_overflow_capabilities() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel { border-width: 1px; border-radius: 4px; opacity: 0.4; }
                @media (overflow-block) {
                    Panel { border-width: 2px; }
                }
                @media (overflow-block: scroll) {
                    Panel { border-radius: 10px; }
                }
                @media (overflow-inline: scroll) {
                    Panel { opacity: 0.8; }
                }
                @media (overflow-block: paged) {
                    Panel { background: danger; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut scroll = DgMediaEnvironment::new(900.0, 600.0);
        scroll.overflow_block = DgMediaOverflow::Scroll;
        scroll.overflow_inline = DgMediaOverflow::Scroll;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, scroll);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(10.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_ne!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );

        let mut paged = DgMediaEnvironment::new(900.0, 600.0);
        paged.overflow_block = DgMediaOverflow::Paged;
        paged.overflow_inline = DgMediaOverflow::None;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, paged);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(4.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.4));
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn media_rules_support_color_depth_capabilities() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel { border-width: 1px; border-radius: 4px; opacity: 0.4; }
                @media (color) {
                    Panel { border-width: 2px; }
                }
                @media (color >= 8) {
                    Panel { border-radius: 10px; }
                }
                @media (monochrome) {
                    Panel { opacity: 0.8; }
                }
                @media (color-index) {
                    Panel { background: danger; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut color = DgMediaEnvironment::new(900.0, 600.0);
        color.color_bits = 8.0;
        color.monochrome_bits = 0.0;
        color.color_index = 0.0;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, color);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(10.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.4));
        assert_ne!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );

        let mut indexed_monochrome = DgMediaEnvironment::new(900.0, 600.0);
        indexed_monochrome.color_bits = 0.0;
        indexed_monochrome.monochrome_bits = 2.0;
        indexed_monochrome.color_index = 256.0;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, indexed_monochrome);
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(4.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn media_rules_support_display_surface_capabilities() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel { border-width: 1px; border-radius: 4px; opacity: 0.4; }
                @media (scan: progressive) {
                    Panel { border-width: 2px; }
                }
                @media (environment-blending: opaque) {
                    Panel { border-radius: 10px; }
                }
                @media (grid: 0) {
                    Panel { opacity: 0.8; }
                }
                @media (grid) {
                    Panel { background: danger; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let mut normal = DgMediaEnvironment::new(900.0, 600.0);
        normal.scan = DgMediaScan::Progressive;
        normal.environment_blending = DgMediaEnvironmentBlending::Opaque;
        normal.grid = false;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, normal);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(10.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_ne!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );

        let mut grid = DgMediaEnvironment::new(900.0, 600.0);
        grid.scan = DgMediaScan::Interlace;
        grid.environment_blending = DgMediaEnvironmentBlending::Additive;
        grid.grid = true;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, grid);
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(4.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.4));
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn media_rules_support_device_size_and_viewport_segments() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel { border-width: 1px; border-radius: 4px; opacity: 0.4; }
                @media (device-width >= 900px) {
                    Panel { border-width: 2px; }
                }
                @media (device-aspect-ratio >= 4/3) {
                    Panel { border-radius: 10px; }
                }
                @media (horizontal-viewport-segments: 1) and (vertical-viewport-segments: 1) {
                    Panel { opacity: 0.8; }
                }
                @media (horizontal-viewport-segments: 2) {
                    Panel { background: danger; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let normal = DgMediaEnvironment::new(900.0, 600.0);
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, normal);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(10.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_ne!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );

        let mut folded = DgMediaEnvironment::new(700.0, 700.0);
        folded.device_width = 700.0;
        folded.device_height = 700.0;
        folded.horizontal_viewport_segments = 2.0;
        folded.vertical_viewport_segments = 1.0;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, folded);
        assert_eq!(tree.children[0].style.visual.border_width, Some(1.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(4.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.4));
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn media_rules_support_video_color_gamut_and_nav_controls() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Panel { border-width: 1px; border-radius: 4px; opacity: 0.4; }
                @media (video-color-gamut: srgb) {
                    Panel { border-width: 2px; }
                }
                @media (video-color-gamut: p3) {
                    Panel { border-radius: 10px; }
                }
                @media (nav-controls: none) {
                    Panel { opacity: 0.8; }
                }
                @media (nav-controls) {
                    Panel { background: danger; }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        let normal = DgMediaEnvironment::new(900.0, 600.0);
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, normal);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(4.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.8));
        assert_ne!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );

        let mut p3_with_back = DgMediaEnvironment::new(900.0, 600.0);
        p3_with_back.video_color_gamut = DgMediaColorGamut::P3;
        p3_with_back.nav_controls = DgMediaNavControls::Back;
        apply_stylesheets_to_tree_for_media(&mut tree, &mut store, p3_with_back);
        assert_eq!(tree.children[0].style.visual.border_width, Some(2.0));
        assert_eq!(tree.children[0].style.visual.border_radius, Some(10.0));
        assert_eq!(tree.children[0].style.visual.opacity, Some(0.4));
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );
    }

    #[test]
    fn media_scoped_root_variables_apply_to_nested_rules() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "card",
                "type": "panel",
                "props": {"title": "Card"}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                :root { --card-bg: #101820; --card-radius: 4px; }
                Panel { background: var(--card-bg); border-radius: var(--card-radius); }
                @media (min-width: 800px) {
                    :root {
                        --card-bg: #f5f7fb;
                        --card-radius: 12px;
                    }
                    Panel { background: var(--card-bg); border-radius: var(--card-radius); }
                }
                "#,
            )
            .unwrap();

        assert!(store.warnings().is_empty(), "{:?}", store.warnings());

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(600.0, 600.0),
        );
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Rgba([
                0x10 as f32 / 255.0,
                0x18 as f32 / 255.0,
                0x20 as f32 / 255.0,
                1.0
            ]))
        );
        assert_eq!(tree.children[0].style.visual.border_radius, Some(4.0));

        apply_stylesheets_to_tree_for_media(
            &mut tree,
            &mut store,
            DgMediaEnvironment::new(900.0, 600.0),
        );
        assert_eq!(
            tree.children[0].style.visual.background,
            Some(ColorRef::Rgba([
                0xf5 as f32 / 255.0,
                0xf7 as f32 / 255.0,
                0xfb as f32 / 255.0,
                1.0
            ]))
        );
        assert_eq!(tree.children[0].style.visual.border_radius, Some(12.0));
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
                @supports (backdrop-filter: blur(8px)) {
                    Button.primary { border-radius: 12px; }
                }
                @supports font-format(woff) {
                    Button.primary { opacity: 0.8; }
                }
                @supports font-format("opentype") {
                    Button.primary { outline-width: 2px; }
                }
                @supports font-format(ttf) {
                    Button.primary { outline-color: success; }
                }
                @supports font-format(woff2) {
                    Button.primary { color: danger; }
                }
                @supports at-rule(@media) {
                    Button.primary { outline-offset: 3px; }
                }
                @supports at-rule(@container) {
                    Button.primary { min-width: 240px; }
                }
                @supports font-tech(features-opentype) {
                    Button.primary { max-width: 320px; }
                }
                @supports font-tech(color-COLRv1) {
                    Button.primary { height: 80px; }
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
        assert_eq!(button.style.visual.opacity, Some(0.8));
        assert_eq!(button.style.visual.outline_width, Some(2.0));
        assert_eq!(
            button.style.visual.outline_color,
            Some(ColorRef::Token("success".to_string()))
        );
        assert_ne!(
            button.style.text.color,
            Some(ColorRef::Token("danger".to_string()))
        );
        assert_eq!(button.style.visual.outline_offset, Some(3.0));
        assert_eq!(button.style.layout.min_width, Some(240.0));
        assert_eq!(button.style.layout.max_width, Some(320.0));
        assert_ne!(button.style.layout.height, Some(80.0));
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
    fn stylesheet_cascade_applies_bar_chart_value_label_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "bars",
                "type": "bar_chart"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                BarChart::label {
                    color: muted_text;
                }
                BarChart::value-label {
                    color: warning;
                    font-size: 11px;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let chart = &tree.children[0];
        let label_style = chart.style.parts.parts.get("label").unwrap();
        let value_label_style = chart.style.parts.parts.get("value-label").unwrap();

        assert_eq!(
            label_style.text.color,
            Some(ColorRef::Token("muted_text".to_string()))
        );
        assert_eq!(
            value_label_style.text.color,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_eq!(value_label_style.text.font_size, Some(11.0));
        assert!(store.warnings().is_empty());
    }

    #[test]
    fn stylesheet_cascade_applies_menu_popup_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "file",
                "type": "menu"
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                Menu::menu {
                    background: surface_alt;
                    border-color: accent;
                }
                Menu::item {
                    color: text;
                }
                Menu::item-hover {
                    background: accent;
                }
                Menu::item-disabled {
                    color: muted_text;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let menu = &tree.children[0];

        assert_eq!(
            menu.style
                .parts
                .parts
                .get("menu")
                .and_then(|style| style.visual.background.clone()),
            Some(ColorRef::Token("surface_alt".to_string()))
        );
        assert_eq!(
            menu.style
                .parts
                .parts
                .get("item")
                .and_then(|style| style.text.color.clone()),
            Some(ColorRef::Token("text".to_string()))
        );
        assert_eq!(
            menu.style
                .parts
                .parts
                .get("item-hover")
                .and_then(|style| style.visual.background.clone()),
            Some(ColorRef::Token("accent".to_string()))
        );
        assert_eq!(
            menu.style
                .parts
                .parts
                .get("item-disabled")
                .and_then(|style| style.text.color.clone()),
            Some(ColorRef::Token("muted_text".to_string()))
        );
        assert!(store.warnings().is_empty());
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
    fn stylesheet_cascade_applies_led_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "status",
                "type": "led",
                "props": {"state": "busy", "size": 16}
            }]
        }))
        .unwrap();
        let mut store = StylesheetStore::default();
        store
            .set_stylesheet(
                StylesheetOrigin::User,
                r#"
                LED {
                    width: 20px;
                    height: 20px;
                    border-radius: 3px;
                }
                LED.busy::dot {
                    background: warning;
                    border-color: #112233;
                    border-width: 2px;
                    border-radius: 4px;
                }
                LED::glow {
                    width: 28px;
                    height: 28px;
                    background: warning;
                    opacity: 0.22;
                    box-shadow: none;
                }
                LED::highlight {
                    width: 6px;
                    height: 4px;
                    background: rgba(255, 255, 255, 0.6);
                    opacity: 0.5;
                }
                "#,
            )
            .unwrap();

        apply_stylesheets_to_tree(&mut tree, &mut store);
        let led = &tree.children[0];
        let dot = led.style.parts.parts.get("dot").unwrap();
        let glow = led.style.parts.parts.get("glow").unwrap();
        let highlight = led.style.parts.parts.get("highlight").unwrap();

        assert_eq!(led.style.layout.width, Some(20.0));
        assert_eq!(led.style.layout.height, Some(20.0));
        assert_eq!(led.style.visual.border_radius, Some(3.0));
        assert_eq!(
            dot.visual.background,
            Some(ColorRef::Token("warning".to_string()))
        );
        assert_eq!(dot.visual.border_width, Some(2.0));
        assert_eq!(dot.visual.border_radius, Some(4.0));
        assert_eq!(glow.layout.width, Some(28.0));
        assert_eq!(glow.layout.height, Some(28.0));
        assert_eq!(glow.visual.opacity, Some(0.22));
        assert_eq!(glow.visual.box_shadows, Some(Vec::new()));
        assert_eq!(highlight.layout.width, Some(6.0));
        assert_eq!(highlight.layout.height, Some(4.0));
        assert_eq!(highlight.visual.opacity, Some(0.5));
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
    fn stylesheet_cascade_applies_scrollbar_part_styles() {
        let mut tree = crate::document::parse_widget_node(&serde_json::json!({
            "id": "root",
            "type": "window",
            "children": [{
                "id": "panel",
                "type": "panel"
            }, {
                "id": "strip",
                "type": "h_layout"
            }, {
                "id": "sidebar",
                "type": "sidebar"
            }, {
                "id": "modal",
                "type": "modal"
            }, {
                "id": "collapsible",
                "type": "collapsible"
            }, {
                "id": "pages",
                "type": "pages",
                "children": [{
                    "id": "page",
                    "type": "page"
                }]
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

                HLayout::scrollbar-track {
                    width: 7px;
                    padding: 11px;
                    background: rgba(90, 169, 255, 0.18);
                }

                HLayout::scrollbar-thumb {
                    width: 9px;
                    background: success;
                }

                Sidebar::scrollbar-track {
                    width: 10px;
                    background: danger;
                }

                Modal::scrollbar-thumb {
                    width: 11px;
                    background: warning;
                }

                Modal::scrim {
                    background: rgba(10, 20, 30, 0.42);
                    opacity: 0.8;
                }

                Collapsible::scrollbar-thumb {
                    width: 12px;
                    background: accent;
                }

                Pages::scrollbar-track {
                    width: 13px;
                    background: surface_alt;
                }

                Page::scrollbar-thumb {
                    width: 14px;
                    background: text;
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

        let strip = &tree.children[1];
        let strip_track = strip.style.parts.parts.get("scrollbar-track").unwrap();
        let strip_thumb = strip.style.parts.parts.get("scrollbar-thumb").unwrap();
        assert_eq!(strip_track.layout.width, Some(7.0));
        assert_eq!(strip_track.layout.padding, Some(11.0));
        assert!(matches!(
            strip_track.visual.background,
            Some(ColorRef::Rgba([_, _, _, alpha])) if (alpha - 0.18).abs() < 0.003
        ));
        assert_eq!(strip_thumb.layout.width, Some(9.0));
        assert_eq!(
            strip_thumb.visual.background,
            Some(ColorRef::Token("success".to_string()))
        );

        let sidebar = &tree.children[2];
        let sidebar_track = sidebar.style.parts.parts.get("scrollbar-track").unwrap();
        assert_eq!(sidebar_track.layout.width, Some(10.0));
        assert_eq!(
            sidebar_track.visual.background,
            Some(ColorRef::Token("danger".to_string()))
        );

        let modal = &tree.children[3];
        let modal_thumb = modal.style.parts.parts.get("scrollbar-thumb").unwrap();
        assert_eq!(modal_thumb.layout.width, Some(11.0));
        assert_eq!(
            modal_thumb.visual.background,
            Some(ColorRef::Token("warning".to_string()))
        );
        let modal_scrim = modal.style.parts.parts.get("scrim").unwrap();
        assert!(matches!(
            modal_scrim.visual.background,
            Some(ColorRef::Rgba([_, _, _, alpha])) if (alpha - 0.42).abs() < 0.003
        ));
        assert_eq!(modal_scrim.visual.opacity, Some(0.8));

        let collapsible = &tree.children[4];
        let collapsible_thumb = collapsible
            .style
            .parts
            .parts
            .get("scrollbar-thumb")
            .unwrap();
        assert_eq!(collapsible_thumb.layout.width, Some(12.0));
        assert_eq!(
            collapsible_thumb.visual.background,
            Some(ColorRef::Token("accent".to_string()))
        );

        let pages = &tree.children[5];
        let pages_track = pages.style.parts.parts.get("scrollbar-track").unwrap();
        assert_eq!(pages_track.layout.width, Some(13.0));
        assert_eq!(
            pages_track.visual.background,
            Some(ColorRef::Token("surface_alt".to_string()))
        );

        let page = &pages.children[0];
        let page_thumb = page.style.parts.parts.get("scrollbar-thumb").unwrap();
        assert_eq!(page_thumb.layout.width, Some(14.0));
        assert_eq!(
            page_thumb.visual.background,
            Some(ColorRef::Token("text".to_string()))
        );
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
