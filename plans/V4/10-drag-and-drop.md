# V4 Drag-And-Drop API

## Objective

Add a general drag-and-drop interaction primitive for reorderable lists, file
drop zones, asset browsers, dashboards, and tree editing.

## Proposed API

```python
with dg.DragSource(payload={"kind": "file", "path": path}):
    dg.Selectable(path.name)

with dg.DropTarget(accept="file", on_drop=lambda payload: ...):
    dg.Panel("Drop files here")
```

Convenience:

```python
dg.DropZone("Drop CSV files", accept=[".csv"], on_drop=...)
```

## Behavior

- Drag starts after movement threshold.
- Drop target highlights on hover.
- Payload is app-local initially.
- OS file-drop support can be phase 2.
- Reorderable list can be built on top.

## Native Work

- Track active drag payload, source id, cursor position, and hovered target.
- Add hit testing for drop targets.
- Add visual overlay/drop hint primitives.
- Emit drop callback to Python when released over accepted target.

## Python Work

- Add `DragSource`, `DropTarget`, and `DropZone`.
- Define payload serialization rules.
- Add `DragDropPayload` callback dataclass.

## Acceptance

- Drag source can drop on compatible target.
- Incompatible targets do not highlight or receive callback.
- Drag cancel works when released outside targets.
- Reorderable list demo can be implemented with the API.

