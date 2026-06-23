from __future__ import annotations

import time
from collections import defaultdict
from collections.abc import Mapping
from dataclasses import dataclass, field

_AGENT_MESSAGE_SCHEMA_VERSION = 1
_REQUIRED_FIELDS = ("to", "from", "type", "id")
_QUEUE_STATUSES = {"queued", "held", "delivered", "failed"}


@dataclass(frozen=True, slots=True)
class AgentMessage:
    """Parsed agent-to-agent text envelope."""

    to: str
    from_: str
    type: str
    id: str
    body: str
    fields: dict[str, str] = field(default_factory=dict)
    schema_version: int = _AGENT_MESSAGE_SCHEMA_VERSION
    timestamp: float = field(default_factory=time.time)

    def __post_init__(self) -> None:
        object.__setattr__(self, "to", str(self.to))
        object.__setattr__(self, "from_", str(self.from_))
        object.__setattr__(self, "type", str(self.type))
        object.__setattr__(self, "id", str(self.id))
        object.__setattr__(self, "body", str(self.body))
        object.__setattr__(self, "fields", {str(key): str(value) for key, value in self.fields.items()})

    @property
    def sender(self) -> str:
        return self.from_

    @property
    def target(self) -> str:
        return self.to

    def to_dict(self) -> dict[str, object]:
        return {
            "schema_version": self.schema_version,
            "to": self.to,
            "from": self.from_,
            "type": self.type,
            "id": self.id,
            "fields": dict(self.fields),
            "body": self.body,
            "timestamp": self.timestamp,
        }


@dataclass(frozen=True, slots=True)
class AgentEnvelopeParseEvent:
    """Parser event for accepted, partial, malformed, or duplicate envelopes."""

    event: str
    message_id: str | None = None
    reason: str | None = None
    data: dict[str, object] = field(default_factory=dict)
    schema_version: int = _AGENT_MESSAGE_SCHEMA_VERSION
    timestamp: float = field(default_factory=time.time)

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "schema_version": self.schema_version,
            "event": self.event,
            "timestamp": self.timestamp,
        }
        if self.message_id is not None:
            payload["message_id"] = self.message_id
        if self.reason is not None:
            payload["reason"] = self.reason
        if self.data:
            payload["data"] = dict(self.data)
        return payload


class AgentEnvelopeParser:
    """Incremental parser for terminal transcript message envelopes."""

    def __init__(self) -> None:
        self._buffer = ""
        self._seen_ids: set[str] = set()
        self._events: list[AgentEnvelopeParseEvent] = []

    @property
    def pending_text(self) -> str:
        return self._buffer

    @property
    def events(self) -> list[dict[str, object]]:
        return [event.to_dict() for event in self._events]

    def drain_events(self) -> list[dict[str, object]]:
        events = self.events
        self._events.clear()
        return events

    def feed(self, text: object) -> list[AgentMessage]:
        self._buffer += str(text)
        messages: list[AgentMessage] = []

        while True:
            start = _find_envelope_start(self._buffer)
            if start < 0:
                self._buffer = _partial_marker_suffix(self._buffer)
                break
            if start > 0:
                self._buffer = self._buffer[start:]

            envelope, remainder = _pop_complete_envelope(self._buffer)
            if envelope is None:
                self._record("partial", reason="waiting for @end")
                break

            self._buffer = remainder
            message, error = _parse_envelope(envelope)
            if error is not None:
                self._record("malformed", reason=error)
                continue
            assert message is not None
            if message.id in self._seen_ids:
                self._record("duplicate", message_id=message.id, reason="duplicate message id rejected")
                continue
            self._seen_ids.add(message.id)
            self._record("parsed", message_id=message.id, data={"to": message.to, "type": message.type})
            messages.append(message)

        return messages

    def _record(
        self,
        event: str,
        *,
        message_id: str | None = None,
        reason: str | None = None,
        data: Mapping[str, object] | None = None,
    ) -> None:
        self._events.append(
            AgentEnvelopeParseEvent(
                event=event,
                message_id=message_id,
                reason=reason,
                data={} if data is None else dict(data),
            )
        )


@dataclass(slots=True)
class AgentRouterQueueItem:
    """Delivery-state record for one parsed agent message."""

    message: AgentMessage
    status: str = "queued"
    reason: str | None = None
    queued_at: float = field(default_factory=time.time)
    updated_at: float = field(default_factory=time.time)
    schema_version: int = _AGENT_MESSAGE_SCHEMA_VERSION

    def __post_init__(self) -> None:
        if self.status not in _QUEUE_STATUSES:
            raise ValueError(f"Unsupported queue status: {self.status}")
        self.reason = None if self.reason is None else str(self.reason)

    def set_status(self, status: str, reason: str | None = None) -> None:
        if status not in _QUEUE_STATUSES:
            raise ValueError(f"Unsupported queue status: {status}")
        self.status = status
        self.reason = None if reason is None else str(reason)
        self.updated_at = time.time()

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "schema_version": self.schema_version,
            "message": self.message.to_dict(),
            "target": self.message.to,
            "message_id": self.message.id,
            "status": self.status,
            "queued_at": self.queued_at,
            "updated_at": self.updated_at,
        }
        if self.reason is not None:
            payload["reason"] = self.reason
        return payload


