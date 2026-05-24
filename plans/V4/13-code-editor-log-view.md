# V4 CodeEditor And LogView

## Objective

Support developer and operations tools that need readable code, queries,
tracebacks, structured logs, and streaming diagnostics.

## Proposed API

```python
dg.CodeEditor(value=sql, language="sql", on_change=...)
dg.LogView(lines, follow=True, max_lines=10_000)
```

## Behavior

- CodeEditor starts as monospaced multiline TextArea with line numbers.
- Syntax highlighting can be phase 2.
- LogView supports append-only updates and auto-follow.
- LogView should virtualize lines for large logs.

## Native Work

- Reuse TextArea editing for CodeEditor.
- Add line number gutter rendering.
- Add LogView line virtualization and append command.
- Avoid rebuilding full text on every appended log line.

## Python Work

- Add `CodeEditor`.
- Add `LogView`.
- Add `append_line(...)`, `append_lines(...)`, and `clear()`.
- Export callback payloads if selection/clickable links are added later.

## Acceptance

- Large logs remain responsive.
- Follow mode stays pinned to bottom until user scrolls away.
- CodeEditor line numbers align with wrapped/scrolling text.
- First version explicitly documents no full IDE behavior.

