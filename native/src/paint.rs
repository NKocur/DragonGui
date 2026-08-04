use crate::document::WidgetKind;
use crate::theme::{Color, Theme};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct NativePaintFallback {
    pub background: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: Option<f32>,
    pub border_radius: Option<f32>,
    pub accent: Option<Color>,
    pub track_color: Option<Color>,
    pub thumb_color: Option<Color>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum PaintInteraction {
    #[default]
    Resting,
    Hovered,
    Focused,
    Pressed,
    Disabled,
}

pub(crate) fn native_widget_paint_fallback(
    kind: WidgetKind,
    theme: &Theme,
    interaction: PaintInteraction,
) -> NativePaintFallback {
    use WidgetKind::*;

    let mut fallback = match kind {
        Panel => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(theme.radius * 0.5),
            ..NativePaintFallback::default()
        },
        Collapsible => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        Modal => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        Tooltip => NativePaintFallback {
            background: Some(theme.surface_alt),
            border_color: Some(mix(theme.border, theme.accent, 0.18)),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        Toast => NativePaintFallback {
            background: Some(mix(theme.surface, theme.accent, 0.18)),
            border_color: Some(mix(theme.border, theme.accent, 0.62)),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        Sidebar | StatusBar | MenuBar => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        Button | SmallButton | IconButton | ImageButton | ArrowButton | Dropdown => {
            NativePaintFallback {
                background: Some(theme.surface_alt),
                border_color: Some(theme.border),
                border_width: Some(1.0),
                border_radius: Some(theme.radius),
                ..NativePaintFallback::default()
            }
        }
        Selectable | RadioButton => NativePaintFallback {
            background: Some([0.0, 0.0, 0.0, 0.0]),
            border_color: Some([0.0, 0.0, 0.0, 0.0]),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        TreeView => NativePaintFallback {
            background: Some([0.0, 0.0, 0.0, 0.0]),
            ..NativePaintFallback::default()
        },
        Separator => NativePaintFallback {
            background: Some(theme.border),
            border_radius: Some(0.0),
            ..NativePaintFallback::default()
        },
        TreeNode => NativePaintFallback {
            background: Some([0.0, 0.0, 0.0, 0.0]),
            border_color: Some([0.0, 0.0, 0.0, 0.0]),
            border_width: Some(1.0),
            border_radius: Some(2.0),
            accent: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        Tab => NativePaintFallback {
            border_radius: Some(theme.radius),
            accent: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        NavItem => NativePaintFallback {
            border_radius: Some(2.0),
            accent: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        Badge | Tag => NativePaintFallback {
            border_radius: Some(999.0),
            ..NativePaintFallback::default()
        },
        Histogram | BarChart | Heatmap | PieChart | LinePlot => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        Scatter3D => NativePaintFallback {
            background: Some([0.0, 0.0, 0.0, 0.0]),
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        DataFrameTable => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        TextInput | TextArea | CodeEditor | LogView | NumberInput | DragNumber => {
            NativePaintFallback {
                background: Some(mix(theme.surface, theme.surface_alt, 0.55)),
                border_color: Some(theme.border),
                border_width: Some(1.0),
                border_radius: Some(theme.radius),
                ..NativePaintFallback::default()
            }
        }
        Slider | RangeSlider => NativePaintFallback {
            accent: Some(theme.accent),
            track_color: Some(theme.border),
            thumb_color: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        ProgressBar | LimitsBar => NativePaintFallback {
            background: Some(mix(theme.surface, theme.surface_alt, 0.60)),
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            accent: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        LoadingSpinner => NativePaintFallback {
            accent: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        Checkbox | ToggleSwitch => NativePaintFallback {
            border_radius: Some(theme.radius),
            accent: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        _ => NativePaintFallback::default(),
    };

    if matches!(
        kind,
        Button | SmallButton | IconButton | ImageButton | ArrowButton | Dropdown
    ) {
        match interaction {
            PaintInteraction::Resting => {}
            PaintInteraction::Hovered => {
                fallback.background = Some(mix(theme.surface_alt, theme.accent, 0.20));
                fallback.border_color = Some(mix(theme.border, theme.accent, 0.35));
            }
            PaintInteraction::Focused => {
                fallback.background = Some(mix(theme.surface_alt, theme.accent, 0.20));
                fallback.border_color = Some(theme.accent);
            }
            PaintInteraction::Pressed => {
                fallback.background = Some(darken(theme.accent, 0.15));
                fallback.border_color = Some(darken(theme.accent, 0.08));
            }
            PaintInteraction::Disabled => {
                fallback.background = Some(mix(theme.surface_alt, theme.disabled, 0.28));
                fallback.border_color = Some(mix(theme.border, theme.disabled, 0.45));
            }
        }
    }

    if matches!(
        kind,
        TextInput | TextArea | CodeEditor | LogView | NumberInput | DragNumber
    ) {
        match interaction {
            PaintInteraction::Resting => {}
            PaintInteraction::Hovered | PaintInteraction::Focused => {
                fallback.background = Some(mix(theme.surface, theme.surface_alt, 0.70));
                fallback.border_color = Some(if interaction == PaintInteraction::Focused {
                    theme.accent
                } else {
                    mix(theme.border, theme.accent, 0.35)
                });
            }
            PaintInteraction::Pressed if kind == DragNumber => {
                fallback.background = Some(mix(theme.surface_alt, theme.accent, 0.20));
                fallback.border_color = Some(darken(theme.accent, 0.08));
            }
            PaintInteraction::Pressed => {}
            PaintInteraction::Disabled => {
                fallback.background = Some(mix(theme.surface_alt, theme.disabled, 0.24));
                fallback.border_color = Some(mix(theme.border, theme.disabled, 0.45));
            }
        }
    }

    if kind == Collapsible {
        fallback.border_color = Some(interaction_border(theme, interaction));
    }

    fallback
}

pub(crate) fn native_widget_paint_fallback_with_level(
    kind: WidgetKind,
    level: Option<&str>,
    theme: &Theme,
    interaction: PaintInteraction,
) -> NativePaintFallback {
    let mut fallback = native_widget_paint_fallback(kind, theme, interaction);
    if matches!(kind, WidgetKind::Badge | WidgetKind::Tag) {
        let semantic = semantic_level_color(level, theme);
        fallback.background = Some(if kind == WidgetKind::Tag {
            mix(theme.surface_alt, semantic, 0.22)
        } else {
            semantic
        });
        fallback.border_color = Some(semantic);
        fallback.border_width = Some(if kind == WidgetKind::Tag { 1.0 } else { 0.0 });
    }
    if kind == WidgetKind::Toast {
        let semantic = toast_level_color(level, theme);
        fallback.background = Some(mix(theme.surface, semantic, 0.18));
        fallback.border_color = Some(mix(theme.border, semantic, 0.62));
    }
    fallback
}

pub(crate) fn native_widget_paint_fallback_parts(kind: WidgetKind) -> &'static [&'static str] {
    use WidgetKind::*;

    match kind {
        IconButton | ArrowButton => &["icon"],
        Selectable => &["row"],
        Slider => &["track", "fill", "thumb"],
        RangeSlider => &["track", "range", "thumb-min", "thumb-max"],
        ProgressBar => &["track", "fill"],
        LimitsBar => &[
            "track",
            "red-low",
            "yellow-low",
            "green",
            "yellow-high",
            "red-high",
            "indicator",
        ],
        LoadingSpinner => &["track", "arc"],
        Checkbox => &["row", "box"],
        ToggleSwitch => &["row", "track", "thumb"],
        RadioButton => &["indicator"],
        TreeNode => &["row", "indicator", "guide"],
        Tab => &["tab"],
        NavItem => &["item"],
        DataFrameTable => &[
            "header",
            "row-selected",
            "grid-line",
            "scrollbar-track",
            "scrollbar-thumb",
        ],
        Heatmap => &["grid", "scalar-bar"],
        Splitter => &["gutter"],
        NumberInput => &[
            "stepper",
            "stepper-up",
            "stepper-down",
            "stepper-divider",
            "divider",
        ],
        DragNumber => &["grip"],
        HLayout | VLayout | ScrollArea | Pages | Page | Sidebar | Panel => {
            &["scrollbar-track", "scrollbar-thumb"]
        }
        Collapsible => &["header", "indicator", "scrollbar-track", "scrollbar-thumb"],
        Modal => &["scrim", "scrollbar-track", "scrollbar-thumb"],
        Menu | ContextMenu => &["menu", "item", "item-hover", "item-disabled"],
        Dropdown => &["chevron", "menu", "item"],
        _ => &[],
    }
}

pub(crate) fn native_widget_part_paint_fallback(
    kind: WidgetKind,
    part: &str,
    theme: &Theme,
    interaction: PaintInteraction,
    checked: bool,
) -> NativePaintFallback {
    native_widget_part_paint_fallback_with_selection(kind, part, theme, interaction, checked, false)
}

pub(crate) fn native_widget_part_paint_fallback_with_selection(
    kind: WidgetKind,
    part: &str,
    theme: &Theme,
    interaction: PaintInteraction,
    checked: bool,
    selected: bool,
) -> NativePaintFallback {
    use WidgetKind::*;

    let disabled = interaction == PaintInteraction::Disabled;
    match (kind, part) {
        (IconButton | ArrowButton, "icon") => NativePaintFallback {
            background: Some(if disabled { theme.disabled } else { theme.text }),
            ..NativePaintFallback::default()
        },
        (Selectable, "row") => NativePaintFallback {
            background: Some(if selected {
                mix(theme.surface_alt, theme.accent, 0.24)
            } else {
                match interaction {
                    PaintInteraction::Hovered | PaintInteraction::Focused => {
                        mix(theme.surface, theme.surface_alt, 0.62)
                    }
                    PaintInteraction::Resting
                    | PaintInteraction::Pressed
                    | PaintInteraction::Disabled => [0.0, 0.0, 0.0, 0.0],
                }
            }),
            ..NativePaintFallback::default()
        },
        (Selectable, "indicator") if selected => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.accent
            }),
            ..NativePaintFallback::default()
        },
        (Slider | RangeSlider, "track") => NativePaintFallback {
            background: Some(theme.border),
            ..NativePaintFallback::default()
        },
        (Slider, "fill") | (RangeSlider, "range") => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.accent
            }),
            ..NativePaintFallback::default()
        },
        (Slider, "thumb") | (RangeSlider, "thumb-min" | "thumb-max") => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.accent
            }),
            border_color: Some(if disabled {
                mix(theme.border, theme.disabled, 0.45)
            } else {
                theme.border
            }),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (ProgressBar, "track") => NativePaintFallback {
            background: Some(if disabled {
                mix(theme.surface_alt, theme.disabled, 0.24)
            } else {
                mix(theme.surface, theme.surface_alt, 0.60)
            }),
            border_color: Some(if disabled {
                mix(theme.border, theme.disabled, 0.45)
            } else {
                theme.border
            }),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        (ProgressBar, "fill") => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.accent
            }),
            ..NativePaintFallback::default()
        },
        (LimitsBar, "track") => NativePaintFallback {
            background: Some(if disabled {
                mix(theme.surface_alt, theme.disabled, 0.24)
            } else {
                mix(theme.surface, theme.surface_alt, 0.60)
            }),
            border_color: Some(if disabled {
                mix(theme.border, theme.disabled, 0.45)
            } else {
                theme.border
            }),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        (LimitsBar, "red-low" | "red-high") => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.danger
            }),
            ..NativePaintFallback::default()
        },
        (LimitsBar, "yellow-low" | "yellow-high") => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.warning
            }),
            ..NativePaintFallback::default()
        },
        (LimitsBar, "green") => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.success
            }),
            ..NativePaintFallback::default()
        },
        (LimitsBar, "indicator") => NativePaintFallback {
            background: Some(if disabled { theme.disabled } else { theme.text }),
            border_color: Some(if disabled {
                theme.border
            } else {
                theme.surface
            }),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (LoadingSpinner, "track") => NativePaintFallback {
            background: Some(with_opacity(
                if disabled {
                    theme.disabled
                } else {
                    mix(theme.border, theme.surface_alt, 0.35)
                },
                if disabled { 0.68 * 0.52 } else { 0.52 },
            )),
            ..NativePaintFallback::default()
        },
        (LoadingSpinner, "arc") => NativePaintFallback {
            background: Some(with_opacity(
                if disabled {
                    theme.disabled
                } else {
                    theme.accent
                },
                if disabled { 0.66 } else { 1.0 },
            )),
            ..NativePaintFallback::default()
        },
        (Checkbox, "row") => NativePaintFallback {
            background: Some(match interaction {
                PaintInteraction::Pressed => with_opacity(darken(theme.accent, 0.15), 0.20),
                PaintInteraction::Hovered | PaintInteraction::Focused => {
                    with_opacity(mix(theme.surface_alt, theme.accent, 0.20), 0.35)
                }
                PaintInteraction::Resting | PaintInteraction::Disabled => [0.0, 0.0, 0.0, 0.0],
            }),
            ..NativePaintFallback::default()
        },
        (Checkbox, "box") => NativePaintFallback {
            background: Some(if checked {
                match interaction {
                    PaintInteraction::Disabled => theme.disabled,
                    PaintInteraction::Pressed => darken(theme.accent, 0.15),
                    _ => theme.accent,
                }
            } else {
                let control = match interaction {
                    PaintInteraction::Disabled => mix(theme.surface_alt, theme.disabled, 0.28),
                    PaintInteraction::Pressed => darken(theme.accent, 0.15),
                    PaintInteraction::Hovered | PaintInteraction::Focused => {
                        mix(theme.surface_alt, theme.accent, 0.20)
                    }
                    PaintInteraction::Resting => theme.surface_alt,
                };
                mix(theme.surface, control, 0.55)
            }),
            border_color: Some(if checked {
                if disabled {
                    theme.disabled
                } else {
                    theme.accent
                }
            } else {
                interaction_border(theme, interaction)
            }),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (Checkbox, "indicator") if checked => NativePaintFallback {
            background: Some(if disabled {
                mix(theme.surface_alt, theme.disabled, 0.35)
            } else {
                theme.text
            }),
            ..NativePaintFallback::default()
        },
        (ToggleSwitch, "row") => NativePaintFallback {
            background: Some(match interaction {
                PaintInteraction::Pressed => with_opacity(darken(theme.accent, 0.12), 0.20),
                PaintInteraction::Hovered | PaintInteraction::Focused => {
                    with_opacity(mix(theme.surface_alt, theme.accent, 0.18), 0.32)
                }
                PaintInteraction::Resting | PaintInteraction::Disabled => [0.0, 0.0, 0.0, 0.0],
            }),
            ..NativePaintFallback::default()
        },
        (ToggleSwitch, "track") => NativePaintFallback {
            background: Some(if checked {
                match interaction {
                    PaintInteraction::Disabled => theme.disabled,
                    PaintInteraction::Pressed => darken(theme.accent, 0.12),
                    _ => theme.accent,
                }
            } else {
                mix(theme.surface, theme.surface_alt, 0.70)
            }),
            border_color: Some(if checked {
                if disabled {
                    theme.disabled
                } else {
                    theme.accent
                }
            } else {
                interaction_border(theme, interaction)
            }),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (ToggleSwitch, "thumb") => NativePaintFallback {
            background: Some(if disabled {
                mix(theme.surface_alt, theme.disabled, 0.32)
            } else {
                theme.text
            }),
            border_color: Some(if disabled {
                theme.disabled
            } else {
                mix(theme.border, theme.text, 0.28)
            }),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (RadioButton, "indicator") => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(if disabled {
                theme.disabled
            } else if checked {
                theme.accent
            } else {
                theme.border
            }),
            border_width: Some(1.5),
            ..NativePaintFallback::default()
        },
        (RadioButton, "dot") if checked => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.accent
            }),
            ..NativePaintFallback::default()
        },
        (TreeNode, "row") => NativePaintFallback {
            background: Some(if selected {
                mix(theme.surface_alt, theme.accent, 0.24)
            } else {
                match interaction {
                    PaintInteraction::Pressed => mix(theme.surface_alt, theme.accent, 0.20),
                    PaintInteraction::Hovered | PaintInteraction::Focused => {
                        mix(theme.surface, theme.surface_alt, 0.62)
                    }
                    PaintInteraction::Resting | PaintInteraction::Disabled => [0.0, 0.0, 0.0, 0.0],
                }
            }),
            border_color: Some([0.0, 0.0, 0.0, 0.0]),
            border_width: Some(1.0),
            border_radius: Some(2.0),
            ..NativePaintFallback::default()
        },
        (TreeNode, "indicator") => NativePaintFallback {
            background: Some(theme.muted_text),
            ..NativePaintFallback::default()
        },
        (TreeNode, "guide") => NativePaintFallback {
            background: Some(theme.border),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (Tab, "tab") => NativePaintFallback {
            background: Some(if selected {
                mix(theme.surface_alt, theme.accent, 0.24)
            } else {
                match interaction {
                    PaintInteraction::Hovered | PaintInteraction::Focused => {
                        mix(theme.surface_alt, theme.accent, 0.12)
                    }
                    PaintInteraction::Disabled => mix(theme.surface_alt, theme.disabled, 0.28),
                    PaintInteraction::Pressed | PaintInteraction::Resting => theme.surface_alt,
                }
            }),
            border_color: Some(if selected { theme.accent } else { theme.border }),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (Tab, "accent") if selected => NativePaintFallback {
            background: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        (NavItem, "item") => NativePaintFallback {
            background: Some(if selected {
                mix(theme.surface_alt, theme.accent, 0.20)
            } else {
                match interaction {
                    PaintInteraction::Disabled => mix(theme.surface_alt, theme.disabled, 0.28),
                    PaintInteraction::Pressed => darken(theme.accent, 0.15),
                    PaintInteraction::Hovered | PaintInteraction::Focused => {
                        mix(theme.surface_alt, theme.accent, 0.20)
                    }
                    PaintInteraction::Resting => theme.surface_alt,
                }
            }),
            ..NativePaintFallback::default()
        },
        (NavItem, "accent") if selected => NativePaintFallback {
            background: Some(theme.accent),
            ..NativePaintFallback::default()
        },
        (DataFrameTable, "header") => NativePaintFallback {
            background: Some(mix(theme.surface_alt, theme.accent, 0.10)),
            ..NativePaintFallback::default()
        },
        (DataFrameTable, "row-selected") => NativePaintFallback {
            background: Some(mix(theme.surface_alt, theme.accent, 0.22)),
            ..NativePaintFallback::default()
        },
        (DataFrameTable, "grid-line") => NativePaintFallback {
            background: Some(theme.border),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (Heatmap, "grid") => NativePaintFallback {
            background: Some(with_alpha(mix(theme.border, theme.background, 0.16), 0.38)),
            border_width: Some(1.0),
            ..NativePaintFallback::default()
        },
        (Heatmap, "scalar-bar") => NativePaintFallback {
            border_color: Some(theme.border),
            border_width: Some(1.0),
            border_radius: Some(2.0),
            ..NativePaintFallback::default()
        },
        (Heatmap, "hover") => NativePaintFallback {
            background: Some(with_alpha(mix(theme.accent, theme.surface, 0.26), 0.14)),
            border_color: Some(with_alpha(mix(theme.text, theme.accent, 0.20), 0.94)),
            border_width: Some(2.0),
            border_radius: Some(1.5),
            ..NativePaintFallback::default()
        },
        (Collapsible, "header") => NativePaintFallback {
            background: Some(match interaction {
                PaintInteraction::Pressed => mix(theme.surface_alt, theme.accent, 0.24),
                PaintInteraction::Hovered | PaintInteraction::Focused => {
                    mix(theme.surface_alt, theme.accent, 0.14)
                }
                PaintInteraction::Resting | PaintInteraction::Disabled => theme.surface_alt,
            }),
            ..NativePaintFallback::default()
        },
        (Collapsible, "indicator") | (Dropdown, "chevron") => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                theme.muted_text
            }),
            ..NativePaintFallback::default()
        },
        (Splitter, "gutter") => NativePaintFallback {
            background: Some(theme.border),
            ..NativePaintFallback::default()
        },
        (NumberInput, "stepper" | "stepper-up" | "stepper-down") => NativePaintFallback {
            background: Some(match interaction {
                PaintInteraction::Disabled => mix(theme.surface_alt, theme.disabled, 0.30),
                PaintInteraction::Hovered
                | PaintInteraction::Focused
                | PaintInteraction::Pressed => mix(theme.surface_alt, theme.accent, 0.16),
                PaintInteraction::Resting => theme.surface_alt,
            }),
            ..NativePaintFallback::default()
        },
        (NumberInput, "stepper-divider" | "divider") => NativePaintFallback {
            background: Some(theme.border),
            ..NativePaintFallback::default()
        },
        (DragNumber, "grip") => NativePaintFallback {
            background: Some(if disabled {
                theme.disabled
            } else {
                mix(theme.muted_text, theme.accent, 0.32)
            }),
            ..NativePaintFallback::default()
        },
        (DataFrameTable, "scrollbar-track") => NativePaintFallback {
            background: Some(with_alpha(mix(theme.surface, theme.muted_text, 0.25), 0.20)),
            ..NativePaintFallback::default()
        },
        (DataFrameTable, "scrollbar-thumb") => NativePaintFallback {
            background: Some(with_alpha(
                mix(theme.surface_alt, theme.muted_text, 0.52),
                0.68,
            )),
            ..NativePaintFallback::default()
        },
        (
            HLayout | VLayout | ScrollArea | Pages | Page | Sidebar | Panel | Collapsible | Modal,
            "scrollbar-track",
        ) => NativePaintFallback {
            background: Some(with_alpha(mix(theme.surface, theme.muted_text, 0.25), 0.22)),
            ..NativePaintFallback::default()
        },
        (
            HLayout | VLayout | ScrollArea | Pages | Page | Sidebar | Panel | Collapsible | Modal,
            "scrollbar-thumb",
        ) => NativePaintFallback {
            background: Some(with_alpha(
                mix(theme.surface_alt, theme.muted_text, 0.45),
                0.58,
            )),
            ..NativePaintFallback::default()
        },
        (Modal, "scrim") => NativePaintFallback {
            background: Some([0.0, 0.0, 0.0, 0.52]),
            ..NativePaintFallback::default()
        },
        (Menu | ContextMenu, "menu") => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(mix(theme.border, theme.accent, 0.18)),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        (Menu | ContextMenu, "item") => NativePaintFallback {
            background: Some(theme.surface_alt),
            ..NativePaintFallback::default()
        },
        (Menu | ContextMenu, "item-hover") => NativePaintFallback {
            background: Some(mix(theme.surface_alt, theme.accent, 0.24)),
            ..NativePaintFallback::default()
        },
        (Menu | ContextMenu, "item-disabled") => NativePaintFallback {
            background: Some(mix(theme.surface, theme.disabled, 0.18)),
            ..NativePaintFallback::default()
        },
        (Dropdown, "menu") => NativePaintFallback {
            background: Some(theme.surface),
            border_color: Some(mix(theme.border, theme.accent, 0.18)),
            border_width: Some(1.0),
            border_radius: Some(theme.radius),
            ..NativePaintFallback::default()
        },
        (Dropdown, "item") => NativePaintFallback {
            background: Some(if selected && interaction == PaintInteraction::Hovered {
                mix(theme.surface_alt, theme.accent, 0.42)
            } else if selected {
                mix(theme.surface_alt, theme.accent, 0.28)
            } else if interaction == PaintInteraction::Hovered {
                mix(theme.surface_alt, theme.accent, 0.24)
            } else {
                theme.surface_alt
            }),
            ..NativePaintFallback::default()
        },
        _ => NativePaintFallback::default(),
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn darken(color: Color, t: f32) -> Color {
    mix(color, [0.0, 0.0, 0.0, color[3]], t)
}

fn with_opacity(mut color: Color, opacity: f32) -> Color {
    color[3] *= opacity.clamp(0.0, 1.0);
    color
}

fn with_alpha(mut color: Color, alpha: f32) -> Color {
    color[3] = alpha.clamp(0.0, 1.0);
    color
}

fn toast_level_color(level: Option<&str>, theme: &Theme) -> Color {
    match level.unwrap_or("info").trim().to_ascii_lowercase().as_str() {
        "success" => theme.success,
        "warning" => theme.warning,
        "error" => theme.danger,
        _ => theme.accent,
    }
}

fn interaction_border(theme: &Theme, interaction: PaintInteraction) -> Color {
    match interaction {
        PaintInteraction::Disabled => mix(theme.border, theme.disabled, 0.45),
        PaintInteraction::Focused => theme.accent,
        PaintInteraction::Pressed => darken(theme.accent, 0.08),
        PaintInteraction::Hovered => mix(theme.border, theme.accent, 0.35),
        PaintInteraction::Resting => theme.border,
    }
}

fn semantic_level_color(level: Option<&str>, theme: &Theme) -> Color {
    match level.unwrap_or("info") {
        "success" => theme.success,
        "warning" => theme.warning,
        "danger" | "error" => theme.danger,
        "neutral" => theme.muted_text,
        _ => theme.accent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_box_and_control_defaults_are_explicit() {
        let theme = Theme::dark();
        let panel =
            native_widget_paint_fallback(WidgetKind::Panel, &theme, PaintInteraction::Resting);
        let button =
            native_widget_paint_fallback(WidgetKind::Button, &theme, PaintInteraction::Resting);

        assert_eq!(panel.background, Some(theme.surface));
        assert_eq!(panel.border_color, Some(theme.border));
        assert_eq!(panel.border_radius, Some(theme.radius * 0.5));
        assert_eq!(button.background, Some(theme.surface_alt));
        assert_eq!(button.border_color, Some(theme.border));
        assert_eq!(button.border_width, Some(1.0));
        assert_eq!(button.border_radius, Some(theme.radius));
    }

    #[test]
    fn control_interactions_change_only_runtime_colors() {
        let theme = Theme::dark();
        let resting =
            native_widget_paint_fallback(WidgetKind::Button, &theme, PaintInteraction::Resting);
        let focused =
            native_widget_paint_fallback(WidgetKind::Button, &theme, PaintInteraction::Focused);

        assert_ne!(focused.background, resting.background);
        assert_eq!(focused.border_color, Some(theme.accent));
        assert_eq!(focused.border_width, resting.border_width);
        assert_eq!(focused.border_radius, resting.border_radius);
    }

    #[test]
    fn selection_and_field_defaults_preserve_distinct_surface_contracts() {
        let theme = Theme::dark();
        let selectable =
            native_widget_paint_fallback(WidgetKind::Selectable, &theme, PaintInteraction::Resting);
        let input =
            native_widget_paint_fallback(WidgetKind::TextInput, &theme, PaintInteraction::Resting);
        let focused =
            native_widget_paint_fallback(WidgetKind::TextInput, &theme, PaintInteraction::Focused);

        assert_eq!(selectable.background, Some([0.0, 0.0, 0.0, 0.0]));
        assert_eq!(selectable.border_color, Some([0.0, 0.0, 0.0, 0.0]));
        assert_eq!(
            input.background,
            Some(mix(theme.surface, theme.surface_alt, 0.55))
        );
        assert_eq!(input.border_color, Some(theme.border));
        assert_eq!(focused.border_color, Some(theme.accent));
        assert_ne!(focused.background, input.background);
    }

    #[test]
    fn range_and_progress_parts_have_stable_resting_paint() {
        let theme = Theme::dark();
        let slider =
            native_widget_paint_fallback(WidgetKind::Slider, &theme, PaintInteraction::Resting);
        let thumb = native_widget_part_paint_fallback(
            WidgetKind::Slider,
            "thumb",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let progress_track = native_widget_part_paint_fallback(
            WidgetKind::ProgressBar,
            "track",
            &theme,
            PaintInteraction::Resting,
            false,
        );

        assert_eq!(slider.track_color, Some(theme.border));
        assert_eq!(slider.thumb_color, Some(theme.accent));
        assert_eq!(thumb.background, Some(theme.accent));
        assert_eq!(thumb.border_color, Some(theme.border));
        assert_eq!(thumb.border_width, Some(1.0));
        assert_eq!(
            progress_track.background,
            Some(mix(theme.surface, theme.surface_alt, 0.60))
        );
        assert_eq!(progress_track.border_radius, Some(theme.radius));
    }

    #[test]
    fn checked_control_parts_separate_resting_and_checked_paint() {
        let theme = Theme::dark();
        let unchecked = native_widget_part_paint_fallback(
            WidgetKind::Checkbox,
            "box",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let checked = native_widget_part_paint_fallback(
            WidgetKind::Checkbox,
            "box",
            &theme,
            PaintInteraction::Resting,
            true,
        );
        let toggle_thumb = native_widget_part_paint_fallback(
            WidgetKind::ToggleSwitch,
            "thumb",
            &theme,
            PaintInteraction::Resting,
            false,
        );

        assert_ne!(unchecked.background, checked.background);
        assert_eq!(unchecked.border_color, Some(theme.border));
        assert_eq!(checked.background, Some(theme.accent));
        assert_eq!(checked.border_color, Some(theme.accent));
        assert_eq!(toggle_thumb.background, Some(theme.text));
        assert_eq!(toggle_thumb.border_width, Some(1.0));
    }

    #[test]
    fn navigation_parts_separate_resting_and_selected_paint() {
        let theme = Theme::dark();
        let tree_resting = native_widget_part_paint_fallback_with_selection(
            WidgetKind::TreeNode,
            "row",
            &theme,
            PaintInteraction::Resting,
            false,
            false,
        );
        let tree_selected = native_widget_part_paint_fallback_with_selection(
            WidgetKind::TreeNode,
            "row",
            &theme,
            PaintInteraction::Resting,
            false,
            true,
        );
        let tab_selected = native_widget_part_paint_fallback_with_selection(
            WidgetKind::Tab,
            "tab",
            &theme,
            PaintInteraction::Resting,
            false,
            true,
        );
        let nav_selected = native_widget_part_paint_fallback_with_selection(
            WidgetKind::NavItem,
            "item",
            &theme,
            PaintInteraction::Resting,
            false,
            true,
        );

        assert_eq!(tree_resting.background, Some([0.0, 0.0, 0.0, 0.0]));
        assert_eq!(
            tree_selected.background,
            Some(mix(theme.surface_alt, theme.accent, 0.24))
        );
        assert_eq!(tab_selected.border_color, Some(theme.accent));
        assert_eq!(
            nav_selected.background,
            Some(mix(theme.surface_alt, theme.accent, 0.20))
        );
        assert_eq!(
            native_widget_paint_fallback(WidgetKind::Badge, &theme, PaintInteraction::Resting)
                .border_radius,
            Some(999.0)
        );
    }

    #[test]
    fn badge_and_tag_semantic_paint_uses_node_level() {
        let theme = Theme::dark();
        let badge = native_widget_paint_fallback_with_level(
            WidgetKind::Badge,
            Some("warning"),
            &theme,
            PaintInteraction::Resting,
        );
        let tag = native_widget_paint_fallback_with_level(
            WidgetKind::Tag,
            Some("success"),
            &theme,
            PaintInteraction::Resting,
        );
        let neutral = native_widget_paint_fallback_with_level(
            WidgetKind::Badge,
            Some("neutral"),
            &theme,
            PaintInteraction::Resting,
        );

        assert_eq!(badge.background, Some(theme.warning));
        assert_eq!(badge.border_color, Some(theme.warning));
        assert_eq!(badge.border_width, Some(0.0));
        assert_eq!(
            tag.background,
            Some(mix(theme.surface_alt, theme.success, 0.22))
        );
        assert_eq!(tag.border_color, Some(theme.success));
        assert_eq!(tag.border_width, Some(1.0));
        assert_eq!(neutral.background, Some(theme.muted_text));
    }

    #[test]
    fn data_widget_chrome_and_table_parts_are_stable() {
        let theme = Theme::dark();
        for kind in [
            WidgetKind::Histogram,
            WidgetKind::BarChart,
            WidgetKind::Heatmap,
            WidgetKind::PieChart,
            WidgetKind::LinePlot,
            WidgetKind::DataFrameTable,
        ] {
            let fallback = native_widget_paint_fallback(kind, &theme, PaintInteraction::Resting);
            assert_eq!(fallback.background, Some(theme.surface));
            assert_eq!(fallback.border_color, Some(theme.border));
            assert_eq!(fallback.border_width, Some(1.0));
            assert_eq!(fallback.border_radius, Some(theme.radius));
        }

        let selected = native_widget_part_paint_fallback(
            WidgetKind::DataFrameTable,
            "row-selected",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let grid = native_widget_part_paint_fallback(
            WidgetKind::DataFrameTable,
            "grid-line",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        assert_eq!(
            selected.background,
            Some(mix(theme.surface_alt, theme.accent, 0.22))
        );
        assert_eq!(grid.background, Some(theme.border));
        assert_eq!(grid.border_width, Some(1.0));
    }

    #[test]
    fn scrollbar_parts_preserve_generic_and_table_palettes() {
        let theme = Theme::dark();
        let panel_track = native_widget_part_paint_fallback(
            WidgetKind::Panel,
            "scrollbar-track",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let panel_thumb = native_widget_part_paint_fallback(
            WidgetKind::Panel,
            "scrollbar-thumb",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let table_track = native_widget_part_paint_fallback(
            WidgetKind::DataFrameTable,
            "scrollbar-track",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let table_thumb = native_widget_part_paint_fallback(
            WidgetKind::DataFrameTable,
            "scrollbar-thumb",
            &theme,
            PaintInteraction::Resting,
            false,
        );

        assert_eq!(
            panel_track.background,
            Some(with_alpha(mix(theme.surface, theme.muted_text, 0.25), 0.22))
        );
        assert_eq!(
            panel_thumb.background,
            Some(with_alpha(
                mix(theme.surface_alt, theme.muted_text, 0.45),
                0.58
            ))
        );
        assert_eq!(table_track.background.unwrap()[3], 0.20);
        assert_eq!(table_thumb.background.unwrap()[3], 0.68);
        assert_ne!(panel_thumb.background, table_thumb.background);
    }

    #[test]
    fn stable_overlay_surfaces_and_row_states_are_cataloged() {
        let theme = Theme::dark();
        let tooltip =
            native_widget_paint_fallback(WidgetKind::Tooltip, &theme, PaintInteraction::Resting);
        let scrim = native_widget_part_paint_fallback(
            WidgetKind::Modal,
            "scrim",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let menu = native_widget_part_paint_fallback(
            WidgetKind::Menu,
            "menu",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let hovered = native_widget_part_paint_fallback(
            WidgetKind::ContextMenu,
            "item-hover",
            &theme,
            PaintInteraction::Resting,
            false,
        );

        assert_eq!(tooltip.background, Some(theme.surface_alt));
        assert_eq!(
            tooltip.border_color,
            Some(mix(theme.border, theme.accent, 0.18))
        );
        assert_eq!(tooltip.border_width, Some(1.0));
        assert_eq!(scrim.background, Some([0.0, 0.0, 0.0, 0.52]));
        assert_eq!(menu.background, Some(theme.surface));
        assert_eq!(menu.border_width, Some(1.0));
        assert_eq!(
            hovered.background,
            Some(mix(theme.surface_alt, theme.accent, 0.24))
        );
    }

    #[test]
    fn dropdown_rows_and_semantic_toasts_are_cataloged_without_runtime_geometry() {
        let theme = Theme::dark();
        let menu = native_widget_part_paint_fallback(
            WidgetKind::Dropdown,
            "menu",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let item = native_widget_part_paint_fallback_with_selection(
            WidgetKind::Dropdown,
            "item",
            &theme,
            PaintInteraction::Resting,
            false,
            false,
        );
        let selected = native_widget_part_paint_fallback_with_selection(
            WidgetKind::Dropdown,
            "item",
            &theme,
            PaintInteraction::Resting,
            false,
            true,
        );
        let selected_hover = native_widget_part_paint_fallback_with_selection(
            WidgetKind::Dropdown,
            "item",
            &theme,
            PaintInteraction::Hovered,
            false,
            true,
        );
        let error = native_widget_paint_fallback_with_level(
            WidgetKind::Toast,
            Some("error"),
            &theme,
            PaintInteraction::Resting,
        );

        assert_eq!(menu.background, Some(theme.surface));
        assert_eq!(menu.border_width, Some(1.0));
        assert_eq!(item.background, Some(theme.surface_alt));
        assert_eq!(
            selected.background,
            Some(mix(theme.surface_alt, theme.accent, 0.28))
        );
        assert_eq!(
            selected_hover.background,
            Some(mix(theme.surface_alt, theme.accent, 0.42))
        );
        assert_eq!(
            error.background,
            Some(mix(theme.surface, theme.danger, 0.18))
        );
        assert_eq!(
            error.border_color,
            Some(mix(theme.border, theme.danger, 0.62))
        );
    }

    #[test]
    fn heatmap_chrome_is_cataloged_while_hover_remains_runtime_only() {
        let theme = Theme::dark();
        let grid = native_widget_part_paint_fallback(
            WidgetKind::Heatmap,
            "grid",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let scalar_bar = native_widget_part_paint_fallback(
            WidgetKind::Heatmap,
            "scalar-bar",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let hover = native_widget_part_paint_fallback(
            WidgetKind::Heatmap,
            "hover",
            &theme,
            PaintInteraction::Hovered,
            false,
        );

        assert_eq!(
            grid.background,
            Some(with_alpha(mix(theme.border, theme.background, 0.16), 0.38))
        );
        assert_eq!(grid.border_width, Some(1.0));
        assert_eq!(scalar_bar.border_color, Some(theme.border));
        assert_eq!(scalar_bar.border_radius, Some(2.0));
        assert_eq!(hover.border_width, Some(2.0));
        assert!(native_widget_paint_fallback_parts(WidgetKind::Heatmap).contains(&"grid"));
        assert!(!native_widget_paint_fallback_parts(WidgetKind::Heatmap).contains(&"hover"));
    }

    #[test]
    fn collapsible_surface_and_header_separate_static_and_interactive_paint() {
        let theme = Theme::dark();
        let surface = native_widget_paint_fallback(
            WidgetKind::Collapsible,
            &theme,
            PaintInteraction::Resting,
        );
        let header = native_widget_part_paint_fallback(
            WidgetKind::Collapsible,
            "header",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let hovered_header = native_widget_part_paint_fallback(
            WidgetKind::Collapsible,
            "header",
            &theme,
            PaintInteraction::Hovered,
            false,
        );

        assert_eq!(surface.background, Some(theme.surface));
        assert_eq!(surface.border_color, Some(theme.border));
        assert_eq!(surface.border_width, Some(1.0));
        assert_eq!(surface.border_radius, Some(theme.radius));
        assert_eq!(header.background, Some(theme.surface_alt));
        assert_eq!(
            hovered_header.background,
            Some(mix(theme.surface_alt, theme.accent, 0.14))
        );
        assert!(native_widget_paint_fallback_parts(WidgetKind::Collapsible).contains(&"header"));
        assert!(!native_widget_paint_fallback_parts(WidgetKind::Collapsible).contains(&"body"));
    }

    #[test]
    fn structural_separator_and_splitter_gutter_share_stable_divider_paint() {
        let theme = Theme::dark();
        let separator =
            native_widget_paint_fallback(WidgetKind::Separator, &theme, PaintInteraction::Resting);
        let gutter = native_widget_part_paint_fallback(
            WidgetKind::Splitter,
            "gutter",
            &theme,
            PaintInteraction::Resting,
            false,
        );

        assert_eq!(separator.background, Some(theme.border));
        assert_eq!(separator.border_radius, Some(0.0));
        assert_eq!(gutter.background, Some(theme.border));
        assert_eq!(
            native_widget_paint_fallback_parts(WidgetKind::Splitter),
            &["gutter"]
        );
        assert!(native_widget_paint_fallback_parts(WidgetKind::Pane).is_empty());
    }

    #[test]
    fn number_input_stepper_surfaces_and_dividers_are_cataloged_without_marks() {
        let theme = Theme::dark();
        for part in ["stepper", "stepper-up", "stepper-down"] {
            let resting = native_widget_part_paint_fallback(
                WidgetKind::NumberInput,
                part,
                &theme,
                PaintInteraction::Resting,
                false,
            );
            let hovered = native_widget_part_paint_fallback(
                WidgetKind::NumberInput,
                part,
                &theme,
                PaintInteraction::Hovered,
                false,
            );
            assert_eq!(resting.background, Some(theme.surface_alt));
            assert_eq!(
                hovered.background,
                Some(mix(theme.surface_alt, theme.accent, 0.16))
            );
        }
        for part in ["stepper-divider", "divider"] {
            assert_eq!(
                native_widget_part_paint_fallback(
                    WidgetKind::NumberInput,
                    part,
                    &theme,
                    PaintInteraction::Resting,
                    false,
                )
                .background,
                Some(theme.border)
            );
        }
        assert!(!native_widget_paint_fallback_parts(WidgetKind::NumberInput).contains(&"field"));
        assert!(!native_widget_paint_fallback_parts(WidgetKind::NumberInput).contains(&"caret"));
    }

    #[test]
    fn radio_button_indicator_is_stable_while_dot_requires_selection() {
        let theme = Theme::dark();
        let indicator = native_widget_part_paint_fallback(
            WidgetKind::RadioButton,
            "indicator",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        assert_eq!(indicator.background, Some(theme.surface));
        assert_eq!(indicator.border_color, Some(theme.border));
        assert_eq!(indicator.border_width, Some(1.5));
        assert_eq!(
            native_widget_paint_fallback_parts(WidgetKind::RadioButton),
            &["indicator"]
        );

        assert_eq!(
            native_widget_part_paint_fallback(
                WidgetKind::RadioButton,
                "dot",
                &theme,
                PaintInteraction::Resting,
                false,
            ),
            NativePaintFallback::default()
        );
        assert_eq!(
            native_widget_part_paint_fallback(
                WidgetKind::RadioButton,
                "dot",
                &theme,
                PaintInteraction::Resting,
                true,
            )
            .background,
            Some(theme.accent)
        );
    }

    #[test]
    fn drag_number_grip_is_cataloged_without_field_or_value_text() {
        let theme = Theme::dark();
        let grip = native_widget_part_paint_fallback(
            WidgetKind::DragNumber,
            "grip",
            &theme,
            PaintInteraction::Resting,
            false,
        );
        let disabled_grip = native_widget_part_paint_fallback(
            WidgetKind::DragNumber,
            "grip",
            &theme,
            PaintInteraction::Disabled,
            false,
        );

        assert_eq!(
            grip.background,
            Some(mix(theme.muted_text, theme.accent, 0.32))
        );
        assert_eq!(disabled_grip.background, Some(theme.disabled));
        assert_eq!(
            native_widget_paint_fallback_parts(WidgetKind::DragNumber),
            &["grip"]
        );
        assert!(!native_widget_paint_fallback_parts(WidgetKind::DragNumber).contains(&"field"));
        assert!(!native_widget_paint_fallback_parts(WidgetKind::DragNumber).contains(&"value"));
        assert!(native_widget_paint_fallback_parts(WidgetKind::Led).is_empty());
    }

    #[test]
    fn disclosure_mark_colors_are_cataloged_without_direction_geometry() {
        let theme = Theme::dark();
        for (kind, part) in [
            (WidgetKind::Collapsible, "indicator"),
            (WidgetKind::Dropdown, "chevron"),
        ] {
            let resting = native_widget_part_paint_fallback(
                kind,
                part,
                &theme,
                PaintInteraction::Resting,
                false,
            );
            let disabled = native_widget_part_paint_fallback(
                kind,
                part,
                &theme,
                PaintInteraction::Disabled,
                false,
            );
            assert_eq!(resting.background, Some(theme.muted_text));
            assert_eq!(disabled.background, Some(theme.disabled));
            assert!(native_widget_paint_fallback_parts(kind).contains(&part));
        }
    }

    #[test]
    fn action_button_icon_colors_are_cataloged_without_glyph_geometry() {
        let theme = Theme::dark();
        for kind in [WidgetKind::IconButton, WidgetKind::ArrowButton] {
            let resting = native_widget_part_paint_fallback(
                kind,
                "icon",
                &theme,
                PaintInteraction::Resting,
                false,
            );
            let disabled = native_widget_part_paint_fallback(
                kind,
                "icon",
                &theme,
                PaintInteraction::Disabled,
                false,
            );
            assert_eq!(resting.background, Some(theme.text));
            assert_eq!(disabled.background, Some(theme.disabled));
            assert_eq!(native_widget_paint_fallback_parts(kind), &["icon"]);
        }
        assert!(native_widget_paint_fallback_parts(WidgetKind::ImageButton).is_empty());
    }

    #[test]
    fn selectable_row_is_cataloged_while_indicator_requires_selection() {
        let theme = Theme::dark();
        let resting = native_widget_part_paint_fallback_with_selection(
            WidgetKind::Selectable,
            "row",
            &theme,
            PaintInteraction::Resting,
            false,
            false,
        );
        let hovered = native_widget_part_paint_fallback_with_selection(
            WidgetKind::Selectable,
            "row",
            &theme,
            PaintInteraction::Hovered,
            false,
            false,
        );
        let selected = native_widget_part_paint_fallback_with_selection(
            WidgetKind::Selectable,
            "row",
            &theme,
            PaintInteraction::Resting,
            false,
            true,
        );
        assert_eq!(resting.background, Some([0.0, 0.0, 0.0, 0.0]));
        assert_eq!(
            hovered.background,
            Some(mix(theme.surface, theme.surface_alt, 0.62))
        );
        assert_eq!(
            selected.background,
            Some(mix(theme.surface_alt, theme.accent, 0.24))
        );
        assert_eq!(
            native_widget_paint_fallback_parts(WidgetKind::Selectable),
            &["row"]
        );
        assert_eq!(
            native_widget_part_paint_fallback_with_selection(
                WidgetKind::Selectable,
                "indicator",
                &theme,
                PaintInteraction::Resting,
                false,
                false,
            ),
            NativePaintFallback::default()
        );
        assert_eq!(
            native_widget_part_paint_fallback_with_selection(
                WidgetKind::Selectable,
                "indicator",
                &theme,
                PaintInteraction::Resting,
                false,
                true,
            )
            .background,
            Some(theme.accent)
        );
    }
}
