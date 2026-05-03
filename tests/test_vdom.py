from __future__ import annotations

import json

import pytest

import dragongui as dg
from dragongui.runtime import AppHandle
from dragongui.vdom import (
    Patch,
    ResourceRef,
    VNode,
    diff,
    retain_old_ids,
    shallow_value_equal,
    widget_to_vnode,
)


def test_widget_tree_converts_to_vnode_document_shape() -> None:
    app = dg.App()
    win = dg.Window("VNode", width=640, height=480, key="root", class_="demo")

    with dg.HLayout(key="row", style={"gap": 12}):
        dg.Label("Ready", key="label")
        dg.Button("Run", key="run", style={"background": "accent"})

    vnode = widget_to_vnode(win)

    assert vnode.to_dict() == win.to_dict()
    assert win.to_vnode().to_dict() == win.to_dict()
    assert app.document(win)["window"] == vnode.to_dict()


def test_vdom_diff_emits_targeted_prop_and_style_patches() -> None:
    old = widget_to_vnode(
        dg.Button(
            "Run",
            id="run",
            key="run",
            class_="primary",
            style={"background": "surface_alt", "hover": {"background": "accent_mix_20"}},
            parent=None,
        )
    )
    new = widget_to_vnode(
        dg.Button(
            "Stop",
            id="run",
            key="run",
            class_="danger",
            style={"background": "danger", "hover": {"background": "accent_mix_20"}},
            parent=None,
        )
    )

    patches = diff(old, new)

    assert {
        (patch.kind, patch.prop) for patch in patches if patch.kind == Patch.SET_PROP
    } == {
        (Patch.SET_PROP, "class"),
        (Patch.SET_PROP, "text"),
    }
    style_patches = [patch for patch in patches if patch.kind == Patch.SET_STYLE]
    assert len(style_patches) == 1
    assert style_patches[0].style == {"background": "danger"}
    assert all(patch.kind != Patch.REPLACE_NODE for patch in patches)


def test_vdom_diff_replaces_node_when_identity_changes() -> None:
    old = widget_to_vnode(dg.Label("Old", id="same-id", key="old", parent=None))
    new = widget_to_vnode(dg.Label("New", id="same-id", key="new", parent=None))

    patches = diff(old, new)

    assert len(patches) == 1
    assert patches[0].kind == Patch.REPLACE_NODE
    assert patches[0].node is new


def test_vdom_diff_uses_keyed_child_identity_for_reorder() -> None:
    old = dg.HLayout(id="row", key="row", parent=None)
    dg.Label("A", id="a", key="a", parent=old)
    dg.Label("B", id="b", key="b", parent=old)

    new = dg.HLayout(id="row", key="row", parent=None)
    dg.Label("B", id="b", key="b", parent=new)
    dg.Label("A", id="a", key="a", parent=new)

    patches = diff(widget_to_vnode(old), widget_to_vnode(new))

    assert len(patches) == 1
    assert patches[0].kind == Patch.REPLACE_CHILDREN
    assert [child.key for child in patches[0].children] == ["b", "a"]


def test_vdom_diff_recurses_when_child_identity_is_stable() -> None:
    old = dg.HLayout(id="row", key="row", parent=None)
    dg.Label("A", id="a", key="a", parent=old)

    new = dg.HLayout(id="row", key="row", parent=None)
    dg.Label("A updated", id="a", key="a", parent=new)

    patches = diff(widget_to_vnode(old), widget_to_vnode(new))

    assert len(patches) == 1
    assert patches[0].kind == Patch.SET_PROP
    assert patches[0].node_id == "a"
    assert patches[0].prop == "text"
    assert patches[0].value == "A updated"


