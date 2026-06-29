# Incomplete API Inventory

Code-verified gaps as of 2026-06-28. Each item was checked against both the
Python source and the Rust runtime before being listed.

---

## 1. Histogram — live data updates implemented

**File:** `python/dragongui/widgets.py:7611`

Resolved 2026-06-28.

`Histogram.set_data()` now recomputes bin data in Python and, when mounted,
enqueues a dedicated live histogram data update. The native command queue
validates `edges` and `counts`, coalesces pending updates for the same widget,
replaces the Rust-side histogram payload, clears stale selection rectangles,
and redraws the chart.

**Implemented:**
- `LiveWidgetHandle.enqueue_set_histogram_data(...)`.
- `AppHandle.enqueue_set_histogram_data(...)`.
- Native `Command::SetHistogramData`.
- Rust runtime application path replacing `node.props.histogram.edges` and
  `node.props.histogram.counts`.
- Probe: `examples/css_feature_probes/histogram_live_update_probe.py`.

**Verification:**
- `python -m pytest tests\test_python_api.py -q -k "histogram_serializes_binned_data or histogram_live_set_data_enqueues_binned_update"`
- `cargo check --manifest-path native\Cargo.toml --target x86_64-pc-windows-msvc`
- `DRAGONGUI_SMOKE_FRAMES=3 C:\msys64\mingw64\bin\python.exe examples\css_feature_probes\histogram_live_update_probe.py`
- Manual probe confirmed working with `C:\msys64\mingw64\bin\python.exe` on 2026-06-28.

**Install/build note:**
Source-tree probes prepend `J:\Projects\DragonFrame\python` and therefore load
`python\dragongui\_dragongui.pyd` before any installed wheel. That local native
extension must match the interpreter/toolchain used to launch the probe. For the
MinGW Python command above, the repo-local extension was rebuilt with
`--target x86_64-pc-windows-gnu` and copied from the generated wheel.

---

## 2. HtmlReport — no cross-platform native renderer

**File:** `native/src/html_report_webview.rs:846`

On non-Windows platforms the entire `PlatformHtmlReportWebViewManager` is a
no-op stub: `sync()` does nothing, `drain_messages()` returns an empty vec,
`snapshot()` returns `{"platform": "unsupported", "enabled": false}`. No
content is rendered.

On Windows the widget uses WebView2. If WebView2 initialization fails the
runtime logs `"WebView2 initialization failed; using native fallback"` — but
the "native fallback" is a wgpu-rendered placeholder shape, not a functioning
HTML renderer.

`NodeGraph`, `Terminal`, and any other `HtmlReport`-backed widget therefore
only function on Windows with a working WebView2 install.

**What exists:** Full Windows WebView2 path including bounds sync, visibility,
script control, and message passing.

**What's missing:**
- macOS and Linux support (no WebKit/WebKitGTK backend).
- A real rendering fallback when WebView2 is unavailable on Windows.
- Documentation that clearly states this is Windows-only.

---

## 3. NodeGraph — viewport not hydrated on WebView recreation

**File:** `python/dragongui/node_graph.py:3380`

The generated JS initialises canvas state as a literal:

```js
const state = { ..., viewX: 34, viewY: 32, zoom: 1, ... };
```

Python tracks the live viewport in `_viewport_state` (updated via
`viewport_changed` events) and exposes it through `navigation_state()`, but
that value is never written into the generated HTML config. When a WebView is
closed and recreated (e.g. after a forced tab navigation), the canvas always
resets to the hardcoded defaults.

**What exists:** `navigation_state()` returns the last known viewport;
`_viewport_state` is updated correctly on `viewport_changed` events; Python
owns undo/redo stacks and all graph content.

**What's missing:**
- Pass `navigation_state()` into the `_generate_node_graph_html()` config dict.
- Initialize `state.viewX`, `state.viewY`, `state.zoom` from `config.viewport`
  instead of hardcoded literals.
- Serialize and restore selected edge and selected section from Python state.
- Decide and document the WebView recreation policy (see
  `webview-tabs-lifecycle.md`).

---

## 4. ExtensionWidget — no custom GPU paint callback

**File:** `python/dragongui/widgets.py:1852`, `native/src/primitives/mod.rs:11755`

`ExtensionWidget` is more complete than its docstring implies. Layout, hit
testing, and pointer events (click, pointer_down, pointer_move, pointer_up,
wheel, key_down) are all wired in the Rust runtime. The widget renders a
bordered rect with a `surface_alt` fill so it is visible.

What the docstring promises but does not exist is a **custom GPU paint
callback**: there is no mechanism for Python to supply draw commands (rects,
lines, images, text) that the Rust renderer executes inside the widget's
bounds.

**What exists:** Layout, all pointer/key events, bordered rect background
render, `on_click`, `on_pointer_*`, `on_wheel`, `on_key_down` callbacks.

**What's missing:**
- A paint command protocol (e.g. a list of primitive draw ops per frame).
- A Rust-side primitive emitter that reads those commands and renders into the
  widget rect.
- Public documentation — the widget is currently marked "Internal".

---

## 5. Document and command schema — written but not validated

**Files:** `python/dragongui/app.py:94`, `native/src/runtime.rs:14896`,
`native/src/layout.rs:7594`

Python writes `"schema": 1` in every startup document. Rust writes it in
debug snapshot outputs. No code on either side checks the incoming schema
version or rejects a mismatched value. A future protocol change could silently
send schema-2 commands to a schema-1 runtime with no error.

**What's missing:**
- Rust reads and validates the `schema` field on the startup document and each
  command envelope.
- Python and Rust share a schema version constant (or the Rust side surfaces a
  version query).
- A clear policy on what constitutes a breaking schema change.

---

## 6. Build Text node — partial ergonomics complete

**File:** `python/dragongui/node_graph.py:4832`

`Build Text` is implemented and functional: `input_count` controls how many
`part_N` input pins are shown; the runtime joins them in port order; inspector
fields for separator preset, custom separator, skip-empty, trim, and final
newline all work.

Resolved in part on 2026-06-28: the inspector now shows `+` / `-` controls on
the `Inputs` row for `Build Text`, and the active field can also be nudged with
arrow keys.

**What's missing:**
- A drag-to-resize affordance on the node body that adds/removes input ports.
- Per-input labels (currently all ports are named `part_N` with no user-editable
  label).
- Decision on whether to expose a terminal-input profile preset that enforces
  a trailing newline.

---

## Not confirmed / corrected from prior notes

- **HtmlReport "styled placeholder" in the native renderer** — the wgpu path
  draws a generic bordered rect (same as `ExtensionWidget`), not a styled
  placeholder specific to HTML content. The real story is that the widget is
  a no-op on non-Windows.
- **ExtensionWidget "no pointer events"** — incorrect. Pointer and key events
  are fully wired. Only custom drawing is absent.
