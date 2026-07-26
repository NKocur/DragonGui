# Layout and Styling Evolution Release Notes

Status: release candidate, Phase 11 hardening complete on 2026-07-25.

## What changed

DragonGUI now treats public widget identity, sizing, layout diagnostics, and
responsive behavior as explicit library contracts.

- Public selectors such as `SearchBox`, `AppShell`, `WorkbenchLayout`,
  `Sidebar`, `GridLayout`, and widget part selectors no longer require
  knowledge of private native render kinds.
- Widget defaults, framework CSS, theme values, application CSS, and inline
  styles participate in a documented cascade with computed-style provenance.
- Panels are content-sized unless the application explicitly makes them
  flexible. Main workbench/body regions remain shrink-safe and own their
  intended scrolling.
- Composite controls expose preferred/minimum/maximum sizing and predictable
  grow/shrink behavior. `SearchBox(grow=True)` is the explicit fill-space form.
- `FlexLayout` and `AppShell` can change direction at responsive breakpoints.
  `Sidebar` supports expanded, collapsed, hidden, automatic rail, and mobile
  drawer policies.
- `GridLayout.columns` accepts responsive breakpoint maps, while
  `balance_last_row=True` centers incomplete non-masonry rows.
- Debug snapshots and visual audits report structural errors separately from
  usability advisories, along with resolved grids, selector matches, cascade
  provenance, flex allocation, clips, and scroll ownership.

## Compatibility guidance

Existing integer grid columns, ordinary `HLayout`/`VLayout`, inline style
dictionaries, fixed Sidebar widths, and `Modal.set_open(...)` continue to work.
The following explicit forms retain behavior that older applications may have
received implicitly:

| Older expectation | Explicit current form |
| --- | --- |
| A `Panel` fills remaining height | `Panel(..., style={"flex": 1, "min_height": 0})` |
| A search field fills its toolbar row | `SearchBox(..., grow=True)` |
| A search field keeps an exact width | `SearchBox(..., width=280, grow=False)` |
| Navigation never changes at small widths | `Sidebar(..., state="expanded")` |
| Fixed grid column count | `GridLayout(columns=4)` |
| Responsive grid | `GridLayout(columns={"default": 4, 1100: 2, 700: 1})` |
| Horizontal controls must wrap | `FlexLayout(direction="row", wrap=True)` or `FlowLayout(...)` |
| Content taller than its viewport remains reachable | Put it in an explicit `Body`/`ScrollArea` and keep the scroll owner shrink-safe |

Application styles should use public widget selectors and documented parts.
Private lowercase render-kind selectors are implementation details and should
be migrated. Strict CSS audit mode reports public selectors that match no
eligible node; conditional selectors are evaluated in their active responsive
state.

## Release validation

- Native suite: 738 passed, 8 known unrelated failures, 12 ignored.
- Python API/VDOM/layout/visual suite: 479 passed, 4 known unrelated failures.
- Four CSS demos: zero warnings, layout issues, structural errors, usability
  advisories, or unmatched selectors under all strict modes.
- Required torture matrix: 112 passing captures across seven sizes and
  1x/1.25x/1.5x/2x scales.
- Aurora: every page and interactive state passed compact/desktop and 1x/2x
  checks; explicit sidebar, modal, and scroll round trips also passed at 1.5x.
- Professional demo: all eight pages pass compact/desktop and 1x/2x checks.
- The final ABI3 wheel passed isolated import, WGPU runtime, live stylesheet
  reload, strict diagnostic, and memory smokes.

The eight native failures are pre-existing primitive/text rendering assertions.
The four Python failures are an environment-dependent integer buffer width, two
tests blocked by the local broken SciPy `_fblas` DLL, and one existing NodeGraph
terminal-bridge replacement assertion. Assertions were not weakened or skipped.

Detailed measurements and artifact locations are in
`artifacts/layout-styling-phase11/METRICS.md`.

