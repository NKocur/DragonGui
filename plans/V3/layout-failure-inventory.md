# V3 Layout Failure Inventory

Audit of layout failure modes found in the example files listed in the V3 plan.
Each entry records the widget tree shape, CSS involved, expected behavior, actual
behavior, and root cause classification.

---

## FM-01 — Fragile Two-Column calc Sizing

**Severity:** High  
**Files:** `form_controls_probe.py`, `overlay_stack_probe.py`, `widget_metrics_probe.py`

**Tree shape:**

```
HLayout.grid (gap: 12–14px)
  Panel.case (width: calc(50% - 6px), min-width: 390px)
  Panel.case (width: calc(50% - 6px), min-width: 390px)
```

**CSS involved:**

```css
HLayout.grid {
    width: 100%;
    gap: 12px;  /* or 14px depending on probe */
}

Panel.case {
    width: calc(50% - 6px);  /* gap / 2 must be manually kept in sync */
    min-width: 390px;
}
```

**Expected:** Two equal-width panels fill one row at any window width, wrapping
or collapsing gracefully when too narrow.

**Actual:** At window widths below `2 × 390 + gap + padding ≈ 808 px`, both
panels assert `min-width` and the total exceeds the available width. `HLayout`
does not wrap, so the second panel is pushed off-screen or clips the first.
The magic number `6px` in `calc(50% - 6px)` must be kept manually in sync with
half the `gap` value; when the gap changes (e.g., 12 vs 14) the formula drifts.

**Classification:** Missing primitive — `GridLayout` with `columns=2` would
express this correctly without per-card calc formulas and would enable wrapping.

---

## FM-02 — MenuBar Height Manually Subtracted from Content Area

**Severity:** Medium  
**Files:** `overlay_stack_probe.py`, `navigation_widgets_probe.py`

**Tree shape:**

```
Window
  MenuBar (height: 34)
  VLayout.root / HLayout.shell  (height: calc(100% - 34px))
```

**CSS involved:**

```css
VLayout.root {
    height: calc(100% - 34px);
}
```

**Expected:** Content area fills the remaining space below the MenuBar
regardless of MenuBar height.

**Actual:** Works only when `MenuBar.height == 34`. If height is changed or a
second bar is added, the formula goes stale and content either overflows the
window or leaves a gap.

**Classification:** Missing default — Window layout should compute available
height for main content automatically after reserving MenuBar/StatusBar slots,
so authors do not need to subtract bar heights in CSS.

---

## FM-03 — Scrollbar Gutter Requires Manual padding-right

**Severity:** High  
**Files:** All scrollable probes and demos

**Tree shape:**

```
VLayout.root (overflow-y: auto)
  ...children...
```

**CSS involved:**

```css
VLayout.root {
    overflow-y: auto;
    padding-right: 24px;  /* 22–28 across probes — avoids scrollbar overlap */
}
```

**Expected:** The scrollbar reserves space on its own so rightmost child content
is not obscured.

**Actual:** Without `padding-right`, the scrollbar draws over the trailing edge
of children. The exact value (22–28 px) is copy-pasted into every scrollable
container as per-example boilerplate and changes independently across probes.

**Classification:** Missing default — scroll containers should reserve a
`scrollbar-gutter` equivalent automatically, or expose a single CSS property
that sets the gutter for the entire probe.

---

## FM-04 — Last-Child Reachability Requires Manual padding-bottom

**Severity:** Medium  
**Files:** `form_controls_probe.py`, `overlay_stack_probe.py`,
`navigation_widgets_probe.py`, `widget_metrics_probe.py`

**Tree shape:**

```
VLayout.root (overflow-y: auto)
  ...N children...
  Label.last-child
```

**CSS involved:**

```css
VLayout.root {
    padding-bottom: 76px;  /* 48–80px across probes */
}
```

**Expected:** Scrolling to the bottom fully reveals the last child with
comfortable clearance.

**Actual:** Without `padding-bottom`, the last child sits at the visible scroll
limit and can be partially hidden by the window edge or a StatusBar. The value
varies (48–80 px) and is tuned per probe. There is no systemic fix.

**Classification:** Missing default / fragile workaround. A `scrollbar-gutter`
system combined with proper Panel body/title margin would make this unnecessary.

---

## FM-05 — Sidebar-Width-Coupled Content Area

**Severity:** Medium  
**Files:** `navigation_widgets_probe.py`

**Tree shape:**

```
HLayout.shell (width: 100%, height: calc(100% - 34px))
  Sidebar.nav (width: 238)
  VLayout.content (width: calc(100% - 238px))
```

**CSS involved:**

```css
VLayout.content {
    width: calc(100% - 238px);
}
```

**Expected:** The content area fills the space not taken by the sidebar,
regardless of the sidebar's width.

