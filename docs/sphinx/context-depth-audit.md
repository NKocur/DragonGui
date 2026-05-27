# Sphinx Context Depth Audit

This audit checks whether the Sphinx documentation gives enough context for a
new DragonGUI user or agent to use the framework without falling back to source
inspection.

## Current State

The Sphinx tree builds cleanly and the API reference covers the public package
modules, but the guide layer is still thin. Most user-guide pages are under 150
words and several pages point to root-level Markdown notes instead of carrying
the useful material into the Sphinx guide.

Depth metrics from the current Sphinx source:

| Page | Words | Code Blocks | Main Gap |
| --- | ---: | ---: | --- |
| `quickstart.md` | 147 | 3 | Needs install/build/run variants and a real callback example. |
| `layout.md` | 138 | 0 | Lists containers but does not show layout patterns. |
| `widgets.md` | 147 | 0 | Good coverage list, but no widget-specific usage context. |
| `styling.md` | 1,364 | 18 | First expansion complete; exhaustive CSS inventories still live in root notes. |
| `plots.md` | 143 | 3 | Covers `Scatter3D` and `LinePlot` only; chart/table plotting context is missing. |
| `live-updates.md` | 102 | 2 | Needs callback, thread, and streaming lifecycle examples. |
| `performance.md` | 101 | 0 | Mostly links to benchmark notes; not yet a practical performance guide. |
| `troubleshooting.md` | 115 | 1 | Has important issues, but each entry is still abbreviated. |
| `notes.md` | 70 | 0 | Acts as an index to root notes, not stable documentation. |

The API pages are intentionally small wrappers around autodoc. That is fine for
reference generation, but the user guide should not depend on those pages to
explain workflows.

## Coverage Against `dragongui.help`

`dragongui.help` currently exposes 242 topics. The Sphinx guide has top-level
pages for:

- `quickstart`
- `layout`
- `widgets`
- `styling`
- `live_updates`
- `performance`
- `troubleshooting`

Important `dragongui.help` categories without dedicated Sphinx guide pages:

- `app_model`
- `callbacks`
- `components`
- `validation`
- `recipes`
- `extensions`
- `debugging`
- `decisions`

The largest missing area is `recipes`. The runtime manual already has entries
for dashboards, forms, plotting, app shells, settings panels, streaming plots,
table browsers, drag/drop, command palettes, custom composites, and PyTorch
dashboards. None of those are first-class Sphinx pages yet.

## Root Notes Not Yet Migrated

The root `docs/` folder contains much deeper material than the Sphinx guide:

| Root Note | Words | Use In Sphinx |
| --- | ---: | --- |
| `css-capabilities-reference.md` | 8,188 | Convert into CSS reference pages. |
| `widgets-reference.md` | 6,307 | Split into widget category guide pages. |
| `css-styling.md` | 4,448 | Merge into `styling.md` or a styling section. |
| `library-overview.md` | 2,838 | Use for app model, design goals, and architecture overview. |
| `widgets.md` | 2,159 | Use for widget guide prose and examples. |
| `css-cascade-optimization.md` | 1,103 | Keep as performance implementation note. |
| `dragon-gui-benchmark-audit.md` | 1,099 | Convert into benchmark report pages. |
| `scatter3d-streaming-colormap-fix.md` | 778 | Keep as Scatter3D troubleshooting note. |
| `scatter3d-point-depth-fix.md` | 613 | Keep as Scatter3D troubleshooting note. |

Current Sphinx pages reference these notes with inline paths such as
`../css-styling.md`. Those references are useful during migration, but they are
not a finished documentation experience.

## Priority Fixes

1. Add guide pages for `app_model`, `callbacks`, `components`, `validation`,
   `recipes`, and `extensions`.
2. Continue splitting styling reference depth from `css-styling.md` and
   `css-capabilities-reference.md` into focused Sphinx pages.
3. Expand `widgets.md` into category pages instead of one list page:
   inputs, navigation, data tables, charts, feedback, media, and extensions.
4. Expand `plots.md` to cover `Scatter3D`, `LinePlot`, `ScatterPlot2D`,
   `Heatmap`, `Histogram`, `BarChart`, and `PieChart` with working examples.
5. Turn `performance.md` into a practical guide, then keep benchmark snapshots
   as separate reports.
6. Replace inline root-note references with either migrated Sphinx content or
   deliberate project-note pages.

## Suggested Next Pass

Continue with widget category pages. `widgets.md` now lists the public widget
surface, but it does not yet teach users how to choose and combine controls,
tables, navigation, feedback, media, and extension widgets.
