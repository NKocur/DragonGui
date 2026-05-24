from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


PYTHON_SAMPLE = """from __future__ import annotations

def moving_average(values: list[float], width: int) -> list[float]:
    if width <= 0:
        raise ValueError("width must be positive")
    out: list[float] = []
    total = 0.0
    for index, value in enumerate(values):
        total += value
        if index >= width:
            total -= values[index - width]
        out.append(total / min(index + 1, width))
    return out
"""

SQL_SAMPLE = """select
    sensor_id,
    date_trunc('minute', captured_at) as minute,
    avg(temperature_c) as average_temperature,
    max(vibration_mm_s) as peak_vibration
from sensor_samples
where captured_at >= now() - interval '6 hours'
group by sensor_id, minute
order by minute desc, sensor_id;
"""


app = dg.App(theme=dg.Theme.dark(accent="#74ddb0", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #10141b;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    HLayout.content {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        gap: 12px;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 380px;
        height: 100%;
        min-height: 0;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 12px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    HLayout.actions {
        width: 100%;
        height: 36px;
        gap: 8px;
    }

    Button {
        min-width: 106px;
        border-radius: 8px;
        font-weight: 800;
    }

    CodeEditor {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        background: rgba(5, 9, 14, 0.74);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 9px;
        color: rgba(242, 247, 255, 0.94);
        font-family: "Consolas";
        font-size: 13px;
        line-height: 19px;
    }

    CodeEditor:focus {
        outline: 2px solid rgba(255, 209, 102, 0.62);
        outline-offset: 2px;
    }

    CodeEditor::gutter {
        width: 52px;
        background: rgba(255, 255, 255, 0.055);
        border-color: rgba(255, 255, 255, 0.13);
    }

    CodeEditor::line-number {
        color: rgba(246, 249, 255, 0.42);
        font-family: "Consolas";
        font-size: 12px;
        font-variant-numeric: tabular-nums;
    }

    CodeEditor.readonly {
        opacity: 0.70;
    }
    """
)

win = dg.Window("CodeEditor probe", width=960, height=560)

state = {"language": "python", "chars": len(PYTHON_SAMPLE)}


def status_text() -> str:
    return f"Language: {state['language']} | Characters: {state['chars']}"


with dg.VLayout(class_="root"):
    dg.Label("CodeEditor", class_="title")
    status = dg.Label(status_text(), class_="status")

    def mark(language: str, value: str) -> None:
        state["language"] = language
        state["chars"] = len(value)
        status.set_value(status_text())

    with dg.HLayout(class_="content"):
        with dg.Panel("Editable", class_="case"):
            dg.Label("Native multiline editing should keep the gutter, line numbers, caret, and scroll position aligned.", class_="caption")
            editor = dg.CodeEditor(
                PYTHON_SAMPLE,
                language="python",
                rows=12,
                on_change=lambda value: mark(state["language"], value),
            )
            with dg.HLayout(class_="actions"):
                dg.Button(
                    "Load Python",
                    on_click=lambda: (editor.set_value(PYTHON_SAMPLE), mark("python", PYTHON_SAMPLE)),
                )
                dg.Button(
                    "Load SQL",
                    on_click=lambda: (editor.set_value(SQL_SAMPLE), mark("sql", SQL_SAMPLE)),
                )

        with dg.Panel("Read-only styled", class_="case"):
            dg.Label("Disabled editors keep the same code layout and gutter styling without accepting edits.", class_="caption")
            dg.CodeEditor(
                SQL_SAMPLE,
                language="sql",
                rows=12,
                disabled=True,
                class_="readonly",
            )


if __name__ == "__main__":
    print(app.run(win))
