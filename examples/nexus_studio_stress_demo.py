from __future__ import annotations

import argparse
import math
import random
import sys
from pathlib import Path
from typing import Any

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


NEXUS_THEME = dg.Theme.dark(accent="#8b7cff", radius=10, spacing=6)
WINDOWS_311_THEME = dg.Theme.light(
    background="#c0c0c0",
    surface="#c0c0c0",
    surface_alt="#ffffff",
    text="#000000",
    muted_text="#404040",
    accent="#000080",
    border="#808080",
    danger="#800000",
    warning="#808000",
    success="#008000",
    focus="#000080",
    disabled="#808080",
    radius=0,
    spacing=6,
    font_size=12,
)
MAC_OS_90S_THEME = dg.Theme.light(
    background="#d9d9d9",
    surface="#dedede",
    surface_alt="#ffffff",
    text="#000000",
    muted_text="#505050",
    accent="#333366",
    border="#606060",
    danger="#770000",
    warning="#777000",
    success="#247024",
    focus="#000000",
    disabled="#888888",
    radius=3,
    spacing=6,
    font_size=12,
)
SPACE_MD = NEXUS_THEME.space_md

WINDOWS_311_CSS = """
    :root {
        --win-face: #c0c0c0;
        --win-light: #ffffff;
        --win-highlight: #dfdfdf;
        --win-shadow: #808080;
        --win-dark: #000000;
        --win-title: #000080;
        --win-selection: #000080;
        --win-text: #000000;
        --win-selected-text: #ffffff;
        --win-disabled: #808080;
        --win-field: #ffffff;
        --win-desktop: #008080;
    }

    Window {
        background: var(--win-face);
        color: var(--win-text);
        font-family: "Microsoft Sans Serif";
        font-size: 12px;
    }

    AppShell.nexus-shell,
    WorkbenchLayout.nexus-workbench,
    Body,
    Pages,
    Page {
        background: var(--win-face);
        color: var(--win-text);
    }

    Sidebar.nexus-sidebar {
        background: var(--win-face);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        padding: 8px;
        gap: 6px;
        box-shadow:
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    .sidebar-subtitle,
    .muted,
    .page-description {
        color: #404040;
    }

    .sidebar-subtitle { font-size: 11px; }

    .sidebar-section,
    .kicker {
        color: var(--win-title);
        font-size: 11px;
        font-weight: 700;
        letter-spacing: 0;
        text-transform: none;
    }

    NavItem::item {
        background: var(--win-face);
        border: 1px solid transparent;
        border-radius: 0;
        color: var(--win-text);
    }

    NavItem:hover::item {
        border-color: var(--win-shadow);
    }

    NavItem:selected::item {
        background: var(--win-selection);
        border-color: var(--win-selection);
        color: var(--win-selected-text);
    }

    NavItem::accent {
        width: 0;
        background: transparent;
    }

    NavItem::badge,
    Tab::badge {
        background: var(--win-face);
        border: 1px solid var(--win-dark);
        border-radius: 0;
        color: var(--win-text);
        box-shadow: inset 1px 1px 0 var(--win-light);
    }

    NavItem:selected::badge,
    Tab:selected::badge {
        background: var(--win-light);
        color: var(--win-text);
    }

    MenuBar {
        background: var(--win-face);
        border-color: var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
        padding: 2px 4px;
    }

    Menu,
    MenuItem {
        background: var(--win-face);
        border-radius: 0;
        color: var(--win-text);
    }

    Menu:hover,
    MenuItem:hover {
        background: var(--win-selection);
        color: var(--win-selected-text);
    }

    Toolbar.nexus-toolbar {
        background: var(--win-face);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        padding: 4px 5px;
        gap: 5px;
        box-shadow:
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    ToolbarSeparator {
        background: var(--win-shadow);
        border-color: var(--win-light);
    }

    Tabs {
        background: var(--win-face);
        border-color: var(--win-shadow);
        border-radius: 0;
    }

    Tab::tab {
        background: var(--win-face);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
        box-shadow:
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    Tab:selected::tab {
        background: var(--win-face);
        color: var(--win-text);
        box-shadow:
            inset 2px 2px 0 var(--win-light),
            inset -1px -1px 0 var(--win-shadow);
    }

    Tab::accent {
        height: 0;
        background: transparent;
    }

    StatusBar.nexus-status {
        background: var(--win-face);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
        box-shadow:
            inset 1px 1px 0 var(--win-shadow),
            inset -1px -1px 0 var(--win-light);
    }

    SearchBox.global-search { width: 300px; }

    ScrollArea.nexus-page-scroll {
        background: var(--win-face);
        padding: 6px 10px 22px 4px;
        gap: 12px;
    }

    ScrollArea#diagnostics-scroll { gap: var(--dg-space-md); }

    DataFrameTable::scrollbar-track {
        background: var(--win-highlight);
        border: 1px solid var(--win-shadow);
        border-radius: 0;
    }

    DataFrameTable::scrollbar-thumb {
        background: var(--win-face);
        border: 1px solid var(--win-dark);
        border-radius: 0;
        box-shadow: inset 1px 1px 0 var(--win-light);
    }

    Splitter.diagnostic-splitter::gutter {
        width: 4px;
        background: var(--win-face);
        border: 1px solid var(--win-shadow);
    }

    Panel {
        background: var(--win-face);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
        box-shadow:
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    Panel.sidebar-card,
    Panel.hero-card {
        background: var(--win-face);
        border-color: var(--win-shadow);
        border-radius: 0;
        padding: 10px;
        box-shadow:
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    Panel Panel {
        box-shadow:
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    Button,
    SmallButton,
    IconButton,
    Dropdown,
    NumberInput,
    DateInput,
    TimeInput,
    DateTimeInput {
        background: var(--win-face);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
        box-shadow:
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    Button:hover,
    SmallButton:hover,
    IconButton:hover,
    Dropdown:hover {
        background: var(--win-highlight);
        border-color: var(--win-shadow);
    }

    Button:active,
    SmallButton:active,
    IconButton:active {
        background: var(--win-face);
        box-shadow:
            inset 1px 1px 0 var(--win-dark),
            inset -1px -1px 0 var(--win-light);
    }

    Button:focus,
    SmallButton:focus,
    IconButton:focus,
    Dropdown:focus,
    TextInput:focus,
    TextArea:focus,
    NumberInput:focus {
        outline: 1px solid var(--win-dark);
        outline-offset: -4px;
    }

    Button.primary {
        background: var(--win-face);
        border-color: var(--win-shadow);
        color: var(--win-text);
        font-weight: 700;
        box-shadow:
            0 0 0 1px var(--win-dark),
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    Button.primary:hover {
        background: var(--win-highlight);
        box-shadow:
            0 0 0 1px var(--win-dark),
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-dark);
    }

    Button.primary:active {
        box-shadow:
            0 0 0 1px var(--win-dark),
            inset 1px 1px 0 var(--win-dark),
            inset -1px -1px 0 var(--win-light);
    }

    TextInput,
    TextArea,
    SearchBox,
    CodeEditor,
    LogView {
        background: var(--win-field);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
        box-shadow:
            inset 1px 1px 0 var(--win-dark),
            inset -1px -1px 0 var(--win-light);
    }

    SearchBox::field {
        background: var(--win-field);
        border: 1px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
    }

    SearchBox::icon,
    SearchBox::clear {
        background: var(--win-face);
        border: 1px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
    }

    Dropdown::field {
        background: var(--win-field);
        border-radius: 0;
        color: var(--win-text);
    }

    Dropdown::menu {
        background: var(--win-face);
        border: 2px solid var(--win-dark);
        border-radius: 0;
        color: var(--win-text);
        box-shadow: inset 1px 1px 0 var(--win-light);
    }

    Dropdown::item-hover,
    Dropdown::item-selected {
        background: var(--win-selection);
        color: var(--win-selected-text);
    }

    Badge,
    Tag {
        background: var(--win-face);
        border: 1px solid var(--win-dark);
        border-radius: 0;
        color: var(--win-text);
        box-shadow: inset 1px 1px 0 var(--win-light);
    }

    Badge.success,
    Badge.info,
    Badge.warning {
        background: var(--win-face);
        border-color: var(--win-dark);
        color: var(--win-text);
    }

    ProgressBar {
        background: var(--win-field);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-selected-text);
        box-shadow:
            inset 1px 1px 0 var(--win-dark),
            inset -1px -1px 0 var(--win-light);
    }

    ProgressBar::track {
        background: var(--win-field);
        border-radius: 0;
    }

    ProgressBar::fill {
        background: var(--win-selection);
        border-radius: 0;
    }

    Checkbox::box,
    RadioButton {
        background: var(--win-field);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        box-shadow:
            inset 1px 1px 0 var(--win-dark),
            inset -1px -1px 0 var(--win-light);
    }

    ToggleSwitch::track,
    Slider::track,
    RangeSlider::track {
        background: var(--win-field);
        border: 1px solid var(--win-shadow);
        border-radius: 0;
    }

    ToggleSwitch::thumb,
    Slider::thumb,
    RangeSlider::thumb-min,
    RangeSlider::thumb-max {
        background: var(--win-face);
        border: 1px solid var(--win-dark);
        border-radius: 0;
        box-shadow: inset 1px 1px 0 var(--win-light);
    }

    DataFrameTable {
        background: var(--win-field);
        border: 2px solid var(--win-shadow);
        border-radius: 0;
        color: var(--win-text);
        table-row-height: 22px;
        table-header-height: 24px;
        box-shadow:
            inset 1px 1px 0 var(--win-dark),
            inset -1px -1px 0 var(--win-light);
    }

    DataFrameTable::header {
        background: var(--win-face);
        color: var(--win-text);
        font-weight: 700;
    }

    DataFrameTable::row { background: var(--win-field); }

    DataFrameTable::row-selected {
        background: var(--win-selection);
        color: var(--win-selected-text);
    }

    DataFrameTable::grid-line { background: var(--win-shadow); }

    Modal,
    CommandPalette,
    Tooltip,
    ContextMenu {
        background: var(--win-face);
        border: 2px solid var(--win-dark);
        border-radius: 0;
        color: var(--win-text);
        box-shadow:
            inset 1px 1px 0 var(--win-light),
            inset -1px -1px 0 var(--win-shadow);
    }

    .page-heading { gap: 4px; padding-bottom: 2px; }
    .page-title { color: var(--win-title); font-size: 20px; font-weight: 700; }
    .hero-copy { flex: 1; min-width: 250px; gap: 4px; }
    .hero-title { color: var(--win-title); font-size: 18px; font-weight: 700; }
    .sidebar-card-title { font-weight: 700; }

    .metric-label {
        color: #404040;
        font-size: 10px;
        font-weight: 700;
        text-transform: uppercase;
    }

    .metric-value {
        color: var(--win-title);
        font-size: 24px;
        font-weight: 700;
    }

    .mono {
        color: var(--win-text);
        font-family: "Courier New";
    }

    Panel.metric-card { min-height: 142px; }
    .metric-header { min-height: 20px; }
    .metric-progress { height: 8px; }
    .capacity-row { gap: 4px; }
    .capacity-heading { min-height: 28px; gap: 7px; }
    Panel.chart-card,
    Panel.capacity-card { min-height: 365px; }
    Panel.small-chart-card { min-height: 315px; }
    Panel.table-card { min-height: 400px; }
    Panel.filter-card { padding: 12px; }
    Panel.analysis-card { min-height: 365px; }
    Panel.workflow-card { min-height: 390px; }
    Panel.workflow-code-card { min-height: 470px; }
    Panel.control-card { min-height: 330px; }
    Panel.diagnostic-card { min-height: 300px; }
    DataFrameTable { min-height: 180px; }
    .check-row { gap: 9px; min-height: 42px; }
    .check-copy { flex: 1; min-width: 0; gap: 2px; }

    @media (max-width: 760px) {
        Toolbar.nexus-toolbar { padding: 3px; }
        SearchBox.global-search { width: 100%; flex-basis: 100%; }
        .page-heading Tag { display: none; }
        StatusBar.nexus-status Tag { display: none; }
        ScrollArea.nexus-page-scroll { padding: 5px 8px 18px 2px; gap: 10px; }
        .page-title { font-size: 17px; }
        .hero-title { font-size: 15px; }
        .hero-copy { min-width: 0; }
        Panel.metric-card,
        Panel.chart-card,
        Panel.capacity-card,
        Panel.small-chart-card,
        Panel.table-card,
        Panel.analysis-card,
        Panel.workflow-card,
        Panel.workflow-code-card,
        Panel.control-card,
        Panel.diagnostic-card {
            min-height: auto;
        }
    }
"""

