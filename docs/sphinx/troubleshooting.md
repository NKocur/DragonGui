# Troubleshooting

## `import dragongui` Finds An Old Install

When working from a checkout, use:

```powershell
$env:PYTHONPATH = "python"
```

or install the current checkout in editable/development form. Otherwise Python
may import an older `site-packages` build that lacks recent APIs.

## Native Changes Do Not Appear

Rebuild the native extension after Rust changes. Python-only reloads will not
pick up changes in `native/src`.

## Scatter3D Colormap Reverts While Streaming

This was caused by stale compact stream metadata and native retained chrome
reapplying the startup scalar-bar colormap. See
`../scatter3d-streaming-colormap-fix.md`.

## CSS Change Does Not Apply

Check selector support, widget class names, and supported parts. Start with
`../css-styling.md` and `../css-capabilities-reference.md`.

