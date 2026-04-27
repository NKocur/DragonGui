from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual demo guard
    raise SystemExit("css_design_system_demo.py requires NumPy") from exc


class DemoFrame:
    columns = ("x", "y", "z", "signal", "score", "segment", "active")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "str", "bool")

    def __init__(self, rows: int = 24_000, phase: float = 0.0) -> None:
        self.shape = (rows, len(self.columns))
        t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
        theta = t * np.float32(math.tau * 10.0 + phase)
        radius = np.float32(0.7) + t * np.float32(2.6)
        self.x = np.cos(theta) * radius
        self.y = np.sin(theta * np.float32(0.71)) * np.float32(2.4)
        self.z = (t - np.float32(0.5)) * np.float32(7.0)
        self.signal = np.sin(theta).astype(np.float32)
        self.score = np.cos(theta * np.float32(0.27)).astype(np.float32)
        self.segment = np.where(self.signal > 0.5, "hot", np.where(self.signal < -0.5, "cold", "nominal"))
        self.active = self.score > 0.72

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


CSS_DENSE = """
:root {
    --panel-radius: 0px;
    --control-radius: 0px;
}

Window {
    background: #111315;
}

MenuBar,
StatusBar {
    background: #171a1d;
    border-color: #333940;
}

HLayout.app-shell {
    padding: 8px;
    gap: 8px;
}

VLayout.workspace {
    flex-grow: 1;
    gap: 8px;
}

HLayout.metrics,
HLayout.lower-grid {
    gap: 8px;
}

HLayout.metrics {
    height: 82px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel {
    padding: 8px;
    gap: 6px;
    background: #1b1f22;
    border: 1px solid #3a424a;
    border-radius: var(--panel-radius);
    color: #d7dde2;
    font-size: 12px;
}

Panel.rail {
    width: 292px;
    background: #15191c;
    border-color: #d8a21b;
}

Panel.hero {
    height: 218px;
    background: #20252a;
}

Panel.card {
    flex-grow: 1;
    padding: 7px;
    gap: 3px;
    background: #15191c;
    border-color: #57606a;
}

VLayout.plot-stack {
    height: 300px;
    gap: 0px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel.plot-body {
    flex-grow: 1;
    padding: 0px;
    gap: 0px;
    background: #080a0c;
    border: 1px solid #57606a;
    border-radius: 0px;
}

Panel.table-panel {
    width: 520px;
    height: 198px;
}

Panel.table-frame {
    flex-grow: 1;
    padding: 0px;
    gap: 0px;
    background: #101214;
    border: 1px solid #57606a;
    border-radius: 0px;
}

Panel.selector-panel {
    flex-grow: 1;
}

Panel.section-header {
    height: 30px;
    flex-grow: 0;
    flex-shrink: 0;
    padding-left: 8px;
    padding-right: 8px;
    padding-top: 4px;
    padding-bottom: 4px;
    gap: 0px;
    background: #101214;
    border: 1px solid #57606a;
    border-radius: 0px;
}

Label {
    color: #d7dde2;
    font-size: 12px;
}

Label.eyebrow {
    color: #d8a21b;
    font-weight: 800;
}

Label.section-title {
    height: 22px;
    color: #d8a21b;
    font-weight: 800;
}

Label.headline {
    height: 32px;
    color: #ffffff;
    font-size: 18px;
    font-weight: 800;
}

Label.hero-copy {
    height: 24px;
    color: #93a0aa;
    font-size: 11px;
}

Label.metric-title {
    height: 16px;
    color: #d8a21b;
    font-size: 10px;
    font-weight: 800;
}

Label.metric {
    height: 24px;
    color: #ffffff;
    font-size: 16px;
    font-weight: 800;
}

Label.metric-hint {
    height: 16px;
    color: #93a0aa;
    font-size: 10px;
}

Label.hint {
    color: #93a0aa;
}

Button,
Dropdown,
TextInput,
NumberInput {
    height: 30px;
    background: #101214;
    border: 1px solid #59636d;
    border-radius: var(--control-radius);
    color: #e8edf2;
    font-size: 12px;
}

Panel.rail > Button {
    height: 28px;
}

Button:hover,
Dropdown:hover {
    background: #272d33;
    border-color: #d8a21b;
}

Dropdown::chevron {
    width: 18px;
    color: #d8a21b;
}

Dropdown::menu {
    background: #0f1215;
    border-color: #d8a21b;
    border-radius: 2px;
}

Dropdown::item {
    background: #151a1f;
    color: #e8edf2;
    padding: 9px;
}

Dropdown::item-selected {
    background: #2b2514;
    color: #ffffff;
}

Dropdown::item-hover {
    background: #433513;
    color: #ffffff;
}

Button.primary {
    background: #d8a21b;
    border-color: #d8a21b;
    color: #111315;
    font-weight: 800;
}

Button.danger {
    border-color: #e35d5d;
    color: #ffdede;
}

TextInput:focus,
NumberInput:focus {
    border-color: #61d394;
}

NumberInput::stepper {
    width: 22px;
    background: #232a30;
    color: #d8a21b;
}

NumberInput::stepper-divider {
    background: #59636d;
}

Slider {
    accent: #d8a21b;
    track-color: #343c44;
    thumb-color: #eeeeee;
}

Slider::track {
    height: 3px;
    background: #343c44;
}

Slider::fill {
    background: #d8a21b;
}

Slider::thumb {
    width: 12px;
    height: 18px;
    background: #d8a21b;
    border-color: #111315;
    border-radius: 2px;
}

Checkbox,
ProgressBar {
    accent: #61d394;
}

Checkbox::row {
    background: #1a211f;
}

Checkbox::box {
    background: #0e1214;
    border-color: #61d394;
    border-radius: 1px;
}

Checkbox:checked::indicator {
    background: #d8a21b;
}

Checkbox::label {
    color: #d7dde2;
}

ProgressBar::track {
    background: #101214;
    border-color: #59636d;
    border-radius: 2px;
}

ProgressBar::fill {
    background: #61d394;
    height: 8px;
    border-radius: 1px;
}

ProgressBar::label {
    color: #ffffff;
    font-weight: 800;
}

Tab {
    height: 30px;
    border-radius: 0px;
    color: #d7dde2;
}

Tabs::header {
    height: 30px;
    background: #161b1f;
    border-color: #57606a;
}

Tab::tab {
    background: #20262d;
    border-color: #57606a;
    border-radius: 0px;
    color: #d7dde2;
    padding: 8px;
}

Tab::accent {
    background: #d8a21b;
    height: 4px;
}

NavItem::item {
    background: #20262d;
    color: #d7dde2;
    padding: 8px;
}

NavItem::accent {
    background: #d8a21b;
    width: 4px;
    border-radius: 0px;
}

DataFrameTable {
    table-row-height: 22px;
    table-header-height: 26px;
    border-color: #57606a;
    border-width: 1px;
}

DataFrameTable::header {
    background: #222a31;
    color: #ffffff;
    font-weight: 800;
}

DataFrameTable::row {
    background: #161b1f;
    color: #cdd5dc;
}

DataFrameTable::row-selected {
    background: #2c3d36;
    color: #ffffff;
}

DataFrameTable::grid-line {
    background: #57606a;
    width: 1px;
}

Scatter3D {
    border-color: #57606a;
    border-width: 1px;
}
"""


