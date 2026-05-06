# Raspberry Pi Port Progress and Audit

Last updated: 2026-05-06

## Status

The pre-hardware implementation pass is complete. The code now has a Pi runtime
profile, conservative widget caps, debug diagnostics, CI hooks, install docs,
and a release checklist. Local validation passes on the Windows development
machine, using the GNU Rust test target for native tests because the local MSVC
test link path is missing `python3.lib`.

This does not mean the Raspberry Pi port is ready to ship. It means the work
that can be implemented and tested without V3D/Mesa hardware has been done.
The next meaningful step is running the release checklist on real Raspberry Pi
5 hardware.

## Implemented

- Native `RuntimeProfileSelection` with `DRAGONGUI_PROFILE=desktop|pi|auto`.
- Pi profile auto-selection for `linux/aarch64` and for native builds compiled
  with the `pi` Cargo feature.
- Pi GPU defaults:
  - Vulkan-or-GL backend set for the Pi profile.
  - low-power adapter preference.
  - downlevel wgpu limits resolved against adapter limits.
- Explicit backend triage override:
  - `DRAGONGUI_WGPU_BACKEND=auto`
  - `DRAGONGUI_WGPU_BACKEND=vulkan`
  - `DRAGONGUI_WGPU_BACKEND=gl`
  - comma/plus/pipe separated backend lists.
- `DRAGONGUI_LOG=debug` startup diagnostics for selected profile, requested
  backends, adapter, driver, downlevel mode, and required buffer limit.
- `backend_info()` and `app.debug_snapshot()` platform diagnostics:
  - OS and architecture.
  - selected/requested profile.
  - Pi feature and auto-target flags.
  - WebView availability.
  - adapter/backend/driver details after GPU init.
  - effective scatter, line plot, and table caps.
- Scatter protections:
  - Pi scatter payload cap: 100,000 points.
  - deterministic errors for payloads above the profile cap or device buffer
    size.
  - payload stride validation.
  - lower LOD threshold and interactive render scale under Pi profile.
  - chunked large scatter uploads.
  - contiguous ring-stream writes where possible.
- LinePlot protections:
  - 50,000 retained points per series under Pi profile.
  - startup and live payload trimming.
- DataFrameTable protections:
  - page size capped to 64 under Pi profile.
  - startup sample rows capped to 512 under Pi profile.
  - packed column buffers capped to 10,000 rows under Pi profile.
  - native resource registry accepts partial column buffers.
- HtmlReport behavior:
  - non-Windows embedded backend remains an unsupported snapshot.
  - Python `HtmlReport.open_external()` fallback remains the Pi path.
  - V3 demo/probe show the fallback route.
- V3 demo and scatter performance lab:
  - Pi profile constants reduce point/table/stream workloads.
  - modules are import-safe for tests.
- Docs and distribution:
  - Raspberry Pi install/troubleshooting guide.
  - Raspberry Pi release checklist.
  - widget-level Pi caps in widget docs.
  - manual arm64 wheel job in CI.
  - sdist includes `plans/RPi`.

## Validation Run Locally

Commands run successfully:

```powershell
cargo check --manifest-path native\Cargo.toml
cargo check --manifest-path native\Cargo.toml --features pi
python -m pytest tests\test_python_api.py -q -k "pi_profile or html_report or dataframe_table or backend_info"
python -m pytest tests\test_python_api.py -q -k "pi_profile or backend_info or dev_fallback or line_plot or dataframe_table or scatter_lod"
cargo test --manifest-path native\Cargo.toml --target x86_64-pc-windows-gnu wgpu_backend_override -- --nocapture
cargo test --manifest-path native\Cargo.toml --target x86_64-pc-windows-gnu required_wgpu_limits -- --nocapture
cargo test --manifest-path native\Cargo.toml --target x86_64-pc-windows-gnu debug_log_env -- --nocapture
git diff --check
```

Earlier focused native tests also passed for runtime profile selection, scatter
cap errors, scatter payload stride validation, and partial table column buffers.

## Audit Findings

### Fixed During This Audit

