from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

from .runtime import AppHandle
from .vdom import VNode, diff, retain_old_ids, shallow_value_equal, widget_to_vnode
from .widgets import _BuildContext, Container, Widget, Window, _walk_widget_tree


_ACTIVE_RUNTIME_STACK: list[ComponentRuntime] = []


class StateSlot:
    """Keyed component state value returned by `ctx.state(...)`."""

    def __init__(self, runtime: ComponentRuntime, key: str) -> None:
        self._runtime = runtime
        self.key = key

    @property
    def value(self) -> object:
        return self._runtime.state[self.key]

    def set(self, value: object) -> None:
        self._runtime.set_state(self.key, value)


class ComponentCtx:
    """Render-time context passed to component functions."""

    def __init__(self, runtime: ComponentRuntime) -> None:
        self._runtime = runtime
        self._seen_state_keys: set[str] = set()

    @property
    def app(self) -> AppHandle | None:
        return self._runtime.app_handle

    def state(self, key: str, default: object) -> StateSlot:
        if not isinstance(key, str) or not key:
            raise ValueError("component state key must be a non-empty string")
        if key in self._seen_state_keys:
            raise ValueError(f"duplicate component state key: {key!r}")
        self._seen_state_keys.add(key)
        self._runtime.state.setdefault(key, default)
        return StateSlot(self._runtime, key)


class ComponentDefinition:
    def __init__(self, fn: Callable[..., Widget]) -> None:
        self.fn = fn
        self.__name__ = getattr(fn, "__name__", "component")
        self.__doc__ = getattr(fn, "__doc__", None)

    def __call__(
        self,
        *args: object,
        key: str | None = None,
        **kwargs: object,
    ) -> ComponentInstance | Widget:
        if _ACTIVE_RUNTIME_STACK:
            return _ACTIVE_RUNTIME_STACK[-1]._render_child(self, args, kwargs, key)
        return ComponentInstance(self, args=args, kwargs=kwargs, key=key)


@dataclass
class ComponentInstance:
    definition: ComponentDefinition
    args: tuple[object, ...] = ()
    kwargs: dict[str, object] = field(default_factory=dict)
    key: str | None = None
    _runtime: ComponentRuntime = field(init=False, repr=False)

    def __post_init__(self) -> None:
        if self.key is not None and (not isinstance(self.key, str) or not self.key):
            raise ValueError("component key must be a non-empty string")
        self._runtime = ComponentRuntime(self)


