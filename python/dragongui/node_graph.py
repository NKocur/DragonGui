from __future__ import annotations

from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
import json
import math

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
            },
            "NodeGraph graph data",
        )

    def set_graph_data(self, data: Mapping[str, object]) -> None:
        """Replace graph contents from :meth:`to_graph_data` data."""

        nodes, edges = self._graph_items_from_data(data)
        self.nodes = nodes
        self.edges = edges
        if self.selected_node not in self._node_ids():
            self.selected_node = None
        self._clear_history()
        self.set_html(self._html())

    @classmethod
    def from_graph_data(cls, data: Mapping[str, object], **kwargs: object) -> NodeGraph:
        """Create a :class:`NodeGraph` from versioned graph data."""

        nodes, edges = cls._graph_items_from_data(data)
        return cls(nodes, edges, **kwargs)

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

    def _restore_graph_data(self, data: Mapping[str, object]) -> None:
        nodes, edges = self._graph_items_from_data(data)
        self.nodes = nodes
        self.edges = edges
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
    ) -> tuple[tuple[NodeGraphNode, ...], tuple[NodeGraphEdge, ...]]:
        if not isinstance(data, Mapping):
            raise TypeError("NodeGraph graph data must be a mapping")
        version = data.get("schema_version")
        if version != _GRAPH_SCHEMA_VERSION:
            raise ValueError(f"unsupported NodeGraph schema_version {version!r}")
        nodes = data.get("nodes", ())
        edges = data.get("edges", ())
        if isinstance(nodes, (str, bytes, bytearray)) or not isinstance(nodes, Sequence):
            raise TypeError("NodeGraph graph data nodes must be a sequence")
        if isinstance(edges, (str, bytes, bytearray)) or not isinstance(edges, Sequence):
            raise TypeError("NodeGraph graph data edges must be a sequence")
        return (
            tuple(cls._node_from_value(node) for node in nodes),
            tuple(cls._edge_from_value(edge) for edge in edges),
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
    const state = {{ nodes: config.nodes, edges: config.edges, selected: config.selectedNode, selectedEdge: null, viewX: 34, viewY: 32, zoom: 1, drag: null, hoverPort: null, showGrid: true }};
    let nodeSerial = state.nodes.length;
    let edgeSerial = state.edges.length;
    for (const edge of state.edges) if (!edge.id) edge.id = `edge-${{++edgeSerial}}`;
    const history = {{ undo: [], redo: [], initial: null }};
    const HEADER = 36;
    const HEADER_PAD = 10;
    const PALETTE_X = 12;
    const PALETTE_Y = 10;
    const PALETTE_H = 28;
    const palette = {{ selected: config.templates[0] ? config.templates[0].id : null, items: [] }};
    const nodePicker = {{ open: false, x: 0, y: 0, graphX: 0, graphY: 0, query: '', selected: 0, rect: null, items: [] }};
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
      for (const item of palette.items) {{
        if (sx >= item.x && sx <= item.x + item.w && sy >= item.y && sy <= item.y + item.h) return item.template;
      }}
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
      return items.slice(0, 12);
    }}

    function clampNodePicker() {{
      const rect = canvas.getBoundingClientRect();
      const width = Math.min(420, Math.max(280, rect.width - 24));
      const maxRows = Math.max(1, Math.min(8, nodePickerItems().length || 1));
      const height = 72 + maxRows * 44 + 14;
      nodePicker.x = Math.max(12, Math.min(nodePicker.x, rect.width - width - 12));
      nodePicker.y = Math.max(12, Math.min(nodePicker.y, rect.height - Math.min(height, rect.height - 24) - 12));
      nodePicker.rect = {{ x: nodePicker.x, y: nodePicker.y, w: width, h: Math.min(height, rect.height - 24) }};
    }}

    function openNodePicker(point) {{
      if (!config.templates.length) {{
        addNode(point.x, point.y);
        draw();
        return;
      }}
      nodePicker.open = true;
      nodePicker.x = point.sx + 10;
      nodePicker.y = point.sy + 10;
      nodePicker.graphX = point.x;
      nodePicker.graphY = point.y;
      nodePicker.query = '';
      nodePicker.selected = Math.max(0, config.templates.findIndex(template => template.id === palette.selected));
      clampNodePicker();
      emitGraphEvent({{ event: 'node_picker_opened', position: {{ x: point.x, y: point.y }}, template: palette.selected || null }});
      draw();
    }}

    function closeNodePicker(notify = true) {{
      if (!nodePicker.open) return;
      nodePicker.open = false;
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
      for (const item of nodePicker.items) {{
        if (sx >= item.x && sx <= item.x + item.w && sy >= item.y && sy <= item.y + item.h) return {{ kind: 'item', index: item.index, template: item.template }};
      }}
      return {{ kind: 'inside' }};
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
        selected: state.selected,
        selectedEdge: state.selectedEdge
      }};
    }}

    function restoreGraphSnapshot(snapshot) {{
      state.nodes = snapshot.nodes.map(node => ({{
        ...node,
        inputs: node.inputs.map(port => ({{ ...port }})),
        outputs: node.outputs.map(port => ({{ ...port }}))
      }}));
      state.edges = snapshot.edges.map(edge => ({{ ...edge }}));
      state.selected = snapshot.selected || null;
      state.selectedEdge = snapshot.selectedEdge || null;
      nodeSerial = state.nodes.length;
      edgeSerial = state.edges.length;
      for (const node of state.nodes) {{
        const match = String(node.id || '').match(/(\\d+)$/);
        if (match) nodeSerial = Math.max(nodeSerial, Number(match[1]));
      }}
      for (const edge of state.edges) {{
        const match = String(edge.id || '').match(/(\\d+)$/);
        if (match) edgeSerial = Math.max(edgeSerial, Number(match[1]));
      }}
    }}

    function snapshotCore(snapshot) {{
      return JSON.stringify({{ nodes: snapshot.nodes, edges: snapshot.edges }});
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
      if (!state.nodes.length) return null;
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
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
      emitGraphMutation({{ event: 'node_created', node: nodeEventPayload(state.nodes[state.nodes.length - 1]) }}, before);
    }}

    function editSelectedNodeTitle() {{
      const node = state.nodes.find(n => n.id === state.selected);
      if (!node) return false;
      const value = window.prompt('Node title', node.title || '');
      if (value === null) return false;
      const title = String(value).trim();
      if (!title || title === node.title) return false;
      const before = graphSnapshot();
      node.title = title;
      emitGraphMutation({{ event: 'node_updated', node: node.id, updates: {{ title }} }}, before);
      draw();
      return true;
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
        emitGraphMutation({{ event: 'edge_deleted', edge: edgeId }}, before);
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
      if (!config.templates.length) return;
      let x = PALETTE_X;
      ctx.save();
      ctx.textBaseline = 'middle';
      ctx.font = '12px Segoe UI';
      for (const template of config.templates) {{
        const label = templateLabel(template);
        const w = Math.max(74, ctx.measureText(label).width + 22);
        const selected = template.id === palette.selected;
        ctx.fillStyle = selected ? '#26384a' : '#171d27';
        ctx.strokeStyle = selected ? '#eef4ff' : '#354255';
        ctx.lineWidth = selected ? 1.6 : 1;
        roundedRect(x, PALETTE_Y, w, PALETTE_H, 6);
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = selected ? '#eef4ff' : '#cbd6e2';
        ctx.textAlign = 'center';
        ctx.fillText(label, x + w / 2, PALETTE_Y + PALETTE_H / 2);
        palette.items.push({{ template, x, y: PALETTE_Y, w, h: PALETTE_H }});
        x += w + 8;
      }}
      ctx.restore();
    }}

    function drawNodePicker() {{
      if (!nodePicker.open) return;
      clampNodePicker();
      const rect = nodePicker.rect;
      const items = nodePickerItems();
      nodePicker.selected = Math.max(0, Math.min(nodePicker.selected, Math.max(0, items.length - 1)));
      nodePicker.items = [];
      ctx.save();
      ctx.globalAlpha = 0.96;
      roundedRect(rect.x, rect.y, rect.w, rect.h, 8);
      ctx.fillStyle = '#111821';
      ctx.fill();
      ctx.globalAlpha = 1;
      ctx.strokeStyle = '#43c6ac';
      ctx.lineWidth = 1.4;
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

      let y = rect.y + 82;
      if (!items.length) {{
        ctx.fillStyle = '#9aa8b8';
        ctx.fillText('No matching templates', rect.x + 16, y + 18);
      }}
      for (let index = 0; index < items.length; index++) {{
        const template = items[index];
        const selected = index === nodePicker.selected;
        const rowH = 40;
        roundedRect(rect.x + 10, y, rect.w - 20, rowH, 6);
        ctx.fillStyle = selected ? '#26384a' : '#151c26';
        ctx.fill();
        ctx.strokeStyle = selected ? '#eef4ff' : '#26384a';
        ctx.lineWidth = selected ? 1.2 : 1;
        ctx.stroke();
        ctx.fillStyle = template.color || '#43c6ac';
        ctx.beginPath();
        ctx.arc(rect.x + 26, y + rowH / 2, 5, 0, Math.PI * 2);
        ctx.fill();
        ctx.textAlign = 'left';
        ctx.fillStyle = '#eef4ff';
        ctx.font = '12px Segoe UI';
        ctx.fillText(templateLabel(template), rect.x + 40, y + 14);
        if (template.subtitle || template.status) {{
          ctx.fillStyle = '#9aa8b8';
          ctx.font = '10.5px Segoe UI';
          ctx.fillText(template.subtitle || template.status, rect.x + 40, y + 29);
        }}
        nodePicker.items.push({{ template, index, x: rect.x + 10, y, w: rect.w - 20, h: rowH }});
        y += rowH + 4;
      }}
      ctx.restore();
    }}
    function drawToolbar(width) {{
      toolbar.items = [];
      const actions = [
        {{ action: 'fit', label: '[]' }},
        {{ action: 'zoom_in', label: '+' }},
        {{ action: 'zoom_out', label: '-' }},
        {{ action: 'grid', label: '#' }}
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

    function draw() {{
      const rect = canvas.getBoundingClientRect();
      ctx.clearRect(0, 0, rect.width, rect.height);
      ctx.fillStyle = '#0d1117'; ctx.fillRect(0, 0, rect.width, rect.height);
      drawGrid(rect.width, rect.height);
      for (const edge of state.edges) drawEdge(edge);
      if (state.drag && state.drag.kind === 'edge') drawTempEdge(state.drag.from, state.drag.to);
      for (const node of state.nodes) drawNode(node);
      drawPalette();
      drawMinimap(rect.width, rect.height);
      drawToolbar(rect.width);
      drawNodePicker();
      ctx.fillStyle = '#8b98a8'; ctx.font = '11px Segoe UI'; ctx.textAlign = 'left';
      ctx.fillText(`${{state.nodes.length}} nodes / ${{state.edges.length}} edges`, 12, rect.height - 16);
    }}

    canvas.addEventListener('mousedown', event => {{
      canvas.focus();
      const p = graphPoint(event);
      if (nodePicker.open) {{
        const pickerHit = hitNodePicker(p.sx, p.sy);
        if (pickerHit && pickerHit.kind === 'item') chooseNodePickerSelection(pickerHit.index);
        else if (pickerHit && pickerHit.kind === 'close') closeNodePicker();
        else if (pickerHit && pickerHit.kind === 'outside') closeNodePicker();
        event.preventDefault();
        return;
      }}      const toolbarAction = hitToolbar(p.sx, p.sy);
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
        state.drag = {{ kind: 'edge', from: output, to: {{ x: p.sx, y: p.sy }} }};
        canvas.style.cursor = 'crosshair';
        draw();
        return;
      }}
      const header = hitHeader(p.x, p.y);
      if (header) {{
        state.selected = header.id;
        state.selectedEdge = null;
        state.drag = {{ kind: 'node', id: header.id, ox: p.x - header.x, oy: p.y - header.y, before: graphSnapshot() }};
        canvas.style.cursor = 'grabbing';
        emitGraphEvent({{ event: 'node_selected', node: header.id }});
      }} else {{
        const body = hitNode(p.x, p.y);
        if (body) {{
          state.selected = body.id;
          state.selectedEdge = null;
          state.drag = null;
          emitGraphEvent({{ event: 'node_selected', node: body.id }});
          draw();
          return;
        }}
        const edge = hitEdge(p.sx, p.sy);
        if (edge) {{
          state.selected = null;
          state.selectedEdge = edge.id;
          state.drag = null;
          emitGraphEvent({{ event: 'edge_selected', edge: edge.id }});
          draw();
          return;
        }}
        state.selected = null;
        state.selectedEdge = null;
        state.drag = {{ kind: 'pan', sx: p.sx, sy: p.sy, vx: state.viewX, vy: state.viewY }};
        canvas.style.cursor = 'grabbing';
        emitGraphEvent({{ event: 'selection_cleared' }});
      }}
      draw();
    }});

    window.addEventListener('mousemove', event => {{
      const p = graphPoint(event);
      state.hoverPort = hitPort(p.sx, p.sy);
      if (!state.drag) {{
        canvas.style.cursor = state.hoverPort && state.hoverPort.side === 'output' ? 'crosshair' : (hitHeader(p.x, p.y) ? 'grab' : (hitEdge(p.sx, p.sy) ? 'pointer' : (hitNode(p.x, p.y) ? 'default' : 'move')));
        draw();
        return;
      }}
      if (state.drag.kind === 'node') {{
        const node = state.nodes.find(n => n.id === state.drag.id);
        if (node) {{ node.x = p.x - state.drag.ox; node.y = p.y - state.drag.oy; }}
      }} else if (state.drag.kind === 'edge') {{
        state.drag.to = {{ x: p.sx, y: p.sy }};
      }} else {{
        state.viewX = state.drag.vx + p.sx - state.drag.sx;
        state.viewY = state.drag.vy + p.sy - state.drag.sy;
      }}
      draw();
    }});

    window.addEventListener('mouseup', event => {{
      if (state.drag && state.drag.kind === 'edge') {{
        const p = graphPoint(event);
        const target = hitPort(p.sx, p.sy, 'input');
        if (createEdge(state.drag.from, target)) {{
          state.selected = null;
          state.selectedEdge = state.edges[state.edges.length - 1].id;
        }}
      }} else if (state.drag && state.drag.kind === 'node') {{
        const node = state.nodes.find(n => n.id === state.drag.id);
        if (node && snapshotCore(state.drag.before) !== snapshotCore(graphSnapshot())) emitGraphMutation({{ event: 'node_moved', node: node.id, position: {{ x: node.x, y: node.y }} }}, state.drag.before);
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
      if (!config.emitEvents) {{
        addNode(p.x, p.y);
        draw();
        return;
      }}
      openNodePicker(p);
    }});

    canvas.addEventListener('keydown', event => {{
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
          draw();
        }} else if (event.key === 'ArrowUp') {{
          event.preventDefault();
          nodePicker.selected = Math.max(0, nodePicker.selected - 1);
          draw();
        }} else if (event.key === 'Backspace') {{
          event.preventDefault();
          nodePicker.query = nodePicker.query.slice(0, -1);
          nodePicker.selected = 0;
          draw();
        }} else if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {{
          event.preventDefault();
          nodePicker.query += event.key;
          nodePicker.selected = 0;
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
        editSelectedNodeTitle();
      }} else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'd') {{
        event.preventDefault();
        duplicateSelectedNode();
        draw();
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
        emitGraphEvent({{ event: 'selection_cleared' }});
        draw();
      }}
    }});

    canvas.addEventListener('wheel', event => {{
      if (!config.enableZoom) return;
      event.preventDefault();
      const p = graphPoint(event);
      const factor = event.deltaY < 0 ? 1.08 : 0.92;
      setZoom(state.zoom * factor, p, event.deltaY < 0 ? 'zoom_in' : 'zoom_out');
    }}, {{ passive: false }});
    window.addEventListener('resize', resize);
    resize();
  </script>