MAC_OS_90S_CSS = WINDOWS_311_CSS + """
    /*
     * Classic Macintosh "Platinum" experiment.
     *
     * The Windows 3.11 sheet supplies the shared retro box-model reset. This
     * later layer deliberately restyles that geometry into a System 7 / early
     * Mac OS 8 vocabulary, exercising DragonGui's cascade as well as its
     * widget-part styling.
     */
    :root {
        --win-face: #dedede;
        --win-light: #ffffff;
        --win-highlight: #eeeeee;
        --win-shadow: #888888;
        --win-dark: #000000;
        --win-title: #000000;
        --win-selection: #333366;
        --win-text: #000000;
        --win-selected-text: #ffffff;
        --win-disabled: #888888;
        --win-field: #ffffff;
        --mac-platinum: #dedede;
        --mac-platinum-light: #eeeeee;
        --mac-stripe: #b8b8b8;
        --mac-selection: #333366;
        --mac-ink: #000000;
    }

    Window {
        background: var(--mac-platinum);
        color: var(--mac-ink);
        font-family: "Arial";
        font-size: 12px;
    }

    AppShell.nexus-shell,
    WorkbenchLayout.nexus-workbench,
    Body,
    Pages,
    Page,
    ScrollArea.nexus-page-scroll {
        background: var(--mac-platinum);
    }

    Sidebar.nexus-sidebar {
        background:
            repeating-linear-gradient(
                0deg,
                #eeeeee 0%,
                #eeeeee 3%,
                #c4c4c4 3%,
                #c4c4c4 6%
            );
        border: 1px solid var(--mac-ink);
        border-radius: 0;
        padding: 9px;
        box-shadow:
            inset 1px 1px 0 #ffffff,
            inset -1px -1px 0 #888888;
    }

    .sidebar-section,
    .kicker {
        color: var(--mac-ink);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.6px;
        text-transform: uppercase;
    }

    .sidebar-subtitle,
    .muted,
    .page-description {
        color: #505050;
    }

    NavItem::item {
        background: rgba(222, 222, 222, 0.86);
        border: 1px solid transparent;
        border-radius: 2px;
        color: var(--mac-ink);
    }

    NavItem:hover::item {
        background: #eeeeee;
        border-color: #888888;
    }

    NavItem:selected::item {
        background: var(--mac-selection);
        border-color: var(--mac-ink);
        color: #ffffff;
    }

    NavItem::badge,
    Tab::badge,
    Badge,
    Tag {
        background: #ffffff;
        border: 1px solid var(--mac-ink);
        border-radius: 7px;
        color: var(--mac-ink);
        box-shadow: none;
    }

    NavItem:selected::badge,
    Tab:selected::badge {
        background: #ffffff;
        color: var(--mac-ink);
    }

    MenuBar {
        background: #ffffff;
        border: 1px solid var(--mac-ink);
        border-radius: 0;
        color: var(--mac-ink);
        padding: 2px 7px;
        font-weight: 700;
        box-shadow: none;
    }

    Menu,
    MenuItem {
        background: #ffffff;
        border-radius: 0;
        color: var(--mac-ink);
    }

    Menu:hover,
    MenuItem:hover {
        background: var(--mac-selection);
        color: #ffffff;
    }

    Toolbar.nexus-toolbar {
        background:
            linear-gradient(180deg, #eeeeee, #d2d2d2);
        border: 1px solid #777777;
        border-radius: 0;
        padding: 5px 6px;
        gap: 6px;
        box-shadow:
            inset 1px 1px 0 #ffffff,
            inset -1px -1px 0 #aaaaaa;
    }

    ToolbarSeparator {
        background: #777777;
        border-color: #ffffff;
    }

    Tabs {
        background: var(--mac-platinum);
        border-color: #777777;
        border-radius: 0;
    }

    Tab::tab {
        background: linear-gradient(180deg, #ffffff, #d0d0d0);
        border: 1px solid var(--mac-ink);
        border-radius: 4px;
        color: var(--mac-ink);
        box-shadow: 1px 1px 0 #888888;
    }

    Tab:hover::tab {
        background: #ffffff;
    }

    Tab:selected::tab {
        background: #ffffff;
        border: 2px solid var(--mac-ink);
        color: var(--mac-ink);
        font-weight: 700;
        box-shadow: none;
    }

    StatusBar.nexus-status {
        background: #dedede;
        border: 1px solid #777777;
        border-radius: 0;
        color: var(--mac-ink);
        box-shadow:
            inset 1px 1px 0 #888888,
            inset -1px -1px 0 #ffffff;
    }

    Panel {
        background: #dedede;
        border: 1px solid var(--mac-ink);
        border-radius: 2px;
        color: var(--mac-ink);
        box-shadow:
            1px 1px 0 #777777,
            inset 1px 1px 0 #ffffff;
    }

    Panel Panel {
        box-shadow:
            inset 1px 1px 0 #ffffff,
            inset -1px -1px 0 #999999;
    }

    Panel.sidebar-card {
        background: rgba(255, 255, 255, 0.68);
        border: 1px solid var(--mac-ink);
        border-radius: 2px;
        box-shadow: inset 1px 1px 0 #ffffff;
    }

    Panel.hero-card {
        background:
            repeating-linear-gradient(
                0deg,
                #eeeeee 0%,
                #eeeeee 3%,
                #c7c7c7 3%,
                #c7c7c7 6%
            );
        border: 2px solid var(--mac-ink);
        border-radius: 0;
        box-shadow:
            inset 2px 2px 0 #ffffff,
            inset -2px -2px 0 #777777;
    }

    Button,
    SmallButton,
    IconButton,
    Dropdown,
    NumberInput,
    DateInput,
    TimeInput,
    DateTimeInput {
        background: linear-gradient(180deg, #ffffff, #d2d2d2);
        border: 1px solid var(--mac-ink);
        border-radius: 4px;
        color: var(--mac-ink);
        box-shadow:
            1px 1px 0 #777777,
            inset 1px 1px 0 #ffffff;
    }

    Button:hover,
    SmallButton:hover,
    IconButton:hover,
    Dropdown:hover {
        background: #ffffff;
        border-color: var(--mac-ink);
    }

    Button:active,
    SmallButton:active,
    IconButton:active {
        background: #888888;
        color: #ffffff;
        box-shadow: inset 1px 1px 0 var(--mac-ink);
    }

    Button.primary {
        background: linear-gradient(180deg, #ffffff, #cccccc);
        border: 2px solid var(--mac-ink);
        border-radius: 6px;
        color: var(--mac-ink);
        font-weight: 700;
        box-shadow:
            0 0 0 1px #ffffff,
            0 0 0 2px var(--mac-ink),
            inset 1px 1px 0 #ffffff;
    }

    Button.primary:hover {
        background: #ffffff;
        box-shadow:
            0 0 0 1px #ffffff,
            0 0 0 2px var(--mac-ink),
            inset 1px 1px 0 #ffffff;
    }

    Button.primary:active {
        background: var(--mac-selection);
        color: #ffffff;
        box-shadow:
            0 0 0 1px #ffffff,
            0 0 0 2px var(--mac-ink),
            inset 1px 1px 0 var(--mac-ink);
    }

    Button:focus,
    SmallButton:focus,
    IconButton:focus,
    Dropdown:focus,
    TextInput:focus,
    TextArea:focus,
    NumberInput:focus {
        outline: 1px solid var(--mac-ink);
        outline-offset: -3px;
    }

    TextInput,
    TextArea,
    SearchBox,
    CodeEditor,
    LogView {
        background: #ffffff;
        border: 1px solid var(--mac-ink);
        border-radius: 0;
        color: var(--mac-ink);
        box-shadow:
            inset 1px 1px 0 #777777,
            inset -1px -1px 0 #ffffff;
    }

    SearchBox {
        padding: 3px;
        gap: 5px;
    }

    SearchBox::field {
        border: 1px solid #777777;
        box-shadow: inset 1px 1px 0 #aaaaaa;
    }

    SearchBox::icon,
    SearchBox::clear {
        background: #dedede;
        border: 1px solid #777777;
        border-radius: 2px;
        box-shadow: 1px 1px 0 #ffffff;
    }

    Dropdown::field {
        background: #ffffff;
        border-radius: 2px;
        color: var(--mac-ink);
    }

    Dropdown::menu {
        background: #ffffff;
        border: 1px solid var(--mac-ink);
        border-radius: 0;
        color: var(--mac-ink);
        box-shadow: 2px 2px 0 #777777;
    }

    Dropdown::item-hover,
    Dropdown::item-selected {
        background: var(--mac-selection);
        color: #ffffff;
    }

    Badge.success,
    Badge.info,
    Badge.warning {
        background: #ffffff;
        border-color: var(--mac-ink);
        color: var(--mac-ink);
    }

    ProgressBar {
        background: #ffffff;
        border: 1px solid var(--mac-ink);
        border-radius: 0;
        color: #ffffff;
        box-shadow: inset 1px 1px 0 #777777;
    }

    ProgressBar::track {
        background: #ffffff;
        border-radius: 0;
    }

    ProgressBar::fill {
        background:
            repeating-linear-gradient(
                90deg,
                #333366 0%,
                #333366 8%,
                #ffffff 8%,
                #ffffff 10%
            );
        border-radius: 0;
    }

    Checkbox::box,
    RadioButton {
        background: #ffffff;
        border: 1px solid var(--mac-ink);
        border-radius: 0;
        box-shadow: inset 1px 1px 0 #777777;
    }

    ToggleSwitch::track,
    Slider::track,
    RangeSlider::track {
        background: #ffffff;
        border: 1px solid var(--mac-ink);
        border-radius: 0;
    }

    ToggleSwitch::thumb,
    Slider::thumb,
    RangeSlider::thumb-min,
    RangeSlider::thumb-max {
        background: linear-gradient(180deg, #ffffff, #cccccc);
        border: 1px solid var(--mac-ink);
        border-radius: 2px;
        box-shadow: 1px 1px 0 #777777;
    }

    DataFrameTable {
        background: #ffffff;
        border: 1px solid var(--mac-ink);
        border-radius: 0;
        color: var(--mac-ink);
        box-shadow: inset 1px 1px 0 #777777;
    }

    DataFrameTable::header {
        background:
            repeating-linear-gradient(
                0deg,
                #eeeeee 0%,
                #eeeeee 3%,
                #c7c7c7 3%,
                #c7c7c7 6%
            );
        color: var(--mac-ink);
        font-weight: 700;
    }

    DataFrameTable::row { background: #ffffff; }

    DataFrameTable::row-selected {
        background: var(--mac-selection);
        color: #ffffff;
    }

    DataFrameTable::grid-line { background: #999999; }

    Modal,
    CommandPalette,
    Tooltip,
    ContextMenu {
        background: #dedede;
        border: 2px solid var(--mac-ink);
        border-radius: 2px;
        color: var(--mac-ink);
        box-shadow:
            2px 2px 0 #777777,
            inset 1px 1px 0 #ffffff;
    }

    .page-title,
    .hero-title,
    .metric-value {
        color: var(--mac-ink);
        font-family: "Arial";
        font-weight: 700;
    }

    .page-title { font-size: 20px; }
    .hero-title { font-size: 17px; }
    .metric-value { font-size: 24px; }

    .metric-label {
        color: #404040;
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.5px;
    }

    .mono {
        color: var(--mac-ink);
        font-family: "Courier New";
    }
"""