class ComponentRuntime:
    def __init__(self, instance: ComponentInstance) -> None:
        self.instance = instance
        self.parent: ComponentRuntime | None = None
        self.state: dict[str, object] = {}
        self.child_runtimes: dict[tuple[ComponentDefinition, str], ComponentRuntime] = {}
        self._active_child_keys: set[tuple[ComponentDefinition, str]] = set()
        self.current_widget: Widget | None = None
        self.current_vnode: VNode | None = None
        self.app_handle: AppHandle | None = None
        self._rendering = False

    def render_initial(self) -> Widget:
        return self._render_and_store()

    def attach(self, app_handle: AppHandle) -> None:
        self.app_handle = app_handle
        for child in self.child_runtimes.values():
            child.attach(app_handle)

    def detach(self) -> None:
        for child in self.child_runtimes.values():
            child.detach()
        if self.current_widget is not None:
            _unbind_widget_tree(self.current_widget)
        self.app_handle = None

    def set_state(self, key: str, value: object) -> None:
        old = self.state.get(key)
        if shallow_value_equal(old, value):
            return
        if self._rendering:
            raise RuntimeError("component state cannot be updated during render")
        old_present = key in self.state
        self.state[key] = value
        if self.app_handle is None or self.current_vnode is None:
            return

        try:
            new_widget = self._render_widget()
            new_vnode = retain_old_ids(self.current_vnode, widget_to_vnode(new_widget))
            if _sync_widget_tree_from_vnode(new_widget, new_vnode):
                new_vnode = widget_to_vnode(new_widget)
            patches = diff(self.current_vnode, new_vnode)
            self.app_handle.apply_patches(patches)
        except Exception:
            if old_present:
                self.state[key] = old
            else:
                self.state.pop(key, None)
            raise
        if self.current_widget is not None:
            self.app_handle.unregister_widget_callbacks(self.current_widget)
            _unbind_widget_tree(self.current_widget)
        _bind_widget_tree(new_widget, self.app_handle)
        self.app_handle.register_widget_callbacks(new_widget)
        self.current_widget = new_widget
        self.current_vnode = new_vnode
        _queue_startup_resources_for_patches(new_widget, patches)

    def _render_child(
        self,
        definition: ComponentDefinition,
        args: tuple[object, ...],
        kwargs: dict[str, object],
        key: str | None,
    ) -> Widget:
        if key is None:
            raise ValueError("nested component calls require an explicit key")
        if not isinstance(key, str) or not key:
            raise ValueError("component key must be a non-empty string")
        identity = (definition, key)
        self._active_child_keys.add(identity)
        child = self.child_runtimes.get(identity)
        if child is None:
            instance = ComponentInstance(definition, args=args, kwargs=dict(kwargs), key=key)
            child = instance._runtime
            child.parent = self
            if self.app_handle is not None:
                child.attach(self.app_handle)
            self.child_runtimes[identity] = child
        else:
            child.instance.args = args
            child.instance.kwargs = dict(kwargs)
            child.instance.key = key

        widget = child._render_and_store()
        parent = _BuildContext.parent()
        if parent is not None and widget.parent is None:
            parent.add(widget)
        return widget

    def _render_and_store(self) -> Widget:
        widget = self._render_widget()
        vnode = widget_to_vnode(widget)
        if self.current_vnode is not None:
            vnode = retain_old_ids(self.current_vnode, vnode)
            if _sync_widget_tree_from_vnode(widget, vnode):
                vnode = widget_to_vnode(widget)
        self.current_widget = widget
        self.current_vnode = vnode
        return widget

    def _render_widget(self) -> Widget:
        if self._rendering:
            raise RuntimeError("component render re-entered")
        old_stack = list(_BuildContext.stack)
        old_root = _BuildContext.root
        _BuildContext.stack = []
        _BuildContext.root = None
        self._active_child_keys = set()
        self._rendering = True
        _ACTIVE_RUNTIME_STACK.append(self)
        try:
            ctx = ComponentCtx(self)
            result = self.instance.definition.fn(ctx, *self.instance.args, **self.instance.kwargs)
        finally:
            _ACTIVE_RUNTIME_STACK.pop()
            self._rendering = False
            _BuildContext.stack = old_stack
            _BuildContext.root = old_root
        self._prune_inactive_children()
        if not isinstance(result, Widget):
            raise TypeError("component functions must return a DragonGUI widget")
        return result

    def _prune_inactive_children(self) -> None:
        stale = set(self.child_runtimes) - self._active_child_keys
        for key in stale:
            self.child_runtimes[key].detach()
            del self.child_runtimes[key]


def component(fn: Callable[..., Widget]) -> ComponentDefinition:
    return ComponentDefinition(fn)


def render_component_window(root: ComponentInstance) -> Window:
    widget = root._runtime.render_initial()
    if not isinstance(widget, Window):
        raise TypeError("root component passed to App.run must return a Window")
    return widget


def _sync_widget_tree_from_vnode(widget: Widget, vnode: VNode) -> bool:
    """Copy retained VNode ids back onto freshly rendered widget objects."""

    old_id = widget.id
    widget.id = vnode.id or widget.id
    changed = widget.id != old_id
    if widget.id != old_id:
        widget._sync_after_id_change(old_id)
    if not isinstance(widget, Container):
        return changed
    if len(widget.children) != len(vnode.children):
        raise RuntimeError("component widget tree and VNode tree diverged")
    for child, child_vnode in zip(widget.children, vnode.children):
        changed = _sync_widget_tree_from_vnode(child, child_vnode) or changed
    return changed


def _bind_widget_tree(widget: Widget, app_handle: AppHandle) -> None:
    for item in _walk_widget_tree(widget):
        item._bind_live(app_handle.widget_handle(item.id))


def _unbind_widget_tree(widget: Widget) -> None:
    for item in _walk_widget_tree(widget):
        item._unbind_live()


def _queue_startup_resources_for_patches(widget: Widget, patches: list[object]) -> None:
    ids = _resource_candidate_ids(patches)
    if not ids:
        return
    for item in _walk_widget_tree(widget):
        if item.id in ids:
            item._queue_startup_resources()


def _resource_candidate_ids(patches: list[object]) -> set[str]:
    from .vdom import Patch

    ids: set[str] = set()
    for patch in patches:
        if not isinstance(patch, Patch):
            continue
        if patch.kind == Patch.SET_PROP and patch.prop == "table" and patch.node_id is not None:
            ids.add(patch.node_id)
        elif patch.kind == Patch.REPLACE_NODE and patch.node is not None:
            _collect_vnode_ids(patch.node, ids)
        elif patch.kind == Patch.REPLACE_CHILDREN:
            for child in patch.children:
                _collect_vnode_ids(child, ids)
    return ids


def _collect_vnode_ids(node: VNode, out: set[str]) -> None:
    if node.id is not None:
        out.add(node.id)
    for child in node.children:
        _collect_vnode_ids(child, out)
