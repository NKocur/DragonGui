from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


class ClickTile(dg.PaintWidget):
    def __init__(self, title: str, color: str, **kwargs: object) -> None:
        self.title = title
        self.color = color
        self.clicks = 0
        self.active = False
        self.status: dg.Label | None = None
        self.badge: dg.Badge | None = None
        super().__init__(
            extension_type="click_tile",
            on_click=self._clicked,
            on_pointer_down=self._pointer_down,
            on_wheel=self._wheel,
            on_key_down=self._key_down,
            **kwargs,
        )

    def bind_outputs(self, status: dg.Label, badge: dg.Badge) -> None:
        self.status = status
        self.badge = badge

    def measure(self, constraints: dg.MeasureConstraints) -> dg.Size:
        return constraints.clamp(dg.Size(180, 88))

    def paint(self, ctx: dg.PaintContext) -> None:
        fill = self.color if self.active else "surface"
        ctx.rounded_rect(0, 0, ctx.width, ctx.height, radius=10, fill=fill)
        ctx.rounded_rect(
            1.5,
            1.5,
            max(1.0, ctx.width - 3.0),
            max(1.0, ctx.height - 3.0),
            radius=9,
            stroke=self.color,
            stroke_width=1.5,
        )
        ctx.circle(22, 25, 8, fill="accent" if self.active else self.color)
        ctx.text(42, 14, self.title, fill="text", font_size=13, font_weight=800)
        ctx.text(42, 40, f"{self.clicks} clicks", fill="muted", font_size=11)

    def _clicked(self) -> None:
        self.clicks += 1
        self.active = not self.active
        if self.status is not None:
            state = "active" if self.active else "idle"
            self.status.set_value(f"{self.title} clicked: {self.clicks} ({state})")
        if self.badge is not None:
            self.badge.set_value(f"{self.clicks}")
        self.repaint()

    def _pointer_down(self, event: dg.PaintPointerEvent) -> None:
        if self.status is not None:
            self.status.set_value(
                f"{self.title} pointer down at {event.local_x:.0f}, {event.local_y:.0f}"
            )

    def _wheel(self, event: dg.PaintPointerEvent) -> None:
        if self.status is not None:
            self.status.set_value(f"{self.title} wheel delta {event.dy:+.1f}")

    def _key_down(self, event: dg.PaintKeyEvent) -> None:
        if self.status is not None:
            self.status.set_value(f"{self.title} key {event.key}")


app = dg.App(theme=dg.Theme.dark(accent="#7dd3fc", radius=8, focus="#fbbf24"))
app.stylesheet(
    """
    Window {
        background: #10151f;
        color: #eef4ff;
        padding: 18px;
        gap: 14px;
        font-size: 14px;
    }

    Label.title {
        font-size: 20px;
        font-weight: 850;
        color: white;
    }

    Label.caption {
        color: rgba(238, 244, 255, 0.7);
    }

    HLayout.tiles {
        gap: 12px;
        flex-wrap: wrap;
    }

    Panel.tile-card {
        width: 216px;
        min-height: 0;
        padding: 12px;
        gap: 8px;
        background: rgba(20, 29, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 9px;
    }

    ExtensionWidget.click-tile {
        width: 100%;
        height: 88px;
        border-radius: 10px;
    }
    """
)

win = dg.Window("PaintWidget Events Probe", width=860, height=420)

with dg.VLayout(style={"width": "100%", "height": "100%", "gap": 12}):
    dg.Label("PaintWidget Events Probe", class_="title")
    status = dg.Label(
        "Click a custom-painted tile. The tile repaints; wheel and focused key events update this line.",
        class_="caption",
    )
    with dg.HLayout(class_="tiles"):
        for title, color in (
            ("Loader", "#7dd3fc"),
            ("Trainer", "#86efac"),
            ("Evaluator", "#fbbf24"),
            ("Exporter", "#f472b6"),
        ):
            with dg.Panel(title, class_="tile-card"):
                tile = ClickTile(title, color, class_="click-tile")
                badge = dg.Badge("0", level="info")
                tile.bind_outputs(status, badge)


if __name__ == "__main__":
    app.run(win)
