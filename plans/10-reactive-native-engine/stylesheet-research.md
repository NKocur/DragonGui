# Stylesheet Selector Research

## Scope

This is R7 research only. No selector or cascade code is part of the DragonGUI
runtime yet.

The prototype in `selector_prototype.py` supports a deliberately small selector
subset:

- type selectors: `button`
- class selectors: `.primary`
- id selectors: `#run`
- pseudo-state selectors: `:hover`, `:active`, `:focus`, `:disabled`
- combined selectors: `button.primary:hover`

It does not support descendant selectors, child selectors, attributes, comma
groups, media queries, inheritance, or cascade layers.

## Findings

The simple parser/matcher is easy to build, but selector matching alone is not
the hard part. The runtime cost and complexity come from:

- maintaining computed style for every retained node
- invalidating style when classes, ids, pseudo-state, or theme tokens change
- resolving specificity and source order consistently
- keeping selector matching out of the render hot path
- explaining interactions between inline `style={...}` and stylesheet rules

Even with a small selector subset, the renderer needs a computed-style cache and
dirty reason tracking before stylesheets are worth merging.

## Recommendation

Keep structured inline style maps as the primary V1 API.

Add stylesheet support later only as a convenience layer that compiles into the
same `NodeStyle` model. The minimum safe design is:

1. Parse stylesheets into rules outside the frame loop.
2. Match rules only when the tree structure, `class_`, id, or pseudo-state
   changes.
3. Store computed style on retained nodes.
4. Let inline style override stylesheet rules.
5. Record style dirty reasons in `debug_snapshot()`.

Until that computed-style cache exists, selector support should stay out of the
main runtime.

## Prototype

Run:

```powershell
python plans\10-reactive-native-engine\selector_prototype.py
```

The benchmark is intentionally simple. Its job is not to prove a final
implementation is fast; it proves that selector parsing and matching can be
kept isolated while the cache/invalidation design is worked out.