class ColumnFrame:
    """Tiny dataframe-like object accepted by the plot widgets."""

    def __init__(self, **columns: list[Any]) -> None:
        self.columns = tuple(columns)
        self.dtypes = tuple(
            "float64"
            if all(isinstance(value, (int, float)) for value in values)
            else "str"
            for values in columns.values()
        )
        self.shape = (len(next(iter(columns.values()), [])), len(columns))
        self._columns = columns

    def __getitem__(self, column: str) -> list[Any]:
        return self._columns[column]


SAMPLES = list(range(240))
TELEMETRY = ColumnFrame(
    sample=SAMPLES,
    requests=[
        720
        + math.sin(index / 13.0) * 105
        + math.cos(index / 4.2) * 26
        for index in SAMPLES
    ],
    latency=[
        41
        + math.sin(index / 19.0 + 0.6) * 12
        + math.cos(index / 5.4) * 4
        for index in SAMPLES
    ],
    errors=[
        2.8
        + math.sin(index / 8.0) * 1.7
        + abs(math.cos(index / 17.0)) * 0.8
        for index in SAMPLES
    ],
)

REGIONS = ColumnFrame(
    region=["East", "Central", "West", "Europe", "Asia"],
    volume=[92.0, 78.0, 71.0, 66.0, 58.0],
    latency=[28.0, 34.0, 39.0, 46.0, 51.0],
)

