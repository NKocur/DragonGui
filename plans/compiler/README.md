# DragonGUI Application Compiler Plan

## Objective

Make applications built with DragonGUI distributable as a single Windows `.exe`.
The first implementation should be a pragmatic packaging pipeline, not a Python
compiler in the language-runtime sense.

The compiler should bundle:

- The user's DragonGUI application entry script.
- A supported CPython runtime.
- The current `dragongui` Python package.
- The `dragongui._dragongui` PyO3 native extension.
- DragonGUI package data, currently terminal JavaScript/CSS assets.
- The application's Python dependencies.
- User-provided assets such as CSS, images, HTML files, fonts, icons, and data.

The first backend should be PyInstaller. Nuitka and installer generation can be
evaluated only after the PyInstaller route is boring and repeatable.

## Current Codebase Findings

This plan was reviewed against the current repository shape, not just the older
packaging plan.

### Package And Native Backend

- `pyproject.toml` uses `maturin` with `python-source = "python"` and
  `module-name = "dragongui._dragongui"`.
- The Rust crate builds a `cdylib` named `_dragongui` with PyO3
  `abi3-py311`, so the package requires Python 3.11+.
- `python/dragongui/_backend.py` imports `from . import _dragongui as _native`
  and exposes `native_backend_available()` and `backend_info()`.
- `DRAGONGUI_DEV_FALLBACK=1` bypasses the native event loop and returns a
  serialized document. That is useful for import/document tests, but it is not
  enough to prove a packaged `.exe` works.
- The current source tree imports successfully with `py -3.11` and reports a
  native backend when `PYTHONPATH=python` is set.

### Current Public Python Surface

The top-level package imports more than the earlier plan assumed:

- `agent_messages`
- `agent_session`
- `node_graph`
- `terminal.TerminalEvent`
- all existing core widgets and runtime modules

A compiler hook must either collect all `dragongui` submodules or explicitly
include this updated public surface. A narrow handwritten list will go stale
quickly.

### Existing Wheel Is Stale

The wheel currently present in `dist/` contains the older package surface. It
does not include newer modules such as `agent_messages.py`, `agent_session.py`,
or `node_graph.py`.

Compiler work should therefore not trust whatever happens to be in `dist/`.
Before packaging smoke tests, rebuild or install the current package with the
same Python interpreter used by the packager.

Required preflight:

```powershell
py -3.11 -m maturin develop
py -3.11 -c "import dragongui; print(dragongui.backend_info())"
```

For release-like validation, build a fresh wheel and install it into a clean
virtual environment before running `dragongui-pack`.

### Plan Folder Packaging

`pyproject.toml` includes `plans/*.md`, `plans/V2/*.md`, and `plans/V3/*.md` in
sdists, but it does not currently include `plans/compiler/*.md`. If this plan is
meant to ship in source distributions, add:

```toml
{ path = "plans/compiler/*.md", format = "sdist" },
```

This does not affect runtime `.exe` builds, but it matters for repository and
sdist hygiene.

### Native Resource Loading

Most rendering resources are embedded into the native extension with
`include_str!`:

- `native/src/framework.dg.css`
- WGSL shaders for scatter, images, and primitives

These should not require PyInstaller data-file handling.

Runtime file paths still matter for:

- `Image(path)`, where the native backend reads the image file path.
- `HtmlReport(path=...)`, where WebView2 navigates to a file URL.
- `HtmlReport(html=..., base_dir=...)`, where relative content can depend on a
  filesystem base path.
- CSS `@font-face` local font files, where native text code reads `.ttf`,
  `.otf`, `.ttc`, or `.woff` files from paths.
- User code that calls `App.load_stylesheet(path)`.

The compiler therefore needs both package-data collection and app-asset
collection. These are different jobs.

### WebView2 And HTML Surfaces

`HtmlReport`, `Terminal`, and `NodeGraph` rely on the WebView2-backed
HtmlReport path on Windows. The native code has fallback behavior and WebView2
user-data handling, but a packaged app still depends on WebView2 runtime
availability unless we later add installer/runtime bootstrapping.

Relevant environment knobs that should appear in troubleshooting docs:

- `DRAGONGUI_HTMLREPORT_WEBVIEW2`
- `DRAGONGUI_HTMLREPORT_USER_DATA_DIR`
- `DRAGONGUI_SMOKE_FRAMES`

### V3 Demo Reality

`examples/all_features_v3_demo.py` is the right broad smoke target, but it is
not a minimal dependency target.