CSS_PRESENTATION = """
:root {
    --panel-radius: 22px;
    --control-radius: 18px;
}

Window {
    background: #f4efe6;
}

MenuBar,
StatusBar {
    background: #fffaf1;
    border-color: #d7c5a8;
}

HLayout.app-shell {
    padding: 22px;
    gap: 22px;
}

VLayout.workspace {
    flex-grow: 1;
    gap: 18px;
}

HLayout.metrics,
HLayout.lower-grid {
    gap: 16px;
}

HLayout.metrics {
    height: 98px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel {
    padding: 18px;
    gap: 12px;
    background: #fffaf1;
    border: 1px solid #d7c5a8;
    border-radius: var(--panel-radius);
    color: #24313d;
    font-size: 16px;
}

Panel.rail {
    width: 372px;
    background: #fcf1dc;
    border-color: #c06a2f;
}

Panel.hero {
    height: 280px;
    padding: 16px;
    gap: 10px;
    background: #fffdf8;
}

Panel.card {
    flex-grow: 1;
    padding: 9px;
    gap: 4px;
    background: #f7e7cd;
    border-color: #c06a2f;
}

VLayout.plot-stack {
    height: 232px;
    gap: 0px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel.plot-body {
    flex-grow: 1;
    padding: 0px;
    gap: 0px;
    background: #fffdf8;
    border: 1px solid #bda98d;
    border-radius: 22px;
}

Panel.table-panel {
    width: 600px;
    height: 170px;
}

Panel.table-frame {
    flex-grow: 1;
    padding: 0px;
    gap: 0px;
    background: #ffffff;
    border: 1px solid #bda98d;
    border-radius: 18px;
}

Panel.selector-panel {
    flex-grow: 1;
}

Panel.section-header {
    height: 42px;
    flex-grow: 0;
    flex-shrink: 0;
    padding-left: 16px;
    padding-right: 16px;
    padding-top: 8px;
    padding-bottom: 8px;
    gap: 0px;
    background: #f7e7cd;
    border: 1px solid #c06a2f;
    border-radius: 18px;
}

Label {
    color: #24313d;
    font-size: 16px;
}

Label.eyebrow {
    color: #c06a2f;
    font-size: 14px;
    font-weight: 800;
}

Label.section-title {
    height: 28px;
    color: #c06a2f;
    font-size: 18px;
    font-weight: 800;
}

Label.headline {
    height: 38px;
    color: #182433;
    font-size: 24px;
    font-weight: 800;
}

Label.hero-copy {
    height: 24px;
    color: #6c5f50;
    font-size: 13px;
}

Label.metric-title {
    height: 18px;
    color: #c06a2f;
    font-size: 12px;
    font-weight: 800;
}

Label.metric {
    height: 32px;
    color: #182433;
    font-size: 22px;
    font-weight: 800;
}

Label.metric-hint {
    height: 18px;
    color: #6c5f50;
    font-size: 12px;
}

Label.hint {
    color: #6c5f50;
}

Button,
Dropdown,
TextInput,
NumberInput {
    height: 48px;
    background: #ffffff;
    border: 1px solid #bda98d;
    border-radius: var(--control-radius);
    color: #182433;
    font-size: 16px;
}

Panel.rail > Button {
    height: 50px;
}

Button:hover,
Dropdown:hover {
    background: #ffe5c4;
    border-color: #c06a2f;
}

Dropdown::chevron {
    width: 28px;
    color: #c06a2f;
}

Dropdown::menu {
    background: #fff8ed;
    border-color: #c06a2f;
    border-radius: 14px;
}

Dropdown::item {
    background: #fff3df;
    color: #182433;
    padding: 14px;
}

Dropdown::item-selected {
    background: #c06a2f;
    color: #ffffff;
}

Dropdown::item-hover {
    background: #ffe5c4;
    color: #182433;
}

Button.primary {
    background: #c06a2f;
    border-color: #c06a2f;
    color: #ffffff;
    font-weight: 800;
}

Button.danger {
    background: #8f2f28;
    border-color: #8f2f28;
    color: #ffffff;
}

TextInput:focus,
NumberInput:focus {
    border-color: #2c7d8d;
}

NumberInput::stepper {
    width: 38px;
    background: #ead8bd;
    color: #8f2f28;
    border-top-right-radius: var(--control-radius);
    border-bottom-right-radius: var(--control-radius);
}

NumberInput::stepper-divider {
    background: #bda98d;
}

Slider {
    accent: #c06a2f;
    track-color: #d7c5a8;
    thumb-color: #ffffff;
}

Slider::track {
    height: 10px;
    background: #d7c5a8;
    border-radius: 999px;
}

Slider::fill {
    background: #c06a2f;
    border-radius: 999px;
}

Slider::thumb {
    width: 24px;
    height: 24px;
    background: #ffffff;
    border-color: #c06a2f;
    border-width: 2px;
    border-radius: 999px;
}

Checkbox,
ProgressBar {
    accent: #2c7d8d;
}

Checkbox::box {
    background: #fff8ed;
    border-color: #2c7d8d;
    border-radius: 8px;
}

Checkbox:checked::indicator {
    background: #c06a2f;
    border-radius: 999px;
}

Checkbox::label {
    color: #24313d;
}

ProgressBar::track {
    background: #fff8ed;
    border-color: #bda98d;
    border-radius: 999px;
}

ProgressBar::fill {
    background: #2c7d8d;
    height: 12px;
    border-radius: 999px;
}

ProgressBar::label {
    color: #182433;
    font-weight: 800;
}

Tab {
    height: 44px;
    border-radius: 18px;
    color: #24313d;
}

Tabs::header {
    height: 46px;
    background: #efe2cf;
    border-color: #bda98d;
}

Tab::tab {
    background: #fff8ed;
    border-color: #bda98d;
    border-radius: 18px;
    color: #24313d;
    padding: 16px;
}

Tab::accent {
    background: #c06a2f;
    height: 6px;
    border-radius: 999px;
}

NavItem::item {
    background: #fff8ed;
    color: #24313d;
    padding: 16px;
    border-radius: 18px;
}

NavItem::accent {
    background: #c06a2f;
    width: 6px;
    border-radius: 999px;
}

DataFrameTable {
    table-row-height: 34px;
    table-header-height: 42px;
    border-color: #bda98d;
    border-width: 1px;
}

DataFrameTable::header {
    background: #ead8bf;
    color: #182433;
    font-weight: 800;
}

DataFrameTable::row {
    background: #fff8ed;
    color: #24313d;
}

DataFrameTable::row-selected {
    background: #d7ebee;
    color: #112c33;
}

DataFrameTable::grid-line {
    background: #bda98d;
    width: 1px;
}

Scatter3D {
    border-color: #bda98d;
    border-width: 1px;
}
"""


