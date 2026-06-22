from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
import json
import math

from .widgets import Container, HtmlReport, _AUTO_PARENT


@dataclass(slots=True)
class NodeGraphPort:
    """One input or output socket on a node graph node."""

    id: str
    label: str | None = None


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


@dataclass(frozen=True, slots=True)
class NodeGraphEdge:
    """Connection from one node output port to another node input port."""

    source_node: str
    source_port: str
    target_node: str
    target_port: str
    label: str | None = None
    color: str = "#43c6ac"


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
        width: int | float | None = 920,
        height: int | float | None = 560,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
        **_callbacks: object,
    ) -> None:
        self.nodes = tuple(self._node_from_value(node) for node in nodes)
        self.edges = tuple(self._edge_from_value(edge) for edge in edges)
        self.selected_node = selected_node if selected_node in self._node_ids() else None
        self.show_edge_labels = bool(show_edge_labels)
        self.show_port_labels = bool(show_port_labels)
        self.show_status_labels = bool(show_status_labels)
        self.show_subtitles = bool(show_subtitles)
        self.enable_zoom = bool(enable_zoom)
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
        self.set_html(self._html())

    def set_edges(self, edges: Sequence[NodeGraphEdge | Mapping[str, object]]) -> None:
        self.edges = tuple(self._edge_from_value(edge) for edge in edges)
        self.set_html(self._html())

    def set_node_position(self, node_id: str, x: float, y: float, *, notify: bool = False) -> None:
        del notify
        node = self._node_by_id(node_id)
        node.x = self._finite(x, "x")
        node.y = self._finite(y, "y")
        self.set_html(self._html())

    def node_position(self, node_id: str) -> tuple[float, float]:
        node = self._node_by_id(node_id)
        return node.x, node.y

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
        )

    def _node_ids(self) -> set[str]:
        return {node.id for node in self.nodes}

    def _node_by_id(self, node_id: str) -> NodeGraphNode:
        for node in self.nodes:
            if node.id == node_id:
                return node
        raise KeyError(f"unknown node id {node_id!r}")

    @classmethod
    def _node_from_value(cls, value: NodeGraphNode | Mapping[str, object]) -> NodeGraphNode:
        if isinstance(value, NodeGraphNode):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph nodes must be NodeGraphNode instances or mappings")
        node_id = cls._text(value.get("id"), "node id")
        title = cls._text(value.get("title", node_id), "node title")
        return NodeGraphNode(
            id=node_id,
            title=title,
            x=cls._finite(value.get("x", 0.0), "node x"),
            y=cls._finite(value.get("y", 0.0), "node y"),
            inputs=cls._ports_from_value(value.get("inputs", ())),
            outputs=cls._ports_from_value(value.get("outputs", ())),
            subtitle=None if value.get("subtitle") is None else str(value.get("subtitle")),
            status=None if value.get("status") is None else str(value.get("status")),
            color=str(value.get("color", "#43c6ac")),
            width=cls._positive(value.get("width", 190.0), "node width"),
        )

    @classmethod
    def _edge_from_value(cls, value: NodeGraphEdge | Mapping[str, object]) -> NodeGraphEdge:
        if isinstance(value, NodeGraphEdge):
            return value
        if not isinstance(value, Mapping):
            raise TypeError("NodeGraph edges must be NodeGraphEdge instances or mappings")
        return NodeGraphEdge(
            source_node=cls._text(value.get("source_node"), "source_node"),
            source_port=cls._text(value.get("source_port"), "source_port"),
            target_node=cls._text(value.get("target_node"), "target_node"),
            target_port=cls._text(value.get("target_port"), "target_port"),
            label=None if value.get("label") is None else str(value.get("label")),
            color=str(value.get("color", "#43c6ac")),
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
                ports.append(NodeGraphPort(port_id, None if item.get("label") is None else str(item.get("label"))))
            else:
                raise TypeError("node ports must contain strings, mappings, or NodeGraphPort instances")
        return tuple(ports)

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
    selected_node: str | None,
    show_edge_labels: bool,
    show_port_labels: bool,
    show_status_labels: bool,
    show_subtitles: bool,
    enable_zoom: bool,
) -> str:
    config = {
        "nodes": [_node_payload(node) for node in nodes],
        "edges": [_edge_payload(edge) for edge in edges],
        "selectedNode": selected_node,
        "showEdgeLabels": bool(show_edge_labels),
        "showPortLabels": bool(show_port_labels),
        "showStatusLabels": bool(show_status_labels),
        "showSubtitles": bool(show_subtitles),
        "enableZoom": bool(enable_zoom),
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
    const state = {{ nodes: config.nodes, edges: config.edges, selected: config.selectedNode, selectedEdge: null, viewX: 34, viewY: 32, zoom: 1, drag: null, hoverPort: null }};
    let nodeSerial = state.nodes.length;
    let edgeSerial = state.edges.length;
    for (const edge of state.edges) if (!edge.id) edge.id = `edge-${{++edgeSerial}}`;
    const HEADER = 36;
    const HEADER_PAD = 10;

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

    function nodeHeight(node) {{
      const ports = Math.max(node.inputs.length, node.outputs.length, 1);
      return HEADER + 16 + (config.showSubtitles && node.subtitle ? 18 : 0) + ports * 22;
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
      return screen(side === 'input' ? node.x : node.x + nodeWidth(node), node.y + HEADER + 22 + index * 22);
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

    function canConnect(from, to) {{
      return Boolean(from && to && from.side === 'output' && to.side === 'input' && from.node.id !== to.node.id);
    }}

    function createEdge(from, to) {{
      if (!canConnect(from, to)) return false;
      const exists = state.edges.some(edge => edge.sourceNode === from.node.id && edge.sourcePort === from.port.id && edge.targetNode === to.node.id && edge.targetPort === to.port.id);
      if (exists) return false;
      state.edges.push({{
        id: `edge-${{++edgeSerial}}`,
        sourceNode: from.node.id,
        sourcePort: from.port.id,
        targetNode: to.node.id,
        targetPort: to.port.id,
        label: null,
        color: from.node.color || '#43c6ac'
      }});
      return true;
    }}

    function addNode(x, y) {{
      const id = `node-${{++nodeSerial}}`;
      state.nodes.push({{
        id,
        title: `Node ${{nodeSerial}}`,
        x,
        y,
        inputs: [{{ id: 'in', label: 'in' }}],
        outputs: [{{ id: 'out', label: 'out' }}],
        subtitle: null,
        status: null,
        color: '#43c6ac',
        width: 150
      }});
      state.selected = id;
      state.selectedEdge = null;
    }}

    function duplicateSelectedNode() {{
      const node = state.nodes.find(n => n.id === state.selected);
      if (!node) return;
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
    }}

    function deleteSelection() {{
      if (state.selected) {{
        const nodeId = state.selected;
        state.nodes = state.nodes.filter(node => node.id !== nodeId);
        state.edges = state.edges.filter(edge => edge.sourceNode !== nodeId && edge.targetNode !== nodeId);
        state.selected = null;
      }} else if (state.selectedEdge) {{
        state.edges = state.edges.filter(edge => edge.id !== state.selectedEdge);
        state.selectedEdge = null;
      }}
      draw();
    }}

    function drawGrid(width, height) {{
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
      ctx.fillStyle = '#8b98a8'; ctx.font = '11px Segoe UI'; ctx.textAlign = 'left';
      ctx.fillText('drag headers; output pin to input pin connects; double-click adds; Delete removes' + (config.enableZoom ? '; wheel zooms' : ''), 12, rect.height - 16);
    }}

    canvas.addEventListener('mousedown', event => {{
      canvas.focus();
      const p = graphPoint(event);
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
        state.drag = {{ kind: 'node', id: header.id, ox: p.x - header.x, oy: p.y - header.y }};
        canvas.style.cursor = 'grabbing';
      }} else {{
        const body = hitNode(p.x, p.y);
        if (body) {{
          state.selected = body.id;
          state.selectedEdge = null;
          state.drag = null;
          draw();
          return;
        }}
        const edge = hitEdge(p.sx, p.sy);
        if (edge) {{
          state.selected = null;
          state.selectedEdge = edge.id;
          state.drag = null;
          draw();
          return;
        }}
        state.selected = null;
        state.selectedEdge = null;
        state.drag = {{ kind: 'pan', sx: p.sx, sy: p.sy, vx: state.viewX, vy: state.viewY }};
        canvas.style.cursor = 'grabbing';
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
      }}
      state.drag = null;
      canvas.style.cursor = 'default';
      draw();
    }});

    canvas.addEventListener('dblclick', event => {{
      const p = graphPoint(event);
      if (hitPort(p.sx, p.sy) || hitEdge(p.sx, p.sy) || hitNode(p.x, p.y)) return;
      addNode(p.x, p.y);
      draw();
    }});

    canvas.addEventListener('keydown', event => {{
      if (event.key === 'Delete' || event.key === 'Backspace') {{
        event.preventDefault();
        deleteSelection();
      }} else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'd') {{
        event.preventDefault();
        duplicateSelectedNode();
        draw();
      }} else if (event.key === 'Escape') {{
        state.drag = null;
        state.selected = null;
        state.selectedEdge = null;
        draw();
      }}
    }});

    canvas.addEventListener('wheel', event => {{
      if (!config.enableZoom) return;
      event.preventDefault();
      const p = graphPoint(event);
      const factor = event.deltaY < 0 ? 1.08 : 0.92;
      state.zoom = Math.max(0.55, Math.min(1.8, state.zoom * factor));
      state.viewX = p.sx - p.x * state.zoom;
      state.viewY = p.sy - p.y * state.zoom;
      draw();
    }}, {{ passive: false }});
    window.addEventListener('resize', resize);
    resize();
  </script>
</body>
</html>"""


def _node_payload(node: NodeGraphNode) -> dict[str, object]:
    return {
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


def _edge_payload(edge: NodeGraphEdge) -> dict[str, object]:
    return {
        "sourceNode": edge.source_node,
        "sourcePort": edge.source_port,
        "targetNode": edge.target_node,
        "targetPort": edge.target_port,
        "label": edge.label,
        "color": edge.color,
    }


def _port_payload(port: NodeGraphPort) -> dict[str, object]:
    return {"id": port.id, "label": port.label}


__all__ = ["NodeGraph", "NodeGraphEdge", "NodeGraphNode", "NodeGraphPort"]












