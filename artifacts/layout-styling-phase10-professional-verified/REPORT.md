# DragonGUI Visual Audit Report

Generated: 2026-07-25 14:59:01 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### All Features Professional Demo - Overview (`all-features-professional-overview`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\all_features_professional_demo_probe.py`
- Features: `Professional demo`, `AppShell`, `Sidebar`, `Toolbar`, `Tabs`, `Body`, `StatusBar`, `LinePlot`, `PieChart`, `DataFrameTable`
- Screenshots: [all-features-professional-overview-1440x900@1x.png](screenshots/all-features-professional-overview-1440x900@1x.png), [all-features-professional-overview-390x720@1x.png](screenshots/all-features-professional-overview-390x720@1x.png)
- Debug snapshots: [all-features-professional-overview-1440x900@1x.json](snapshots/all-features-professional-overview-1440x900@1x.json), [all-features-professional-overview-390x720@1x.json](snapshots/all-features-professional-overview-390x720@1x.json)
- Logs: [all-features-professional-overview-1440x900@1x.stdout.txt](logs/all-features-professional-overview-1440x900@1x.stdout.txt), [all-features-professional-overview-1440x900@1x.stderr.txt](logs/all-features-professional-overview-1440x900@1x.stderr.txt), [all-features-professional-overview-390x720@1x.stdout.txt](logs/all-features-professional-overview-390x720@1x.stdout.txt), [all-features-professional-overview-390x720@1x.stderr.txt](logs/all-features-professional-overview-390x720@1x.stderr.txt)
- Unmatched selectors: _none_
- Layout diagnostics by code: _none_
- Notes: Route-specific capture for the professional all-features demo overview page. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`, `native/src/scatter/mod.rs`, `native/src/table.rs`
- Reproduction: `python tools/visual_audit.py --target all-features-professional-overview --sizes 1440x900 --scales 1`; `python tools/visual_audit.py --target all-features-professional-overview --sizes 390x720 --scales 1`

#### Capture Gallery

| Size | Scale | Route | State | Thumbnail |
| --- | ---: | --- | --- | --- |
| `1440x900` | `1x` | `_default_` | `default` | <a href="screenshots/all-features-professional-overview-1440x900@1x.png"><img src="screenshots/all-features-professional-overview-1440x900@1x.png" width="240" alt="all-features-professional-overview 1440x900 default"></a> |
| `390x720` | `1x` | `_default_` | `default` | <a href="screenshots/all-features-professional-overview-390x720@1x.png"><img src="screenshots/all-features-professional-overview-390x720@1x.png" width="240" alt="all-features-professional-overview 390x720 default"></a> |