It requires:

- `numpy`

It optionally uses:

- `plotly.graph_objects`

It creates runtime assets in temp directories:

- `dragongui_all_features_v3_demo.png`
- `dragongui_all_features_v3_reports/*.html`

That means the V3 demo does not need external image or report files bundled for
its current path, but it does exercise native image loading and WebView2 file
HTML loading through temp-file paths.

It also contains the example source-layout shim:

```python
if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
```

In a frozen app this may add a meaningless `python` directory under the
PyInstaller extraction root. That is probably harmless, but the compiler smoke
path should track it. Longer term, examples should guard this with
`not getattr(sys, "frozen", False)` or move smoke app entry points into a package
module without source-tree path mutation.

## Target Smoke Applications

Use a layered smoke set instead of one demo doing everything.

Primary broad smoke:

```text
examples/all_features_v3_demo.py
```

This is the canonical broad smoke gate for compiler work. Do not replace it with whichever example is currently active in the IDE; other examples are secondary probes only.

Fast sanity smoke:

```text
examples/multi_agent_cockpit_mockup.py
```

WebView2/node smoke:

```text
examples/node_graph_editor_probe.py
```

Terminal smoke:

```text
examples/terminal_wrapper_demo.py
```

Focused asset smoke:

- A tiny app that bundles an image file and displays it with `dg.Image`.
- A tiny app that bundles a stylesheet and loads it with `App.load_stylesheet`.
- A tiny app that uses `HtmlReport(path=...)` with a bundled local HTML file.
- A tiny app that uses CSS `@font-face` with a bundled font file.

Scientific smoke:

- Scatter3D with NumPy.
- DataFrameTable with pandas and/or polars.

## MVP Definition

The MVP is complete only when both of these work:

1. A minimal/std-library DragonGUI app packages and launches quickly.
2. `examples/all_features_v3_demo.py` packages and launches as the primary broad
   smoke artifact.

MVP requirements:

- Build a one-file Windows `.exe` with PyInstaller.
- Build a one-folder variant for easier debugging.
- Launch from outside the repository.
- Load `dragongui._dragongui` from the packaged app.
- Include all current `dragongui` Python modules.
- Include `dragongui.assets.terminal` package data.
- Include NumPy for the V3 demo.
- Document optional Plotly behavior for the V3 demo.
- Document WebView2 expectations for `HtmlReport`, `Terminal`, and `NodeGraph`.
- Produce a packaging report.

MVP non-goals:

- Cross-platform packaging.
- Code signing.
- Installer generation.
- Automatic installation of missing dependencies.
- Perfect dynamic import discovery for arbitrary user code.
- True Python-to-native compilation.
- Bundling a fixed WebView2 runtime.

## Recommended Build Flow

The compiler should support two modes:

### Source Tree Development Mode

This is for this repository while DragonGUI is being developed.

Preflight:

```powershell
py -3.11 -m maturin develop
$env:PYTHONPATH = "python"
py -3.11 -c "import dragongui; print(dragongui.backend_info())"
```

Then package:

```powershell
py -3.11 -m dragongui.compiler examples\all_features_v3_demo.py --onefile --name DragonGUIV3Demo --with v3-demo
```

The compiler should detect source-tree mode when the imported `dragongui.__file__`
resolves under `<repo>/python/dragongui`. In this mode, PyInstaller analysis
should receive `--paths <repo>\python`; setting `PYTHONPATH` for the compiler
process is not enough to make PyInstaller's module graph reliable.

Source-tree mode should warn if:

- `dragongui.native_backend_available()` is false.
- `python/dragongui/_dragongui.pyd` is missing.
- `dist/*.whl` exists but is older or missing current package modules.
- The entry script uses the examples source-layout `sys.path` shim.

### Installed Package Mode

This is the user-facing mode after DragonGUI is installed from a wheel.

Preflight:

```powershell
py -3.11 -m venv .venv-pack
.\.venv-pack\Scripts\python -m pip install dragongui[terminal]
.\.venv-pack\Scripts\python -c "import dragongui; print(dragongui.backend_info())"
```

Then package with the same interpreter:

```powershell
.\.venv-pack\Scripts\python -m dragongui.compiler app.py --onefile --name MyApp
```

The compiler should always call PyInstaller through `sys.executable` so the
build uses the same environment that imported DragonGUI.

## PyInstaller Backend

PyInstaller is the first backend because it is the fastest path to a working
single `.exe`.