HEAT_FIELD = [
    [
        round(
            24
            + row * 4.8
            + math.sin(column / 2.0 + row * 0.72) * 9
            + math.cos(column / 3.4) * 3,
            2,
        )
        for column in range(12)
    ]
    for row in range(7)
]

JOBS = [
    {
        "job": f"NX-{4100 + index}",
        "workspace": ("Search", "Billing", "Identity", "Catalog")[index % 4],
        "owner": ("A. Chen", "M. Rivera", "S. Patel", "J. Morgan")[index % 4],
        "state": ("running", "review", "healthy", "queued")[index % 4],
        "p95_ms": round(27 + (index * 8.7) % 61, 1),
        "progress": f"{42 + (index * 7) % 57}%",
        "updated": f"{2 + index:02d}m ago",
    }
    for index in range(28)
]


class DemoState:
    def __init__(self) -> None:
        self.app: dg.App | None = None
        self.pages: dg.Pages | None = None
        self.tabs: dg.Tabs | None = None
        self.sidebar: dg.Sidebar | None = None
        self.status: dg.Label | None = None
        self.status_badge: dg.Badge | None = None
        self.modal: dg.Modal | None = None
        self.palette: dg.CommandPalette | None = None
        self.table: dg.DataFrameTable | None = None
        self.progress: dg.ProgressBar | None = None
        self.random = random.Random(20260725)


state = DemoState()

ROUTES = (
    ("command", "Command", "live"),
    ("data", "Data Lab", "28"),
    ("workflow", "Workflow", "6"),
    ("controls", "Controls", None),
    ("diagnostics", "Diagnostics", None),
)


def set_status(message: str, level: str = "info") -> None:
    if state.status is not None:
        state.status.set_value(message)
    if state.status_badge is not None:
        state.status_badge.set_value(level)


def navigate(route: str) -> None:
    if state.pages is not None:
        state.pages.set_value(route)
    if state.tabs is not None:
        state.tabs.set_value(route)
    set_status(f"Opened {route}", "ready")


def toggle_sidebar() -> None:
    if state.sidebar is not None:
        state.sidebar.toggle_collapsed()


def show_modal() -> None:
    if state.modal is not None:
        state.modal.show()
    set_status("Launch dialog opened", "review")


def show_palette() -> None:
    if state.palette is not None:
        state.palette.show()
    set_status("Command palette opened", "search")


def refresh_jobs() -> None:
    rows = list(JOBS)
    state.random.shuffle(rows)
    if state.table is not None:
        state.table.set_frame(rows)
    set_status("Job stream refreshed", "live")


def page_scroll(page: str) -> dg.ScrollArea:
    return dg.ScrollArea(
        axis="y",
        gap=14,
        class_="nexus-page-scroll",
        id=f"{page}-scroll",
    )


def page_heading(kicker: str, title: str, description: str) -> None:
    with dg.VLayout(class_="page-heading"):
        dg.Breadcrumbs(
            [
                ("Nexus Studio", "command"),
                ("Workspace", "command"),
                (title, title.lower()),
            ],
            on_select=lambda item: set_status(f"Breadcrumb: {item.label}"),
        )
        dg.Label(kicker, class_="kicker", wrap=False)
        with dg.FlowLayout(gap=10, row_gap=6, style={"align_items": "center"}):
            dg.Label(title, class_="page-title", wrap=False)
            dg.Tag("STRESS TARGET", level="info")
        dg.Label(description, class_="page-description")


def metric_card(
    label: str,
    value: str,
    detail: str,
    *,
    progress: float,
    level: str,
) -> None:
    with dg.Panel(class_=f"metric-card metric-{level}"):
        with dg.HLayout(class_="metric-header", style={"align_items": "center"}):
            dg.Label(label, class_="metric-label", wrap=False)
            dg.Spacer()
            dg.LED(level in {"success", "info"})
        dg.Label(value, class_="metric-value", wrap=False)
        dg.Label(detail, class_="muted")
        dg.ProgressBar(progress, show_value=False, class_="metric-progress")


