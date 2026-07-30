# DragonGUI Visual Audit Report

Generated: 2026-07-29 15:43:47 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### Application Icon Theme (`icon-theme`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\icon_theme_demo.py`
- Features: `IconButton`, `IconResource`, `IconStroke`, `semantic icon aliases`
- Screenshots: [icon-theme-desktop-1@1x.png](screenshots/icon-theme-desktop-1@1x.png), [icon-theme-desktop-2@1x-live-theme-swapped.png](screenshots/icon-theme-desktop-2@1x-live-theme-swapped.png), [icon-theme-desktop-3@1x-live-icon-changed.png](screenshots/icon-theme-desktop-3@1x-live-icon-changed.png), [icon-theme-desktop-4@1.5x.png](screenshots/icon-theme-desktop-4@1.5x.png), [icon-theme-desktop-5@1.5x-live-theme-swapped.png](screenshots/icon-theme-desktop-5@1.5x-live-theme-swapped.png), [icon-theme-desktop-6@1.5x-live-icon-changed.png](screenshots/icon-theme-desktop-6@1.5x-live-icon-changed.png), [icon-theme-desktop-7@2x.png](screenshots/icon-theme-desktop-7@2x.png), [icon-theme-desktop-8@2x-live-theme-swapped.png](screenshots/icon-theme-desktop-8@2x-live-theme-swapped.png), [icon-theme-desktop-9@2x-live-icon-changed.png](screenshots/icon-theme-desktop-9@2x-live-icon-changed.png)
- Debug snapshots: [icon-theme-desktop-1@1x.json](snapshots/icon-theme-desktop-1@1x.json), [icon-theme-desktop-2@1x-live-theme-swapped.json](snapshots/icon-theme-desktop-2@1x-live-theme-swapped.json), [icon-theme-desktop-3@1x-live-icon-changed.json](snapshots/icon-theme-desktop-3@1x-live-icon-changed.json), [icon-theme-desktop-4@1.5x.json](snapshots/icon-theme-desktop-4@1.5x.json), [icon-theme-desktop-5@1.5x-live-theme-swapped.json](snapshots/icon-theme-desktop-5@1.5x-live-theme-swapped.json), [icon-theme-desktop-6@1.5x-live-icon-changed.json](snapshots/icon-theme-desktop-6@1.5x-live-icon-changed.json), [icon-theme-desktop-7@2x.json](snapshots/icon-theme-desktop-7@2x.json), [icon-theme-desktop-8@2x-live-theme-swapped.json](snapshots/icon-theme-desktop-8@2x-live-theme-swapped.json), [icon-theme-desktop-9@2x-live-icon-changed.json](snapshots/icon-theme-desktop-9@2x-live-icon-changed.json)
- Logs: [icon-theme-desktop-1@1x.stdout.txt](logs/icon-theme-desktop-1@1x.stdout.txt), [icon-theme-desktop-1@1x.stderr.txt](logs/icon-theme-desktop-1@1x.stderr.txt), [icon-theme-desktop-2@1x-live-theme-swapped.stdout.txt](logs/icon-theme-desktop-2@1x-live-theme-swapped.stdout.txt), [icon-theme-desktop-2@1x-live-theme-swapped.stderr.txt](logs/icon-theme-desktop-2@1x-live-theme-swapped.stderr.txt), [icon-theme-desktop-3@1x-live-icon-changed.stdout.txt](logs/icon-theme-desktop-3@1x-live-icon-changed.stdout.txt), [icon-theme-desktop-3@1x-live-icon-changed.stderr.txt](logs/icon-theme-desktop-3@1x-live-icon-changed.stderr.txt), [icon-theme-desktop-4@1.5x.stdout.txt](logs/icon-theme-desktop-4@1.5x.stdout.txt), [icon-theme-desktop-4@1.5x.stderr.txt](logs/icon-theme-desktop-4@1.5x.stderr.txt), [icon-theme-desktop-5@1.5x-live-theme-swapped.stdout.txt](logs/icon-theme-desktop-5@1.5x-live-theme-swapped.stdout.txt), [icon-theme-desktop-5@1.5x-live-theme-swapped.stderr.txt](logs/icon-theme-desktop-5@1.5x-live-theme-swapped.stderr.txt), [icon-theme-desktop-6@1.5x-live-icon-changed.stdout.txt](logs/icon-theme-desktop-6@1.5x-live-icon-changed.stdout.txt), [icon-theme-desktop-6@1.5x-live-icon-changed.stderr.txt](logs/icon-theme-desktop-6@1.5x-live-icon-changed.stderr.txt), [icon-theme-desktop-7@2x.stdout.txt](logs/icon-theme-desktop-7@2x.stdout.txt), [icon-theme-desktop-7@2x.stderr.txt](logs/icon-theme-desktop-7@2x.stderr.txt), [icon-theme-desktop-8@2x-live-theme-swapped.stdout.txt](logs/icon-theme-desktop-8@2x-live-theme-swapped.stdout.txt), [icon-theme-desktop-8@2x-live-theme-swapped.stderr.txt](logs/icon-theme-desktop-8@2x-live-theme-swapped.stderr.txt), [icon-theme-desktop-9@2x-live-icon-changed.stdout.txt](logs/icon-theme-desktop-9@2x-live-icon-changed.stdout.txt), [icon-theme-desktop-9@2x-live-icon-changed.stderr.txt](logs/icon-theme-desktop-9@2x-live-icon-changed.stderr.txt)
- Unmatched selectors: _none_
- Layout diagnostics by code: _none_
- Notes: Phase 9 regression for resource-backed monochrome icon overrides and built-in fallback. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target icon-theme --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target icon-theme --sizes desktop-2 --scales 1 # state=live-theme-swapped`; `python tools/visual_audit.py --target icon-theme --sizes desktop-3 --scales 1 # state=live-icon-changed`; `python tools/visual_audit.py --target icon-theme --sizes desktop-4 --scales 1.5`; `python tools/visual_audit.py --target icon-theme --sizes desktop-5 --scales 1.5 # state=live-theme-swapped`; `python tools/visual_audit.py --target icon-theme --sizes desktop-6 --scales 1.5 # state=live-icon-changed`; `python tools/visual_audit.py --target icon-theme --sizes desktop-7 --scales 2`; `python tools/visual_audit.py --target icon-theme --sizes desktop-8 --scales 2 # state=live-theme-swapped`; `python tools/visual_audit.py --target icon-theme --sizes desktop-9 --scales 2 # state=live-icon-changed`

