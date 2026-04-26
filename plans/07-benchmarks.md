# Benchmarks Plan

## Objective

Prove the differentiated claims with reproducible numbers.

Benchmarks should compare DragonGUI with Dear PyGui where the comparison is
fair and relevant. The 500k scatter benchmark should appear immediately after
M2, not at the end of the project.

## Benchmark Areas

- Startup time to first visible window.
- Static frame time for an empty window.
- Scatter rendering:
  - 100k points
  - 500k points
  - 1M points
- Scatter data replacement time.
- Static frame time for basic controls after M5.
- DataFrame table after M6:
  - 100k rows
  - 1M rows
  - 20 columns
- UI update latency from a background worker thread.

## Metrics

- Mean frame time.
- p95 frame time.
- p99 frame time.
- Time to upload data.
- Time to replace data.
- Memory usage after data load.
- CPU usage while idle.
- Python time spent per frame.

## Benchmark Harness

Proposed layout:

```text
benchmarks/
  README.md
  run_dragongui.py
  run_dearpygui.py
  datasets.py
  report.py
  results/
```

## Acceptance Criteria

- Benchmarks run from a clean checkout.
- Results include machine, OS, Python version, GPU, and driver information.
- The report clearly separates startup, upload, replacement, and steady-state
  frame time.
- Numbers are saved as JSON and summarized as Markdown.
- The 500k scatter benchmark remains part of the default benchmark suite.

## First Benchmark To Build

After M2 renders embedded scatter:

- Generate 500k float32 xyz points.
- Upload once.
- Render for 10 seconds.
- Report upload time, mean frame time, p95, and p99.

This directly tests the headline scatter claim before the full widget stack is
implemented.