def build_command_page() -> None:
    page_heading(
        "GLOBAL OPERATIONS / NOW",
        "Command overview",
        "A dense dashboard combining responsive cards, charts, tables, nested flow layouts, and explicit scrolling.",
    )

    with dg.Panel(class_="hero-card"):
        with dg.FlowLayout(gap=18, row_gap=12, style={"align_items": "center"}):
            with dg.VLayout(class_="hero-copy"):
                dg.Label("SYSTEM POSTURE", class_="kicker", wrap=False)
                dg.Label(
                    "Every critical service is inside its operating envelope",
                    class_="hero-title",
                )
                dg.Label(
                    "Two changes await review. Global request volume is 8.2% above the weekly baseline.",
                    class_="page-description",
                )
            with dg.FlowLayout(
                gap=8,
                row_gap=8,
                class_="hero-actions",
                style={"align_items": "center"},
            ):
                dg.Button(
                    "Launch workflow",
                    class_="primary",
                    on_click=show_modal,
                    id="nexus-launch",
                )
                dg.SmallButton("Refresh", on_click=refresh_jobs)
                dg.Tag("99.995% available", level="success")

    with dg.GridLayout(
        columns={"default": 4, 1100: 2, 700: 1},
        min_column_width=205,
        gap=12,
        balance_last_row=True,
    ):
        metric_card("Availability", "99.995%", "+0.018% this week", progress=0.96, level="success")
        metric_card("Request rate", "42.8k/s", "8.2% above baseline", progress=0.84, level="info")
        metric_card("Median latency", "38 ms", "p95 is currently 71 ms", progress=0.69, level="neutral")
        metric_card("Change queue", "12", "two need approval", progress=0.43, level="warning")

    with dg.GridLayout(columns=2, min_column_width=420, gap=12):
        with dg.Panel("Traffic pulse", class_="chart-card"):
            with dg.FlowLayout(
                gap=7,
                row_gap=6,
                class_="panel-tools",
                style={"align_items": "center"},
            ):
                dg.Tag("240 samples", level="neutral")
                dg.SmallButton("1H")
                dg.SmallButton("4H")
                dg.SmallButton("Fit")
            dg.LinePlot(
                TELEMETRY,
                x="sample",
                y=["requests", "latency"],
                labels=["requests", "latency"],
                colors=["#8b7cff", "#44d7e8"],
                show_toolbar=False,
                show_legend=True,
                style={"height": 286},
            )

        with dg.Panel("Capacity lanes", class_="capacity-card"):
            dg.Label(
                "Mixed intrinsic labels and flexible progress tracks should remain aligned at every width.",
                class_="muted",
            )
            for label, value, tag in (
                ("Edge ingest", 0.82, "steady"),
                ("Search compute", 0.71, "warm"),
                ("Identity cache", 0.56, "ready"),
                ("Report export", 0.39, "idle"),
            ):
                with dg.VLayout(class_="capacity-row"):
                    with dg.HLayout(
                        class_="capacity-heading",
                        style={"align_items": "center"},
                    ):
                        dg.Label(label, wrap=False)
                        dg.Spacer()
                        dg.Tag(tag, level="neutral")
                        dg.Label(f"{value:.0%}", class_="mono", wrap=False)
                    dg.ProgressBar(value, show_value=False)
            with dg.Collapsible("Change advisory", expanded=True):
                dg.Label(
                    "Catalog indexing is approaching its autoscale threshold. No intervention is required.",
                    class_="muted",
                )

    with dg.GridLayout(columns=3, min_column_width=275, gap=12):
        with dg.Panel("Regional mix", class_="small-chart-card"):
            dg.PieChart(
                labels=["East", "Central", "West", "Europe", "Asia"],
                values=[29, 23, 20, 16, 12],
                donut=True,
                center_value="42.8k",
                center_label="requests/s",
                show_legend=True,
                legend_position="bottom",
                style={"height": 250},
            )
        with dg.Panel("Latency field", class_="small-chart-card"):
            dg.Heatmap(
                HEAT_FIELD,
                x_labels=[f"{hour:02d}" for hour in range(0, 24, 2)],
                y_labels=[f"R{row}" for row in range(1, 8)],
                title="p95 by region / hour",
                colormap="viridis",
                style={"height": 250},
            )
        with dg.Panel("Volume by region", class_="small-chart-card"):
            dg.BarChart(
                REGIONS,
                category="region",
                value="volume",
                aggregate="mean",
                show_toolbar=False,
                style={"height": 250},
            )

    with dg.Panel("Live job stream", class_="table-card"):
        with dg.FlowLayout(
            gap=8,
            row_gap=8,
            class_="table-tools",
            style={"align_items": "center"},
        ):
            dg.SearchBox("", placeholder="Filter job, owner, state", width=280)
            dg.Dropdown(("All states", "Running", "Review", "Healthy"), value="All states")
            dg.SmallButton("Refresh", on_click=refresh_jobs)
            dg.Spacer()
            dg.Badge("28 jobs", level="info")
        state.table = dg.DataFrameTable(
            JOBS,
            page_size=18,
            sample_rows=28,
            sortable=True,
            resizable_columns=True,
            style={"height": 310},
            on_select=lambda selection: set_status(
                f"Selected {selection.column}: {selection.value}",
                "selected",
            ),
        )


def build_data_page() -> None:
    page_heading(
        "EXPLORATION / DATA LAB",
        "Signal laboratory",
        "Plot grids, filters, a large table, and inspector controls test minimum sizing and vertical reachability.",
    )
    with dg.Panel("Query controls", class_="filter-card"):
        with dg.FlowLayout(gap=9, row_gap=9, style={"align_items": "center"}):
            dg.SearchBox("", placeholder="Search dimensions", grow=True)
            dg.Dropdown(("Last hour", "Last 6 hours", "Last 24 hours"), value="Last 6 hours")
            dg.Dropdown(("All regions", "East", "Central", "West"), value="All regions")
            dg.ToggleSwitch("Anomalies only", checked=False)
            dg.Button("Run query", class_="primary")

    with dg.GridLayout(columns=2, min_column_width=390, gap=12):
        with dg.Panel("Latency distribution", class_="analysis-card"):
            dg.Histogram(TELEMETRY["latency"], bins=34, mode="count", style={"height": 300})
        with dg.Panel("Regional comparison", class_="analysis-card"):
            dg.BarChart(
                REGIONS,
                category="region",
                value=["volume", "latency"],
                series=["volume", "latency"],
                aggregate="mean",
                show_toolbar=True,
                style={"height": 300},
            )
        with dg.Panel("Temporal response", class_="analysis-card"):
            dg.Heatmap(
                HEAT_FIELD,
                x_labels=[f"{hour:02d}:00" for hour in range(0, 24, 2)],
                y_labels=[f"Region {row}" for row in range(1, 8)],
                title="Response time field",
                colormap="magma",
                style={"height": 320},
            )
        with dg.Panel("Model inspector", class_="analysis-card"):
            dg.RangeSlider((24, 81), min=0, max=100, step=1)
            dg.Slider(0.72, min=0, max=1, step=0.01)
            dg.NumberInput(120, min=20, max=500, step=10)
            dg.RadioGroup(
                ["Automatic", "Conservative", "Aggressive"],
                value="Automatic",
                orientation="horizontal",
            )
            dg.Checkbox("Normalize by request volume", checked=True)
            dg.Checkbox("Include maintenance windows", checked=False)
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Button("Apply", class_="primary")
                dg.SmallButton("Reset")

    with dg.Panel("Sample records", class_="table-card"):
        dg.DataFrameTable(
            JOBS,
            page_size=24,
            sample_rows=28,
            sortable=True,
            resizable_columns=True,
            style={"height": 360},
        )


