from __future__ import annotations

import atexit
import base64
import hashlib
from importlib import resources
import json
import os
from collections import deque
import socket
import struct
import subprocess
import threading
import time
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from html import escape
from pathlib import Path
from typing import Any

from .widgets import HtmlReport, _AUTO_PARENT, Container

_XTERM_VERSION = "5.5.0"
_ASSET_PACKAGE = "dragongui.assets.terminal"
_TERMINAL_EVENT_SCHEMA_VERSION = 1


@dataclass(slots=True)
class TerminalCommand:
    """Normalized terminal command information."""

    command: str
    args: tuple[str, ...] = ()

    @classmethod
    def from_value(cls, command: str | Sequence[object], args: Sequence[object] = ()) -> "TerminalCommand":
        if isinstance(command, str):
            text = command.strip()
            if not text:
                raise ValueError("Terminal command must be non-empty")
            return cls(text, tuple(str(arg) for arg in args))
        values = tuple(str(part) for part in command)
        if not values or not values[0].strip():
            raise ValueError("Terminal command sequence must start with a non-empty executable")
        if args:
            raise ValueError("Terminal args cannot be supplied when command is already a sequence")
        return cls(values[0], values[1:])

    @property
    def argv(self) -> list[str]:
        return [self.command, *self.args]

    @property
    def label(self) -> str:
        return " ".join(self.argv)

    @property
    def command_line(self) -> str:
        return subprocess.list2cmdline(self.argv)


@dataclass(frozen=True, slots=True)
class TerminalEvent:
    """Structured terminal bridge event for lifecycle and output consumers."""

    event: str
    session_id: int | None = None
    data: str | None = None
    schema_version: int = _TERMINAL_EVENT_SCHEMA_VERSION
    timestamp: float = field(default_factory=time.time)

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "schema_version": self.schema_version,
            "event": self.event,
            "timestamp": self.timestamp,
        }
        if self.session_id is not None:
            payload["session_id"] = self.session_id
        if self.data is not None:
            payload["data"] = self.data
        return payload


@dataclass(frozen=True, slots=True)
class TerminalTranscriptEntry:
    """Append-only terminal transcript chunk independent of rendered xterm state."""

    stream: str
    data: str
    session_id: int | None = None
    timestamp: float = field(default_factory=time.time)

    def to_dict(self) -> dict[str, object]:
        payload: dict[str, object] = {
            "timestamp": self.timestamp,
            "stream": self.stream,
            "data": self.data,
        }
        if self.session_id is not None:
            payload["session_id"] = self.session_id
        return payload


class _SubprocessSession:
    def __init__(
        self,
        command: TerminalCommand,
        *,
        cwd: str | None,
        env: Mapping[str, str] | None,
        cols: int,
        rows: int,
    ) -> None:
        del cols, rows
        merged_env = os.environ.copy()
        if env:
            merged_env.update({str(key): str(value) for key, value in env.items()})
        self.process = subprocess.Popen(
            command.argv,
            cwd=cwd,
            env=merged_env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            shell=False,
        )

    def read(self) -> str:
        if self.process.stdout is None:
            return ""
        data = self.process.stdout.read(4096)
        return data.decode(errors="replace") if data else ""

    def write(self, data: str) -> None:
        if self.process.stdin is None:
            return
        self.process.stdin.write(data.encode())
        self.process.stdin.flush()

    def resize(self, cols: int, rows: int) -> None:
        del cols, rows

    def is_alive(self) -> bool:
        return self.process.poll() is None

    def terminate(self) -> None:
        if self.is_alive():
            self.process.terminate()


