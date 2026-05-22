# CSS Cascade Optimization

## Problem

The framework-wide benchmark audit showed CSS cascade as one of the highest
per-widget CPU costs. In the synthetic cascade benchmark, DragonGUI applied a
mixed stylesheet over 2,000 widgets using type, class, part, and direct-child
selectors.

Initial result:

```text
css cascade many widgets: 15,148 ns/widget
```

The hot path was `apply_stylesheets_to_node` in `native/src/css_style.rs`.
For every widget, the cascade walked every stylesheet rule and then ran the
full selector matcher to determine whether the rule applied.

## Root Cause

The cascade did avoid incorrect matches, but it did too much work before
rejecting obvious misses.

Examples:

- A `Button.primary` rule was still sent through the general selector matcher
  for `Label`, `Slider`, and `DataFrameTable` widgets.
- A `.dense` rule was checked against widgets without that class through the
  full selector path.
- Attribute and sibling/child snapshots were built for every node even when the
  active stylesheet did not contain attribute selectors, structural selectors,
  or `:has(...)`.

That made common CSS, such as type and class selectors, pay costs intended for
more complex selectors.

## Fix

Three fast paths were added.

### 1. Rule Target Filtering

Each `DgStyleRule` now precomputes a `DgSelectorTargetFilter` from the selector's
target compound selector.

The filter stores:

- target widget kind
- target id
- target key
- required classes

Before running full selector matching, cascade now checks:

```rust
rule.target_may_match(&element)
```

If the widget kind, id, key, or classes cannot match, the rule is skipped
immediately.

This preserves the full selector engine for selectors that survive the cheap
filter, including descendant chains, child selectors, pseudo states, parts,
attributes, and functions.

### 2. Snapshot Feature Gating

Cascade now scans the active rules once and records which expensive selector
features are actually needed:

- attribute selectors
- sibling/child snapshots for structural selectors and `:has(...)`

If the stylesheet does not use those features, cascade avoids building those
snapshots while walking the widget tree.

This keeps simple stylesheets on a cheaper path while preserving support for
complex selectors when they are present.

### 3. Compiled Rule Buckets

`ParsedStylesheet` now builds runtime rule buckets after parsing. The parser is
unchanged; parsed rules are lowered into DragonGUI-owned `DgStyleRule` values
and then indexed by the target selector's most selective cheap key:

- id
- key
- first required class
- widget kind
- universal fallback

For stylesheets above the small-rule threshold, cascade asks the buckets for
candidate rules for the current widget and then still runs the normal
`target_may_match` and full selector matcher. Small stylesheets keep the linear
scan path because benchmarking showed bucket setup/lookup overhead was not worth
paying for the common small mixed stylesheet.

## Files Changed

- `native/src/css_style.rs`
- `docs/dragon-gui-benchmark-audit.md`

Key implementation changes:

- Added `DgSelectorTargetFilter`.
- Added `DgStyleRule::target_may_match`.
- Added `StylesheetRuleBuckets` beside parsed rules.
- Added selector feature detection helpers:
  - `contains_attribute_selector`
  - `requires_sibling_snapshots`
- Added `StylesheetMatchFeatures`.
- Routed cascade and matched-rule label traversal through bucketed candidate
  collection when stylesheets are large enough.
- Passed match features through cascade and matched-rule label traversal.
- Avoided attribute and sibling snapshot construction unless required.
- Updated the benchmark audit with the new CSS cascade result.

## Benchmark Result

Command:

```powershell
$env:PYO3_PYTHON='C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe'
cargo test --release --manifest-path native\Cargo.toml bench_css_cascade_many_widgets --lib -- --ignored --nocapture
```

Before:

```text
15,148 ns/widget
```

After:

```text
10,874 ns/widget
```

That is about a 28% reduction for the benchmarked mixed-selector case.

## Follow-Up Benchmark Split

The original benchmark cloned the tree on each iteration, so a follow-up split
was added to separate allocation cost from cascade cost:

```powershell
cargo test --release --manifest-path native\Cargo.toml bench_css_clone_many_widgets --lib -- --ignored --nocapture
cargo test --release --manifest-path native\Cargo.toml bench_css_cascade_many_widgets --lib -- --ignored --nocapture
```

Results after the first optimization pass:

```text
clone only:       6,389 ns/widget
pure cascade:     4,004 ns/widget
clone + cascade: 10,976 ns/widget
```

That changes the interpretation of the benchmark: the original
`bench_css_cascade_many_widgets` number is mostly tree clone/allocation plus
roughly `4 us/widget` of actual cascade work.

An experimental repeated-widget computed-style cache was also tested. It was not
kept because cache-key construction and `NodeStyle` cloning outweighed selector
reuse:

```text
pure cascade with cache:     4,974 ns/widget
clone + cascade with cache: 14,076 ns/widget
```

The useful result from that experiment is negative: broad computed-style caching
is not worth adding in this shape.

Two smaller pure-cascade experiments were also tested and removed:

- lazy construction of ancestor selector views only for nodes that might match
  ancestor-dependent selectors
- pre-sizing matched declaration vectors and skipping sort for one-item lists

Both regressed the benchmark on this workload. The likely reason is that their
extra branch/rule-scan or allocation behavior outweighed the small work they
avoided.

## Compiled Bucket Follow-Up

A first compiled-style layer was added without replacing the CSS parser. The
parsed rules now keep precomputed rule buckets, and cascade uses those buckets
only for larger stylesheets.

Small mixed stylesheet benchmark after adding the guarded bucket path:

```text
pure cascade:     3,803 ns/widget
clone + cascade: 10,602 ns/widget
```

The pure cascade result is effectively flat with the previous `4,004 ns/widget`
measurement, and slightly faster after reusing cascade scratch buffers.

Large stylesheet benchmark with 400 extra non-matching rules:

```text
linear scan:       6,641 ns/widget
bucketed cascade: 3,233 ns/widget
```

That is about a 50% reduction for a larger stylesheet where skipping irrelevant
rules outweighs the candidate bucket lookup cost. Reusing the matched-declaration
and candidate-rule scratch buffers shaved the bucketed path from `3,503
ns/widget` to `3,233 ns/widget` in the measured large-rule benchmark.

## Ancestor Snapshot Gating Follow-Up

Cascade now also detects whether any active selector needs ancestor matching.
For stylesheets without child, descendant, or ancestor-dependent selector
functions, runtime style application skips:

- ancestor snapshot pushes
- ancestor class view construction
- `StyleAncestor` view construction

This does not change behavior for stylesheets with selectors such as
`Panel.content > Button`; those still use the full ancestor path.

Simple no-ancestor stylesheet benchmark:

```text
forced ancestor path: 3,347 ns/widget
gated ancestor path:  2,534 ns/widget
```

That is about a 24% reduction for simple target-only stylesheets.

## Verification

Focused CSS tests:

```powershell
cargo test --manifest-path native\Cargo.toml css_style::tests:: --lib
```

Full native test suite:

```powershell
cargo test --manifest-path native\Cargo.toml --lib
```

Result:

```text
484 passed; 11 ignored
```

## Remaining Work

The cascade is still worth optimizing, but the next pass should target
declaration/property work directly. Good candidates are property masks and
selector shape-specific fast paths so matching simple type/class/id selectors
does less generic selector-engine work.