def build_workflow_page() -> None:
    page_heading(
        "AUTOMATION / WORKFLOW",
        "Release composer",
        "Tree navigation, forms, selection, drag/drop, code, and logs share a responsive two-column surface.",
    )
    with dg.GridLayout(columns=2, min_column_width=370, gap=12):
        with dg.Panel("Pipeline map", class_="workflow-card"):
            with dg.ScrollArea(axis="y", height=190, gap=4):
                with dg.TreeView(selected="validate"):
                    with dg.TreeNode("Nexus release 24.7", node_id="release", expanded=True):
                        dg.TreeNode("Collect telemetry", node_id="collect", leaf=True)
                        dg.TreeNode("Validate policy", node_id="validate", leaf=True)
                        with dg.TreeNode("Regional rollout", node_id="rollout", expanded=True):
                            dg.TreeNode("East canary", node_id="east", leaf=True)
                            dg.TreeNode("Central canary", node_id="central", leaf=True)
                        dg.TreeNode("Promote globally", node_id="promote", leaf=True)
            dg.Separator()
            dg.SelectableList(
                [
                    {"label": "Verify SLO window", "value": "slo"},
                    {"label": "Attach audit snapshot", "value": "audit"},
                    {"label": "Notify service owners", "value": "notify"},
                ],
                selected=["slo", "audit"],
                selection_mode="multiple",
            )

        with dg.Panel("Deployment request", class_="workflow-card"):
            dg.TextInput("catalog.search.release-24.7", placeholder="Release route")
            dg.Dropdown(("Canary", "Staged", "Full rollout"), value="Canary")
            dg.TextArea(
                "Validate search relevance and p95 latency before promoting the bundle.",
                rows=4,
            )
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.DateInput("2026-07-25")
                dg.TimeInput("16:30")
                dg.DateTimeInput("2026-07-25T16:30:00")
            dg.ToggleSwitch("Require owner approval", checked=True)
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Button("Queue release", class_="primary")
                dg.SmallButton("Save draft")

        with dg.Panel("Payload staging", class_="workflow-card"):
            with dg.FlowLayout(gap=8, row_gap=8):
                for payload in ("telemetry.json", "policy.py", "audit.zip"):
                    with dg.DragSource(
                        {"asset": payload},
                        drag_kind="release-asset",
                        style={"padding": 8, "border_width": 1, "border_color": "border"},
                    ):
                        dg.Tag(payload, level="info")
            dg.DropZone(
                "Drop release assets here",
                accept="release-asset",
                style={"height": 120},
                on_drop=lambda payload: set_status(f"Staged {payload}"),
            )
            dg.ProgressBar(0.68, label="Bundle readiness")

        with dg.Panel("Policy script", class_="workflow-card workflow-code-card"):
            dg.CodeEditor(
                "def approve(snapshot):\n"
                "    latency_ok = snapshot.p95_ms < 75\n"
                "    errors_ok = snapshot.error_rate < 0.02\n"
                "    return latency_ok and errors_ok\n",
                language="python",
                rows=10,
                style={"height": 230},
            )
            dg.LogView(
                [
                    "16:02:10 telemetry snapshot attached",
                    "16:02:11 policy syntax validated",
                    "16:02:13 east canary waiting",
                    "16:02:16 owner approval requested",
                ],
                rows=6,
                follow=True,
                wrap=True,
                variant="activity",
                style={"height": 150},
            )


def build_controls_page() -> None:
    page_heading(
        "DESIGN SYSTEM / CONTROLS",
        "Component laboratory",
        "A broad widget gallery organized into realistic cards to stress wrapping, intrinsic widths, and state styling.",
    )
    with dg.GridLayout(
        columns={"default": 3, 1050: 2, 690: 1},
        min_column_width=285,
        gap=12,
    ):
        with dg.Panel("Text and numeric", class_="control-card"):
            dg.TextInput("Nexus workspace", placeholder="Workspace name")
            dg.TextArea("A compact multiline field that should remain inside its card.", rows=4)
            dg.NumberInput(42, min=0, max=100, step=1)
            dg.DragNumber(0.625, min=0, max=1, step=0.005)
            dg.DragVector((0.25, 0.5, 0.75), labels=("x", "y", "z"), min=0, max=1, step=0.05)

        with dg.Panel("Selection", class_="control-card"):
            dg.Dropdown(("Automatic", "Balanced", "Performance"), value="Balanced")
            dg.Checkbox("Enable telemetry", checked=True)
            dg.Checkbox("Include archived jobs", checked=False)
            dg.ToggleSwitch("Live updates", checked=True)
            dg.RadioGroup(
                ["Compact", "Comfortable", "Spacious"],
                value="Comfortable",
                orientation="horizontal",
            )
            dg.SelectableList(
                ["Overview", "Analytics", "Automation", "Diagnostics"],
                selected=["Overview"],
            )

        with dg.Panel("Ranges and status", class_="control-card"):
            dg.Slider(0.64, min=0, max=1, step=0.01)
            dg.RangeSlider((18, 82), min=0, max=100, step=1)
            dg.ProgressBar(0.74, label="Validation")
            dg.ProgressBar(0.38, show_value=False)
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.LED(True)
                dg.Badge("online", level="success")
                dg.Badge("12", level="warning")
                dg.Tag("beta", level="info")
                dg.LoadingSpinner(size=18)

        with dg.Panel("Date and time", class_="control-card"):
            dg.DateInput("2026-07-25")
            dg.TimeInput("16:30")
            dg.DateTimeInput("2026-07-25T16:30:00")
            dg.Label("Temporal controls should share a stable baseline and never escape the panel.", class_="muted")

        with dg.Panel("Color and appearance", class_="control-card"):
            dg.ColorPicker((139, 124, 255, 255), title="Accent")
            dg.Dropdown(("Obsidian", "Graphite", "High contrast"), value="Obsidian")
            dg.ToggleSwitch("Reduced motion", checked=False)
            dg.ToggleSwitch("Compact density", checked=True)

        with dg.Panel("Buttons and disclosure", class_="control-card"):
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Button("Primary", class_="primary")
                dg.Button("Standard")
                dg.SmallButton("Small")
                dg.IconButton("search", tooltip="Search")
                dg.ArrowButton("right")
            with dg.Collapsible("Advanced options", expanded=True):
                dg.Checkbox("Retain diagnostics", checked=True)
                dg.Checkbox("Capture screenshots", checked=True)
            with dg.Collapsible("Collapsed section", expanded=False):
                dg.Label("This content should not consume layout space while collapsed.")


def build_diagnostics_page() -> None:
    page_heading(
        "RUNTIME / INSPECTOR",
        "Diagnostics workbench",
        "Split panes, property grids, scroll owners, runtime logs, and compact tables exercise nested viewport contracts.",
    )
    with dg.Panel("Runtime summary", class_="diagnostic-summary"):
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.Badge("WGPU", level="success")
            dg.Tag("ABI3 wheel", level="info")
            dg.Tag("strict layout", level="neutral")
            dg.Tag("1x–2x scale", level="neutral")
            dg.Spacer()
            dg.SmallButton("Capture snapshot", on_click=lambda: set_status("Snapshot requested"))

    with dg.Splitter(
        orientation="horizontal",
        gutter_size=SPACE_MD,
        class_="diagnostic-splitter",
        style={"height": 510, "min_height": 390},
    ):
        with dg.Pane(flex=42, min_size=300):
            with dg.Panel(
                "Computed contract",
                style={"flex": 1, "min_height": 0},
            ):
                dg.PropertyGrid(
                    {
                        "Viewport": "1440 × 900",
                        "Scale factor": "1.0",
                        "Renderer": "WGPU",
                        "Layout mode": "responsive",
                        "Structural errors": 0,
                        "Usability advisories": 0,
                        "Stylesheet warnings": 0,
                        "Snapshot schema": 1,
                    },
                    label_width=132,
                )
        with dg.Pane(flex=58, min_size=330):
            with dg.Panel(
                "Runtime event stream",
                style={"flex": 1, "min_height": 0},
            ):
                dg.LogView(
                    [
                        "16:30:00 renderer initialized",
                        "16:30:01 stylesheet cascade resolved",
                        "16:30:01 responsive grid selected 4 tracks",
                        "16:30:02 scroll ownership validated",
                        "16:30:02 zero structural diagnostics",
                    ],
                    rows=16,
                    follow=True,
                    wrap=True,
                    variant="debug",
                    style={"height": "100%"},
                )

    with dg.GridLayout(columns=2, min_column_width=350, gap=SPACE_MD):
        with dg.Panel("Sizing contract", class_="diagnostic-card"):
            dg.DataFrameTable(
                [
                    {"widget": "Panel", "grow": "0", "shrink": "1", "overflow": "visible"},
                    {"widget": "Body", "grow": "1", "shrink": "1", "overflow": "auto"},
                    {"widget": "Sidebar", "grow": "0", "shrink": "1", "overflow": "hidden"},
                    {"widget": "SearchBox", "grow": "opt-in", "shrink": "1", "overflow": "hidden"},
                    {"widget": "GridLayout", "grow": "1", "shrink": "1", "overflow": "visible"},
                ],
                page_size=8,
                sample_rows=8,
                style={"height": 250},
            )
        with dg.Panel("Audit checklist", class_="diagnostic-card"):
            for label, detail in (
                ("Public selectors", "All selectors use stable widget identity"),
                ("Scroll reachability", "Every page owns explicit vertical scrolling"),
                ("Responsive grids", "4/2/1 and 3/2/1 contracts are active"),
                ("Overlay bounds", "Modal and palette stay inside the viewport"),
                ("DPI behavior", "Logical breakpoints remain scale independent"),
            ):
                with dg.HLayout(class_="check-row", style={"align_items": "center"}):
                    dg.LED(True)
                    with dg.VLayout(class_="check-copy"):
                        dg.Label(label, wrap=False)
                        dg.Label(detail, class_="muted")


