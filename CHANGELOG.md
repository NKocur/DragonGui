# Changelog

All notable changes to DragonGUI are documented here. The project follows
[Semantic Versioning](https://semver.org/) from version 1.0.0 onward.

## [1.0.0] - 2026-08-05

First stable production release.

### Highlights

- GPU-native desktop application runtime backed by Rust, wgpu, winit, Taffy,
  and glyphon.
- Stable Python API for application shells, layouts, controls, charts, tables,
  dialogs, themes, components, live updates, and retained resources.
- GPU scatter, line, histogram, bar, heatmap, pie, and custom paint widgets.
- CSS styling, responsive layout, transitions, animations, widget parts, and
  runtime theme replacement.
- Windows x86-64, Linux x86-64, macOS Intel, and macOS Apple Silicon wheels
  supporting Python 3.12 and 3.13 through the CPython stable ABI.
- PyO3 0.29 and refreshed transitive dependencies with zero known RustSec
  vulnerabilities at release preparation time.
- Complete MIT project licensing and generated third-party dependency notices.

### Compatibility

- `NodeGraph` remains experimental and is not covered by the stable 1.x API
  compatibility guarantee.
- Python 3.12 or newer is required; the 1.0.0 validation matrix covers Python
  3.12 and 3.13.

[1.0.0]: https://github.com/NKocur/DragonGui/releases/tag/v1.0.0
