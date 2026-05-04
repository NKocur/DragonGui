# V3 Agent Help And Guidance

AI coding agents can use DragonGUI more effectively when the library exposes a
small, current, versioned guide from the package itself. This is especially
valuable because DragonGUI has local rules that differ from PyQt, Tkinter,
browser CSS, and React-style UI systems.

This plan defines a built-in help surface that returns concise instructions for
humans and AI agents without requiring access to the repository docs.

## Problem

Agents often infer APIs and layout behavior from other GUI frameworks. That can
produce plausible but wrong code:

- inventing widget names or live setters
- using CSS patterns that do not match DragonGUI layout behavior
- missing library-specific flags such as `Scatter3D.set_points(..., fit=True)`
- mutating widgets from background threads instead of using safe enqueue paths
- forgetting widget CSS parts such as `LED::dot` or `ScrollArea::scrollbar-thumb`
- repeating known pitfalls that have already been fixed or documented

Normal docs help when the agent reads them, but many agent workflows start from
an installed package or a narrow file context. A package-level guide gives the
agent a reliable first stop.

## Goals

- Provide compact, copyable guidance through the Python package.
- Keep instructions versioned with the installed library.
- Make common DragonGUI patterns easy for agents to discover.
- Reduce hallucinated APIs by listing exact constructors, live methods, and
  preferred layout patterns.
- Include "pitfalls" that prevent repeated mistakes.
- Support both human-readable Markdown and machine-readable topic metadata.

## Non-Goals

- Replacing full documentation.
- Dumping every doc page through one huge string.
- Building an interactive chatbot inside DragonGUI.
- Adding network access or external documentation lookup.
- Making the guide a runtime dependency for normal apps.

## Proposed Public API

Primary API:

```python
dg.help()
dg.help("layout")
dg.help("scatter3d")
dg.help("css")
dg.help("live-updates")
dg.help("background-work")
dg.help("widgets")
dg.help("examples")
dg.help("pitfalls")
```

Recommended alias:

```python
dg.agent_help()
```

Reasoning:

- `dg.help()` is easy to guess and matches the user's mental model.
- `dg.agent_help()` is explicit and avoids confusion with Python's built-in
  `help(...)`.
- Both can call the same implementation.

Optional output modes:

```python
dg.help("scatter3d", format="markdown")
dg.help("scatter3d", format="text")
dg.help("scatter3d", format="json")
dg.help(format="index")
```

Return type:

- `format="markdown"` and `format="text"` return `str`.
- `format="json"` returns a `dict` containing topic, version, summary,
  patterns, APIs, pitfalls, and examples.
- `format="index"` returns the available topics and one-line descriptions.

## Topic Content

Each topic should be short enough for an agent to read before editing code.

Recommended structure:

- summary
- when to use this topic
- exact APIs
- canonical snippets
- CSS selectors or widget parts, where relevant
- common pitfalls
- links or filenames for deeper docs

### Core Topic

Cover:

- `App`, `Window`, widgets, containers, and context-manager construction
- declarative startup tree versus live runtime updates
- `id`, `key`, `class_`, `style`, and `tooltip`
- when to use `Widget.set_style(...)`
- when to use `Container.replace_children(...)`

### Layout Topic

Cover:

- `VLayout`, `HLayout`, `GridLayout`, `FlowLayout`, `ScrollArea`, `Panel`
- use `GridLayout` for responsive card/dashboard layouts
- use `FlowLayout` for wrapping button/tag rows
- leave `ScrollArea` height unset when it shares a column with fixed controls
- set fixed sibling controls to non-shrinking styles when appropriate
- avoid `height: 100%` on scroll areas with siblings

### Scatter3D Topic

Cover:

- constructor and `set_points(...)`
- `fit=True` when replacing the scene with different bounds
- preserve camera for streaming updates in the same coordinate frame
- `prepare_points(...)`, `set_prepared_points(...)`, and
  `enqueue_prepared_points(...)`
- colormap, scalar bar, grid, point style, point size, hover metadata
- background streaming patterns and performance notes

### CSS Topic

Cover:

