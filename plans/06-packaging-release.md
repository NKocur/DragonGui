# Packaging And Release Plan

## Objective

Keep the project installable from source during development and publishable to
PyPI when the native backend is useful.

Cross-platform smoke coverage starts at M1. Wheel release work happens later,
but platform drift should not accumulate silently.

## Development Workflows

No-install source run:

```powershell
.\start.bat
```

Editable native development:

```powershell
python -m pip install -e ".[dev]"
python -m maturin develop
python examples\scatter_tool.py
```

Wheel build:

```powershell
python -m maturin build --release --out dist --target x86_64-pc-windows-gnu
```

Source distribution:

```powershell
python -m maturin sdist --out dist
```

## CI Work Items

M1:

- Windows build and import smoke test.
- macOS build and native-window smoke path, or documented skip if graphical CI
  is not available.
- Linux build smoke test.

Before TestPyPI:

- Wheel builds for:
  - Windows x86_64
  - macOS x86_64
  - macOS arm64
  - Linux x86_64 manylinux
- Source distribution build.
- `twine check dist/*`.
- Fresh virtual environment install tests.

## Release Work Items

- Publish to TestPyPI first.
- Verify install in fresh virtual environments.
- Add a short limitations section before first release.
- Add changelog before version `0.1.0` is published publicly.
- Add compatibility matrix for Python, OS, and GPU backend support.

## PyPI Hygiene

- Keep runtime dependencies minimal.
- Put pandas, Polars, NumPy, and Arrow behind optional extras unless required.
- Keep examples and tests in sdist.
- Keep wheel contents limited to `dragongui`, the native extension, metadata,
  and license files.
- Do not include `target`, `.test-cache`, `dist`, or `__pycache__`.

## Acceptance Criteria

- `pip install dist\*.whl` works in a normal CPython virtual environment.
- `python -c "import dragongui; print(dragongui.backend_info())"` works after
  install.
- `twine check dist/*` passes once `twine` is available.
- TestPyPI install instructions are documented.