CSS_MISSION = """
:root {
    --panel-radius: 8px;
    --control-radius: 6px;
}

Window {
    background: #03070b;
}

MenuBar,
StatusBar {
    background: #07111c;
    border-color: #ff3535;
}

HLayout.app-shell {
    padding: 14px;
    gap: 14px;
}

VLayout.workspace {
    flex-grow: 1;
    gap: 14px;
}

HLayout.metrics,
HLayout.lower-grid {
    gap: 12px;
}

HLayout.metrics {
    height: 92px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel {
    padding: 12px;
    gap: 8px;
    background: #07111c;
    border: 2px solid #1bb7ff;
    border-radius: var(--panel-radius);
    accent: #ff3535;
    color: #d8f2ff;
    font-size: 14px;
}

Panel.rail {
    width: 315px;
    background: #0b1017;
    border-color: #ff3535;
}

Panel.hero {
    height: 250px;
    background: #071827;
}

Panel.card {
    flex-grow: 1;
    padding: 8px;
    gap: 4px;
    background: #0c1624;
    border-color: #ffd23c;
}

VLayout.plot-stack {
    height: 270px;
    gap: 0px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel.plot-body {
    flex-grow: 1;
    padding: 0px;
    gap: 0px;
    background: #03070b;
    border: 2px solid #1bb7ff;
    border-radius: 8px;
}

Panel.table-panel {
    width: 570px;
    height: 176px;
}

Panel.table-frame {
    flex-grow: 1;
    padding: 0px;
    gap: 0px;
    background: #07111c;
    border: 2px solid #1bb7ff;
    border-radius: 8px;
}

Panel.selector-panel {
    flex-grow: 1;
}

Panel.section-header {
    height: 38px;
    flex-grow: 0;
    flex-shrink: 0;
    padding-left: 12px;
    padding-right: 12px;
    padding-top: 6px;
    padding-bottom: 6px;
    gap: 0px;
    background: #0d1c2d;
    border: 2px solid #ffd23c;
    border-radius: 6px;
}

Label {
    color: #d8f2ff;
}

Label.eyebrow {
    color: #ffd23c;
    font-weight: 800;
}

Label.section-title {
    height: 30px;
    color: #ffd23c;
    font-size: 15px;
    font-weight: 800;
}

Label.headline {
    height: 34px;
    color: #ffffff;
    font-size: 20px;
    font-weight: 800;
}

Label.hero-copy {
    height: 22px;
    color: #78a8c0;
    font-size: 12px;
}

Label.metric-title {
    height: 18px;
    color: #ffd23c;
    font-size: 11px;
    font-weight: 800;
}

Label.metric {
    height: 30px;
    color: #ffd23c;
    font-size: 20px;
    font-weight: 800;
}

Label.metric-hint {
    height: 18px;
    color: #78a8c0;
    font-size: 11px;
}

Label.hint {
    color: #78a8c0;
}

Button,
Dropdown,
TextInput,
NumberInput {
    height: 38px;
    background: #0d1c2d;
    border: 2px solid #1bb7ff;
    border-radius: var(--control-radius);
    color: #ffffff;
    font-weight: 700;
}

Panel.rail > Button {
    height: 40px;
}

Button:hover,
Dropdown:hover {
    background: #133756;
    border-color: #ffd23c;
}

Dropdown::chevron {
    width: 24px;
    color: #ffd23c;
}

Dropdown::menu {
    background: #07111d;
    border-color: #1bb7ff;
    border-radius: 4px;
}

Dropdown::item {
    background: #0d1c2d;
    color: #ffffff;
    padding: 10px;
}

Dropdown::item-selected {
    background: #ff3535;
    color: #ffffff;
}

Dropdown::item-hover {
    background: #123b5e;
    color: #ffd23c;
}

Button.primary {
    background: #ff3535;
    border-color: #ff3535;
    color: #ffffff;
}

Button.danger {
    background: #ffd23c;
    border-color: #ffd23c;
    color: #03070b;
}

TextInput:focus,
NumberInput:focus {
    border-color: #55ff9b;
}

NumberInput::stepper {
    width: 34px;
    background: #07111d;
    color: #ffd23c;
}

NumberInput::stepper-up {
    border-top-right-radius: var(--control-radius);
}

NumberInput::stepper-down {
    border-bottom-right-radius: var(--control-radius);
}

NumberInput::stepper-divider {
    background: #1bb7ff;
}

Slider {
    accent: #ff3535;
    track-color: #16324c;
    thumb-color: #ffd23c;
}

Slider::track {
    height: 6px;
    background: #16324c;
    border-color: #1bb7ff;
    border-width: 1px;
    border-radius: 2px;
}

Slider::fill {
    background: #ff3535;
    border-radius: 2px;
}

Slider::thumb {
    width: 18px;
    height: 26px;
    background: #ffd23c;
    border-color: #03070b;
    border-radius: 3px;
}

Checkbox,
ProgressBar {
    accent: #55ff9b;
}

Checkbox::row {
    background: #081725;
}

Checkbox::box {
    background: #03070b;
    border-color: #1bb7ff;
    border-radius: 3px;
}

Checkbox:checked::indicator {
    background: #ffd23c;
    border-radius: 2px;
}

Checkbox::label {
    color: #ffffff;
}

ProgressBar::track {
    background: #07111d;
    border-color: #1bb7ff;
    border-radius: 3px;
}

ProgressBar::fill {
    background: #ff3535;
    height: 10px;
    border-radius: 2px;
}

ProgressBar::label {
    color: #ffd23c;
    font-weight: 800;
}

Tab {
    height: 38px;
    border-radius: 6px;
    accent: #ff3535;
    color: #ffffff;
}

Tabs::header {
    height: 40px;
    background: #07111d;
    border-color: #1bb7ff;
}

Tab::tab {
    background: #081725;
    border-color: #1bb7ff;
    border-width: 2px;
    border-radius: 6px;
    color: #ffffff;
    padding: 10px;
}

Tab::accent {
    background: #ff3535;
    height: 5px;
    border-radius: 2px;
}

NavItem::item {
    background: #081725;
    color: #ffffff;
    padding: 10px;
    border-color: #1bb7ff;
    border-width: 2px;
    border-radius: 4px;
}

NavItem::accent {
    background: #ff3535;
    width: 7px;
    border-radius: 2px;
}

DataFrameTable {
    table-row-height: 26px;
    table-header-height: 32px;
    border-color: #1bb7ff;
    border-width: 2px;
}

DataFrameTable::header {
    background: #0c2840;
    color: #ffd23c;
    font-weight: 900;
}

DataFrameTable::row {
    background: #07111d;
    color: #c7f2ff;
}

DataFrameTable::row-selected {
    background: #3a1620;
    color: #ffffff;
}

DataFrameTable::grid-line {
    background: #1bb7ff;
    width: 2px;
}

Scatter3D {
    border-color: #1bb7ff;
    border-width: 2px;
}
"""


