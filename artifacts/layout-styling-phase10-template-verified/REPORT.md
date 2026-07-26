# DragonGUI Visual Audit Report

Generated: 2026-07-25 14:54:37 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### Responsive Application Template (`responsive-app-template`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\responsive_app_template.py`
- Features: `AppShell`, `Sidebar`, `WorkbenchLayout`, `Toolbar`, `Body`, `Pages`, `ScrollArea`, `responsive GridLayout`, `StatusBar`
- Screenshots: [responsive-app-template-1100x720@1x-overview.png](screenshots/responsive-app-template-1100x720@1x-overview.png), [responsive-app-template-1100x720@1x-activity.png](screenshots/responsive-app-template-1100x720@1x-activity.png), [responsive-app-template-1100x720@1x-settings.png](screenshots/responsive-app-template-1100x720@1x-settings.png), [responsive-app-template-1100x720@1x-sidebar-collapsed.png](screenshots/responsive-app-template-1100x720@1x-sidebar-collapsed.png), [responsive-app-template-390x720@1x-overview.png](screenshots/responsive-app-template-390x720@1x-overview.png), [responsive-app-template-390x720@1x-activity.png](screenshots/responsive-app-template-390x720@1x-activity.png), [responsive-app-template-390x720@1x-settings.png](screenshots/responsive-app-template-390x720@1x-settings.png), [responsive-app-template-390x720@1x-sidebar-collapsed.png](screenshots/responsive-app-template-390x720@1x-sidebar-collapsed.png)
- Debug snapshots: [responsive-app-template-1100x720@1x-overview.json](snapshots/responsive-app-template-1100x720@1x-overview.json), [responsive-app-template-1100x720@1x-activity.json](snapshots/responsive-app-template-1100x720@1x-activity.json), [responsive-app-template-1100x720@1x-settings.json](snapshots/responsive-app-template-1100x720@1x-settings.json), [responsive-app-template-1100x720@1x-sidebar-collapsed.json](snapshots/responsive-app-template-1100x720@1x-sidebar-collapsed.json), [responsive-app-template-390x720@1x-overview.json](snapshots/responsive-app-template-390x720@1x-overview.json), [responsive-app-template-390x720@1x-activity.json](snapshots/responsive-app-template-390x720@1x-activity.json), [responsive-app-template-390x720@1x-settings.json](snapshots/responsive-app-template-390x720@1x-settings.json), [responsive-app-template-390x720@1x-sidebar-collapsed.json](snapshots/responsive-app-template-390x720@1x-sidebar-collapsed.json)
- Logs: [responsive-app-template-1100x720@1x-overview.stdout.txt](logs/responsive-app-template-1100x720@1x-overview.stdout.txt), [responsive-app-template-1100x720@1x-overview.stderr.txt](logs/responsive-app-template-1100x720@1x-overview.stderr.txt), [responsive-app-template-1100x720@1x-activity.stdout.txt](logs/responsive-app-template-1100x720@1x-activity.stdout.txt), [responsive-app-template-1100x720@1x-activity.stderr.txt](logs/responsive-app-template-1100x720@1x-activity.stderr.txt), [responsive-app-template-1100x720@1x-settings.stdout.txt](logs/responsive-app-template-1100x720@1x-settings.stdout.txt), [responsive-app-template-1100x720@1x-settings.stderr.txt](logs/responsive-app-template-1100x720@1x-settings.stderr.txt), [responsive-app-template-1100x720@1x-sidebar-collapsed.stdout.txt](logs/responsive-app-template-1100x720@1x-sidebar-collapsed.stdout.txt), [responsive-app-template-1100x720@1x-sidebar-collapsed.stderr.txt](logs/responsive-app-template-1100x720@1x-sidebar-collapsed.stderr.txt), [responsive-app-template-390x720@1x-overview.stdout.txt](logs/responsive-app-template-390x720@1x-overview.stdout.txt), [responsive-app-template-390x720@1x-overview.stderr.txt](logs/responsive-app-template-390x720@1x-overview.stderr.txt), [responsive-app-template-390x720@1x-activity.stdout.txt](logs/responsive-app-template-390x720@1x-activity.stdout.txt), [responsive-app-template-390x720@1x-activity.stderr.txt](logs/responsive-app-template-390x720@1x-activity.stderr.txt), [responsive-app-template-390x720@1x-settings.stdout.txt](logs/responsive-app-template-390x720@1x-settings.stdout.txt), [responsive-app-template-390x720@1x-settings.stderr.txt](logs/responsive-app-template-390x720@1x-settings.stderr.txt), [responsive-app-template-390x720@1x-sidebar-collapsed.stdout.txt](logs/responsive-app-template-390x720@1x-sidebar-collapsed.stdout.txt), [responsive-app-template-390x720@1x-sidebar-collapsed.stderr.txt](logs/responsive-app-template-390x720@1x-sidebar-collapsed.stderr.txt)
- Unmatched selectors: _none_
- Layout diagnostics by code: _none_
- Notes: Small public-API template for responsive shells, navigation, grids, and explicit scroll ownership. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target responsive-app-template --sizes 1100x720 --scales 1 # state=overview`; `python tools/visual_audit.py --target responsive-app-template --sizes 1100x720 --scales 1 # state=activity`; `python tools/visual_audit.py --target responsive-app-template --sizes 1100x720 --scales 1 # state=settings`; `python tools/visual_audit.py --target responsive-app-template --sizes 1100x720 --scales 1 # state=sidebar-collapsed`; `python tools/visual_audit.py --target responsive-app-template --sizes 390x720 --scales 1 # state=overview`; `python tools/visual_audit.py --target responsive-app-template --sizes 390x720 --scales 1 # state=activity`; `python tools/visual_audit.py --target responsive-app-template --sizes 390x720 --scales 1 # state=settings`; `python tools/visual_audit.py --target responsive-app-template --sizes 390x720 --scales 1 # state=sidebar-collapsed`