class AgentRouterQueue:
    """Model-level queue for parsed agent messages grouped by target."""

    def __init__(self) -> None:
        self._items: dict[str, AgentRouterQueueItem] = {}
        self._by_target: dict[str, list[str]] = defaultdict(list)
        self._events: list[dict[str, object]] = []

    @property
    def events(self) -> list[dict[str, object]]:
        return [dict(event) for event in self._events]

    def enqueue(
        self,
        message: AgentMessage,
        *,
        hold: bool = False,
        reason: str | None = None,
    ) -> AgentRouterQueueItem:
        if message.id in self._items:
            item = self._items[message.id]
            self._record("duplicate", message.id, item.status, "duplicate message id rejected")
            return item
        status = "held" if hold else "queued"
        item = AgentRouterQueueItem(message=message, status=status, reason=reason)
        self._items[message.id] = item
        self._by_target[message.to].append(message.id)
        self._record("enqueued", message.id, status, reason)
        return item

    def for_target(self, target: str, *, status: str | None = None) -> list[AgentRouterQueueItem]:
        ids = self._by_target.get(str(target), [])
        items = [self._items[message_id] for message_id in ids]
        if status is not None:
            items = [item for item in items if item.status == status]
        return list(items)

    def mark_delivered(self, message_id: str, reason: str | None = None) -> bool:
        return self._set_status(message_id, "delivered", reason)

    def mark_held(self, message_id: str, reason: str | None = None) -> bool:
        return self._set_status(message_id, "held", reason)

    def mark_failed(self, message_id: str, reason: str | None = None) -> bool:
        return self._set_status(message_id, "failed", reason)

    def to_dict(self) -> dict[str, object]:
        return {
            "schema_version": _AGENT_MESSAGE_SCHEMA_VERSION,
            "items": [item.to_dict() for item in self._items.values()],
            "by_target": {
                target: [self._items[message_id].to_dict() for message_id in message_ids]
                for target, message_ids in self._by_target.items()
            },
            "events": self.events,
        }

    snapshot = to_dict

    def _set_status(self, message_id: str, status: str, reason: str | None) -> bool:
        item = self._items.get(str(message_id))
        if item is None:
            self._record("failed", str(message_id), "failed", "unknown message id")
            return False
        item.set_status(status, reason)
        self._record("status_changed", item.message.id, status, reason)
        return True

    def _record(self, event: str, message_id: str, status: str, reason: str | None) -> None:
        payload: dict[str, object] = {
            "schema_version": _AGENT_MESSAGE_SCHEMA_VERSION,
            "event": event,
            "message_id": message_id,
            "status": status,
            "timestamp": time.time(),
        }
        if reason is not None:
            payload["reason"] = str(reason)
        self._events.append(payload)


def _find_envelope_start(text: str) -> int:
    for marker in ("@to ", "@to\t"):
        index = text.find(marker)
        if index >= 0 and (index == 0 or text[index - 1] in "\r\n"):
            return index
    return -1


def _partial_marker_suffix(text: str) -> str:
    for size in range(min(len(text), 3), 0, -1):
        suffix = text[-size:]
        if "@to".startswith(suffix):
            return suffix
    return ""


def _pop_complete_envelope(text: str) -> tuple[str | None, str]:
    offset = 0
    for line in text.splitlines(keepends=True):
        next_offset = offset + len(line)
        if line.strip() == "@end":
            return text[:next_offset], text[next_offset:]
        offset = next_offset
    return None, text


def _parse_envelope(text: str) -> tuple[AgentMessage | None, str | None]:
    fields: dict[str, str] = {}
    body_lines: list[str] = []
    in_body = False

    for raw_line in text.splitlines():
        line = raw_line.rstrip("\r")
        if line.strip() == "@end":
            break
        if not in_body and line.startswith("@"):
            name, value = _parse_field_line(line)
            if name is None:
                return None, f"invalid field line: {line}"
            fields[name] = value
            continue
        in_body = True
        body_lines.append(line)

    missing = [name for name in _REQUIRED_FIELDS if not fields.get(name)]
    if missing:
        return None, f"missing required field(s): {', '.join(missing)}"
    body = "\n".join(body_lines).strip()
    if not body:
        return None, "missing body"

    optional_fields = {key: value for key, value in fields.items() if key not in _REQUIRED_FIELDS}
    return (
        AgentMessage(
            to=fields["to"],
            from_=fields["from"],
            type=fields["type"],
            id=fields["id"],
            fields=optional_fields,
            body=body,
        ),
        None,
    )


def _parse_field_line(line: str) -> tuple[str | None, str]:
    text = line[1:]
    if not text:
        return None, ""
    parts = text.split(None, 1)
    name = parts[0].strip().lower()
    if not name:
        return None, ""
    value = parts[1].strip() if len(parts) > 1 else ""
    return name, value
