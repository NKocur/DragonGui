# V4 ToggleSwitch

## Objective

Add a binary setting control with switch styling. This complements `Checkbox`
for settings panels where on/off state should be visually prominent.

## Proposed API

```python
dg.ToggleSwitch("Live updates", checked=True, on_change=lambda checked: ...)
```

## Behavior

- Pointer click toggles state.
- Space toggles when focused.
- Disabled state prevents toggling.
- Optional label may appear left or right.

## Native Work

- Add widget kind or extend Checkbox style.
- Render track and thumb primitives.
- Animate thumb position if transition system can support it.

## Acceptance

- State, callback, disabled, focus, hover, and active behavior work.
- CSS can style `track` and `thumb` parts.
- Does not replace Checkbox; docs explain intended usage.