def build_window() -> dg.Window:
    with dg.Window("Nexus Studio — DragonGUI Stress Demo", width=1440, height=900) as window:
        with dg.AppShell(class_="nexus-shell"):
            state.sidebar = dg.Sidebar(
                title="NEXUS",
                width=236,
                collapsed_width=70,
                class_="nexus-sidebar",
                id="nexus-sidebar",
            )
            with state.sidebar:
                dg.Label("Operations studio", class_="sidebar-subtitle")
                dg.Label("WORKSPACES", class_="sidebar-section", wrap=False)
                for route, label, badge in ROUTES:
                    dg.NavItem(label, page=route, badge=badge)
                dg.Spacer(height=8)
                dg.Label("ENVIRONMENT", class_="sidebar-section", wrap=False)
                with dg.Panel(class_="sidebar-card"):
                    with dg.FlowLayout(gap=7, row_gap=5, style={"align_items": "center"}):
                        dg.LED(True)
                        dg.Label("Production", class_="sidebar-card-title", wrap=False)
                    dg.Label("us-east · build 24.7.183", class_="sidebar-subtitle")
                    dg.ProgressBar(0.76, show_value=False)

            with dg.WorkbenchLayout(gap=7, padding=9, class_="nexus-workbench"):
                with dg.MenuBar(height=30):
                    with dg.Menu("Workspace", id="nexus-workspace-menu"):
                        for route, label, _badge in ROUTES:
                            dg.MenuItem(label, on_click=lambda selected=route: navigate(selected))
                    with dg.Menu("Actions"):
                        dg.MenuItem("Launch workflow", on_click=show_modal)
                        dg.MenuItem("Refresh job stream", on_click=refresh_jobs)
                        dg.MenuItem("Command palette", on_click=show_palette)
                    with dg.Menu("View"):
                        dg.MenuItem("Toggle navigation", on_click=toggle_sidebar)
                    with dg.Menu("Help"):
                        dg.MenuItem("Layout diagnostics", on_click=lambda: navigate("diagnostics"))

                with dg.Toolbar(class_="nexus-toolbar"):
                    dg.IconButton(
                        "menu",
                        tooltip="Toggle navigation",
                        on_click=toggle_sidebar,
                        id="nexus-sidebar-toggle",
                    )
                    dg.ToolbarSeparator()
                    dg.IconButton("search", tooltip="Command palette", on_click=show_palette, id="nexus-palette-button")
                    dg.IconButton("refresh", tooltip="Refresh jobs", on_click=refresh_jobs)
                    dg.ToolbarSeparator()
                    dg.SearchBox(
                        "",
                        placeholder="Search jobs, owners, commands",
                        class_="global-search",
                        id="nexus-search",
                    )
                    dg.Spacer()
                    dg.SmallButton("Snapshot", on_click=lambda: set_status("Snapshot requested"))
                    dg.Button(
                        "Launch",
                        class_="primary",
                        on_click=show_modal,
                        id="nexus-launch-toolbar",
                    )
                    dg.Badge("online", level="success")

                state.tabs = dg.Tabs(value="command", on_change=navigate, id="nexus-tabs")
                with state.tabs:
                    for route, label, badge in ROUTES:
                        with dg.Tab(label, value=route, badge=badge):
                            pass

                with dg.Body():
                    state.pages = dg.Pages(value="command", id="nexus-pages")
                    with state.pages:
                        with dg.Page("command", title="Command"):
                            with page_scroll("command"):
                                build_command_page()
                        with dg.Page("data", title="Data Lab"):
                            with page_scroll("data"):
                                build_data_page()
                        with dg.Page("workflow", title="Workflow"):
                            with page_scroll("workflow"):
                                build_workflow_page()
                        with dg.Page("controls", title="Controls"):
                            with page_scroll("controls"):
                                build_controls_page()
                        with dg.Page("diagnostics", title="Diagnostics"):
                            with page_scroll("diagnostics"):
                                build_diagnostics_page()

                with dg.StatusBar(height=27, class_="nexus-status"):
                    state.status_badge = dg.Badge("ready", level="success")
                    state.status = dg.Label(
                        "Nexus Studio ready",
                        wrap=False,
                        style={"flex": 1, "min_width": 0},
                    )
                    dg.Tag("layout stress", level="neutral")

    state.modal = dg.Modal(
        "Launch release workflow",
        open=False,
        width=560,
        height=320,
        parent=window,
        id="nexus-modal",
    )
    with state.modal:
        dg.Label(
            "Stage a release while verifying that controls and actions remain bounded at compact sizes.",
            class_="page-description",
        )
        dg.TextInput("catalog.search.release-24.7", placeholder="Release route")
        dg.Dropdown(("Canary", "Staged", "Full rollout"), value="Canary")
        dg.TextArea("Validate the current SLO window before promotion.", rows=4)
        dg.ToggleSwitch("Require owner approval", checked=True)
        with dg.FlowLayout(gap=8, row_gap=8, style={"justify_content": "flex_end"}):
            dg.SmallButton(
                "Cancel",
                id="nexus-modal-cancel",
                on_click=lambda: state.modal.close() if state.modal else None,
            )
            dg.Button(
                "Queue release",
                class_="primary",
                on_click=lambda: (
                    state.modal.close(),
                    set_status("Release queued", "success"),
                )
                if state.modal
                else None,
            )

    state.palette = dg.CommandPalette(
        [
            dg.Command("route.command", "Open Command", on_run=lambda: navigate("command")),
            dg.Command("route.data", "Open Data Lab", on_run=lambda: navigate("data")),
            dg.Command("route.workflow", "Open Workflow", on_run=lambda: navigate("workflow")),
            dg.Command("route.controls", "Open Controls", on_run=lambda: navigate("controls")),
            dg.Command("route.diagnostics", "Open Diagnostics", on_run=lambda: navigate("diagnostics")),
            dg.Command("release.launch", "Launch release workflow", on_run=show_modal),
            dg.Command("jobs.refresh", "Refresh job stream", on_run=refresh_jobs),
        ],
        open=False,
        max_results=8,
        parent=window,
        id="nexus-palette",
    )
    return window