def test_retain_old_ids_preserves_matching_children_when_sibling_removed() -> None:
    old = VNode(
        type="h_layout",
        id="row-old",
        key="row",
        children=(
            VNode(type="button", id="a-old", key="a", props={"text": "A"}),
            VNode(type="button", id="b-old", key="b", props={"text": "B"}),
            VNode(type="button", id="c-old", key="c", props={"text": "C"}),
        ),
        container=True,
    )
    new = VNode(
        type="h_layout",
        id="row-new",
        key="row",
        children=(
            VNode(type="button", id="a-new", key="a", props={"text": "A"}),
            VNode(type="button", id="b-new", key="b", props={"text": "B"}),
        ),
        container=True,
    )

    retained = retain_old_ids(old, new)

    assert retained.id == "row-old"
    assert [child.id for child in retained.children] == ["a-old", "b-old"]
    patches = diff(old, retained)
    assert len(patches) == 1
    assert patches[0].kind == Patch.REPLACE_CHILDREN
    assert [child.id for child in patches[0].children] == ["a-old", "b-old"]


def test_pseudo_state_style_mappings_compare_structurally() -> None:
    style = {
        "background": "surface",
        "hover": {"background": "accent_mix_20"},
        "active": {"background": "accent_dark"},
    }

    assert shallow_value_equal(style, dict(style))
    assert diff(
        VNode(type="button", id="run", key="run", style=style),
        VNode(type="button", id="run", key="run", style=dict(style)),
    ) == []


def test_resource_refs_compare_by_identity_and_version_only() -> None:
    class ExplosiveEq:
        def __eq__(self, other: object) -> bool:
            raise AssertionError("deep equality should not be called")

    first = ExplosiveEq()
    second = ExplosiveEq()

    assert ResourceRef.from_value(first, version=1) == ResourceRef.from_value(first, version=1)
    assert ResourceRef.from_value(first, version=1) != ResourceRef.from_value(first, version=2)
    assert ResourceRef.from_value(first, version=1) != ResourceRef.from_value(second, version=1)
    assert shallow_value_equal(first, first)
    assert not shallow_value_equal(first, second)


def test_vdom_diff_treats_resource_like_props_as_handles() -> None:
    class ExplosiveEq:
        def __eq__(self, other: object) -> bool:
            raise AssertionError("deep equality should not be called")

    frame = ExplosiveEq()
    old = VNode(
        type="scatter_3d",
        id="scatter",
        key="scatter",
        props={"frame": ResourceRef.from_value(frame, version=1), "x": "x"},
    )
    same = VNode(
        type="scatter_3d",
        id="scatter",
        key="scatter",
        props={"frame": ResourceRef.from_value(frame, version=1), "x": "x"},
    )
    updated = VNode(
        type="scatter_3d",
        id="scatter",
        key="scatter",
        props={"frame": ResourceRef.from_value(frame, version=2), "x": "x"},
    )

    assert diff(old, same) == []
    patches = diff(old, updated)
    assert len(patches) == 1
    assert patches[0].kind == Patch.SET_PROP
    assert patches[0].prop == "scatter"
    assert patches[0].value == updated.props


def test_vdom_scatter_detects_data_only_change_via_payload_token() -> None:
    """Same frame shape but different data must produce a scatter patch."""
    import dragongui as dg
    import numpy as np
    from dragongui.vdom import diff, widget_to_vnode

    class F:
        shape = (3, 3)
        columns = ("x", "y", "z")
        dtypes = ("float32", "float32", "float32")
        def __init__(self, vals):
            self.x = np.array(vals, dtype=np.float32)
            self.y = np.array(vals, dtype=np.float32)
            self.z = np.array(vals, dtype=np.float32)
        def __getitem__(self, c): return getattr(self, c)

    s1 = dg.Scatter3D(F([1.0, 2.0, 3.0]), x="x", y="y", z="z", parent=None)
    s2 = dg.Scatter3D(F([4.0, 5.0, 6.0]), x="x", y="y", z="z", parent=None)
    # Give them the same widget id so the diff treats them as same-key
    s2.id = s1.id

    old = widget_to_vnode(s1)
    updated = widget_to_vnode(s2)

    patches = diff(old, updated)
    assert len(patches) == 1
    assert patches[0].prop == "scatter"


