# Scatter3D Point Depth Ordering Fix

## Symptom

At some camera angles, points farther from the viewer could visually dominate
points closer to the viewer. This made rear points appear to take precedence
over front points in dense Scatter3D views.

## Root Cause

Scatter3D renders each point as a screen-space billboard quad. The point shader
outputs alpha so circular and Gaussian point styles can have antialiased or soft
edges.

Before the fix, the point pipeline used alpha blending and depth testing, but it
did not write point fragments into the depth buffer. That meant points were
tested against previously rendered scene depth, but one point could not occlude
another point. Later-drawn points could blend over earlier points even when they
were farther away, so visual precedence depended partly on point buffer order.

There was a second path-specific issue for streaming prepared frames. The V3
demo's static update path packed `point_instance_v1` frames with alpha `1.0`,
but the streaming path used compact `xyz_f32_v0` payloads. Native expansion of
`xyz_f32_v0` hard-coded alpha `0.85`, so streamed frames were still classified
as translucent and kept the non-depth-writing path.

## Fix

The renderer now has two point pipelines:

- An opaque point pipeline that writes depth.
- The existing translucent point pipeline that keeps depth writes disabled.

Point buffers are classified by alpha:

- If every point has alpha `>= 0.999`, the buffer is treated as opaque and uses
  the depth-writing pipeline.
- If any point is translucent, the buffer keeps the old blend path.

Render order was also adjusted:

1. Grid and user lines.
2. Opaque primary and actor point buffers, with depth writes enabled.
3. Translucent primary and actor point buffers, with depth writes disabled.
4. Mesh overlays and 2D overlays.

This keeps the default Scatter3D case correct: opaque points now occlude farther
points regardless of upload order. Transparent point clouds remain order
dependent unless a future sorted or order-independent transparency path is added.

The compact `xyz_f32_v0` decoder now expands points with alpha `1.0`, matching
the public Python default opacity. Explicit translucent point clouds still use
`point_instance_v1` and remain on the translucent path.

## Zoomed-Out Interference Follow-Up

After depth ordering was fixed, very zoomed-out dense views could still show
interference or moire-like patterns. That is a separate sampling issue: point
markers are screen-space billboards, so many world-space points can collapse
into the same few pixels while keeping the same apparent marker size.

The auto point-size path now includes a camera-distance term in addition to the
existing point-count-per-pixel density term. When the camera moves far beyond
the fitted view distance, `point_size_scale` smoothly shrinks markers down to a
smaller minimum. This reduces dense billboard overlap at far zoom levels while
preserving normal point sizes around the fitted/default view.

## Files Changed

- `native/src/scatter/mod.rs`
- `native/src/runtime.rs`

Key changes:

- Added opacity detection helpers for `PointInstance` slices and raw
  `point_instance_v1` bytes.
- Added `opaque_pipeline` to `ScatterWidget`.
- Tracked opacity state for the primary point buffer and extra point actors.
- Updated render order to draw opaque point sets before translucent point sets.
- Added tests for alpha detection.
- Updated compact `xyz_f32_v0` decode to use alpha `1.0`.
- Added a decode regression assertion so prepared stream frames do not silently
  become translucent again.
- Added zoom-aware auto point-size scaling for far camera distances.

## Verification

Focused scatter tests:

```powershell
$env:PYO3_PYTHON='C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe'
cargo test --manifest-path native\Cargo.toml scatter --lib
```

Result:

```text
57 passed
```

Manual verification:

- Ran the Scatter3D demo and rotated the plot through the problem angles.
- Confirmed closer opaque points now take visual precedence over farther points.
