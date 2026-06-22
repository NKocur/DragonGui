# Performance

DragonGUI is optimized around native rendering, packed data paths, and retained
widget state.

## General Guidance

- Reuse widgets and update values through live methods.
- Avoid rebuilding large tables or plot subtrees every frame.
- Use bounded table resources for large datasets.
- Prebuild scatter payloads for streaming point clouds.
- Use interaction LOD and render scaling only where they improve perceived
  latency.

## Current Benchmark Notes

The current benchmark notes live in:

- `../dragon-gui-benchmark-audit.md`
- `../dragon-gui-benchmark-pie-charts.md`
- `../css-cascade-optimization.md`

As the Sphinx docs mature, these should be converted into stable performance
guide pages and separate benchmark reports.