CSS_GLASSMORPHIC = """
:root {
    --panel-radius: 18px;
    --control-radius: 12px;
    --glass-bg: #0d1117cc;
    --glass-border: #ffffff18;
    --glass-bright: #ffffff26;
    --neon-blue: #58a6ff;
    --neon-green: #3fb950;
    --neon-pink: #f778ba;
    --neon-amber: #d29922;
    --surface-dim: #010409;
    --text-primary: #f0f6fc;
    --text-secondary: #8b949e;
    --text-muted: #484f58;
}

Window {
    background: var(--surface-dim);
}

MenuBar,
StatusBar {
    background: #010409e8;
    border-color: var(--glass-border);
}

HLayout.app-shell {
    padding: 16px;
    gap: 16px;
}

VLayout.workspace {
    flex-grow: 1;
    gap: 16px;
}

HLayout.metrics,
HLayout.lower-grid {
    gap: 14px;
}

HLayout.metrics {
    height: 94px;
    flex-grow: 0;
    flex-shrink: 0;
}

/* ── panels ────────────────────────────────────────────────────────────── */

Panel {
    padding: 14px;
    gap: 10px;
    background: var(--glass-bg);
    border: 1px solid var(--glass-border);
    border-radius: var(--panel-radius);
    color: var(--text-primary);
    font-size: 13px;
}

Panel.rail {
    width: 320px;
    background: #0d1117e8;
    border-color: var(--neon-blue);
    accent: var(--neon-blue);
}

Panel.hero {
    height: 240px;
    padding: 14px;
    gap: 8px;
    background: #0d1117f0;
    border-color: var(--glass-bright);
}

Panel.card {
    flex-grow: 1;
    padding: 8px;
    gap: 3px;
    background: #161b2280;
    border-color: var(--glass-bright);
}

VLayout.plot-stack {
    height: 280px;
    gap: 0px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel.plot-body {
    flex-grow: 1;
    padding: 0px;
    gap: 0px;
    background: var(--surface-dim);
    border: 1px solid var(--glass-border);
    border-radius: var(--panel-radius);
}

Panel.table-panel {
    width: 560px;
    height: 186px;
}

Panel.table-frame {
    flex-grow: 1;
    padding: 0px;
    gap: 0px;
    background: #010409e0;
    border: 1px solid var(--glass-border);
    border-radius: 14px;
}

Panel.selector-panel {
    flex-grow: 1;
}

Panel.section-header {
    height: 34px;
    flex-grow: 0;
    flex-shrink: 0;
    padding-left: 12px;
    padding-right: 12px;
    padding-top: 6px;
    padding-bottom: 6px;
    gap: 0px;
    background: #161b2280;
    border: 1px solid var(--glass-bright);
    border-radius: 10px;
}

/* ── typography ────────────────────────────────────────────────────────── */

Label {
    color: var(--text-primary);
    font-size: 13px;
}

Label.eyebrow {
    color: var(--neon-blue);
    font-size: 11px;
    font-weight: 800;
}

Label.section-title {
    height: 24px;
    color: var(--neon-blue);
    font-size: 13px;
    font-weight: 800;
}

Label.headline {
    height: 36px;
    color: #ffffff;
    font-size: 22px;
    font-weight: 800;
}

Label.hero-copy {
    height: 22px;
    color: var(--text-secondary);
    font-size: 12px;
}

Label.metric-title {
    height: 16px;
    color: var(--neon-green);
    font-size: 10px;
    font-weight: 800;
}

Label.metric {
    height: 28px;
    color: #ffffff;
    font-size: 18px;
    font-weight: 800;
}

Label.metric-hint {
    height: 16px;
    color: var(--text-muted);
    font-size: 10px;
}

Label.hint {
    color: var(--text-secondary);
}

/* ── interactive controls ──────────────────────────────────────────────── */

Button,
Dropdown,
TextInput,
NumberInput {
    height: 34px;
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: var(--control-radius);
    color: var(--text-primary);
    font-size: 13px;
}

/* direct-child: rail buttons are taller with neon accent border */
Panel.rail > Button {
    height: 36px;
    border-color: var(--neon-blue);
}

/* multi-class: primary buttons use the green neon */
Button.primary {
    background: var(--neon-green);
    border-color: var(--neon-green);
    color: var(--surface-dim);
    font-weight: 800;
}

/* multi-class: danger buttons use the pink neon */
Button.danger {
    background: var(--neon-pink);
    border-color: var(--neon-pink);
    color: var(--surface-dim);
    font-weight: 800;
}

/* pseudo-state: hover lifts the border to neon blue */
Button:hover,
Dropdown:hover {
    background: #1c2333;
    border-color: var(--neon-blue);
}

Dropdown::chevron {
    width: 22px;
    color: var(--neon-blue);
}

Dropdown::menu {
    background: #0d1117;
    border-color: var(--neon-blue);
    border-radius: var(--control-radius);
}

Dropdown::item {
    background: #161b22;
    color: var(--text-primary);
    padding: 10px;
}

Dropdown::item-selected {
    background: var(--neon-blue);
    color: var(--surface-dim);
}

Dropdown::item-hover {
    background: #1c2333;
    color: var(--neon-green);
}

/* pseudo-state: focus uses the neon green for inputs */
TextInput:focus,
NumberInput:focus {
    border-color: var(--neon-green);
}

NumberInput::stepper {
    width: 30px;
    background: #0d1117;
    color: var(--neon-blue);
}

NumberInput::stepper-up {
    border-top-right-radius: var(--control-radius);
}

NumberInput::stepper-down {
    border-bottom-right-radius: var(--control-radius);
}

NumberInput::stepper-divider {
    background: var(--neon-blue);
}

/* pseudo-state: disabled dims to a muted transparent fill */
Button:disabled,
Dropdown:disabled,
TextInput:disabled,
NumberInput:disabled {
    background: #21262d40;
    border-color: var(--text-muted);
    opacity: 0.48;
}

Slider {
    accent: var(--neon-blue);
    track-color: #30363d;
    thumb-color: var(--neon-green);
}

Slider::track {
    height: 8px;
    background: #30363d;
    border-radius: 999px;
}

Slider::fill {
    background: var(--neon-blue);
    border-radius: 999px;
}

Slider::thumb {
    width: 20px;
    height: 20px;
    background: var(--neon-green);
    border-color: var(--surface-dim);
    border-width: 2px;
    border-radius: 6px;
}

Checkbox {
    accent: var(--neon-green);
    color: var(--text-primary);
}

Checkbox::row {
    background: #111827;
}

Checkbox::box {
    background: #0d1117;
    border-color: var(--neon-green);
    border-radius: 4px;
}

Checkbox:checked::indicator {
    background: var(--neon-pink);
    border-radius: 999px;
}

Checkbox::label {
    color: var(--text-secondary);
}

ProgressBar {
    accent: var(--neon-blue);
    background: #161b22;
    border-color: #30363d;
    border-radius: var(--control-radius);
}

ProgressBar::track {
    background: #0d1117;
    border-color: #30363d;
    border-radius: var(--control-radius);
}

ProgressBar::fill {
    background: var(--neon-blue);
    height: 9px;
    border-radius: var(--control-radius);
}

ProgressBar::label {
    color: var(--neon-green);
    font-weight: 800;
}

/* ── tabs ──────────────────────────────────────────────────────────────── */

Tab {
    height: 32px;
    border-radius: 10px;
    accent: var(--neon-blue);
    color: var(--text-primary);
}

Tabs::header {
    height: 34px;
    background: #0d1117;
    border-color: #30363d;
}

Tab::tab {
    background: #161b22;
    border-color: #30363d;
    border-radius: 10px;
    color: var(--text-primary);
    padding: 10px;
}

Tab::accent {
    background: var(--neon-pink);
    height: 4px;
    border-radius: 999px;
}

NavItem::item {
    background: #161b22;
    color: var(--text-primary);
    padding: 10px;
    border-radius: 10px;
}

NavItem::accent {
    background: var(--neon-blue);
    width: 5px;
    border-radius: 999px;
}

/* ── data widgets ─────────────────────────────────────────────────────── */

DataFrameTable {
    table-row-height: 24px;
    table-header-height: 28px;
    border-color: #30363d;
    border-width: 1px;
}

DataFrameTable::header {
    background: #161b22;
    color: var(--neon-blue);
    font-weight: 900;
}

DataFrameTable::row {
    background: #0d1117;
    color: var(--text-secondary);
}

DataFrameTable::row-selected {
    background: #1c2d3f;
    color: var(--text-primary);
}

DataFrameTable::grid-line {
    background: #30363d;
    width: 1px;
}

Scatter3D {
    border-color: #30363d;
    border-width: 1px;
}
"""