**Actual:** The constant `238px` must be kept manually in sync with the
`Sidebar(width=238, ...)` constructor argument. A sidebar resize or CSS width
override leaves the formula stale, creating an overlap or gap.

**Classification:** Author misuse of CSS — should use `flex-grow: 1` (or
`width: auto; flex-grow: 1`) on the content area rather than `calc(100% - X)`.
The fix does not require a new primitive, only guidance and probe correction.

---

## FM-06 — Absolute Children Need Hardcoded Title-Aware top Offset

**Severity:** Medium  
**Files:** `layout_containers_probe.py`, `overlay_stack_probe.py`

**Tree shape:**

```
Panel (position: relative)
  Tag.pin (position: absolute, top: 12px, right: 14px)
  ...flow children...
```

**CSS involved:**

```css
Tag.pin {
    position: absolute;
    top: 12px;
    right: 14px;
}
```

**Expected:** The pinned widget appears inside the panel body, below the title
band, without overlapping title text.

**Actual:** `top: 12px` is a magic number that happens to clear the title only
when the panel's title height, padding, and font size happen to produce a title
band shorter than ~12 px. With a larger font or extra title padding this offset
collides with the title. There is no CSS variable or layout mechanism to compute
"body top start" programmatically.

**Classification:** Missing primitive — Phase 4 (Panel body/title model) should
expose a `--panel-body-top` variable or ensure the absolute-positioned child's
coordinate origin begins at the body region, not the panel border.

---

## FM-07 — Duplicate Scroll Style in CSS Class and Inline Style Dict

**Severity:** Low  
**Files:** `layout_containers_probe.py`

**Tree shape:**

```python
with dg.VLayout(
    id="layout-scroll-body",
    class_="scroll-case",         # sets width, height, overflow-y, padding-right, etc.
    style={
        "width": "100%",
        "height": 210,
        "overflow_y": "auto",     # same properties duplicated here
        "overflow_x": "hidden",
        "padding_right": 26,
        "padding_bottom": 22,
        "gap": 10,
    },
):
```

**CSS involved:**

```css
VLayout.scroll-case {
    width: 100%;
    height: 210px;
    overflow-y: auto;
    overflow-x: hidden;
    padding-right: 26px;
    padding-bottom: 22px;
    gap: 10px;
}
```

**Expected:** One source of truth for scroll container dimensions.

**Actual:** The same properties appear in both the CSS class and the `style={}`
kwarg. When one is changed, the other may override it silently depending on
specificity rules, causing confusing layout. The probe was written this way to
test both paths simultaneously but the duplication obscures intent and is easy
to copy incorrectly.

**Classification:** Author misuse — should use CSS class or inline `style={}`,
not both for the same properties.

---

## FM-08 — Scatter Panel Collapses Without Extra scatter-case Class

**Severity:** Medium  
**Files:** `widget_metrics_probe.py`

**Tree shape:**

```
HLayout.grid
  Panel.case.scatter-case   ← needs scatter-case for explicit height
    Label
    Scatter3D (flex-grow: 1)
    Label
```

**CSS involved:**

```css
Panel.case {
    width: calc(50% - 6px);
    min-width: 390px;
    /* no height */
}

Panel.scatter-case {
    height: 340px;
}
```

**Expected:** Any panel that contains a Scatter3D fills a reasonable height
automatically.

**Actual:** Without `scatter-case`, the panel height is determined by its
children. `Scatter3D { flex-grow: 1 }` only grows if the parent has a known
height. A panel without an explicit height collapses to the minimum needed for
the non-scatter children (two small labels), causing the scatter to render at
zero or near-zero height.

**Classification:** Missing default / documentation gap. Panels that host
flex-grow children should either default to a minimum content height or document
the requirement clearly. The `GridLayout` primitive could include a `row-height`
mechanism to make this obvious.

---

## FM-09 — HLayout Has No Wrapping Mode

**Severity:** High  
**Files:** `form_controls_probe.py`, `overlay_stack_probe.py`,
`widget_metrics_probe.py`, `navigation_widgets_probe.py`

**Tree shape:**

```
HLayout.grid
  Panel (width: calc(50% - 6px), min-width: 390px)
  Panel (width: calc(50% - 6px), min-width: 390px)
```

**Expected:** At narrow widths where both panels cannot fit, they wrap onto
separate rows.

**Actual:** `HLayout` maps directly to `flex-direction: row; flex-wrap: nowrap`.
There is no `flex-wrap: wrap` support. Children that cannot fit overflow or
squash regardless of `min-width`. All two-column probe layouts break below
approximately 808 px window width.

**Classification:** Missing primitive — `FlowLayout` (Phase 3) or enabling
`flex-wrap` on `HLayout` would solve this. `GridLayout` (Phase 2) also solves
the two-column case with responsive collapse to a single column.

---

## FM-10 — Fixed-Width Sidebar Panels in Demos Force Remaining Panel to Free-Fill