Manual V3 proof command:

```powershell
py -3.11 -m PyInstaller `
  --noconfirm `
  --clean `
  --onefile `
  --windowed `
  --name DragonGUIV3Demo `
  --paths python `
  --collect-data dragongui.assets `
  --collect-submodules dragongui `
  --hidden-import dragongui._dragongui `
  --hidden-import numpy `
  examples\all_features_v3_demo.py
```

For debugging, prefer one-folder first:

```powershell
py -3.11 -m PyInstaller `
  --noconfirm `
  --clean `
  --onedir `
  --windowed `
  --name DragonGUIV3Demo `
  --paths python `
  --collect-data dragongui.assets `
  --collect-submodules dragongui `
  --hidden-import dragongui._dragongui `
  --hidden-import numpy `
  examples\all_features_v3_demo.py
```

The compiler should generate these arguments rather than asking users to copy
PyInstaller commands.

## PyInstaller Hook Design

For the prototype, prefer a hook directory outside the runtime package:

```text
tools/pyinstaller_hooks/
  hook-dragongui.py
```

The compiler CLI can pass `--additional-hooks-dir <repo>/tools/pyinstaller_hooks`
in source-tree mode. Later, an installed package can expose hooks through a
PyInstaller hook entry point or a package-local hook directory, but compiler
implementation modules must not become runtime hidden imports.

A simple hook should start broad for runtime modules and narrow for tooling:

```python
from PyInstaller.utils.hooks import collect_data_files, collect_submodules


def _is_runtime_module(name: str) -> bool:
    return not (
        name == "dragongui.compiler"
        or name.startswith("dragongui.compiler.")
        or name.startswith("dragongui._compiler")
        or name.startswith("dragongui._pyinstaller")
    )


hiddenimports = collect_submodules("dragongui", filter=_is_runtime_module)
datas = collect_data_files("dragongui.assets")
```

Verify the exact `collect_submodules(..., filter=...)` API against the pinned
PyInstaller version before implementing; if needed, collect broadly and then
filter the returned list.

Hook responsibilities:

- Include `dragongui._dragongui`.
- Include all runtime Python modules under `dragongui`, including current agent
  and node graph modules.
- Exclude compiler-only modules and hook modules from packaged user apps.
- Include `dragongui.assets.terminal.xterm.js`.
- Include `dragongui.assets.terminal.xterm.css`.
- Include `dragongui.assets.terminal.addon-fit.js`.
- Avoid repository-only content: docs, examples, tests, plans, build outputs,
  `.test-cache`, and `target`.

Open question:

- Should the production hook keep collecting all runtime submodules, or should
  the compiler generate a narrower list after the public API settles? Runtime
  breadth is correct for MVP because the public API imports many modules at top
  level, but compiler/tooling modules should stay out.

## Compiler Tooling Exclusion

Treat compiler code as build tooling, not app runtime. This means:

- Keep the first hook implementation under `tools/pyinstaller_hooks` if practical.
- If a future `dragongui.compiler` package is added, exclude it from hidden
  imports generated for end-user apps.
- Do not make `dragongui.__init__` import compiler modules.
- Keep PyInstaller itself in an optional compiler dependency group so normal
  DragonGUI apps do not import or require PyInstaller at runtime.
- Add a packaged-app inspection check that fails if `dragongui.compiler`,
  `dragongui._pyinstaller`, or `PyInstaller` appears in the collected runtime
  tree without an explicit developer override.

## Compiler CLI Design

First public command:

```text
dragongui-pack ENTRY
  --name NAME
  --onefile
  --onedir
  --windowed
  --console
  --icon PATH
  --asset PATH_OR_GLOB
  --asset-dir PATH
  --include MODULE
  --exclude MODULE
  --with PRESET
  --output-dir PATH
  --clean
  --debug
  --backend pyinstaller
```

MVP options:

- `ENTRY`
- `--name`
- `--onefile`
- `--onedir`
- `--windowed`
- `--console`
- `--asset`
- `--asset-dir`
- `--include`
- `--with`
- `--clean`
- `--debug`

Implementation details:

- Implement as `python/dragongui/compiler.py` or
  `python/dragongui/compiler/__main__.py`.
- Add a console script in `pyproject.toml`:

```toml
[project.scripts]
dragongui-pack = "dragongui.compiler:main"
```

- Add an optional compiler dependency group:

```toml
[project.optional-dependencies]
compiler = ["pyinstaller>=6"]
```