class _WinPtySession:
    def __init__(
        self,
        command: TerminalCommand,
        *,
        cwd: str | None,
        env: Mapping[str, str] | None,
        cols: int,
        rows: int,
    ) -> None:
        from winpty import PtyProcess  # type: ignore[import-not-found]

        merged_env = os.environ.copy()
        if env:
            merged_env.update({str(key): str(value) for key, value in env.items()})
        dimensions = (max(int(rows), 1), max(int(cols), 2))
        try:
            self.process = PtyProcess.spawn(
                command.command_line,
                cwd=cwd,
                env=merged_env,
                dimensions=dimensions,
            )
        except TypeError:
            try:
                self.process = PtyProcess.spawn(
                    command.command_line,
                    cwd=cwd,
                    dimensions=dimensions,
                )
            except TypeError:
                self.process = PtyProcess.spawn(command.command_line, dimensions=dimensions)

    def read(self) -> str:
        try:
            return self.process.read(4096)
        except TypeError:
            return self.process.read()

    def write(self, data: str) -> None:
        self.process.write(data)

    def resize(self, cols: int, rows: int) -> None:
        rows = max(int(rows), 1)
        cols = max(int(cols), 2)
        if hasattr(self.process, "setwinsize"):
            self.process.setwinsize(rows, cols)
        elif hasattr(self.process, "set_size"):
            self.process.set_size(rows, cols)

    def is_alive(self) -> bool:
        if hasattr(self.process, "isalive"):
            return bool(self.process.isalive())
        return True

    def terminate(self) -> None:
        if hasattr(self.process, "terminate"):
            try:
                self.process.terminate(force=True)
            except TypeError:
                self.process.terminate()


