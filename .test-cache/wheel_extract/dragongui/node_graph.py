from __future__ import annotations

from collections.abc import Callable, Mapping, MutableMapping, Sequence
from dataclasses import dataclass
import json
import math
import os
import re
import shlex
import shutil
import subprocess
import time
from typing import Any

from .agent_messages import AgentEnvelopeParser, AgentMessage
from .widgets import Container, HtmlReport, Widget, _AUTO_PARENT, _walk_widget_tree


_GRAPH_SCHEMA_VERSION = 1
_GRAPH_MUTATION_EVENTS = {
    "node_moved",
    "node_created",
    "node_duplicated",
    "node_deleted",
    "node_updated",
    "edge_created",
    "edge_deleted",
    "edge_waypoints_changed",
    "section_created",
    "section_updated",
    "section_moved",
    "section_resized",
    "section_deleted",
}
_NODE_GRAPH_RUNTIME_SCHEMA_VERSION = 1
_NODE_GRAPH_RUNTIME_POLICIES = {"auto", "ephemeral", "persistent", "manual"}
_PERSISTENT_RUNTIME_OBJECT_TYPES = {"terminal_session", "agent_session", "subprocess", "watcher", "file_watcher"}

_NODE_GRAPH_PORT_TYPE_CONVERSIONS: dict[tuple[str, str], str] = {
    ("text", "terminal_input"): "text_to_terminal_input",
}

_NODE_GRAPH_WIDGET_SINK_PORT_PROFILES: tuple[str, ...] = ("text", "terminal_output", "message", "json", "artifact", "status", "error")
_NODE_GRAPH_WIDGET_SOURCE_PORT_PROFILES: tuple[str, ...] = ("text", "json", "status", "message", "artifact")
_NODE_GRAPH_AUTO_BINDING_DATA_KEY = "auto_discovered_binding_target"
_NODE_GRAPH_AUTO_BINDING_WIDGET_KINDS = {
    "log_view",
    "text_input",
    "text_area",
    "code_editor",
    "label",
    "badge",
    "tag",
    "led",
}

_NODE_GRAPH_PORT_TYPE_COLORS: dict[str, str] = {
    "terminal_input": "#4fd6be",
    "terminal_output": "#43c6ac",
    "terminal_error": "#f7768e",
    "stream:text": "#73daca",
    "text": "#7aa2f7",
    "text:list": "#89b4fa",
    "message": "#9ece6a",
    "json": "#bb9af7",
    "bool": "#e0af68",
    "number": "#ff9e64",
    "error": "#f7768e",
    "event": "#e0af68",
    "status": "#f8c14a",
    "control": "#c0caf5",
    "approval_request": "#e0af68",
    "approval_result": "#e0af68",
    "test_request": "#bb9af7",
    "test_report": "#bb9af7",
    "artifact": "#f7768e",
    "file:path": "#ff9e64",
}


@dataclass(frozen=True, slots=True)
class NodeGraphRuntimeEvent:
    """Schema-versioned runtime event emitted by a live node graph session."""

    event: str
    node_id: str | None = None
    port_id: str | None = None
    section_id: str | None = None
    object_id: str | None = None
    value: object | None = None
    data: dict[str, object] | None = None
    sequence: int | None = None
    timestamp: float | None = None
    schema_version: int = _NODE_GRAPH_RUNTIME_SCHEMA_VERSION

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "schema_version": self.schema_version,
            "event": self.event,
            "timestamp": time.time() if self.timestamp is None else self.timestamp,
        }
        if self.sequence is not None:
            payload["sequence"] = self.sequence
        if self.node_id is not None:
            payload["node_id"] = self.node_id
        if self.port_id is not None:
            payload["port_id"] = self.port_id
        if self.section_id is not None:
            payload["section_id"] = self.section_id
        if self.object_id is not None:
            payload["object_id"] = self.object_id
        if self.value is not None:
            payload["value"] = _json_safe_value(self.value)
        if self.data is not None:
            payload["data"] = _json_copy(self.data, "runtime event data")
        return payload


@dataclass(slots=True)
class NodeGraphRuntimeHandle:
    """Live runtime object state owned by a node graph runtime session."""

    object_id: str
    object_type: str
    owner_node_id: str | None = None
    status: str = "created"
    config: dict[str, object] | None = None
    handle: Any | None = None
    error: str | None = None
    created_at: float | None = None
    updated_at: float | None = None

    def __post_init__(self) -> None:
        now = time.time()
        self.object_id = str(self.object_id)
        self.object_type = str(self.object_type)
        self.owner_node_id = None if self.owner_node_id is None else str(self.owner_node_id)
        self.status = str(self.status)
        self.config = _mapping_copy(self.config, "runtime handle config")
        self.error = None if self.error is None else str(self.error)
        if self.created_at is None:
            self.created_at = now
        if self.updated_at is None:
            self.updated_at = self.created_at

    def set_status(self, status: str, error: object | None = None) -> None:
        self.status = str(status)
        self.error = None if error is None else str(error)
        self.updated_at = time.time()

    def attach(self, handle: Any, *, status: str = "attached") -> None:
        self.handle = handle
        self.set_status(status)

    def detach(self, *, status: str = "detached") -> Any | None:
        handle = self.handle
        self.handle = None
        self.set_status(status)
        return handle

    @property
    def handle_attached(self) -> bool:
        return self.handle is not None

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "object_id": self.object_id,
            "object_type": self.object_type,
            "owner_node_id": self.owner_node_id,
            "status": self.status,
            "handle_attached": self.handle_attached,
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        }
        if self.config is not None:
            payload["config"] = _json_copy(self.config, "runtime handle config")
        if self.error is not None:
            payload["error"] = self.error
        return payload


@dataclass(frozen=True, slots=True)
class NodeGraphRuntimeObject:
    """Registry record for a graph-owned runtime object."""

    object_id: str
    object_type: str
    owner_node_id: str | None = None
    status: str | None = None
    config: dict[str, object] | None = None

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "object_id": self.object_id,
            "object_type": self.object_type,
            "owner_node_id": self.owner_node_id,
            "status": self.status,
        }
        if self.config is not None:
            payload["config"] = _json_copy(self.config, "runtime object config")
        return payload


@dataclass(frozen=True, slots=True)
class NodeGraphRuntimeObjectRef:
    """Reference from a graph node to a named runtime object."""

    node_id: str
    object_id: str
    object_type: str | None = None
    key: str | None = None

    def to_dict(self) -> dict[str, object]:
        return {
            "node_id": self.node_id,
            "object_id": self.object_id,
            "object_type": self.object_type,
            "key": self.key,
        }


class NodeGraphObjectRegistry:
    """In-memory registry of named runtime objects used by a node graph."""

    def __init__(self, objects: Sequence[NodeGraphRuntimeObject | Mapping[str, object]] = ()) -> None:
        self._objects: dict[str, NodeGraphRuntimeObject] = {}
        for value in objects:
            if isinstance(value, NodeGraphRuntimeObject):
                obj = value
            elif isinstance(value, Mapping):
                obj = NodeGraphRuntimeObject(
                    object_id=_required_text(value.get("object_id", value.get("id")), "object_id"),
                    object_type=_required_text(value.get("object_type", value.get("type")), "object_type"),
                    owner_node_id=None if value.get("owner_node_id") is None else str(value.get("owner_node_id")),
                    status=None if value.get("status") is None else str(value.get("status")),
                    config=_mapping_copy(value.get("config"), "runtime object config"),
                )
            else:
                raise TypeError("registry objects must be runtime object records or mappings")
            self.register(obj)

    def register(
        self,
        obj: NodeGraphRuntimeObject | None = None,
        *,
        object_id: str | None = None,
        object_type: str | None = None,
        owner_node_id: str | None = None,
        status: str | None = None,
        config: Mapping[str, object] | None = None,
    ) -> NodeGraphRuntimeObject:
        if obj is None:
            obj = NodeGraphRuntimeObject(
                object_id=_required_text(object_id, "object_id"),
                object_type=_required_text(object_type, "object_type"),
                owner_node_id=None if owner_node_id is None else str(owner_node_id),
                status=None if status is None else str(status),
                config=_mapping_copy(config, "runtime object config"),
            )
        if obj.object_id in self._objects:
            raise ValueError(f"duplicate runtime object id {obj.object_id!r}")
        self._objects[obj.object_id] = obj
        return obj

    def object_ref(self, object_id: str) -> NodeGraphRuntimeObject | None:
        return self._objects.get(str(object_id))

    def objects_for_type(self, object_type: str) -> tuple[NodeGraphRuntimeObject, ...]:
        normalized = str(object_type)
        return tuple(obj for obj in self._objects.values() if obj.object_type == normalized)

    def missing_object_refs(self, refs: Sequence[NodeGraphRuntimeObjectRef | Mapping[str, object]]) -> tuple[NodeGraphRuntimeObjectRef, ...]:
        missing: list[NodeGraphRuntimeObjectRef] = []
        for value in refs:
            ref = _missing_ref_from_value(value)
            obj = self.object_ref(ref.object_id)
            if obj is None or (ref.object_type is not None and obj.object_type != ref.object_type):
                missing.append(ref)
        return tuple(missing)

    def to_list(self) -> list[dict[str, object]]:
        return [obj.to_dict() for obj in self._objects.values()]

    def __contains__(self, object_id: object) -> bool:
        return str(object_id) in self._objects

    def __iter__(self):
        return iter(self._objects.values())

    def __len__(self) -> int:
        return len(self._objects)


@dataclass(frozen=True, slots=True)
class NodeGraphNodeBinding:
    """Runtime-facing description of one graph node."""

    node_id: str
    node_type: str
    title: str
    status: str | None = None
    config: dict[str, object] | None = None
    owned_object_id: str | None = None
    object_refs: tuple[NodeGraphRuntimeObjectRef, ...] = ()

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "node_id": self.node_id,
            "node_type": self.node_type,
            "title": self.title,
            "status": self.status,
            "owned_object_id": self.owned_object_id,
            "object_refs": [ref.to_dict() for ref in self.object_refs],
        }
        if self.config is not None:
            payload["config"] = _json_copy(self.config, "node binding config")
        return payload


@dataclass(frozen=True, slots=True)
class NodeGraphSectionBinding:
    """Runtime-facing description of one graph section."""

    section_id: str
    title: str
    node_ids: tuple[str, ...]
    purpose: str | None = None
    trigger: str | None = None
    config: dict[str, object] | None = None

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "section_id": self.section_id,
            "title": self.title,
            "node_ids": list(self.node_ids),
            "purpose": self.purpose,
            "trigger": self.trigger,
        }
        if self.config is not None:
            payload["config"] = _json_copy(self.config, "section binding config")
        return payload


@dataclass(frozen=True, slots=True)
class NodeGraphRuntimeEdgeBinding:
    """Runtime-facing directed connection between two graph ports."""

    edge_id: str
    source_node: str
    source_port: str
    target_node: str
    target_port: str
    label: str | None = None
    conversion: str | None = None
    config: dict[str, object] | None = None

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "edge_id": self.edge_id,
            "source": {"node": self.source_node, "port": self.source_port},
            "target": {"node": self.target_node, "port": self.target_port},
            "label": self.label,
            "conversion": self.conversion,
        }
        if self.config is not None:
            payload["config"] = _json_copy(self.config, "runtime edge config")
        return payload


@dataclass(frozen=True, slots=True)
class NodeGraphRuntimeViewBinding:
    """Runtime-facing view binding for an observable graph node."""

    node_id: str
    view_type: str
    object_id: str | None = None
    object_type: str | None = None
    title: str | None = None
    config: dict[str, object] | None = None
    available: bool = False
    reason: str | None = None

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "node_id": self.node_id,
            "view_type": self.view_type,
            "object_id": self.object_id,
            "object_type": self.object_type,
            "title": self.title,
            "available": self.available,
        }
        if self.config is not None:
            payload["config"] = _json_copy(self.config, "runtime view config")
        if self.reason is not None:
            payload["reason"] = self.reason
        return payload


@dataclass(frozen=True, slots=True)
class NodeGraphRuntimeBinding:
    """Static binding plan that maps graph structure to runtime objects."""

    nodes: tuple[NodeGraphNodeBinding, ...]
    sections: tuple[NodeGraphSectionBinding, ...]
    edges: tuple[NodeGraphRuntimeEdgeBinding, ...]
    registry: NodeGraphObjectRegistry
    missing_refs: tuple[NodeGraphRuntimeObjectRef, ...] = ()

    @property
    def valid(self) -> bool:
        return not self.missing_refs

    def node_binding(self, node_id: str) -> NodeGraphNodeBinding | None:
        normalized = str(node_id)
        return next((binding for binding in self.nodes if binding.node_id == normalized), None)

    def section_binding(self, section_id: str) -> NodeGraphSectionBinding | None:
        normalized = str(section_id)
        return next((binding for binding in self.sections if binding.section_id == normalized), None)

    def validate(self) -> dict[str, object]:
        return {
            "valid": self.valid,
            "missing_refs": [ref.to_dict() for ref in self.missing_refs],
        }

    def to_dict(self) -> dict[str, object]:
        return {
            "valid": self.valid,
            "nodes": [binding.to_dict() for binding in self.nodes],
            "sections": [binding.to_dict() for binding in self.sections],
            "edges": [binding.to_dict() for binding in self.edges],
            "registry": self.registry.to_list(),
            "missing_refs": [ref.to_dict() for ref in self.missing_refs],
        }


class NodeGraphRuntimeSession:
    """Live runtime session state for a graph binding.

    A saved graph remains a template. This session owns the transient runtime
    handles, statuses, and event log that should not be persisted into graph
    layout data.
    """

    def __init__(
        self,
        binding: NodeGraphRuntimeBinding,
        *,
        session_id: str | None = None,
        status: str = "created",
    ) -> None:
        self.binding = binding
        self.session_id = str(session_id) if session_id is not None else f"graph-runtime-{int(time.time() * 1000)}"
        self.status = str(status)
        self._sequence = 0
        self._events: list[NodeGraphRuntimeEvent] = []
        self._handles: dict[str, NodeGraphRuntimeHandle] = {}
        self._port_values: dict[tuple[str, str], list[object]] = {}
        self._widgets: dict[str, Any] = {}
        self._terminal_event_listener_removers: dict[str, Callable[[], None]] = {}
        self._parser_state: dict[str, AgentEnvelopeParser] = {}
        self._execution_depth = 0
        for obj in binding.registry:
            handle = NodeGraphRuntimeHandle(
                object_id=obj.object_id,
                object_type=obj.object_type,
                owner_node_id=obj.owner_node_id,
                status=obj.status or "declared",
                config=obj.config,
            )
            self._handles[handle.object_id] = handle
        self.emit_event("session_created", data={"valid": binding.valid})

    @classmethod
    def from_graph(
        cls,
        graph: "NodeGraph",
        *,
        registry: NodeGraphObjectRegistry | None = None,
        session_id: str | None = None,
    ) -> "NodeGraphRuntimeSession":
        return cls(graph.runtime_binding(registry), session_id=session_id)

    @property
    def valid(self) -> bool:
        return self.binding.valid

    @property
    def events(self) -> tuple[NodeGraphRuntimeEvent, ...]:
        return tuple(self._events)

    @property
    def handles(self) -> tuple[NodeGraphRuntimeHandle, ...]:
        return tuple(self._handles.values())

    def port_values(self, node_id: str, port_id: str) -> list[object]:
        """Return runtime values delivered to or emitted from a graph port."""

        return list(self._port_values.get((str(node_id), str(port_id)), []))

    def register_widget(self, widget_id: str | None, widget: Any | None = None) -> Any:
        """Register a transient GUI widget handle for runtime widget sink nodes."""

        if widget is None:
            widget = widget_id
            widget_id = getattr(widget, "id", None)
        text = "" if widget_id is None else str(widget_id).strip()
        if not text:
            raise ValueError("widget_id is required")
        if widget is None:
            raise ValueError("widget is required")
        self._widgets[text] = widget
        self.emit_event("widget_registered", data={"widget_id": text, "widget_type": _widget_kind(widget)})
        return widget

    def unregister_widget(self, widget_id: str) -> Any | None:
        """Remove and return a registered runtime widget handle, if present."""

        text = str(widget_id).strip()
        widget = self._widgets.pop(text, None)
        if widget is not None:
            self.emit_event("widget_unregistered", data={"widget_id": text, "widget_type": _widget_kind(widget)})
        return widget

    def widget_handle(self, widget_id: str) -> Any | None:
        """Return a registered runtime widget handle by stable widget ID."""

        return self._widgets.get(str(widget_id).strip())

    def widget_ids(self) -> tuple[str, ...]:
        """Return registered runtime widget IDs."""

        return tuple(self._widgets)

    def view_binding(self, node_id: str) -> NodeGraphRuntimeViewBinding | None:
        """Return the runtime view binding for a graph node, when one is known."""

        binding = self.binding.node_binding(node_id)
        if binding is None:
            return None
        return self._view_binding_for_node(binding)

    def view_bindings(self) -> tuple[NodeGraphRuntimeViewBinding, ...]:
        """Return runtime view bindings for all nodes with observable views."""

        bindings: list[NodeGraphRuntimeViewBinding] = []
        for binding in self.binding.nodes:
            view = self._view_binding_for_node(binding)
            if view is not None:
                bindings.append(view)
        return tuple(bindings)

    def _view_binding_for_node(self, binding: NodeGraphNodeBinding) -> NodeGraphRuntimeViewBinding | None:
        config = dict(binding.config or {})
        view_type = _runtime_view_type(binding, config)
        if view_type is None:
            return None
        object_id = binding.owned_object_id
        object_type: str | None = None
        if object_id is not None:
            handle = self.object_handle(object_id)
            object_type = handle.object_type if handle is not None else None
        elif binding.object_refs:
            ref = binding.object_refs[0]
            object_id = ref.object_id
            object_type = ref.object_type
        handle = self.object_handle(object_id) if object_id is not None else None
        available = handle is not None and handle.handle_attached
        reason = None if available else "runtime handle is not attached" if object_id else "node has no runtime object"
        return NodeGraphRuntimeViewBinding(
            node_id=binding.node_id,
            view_type=view_type,
            object_id=object_id,
            object_type=object_type,
            title=binding.title,
            config=config,
            available=available,
            reason=reason,
        )

    def object_handle(self, object_id: str) -> NodeGraphRuntimeHandle | None:
        return self._handles.get(str(object_id))

    def require_object_handle(self, object_id: str) -> NodeGraphRuntimeHandle:
        handle = self.object_handle(object_id)
        if handle is None:
            raise KeyError(f"runtime object {object_id!r} does not exist")
        return handle

    def attach_handle(self, object_id: str, handle: Any, *, status: str = "attached") -> NodeGraphRuntimeHandle:
        runtime_handle = self.require_object_handle(object_id)
        self._detach_terminal_event_listener(runtime_handle.object_id)
        runtime_handle.attach(handle, status=status)
        self._attach_terminal_event_listener(runtime_handle)
        self.emit_event(
            "object_handle_attached",
            object_id=runtime_handle.object_id,
            node_id=runtime_handle.owner_node_id,
            data={"object_type": runtime_handle.object_type, "status": runtime_handle.status},
        )
        return runtime_handle

    def detach_handle(self, object_id: str, *, status: str = "detached") -> Any | None:
        runtime_handle = self.require_object_handle(object_id)
        self._detach_terminal_event_listener(runtime_handle.object_id)
        detached = runtime_handle.detach(status=status)
        self.emit_event(
            "object_handle_detached",
            object_id=runtime_handle.object_id,
            node_id=runtime_handle.owner_node_id,
            data={"object_type": runtime_handle.object_type, "status": runtime_handle.status},
        )
        return detached

    def _attach_terminal_event_listener(self, runtime_handle: NodeGraphRuntimeHandle) -> None:
        if runtime_handle.object_type != "terminal_session" or runtime_handle.handle is None:
            return
        add_listener = getattr(runtime_handle.handle, "add_event_listener", None)
        if not callable(add_listener):
            return

        object_id = runtime_handle.object_id

        def handle_terminal_event(event: Any) -> None:
            self.apply_terminal_event(object_id, event)

        remover = add_listener(handle_terminal_event)
        if callable(remover):
            self._terminal_event_listener_removers[object_id] = remover

    def _detach_terminal_event_listener(self, object_id: str) -> None:
        remover = self._terminal_event_listener_removers.pop(str(object_id), None)
        if remover is not None:
            remover()

    def set_object_status(self, object_id: str, status: str, *, error: object | None = None) -> NodeGraphRuntimeHandle:
        runtime_handle = self.require_object_handle(object_id)
        runtime_handle.set_status(status, error=error)
        data: dict[str, object] = {"object_type": runtime_handle.object_type, "status": runtime_handle.status}
        if runtime_handle.error is not None:
            data["error"] = runtime_handle.error
        self.emit_event(
            "object_status_changed",
            object_id=runtime_handle.object_id,
            node_id=runtime_handle.owner_node_id,
            data=data,
        )
        return runtime_handle

    def create_terminal_bridge(
        self,
        object_id: str,
        *,
        start: bool = False,
        on_event: Callable[[Any], object] | None = None,
        on_output: Callable[[str], object] | None = None,
        **overrides: object,
    ) -> Any:
        """Create and attach a TerminalBridge for a terminal_session object.

        The bridge is not started unless ``start=True``. This keeps graph
        runtime construction non-destructive while giving Terminal Session nodes
        a concrete live handle path.
        """

        runtime_handle = self.require_object_handle(object_id)
        if runtime_handle.object_type != "terminal_session":
            raise ValueError(f"runtime object {object_id!r} is {runtime_handle.object_type!r}, not 'terminal_session'")
        config = dict(runtime_handle.config or {})
        config.update(overrides)
        command = config.get("command") or config.get("cmd") or config.get("executable") or "powershell.exe"
        args = _sequence_config(config.get("args"), "terminal args")
        env_value = config.get("env")
        if env_value is not None and not isinstance(env_value, Mapping):
            raise TypeError("terminal env config must be a mapping")
        cwd = config.get("cwd")

        from .terminal import TerminalBridge

        bridge = TerminalBridge(
            command,
            args=args,
            cwd=None if cwd is None else str(cwd),
            env=None if env_value is None else {str(key): str(value) for key, value in env_value.items()},
            cols=int(config.get("cols", 100)),
            rows=int(config.get("rows", 30)),
            prefer_pty=bool(config.get("prefer_pty", True)),
            on_output=on_output,
            on_event=on_event,
            capture_transcript=bool(config.get("capture_transcript", True)),
            max_transcript_entries=int(config.get("max_transcript_entries", 10000)),
        )
        self.attach_handle(runtime_handle.object_id, bridge, status="ready")
        self.emit_event(
            "terminal_bridge_created",
            node_id=runtime_handle.owner_node_id,
            object_id=runtime_handle.object_id,
            data={"command": bridge.command.label, "started": False},
        )
        if start:
            self.start_terminal_session(runtime_handle.object_id)
        return bridge

    def ensure_runtime_dependencies(self) -> dict[str, object]:
        """Create non-destructive live handles required by declared runtime objects."""

        created: list[str] = []
        started: list[str] = []
        attached: list[str] = []
        errors: dict[str, str] = {}
        for runtime_handle in list(self._handles.values()):
            if runtime_handle.object_type != "terminal_session":
                continue
            try:
                if runtime_handle.handle is None:
                    if self._terminal_has_dynamic_config_inputs(runtime_handle):
                        runtime_handle.set_status("pending_config")
                    else:
                        self.create_terminal_bridge(runtime_handle.object_id, start=False)
                        created.append(runtime_handle.object_id)
                else:
                    attached.append(runtime_handle.object_id)
                config = runtime_handle.config or {}
                if bool(config.get("auto_start", False)):
                    bridge = self.start_terminal_session(runtime_handle.object_id)
                    if bool(getattr(bridge, "session_active", False)):
                        started.append(runtime_handle.object_id)
            except Exception as exc:
                runtime_handle.set_status("failed", error=exc)
                errors[runtime_handle.object_id] = str(exc)
        result = {"created": created, "started": started, "attached": attached, "errors": errors}
        self.emit_event("runtime_dependencies_ensured", data=result)
        return result

    def _terminal_has_dynamic_config_inputs(self, runtime_handle: NodeGraphRuntimeHandle) -> bool:
        owner_node_id = runtime_handle.owner_node_id
        if not owner_node_id:
            return False
        return any(
            edge.target_node == owner_node_id and edge.target_port in _TERMINAL_CONFIG_INPUT_PORTS
            for edge in self.binding.edges
        )

    def start_terminal_session(self, object_id: str) -> Any:
        """Start an attached terminal bridge, creating one from config when needed."""

        runtime_handle = self.require_object_handle(object_id)
        if runtime_handle.object_type != "terminal_session":
            raise ValueError(f"runtime object {object_id!r} is {runtime_handle.object_type!r}, not 'terminal_session'")
        bridge = runtime_handle.handle
        if bridge is None:
            bridge = self.create_terminal_bridge(runtime_handle.object_id, start=False)
        already_running = bool(getattr(bridge, "session_active", False))
        if already_running:
            runtime_handle.set_status("running")
            self.emit_event(
                "terminal_start_requested",
                node_id=runtime_handle.owner_node_id,
                object_id=runtime_handle.object_id,
                data={"object_type": runtime_handle.object_type, "already_running": True},
            )
            return bridge
        self.set_object_status(runtime_handle.object_id, "starting")
        if not hasattr(bridge, "start"):
            raise TypeError(f"runtime object {object_id!r} handle does not support start()")
        started = bridge.start()
        if started is not None:
            bridge = started
            runtime_handle.handle = bridge
        self.emit_event(
            "terminal_start_requested",
            node_id=runtime_handle.owner_node_id,
            object_id=runtime_handle.object_id,
            data={"object_type": runtime_handle.object_type},
        )
        return bridge

    def send_terminal_input(self, object_id: str, text: object, *, newline: bool = False) -> bool:
        """Send text to an attached terminal bridge and emit a stdin runtime event."""

        runtime_handle = self.require_object_handle(object_id)
        if runtime_handle.object_type != "terminal_session":
            raise ValueError(f"runtime object {object_id!r} is {runtime_handle.object_type!r}, not 'terminal_session'")
        bridge = runtime_handle.handle
        if bridge is None:
            bridge = self.start_terminal_session(runtime_handle.object_id)
        if hasattr(bridge, "session_active") and not bool(getattr(bridge, "session_active", False)):
            bridge = self.start_terminal_session(runtime_handle.object_id)
        method_name = "send_line" if newline else "send_text"
        method = getattr(bridge, method_name, None)
        if method is None:
            raise TypeError(f"runtime object {object_id!r} handle does not support {method_name}()")
        payload = str(text)
        delivered = bool(method(payload))
        self.emit_event(
            "terminal_stdin",
            node_id=runtime_handle.owner_node_id,
            port_id="stdin",
            object_id=runtime_handle.object_id,
            value=payload + ("\n" if newline else ""),
            data={"delivered": delivered, "newline": bool(newline), "object_type": runtime_handle.object_type},
        )
        return delivered

    def stop_runtime_object(self, object_id: str, *, detach: bool = False) -> bool:
        """Stop an attached runtime handle when it exposes stop/close/dispose."""

        runtime_handle = self.require_object_handle(object_id)
        handle = runtime_handle.handle
        stopped = False
        external_handle = bool((runtime_handle.config or {}).get("_external_terminal_widget_bridge"))
        if handle is not None and not external_handle:
            for method_name in ("stop", "close", "dispose"):
                method = getattr(handle, method_name, None)
                if method is None:
                    continue
                method()
                stopped = True
                break
        runtime_handle.set_status("detached" if external_handle else "stopped")
        self.emit_event(
            "object_stop_requested",
            node_id=runtime_handle.owner_node_id,
            object_id=runtime_handle.object_id,
            data={"object_type": runtime_handle.object_type, "stopped": stopped, "external_handle": external_handle},
        )
        if detach or external_handle:
            self.detach_handle(runtime_handle.object_id, status="detached" if external_handle else "stopped")
        return stopped

    def cleanup(self) -> dict[str, object]:
        """Stop all attached runtime handles and mark the runtime session stopped."""

        stopped: list[str] = []
        errors: dict[str, str] = {}
        for runtime_handle in list(self._handles.values()):
            if runtime_handle.handle is None:
                continue
            try:
                if self.stop_runtime_object(runtime_handle.object_id):
                    stopped.append(runtime_handle.object_id)
            except Exception as exc:  # pragma: no cover - defensive cleanup path
                runtime_handle.set_status("failed", error=exc)
                errors[runtime_handle.object_id] = str(exc)
        self.status = "stopped" if not errors else "failed"
        self.emit_event("session_cleanup", data={"stopped": stopped, "errors": errors})
        return {"stopped": stopped, "errors": errors}

    def apply_terminal_event(self, object_id: str, event: Any) -> NodeGraphRuntimeEvent:
        """Apply a TerminalEvent-like payload to an attached terminal object."""

        runtime_handle = self.require_object_handle(object_id)
        payload = event.to_dict() if hasattr(event, "to_dict") else dict(event)
        name = str(payload.get("event", ""))
        runtime_event = {
            "bridge_started": "terminal_bridge_started",
            "session_started": "terminal_started",
            "session_ended": "terminal_stopped",
            "bridge_stopped": "terminal_bridge_stopped",
            "output": "terminal_stdout",
            "screen_snapshot": "terminal_screen",
            "input": "terminal_stdin",
        }.get(name, f"terminal_{name}" if name else "terminal_event")
        port_id = {"output": "stdout", "screen_snapshot": "screen", "input": "stdin"}.get(name)
        status = {
            "bridge_started": "starting",
            "session_started": "running",
            "session_ended": "exited",
            "bridge_stopped": "stopped",
        }.get(name)
        if status is not None:
            runtime_handle.set_status(status)
        data = {"terminal_event": _json_safe_value(payload), "object_type": runtime_handle.object_type}
        value = payload.get("data") if name in {"output", "screen_snapshot"} else None
        return self.emit_event(
            runtime_event,
            node_id=runtime_handle.owner_node_id,
            port_id=port_id,
            object_id=runtime_handle.object_id,
            value=value,
            data=data,
            timestamp=float(payload.get("timestamp", time.time())),
        )

    def run_node(self, node_id: str, *, timestamp: float | None = None) -> NodeGraphRuntimeEvent:
        """Run a source node and emit its configured outputs into the live graph."""

        binding = self.binding.node_binding(node_id)
        if binding is None:
            raise KeyError(f"runtime node {node_id!r} does not exist")
        if binding.node_type not in _RUNTIME_SOURCE_NODE_TYPES:
            return self.emit_event(
                "node_run_skipped",
                node_id=binding.node_id,
                data={"reason": "node is not a runnable source", "node_type": binding.node_type},
                timestamp=timestamp,
            )
        return self._run_runtime_source_node(binding, timestamp=timestamp)

    def run_section(self, section_id: str, *, timestamp: float | None = None) -> NodeGraphRuntimeEvent:
        """Run source nodes bound to a section, including upstream dependencies."""

        section = self.binding.section_binding(section_id)
        if section is None:
            raise KeyError(f"runtime section {section_id!r} does not exist")
        before_sequence = self._sequence
        executed_nodes: list[str] = []
        skipped_nodes: list[dict[str, object]] = []
        runnable_node_ids = self._section_runnable_source_node_ids(section)
        for node_id in section.node_ids:
            binding = self.binding.node_binding(node_id)
            if binding is None:
                skipped_nodes.append({"node_id": str(node_id), "reason": "node binding is missing"})
                continue
            if binding.node_type not in _RUNTIME_SOURCE_NODE_TYPES:
                skipped_nodes.append(
                    {
                        "node_id": binding.node_id,
                        "node_type": binding.node_type,
                        "reason": "node is not a runnable source",
                    }
                )
        for node_id in runnable_node_ids:
            binding = self.binding.node_binding(node_id)
            if binding is None:
                continue
            self._run_runtime_source_node(binding, timestamp=timestamp)
            executed_nodes.append(binding.node_id)
        generated = [event for event in self._events if event.sequence > before_sequence]
        return self.emit_event(
            "section_run",
            section_id=section.section_id,
            data={
                "title": section.title,
                "trigger": section.trigger,
                "executed_nodes": executed_nodes,
                "runnable_source_nodes": runnable_node_ids,
                "skipped_nodes": skipped_nodes,
                "event_count": len(generated),
                "events": [event.event for event in generated],
            },
            timestamp=timestamp,
        )

    def _section_runnable_source_node_ids(self, section: NodeGraphSectionBinding) -> list[str]:
        section_node_ids = set(section.node_ids)
        upstream: set[str] = set(section_node_ids)
        stack = list(section_node_ids)
        while stack:
            target_id = stack.pop()
            for edge in self.binding.edges:
                if edge.target_node != target_id:
                    continue
                source_id = edge.source_node
                if source_id in upstream:
                    continue
                upstream.add(source_id)
                stack.append(source_id)
        ordered: list[str] = []
        for binding in self.binding.nodes:
            if binding.node_id in upstream and binding.node_type in _RUNTIME_SOURCE_NODE_TYPES:
                ordered.append(binding.node_id)
        return ordered

    def run_section_command(
        self, section_id: str, command: str = "run", *, timestamp: float | None = None
    ) -> NodeGraphRuntimeEvent:
        """Run a first-pass lifecycle command for a section."""

        command_s = str(command or "run").strip() or "run"
        if command_s in {"run", "replay"}:
            event = self.run_section(section_id, timestamp=timestamp)
            if command_s == "replay" and event.data is not None:
                event.data["command"] = "replay"
            return event
        section = self.binding.section_binding(section_id)
        if section is None:
            raise KeyError(f"runtime section {section_id!r} does not exist")
        object_ids = self._section_object_ids(section)
        stopped: list[str] = []
        detached: list[str] = []
        errors: dict[str, str] = {}
        for object_id in object_ids:
            handle = self.object_handle(object_id)
            if handle is None:
                continue
            try:
                if handle.handle is not None and self.stop_runtime_object(object_id):
                    stopped.append(object_id)
                elif command_s == "stop":
                    handle.set_status("stopped")
            except Exception as exc:  # pragma: no cover - defensive runtime command path
                handle.set_status("failed", error=exc)
                errors[object_id] = str(exc)
            if command_s == "reset" and handle.handle is not None:
                self.detach_handle(object_id, status="declared")
                detached.append(object_id)
        if command_s == "reset":
            self._clear_section_port_values(section)
            return self.emit_event(
                "section_reset",
                section_id=section.section_id,
                data={"title": section.title, "object_ids": object_ids, "stopped": stopped, "detached": detached, "errors": errors},
                timestamp=timestamp,
            )
        if command_s == "stop":
            return self.emit_event(
                "section_stop",
                section_id=section.section_id,
                data={"title": section.title, "object_ids": object_ids, "stopped": stopped, "errors": errors},
                timestamp=timestamp,
            )
        return self.emit_event(
            "section_command_unsupported",
            section_id=section.section_id,
            data={"title": section.title, "command": command_s, "supported_commands": ["run", "stop", "reset", "replay"]},
            timestamp=timestamp,
        )

    def _section_object_ids(self, section: NodeGraphSectionBinding) -> list[str]:
        ids: list[str] = []
        for node_id in section.node_ids:
            binding = self.binding.node_binding(node_id)
            if binding is not None and binding.owned_object_id is not None and binding.owned_object_id not in ids:
                ids.append(binding.owned_object_id)
        return ids

    def _clear_section_port_values(self, section: NodeGraphSectionBinding) -> None:
        node_ids = set(section.node_ids)
        for key in list(self._port_values):
            if key[0] in node_ids:
                self._port_values.pop(key, None)

    def _run_runtime_source_node(
        self, binding: NodeGraphNodeBinding, *, timestamp: float | None = None
    ) -> NodeGraphRuntimeEvent:
        if self._execution_depth >= 32:
            return self.emit_event(
                "node_execution_skipped",
                node_id=binding.node_id,
                data={"reason": "max execution depth reached", "node_type": binding.node_type},
                timestamp=timestamp,
            )
        node = _runtime_node_from_binding(binding)
        log: list[dict[str, object]] = []
        self._execution_depth += 1
        try:
            if binding.node_type == "widget_source":
                outputs, source_log = self._read_widget_source(binding, timestamp=timestamp)
                log.extend(source_log)
            else:
                outputs = _execute_flow_node(node, {}, self._parser_state, log)
        finally:
            self._execution_depth -= 1
        executed = self.emit_event(
            "node_executed",
            node_id=binding.node_id,
            data={
                "node_type": binding.node_type,
                "input_port": None,
                "source": "run_node",
                "output_counts": {port: len(items) for port, items in outputs.items()},
                "log": _json_safe_value(log),
            },
            timestamp=timestamp,
        )
        for output_port, values in outputs.items():
            for output_value in values:
                self.emit_event(
                    "node_output",
                    node_id=binding.node_id,
                    port_id=output_port,
                    value=output_value,
                    data={"node_type": binding.node_type, "source": "run_node"},
                    timestamp=timestamp,
                )
        return executed

    def emit_event(
        self,
        event: str,
        *,
        node_id: str | None = None,
        port_id: str | None = None,
        section_id: str | None = None,
        object_id: str | None = None,
        value: object | None = None,
        data: Mapping[str, object] | None = None,
        timestamp: float | None = None,
    ) -> NodeGraphRuntimeEvent:
        self._sequence += 1
        item = NodeGraphRuntimeEvent(
            event=str(event),
            node_id=None if node_id is None else str(node_id),
            port_id=None if port_id is None else str(port_id),
            section_id=None if section_id is None else str(section_id),
            object_id=None if object_id is None else str(object_id),
            value=value,
            data=None if data is None else _json_copy(data, "runtime event data"),
            sequence=self._sequence,
            timestamp=time.time() if timestamp is None else float(timestamp),
        )
        self._events.append(item)
        self._record_and_propagate_port_event(item)
        return item

    def _record_and_propagate_port_event(self, item: NodeGraphRuntimeEvent) -> None:
        if item.node_id is None or item.port_id is None or item.value is None:
            return
        key = (item.node_id, item.port_id)
        self._port_values.setdefault(key, []).append(item.value)
        if item.event == "edge_value":
            self._apply_edge_value_to_runtime_target(item)
            self._execute_runtime_node(item.node_id, item.port_id, item.value, timestamp=item.timestamp)
            return
        for edge in self.binding.edges:
            if edge.source_node != item.node_id or edge.source_port != item.port_id:
                continue
            self.emit_event(
                "edge_value",
                node_id=edge.target_node,
                port_id=edge.target_port,
                value=_convert_edge_value(edge, item.value),
                data={
                    "edge_id": edge.edge_id,
                    "source_node": edge.source_node,
                    "source_port": edge.source_port,
                    "target_node": edge.target_node,
                    "target_port": edge.target_port,
                    "source_event": item.event,
                    "conversion": edge.conversion,
                    "config": _json_copy(edge.config or {}, "runtime edge config"),
                },
                timestamp=item.timestamp,
            )

    def _apply_edge_value_to_runtime_target(self, item: NodeGraphRuntimeEvent) -> None:
        data = item.data or {}
        if self._apply_terminal_config_input(item):
            return
        if data.get("conversion") != "text_to_terminal_input":
            return
        if item.node_id is None:
            return
        binding = self.binding.node_binding(item.node_id)
        if binding is None or binding.node_type != "terminal":
            return
        object_id = binding.owned_object_id
        if not object_id:
            self.emit_event(
                "edge_conversion_failed",
                node_id=item.node_id,
                value=item.value,
                data={"reason": "target terminal has no runtime object", "conversion": data.get("conversion"), "edge_id": data.get("edge_id")},
                timestamp=item.timestamp,
            )
            return
        config = data.get("config") if isinstance(data.get("config"), Mapping) else {}
        newline = bool(config.get("newline", True)) if isinstance(config, Mapping) else True
        try:
            delivered = self.send_terminal_input(object_id, item.value, newline=newline)
        except Exception as exc:
            self.emit_event(
                "edge_conversion_failed",
                node_id=item.node_id,
                value=item.value,
                data={"reason": str(exc), "conversion": data.get("conversion"), "edge_id": data.get("edge_id"), "object_id": object_id},
                timestamp=item.timestamp,
            )
            return
        self.emit_event(
            "edge_conversion_applied",
            node_id=item.node_id,
            object_id=object_id,
            value=item.value,
            data={"delivered": delivered, "conversion": data.get("conversion"), "edge_id": data.get("edge_id"), "newline": newline},
            timestamp=item.timestamp,
        )

    def _apply_terminal_config_input(self, item: NodeGraphRuntimeEvent) -> bool:
        if item.node_id is None or item.port_id not in _TERMINAL_CONFIG_INPUT_PORTS:
            return False
        binding = self.binding.node_binding(item.node_id)
        if binding is None or binding.node_type != "terminal":
            return False
        object_id = binding.owned_object_id
        if not object_id:
            self.emit_event(
                "terminal_config_rejected",
                node_id=item.node_id,
                port_id=item.port_id,
                value=item.value,
                data={"reason": "target terminal has no runtime object"},
                timestamp=item.timestamp,
            )
            return True
        runtime_handle = self.require_object_handle(object_id)
        bridge = runtime_handle.handle
        if bridge is not None and bool(getattr(bridge, "session_active", False)):
            self.emit_event(
                "terminal_config_rejected",
                node_id=item.node_id,
                port_id=item.port_id,
                object_id=object_id,
                value=item.value,
                data={"reason": "terminal is already running", "config_key": item.port_id},
                timestamp=item.timestamp,
            )
            return True
        config = dict(runtime_handle.config or {})
        try:
            config[item.port_id] = _terminal_config_input_value(item.port_id, item.value)
        except Exception as exc:
            self.emit_event(
                "terminal_config_rejected",
                node_id=item.node_id,
                port_id=item.port_id,
                object_id=object_id,
                value=item.value,
                data={"reason": str(exc), "config_key": item.port_id},
                timestamp=item.timestamp,
            )
            return True
        runtime_handle.config = config
        runtime_handle.updated_at = time.time()
        if bridge is not None:
            stop = getattr(bridge, "stop", None)
            if callable(stop):
                stop()
            runtime_handle.detach(status="pending_config")
        else:
            runtime_handle.set_status("pending_config")
        self.emit_event(
            "terminal_config_applied",
            node_id=item.node_id,
            port_id=item.port_id,
            object_id=object_id,
            value=item.value,
            data={"config_key": item.port_id, "config": _json_safe_value(config)},
            timestamp=item.timestamp,
        )
        return True

    def _execute_runtime_node(self, node_id: str, port_id: str, value: object, *, timestamp: float | None = None) -> None:
        binding = self.binding.node_binding(node_id)
        if binding is None or binding.node_type not in _RUNTIME_EXECUTABLE_NODE_TYPES:
            return
        if self._execution_depth >= 32:
            self.emit_event(
                "node_execution_skipped",
                node_id=node_id,
                data={"reason": "max execution depth reached", "node_type": binding.node_type},
                timestamp=timestamp,
            )
            return
        node = _runtime_node_from_binding(binding)
        log: list[dict[str, object]] = []
        self._execution_depth += 1
        try:
            if binding.node_type == "widget_sink":
                log.extend(self._apply_widget_sink(binding, value, timestamp=timestamp))
                outputs = {"value": [value]}
            else:
                inputs = self._runtime_node_inputs(binding, port_id, value)
                missing_inputs = self._runtime_missing_connected_inputs(binding, inputs)
                if missing_inputs:
                    self.emit_event(
                        "node_execution_waiting",
                        node_id=node_id,
                        data={
                            "node_type": binding.node_type,
                            "input_port": port_id,
                            "missing_inputs": missing_inputs,
                        },
                        timestamp=timestamp,
                    )
                    return
                outputs = _execute_flow_node(node, inputs, self._parser_state, log)
        finally:
            self._execution_depth -= 1
        self.emit_event(
            "node_executed",
            node_id=node_id,
            data={
                "node_type": binding.node_type,
                "input_port": port_id,
                "output_counts": {port: len(items) for port, items in outputs.items()},
                "log": _json_safe_value(log),
            },
            timestamp=timestamp,
        )
        for output_port, values in outputs.items():
            for output_value in values:
                self.emit_event(
                    "node_output",
                    node_id=node_id,
                    port_id=output_port,
                    value=output_value,
                    data={"node_type": binding.node_type},
                    timestamp=timestamp,
                )

    def _runtime_node_inputs(
        self, binding: NodeGraphNodeBinding, port_id: str, value: object
    ) -> dict[str, list[object]]:
        inputs: dict[str, list[object]] = {}
        connected_ports = {
            edge.target_port for edge in self.binding.edges if edge.target_node == binding.node_id
        }
        connected_ports.add(str(port_id))
        for input_port in connected_ports:
            values = self._port_values.get((binding.node_id, input_port), [])
            if values:
                inputs[input_port] = list(values)
        inputs.setdefault(str(port_id), [value])
        return inputs

    def _runtime_missing_connected_inputs(
        self, binding: NodeGraphNodeBinding, inputs: Mapping[str, list[object]]
    ) -> list[str]:
        connected_ports = {
            edge.target_port for edge in self.binding.edges if edge.target_node == binding.node_id
        }
        return sorted(port for port in connected_ports if not inputs.get(port))

    def _apply_widget_sink(
        self, binding: NodeGraphNodeBinding, value: object, *, timestamp: float | None = None
    ) -> list[dict[str, object]]:
        config = binding.config or {}
        widget_id = str(config.get("widget_id", "")).strip()
        widget_type = str(config.get("widget_type", "")).strip()
        update_mode = str(config.get("update_mode", "") or "auto").strip()
        value_format = str(config.get("format", "") or "text").strip()
        port_profile = str(config.get("port_profile", "") or "").strip()
        if port_profile == "terminal_output" and value_format == "text":
            value_format = "terminal_text"
        if not widget_id:
            result = {"ok": False, "reason": "widget_id is required"}
            self.emit_event("widget_update_failed", node_id=binding.node_id, data=result, timestamp=timestamp)
            return [result]
        widget = self.widget_handle(widget_id)
        if widget is None:
            result = {"ok": False, "widget_id": widget_id, "reason": "widget is not registered"}
            self.emit_event("widget_update_failed", node_id=binding.node_id, data=result, timestamp=timestamp)
            return [result]
        actual_type = _widget_kind(widget)
        if widget_type and widget_type != actual_type:
            result = {
                "ok": False,
                "widget_id": widget_id,
                "widget_type": actual_type,
                "expected_widget_type": widget_type,
                "reason": "registered widget type does not match node config",
            }
            self.emit_event("widget_update_failed", node_id=binding.node_id, data=result, timestamp=timestamp)
            return [result]
        try:
            applied_mode = _update_widget_sink(widget, value, update_mode=update_mode, value_format=value_format)
        except Exception as exc:
            result = {
                "ok": False,
                "widget_id": widget_id,
                "widget_type": actual_type,
                "update_mode": update_mode,
                "reason": str(exc),
            }
            self.emit_event("widget_update_failed", node_id=binding.node_id, data=result, timestamp=timestamp)
            return [result]
        result = {
            "ok": True,
            "widget_id": widget_id,
            "widget_type": actual_type,
            "update_mode": applied_mode,
            "format": value_format,
        }
        self.emit_event("widget_updated", node_id=binding.node_id, data=result, timestamp=timestamp)
        return [result]

    def _read_widget_source(
        self, binding: NodeGraphNodeBinding, *, timestamp: float | None = None
    ) -> tuple[dict[str, list[object]], list[dict[str, object]]]:
        config = binding.config or {}
        widget_id = str(config.get("widget_id", "")).strip()
        widget_type = str(config.get("widget_type", "")).strip()
        value_format = str(config.get("format", "") or "text").strip()
        port_profile = str(config.get("port_profile", "") or "text").strip()
        if not widget_id:
            result = {"ok": False, "reason": "widget_id is required"}
            self.emit_event("widget_read_failed", node_id=binding.node_id, data=result, timestamp=timestamp)
            return {}, [result]
        widget = self.widget_handle(widget_id)
        if widget is None:
            result = {"ok": False, "widget_id": widget_id, "reason": "widget is not registered"}
            self.emit_event("widget_read_failed", node_id=binding.node_id, data=result, timestamp=timestamp)
            return {}, [result]
        actual_type = _widget_kind(widget)
        if widget_type and widget_type != actual_type:
            result = {
                "ok": False,
                "widget_id": widget_id,
                "widget_type": actual_type,
                "expected_widget_type": widget_type,
                "reason": "registered widget type does not match node config",
            }
            self.emit_event("widget_read_failed", node_id=binding.node_id, data=result, timestamp=timestamp)
            return {}, [result]
        try:
            value = _widget_source_value(widget, value_format)
        except Exception as exc:
            result = {
                "ok": False,
                "widget_id": widget_id,
                "widget_type": actual_type,
                "format": value_format,
                "reason": str(exc),
            }
            self.emit_event("widget_read_failed", node_id=binding.node_id, data=result, timestamp=timestamp)
            return {}, [result]
        result = {
            "ok": True,
            "widget_id": widget_id,
            "widget_type": actual_type,
            "format": value_format,
            "port_profile": port_profile,
        }
        self.emit_event(
            "widget_read",
            node_id=binding.node_id,
            value=value,
            data=result,
            timestamp=timestamp,
        )
        return {"value": [value]}, [result]
    def validate(self) -> dict[str, object]:
        return self.binding.validate()

    def to_dict(self) -> dict[str, object]:
        return {
            "schema_version": _NODE_GRAPH_RUNTIME_SCHEMA_VERSION,
            "session_id": self.session_id,
            "status": self.status,
            "valid": self.valid,
            "validation": self.validate(),
            "objects": [handle.to_dict() for handle in self._handles.values()],
            "widgets": [{"widget_id": widget_id, "widget_type": _widget_kind(widget)} for widget_id, widget in self._widgets.items()],
            "views": [binding.to_dict() for binding in self.view_bindings()],
            "port_values": {
                f"{node}.{port}": [_json_safe_value(value) for value in values]
                for (node, port), values in self._port_values.items()
            },
            "events": [event.to_dict() for event in self._events],
        }

    snapshot = to_dict


@dataclass(frozen=True, slots=True)
class NodeGraphFlowRun:
    """Result of a non-destructive primitive node graph flow run."""

    values: dict[str, list[object]]
    log: tuple[dict[str, object], ...]
    binding: NodeGraphRuntimeBinding

    @property
    def valid(self) -> bool:
        return self.binding.valid

    def port_values(self, node_id: str, port_id: str) -> list[object]:
        return list(self.values.get(f"{node_id}.{port_id}", []))

    def to_dict(self) -> dict[str, object]:
        return {
            "valid": self.valid,
            "values": _json_safe_value(self.values),
            "log": list(self.log),
            "binding": self.binding.to_dict(),
        }


@dataclass(slots=True)
class NodeGraphPort:
    """One input or output socket on a node graph node."""

    id: str
    label: str | None = None
    data: dict[str, object] | None = None
    port_type: str | None = None


@dataclass(slots=True)
class NodeGraphNode:
    """Node model used by :class:`NodeGraph`."""

    id: str
    title: str
    x: float
    y: float
    inputs: tuple[NodeGraphPort, ...] = ()
    outputs: tuple[NodeGraphPort, ...] = ()
    subtitle: str | None = None
    status: str | None = None
    color: str = "#43c6ac"
    width: float = 190.0
    data: dict[str, object] | None = None


@dataclass(frozen=True, slots=True)
class NodeGraphEdge:
    """Connection from one node output port to another node input port."""

    source_node: str
    source_port: str
    target_node: str
    target_port: str
    label: str | None = None
    color: str = "#43c6ac"
    id: str | None = None
    data: dict[str, object] | None = None


@dataclass(slots=True)
class NodeGraphTemplate:
    """Palette template for creating a node graph node."""

    id: str
    title: str
    inputs: tuple[NodeGraphPort, ...] = ()
    outputs: tuple[NodeGraphPort, ...] = ()
    subtitle: str | None = None
    status: str | None = None
    color: str = "#43c6ac"
    width: float = 190.0
    data: dict[str, object] | None = None



@dataclass(slots=True)
class NodeGraphBindingTarget:
    """Unified transient GUI binding target exposed to node graph inspectors."""

    id: str
    label: str | None = None
    target_type: str | None = None
    widget_type: str | None = None
    widget: Any | None = None
    action_type: str | None = None
    callback: Callable[[str, str], object] | None = None
    supported_update_modes: tuple[str, ...] = ()
    default_update_mode: str | None = None
    supported_port_profiles: tuple[str, ...] = ()
    default_port_profile: str | None = None
    supported_formats: tuple[str, ...] = ()
    supported_commands: tuple[str, ...] = ()
    default_command: str | None = None
    data: dict[str, object] | None = None


@dataclass(slots=True)
class NodeGraphWidgetTarget:
    """Transient GUI widget target exposed to Widget Sink node editors."""

    id: str
    label: str | None = None
    widget_type: str | None = None
    supported_update_modes: tuple[str, ...] = ()
    default_update_mode: str | None = None
    supported_port_profiles: tuple[str, ...] = ()
    default_port_profile: str | None = None
    supported_formats: tuple[str, ...] = ()
    widget: Any | None = None
    data: dict[str, object] | None = None

@dataclass(slots=True)
class NodeGraphActionTarget:
    """Transient GUI action target exposed to section inspectors."""

    id: str
    label: str | None = None
    action_type: str | None = None
    supported_commands: tuple[str, ...] = ()
    default_command: str | None = None
    callback: Callable[[str, str], object] | None = None
    data: dict[str, object] | None = None

@dataclass(slots=True)
class NodeGraphSection:
    """Visual section region that groups nodes by purpose or runtime scope."""

    id: str
    title: str
    x: float
    y: float
    width: float
    height: float
    purpose: str | None = None
    trigger: str | None = None
    color: str = "#43c6ac"
    collapsed: bool = False
    locked: bool = False
    data: dict[str, object] | None = None

class NodeGraph(HtmlReport):
    """Interactive node editor surface for routing graphs and agent workflows.

    This implementation uses a self-contained HTML canvas hosted by DragonGui's
    WebView-backed HtmlReport widget. The canvas owns drawing and hit-testing so
    node cards, headers, sockets, panning, and zooming stay in the same
    coordinate system.
    """

    def __init__(
        self,
        nodes: Sequence[NodeGraphNode | Mapping[str, object]],
        edges: Sequence[NodeGraphEdge | Mapping[str, object]] = (),
        *,
        sections: Sequence[NodeGraphSection | Mapping[str, object]] = (),
        selected_node: str | None = None,
        show_edge_labels: bool = False,
        show_port_labels: bool = True,
        show_status_labels: bool = False,
        show_subtitles: bool = True,
        enable_zoom: bool = True,
        templates: Sequence[NodeGraphTemplate | Mapping[str, object]] | None = None,
        widget_targets: Sequence[NodeGraphWidgetTarget | Mapping[str, object]] = (),
        action_targets: Sequence[NodeGraphActionTarget | Mapping[str, object]] = (),
        binding_targets: Sequence[NodeGraphBindingTarget | Mapping[str, object]] = (),
        runtime_policy: str = "auto",
        show_editor_chrome: bool = False,
        editor_title: str = "NodeGraph",
        editor_actions: Sequence[Mapping[str, object]] = (),
        width: int | float | None = 920,
        height: int | float | None = 560,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
        on_graph_event: Callable[[dict[str, object]], object] | None = None,
        **callbacks: object,
    ) -> None:
        self.nodes = tuple(self._node_from_value(node) for node in nodes)
        self.edges = tuple(self._edge_from_value(edge) for edge in edges)
        self.sections = tuple(self._section_from_value(section) for section in sections)
        self.selected_node = selected_node if selected_node in self._node_ids() else None
        self.show_edge_labels = bool(show_edge_labels)
        self.show_port_labels = bool(show_port_labels)
        self.show_status_labels = bool(show_status_labels)
        self.show_subtitles = bool(show_subtitles)
        self.enable_zoom = bool(enable_zoom)
        template_values = _default_templates() if templates is None else templates
        self.templates = tuple(self._template_from_value(template) for template in template_values)
        self.widget_targets = tuple(self._widget_target_from_value(target) for target in widget_targets)
        self.action_targets = tuple(self._action_target_from_value(target) for target in action_targets)
        self.binding_targets = tuple(self._binding_target_from_value(target) for target in binding_targets)
        for target in self.binding_targets:
            self._apply_binding_target(target)
        self.runtime_policy = self._runtime_policy_from_value(runtime_policy)
        self.show_editor_chrome = bool(show_editor_chrome)
        self.editor_title = self._text(editor_title, "NodeGraph editor title")
        self.editor_actions = tuple(_editor_action_payload(action) for action in editor_actions)
        self._managed_runtime_session: NodeGraphRuntimeSession | None = None
        self._managed_runtime_policy: str | None = None
        self._managed_runtime_binding_signature: str | None = None
        self._refreshing_auto_binding_targets = False
        self.on_graph_event = on_graph_event
        self._callback_compat = dict(callbacks)
        self._undo_stack: list[dict[str, object]] = []
        self._redo_stack: list[dict[str, object]] = []
        self._viewport_state: dict[str, object] = {"schema_version": _GRAPH_SCHEMA_VERSION, "x": 34.0, "y": 32.0, "zoom": 1.0}
        self._history_baseline = self.to_graph_data()
        html = self._html()
        super().__init__(
            html=html,
            allow_scripts=True,
            external_fallback=False,
            width=width,
            height=height,
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

    def set_nodes(self, nodes: Sequence[NodeGraphNode | Mapping[str, object]]) -> None:
        self.nodes = tuple(self._node_from_value(node) for node in nodes)
        if self.selected_node not in self._node_ids():
            self.selected_node = None
        self._clear_history()
        self.set_html(self._html())

    def set_edges(self, edges: Sequence[NodeGraphEdge | Mapping[str, object]]) -> None:
        self.edges = tuple(self._edge_from_value(edge) for edge in edges)
        self._clear_history()
        self.set_html(self._html())

    def set_templates(self, templates: Sequence[NodeGraphTemplate | Mapping[str, object]]) -> None:
        self.templates = tuple(self._template_from_value(template) for template in templates)
        self.set_html(self._html())

    def set_widget_targets(self, targets: Sequence[NodeGraphWidgetTarget | Mapping[str, object]]) -> None:
        """Replace assignable widget targets exposed to Widget Sink inspectors."""

        self.widget_targets = tuple(self._widget_target_from_value(target) for target in targets)
        self.set_html(self._html())

    def set_action_targets(self, targets: Sequence[NodeGraphActionTarget | Mapping[str, object]]) -> None:
        """Replace assignable action targets exposed to section inspectors."""

        self.action_targets = tuple(self._action_target_from_value(target) for target in targets)
        self.set_html(self._html())

    def set_binding_targets(self, targets: Sequence[NodeGraphBindingTarget | Mapping[str, object]]) -> None:
        """Replace unified GUI binding targets and refresh derived widget/action targets."""

        old_ids = {target.id for target in self.binding_targets}
        self.binding_targets = tuple(self._binding_target_from_value(target) for target in targets)
        new_ids = {target.id for target in self.binding_targets}
        remove_ids = old_ids | new_ids
        self.widget_targets = tuple(
            target for target in self.widget_targets if (target.data or {}).get("binding_target_id") not in remove_ids
        )
        self.action_targets = tuple(
            target for target in self.action_targets if (target.data or {}).get("binding_target_id") not in remove_ids
        )
        for target in self.binding_targets:
            self._apply_binding_target(target)
        self.set_html(self._html())

    def register_binding_target(
        self,
        id: str,
        *,
        label: str | None = None,
        target_type: str | None = None,
        widget_type: str | None = None,
        widget: Any | None = None,
        action_type: str | None = None,
        callback: Callable[[str, str], object] | None = None,
        supported_update_modes: Sequence[str] = (),
        default_update_mode: str | None = None,
        supported_port_profiles: Sequence[str] = (),
        default_port_profile: str | None = None,
        supported_formats: Sequence[str] = (),
        supported_commands: Sequence[str] = (),
        default_command: str | None = None,
        data: Mapping[str, object] | None = None,
    ) -> NodeGraphBindingTarget:
        """Expose a GUI component/action through the unified binding registry."""

        target_id = self._text(id, "binding target id")
        target = NodeGraphBindingTarget(
            id=target_id,
            label=None if label is None else str(label),
            target_type=None if target_type is None else str(target_type),
            widget_type=None if widget_type is None else str(widget_type),
            widget=widget,
            action_type=None if action_type is None else str(action_type),
            callback=callback,
            supported_update_modes=tuple(str(mode) for mode in supported_update_modes),
            default_update_mode=None if default_update_mode is None else str(default_update_mode),
            supported_port_profiles=tuple(str(profile) for profile in supported_port_profiles),
            default_port_profile=None if default_port_profile is None else str(default_port_profile),
            supported_formats=tuple(str(fmt) for fmt in supported_formats),
            supported_commands=tuple(str(command) for command in supported_commands),
            default_command=None if default_command is None else str(default_command),
            data=None if data is None else _json_copy(dict(data), "binding target data"),
        )
        self.binding_targets = tuple(existing for existing in self.binding_targets if existing.id != target.id) + (target,)
        self.widget_targets = tuple(
            existing for existing in self.widget_targets if (existing.data or {}).get("binding_target_id") != target.id
        )
        self.action_targets = tuple(
            existing for existing in self.action_targets if (existing.data or {}).get("binding_target_id") != target.id
        )
        self._apply_binding_target(target)
        self.set_html(self._html())
        return target

    def unregister_binding_target(self, target_id: str) -> NodeGraphBindingTarget | None:
        """Remove a unified binding target and its derived widget/action targets."""

        text = self._text(target_id, "binding target id")
        removed = next((target for target in self.binding_targets if target.id == text), None)
        if removed is not None:
            self.binding_targets = tuple(target for target in self.binding_targets if target.id != text)
            self.widget_targets = tuple(
                target for target in self.widget_targets if (target.data or {}).get("binding_target_id") != text
            )
            self.action_targets = tuple(
                target for target in self.action_targets if (target.data or {}).get("binding_target_id") != text
            )
            self.set_html(self._html())
        return removed

    def binding_target(self, target_id: str) -> NodeGraphBindingTarget | None:
        """Return a unified GUI binding target by ID, if registered."""

        self._ensure_auto_binding_targets()
        text = self._text(target_id, "binding target id")
        return next((target for target in self.binding_targets if target.id == text), None)

    def binding_target_ids(self) -> tuple[str, ...]:
        """Return unified GUI binding target IDs."""

        self._ensure_auto_binding_targets()
        return tuple(target.id for target in self.binding_targets)

    def register_action_target(
        self,
        id: str,
        *,
        label: str | None = None,
        action_type: str | None = None,
        callback: Callable[[str, str], object] | None = None,
        supported_commands: Sequence[str] = (),
        default_command: str | None = None,
        data: Mapping[str, object] | None = None,
    ) -> NodeGraphActionTarget:
        """Expose a live GUI action as an assignable section target."""

        target_id = self._text(id, "action target id")
        target = NodeGraphActionTarget(
            id=target_id,
            label=None if label is None else str(label),
            action_type=None if action_type is None else str(action_type),
            supported_commands=tuple(str(command) for command in supported_commands),
            default_command=None if default_command is None else str(default_command),
            callback=callback,
            data=None if data is None else _json_copy(dict(data), "action target data"),
        )
        self.action_targets = tuple(existing for existing in self.action_targets if existing.id != target.id) + (target,)
        self.set_html(self._html())
        return target

    def unregister_action_target(self, action_id: str) -> NodeGraphActionTarget | None:
        """Remove and return an assignable section action target by ID."""

        target_id = self._text(action_id, "action target id")
        removed = next((target for target in self.action_targets if target.id == target_id), None)
        if removed is not None:
            self.action_targets = tuple(target for target in self.action_targets if target.id != target_id)
            self.set_html(self._html())
        return removed

    def action_target(self, action_id: str) -> NodeGraphActionTarget | None:
        """Return an assignable section action target by ID, if registered."""

        self._ensure_auto_binding_targets()
        target_id = self._text(action_id, "action target id")
        return next((target for target in self.action_targets if target.id == target_id), None)

    def action_target_ids(self) -> tuple[str, ...]:
        """Return assignable section action target IDs."""

        self._ensure_auto_binding_targets()
        return tuple(target.id for target in self.action_targets)

    def run_action_target(self, action_id: str, command: str = "run") -> object:
        """Invoke a registered action target callback."""

        target = self.action_target(action_id)
        if target is None:
            raise KeyError(f"NodeGraph action target {action_id!r} is not registered")
        command_s = str(command or target.default_command or "run")
        if target.supported_commands and command_s not in target.supported_commands:
            raise ValueError(f"action target {target.id!r} does not support command {command_s!r}")
        if target.callback is None:
            raise RuntimeError(f"action target {target.id!r} has no callback")
        return target.callback(target.id, command_s)

    def run_section_action(self, section_id: str) -> object:
        """Invoke the action target configured on a section."""

        section = next((existing for existing in self.sections if existing.id == str(section_id)), None)
        if section is None:
            raise KeyError(f"NodeGraph section {section_id!r} does not exist")
        config = _section_config(section)
        action_id = str(config.get("action_id", "") or "").strip()
        command = str(config.get("section_command", "") or "run").strip() or "run"
        if not action_id:
            raise RuntimeError(f"section {section.id!r} has no action_id configured")
        return self.run_action_target(action_id, command)

    def register_widget_target(
        self,
        id: str | None = None,
        *,
        label: str | None = None,
        widget_type: str | None = None,
        widget: Any | None = None,
        supported_update_modes: Sequence[str] = (),
        default_update_mode: str | None = None,
        supported_port_profiles: Sequence[str] = (),
        default_port_profile: str | None = None,
        supported_formats: Sequence[str] = (),
        data: Mapping[str, object] | None = None,
    ) -> NodeGraphWidgetTarget:
        """Expose a live GUI widget as an assignable Widget Sink target."""

        widget_id = id if id is not None else getattr(widget, "id", None)
        target_id = self._text(widget_id, "widget target id")
        actual_type = widget_type or (_widget_kind(widget) if widget is not None else None)
        target = NodeGraphWidgetTarget(
            id=target_id,
            label=None if label is None else str(label),
            widget_type=None if actual_type is None else str(actual_type),
            supported_update_modes=tuple(str(mode) for mode in supported_update_modes),
            default_update_mode=None if default_update_mode is None else str(default_update_mode),
            supported_port_profiles=tuple(str(profile) for profile in supported_port_profiles),
            default_port_profile=None if default_port_profile is None else str(default_port_profile),
            supported_formats=tuple(str(fmt) for fmt in supported_formats),
            widget=widget,
            data=None if data is None else _json_copy(dict(data), "widget target data"),
        )
        self.widget_targets = tuple(existing for existing in self.widget_targets if existing.id != target.id) + (target,)
        self.set_html(self._html())
        return target

    def unregister_widget_target(self, widget_id: str) -> NodeGraphWidgetTarget | None:
        """Remove and return an assignable Widget Sink target by ID."""

        target_id = self._text(widget_id, "widget target id")
        removed = next((target for target in self.widget_targets if target.id == target_id), None)
        if removed is not None:
            self.widget_targets = tuple(target for target in self.widget_targets if target.id != target_id)
            self.set_html(self._html())
        return removed

    def widget_target(self, widget_id: str) -> NodeGraphWidgetTarget | None:
        """Return an assignable Widget Sink target by ID, if registered."""

        self._ensure_auto_binding_targets()
        target_id = self._text(widget_id, "widget target id")
        return next((target for target in self.widget_targets if target.id == target_id), None)

    def widget_target_ids(self) -> tuple[str, ...]:
        """Return assignable Widget Sink target IDs."""

        self._ensure_auto_binding_targets()
        return tuple(target.id for target in self.widget_targets)

    def refresh_binding_targets_from_host(self, root: Container | None = None) -> tuple[NodeGraphBindingTarget, ...]:
        """Refresh bindable targets discovered from the surrounding DragonGUI tree.

        Host widgets with explicit stable IDs are treated as intentional public
        targets. Generated ``dg-*`` IDs are ignored so anonymous layout controls
        do not clutter graph inspector dropdowns.
        """

        targets = self._sync_auto_binding_targets(root=root)
        self.set_html(self._render_html())
        return targets

    def _ensure_auto_binding_targets(self) -> None:
        if not getattr(self, "_refreshing_auto_binding_targets", False):
            self._sync_auto_binding_targets()

    def _sync_auto_binding_targets(self, root: Container | None = None) -> tuple[NodeGraphBindingTarget, ...]:
        if getattr(self, "_refreshing_auto_binding_targets", False):
            return tuple(
                target for target in self.binding_targets if _target_data_is_auto_binding(target.data)
            )
        self._refreshing_auto_binding_targets = True
        try:
            self._clear_auto_binding_targets()
            reserved_binding_ids = {target.id for target in self.binding_targets}
            reserved_widget_ids = {target.id for target in self.widget_targets}
            reserved_action_ids = {target.id for target in self.action_targets}
            applied: list[NodeGraphBindingTarget] = []
            for target in self._discover_host_binding_targets(root=root):
                exposes_widget = self._binding_target_exposes_widget(target)
                exposes_action = self._binding_target_exposes_action(target)
                if target.id in reserved_binding_ids:
                    continue
                if exposes_widget and target.id in reserved_widget_ids:
                    continue
                if exposes_action and target.id in reserved_action_ids:
                    continue
                self.binding_targets = (*self.binding_targets, target)
                self._apply_binding_target(target)
                reserved_binding_ids.add(target.id)
                if exposes_widget:
                    reserved_widget_ids.add(target.id)
                if exposes_action:
                    reserved_action_ids.add(target.id)
                applied.append(target)
            return tuple(applied)
        finally:
            self._refreshing_auto_binding_targets = False

    def _clear_auto_binding_targets(self) -> None:
        self.binding_targets = tuple(
            target for target in self.binding_targets if not _target_data_is_auto_binding(target.data)
        )
        self.widget_targets = tuple(
            target for target in self.widget_targets if not _target_data_is_auto_binding(target.data)
        )
        self.action_targets = tuple(
            target for target in self.action_targets if not _target_data_is_auto_binding(target.data)
        )

    def _discover_host_binding_targets(self, root: Container | None = None) -> tuple[NodeGraphBindingTarget, ...]:
        host_root = self._host_binding_root(root)
        if host_root is None:
            return ()
        targets: list[NodeGraphBindingTarget] = []
        seen: set[str] = set()
        for widget in _walk_widget_tree(host_root):
            if widget is self or not isinstance(widget, Widget):
                continue
            target = self._binding_target_from_widget(widget)
            if target is None or target.id in seen:
                continue
            seen.add(target.id)
            targets.append(target)
        return tuple(targets)

    def _host_binding_root(self, root: Container | None = None) -> Container | None:
        if root is not None:
            return root
        parent = getattr(self, "parent", None)
        if parent is None:
            return None
        current: Container = parent
        while isinstance(current.parent, Container):
            current = current.parent
        return current

    def _binding_target_from_widget(self, widget: Widget) -> NodeGraphBindingTarget | None:
        widget_id = str(getattr(widget, "id", "") or "").strip()
        if not widget_id or not bool(getattr(widget, "_explicit_id", False)):
            return None
        kind = _widget_kind(widget)
        label = _binding_target_label(widget)
        data = {
            _NODE_GRAPH_AUTO_BINDING_DATA_KEY: True,
            "binding_target_source": "host_widget_tree",
            "widget_kind": kind,
        }
        if hasattr(widget, "bridge"):
            data["runtime_object_id"] = widget_id
            return NodeGraphBindingTarget(
                id=widget_id,
                label=label,
                target_type="terminal",
                widget_type="terminal",
                widget=widget,
                supported_update_modes=("append", "set"),
                default_update_mode="append",
                supported_port_profiles=("terminal_output", "text"),
                default_port_profile="terminal_output",
                supported_formats=("terminal_text", "text", "repr"),
                data=data,
            )
        if kind in _NODE_GRAPH_AUTO_BINDING_WIDGET_KINDS:
            modes, default_mode, profiles, default_profile, formats = _auto_widget_target_capabilities(kind)
            return NodeGraphBindingTarget(
                id=widget_id,
                label=label,
                target_type=kind,
                widget_type=kind,
                widget=widget,
                supported_update_modes=modes,
                default_update_mode=default_mode,
                supported_port_profiles=profiles,
                default_port_profile=default_profile,
                supported_formats=formats,
                data=data,
            )
        if _widget_is_action_target(widget):
            return NodeGraphBindingTarget(
                id=widget_id,
                label=label,
                target_type="action",
                action_type=kind,
                callback=_widget_action_callback(widget),
                supported_commands=("run",),
                default_command="run",
                data=data,
            )
        return None

    def _binding_target_data(self, target: NodeGraphBindingTarget) -> dict[str, object]:
        data = dict(target.data or {})
        data["binding_target_id"] = target.id
        if target.target_type is not None:
            data["binding_target_type"] = target.target_type
        return _json_copy(data, "binding target data")

    def _binding_target_exposes_widget(self, target: NodeGraphBindingTarget) -> bool:
        target_type = str(target.target_type or "").lower()
        widget_types = {
            "widget",
            "source",
            "sink",
            "display",
            "log",
            "terminal",
            "terminal_output",
            "text_input",
            "text_source",
            "event_log",
        }
        return (
            target.widget is not None
            or target.widget_type is not None
            or bool(target.supported_update_modes)
            or bool(target.supported_port_profiles)
            or bool(target.supported_formats)
            or target_type in widget_types
        )

    def _binding_target_exposes_action(self, target: NodeGraphBindingTarget) -> bool:
        target_type = str(target.target_type or "").lower()
        action_types = {"action", "button", "command", "section_action", "trigger"}
        return (
            target.callback is not None
            or target.action_type is not None
            or bool(target.supported_commands)
            or target_type in action_types
        )

    def _apply_binding_target(self, target: NodeGraphBindingTarget) -> None:
        data = self._binding_target_data(target)
        if self._binding_target_exposes_widget(target):
            actual_type = target.widget_type or (_widget_kind(target.widget) if target.widget is not None else target.target_type)
            widget_target = NodeGraphWidgetTarget(
                id=target.id,
                label=target.label,
                widget_type=None if actual_type is None else str(actual_type),
                supported_update_modes=target.supported_update_modes,
                default_update_mode=target.default_update_mode,
                supported_port_profiles=target.supported_port_profiles,
                default_port_profile=target.default_port_profile,
                supported_formats=target.supported_formats,
                widget=target.widget,
                data=data,
            )
            self.widget_targets = tuple(existing for existing in self.widget_targets if existing.id != target.id) + (widget_target,)
        if self._binding_target_exposes_action(target):
            actual_type = target.action_type or target.target_type
            action_target = NodeGraphActionTarget(
                id=target.id,
                label=target.label,
                action_type=None if actual_type is None else str(actual_type),
                supported_commands=target.supported_commands,
                default_command=target.default_command,
                callback=target.callback,
                data=data,
            )
            self.action_targets = tuple(existing for existing in self.action_targets if existing.id != target.id) + (action_target,)

    def to_graph_data(self) -> dict[str, object]:
        """Return a JSON-serializable, versioned snapshot of the graph."""

        return _json_copy(
            {
                "schema_version": _GRAPH_SCHEMA_VERSION,
                "nodes": [_node_graph_data(node) for node in self.nodes],
                "edges": [_edge_graph_data(edge, index, self.nodes) for index, edge in enumerate(self.edges)],
                "sections": [_section_graph_data(section) for section in self.sections],
            },
            "NodeGraph graph data",
        )

    def set_graph_data(self, data: Mapping[str, object]) -> None:
        """Replace graph contents from :meth:`to_graph_data` data."""

        nodes, edges, sections = self._graph_items_from_data(data)
        self.nodes = nodes
        self.edges = edges
        self.sections = sections
        if self.selected_node not in self._node_ids():
            self.selected_node = None
        self._clear_history()
        self.set_html(self._html())

    @classmethod
    def from_graph_data(cls, data: Mapping[str, object], **kwargs: object) -> NodeGraph:
        """Create a :class:`NodeGraph` from versioned graph data."""

        nodes, edges, sections = cls._graph_items_from_data(data)
        return cls(nodes, edges, sections=sections, **kwargs)

    def runtime_object_registry(self) -> NodeGraphObjectRegistry:
        """Return registry records for runtime objects declared by graph nodes."""

        registry = NodeGraphObjectRegistry()
        for node in self.nodes:
            obj = _runtime_object_from_node(node)
            if obj is not None:
                registry.register(obj)
        return registry

    def run_text_flow(
        self,
        initial_inputs: Mapping[str, object] | None = None,
        *,
        max_steps: int = 128,
        registry: NodeGraphObjectRegistry | None = None,
    ) -> NodeGraphFlowRun:
        """Run a small non-destructive text/message flow through primitive nodes."""

        return _run_node_graph_text_flow(self, initial_inputs or {}, max_steps=max_steps, registry=registry)


    def runtime_binding(self, registry: NodeGraphObjectRegistry | None = None) -> NodeGraphRuntimeBinding:
        """Return a static binding plan for graph runtime execution."""

        active_registry = self.runtime_object_registry() if registry is None else registry
        refs = self.runtime_object_refs()
        return NodeGraphRuntimeBinding(
            nodes=tuple(_node_runtime_binding(node) for node in self.nodes),
            sections=tuple(_section_runtime_binding(section, self.section_nodes(section.id)) for section in self.sections),
            edges=tuple(_edge_runtime_binding(edge, index, self.nodes) for index, edge in enumerate(self.edges)),
            registry=active_registry,
            missing_refs=active_registry.missing_object_refs(refs),
        )


    def runtime_session(
        self,
        registry: NodeGraphObjectRegistry | None = None,
        *,
        session_id: str | None = None,
    ) -> NodeGraphRuntimeSession:
        """Create live runtime session state for this graph's current binding."""

        return NodeGraphRuntimeSession(self.runtime_binding(registry), session_id=session_id)


    def managed_runtime_session(
        self,
        registry: NodeGraphObjectRegistry | None = None,
        *,
        session_id: str | None = None,
        policy: str | None = None,
    ) -> NodeGraphRuntimeSession:
        """Return a widget-managed runtime session, creating one on demand."""

        resolved = self._runtime_policy_from_value(self.runtime_policy if policy is None else policy)
        binding = self.runtime_binding(registry)
        binding_signature = json.dumps(binding.to_dict(), sort_keys=True)
        session = self._managed_runtime_session
        if (
            session is not None
            and self._managed_runtime_binding_signature is not None
            and self._managed_runtime_binding_signature != binding_signature
        ):
            self.cleanup_managed_runtime()
            session = None
        if session is None or session.status in {"stopped", "failed"}:
            session = NodeGraphRuntimeSession(binding, session_id=session_id)
            self._managed_runtime_session = session
            self._managed_runtime_binding_signature = binding_signature
        self._managed_runtime_policy = resolved
        self._register_managed_runtime_widgets(session)
        session.ensure_runtime_dependencies()
        return session


    @property
    def managed_runtime(self) -> NodeGraphRuntimeSession | None:
        """Return the current widget-managed runtime session, if one is alive."""

        return self._managed_runtime_session

    def managed_runtime_status(self) -> dict[str, object]:
        """Return a compact status summary for the widget-managed runtime."""

        session = self._managed_runtime_session
        resolved_policy = self.resolved_runtime_policy()
        persistent = self.runtime_has_persistent_objects()
        if session is None:
            return {
                "active": False,
                "status": "idle",
                "runtime_policy": self.runtime_policy,
                "resolved_policy": resolved_policy,
                "persistent_objects": persistent,
                "session_id": None,
                "widgets": 0,
                "handles": 0,
                "events": 0,
                "last_event": None,
            }
        last_event = session.events[-1].event if session.events else None
        return {
            "active": session.status not in {"stopped", "failed"},
            "status": session.status,
            "runtime_policy": self.runtime_policy,
            "resolved_policy": resolved_policy,
            "persistent_objects": persistent,
            "session_id": session.session_id,
            "widgets": len(session.widget_ids()),
            "handles": len(session.handles),
            "events": len(session.events),
            "last_event": last_event,
        }

    def managed_runtime_status_text(self) -> str:
        """Return a short human-readable managed runtime status line."""

        status = self.managed_runtime_status()
        state = "active" if status["active"] else "idle"
        details = [
            f"Runtime: {state}",
            f"policy {status['resolved_policy']}",
            f"status {status['status']}",
            f"widgets {status['widgets']}",
            f"handles {status['handles']}",
        ]
        if status["last_event"]:
            details.append(f"last {status['last_event']}")
        return " | ".join(details)

    def runtime_has_persistent_objects(self) -> bool:
        """Return True when the graph declares live runtime objects such as terminals."""

        return any(_runtime_object_is_persistent(obj) for obj in self.runtime_object_registry())

    def resolved_runtime_policy(self, policy: str | None = None) -> str:
        """Resolve auto runtime lifetime into ephemeral or persistent for the current graph."""

        requested = self._runtime_policy_from_value(self.runtime_policy if policy is None else policy)
        if requested == "auto":
            return "persistent" if self.runtime_has_persistent_objects() else "ephemeral"
        return requested

    def cleanup_managed_runtime(self) -> dict[str, object]:
        """Stop and detach the widget-managed runtime session, if present."""

        session = self._managed_runtime_session
        self._managed_runtime_session = None
        self._managed_runtime_policy = None
        self._managed_runtime_binding_signature = None
        if session is None:
            return {"stopped": [], "errors": {}}
        return session.cleanup()

    def run_node_runtime(
        self,
        node_id: str,
        *,
        policy: str | None = None,
        session_id: str | None = None,
    ) -> NodeGraphRuntimeEvent:
        """Run a node using a runtime session managed by this NodeGraph widget."""

        session = self.managed_runtime_session(session_id=session_id, policy=policy)
        try:
            return session.run_node(node_id)
        finally:
            self._finish_managed_runtime(policy)

    def run_section_runtime(
        self,
        section_id: str,
        command: str | None = None,
        *,
        policy: str | None = None,
        session_id: str | None = None,
    ) -> NodeGraphRuntimeEvent:
        """Run a section command using a runtime session managed by this NodeGraph widget."""

        command_s = command
        if command_s is None:
            section = next((existing for existing in self.sections if existing.id == str(section_id)), None)
            config = _section_config(section) if section is not None else {}
            command_s = str(config.get("section_command", "") or "run")
        session = self.managed_runtime_session(session_id=session_id, policy=policy)
        try:
            return session.run_section_command(section_id, command_s)
        finally:
            self._finish_managed_runtime(policy)

    def _register_managed_runtime_widgets(self, session: NodeGraphRuntimeSession) -> None:
        self._ensure_auto_binding_targets()
        for target in self.widget_targets:
            if target.widget is not None:
                session.register_widget(target.id, target.widget)
                self._attach_terminal_widget_target(session, target)

    def _attach_terminal_widget_target(
        self, session: NodeGraphRuntimeSession, target: NodeGraphWidgetTarget
    ) -> None:
        widget = target.widget
        bridge = getattr(widget, "bridge", None)
        if bridge is None:
            return
        selected_object_ids = self._runtime_object_ids_for_terminal_widget_target(target.id)
        if selected_object_ids:
            candidates = selected_object_ids
        else:
            candidates = []
            data = target.data or {}
            for key in ("runtime_object_id", "object_id", "session_id", "terminal_session_id"):
                value = data.get(key)
                if value is not None and str(value).strip():
                    candidates.append(str(value).strip())
            candidates.append(target.id)
        for object_id in candidates:
            handle = session.object_handle(object_id)
            if handle is None or handle.object_type != "terminal_session":
                continue
            if handle.handle is bridge:
                return
            if selected_object_ids:
                if handle.handle is not None:
                    stop = getattr(handle.handle, "stop", None)
                    external = bool((handle.config or {}).get("_external_terminal_widget_bridge"))
                    if callable(stop) and not external:
                        stop()
                    session.detach_handle(object_id, status="detached")
                config = dict(handle.config or {})
                config["_external_terminal_widget_bridge"] = True
                config["terminal_widget_id"] = target.id
                handle.config = config
                session.attach_handle(object_id, bridge, status="running" if bool(getattr(bridge, "session_active", False)) else "ready")
            elif handle.handle is None:
                config = dict(handle.config or {})
                config["_external_terminal_widget_bridge"] = True
                config["terminal_widget_id"] = target.id
                handle.config = config
                session.attach_handle(object_id, bridge, status="running" if bool(getattr(bridge, "session_active", False)) else "ready")
            return

    def _runtime_object_ids_for_terminal_widget_target(self, widget_id: str) -> list[str]:
        selected_widget_id = str(widget_id).strip()
        if not selected_widget_id:
            return []
        object_ids: list[str] = []
        for node in self.nodes:
            if _node_type(node) != "terminal":
                continue
            config = _node_config(node)
            if str(config.get("terminal_widget_id", "")).strip() != selected_widget_id:
                continue
            runtime_object = _runtime_object_from_node(node)
            if runtime_object is not None and runtime_object.object_id not in object_ids:
                object_ids.append(runtime_object.object_id)
        return object_ids

    def _finish_managed_runtime(self, policy: str | None = None) -> None:
        if self.resolved_runtime_policy(policy) == "ephemeral":
            self.cleanup_managed_runtime()

    def runtime_object_refs(self) -> tuple[NodeGraphRuntimeObjectRef, ...]:
        """Return runtime object references declared by graph nodes."""

        refs: list[NodeGraphRuntimeObjectRef] = []
        for node in self.nodes:
            refs.extend(_runtime_refs_from_node(node))
        return tuple(refs)

    def missing_runtime_object_refs(
        self, registry: NodeGraphObjectRegistry | None = None
    ) -> tuple[NodeGraphRuntimeObjectRef, ...]:
        """Return graph runtime references that are absent from the registry."""

        active_registry = self.runtime_object_registry() if registry is None else registry
        return active_registry.missing_object_refs(self.runtime_object_refs())


    def set_node_position(self, node_id: str, x: float, y: float, *, notify: bool = False) -> None:
        node = self._node_by_id(node_id)
        node.x = self._finite(x, "x")
        node.y = self._finite(y, "y")
        self.set_html(self._html())
        if notify:
            self._dispatch_graph_event(
                {
                    "schema_version": _GRAPH_SCHEMA_VERSION,
                    "event": "node_moved",
                    "node": node.id,
                    "position": {"x": node.x, "y": node.y},
                }
            )

    def update_node(
        self,
        node_id: str,
        *,
        title: object | None = None,
        subtitle: object | None = None,
        status: object | None = None,
        color: object | None = None,
        notify: bool = False,
    ) -> None:
        before = self.to_graph_data()
        node = self._node_by_id(node_id)
        updates: dict[str, object] = {}
        if title is not None:
            node.title = self._text(title, "node title")
            updates["title"] = node.title
        if subtitle is not None:
            node.subtitle = str(subtitle) if str(subtitle) else None
            updates["subtitle"] = node.subtitle
        if status is not None:
            node.status = str(status) if str(status) else None
            updates["status"] = node.status
        if color is not None:
            node.color = str(color)
            updates["color"] = node.color
        if updates and self.to_graph_data() != before:
            self._undo_stack.append(before)
            self._redo_stack.clear()
        self.set_html(self._html())
        if notify and updates:
            self._dispatch_graph_event(
                {
                    "schema_version": _GRAPH_SCHEMA_VERSION,
                    "event": "node_updated",
                    "node": node.id,
                    "updates": updates,
                    "history": self.history_state(),
                }
            )
            self._dispatch_graph_event(
                {"schema_version": _GRAPH_SCHEMA_VERSION, "event": "graph_changed", "history": self.history_state()}
            )

    def node_position(self, node_id: str) -> tuple[float, float]:
        node = self._node_by_id(node_id)
        return node.x, node.y

    def section_nodes(self, section_id: str) -> tuple[str, ...]:
        """Return node IDs whose centers are currently inside a section."""

        section = next((existing for existing in self.sections if existing.id == section_id), None)
        if section is None:
            raise KeyError(f"Unknown NodeGraph section {section_id!r}")
        return tuple(node.id for node in self.nodes if _node_center_in_section(node, section))

    def create_node_from_template(
        self,
        template_id: str,
        x: float,
        y: float,
        *,
        node_id: str | None = None,
        notify: bool = False,
    ) -> NodeGraphNode:
        """Create a node from a registered template at graph coordinates."""

        template = self._template_by_id(template_id)
        new_id = self._text(node_id, "node id") if node_id is not None else self._next_node_id(template.id)
        if new_id in self._node_ids():
            raise ValueError(f"NodeGraph node id {new_id!r} already exists")
        data = _json_copy(template.data or {}, "template custom data")
        data["template_id"] = template.id
        data["template_title"] = template.title
        node = NodeGraphNode(
            id=new_id,
            title=template.title,
            x=self._finite(x, "node x"),
            y=self._finite(y, "node y"),
            inputs=self._clone_ports(template.inputs),
            outputs=self._clone_ports(template.outputs),
            subtitle=template.subtitle,
            status=template.status,
            color=template.color,
            width=template.width,
            data=data,
        )
        before = self.to_graph_data()
        self.nodes = (*self.nodes, node)
        self.selected_node = node.id
        if self.to_graph_data() != before:
            self._undo_stack.append(before)
            self._redo_stack.clear()
        self.set_html(self._html())
        if notify:
            self._dispatch_graph_event(
                {
                    "schema_version": _GRAPH_SCHEMA_VERSION,
                    "event": "node_created",
                    "node": _node_graph_data(node),
                    "history": self.history_state(),
                }
            )
            self._dispatch_graph_event(
                {"schema_version": _GRAPH_SCHEMA_VERSION, "event": "graph_changed", "history": self.history_state()}
            )
        return node

    def history_state(self) -> dict[str, object]:
        """Return compact undo/redo and dirty state for the graph editor."""

        return {
            "schema_version": _GRAPH_SCHEMA_VERSION,
            "can_undo": bool(self._undo_stack),
            "can_redo": bool(self._redo_stack),
            "dirty": self.to_graph_data() != self._history_baseline,
            "undo_depth": len(self._undo_stack),
            "redo_depth": len(self._redo_stack),
        }

    def navigation_state(self) -> dict[str, object]:
        """Return the last canvas viewport state reported to Python."""

        return _json_copy(self._viewport_state, "NodeGraph navigation state")

    def fit_to_view(self, *, notify: bool = True) -> dict[str, object]:
        """Emit a schema-v1 fit-to-view navigation request for callback consumers."""

        payload = {
            "schema_version": _GRAPH_SCHEMA_VERSION,
            "event": "fit_to_view",
            "viewport": self.navigation_state(),
        }
        if notify:
            self._dispatch_graph_event(payload)
        return _json_copy(payload, "NodeGraph fit_to_view payload")

    def undo(self, *, notify: bool = False) -> bool:
        """Restore the previous graph snapshot when one is available."""

        if not self._undo_stack:
            return False
        current = self.to_graph_data()
        previous = self._undo_stack.pop()
        self._redo_stack.append(current)
        self._restore_graph_data(previous)
        self.set_html(self._html())
        if notify:
            self._dispatch_graph_event(
                {"schema_version": _GRAPH_SCHEMA_VERSION, "event": "undo", "history": self.history_state()}
            )
            self._dispatch_graph_event(
                {"schema_version": _GRAPH_SCHEMA_VERSION, "event": "graph_changed", "history": self.history_state()}
            )
        return True

    def redo(self, *, notify: bool = False) -> bool:
        """Restore the next graph snapshot when one is available."""

        if not self._redo_stack:
            return False
        current = self.to_graph_data()
        next_graph = self._redo_stack.pop()
        self._undo_stack.append(current)
        self._restore_graph_data(next_graph)
        self.set_html(self._html())
        if notify:
            self._dispatch_graph_event(
                {"schema_version": _GRAPH_SCHEMA_VERSION, "event": "redo", "history": self.history_state()}
            )
            self._dispatch_graph_event(
                {"schema_version": _GRAPH_SCHEMA_VERSION, "event": "graph_changed", "history": self.history_state()}
            )
        return True

    def _html(self) -> str:
        self._ensure_auto_binding_targets()
        return self._render_html()

    def _render_html(self) -> str:
        return _node_graph_html(
            nodes=self.nodes,
            edges=self.edges,
            selected_node=self.selected_node,
            show_edge_labels=self.show_edge_labels,
            show_port_labels=self.show_port_labels,
            show_status_labels=self.show_status_labels,
            show_subtitles=self.show_subtitles,
            enable_zoom=self.enable_zoom,
            templates=self.templates,
            sections=self.sections,
            widget_targets=self.widget_targets,
            action_targets=self.action_targets,
            show_editor_chrome=self.show_editor_chrome,
            editor_title=self.editor_title,
            editor_actions=self.editor_actions,
            emit_events=True,
        )

    def props(self) -> dict[str, object]:
        self.html = self._html()
        props = super().props()
        props["events"] = ["change"]
        return props

    def _handle_graph_event(self, value: object) -> None:
        payload = json.loads(value) if isinstance(value, str) else value
        if not isinstance(payload, Mapping):
            raise TypeError("NodeGraph event payload must be a mapping")
        event = _json_copy(dict(payload), "NodeGraph event payload")
        if event.get("schema_version") != _GRAPH_SCHEMA_VERSION:
            raise ValueError(f"unsupported NodeGraph event schema_version {event.get('schema_version')!r}")
        event_name = str(event.get("event", ""))
        if event_name in _GRAPH_MUTATION_EVENTS:
            if event_name == "edge_created":
                edge_data = event.get("edge")
                if isinstance(edge_data, Mapping):
                    rejection = self._connection_rejection(edge_data)
                    if rejection is not None:
                        self._dispatch_graph_event(
                            {
                                "schema_version": _GRAPH_SCHEMA_VERSION,
                                "event": "connection_rejected",
                                "reason": rejection,
                                "edge": _json_copy(dict(edge_data), "NodeGraph rejected edge payload"),
                                "history": self.history_state(),
                            }
                        )
                        return
            before = self.to_graph_data()
            self._apply_graph_event(event)
            if self.to_graph_data() != before:
                self._undo_stack.append(before)
                self._redo_stack.clear()
            event["history"] = self.history_state()
            self._dispatch_graph_event(event)
        elif event_name == "undo":
            if self._undo_from_canvas():
                event["history"] = self.history_state()
                self._dispatch_graph_event(event)
                self._dispatch_graph_event(
                    {"schema_version": _GRAPH_SCHEMA_VERSION, "event": "graph_changed", "history": self.history_state()}
                )
        elif event_name == "redo":
            if self._redo_from_canvas():
                event["history"] = self.history_state()
                self._dispatch_graph_event(event)
                self._dispatch_graph_event(
                    {"schema_version": _GRAPH_SCHEMA_VERSION, "event": "graph_changed", "history": self.history_state()}
                )
        else:
            self._apply_graph_event(event)
            if event_name == "viewport_changed":
                viewport = event.get("viewport")
                if isinstance(viewport, Mapping):
                    self._viewport_state = self._viewport_from_value(viewport)
                    event["viewport"] = self.navigation_state()
            elif event_name == "fit_to_view":
                event["viewport"] = self.navigation_state()
            elif event_name == "graph_changed":
                event["history"] = self.history_state()
            self._dispatch_graph_event(event)

    def _dispatch_graph_event(self, payload: dict[str, object]) -> None:
        if self.on_graph_event is not None:
            self.on_graph_event(payload)
        event_name = str(payload.get("event", ""))
        if event_name == "node_selected":
            callback = self._callback_compat.get("on_node_select")
            if callable(callback):
                callback(payload.get("node"))
        elif event_name == "node_moved":
            callback = self._callback_compat.get("on_node_move")
            if callable(callback):
                position = payload.get("position", {})
                if isinstance(position, Mapping):
                    callback(payload.get("node"), position.get("x"), position.get("y"))

    def _apply_graph_event(self, payload: Mapping[str, object]) -> None:
        event = str(payload.get("event", ""))
        if event == "node_selected":
            node = payload.get("node")
            self.selected_node = str(node) if node is not None and str(node) in self._node_ids() else None
        elif event in {"edge_selected", "selection_cleared"}:
            self.selected_node = None
        elif event == "node_moved":
            node_id = self._text(payload.get("node"), "node id")
            position = payload.get("position", {})
            if not isinstance(position, Mapping):
                raise TypeError("NodeGraph node_moved position must be a mapping")
            node = self._node_by_id(node_id)
            node.x = self._finite(position.get("x"), "node x")
            node.y = self._finite(position.get("y"), "node y")
        elif event in {"node_created", "node_duplicated"}:
            node_data = payload.get("node")
            if not isinstance(node_data, Mapping):
                raise TypeError(f"NodeGraph {event} node must be a mapping")
            node = self._node_from_value(node_data)
            self.nodes = tuple(existing for existing in self.nodes if existing.id != node.id) + (node,)
            self.selected_node = node.id
        elif event == "node_updated":
            node_id = self._text(payload.get("node"), "node id")
            updates = payload.get("updates", {})
            if not isinstance(updates, Mapping):
                raise TypeError("NodeGraph node_updated updates must be a mapping")
            node = self._node_by_id(node_id)
            if "title" in updates:
                node.title = self._text(updates.get("title"), "node title")
            if "subtitle" in updates:
                value = updates.get("subtitle")
                node.subtitle = None if value is None or str(value) == "" else str(value)
            if "status" in updates:
                value = updates.get("status")
                node.status = None if value is None or str(value) == "" else str(value)
            if "color" in updates:
                node.color = str(updates.get("color"))
            if "inputs" in updates:
                node.inputs = self._ports_from_value(updates.get("inputs") or ())
            if "outputs" in updates:
                node.outputs = self._ports_from_value(updates.get("outputs") or ())
            if "data" in updates:
                value = updates.get("data")
                if value is None:
                    node.data = None
                elif isinstance(value, Mapping):
                    node.data = _json_copy(value, "node custom data")
                else:
                    raise TypeError("NodeGraph node_updated data must be a mapping or None")
            self.selected_node = node.id
        elif event == "node_deleted":
            node_id = self._text(payload.get("node"), "node id")
            self.nodes = tuple(node for node in self.nodes if node.id != node_id)
            self.edges = tuple(edge for edge in self.edges if edge.source_node != node_id and edge.target_node != node_id)
            if self.selected_node == node_id:
                self.selected_node = None
        elif event == "edge_created":
            edge_data = payload.get("edge")
            if not isinstance(edge_data, Mapping):
                raise TypeError("NodeGraph edge_created edge must be a mapping")
            edge = self._edge_from_value(edge_data)
            self.edges = tuple(existing for existing in self.edges if existing.id != edge.id) + (edge,)
            self.selected_node = None
        elif event == "edge_deleted":
            edge_id = self._text(payload.get("edge"), "edge id")
            self.edges = tuple(
                edge
                for index, edge in enumerate(self.edges)
                if (edge.id or f"edge-{index + 1}") != edge_id
            )
        elif event == "edge_waypoints_changed":
            edge_data = payload.get("edge")
            if not isinstance(edge_data, Mapping):
                raise TypeError("NodeGraph edge_waypoints_changed edge must be a mapping")
            edge = self._edge_from_value(edge_data)
            if edge.id is None:
                raise ValueError("NodeGraph edge_waypoints_changed edge id is required")
            updated: list[NodeGraphEdge] = []
            found = False
            for index, existing in enumerate(self.edges):
                existing_id = existing.id or f"edge-{index + 1}"
                if existing_id == edge.id:
                    updated.append(edge)
                    found = True
                else:
                    updated.append(existing)
            if not found:
                raise ValueError(f"NodeGraph edge {edge.id!r} does not exist")
            self.edges = tuple(updated)
            self.selected_node = None
        elif event == "section_created":
            section_data = payload.get("section")
            if not isinstance(section_data, Mapping):
                raise TypeError("NodeGraph section_created section must be a mapping")
            section = self._section_from_value(section_data)
            if any(existing.id == section.id for existing in self.sections):
                raise ValueError(f"NodeGraph section {section.id!r} already exists")
            self.sections = (*self.sections, section)
            self.selected_node = None
        elif event == "section_selected":
            section_id = self._text(payload.get("section"), "section id")
            if any(section.id == section_id for section in self.sections):
                self.selected_node = None
        elif event == "section_updated":
            section_id = self._text(payload.get("section"), "section id")
            updates = payload.get("updates", {})
            if not isinstance(updates, Mapping):
                raise TypeError("NodeGraph section_updated updates must be a mapping")
            section = next((existing for existing in self.sections if existing.id == section_id), None)
            if section is None:
                raise ValueError(f"NodeGraph section {section_id!r} does not exist")
            if "title" in updates:
                section.title = self._text(updates.get("title"), "section title")
            if "purpose" in updates:
                value = updates.get("purpose")
                section.purpose = None if value is None or str(value) == "" else str(value)
            if "trigger" in updates:
                value = updates.get("trigger")
                section.trigger = None if value is None or str(value) == "" else str(value)
            if "color" in updates:
                section.color = str(updates.get("color"))
            if "collapsed" in updates:
                section.collapsed = bool(updates.get("collapsed"))
            if "locked" in updates:
                section.locked = bool(updates.get("locked"))
            if "inputs" in updates:
                node.inputs = self._ports_from_value(updates.get("inputs") or ())
            if "outputs" in updates:
                node.outputs = self._ports_from_value(updates.get("outputs") or ())
            if "data" in updates:
                value = updates.get("data")
                if value is None:
                    section.data = None
                elif isinstance(value, Mapping):
                    section.data = _json_copy(value, "section custom data")
                else:
                    raise TypeError("NodeGraph section_updated data must be a mapping or None")
            self.selected_node = None
        elif event in {"section_moved", "section_resized"}:
            section_data = payload.get("section")
            if not isinstance(section_data, Mapping):
                raise TypeError(f"NodeGraph {event} section must be a mapping")
            section = self._section_from_value(section_data)
            if not any(existing.id == section.id for existing in self.sections):
                raise ValueError(f"NodeGraph section {section.id!r} does not exist")
            self.sections = tuple(section if existing.id == section.id else existing for existing in self.sections)
            moved_nodes = payload.get("nodes", ())
            if moved_nodes is not None:
                if isinstance(moved_nodes, (str, bytes, bytearray)) or not isinstance(moved_nodes, Sequence):
                    raise TypeError(f"NodeGraph {event} nodes must be a sequence")
                for node_data in moved_nodes:
                    if not isinstance(node_data, Mapping):
                        raise TypeError(f"NodeGraph {event} node positions must be mappings")
                    node = self._node_by_id(self._text(node_data.get("id"), "node id"))
                    position = node_data.get("position", {})
                    if not isinstance(position, Mapping):
                        raise TypeError(f"NodeGraph {event} node position must be a mapping")
                    node.x = self._finite(position.get("x"), "node x")
                    node.y = self._finite(position.get("y"), "node y")
            self.selected_node = None
        elif event == "section_deleted":
            section_id = self._text(payload.get("section"), "section id")
            self.sections = tuple(section for section in self.sections if section.id != section_id)
            self.selected_node = None

    def _restore_graph_data(self, data: Mapping[str, object]) -> None:
        nodes, edges, sections = self._graph_items_from_data(data)
        self.nodes = nodes
        self.edges = edges
        self.sections = sections
        if self.selected_node not in self._node_ids():
            self.selected_node = None

    def _undo_from_canvas(self) -> bool:
        if not self._undo_stack:
            return False
        current = self.to_graph_data()
        previous = self._undo_stack.pop()
        self._redo_stack.append(current)
        self._restore_graph_data(previous)
        return True

    def _redo_from_canvas(self) -> bool:
        if not self._redo_stack:
            return False
        current = self.to_graph_data()
        next_graph = self._redo_stack.pop()
        self._undo_stack.append(current)
        self._restore_graph_data(next_graph)
        return True

    def _clear_history(self) -> None:
        self._undo_stack.clear()
        self._redo_stack.clear()
        self._history_baseline = self.to_graph_data()

    @classmethod
    def _viewport_from_value(cls, value: Mapping[str, object]) -> dict[str, object]:
        return {
            "schema_version": _GRAPH_SCHEMA_VERSION,
            "x": cls._finite(value.get("x", 34.0), "viewport x"),
            "y": cls._finite(value.get("y", 32.0), "viewport y"),
            "zoom": cls._positive(value.get("zoom", 1.0), "viewport zoom"),
        }

    def _connection_rejection(self, edge_data: Mapping[str, object]) -> str | None:
        try:
            edge = self._edge_from_value(edge_data)
        except (TypeError, ValueError, KeyError) as exc:
            return str(exc)
        source_node = self._node_by_id_or_none(edge.source_node)
        if source_node is None:
            return f"unknown source node {edge.source_node!r}"
        target_node = self._node_by_id_or_none(edge.target_node)
        if target_node is None:
            return f"unknown target node {edge.target_node!r}"
        if source_node.id == target_node.id:
            return "self connection rejected"
        source_port = _port_by_id(source_node.outputs, edge.source_port)
        if source_port is None:
            if _port_by_id(source_node.inputs, edge.source_port) is not None:
                return "source port must be an output"
            return f"unknown source port {edge.source_port!r}"
        target_port = _port_by_id(target_node.inputs, edge.target_port)
        if target_port is None:
            if _port_by_id(target_node.outputs, edge.target_port) is not None:
                return "target port must be an input"
            return f"unknown target port {edge.target_port!r}"
        source_type = source_port.port_type
        target_type = target_port.port_type
        if source_type and target_type and source_type != target_type and _port_type_conversion(source_type, target_type) is None:
            return f"incompatible port types: {source_type!r} -> {target_type!r}"
        for index, existing in enumerate(self.edges):
            existing_id = existing.id or f"edge-{index + 1}"
            new_id = edge.id
            same_ports = (
                existing.source_node == edge.source_node
                and existing.source_port == edge.source_port
                and existing.target_node == edge.target_node
                and existing.target_port == edge.target_port
            )
            same_id = new_id is not None and existing_id == new_id
            if same_ports or same_id:
                return "duplicate edge"
        return None

    def _node_ids(self) -> set[str]:
        return {node.id for node in self.nodes}

    def _node_by_id(self, node_id: str) -> NodeGraphNode:
        for node in self.nodes:
            if node.id == node_id:
                return node
        raise KeyError(f"unknown node id {node_id!r}")

    def _node_by_id_or_none(self, node_id: str) -> NodeGraphNode | None:
        for node in self.nodes:
            if node.id == node_id:
                return node
        return None

    def _template_by_id(self, template_id: str) -> NodeGraphTemplate:
        normalized = self._text(template_id, "template id")
        template = next((template for template in self.templates if template.id == normalized), None)
        if template is None:
            raise ValueError(f"unknown NodeGraph template {normalized!r}")
        return template

    def _next_node_id(self, template_id: str) -> str:
        prefix = "".join(ch.lower() if ch.isalnum() else "-" for ch in template_id).strip("-") or "node"
        existing = self._node_ids()
        index = 1
        while f"{prefix}-{index}" in existing:
            index += 1
        return f"{prefix}-{index}"

    @classmethod
    def _clone_ports(cls, ports: Sequence[NodeGraphPort]) -> tuple[NodeGraphPort, ...]:
        cloned: list[NodeGraphPort] = []
        for port in ports:
            cloned.append(
                NodeGraphPort(
                    port.id,
                    port.label,
                    None if port.data is None else _json_copy(port.data, "port custom data"),
                    port.port_type,
                )
            )
        return tuple(cloned)

    @classmethod
    def _graph_items_from_data(
        cls, data: Mapping[str, object]
    ) -> tuple[tuple[NodeGraphNode, ...], tuple[NodeGraphEdge, ...], tuple[NodeGraphSection, ...]]:
        if not isinstance(data, Mapping):
            raise TypeError("NodeGraph graph data must be a mapping")
        version = data.get("schema_version")
        if version != _GRAPH_SCHEMA_VERSION:
            raise ValueError(f"unsupported NodeGraph schema_version {version!r}")
        nodes = data.get("nodes", ())
        edges = data.get("edges", ())
        sections = data.get("sections", ())
        if isinstance(nodes, (str, bytes, bytearray)) or not isinstance(nodes, Sequence):
            raise TypeError("NodeGraph graph data nodes must be a sequence")
        if isinstance(edges, (str, bytes, bytearray)) or not isinstance(edges, Sequence):
            raise TypeError("NodeGraph graph data edges must be a sequence")
        if isinstance(sections, (str, bytes, bytearray)) or not isinstance(sections, Sequence):
            raise TypeError("NodeGraph graph data sections must be a sequence")
        return (
            tuple(cls._node_from_value(node) for node in nodes),
            tuple(cls._edge_from_value(edge) for edge in edges),
            tuple(cls._section_from_value(section) for section in sections),
        )

    @classmethod
    def _section_from_value(cls, value: NodeGraphSection | Mapping[str, object]) -> NodeGraphSection:
        if isinstance(value, NodeGraphSection):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph sections must be NodeGraphSection instances or mappings")
        section_id = cls._text(value.get("id", value.get("section_id")), "section id")
        title = cls._text(value.get("title", value.get("label", section_id)), "section title")
        position = value.get("position", {})
        if position is None:
            position = {}
        if not isinstance(position, Mapping):
            raise TypeError("section position must be a mapping")
        size = value.get("size", {})
        if size is None:
            size = {}
        if not isinstance(size, Mapping):
            raise TypeError("section size must be a mapping")
        return NodeGraphSection(
            id=section_id,
            title=title,
            x=cls._finite(value.get("x", position.get("x", 0.0)), "section x"),
            y=cls._finite(value.get("y", position.get("y", 0.0)), "section y"),
            width=cls._positive(value.get("width", size.get("width", 320.0)), "section width"),
            height=cls._positive(value.get("height", size.get("height", 220.0)), "section height"),
            purpose=None if value.get("purpose") is None else str(value.get("purpose")),
            trigger=None if value.get("trigger") is None else str(value.get("trigger")),
            color=str(value.get("color", "#43c6ac")),
            collapsed=bool(value.get("collapsed", False)),
            locked=bool(value.get("locked", False)),
            data=cls._data_from_value(value),
        )

    @classmethod
    def _node_from_value(cls, value: NodeGraphNode | Mapping[str, object]) -> NodeGraphNode:
        if isinstance(value, NodeGraphNode):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph nodes must be NodeGraphNode instances or mappings")
        node_id = cls._text(value.get("id"), "node id")
        title = cls._text(value.get("title", value.get("label", node_id)), "node title")
        position = value.get("position", {})
        if position is None:
            position = {}
        if not isinstance(position, Mapping):
            raise TypeError("node position must be a mapping")
        return NodeGraphNode(
            id=node_id,
            title=title,
            x=cls._finite(value.get("x", position.get("x", 0.0)), "node x"),
            y=cls._finite(value.get("y", position.get("y", 0.0)), "node y"),
            inputs=cls._ports_from_value(value.get("inputs", ())),
            outputs=cls._ports_from_value(value.get("outputs", ())),
            subtitle=None if value.get("subtitle") is None else str(value.get("subtitle")),
            status=None if value.get("status") is None else str(value.get("status")),
            color=str(value.get("color", "#43c6ac")),
            width=cls._positive(value.get("width", value.get("width_hint", 190.0)), "node width"),
            data=cls._data_from_value(value),
        )

    @classmethod
    def _template_from_value(cls, value: NodeGraphTemplate | Mapping[str, object]) -> NodeGraphTemplate:
        if isinstance(value, NodeGraphTemplate):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph templates must be NodeGraphTemplate instances or mappings")
        template_id = cls._text(value.get("id"), "template id")
        title = cls._text(value.get("title", value.get("label", template_id)), "template title")
        return NodeGraphTemplate(
            id=template_id,
            title=title,
            inputs=cls._ports_from_value(value.get("inputs", ())),
            outputs=cls._ports_from_value(value.get("outputs", ())),
            subtitle=None if value.get("subtitle") is None else str(value.get("subtitle")),
            status=None if value.get("status") is None else str(value.get("status")),
            color=str(value.get("color", "#43c6ac")),
            width=cls._positive(value.get("width", value.get("width_hint", 190.0)), "template width"),
            data=cls._data_from_value(value),
        )

    @classmethod
    def _binding_target_from_value(cls, value: NodeGraphBindingTarget | Mapping[str, object]) -> NodeGraphBindingTarget:
        if isinstance(value, NodeGraphBindingTarget):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph binding targets must be NodeGraphBindingTarget instances or mappings")
        target_id = cls._text(value.get("id", value.get("target_id", value.get("widget_id", value.get("action_id")))), "binding target id")
        callback = value.get("callback")
        if callback is not None and not callable(callback):
            raise TypeError("binding target callback must be callable")
        return NodeGraphBindingTarget(
            id=target_id,
            label=None if value.get("label") is None else str(value.get("label")),
            target_type=None if value.get("target_type", value.get("type")) is None else str(value.get("target_type", value.get("type"))),
            widget_type=None if value.get("widget_type") is None else str(value.get("widget_type")),
            widget=value.get("widget"),
            action_type=None if value.get("action_type") is None else str(value.get("action_type")),
            callback=callback,
            supported_update_modes=cls._string_tuple(value.get("supported_update_modes", value.get("update_modes", ())), "binding target update modes"),
            default_update_mode=None if value.get("default_update_mode") is None else str(value.get("default_update_mode")),
            supported_port_profiles=cls._string_tuple(value.get("supported_port_profiles", value.get("port_profiles", ())), "binding target port profiles"),
            default_port_profile=None if value.get("default_port_profile") is None else str(value.get("default_port_profile")),
            supported_formats=cls._string_tuple(value.get("supported_formats", value.get("formats", ())), "binding target formats"),
            supported_commands=cls._string_tuple(value.get("supported_commands", value.get("commands", ())), "binding target commands"),
            default_command=None if value.get("default_command") is None else str(value.get("default_command")),
            data=cls._data_from_value(value),
        )

    @classmethod
    def _widget_target_from_value(cls, value: NodeGraphWidgetTarget | Mapping[str, object]) -> NodeGraphWidgetTarget:
        if isinstance(value, NodeGraphWidgetTarget):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph widget targets must be NodeGraphWidgetTarget instances or mappings")
        target_id = cls._text(value.get("id", value.get("widget_id")), "widget target id")
        return NodeGraphWidgetTarget(
            id=target_id,
            label=None if value.get("label") is None else str(value.get("label")),
            widget_type=None if value.get("widget_type", value.get("type")) is None else str(value.get("widget_type", value.get("type"))),
            supported_update_modes=cls._string_tuple(value.get("supported_update_modes", value.get("update_modes", ())), "widget target update modes"),
            default_update_mode=None if value.get("default_update_mode") is None else str(value.get("default_update_mode")),
            supported_port_profiles=cls._string_tuple(value.get("supported_port_profiles", value.get("port_profiles", ())), "widget target port profiles"),
            default_port_profile=None if value.get("default_port_profile") is None else str(value.get("default_port_profile")),
            supported_formats=cls._string_tuple(value.get("supported_formats", value.get("formats", ())), "widget target formats"),
            widget=value.get("widget"),
            data=cls._data_from_value(value),
        )

    @classmethod
    def _action_target_from_value(cls, value: NodeGraphActionTarget | Mapping[str, object]) -> NodeGraphActionTarget:
        if isinstance(value, NodeGraphActionTarget):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph action targets must be NodeGraphActionTarget instances or mappings")
        target_id = cls._text(value.get("id", value.get("action_id")), "action target id")
        return NodeGraphActionTarget(
            id=target_id,
            label=None if value.get("label") is None else str(value.get("label")),
            action_type=None if value.get("action_type", value.get("type")) is None else str(value.get("action_type", value.get("type"))),
            supported_commands=cls._string_tuple(value.get("supported_commands", value.get("commands", ())), "action target commands"),
            default_command=None if value.get("default_command") is None else str(value.get("default_command")),
            callback=value.get("callback"),
            data=cls._data_from_value(value),
        )

    @classmethod
    def _edge_from_value(cls, value: NodeGraphEdge | Mapping[str, object]) -> NodeGraphEdge:
        if isinstance(value, NodeGraphEdge):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph edges must be NodeGraphEdge instances or mappings")
        source = value.get("source", {})
        target = value.get("target", {})
        if source is None:
            source = {}
        if target is None:
            target = {}
        if not isinstance(source, Mapping):
            raise TypeError("edge source must be a mapping")
        if not isinstance(target, Mapping):
            raise TypeError("edge target must be a mapping")
        return NodeGraphEdge(
            source_node=cls._text(value.get("source_node", source.get("node")), "source_node"),
            source_port=cls._text(value.get("source_port", source.get("port")), "source_port"),
            target_node=cls._text(value.get("target_node", target.get("node")), "target_node"),
            target_port=cls._text(value.get("target_port", target.get("port")), "target_port"),
            label=None if value.get("label") is None else str(value.get("label")),
            color=str(value.get("color", "#43c6ac")),
            id=None if value.get("id") is None else cls._text(value.get("id"), "edge id"),
            data=cls._data_from_value(value),
        )

    @classmethod
    def _ports_from_value(cls, value: object) -> tuple[NodeGraphPort, ...]:
        if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
            raise TypeError("node ports must be a sequence")
        ports: list[NodeGraphPort] = []
        for item in value:
            if isinstance(item, NodeGraphPort):
                ports.append(item)
            elif isinstance(item, str):
                ports.append(NodeGraphPort(item, item))
            elif isinstance(item, Mapping):
                port_id = cls._text(item.get("id"), "port id")
                ports.append(
                    NodeGraphPort(
                        port_id,
                        None if item.get("label") is None else str(item.get("label")),
                        cls._data_from_value(item),
                        None if item.get("port_type", item.get("type")) is None else str(item.get("port_type", item.get("type"))),
                    )
                )
            else:
                raise TypeError("node ports must contain strings, mappings, or NodeGraphPort instances")
        return tuple(ports)

    @classmethod
    def _data_from_value(cls, value: Mapping[str, object]) -> dict[str, object] | None:
        custom_data = value.get("data", value.get("custom_data"))
        if custom_data is None:
            return None
        if not isinstance(custom_data, Mapping):
            raise TypeError("custom data must be a mapping")
        return _json_copy(dict(custom_data), "NodeGraph custom data")

    @classmethod
    def _runtime_policy_from_value(cls, value: object) -> str:
        policy = str(value or "auto").strip().lower()
        if policy not in _NODE_GRAPH_RUNTIME_POLICIES:
            allowed = ", ".join(sorted(_NODE_GRAPH_RUNTIME_POLICIES))
            raise ValueError(f"runtime_policy must be one of {allowed}")
        return policy

    @staticmethod
    def _string_tuple(value: object, name: str) -> tuple[str, ...]:
        if value is None:
            return ()
        if isinstance(value, str):
            return (value,) if value else ()
        if isinstance(value, (bytes, bytearray)) or not isinstance(value, Sequence):
            raise TypeError(f"{name} must be a sequence of strings")
        return tuple(str(item) for item in value)

    @staticmethod
    def _text(value: object, name: str) -> str:
        text = "" if value is None else str(value).strip()
        if not text:
            raise ValueError(f"{name} must be a non-empty string")
        return text

    @staticmethod
    def _finite(value: object, name: str) -> float:
        number = float(value)  # type: ignore[arg-type]
        if not math.isfinite(number):
            raise ValueError(f"{name} must be finite")
        return number

    @classmethod
    def _positive(cls, value: object, name: str) -> float:
        number = cls._finite(value, name)
        if number <= 0:
            raise ValueError(f"{name} must be positive")
        return number


def _node_graph_html(
    *,
    nodes: Sequence[NodeGraphNode],
    edges: Sequence[NodeGraphEdge],
    templates: Sequence[NodeGraphTemplate],
    sections: Sequence[NodeGraphSection],
    widget_targets: Sequence[NodeGraphWidgetTarget],
    action_targets: Sequence[NodeGraphActionTarget],
    selected_node: str | None,
    show_edge_labels: bool,
    show_port_labels: bool,
    show_status_labels: bool,
    show_subtitles: bool,
    enable_zoom: bool,
    show_editor_chrome: bool,
    editor_title: str,
    editor_actions: Sequence[Mapping[str, object]],
    emit_events: bool,
) -> str:
    config = {
        "nodes": [_node_payload(node) for node in nodes],
        "edges": [_edge_payload(edge, nodes) for edge in edges],
        "templates": [_template_payload(template) for template in templates],
        "sections": [_section_payload(section) for section in sections],
        "widgetTargets": [_widget_target_payload(target) for target in widget_targets],
        "actionTargets": [_action_target_payload(target) for target in action_targets],
        "portTypeColors": dict(_NODE_GRAPH_PORT_TYPE_COLORS),
        "portTypeConversions": {f"{source}->{target}": conversion for (source, target), conversion in _NODE_GRAPH_PORT_TYPE_CONVERSIONS.items()},
        "widgetSinkPortProfiles": list(_NODE_GRAPH_WIDGET_SINK_PORT_PROFILES),
        "widgetSourcePortProfiles": list(_NODE_GRAPH_WIDGET_SOURCE_PORT_PROFILES),
        "selectedNode": selected_node,
        "showEdgeLabels": bool(show_edge_labels),
        "showPortLabels": bool(show_port_labels),
        "showStatusLabels": bool(show_status_labels),
        "showSubtitles": bool(show_subtitles),
        "enableZoom": bool(enable_zoom),
        "showEditorChrome": bool(show_editor_chrome),
        "editorTitle": str(editor_title),
        "editorActions": [_editor_action_payload(action) for action in editor_actions],
        "emitEvents": bool(emit_events),
    }
    payload = json.dumps(config)
    chrome_html = """
  <div id=\"editorChrome\" class=\"editor-shell\">
    <div class=\"editor-topbar\">
      <div class=\"editor-title\" id=\"editorTitle\"></div>
      <div class=\"editor-state\" id=\"editorState\">Ready</div>
      <div class=\"editor-tools\" id=\"editorTools\"></div>
    </div>
    <canvas id=\"graph\" tabindex=\"0\"></canvas>
    <div class=\"editor-statusbar\">
      <div class=\"status-item\" id=\"selectedStatus\">Selected: none</div>
      <div class=\"status-item\" id=\"viewportStatus\">Viewport: loading</div>
      <div class=\"status-item\" id=\"graphStatus\">Graph: loading</div>
    </div>
  </div>
""" if show_editor_chrome else "  <canvas id=\"graph\" tabindex=\"0\"></canvas>"
    return f"""<!doctype html>
<html>
<head>
  <meta charset=\"utf-8\" />
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
  <style>
    html, body {{ width: 100%; height: 100%; margin: 0; overflow: hidden; background: #0d1117; color: rgba(245, 248, 252, 0.94); font-family: Segoe UI, Arial, sans-serif; }}
    canvas {{ width: 100%; height: 100%; min-height: 0; display: block; background: #0d1117; cursor: default; flex: 1 1 auto; }}
    .editor-shell {{ width: 100%; height: 100%; min-width: 0; min-height: 0; display: flex; flex-direction: column; background: #0d1117; }}
    .editor-topbar {{ height: 42px; flex: 0 0 auto; display: flex; align-items: center; gap: 8px; padding: 5px 7px; background: #111821; border-bottom: 1px solid rgba(255, 255, 255, 0.09); box-sizing: border-box; }}
    .editor-title {{ flex: 0 0 auto; color: #ffffff; font-size: 15px; font-weight: 850; padding: 0 4px; }}
    .editor-state {{ flex: 1 1 auto; min-width: 0; color: rgba(226, 255, 248, 0.88); font-size: 12px; font-weight: 750; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }}
    .editor-tools {{ flex: 0 0 auto; display: flex; align-items: center; gap: 5px; }}
    .tool-button {{ width: 31px; height: 31px; border-radius: 7px; border: 1px solid rgba(255, 255, 255, 0.13); background: rgba(255, 255, 255, 0.055); color: rgba(247, 250, 255, 0.94); font-size: 14px; font-weight: 850; line-height: 1; padding: 0; cursor: default; }}
    .tool-button:hover {{ background: rgba(255, 255, 255, 0.095); border-color: rgba(255, 255, 255, 0.22); }}
    .tool-button.wide {{ width: auto; min-width: 58px; padding: 0 10px; font-size: 12px; }}
    .tool-button.primary {{ width: auto; min-width: 74px; padding: 0 10px; background: rgba(67, 198, 172, 0.18); border-color: rgba(67, 198, 172, 0.44); font-size: 12px; }}
    .tool-separator {{ width: 1px; height: 22px; background: rgba(255, 255, 255, 0.16); margin: 0 2px; }}
    .editor-statusbar {{ height: 29px; flex: 0 0 auto; display: flex; align-items: center; gap: 2px; padding: 2px 6px; box-sizing: border-box; background: #101721; border-top: 1px solid rgba(255, 255, 255, 0.08); }}
    .status-item {{ flex: 1 1 0; min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; color: rgba(226, 235, 246, 0.74); font-size: 12px; font-weight: 650; padding: 3px 6px; }}
  </style>
</head>
<body>
{chrome_html}
  <script>
    const config = {payload};
    const canvas = document.getElementById('graph');
    const editorTitleEl = document.getElementById('editorTitle');
    const editorStateEl = document.getElementById('editorState');
    const editorToolsEl = document.getElementById('editorTools');
    const selectedStatusEl = document.getElementById('selectedStatus');
    const viewportStatusEl = document.getElementById('viewportStatus');
    const graphStatusEl = document.getElementById('graphStatus');
    const ctx = canvas.getContext('2d');

    function focusCanvas() {{
      try {{ canvas.focus({{ preventScroll: true }}); }}
      catch (error) {{ canvas.focus(); }}
    }}
    const state = {{ nodes: config.nodes, edges: config.edges, sections: config.sections || [], selected: config.selectedNode, selectedEdge: null, selectedSection: null, viewX: 34, viewY: 32, zoom: 1, drag: null, hoverPort: null, showGrid: true }};
    let nodeSerial = state.nodes.length;
    let edgeSerial = state.edges.length;
    let sectionSerial = state.sections.length;
    for (const edge of state.edges) if (!edge.id) edge.id = `edge-${{++edgeSerial}}`;
    for (const section of state.sections) {{
      const match = String(section.id || '').match(/(\\d+)$/);
      if (match) sectionSerial = Math.max(sectionSerial, Number(match[1]));
    }}
    const history = {{ undo: [], redo: [], initial: null }};
    const HEADER = 36;
    const HEADER_PAD = 10;
    const PALETTE_X = 12;
    const PALETTE_Y = 10;
    const PALETTE_H = 28;
    const palette = {{ selected: config.templates[0] ? config.templates[0].id : null, items: [] }};
    const nodePicker = {{ open: false, x: 0, y: 0, graphX: 0, graphY: 0, query: '', selected: 0, scroll: 0, scrollDrag: null, rect: null, listRect: null, scrollBar: null, items: [] }};
    const renameEditor = {{ open: false, kind: null, id: null, value: '', original: '', selectAll: false, rect: null, buttons: [] }};
    const propertyEditor = {{ open: false, kind: null, id: null, fields: [], active: 0, scroll: 0, scrollDrag: null, rect: null, listRect: null, scrollBar: null, buttons: [], selectPopup: null }};
    const textEdit = {{ key: null, owner: null, prop: null, caret: 0, anchor: 0, dragging: false, font: '12px Segoe UI' }};
    const TOOLBAR_Y = 10;
    const TOOLBAR_H = 28;
    const TOOLBAR_W = 34;
    const toolbar = {{ items: [] }};

    function selectedStatusText() {{
      if (state.selected) return `Selected: ${{state.selected}}`;
      if (state.selectedEdge) return `Selected: edge ${{state.selectedEdge}}`;
      if (state.selectedSection) return `Selected: section ${{state.selectedSection}}`;
      return 'Selected: none';
    }}

    function updateEditorChromeStatus(message = null) {{
      if (!config.showEditorChrome) return;
      if (editorTitleEl) editorTitleEl.textContent = config.editorTitle || 'NodeGraph';
      if (editorStateEl && message) editorStateEl.textContent = message;
      if (selectedStatusEl) selectedStatusEl.textContent = selectedStatusText();
      if (viewportStatusEl) viewportStatusEl.textContent = `Viewport: x=${{state.viewX.toFixed(1)}} y=${{state.viewY.toFixed(1)}} zoom=${{state.zoom.toFixed(2)}}`;
      if (graphStatusEl) graphStatusEl.textContent = `Graph: ${{state.nodes.length}} nodes / ${{state.edges.length}} edges`;
    }}

    function editorToolLabel(action) {{
      const icon = String(action.icon || '').trim();
      if (icon) return icon;
      const id = String(action.id || action.action || '');
      if (id === 'add') return '+';
      if (id === 'rename') return 'E';
      if (id === 'undo') return 'U';
      if (id === 'redo') return 'R';
      if (id === 'fit') return 'Fit';
      if (id === 'snapshot') return 'S';
      if (id === 'run_section') return 'Run';
      if (id === 'stop_terminal') return 'Stop';
      if (id === 'cleanup_runtime') return 'X';
      return String(action.label || id || '?').slice(0, 4);
    }}

    function runEditorAction(id) {{
      if (id === 'add') {{
        const rect = canvas.getBoundingClientRect();
        const sx = rect.width * 0.5;
        const sy = rect.height * 0.5;
        openNodePicker({{ sx, sy, x: (sx - state.viewX) / state.zoom, y: (sy - state.viewY) / state.zoom }});
        updateEditorChromeStatus('Choose node');
        return;
      }}
      if (id === 'rename') {{
        if (!editSelectedNodeTitle()) editSelectedSectionTitle();
        updateEditorChromeStatus('Rename');
        return;
      }}
      if (id === 'undo') {{
        undoGraph();
        updateEditorChromeStatus('Undo');
        return;
      }}
      if (id === 'redo') {{
        redoGraph();
        updateEditorChromeStatus('Redo');
        return;
      }}
      if (id === 'fit') {{
        fitToView();
        updateEditorChromeStatus('Fit graph');
        return;
      }}
      emitGraphEvent({{ event: 'editor_action', action: id }});
      updateEditorChromeStatus(String(id).replace(/_/g, ' '));
    }}

    function setupEditorChrome() {{
      if (!config.showEditorChrome || !editorToolsEl) return;
      const defaults = [
        {{ id: 'add', label: 'Add node', icon: '+' }},
        {{ id: 'rename', label: 'Rename selected', icon: 'E' }},
        {{ separator: true }},
        {{ id: 'undo', label: 'Undo', icon: 'U' }},
        {{ id: 'redo', label: 'Redo', icon: 'R' }},
        {{ id: 'fit', label: 'Fit graph', icon: 'Fit' }},
      ];
      const actions = [...defaults, ...(config.editorActions || [])];
      editorToolsEl.replaceChildren();
      for (const action of actions) {{
        if (action.separator) {{
          const sep = document.createElement('div');
          sep.className = 'tool-separator';
          editorToolsEl.appendChild(sep);
          continue;
        }}
        const id = String(action.id || action.action || '').trim();
        if (!id) continue;
        if (action.separator_before) {{
          const sep = document.createElement('div');
          sep.className = 'tool-separator';
          editorToolsEl.appendChild(sep);
        }}
        const button = document.createElement('button');
        button.type = 'button';
        button.className = action.primary ? 'tool-button primary' : (action.wide ? 'tool-button wide' : 'tool-button');
        button.textContent = editorToolLabel(action);
        button.title = String(action.tooltip || action.label || id);
        button.addEventListener('click', event => {{
          event.preventDefault();
          runEditorAction(id);
          focusCanvas();
        }});
        editorToolsEl.appendChild(button);
      }}
      updateEditorChromeStatus('Ready');
    }}

    function resize() {{
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      canvas.width = Math.max(1, Math.floor(rect.width * dpr));
      canvas.height = Math.max(1, Math.floor(rect.height * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      draw();
    }}

    function graphPoint(event) {{
      const rect = canvas.getBoundingClientRect();
      const sx = event.clientX - rect.left;
      const sy = event.clientY - rect.top;
      return {{ sx, sy, x: (sx - state.viewX) / state.zoom, y: (sy - state.viewY) / state.zoom }};
    }}

    function selectedTemplate() {{
      return config.templates.find(template => template.id === palette.selected) || config.templates[0] || null;
    }}

    function templateLabel(template) {{
      return String(template.title || template.id || 'Node');
    }}

    function hitPalette(sx, sy) {{
      return null;
    }}

    function hitToolbar(sx, sy) {{
      for (const item of toolbar.items) {{
        if (sx >= item.x && sx <= item.x + item.w && sy >= item.y && sy <= item.y + item.h) return item.action;
      }}
      return null;
    }}

    function templateSearchText(template) {{
      const parts = [template.id, template.title, template.subtitle, template.status];
      for (const port of [...(template.inputs || []), ...(template.outputs || [])]) parts.push(port.id, port.label, port.port_type || port.type);
      if (template.data) for (const value of Object.values(template.data)) parts.push(value);
      return parts.filter(value => value !== undefined && value !== null).join(' ').toLowerCase();
    }}

    function nodePickerItems() {{
      const query = nodePicker.query.trim().toLowerCase();
      const items = query
        ? config.templates.filter(template => templateSearchText(template).includes(query))
        : config.templates.slice();
      return items;
    }}

    function clampNodePicker() {{
      const rect = canvas.getBoundingClientRect();
      const width = Math.min(420, Math.max(280, rect.width - 24));
      const maxRows = Math.max(1, Math.min(8, nodePickerItems().length || 1));
      const height = 72 + maxRows * 44 + 14;
      nodePicker.x = Math.max(12, Math.min(nodePicker.x, rect.width - width - 12));
      nodePicker.y = Math.max(12, Math.min(nodePicker.y, rect.height - Math.min(height, rect.height - 24) - 12));
      nodePicker.rect = {{ x: nodePicker.x, y: nodePicker.y, w: width, h: Math.min(height, rect.height - 24) }};
      clampNodePickerScroll();
    }}

    function nodePickerListHeight() {{
      return nodePicker.rect ? Math.max(40, nodePicker.rect.h - 96) : 40;
    }}

    function nodePickerContentHeight(items = nodePickerItems()) {{
      return Math.max(0, items.length * 44 - 4);
    }}

    function clampNodePickerScroll(items = nodePickerItems()) {{
      const maxScroll = Math.max(0, nodePickerContentHeight(items) - nodePickerListHeight());
      nodePicker.scroll = Math.max(0, Math.min(nodePicker.scroll || 0, maxScroll));
      return maxScroll;
    }}

    function ensureNodePickerSelectionVisible(items = nodePickerItems()) {{
      if (!nodePicker.rect || !items.length) return;
      const listH = nodePickerListHeight();
      const rowTop = nodePicker.selected * 44;
      const rowBottom = rowTop + 40;
      if (rowTop < nodePicker.scroll) nodePicker.scroll = rowTop;
      else if (rowBottom > nodePicker.scroll + listH) nodePicker.scroll = rowBottom - listH;
      clampNodePickerScroll(items);
    }}

    function openNodePicker(point) {{
      if (!config.templates.length) {{
        addNode(point.x, point.y);
        draw();
        return;
      }}
      closeRenameEditor(false);
      nodePicker.open = true;
      nodePicker.x = point.sx + 10;
      nodePicker.y = point.sy + 10;
      nodePicker.graphX = point.x;
      nodePicker.graphY = point.y;
      nodePicker.query = '';
      nodePicker.scroll = 0;
      nodePicker.selected = Math.max(0, config.templates.findIndex(template => template.id === palette.selected));
      setTextEditTarget('nodePicker.query', nodePicker, 'query', false, '13px Segoe UI');
      clampNodePicker();
      emitGraphEvent({{ event: 'node_picker_opened', position: {{ x: point.x, y: point.y }}, template: palette.selected || null }});
      draw();
    }}

    function closeNodePicker(notify = true) {{
      if (!nodePicker.open) return;
      nodePicker.open = false;
      nodePicker.scrollDrag = null;
      nodePicker.items = [];
      if (textEdit.key === 'nodePicker.query') clearTextEditTarget();
      if (notify) emitGraphEvent({{ event: 'node_picker_closed' }});
      draw();
    }}

    function clearTextEditTarget() {{
      textEdit.key = null;
      textEdit.owner = null;
      textEdit.prop = null;
      textEdit.caret = 0;
      textEdit.anchor = 0;
      textEdit.dragging = false;
    }}

    function textEditValue() {{
      return textEdit.owner && textEdit.prop ? String(textEdit.owner[textEdit.prop] || '') : '';
    }}

    function setTextEditValue(value) {{
      if (!textEdit.owner || !textEdit.prop) return;
      textEdit.owner[textEdit.prop] = String(value || '');
      if (textEdit.key === 'nodePicker.query') {{
        nodePicker.selected = 0;
        nodePicker.scroll = 0;
      }}
    }}

    function setTextEditTarget(key, owner, prop, selectAll = false, font = '12px Segoe UI') {{
      const value = String(owner && prop ? owner[prop] || '' : '');
      if (textEdit.key !== key || textEdit.owner !== owner || textEdit.prop !== prop) {{
        textEdit.key = key;
        textEdit.owner = owner;
        textEdit.prop = prop;
        textEdit.font = font;
        textEdit.caret = value.length;
        textEdit.anchor = selectAll ? 0 : value.length;
      }} else if (selectAll) {{
        textEdit.caret = value.length;
        textEdit.anchor = 0;
      }}
      textEdit.dragging = false;
    }}

    function textSelectionRange() {{
      const a = Math.max(0, Math.min(textEdit.anchor, textEditValue().length));
      const b = Math.max(0, Math.min(textEdit.caret, textEditValue().length));
      return {{ start: Math.min(a, b), end: Math.max(a, b) }};
    }}

    function hasTextSelection() {{
      const range = textSelectionRange();
      return range.end > range.start;
    }}

    function setTextCaret(index, extend = false) {{
      const value = textEditValue();
      const next = Math.max(0, Math.min(Number(index) || 0, value.length));
      textEdit.caret = next;
      if (!extend) textEdit.anchor = next;
    }}

    function deleteTextSelection() {{
      if (!hasTextSelection()) return false;
      const value = textEditValue();
      const range = textSelectionRange();
      setTextEditValue(value.slice(0, range.start) + value.slice(range.end));
      setTextCaret(range.start);
      return true;
    }}

    function insertTextAtCaret(text) {{
      if (!textEdit.owner || !textEdit.prop) return false;
      const clean = String(text || '').replace(/\\r\\n?/g, '\\n').replace(/\\n/g, ' ');
      deleteTextSelection();
      const value = textEditValue();
      const before = value.slice(0, textEdit.caret);
      const after = value.slice(textEdit.caret);
      setTextEditValue(before + clean + after);
      setTextCaret(before.length + clean.length);
      return true;
    }}

    function deleteTextBackward() {{
      if (!textEdit.owner || !textEdit.prop) return false;
      if (deleteTextSelection()) return true;
      if (textEdit.caret <= 0) return false;
      const value = textEditValue();
      setTextEditValue(value.slice(0, textEdit.caret - 1) + value.slice(textEdit.caret));
      setTextCaret(textEdit.caret - 1);
      return true;
    }}

    function deleteTextForward() {{
      if (!textEdit.owner || !textEdit.prop) return false;
      if (deleteTextSelection()) return true;
      const value = textEditValue();
      if (textEdit.caret >= value.length) return false;
      setTextEditValue(value.slice(0, textEdit.caret) + value.slice(textEdit.caret + 1));
      setTextCaret(textEdit.caret);
      return true;
    }}

    function moveTextCaret(delta, extend = false) {{
      setTextCaret(textEdit.caret + delta, extend);
    }}

    function selectedTextValue() {{
      if (!hasTextSelection()) return '';
      const value = textEditValue();
      const range = textSelectionRange();
      return value.slice(range.start, range.end);
    }}

    function writeClipboardText(text) {{
      if (!text) return;
      if (navigator.clipboard && navigator.clipboard.writeText) {{
        navigator.clipboard.writeText(text).catch(() => {{}});
      }}
    }}

    function pasteClipboardText() {{
      if (navigator.clipboard && navigator.clipboard.readText) {{
        navigator.clipboard.readText().then(text => {{
          if (textEdit.owner && text) {{
            insertTextAtCaret(text);
            draw();
          }}
        }}).catch(() => {{}});
      }}
    }}

    function textIndexFromX(text, localX, font) {{
      const value = String(text || '');
      ctx.save();
      ctx.font = font || textEdit.font || '12px Segoe UI';
      let best = value.length;
      for (let index = 0; index <= value.length; index++) {{
        const left = ctx.measureText(value.slice(0, index)).width;
        const right = index < value.length ? ctx.measureText(value.slice(0, index + 1)).width : left;
        const midpoint = (left + right) / 2;
        if (localX <= midpoint) {{
          best = index;
          break;
        }}
      }}
      ctx.restore();
      return best;
    }}

    function caretXForText(text, index, font) {{
      ctx.save();
      ctx.font = font || textEdit.font || '12px Segoe UI';
      const width = ctx.measureText(String(text || '').slice(0, Math.max(0, index))).width;
      ctx.restore();
      return width;
    }}

    function drawEditableText(text, placeholder, rect, key, font = '12px Segoe UI') {{
      const value = String(text || '');
      const active = textEdit.key === key;
      ctx.save();
      ctx.beginPath();
      ctx.rect(rect.x, rect.y, rect.w, rect.h);
      ctx.clip();
      ctx.font = font;
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      const textX = rect.x + 2;
      const textY = rect.y + rect.h / 2;
      if (active && hasTextSelection()) {{
        const range = textSelectionRange();
        const startX = textX + caretXForText(value, range.start, font);
        const endX = textX + caretXForText(value, range.end, font);
        ctx.fillStyle = 'rgba(67, 198, 172, 0.32)';
        roundedRect(startX - 1, rect.y + 4, Math.max(2, endX - startX + 2), rect.h - 8, 4);
        ctx.fill();
      }}
      ctx.fillStyle = value ? '#eef4ff' : '#738296';
      ctx.fillText(value || placeholder || '', textX, textY);
      if (active && Math.floor(Date.now() / 530) % 2 === 0) {{
        const caretX = textX + caretXForText(value, textEdit.caret, font);
        ctx.strokeStyle = '#eef4ff';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(caretX, rect.y + 7);
        ctx.lineTo(caretX, rect.y + rect.h - 7);
        ctx.stroke();
      }}
      ctx.restore();
    }}

    function pointInRect(sx, sy, rect) {{
      return !!rect && sx >= rect.x && sx <= rect.x + rect.w && sy >= rect.y && sy <= rect.y + rect.h;
    }}

    function beginTextDrag(key, owner, prop, rect, sx, sy, font = '12px Segoe UI', selectAll = false) {{
      if (!pointInRect(sx, sy, rect)) return false;
      setTextEditTarget(key, owner, prop, selectAll, font);
      const index = textIndexFromX(textEditValue(), sx - rect.x - 2, font);
      setTextCaret(index, false);
      textEdit.dragging = true;
      draw();
      return true;
    }}

    function updateTextDrag(sx, sy) {{
      if (!textEdit.dragging || !textEdit.owner || !textEdit.prop) return false;
      let rect = null;
      let font = textEdit.font || '12px Segoe UI';
      if (textEdit.key === 'nodePicker.query') rect = nodePicker.inputRect;
      else if (textEdit.key === 'renameEditor.value') rect = renameEditor.inputRect;
      else if (textEdit.key && textEdit.key.startsWith('propertyEditor.')) {{
        const field = activePropertyField();
        rect = field ? field.rect : null;
      }}
      if (!rect) return false;
      const index = textIndexFromX(textEditValue(), sx - rect.x - 2, font);
      setTextCaret(index, true);
      draw();
      return true;
    }}

    function chooseNodePickerSelection(index = nodePicker.selected) {{
      const items = nodePickerItems();
      if (!items.length) return false;
      const bounded = Math.max(0, Math.min(index, items.length - 1));
      const template = items[bounded];
      palette.selected = template.id;
      addNode(nodePicker.graphX, nodePicker.graphY, template);
      emitGraphEvent({{ event: 'node_picker_selected', template: template.id, position: {{ x: nodePicker.graphX, y: nodePicker.graphY }} }});
      closeNodePicker(false);
      draw();
      return true;
    }}

    function hitNodePicker(sx, sy) {{
      if (!nodePicker.open || !nodePicker.rect) return null;
      const rect = nodePicker.rect;
      if (sx < rect.x || sx > rect.x + rect.w || sy < rect.y || sy > rect.y + rect.h) return {{ kind: 'outside' }};
      const closeRect = {{ x: rect.x + rect.w - 34, y: rect.y + 10, w: 22, h: 22 }};
      if (sx >= closeRect.x && sx <= closeRect.x + closeRect.w && sy >= closeRect.y && sy <= closeRect.y + closeRect.h) return {{ kind: 'close' }};
      if (nodePicker.scrollBar) {{
        const bar = nodePicker.scrollBar;
        if (sx >= bar.x - 5 && sx <= bar.x + bar.w + 5 && sy >= bar.y && sy <= bar.y + bar.h) {{
          return {{ kind: 'scrollbar', onThumb: sy >= bar.thumbY && sy <= bar.thumbY + bar.thumbH }};
        }}
      }}
      for (const item of nodePicker.items) {{
        if (sx >= item.x && sx <= item.x + item.w && sy >= item.y && sy <= item.y + item.h) return {{ kind: 'item', index: item.index, template: item.template }};
      }}
      return {{ kind: 'inside' }};
    }}
    function hitRenameEditor(sx, sy) {{
      if (!renameEditor.open || !renameEditor.rect) return null;
      const rect = renameEditor.rect;
      for (const button of renameEditor.buttons) {{
        if (sx >= button.x && sx <= button.x + button.w && sy >= button.y && sy <= button.y + button.h) return {{ kind: button.action }};
      }}
      if (sx >= rect.x && sx <= rect.x + rect.w && sy >= rect.y && sy <= rect.y + rect.h) return {{ kind: 'inside' }};
      return {{ kind: 'outside' }};
    }}

    function hitPropertyEditor(sx, sy) {{
      if (!propertyEditor.open || !propertyEditor.rect) return null;
      const rect = propertyEditor.rect;
      for (const button of propertyEditor.buttons) {{
        if (sx >= button.x && sx <= button.x + button.w && sy >= button.y && sy <= button.y + button.h) return {{ kind: button.action }};
      }}
      if (propertyEditor.selectPopup && Array.isArray(propertyEditor.selectPopup.items)) {{
        const popup = propertyEditor.selectPopup;
        if (popup.scrollBar) {{
          const bar = popup.scrollBar;
          if (sx >= bar.x - 5 && sx <= bar.x + bar.w + 5 && sy >= bar.y && sy <= bar.y + bar.h) {{
            return {{ kind: 'select_scrollbar', onThumb: sy >= bar.thumbY && sy <= bar.thumbY + bar.thumbH }};
          }}
        }}
        for (const item of propertyEditor.selectPopup.items) {{
          if (sx >= item.x && sx <= item.x + item.w && sy >= item.y && sy <= item.y + item.h) {{
            return {{ kind: 'select_option', index: item.index, value: item.value }};
          }}
        }}
        if (popup.rect && sx >= popup.rect.x && sx <= popup.rect.x + popup.rect.w && sy >= popup.rect.y && sy <= popup.rect.y + popup.rect.h) {{
          return {{ kind: 'select_popup' }};
        }}
      }}
      if (propertyEditor.scrollBar) {{
        const bar = propertyEditor.scrollBar;
        if (sx >= bar.x - 5 && sx <= bar.x + bar.w + 5 && sy >= bar.y && sy <= bar.y + bar.h) {{
          return {{ kind: 'scrollbar', onThumb: sy >= bar.thumbY && sy <= bar.thumbY + bar.thumbH }};
        }}
      }}
      for (let index = 0; index < propertyEditor.fields.length; index++) {{
        const field = propertyEditor.fields[index];
        if (Array.isArray(field.stepperButtons)) {{
          for (const button of field.stepperButtons) {{
            if (sx >= button.x && sx <= button.x + button.w && sy >= button.y && sy <= button.y + button.h) {{
              return {{ kind: 'field_stepper', index, delta: button.delta }};
            }}
          }}
        }}
        if (field.rect && sx >= field.rect.x && sx <= field.rect.x + field.rect.w && sy >= field.rect.y && sy <= field.rect.y + field.rect.h) return {{ kind: 'field', index }};
      }}
      if (sx >= rect.x && sx <= rect.x + rect.w && sy >= rect.y && sy <= rect.y + rect.h) return {{ kind: 'inside' }};
      return {{ kind: 'outside' }};
    }}
    const PORT_STEP = 22;
    const PORT_TOP_PAD = 22;
    const PORT_BOTTOM_PAD = 22;
    const SUBTITLE_SPACE = 18;

    function portTop(node) {{
      return HEADER + PORT_TOP_PAD + (config.showSubtitles && node.subtitle ? SUBTITLE_SPACE : 0);
    }}

    function nodeHeight(node) {{
      const ports = Math.max(node.inputs.length, node.outputs.length, 1);
      return portTop(node) + Math.max(0, ports - 1) * PORT_STEP + PORT_BOTTOM_PAD;
    }}

    function textWidth(text, size, minSize = 0) {{
      ctx.save();
      ctx.font = `${{Math.max(minSize, size * state.zoom)}}px Segoe UI`;
      const width = ctx.measureText(String(text || '')).width / state.zoom;
      ctx.restore();
      return width;
    }}

    function maxPortLabelWidth(ports) {{
      let width = 0;
      for (const port of ports) width = Math.max(width, textWidth(port.label || port.id, 10.5, 9));
      return width;
    }}

    function nodeWidth(node) {{
      let width = Math.max(node.width || 190, textWidth(node.title, 13, 10) + 32);
      if (config.showStatusLabels && node.status) width = Math.max(width, textWidth(node.title, 13, 10) + textWidth(node.status, 10, 9) + 48);
      if (config.showSubtitles && node.subtitle) width = Math.max(width, textWidth(node.subtitle, 10.5, 9) + 28);
      if (config.showPortLabels) {{
        const inputWidth = maxPortLabelWidth(node.inputs);
        const outputWidth = maxPortLabelWidth(node.outputs);
        const bothSides = inputWidth > 0 && outputWidth > 0;
        const sidePadding = 48;
        const labelGutter = 34;
        width = Math.max(width, bothSides ? inputWidth + outputWidth + sidePadding + labelGutter : Math.max(inputWidth, outputWidth) + sidePadding);
      }}
      return Math.ceil(width);
    }}

    function screen(x, y) {{ return {{ x: state.viewX + x * state.zoom, y: state.viewY + y * state.zoom }}; }}

    function hitHeader(x, y) {{
      for (let i = state.nodes.length - 1; i >= 0; i--) {{
        const node = state.nodes[i];
        if (x >= node.x - HEADER_PAD && x <= node.x + nodeWidth(node) + HEADER_PAD && y >= node.y - HEADER_PAD && y <= node.y + HEADER + HEADER_PAD) return node;
      }}
      return null;
    }}

    function hitNode(x, y) {{
      for (let i = state.nodes.length - 1; i >= 0; i--) {{
        const node = state.nodes[i];
        if (x >= node.x && x <= node.x + nodeWidth(node) && y >= node.y && y <= node.y + nodeHeight(node)) return node;
      }}
      return null;
    }}

    function portPoint(nodeId, portId, side) {{
      const node = state.nodes.find(n => n.id === nodeId);
      if (!node) return null;
      const ports = side === 'input' ? node.inputs : node.outputs;
      const index = ports.findIndex(p => p.id === portId);
      if (index < 0) return null;
      return screen(side === 'input' ? node.x : node.x + nodeWidth(node), node.y + portTop(node) + index * PORT_STEP);
    }}
    function allPorts(node, side) {{ return side === 'input' ? node.inputs : node.outputs; }}

    function hitPort(sx, sy, sideFilter = null) {{
      const sides = sideFilter ? [sideFilter] : ['output', 'input'];
      for (let i = state.nodes.length - 1; i >= 0; i--) {{
        const node = state.nodes[i];
        for (const side of sides) {{
          for (const port of allPorts(node, side)) {{
            const point = portPoint(node.id, port.id, side);
            if (!point) continue;
            const radius = Math.max(9, 7 * state.zoom);
            if (Math.hypot(sx - point.x, sy - point.y) <= radius) return {{ node, port, side, point }};
          }}
        }}
      }}
      return null;
    }}

    function bezierPoint(a, b, dx, t) {{
      const mt = 1 - t;
      const x = mt * mt * mt * a.x + 3 * mt * mt * t * (a.x + dx) + 3 * mt * t * t * (b.x - dx) + t * t * t * b.x;
      const y = mt * mt * mt * a.y + 3 * mt * mt * t * a.y + 3 * mt * t * t * b.y + t * t * t * b.y;
      return {{ x, y }};
    }}

    function distanceToSegment(p, a, b) {{
      const vx = b.x - a.x;
      const vy = b.y - a.y;
      const wx = p.x - a.x;
      const wy = p.y - a.y;
      const lengthSq = vx * vx + vy * vy;
      const t = lengthSq === 0 ? 0 : Math.max(0, Math.min(1, (wx * vx + wy * vy) / lengthSq));
      const x = a.x + t * vx;
      const y = a.y + t * vy;
      return Math.hypot(p.x - x, p.y - y);
    }}

    function cleanWaypoints(value) {{
      if (!Array.isArray(value)) return [];
      return value
        .map(point => ({{ x: Number(point && point.x), y: Number(point && point.y) }}))
        .filter(point => Number.isFinite(point.x) && Number.isFinite(point.y));
    }}

    function edgeWaypoints(edge) {{
      return cleanWaypoints(edge && edge.data ? edge.data.waypoints : null);
    }}

    function setEdgeWaypoints(edge, waypoints) {{
      if (!edge) return [];
      const cleaned = cleanWaypoints(waypoints);
      const data = edge.data ? {{ ...edge.data }} : {{}};
      if (cleaned.length) data.waypoints = cleaned;
      else delete data.waypoints;
      edge.data = Object.keys(data).length ? data : null;
      return cleaned;
    }}

    function cloneEdgeData(data) {{
      if (!data) return null;
      const copy = {{ ...data }};
      if (Array.isArray(data.waypoints)) copy.waypoints = cleanWaypoints(data.waypoints);
      return Object.keys(copy).length ? copy : null;
    }}

    function edgePoints(edge) {{
      const a = portPoint(edge.sourceNode, edge.sourcePort, 'output');
      const b = portPoint(edge.targetNode, edge.targetPort, 'input');
      if (!a || !b) return null;
      const waypoints = edgeWaypoints(edge);
      const route = [a].concat(waypoints.map(point => screen(point.x, point.y))).concat([b]);
      return {{ a, b, waypoints, route }};
    }}

    function edgeSegmentDx(a, b) {{
      return Math.max(24, Math.abs(b.x - a.x) * 0.45);
    }}

    function hitEdgeSegment(sx, sy) {{
      const p = {{ x: sx, y: sy }};
      for (let i = state.edges.length - 1; i >= 0; i--) {{
        const edge = state.edges[i];
        const points = edgePoints(edge);
        if (!points) continue;
        for (let segment = 0; segment < points.route.length - 1; segment++) {{
          const a = points.route[segment];
          const b = points.route[segment + 1];
          const dx = edgeSegmentDx(a, b);
          let prev = a;
          for (let step = 1; step <= 18; step++) {{
            const next = bezierPoint(a, b, dx, step / 18);
            if (distanceToSegment(p, prev, next) <= 7) return {{ edge, segment }};
            prev = next;
          }}
        }}
      }}
      return null;
    }}

    function hitEdge(sx, sy) {{
      const hit = hitEdgeSegment(sx, sy);
      return hit ? hit.edge : null;
    }}

    function hitEdgeWaypoint(sx, sy) {{
      const radius = Math.max(8, 7 * state.zoom);
      for (let edgeIndex = state.edges.length - 1; edgeIndex >= 0; edgeIndex--) {{
        const edge = state.edges[edgeIndex];
        const waypoints = edgeWaypoints(edge);
        for (let index = waypoints.length - 1; index >= 0; index--) {{
          const point = screen(waypoints[index].x, waypoints[index].y);
          if (Math.hypot(sx - point.x, sy - point.y) <= radius) return {{ edge, index, point }};
        }}
      }}
      return null;
    }}

    function addEdgeWaypoint(edge, segment, x, y) {{
      if (!edge) return false;
      const before = graphSnapshot();
      const waypoints = edgeWaypoints(edge);
      const insertAt = Math.max(0, Math.min(Number(segment) || 0, waypoints.length));
      waypoints.splice(insertAt, 0, {{ x, y }});
      setEdgeWaypoints(edge, waypoints);
      state.selected = null;
      state.selectedEdge = edge.id;
      state.selectedSection = null;
      emitGraphMutation({{ event: 'edge_waypoints_changed', edge: edgeEventPayload(edge) }}, before);
      draw();
      return true;
    }}

    function portType(port) {{
      return port ? (port.port_type || port.type || null) : null;
    }}

    function portTypeColor(type, fallback = '#43c6ac') {{
      if (!type) return fallback;
      return (config.portTypeColors && config.portTypeColors[String(type)]) || fallback;
    }}

    function nodePort(nodeId, portId, side) {{
      const node = state.nodes.find(candidate => candidate.id === nodeId);
      if (!node) return null;
      const ports = side === 'input' ? node.inputs : node.outputs;
      return (ports || []).find(port => port.id === portId) || null;
    }}

    function edgeDataType(edge) {{
      const source = nodePort(edge.sourceNode, edge.sourcePort, 'output');
      const target = nodePort(edge.targetNode, edge.targetPort, 'input');
      return portType(source) || portType(target) || null;
    }}

    function edgeColor(edge, fallback = '#43c6ac') {{
      return portTypeColor(edgeDataType(edge), edge.color || fallback);
    }}

    function connectionColor(from, fallback = '#43c6ac') {{
      return portTypeColor(from && from.port ? portType(from.port) : null, fallback);
    }}

    function portTypeConversion(sourceType, targetType) {{
      if (!sourceType || !targetType || sourceType === targetType) return null;
      const key = `${{sourceType}}->${{targetType}}`;
      return (config.portTypeConversions && config.portTypeConversions[key]) || null;
    }}

    function edgeConversion(from, to) {{
      if (!from || !to) return null;
      return portTypeConversion(portType(from.port), portType(to.port));
    }}

    function connectionRejection(from, to) {{
      if (!from) return 'missing source port';
      if (!to) return 'missing target port';
      if (from.side !== 'output') return 'source port must be an output';
      if (to.side !== 'input') return 'target port must be an input';
      if (from.node.id === to.node.id) return 'self connection rejected';
      const sourceType = portType(from.port);
      const targetType = portType(to.port);
      if (sourceType && targetType && sourceType !== targetType && !portTypeConversion(sourceType, targetType)) return `incompatible port types: ${{sourceType}} -> ${{targetType}}`;
      const duplicate = state.edges.some(edge => edge.sourceNode === from.node.id && edge.sourcePort === from.port.id && edge.targetNode === to.node.id && edge.targetPort === to.port.id);
      if (duplicate) return 'duplicate edge';
      return null;
    }}

    function canConnect(from, to) {{
      return connectionRejection(from, to) === null;
    }}

    function edgeEventPayload(edge) {{
      const payload = {{
        id: edge.id,
        source_node: edge.sourceNode,
        source_port: edge.sourcePort,
        target_node: edge.targetNode,
        target_port: edge.targetPort,
        label: edge.label || null,
        color: edgeColor(edge)
      }};
      const data = cloneEdgeData(edge.data);
      if (data) payload.data = data;
      return payload;
    }}

    function nodeEventPayload(node) {{
      const payload = {{
        id: node.id,
        title: node.title,
        position: {{ x: node.x, y: node.y }},
        inputs: node.inputs.map(port => ({{ ...port }})),
        outputs: node.outputs.map(port => ({{ ...port }})),
        subtitle: node.subtitle || null,
        status: node.status || null,
        color: node.color || '#43c6ac',
        width: node.width || 190
      }};
      if (node.data) payload.data = {{ ...node.data }};
      return payload;
    }}

    function sectionMemberIds(section) {{
      return state.nodes
        .filter(node => {{
          const cx = node.x + nodeWidth(node) / 2;
          const cy = node.y + nodeHeight(node) / 2;
          return cx >= section.x && cx <= section.x + section.width && cy >= section.y && cy <= section.y + section.height;
        }})
        .map(node => node.id);
    }}

    function nodePositionPayloads(nodeIds) {{
      return nodeIds
        .map(id => state.nodes.find(node => node.id === id))
        .filter(Boolean)
        .map(node => ({{ id: node.id, position: {{ x: node.x, y: node.y }} }}));
    }}

    function sectionEventPayload(section) {{
      const payload = {{
        id: section.id,
        title: section.title,
        position: {{ x: section.x, y: section.y }},
        size: {{ width: section.width, height: section.height }},
        purpose: section.purpose || null,
        trigger: section.trigger || null,
        color: section.color || '#43c6ac',
        collapsed: !!section.collapsed,
        locked: !!section.locked,
        members: sectionMemberIds(section)
      }};
      if (section.data) payload.data = {{ ...section.data }};
      return payload;
    }}
    function sectionDraftRect(drag) {{
      const x1 = Math.min(drag.startX, drag.currentX);
      const y1 = Math.min(drag.startY, drag.currentY);
      const x2 = Math.max(drag.startX, drag.currentX);
      const y2 = Math.max(drag.startY, drag.currentY);
      return {{ x: x1, y: y1, width: Math.max(1, x2 - x1), height: Math.max(1, y2 - y1) }};
    }}

    function createSectionFromRect(rect, before) {{
      if (rect.width < 72 || rect.height < 54) return false;
      const id = `section-${{++sectionSerial}}`;
      const section = {{
        id,
        title: `Section ${{sectionSerial}}`,
        x: rect.x,
        y: rect.y,
        width: Math.max(140, rect.width),
        height: Math.max(90, rect.height),
        purpose: null,
        trigger: null,
        color: '#7aa2f7',
        collapsed: false,
        locked: false,
        data: {{ created_via: 'canvas_shift_drag' }}
      }};
      state.sections.push(section);
      state.selected = null;
      state.selectedEdge = null;
      state.selectedSection = section.id;
      emitGraphMutation({{ event: 'section_created', section: sectionEventPayload(section) }}, before);
      return true;
    }}

    function sectionHandleSize() {{ return Math.max(10, 14 / state.zoom); }}

    function hitSectionResize(x, y) {{
      for (let i = state.sections.length - 1; i >= 0; i--) {{
        const section = state.sections[i];
        const handle = sectionHandleSize();
        if (x >= section.x + section.width - handle && x <= section.x + section.width + handle && y >= section.y + section.height - handle && y <= section.y + section.height + handle) return section;
      }}
      return null;
    }}

    function hitSectionMove(x, y) {{
      for (let i = state.sections.length - 1; i >= 0; i--) {{
        const section = state.sections[i];
        const border = Math.max(8, 10 / state.zoom);
        const inBounds = x >= section.x && x <= section.x + section.width && y >= section.y && y <= section.y + section.height;
        if (!inBounds) continue;
        const inHeader = y <= section.y + Math.max(42, 44 / state.zoom);
        const nearBorder = x <= section.x + border || x >= section.x + section.width - border || y <= section.y + border || y >= section.y + section.height - border;
        if (inHeader || nearBorder) return section;
      }}
      return null;
    }}

    function hitSection(x, y) {{
      for (let i = state.sections.length - 1; i >= 0; i--) {{
        const section = state.sections[i];
        if (x >= section.x && x <= section.x + section.width && y >= section.y && y <= section.y + section.height) return section;
      }}
      return null;
    }}

    function emitGraphEvent(payload) {{
      if (!config.emitEvents) return;
      const eventPayload = {{ schema_version: 1, ...payload }};
      if (eventPayload.event === 'node_selected') updateEditorChromeStatus(`Selected: ${{eventPayload.node || 'none'}}`);
      else if (eventPayload.event === 'edge_selected') updateEditorChromeStatus(`Selected: edge ${{eventPayload.edge || ''}}`);
      else if (eventPayload.event === 'selection_cleared') updateEditorChromeStatus('Selection cleared');
      else if (eventPayload.event === 'viewport_changed') updateEditorChromeStatus(`Navigation: ${{eventPayload.action || 'viewport'}}`);
      else if (eventPayload.event === 'graph_changed') updateEditorChromeStatus('Graph changed');
      else if (eventPayload.event === 'connection_rejected') updateEditorChromeStatus(`Rejected: ${{eventPayload.reason || 'connection'}}`);
      if (window.chrome && window.chrome.webview && window.chrome.webview.postMessage) {{
        window.chrome.webview.postMessage(eventPayload);
      }}
    }}

    function viewportPayload() {{
      return {{ x: state.viewX, y: state.viewY, zoom: state.zoom }};
    }}

    function emitViewportChanged(action) {{
      emitGraphEvent({{ event: 'viewport_changed', action, viewport: viewportPayload() }});
    }}

    function graphSnapshot() {{
      return {{
        nodes: state.nodes.map(node => ({{
          ...node,
          inputs: node.inputs.map(port => ({{ ...port }})),
          outputs: node.outputs.map(port => ({{ ...port }}))
        }})),
        edges: state.edges.map(edge => ({{ ...edge, data: cloneEdgeData(edge.data) || undefined }})),
        sections: state.sections.map(section => ({{ ...section, data: section.data ? {{ ...section.data }} : undefined }})),
        selected: state.selected,
        selectedEdge: state.selectedEdge,
        selectedSection: state.selectedSection
      }};
    }}

    function restoreGraphSnapshot(snapshot) {{
      state.nodes = snapshot.nodes.map(node => ({{
        ...node,
        inputs: node.inputs.map(port => ({{ ...port }})),
        outputs: node.outputs.map(port => ({{ ...port }}))
      }}));
      state.edges = snapshot.edges.map(edge => ({{ ...edge, data: cloneEdgeData(edge.data) || undefined }}));
      state.sections = (snapshot.sections || []).map(section => ({{ ...section, data: section.data ? {{ ...section.data }} : undefined }}));
      state.selected = snapshot.selected || null;
      state.selectedEdge = snapshot.selectedEdge || null;
      state.selectedSection = snapshot.selectedSection || null;
      nodeSerial = state.nodes.length;
      edgeSerial = state.edges.length;
      sectionSerial = state.sections.length;
      for (const node of state.nodes) {{
        const match = String(node.id || '').match(/(\\d+)$/);
        if (match) nodeSerial = Math.max(nodeSerial, Number(match[1]));
      }}
      for (const edge of state.edges) {{
        const match = String(edge.id || '').match(/(\\d+)$/);
        if (match) edgeSerial = Math.max(edgeSerial, Number(match[1]));
      }}
      for (const section of state.sections) {{
        const match = String(section.id || '').match(/(\\d+)$/);
        if (match) sectionSerial = Math.max(sectionSerial, Number(match[1]));
      }}
    }}

    function snapshotCore(snapshot) {{
      return JSON.stringify({{ nodes: snapshot.nodes, edges: snapshot.edges, sections: snapshot.sections }});
    }}

    function historyStatePayload() {{
      return {{
        can_undo: history.undo.length > 0,
        can_redo: history.redo.length > 0,
        dirty: history.initial !== snapshotCore(graphSnapshot()),
        undo_depth: history.undo.length,
        redo_depth: history.redo.length
      }};
    }}

    function pushHistory(before) {{
      if (!before) return;
      history.undo.push(before);
      history.redo = [];
    }}

    function emitGraphMutation(payload, before) {{
      pushHistory(before);
      emitGraphEvent({{ ...payload, history: historyStatePayload() }});
      emitGraphEvent({{ event: 'graph_changed', history: historyStatePayload() }});
    }}

    function undoGraph() {{
      if (history.undo.length === 0) return false;
      const current = graphSnapshot();
      const previous = history.undo.pop();
      history.redo.push(current);
      restoreGraphSnapshot(previous);
      emitGraphEvent({{ event: 'undo', history: historyStatePayload() }});
      emitGraphEvent({{ event: 'graph_changed', history: historyStatePayload() }});
      draw();
      return true;
    }}

    function redoGraph() {{
      if (history.redo.length === 0) return false;
      const current = graphSnapshot();
      const next = history.redo.pop();
      history.undo.push(current);
      restoreGraphSnapshot(next);
      emitGraphEvent({{ event: 'redo', history: historyStatePayload() }});
      emitGraphEvent({{ event: 'graph_changed', history: historyStatePayload() }});
      draw();
      return true;
    }}

    history.initial = snapshotCore(graphSnapshot());

    function graphBounds() {{
      if (!state.nodes.length && !state.sections.length) return null;
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const section of state.sections) {{
        minX = Math.min(minX, section.x);
        minY = Math.min(minY, section.y);
        maxX = Math.max(maxX, section.x + section.width);
        maxY = Math.max(maxY, section.y + section.height);
      }}
      for (const node of state.nodes) {{
        minX = Math.min(minX, node.x);
        minY = Math.min(minY, node.y);
        maxX = Math.max(maxX, node.x + nodeWidth(node));
        maxY = Math.max(maxY, node.y + nodeHeight(node));
      }}
      return {{ minX, minY, maxX, maxY, width: Math.max(1, maxX - minX), height: Math.max(1, maxY - minY) }};
    }}

    function setZoom(nextZoom, anchor = null, action = 'zoom') {{
      if (!config.enableZoom) return false;
      const rect = canvas.getBoundingClientRect();
      const sx = anchor ? anchor.sx : rect.width / 2;
      const sy = anchor ? anchor.sy : rect.height / 2;
      const gx = anchor ? anchor.x : (sx - state.viewX) / state.zoom;
      const gy = anchor ? anchor.y : (sy - state.viewY) / state.zoom;
      state.zoom = Math.max(0.55, Math.min(1.8, nextZoom));
      state.viewX = sx - gx * state.zoom;
      state.viewY = sy - gy * state.zoom;
      emitViewportChanged(action);
      draw();
      return true;
    }}

    function zoomBy(factor, action) {{
      return setZoom(state.zoom * factor, null, action);
    }}

    function fitToView(action = 'fit_to_view') {{
      const rect = canvas.getBoundingClientRect();
      const bounds = graphBounds();
      if (!bounds) {{
        state.viewX = 34;
        state.viewY = 48;
        state.zoom = 1;
        emitViewportChanged(action);
        draw();
        return true;
      }}
      const pad = 58;
      const topPad = 58;
      const availableW = Math.max(1, rect.width - pad * 2);
      const availableH = Math.max(1, rect.height - topPad - pad);
      const fitZoom = Math.min(1.8, Math.max(0.55, Math.min(availableW / bounds.width, availableH / bounds.height)));
      state.zoom = fitZoom;
      state.viewX = (rect.width - bounds.width * fitZoom) / 2 - bounds.minX * fitZoom;
      state.viewY = topPad + (availableH - bounds.height * fitZoom) / 2 - bounds.minY * fitZoom;
      emitViewportChanged(action);
      draw();
      return true;
    }}

    function createEdge(from, to) {{
      const rejection = connectionRejection(from, to);
      if (rejection) {{
        emitGraphEvent({{
          event: 'connection_rejected',
          reason: rejection,
          edge: {{
            source_node: from && from.node ? from.node.id : null,
            source_port: from && from.port ? from.port.id : null,
            target_node: to && to.node ? to.node.id : null,
            target_port: to && to.port ? to.port.id : null
          }},
          history: historyStatePayload()
        }});
        return false;
      }}
      const before = graphSnapshot();
      const conversion = edgeConversion(from, to);
      state.edges.push({{
        id: `edge-${{++edgeSerial}}`,
        sourceNode: from.node.id,
        sourcePort: from.port.id,
        targetNode: to.node.id,
        targetPort: to.port.id,
        label: conversion ? conversion : null,
        color: connectionColor(from, from.node.color || '#43c6ac'),
        data: conversion ? {{ conversion, newline: true }} : null
      }});
      emitGraphMutation({{ event: 'edge_created', edge: edgeEventPayload(state.edges[state.edges.length - 1]) }}, before);
      return true;
    }}

    function addNode(x, y, template = selectedTemplate()) {{
      const before = graphSnapshot();
      const id = `node-${{++nodeSerial}}`;
      const templateData = template && template.data ? {{ ...template.data }} : {{}};
      state.nodes.push({{
        id,
        title: template ? template.title : `Node ${{nodeSerial}}`,
        x,
        y,
        inputs: template ? template.inputs.map(port => ({{ ...port }})) : [{{ id: 'in', label: 'in' }}],
        outputs: template ? template.outputs.map(port => ({{ ...port }})) : [{{ id: 'out', label: 'out' }}],
        subtitle: template ? template.subtitle || null : null,
        status: template ? template.status || null : null,
        color: template ? template.color || '#43c6ac' : '#43c6ac',
        width: template ? template.width || 190 : 150,
        data: {{ ...templateData, template_id: template ? template.id : 'generic', template_title: template ? template.title : 'Node' }}
      }});
      state.selected = id;
      state.selectedEdge = null;
      state.selectedSection = null;
      emitGraphMutation({{ event: 'node_created', node: nodeEventPayload(state.nodes[state.nodes.length - 1]) }}, before);
    }}

    function openRenameEditor(kind, id, value) {{
      closeNodePicker(false);
      renameEditor.open = true;
      renameEditor.kind = kind;
      renameEditor.id = id;
      renameEditor.value = String(value || '');
      renameEditor.original = renameEditor.value;
      renameEditor.selectAll = true;
      renameEditor.rect = null;
      renameEditor.buttons = [];
      setTextEditTarget('renameEditor.value', renameEditor, 'value', true, '13px Segoe UI');
      emitGraphEvent({{ event: 'rename_editor_opened', target_type: kind, target: id }});
      draw();
      return true;
    }}

    function closeRenameEditor(notify = true) {{
      if (!renameEditor.open) return false;
      const kind = renameEditor.kind;
      const id = renameEditor.id;
      renameEditor.open = false;
      renameEditor.kind = null;
      renameEditor.id = null;
      renameEditor.value = '';
      renameEditor.original = '';
      renameEditor.selectAll = false;
      renameEditor.rect = null;
      renameEditor.buttons = [];
      if (textEdit.key === 'renameEditor.value') clearTextEditTarget();
      if (notify) emitGraphEvent({{ event: 'rename_editor_closed', target_type: kind, target: id }});
      draw();
      return true;
    }}


    function selectedPropertyTarget() {{
      if (state.selected) {{
        const node = state.nodes.find(n => n.id === state.selected);
        if (node) return {{ kind: 'node', target: node }};
      }}
      if (state.selectedSection) {{
        const section = state.sections.find(s => s.id === state.selectedSection);
        if (section) return {{ kind: 'section', target: section }};
      }}
      return null;
    }}

    function runtimeId(target) {{
      return target.data && target.data.runtime_id !== undefined && target.data.runtime_id !== null ? String(target.data.runtime_id) : '';
    }}

    function normalizeFieldOption(option) {{
      if (option && typeof option === 'object' && !Array.isArray(option)) {{
        const value = String(option.value ?? option.id ?? option.widget_id ?? option.label ?? '');
        return {{ value, label: String(option.label ?? value), meta: option }};
      }}
      const value = String(option ?? '');
      return {{ value, label: value, meta: {{ value, label: value }} }};
    }}

    function optionLabel(field, value) {{
      const key = String(value || '');
      return field.optionLabels && Object.prototype.hasOwnProperty.call(field.optionLabels, key)
        ? field.optionLabels[key]
        : key;
    }}

    function optionMeta(field, value) {{
      const key = String(value || '');
      return field.optionMeta && Object.prototype.hasOwnProperty.call(field.optionMeta, key)
        ? field.optionMeta[key]
        : null;
    }}

    function makePropertyField(key, label, value, type = 'text', options = {{}}) {{
      const normalizedType = ['bool', 'number', 'select', 'json', 'textarea'].includes(type) ? type : 'text';
      const normalizedOptions = Array.isArray(options.options) ? options.options.map(normalizeFieldOption) : [];
      let normalizedValue = value;
      if (normalizedType === 'bool') normalizedValue = !!value;
      else if (normalizedType === 'number') normalizedValue = value === undefined || value === null || value === '' ? '' : String(value);
      else if (normalizedType === 'json' && value !== undefined && value !== null && typeof value !== 'string') normalizedValue = JSON.stringify(value);
      else normalizedValue = String(value || '');
      const optionLabels = {{}};
      const optionMeta = {{}};
      for (const option of normalizedOptions) {{
        optionLabels[option.value] = option.label;
        optionMeta[option.value] = option.meta;
      }}
      return {{
        key,
        label,
        value: normalizedValue,
        type: normalizedType,
        options: normalizedOptions.map(option => option.value),
        optionLabels,
        optionMeta,
        placeholder: options.placeholder ? String(options.placeholder) : '',
        help: options.help || options.description ? String(options.help || options.description) : '',
        required: !!options.required,
        rect: null
      }};
    }}

    function nodeConfig(target) {{
      return target.data && target.data.config && typeof target.data.config === 'object' && !Array.isArray(target.data.config) ? target.data.config : {{}};
    }}

    function configSchemaFields(target) {{
      if (!target.data || !target.data.config_schema) return null;
      const schema = target.data.config_schema;
      if (Array.isArray(schema)) return schema;
      if (schema && Array.isArray(schema.fields)) return schema.fields;
      return null;
    }}

    function schemaFieldType(spec) {{
      const type = String(spec.type || 'text').toLowerCase();
      if (['bool', 'number', 'select', 'json', 'textarea'].includes(type)) return type;
      if (type === 'boolean') return 'bool';
      if (type === 'integer' || type === 'float') return 'number';
      return 'text';
    }}

    function schemaFieldValue(target, spec, storage) {{
      const key = String(spec.key);
      const config = nodeConfig(target);
      if (storage === 'config' && Object.prototype.hasOwnProperty.call(config, key)) return config[key];
      if (target.data && Object.prototype.hasOwnProperty.call(target.data, key)) return target.data[key];
      if (Object.prototype.hasOwnProperty.call(spec, 'default')) return spec.default;
      return '';
    }}

    function actionTargetById(id) {{
      const targetId = String(id || '');
      const targets = Array.isArray(config.actionTargets) ? config.actionTargets : [];
      return targets.find(targetInfo => String(targetInfo.action_id || targetInfo.id || '') === targetId) || null;
    }}

    function actionCommandOptions(targetInfo, fallback = 'run') {{
      let commands = [];
      if (targetInfo && Array.isArray(targetInfo.supported_commands) && targetInfo.supported_commands.length) {{
        commands = targetInfo.supported_commands.map(command => String(command)).filter(Boolean);
      }} else {{
        commands = ['run', 'stop', 'reset', 'replay'];
      }}
      const preferred = targetInfo && targetInfo.default_command ? String(targetInfo.default_command) : fallback;
      if (preferred && !commands.includes(preferred)) commands.unshift(preferred);
      return commands.map(command => {{ return {{ value: command, label: command }}; }});
    }}

    function sectionActionTargetFieldSpec(section, value) {{
      const targets = Array.isArray(config.actionTargets) ? config.actionTargets : [];
      const options = [{{ value: '', label: 'Choose action...', action_type: '', supported_commands: [] }}];
      for (const targetInfo of targets) {{
        const value = String(targetInfo.action_id || targetInfo.id || '');
        if (!value) continue;
        options.push({{
          value,
          label: String(targetInfo.label || value),
          action_type: String(targetInfo.action_type || ''),
          supported_commands: Array.isArray(targetInfo.supported_commands) ? targetInfo.supported_commands : [],
          default_command: String(targetInfo.default_command || '')
        }});
      }}
      const current = String(value || '');
      if (current && !options.some(option => option.value === current)) {{
        options.push({{ value: current, label: `${{current}} (unregistered)`, missing: true }});
      }}
      return makePropertyField('data:action_id', 'Action Target', current, 'select', {{ options, help: 'Registered GUI action' }});
    }}

    function sectionCommandFieldSpec(section, value) {{
      const data = section && section.data && typeof section.data === 'object' && !Array.isArray(section.data) ? section.data : {{}};
      const targetInfo = actionTargetById(data.action_id);
      const options = actionCommandOptions(targetInfo, 'run');
      return makePropertyField('data:section_command', 'Command', String(value || (targetInfo && targetInfo.default_command) || 'run'), 'select', {{ options, help: 'Section action command' }});
    }}

    function applyActionTargetSelection(field) {{
      if (!field || field.key !== 'data:action_id') return;
      const selected = selectedPropertyTarget();
      if (!selected || selected.kind !== 'section') return;
      const meta = optionMeta(field, field.value);
      if (!meta || meta.missing) return;
      const commandField = propertyField('data:section_command');
      if (commandField) {{
        const options = actionCommandOptions(meta, commandField.value || 'run');
        commandField.options = options.map(option => option.value);
        commandField.optionLabels = {{}};
        commandField.optionMeta = {{}};
        for (const option of options) {{
          commandField.optionLabels[option.value] = option.label;
          commandField.optionMeta[option.value] = option;
        }}
        const preferred = String(meta.default_command || options[0]?.value || 'run');
        if (!commandField.value || !commandField.options.includes(commandField.value)) commandField.value = preferred;
      }}
    }}
    function widgetBindingKind(target) {{
      return target && target.data ? String(target.data.node_type || target.data.template_id || '') : '';
    }}

    function isWidgetSinkTarget(target) {{
      return widgetBindingKind(target) === 'widget_sink';
    }}

    function isWidgetSourceTarget(target) {{
      return widgetBindingKind(target) === 'widget_source';
    }}

    function isWidgetBindingTarget(target) {{
      return isWidgetSinkTarget(target) || isWidgetSourceTarget(target);
    }}

    function widgetSinkPortProfiles() {{
      const profiles = Array.isArray(config.widgetSinkPortProfiles) ? config.widgetSinkPortProfiles.map(profile => String(profile)).filter(Boolean) : [];
      return profiles.length ? profiles : ['text', 'terminal_output', 'message', 'json', 'artifact', 'status', 'error'];
    }}

    function widgetSourcePortProfiles() {{
      const profiles = Array.isArray(config.widgetSourcePortProfiles) ? config.widgetSourcePortProfiles.map(profile => String(profile)).filter(Boolean) : [];
      return profiles.length ? profiles : ['text', 'json', 'status', 'message', 'artifact'];
    }}

    function widgetTargetById(id) {{
      const targetId = String(id || '');
      const targets = Array.isArray(config.widgetTargets) ? config.widgetTargets : [];
      return targets.find(targetInfo => String(targetInfo.widget_id || targetInfo.id || '') === targetId) || null;
    }}

    function terminalWidgetOptions(value) {{
      const targets = Array.isArray(config.widgetTargets) ? config.widgetTargets : [];
      const options = [{{ value: '', label: 'Choose terminal widget...', widget_type: 'terminal' }}];
      for (const targetInfo of targets) {{
        const targetId = String(targetInfo.widget_id || targetInfo.id || '');
        const widgetType = String(targetInfo.widget_type || targetInfo.target_type || '').toLowerCase();
        if (!targetId || widgetType !== 'terminal') continue;
        options.push({{
          value: targetId,
          label: String(targetInfo.label || targetId),
          widget_type: 'terminal'
        }});
      }}
      const current = String(value || '');
      if (current && !options.some(option => option.value === current)) {{
        options.push({{ value: current, label: `${{current}} (unregistered)`, widget_type: 'terminal', missing: true }});
      }}
      return options;
    }}

    function widgetProfileOptions(target, value, spec) {{
      const configData = nodeConfig(target);
      const targetInfo = widgetTargetById(configData.widget_id);
      let profiles = [];
      if (targetInfo && Array.isArray(targetInfo.supported_port_profiles) && targetInfo.supported_port_profiles.length) {{
        profiles = targetInfo.supported_port_profiles.map(profile => String(profile)).filter(Boolean);
      }} else if (spec && Array.isArray(spec.options) && spec.options.length) {{
        profiles = spec.options.map(option => normalizeFieldOption(option).value).filter(Boolean);
      }} else if (isWidgetSourceTarget(target)) {{
        profiles = widgetSourcePortProfiles();
      }} else {{
        profiles = widgetSinkPortProfiles();
      }}
      const current = String(value || '');
      if (current && !profiles.includes(current)) profiles.push(current);
      return profiles.map(profile => {{ return {{ value: profile, label: profile }}; }});
    }}

    function widgetProfilePort(profile) {{
      const value = String(profile || '').trim() || 'text';
      return {{ id: 'value', label: value, port_type: value }};
    }}

    function applyWidgetBindingPortProfile(node, updates, data) {{
      if (!isWidgetBindingTarget(node)) return;
      const configData = data && data.config && typeof data.config === 'object' && !Array.isArray(data.config) ? data.config : {{}};
      const port = widgetProfilePort(configData.port_profile || 'text');
      if (isWidgetSourceTarget(node)) {{
        updates.inputs = [];
        updates.outputs = [{{ ...port }}];
      }} else {{
        updates.inputs = [{{ ...port }}];
        updates.outputs = [{{ ...port }}];
      }}
      node.inputs = updates.inputs.map(item => ({{ ...item }}));
      node.outputs = updates.outputs.map(item => ({{ ...item }}));
    }}

    function buildTextInputCount(data) {{
      const configData = data && data.config && typeof data.config === 'object' && !Array.isArray(data.config) ? data.config : {{}};
      const raw = Number(configData.input_count || {_BUILD_TEXT_DEFAULT_INPUTS});
      return Math.max({_BUILD_TEXT_MIN_INPUTS}, Math.min({_BUILD_TEXT_MAX_INPUTS}, Number.isFinite(raw) ? Math.round(raw) : {_BUILD_TEXT_DEFAULT_INPUTS}));
    }}

    function applyBuildTextPorts(node, updates, data) {{
      if (!node || !node.data || String(node.data.node_type || node.data.template_id || '') !== 'build_text') return;
      const count = buildTextInputCount(data);
      updates.inputs = [];
      for (let index = 1; index <= count; index++) {{
        updates.inputs.push({{ id: `part_${{index}}`, label: `part ${{index}}`, port_type: 'text' }});
      }}
      updates.outputs = [{{ id: 'text', label: 'text', port_type: 'text' }}];
      node.inputs = updates.inputs.map(item => ({{ ...item }}));
      node.outputs = updates.outputs.map(item => ({{ ...item }}));
    }}

    function widgetTargetFieldSpec(target, spec, value) {{
      if (!isWidgetBindingTarget(target)) return null;
      const key = String(spec.key || '');
      if (key === 'port_profile') {{
        const help = isWidgetSourceTarget(target) ? 'Outgoing graph value type' : 'Incoming graph value type';
        return {{ ...spec, type: 'select', options: widgetProfileOptions(target, value, spec), help: spec.help || spec.description || help }};
      }}
      if (key !== 'widget_id') return null;
      const targets = Array.isArray(config.widgetTargets) ? config.widgetTargets : [];
      const options = [{{ value: '', label: 'Choose widget...', widget_type: '', supported_update_modes: [], supported_port_profiles: [] }}];
      for (const targetInfo of targets) {{
        const value = String(targetInfo.widget_id || targetInfo.id || '');
        if (!value) continue;
        options.push({{
          value,
          label: String(targetInfo.label || value),
          widget_type: String(targetInfo.widget_type || ''),
          supported_update_modes: Array.isArray(targetInfo.supported_update_modes) ? targetInfo.supported_update_modes : [],
          default_update_mode: String(targetInfo.default_update_mode || ''),
          supported_port_profiles: Array.isArray(targetInfo.supported_port_profiles) ? targetInfo.supported_port_profiles : [],
          default_port_profile: String(targetInfo.default_port_profile || ''),
          supported_formats: Array.isArray(targetInfo.supported_formats) ? targetInfo.supported_formats : []
        }});
      }}
      const current = String(value || '');
      if (current && !options.some(option => option.value === current)) {{
        options.push({{ value: current, label: `${{current}} (unregistered)`, missing: true }});
      }}
      return {{ ...spec, type: 'select', options, placeholder: 'Choose widget...', help: spec.help || spec.description || 'Registered GUI widget' }};
    }}

    function terminalTargetFieldSpec(target, spec, value) {{
      const kind = widgetBindingKind(target);
      if (kind !== 'terminal') return null;
      const key = String(spec.key || '');
      if (key !== 'terminal_widget_id' && String(spec.target_type || '') !== 'terminal_widget') return null;
      return {{
        ...spec,
        type: 'select',
        options: terminalWidgetOptions(value),
        placeholder: 'Choose terminal widget...',
        help: spec.help || spec.description || 'Attach this Terminal Session to a DragonGUI Terminal widget'
      }};
    }}

    function schemaPropertyFields(target) {{
      const schemaFields = configSchemaFields(target);
      const storage = schemaFields ? 'config' : 'data';
      const schema = schemaFields || (target.data && Array.isArray(target.data.property_fields) ? target.data.property_fields : []);
      return schema
        .filter(spec => spec && spec.key)
        .map(spec => {{
          const key = String(spec.key);
          const value = schemaFieldValue(target, spec, storage);
          const effectiveSpec = widgetTargetFieldSpec(target, spec, value) || terminalTargetFieldSpec(target, spec, value) || spec;
          const type = schemaFieldType(effectiveSpec);
          const fieldKey = storage === 'config' ? `config:${{key}}` : `data:${{key}}`;
          const field = makePropertyField(fieldKey, String(effectiveSpec.label || key), value, type, effectiveSpec);
          field.dataKey = key;
          field.storage = storage;
          return field;
        }});
    }}

    function fieldStorageValue(field) {{
      if (field.type === 'bool') return !!field.value;
      const raw = String(field.value || '').trim();
      if (!raw) return field.required ? '' : undefined;
      if (field.type === 'number') {{
        const parsed = Number(raw);
        return Number.isFinite(parsed) ? parsed : raw;
      }}
      if (field.type === 'json') {{
        try {{ return JSON.parse(raw); }} catch (error) {{ return raw; }}
      }}
      return raw;
    }}

    function applySchemaPropertyFields(data) {{
      let config = data.config && typeof data.config === 'object' && !Array.isArray(data.config) ? {{ ...data.config }} : {{}};
      let touchedConfig = false;
      for (const field of propertyEditor.fields) {{
        const fieldKey = String(field.key || '');
        if (!fieldKey.startsWith('data:') && !fieldKey.startsWith('config:')) continue;
        const key = field.dataKey || fieldKey.slice(fieldKey.indexOf(':') + 1);
        if (!key || key === 'property_fields' || key === 'config_schema') continue;
        const value = fieldStorageValue(field);
        if (fieldKey.startsWith('config:')) {{
          touchedConfig = true;
          if (value === undefined) delete config[key];
          else config[key] = value;
        }} else {{
          if (value === undefined) delete data[key];
          else data[key] = value;
        }}
      }}
      if (touchedConfig) {{
        if (Object.keys(config).length) data.config = config;
        else delete data.config;
      }}
    }}

    function openPropertyEditor() {{
      const selected = selectedPropertyTarget();
      if (!selected) return false;
      closeNodePicker(false);
      closeRenameEditor(false);
      const target = selected.target;
      propertyEditor.open = true;
      propertyEditor.kind = selected.kind;
      propertyEditor.id = target.id;
      propertyEditor.active = 0;
      propertyEditor.fields = selected.kind === 'node'
        ? [
            makePropertyField('title', 'Title', target.title || target.id),
            makePropertyField('subtitle', 'Subtitle', target.subtitle || ''),
            makePropertyField('status', 'Status', target.status || ''),
            makePropertyField('color', 'Color', target.color || '#43c6ac'),
            makePropertyField('runtime_id', 'Runtime ID', runtimeId(target))
          ].concat(schemaPropertyFields(target))
        : [
            makePropertyField('title', 'Title', target.title || target.id),
            makePropertyField('purpose', 'Purpose', target.purpose || ''),
            makePropertyField('trigger', 'Trigger', target.trigger || ''),
            makePropertyField('color', 'Color', target.color || '#43c6ac'),
            makePropertyField('runtime_id', 'Runtime ID', runtimeId(target)),
            sectionActionTargetFieldSpec(target, target.data && target.data.action_id),
            sectionCommandFieldSpec(target, target.data && target.data.section_command),
            makePropertyField('locked', 'Locked', !!target.locked, 'bool'),
            makePropertyField('collapsed', 'Collapsed', !!target.collapsed, 'bool')
          ];
      propertyEditor.scroll = 0;
      propertyEditor.scrollDrag = null;
      propertyEditor.rect = null;
      propertyEditor.listRect = null;
      propertyEditor.scrollBar = null;
      propertyEditor.buttons = [];
      propertyEditor.selectPopup = null;
      setActivePropertyTextTarget(true);
      emitGraphEvent({{ event: 'property_editor_opened', target_type: propertyEditor.kind, target: propertyEditor.id }});
      draw();
      return true;
    }}

    function closePropertyEditor(notify = true) {{
      if (!propertyEditor.open) return false;
      const kind = propertyEditor.kind;
      const id = propertyEditor.id;
      propertyEditor.open = false;
      propertyEditor.kind = null;
      propertyEditor.id = null;
      propertyEditor.fields = [];
      propertyEditor.active = 0;
      propertyEditor.scroll = 0;
      propertyEditor.scrollDrag = null;
      propertyEditor.rect = null;
      propertyEditor.listRect = null;
      propertyEditor.scrollBar = null;
      propertyEditor.buttons = [];
      propertyEditor.selectPopup = null;
      if (textEdit.key && textEdit.key.startsWith('propertyEditor.')) clearTextEditTarget();
      if (notify) emitGraphEvent({{ event: 'property_editor_closed', target_type: kind, target: id }});
      draw();
      return true;
    }}

    function propertyField(key) {{
      return propertyEditor.fields.find(field => field.key === key);
    }}

    function isBuildTextPropertyTarget() {{
      if (!propertyEditor.open || propertyEditor.kind !== 'node' || !propertyEditor.id) return false;
      const node = state.nodes.find(n => n.id === propertyEditor.id);
      return !!node && !!node.data && String(node.data.node_type || node.data.template_id || '') === 'build_text';
    }}

    function isBuildTextInputCountField(field) {{
      return isBuildTextPropertyTarget() && !!field && field.key === 'config:input_count';
    }}

    function clampBuildTextInputCount(value) {{
      const parsed = Number(value);
      const fallback = {_BUILD_TEXT_DEFAULT_INPUTS};
      const count = Number.isFinite(parsed) ? Math.round(parsed) : fallback;
      return Math.max({_BUILD_TEXT_MIN_INPUTS}, Math.min({_BUILD_TEXT_MAX_INPUTS}, count));
    }}

    function adjustBuildTextInputCount(index, delta) {{
      const field = propertyEditor.fields[index];
      if (!isBuildTextInputCountField(field)) return false;
      field.value = String(clampBuildTextInputCount(field.value) + delta);
      field.value = String(clampBuildTextInputCount(field.value));
      propertyEditor.active = index;
      closeSelectPopup();
      setActivePropertyTextTarget(true);
      draw();
      return true;
    }}

    function textProperty(key, fallback = '') {{
      const field = propertyField(key);
      return field ? String(field.value || '').trim() : fallback;
    }}

    function applyWidgetTargetSelection(field) {{
      if (!field || field.key !== 'config:widget_id') return;
      const target = selectedPropertyTarget();
      if (!target || target.kind !== 'node' || !isWidgetBindingTarget(target.target)) return;
      const meta = optionMeta(field, field.value);
      if (!meta || meta.missing) return;
      const typeField = propertyField('config:widget_type');
      if (typeField && meta.widget_type) typeField.value = String(meta.widget_type);
      const profileField = propertyField('config:port_profile');
      const profiles = Array.isArray(meta.supported_port_profiles) ? meta.supported_port_profiles.map(profile => String(profile)).filter(Boolean) : [];
      if (profileField && profiles.length) {{
        profileField.options = profiles.slice();
        profileField.optionLabels = {{}};
        profileField.optionMeta = {{}};
        for (const profile of profiles) {{
          profileField.optionLabels[profile] = profile;
          profileField.optionMeta[profile] = {{ value: profile, label: profile }};
        }}
        const preferred = String(meta.default_port_profile || profiles[0] || 'text');
        if (!profileField.value || !profileField.options.includes(profileField.value)) profileField.value = preferred;
      }}
      const updateField = propertyField('config:update_mode');
      const modes = Array.isArray(meta.supported_update_modes) ? meta.supported_update_modes.map(mode => String(mode)) : [];
      if (updateField && modes.length) {{
        updateField.options = ['auto'].concat(modes.filter(mode => mode && mode !== 'auto'));
        updateField.optionLabels = {{ auto: 'auto' }};
        updateField.optionMeta = {{ auto: {{ value: 'auto', label: 'auto' }} }};
        for (const mode of modes) {{
          if (!mode) continue;
          updateField.optionLabels[mode] = mode;
          updateField.optionMeta[mode] = {{ value: mode, label: mode }};
        }}
        const preferred = String(meta.default_update_mode || modes[0] || 'auto');
        if (!updateField.value || updateField.value === 'auto' || !updateField.options.includes(updateField.value)) updateField.value = preferred;
      }}
    }}
    function setSelectFieldValue(field, value) {{
      if (!field || field.type !== 'select') return false;
      field.value = String(value || '');
      applyWidgetTargetSelection(field);
      applyActionTargetSelection(field);
      return true;
    }}

    function cycleSelectField(field, direction) {{
      if (!field || field.type !== 'select' || !field.options.length) return false;
      const current = field.options.indexOf(String(field.value || ''));
      const next = (current + direction + field.options.length) % field.options.length;
      return setSelectFieldValue(field, field.options[next]);
    }}

    function closeSelectPopup() {{
      propertyEditor.selectPopup = null;
    }}

    function openSelectPopup(index) {{
      const field = propertyEditor.fields[index];
      if (!field || field.type !== 'select' || !field.options.length) return false;
      propertyEditor.active = index;
      const selectedIndex = Math.max(0, field.options.findIndex(value => String(value || '') === String(field.value || '')));
      propertyEditor.selectPopup = {{ fieldIndex: index, items: [], scroll: Math.max(0, selectedIndex - 2) * 28, rect: null, scrollBar: null }};
      focusCanvas();
      return true;
    }}

    function commitPropertyEditor() {{
      if (!propertyEditor.open) return false;
      closeSelectPopup();
      const before = graphSnapshot();
      if (propertyEditor.kind === 'node') {{
        const node = state.nodes.find(n => n.id === propertyEditor.id);
        if (!node) return closePropertyEditor(false);
        const data = node.data ? {{ ...node.data }} : {{}};
        const runtime = textProperty('runtime_id');
        if (runtime) data.runtime_id = runtime; else delete data.runtime_id;
        applySchemaPropertyFields(data);
        const updates = {{
          title: textProperty('title', node.id) || node.id,
          subtitle: textProperty('subtitle') || null,
          status: textProperty('status') || null,
          color: textProperty('color', node.color || '#43c6ac') || '#43c6ac',
          data: Object.keys(data).length ? data : null
        }};
        applyWidgetBindingPortProfile(node, updates, data);
        applyBuildTextPorts(node, updates, data);
        node.title = updates.title;
        node.subtitle = updates.subtitle;
        node.status = updates.status;
        node.color = updates.color;
        node.data = updates.data;
        propertyEditor.open = false;
        emitGraphMutation({{ event: 'node_updated', node: node.id, updates }}, before);
      }} else if (propertyEditor.kind === 'section') {{
        const section = state.sections.find(s => s.id === propertyEditor.id);
        if (!section) return closePropertyEditor(false);
        const data = section.data ? {{ ...section.data }} : {{}};
        const runtime = textProperty('runtime_id');
        if (runtime) data.runtime_id = runtime; else delete data.runtime_id;
        const action = textProperty('data:action_id');
        const command = textProperty('data:section_command', 'run') || 'run';
        if (action) data.action_id = action; else delete data.action_id;
        if (action || command !== 'run') data.section_command = command; else delete data.section_command;
        const lockedField = propertyField('locked');
        const collapsedField = propertyField('collapsed');
        const updates = {{
          title: textProperty('title', section.id) || section.id,
          purpose: textProperty('purpose') || null,
          trigger: textProperty('trigger') || null,
          color: textProperty('color', section.color || '#43c6ac') || '#43c6ac',
          locked: lockedField ? !!lockedField.value : !!section.locked,
          collapsed: collapsedField ? !!collapsedField.value : !!section.collapsed,
          data: Object.keys(data).length ? data : null
        }};
        section.title = updates.title;
        section.purpose = updates.purpose;
        section.trigger = updates.trigger;
        section.color = updates.color;
        section.locked = updates.locked;
        section.collapsed = updates.collapsed;
        section.data = updates.data;
        propertyEditor.open = false;
        emitGraphMutation({{ event: 'section_updated', section: section.id, updates }}, before);
      }} else {{
        return closePropertyEditor(false);
      }}
      propertyEditor.kind = null;
      propertyEditor.id = null;
      propertyEditor.fields = [];
      propertyEditor.active = 0;
      propertyEditor.scroll = 0;
      propertyEditor.scrollDrag = null;
      propertyEditor.rect = null;
      propertyEditor.listRect = null;
      propertyEditor.scrollBar = null;
      propertyEditor.buttons = [];
      propertyEditor.selectPopup = null;
      if (textEdit.key && textEdit.key.startsWith('propertyEditor.')) clearTextEditTarget();
      draw();
      return true;
    }}

    function editPropertyField(index) {{
      if (!propertyEditor.open || index < 0 || index >= propertyEditor.fields.length) return false;
      propertyEditor.active = index;
      focusCanvas();
      const field = propertyEditor.fields[index];
      if (field.type === 'bool') {{
        closeSelectPopup();
        field.value = !field.value;
      }} else if (field.type === 'select') {{
        if (propertyEditor.selectPopup && propertyEditor.selectPopup.fieldIndex === index) closeSelectPopup();
        else openSelectPopup(index);
      }} else {{
        closeSelectPopup();
        setActivePropertyTextTarget(false);
      }}
      draw();
      return true;
    }}

    function activePropertyField() {{
      return propertyEditor.fields[propertyEditor.active] || null;
    }}

    function setActivePropertyTextTarget(selectAll = false) {{
      const field = activePropertyField();
      if (!field || field.type === 'bool' || field.type === 'select') {{
        if (textEdit.key && textEdit.key.startsWith('propertyEditor.')) clearTextEditTarget();
        return false;
      }}
      setTextEditTarget(`propertyEditor.${{propertyEditor.active}}`, field, 'value', selectAll, '12px Segoe UI');
      return true;
    }}

    function propertyEditorRowHeight() {{
      return 48;
    }}

    function propertyEditorListHeight() {{
      return propertyEditor.listRect ? propertyEditor.listRect.h : (propertyEditor.rect ? Math.max(40, propertyEditor.rect.h - 122) : 40);
    }}

    function propertyEditorContentHeight() {{
      return Math.max(0, propertyEditor.fields.length * propertyEditorRowHeight());
    }}

    function clampPropertyEditorScroll() {{
      const maxScroll = Math.max(0, propertyEditorContentHeight() - propertyEditorListHeight());
      propertyEditor.scroll = Math.max(0, Math.min(propertyEditor.scroll || 0, maxScroll));
      return maxScroll;
    }}

    function ensurePropertyFieldVisible() {{
      if (!propertyEditor.open || !propertyEditor.fields.length) return;
      const rowH = propertyEditorRowHeight();
      const listH = propertyEditorListHeight();
      const rowTop = propertyEditor.active * rowH;
      const rowBottom = rowTop + rowH;
      if (rowTop < propertyEditor.scroll) propertyEditor.scroll = rowTop;
      else if (rowBottom > propertyEditor.scroll + listH) propertyEditor.scroll = rowBottom - listH;
      clampPropertyEditorScroll();
    }}
    function commitRenameEditor() {{
      if (!renameEditor.open) return false;
      const title = renameEditor.value.trim();
      if (!title) return false;
      const kind = renameEditor.kind;
      const id = renameEditor.id;
      if (title === renameEditor.original) return closeRenameEditor(false);
      const before = graphSnapshot();
      if (kind === 'node') {{
        const node = state.nodes.find(n => n.id === id);
        if (!node) return closeRenameEditor(false);
        node.title = title;
        renameEditor.open = false;
        emitGraphMutation({{ event: 'node_updated', node: node.id, updates: {{ title }} }}, before);
      }} else if (kind === 'section') {{
        const section = state.sections.find(s => s.id === id);
        if (!section) return closeRenameEditor(false);
        section.title = title;
        renameEditor.open = false;
        emitGraphMutation({{ event: 'section_updated', section: section.id, updates: {{ title }} }}, before);
      }} else {{
        return closeRenameEditor(false);
      }}
      renameEditor.kind = null;
      renameEditor.id = null;
      renameEditor.value = '';
      renameEditor.original = '';
      renameEditor.selectAll = false;
      renameEditor.rect = null;
      renameEditor.buttons = [];
      draw();
      return true;
    }}

    function editSelectedNodeTitle() {{
      const node = state.nodes.find(n => n.id === state.selected);
      if (!node) return false;
      return openRenameEditor('node', node.id, node.title || '');
    }}

    function editSelectedSectionTitle() {{
      const section = state.sections.find(s => s.id === state.selectedSection);
      if (!section) return false;
      return openRenameEditor('section', section.id, section.title || section.id || '');
    }}

    function duplicateSelectedNode() {{
      const node = state.nodes.find(n => n.id === state.selected);
      if (!node) return;
      const before = graphSnapshot();
      const id = `${{node.id}}-${{++nodeSerial}}`;
      state.nodes.push({{
        ...node,
        id,
        title: `${{node.title}} Copy`,
        x: node.x + 34,
        y: node.y + 34,
        inputs: node.inputs.map(port => ({{ ...port }})),
        outputs: node.outputs.map(port => ({{ ...port }}))
      }});
      state.selected = id;
      state.selectedEdge = null;
      state.selectedSection = null;
      emitGraphMutation({{ event: 'node_duplicated', node: nodeEventPayload(state.nodes[state.nodes.length - 1]), source: node.id }}, before);
    }}

    function deleteSelection() {{
      if (state.selected) {{
        const before = graphSnapshot();
        const nodeId = state.selected;
        state.nodes = state.nodes.filter(node => node.id !== nodeId);
        state.edges = state.edges.filter(edge => edge.sourceNode !== nodeId && edge.targetNode !== nodeId);
        state.selected = null;
        emitGraphMutation({{ event: 'node_deleted', node: nodeId }}, before);
      }} else if (state.selectedEdge) {{
        const before = graphSnapshot();
        const edgeId = state.selectedEdge;
        state.edges = state.edges.filter(edge => edge.id !== edgeId);
        state.selectedEdge = null;
        state.selectedSection = null;
        emitGraphMutation({{ event: 'edge_deleted', edge: edgeId }}, before);
      }} else if (state.selectedSection) {{
        const before = graphSnapshot();
        const sectionId = state.selectedSection;
        state.sections = state.sections.filter(section => section.id !== sectionId);
        state.selectedSection = null;
        emitGraphMutation({{ event: 'section_deleted', section: sectionId }}, before);
      }}
      draw();
    }}
    function drawGrid(width, height) {{
      if (!state.showGrid) return;
      const step = Math.max(18, 32 * state.zoom);
      ctx.strokeStyle = '#161d27';
      ctx.lineWidth = 1;
      for (let x = state.viewX % step; x < width; x += step) {{ ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, height); ctx.stroke(); }}
      for (let y = state.viewY % step; y < height; y += step) {{ ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(width, y); ctx.stroke(); }}
    }}

    function roundedRect(x, y, w, h, r) {{
      const rr = Math.min(r, w / 2, h / 2);
      ctx.beginPath();
      ctx.moveTo(x + rr, y);
      ctx.arcTo(x + w, y, x + w, y + h, rr);
      ctx.arcTo(x + w, y + h, x, y + h, rr);
      ctx.arcTo(x, y + h, x, y, rr);
      ctx.arcTo(x, y, x + w, y, rr);
      ctx.closePath();
    }}

    function topRoundedRect(x, y, w, h, r) {{
      const rr = Math.min(r, w / 2, h / 2);
      ctx.beginPath();
      ctx.moveTo(x + rr, y);
      ctx.arcTo(x + w, y, x + w, y + h, rr);
      ctx.lineTo(x + w, y + h);
      ctx.lineTo(x, y + h);
      ctx.lineTo(x, y + rr);
      ctx.arcTo(x, y, x + rr, y, rr);
      ctx.closePath();
    }}

    function drawSections() {{
      if (!state.sections.length) return;
      ctx.save();
      ctx.textBaseline = 'middle';
      ctx.textAlign = 'left';
      for (const section of state.sections) {{
        const p = screen(section.x, section.y);
        const w = Math.max(1, section.width * state.zoom);
        const h = Math.max(1, section.height * state.zoom);
        const color = section.color || '#43c6ac';
        const selected = state.selectedSection === section.id;
        ctx.globalAlpha = section.collapsed ? 0.10 : 0.13;
        roundedRect(p.x, p.y, w, h, 10);
        ctx.fillStyle = color;
        ctx.fill();
        ctx.globalAlpha = selected ? 1 : 0.88;
        ctx.setLineDash(selected ? [] : [7, 6]);
        ctx.strokeStyle = selected ? '#eef4ff' : color;
        ctx.lineWidth = selected ? Math.max(1.5, 1.9 * state.zoom) : Math.max(1, 1.2 * state.zoom);
        ctx.stroke();
        ctx.setLineDash([]);
        ctx.globalAlpha = 1;
        if (selected && !section.locked) {{
          const handle = Math.max(8, 10 * state.zoom);
          ctx.fillStyle = '#eef4ff';
          roundedRect(p.x + w - handle - 4, p.y + h - handle - 4, handle, handle, 3);
          ctx.fill();
        }}
        ctx.fillStyle = '#eef4ff';
        ctx.font = `${{Math.max(10, 12 * state.zoom)}}px Segoe UI`;
        ctx.fillText(section.title || section.id, p.x + 12 * state.zoom, p.y + 18 * state.zoom);
        const meta = [section.purpose, section.trigger ? `trigger: ${{section.trigger}}` : null].filter(Boolean).join(' | ');
        if (meta) {{
          ctx.fillStyle = '#9aa8b8';
          ctx.font = `${{Math.max(9, 10.5 * state.zoom)}}px Segoe UI`;
          ctx.fillText(meta, p.x + 12 * state.zoom, p.y + 36 * state.zoom);
        }}
      }}
      ctx.restore();
    }}

    function drawSectionDraft() {{
      if (!state.drag || state.drag.kind !== 'section-create') return;
      const rect = sectionDraftRect(state.drag);
      const p = screen(rect.x, rect.y);
      const w = rect.width * state.zoom;
      const h = rect.height * state.zoom;
      ctx.save();
      ctx.globalAlpha = 0.16;
      roundedRect(p.x, p.y, w, h, 10);
      ctx.fillStyle = '#7aa2f7';
      ctx.fill();
      ctx.globalAlpha = 0.95;
      ctx.setLineDash([7, 6]);
      ctx.strokeStyle = '#eef4ff';
      ctx.lineWidth = Math.max(1.5, 1.8 * state.zoom);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = '#eef4ff';
      ctx.font = `${{Math.max(10, 12 * state.zoom)}}px Segoe UI`;
      ctx.textBaseline = 'middle';
      ctx.textAlign = 'left';
      ctx.fillText('New Section', p.x + 12 * state.zoom, p.y + 18 * state.zoom);
      ctx.restore();
    }}

    function drawEdge(edge) {{
      const points = edgePoints(edge);
      if (!points) return;
      const selected = state.selectedEdge === edge.id;
      const color = edgeColor(edge);
      ctx.strokeStyle = selected ? '#eef4ff' : color;
      ctx.lineWidth = selected ? 4 : 2.4;
      ctx.beginPath();
      ctx.moveTo(points.route[0].x, points.route[0].y);
      for (let segment = 0; segment < points.route.length - 1; segment++) {{
        const a = points.route[segment];
        const b = points.route[segment + 1];
        const dx = edgeSegmentDx(a, b);
        ctx.bezierCurveTo(a.x + dx, a.y, b.x - dx, b.y, b.x, b.y);
      }}
      ctx.stroke();
      ctx.fillStyle = selected ? '#eef4ff' : color;
      ctx.beginPath(); ctx.arc(points.a.x, points.a.y, 3.5, 0, Math.PI * 2); ctx.fill();
      ctx.beginPath(); ctx.arc(points.b.x, points.b.y, 3.5, 0, Math.PI * 2); ctx.fill();
      if (selected && points.waypoints.length) {{
        for (const waypoint of points.waypoints) {{
          const point = screen(waypoint.x, waypoint.y);
          ctx.beginPath();
          ctx.arc(point.x, point.y, Math.max(4.5, 4.2 * state.zoom), 0, Math.PI * 2);
          ctx.fillStyle = '#101721';
          ctx.fill();
          ctx.lineWidth = Math.max(1.4, 1.2 * state.zoom);
          ctx.strokeStyle = color;
          ctx.stroke();
        }}
      }}
      if (config.showEdgeLabels && edge.label) {{
        const mid = points.route[Math.floor(points.route.length / 2)];
        ctx.fillStyle = '#9fb0c3'; ctx.font = '11px Segoe UI'; ctx.textAlign = 'center';
        ctx.fillText(edge.label, mid.x, mid.y - 12);
      }}
    }}

    function drawTempEdge(from, to) {{
      const a = from.point;
      const b = {{ x: to.x, y: to.y }};
      const dx = Math.max(48, Math.abs(b.x - a.x) * 0.45);
      const target = hitPort(to.x, to.y, 'input');
      const valid = canConnect(from, target);
      const color = connectionColor(from, '#43c6ac');
      ctx.strokeStyle = valid ? color : 'rgba(159, 176, 195, 0.68)';
      ctx.lineWidth = valid ? 3 : 2;
      ctx.setLineDash(valid ? [] : [7, 6]);
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.bezierCurveTo(a.x + dx, a.y, b.x - dx, b.y, b.x, b.y);
      ctx.stroke();
      ctx.setLineDash([]);
    }}

    function drawPalette() {{
      palette.items = [];
    }}

    function drawNodePicker() {{
      if (!nodePicker.open) return;
      clampNodePicker();
      const rect = nodePicker.rect;
      const items = nodePickerItems();
      nodePicker.selected = Math.max(0, Math.min(nodePicker.selected, Math.max(0, items.length - 1)));
      const maxScroll = clampNodePickerScroll(items);
      nodePicker.items = [];
      nodePicker.scrollBar = null;

      ctx.save();
      ctx.shadowColor = 'rgba(0,0,0,0.35)';
      ctx.shadowBlur = 18;
      roundedRect(rect.x, rect.y, rect.w, rect.h, 8);
      ctx.fillStyle = '#101721';
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.strokeStyle = '#33465c';
      ctx.lineWidth = 1;
      ctx.stroke();
      ctx.fillStyle = '#eef4ff';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'middle';
      ctx.font = '700 13px Segoe UI';
      ctx.fillText('Add Node', rect.x + 14, rect.y + 22);
      ctx.textAlign = 'center';
      ctx.font = '15px Segoe UI';
      ctx.fillStyle = '#9aa8b8';
      ctx.fillText('x', rect.x + rect.w - 23, rect.y + 22);

      const inputX = rect.x + 14;
      const inputY = rect.y + 44;
      const inputW = rect.w - 28;
      nodePicker.inputRect = {{ x: inputX + 8, y: inputY + 2, w: inputW - 16, h: 24 }};
      roundedRect(inputX, inputY, inputW, 28, 6);
      ctx.fillStyle = '#0b1017';
      ctx.fill();
      ctx.strokeStyle = '#26384a';
      ctx.lineWidth = 1;
      ctx.stroke();
      drawEditableText(
        nodePicker.query,
        'Type to filter templates...',
        nodePicker.inputRect,
        'nodePicker.query',
        '12px Segoe UI'
      );

      const listX = rect.x + 10;
      const listY = rect.y + 82;
      const listW = rect.w - 20;
      const listH = nodePickerListHeight();
      const contentH = nodePickerContentHeight(items);
      const needsScrollbar = maxScroll > 0;
      const rowW = listW - (needsScrollbar ? 14 : 0);
      nodePicker.listRect = {{ x: listX, y: listY, w: listW, h: listH }};

      ctx.save();
      ctx.beginPath();
      ctx.rect(listX, listY, listW, listH);
      ctx.clip();

      if (!items.length) {{
        ctx.fillStyle = '#9aa8b8';
        ctx.fillText('No matching templates', listX + 6, listY + 18);
      }}

      const startIndex = Math.max(0, Math.floor(nodePicker.scroll / 44));
      const endIndex = Math.min(items.length, Math.ceil((nodePicker.scroll + listH) / 44) + 1);
      for (let index = startIndex; index < endIndex; index++) {{
        const template = items[index];
        const selected = index === nodePicker.selected;
        const rowH = 40;
        const y = listY + index * 44 - nodePicker.scroll;
        roundedRect(listX, y, rowW, rowH, 6);
        ctx.fillStyle = selected ? '#26384a' : '#151c26';
        ctx.fill();
        ctx.strokeStyle = selected ? '#eef4ff' : '#26384a';
        ctx.lineWidth = selected ? 1.2 : 1;
        ctx.stroke();
        ctx.fillStyle = template.color || '#43c6ac';
        ctx.beginPath();
        ctx.arc(listX + 16, y + rowH / 2, 5, 0, Math.PI * 2);
        ctx.fill();
        ctx.textAlign = 'left';
        ctx.fillStyle = '#eef4ff';
        ctx.font = '12px Segoe UI';
        ctx.fillText(templateLabel(template), listX + 30, y + 14);
        if (template.subtitle || template.status) {{
          ctx.fillStyle = '#9aa8b8';
          ctx.font = '10.5px Segoe UI';
          ctx.fillText(template.subtitle || template.status, listX + 30, y + 29);
        }}
        nodePicker.items.push({{ template, index, x: listX, y, w: rowW, h: rowH }});
      }}
      ctx.restore();

      if (needsScrollbar) {{
        const trackX = rect.x + rect.w - 14;
        const trackY = listY;
        const trackH = listH;
        const thumbH = Math.max(24, listH * listH / Math.max(listH, contentH));
        const thumbY = trackY + (nodePicker.scroll / maxScroll) * (trackH - thumbH);
        roundedRect(trackX, trackY, 6, trackH, 3);
        ctx.fillStyle = '#0b1017';
        ctx.fill();
        roundedRect(trackX, thumbY, 6, thumbH, 3);
        ctx.fillStyle = '#6f849d';
        ctx.fill();
        nodePicker.scrollBar = {{ x: trackX, y: trackY, w: 6, h: trackH, thumbY, thumbH }};
      }}
      ctx.restore();
    }}
    function drawToolbar(width) {{
      toolbar.items = [];
      const actions = [
        {{ action: 'fit', icon: 'fit' }},
        {{ action: 'zoom_in', icon: 'zoom_in' }},
        {{ action: 'zoom_out', icon: 'zoom_out' }},
        {{ action: 'grid', icon: 'grid' }},
        {{ action: 'inspect', icon: 'inspect' }}
      ];
      let x = width - actions.length * (TOOLBAR_W + 6) - 6;
      ctx.save();
      ctx.textBaseline = 'middle';
      for (const item of actions) {{
        const active = item.action === 'grid' ? state.showGrid : false;
        ctx.fillStyle = active ? '#26384a' : '#171d27';
        ctx.strokeStyle = active ? '#eef4ff' : '#354255';
        ctx.lineWidth = active ? 1.5 : 1;
        roundedRect(x, TOOLBAR_Y, TOOLBAR_W, TOOLBAR_H, 6);
        ctx.fill();
        ctx.stroke();
        drawToolbarIcon(item.icon, x, TOOLBAR_Y, TOOLBAR_W, TOOLBAR_H, active ? '#eef4ff' : '#cbd6e2');
        toolbar.items.push({{ action: item.action, x, y: TOOLBAR_Y, w: TOOLBAR_W, h: TOOLBAR_H }});
        x += TOOLBAR_W + 6;
      }}
      ctx.restore();
    }}

    function drawToolbarIcon(icon, x, y, w, h, color) {{
      ctx.save();
      ctx.strokeStyle = color;
      ctx.fillStyle = color;
      ctx.lineWidth = 1.6;
      ctx.lineCap = 'round';
      ctx.lineJoin = 'round';
      if (icon === 'fit') drawFitToolbarIcon(x, y, w, h);
      else if (icon === 'zoom_in' || icon === 'zoom_out') drawZoomToolbarIcon(x, y, w, h, icon === 'zoom_in' ? 1 : -1);
      else if (icon === 'grid') drawGridToolbarIcon(x, y, w, h);
      else if (icon === 'inspect') drawInspectToolbarIcon(x, y, w, h);
      ctx.restore();
    }}

    function drawFitToolbarIcon(x, y, w, h) {{
      const inset = 9;
      const len = 6;
      const left = x + inset;
      const right = x + w - inset;
      const top = y + inset;
      const bottom = y + h - inset;
      for (const corner of [
        [left, top, 1, 1],
        [right, top, -1, 1],
        [right, bottom, -1, -1],
        [left, bottom, 1, -1],
      ]) {{
        const [cx, cy, sx, sy] = corner;
        ctx.beginPath();
        ctx.moveTo(cx, cy);
        ctx.lineTo(cx + len * sx, cy);
        ctx.moveTo(cx, cy);
        ctx.lineTo(cx, cy + len * sy);
        ctx.stroke();
      }}
    }}

    function drawZoomToolbarIcon(x, y, w, h, sign) {{
      const cx = x + w * 0.47;
      const cy = y + h * 0.46;
      const r = 5.7;
      ctx.beginPath();
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(cx - 3.2, cy);
      ctx.lineTo(cx + 3.2, cy);
      if (sign > 0) {{
        ctx.moveTo(cx, cy - 3.2);
        ctx.lineTo(cx, cy + 3.2);
      }}
      ctx.stroke();
      ctx.beginPath();
      ctx.moveTo(cx + r * 0.72, cy + r * 0.72);
      ctx.lineTo(cx + r * 0.72 + 6.2, cy + r * 0.72 + 6.2);
      ctx.stroke();
    }}

    function drawGridToolbarIcon(x, y, w, h) {{
      const size = 14;
      const left = x + (w - size) * 0.5;
      const top = y + (h - size) * 0.5;
      ctx.beginPath();
      for (let i = 0; i <= 2; i++) {{
        const t = i / 2;
        const gx = left + size * t;
        const gy = top + size * t;
        ctx.moveTo(gx, top);
        ctx.lineTo(gx, top + size);
        ctx.moveTo(left, gy);
        ctx.lineTo(left + size, gy);
      }}
      ctx.stroke();
    }}

    function drawInspectToolbarIcon(x, y, w, h) {{
      const cx = x + w * 0.5;
      ctx.beginPath();
      ctx.arc(cx, y + 8.2, 1.4, 0, Math.PI * 2);
      ctx.fill();
      ctx.beginPath();
      ctx.moveTo(cx, y + 13);
      ctx.lineTo(cx, y + h - 8);
      ctx.stroke();
    }}

    function runToolbarAction(action) {{
      if (action === 'fit') return fitToView();
      if (action === 'zoom_in') return zoomBy(1.16, 'zoom_in');
      if (action === 'zoom_out') return zoomBy(0.86, 'zoom_out');
      if (action === 'inspect') return openPropertyEditor();
      if (action === 'grid') {{
        state.showGrid = !state.showGrid;
        emitViewportChanged('grid_toggled');
        draw();
        return true;
      }}
      return false;
    }}

    function drawMinimap(width, height) {{
      const bounds = graphBounds();
      if (!bounds) return;
      const mapW = Math.min(160, Math.max(96, width * 0.18));
      const mapH = Math.min(110, Math.max(74, height * 0.18));
      const x = width - mapW - 12;
      const y = height - mapH - 12;
      const pad = 8;
      const scale = Math.min((mapW - pad * 2) / bounds.width, (mapH - pad * 2) / bounds.height);
      ctx.save();
      ctx.globalAlpha = 0.92;
      roundedRect(x, y, mapW, mapH, 6);
      ctx.fillStyle = '#111821';
      ctx.fill();
      ctx.strokeStyle = '#354255';
      ctx.lineWidth = 1;
      ctx.stroke();
      for (const node of state.nodes) {{
        const nx = x + pad + (node.x - bounds.minX) * scale;
        const ny = y + pad + (node.y - bounds.minY) * scale;
        const nw = Math.max(3, nodeWidth(node) * scale);
        const nh = Math.max(3, nodeHeight(node) * scale);
        ctx.fillStyle = node.color || '#43c6ac';
        ctx.fillRect(nx, ny, nw, nh);
      }}
      const rect = canvas.getBoundingClientRect();
      const viewX = x + pad + ((-state.viewX / state.zoom) - bounds.minX) * scale;
      const viewY = y + pad + ((-state.viewY / state.zoom) - bounds.minY) * scale;
      const viewW = (rect.width / state.zoom) * scale;
      const viewH = (rect.height / state.zoom) * scale;
      ctx.strokeStyle = '#eef4ff';
      ctx.lineWidth = 1.5;
      ctx.strokeRect(viewX, viewY, viewW, viewH);
      ctx.restore();
    }}

    function drawNode(node) {{
      const p = screen(node.x, node.y);
      const w = nodeWidth(node) * state.zoom;
      const h = nodeHeight(node) * state.zoom;
      const selected = state.selected === node.id;
      roundedRect(p.x, p.y, w, h, 8);
      ctx.fillStyle = '#171d27'; ctx.fill();
      topRoundedRect(p.x, p.y, w, HEADER * state.zoom, 8);
      ctx.fillStyle = '#26384a'; ctx.fill();
      roundedRect(p.x, p.y, w, h, 8);
      ctx.lineWidth = selected ? 2.4 : 1.2; ctx.strokeStyle = selected ? node.color : '#354255'; ctx.stroke();
      ctx.textBaseline = 'middle';
      ctx.fillStyle = '#eef4ff'; ctx.font = `${{Math.max(10, 13 * state.zoom)}}px Segoe UI`; ctx.textAlign = 'left';
      ctx.fillText(node.title, p.x + 12 * state.zoom, p.y + (HEADER * state.zoom) / 2);
      if (config.showStatusLabels && node.status) {{
        ctx.fillStyle = node.color; ctx.font = `${{Math.max(9, 10 * state.zoom)}}px Segoe UI`; ctx.textAlign = 'right';
        ctx.fillText(node.status, p.x + w - 12 * state.zoom, p.y + (HEADER * state.zoom) / 2);
      }}
      ctx.textBaseline = 'alphabetic';
      if (config.showSubtitles && node.subtitle) {{
        ctx.fillStyle = '#9aa8b8'; ctx.font = `${{Math.max(9, 10.5 * state.zoom)}}px Segoe UI`; ctx.textAlign = 'left';
        ctx.fillText(node.subtitle, p.x + 12 * state.zoom, p.y + (HEADER + 12) * state.zoom);
      }}
      drawPorts(node, 'input');
      drawPorts(node, 'output');
    }}

    function drawPorts(node, side) {{
      const ports = side === 'input' ? node.inputs : node.outputs;
      const np = screen(node.x, node.y);
      const w = nodeWidth(node) * state.zoom;
      const labelInset = 10 * state.zoom;
      for (const port of ports) {{
        const point = portPoint(node.id, port.id, side);
        if (!point) continue;
        const hovered = state.hoverPort && state.hoverPort.node.id === node.id && state.hoverPort.port.id === port.id && state.hoverPort.side === side;
        const connectTarget = state.drag && state.drag.kind === 'edge' && side === 'input' && canConnect(state.drag.from, {{ node, port, side, point }});
        const color = portTypeColor(portType(port), node.color || '#43c6ac');
        ctx.fillStyle = '#0d1117'; ctx.strokeStyle = hovered || connectTarget ? '#eef4ff' : color; ctx.lineWidth = hovered || connectTarget ? 3 : 2;
        ctx.beginPath(); ctx.arc(point.x, point.y, 5 * state.zoom, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
        if (!config.showPortLabels) continue;
        ctx.fillStyle = '#cbd6e2'; ctx.font = `${{Math.max(9, 10.5 * state.zoom)}}px Segoe UI`;
        ctx.textAlign = side === 'input' ? 'left' : 'right';
        ctx.textBaseline = 'middle';
        ctx.fillText(port.label || port.id, side === 'input' ? np.x + labelInset : np.x + w - labelInset, point.y);
        ctx.textBaseline = 'alphabetic';
      }}
    }}

    function drawRenameEditor(width, height) {{
      if (!renameEditor.open) return;
      const panelW = Math.min(420, Math.max(300, width - 32));
      const panelH = 154;
      const x = Math.max(16, (width - panelW) / 2);
      const y = Math.max(16, (height - panelH) / 2);
      renameEditor.rect = {{ x, y, w: panelW, h: panelH }};
      renameEditor.buttons = [
        {{ action: 'cancel', x: x + panelW - 164, y: y + panelH - 44, w: 68, h: 30 }},
        {{ action: 'commit', x: x + panelW - 86, y: y + panelH - 44, w: 70, h: 30 }}
      ];
      ctx.save();
      ctx.fillStyle = 'rgba(7, 11, 17, 0.58)';
      ctx.fillRect(0, 0, width, height);
      roundedRect(x, y, panelW, panelH, 8);
      ctx.fillStyle = '#111821';
      ctx.fill();
      ctx.strokeStyle = '#43c6ac';
      ctx.lineWidth = 1.4;
      ctx.stroke();
      ctx.textBaseline = 'middle';
      ctx.textAlign = 'left';
      ctx.fillStyle = '#eef4ff';
      ctx.font = '700 13px Segoe UI';
      ctx.fillText(renameEditor.kind === 'section' ? 'Rename Section' : 'Rename Node', x + 16, y + 22);
      ctx.fillStyle = '#9aa8b8';
      ctx.font = '11px Segoe UI';
      ctx.fillText(renameEditor.id || '', x + 16, y + 45);
      const inputX = x + 16;
      const inputY = y + 64;
      const inputW = panelW - 32;
      renameEditor.inputRect = {{ x: inputX + 8, y: inputY + 4, w: inputW - 16, h: 26 }};
      roundedRect(inputX, inputY, inputW, 34, 6);
      ctx.fillStyle = '#0b1017';
      ctx.fill();
      ctx.strokeStyle = '#354255';
      ctx.lineWidth = 1;
      ctx.stroke();
      drawEditableText(
        renameEditor.value,
        'Title',
        renameEditor.inputRect,
        'renameEditor.value',
        '13px Segoe UI'
      );
      for (const button of renameEditor.buttons) {{
        const primary = button.action === 'commit';
        roundedRect(button.x, button.y, button.w, button.h, 6);
        ctx.fillStyle = primary ? '#26384a' : '#171d27';
        ctx.fill();
        ctx.strokeStyle = primary ? '#43c6ac' : '#354255';
        ctx.stroke();
        ctx.fillStyle = '#eef4ff';
        ctx.textAlign = 'center';
        ctx.font = '12px Segoe UI';
        ctx.fillText(primary ? 'Save' : 'Cancel', button.x + button.w / 2, button.y + button.h / 2);
      }}
      ctx.restore();
    }}


    function drawPropertyEditor(width, height) {{
      if (!propertyEditor.open) return;
      const panelW = Math.min(520, Math.max(340, width - 32));
      const rowH = propertyEditorRowHeight();
      const idealH = 78 + propertyEditor.fields.length * rowH + 54;
      const panelH = Math.min(height - 32, idealH);
      const x = Math.max(16, (width - panelW) / 2);
      const y = Math.max(16, (height - panelH) / 2);
      propertyEditor.rect = {{ x, y, w: panelW, h: panelH }};
      propertyEditor.buttons = [
        {{ action: 'cancel', x: x + panelW - 164, y: y + panelH - 44, w: 68, h: 30 }},
        {{ action: 'commit', x: x + panelW - 86, y: y + panelH - 44, w: 70, h: 30 }}
      ];
      const listX = x + 16;
      const listY = y + 68;
      const listW = panelW - 32;
      const listH = Math.max(40, panelH - 122);
      propertyEditor.listRect = {{ x: listX, y: listY, w: listW, h: listH }};
      const maxScroll = clampPropertyEditorScroll();
      propertyEditor.scrollBar = null;
      for (const field of propertyEditor.fields) {{
        field.rect = null;
        field.stepperButtons = null;
      }}

      ctx.save();
      ctx.fillStyle = 'rgba(7, 11, 17, 0.58)';
      ctx.fillRect(0, 0, width, height);
      roundedRect(x, y, panelW, panelH, 8);
      ctx.fillStyle = '#111821';
      ctx.fill();
      ctx.strokeStyle = '#43c6ac';
      ctx.lineWidth = 1.4;
      ctx.stroke();
      ctx.textBaseline = 'middle';
      ctx.textAlign = 'left';
      ctx.fillStyle = '#eef4ff';
      ctx.font = '700 13px Segoe UI';
      ctx.fillText(propertyEditor.kind === 'section' ? 'Section Properties' : 'Node Properties', x + 16, y + 22);
      ctx.fillStyle = '#9aa8b8';
      ctx.font = '11px Segoe UI';
      ctx.fillText(propertyEditor.id || '', x + 16, y + 45);

      const needsScrollbar = maxScroll > 0;
      const labelX = listX;
      const labelW = 104;
      const inputX = x + 124;
      const inputW = panelW - 140 - (needsScrollbar ? 14 : 0);
      const startIndex = Math.max(0, Math.floor(propertyEditor.scroll / rowH));
      const endIndex = Math.min(propertyEditor.fields.length, Math.ceil((propertyEditor.scroll + listH) / rowH) + 1);

      ctx.save();
      ctx.beginPath();
      ctx.rect(listX, listY, listW, listH);
      ctx.clip();
      for (let index = startIndex; index < endIndex; index++) {{
        const field = propertyEditor.fields[index];
        const rowY = listY + index * rowH - propertyEditor.scroll;
        const hasStepper = isBuildTextInputCountField(field);
        field.stepperButtons = null;
        field.rect = {{ x: inputX, y: rowY + 4, w: inputW, h: 30 }};
        ctx.fillStyle = '#9aa8b8';
        ctx.font = '11px Segoe UI';
        ctx.textAlign = 'left';
        ctx.fillText(field.label + (field.required ? ' *' : ''), labelX, rowY + 16);
        if (field.help) {{
          ctx.fillStyle = '#607086';
          ctx.font = '9.5px Segoe UI';
          ctx.fillText(field.help, labelX, rowY + 31);
        }}
        if (field.type === 'bool') {{
          const box = {{ x: inputX, y: rowY + 8, w: 22, h: 22 }};
          field.rect = box;
          roundedRect(box.x, box.y, box.w, box.h, 5);
          ctx.fillStyle = field.value ? '#26384a' : '#0b1017';
          ctx.fill();
          ctx.strokeStyle = index === propertyEditor.active ? '#eef4ff' : '#354255';
          ctx.lineWidth = 1.2;
          ctx.stroke();
          if (field.value) {{
            ctx.strokeStyle = '#43c6ac';
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.moveTo(box.x + 5, box.y + 12);
            ctx.lineTo(box.x + 10, box.y + 17);
            ctx.lineTo(box.x + 18, box.y + 6);
            ctx.stroke();
          }}
        }} else {{
          roundedRect(inputX, rowY + 4, inputW, 30, 6);
          ctx.fillStyle = '#0b1017';
          ctx.fill();
          ctx.strokeStyle = index === propertyEditor.active ? '#eef4ff' : '#354255';
          ctx.lineWidth = index === propertyEditor.active ? 1.4 : 1;
          ctx.stroke();
          const text = String(field.value || '');
          const visible = field.type === 'select' ? (optionLabel(field, field.value) || field.placeholder || (field.options.length ? optionLabel(field, field.options[0]) : '')) : (text || field.placeholder || '');
          ctx.save();
          ctx.beginPath();
          const textClipW = inputW - (hasStepper ? 82 : 16);
          ctx.rect(inputX + 8, rowY + 5, Math.max(20, textClipW), 28);
          ctx.clip();
          if (field.type === 'select') {{
            ctx.fillStyle = text ? '#eef4ff' : '#738296';
            ctx.font = '12px Segoe UI';
            ctx.fillText(visible, inputX + 10, rowY + 19);
          }} else {{
            drawEditableText(
              text,
              field.placeholder || '',
              {{ x: inputX + 8, y: rowY + 5, w: Math.max(20, textClipW), h: 28 }},
              `propertyEditor.${{index}}`,
              '12px Segoe UI'
            );
          }}
          if (field.type === 'select' && field.options.length) {{
            ctx.fillStyle = '#607086';
            ctx.font = '10px Segoe UI';
            ctx.textAlign = 'right';
            const labels = field.options.slice(0, 3).map(value => optionLabel(field, value)).filter(Boolean);
            ctx.fillText(labels.length > 1 ? labels.join(' / ') : 'select', inputX + inputW - 10, rowY + 19);
            ctx.textAlign = 'left';
          }}
          ctx.restore();
          if (hasStepper) {{
            const buttonY = rowY + 7;
            const minus = {{ x: inputX + inputW - 66, y: buttonY, w: 27, h: 24, delta: -1 }};
            const plus = {{ x: inputX + inputW - 34, y: buttonY, w: 27, h: 24, delta: 1 }};
            field.stepperButtons = [minus, plus];
            for (const button of field.stepperButtons) {{
              const nextValue = clampBuildTextInputCount(field.value) + button.delta;
              const disabled = nextValue < {_BUILD_TEXT_MIN_INPUTS} || nextValue > {_BUILD_TEXT_MAX_INPUTS};
              roundedRect(button.x, button.y, button.w, button.h, 5);
              ctx.fillStyle = disabled ? '#10151d' : '#171d27';
              ctx.fill();
              ctx.strokeStyle = disabled ? '#253043' : '#354255';
              ctx.lineWidth = 1;
              ctx.stroke();
              ctx.fillStyle = disabled ? '#4f5f73' : '#eef4ff';
              ctx.textAlign = 'center';
              ctx.font = '700 14px Segoe UI';
              ctx.fillText(button.delta > 0 ? '+' : '-', button.x + button.w / 2, button.y + button.h / 2);
            }}
            ctx.textAlign = 'left';
          }}
        }}
      }}
      ctx.restore();

      if (needsScrollbar) {{
        const trackX = x + panelW - 14;
        const trackY = listY;
        const trackH = listH;
        const contentH = propertyEditorContentHeight();
        const thumbH = Math.max(24, listH * listH / Math.max(listH, contentH));
        const thumbY = trackY + (propertyEditor.scroll / maxScroll) * (trackH - thumbH);
        roundedRect(trackX, trackY, 6, trackH, 3);
        ctx.fillStyle = '#0b1017';
        ctx.fill();
        roundedRect(trackX, thumbY, 6, thumbH, 3);
        ctx.fillStyle = '#6f849d';
        ctx.fill();
        propertyEditor.scrollBar = {{ x: trackX, y: trackY, w: 6, h: trackH, thumbY, thumbH }};
      }}

      if (propertyEditor.selectPopup) {{
        const popup = propertyEditor.selectPopup;
        popup.items = [];
        popup.rect = null;
        popup.scrollBar = null;
        const field = propertyEditor.fields[popup.fieldIndex];
        if (field && field.rect && field.options.length) {{
          const optionH = 28;
          const maxVisible = Math.min(6, field.options.length);
          const popupW = field.rect.w;
          const popupH = maxVisible * optionH + 8;
          const popupX = field.rect.x;
          const popupY = Math.min(y + panelH - 52 - popupH, field.rect.y + field.rect.h + 4);
          const contentH = field.options.length * optionH;
          const viewportH = maxVisible * optionH;
          const maxScroll = Math.max(0, contentH - viewportH);
          popup.scroll = Math.max(0, Math.min(popup.scroll || 0, maxScroll));
          popup.rect = {{ x: popupX, y: popupY, w: popupW, h: popupH }};
          roundedRect(popupX, popupY, popupW, popupH, 7);
          ctx.fillStyle = '#0b1017';
          ctx.fill();
          ctx.strokeStyle = '#43c6ac';
          ctx.lineWidth = 1.1;
          ctx.stroke();
          ctx.save();
          ctx.beginPath();
          ctx.rect(popupX + 4, popupY + 4, popupW - 8, viewportH);
          ctx.clip();
          const startIndex = Math.max(0, Math.floor(popup.scroll / optionH));
          const endIndex = Math.min(field.options.length, Math.ceil((popup.scroll + viewportH) / optionH) + 1);
          const itemW = popupW - (maxScroll > 0 ? 18 : 8);
          for (let optionIndex = startIndex; optionIndex < endIndex; optionIndex++) {{
            const value = field.options[optionIndex];
            const itemY = popupY + 4 + optionIndex * optionH - popup.scroll;
            const selected = String(field.value || '') === String(value || '');
            roundedRect(popupX + 4, itemY + 2, itemW, optionH - 4, 5);
            ctx.fillStyle = selected ? '#26384a' : '#111821';
            ctx.fill();
            ctx.fillStyle = selected ? '#eef4ff' : '#9aa8b8';
            ctx.textAlign = 'left';
            ctx.font = '12px Segoe UI';
            ctx.fillText(optionLabel(field, value) || '(empty)', popupX + 12, itemY + optionH / 2);
            popup.items.push({{ index: optionIndex, value, x: popupX + 4, y: itemY + 2, w: itemW, h: optionH - 4 }});
          }}
          ctx.restore();
          if (maxScroll > 0) {{
            const trackX = popupX + popupW - 12;
            const trackY = popupY + 5;
            const trackH = viewportH - 2;
            const thumbH = Math.max(22, viewportH * viewportH / Math.max(viewportH, contentH));
            const thumbY = trackY + (popup.scroll / maxScroll) * (trackH - thumbH);
            roundedRect(trackX, trackY, 6, trackH, 3);
            ctx.fillStyle = '#111821';
            ctx.fill();
            roundedRect(trackX, thumbY, 6, thumbH, 3);
            ctx.fillStyle = '#6f849d';
            ctx.fill();
            popup.scrollBar = {{ x: trackX, y: trackY, w: 6, h: trackH, thumbY, thumbH, maxScroll }};
          }}
        }}
      }}

      for (const button of propertyEditor.buttons) {{
        const primary = button.action === 'commit';
        roundedRect(button.x, button.y, button.w, button.h, 6);
        ctx.fillStyle = primary ? '#26384a' : '#171d27';
        ctx.fill();
        ctx.strokeStyle = primary ? '#43c6ac' : '#354255';
        ctx.stroke();
        ctx.fillStyle = '#eef4ff';
        ctx.textAlign = 'center';
        ctx.font = '12px Segoe UI';
        ctx.fillText(primary ? 'Save' : 'Cancel', button.x + button.w / 2, button.y + button.h / 2);
      }}
      ctx.restore();
    }}
    function draw() {{
      const rect = canvas.getBoundingClientRect();
      ctx.clearRect(0, 0, rect.width, rect.height);
      ctx.fillStyle = '#0d1117'; ctx.fillRect(0, 0, rect.width, rect.height);
      drawGrid(rect.width, rect.height);
      drawSections();
      drawSectionDraft();
      for (const edge of state.edges) drawEdge(edge);
      if (state.drag && state.drag.kind === 'edge') drawTempEdge(state.drag.from, state.drag.to);
      for (const node of state.nodes) drawNode(node);
      drawPalette();
      drawMinimap(rect.width, rect.height);
      drawToolbar(rect.width);
      drawNodePicker();
      ctx.fillStyle = '#8b98a8'; ctx.font = '11px Segoe UI'; ctx.textAlign = 'left';
      ctx.fillText(`${{state.nodes.length}} nodes / ${{state.edges.length}} edges`, 12, rect.height - 16);
      drawRenameEditor(rect.width, rect.height);
      drawPropertyEditor(rect.width, rect.height);
      updateEditorChromeStatus();
    }}

    setupEditorChrome();

    canvas.addEventListener('mousedown', event => {{
      focusCanvas();
      const p = graphPoint(event);
      if (propertyEditor.open) {{
        const propertyHit = hitPropertyEditor(p.sx, p.sy);
        if (propertyHit && propertyHit.kind === 'commit') commitPropertyEditor();
        else if (propertyHit && propertyHit.kind === 'select_option') {{
          const popup = propertyEditor.selectPopup;
          const field = popup ? propertyEditor.fields[popup.fieldIndex] : null;
          if (field) setSelectFieldValue(field, propertyHit.value);
          closeSelectPopup();
        }}
        else if (propertyHit && propertyHit.kind === 'select_scrollbar') {{
          const popup = propertyEditor.selectPopup;
          const bar = popup && popup.scrollBar;
          if (popup && bar) {{
            const trackTravel = Math.max(1, bar.h - bar.thumbH);
            if (!propertyHit.onThumb && bar.maxScroll > 0) {{
              popup.scroll = ((p.sy - bar.y - bar.thumbH / 2) / trackTravel) * bar.maxScroll;
            }}
            popup.scrollDrag = {{ startY: p.sy, startScroll: popup.scroll || 0, maxScroll: bar.maxScroll, trackH: bar.h, thumbH: bar.thumbH }};
            canvas.style.cursor = 'grabbing';
          }}
        }}
        else if (propertyHit && propertyHit.kind === 'scrollbar' && propertyEditor.scrollBar) {{
          const bar = propertyEditor.scrollBar;
          const maxScroll = clampPropertyEditorScroll();
          if (!propertyHit.onThumb && maxScroll > 0) {{
            const trackTravel = Math.max(1, bar.h - bar.thumbH);
            propertyEditor.scroll = ((p.sy - bar.y - bar.thumbH / 2) / trackTravel) * maxScroll;
            clampPropertyEditorScroll();
          }}
          propertyEditor.scrollDrag = {{ startY: p.sy, startScroll: propertyEditor.scroll, maxScroll, trackH: bar.h, thumbH: bar.thumbH }};
          canvas.style.cursor = 'grabbing';
        }}
        else if (propertyHit && propertyHit.kind === 'field_stepper') {{
          adjustBuildTextInputCount(propertyHit.index, propertyHit.delta);
        }}
        else if (propertyHit && (propertyHit.kind === 'cancel' || propertyHit.kind === 'outside')) closePropertyEditor();
        else if (propertyHit && propertyHit.kind === 'field') {{
          propertyEditor.active = propertyHit.index;
          const field = activePropertyField();
          if (field && field.type !== 'bool' && field.type !== 'select') {{
            closeSelectPopup();
            beginTextDrag(`propertyEditor.${{propertyHit.index}}`, field, 'value', field.rect, p.sx, p.sy, '12px Segoe UI');
          }} else {{
            editPropertyField(propertyHit.index);
          }}
        }}
        event.preventDefault();
        draw();
        return;
      }}
      if (renameEditor.open) {{
        const renameHit = hitRenameEditor(p.sx, p.sy);
        if (renameHit && renameHit.kind === 'commit') commitRenameEditor();
        else if (renameHit && (renameHit.kind === 'cancel' || renameHit.kind === 'outside')) closeRenameEditor();
        else if (renameHit && renameHit.kind === 'inside') {{
          renameEditor.selectAll = false;
          beginTextDrag('renameEditor.value', renameEditor, 'value', renameEditor.inputRect, p.sx, p.sy, '13px Segoe UI');
        }}
        event.preventDefault();
        draw();
        return;
      }}
      if (nodePicker.open) {{
        const pickerHit = hitNodePicker(p.sx, p.sy);
        if (beginTextDrag('nodePicker.query', nodePicker, 'query', nodePicker.inputRect, p.sx, p.sy, '12px Segoe UI')) {{
          event.preventDefault();
          return;
        }}
        if (pickerHit && pickerHit.kind === 'item') chooseNodePickerSelection(pickerHit.index);
        else if (pickerHit && pickerHit.kind === 'scrollbar' && nodePicker.scrollBar) {{
          const bar = nodePicker.scrollBar;
          const maxScroll = clampNodePickerScroll();
          if (!pickerHit.onThumb && maxScroll > 0) {{
            const trackTravel = Math.max(1, bar.h - bar.thumbH);
            nodePicker.scroll = ((p.sy - bar.y - bar.thumbH / 2) / trackTravel) * maxScroll;
            clampNodePickerScroll();
          }}
          nodePicker.scrollDrag = {{ startY: p.sy, startScroll: nodePicker.scroll, maxScroll, trackH: bar.h, thumbH: bar.thumbH }};
          canvas.style.cursor = 'grabbing';
          draw();
        }}
        else if (pickerHit && pickerHit.kind === 'close') closeNodePicker();
        else if (pickerHit && pickerHit.kind === 'outside') closeNodePicker();
        event.preventDefault();
        return;
      }}
      const toolbarAction = hitToolbar(p.sx, p.sy);
      if (toolbarAction) {{
        runToolbarAction(toolbarAction);
        state.drag = null;
        event.preventDefault();
        return;
      }}
      const paletteTemplate = hitPalette(p.sx, p.sy);
      if (paletteTemplate) {{
        palette.selected = paletteTemplate.id;
        state.drag = null;
        draw();
        return;
      }}
      const waypoint = hitEdgeWaypoint(p.sx, p.sy);
      if (waypoint) {{
        state.selected = null;
        state.selectedEdge = waypoint.edge.id;
        state.selectedSection = null;
        state.drag = {{ kind: 'edge-waypoint', edge: waypoint.edge.id, index: waypoint.index, before: graphSnapshot() }};
        canvas.style.cursor = 'grabbing';
        emitGraphEvent({{ event: 'edge_selected', edge: waypoint.edge.id }});
        draw();
        return;
      }}
      const output = hitPort(p.sx, p.sy, 'output');
      if (output) {{
        state.selected = null;
        state.selectedEdge = null;
        state.selectedSection = null;
        state.drag = {{ kind: 'edge', from: output, to: {{ x: p.sx, y: p.sy }} }};
        canvas.style.cursor = 'crosshair';
        draw();
        return;
      }}
      const header = hitHeader(p.x, p.y);
      if (header) {{
        state.selected = header.id;
        state.selectedEdge = null;
        state.selectedSection = null;
        state.drag = {{ kind: 'node', id: header.id, ox: p.x - header.x, oy: p.y - header.y, before: graphSnapshot() }};
        canvas.style.cursor = 'grabbing';
        emitGraphEvent({{ event: 'node_selected', node: header.id }});
      }} else {{
        const body = hitNode(p.x, p.y);
        if (body) {{
          state.selected = body.id;
          state.selectedEdge = null;
          state.selectedSection = null;
          state.drag = null;
          emitGraphEvent({{ event: 'node_selected', node: body.id }});
          draw();
          return;
        }}
        const edge = hitEdge(p.sx, p.sy);
        if (edge) {{
          state.selected = null;
          state.selectedEdge = edge.id;
          state.selectedSection = null;
          state.drag = null;
          emitGraphEvent({{ event: 'edge_selected', edge: edge.id }});
          draw();
          return;
        }}
        if (event.shiftKey && !hitSection(p.x, p.y)) {{
          state.selected = null;
          state.selectedEdge = null;
          state.selectedSection = null;
          state.drag = {{ kind: 'section-create', startX: p.x, startY: p.y, currentX: p.x, currentY: p.y, before: graphSnapshot() }};
          canvas.style.cursor = 'crosshair';
          emitGraphEvent({{ event: 'section_create_started', position: {{ x: p.x, y: p.y }} }});
          draw();
          return;
        }}
        const resizeSection = hitSectionResize(p.x, p.y);
        if (resizeSection) {{
          state.selected = null;
          state.selectedEdge = null;
          state.selectedSection = resizeSection.id;
          if (!resizeSection.locked) {{
            state.drag = {{ kind: 'section-resize', id: resizeSection.id, startX: p.x, startY: p.y, startW: resizeSection.width, startH: resizeSection.height, before: graphSnapshot() }};
            canvas.style.cursor = 'nwse-resize';
          }} else {{
            state.drag = null;
          }}
          emitGraphEvent({{ event: 'section_selected', section: resizeSection.id }});
          draw();
          return;
        }}
        const moveSection = hitSectionMove(p.x, p.y);
        const section = moveSection || hitSection(p.x, p.y);
        if (section) {{
          state.selected = null;
          state.selectedEdge = null;
          state.selectedSection = section.id;
          if (moveSection && !section.locked) {{
            const members = sectionMemberIds(section);
            state.drag = {{
              kind: 'section',
              id: section.id,
              ox: p.x - section.x,
              oy: p.y - section.y,
              startX: section.x,
              startY: section.y,
              members,
              memberStarts: members.map(id => {{
                const node = state.nodes.find(candidate => candidate.id === id);
                return node ? {{ id, x: node.x, y: node.y }} : null;
              }}).filter(Boolean),
              before: graphSnapshot()
            }};
            canvas.style.cursor = 'grabbing';
          }} else {{
            state.drag = null;
          }}
          emitGraphEvent({{ event: 'section_selected', section: section.id }});
          draw();
          return;
        }}
        state.selected = null;
        state.selectedEdge = null;
        state.selectedSection = null;
        state.drag = {{ kind: 'pan', sx: p.sx, sy: p.sy, vx: state.viewX, vy: state.viewY }};
        canvas.style.cursor = 'grabbing';
        emitGraphEvent({{ event: 'selection_cleared' }});
      }}
      draw();
    }});

    window.addEventListener('mousemove', event => {{
      const p = graphPoint(event);
      if (textEdit.dragging) {{
        updateTextDrag(p.sx, p.sy);
        event.preventDefault();
        return;
      }}
      if (propertyEditor.selectPopup && propertyEditor.selectPopup.scrollDrag) {{
        const drag = propertyEditor.selectPopup.scrollDrag;
        const trackTravel = Math.max(1, drag.trackH - drag.thumbH);
        propertyEditor.selectPopup.scroll = drag.startScroll + (p.sy - drag.startY) * (drag.maxScroll / trackTravel);
        draw();
        event.preventDefault();
        return;
      }}
      if (propertyEditor.scrollDrag) {{
        const drag = propertyEditor.scrollDrag;
        const trackTravel = Math.max(1, drag.trackH - drag.thumbH);
        propertyEditor.scroll = drag.startScroll + (p.sy - drag.startY) * (drag.maxScroll / trackTravel);
        clampPropertyEditorScroll();
        canvas.style.cursor = 'grabbing';
        draw();
        return;
      }}
      if (nodePicker.scrollDrag) {{
        const drag = nodePicker.scrollDrag;
        const trackTravel = Math.max(1, drag.trackH - drag.thumbH);
        nodePicker.scroll = drag.startScroll + (p.sy - drag.startY) * (drag.maxScroll / trackTravel);
        clampNodePickerScroll();
        canvas.style.cursor = 'grabbing';
        draw();
        return;
      }}
      state.hoverPort = hitPort(p.sx, p.sy);
      if (!state.drag) {{
        const resizeSection = hitSectionResize(p.x, p.y);
        const movableSection = hitSectionMove(p.x, p.y);
        const section = movableSection || hitSection(p.x, p.y);
        const waypointHit = hitEdgeWaypoint(p.sx, p.sy);
        const edgeHit = hitEdge(p.sx, p.sy);
        canvas.style.cursor = state.hoverPort && state.hoverPort.side === 'output'
          ? 'crosshair'
          : waypointHit
            ? 'grab'
            : hitHeader(p.x, p.y)
              ? 'grab'
              : edgeHit
                ? 'pointer'
                : hitNode(p.x, p.y)
                  ? 'default'
                  : resizeSection && !resizeSection.locked
                    ? 'nwse-resize'
                    : movableSection && !movableSection.locked
                      ? 'grab'
                      : section
                        ? 'pointer'
                        : 'move';
        draw();
        return;
      }}
      if (state.drag.kind === 'node') {{
        const node = state.nodes.find(n => n.id === state.drag.id);
        if (node) {{ node.x = p.x - state.drag.ox; node.y = p.y - state.drag.oy; }}
      }} else if (state.drag.kind === 'section-create') {{
        state.drag.currentX = p.x;
        state.drag.currentY = p.y;
      }} else if (state.drag.kind === 'section') {{
        const section = state.sections.find(s => s.id === state.drag.id);
        if (section) {{
          const nextX = p.x - state.drag.ox;
          const nextY = p.y - state.drag.oy;
          const dx = nextX - state.drag.startX;
          const dy = nextY - state.drag.startY;
          section.x = nextX;
          section.y = nextY;
          for (const memberStart of state.drag.memberStarts || []) {{
            const node = state.nodes.find(candidate => candidate.id === memberStart.id);
            if (node) {{ node.x = memberStart.x + dx; node.y = memberStart.y + dy; }}
          }}
        }}
      }} else if (state.drag.kind === 'section-resize') {{
        const section = state.sections.find(s => s.id === state.drag.id);
        if (section) {{ section.width = Math.max(140, state.drag.startW + p.x - state.drag.startX); section.height = Math.max(90, state.drag.startH + p.y - state.drag.startY); }}
      }} else if (state.drag.kind === 'edge-waypoint') {{
        const edge = state.edges.find(candidate => candidate.id === state.drag.edge);
        if (edge) {{
          const waypoints = edgeWaypoints(edge);
          if (state.drag.index >= 0 && state.drag.index < waypoints.length) {{
            waypoints[state.drag.index] = {{ x: p.x, y: p.y }};
            setEdgeWaypoints(edge, waypoints);
          }}
        }}
      }} else if (state.drag.kind === 'edge') {{
        state.drag.to = {{ x: p.sx, y: p.sy }};
      }} else {{
        state.viewX = state.drag.vx + p.sx - state.drag.sx;
        state.viewY = state.drag.vy + p.sy - state.drag.sy;
      }}
      draw();
    }});

    window.addEventListener('mouseup', event => {{
      if (textEdit.dragging) {{
        textEdit.dragging = false;
        event.preventDefault();
      }}
      if (propertyEditor.selectPopup && propertyEditor.selectPopup.scrollDrag) {{
        propertyEditor.selectPopup.scrollDrag = null;
        canvas.style.cursor = 'default';
        event.preventDefault();
      }}
      if (propertyEditor.scrollDrag) {{
        propertyEditor.scrollDrag = null;
        canvas.style.cursor = 'default';
        draw();
        return;
      }}
      if (nodePicker.scrollDrag) {{
        nodePicker.scrollDrag = null;
        canvas.style.cursor = 'default';
        draw();
        return;
      }}
      if (state.drag && state.drag.kind === 'edge') {{
        const p = graphPoint(event);
        const target = hitPort(p.sx, p.sy, 'input');
        if (createEdge(state.drag.from, target)) {{
          state.selected = null;
          state.selectedEdge = state.edges[state.edges.length - 1].id;
          state.selectedSection = null;
        }}
      }} else if (state.drag && state.drag.kind === 'section-create') {{
        state.drag.currentX = graphPoint(event).x;
        state.drag.currentY = graphPoint(event).y;
        createSectionFromRect(sectionDraftRect(state.drag), state.drag.before);
      }} else if (state.drag && state.drag.kind === 'node') {{
        const node = state.nodes.find(n => n.id === state.drag.id);
        if (node && snapshotCore(state.drag.before) !== snapshotCore(graphSnapshot())) emitGraphMutation({{ event: 'node_moved', node: node.id, position: {{ x: node.x, y: node.y }} }}, state.drag.before);
      }} else if (state.drag && state.drag.kind === 'section') {{
        const section = state.sections.find(s => s.id === state.drag.id);
        if (section && snapshotCore(state.drag.before) !== snapshotCore(graphSnapshot())) emitGraphMutation({{ event: 'section_moved', section: sectionEventPayload(section), nodes: nodePositionPayloads(state.drag.members || []) }}, state.drag.before);
      }} else if (state.drag && state.drag.kind === 'section-resize') {{
        const section = state.sections.find(s => s.id === state.drag.id);
        if (section && snapshotCore(state.drag.before) !== snapshotCore(graphSnapshot())) emitGraphMutation({{ event: 'section_resized', section: sectionEventPayload(section) }}, state.drag.before);
      }} else if (state.drag && state.drag.kind === 'edge-waypoint') {{
        const edge = state.edges.find(candidate => candidate.id === state.drag.edge);
        if (edge && snapshotCore(state.drag.before) !== snapshotCore(graphSnapshot())) emitGraphMutation({{ event: 'edge_waypoints_changed', edge: edgeEventPayload(edge) }}, state.drag.before);
      }} else if (state.drag && state.drag.kind === 'pan') {{
        emitViewportChanged('pan');
      }}
      state.drag = null;
      canvas.style.cursor = 'default';
      draw();
    }});

    canvas.addEventListener('dblclick', event => {{
      const p = graphPoint(event);
      if (hitPort(p.sx, p.sy) || hitNode(p.x, p.y)) return;
      const edgeHit = hitEdgeSegment(p.sx, p.sy);
      if (edgeHit) {{
        addEdgeWaypoint(edgeHit.edge, edgeHit.segment, p.x, p.y);
        event.preventDefault();
        return;
      }}
      const section = hitSection(p.x, p.y);
      if (section) {{
        state.selected = null;
        state.selectedEdge = null;
        state.selectedSection = section.id;
        editSelectedSectionTitle();
        return;
      }}
      if (!config.emitEvents) {{
        addNode(p.x, p.y);
        draw();
        return;
      }}
      openNodePicker(p);
    }});
    canvas.addEventListener('keydown', event => {{
      function handleTextEditKey() {{
        if (!textEdit.owner || !textEdit.prop) return false;
        const key = event.key;
        const ctrl = event.ctrlKey || event.metaKey;
        if (ctrl && key.toLowerCase() === 'a') {{
          event.preventDefault();
          textEdit.anchor = 0;
          textEdit.caret = textEditValue().length;
          draw();
          return true;
        }}
        if (ctrl && key.toLowerCase() === 'c') {{
          event.preventDefault();
          writeClipboardText(selectedTextValue());
          return true;
        }}
        if (ctrl && key.toLowerCase() === 'x') {{
          event.preventDefault();
          writeClipboardText(selectedTextValue());
          deleteTextSelection();
          draw();
          return true;
        }}
        if (ctrl && key.toLowerCase() === 'v') {{
          event.preventDefault();
          pasteClipboardText();
          return true;
        }}
        if (key === 'ArrowLeft') {{
          event.preventDefault();
          moveTextCaret(-1, event.shiftKey);
          draw();
          return true;
        }}
        if (key === 'ArrowRight') {{
          event.preventDefault();
          moveTextCaret(1, event.shiftKey);
          draw();
          return true;
        }}
        if (key === 'Home') {{
          event.preventDefault();
          setTextCaret(0, event.shiftKey);
          draw();
          return true;
        }}
        if (key === 'End') {{
          event.preventDefault();
          setTextCaret(textEditValue().length, event.shiftKey);
          draw();
          return true;
        }}
        if (key === 'Backspace') {{
          event.preventDefault();
          deleteTextBackward();
          draw();
          return true;
        }}
        if (key === 'Delete') {{
          event.preventDefault();
          deleteTextForward();
          draw();
          return true;
        }}
        if (key.length === 1 && !ctrl && !event.altKey) {{
          event.preventDefault();
          insertTextAtCaret(key);
          draw();
          return true;
        }}
        return false;
      }}
      if (propertyEditor.open) {{
        const field = activePropertyField();
        if (event.key === 'Escape') {{
          event.preventDefault();
          closePropertyEditor();
        }} else if (event.key === 'Enter') {{
          event.preventDefault();
          commitPropertyEditor();
        }} else if (event.key === 'Tab') {{
          event.preventDefault();
          const direction = event.shiftKey ? -1 : 1;
          propertyEditor.active = (propertyEditor.active + direction + propertyEditor.fields.length) % propertyEditor.fields.length;
          setActivePropertyTextTarget(true);
          ensurePropertyFieldVisible();
          draw();
        }} else if ((event.key === ' ' || event.key === 'ArrowDown' || event.key === 'ArrowRight') && field && field.type === 'select') {{
          event.preventDefault();
          closeSelectPopup();
          cycleSelectField(field, 1);
        }} else if ((event.key === 'ArrowUp' || event.key === 'ArrowLeft') && field && field.type === 'select') {{
          event.preventDefault();
          closeSelectPopup();
          cycleSelectField(field, -1);
        }} else if (event.key === ' ' && field && field.type === 'bool') {{
          event.preventDefault();
          field.value = !field.value;
          draw();
        }} else if ((event.key === 'ArrowUp' || event.key === 'ArrowRight') && isBuildTextInputCountField(field)) {{
          event.preventDefault();
          adjustBuildTextInputCount(propertyEditor.active, 1);
        }} else if ((event.key === 'ArrowDown' || event.key === 'ArrowLeft') && isBuildTextInputCountField(field)) {{
          event.preventDefault();
          adjustBuildTextInputCount(propertyEditor.active, -1);
        }} else if (field && field.type !== 'bool' && field.type !== 'select') {{
          setActivePropertyTextTarget(false);
          handleTextEditKey();
        }}
        return;
      }}
      if (renameEditor.open) {{
        if (event.key === 'Escape') {{
          event.preventDefault();
          closeRenameEditor();
        }} else if (event.key === 'Enter') {{
          event.preventDefault();
          commitRenameEditor();
        }} else {{
          renameEditor.selectAll = false;
          handleTextEditKey();
        }}
        return;
      }}
      if (nodePicker.open) {{
        if (event.key === 'Escape') {{
          event.preventDefault();
          closeNodePicker();
        }} else if (event.key === 'Enter') {{
          event.preventDefault();
          chooseNodePickerSelection();
        }} else if (event.key === 'ArrowDown') {{
          event.preventDefault();
          nodePicker.selected = Math.min(nodePicker.selected + 1, Math.max(0, nodePickerItems().length - 1));
          ensureNodePickerSelectionVisible();
          draw();
        }} else if (event.key === 'ArrowUp') {{
          event.preventDefault();
          nodePicker.selected = Math.max(0, nodePicker.selected - 1);
          ensureNodePickerSelectionVisible();
          draw();
        }} else {{
          handleTextEditKey();
        }}
        return;
      }}
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z' && !event.shiftKey) {{
        event.preventDefault();
        undoGraph();
      }} else if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === 'y' || (event.key.toLowerCase() === 'z' && event.shiftKey))) {{
        event.preventDefault();
        redoGraph();
      }} else if (event.key === 'Delete' || event.key === 'Backspace') {{
        event.preventDefault();
        deleteSelection();
      }} else if (event.key === 'Enter' || event.key === 'F2') {{
        event.preventDefault();
        if (!editSelectedNodeTitle()) editSelectedSectionTitle();
      }} else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'd') {{
        event.preventDefault();
        duplicateSelectedNode();
        draw();
      }} else if (event.key.toLowerCase() === 'i') {{
        event.preventDefault();
        openPropertyEditor();
      }} else if (event.key.toLowerCase() === 'f') {{
        event.preventDefault();
        fitToView();
      }} else if (event.key === '+' || event.key === '=') {{
        event.preventDefault();
        zoomBy(1.16, 'zoom_in');
      }} else if (event.key === '-' || event.key === '_') {{
        event.preventDefault();
        zoomBy(0.86, 'zoom_out');
      }} else if (event.key.toLowerCase() === 'g') {{
        event.preventDefault();
        state.showGrid = !state.showGrid;
        emitViewportChanged('grid_toggled');
        draw();
      }} else if (event.key === 'Escape') {{
        state.drag = null;
        state.selected = null;
        state.selectedEdge = null;
        state.selectedSection = null;
        emitGraphEvent({{ event: 'selection_cleared' }});
        draw();
      }}
    }});

    canvas.addEventListener('copy', event => {{
      if (!textEdit.owner || !textEdit.prop || !hasTextSelection()) return;
      event.preventDefault();
      if (event.clipboardData) event.clipboardData.setData('text/plain', selectedTextValue());
    }});

    canvas.addEventListener('cut', event => {{
      if (!textEdit.owner || !textEdit.prop || !hasTextSelection()) return;
      event.preventDefault();
      if (event.clipboardData) event.clipboardData.setData('text/plain', selectedTextValue());
      deleteTextSelection();
      draw();
    }});

    canvas.addEventListener('paste', event => {{
      if (!textEdit.owner || !textEdit.prop) return;
      const text = event.clipboardData ? event.clipboardData.getData('text/plain') : '';
      if (!text) return;
      event.preventDefault();
      insertTextAtCaret(text);
      draw();
    }});

    canvas.addEventListener('wheel', event => {{
      const p = graphPoint(event);
      if (propertyEditor.open && propertyEditor.rect) {{
        const hit = hitPropertyEditor(p.sx, p.sy);
        if (hit && hit.kind !== 'outside') {{
          event.preventDefault();
          if (propertyEditor.selectPopup && (hit.kind === 'select_popup' || hit.kind === 'select_option' || hit.kind === 'select_scrollbar')) {{
            const popup = propertyEditor.selectPopup;
            const bar = popup.scrollBar;
            if (bar) {{
              popup.scroll = Math.max(0, Math.min((popup.scroll || 0) + event.deltaY, bar.maxScroll));
            }}
            draw();
            return;
          }}
          propertyEditor.scroll += event.deltaY;
          clampPropertyEditorScroll();
          draw();
          return;
        }}
      }}
      if (nodePicker.open && nodePicker.rect) {{
        const hit = hitNodePicker(p.sx, p.sy);
        if (hit && hit.kind !== 'outside') {{
          event.preventDefault();
          nodePicker.scroll += event.deltaY;
          clampNodePickerScroll();
          draw();
          return;
        }}
      }}
      if (!config.enableZoom) return;
      event.preventDefault();
      const factor = event.deltaY < 0 ? 1.08 : 0.92;
      setZoom(state.zoom * factor, p, event.deltaY < 0 ? 'zoom_in' : 'zoom_out');
    }}, {{ passive: false }});
    window.addEventListener('resize', resize);
    resize();
  </script>
</body>
</html>"""


def _editor_action_payload(value: Mapping[str, object]) -> dict[str, object]:
    if not isinstance(value, Mapping):
        raise TypeError("NodeGraph editor action must be a mapping")
    raw_id = value.get("id", value.get("action"))
    action_id = str(raw_id or "").strip()
    if not action_id:
        raise ValueError("NodeGraph editor action id cannot be empty")
    payload: dict[str, object] = {"id": action_id}
    for key in ("label", "tooltip", "icon"):
        raw = value.get(key)
        if raw is not None:
            text = str(raw).strip()
            if text:
                payload[key] = text
    if bool(value.get("primary", False)):
        payload["primary"] = True
    if bool(value.get("wide", False)):
        payload["wide"] = True
    if bool(value.get("separator_before", False)):
        payload["separator_before"] = True
    return payload


def _section_payload(section: NodeGraphSection) -> dict[str, object]:
    payload = {
        "id": section.id,
        "title": section.title,
        "x": section.x,
        "y": section.y,
        "width": section.width,
        "height": section.height,
        "purpose": section.purpose,
        "trigger": section.trigger,
        "color": section.color,
        "collapsed": section.collapsed,
        "locked": section.locked,
    }
    if section.data is not None:
        payload["data"] = _json_copy(section.data, "section custom data")
    return payload


def _node_payload(node: NodeGraphNode) -> dict[str, object]:
    payload = {
        "id": node.id,
        "title": node.title,
        "x": node.x,
        "y": node.y,
        "inputs": [_port_payload(port) for port in node.inputs],
        "outputs": [_port_payload(port) for port in node.outputs],
        "subtitle": node.subtitle,
        "status": node.status,
        "color": node.color,
        "width": node.width,
    }
    if node.data is not None:
        payload["data"] = _json_copy(node.data, "node custom data")
    return payload


def _template_payload(template: NodeGraphTemplate) -> dict[str, object]:
    payload = {
        "id": template.id,
        "title": template.title,
        "inputs": [_port_payload(port) for port in template.inputs],
        "outputs": [_port_payload(port) for port in template.outputs],
        "subtitle": template.subtitle,
        "status": template.status,
        "color": template.color,
        "width": template.width,
    }
    if template.data is not None:
        payload["data"] = _json_copy(template.data, "template custom data")
    return payload


def _action_target_payload(target: NodeGraphActionTarget) -> dict[str, object]:
    payload: dict[str, object] = {
        "action_id": target.id,
        "id": target.id,
        "label": target.label or target.id,
        "action_type": target.action_type,
        "supported_commands": list(target.supported_commands),
        "default_command": target.default_command,
    }
    if target.data is not None:
        payload["data"] = _json_copy(target.data, "action target data")
    return payload


def _widget_target_payload(target: NodeGraphWidgetTarget) -> dict[str, object]:
    payload: dict[str, object] = {
        "widget_id": target.id,
        "id": target.id,
        "label": target.label or target.id,
        "widget_type": target.widget_type,
        "supported_update_modes": list(target.supported_update_modes),
        "default_update_mode": target.default_update_mode,
        "supported_port_profiles": list(target.supported_port_profiles),
        "default_port_profile": target.default_port_profile,
        "supported_formats": list(target.supported_formats),
    }
    if target.data is not None:
        payload["data"] = _json_copy(target.data, "widget target data")
    return payload


def _edge_payload(edge: NodeGraphEdge, nodes: Sequence[NodeGraphNode] = ()) -> dict[str, object]:
    conversion = _edge_conversion(edge, nodes)
    data = _edge_data(edge)
    if conversion and "conversion" not in data:
        data["conversion"] = conversion
        data.setdefault("newline", True)
    payload = {
        "sourceNode": edge.source_node,
        "sourcePort": edge.source_port,
        "targetNode": edge.target_node,
        "targetPort": edge.target_port,
        "label": edge.label,
        "color": _edge_type_color(edge, nodes, edge.color),
    }
    if edge.id is not None:
        payload["id"] = edge.id
    if data:
        payload["data"] = data
    return payload


def node_graph_port_type_color(port_type: object | None, fallback: str = "#43c6ac") -> str:
    """Return the standard NodeGraph wire/socket color for a port data type."""

    return _port_type_color(port_type, fallback)


def _port_type_conversion(source_type: object | None, target_type: object | None) -> str | None:
    if source_type is None or target_type is None:
        return None
    return _NODE_GRAPH_PORT_TYPE_CONVERSIONS.get((str(source_type), str(target_type)))


def _edge_data(edge: NodeGraphEdge) -> dict[str, object]:
    if edge.data is not None:
        return _json_copy(edge.data, "edge data")
    return {}


def _edge_conversion(edge: NodeGraphEdge, nodes: Sequence[NodeGraphNode]) -> str | None:
    data = _edge_data(edge)
    configured = data.get("conversion")
    if configured is not None and str(configured).strip():
        return str(configured).strip()
    source_node = next((candidate for candidate in nodes if candidate.id == edge.source_node), None)
    target_node = next((candidate for candidate in nodes if candidate.id == edge.target_node), None)
    if source_node is None or target_node is None:
        return None
    source_port = _port_by_id(source_node.outputs, edge.source_port)
    target_port = _port_by_id(target_node.inputs, edge.target_port)
    if source_port is None or target_port is None:
        return None
    return _port_type_conversion(source_port.port_type, target_port.port_type)


def _port_type_color(port_type: object | None, fallback: str = "#43c6ac") -> str:
    if port_type is None:
        return fallback
    return _NODE_GRAPH_PORT_TYPE_COLORS.get(str(port_type), fallback)


def _edge_type_color(edge: NodeGraphEdge, nodes: Sequence[NodeGraphNode], fallback: str = "#43c6ac") -> str:
    node = next((candidate for candidate in nodes if candidate.id == edge.source_node), None)
    if node is None:
        return fallback
    port = _port_by_id(node.outputs, edge.source_port)
    if port is None:
        return fallback
    return _port_type_color(port.port_type, fallback)


def _port_payload(port: NodeGraphPort) -> dict[str, object]:
    payload = {"id": port.id, "label": port.label}
    if port.port_type is not None:
        payload["port_type"] = port.port_type
        payload["type"] = port.port_type
    if port.data is not None:
        payload["data"] = _json_copy(port.data, "port custom data")
    return payload


def _node_center_in_section(node: NodeGraphNode, section: NodeGraphSection) -> bool:
    width = max(float(node.width), 1.0)
    ports = max(len(node.inputs), len(node.outputs), 1)
    height = 36.0 + 22.0 + max(0, ports - 1) * 22.0 + 22.0
    center_x = node.x + width / 2.0
    center_y = node.y + height / 2.0
    return section.x <= center_x <= section.x + section.width and section.y <= center_y <= section.y + section.height



def _section_graph_data(section: NodeGraphSection) -> dict[str, object]:
    payload: dict[str, object] = {
        "id": section.id,
        "title": section.title,
        "label": section.title,
        "position": {"x": section.x, "y": section.y},
        "size": {"width": section.width, "height": section.height},
        "purpose": section.purpose,
        "trigger": section.trigger,
        "color": section.color,
        "collapsed": section.collapsed,
        "locked": section.locked,
    }
    if section.data is not None:
        payload["data"] = section.data
    return payload


def _node_graph_data(node: NodeGraphNode) -> dict[str, object]:
    payload: dict[str, object] = {
        "id": node.id,
        "title": node.title,
        "position": {"x": node.x, "y": node.y},
        "width": node.width,
        "color": node.color,
        "status": node.status,
        "label": node.title,
        "subtitle": node.subtitle,
        "inputs": [_port_graph_data(port) for port in node.inputs],
        "outputs": [_port_graph_data(port) for port in node.outputs],
    }
    if node.data is not None:
        payload["data"] = node.data
    return payload


def _edge_graph_data(edge: NodeGraphEdge, index: int, nodes: Sequence[NodeGraphNode] = ()) -> dict[str, object]:
    conversion = _edge_conversion(edge, nodes)
    data = _edge_data(edge)
    if conversion and "conversion" not in data:
        data["conversion"] = conversion
        data.setdefault("newline", True)
    payload: dict[str, object] = {
        "id": edge.id or f"edge-{index + 1}",
        "source": {"node": edge.source_node, "port": edge.source_port},
        "target": {"node": edge.target_node, "port": edge.target_port},
        "label": edge.label,
        "color": _edge_type_color(edge, nodes, edge.color),
    }
    if data:
        payload["data"] = data
    return payload


def _port_graph_data(port: NodeGraphPort) -> dict[str, object]:
    payload: dict[str, object] = {"id": port.id, "label": port.label}
    if port.port_type is not None:
        payload["port_type"] = port.port_type
        payload["type"] = port.port_type
    if port.data is not None:
        payload["data"] = port.data
    return payload


def _default_templates() -> tuple[NodeGraphTemplate, ...]:
    return (
        NodeGraphTemplate(
            "process",
            "Process",
            inputs=(NodeGraphPort("in", "in"),),
            outputs=(NodeGraphPort("out", "out"),),
            subtitle="work step",
            color="#43c6ac",
            data={"kind": "process"},
        ),
        NodeGraphTemplate(
            "source",
            "Source",
            outputs=(NodeGraphPort("out", "out"),),
            subtitle="input",
            color="#7aa2f7",
            data={"kind": "source"},
        ),
        NodeGraphTemplate(
            "sink",
            "Sink",
            inputs=(NodeGraphPort("in", "in"),),
            subtitle="output",
            color="#e0af68",
            data={"kind": "sink"},
        ),
    )


def _template_data(
    node_type: str,
    default_status: str | None = None,
    property_fields: Sequence[Mapping[str, object]] | None = None,
    **extra: object,
) -> dict[str, object]:
    data: dict[str, object] = {"node_type": node_type}
    if default_status is not None:
        data["default_status"] = default_status
    if property_fields is not None:
        fields = [dict(field) for field in property_fields]
        data["property_fields"] = fields
        data["config_schema"] = {"version": 1, "fields": [dict(field) for field in fields]}
    data.update(extra)
    return data


def multi_agent_node_templates() -> tuple[NodeGraphTemplate, ...]:
    """Return primitive and workflow templates for multi-agent graphs."""

    return (
        NodeGraphTemplate(
            "terminal",
            "Terminal Session",
            inputs=(
                NodeGraphPort("stdin", "stdin", port_type="terminal_input"),
                NodeGraphPort("command", "command", port_type="text"),
                NodeGraphPort("args", "args", port_type="text"),
                NodeGraphPort("control", "control", port_type="control"),
                NodeGraphPort("cwd", "cwd", port_type="text"),
                NodeGraphPort("env", "env", port_type="text"),
            ),
            outputs=(
                NodeGraphPort("stdout", "stdout", port_type="terminal_output"),
                NodeGraphPort("screen", "screen", port_type="text"),
                NodeGraphPort("stderr", "stderr", port_type="terminal_error"),
                NodeGraphPort("transcript", "transcript", port_type="stream:text"),
                NodeGraphPort("status", "status", port_type="status"),
                NodeGraphPort("exit_code", "exit_code", port_type="number"),
            ),
            subtitle="persistent process",
            status="stopped",
            color="#43c6ac",
            width=250,
            data=_template_data(
                "terminal",
                "stopped",
                [
                    {"key": "session_id", "label": "Session ID"},
                    {"key": "terminal_widget_id", "label": "Terminal View", "type": "select", "target_type": "terminal_widget", "placeholder": "Choose terminal widget..."},
                    {"key": "auto_start", "label": "Auto Start", "type": "bool", "default": False},
                    {"key": "restart_policy", "label": "Restart", "type": "select", "options": ["never", "on_exit", "on_error"], "default": "never"},
                ],
                session={"agent_type": "terminal", "command": None, "args": [], "cwd": None, "environment": {}},
                runtime_object="terminal_session",
            ),
        ),
        NodeGraphTemplate(
            "text_input",
            "Text Input",
            outputs=(NodeGraphPort("text", "text", port_type="text"),),
            subtitle="static text source",
            status="ready",
            color="#7aa2f7",
            width=210,
            data=_template_data(
                "text_input",
                "ready",
                [
                    {"key": "text", "label": "Text"},
                    {"key": "emit_on_start", "label": "Emit On Start", "type": "bool", "default": False},
                    {"key": "output_mode", "label": "Mode", "type": "select", "options": ["once", "manual"], "default": "manual"},
                ],
            ),
        ),
        NodeGraphTemplate(
            "codex_exec",
            "Codex Exec",
            inputs=(
                NodeGraphPort("prompt", "prompt", port_type="text"),
                NodeGraphPort("cwd", "cwd", port_type="text"),
                NodeGraphPort("model", "model", port_type="text"),
            ),
            outputs=(
                NodeGraphPort("final", "final", port_type="text"),
                NodeGraphPort("activity", "activity", port_type="text"),
                NodeGraphPort("raw_jsonl", "raw_jsonl", port_type="text"),
                NodeGraphPort("events", "events", port_type="json"),
                NodeGraphPort("stderr", "stderr", port_type="error"),
                NodeGraphPort("exit_code", "exit_code", port_type="number"),
                NodeGraphPort("ok", "ok", port_type="bool"),
            ),
            subtitle="codex exec json",
            status="idle",
            color="#bb9af7",
            width=260,
            data=_template_data(
                "codex_exec",
                "idle",
                [
                    {"key": "prompt", "label": "Prompt"},
                    {"key": "cwd", "label": "Working Dir"},
                    {"key": "model", "label": "Model"},
                    {"key": "codex_cmd", "label": "Codex Command", "default": "codex"},
                    {"key": "sandbox", "label": "Sandbox", "type": "select", "options": ["read-only", "workspace-write", "danger-full-access"], "default": "workspace-write"},
                    {"key": "bypass_approvals_and_sandbox", "label": "Bypass Approvals", "type": "bool", "default": False},
                    {"key": "skip_git_check", "label": "Skip Git Check", "type": "bool", "default": True},
                    {"key": "ephemeral", "label": "Ephemeral", "type": "bool", "default": False},
                    {"key": "extra_args", "label": "Extra Args"},
                    {"key": "timeout_seconds", "label": "Timeout Seconds", "type": "number", "default": 0},
                ],
            ),
        ),
        NodeGraphTemplate(
            "append_text",
            "Append Text",
            inputs=(
                NodeGraphPort("text", "text", port_type="text"),
                NodeGraphPort("appendix", "appendix", port_type="text"),
            ),
            outputs=(NodeGraphPort("text", "text", port_type="text"),),
            subtitle="add suffix",
            status="idle",
            color="#9ece6a",
            width=210,
            data=_template_data(
                "append_text",
                "idle",
                [
                    {"key": "appendix", "label": "Appendix"},
                    {"key": "separator", "label": "Separator", "default": "\n"},
                ],
            ),
        ),
        NodeGraphTemplate(
            "build_text",
            "Build Text",
            inputs=tuple(
                NodeGraphPort(f"part_{index}", f"part {index}", port_type="text")
                for index in range(1, _BUILD_TEXT_DEFAULT_INPUTS + 1)
            ),
            outputs=(NodeGraphPort("text", "text", port_type="text"),),
            subtitle="join text parts",
            status="idle",
            color="#9ece6a",
            width=230,
            data=_template_data(
                "build_text",
                "idle",
                [
                    {"key": "input_count", "label": "Inputs", "type": "number", "default": _BUILD_TEXT_DEFAULT_INPUTS},
                    {
                        "key": "separator",
                        "label": "Separator",
                        "type": "select",
                        "options": ["none", "space", "newline", "blank_line", "custom"],
                        "default": "blank_line",
                    },
                    {"key": "custom_separator", "label": "Custom Separator"},
                    {"key": "skip_empty", "label": "Skip Empty", "type": "bool", "default": True},
                    {"key": "trim_parts", "label": "Trim Parts", "type": "bool", "default": False},
                    {"key": "final_newline", "label": "Final Newline", "type": "bool", "default": False},
                ],
            ),
        ),
        NodeGraphTemplate(
            "extract_between_markers",
            "Extract Between Markers",
            inputs=(
                NodeGraphPort("text", "text", port_type="text"),
                NodeGraphPort("start_marker", "start_marker", port_type="text"),
                NodeGraphPort("end_marker", "end_marker", port_type="text"),
            ),
            outputs=(
                NodeGraphPort("match", "match", port_type="text"),
                NodeGraphPort("matches", "matches", port_type="text:list"),
                NodeGraphPort("before", "before", port_type="text"),
                NodeGraphPort("after", "after", port_type="text"),
                NodeGraphPort("found", "found", port_type="bool"),
            ),
            subtitle="marker parser",
            status="idle",
            color="#9ece6a",
            width=270,
            data=_template_data(
                "extract_between_markers",
                "idle",
                [
                    {"key": "start_marker", "label": "Start Marker", "default": "@to"},
                    {"key": "end_marker", "label": "End Marker", "default": "@end"},
                    {"key": "include_markers", "label": "Include Markers", "type": "bool", "default": False},
                    {"key": "max_matches", "label": "Max Matches", "type": "number", "default": 1},
                ],
            ),
        ),
        NodeGraphTemplate(
            "tui_screen_diff",
            "TUI Screen Diff",
            inputs=(NodeGraphPort("screen", "screen", port_type="text"),),
            outputs=(NodeGraphPort("text", "text", port_type="text"),),
            subtitle="rendered screen -> new text",
            status="watching",
            color="#9ece6a",
            width=240,
            data=_template_data(
                "tui_screen_diff",
                "watching",
                [
                    {"key": "drop_chrome", "label": "Drop Chrome", "type": "bool", "default": True},
                    {"key": "dedupe", "label": "Dedupe", "type": "bool", "default": True},
                    {"key": "emit_trailing_newline", "label": "Trailing Newline", "type": "bool", "default": False},
                ],
            ),
        ),
        NodeGraphTemplate(
            "envelope_parser",
            "Envelope Parser",
            inputs=(NodeGraphPort("text", "text", port_type="text"),),
            outputs=(
                NodeGraphPort("message", "message", port_type="message"),
                NodeGraphPort("to", "to", port_type="text"),
                NodeGraphPort("from", "from", port_type="text"),
                NodeGraphPort("type", "type", port_type="text"),
                NodeGraphPort("body", "body", port_type="text"),
                NodeGraphPort("id", "id", port_type="text"),
                NodeGraphPort("fields", "fields", port_type="json"),
                NodeGraphPort("malformed", "malformed", port_type="bool"),
                NodeGraphPort("duplicate", "duplicate", port_type="bool"),
            ),
            subtitle="structured messages",
            status="idle",
            color="#9ece6a",
            width=260,
            data=_template_data(
                "envelope_parser",
                "idle",
                [
                    {"key": "start_marker", "label": "Start Marker", "default": "@to"},
                    {"key": "end_marker", "label": "End Marker", "default": "@end"},
                    {"key": "streaming", "label": "Streaming", "type": "bool", "default": True},
                ],
            ),
        ),
        NodeGraphTemplate(
            "message_router",
            "Message Router",
            inputs=(
                NodeGraphPort("message", "message", port_type="message"),
                NodeGraphPort("rules", "rules", port_type="json"),
            ),
            outputs=(
                NodeGraphPort("default", "default", port_type="message"),
                NodeGraphPort("route_1", "route 1", port_type="message"),
                NodeGraphPort("route_2", "route 2", port_type="message"),
                NodeGraphPort("route_3", "route 3", port_type="message"),
            ),
            subtitle="field-based routing",
            status="idle",
            color="#ff9e64",
            width=250,
            data=_template_data(
                "message_router",
                "idle",
                [
                    {"key": "rules", "label": "Rules", "type": "json", "default": []},
                    {"key": "default_target", "label": "Default Target", "default": "default"},
                ],
            ),
        ),
        NodeGraphTemplate(
            "approval_gate",
            "Approval Gate",
            inputs=(
                NodeGraphPort("message", "message", port_type="message"),
                NodeGraphPort("summary", "summary", port_type="text"),
                NodeGraphPort("risk", "risk", port_type="text"),
            ),
            outputs=(
                NodeGraphPort("approved", "approved", port_type="message"),
                NodeGraphPort("rejected", "rejected", port_type="message"),
                NodeGraphPort("edited", "edited", port_type="message"),
                NodeGraphPort("needs_user", "needs_user", port_type="event"),
            ),
            subtitle="human checkpoint",
            status="waiting",
            color="#e0af68",
            width=240,
            data=_template_data(
                "approval_gate",
                "waiting",
                [
                    {"key": "requires_human", "label": "Requires Human", "type": "bool", "default": True},
                    {"key": "risk_label", "label": "Risk Label", "default": "review"},
                    {"key": "allow_edit", "label": "Allow Edit", "type": "bool", "default": True},
                ],
                safety_policy={"requires_human": True},
            ),
        ),
        NodeGraphTemplate(
            "log",
            "Log",
            inputs=(NodeGraphPort("value", "value", port_type="json"),),
            outputs=(NodeGraphPort("value", "value", port_type="json"),),
            subtitle="show value",
            status="recording",
            color="#2ac3de",
            width=190,
            data=_template_data(
                "log",
                "recording",
                [
                    {"key": "label", "label": "Label", "default": "Log"},
                    {"key": "persist", "label": "Persist", "type": "bool", "default": False},
                ],
            ),
        ),
        NodeGraphTemplate(
            "probe",
            "Probe",
            inputs=(NodeGraphPort("value", "value", port_type="json"),),
            outputs=(NodeGraphPort("value", "value", port_type="json"),),
            subtitle="wire monitor",
            status="watching",
            color="#2ac3de",
            width=190,
            data=_template_data(
                "probe",
                "watching",
                [
                    {"key": "label", "label": "Label", "default": "Probe"},
                    {"key": "show_inline", "label": "Show Inline", "type": "bool", "default": True},
                ],
            ),
        ),
        NodeGraphTemplate(
            "widget_sink",
            "Widget Sink",
            inputs=(NodeGraphPort("value", "text", port_type="text"),),
            outputs=(NodeGraphPort("value", "text", port_type="text"),),
            subtitle="update GUI widget",
            status="watching",
            color="#2ac3de",
            width=220,
            data=_template_data(
                "widget_sink",
                "watching",
                [
                    {"key": "widget_id", "label": "Widget ID"},
                    {
                        "key": "widget_type",
                        "label": "Widget Type",
                        "type": "select",
                        "options": ["", "label", "badge", "log_view", "text_input", "text_area", "code_editor", "led"],
                        "default": "",
                    },
                    {
                        "key": "port_profile",
                        "label": "Port Profile",
                        "type": "select",
                        "options": list(_NODE_GRAPH_WIDGET_SINK_PORT_PROFILES),
                        "default": "text",
                    },
                    {
                        "key": "update_mode",
                        "label": "Update",
                        "type": "select",
                        "options": ["auto", "set", "append", "state"],
                        "default": "auto",
                    },
                    {
                        "key": "format",
                        "label": "Format",
                        "type": "select",
                        "options": ["text", "terminal_text", "json", "repr", "message_body"],
                        "default": "text",
                    },
                ],
            ),
        ),
        NodeGraphTemplate(
            "terminal_log_sink",
            "Terminal Log Sink",
            inputs=(NodeGraphPort("value", "terminal_output", port_type="terminal_output"),),
            outputs=(NodeGraphPort("value", "terminal_output", port_type="terminal_output"),),
            subtitle="terminal stdout -> GUI log",
            status="watching",
            color="#2ac3de",
            width=240,
            data=_template_data(
                "widget_sink",
                "watching",
                [
                    {"key": "widget_id", "label": "Widget ID"},
                    {
                        "key": "widget_type",
                        "label": "Widget Type",
                        "type": "select",
                        "options": ["", "log_view"],
                        "default": "log_view",
                    },
                    {
                        "key": "port_profile",
                        "label": "Port Profile",
                        "type": "select",
                        "options": ["terminal_output", "text"],
                        "default": "terminal_output",
                    },
                    {
                        "key": "update_mode",
                        "label": "Update",
                        "type": "select",
                        "options": ["append", "auto", "set"],
                        "default": "append",
                    },
                    {
                        "key": "format",
                        "label": "Format",
                        "type": "select",
                        "options": ["clean_stream_text", "stream_text", "terminal_text", "text", "repr"],
                        "default": "clean_stream_text",
                    },
                ],
                template_id="terminal_log_sink",
                config={
                    "widget_type": "log_view",
                    "port_profile": "terminal_output",
                    "update_mode": "append",
                    "format": "clean_stream_text",
                },
            ),
        ),
        NodeGraphTemplate(
            "widget_source",
            "Widget Source",
            outputs=(NodeGraphPort("value", "text", port_type="text"),),
            subtitle="read GUI widget",
            status="ready",
            color="#7aa2f7",
            width=220,
            data=_template_data(
                "widget_source",
                "ready",
                [
                    {"key": "widget_id", "label": "Widget ID"},
                    {
                        "key": "widget_type",
                        "label": "Widget Type",
                        "type": "select",
                        "options": ["", "label", "badge", "log_view", "text_input", "text_area", "code_editor", "led"],
                        "default": "",
                    },
                    {
                        "key": "port_profile",
                        "label": "Port Profile",
                        "type": "select",
                        "options": list(_NODE_GRAPH_WIDGET_SOURCE_PORT_PROFILES),
                        "default": "text",
                    },
                    {
                        "key": "format",
                        "label": "Format",
                        "type": "select",
                        "options": ["text", "json", "repr", "raw"],
                        "default": "text",
                    },
                ],
            ),
        ),
        NodeGraphTemplate(
            "agent",
            "Agent",
            inputs=(
                NodeGraphPort("in", "message", port_type="message"),
                NodeGraphPort("approval", "approval_result", port_type="approval_result"),
                NodeGraphPort("artifact", "artifact", port_type="artifact"),
            ),
            outputs=(
                NodeGraphPort("out", "message", port_type="message"),
                NodeGraphPort("approval_request", "approval_request", port_type="approval_request"),
                NodeGraphPort("test_request", "test_request", port_type="test_request"),
                NodeGraphPort("artifact", "artifact", port_type="artifact"),
                NodeGraphPort("error", "error", port_type="error"),
            ),
            subtitle="agent session",
            status="idle",
            color="#7aa2f7",
            width=230,
            data=_template_data(
                "agent",
                "idle",
                [
                    {"key": "agent_type", "label": "Agent Type", "default": "codex"},
                    {"key": "role", "label": "Role"},
                    {"key": "model", "label": "Model"},
                    {"key": "requires_approval", "label": "Requires Approval", "type": "bool", "default": True},
                ],
                session={
                    "agent_type": "codex",
                    "capabilities": {"terminal": True, "tools": True},
                    "safety_policy": {"requires_approval": True},
                },
            ),
        ),
        NodeGraphTemplate(
            "parser",
            "Parser",
            inputs=(NodeGraphPort("in", "terminal_output", port_type="terminal_output"),),
            outputs=(
                NodeGraphPort("message", "message", port_type="message"),
                NodeGraphPort("error", "error", port_type="error"),
            ),
            subtitle="envelope parser",
            status="idle",
            color="#9ece6a",
            data=_template_data(
                "parser",
                "idle",
                [
                    {"key": "start_marker", "label": "Start Marker", "default": "@to"},
                    {"key": "end_marker", "label": "End Marker", "default": "@end"},
                ],
            ),
        ),
        NodeGraphTemplate(
            "tester",
            "Tester",
            inputs=(
                NodeGraphPort("request", "test_request", port_type="test_request"),
                NodeGraphPort("artifact", "artifact", port_type="artifact"),
            ),
            outputs=(
                NodeGraphPort("report", "test_report", port_type="test_report"),
                NodeGraphPort("error", "error", port_type="error"),
            ),
            subtitle="verification",
            status="idle",
            color="#bb9af7",
            data=_template_data(
                "tester",
                "idle",
                [
                    {"key": "command", "label": "Test Command", "default": "py -3 -m pytest"},
                    {"key": "cwd", "label": "Working Dir"},
                ],
            ),
        ),
        NodeGraphTemplate(
            "artifact",
            "Artifact",
            inputs=(NodeGraphPort("in", "artifact", port_type="artifact"),),
            outputs=(NodeGraphPort("out", "artifact", port_type="artifact"),),
            subtitle="produced file",
            status="ready",
            color="#f7768e",
            data=_template_data(
                "artifact",
                "ready",
                [{"key": "output_path", "label": "Output Path"}],
            ),
        ),
        NodeGraphTemplate(
            "human_input",
            "Human Input",
            inputs=(NodeGraphPort("prompt", "message", port_type="message"),),
            outputs=(
                NodeGraphPort("message", "message", port_type="message"),
                NodeGraphPort("approval", "approval_result", port_type="approval_result"),
            ),
            subtitle="operator response",
            status="waiting",
            color="#2ac3de",
            width=220,
            data=_template_data(
                "human_input",
                "waiting",
                [{"key": "prompt", "label": "Prompt", "default": "What should happen next?"}],
            ),
        ),
        NodeGraphTemplate(
            "rule",
            "Rule",
            inputs=(NodeGraphPort("in", "message", port_type="message"),),
            outputs=(
                NodeGraphPort("out", "message", port_type="message"),
                NodeGraphPort("error", "error", port_type="error"),
            ),
            subtitle="policy",
            status="active",
            color="#ff9e64",
            data=_template_data(
                "rule",
                "active",
                [
                    {"key": "match_type", "label": "Match Type"},
                    {"key": "target", "label": "Target"},
                ],
            ),
        ),
    )

def _port_by_id(ports: Sequence[NodeGraphPort], port_id: str) -> NodeGraphPort | None:
    for port in ports:
        if port.id == port_id:
            return port
    return None


_RUNTIME_OBJECT_ID_KEYS: tuple[tuple[str, str | None], ...] = (
    ("object_id", None),
    ("session_id", "terminal_session"),
    ("queue_id", "message_queue"),
    ("store_id", "memory_store"),
    ("watcher_id", "file_watcher"),
    ("recorder_id", "transcript_recorder"),
)

_RUNTIME_OBJECT_REF_KEYS: tuple[tuple[str, str | None], ...] = (
    ("object_ref", None),
    ("session_ref", "terminal_session"),
    ("queue_ref", "message_queue"),
    ("store_ref", "memory_store"),
    ("watcher_ref", "file_watcher"),
    ("recorder_ref", "transcript_recorder"),
)
_RUNTIME_SOURCE_NODE_TYPES = {
    "text_input",
    "widget_source",
}
_RUNTIME_EXECUTABLE_NODE_TYPES = {
    "append_text",
    "build_text",
    "codex_exec",
    "extract_between_markers",
    "tui_screen_diff",
    "envelope_parser",
    "parser",
    "message_router",
    "log",
    "probe",
    "widget_sink",
}
_TERMINAL_CONFIG_INPUT_PORTS = {"command", "args", "cwd", "env"}

_WIDGET_SINK_SET_TYPES = {
    "label",
    "badge",
    "text_input",
    "text_area",
    "code_editor",
}

_WIDGET_SINK_APPEND_TYPES = {"log_view"}

_ANSI_CONTROL_SEQUENCE_RE = re.compile(
    r"\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1b]*(?:\x07|\x1b\\)|[PX^_].*?\x1b\\|[@-Z\\-_])",
    re.DOTALL,
)
_ANSI_TERMINAL_LINE_REWRITE_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[GK]")
_TERMINAL_CONTROL_CHAR_RE = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")
_BUILD_TEXT_MIN_INPUTS = 1
_BUILD_TEXT_DEFAULT_INPUTS = 3
_BUILD_TEXT_MAX_INPUTS = 12


def _terminal_display_text(value: object) -> str:
    text = str(value)
    text = _ANSI_TERMINAL_LINE_REWRITE_RE.sub("\r", text)
    text = _ANSI_CONTROL_SEQUENCE_RE.sub("", text)
    return _TERMINAL_CONTROL_CHAR_RE.sub("", text)


def _terminal_clean_stream_text(value: object) -> str:
    text = str(value)
    text = _ANSI_CONTROL_SEQUENCE_RE.sub("", text)
    return _TERMINAL_CONTROL_CHAR_RE.sub("", text)

def _widget_kind(widget: object) -> str:
    kind = getattr(widget, "kind", None)
    if kind is not None and str(kind).strip():
        return str(kind).strip()
    return type(widget).__name__


def _target_data_is_auto_binding(data: Mapping[str, object] | None) -> bool:
    return bool(isinstance(data, Mapping) and data.get(_NODE_GRAPH_AUTO_BINDING_DATA_KEY))


def _binding_target_label(widget: object) -> str:
    for attr in ("text", "label", "title", "tooltip", "placeholder"):
        value = getattr(widget, attr, None)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return _humanize_identifier(str(getattr(widget, "id", "") or _widget_kind(widget)))


def _humanize_identifier(value: str) -> str:
    text = re.sub(r"[_\-]+", " ", value).strip()
    if not text:
        return "Target"
    return " ".join(part[:1].upper() + part[1:] for part in text.split())


def _widget_is_action_target(widget: object) -> bool:
    return callable(getattr(widget, "on_click", None)) and not bool(getattr(widget, "disabled", False))


def _widget_action_callback(widget: object) -> Callable[[str, str], object]:
    click = getattr(widget, "click", None)
    on_click = getattr(widget, "on_click", None)

    def callback(_action_id: str, _command: str) -> object:
        if callable(click):
            return click()
        if callable(on_click):
            return on_click()
        return None

    return callback


def _auto_widget_target_capabilities(
    widget_kind: str,
) -> tuple[tuple[str, ...], str, tuple[str, ...], str, tuple[str, ...]]:
    kind = str(widget_kind).strip()
    if kind == "log_view":
        return (
            ("append", "set"),
            "append",
            _NODE_GRAPH_WIDGET_SINK_PORT_PROFILES,
            "text",
            ("text", "terminal_text", "message_body", "json", "repr"),
        )
    if kind in {"text_input", "text_area", "code_editor"}:
        return (
            ("set",),
            "set",
            _NODE_GRAPH_WIDGET_SOURCE_PORT_PROFILES,
            "text",
            ("text", "message_body", "json", "repr"),
        )
    if kind == "led":
        return (
            ("set",),
            "set",
            ("status", "bool", "text", "json"),
            "status",
            ("text", "json", "repr"),
        )
    return (
        ("set",),
        "set",
        ("text", "status", "message", "json", "artifact", "error"),
        "text",
        ("text", "message_body", "json", "repr"),
    )


def _widget_sink_text(value: object, value_format: str) -> str:
    mode = str(value_format or "text").strip()
    if mode == "json":
        return json.dumps(_json_safe_value(value), sort_keys=True)
    if mode == "repr":
        return repr(value)
    if mode == "message_body" and isinstance(value, Mapping):
        body = value.get("body")
        if body is not None:
            return str(body)
    return str(value)


def _update_widget_sink(widget: object, value: object, *, update_mode: str, value_format: str) -> str:
    kind = _widget_kind(widget)
    mode = str(update_mode or "auto").strip()
    if mode == "auto":
        if kind in _WIDGET_SINK_APPEND_TYPES:
            mode = "append"
        elif kind == "led":
            mode = "state"
        else:
            mode = "set"
    if mode == "append":
        text = _widget_sink_text(value, value_format)
        if value_format == "clean_stream_text":
            text = _terminal_clean_stream_text(value)
            append_stream = getattr(widget, "append_stream", None)
            if not callable(append_stream):
                append_stream = getattr(widget, "append_text", None)
            if not callable(append_stream):
                append_stream = getattr(widget, "write", None)
            if callable(append_stream):
                append_stream(text)
                return mode
        if value_format == "stream_text":
            append_stream = getattr(widget, "append_stream", None)
            if not callable(append_stream):
                append_stream = getattr(widget, "append_text", None)
            if not callable(append_stream):
                append_stream = getattr(widget, "write", None)
            if callable(append_stream):
                append_stream(text)
                return mode
        if value_format == "terminal_text":
            append_stream = getattr(widget, "append_terminal_text", None)
            if not callable(append_stream):
                text = _terminal_display_text(value)
                append_stream = getattr(widget, "append_stream", None)
            if not callable(append_stream):
                append_stream = getattr(widget, "append_text", None)
            if not callable(append_stream):
                append_stream = getattr(widget, "write", None)
            if callable(append_stream):
                append_stream(text)
                return mode
        append_line = getattr(widget, "append_line", None)
        if not callable(append_line):
            raise TypeError(f"widget type {kind!r} does not support append")
        append_line(text)
        return mode
    if mode == "set":
        set_value = getattr(widget, "set_value", None)
        if not callable(set_value):
            raise TypeError(f"widget type {kind!r} does not support set")
        set_value(_widget_sink_text(value, value_format))
        return mode
    if mode == "state":
        set_state = getattr(widget, "set_state", None)
        if not callable(set_state):
            raise TypeError(f"widget type {kind!r} does not support state")
        set_state(value)
        return mode
    raise ValueError(f"unsupported widget sink update mode {mode!r}")

def _read_widget_raw_value(widget: object) -> object:
    for attr in ("value", "text", "checked", "state"):
        if hasattr(widget, attr):
            return getattr(widget, attr)
    lines = getattr(widget, "lines", None)
    if isinstance(lines, Sequence) and not isinstance(lines, (str, bytes, bytearray)):
        return "\n".join(str(line) for line in lines)
    raise TypeError(f"widget type {_widget_kind(widget)!r} does not expose a readable value")


def _widget_source_value(widget: object, value_format: str) -> object:
    raw = _read_widget_raw_value(widget)
    mode = str(value_format or "text").strip()
    if mode == "raw":
        return _json_safe_value(raw)
    if mode == "json":
        if isinstance(raw, str):
            text = raw.strip()
            if text:
                try:
                    return _json_safe_value(json.loads(text))
                except json.JSONDecodeError:
                    return raw
        return _json_safe_value(raw)
    if mode == "repr":
        return repr(raw)
    if raw is None:
        return ""
    return str(raw)
def _required_text(value: object, name: str) -> str:
    if value is None:
        raise ValueError(f"{name} is required")
    text = str(value).strip()
    if not text:
        raise ValueError(f"{name} is required")
    return text


def _mapping_copy(value: object, context: str) -> dict[str, object] | None:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise TypeError(f"{context} must be a mapping")
    return _json_copy(dict(value), context)


def _sequence_config(value: object, context: str) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        return (value,)
    if isinstance(value, (bytes, bytearray)) or not isinstance(value, Sequence):
        raise TypeError(f"{context} must be a sequence")
    return tuple(str(item) for item in value)


def _node_config(node: NodeGraphNode) -> dict[str, object]:
    data = node.data or {}
    config = data.get("config") if isinstance(data, Mapping) else None
    return _mapping_copy(config, "node config") or {}


def _node_type(node: NodeGraphNode) -> str:
    data = node.data or {}
    if isinstance(data, Mapping):
        for key in ("node_type", "template_id", "kind"):
            value = data.get(key)
            if value is not None and str(value).strip():
                return str(value).strip()
    return "node"


def _runtime_view_type(binding: NodeGraphNodeBinding, config: Mapping[str, object]) -> str | None:
    value = config.get("view_type")
    if value is not None and str(value).strip():
        return str(value).strip()
    node_type_views = {
        "terminal": "terminal",
        "terminal_session": "terminal",
        "envelope_parser": "parser_trace",
        "parser": "parser_trace",
        "message_router": "queue",
        "router": "queue",
        "approval_gate": "approval",
        "log": "event_log",
        "probe": "event_log",
        "artifact": "artifact_list",
        "recorder": "artifact_list",
        "tester": "test_results",
        "command_runner": "test_results",
    }
    if binding.node_type in node_type_views:
        return node_type_views[binding.node_type]
    object_type_views = {
        "terminal_session": "terminal",
        "message_queue": "queue",
        "transcript_recorder": "artifact_list",
    }
    for ref in binding.object_refs:
        if ref.object_type in object_type_views:
            return object_type_views[ref.object_type]
    return None


def _section_config(section: NodeGraphSection) -> dict[str, object]:
    data = section.data or {}
    if not isinstance(data, Mapping):
        return {}
    config = data.get("config")
    if isinstance(config, Mapping):
        return _json_copy(dict(config), "section config")
    return _json_copy(dict(data), "section data") if data else {}


def _node_runtime_binding(node: NodeGraphNode) -> NodeGraphNodeBinding:
    config = _node_config(node)
    data = node.data or {}
    if isinstance(data, Mapping):
        for key in ("view_type",):
            value = data.get(key)
            if value is not None and str(value).strip() and key not in config:
                config[key] = str(value).strip()
    owned = _runtime_object_from_node(node)
    refs = _runtime_refs_from_node(node)
    return NodeGraphNodeBinding(
        node_id=node.id,
        node_type=_node_type(node),
        title=node.title,
        status=node.status,
        config=config or None,
        owned_object_id=None if owned is None else owned.object_id,
        object_refs=refs,
    )


def _runtime_node_from_binding(binding: NodeGraphNodeBinding) -> NodeGraphNode:
    data: dict[str, object] = {"node_type": binding.node_type}
    if binding.config is not None:
        data["config"] = _json_copy(binding.config, "runtime node config")
    return NodeGraphNode(
        binding.node_id,
        binding.title,
        0,
        0,
        status=binding.status,
        data=data,
    )


def _section_runtime_binding(section: NodeGraphSection, node_ids: Sequence[str]) -> NodeGraphSectionBinding:
    config = _section_config(section)
    return NodeGraphSectionBinding(
        section_id=section.id,
        title=section.title,
        node_ids=tuple(str(node_id) for node_id in node_ids),
        purpose=section.purpose,
        trigger=section.trigger,
        config=config or None,
    )


def _convert_edge_value(edge: NodeGraphRuntimeEdgeBinding, value: object) -> object:
    if edge.conversion == "text_to_terminal_input":
        return "" if value is None else str(value)
    return value


def _terminal_config_input_value(port_id: str, value: object) -> object:
    key = str(port_id)
    if key == "command":
        text = "" if value is None else str(value).strip()
        if not text:
            raise ValueError("terminal command input cannot be empty")
        return text
    if key == "args":
        if value is None:
            return []
        if isinstance(value, str):
            text = value.strip()
            if not text:
                return []
            if text.startswith("["):
                parsed = json.loads(text)
                if not isinstance(parsed, Sequence) or isinstance(parsed, (str, bytes, bytearray)):
                    raise ValueError("terminal args JSON input must be a list")
                return [str(item) for item in parsed]
            return text.split()
        if isinstance(value, (bytes, bytearray)) or not isinstance(value, Sequence):
            raise ValueError("terminal args input must be text or a sequence")
        return [str(item) for item in value]
    if key == "cwd":
        text = "" if value is None else str(value).strip()
        return None if not text else text
    if key == "env":
        if value is None or value == "":
            return {}
        parsed = json.loads(value) if isinstance(value, str) else value
        if not isinstance(parsed, Mapping):
            raise ValueError("terminal env input must be a mapping or JSON object")
        return {str(env_key): str(env_value) for env_key, env_value in parsed.items()}
    return value


def _edge_runtime_binding(edge: NodeGraphEdge, index: int, nodes: Sequence[NodeGraphNode] = ()) -> NodeGraphRuntimeEdgeBinding:
    data = _edge_data(edge)
    conversion = _edge_conversion(edge, nodes)
    return NodeGraphRuntimeEdgeBinding(
        edge_id=edge.id or f"edge-{index + 1}",
        source_node=edge.source_node,
        source_port=edge.source_port,
        target_node=edge.target_node,
        target_port=edge.target_port,
        label=edge.label,
        conversion=conversion,
        config=data or None,
    )


def _runtime_object_type_from_key(key_type: str | None, explicit_type: object) -> str | None:
    if explicit_type is not None:
        text = str(explicit_type).strip()
        if text:
            return text
    return key_type


def _runtime_object_id_from_sources(
    config: Mapping[str, object], data: Mapping[str, object]
) -> tuple[str | None, str | None, str | None]:
    for key, key_type in _RUNTIME_OBJECT_ID_KEYS:
        value = config.get(key, data.get(key))
        if value is None:
            continue
        text = str(value).strip()
        if text:
            return text, key, key_type
    return None, None, None


def _runtime_object_is_persistent(obj: NodeGraphRuntimeObject) -> bool:
    object_type = str(obj.object_type or "").strip().lower()
    if object_type in _PERSISTENT_RUNTIME_OBJECT_TYPES:
        return True
    config = obj.config or {}
    if isinstance(config, Mapping):
        value = config.get("persistent", config.get("keep_alive"))
        if value is not None:
            return bool(value)
    return False


def _runtime_object_from_node(node: NodeGraphNode) -> NodeGraphRuntimeObject | None:
    data = node.data or {}
    if not isinstance(data, Mapping):
        return None
    config = _node_config(node)
    object_id, _, key_type = _runtime_object_id_from_sources(config, data)
    if object_id is None and _node_type(node) == "terminal":
        terminal_widget_id = str(config.get("terminal_widget_id", "") or "").strip()
        if terminal_widget_id:
            object_id = node.id
            key_type = "terminal_session"
    if object_id is None:
        return None
    object_type = _runtime_object_type_from_key(key_type, data.get("runtime_object", data.get("object_type")))
    if object_type is None:
        object_type = str(data.get("node_type", "runtime_object"))
    status = node.status or (None if data.get("default_status") is None else str(data.get("default_status")))
    return NodeGraphRuntimeObject(
        object_id=object_id,
        object_type=object_type,
        owner_node_id=node.id,
        status=status,
        config=config or None,
    )


def _missing_ref_from_value(value: NodeGraphRuntimeObjectRef | Mapping[str, object]) -> NodeGraphRuntimeObjectRef:
    if isinstance(value, NodeGraphRuntimeObjectRef):
        return value
    if not isinstance(value, Mapping):
        raise TypeError("runtime object refs must be reference records or mappings")
    return NodeGraphRuntimeObjectRef(
        node_id=_required_text(value.get("node_id", value.get("node")), "node_id"),
        object_id=_required_text(value.get("object_id", value.get("id")), "object_id"),
        object_type=None if value.get("object_type", value.get("type")) is None else str(value.get("object_type", value.get("type"))),
        key=None if value.get("key") is None else str(value.get("key")),
    )


def _runtime_ref_from_mapping(
    node: NodeGraphNode, key: str, value: Mapping[str, object], fallback_type: str | None = None
) -> NodeGraphRuntimeObjectRef | None:
    object_id = value.get("object_id", value.get("id"))
    if object_id is None:
        return None
    object_text = str(object_id).strip()
    if not object_text:
        return None
    object_type = _runtime_object_type_from_key(fallback_type, value.get("object_type", value.get("type")))
    return NodeGraphRuntimeObjectRef(node.id, object_text, object_type, key)


def _runtime_refs_from_node(node: NodeGraphNode) -> tuple[NodeGraphRuntimeObjectRef, ...]:
    data = node.data or {}
    if not isinstance(data, Mapping):
        return ()
    config = _node_config(node)
    refs: list[NodeGraphRuntimeObjectRef] = []
    for key in ("runtime_ref", "object_ref"):
        value = data.get(key, config.get(key))
        if isinstance(value, Mapping):
            ref = _runtime_ref_from_mapping(node, key, value)
            if ref is not None:
                refs.append(ref)
    explicit_ref_type = data.get("runtime_ref_type", data.get("object_ref_type"))
    for key, key_type in _RUNTIME_OBJECT_REF_KEYS:
        value = config.get(key, data.get(key))
        if value is None or isinstance(value, Mapping):
            continue
        text = str(value).strip()
        if not text:
            continue
        object_type = _runtime_object_type_from_key(key_type, explicit_ref_type)
        refs.append(NodeGraphRuntimeObjectRef(node.id, text, object_type, key))
    return tuple(refs)


def _run_node_graph_text_flow(
    graph: NodeGraph,
    initial_inputs: Mapping[str, object],
    *,
    max_steps: int,
    registry: NodeGraphObjectRegistry | None,
) -> NodeGraphFlowRun:
    if max_steps <= 0:
        raise ValueError("max_steps must be positive")
    binding = graph.runtime_binding(registry)
    input_values: dict[tuple[str, str], list[object]] = {}
    output_values: dict[tuple[str, str], list[object]] = {}
    consumed: dict[tuple[str, str], int] = {}
    edge_consumed: dict[tuple[str, str, str, str], int] = {}
    log: list[dict[str, object]] = []
    parser_state: dict[str, AgentEnvelopeParser] = {}

    for key, value in initial_inputs.items():
        if "." not in str(key):
            raise ValueError("initial input keys must use node_id.port_id format")
        node_id, port_id = str(key).split(".", 1)
        _append_flow_value(input_values, node_id, port_id, value)

    for node in graph.nodes:
        if _node_type(node) == "text_input":
            emitted = _execute_flow_node(node, {}, parser_state, log)
            _record_flow_outputs(output_values, node, emitted, log)

    edge_map = _flow_edge_map(graph.edges)
    for step in range(max_steps):
        progressed = False
        _propagate_flow_edges(output_values, input_values, edge_map, edge_consumed)
        for node in graph.nodes:
            inputs = _pending_flow_inputs(node, input_values, consumed)
            if not inputs:
                continue
            emitted = _execute_flow_node(node, inputs, parser_state, log)
            if emitted:
                _record_flow_outputs(output_values, node, emitted, log)
                progressed = True
        if not progressed:
            _propagate_flow_edges(output_values, input_values, edge_map, edge_consumed)
            break
    else:
        log.append({"event": "max_steps_reached", "max_steps": max_steps})

    return NodeGraphFlowRun(
        values={f"{node}.{port}": [_json_safe_value(item) for item in items] for (node, port), items in output_values.items()},
        log=tuple(log),
        binding=binding,
    )


def _flow_edge_map(edges: Sequence[NodeGraphEdge]) -> dict[tuple[str, str], list[tuple[str, str]]]:
    edge_map: dict[tuple[str, str], list[tuple[str, str]]] = {}
    for edge in edges:
        edge_map.setdefault((edge.source_node, edge.source_port), []).append((edge.target_node, edge.target_port))
    return edge_map


def _append_flow_value(values: dict[tuple[str, str], list[object]], node_id: str, port_id: str, value: object) -> None:
    values.setdefault((node_id, port_id), []).append(value)


def _record_flow_outputs(
    values: dict[tuple[str, str], list[object]], node: NodeGraphNode, outputs: Mapping[str, list[object]], log: list[dict[str, object]]
) -> None:
    for port_id, items in outputs.items():
        for item in items:
            _append_flow_value(values, node.id, port_id, item)
            log.append({"event": "emit", "node": node.id, "port": port_id, "value": _json_safe_value(item)})


def _propagate_flow_edges(
    output_values: dict[tuple[str, str], list[object]],
    input_values: dict[tuple[str, str], list[object]],
    edge_map: Mapping[tuple[str, str], Sequence[tuple[str, str]]],
    consumed: dict[tuple[str, str, str, str], int],
) -> None:
    for source, targets in edge_map.items():
        source_values = output_values.get(source, [])
        if not source_values:
            continue
        for target in targets:
            edge_key = (source[0], source[1], target[0], target[1])
            start = consumed.get(edge_key, 0)
            if len(source_values) <= start:
                continue
            input_values.setdefault(target, []).extend(source_values[start:])
            consumed[edge_key] = len(source_values)


def _pending_flow_inputs(
    node: NodeGraphNode, values: Mapping[tuple[str, str], list[object]], consumed: dict[tuple[str, str], int]
) -> dict[str, list[object]]:
    pending: dict[str, list[object]] = {}
    for port in node.inputs:
        key = (node.id, port.id)
        items = values.get(key, [])
        start = consumed.get(key, 0)
        if len(items) > start:
            pending[port.id] = items[start:]
            consumed[key] = len(items)
    return pending


def _execute_flow_node(
    node: NodeGraphNode,
    inputs: Mapping[str, list[object]],
    parser_state: dict[str, object],
    log: list[dict[str, object]],
) -> dict[str, list[object]]:
    node_type = _node_type(node)
    config = _node_config(node)
    if node_type == "text_input":
        text = str(config.get("text", ""))
        return {"text": [text]} if text else {}
    if node_type == "append_text":
        separator = str(config.get("separator", ""))
        text_values = _flow_input_values(inputs, "text", "in")
        appendix_values = _flow_input_values(inputs, "appendix")
        if not appendix_values:
            appendix_values = [config.get("appendix", "")]
        output: list[object] = []
        for value in text_values:
            for appendix_value in appendix_values:
                appendix = str(appendix_value)
                output.append(str(value) + (separator if appendix else "") + appendix)
        return {"text": output}
    if node_type == "build_text":
        return _execute_build_text(inputs, config)
    if node_type == "codex_exec":
        return _execute_codex_exec(inputs, config, log)
    if node_type == "extract_between_markers":
        return _execute_extract_between_markers(inputs, config)
    if node_type == "tui_screen_diff":
        return _execute_tui_screen_diff(node, inputs, config, parser_state, log)
    if node_type in {"envelope_parser", "parser"}:
        return _execute_envelope_parser(node, inputs, parser_state, log)
    if node_type == "message_router":
        return _execute_message_router(inputs, config, log)
    if node_type == "log":
        items = _flow_input_values(inputs, "value", "in", "message", "text")
        for item in items:
            log.append({"event": "log", "node": node.id, "value": _json_safe_value(item)})
        return {"value": list(items)}
    if node_type == "probe":
        items = _flow_input_values(inputs, "value", "in", "message", "text")
        for item in items:
            log.append({"event": "probe", "node": node.id, "value": _json_safe_value(item)})
        return {"value": list(items)}
    if inputs:
        log.append({"event": "node_skipped", "node": node.id, "node_type": node_type})
    return {}


def _flow_input_values(inputs: Mapping[str, list[object]], *ports: str) -> list[object]:
    result: list[object] = []
    for port in ports:
        result.extend(inputs.get(port, []))
    return result


def _build_text_input_count(config: Mapping[str, object]) -> int:
    try:
        count = int(float(config.get("input_count", _BUILD_TEXT_DEFAULT_INPUTS)))
    except (TypeError, ValueError):
        count = _BUILD_TEXT_DEFAULT_INPUTS
    return max(_BUILD_TEXT_MIN_INPUTS, min(_BUILD_TEXT_MAX_INPUTS, count))


def _build_text_separator(config: Mapping[str, object]) -> str:
    mode = str(config.get("separator", "blank_line")).strip()
    if mode == "none":
        return ""
    if mode == "space":
        return " "
    if mode == "newline":
        return "\n"
    if mode == "custom":
        return str(config.get("custom_separator", ""))
    return "\n\n"


def _execute_build_text(
    inputs: Mapping[str, list[object]], config: Mapping[str, object]
) -> dict[str, list[object]]:
    parts: list[str] = []
    trim_parts = bool(config.get("trim_parts", False))
    skip_empty = bool(config.get("skip_empty", True))
    for index in range(1, _build_text_input_count(config) + 1):
        values = _flow_input_values(inputs, f"part_{index}")
        if not values and f"part_{index}" in config:
            values = [config.get(f"part_{index}", "")]
        for value in values:
            text = str(value)
            if trim_parts:
                text = text.strip()
            if skip_empty and not text:
                continue
            parts.append(text)
    if not parts:
        return {}
    output = _build_text_separator(config).join(parts)
    if bool(config.get("final_newline", False)) and not output.endswith("\n"):
        output += "\n"
    return {"text": [output]}


def _execute_extract_between_markers(inputs: Mapping[str, list[object]], config: Mapping[str, object]) -> dict[str, list[object]]:
    start = str(config.get("start_marker", "@to"))
    end = str(config.get("end_marker", "@end"))
    max_matches = int(config.get("max_matches", 1) or 1)
    include = bool(config.get("include_markers", False))
    outputs: dict[str, list[object]] = {"match": [], "matches": [], "before": [], "after": [], "found": []}
    for value in _flow_input_values(inputs, "text", "in"):
        text = str(value)
        matches: list[str] = []
        cursor = 0
        first_before = ""
        last_after = ""
        while len(matches) < max_matches:
            start_index = text.find(start, cursor)
            if start_index < 0:
                break
            body_start = start_index + len(start)
            end_index = text.find(end, body_start)
            if end_index < 0:
                break
            if not matches:
                first_before = text[:start_index]
            last_after = text[end_index + len(end):]
            matches.append(text[start_index:end_index + len(end)] if include else text[body_start:end_index])
            cursor = end_index + len(end)
        outputs["found"].append(bool(matches))
        outputs["matches"].append(matches)
        if matches:
            outputs["match"].append(matches[0])
            outputs["before"].append(first_before)
            outputs["after"].append(last_after)
    return outputs


_TUI_BOX_CODE_RANGES = (
    (0x2500, 0x257F),
    (0x2550, 0x256C),
)
_TUI_ASCII_FRAME_CHARS = set("+-|=:_.,[]()<>/\\")
_TUI_CHROME_PHRASES = (
    "openai codex",
    "/model to change",
    "directory:",
    "use /skills",
    "esc to interrupt",
    "booting mcp server",
    "usage limit resets",
    "tip:",
    "model:",
    "working",
    "microsoft windows",
    "microsoft corporation",
    "all rights reserved",
    "gpt-",
    ">_",
)
_TUI_PROMPT_PREFIXES = (
    chr(0x203A),
    "$ ",
    "# ",
)
_TUI_WINDOWS_PROMPT_RE = re.compile(r"^[A-Za-z]:\\.*>")
_TUI_POWERSHELL_PROMPT_RE = re.compile(r"^PS [A-Za-z]:\\.*>")


def _config_bool(value: object, default: bool = False) -> bool:
    if value is None:
        return default
    if isinstance(value, bool):
        return value
    text = str(value).strip().lower()
    if not text:
        return default
    if text in {"1", "true", "yes", "y", "on"}:
        return True
    if text in {"0", "false", "no", "n", "off"}:
        return False
    return bool(value)


def _resolve_codex_command(value: object = None) -> str:
    configured = "" if value is None else str(value).strip()
    if configured:
        return configured
    for candidate in ("codex", "codex.cmd", "codex.exe"):
        resolved = shutil.which(candidate)
        if resolved:
            return resolved
    appdata = os.environ.get("APPDATA")
    if appdata:
        for name in ("codex.cmd", "codex.exe", "codex.ps1"):
            path = os.path.join(appdata, "npm", name)
            if os.path.exists(path):
                return path
    return "codex"


def _codex_exec_build_command(config: Mapping[str, object], cwd: str, model: str) -> list[str]:
    command = [
        _resolve_codex_command(config.get("codex_cmd")),
        "exec",
        "--json",
        "--color",
        "never",
    ]
    if _config_bool(config.get("bypass_approvals_and_sandbox"), False):
        command.append("--dangerously-bypass-approvals-and-sandbox")
    else:
        sandbox = str(config.get("sandbox", "workspace-write") or "workspace-write").strip()
        if sandbox:
            command.extend(["--sandbox", sandbox])
    if cwd:
        command.extend(["--cd", cwd])
    if _config_bool(config.get("skip_git_check"), True):
        command.append("--skip-git-repo-check")
    if _config_bool(config.get("ephemeral"), False):
        command.append("--ephemeral")
    if model:
        command.extend(["--model", model])
    extra_args = str(config.get("extra_args", "") or "").strip()
    if extra_args:
        command.extend(shlex.split(extra_args))
    command.append("-")
    return command


def _codex_exec_text_from_value(value: object) -> str:
    if value is None:
        return ""
    if isinstance(value, str):
        return value
    if isinstance(value, Mapping):
        for key in ("text", "message", "body", "output"):
            if key in value:
                text = _codex_exec_text_from_value(value.get(key))
                if text:
                    return text
        content = value.get("content")
        if content is not None:
            return _codex_exec_text_from_value(content)
        return ""
    if isinstance(value, Sequence) and not isinstance(value, (bytes, bytearray)):
        parts = [_codex_exec_text_from_value(item) for item in value]
        return "".join(part for part in parts if part)
    return str(value)


def _codex_exec_event_activity(event: Mapping[str, object]) -> str | None:
    event_type = str(event.get("type", event.get("event", "")) or "")
    item = event.get("item") if isinstance(event.get("item"), Mapping) else None
    item_type = "" if item is None else str(item.get("type", item.get("item_type", "")) or "")
    if event_type in {"thread.started", "turn.started", "turn.completed", "turn.failed"}:
        return event_type.replace(".", " ")
    if item_type == "command_execution":
        command = _codex_exec_text_from_value(item.get("command") or item.get("cmd")) if item is not None else ""
        status = str(item.get("status", "") or "") if item is not None else ""
        label = "command" if not status else f"command {status}"
        return f"{label}: {command}" if command else label
    if item_type == "file_change":
        action = str(item.get("action", item.get("change", "file_change")) or "file_change") if item is not None else "file_change"
        path = str(item.get("path", item.get("file", "")) or "") if item is not None else ""
        return f"{action}: {path}" if path else action
    if "error" in event_type or "failed" in event_type:
        text = _codex_exec_text_from_value(event.get("message") or event.get("error"))
        return f"{event_type}: {text}" if text else event_type
    return None


def _execute_codex_exec(
    inputs: Mapping[str, list[object]],
    config: Mapping[str, object],
    log: list[dict[str, object]],
) -> dict[str, list[object]]:
    prompt_values = _flow_input_values(inputs, "prompt", "text", "in")
    prompt = str(prompt_values[-1] if prompt_values else config.get("prompt", "") or "")
    if not prompt.strip():
        log.append({"event": "codex_exec_skipped", "reason": "prompt is required"})
        return {}
    cwd_values = _flow_input_values(inputs, "cwd")
    model_values = _flow_input_values(inputs, "model")
    cwd = str(cwd_values[-1] if cwd_values else config.get("cwd", "") or "").strip()
    model = str(model_values[-1] if model_values else config.get("model", "") or "").strip()
    command = _codex_exec_build_command(config, cwd, model)
    timeout_value = config.get("timeout_seconds", 0) or 0
    timeout = float(timeout_value) if float(timeout_value) > 0 else None
    stdin_text = prompt if prompt.endswith("\n") else f"{prompt}\n"
    log.append({"event": "codex_exec_started", "command": command, "cwd": cwd or None})
    try:
        process = subprocess.Popen(
            command,
            cwd=cwd or None,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
            bufsize=1,
        )
        try:
            stdout, stderr = process.communicate(stdin_text, timeout=timeout)
        except subprocess.TimeoutExpired:
            process.kill()
            stdout, stderr = process.communicate()
            stderr = (stderr or "") + f"\nCodex exec timed out after {timeout:g} seconds."
    except Exception as exc:
        message = str(exc)
        log.append({"event": "codex_exec_failed", "reason": message})
        return {"stderr": [message], "exit_code": [-1], "ok": [False]}

    events: list[object] = []
    activity: list[str] = []
    final_parts: list[str] = []
    raw_lines = stdout.splitlines()
    for raw_line in raw_lines:
        line = raw_line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            activity.append(f"unparsed: {line}")
            continue
        events.append(_json_safe_value(event))
        if isinstance(event, Mapping):
            item = event.get("item") if isinstance(event.get("item"), Mapping) else None
            item_type = "" if item is None else str(item.get("type", item.get("item_type", "")) or "")
            if item_type in {"agent_message", "assistant_message", "message"}:
                text = _codex_exec_text_from_value(item)
                if text:
                    final_parts.append(text)
            event_type = str(event.get("type", event.get("event", "")) or "")
            if event_type in {"agent_message", "assistant_message"}:
                text = _codex_exec_text_from_value(event)
                if text:
                    final_parts.append(text)
            event_text = _codex_exec_event_activity(event)
            if event_text:
                activity.append(event_text)
    return_code = int(process.returncode or 0)
    final_text = "\n".join(part.strip() for part in final_parts if part.strip())
    activity_text = "\n".join(line for line in activity if line)
    outputs: dict[str, list[object]] = {
        "raw_jsonl": ["\n".join(raw_lines)] if raw_lines else [],
        "events": [events] if events else [],
        "stderr": [stderr.strip()] if stderr and stderr.strip() else [],
        "exit_code": [return_code],
        "ok": [return_code == 0],
    }
    if final_text:
        outputs["final"] = [final_text]
    if activity_text:
        outputs["activity"] = [activity_text]
    log.append({"event": "codex_exec_completed", "exit_code": return_code, "events": len(events), "final": bool(final_text)})
    return outputs


def _tui_screen_char_is_frame(char: str) -> bool:
    codepoint = ord(char)
    return any(start <= codepoint <= end for start, end in _TUI_BOX_CODE_RANGES) or char in _TUI_ASCII_FRAME_CHARS


def _tui_screen_line_is_chrome(line: str) -> bool:
    text = line.strip()
    if not text:
        return True
    if all(_tui_screen_char_is_frame(char) or char.isspace() for char in text):
        return True
    if text.startswith(_TUI_PROMPT_PREFIXES):
        return True
    if _TUI_WINDOWS_PROMPT_RE.match(text) or _TUI_POWERSHELL_PROMPT_RE.match(text):
        return True
    lowered = text.lower()
    return any(phrase in lowered for phrase in _TUI_CHROME_PHRASES)


def _execute_tui_screen_diff(
    node: NodeGraphNode,
    inputs: Mapping[str, list[object]],
    config: Mapping[str, object],
    state: dict[str, object],
    log: list[dict[str, object]],
) -> dict[str, list[object]]:
    drop_chrome = bool(config.get("drop_chrome", True))
    dedupe = bool(config.get("dedupe", True))
    trailing_newline = bool(config.get("emit_trailing_newline", False))
    state_key = f"{node.id}:tui_screen_diff"
    record = state.setdefault(state_key, {"previous_lines": [], "emitted_lines": []})
    previous_lines: set[str] = set()
    emitted_lines: set[str] = set()
    if dedupe and isinstance(record, MutableMapping):
        previous_lines = {str(line) for line in record.get("previous_lines", [])}
        emitted_lines = {str(line) for line in record.get("emitted_lines", [])}
    emitted: list[str] = []
    for value in _flow_input_values(inputs, "screen", "text", "in"):
        lines = [line.rstrip() for line in str(value).splitlines()]
        normalized_lines = [line.strip() for line in lines if line.strip()]
        for normalized in normalized_lines:
            if drop_chrome and _tui_screen_line_is_chrome(normalized):
                continue
            if dedupe and (normalized in previous_lines or normalized in emitted_lines):
                continue
            emitted.append(normalized)
            if dedupe:
                emitted_lines.add(normalized)
        if isinstance(record, MutableMapping):
            record["previous_lines"] = normalized_lines
            if dedupe:
                record["emitted_lines"] = list(emitted_lines)[-500:]
            previous_lines = set(normalized_lines) if dedupe else set()
    if not emitted:
        return {}
    text = "\n".join(emitted)
    if trailing_newline and not text.endswith("\n"):
        text += "\n"
    log.append({"event": "tui_screen_diff", "node": node.id, "lines": len(emitted)})
    return {"text": [text]}


def _execute_envelope_parser(
    node: NodeGraphNode, inputs: Mapping[str, list[object]], parser_state: dict[str, AgentEnvelopeParser], log: list[dict[str, object]]
) -> dict[str, list[object]]:
    parser = parser_state.setdefault(node.id, AgentEnvelopeParser())
    outputs: dict[str, list[object]] = {"message": [], "to": [], "from": [], "type": [], "body": [], "id": [], "fields": []}
    for value in _flow_input_values(inputs, "text", "in"):
        for message in parser.feed(value):
            message_dict = message.to_dict()
            outputs["message"].append(message_dict)
            outputs["to"].append(message.to)
            outputs["from"].append(message.from_)
            outputs["type"].append(message.type)
            outputs["body"].append(message.body)
            outputs["id"].append(message.id)
            outputs["fields"].append(dict(message.fields))
    for event in parser.drain_events():
        log.append({"event": "parser", "node": node.id, "parser_event": event})
    return outputs


def _execute_message_router(inputs: Mapping[str, list[object]], config: Mapping[str, object], log: list[dict[str, object]]) -> dict[str, list[object]]:
    outputs: dict[str, list[object]] = {}
    rules = config.get("rules", [])
    if not isinstance(rules, Sequence) or isinstance(rules, (str, bytes, bytearray)):
        rules = []
    default_output = str(config.get("default_target", "default") or "default")
    for message in _flow_input_values(inputs, "message", "in"):
        message_map = _message_mapping(message)
        output = _route_message_output(message_map, rules, default_output)
        outputs.setdefault(output, []).append(message)
        log.append({"event": "route", "node_type": "message_router", "message_id": message_map.get("id"), "output": output})
    return outputs


def _message_mapping(message: object) -> dict[str, object]:
    if isinstance(message, AgentMessage):
        return message.to_dict()
    if isinstance(message, Mapping):
        return dict(message)
    return {"body": str(message)}


def _route_message_output(message: Mapping[str, object], rules: Sequence[object], default_output: str) -> str:
    for rule in rules:
        if not isinstance(rule, Mapping):
            continue
        field = str(rule.get("field", "to"))
        expected = rule.get("equals", rule.get("value", rule.get("is")))
        if expected is None:
            continue
        if str(message.get(field, "")) == str(expected):
            return str(rule.get("output", rule.get("target", default_output)) or default_output)
    return default_output


def _json_safe_value(value: object) -> object:
    if isinstance(value, AgentMessage):
        return value.to_dict()
    if isinstance(value, Mapping):
        return {str(key): _json_safe_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_json_safe_value(item) for item in value]
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    return str(value)


def _json_copy(value: object, context: str) -> dict[str, object]:
    try:
        copied = json.loads(json.dumps(value))
    except (TypeError, ValueError) as exc:
        raise TypeError(f"{context} must be JSON-serializable") from exc
    if not isinstance(copied, dict):
        raise TypeError(f"{context} must be a mapping")
    return copied


__all__ = [
    "NodeGraph",
    "NodeGraphRuntimeEdgeBinding",
    "NodeGraphFlowRun",
    "NodeGraphNodeBinding",
    "NodeGraphRuntimeBinding",
    "NodeGraphRuntimeEvent",
    "NodeGraphRuntimeHandle",
    "NodeGraphRuntimeSession",
    "NodeGraphRuntimeViewBinding",
    "NodeGraphSectionBinding",
    "NodeGraphEdge",
    "NodeGraphNode",
    "NodeGraphObjectRegistry",
    "NodeGraphPort",
    "NodeGraphRuntimeObject",
    "NodeGraphRuntimeObjectRef",
    "NodeGraphSection",
    "NodeGraphActionTarget",
    "NodeGraphBindingTarget",
    "NodeGraphTemplate",
    "NodeGraphWidgetTarget",
    "multi_agent_node_templates",
    "node_graph_port_type_color",
]