- CI Pi profile tests were not installing NumPy. Several Pi cap tests use
  `pytest.importorskip("numpy")`, so the profile job could pass while skipping
  meaningful coverage. The CI test dependency install now includes `numpy`.

### Remaining Implementation Gaps

These are not blockers for first hardware validation, but they are worth
tracking.

1. Python and native profile logic are duplicated.

   The Python fallback path and widget constructors have their own Pi profile
   constants. Native has the authoritative runtime profile. This is acceptable
   for now because tests cover the current values, but it is a drift risk if the
   caps change later. A future cleanup should expose a single Python-visible
   profile/cap helper, backed by native when available and mirrored only when
   native is absent.

2. Python `Scatter3D` can still pack oversized frames before native rejects
   them.

   Native now rejects over-cap Pi scatter payloads before GPU allocation, but
   Python may still spend CPU and memory packing a too-large frame first. This
   is safe but inefficient. A future improvement should add Python-side preflight
   for obvious over-cap frames using frame metadata before packing.

3. Live native table update commands rely on Python for Pi caps.

   Startup tree parsing reapplies table caps in native, and the Python API caps
   table payloads before sending live updates. Non-Python clients or malformed
   live commands could still send larger page/sample values. If non-Python live
   clients become real, reapply `effective_table_page_size` and
   `effective_table_sample_rows` inside native live table update handlers.

4. Partial table column buffers make sorting approximate beyond the packed
   prefix.

   Under the Pi profile, table column buffers can contain only the first 10,000
   rows while the table reports more virtual rows. Native cell lookup handles
   missing rows, but sorting a partially buffered column sorts known rows by
   typed value and compares unbuffered rows as missing values. This is a
   deliberate memory tradeoff for now, but the UI behavior should be validated
   on real datasets. We may want to disable typed sorting for partial buffers or
   show that sorting is limited to the buffered prefix.

5. Dev fallback cannot infer a native `pi` feature build on desktop.

   `DRAGONGUI_DEV_FALLBACK=1` uses Python-side profile detection. On desktop,
   it reports desktop unless `DRAGONGUI_PROFILE=pi` is set, even if a native
   build was compiled with the `pi` feature. The CI Pi profile job sets
   `DRAGONGUI_PROFILE=pi`, so coverage is aligned. This is only a diagnostics
   caveat for local fallback runs.

## Hardware-Only Validation Still Required

- Clean install on a supported Raspberry Pi OS Bookworm image.
- Import smoke from the built aarch64 wheel, not from the source tree.
- Minimal window smoke under the default desktop session.
- V3 demo with `DRAGONGUI_PROFILE=pi`.
- CSS feature probes that do not require embedded WebView.
- Line plot, table, histogram, and scatter probes.
- Interaction pass:
  - click
  - scroll
  - text input
  - context menu
  - scatter rotate/pan/zoom
- Backend comparison:
  - default backend selection
  - `DRAGONGUI_WGPU_BACKEND=gl`
  - `DRAGONGUI_WGPU_BACKEND=vulkan`
- Capture:
  - `backend_info()`
  - `app.debug_snapshot()`
  - `DRAGONGUI_LOG=debug` stderr
  - kernel, Mesa, Vulkan/GL renderer, Python, temperature, and power/cooling
    details.

## Decisions Deferred Until Hardware Data

- Whether Vulkan or GL should be the documented default path.
- Whether Wayland and X11 are both first-release supported, or one is the
  validated path and the other is best-effort.
- Whether MSAA must be disabled globally on Pi.
- Whether raw scatter streaming should be the default recommendation for
  high-frequency Pi workloads.
- Whether the thread monitor should default off or refresh more slowly in the
  Pi V3 demo.
- Whether the current LinePlot and table caps are too high for sustained Pi 5
  interaction.
- Whether Raspberry Pi 4 should get a separate lower-cap profile later.

## Next Step

Run [raspberry-pi-release-checklist.md](raspberry-pi-release-checklist.md) on
real Raspberry Pi 5 hardware, then patch from observed failures or performance
data.