class TerminalBridge:
    """Local WebSocket bridge between xterm.js and a wrapped command.

    The bridge intentionally has no third-party WebSocket dependency. The only
    optional dependency is ``pywinpty``/``winpty`` for real Windows PTY behavior.
    Without it, the bridge falls back to ordinary subprocess pipes, which are
    useful for simple command output but not enough for full-screen CLIs.
    """

    def __init__(
        self,
        command: str | Sequence[object],
        *,
        args: Sequence[object] = (),
        cwd: str | Path | None = None,
        env: Mapping[str, str] | None = None,
        cols: int = 100,
        rows: int = 30,
        prefer_pty: bool = True,
        on_output: Callable[[str], object] | None = None,
        on_event: Callable[[TerminalEvent], object] | None = None,
        capture_transcript: bool = True,
        max_transcript_entries: int = 10000,
    ) -> None:
        self.command = TerminalCommand.from_value(command, args)
        self.cwd = None if cwd is None else str(cwd)
        self.env = None if env is None else {str(key): str(value) for key, value in env.items()}
        self.cols = max(int(cols), 2)
        self.rows = max(int(rows), 1)
        self.prefer_pty = bool(prefer_pty)
        self.on_output = on_output
        self.on_event = on_event
        self.capture_transcript = bool(capture_transcript)
        self._server: socket.socket | None = None
        self._thread: threading.Thread | None = None
        self._stop = threading.Event()
        self._session_lock = threading.Lock()
        self._session: Any | None = None
        self._session_id: int | None = None
        self._session_seq = 0
        self._event_lock = threading.Lock()
        self._events: deque[TerminalEvent] = deque()
        self._transcript: deque[TerminalTranscriptEntry] = deque(maxlen=max(1, int(max_transcript_entries)))
        self._port: int | None = None
        self._closed = False
        self.status = "not started"
        atexit.register(self.stop)

    @property
    def url(self) -> str:
        if self._port is None:
            raise RuntimeError("TerminalBridge has not been started")
        return f"ws://127.0.0.1:{self._port}/terminal"

    def start(self) -> "TerminalBridge":
        if self._thread is not None:
            return self
        server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind(("127.0.0.1", 0))
        server.listen(4)
        server.settimeout(0.25)
        self._server = server
        self._port = int(server.getsockname()[1])
        self.status = f"listening on 127.0.0.1:{self._port}"
        self._thread = threading.Thread(target=self._serve, name="DragonGuiTerminalBridge", daemon=True)
        self._thread.start()
        self._record_event("bridge_started")
        return self

    def stop(self) -> None:
        if self._closed:
            return
        self._closed = True
        self._stop.set()
        with self._session_lock:
            session = self._session
            self._session = None
        if session is not None:
            try:
                session.terminate()
            except Exception:
                pass
        if self._server is not None:
            try:
                self._server.close()
            except OSError:
                pass
        self.status = "stopped"
        self._record_event("bridge_stopped")

    close = stop
    dispose = stop

    def send_text(self, text: object) -> bool:
        """Write text to the active terminal session, returning False if none is attached."""
        data = str(text)
        with self._session_lock:
            session = self._session
            session_id = self._session_id
            alive = session is not None and session.is_alive()
        if not alive:
            return False
        session.write(data)
        self._record_transcript("input", data, session_id)
        return True

    def send_line(self, text: object = "") -> bool:
        """Write text followed by a newline to the active terminal session."""
        return self.send_text(f"{text}\n")

    @property
    def transcript(self) -> list[dict[str, object]]:
        with self._event_lock:
            return [entry.to_dict() for entry in self._transcript]

    @property
    def events(self) -> list[dict[str, object]]:
        with self._event_lock:
            return [event.to_dict() for event in self._events]

    def drain_events(self) -> list[dict[str, object]]:
        with self._event_lock:
            events = [event.to_dict() for event in self._events]
            self._events.clear()
            return events

    def _record_event(self, event: str, *, data: str | None = None, session_id: int | None = None) -> None:
        item = TerminalEvent(event=event, session_id=session_id, data=data)
        with self._event_lock:
            self._events.append(item)
        if self.on_event is not None:
            self.on_event(item)

    def _record_transcript(self, stream: str, data: str, session_id: int | None) -> None:
        if not data:
            return
        if self.capture_transcript:
            item = TerminalTranscriptEntry(stream=stream, data=data, session_id=session_id)
            with self._event_lock:
                self._transcript.append(item)
        if stream == "output":
            self._record_event("output", data=data, session_id=session_id)
            if self.on_output is not None:
                self.on_output(data)

    def _serve(self) -> None:
        assert self._server is not None
        while not self._stop.is_set():
            try:
                client, _addr = self._server.accept()
            except TimeoutError:
                continue
            except OSError:
                break
            thread = threading.Thread(target=self._handle_client, args=(client,), daemon=True)
            thread.start()

    def _handle_client(self, client: socket.socket) -> None:
        client.settimeout(0.5)
        try:
            self._handshake(client)
            self._wait_for_initial_resize(client)
            session = self._spawn_session()
            output_done = threading.Event()
            output_thread = threading.Thread(
                target=self._pump_output,
                args=(client, session, output_done),
                daemon=True,
            )
            output_thread.start()
            self._pump_input(client, session, output_done)
        except Exception as exc:
            try:
                self._send_text(client, f"\r\n\x1b[31mTerminal bridge error: {exc}\x1b[0m\r\n")
            except Exception:
                pass
        finally:
            try:
                client.close()
            except OSError:
                pass

    def _wait_for_initial_resize(self, client: socket.socket) -> None:
        deadline = time.monotonic() + 2.0
        while time.monotonic() < deadline and not self._stop.is_set():
            try:
                message = self._read_message(client)
            except TimeoutError:
                continue
            if message is None:
                raise ConnectionError("WebSocket closed before terminal size was received")
            try:
                payload = json.loads(message)
            except json.JSONDecodeError:
                continue
            if payload.get("type") != "resize":
                continue
            cols = int(payload.get("cols", self.cols))
            rows = int(payload.get("rows", self.rows))
            self.cols = max(cols, 2)
            self.rows = max(rows, 1)
            return

    def _spawn_session(self) -> Any:
        with self._session_lock:
            if self._session is not None and self._session.is_alive():
                return self._session
            self._session_seq += 1
            session_id = self._session_seq
            session: Any | None = None
            if self.prefer_pty and os.name == "nt":
                try:
                    session = _WinPtySession(
                        self.command,
                        cwd=self.cwd,
                        env=self.env,
                        cols=self.cols,
                        rows=self.rows,
                    )
                    self.status = f"PTY session started: {self.command.label}"
                except Exception as exc:
                    self.status = f"PTY unavailable ({exc}); using subprocess pipes"
            if session is None:
                session = _SubprocessSession(
                    self.command,
                    cwd=self.cwd,
                    env=self.env,
                    cols=self.cols,
                    rows=self.rows,
                )
                if "PTY unavailable" not in self.status:
                    self.status = f"subprocess session started: {self.command.label}"
            self._session = session
            self._session_id = session_id
            self._record_event("session_started", session_id=session_id)
            return session

    def _pump_output(self, client: socket.socket, session: Any, done: threading.Event) -> None:
        with self._session_lock:
            session_id = self._session_id if session is self._session else None
        try:
            while not self._stop.is_set() and not done.is_set() and session.is_alive():
                data = session.read()
                if data:
                    self._record_transcript("output", data, session_id)
                    self._send_text(client, data)
                else:
                    time.sleep(0.01)
        except Exception:
            pass
        finally:
            self._record_event("session_ended", session_id=session_id)
            done.set()

    def _pump_input(self, client: socket.socket, session: Any, done: threading.Event) -> None:
        with self._session_lock:
            session_id = self._session_id if session is self._session else None
        while not self._stop.is_set() and not done.is_set():
            try:
                message = self._read_message(client)
            except TimeoutError:
                continue
            if message is None:
                break
            try:
                payload = json.loads(message)
            except json.JSONDecodeError:
                continue
            kind = payload.get("type")
            if kind == "input":
                data = str(payload.get("data", ""))
                session.write(data)
                self._record_transcript("input", data, session_id)
            elif kind == "resize":
                cols = int(payload.get("cols", self.cols))
                rows = int(payload.get("rows", self.rows))
                self.cols = max(cols, 2)
                self.rows = max(rows, 1)
                session.resize(self.cols, self.rows)
            elif kind == "close":
                break
        done.set()

    def _handshake(self, client: socket.socket) -> None:
        request = b""
        while b"\r\n\r\n" not in request:
            chunk = client.recv(4096)
            if not chunk:
                raise ConnectionError("WebSocket client closed during handshake")
            request += chunk
            if len(request) > 65536:
                raise ConnectionError("WebSocket handshake was too large")
        headers: dict[str, str] = {}
        for line in request.decode(errors="replace").split("\r\n")[1:]:
            if ":" in line:
                key, value = line.split(":", 1)
                headers[key.strip().lower()] = value.strip()
        websocket_key = headers.get("sec-websocket-key")
        if not websocket_key:
            raise ConnectionError("Missing Sec-WebSocket-Key")
        accept = base64.b64encode(
            hashlib.sha1((websocket_key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()
        ).decode("ascii")
        response = (
            "HTTP/1.1 101 Switching Protocols\r\n"
            "Upgrade: websocket\r\n"
            "Connection: Upgrade\r\n"
            f"Sec-WebSocket-Accept: {accept}\r\n\r\n"
        )
        client.sendall(response.encode("ascii"))

    def _read_message(self, client: socket.socket) -> str | None:
        header = self._recv_exact(client, 2)
        if not header:
            return None
        first, second = header
        opcode = first & 0x0F
        masked = bool(second & 0x80)
        length = second & 0x7F
        if length == 126:
            length = struct.unpack("!H", self._recv_exact(client, 2))[0]
        elif length == 127:
            length = struct.unpack("!Q", self._recv_exact(client, 8))[0]
        mask = self._recv_exact(client, 4) if masked else b""
        data = self._recv_exact(client, length) if length else b""
        if masked:
            data = bytes(byte ^ mask[index % 4] for index, byte in enumerate(data))
        if opcode == 8:
            return None
        if opcode == 9:
            self._send_frame(client, data, opcode=10)
            return "{}"
        return data.decode(errors="replace")

    def _recv_exact(self, client: socket.socket, size: int) -> bytes:
        data = b""
        while len(data) < size:
            chunk = client.recv(size - len(data))
            if not chunk:
                raise ConnectionError("WebSocket closed")
            data += chunk
        return data

    def _send_text(self, client: socket.socket, text: str) -> None:
        self._send_frame(client, text.encode(errors="replace"), opcode=1)

    def _send_frame(self, client: socket.socket, payload: bytes, *, opcode: int = 1) -> None:
        header = bytearray([0x80 | opcode])
        length = len(payload)
        if length < 126:
            header.append(length)
        elif length <= 0xFFFF:
            header.extend([126, *struct.pack("!H", length)])
        else:
            header.extend([127, *struct.pack("!Q", length)])
        client.sendall(bytes(header) + payload)


class Terminal(HtmlReport):
    """Interactive terminal widget for wrapping command-line tools.

    The first implementation renders xterm.js in DragonGui's HtmlReport webview
    and connects it to a localhost WebSocket bridge. On Windows, install
    ``pywinpty`` for proper interactive PTY behavior with tools such as Codex
    and Claude Code.
    """

    def __init__(
        self,
        command: str | Sequence[object] = "powershell.exe",
        *,
        args: Sequence[object] = (),
        cwd: str | Path | None = None,
        env: Mapping[str, str] | None = None,
        title: str | None = None,
        cols: int = 100,
        rows: int = 30,
        prefer_pty: bool = True,
        on_output: Callable[[str], object] | None = None,
        on_event: Callable[[TerminalEvent], object] | None = None,
        capture_transcript: bool = True,
        max_transcript_entries: int = 10000,
        xterm_version: str = _XTERM_VERSION,
        width: int | float | None = None,
        height: int | float | None = 520,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.bridge = TerminalBridge(
            command,
            args=args,
            cwd=cwd,
            env=env,
            cols=cols,
            rows=rows,
            prefer_pty=prefer_pty,
            on_output=on_output,
            on_event=on_event,
            capture_transcript=capture_transcript,
            max_transcript_entries=max_transcript_entries,
        ).start()
        self.command = self.bridge.command
        self.title = title or self.command.label
        html = _terminal_html(
            title=self.title,
            ws_url=self.bridge.url,
            xterm_version=xterm_version,
            cols=cols,
            rows=rows,
        )
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

    def stop(self) -> None:
        """Stop the terminal bridge and the wrapped process."""
        self.bridge.stop()

    def start(self) -> "Terminal":
        """Start the terminal bridge if it has not already been started."""
        self.bridge.start()
        return self

    def send_text(self, text: object) -> bool:
        """Write text to the active terminal session, returning False when no session is attached."""
        return self.bridge.send_text(text)

    def send_line(self, text: object = "") -> bool:
        """Write text followed by a newline to the active terminal session."""
        return self.bridge.send_line(text)

    @property
    def transcript(self) -> list[dict[str, object]]:
        """Captured terminal input/output chunks independent of rendered xterm state."""
        return self.bridge.transcript

    @property
    def events(self) -> list[dict[str, object]]:
        """Structured lifecycle/output events captured by the terminal bridge."""
        return self.bridge.events

    def drain_events(self) -> list[dict[str, object]]:
        """Return and clear queued terminal bridge events."""
        return self.bridge.drain_events()

    close = stop
    dispose = stop

def _terminal_html(*, title: str, ws_url: str, xterm_version: str, cols: int, rows: int) -> str:
    del xterm_version
    safe_title = escape(title)
    xterm_css = _asset_text("xterm.css")
    xterm_js = _script_text("xterm.js")
    fit_js = _script_text("addon-fit.js")
    payload = json.dumps(
        {
            "title": title,
            "wsUrl": ws_url,
            "cols": max(int(cols), 2),
            "rows": max(int(rows), 1),
        }
    )
    return f"""<!doctype html>
<html>
<head>
  <meta charset=\"utf-8\" />
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
  <title>{safe_title}</title>
  <style>
    {xterm_css}
    html, body {{ width: 100%; height: 100%; margin: 0; overflow: hidden; background: #0b0f14; }}
    #terminal {{ position: absolute; inset: 0; overflow: hidden; background: #0b0f14; }}
    .xterm {{ width: 100%; height: 100%; line-height: normal; }}
    .xterm, .xterm * {{ box-sizing: content-box; }}
    .xterm .xterm-screen canvas {{ image-rendering: auto; }}
    #status {{ color: #9aa7b5; font: 13px Segoe UI, sans-serif; padding: 10px; }}
  </style>
</head>
<body>
  <div id=\"terminal\"><div id=\"status\">Starting terminal...</div></div>
  <script>{xterm_js}</script>
  <script>{fit_js}</script>
  <script>
    const config = {payload};
    const host = document.getElementById('terminal');
    function fail(message) {{
      host.innerHTML = '<div id="status" style="color:#ff8a8a">' + message + '</div>';
    }}
    try {{
      if (typeof Terminal === 'undefined' || typeof FitAddon === 'undefined') {{
        throw new Error('xterm assets did not load');
      }}
      host.innerHTML = '';
      const term = new Terminal({{
        cols: config.cols,
        rows: config.rows,
        cursorBlink: true,
        convertEol: true,
        fontFamily: 'Consolas, Courier New, monospace',
        fontSize: 15,
        letterSpacing: 0,
        scrollback: 5000,
        theme: {{ background: '#0b0f14', foreground: '#edf2f7', cursor: '#3fbf9f' }}
      }});
      const fit = new FitAddon.FitAddon();
      term.loadAddon(fit);
      term.open(host);
      let socket = null;
      function syncRenderSurface() {{
        const dimensions = term._core && term._core._renderService && term._core._renderService.dimensions;
        if (!dimensions || !dimensions.css || !dimensions.css.canvas) {{
          return;
        }}
        const width = dimensions.css.canvas.width + 'px';
        const height = dimensions.css.canvas.height + 'px';
        const screen = host.querySelector('.xterm-screen');
        if (screen) {{
          screen.style.width = width;
          screen.style.height = height;
        }}
        for (const canvas of host.querySelectorAll('.xterm-screen canvas')) {{
          canvas.style.width = width;
          canvas.style.height = height;
        }}
      }}
      function fitAndNotify() {{
        fit.fit();
        syncRenderSurface();
        if (socket && socket.readyState === WebSocket.OPEN) {{
          socket.send(JSON.stringify({{ type: 'resize', cols: term.cols, rows: term.rows }}));
        }}
      }}
      requestAnimationFrame(() => requestAnimationFrame(() => {{
        fitAndNotify();
        term.focus();
      }}));

      socket = new WebSocket(config.wsUrl);
      socket.addEventListener('open', () => {{
        fitAndNotify();
      }});
      let sawFirstOutput = false;
      function refreshAfterStartupOutput() {{
        requestAnimationFrame(() => requestAnimationFrame(() => {{
          fitAndNotify();
          term.refresh(0, Math.max(term.rows - 1, 0));
          term.focus();
        }}));
      }}
      socket.addEventListener('message', (event) => {{
        term.write(event.data, () => {{
          if (!sawFirstOutput) {{
            sawFirstOutput = true;
            setTimeout(refreshAfterStartupOutput, 100);
          }}
        }});
      }});
      socket.addEventListener('close', () => term.writeln('\\r\\n[terminal bridge closed]'));
      socket.addEventListener('error', () => term.writeln('\\r\\n[terminal bridge connection failed]'));
      term.onData((data) => {{
        if (socket.readyState === WebSocket.OPEN) {{
          socket.send(JSON.stringify({{ type: 'input', data }}));
        }}
      }});
      window.addEventListener('resize', () => {{
        requestAnimationFrame(fitAndNotify);
      }});
    }} catch (error) {{
      fail('Terminal startup failed: ' + error.message);
    }}
  </script>
</body>
</html>"""


def _asset_text(name: str) -> str:
    return resources.files(_ASSET_PACKAGE).joinpath(name).read_text(encoding="utf-8")


def _script_text(name: str) -> str:
    return _asset_text(name).replace("</script", "<\\/script")
