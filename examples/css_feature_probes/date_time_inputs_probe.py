from __future__ import annotations

import sys
from datetime import date, datetime, time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


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

    HLayout.grid {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        gap: 12px;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 360px;
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
        font-weight: 800;
        padding: 8px 10px;
        width: 100%;
    }

    HLayout.field-row {
        width: 100%;
        height: 40px;
        gap: 10px;
        align-items: center;
    }

    Label.field-label {
        width: 112px;
        color: rgba(246, 249, 255, 0.70);
        font-weight: 750;
    }

    TextInput.temporal-input {
        flex: 1;
        min-width: 0;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 9px;
        color: rgba(246, 249, 255, 0.94);
        padding-left: 10px;
        padding-right: 10px;
        font-family: "Consolas";
    }

    TextInput.temporal-input:focus {
        outline: 2px solid rgba(255, 209, 102, 0.68);
        outline-offset: 2px;
    }

    TextInput.temporal-input.invalid {
        background: rgba(255, 92, 122, 0.13);
        border-color: rgba(255, 92, 122, 0.86);
        color: rgba(255, 230, 235, 0.98);
    }

    TextInput.date-input {
        border-color: rgba(116, 221, 176, 0.42);
    }

    TextInput.time-input {
        border-color: rgba(90, 169, 255, 0.42);
    }

    TextInput.datetime-input {
        border-color: rgba(255, 209, 102, 0.42);
    }

    Button {
        min-width: 118px;
        border-radius: 9px;
        font-weight: 800;
    }
    """
)

win = dg.Window("Date/time input probe", width=880, height=430)

state = {
    "date": "2026-05-22",
    "time": "14:30",
    "datetime": "2026-05-22T14:30:00",
}


def status_text() -> str:
    return "Date: {date} | Time: {time} | DateTime: {datetime}".format(**state)


with dg.VLayout(class_="root"):
    dg.Label("Date and time inputs", class_="title")
    status = dg.Label(status_text(), class_="status")

    def update(name: str, value: str) -> None:
        state[name] = value
        status.set_value(status_text())

    with dg.HLayout(class_="grid"):
        with dg.Panel("ISO inputs", class_="case"):
            dg.Label("Callbacks commit only valid ISO values; invalid edits keep their text and use the danger class.", class_="caption")
            with dg.HLayout(class_="field-row"):
                dg.Label("Date", class_="field-label")
                date_field = dg.DateInput(
                    date(2026, 5, 22),
                    on_change=lambda value: update("date", value),
                )
            with dg.HLayout(class_="field-row"):
                dg.Label("Time", class_="field-label")
                time_field = dg.TimeInput(
                    time(14, 30),
                    on_change=lambda value: update("time", value),
                )
            with dg.HLayout(class_="field-row"):
                dg.Label("DateTime", class_="field-label")
                datetime_field = dg.DateTimeInput(
                    datetime(2026, 5, 22, 14, 30),
                    on_change=lambda value: update("datetime", value),
                )

        with dg.Panel("Programmatic setters", class_="case"):
            dg.Label("Setters normalize Python date/time objects to the same callback contract.", class_="caption")

            def set_open() -> None:
                date_field.set_value("2026-06-01", notify=True)
                time_field.set_value("09:15", notify=True)
                datetime_field.set_value("2026-06-01T09:15", notify=True)

            def set_close() -> None:
                date_field.set_value(date(2026, 6, 5), notify=True)
                time_field.set_value(time(17, 45), notify=True)
                datetime_field.set_value(datetime(2026, 6, 5, 17, 45), notify=True)

            with dg.HLayout(class_="field-row"):
                dg.Button("Market open", on_click=set_open)
                dg.Button("Market close", on_click=set_close)

    dg.Label("PASS: DateInput, TimeInput, and DateTimeInput normalize ISO values, suppress invalid commits, and expose invalid styling.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