CSS_MODES = {
    "dense": CSS_DENSE,
    "presentation": CSS_PRESENTATION,
    "mission": CSS_MISSION,
    "glass": CSS_GLASSMORPHIC,
}

MODE_COPY = {
    "dense": "Compact spacing, square controls, short table rows.",
    "presentation": "Roomy layout, large type, rounded cards.",
    "mission": "High contrast, thick borders, strong hover/focus.",
    "glass": "Neon-on-dark glass, deep variable cascade, multi-state coverage.",
}


app = dg.App(theme=dg.Theme.dark(accent="#1bb7ff", focus="#ffd23c"))
app.stylesheet(CSS_DENSE)
win = dg.Window("DragonGUI CSS Design System Demo", width=1280, height=820)
frame = DemoFrame()


def apply_mode(name: str) -> None:
    app.stylesheet(CSS_MODES[name])
    mode_label.set_value(MODE_COPY[name])
    status.set_value(f"CSS mode: {name}")


def print_css_snapshot() -> None:
    snapshot = app.debug_snapshot()
    styles = snapshot.get("gpu", {}).get("stylesheets", {})
    status.set_value(
        f"rules user={styles.get('user_rules')} warnings={styles.get('warning_count')}"
    )
    print("stylesheets:", styles)


with dg.MenuBar(class_="topbar"):
    with dg.Menu("CSS"):
        dg.MenuItem("Dense Operator", on_click=lambda: apply_mode("dense"))
        dg.MenuItem("Presentation", on_click=lambda: apply_mode("presentation"))
        dg.MenuItem("Mission Control", on_click=lambda: apply_mode("mission"))
        dg.MenuItem("Glassmorphic", on_click=lambda: apply_mode("glass"))
    with dg.Menu("Debug"):
        dg.MenuItem("Print style snapshot", on_click=print_css_snapshot)

