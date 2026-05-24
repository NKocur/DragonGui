# V4 LoadingSpinner

## Objective

Add indeterminate activity feedback for operations where progress is unknown.
This complements `ProgressBar`.

## Proposed API

```python
dg.LoadingSpinner(size=18, label="Loading...")
```

## Behavior

- Animated spinner while visible.
- Optional label.
- Size and stroke width are configurable.
- Should not force expensive full layout rebuilds each frame.

## Native Work

- Add primitive or renderer support for rotating arc.
- Mark animation as needing redraw while visible.
- Respect reduced-motion setting later if added.

## Acceptance

- Spinner animates smoothly.
- Multiple spinners do not meaningfully affect frame time.
- Smoke demo shows inline and panel-centered spinners.