**Severity:** Low  
**Files:** `all_features_demo.py`, `all_features_css_demo.py`

**Tree shape:**

```
HLayout (gap: 16, padding: 14)
  Panel "Scatter controls" (width=310)   ← fixed-width constructor arg
  Scatter3D                               ← no explicit width; fills rest
```

**Expected:** At window widths approaching `310 + 2×14 + 16 = 354 px`, the
layout is still usable. The controls panel may compress but the scatter should
at minimum remain visible.

**Actual:** The fixed `width=310` from the Python constructor is not subject to
`min-width` shrinking unless explicitly set. At narrow widths the scatter panel
can be squeezed to near-zero. There is no `min-width` guard on the fixed panel
or `flex-shrink: 0` to prevent it from being compressed, and no `min-width` on
the scatter to ensure it stays usable.

**Classification:** Author gap — each demo should add `flex-shrink: 0` or
`min-width` to the fixed panel and `min-width` to the fill panel. A probe
helper template (Phase 6) would establish this pattern.

---

## FM-11 — Scrollable titled Panel Body Start Requires Manual Height Wrangling

**Severity:** Medium  
**Files:** `layout_containers_probe.py`

**Tree shape:**

```
Panel.scroll-shell (height: 318px, overflow: hidden)
  VLayout.scroll-case (height: 210px, overflow-y: auto)
    Label
    Button × 10
    Label
  Button "Print scroll snapshot"
```

**Expected:** The titled panel has a clear title region; the scrollable VLayout
fills the body below it; the final row is reachable by scrolling.

**Actual:** The `VLayout.scroll-case` height (210 px) plus the button outside
it (≈40 px) plus any panel padding and title height must sum to exactly the
`Panel.scroll-shell` height (318 px), or content will either overflow the
shell or leave a visible gap. There is no mechanism to say "fill the body
height minus the title". The probe works, but only by manual arithmetic that
will break if fonts, theme padding, or the title text changes.

**Classification:** Missing default — Panel body/title model (Phase 4) should
allow the body to receive `height: auto; flex-grow: 1` relative to the title,
so scroll regions can be expressed as `height: 100%` within the body.

---

## FM-12 — VLayout Root Scroll Requires Per-Probe Scrollbar Part Duplication

**Severity:** Low  
**Files:** All probes with `VLayout.root`

**CSS involved:**

```css
VLayout.root::scrollbar-track {
    width: 8px;
    padding: 2px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 999px;
}

VLayout.root::scrollbar-thumb {
    width: 6px;
    background: rgba(90, 169, 255, 0.72);
    border-radius: 999px;
}
```

**Expected:** A consistent scrollbar style is available across all probes
without per-probe CSS blocks.

**Actual:** Every probe that has a scrollable root VLayout copies the same
~12-line scrollbar-track and scrollbar-thumb CSS block, varying only colors.
This is the most repeated boilerplate across the probe suite.

**Classification:** Missing helper — Phase 6 (probe helper template) should
provide a shared `probe_scrollbar_css` string or CSS mixin so probes don't
redeclare these dimensions individually.

---

## Summary Table

| ID     | Severity | Root Cause            | Fixing Phase |
|--------|----------|-----------------------|--------------|
| FM-01  | High     | Missing primitive     | Phase 2 (GridLayout) |
| FM-02  | Medium   | Missing default       | Phase 4 (Panel/MenuBar layout) |
| FM-03  | High     | Missing default       | Phase 5 (scrollbar gutter) |
| FM-04  | Medium   | Missing default       | Phase 5 (scrollbar gutter) |
| FM-05  | Medium   | Author misuse         | Phase 6 (helper / doc fix) |
| FM-06  | Medium   | Missing primitive     | Phase 4 (Panel body/title model) |
| FM-07  | Low      | Author misuse         | Phase 6 (helper / doc fix) |
| FM-08  | Medium   | Missing default / doc | Phase 2 + Phase 4 |
| FM-09  | High     | Missing primitive     | Phase 2 (GridLayout) / Phase 3 (FlowLayout) |
| FM-10  | Low      | Author gap            | Phase 6 (helper template) |
| FM-11  | Medium   | Missing default       | Phase 4 (Panel body/title model) |
| FM-12  | Low      | Missing helper        | Phase 6 (probe helpers) |

### Priority order

1. **FM-01, FM-09** — Two-column layout with no wrap is the most visible failure
   and affects every probe grid. Resolved by `GridLayout` (Phase 2).
2. **FM-03, FM-04** — Scrollbar gutter and last-item reachability are copy-paste
   boilerplate in every scrollable probe. Resolved by Phase 5 defaults.
3. **FM-06, FM-11** — Absolute child / Panel body model. Resolved by Phase 4.
4. **FM-02, FM-05, FM-07, FM-08, FM-10, FM-12** — Lower severity; resolved by
   Phase 4 defaults, Phase 6 helper templates, or in-place probe fixes.
