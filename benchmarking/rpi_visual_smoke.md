# Raspberry Pi Visual Smoke

Use this checklist on the Pi after the native wheel has been rebuilt and
installed with the repo helper. It is intentionally manual for now; the goal is
to catch clipped text, broken hit targets, unusable scroll areas, black windows,
and interaction regressions that unit tests and short frame-count smokes miss.

## Baseline

Record before testing:

- Pi model, RAM, OS image date, desktop session, and display resolution.
- `uname -a`.
- `glxinfo -B` renderer.
- `vulkaninfo --summary` if Vulkan is installed.
- DragonGUI commit/build label and wheel path.

Use the validated Pi path:

```bash
DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke benchmarking/rpi_widget_probe.py
```

Pass criteria:

- Native backend import reports `native: True`.
- Profile is `pi`.
- Backend override is `gl`.
- Final probe exits successfully with queue depth `0`.

The optional `live_after_append` probe snapshot may time out on the current
debug-snapshot path; do not fail this checklist for that known probe limitation
unless the final scenario also fails.

## Automated Viewport Sweep

Run every primary V3 route at 800x480:

```bash
for page in overview scatter lineplots histograms piecharts controls data runtime debug styling layout; do
  DRAGONGUI_DEMO_PAGE="$page" DRAGONGUI_SMOKE_FRAMES=3 \
    bash rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py
done
```

Run a representative 1280x720 sweep:

```bash
for page in overview histograms controls data; do
  DRAGONGUI_DEMO_WIDTH=1280 DRAGONGUI_DEMO_HEIGHT=720 \
  DRAGONGUI_DEMO_PAGE="$page" DRAGONGUI_SMOKE_FRAMES=3 \
    bash rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py
done
```

Pass criteria:

- Each result reports `"status": "ok"`.
- No active page descendant overflows outside its page clip without a scroll
  ancestor.
- Requested 1280x720 may be constrained by the window manager; smaller actual
  inner height is acceptable if the check passes against the actual window.

## Manual Screenshots

Open the V3 demo:

```bash
bash rpi_setup_and_run.sh run
```

Capture one screenshot for each route:

- `overview`: top nav, left nav, status bar, and primary panels visible.
- `scatter`: Scatter3D viewport nonblank, labels use the same font as the UI,
  toolbar buttons are reachable, sidebar badges do not sit under the scrollbar.
- `lineplots`: live line plots tick visibly, toolbar icons are aligned, no
  deferred-limit crash on start.
- `histograms`: histogram bars, ticks, and labels are readable; compact plots do
  not flood tick labels.
- `piecharts`: legends and slice labels do not overlap important controls.
- `controls`: checkboxes, buttons, sliders, dropdowns, and panels are not
  clipped; page scroll appears if content exceeds the viewport.
- `data`: tables show useful rows and columns at 800x480; row text aligns with
  grid lines; header sort marker remains visible.
- `runtime`: queue/performance status remains readable.
- `debug`: debug output can scroll and does not push the main layout offscreen.
- `styling`: fonts are consistent across panels, buttons, headings, and badges.
- `layout`: nested scroll areas own their own overflow; parent page scroll still
  works.

Pass criteria:

- No black, blank, or stale surfaces.
- No text from one widget visibly overlaps unrelated controls.
- No required control is hidden behind a scrollbar.
- No panel content is cut off without a scrollbar or explicit clipping reason.

## Input Checklist

Verify with mouse/touchpad on the Pi:

- Navigation sidebar scrolls with wheel and drag.
- Page content scrolls independently of the sidebar.
- Buttons trigger without font/layout jumps.
- Checkboxes toggle and stay separated.
- Dropdown opens, selects an item, and closes.
- Sliders drag smoothly and keep labels in bounds.
- Table wheel/trackpad scroll changes visible rows.
- Table click selection highlights the selected row/cell.
- Table header click sorts and keeps text/grid alignment.
- Scatter3D rotates, pans, zooms, and resets camera.
- LinePlot toolbar `Fit`, `Pan`, `Zoom`, `Box`, `Grid`, and `Axes` hit targets
  work.
- Histogram toolbar `Fit`, `Pan`, `Zoom`, `Box`, `Grid`, and `Axes` hit targets
  work.
- Modal/context-menu close paths work.
- Window close exits cleanly.

Pass criteria:

- Interactions complete without crashes, stuck mouse capture, or runaway queue
  depth.
- Repeated scroll/drag does not make frame time visibly degrade over the test.

## Failure Notes

For each failure, record:

- Route/page.
- Display resolution and actual window inner size from the viewport check.
- Screenshot or short video.
- Final `benchmarking/rpi_widget_probe.py` snapshot.
- Whether the same issue appears with `DRAGONGUI_PROFILE=desktop`.

Do not mix visual-smoke fixes with unrelated rendering or API changes; each
widget-specific fix should land in its own patch.
