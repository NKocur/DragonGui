# Phase 11 Release-Hardening Metrics

Recorded on Windows on 2026-07-25. Phase 0 and Phase 11 values below use the
same probe, logical viewport, scale factor, six layout applications, and native
debug-snapshot fields.

| Probe | Metric | Phase 0 | Phase 11 | Change |
| --- | --- | ---: | ---: | ---: |
| responsive grid/orphan, 1024x640 at 1x | average frame | 4.550 ms | 0.900 ms | -80.2% |
| responsive grid/orphan, 1024x640 at 1x | average apply-layout | 9.923 ms | 3.854 ms | -61.2% |
| responsive grid/orphan, 1024x640 at 1x | average stylesheet reapply | 0.221 ms | 0.561 ms | +153.5% (+0.340 ms) |
| responsive grid/orphan, 1024x640 at 1x | snapshot size | 116,551 B | 428,694 B | +267.8% |
| sidebar allocation, 900x680 at 1x | average frame | 4.949 ms | 0.943 ms | -80.9% |
| sidebar allocation, 900x680 at 1x | average apply-layout | 12.642 ms | 4.426 ms | -65.0% |
| sidebar allocation, 900x680 at 1x | average stylesheet reapply | 0.233 ms | 0.530 ms | +127.4% (+0.297 ms) |
| sidebar allocation, 900x680 at 1x | snapshot size | 136,238 B | 477,113 B | +250.2% |

Frame and layout performance improved substantially. Stylesheet reapplication
remains below 0.6 ms in both probes; its sub-millisecond increase is associated
with the richer computed-style selector and provenance data added since Phase
0. Snapshot growth is intentional debug observability overhead, not retained
per-frame render data.

The final extracted-wheel 600-frame smoke peaked at 25,251,840 bytes (24.08
MiB) of process working set. Phase 0 did not record process memory, so no honest
historical memory delta is available. This value is the release baseline for
future comparisons.

The rebuilt wheel is
`dist/dragongui-0.1.0-cp312-abi3-win_amd64.whl` (7,719,131 bytes). An isolated
runtime smoke imported `.test-cache/wheel_extract/dragongui/__init__.py`,
rendered with WGPU, produced zero stylesheet/layout issues, and observed live
`ClearStylesheets` and `SetStylesheet` commands.

Visual evidence:

- `torture-matrix`: 112 passing combinations across four probes, seven sizes,
  and four scale factors.
- `flagship-states`: Aurora's original nine states at compact/desktop and
  1x/2x, plus all eight professional-demo pages.
- `workflow-fix-verification`: four clean post-fix captures proving complete
  workflow-page scroll reachability.
- `transition-roundtrips`: all 12 Aurora states at 1.5x, including explicit
  sidebar, modal, and scroll round trips plus resize checkpoints.