#### Capture Gallery

| Size | Scale | Route | State | Thumbnail |
| --- | ---: | --- | --- | --- |
| `default` | `1x` | `_default_` | `default` | <a href="screenshots/icon-theme-desktop-1@1x.png"><img src="screenshots/icon-theme-desktop-1@1x.png" width="240" alt="icon-theme default default"></a> |
| `default` | `1x` | `_default_` | `live-theme-swapped` | <a href="screenshots/icon-theme-desktop-2@1x-live-theme-swapped.png"><img src="screenshots/icon-theme-desktop-2@1x-live-theme-swapped.png" width="240" alt="icon-theme default live-theme-swapped"></a> |
| `default` | `1x` | `_default_` | `live-icon-changed` | <a href="screenshots/icon-theme-desktop-3@1x-live-icon-changed.png"><img src="screenshots/icon-theme-desktop-3@1x-live-icon-changed.png" width="240" alt="icon-theme default live-icon-changed"></a> |
| `default` | `1.5x` | `_default_` | `default` | <a href="screenshots/icon-theme-desktop-4@1.5x.png"><img src="screenshots/icon-theme-desktop-4@1.5x.png" width="240" alt="icon-theme default default"></a> |
| `default` | `1.5x` | `_default_` | `live-theme-swapped` | <a href="screenshots/icon-theme-desktop-5@1.5x-live-theme-swapped.png"><img src="screenshots/icon-theme-desktop-5@1.5x-live-theme-swapped.png" width="240" alt="icon-theme default live-theme-swapped"></a> |
| `default` | `1.5x` | `_default_` | `live-icon-changed` | <a href="screenshots/icon-theme-desktop-6@1.5x-live-icon-changed.png"><img src="screenshots/icon-theme-desktop-6@1.5x-live-icon-changed.png" width="240" alt="icon-theme default live-icon-changed"></a> |
| `default` | `2x` | `_default_` | `default` | <a href="screenshots/icon-theme-desktop-7@2x.png"><img src="screenshots/icon-theme-desktop-7@2x.png" width="240" alt="icon-theme default default"></a> |
| `default` | `2x` | `_default_` | `live-theme-swapped` | <a href="screenshots/icon-theme-desktop-8@2x-live-theme-swapped.png"><img src="screenshots/icon-theme-desktop-8@2x-live-theme-swapped.png" width="240" alt="icon-theme default live-theme-swapped"></a> |
| `default` | `2x` | `_default_` | `live-icon-changed` | <a href="screenshots/icon-theme-desktop-9@2x-live-icon-changed.png"><img src="screenshots/icon-theme-desktop-9@2x-live-icon-changed.png" width="240" alt="icon-theme default live-icon-changed"></a> |

#### Diagnostic State Comparisons

- `default @ 1x`: `default` → `live-theme-swapped` — _no diagnostic changes_
- `default @ 1x`: `default` → `live-icon-changed` — _no diagnostic changes_
- `default @ 1.5x`: `default` → `live-theme-swapped` — _no diagnostic changes_
- `default @ 1.5x`: `default` → `live-icon-changed` — _no diagnostic changes_
- `default @ 2x`: `default` → `live-theme-swapped` — _no diagnostic changes_
- `default @ 2x`: `default` → `live-icon-changed` — _no diagnostic changes_