- type selectors and class selectors
- supported properties
- widget parts
- pseudo-state or state classes where implemented
- examples for `Button`, `Panel`, `ScrollArea`, `LED`, `Scatter3D`

### Live Updates Topic

Cover:

- live setters for common widgets
- native enqueue paths
- `App.call_soon_threadsafe(...)`
- when to call `set_style`, `set_prop`-backed setters, or replace children
- thread safety rules

### Background Work Topic

Cover:

- `call_soon_threadsafe` schedules UI work; it is not a worker pool
- prepare large scatter payloads off the UI thread
- use latest-frame/coalesced paths for streams
- avoid repeatedly posting expensive Python work to the UI drain

### Widgets Topic

Cover:

- compact widget catalog
- constructor signatures
- live setters
- supported CSS parts
- short snippets for common controls

### Examples Topic

Cover:

- where to find canonical examples
- which example demonstrates which pattern
- demo-specific build/copy notes for native extension work

### Pitfalls Topic

Cover:

- `ScrollArea` sibling sizing
- scatter scene replacement needs `fit=True`
- high-rate scatter streams should coalesce
- long work should not run in UI callbacks
- do not assume browser CSS parity
- do not mutate arbitrary widget objects from worker threads

## Implementation Plan

### Phase 1: Add Static Topic Data

Create a Python module:

```text
python/dragongui/agent_help.py
```

Suggested internals:

```python
TOPICS = {
    "layout": {
        "summary": "...",
        "markdown": "...",
        "apis": [...],
        "pitfalls": [...],
        "examples": [...],
    },
}
```

Keep topic text as plain strings in source for the first version. Avoid reading
files at runtime so installed wheels work without needing repository docs.

### Phase 2: Expose Public Functions

Add:

```python
def help(topic: str | None = None, *, format: str = "markdown") -> str | dict:
    ...

def agent_help(topic: str | None = None, *, format: str = "markdown") -> str | dict:
    ...
```

Export both from:

```text
python/dragongui/__init__.py
```

Validation:

- unknown topic raises `ValueError` listing available topics
- unsupported format raises `ValueError`
- topic names are case-insensitive and normalize separators

### Phase 3: Add Tests

Add Python API tests:

- `dg.help()` returns an index or overview string
- `dg.help("layout")` includes `ScrollArea`
- `dg.help("scatter3d")` includes `fit=True`
- `dg.agent_help("css", format="json")` returns a dict with expected keys
- unknown topic raises a useful `ValueError`
- all exported topic strings are non-empty

### Phase 4: Add Documentation

Update:

- `docs/library-overview.md`
- `docs/widgets-reference.md`

Document:

- the purpose of `dg.help(...)`
- recommended use by agents
- topic list
- the difference between `help` and full docs

### Phase 5: Keep It Maintained

Add a lightweight consistency test so high-value facts do not drift:

- if `ScrollArea` defaults change, update layout topic
- if `Scatter3D.set_points` signature changes, update scatter topic
- if new widgets expose CSS parts, update widgets/CSS topics

This does not need to prove every doc string is perfect; it only catches the
most expensive stale guidance.

## Example Output

```python
print(dg.help("scatter3d"))
```

Should produce something like:

```text
# DragonGUI Scatter3D

Use Scatter3D for GPU-rendered point clouds.

Replace data:
    scatter.set_points(frame, x="x", y="y", z="z")

When switching to a different coordinate frame, refit the camera:
    scatter.set_points(frame, x="x", y="y", z="z", fit=True)

For high-rate streams, prepare payloads off the UI thread and enqueue prepared
points with coalescing.
```

## Acceptance Criteria

- `dg.help()` and `dg.agent_help()` are available from the top-level package.
- Topic output is concise enough to paste into an AI coding prompt.
- JSON output is structured enough for automated tooling.
- Layout and Scatter3D topics include the current high-value pitfalls.
- Tests cover topic lookup, formats, errors, and key guidance strings.
- Docs mention the feature and recommend it for AI-assisted coding.

## Suggested First Slice

Implement only these topics first:

1. `layout`
2. `scatter3d`
3. `css`
4. `live-updates`
5. `pitfalls`

That gives agents the most useful information without creating a large manual
that becomes stale immediately.