#### Capture Gallery

| Size | Scale | Route | State | Thumbnail |
| --- | ---: | --- | --- | --- |
| `1100x720` | `1x` | `overview` | `overview` | <a href="screenshots/responsive-app-template-1100x720@1x-overview.png"><img src="screenshots/responsive-app-template-1100x720@1x-overview.png" width="240" alt="responsive-app-template 1100x720 overview"></a> |
| `1100x720` | `1x` | `activity` | `activity` | <a href="screenshots/responsive-app-template-1100x720@1x-activity.png"><img src="screenshots/responsive-app-template-1100x720@1x-activity.png" width="240" alt="responsive-app-template 1100x720 activity"></a> |
| `1100x720` | `1x` | `settings` | `settings` | <a href="screenshots/responsive-app-template-1100x720@1x-settings.png"><img src="screenshots/responsive-app-template-1100x720@1x-settings.png" width="240" alt="responsive-app-template 1100x720 settings"></a> |
| `1100x720` | `1x` | `overview` | `sidebar-collapsed` | <a href="screenshots/responsive-app-template-1100x720@1x-sidebar-collapsed.png"><img src="screenshots/responsive-app-template-1100x720@1x-sidebar-collapsed.png" width="240" alt="responsive-app-template 1100x720 sidebar-collapsed"></a> |
| `390x720` | `1x` | `overview` | `overview` | <a href="screenshots/responsive-app-template-390x720@1x-overview.png"><img src="screenshots/responsive-app-template-390x720@1x-overview.png" width="240" alt="responsive-app-template 390x720 overview"></a> |
| `390x720` | `1x` | `activity` | `activity` | <a href="screenshots/responsive-app-template-390x720@1x-activity.png"><img src="screenshots/responsive-app-template-390x720@1x-activity.png" width="240" alt="responsive-app-template 390x720 activity"></a> |
| `390x720` | `1x` | `settings` | `settings` | <a href="screenshots/responsive-app-template-390x720@1x-settings.png"><img src="screenshots/responsive-app-template-390x720@1x-settings.png" width="240" alt="responsive-app-template 390x720 settings"></a> |
| `390x720` | `1x` | `overview` | `sidebar-collapsed` | <a href="screenshots/responsive-app-template-390x720@1x-sidebar-collapsed.png"><img src="screenshots/responsive-app-template-390x720@1x-sidebar-collapsed.png" width="240" alt="responsive-app-template 390x720 sidebar-collapsed"></a> |

#### Diagnostic State Comparisons

- `1100x720 @ 1x`: `overview` → `activity` — _no diagnostic changes_
- `1100x720 @ 1x`: `overview` → `settings` — _no diagnostic changes_
- `1100x720 @ 1x`: `overview` → `sidebar-collapsed` — _no diagnostic changes_
- `390x720 @ 1x`: `overview` → `activity` — _no diagnostic changes_
- `390x720 @ 1x`: `overview` → `settings` — _no diagnostic changes_
- `390x720 @ 1x`: `overview` → `sidebar-collapsed` — _no diagnostic changes_