- Build the PyInstaller command as a list and call it with `subprocess.run()`.
- Never shell-concatenate app paths or asset paths.
- Print the exact PyInstaller command in a copyable form.
- Write a JSON packaging report.

## Presets

Presets should add hidden imports, data collection, warnings, and documentation
notes. They should not install missing packages in the MVP.

### `--with v3-demo`

For `examples/all_features_v3_demo.py`.

Adds:

- `numpy`
- optional warning if `plotly` is absent
- smoke env suggestions: `DRAGONGUI_SMOKE_FRAMES`, `DRAGONGUI_DEMO_PAGE`,
  `DRAGONGUI_DEMO_LAYOUT_SUMMARY`

Does not add external assets because the demo writes its image and HTML reports
into temp directories.

### `--with terminal`

Adds:

- `dragongui.assets.terminal`
- optional `winpty`/`pywinpty` collection if installed
- warning that wrapped commands such as `powershell.exe`, `codex`, or `claude`
  are external executables and are not bundled automatically

### `--with html`

Adds:

- WebView2 documentation warning
- support for bundled HTML assets passed through `--asset`
- optional `HtmlReport` smoke checks

### `--with node-graph`

Adds:

- same WebView2 warning as `html`
- explicit smoke target for `examples/node_graph_editor_probe.py`

### `--with dataframe`

Adds or documents:

- `pandas`
- `polars`
- `numpy` if native table buffer extraction is desired

### `--with scientific`

Adds or documents:

- `numpy`
- `pandas`
- `polars`
- `Pillow`
- `scipy`
- one-folder recommendation for debug builds

### `--with plotly`

Adds:

- `plotly`
- any PyInstaller collect rules needed after empirical testing

## App Asset Handling

The compiler must distinguish three asset classes.

### Package Data

Data owned by the `dragongui` package, currently terminal JS/CSS. Collected by
the hook.

### Generated Runtime Files

Files created by the app while it runs. The V3 demo's image and HTML reports are
examples. These do not need to be bundled, but the packaged app must have access
to a writable temp directory.

### Bundled User Assets

Files that exist before packaging and must be included:

- CSS stylesheets loaded with `App.load_stylesheet(path)`.
- Images passed to `dg.Image(path)`.
- HTML files passed to `dg.HtmlReport(path)`.
- Fonts referenced by CSS `@font-face` file URLs or relative paths.
- Icons, data files, templates, models, config files.

Compiler requirements:

- Provide `--asset` for files/globs.
- Provide `--asset-dir` for directory trees.
- Preserve relative paths under a stable app resource root.
- Generate Windows-correct PyInstaller `--add-data` arguments.
- Include all assets in the packaging report.
- Fail early on asset globs that match nothing unless `--allow-missing-assets`
  is added later.

## Frozen Resource API

Add a public helper so user code does not need to know about PyInstaller.

Possible API:

```python
path = dg.resource_path("styles/app.dg.css")
app.load_stylesheet(path)
```

Behavior:

- In source mode, resolve relative to the entry script directory or configured
  project root.
- In frozen PyInstaller mode, resolve relative to `sys._MEIPASS` or the bundled
  resource root.
- In installed package mode, support package resources through
  `importlib.resources` for package-owned assets.

Implementation should live in one module, not scattered direct `sys._MEIPASS`
checks.

Possible internal helpers:

```python
def is_frozen() -> bool: ...
def frozen_root() -> Path | None: ...
def resource_path(relative: str | Path, *, base: str | Path | None = None) -> str: ...
```

Export `resource_path` from `dragongui.__init__` only when the behavior is
settled.

## Source Layout Shim Cleanup

Many examples contain:

```python
if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
```

For frozen app smoke targets, either:

- Leave it and verify it is harmless, or
- Update examples to also check `not getattr(sys, "frozen", False)`, or
- Create packageable smoke entry modules that do not mutate `sys.path`.

Preferred long-term fix:

```python
if __name__ == "__main__" and __package__ is None and not getattr(sys, "frozen", False):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
```

## WebView2 Requirements

Compiler docs should be explicit:

- A single `.exe` does not necessarily include the Microsoft Edge WebView2
  Runtime.
- `HtmlReport`, `Terminal`, and `NodeGraph` need WebView2 for embedded HTML on
  Windows.
- Native fallback paths may keep the app alive, but they are not equivalent to
  successful embedded HTML rendering.
- If fixed-runtime WebView2 support is needed, that belongs in an installer
  phase, not the MVP one-file compiler.

