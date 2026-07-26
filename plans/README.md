# DragonGUI Plans

This folder tracks the implementation plan for DragonGUI as a Python
application toolkit for GPU-native data tools.

The revised plan optimizes for an early showable demo: a Python script opens a
native `wgpu` window and displays a DragonSci-powered scatter plot before the
full widget system exists.

## Plan Index

1. [Milestones](./00-milestones.md)
2. [Native Backend](./01-native-backend.md)
3. [Python API And Update Protocol](./02-python-api.md)
4. [DragonSci Scatter Integration](./03-dragonsci-integration.md)
5. [Layout, Widgets, And Theme](./04-layout-widgets-theme.md)
6. [DataFrame Table](./05-dataframe-table.md)
7. [Navigation And Multipage](./08-navigation-tabs.md)
8. [Shipping Widget Set](./09-shipping-widget-set.md)
9. [Packaging And Release](./06-packaging-release.md)
10. [Benchmarks](./07-benchmarks.md)

Working note:

- [DragonSci Inventory](./dragonsci-inventory.md)
- [Layout System Remediation](./layout-system-remediation.md)
- [Layout and Styling Evolution](./layout-styling-evolution.md)

## Current Status

- PyPI-ready `maturin` package scaffold exists.
- Python declarative widget tree exists.
- Rust/PyO3 native module scaffold exists.
- Source-tree launch path exists through `start.bat`.
- Native GUI window, renderer, layout, text, events, basic widgets, scatter,
  and initial DataFrame table rendering are implemented.
- DragonSci is available as the Python package `dragonsci` in the Python 3.11
  environment and should be treated as the reference implementation for scatter
  behavior.

## Next Implementation Slice

Continue M6 by replacing startup-only sampled table cells with scroll-time
visible-row extraction, then finish M8 shipping widgets before packaging work.
