# DragonGUI Benchmark Pie Charts

Date: 2026-05-22

These charts group only comparable timings together. They should not be read as
a single global frame-time breakdown because the benchmark units differ by area.

## Per-Widget CPU Prep

This compares normalized per-item native microbenchmarks. The CSS slice uses the
optimized cascade result.

```mermaid
pie showData
    title Per-widget CPU prep after CSS cascade optimization
    "CSS cascade: 10,874 ns/widget" : 10874
    "Layout: 2,908 ns/widget" : 2908
    "Label text collection: 963 ns/label" : 963
    "ProgressBar primitive emit: 547 ns/bar" : 547
```

Before the CSS cascade optimization:

```mermaid
pie showData
    title Per-widget CPU prep before CSS cascade optimization
    "CSS cascade: 15,148 ns/widget" : 15148
    "Layout: 2,908 ns/widget" : 2908
    "Label text collection: 963 ns/label" : 963
    "ProgressBar primitive emit: 547 ns/bar" : 547
```

CSS cascade changed from `15,148 ns/widget` to `10,874 ns/widget`, reducing the
CSS slice from about `77%` of this comparison group to about `71%`.

## DataFrameTable Frame Work

This compares the table-specific visible-cell work from the native
microbenchmarks.

```mermaid
pie showData
    title DataFrameTable visible-cell frame work
    "Table text collect: 138.8 us/frame" : 138.8
    "Table primitive emit: 0.862 us/frame" : 0.862
```

The table is overwhelmingly text-bound in this benchmark. Cell chrome is already
cheap relative to visible-cell text collection.

## Scatter3D Frame Replacement

This compares the measured pieces of a 500k-point packed `xyz_f32_v0` update.

```mermaid
pie showData
    title Scatter3D 500k-point update
    "Native decode: 12.09 ms" : 12.09
    "Native upload: 3.34 ms" : 3.34
    "Python pack: 1.75 ms" : 1.75
```

Scatter3D frame replacement is decode-bound in this benchmark. Upload is the
second-largest measured piece.

After the bounds-hinted decode fast path, the isolated 500k-point decode
microbenchmark measured `4.938 ms` for decode. Keeping the original upload and
Python pack timings for scale:

```mermaid
pie showData
    title Scatter3D 500k-point update with bounds-hinted decode
    "Native decode: 4.94 ms" : 4.938
    "Native upload: 3.34 ms" : 3.34
    "Python pack: 1.75 ms" : 1.75
```

After the compact GPU path, the same windowed 500k-point benchmark measured
native decode as zero and reduced upload/native update substantially:

```mermaid
pie showData
    title Scatter3D 500k-point update with compact GPU path
    "Python pack: 2.69 ms" : 2.69
    "Native upload: 0.88 ms" : 0.88
    "Native decode: 0.00 ms" : 0.001
```

## Takeaways

- CSS cascade is still a major per-widget CPU cost, even after the first
  optimization pass.
- DataFrameTable optimization should focus on text collection and text buffer
  reuse before primitive emission.
- Scatter3D default xyz streaming is no longer native-decode-bound when bounds
  are provided. The next likely win is reducing Python packing/copy cost.