Compiler warnings:

- Warn if static analysis sees `HtmlReport`, `Terminal`, or `NodeGraph`.
- Warn if `--with html`, `--with terminal`, or `--with node-graph` is used.
- Include WebView2 runtime status in manual smoke reports if we can detect it.

## Terminal Requirements

The terminal widget:

- Uses `importlib.resources.files("dragongui.assets.terminal")` to read xterm
  JS/CSS assets.
- Starts a localhost WebSocket bridge.
- Spawns an external command with `subprocess.Popen` or `winpty`.
- Uses `pywinpty` only if available and `prefer_pty=True` on Windows.

Compiler implications:

- Terminal assets must be collected as package data.
- `pywinpty` should be a preset, not a hard dependency.
- External wrapped commands are not bundled. If a packaged app wraps `codex`,
  `claude`, `powershell.exe`, or another executable, that command must exist on
  the target machine or be explicitly shipped by the app author.
- Frozen terminal smoke should test both subprocess fallback and pywinpty mode.

## Native Extension Validation

Preflight should fail before PyInstaller if:

- Python version is older than 3.11.
- `import dragongui` fails.
- `dragongui.native_backend_available()` is false.
- `dragongui.backend_info()["native"]` is not true.

Packaged validation should verify:

- The executable can import `dragongui`.
- The executable can print `dragongui.backend_info()` in a console/self-test
  mode.
- `dragongui._dragongui` is loaded from the frozen application, not from the
  source tree.

Add a tiny generated self-test entry option if needed:

```powershell
dragongui-pack app.py --self-test-backend
```

or provide a separate generated smoke script that imports the package and exits.

## Native Binary Dependency Audit

Before assuming one-file builds are solved, inspect the compiled
`dragongui._dragongui` extension for dependent DLLs.

Validation options:

- Use `delvewheel show` on the built wheel when available.
- Use Dependencies, `dumpbin /dependents`, or an equivalent Windows dependency
  scanner on `_dragongui.pyd`.
- Confirm PyInstaller includes the `.pyd` and any non-system DLLs it requires.
- Record which dependencies are expected system components, such as WebView2
  runtime and graphics stack pieces, versus files DragonGUI must bundle.
- Re-run this audit after changing Rust dependencies such as `winit`, `wgpu`, or
  `webview2-com`.

## Build Outputs

Default outputs:

```text
build/dragongui-pack/<name>/
dist/<name>.exe
dist/<name>.packaging-report.json
```

Report fields:

- Entry script.
- Entry script hash.
- Python executable.
- Python version.
- `dragongui.__file__`.
- `dragongui.__version__`.
- `dragongui.backend_info()`.
- Source-tree or installed-package mode.
- PyInstaller version.
- One-file or one-folder mode.
- Hook directory used.
- Presets used.
- Explicit hidden imports.
- Explicit excluded modules.
- Data files and asset mappings.
- Environment variables suggested for smoke.
- Warnings.
- Final executable path.

## Documentation Requirements

Add `docs/compiler.md` covering:

- Quickstart for packaging a minimal app.
- Quickstart for packaging `all_features_v3_demo.py`.
- Python 3.11+ requirement.
- Source-tree mode versus installed-package mode.
- Why stale wheels in `dist/` should not be trusted.
- Difference between one-file and one-folder.
- How to include assets.
- How to use `dg.resource_path()` once implemented.
- Optional dependency presets.
- WebView2 expectations.
- Terminal/PTY caveats.
- External command caveats for terminal-wrapped tools.
- Scientific dependency size expectations.
- Troubleshooting missing `dragongui._dragongui`.
- Troubleshooting missing assets under PyInstaller.
- Troubleshooting WebView2 user-data directory failures.

## Implementation Phases

### Phase 0: Manual Proof And Package Freshness

- Rebuild/install the current DragonGUI package with Python 3.11.
- Confirm `dragongui.__all__` includes current modules such as `NodeGraph` and
  `TerminalEvent`.
- Confirm the current wheel, if used, includes current package modules.
- Add `plans/compiler/*.md` to `pyproject.toml` sdist includes if desired.
- Manually build one-folder V3 demo with PyInstaller.
- Manually build one-file V3 demo with PyInstaller.
- Launch both from outside the repository.
- Repeat with the cockpit mockup as a fast sanity target.

### Phase 1: Hook And Prototype Tool

