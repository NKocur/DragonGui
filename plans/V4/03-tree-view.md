# V4 TreeView And TreeNode

## Objective

Add a hierarchical control for file trees, scene graphs, object inspectors,
test suites, and nested configuration.

`Collapsible` is useful for panels, but it is too spacious for dense trees.

## Proposed API

```python
with dg.TreeView(on_select=lambda node_id: ...):
    with dg.TreeNode("src", node_id="src", expanded=True):
        dg.TreeNode("main.py", node_id="src/main.py", leaf=True)
```

Data-driven convenience:

```python
dg.TreeView.from_items(items, id_key="id", parent_key="parent", label_key="name")
```

## Behavior

- Expand/collapse branches.
- Select rows.
- Leaf nodes do not show a disclosure indicator.
- Keyboard navigation supports Up/Down, Left/Right, Home/End.
- Optional checkboxes can be phase 2.
- Optional lazy load callback can be phase 2.

## Native Work

- Add `WidgetKind::TreeView` and `TreeNode`.
- Maintain expanded and selected state.
- Render indentation guides, disclosure triangles, selected row background,
  and labels.
- Layout only visible descendants.
- Hit-test disclosure vs row selection separately.

## Python Work

- Add context-manager `TreeView`.
- Add context-manager or leaf `TreeNode`.
- Add `set_expanded(node_id, expanded, notify=False)`.
- Add `set_selected(node_id, notify=False)`.
- Export callback payload if needed:

```python
TreeSelection(node_id: str, label: str, path: tuple[str, ...])
```

## Acceptance

- Large trees remain responsive when collapsed.
- Selection and expansion survive rebuilds with stable ids.
- Keyboard traversal behaves predictably.
- Tree nodes can be styled by depth, selected state, and hover state.

