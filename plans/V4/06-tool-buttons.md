# V4 Tool Buttons

## Objective

Add compact action buttons for toolbars and editor surfaces:
`IconButton`, `ImageButton`, `SmallButton`, and `ArrowButton`.

## Proposed API

```python
dg.IconButton("play", tooltip="Run", on_click=run)
dg.ImageButton("assets/save.png", tooltip="Save", on_click=save)
dg.SmallButton("Reset", on_click=reset)
dg.ArrowButton("left", on_click=previous)
```

## Behavior

- Icon buttons have stable square dimensions.
- Image buttons support fit modes and disabled state.
- Arrow buttons draw native directional arrows.
- Toggle mode can be phase 2:

```python
dg.IconButton("eye", checked=True, on_toggle=...)
```

## Native Work

- Add widget kind(s) or extend Button props.
- Render icons through a built-in icon atlas, text glyphs, or named vector
  primitives. Start with a small internal icon set if no icon system exists.
- ImageButton can reuse the existing image renderer with button hit testing.
- Add keyboard activation and focus ring behavior matching Button.

## Python Work

- Add `IconButton`, `ImageButton`, `SmallButton`, and `ArrowButton`.
- Export from `dragongui`.
- Add stable icon name validation.

## Styling

CSS parts:

| Part | Meaning |
| --- | --- |
| `icon` | Icon glyph/image. |
| `image` | ImageButton image. |

## Acceptance

- Buttons do not resize based on icon name.
- Disabled/hover/active/focus states match normal Button quality.
- Tooltips work.
- Toolbar demo can use icon-only controls without text overflow.