- Add `tools/pyinstaller_hooks/hook-dragongui.py` for the prototype.
- Add `tools/package_app.py` as a prototype that calls PyInstaller.
- Generate `dist/<name>.packaging-report.json` with analysis paths and hook paths.
- Package the V3 demo through the tool script.
- Package the cockpit mockup through the tool script.
- Package the node graph probe through the tool script.

### Phase 2: Public CLI

- Add `dragongui.compiler` module or package.
- Add `dragongui-pack` console script.
- Add `compiler` optional dependency group with PyInstaller.
- Add clean validation and actionable errors.
- Add docs.

### Phase 3: Asset API And Asset Smoke Tests

- Add frozen-aware `resource_path` helper.
- Add image asset smoke app.
- Add stylesheet asset smoke app.
- Add HTML asset smoke app.
- Add font asset smoke app.
- Document how assets map into the frozen resource root.

### Phase 4: Presets

- Add `--with v3-demo`.
- Add `--with terminal`.
- Add `--with html`.
- Add `--with node-graph`.
- Add `--with dataframe`.
- Add `--with scientific`.
- Add `--with plotly`.

### Phase 5: Automated Smoke And CI

- Add Windows CI smoke for one-folder minimal app.
- Add Windows CI smoke for one-folder V3 demo with `DRAGONGUI_SMOKE_FRAMES`.
- Add one-file smoke where CI time permits.
- Track executable size and startup time.
- Capture backend info and packaging report artifacts.
- Add failure triage docs.

### Phase 6: Installer And Alternate Backends

- Evaluate Nuitka after PyInstaller is stable.
- Compare startup time, output size, and native extension handling.
- Evaluate Inno Setup or WiX for WebView2 bootstrap and code signing.

## Validation Strategy

Use three levels of validation.

### Import Validation

Run without opening a GUI:

- `import dragongui`
- print `dragongui.backend_info()`
- verify terminal assets are readable through `importlib.resources`
- verify public modules import: `node_graph`, `agent_messages`, `agent_session`,
  `terminal`

### Native Smoke Validation

Run packaged apps with native windows:

- minimal window
- V3 demo
- node graph probe
- terminal wrapper
- image asset smoke
- HTML asset smoke

Use `DRAGONGUI_SMOKE_FRAMES` where supported by native runtime to keep the app
short-lived in automation.

### Manual Visual Validation

Before calling the compiler usable, manually inspect:

- V3 overview page.
- V3 debug page with HtmlReport.
- V3 styling page with generated image.
- Node graph editor probe.
- Terminal wrapper with subprocess fallback.
- Terminal wrapper with pywinpty if installed.

## Risks

- The source package can drift ahead of the last wheel in `dist/`.
- PyInstaller may miss dynamic imports in optional dependencies.
- Broad `collect_submodules("dragongui")` is safer but may increase size.
- One-file startup can be slower because files extract to a temp directory.
- Scientific stacks can produce very large executables.
- WebView2 availability is outside the `.exe` unless an installer handles it.
- Unsigned one-file executables can trigger security software suspicion.
- `pywinpty` may require additional DLL collection rules.
- Apps that use repository-relative paths will fail unless assets are bundled
  and resolved through a frozen-aware helper.
- Local `@font-face` files and file-backed HtmlReport content need explicit
  asset handling.
- Terminal-wrapped external commands may be absent on target machines.

## Acceptance Criteria

MVP is complete when:

- A fresh Python 3.11 environment can install/build current DragonGUI.
- The compiler preflight rejects missing native backend builds.
- `dragongui-pack examples\all_features_v3_demo.py --onedir --name DragonGUIV3Demo --with v3-demo`
  produces a runnable one-folder app.
- `dragongui-pack examples\all_features_v3_demo.py --onefile --name DragonGUIV3Demo --with v3-demo`
  produces `dist\DragonGUIV3Demo.exe`.
- `DragonGUIV3Demo.exe` launches from outside the repository.
- `dragongui._dragongui` loads from the packaged app.
- Terminal assets are bundled and readable with `importlib.resources`.
- The packaging report records Python version, package path, backend info,
  hook path, presets, included assets, and warnings.
- Documentation explains Python version, package freshness, WebView2, assets,
  optional dependencies, and terminal external-command caveats.

Broader compiler support is complete when:

- Minimal, V3, node graph, terminal, HTML, image, scatter, and DataFrame apps all
  package and launch.
- Users can include assets without writing PyInstaller commands directly.
- Common optional dependency presets work.
- CI catches packaging regressions on Windows.