</body>
</html>"""


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


def multi_agent_node_templates() -> tuple[NodeGraphTemplate, ...]:
    """Return runtime-oriented templates for multi-agent workflow graphs."""

    return (
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
            data={
                "node_type": "agent",
                "default_status": "idle",
                "session": {
                    "agent_type": "codex",
                    "capabilities": {"terminal": True, "tools": True},
                    "safety_policy": {"requires_approval": True},
                },
            },
        ),
        NodeGraphTemplate(
            "terminal",
            "Terminal",
            inputs=(NodeGraphPort("stdin", "terminal_input", port_type="terminal_input"),),
            outputs=(
                NodeGraphPort("stdout", "terminal_output", port_type="terminal_output"),
                NodeGraphPort("error", "error", port_type="error"),
            ),
            subtitle="process bridge",
            status="stopped",
            color="#43c6ac",
            width=220,
            data={
                "node_type": "terminal",
                "default_status": "stopped",
                "session": {"agent_type": "terminal", "command": None, "args": [], "cwd": None, "environment": {}},
            },
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
            data={"node_type": "parser", "default_status": "idle"},
        ),
        NodeGraphTemplate(
            "approval_gate",
            "Approval Gate",
            inputs=(NodeGraphPort("request", "approval_request", port_type="approval_request"),),
            outputs=(
                NodeGraphPort("result", "approval_result", port_type="approval_result"),
                NodeGraphPort("error", "error", port_type="error"),
            ),
            subtitle="safety checkpoint",
            status="waiting",
            color="#e0af68",
            width=220,
            data={
                "node_type": "approval_gate",
                "default_status": "waiting",
                "safety_policy": {"requires_human": True},
            },
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
            data={"node_type": "tester", "default_status": "idle"},
        ),
        NodeGraphTemplate(
            "artifact",
            "Artifact",
            inputs=(NodeGraphPort("in", "artifact", port_type="artifact"),),
            outputs=(NodeGraphPort("out", "artifact", port_type="artifact"),),
            subtitle="produced file",
            status="ready",
            color="#f7768e",
            data={"node_type": "artifact", "default_status": "ready"},
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
            data={"node_type": "human_input", "default_status": "waiting"},
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
            data={"node_type": "rule", "default_status": "active"},
        ),
    )


def _port_by_id(ports: Sequence[NodeGraphPort], port_id: str) -> NodeGraphPort | None:
    for port in ports:
        if port.id == port_id:
            return port
    return None


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
    "NodeGraphEdge",
    "NodeGraphNode",
    "NodeGraphPort",
    "NodeGraphTemplate",
    "multi_agent_node_templates",
]

