with dg.HLayout(class_="app-shell"):
    with dg.Panel("Stylesheet Mode", class_="rail"):
        dg.Label("CSS DESIGN SYSTEM", class_="eyebrow")
        mode_label = dg.Label(MODE_COPY["dense"], class_="hint")
        dg.Button("Dense Operator", class_="primary mode", on_click=lambda: apply_mode("dense"))
        dg.Button("Presentation", class_="mode", on_click=lambda: apply_mode("presentation"))
        dg.Button("Mission Control", class_="danger mode", on_click=lambda: apply_mode("mission"))
        dg.Button("Glassmorphic", class_="mode", on_click=lambda: apply_mode("glass"))
        dg.Separator()
        dg.Label("Controls below are unchanged Python widgets.", class_="hint")
        dg.TextInput("Same TextInput")
        dg.Dropdown(["viridis", "magma", "plasma"], value="viridis")
        dg.NumberInput(42, min=0, max=100)
        dg.Slider(0.62)
        dg.Checkbox("Inherited text color", checked=True)
        dg.ProgressBar(0.74, label="74% pipeline")

    with dg.VLayout(class_="workspace"):
        with dg.Panel(class_="hero"):
            with dg.Panel(class_="section-header"):
                dg.Label("SYSTEM SUMMARY", class_="section-title")
            dg.Label("One Python tree, three design systems", class_="headline")
            dg.Label(
                "Only app.stylesheet(...) changes. Widths, padding, gaps, type, rows, borders, hover, and focus all come from CSS.",
                class_="hero-copy",
            )
            with dg.HLayout(class_="metrics"):
                with dg.Panel(class_="card"):
                    dg.Label("LATENCY", class_="metric-title")
                    dg.Label("18 ms", class_="metric")
                    dg.Label("frame budget", class_="metric-hint")
                with dg.Panel(class_="card"):
                    dg.Label("QUEUE", class_="metric-title")
                    dg.Label("0", class_="metric")
                    dg.Label("pending commands", class_="metric-hint")
                with dg.Panel(class_="card"):
                    dg.Label("ROWS", class_="metric-title")
                    dg.Label("24K", class_="metric")
                    dg.Label("native table/scatter", class_="metric-hint")

        with dg.VLayout(class_="plot-stack"):
            with dg.Panel(class_="section-header"):
                dg.Label("STYLED DATA SURFACE", class_="section-title")
            with dg.Panel(class_="plot-body"):
                dg.Scatter3D(frame, x="x", y="y", z="z", class_="plot-widget", colormap="magma")

        with dg.HLayout(class_="lower-grid"):
            with dg.Panel(class_="table-panel"):
                with dg.Panel(class_="section-header"):
                    dg.Label("VIRTUAL TABLE", class_="section-title")
                with dg.Panel(class_="table-frame"):
                    dg.DataFrameTable(frame, page_size=60, class_="metric-table")

            with dg.Panel(class_="selector-panel"):
                with dg.Panel(class_="section-header"):
                    dg.Label("SELECTOR COVERAGE", class_="section-title")
                dg.Label("Rules demonstrated:", class_="eyebrow")
                dg.Label("Types: Button, Panel, Table, Scatter3D")
                dg.Label("Classes: .rail, .card, .primary, .danger")
                dg.Label("Child selectors, states, variables")
                with dg.Tabs(value="css"):
                    dg.Tab("CSS", value="css")
                    dg.Tab("Layout", value="layout")
                    dg.Tab("State", value="state")

with dg.StatusBar(class_="statusline", height=38):
    status = dg.TextInput("Ready", placeholder="status", style={"width": 360})
    dg.Separator(orientation="vertical")
    dg.Label("Switch modes to see layout density change.")


print(app.run(win))