def test_vdom_scatter_unchanged_data_produces_no_patch() -> None:
    """Same data must produce no patch on re-render."""
    import dragongui as dg
    import numpy as np
    from dragongui.vdom import diff, widget_to_vnode

    class F:
        shape = (2, 3)
        columns = ("x", "y", "z")
        dtypes = ("float32", "float32", "float32")
        x = np.array([1.0, 2.0], dtype=np.float32)
        y = np.array([3.0, 4.0], dtype=np.float32)
        z = np.array([5.0, 6.0], dtype=np.float32)
        def __getitem__(self, c): return getattr(self, c)

    s = dg.Scatter3D(F(), x="x", y="y", z="z", parent=None)
    old = widget_to_vnode(s)
    same = widget_to_vnode(s)
    assert diff(old, same) == []


def test_widget_to_vnode_rejects_non_widgets() -> None:
    with pytest.raises(TypeError, match="DragonGUI widget"):
        widget_to_vnode(object())


def test_app_handle_applies_supported_vdom_set_prop_patches() -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []
            self.styles: list[tuple[str, str]] = []
            self.children: list[tuple[str, str]] = []
            self.nodes: list[tuple[str, str]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def enqueue_set_style(self, widget_id: str, style_json: str) -> None:
            self.styles.append((widget_id, style_json))

        def enqueue_replace_children(self, widget_id: str, children_json: str) -> None:
            self.children.append((widget_id, children_json))

        def enqueue_replace_node(self, widget_id: str, node_json: str) -> None:
            self.nodes.append((widget_id, node_json))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    handle.apply_patches(
        [
            Patch(
                kind=Patch.SET_PROP,
                path=("text_input:key=filter",),
                node_id="filter",
                prop="value",
                value="abc",
            ),
            Patch(
                kind=Patch.SET_PROP,
                path=("panel:key=content",),
                node_id="content",
                prop="class",
                value=None,
            ),
            Patch(
                kind=Patch.SET_STYLE,
                path=("button:key=run",),
                node_id="run",
                style={"background": "danger", "border_width": None},
            ),
            Patch(
                kind=Patch.REPLACE_CHILDREN,
                path=("panel:key=content",),
                node_id="content",
                children=(
                    VNode(
                        type="label",
                        id="status",
                        key="status",
                        props={"text": "Updated"},
                    ),
                ),
            ),
            Patch(
                kind=Patch.REPLACE_NODE,
                path=("label:key=status",),
                node_id="status",
                node=VNode(
                    type="button",
                    id="status-button",
                    key="status",
                    props={"text": "Run"},
                ),
            ),
        ]
    )

    assert sender.props == [("filter", "value", "abc"), ("content", "class", None)]
    assert sender.styles == [("run", '{"background":"danger","border_width":null}')]
    assert json.loads(sender.styles[0][1]) == {"background": "danger", "border_width": None}
    assert sender.children == [
        (
            "content",
            '[{"id":"status","key":"status","props":{"text":"Updated"},"type":"label"}]',
        )
    ]
    assert json.loads(sender.children[0][1])[0]["props"]["text"] == "Updated"
    assert sender.nodes == [
        (
            "status",
            '{"id":"status-button","key":"status","props":{"text":"Run"},"type":"button"}',
        )
    ]
    assert json.loads(sender.nodes[0][1])["type"] == "button"


def test_app_handle_rejects_unsupported_vdom_patches_explicitly() -> None:
    handle = AppHandle()

    with pytest.raises(ValueError, match="node_id"):
        handle.apply_patch(
            Patch(
                kind=Patch.SET_STYLE,
                path=("button:key=run",),
                style={"background": "danger"},
            )
        )

    with pytest.raises(ValueError, match="node_id"):
        handle.apply_patch(
            Patch(
                kind=Patch.REPLACE_CHILDREN,
                path=("h_layout:key=row",),
                children=(),
            )
        )

    with pytest.raises(ValueError, match="node_id and node"):
        handle.apply_patch(
            Patch(
                kind=Patch.REPLACE_NODE,
                path=("h_layout:key=row",),
                node=VNode(type="v_layout", id="row", key="row"),
            )
        )

    with pytest.raises(ValueError, match="node_id"):
        handle.apply_patch(
            Patch(
                kind=Patch.SET_PROP,
                path=("text_input:key=filter",),
                prop="value",
                value="abc",
            )
        )

    with pytest.raises(TypeError, match="VDOM Patch"):
        handle.apply_patch(object())