def build_app(style: str = "nexus") -> tuple[dg.App, dg.Window]:
    themes = {
        "nexus": NEXUS_THEME,
        "windows-3.11": WINDOWS_311_THEME,
        "mac-os-90s": MAC_OS_90S_THEME,
    }
    if style not in themes:
        raise ValueError("style must be 'nexus', 'windows-3.11', or 'mac-os-90s'")

    app = dg.App(theme=themes[style])
    state.app = app
    retro_stylesheets = {
        "windows-3.11": WINDOWS_311_CSS,
        "mac-os-90s": MAC_OS_90S_CSS,
    }
    if style in retro_stylesheets:
        app.stylesheet(retro_stylesheets[style])
        return app, build_window()

    app.stylesheet(
        """
        :root {
            --nx-bg: #080a12;
            --nx-surface: rgba(18, 21, 35, 0.94);
            --nx-surface-soft: rgba(27, 31, 49, 0.84);
            --nx-line: rgba(177, 185, 235, 0.16);
            --nx-text: rgba(247, 248, 255, 0.96);
            --nx-muted: rgba(205, 211, 235, 0.66);
            --nx-purple: #8b7cff;
            --nx-cyan: #44d7e8;
            --nx-shadow-contact: rgba(0, 0, 0, 0.22);
            --nx-shadow-ambient: rgba(0, 0, 0, 0.10);
            --nx-highlight-soft: rgba(255, 255, 255, 0.055);
        }

        Window {
            background:
                radial-gradient(circle at 82% 5%, rgba(94, 76, 210, 0.22), transparent 34%),
                radial-gradient(circle at 18% 92%, rgba(34, 166, 184, 0.12), transparent 38%),
                linear-gradient(145deg, #080a12, #0d101b 58%, #090c15);
            color: var(--nx-text);
            font-size: 13px;
        }

        AppShell.nexus-shell { background: transparent; }

        Sidebar.nexus-sidebar {
            background:
                linear-gradient(180deg, rgba(15, 17, 29, 0.99), rgba(9, 11, 20, 0.99));
            border: 1px solid var(--nx-line);
            border-radius: 0;
            padding: 16px;
            gap: 8px;
        }

        WorkbenchLayout.nexus-workbench { min-width: 0; min-height: 0; }

        MenuBar {
            background: rgba(10, 12, 21, 0.72);
            border-color: var(--nx-line);
        }

        Toolbar.nexus-toolbar {
            background: rgba(19, 22, 36, 0.86);
            border: 1px solid var(--nx-line);
            border-radius: 10px;
            padding: 5px 7px;
            gap: 6px;
        }

        Tabs {
            background: rgba(13, 16, 27, 0.76);
            border-color: var(--nx-line);
        }

        StatusBar.nexus-status {
            background: rgba(10, 12, 21, 0.78);
            border-color: var(--nx-line);
        }

        SearchBox.global-search { width: 300px; }

        ScrollArea.nexus-page-scroll {
            padding: 6px 10px 22px 4px;
            gap: 14px;
        }

        ScrollArea#diagnostics-scroll { gap: var(--dg-space-md); }

        Splitter.diagnostic-splitter::gutter {
            width: 2px;
            background: var(--nx-line);
        }

        Panel {
            background: var(--nx-surface);
            border: 1px solid var(--nx-line);
            border-radius: 11px;
            box-shadow:
                0 1px 2px var(--nx-shadow-contact),
                0 5px 14px var(--nx-shadow-ambient),
                inset 0 1px 0 var(--nx-highlight-soft);
        }

        Panel.sidebar-card {
            background: rgba(139, 124, 255, 0.08);
            border-color: rgba(139, 124, 255, 0.20);
            padding: 10px;
            box-shadow: none;
        }

        Panel Panel {
            box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.035);
        }

        Badge,
        Tag {
            box-shadow: none;
        }

        Badge.success {
            background: rgba(89, 211, 164, 0.82);
            border: 1px solid rgba(143, 238, 201, 0.32);
            color: #07150f;
        }

        Badge.info {
            background: rgba(157, 145, 255, 0.76);
            border: 1px solid rgba(205, 199, 255, 0.30);
            color: #0d0920;
        }

        Badge.warning {
            background: rgba(244, 184, 74, 0.80);
            border: 1px solid rgba(255, 218, 149, 0.32);
            color: #1a1002;
        }

        .sidebar-subtitle,
        .muted,
        .page-description {
            color: var(--nx-muted);
        }

        .sidebar-subtitle { font-size: 11px; }
        .sidebar-card-title { font-weight: 760; }

        .sidebar-section,
        .kicker {
            color: rgba(139, 124, 255, 0.86);
            font-size: 10px;
            font-weight: 860;
            letter-spacing: 1.2px;
        }

        .page-heading { gap: 4px; padding-bottom: 2px; }
        .page-title { color: white; font-size: 23px; font-weight: 880; }

        Panel.hero-card {
            background:
                radial-gradient(circle at 82% 14%, rgba(68, 215, 232, 0.14), transparent 38%),
                linear-gradient(135deg, rgba(38, 33, 74, 0.98), rgba(18, 25, 42, 0.98));
            border-color: rgba(139, 124, 255, 0.30);
            padding: 17px;
        }

        .hero-copy { flex: 1; min-width: 250px; gap: 4px; }
        .hero-title { color: white; font-size: 20px; font-weight: 880; }

        Button.primary {
            background: linear-gradient(135deg, #9d91ff, #7b69f0);
            border-color: rgba(205, 199, 255, 0.70);
            color: #0b081a;
            font-weight: 850;
            box-shadow:
                0 2px 5px rgba(76, 59, 181, 0.22),
                inset 0 1px 0 rgba(255, 255, 255, 0.24);
        }

        Button.primary:hover {
            box-shadow:
                0 3px 8px rgba(76, 59, 181, 0.26),
                inset 0 1px 0 rgba(255, 255, 255, 0.28);
        }

        Button.primary:active {
            box-shadow: inset 0 1px 2px rgba(25, 18, 72, 0.22);
        }

        Panel.metric-card { min-height: 142px; }
        .metric-header { min-height: 20px; }
        .metric-label {
            color: rgba(221, 224, 244, 0.72);
            font-size: 10px;
            font-weight: 820;
            text-transform: uppercase;
        }
        .metric-value { color: white; font-size: 27px; font-weight: 900; }
        .metric-progress { height: 6px; }

        .capacity-row { gap: 4px; }
        .capacity-heading { min-height: 28px; gap: 7px; }
        .mono { color: rgba(222, 225, 246, 0.84); font-family: "Consolas"; }

        Panel.chart-card,
        Panel.capacity-card { min-height: 365px; }
        Panel.small-chart-card { min-height: 315px; }
        Panel.table-card { min-height: 400px; }
        Panel.filter-card { padding: 12px; }
        Panel.analysis-card { min-height: 365px; }
        Panel.workflow-card { min-height: 390px; }
        Panel.workflow-code-card { min-height: 470px; }
        Panel.control-card { min-height: 330px; }
        Panel.diagnostic-card { min-height: 300px; }

        DataFrameTable { min-height: 180px; }

        .check-row { gap: 9px; min-height: 42px; }
        .check-copy { flex: 1; min-width: 0; gap: 2px; }

        @media (max-width: 760px) {
            Toolbar.nexus-toolbar { padding: 4px; }
            SearchBox.global-search { width: 100%; flex-basis: 100%; }
            .page-heading Tag { display: none; }
            StatusBar.nexus-status Tag { display: none; }
            ScrollArea.nexus-page-scroll { padding: 5px 8px 18px 2px; gap: 10px; }
            .page-title { font-size: 18px; }
            .hero-title { font-size: 16px; }
            .hero-copy { min-width: 0; }
            Panel.metric-card,
            Panel.chart-card,
            Panel.capacity-card,
            Panel.small-chart-card,
            Panel.table-card,
            Panel.analysis-card,
            Panel.workflow-card,
            Panel.workflow-code-card,
            Panel.control-card,
            Panel.diagnostic-card {
                min-height: auto;
            }
        }
        """
    )
    return app, build_window()


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the Nexus Studio DragonGui stress demo.")
    parser.add_argument(
        "--style",
        choices=("nexus", "windows-3.11", "mac-os-90s"),
        default="nexus",
        help="Visual style to apply (default: nexus).",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    app, window = build_app(args.style)
    print(app.run(window))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
