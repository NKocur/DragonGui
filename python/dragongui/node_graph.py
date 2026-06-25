from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
import json
import math
import time
from typing import Any

from .agent_messages import AgentEnvelopeParser, AgentMessage
from .widgets import Container, HtmlReport, _AUTO_PARENT


_GRAPH_SCHEMA_VERSION = 1
_GRAPH_MUTATION_EVENTS = {
    "node_moved",
    "node_created",
    "node_duplicated",
    "node_deleted",
    "node_updated",
    "edge_created",
    "edge_deleted",
    "section_created",
    "section_updated",
    "section_moved",
    "section_resized",
    "section_deleted",
}
_NODE_GRAPH_RUNTIME_SCHEMA_VERSION = 1


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

    def to_dict(self) -> dict[str, object]:
        return {
            "edge_id": self.edge_id,
            "source": {"node": self.source_node, "port": self.source_port},
            "target": {"node": self.target_node, "port": self.target_port},
            "label": self.label,
        }


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
        runtime_handle.attach(handle, status=status)
        self.emit_event(
            "object_handle_attached",
            object_id=runtime_handle.object_id,
            node_id=runtime_handle.owner_node_id,
            data={"object_type": runtime_handle.object_type, "status": runtime_handle.status},
        )
        return runtime_handle

    def detach_handle(self, object_id: str, *, status: str = "detached") -> Any | None:
        runtime_handle = self.require_object_handle(object_id)
        detached = runtime_handle.detach(status=status)
        self.emit_event(
            "object_handle_detached",
            object_id=runtime_handle.object_id,
            node_id=runtime_handle.owner_node_id,
            data={"object_type": runtime_handle.object_type, "status": runtime_handle.status},
        )
        return detached

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

        def handle_terminal_event(event: Any) -> None:
            self.apply_terminal_event(runtime_handle.object_id, event)
            if on_event is not None:
                on_event(event)

        bridge = TerminalBridge(
            command,
            args=args,
            cwd=None if cwd is None else str(cwd),
            env=None if env_value is None else {str(key): str(value) for key, value in env_value.items()},
            cols=int(config.get("cols", 100)),
            rows=int(config.get("rows", 30)),
            prefer_pty=bool(config.get("prefer_pty", True)),
            on_output=on_output,
            on_event=handle_terminal_event,
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
            raise RuntimeError(f"runtime object {object_id!r} has no attached terminal bridge")
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
        if handle is not None:
            for method_name in ("stop", "close", "dispose"):
                method = getattr(handle, method_name, None)
                if method is None:
                    continue
                method()
                stopped = True
                break
        runtime_handle.set_status("stopped")
        self.emit_event(
            "object_stop_requested",
            node_id=runtime_handle.owner_node_id,
            object_id=runtime_handle.object_id,
            data={"object_type": runtime_handle.object_type, "stopped": stopped},
        )
        if detach:
            self.detach_handle(runtime_handle.object_id, status="stopped")
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
            "input": "terminal_stdin",
        }.get(name, f"terminal_{name}" if name else "terminal_event")
        port_id = {"output": "stdout", "input": "stdin"}.get(name)
        status = {
            "bridge_started": "starting",
            "session_started": "running",
            "session_ended": "exited",
            "bridge_stopped": "stopped",
        }.get(name)
        if status is not None:
            runtime_handle.set_status(status)
        data = {"terminal_event": _json_safe_value(payload), "object_type": runtime_handle.object_type}
        value = payload.get("data") if name == "output" else None
        return self.emit_event(
            runtime_event,
            node_id=runtime_handle.owner_node_id,
            port_id=port_id,
            object_id=runtime_handle.object_id,
            value=value,
            data=data,
            timestamp=float(payload.get("timestamp", time.time())),
        )

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
            self._execute_runtime_node(item.node_id, item.port_id, item.value, timestamp=item.timestamp)
            return
        for edge in self.binding.edges:
            if edge.source_node != item.node_id or edge.source_port != item.port_id:
                continue
            self.emit_event(
                "edge_value",
                node_id=edge.target_node,
                port_id=edge.target_port,
                value=item.value,
                data={
                    "edge_id": edge.edge_id,
                    "source_node": edge.source_node,
                    "source_port": edge.source_port,
                    "target_node": edge.target_node,
                    "target_port": edge.target_port,
                    "source_event": item.event,
                },
                timestamp=item.timestamp,
            )

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
                outputs = _execute_flow_node(node, {port_id: [value]}, self._parser_state, log)
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

    def _apply_widget_sink(
        self, binding: NodeGraphNodeBinding, value: object, *, timestamp: float | None = None
    ) -> list[dict[str, object]]:
        config = binding.config or {}
        widget_id = str(config.get("widget_id", "")).strip()
        widget_type = str(config.get("widget_type", "")).strip()
        update_mode = str(config.get("update_mode", "") or "auto").strip()
        value_format = str(config.get("format", "") or "text").strip()
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

    def to_graph_data(self) -> dict[str, object]:
        """Return a JSON-serializable, versioned snapshot of the graph."""

        return _json_copy(
            {
                "schema_version": _GRAPH_SCHEMA_VERSION,
                "nodes": [_node_graph_data(node) for node in self.nodes],
                "edges": [_edge_graph_data(edge, index) for index, edge in enumerate(self.edges)],
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
            edges=tuple(_edge_runtime_binding(edge, index) for index, edge in enumerate(self.edges)),
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
            emit_events=True,
        )

    def props(self) -> dict[str, object]:
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
        if source_type and target_type and source_type != target_type:
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
    selected_node: str | None,
    show_edge_labels: bool,
    show_port_labels: bool,
    show_status_labels: bool,
    show_subtitles: bool,
    enable_zoom: bool,
    emit_events: bool,
) -> str:
    config = {
        "nodes": [_node_payload(node) for node in nodes],
        "edges": [_edge_payload(edge) for edge in edges],
        "templates": [_template_payload(template) for template in templates],
        "sections": [_section_payload(section) for section in sections],
        "selectedNode": selected_node,
        "showEdgeLabels": bool(show_edge_labels),
        "showPortLabels": bool(show_port_labels),
        "showStatusLabels": bool(show_status_labels),
        "showSubtitles": bool(show_subtitles),
        "enableZoom": bool(enable_zoom),
        "emitEvents": bool(emit_events),
    }
    payload = json.dumps(config)
    return f"""<!doctype html>
<html>
<head>
  <meta charset=\"utf-8\" />
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
  <style>
    html, body {{ width: 100%; height: 100%; margin: 0; overflow: hidden; background: #0d1117; }}
    canvas {{ width: 100%; height: 100%; display: block; background: #0d1117; cursor: default; }}
  </style>
</head>
<body>
  <canvas id=\"graph\" tabindex=\"0\"></canvas>
  <script>
    const config = {payload};
    const canvas = document.getElementById('graph');
    const ctx = canvas.getContext('2d');
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
    const propertyEditor = {{ open: false, kind: null, id: null, fields: [], active: 0, scroll: 0, scrollDrag: null, rect: null, listRect: null, scrollBar: null, buttons: [] }};
    const TOOLBAR_Y = 10;
    const TOOLBAR_H = 28;
    const TOOLBAR_W = 34;
    const toolbar = {{ items: [] }};

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
      clampNodePicker();
      emitGraphEvent({{ event: 'node_picker_opened', position: {{ x: point.x, y: point.y }}, template: palette.selected || null }});
      draw();
    }}

    function closeNodePicker(notify = true) {{
      if (!nodePicker.open) return;
      nodePicker.open = false;
      nodePicker.scrollDrag = null;
      nodePicker.items = [];
      if (notify) emitGraphEvent({{ event: 'node_picker_closed' }});
      draw();
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
      if (propertyEditor.scrollBar) {{
        const bar = propertyEditor.scrollBar;
        if (sx >= bar.x - 5 && sx <= bar.x + bar.w + 5 && sy >= bar.y && sy <= bar.y + bar.h) {{
          return {{ kind: 'scrollbar', onThumb: sy >= bar.thumbY && sy <= bar.thumbY + bar.thumbH }};
        }}
      }}
      for (let index = 0; index < propertyEditor.fields.length; index++) {{
        const field = propertyEditor.fields[index];
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

    function edgePoints(edge) {{
      const a = portPoint(edge.sourceNode, edge.sourcePort, 'output');
      const b = portPoint(edge.targetNode, edge.targetPort, 'input');
      if (!a || !b) return null;
      return {{ a, b, dx: Math.max(48, Math.abs(b.x - a.x) * 0.45) }};
    }}

    function hitEdge(sx, sy) {{
      const p = {{ x: sx, y: sy }};
      for (let i = state.edges.length - 1; i >= 0; i--) {{
        const edge = state.edges[i];
        const points = edgePoints(edge);
        if (!points) continue;
        let prev = points.a;
        for (let step = 1; step <= 24; step++) {{
          const next = bezierPoint(points.a, points.b, points.dx, step / 24);
          if (distanceToSegment(p, prev, next) <= 7) return edge;
          prev = next;
        }}
      }}
      return null;
    }}

    function portType(port) {{
      return port ? (port.port_type || port.type || null) : null;
    }}

    function connectionRejection(from, to) {{
      if (!from) return 'missing source port';
      if (!to) return 'missing target port';
      if (from.side !== 'output') return 'source port must be an output';
      if (to.side !== 'input') return 'target port must be an input';
      if (from.node.id === to.node.id) return 'self connection rejected';
      const sourceType = portType(from.port);
      const targetType = portType(to.port);
      if (sourceType && targetType && sourceType !== targetType) return `incompatible port types: ${{sourceType}} -> ${{targetType}}`;
      const duplicate = state.edges.some(edge => edge.sourceNode === from.node.id && edge.sourcePort === from.port.id && edge.targetNode === to.node.id && edge.targetPort === to.port.id);
      if (duplicate) return 'duplicate edge';
      return null;
    }}

    function canConnect(from, to) {{
      return connectionRejection(from, to) === null;
    }}

    function edgeEventPayload(edge) {{
      return {{
        id: edge.id,
        source_node: edge.sourceNode,
        source_port: edge.sourcePort,
        target_node: edge.targetNode,
        target_port: edge.targetPort,
        label: edge.label || null,
        color: edge.color || '#43c6ac'
      }};
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
        edges: state.edges.map(edge => ({{ ...edge }})),
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
      state.edges = snapshot.edges.map(edge => ({{ ...edge }}));
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
      state.edges.push({{
        id: `edge-${{++edgeSerial}}`,
        sourceNode: from.node.id,
        sourcePort: from.port.id,
        targetNode: to.node.id,
        targetPort: to.port.id,
        label: null,
        color: from.node.color || '#43c6ac'
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

    function makePropertyField(key, label, value, type = 'text', options = {{}}) {{
      const normalizedType = ['bool', 'number', 'select', 'json', 'textarea'].includes(type) ? type : 'text';
      let normalizedValue = value;
      if (normalizedType === 'bool') normalizedValue = !!value;
      else if (normalizedType === 'number') normalizedValue = value === undefined || value === null || value === '' ? '' : String(value);
      else if (normalizedType === 'json' && value !== undefined && value !== null && typeof value !== 'string') normalizedValue = JSON.stringify(value);
      else normalizedValue = String(value || '');
      return {{
        key,
        label,
        value: normalizedValue,
        type: normalizedType,
        options: Array.isArray(options.options) ? options.options.map(option => String(option)) : [],
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

    function schemaPropertyFields(target) {{
      const schemaFields = configSchemaFields(target);
      const storage = schemaFields ? 'config' : 'data';
      const schema = schemaFields || (target.data && Array.isArray(target.data.property_fields) ? target.data.property_fields : []);
      return schema
        .filter(spec => spec && spec.key)
        .map(spec => {{
          const key = String(spec.key);
          const type = schemaFieldType(spec);
          const value = schemaFieldValue(target, spec, storage);
          const fieldKey = storage === 'config' ? `config:${{key}}` : `data:${{key}}`;
          const field = makePropertyField(fieldKey, String(spec.label || key), value, type, spec);
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
            makePropertyField('locked', 'Locked', !!target.locked, 'bool'),
            makePropertyField('collapsed', 'Collapsed', !!target.collapsed, 'bool')
          ];
      propertyEditor.scroll = 0;
      propertyEditor.scrollDrag = null;
      propertyEditor.rect = null;
      propertyEditor.listRect = null;
      propertyEditor.scrollBar = null;
      propertyEditor.buttons = [];
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
      if (notify) emitGraphEvent({{ event: 'property_editor_closed', target_type: kind, target: id }});
      draw();
      return true;
    }}

    function propertyField(key) {{
      return propertyEditor.fields.find(field => field.key === key);
    }}

    function textProperty(key, fallback = '') {{
      const field = propertyField(key);
      return field ? String(field.value || '').trim() : fallback;
    }}

    function commitPropertyEditor() {{
      if (!propertyEditor.open) return false;
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
      draw();
      return true;
    }}

    function editPropertyField(index) {{
      if (!propertyEditor.open || index < 0 || index >= propertyEditor.fields.length) return false;
      propertyEditor.active = index;
      const field = propertyEditor.fields[index];
      if (field.type === 'bool') field.value = !field.value;
      draw();
      return true;
    }}

    function activePropertyField() {{
      return propertyEditor.fields[propertyEditor.active] || null;
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
      ctx.strokeStyle = selected ? '#eef4ff' : (edge.color || '#43c6ac');
      ctx.lineWidth = selected ? 4 : 2.4;
      ctx.beginPath();
      ctx.moveTo(points.a.x, points.a.y);
      ctx.bezierCurveTo(points.a.x + points.dx, points.a.y, points.b.x - points.dx, points.b.y, points.b.x, points.b.y);
      ctx.stroke();
      ctx.fillStyle = selected ? '#eef4ff' : (edge.color || '#43c6ac');
      ctx.beginPath(); ctx.arc(points.a.x, points.a.y, 3.5, 0, Math.PI * 2); ctx.fill();
      ctx.beginPath(); ctx.arc(points.b.x, points.b.y, 3.5, 0, Math.PI * 2); ctx.fill();
      if (config.showEdgeLabels && edge.label) {{
        ctx.fillStyle = '#9fb0c3'; ctx.font = '11px Segoe UI'; ctx.textAlign = 'center';
        ctx.fillText(edge.label, (points.a.x + points.b.x) / 2, Math.min(points.a.y, points.b.y) - 12);
      }}
    }}

    function drawTempEdge(from, to) {{
      const a = from.point;
      const b = {{ x: to.x, y: to.y }};
      const dx = Math.max(48, Math.abs(b.x - a.x) * 0.45);
      const target = hitPort(to.x, to.y, 'input');
      const valid = canConnect(from, target);
      ctx.strokeStyle = valid ? '#eef4ff' : 'rgba(159, 176, 195, 0.68)';
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
      roundedRect(inputX, inputY, inputW, 28, 6);
      ctx.fillStyle = '#0b1017';
      ctx.fill();
      ctx.strokeStyle = '#26384a';
      ctx.lineWidth = 1;
      ctx.stroke();
      ctx.textAlign = 'left';
      ctx.font = '12px Segoe UI';
      ctx.fillStyle = nodePicker.query ? '#eef4ff' : '#738296';
      ctx.fillText(nodePicker.query || 'Type to filter templates...', inputX + 10, inputY + 14);

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
        {{ action: 'fit', label: '[]' }},
        {{ action: 'zoom_in', label: '+' }},
        {{ action: 'zoom_out', label: '-' }},
        {{ action: 'grid', label: '#' }},
        {{ action: 'inspect', label: 'i' }}
      ];
      let x = width - actions.length * (TOOLBAR_W + 6) - 6;
      ctx.save();
      ctx.textBaseline = 'middle';
      ctx.font = '13px Segoe UI';
      for (const item of actions) {{
        const active = item.action === 'grid' ? state.showGrid : false;
        ctx.fillStyle = active ? '#26384a' : '#171d27';
        ctx.strokeStyle = active ? '#eef4ff' : '#354255';
        ctx.lineWidth = active ? 1.5 : 1;
        roundedRect(x, TOOLBAR_Y, TOOLBAR_W, TOOLBAR_H, 6);
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = '#cbd6e2';
        ctx.textAlign = 'center';
        ctx.fillText(item.label, x + TOOLBAR_W / 2, TOOLBAR_Y + TOOLBAR_H / 2);
        toolbar.items.push({{ action: item.action, x, y: TOOLBAR_Y, w: TOOLBAR_W, h: TOOLBAR_H }});
        x += TOOLBAR_W + 6;
      }}
      ctx.restore();
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
        ctx.fillStyle = '#0d1117'; ctx.strokeStyle = hovered || connectTarget ? '#eef4ff' : node.color; ctx.lineWidth = hovered || connectTarget ? 3 : 2;
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
      roundedRect(inputX, inputY, inputW, 34, 6);
      ctx.fillStyle = '#0b1017';
      ctx.fill();
      ctx.strokeStyle = '#354255';
      ctx.lineWidth = 1;
      ctx.stroke();
      ctx.fillStyle = renameEditor.value ? '#eef4ff' : '#738296';
      ctx.font = '13px Segoe UI';
      const visible = renameEditor.value || 'Title';
      const cursor = Math.floor(Date.now() / 530) % 2 === 0 ? '|' : '';
      if (renameEditor.selectAll && renameEditor.value) {{
        ctx.fillStyle = 'rgba(67, 198, 172, 0.28)';
        roundedRect(inputX + 7, inputY + 7, Math.min(inputW - 14, ctx.measureText(renameEditor.value).width + 8), 20, 4);
        ctx.fill();
        ctx.fillStyle = '#eef4ff';
      }}
      ctx.fillText(visible + (renameEditor.selectAll ? '' : cursor), inputX + 10, inputY + 17);
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
      for (const field of propertyEditor.fields) field.rect = null;

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
          const visible = text || field.placeholder || (field.type === 'select' && field.options.length ? field.options[0] : '');
          ctx.fillStyle = text ? '#eef4ff' : '#738296';
          ctx.font = '12px Segoe UI';
          const cursor = index === propertyEditor.active && Math.floor(Date.now() / 530) % 2 === 0 && field.type !== 'select' ? '|' : '';
          ctx.save();
          ctx.beginPath();
          ctx.rect(inputX + 8, rowY + 5, inputW - 16, 28);
          ctx.clip();
          ctx.fillText(visible + cursor, inputX + 10, rowY + 19);
          if (field.type === 'select' && field.options.length) {{
            ctx.fillStyle = '#607086';
            ctx.font = '10px Segoe UI';
            ctx.textAlign = 'right';
            ctx.fillText(field.options.join(' / '), inputX + inputW - 10, rowY + 19);
            ctx.textAlign = 'left';
          }}
          ctx.restore();
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
    }}

    canvas.addEventListener('mousedown', event => {{
      canvas.focus();
      const p = graphPoint(event);
      if (propertyEditor.open) {{
        const propertyHit = hitPropertyEditor(p.sx, p.sy);
        if (propertyHit && propertyHit.kind === 'commit') commitPropertyEditor();
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
        else if (propertyHit && (propertyHit.kind === 'cancel' || propertyHit.kind === 'outside')) closePropertyEditor();
        else if (propertyHit && propertyHit.kind === 'field') editPropertyField(propertyHit.index);
        event.preventDefault();
        draw();
        return;
      }}
      if (renameEditor.open) {{
        const renameHit = hitRenameEditor(p.sx, p.sy);
        if (renameHit && renameHit.kind === 'commit') commitRenameEditor();
        else if (renameHit && (renameHit.kind === 'cancel' || renameHit.kind === 'outside')) closeRenameEditor();
        else if (renameHit && renameHit.kind === 'inside') renameEditor.selectAll = false;
        event.preventDefault();
        draw();
        return;
      }}
      if (nodePicker.open) {{
        const pickerHit = hitNodePicker(p.sx, p.sy);
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
        canvas.style.cursor = state.hoverPort && state.hoverPort.side === 'output' ? 'crosshair' : (hitHeader(p.x, p.y) ? 'grab' : (hitEdge(p.sx, p.sy) ? 'pointer' : (hitNode(p.x, p.y) ? 'default' : (resizeSection && !resizeSection.locked ? 'nwse-resize' : (movableSection && !movableSection.locked ? 'grab' : (section ? 'pointer' : 'move'))))));
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
      }} else if (state.drag.kind === 'edge') {{
        state.drag.to = {{ x: p.sx, y: p.sy }};
      }} else {{
        state.viewX = state.drag.vx + p.sx - state.drag.sx;
        state.viewY = state.drag.vy + p.sy - state.drag.sy;
      }}
      draw();
    }});

    window.addEventListener('mouseup', event => {{
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
      }} else if (state.drag && state.drag.kind === 'pan') {{
        emitViewportChanged('pan');
      }}
      state.drag = null;
      canvas.style.cursor = 'default';
      draw();
    }});

    canvas.addEventListener('dblclick', event => {{
      const p = graphPoint(event);
      if (hitPort(p.sx, p.sy) || hitEdge(p.sx, p.sy) || hitNode(p.x, p.y)) return;
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
      if (propertyEditor.open) {{
        const field = activePropertyField();
        const cycleSelectField = direction => {{
          if (!field || field.type !== 'select' || !field.options.length) return false;
          const current = field.options.indexOf(String(field.value || ''));
          const next = (current + direction + field.options.length) % field.options.length;
          field.value = field.options[next];
          draw();
          return true;
        }};
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
          ensurePropertyFieldVisible();
          draw();
        }} else if ((event.key === ' ' || event.key === 'ArrowDown' || event.key === 'ArrowRight') && field && field.type === 'select') {{
          event.preventDefault();
          cycleSelectField(1);
        }} else if ((event.key === 'ArrowUp' || event.key === 'ArrowLeft') && field && field.type === 'select') {{
          event.preventDefault();
          cycleSelectField(-1);
        }} else if (event.key === ' ' && field && field.type === 'bool') {{
          event.preventDefault();
          field.value = !field.value;
          draw();
        }} else if (event.key === 'Backspace' && field && field.type !== 'bool' && field.type !== 'select') {{
          event.preventDefault();
          field.value = String(field.value || '').slice(0, -1);
          draw();
        }} else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a' && field && field.type !== 'bool' && field.type !== 'select') {{
          event.preventDefault();
          field.value = '';
          draw();
        }} else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey && field && field.type !== 'bool' && field.type !== 'select') {{
          event.preventDefault();
          field.value = String(field.value || '') + event.key;
          draw();
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
        }} else if (event.key === 'Backspace') {{
          event.preventDefault();
          renameEditor.value = renameEditor.selectAll ? '' : renameEditor.value.slice(0, -1);
          renameEditor.selectAll = false;
          draw();
        }} else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') {{
          event.preventDefault();
          renameEditor.value = '';
          renameEditor.selectAll = false;
          draw();
        }} else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {{
          event.preventDefault();
          renameEditor.value = renameEditor.selectAll ? event.key : renameEditor.value + event.key;
          renameEditor.selectAll = false;
          draw();
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
        }} else if (event.key === 'Backspace') {{
          event.preventDefault();
          nodePicker.query = nodePicker.query.slice(0, -1);
          nodePicker.selected = 0;
          nodePicker.scroll = 0;
          draw();
        }} else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {{
          event.preventDefault();
          nodePicker.query += event.key;
          nodePicker.selected = 0;
          nodePicker.scroll = 0;
          draw();
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

    canvas.addEventListener('wheel', event => {{
      const p = graphPoint(event);
      if (propertyEditor.open && propertyEditor.rect) {{
        const hit = hitPropertyEditor(p.sx, p.sy);
        if (hit && hit.kind !== 'outside') {{
          event.preventDefault();
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


def _edge_payload(edge: NodeGraphEdge) -> dict[str, object]:
    payload = {
        "sourceNode": edge.source_node,
        "sourcePort": edge.source_port,
        "targetNode": edge.target_node,
        "targetPort": edge.target_port,
        "label": edge.label,
        "color": edge.color,
    }
    if edge.id is not None:
        payload["id"] = edge.id
    if edge.data is not None:
        payload["data"] = _json_copy(edge.data, "edge custom data")
    return payload


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


def _edge_graph_data(edge: NodeGraphEdge, index: int) -> dict[str, object]:
    payload: dict[str, object] = {
        "id": edge.id or f"edge-{index + 1}",
        "source": {"node": edge.source_node, "port": edge.source_port},
        "target": {"node": edge.target_node, "port": edge.target_port},
        "label": edge.label,
        "color": edge.color,
    }
    if edge.data is not None:
        payload["data"] = edge.data
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
                NodeGraphPort("control", "control", port_type="control"),
                NodeGraphPort("cwd", "cwd", port_type="file:path"),
                NodeGraphPort("env", "env", port_type="json"),
            ),
            outputs=(
                NodeGraphPort("stdout", "stdout", port_type="terminal_output"),
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
                    {"key": "command", "label": "Command", "default": "codex"},
                    {"key": "args", "label": "Args", "placeholder": "--model gpt-5"},
                    {"key": "cwd", "label": "Working Dir"},
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
            inputs=(NodeGraphPort("value", "value", port_type="json"),),
            outputs=(NodeGraphPort("value", "value", port_type="json"),),
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
                        "options": ["text", "json", "repr", "message_body"],
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
_RUNTIME_EXECUTABLE_NODE_TYPES = {
    "append_text",
    "extract_between_markers",
    "envelope_parser",
    "parser",
    "message_router",
    "log",
    "probe",
    "widget_sink",
}

_WIDGET_SINK_SET_TYPES = {
    "label",
    "badge",
    "text_input",
    "text_area",
    "code_editor",
}

_WIDGET_SINK_APPEND_TYPES = {"log_view"}


def _widget_kind(widget: object) -> str:
    kind = getattr(widget, "kind", None)
    if kind is not None and str(kind).strip():
        return str(kind).strip()
    return type(widget).__name__


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
        append_line = getattr(widget, "append_line", None)
        if not callable(append_line):
            raise TypeError(f"widget type {kind!r} does not support append")
        append_line(_widget_sink_text(value, value_format))
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


def _edge_runtime_binding(edge: NodeGraphEdge, index: int) -> NodeGraphRuntimeEdgeBinding:
    return NodeGraphRuntimeEdgeBinding(
        edge_id=edge.id or f"edge-{index + 1}",
        source_node=edge.source_node,
        source_port=edge.source_port,
        target_node=edge.target_node,
        target_port=edge.target_port,
        label=edge.label,
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


def _runtime_object_from_node(node: NodeGraphNode) -> NodeGraphRuntimeObject | None:
    data = node.data or {}
    if not isinstance(data, Mapping):
        return None
    config = _node_config(node)
    object_id, _, key_type = _runtime_object_id_from_sources(config, data)
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
    parser_state: dict[str, AgentEnvelopeParser],
    log: list[dict[str, object]],
) -> dict[str, list[object]]:
    node_type = _node_type(node)
    config = _node_config(node)
    if node_type == "text_input":
        text = str(config.get("text", ""))
        return {"text": [text]} if text else {}
    if node_type == "append_text":
        appendix = str(config.get("appendix", ""))
        separator = str(config.get("separator", ""))
        return {"text": [str(value) + (separator if appendix else "") + appendix for value in _flow_input_values(inputs, "text", "in")]}
    if node_type == "extract_between_markers":
        return _execute_extract_between_markers(inputs, config)
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
    "NodeGraphTemplate",
    "multi_agent_node_templates",
]



















