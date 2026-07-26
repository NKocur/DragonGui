# DragonGUI Visual Audit Report

Generated: 2026-07-24 18:23:40 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### Aurora Operations Command Center (`aurora-command-center`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\aurora_command_center_demo.py`
- Features: `Sophisticated application demo`, `AppShell`, `Sidebar`, `WorkbenchLayout`, `MenuBar`, `Toolbar`, `Pages`, `Body`, `ScrollArea`, `GridLayout`, `FlowLayout`, `DataFrameTable`, `charts`, `forms`, `Modal`, `StatusBar`
- Screenshots: `screenshots/aurora-command-center-390x720@1x.png`, `screenshots/aurora-command-center-640x480@1x.png`, `screenshots/aurora-command-center-1024x768@1x.png`, `screenshots/aurora-command-center-1440x900@1x.png`
- Debug snapshots: `snapshots/aurora-command-center-390x720@1x.json`, `snapshots/aurora-command-center-390x720@1x-resize-0-start-390x720.json`, `snapshots/aurora-command-center-390x720@1x-resize-1-640x480.json`, `snapshots/aurora-command-center-390x720@1x-resize-2-1024x768.json`, `snapshots/aurora-command-center-390x720@1x-resize-3-390x720.json`, `snapshots/aurora-command-center-640x480@1x.json`, `snapshots/aurora-command-center-640x480@1x-resize-0-start-640x480.json`, `snapshots/aurora-command-center-640x480@1x-resize-1-640x480.json`, `snapshots/aurora-command-center-640x480@1x-resize-2-1024x768.json`, `snapshots/aurora-command-center-640x480@1x-resize-3-640x480.json`, `snapshots/aurora-command-center-1024x768@1x.json`, `snapshots/aurora-command-center-1024x768@1x-resize-0-start-1024x768.json`, `snapshots/aurora-command-center-1024x768@1x-resize-1-640x480.json`, `snapshots/aurora-command-center-1024x768@1x-resize-2-1024x768.json`, `snapshots/aurora-command-center-1024x768@1x-resize-3-1024x768.json`, `snapshots/aurora-command-center-1440x900@1x.json`, `snapshots/aurora-command-center-1440x900@1x-resize-0-start-1440x864.json`, `snapshots/aurora-command-center-1440x900@1x-resize-1-640x480.json`, `snapshots/aurora-command-center-1440x900@1x-resize-2-1024x768.json`, `snapshots/aurora-command-center-1440x900@1x-resize-3-1440x864.json`
- Logs: `logs/aurora-command-center-390x720@1x.stdout.txt`, `logs/aurora-command-center-390x720@1x.stderr.txt`, `logs/aurora-command-center-640x480@1x.stdout.txt`, `logs/aurora-command-center-640x480@1x.stderr.txt`, `logs/aurora-command-center-1024x768@1x.stdout.txt`, `logs/aurora-command-center-1024x768@1x.stderr.txt`, `logs/aurora-command-center-1440x900@1x.stdout.txt`, `logs/aurora-command-center-1440x900@1x.stderr.txt`
- Notes: Sleek full-application stress demo intended to expose responsive shell, dense-card, scrolling, and intrinsic-size regressions. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target aurora-command-center --sizes 390x720 --scales 1`; `python tools/visual_audit.py --target aurora-command-center --sizes 640x480 --scales 1`; `python tools/visual_audit.py --target aurora-command-center --sizes 1024x768 --scales 1`; `python tools/visual_audit.py --target aurora-command-center --sizes 1440x900 --scales 1`
