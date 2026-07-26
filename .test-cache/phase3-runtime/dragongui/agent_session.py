from __future__ import annotations

import time
from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any

from .terminal import TerminalEvent

_AGENT_SESSION_SCHEMA_VERSION = 1


@dataclass(frozen=True, slots=True)
class AgentSessionLogEntry:
    """Append-only agent session event or transcript entry."""

    kind: str
    event: str
    session_id: str
    timestamp: float = field(default_factory=time.time)
    data: object | None = None
    schema_version: int = _AGENT_SESSION_SCHEMA_VERSION

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "schema_version": self.schema_version,
            "kind": self.kind,
            "event": self.event,
            "session_id": self.session_id,
            "timestamp": self.timestamp,
        }
        if self.data is not None:
            payload["data"] = self.data
        return payload


@dataclass(slots=True)
class AgentSessionRecord:
    """Serializable ownership and runtime state for an agent session."""

    session_id: str
    node_id: str
    agent_type: str
    command: str | None = None
    args: tuple[str, ...] = ()
    cwd: str | None = None
    env: dict[str, str] = field(default_factory=dict)
    status: str = "created"
    status_reason: str | None = None
    capabilities: dict[str, object] = field(default_factory=dict)
    safety_policy: dict[str, object] = field(default_factory=dict)
    transcript_cursors: dict[str, int] = field(default_factory=dict)
    schema_version: int = _AGENT_SESSION_SCHEMA_VERSION

    def __post_init__(self) -> None:
        self.session_id = str(self.session_id)
        self.node_id = str(self.node_id)
        self.agent_type = str(self.agent_type)
        self.command = None if self.command is None else str(self.command)
        self.args = tuple(str(arg) for arg in self.args)
        self.cwd = None if self.cwd is None else str(self.cwd)
        self.env = {str(key): str(value) for key, value in self.env.items()}
        self.status = str(self.status)
        self.status_reason = None if self.status_reason is None else str(self.status_reason)
        self.capabilities = dict(self.capabilities)
        self.safety_policy = dict(self.safety_policy)
        self.transcript_cursors = {
            str(key): int(value) for key, value in self.transcript_cursors.items()
        }

    def set_status(self, status: str, reason: str | None = None) -> None:
        self.status = str(status)
        self.status_reason = None if reason is None else str(reason)

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "schema_version": self.schema_version,
            "session_id": self.session_id,
            "node_id": self.node_id,
            "agent_type": self.agent_type,
            "command": self.command,
            "args": list(self.args),
            "cwd": self.cwd,
            "env": dict(self.env),
            "status": self.status,
            "capabilities": dict(self.capabilities),
            "safety_policy": dict(self.safety_policy),
            "transcript_cursors": dict(self.transcript_cursors),
        }
        if self.status_reason is not None:
            payload["status_reason"] = self.status_reason
        return payload


class AgentSession:
    """Python-side model binding a graph node to a terminal-backed agent session."""

    def __init__(
        self,
        session_id: str,
        node_id: str,
        agent_type: str,
        *,
        command: str | Sequence[object] | None = None,
        args: Sequence[object] = (),
        cwd: str | None = None,
        env: Mapping[str, str] | None = None,
        status: str = "created",
        status_reason: str | None = None,
        capabilities: Mapping[str, object] | None = None,
        safety_policy: Mapping[str, object] | None = None,
    ) -> None:
        command_name: str | None
        command_args: tuple[str, ...]
        if command is None or isinstance(command, str):
            command_name = command
            command_args = tuple(str(arg) for arg in args)
        else:
            values = tuple(str(part) for part in command)
            command_name = values[0] if values else None
            command_args = values[1:]
            if args:
                command_args = (*command_args, *(str(arg) for arg in args))

        self.record = AgentSessionRecord(
            session_id=session_id,
            node_id=node_id,
            agent_type=agent_type,
            command=command_name,
            args=command_args,
            cwd=cwd,
            env={} if env is None else dict(env),
            status=status,
            status_reason=status_reason,
            capabilities={} if capabilities is None else dict(capabilities),
            safety_policy={} if safety_policy is None else dict(safety_policy),
        )
        self._events: list[AgentSessionLogEntry] = []
        self._transcript: list[AgentSessionLogEntry] = []

    @property
    def session_id(self) -> str:
        return self.record.session_id

    @property
    def node_id(self) -> str:
        return self.record.node_id

    @property
    def status(self) -> str:
        return self.record.status

    @property
    def events(self) -> list[dict[str, object]]:
        return [entry.to_dict() for entry in self._events]

    @property
    def transcript(self) -> list[dict[str, object]]:
        return [entry.to_dict() for entry in self._transcript]

    def append_event(
        self,
        event: str,
        *,
        data: object | None = None,
        timestamp: float | None = None,
    ) -> AgentSessionLogEntry:
        entry = AgentSessionLogEntry(
            kind="event",
            event=str(event),
            session_id=self.session_id,
            timestamp=time.time() if timestamp is None else float(timestamp),
            data=data,
        )
        self._events.append(entry)
        return entry

    def append_transcript(
        self,
        data: object,
        *,
        stream: str = "output",
        timestamp: float | None = None,
    ) -> AgentSessionLogEntry:
        entry = AgentSessionLogEntry(
            kind="transcript",
            event=str(stream),
            session_id=self.session_id,
            timestamp=time.time() if timestamp is None else float(timestamp),
            data=str(data),
        )
        self._transcript.append(entry)
        self.record.transcript_cursors[str(stream)] = len(self._transcript)
        return entry

    def apply_terminal_event(self, event: TerminalEvent | Mapping[str, object]) -> AgentSessionLogEntry:
        payload = event.to_dict() if isinstance(event, TerminalEvent) else dict(event)
        name = str(payload.get("event", ""))
        timestamp = float(payload.get("timestamp", time.time()))
        data = payload.get("data")

        if name == "bridge_started":
            self.record.set_status("starting", "terminal bridge started")
        elif name == "session_started":
            self.record.set_status("running", "terminal session started")
        elif name == "session_ended":
            self.record.set_status("exited", "terminal session ended")
        elif name == "bridge_stopped":
            self.record.set_status("stopped", "terminal bridge stopped")

        entry = self.append_event(name, data=_compact_terminal_event_payload(payload), timestamp=timestamp)
        if name == "output" and data is not None:
            self.append_transcript(data, stream="output", timestamp=timestamp)
        return entry

    def to_dict(self) -> dict[str, object]:
        return {
            "schema_version": _AGENT_SESSION_SCHEMA_VERSION,
            "record": self.record.to_dict(),
            "events": self.events,
            "transcript": self.transcript,
        }

    snapshot = to_dict


def _compact_terminal_event_payload(payload: Mapping[str, object]) -> dict[str, object] | None:
    compact: dict[str, object] = {}
    if "session_id" in payload:
        compact["terminal_session_id"] = payload["session_id"]
    if "data" in payload:
        compact["data"] = payload["data"]
    return compact or None
