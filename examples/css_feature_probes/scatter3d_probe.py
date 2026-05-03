from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_grid

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("scatter3d_probe.py requires NumPy for packed point data") from exc


class ScatterProbeFrame:
    columns = ("x", "y", "z", "energy", "size", "group", "phase")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "str", "float32")

    def __init__(
        self,
        rows: int = 1200,
        *,
        turns: float = 5.5,
        phase: float = 0.0,
        scale: float = 1.0,
        all_nonfinite: bool = False,
    ) -> None:
        self.shape = (rows, len(self.columns))
        if rows <= 0:
            self.x = np.array([], dtype=np.float32)
            self.y = np.array([], dtype=np.float32)
            self.z = np.array([], dtype=np.float32)
            self.energy = np.array([], dtype=np.float32)
            self.size = np.array([], dtype=np.float32)
            self.group = np.array([], dtype=object)
            self.phase = np.array([], dtype=np.float32)
            return

        t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
        theta = (t * np.float32(math.tau * turns)) + np.float32(phase)
        radius = np.float32(0.28) + t * np.float32(2.8 * scale)
        self.x = np.cos(theta) * radius
        self.y = np.sin(theta) * radius
        self.z = (t - np.float32(0.5)) * np.float32(2.6 * scale)
        self.energy = (np.exp(t * np.float32(2.2)) + np.float32(0.08)).astype(np.float32)
        self.size = (np.float32(1.0) + np.sin(theta * np.float32(1.7)) ** 2).astype(np.float32)
        self.group = np.where(t > 0.68, "outer", np.where(t > 0.34, "middle", "inner"))
        self.phase = t
        if all_nonfinite:
            self.x[:] = np.nan
            self.y[:] = np.inf
            self.z[:] = np.nan

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


class ArrayFrame:
    def __init__(self, **columns: object) -> None:
        self.columns = tuple(columns.keys())
        for key, value in columns.items():
            setattr(self, key, np.asarray(value, dtype=object if key == "group" else np.float32))
        first = next(iter(columns.values()))
        self.shape = (len(first), len(self.columns))

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


base_frame = ScatterProbeFrame()
alternate_frame = ScatterProbeFrame(phase=0.72, scale=0.82)
empty_frame = ScatterProbeFrame(rows=0)
all_nonfinite_frame = ScatterProbeFrame(rows=1200, all_nonfinite=True)
compact_frame = ScatterProbeFrame(rows=900, turns=4.0, scale=0.75)
front_frame = ScatterProbeFrame(rows=420, turns=3.25, phase=0.35, scale=0.42)
back_frame = ScatterProbeFrame(rows=420, turns=3.25, phase=2.2, scale=0.42)

constant_scalar_frame = ArrayFrame(
    x=[-1.2, 0.0, 1.2],
    y=[-0.35, 0.45, -0.35],
    z=[0.0, 0.0, 0.0],
    scalar=[5.0, 5.0, 5.0],
)

log_edge_frame = ArrayFrame(
    x=[-1.5, -0.5, 0.5],
    y=[0.0, 0.0, 0.0],
    z=[-0.25, 0.25, -0.25],
    scalar=[-2.0, -1.0, np.nan],
)

t = base_frame.phase
explicit_colors = np.stack(
    (
        0.24 + 0.74 * t,
        0.86 - 0.55 * t,
        0.48 + 0.36 * np.sin(t * np.float32(math.tau)),
    ),
    axis=1,
).astype(np.float32)

app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #101521;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
        overflow-y: auto;
        padding-right: 22px;
        padding-bottom: 32px;
    }

    VLayout.root::scrollbar-track,
    Panel.scroll-case::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    Panel.scroll-case::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    GridLayout.row,
    FlowLayout.controls {
        width: 100%;
        gap: 12px;
        height: auto;
    }

    Panel {
        background: rgba(19, 26, 41, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 12px;
        padding: 14px;
        gap: 10px;
        max-width: 100%;
        box-shadow: 0 12px 28px rgba(0, 0, 0, 0.24);
    }

    Panel.case {
        height: 360px;
        overflow: hidden;
    }

    Panel.controls-panel {
        width: 100%;
        min-width: 0;
        gap: 8px;
        overflow: hidden;
    }

    Panel.column-panel {
        min-width: 0;
    }

    Panel.stack-stage {
        position: relative;
        height: 340px;
        overflow: hidden;
    }

    Panel.scroll-case {
        height: 440px;
        overflow-y: auto;
    }

    Label.title {
        color: white;
        font-size: 21px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.72);
        line-height: 1.14;
    }

    Label.status {
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 10px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 760;
        padding: 8px 10px;
    }

    Label.case-title {
        color: white;
        font-weight: 850;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    Button {
        width: auto;
        min-width: 96px;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 8px;
        color: rgba(246, 249, 255, 0.92);
        padding: 7px 10px;
    }

    Button.primary {
        background: rgba(90, 169, 255, 0.22);
        border-color: rgba(90, 169, 255, 0.55);
        color: white;
        font-weight: 800;
    }

    Scatter3D {
        width: 100%;
        flex-grow: 1;
        min-height: 230px;
        background: rgba(3, 8, 18, 0.54);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 12px;
    }

    Scatter3D.compact-v0 {
        scatter-point-size: 3px;
        scatter-point-style: circle;
        border-color: rgba(90, 169, 255, 0.42);
    }

    Scatter3D.scalar-v1 {
        scatter-point-size: 5px;
        scatter-point-style: circle;
        border-color: rgba(116, 221, 176, 0.44);
    }

    Scatter3D.category-v1 {
        scatter-point-size: 6px;
        scatter-point-style: square;
        border-color: rgba(255, 211, 106, 0.46);
    }

    Scatter3D.explicit-v1 {
        scatter-point-size: 5px;
        scatter-point-style: circle;
        border-color: rgba(255, 101, 132, 0.46);
    }

    Scatter3D.scroll-plot {
        height: 310px;
        flex-grow: 0;
        scatter-point-size: 7px;
        scatter-point-style: circle;
    }

    Scatter3D.stack-back,
    Scatter3D.stack-front {
        position: absolute;
        width: 66%;
        height: 238px;
        min-height: 238px;
        flex-grow: 0;
    }

    Scatter3D.stack-back {
        left: 16px;
        top: 58px;
        z-index: 1;
        scatter-point-size: 7px;
        scatter-point-style: circle;
        border-color: rgba(90, 169, 255, 0.52);
    }

    Scatter3D.stack-front {
        right: 16px;
        top: 88px;
        z-index: 4;
        scatter-point-size: 8px;
        scatter-point-style: square;
        border-color: rgba(255, 101, 132, 0.62);
    }
    """
)

win = dg.Window("CSS Scatter3D Probe", width=1160, height=820)


def format_pick(name: str, point: dg.ScatterPick) -> str:
    return f"{name}: point {point.index} ({point.x:.2f}, {point.y:.2f}, {point.z:.2f})"


with dg.VLayout(class_="root"):
    dg.Label("Scatter3D", class_="title")
    dg.Label(
        "Dedicated probe for the current DragonFrame compact and point-instance scatter paths, live data updates, "
        "camera commands, point styles, picking, z-order, rounded clipping, and scroll clipping.",
        class_="caption",
    )

    status = dg.Label("Pick a point or use the live controls.", class_="status")

    def set_status(message: str) -> None:
        status.set_value(message)

    def pick_logger(name: str):
        def _on_pick(point: dg.ScatterPick) -> None:
            set_status(format_pick(name, point))

        return _on_pick

    with probe_grid(gap=12, min_column_width=340, class_="row"):
        with dg.Panel("Compact xyz_f32_v0", class_="case"):
            dg.Label("z-derived colormap, CSS circle points", class_="case-title")
            compact_scatter = dg.Scatter3D(
                compact_frame,
                x="x",
                y="y",
                z="z",
                colormap="viridis",
                on_pick=pick_logger("compact"),
                class_="compact-v0",
            )
            dg.Label("PASS: compact packet uses z-derived color and rounded clipping.", class_="pass")

        with dg.Panel("Scalar point_instance_v1", class_="case"):
            dg.Label("log-scaled scalar color, normalized sizes, opaque circle points", class_="case-title")
            live_scatter = dg.Scatter3D(
                base_frame,
                x="x",
                y="y",
                z="z",
                scalars="energy",
                point_sizes="size",
                size_range=(2.0, 12.0),
                opacity=1.0,
                clim=(0.1, 1.05),
                log_scale=True,
                nan_color=(1.0, 0.25, 0.55),
                colormap="turbo",
                on_pick=pick_logger("scalar"),
                class_="scalar-v1",
            )
            dg.Label("PASS: sizes and color are baked into point_instance_v1.", class_="pass")

    with dg.Panel("Live scatter controls", class_="controls-panel"):
        dg.Label("Applies to the scalar plot above", class_="case-title")

        def use_base() -> None:
            live_scatter.set_points(
                base_frame,
                x="x",
                y="y",
                z="z",
                scalars="energy",
                point_sizes="size",
                point_size=5.0,
                size_range=(2.0, 12.0),
                opacity=1.0,
                clim=(0.1, 1.05),
                log_scale=True,
                nan_color=(1.0, 0.25, 0.55),
            )
            set_status("Live data: restored base frame")

        def use_alternate() -> None:
            live_scatter.set_points(
                alternate_frame,
                x="x",
                y="y",
                z="z",
                scalars="energy",
                point_sizes="size",
                point_size=5.0,
                size_range=(3.0, 14.0),
                opacity=1.0,
                clim=(0.1, 1.05),
                log_scale=True,
                nan_color=(1.0, 0.25, 0.55),
            )
            set_status("Live data: same shape, different payload")

        def use_empty() -> None:
            live_scatter.set_points(
                empty_frame,
                x="x",
                y="y",
                z="z",
                scalars="energy",
                point_sizes="size",
                size_range=(2.0, 12.0),
                opacity=1.0,
                log_scale=True,
            )
            set_status("Live data: empty payload")

        def use_all_nonfinite() -> None:
            live_scatter.set_points(
                all_nonfinite_frame,
                x="x",
                y="y",
                z="z",
                scalars="energy",
                point_sizes="size",
                size_range=(2.0, 12.0),
                opacity=1.0,
                log_scale=True,
            )
            set_status("Live data: all non-finite positions")

        def set_colormap(name: str) -> None:
            live_scatter.set_colormap(name)
            set_status(f"Colormap: {name}")

        def set_point_style(name: str) -> None:
            live_scatter.set_point_style(name)
            set_status(f"Point style: {name}")

        def set_parallel(enabled: bool) -> None:
            live_scatter.parallel_projection = enabled
            set_status("Parallel projection" if enabled else "Perspective projection")

        with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
            dg.Button("Frame A", class_="primary", on_click=use_base)
            dg.Button("Frame B", on_click=use_alternate)
            dg.Button("Empty", on_click=use_empty)
            dg.Button("All nonfinite", on_click=use_all_nonfinite)
            dg.Button("Fit", on_click=lambda: (live_scatter.fit(), set_status("Camera fit")))
            dg.Button("Reset", on_click=lambda: (live_scatter.reset_camera(), set_status("Camera reset")))
            dg.Button("XY", on_click=lambda: (live_scatter.view_xy(), set_status("View XY")))
            dg.Button("YZ", on_click=lambda: (live_scatter.view_yz(), set_status("View YZ")))
            dg.Button("Iso", on_click=lambda: (live_scatter.view_isometric(), set_status("View isometric")))
            dg.Button("Parallel", on_click=lambda: set_parallel(True))
            dg.Button("Perspective", on_click=lambda: set_parallel(False))
            dg.Button("Magma", on_click=lambda: set_colormap("magma"))
            dg.Button("Turbo", on_click=lambda: set_colormap("turbo"))
            dg.Button("Circle", on_click=lambda: set_point_style("circle"))
            dg.Button("Square", on_click=lambda: set_point_style("square"))
            dg.Button("Gaussian", on_click=lambda: set_point_style("gaussian"))

    with dg.Panel("Scalar normalization edge cases", class_="controls-panel"):
        dg.Label("Collapsed ranges, raw-domain scalar bars, and NaN color handling", class_="case-title")
        with probe_grid(gap=12, min_column_width=340, class_="row"):
            with dg.Panel("Collapsed linear range", class_="case"):
                dg.Label("All scalar values are 5.0; color should use low-end viridis, bar tick is centered", class_="case-title")
                dg.Scatter3D(
                    constant_scalar_frame,
                    x="x",
                    y="y",
                    z="z",
                    scalars="scalar",
                    point_size=15.0,
                    colormap="viridis",
                    scalar_bar=True,
                    scalar_bar_title="constant = 5",
                    grid=True,
                    class_="scalar-v1",
                )
                dg.Label("PASS: collapsed linear colors map to t=0 and still show a scalar tick.", class_="pass")

            with dg.Panel("Log non-positive + NaN", class_="case"):
                dg.Label("Finite values are <= 0; only the NaN point should use nan_color", class_="case-title")
                dg.Scatter3D(
                    log_edge_frame,
                    x="x",
                    y="y",
                    z="z",
                    scalars="scalar",
                    point_size=15.0,
                    colormap="viridis",
                    log_scale=True,
                    nan_color=(1.0, 0.25, 0.55),
                    scalar_bar=True,
                    scalar_bar_title="log scalar",
                    grid=True,
                    class_="scalar-v1",
                )
                dg.Label("PASS: finite non-positive values are clamped, actual NaN is pink.", class_="pass")

    with probe_grid(gap=12, min_column_width=340, class_="row"):
        with dg.Panel("Categorical colors", class_="case"):
            dg.Label("color='group', square points, independent pick callback", class_="case-title")
            category_scatter = dg.Scatter3D(
                base_frame,
                x="x",
                y="y",
                z="z",
                color="group",
                point_size=6.0,
                opacity=1.0,
                on_pick=pick_logger("category"),
                class_="category-v1",
            )
            dg.Label("PASS: three categories use stable palette colors.", class_="pass")

        with dg.Panel("Explicit RGB colors", class_="case"):
            dg.Label("colors=array, independent camera and pick state", class_="case-title")
            explicit_scatter = dg.Scatter3D(
                base_frame,
                x="x",
                y="y",
                z="z",
                colors=explicit_colors,
                point_size=5.0,
                opacity=1.0,
                on_pick=pick_logger("explicit"),
                class_="explicit-v1",
            )
            dg.Label("PASS: explicit colors are not recolored by native colormap.", class_="pass")

    with probe_grid(gap=12, min_column_width=340, class_="row"):
        with dg.Panel("Z-order and hit target", class_="case stack-stage"):
            dg.Label("Overlapping Scatter3D widgets", class_="case-title")
            stack_back = dg.Scatter3D(
                back_frame,
                x="x",
                y="y",
                z="z",
                colormap="cividis",
                on_pick=pick_logger("z back"),
                class_="stack-back",
            )
            stack_front = dg.Scatter3D(
                front_frame,
                x="x",
                y="y",
                z="z",
                color="group",
                on_pick=pick_logger("z front"),
                class_="stack-front",
            )
            dg.Label("PASS: front plot paints and picks above the back plot.", class_="pass")

        with dg.Panel("Scrollable rounded clip", class_="case scroll-case"):
            dg.Label("Parent scroll clips, viewport size stays stable", class_="case-title")
            scroll_scatter = dg.Scatter3D(
                base_frame,
                x="x",
                y="y",
                z="z",
                scalars="energy",
                point_sizes="size",
                size_range=(4.0, 11.0),
                opacity=1.0,
                colormap="plasma",
                on_pick=pick_logger("scroll"),
                class_="scroll-plot",
            )
            dg.Label("Scroll filler row 1", class_="caption")
            dg.Label("Scroll filler row 2", class_="caption")
            dg.Label("Scroll filler row 3", class_="caption")
            dg.Label("PASS: scrolling clips the plot without shrinking/reprojecting it.", class_="pass")


    # ── Phase 1: Grid and Axes ────────────────────────────────────────────────
    with dg.Panel("Grid and Axes", class_="controls-panel"):
        dg.Label("Phase 1 — grid, planes, ticks, labels, axis visibility, background", class_="case-title")

        with probe_grid(gap=12, min_column_width=340, class_="row"):
            with dg.Panel("Grid scatter", class_="case"):
                dg.Label("Starts with grid on, major planes, default XYZ labels", class_="case-title")
                grid_scatter = dg.Scatter3D(
                    base_frame,
                    x="x",
                    y="y",
                    z="z",
                    scalars="energy",
                    colormap="viridis",
                    grid=True,
                    major_planes=True,
                    axis_x="X axis",
                    axis_y="Y axis",
                    axis_z="Z axis",
                    class_="scalar-v1",
                )

            with dg.Panel("Grid controls", class_="controls-panel column-panel"):
                dg.Label("Live grid updates", class_="case-title")

                def toggle_grid(vis: bool) -> None:
                    grid_scatter.show_grid(vis)
                    set_status(f"Grid: {'on' if vis else 'off'}")

                def set_planes(major: bool, minor: bool) -> None:
                    grid_scatter.show_grid_planes(major=major, minor=minor)
                    set_status(f"Planes: major={major} minor={minor}")

                def set_custom_ticks() -> None:
                    grid_scatter.set_ticks(x=4, y=5, z=3)
                    set_status("Ticks: x=4 y=5 z=3")

                def reset_ticks() -> None:
                    grid_scatter.set_ticks()
                    set_status("Ticks: auto")

                def set_custom_labels() -> None:
                    grid_scatter.set_axes("Time", "Voltage", "Depth")
                    set_status("Axes: Time / Voltage / Depth")

                def reset_labels() -> None:
                    grid_scatter.set_axes("X axis", "Y axis", "Z axis")
                    set_status("Axes: reset to default")

                def hide_z_axis() -> None:
                    grid_scatter.set_axis_visibility(x=True, y=True, z=False)
                    set_status("Z axis hidden")

                def show_all_axes() -> None:
                    grid_scatter.set_axis_visibility(x=True, y=True, z=True)
                    set_status("All axes visible")

                def set_dark_bg() -> None:
                    grid_scatter.set_background(0.03, 0.06, 0.14)
                    set_status("Background: deep navy")

                def reset_bg() -> None:
                    grid_scatter.set_background(0.0, 0.0, 0.0)
                    set_status("Background: black")

                with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
                    dg.Button("Grid on", class_="primary", on_click=lambda: toggle_grid(True))
                    dg.Button("Grid off", on_click=lambda: toggle_grid(False))
                    dg.Button("Major planes", on_click=lambda: set_planes(True, False))
                    dg.Button("+ Minor", on_click=lambda: set_planes(True, True))
                    dg.Button("No planes", on_click=lambda: set_planes(False, False))
                    dg.Button("Custom ticks", on_click=set_custom_ticks)
                    dg.Button("Auto ticks", on_click=reset_ticks)
                    dg.Button("Custom labels", on_click=set_custom_labels)
                    dg.Button("Reset labels", on_click=reset_labels)
                    dg.Button("Hide Z", on_click=hide_z_axis)
                    dg.Button("Show all", on_click=show_all_axes)
                    dg.Button("Dark bg", on_click=set_dark_bg)
                    dg.Button("Reset bg", on_click=reset_bg)
                    dg.Button("Fit", on_click=lambda: grid_scatter.fit())
                    dg.Button("Iso", on_click=lambda: grid_scatter.view_isometric())


    # ── Phase 2: Legend, Scalar Bar, Orientation Axes ────────────────────────
    with dg.Panel("Overlays", class_="controls-panel"):
        dg.Label("Phase 2 — legend, scalar bar, orientation axes", class_="case-title")

        with probe_grid(gap=12, min_column_width=340, class_="row"):
            with dg.Panel("Overlay scatter", class_="case"):
                dg.Label("Groups: inner/middle/outer, energy scalar", class_="case-title")
                overlay_scatter = dg.Scatter3D(
                    base_frame,
                    x="x",
                    y="y",
                    z="z",
                    scalars="energy",
                    colormap="plasma",
                    grid=True,
                    orientation_axes=True,
                    class_="scalar-v1",
                )

            with dg.Panel("Overlay controls", class_="controls-panel column-panel"):
                dg.Label("Live overlay updates", class_="case-title")

                def show_legend() -> None:
                    overlay_scatter.show_legend(
                        True,
                        position="top-right",
                        entries=[
                            ("Inner", 0.2, 0.6, 1.0),
                            ("Middle", 0.4, 0.9, 0.4),
                            ("Outer", 1.0, 0.4, 0.2),
                        ],
                    )
                    set_status("Legend: on")

                def hide_legend() -> None:
                    overlay_scatter.show_legend(False)
                    set_status("Legend: off")

                def legend_bottom_left() -> None:
                    overlay_scatter.show_legend(
                        True,
                        position="bottom-left",
                        entries=[("A", 1.0, 0.3, 0.3), ("B", 0.3, 0.7, 1.0)],
                    )
                    set_status("Legend: bottom-left")

                def show_scalar_bar() -> None:
                    overlay_scatter.show_scalar_bar(
                        True, vmin=0.0, vmax=9.0, log_scale=False,
                        colormap="plasma", title="Energy"
                    )
                    set_status("Scalar bar: on")

                def show_scalar_bar_log() -> None:
                    overlay_scatter.show_scalar_bar(
                        True, vmin=1.0, vmax=9.0, log_scale=True,
                        colormap="viridis", title="log Energy"
                    )
                    set_status("Scalar bar: log scale")

                def hide_scalar_bar() -> None:
                    overlay_scatter.show_scalar_bar(False)
                    set_status("Scalar bar: off")

                def show_orient_axes() -> None:
                    overlay_scatter.show_orientation_axes(True)
                    set_status("Orientation axes: on")

                def hide_orient_axes() -> None:
                    overlay_scatter.show_orientation_axes(False)
                    set_status("Orientation axes: off")

                with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
                    dg.Button("Legend on", class_="primary", on_click=show_legend)
                    dg.Button("Legend off", on_click=hide_legend)
                    dg.Button("Legend BL", on_click=legend_bottom_left)
                    dg.Button("Scalar bar", class_="primary", on_click=show_scalar_bar)
                    dg.Button("Log scale", on_click=show_scalar_bar_log)
                    dg.Button("Bar off", on_click=hide_scalar_bar)
                    dg.Button("Orient on", class_="primary", on_click=show_orient_axes)
                    dg.Button("Orient off", on_click=hide_orient_axes)
                    dg.Button("Fit", on_click=lambda: overlay_scatter.fit())
                    dg.Button("Iso", on_click=lambda: overlay_scatter.view_isometric())


    # ── Phase 3: Labels, Lines, Boxes ────────────────────────────────────────
    with dg.Panel("Annotations", class_="controls-panel"):
        dg.Label("Phase 3 — world-space labels, polylines, bounding boxes", class_="case-title")

        with probe_grid(gap=12, min_column_width=340, class_="row"):
            with dg.Panel("Annotation scatter", class_="case"):
                dg.Label("Data with world-space labels and box overlay", class_="case-title")
                annot_scatter = dg.Scatter3D(
                    base_frame,
                    x="x",
                    y="y",
                    z="z",
                    scalars="energy",
                    colormap="viridis",
                    grid=True,
                    class_="scalar-v1",
                )

            with dg.Panel("Annotation controls", class_="controls-panel column-panel"):
                dg.Label("Live annotation updates", class_="case-title")

                _label_handles: list[int] = []
                _overlay_handles: list[int] = []

                def add_origin_label() -> None:
                    h = annot_scatter.add_label(
                        (0.0, 0.0, 0.0), "Origin",
                        color=(1.0, 1.0, 0.0), size=14.0,
                    )
                    _label_handles.append(h)
                    set_status(f"Label {h}: Origin at (0,0,0)")

                def add_tip_label() -> None:
                    h = annot_scatter.add_label(
                        (2.5, 0.0, 1.2), "Tip",
                        color=(0.3, 1.0, 0.5), size=12.0,
                    )
                    _label_handles.append(h)
                    set_status(f"Label {h}: Tip")

                def clear_all_labels() -> None:
                    annot_scatter.clear_labels()
                    _label_handles.clear()
                    set_status("Labels cleared")

                def add_helix_path() -> None:
                    import math as _math
                    pts = [
                        (
                            _math.cos(t * _math.tau * 3) * 2.0,
                            _math.sin(t * _math.tau * 3) * 2.0,
                            (t - 0.5) * 2.4,
                        )
                        for t in [i / 24.0 for i in range(25)]
                    ]
                    h = annot_scatter.add_lines(pts, color=(1.0, 0.5, 0.0))
                    _overlay_handles.append(h)
                    set_status(f"Helix polyline: handle {h}")

                def add_bounding_box() -> None:
                    h = annot_scatter.add_box(
                        (-2.5, 2.5, -2.5, 2.5, -1.3, 1.3),
                        color=(0.4, 0.8, 1.0),
                    )
                    _overlay_handles.append(h)
                    set_status(f"BBox: handle {h}")

                def clear_all_overlays() -> None:
                    annot_scatter.clear_overlays()
                    _overlay_handles.clear()
                    set_status("Overlays cleared")

                def hide_last_overlay() -> None:
                    if _overlay_handles:
                        annot_scatter.set_overlay_visibility(_overlay_handles[-1], False)
                        set_status(f"Hidden overlay {_overlay_handles[-1]}")

                def show_all_overlays() -> None:
                    for h in _overlay_handles:
                        annot_scatter.set_overlay_visibility(h, True)
                    set_status("All overlays visible")

                with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
                    dg.Button("Label origin", class_="primary", on_click=add_origin_label)
                    dg.Button("Label tip", on_click=add_tip_label)
                    dg.Button("Clear labels", on_click=clear_all_labels)
                    dg.Button("Helix path", class_="primary", on_click=add_helix_path)
                    dg.Button("BBox", on_click=add_bounding_box)
                    dg.Button("Hide last", on_click=hide_last_overlay)
                    dg.Button("Show all", on_click=show_all_overlays)
                    dg.Button("Clear overlays", on_click=clear_all_overlays)
                    dg.Button("Fit", on_click=lambda: annot_scatter.fit())
                    dg.Button("Iso", on_click=lambda: annot_scatter.view_isometric())


    # ── Phase 4: Actors and Streaming ────────────────────────────────────────
    with dg.Panel("Actors & Streaming", class_="controls-panel"):
        dg.Label("Phase 4 — multi-actor layers, streaming ring buffer", class_="case-title")

        with probe_grid(gap=12, min_column_width=340, class_="row"):
            with dg.Panel("Actor scatter", class_="case"):
                dg.Label("Base helix; actors added on demand", class_="case-title")
                actor_scatter = dg.Scatter3D(
                    base_frame,
                    x="x",
                    y="y",
                    z="z",
                    scalars="energy",
                    colormap="viridis",
                    grid=True,
                    class_="scalar-v1",
                )

            with dg.Panel("Actor controls", class_="controls-panel column-panel"):
                dg.Label("Live actor management", class_="case-title")

                import math as _math

                _actor_handles: list[int] = []
                _stream_handle: list[int] = []
                _stream_tick: list[int] = [0]

                def make_ring_frame(radius: float, z_off: float) -> object:
                    rows = 300
                    frame = type("F", (), {
                        "columns": ("x", "y", "z"),
                        "shape": (rows, 3),
                    })()
                    try:
                        import numpy as np
                        frame.x = np.cos(np.linspace(0, 2 * _math.pi, rows, dtype=np.float32)) * radius
                        frame.y = np.sin(np.linspace(0, 2 * _math.pi, rows, dtype=np.float32)) * radius
                        frame.z = np.full(rows, z_off, dtype=np.float32)
                    except ImportError:
                        pass
                    return frame

                def add_actor_red() -> None:
                    h = actor_scatter.add_points(
                        make_ring_frame(1.5, -1.0), x="x", y="y", z="z",
                        color=(1.0, 0.2, 0.2), point_size=5.0,
                    )
                    _actor_handles.append(h)
                    set_status(f"Actor {h}: red ring")

                def add_actor_blue() -> None:
                    h = actor_scatter.add_points(
                        make_ring_frame(2.5, 0.5), x="x", y="y", z="z",
                        color=(0.2, 0.5, 1.0), point_size=4.0,
                    )
                    _actor_handles.append(h)
                    set_status(f"Actor {h}: blue ring")

                def remove_last_actor() -> None:
                    if _actor_handles:
                        h = _actor_handles.pop()
                        actor_scatter.remove_actor(h)
                        set_status(f"Removed actor {h}")

                def hide_last_actor() -> None:
                    if _actor_handles:
                        actor_scatter.set_actor_visibility(_actor_handles[-1], False)
                        set_status(f"Hidden actor {_actor_handles[-1]}")

                def show_all_actors() -> None:
                    for h in _actor_handles:
                        actor_scatter.set_actor_visibility(h, True)
                    set_status("All actors visible")

                def clear_all_actors() -> None:
                    actor_scatter.clear()
                    _actor_handles.clear()
                    set_status("Actors cleared")

                def start_stream() -> None:
                    if _stream_handle:
                        set_status("Stream already active")
                        return
                    h = actor_scatter.add_stream(max_points=500, mode="ring")
                    _stream_handle.append(h)
                    set_status(f"Stream {h}: ring/500 started")

                def push_stream_batch() -> None:
                    if not _stream_handle:
                        set_status("No stream — start one first")
                        return
                    tick = _stream_tick[0]
                    _stream_tick[0] += 1
                    try:
                        import numpy as np
                        rows = 20
                        frame = type("F", (), {
                            "columns": ("x", "y", "z"),
                            "shape": (rows, 3),
                        })()
                        t = np.linspace(tick * 0.1, (tick + 1) * 0.1, rows, dtype=np.float32)
                        frame.x = np.cos(t * _math.tau * 4) * (1.5 + t)
                        frame.y = np.sin(t * _math.tau * 4) * (1.5 + t)
                        frame.z = t * 2.0 - 1.0
                        actor_scatter.stream(
                            _stream_handle[0], frame, x="x", y="y", z="z",
                            color=(0.8, 0.9, 0.2),
                        )
                        set_status(f"Stream tick {tick}")
                    except ImportError:
                        set_status("NumPy required for streaming demo")

                def clear_stream() -> None:
                    if _stream_handle:
                        actor_scatter.clear_stream(_stream_handle[0])
                        set_status("Stream cleared")

                with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
                    dg.Button("Add red ring", class_="primary", on_click=add_actor_red)
                    dg.Button("Add blue ring", on_click=add_actor_blue)
                    dg.Button("Remove last", on_click=remove_last_actor)
                    dg.Button("Hide last", on_click=hide_last_actor)
                    dg.Button("Show all", on_click=show_all_actors)
                    dg.Button("Clear actors", on_click=clear_all_actors)
                    dg.Button("Start stream", class_="primary", on_click=start_stream)
                    dg.Button("Push batch", on_click=push_stream_batch)
                    dg.Button("Clear stream", on_click=clear_stream)
                    dg.Button("Fit", on_click=lambda: actor_scatter.fit())
                    dg.Button("Iso", on_click=lambda: actor_scatter.view_isometric())

    # ── Phase 5: Selection, Hover, LOD ───────────────────────────────────────
    with dg.Panel("Selection & LOD", class_="controls-panel"):
        import numpy as _np5
        _rng5 = _np5.random.default_rng(42)
        _sel_pts = {
            "x": _rng5.standard_normal(300_000).tolist(),
            "y": _rng5.standard_normal(300_000).tolist(),
            "z": _rng5.standard_normal(300_000).tolist(),
        }

        class _SelFrame:
            def __getitem__(self, k):
                return _sel_pts[k]

        _lod_lbl = dg.Label("LOD: off", style={"font-size": "12px"})
        _pick_lbl = dg.Label("Last pick: —", style={"font-size": "12px"})

        def _on_sel_event(data) -> None:
            import json as _json
            try:
                d = _json.loads(data)
                if d.get("event") == "select":
                    counts = {k: len(v) for k, v in d.get("actors", {}).items()}
                    _pick_lbl.set_value(f"Selected: {counts}")
                else:
                    _pick_lbl.set_value(f"Picked idx={d.get('index')}")
            except Exception:
                _pick_lbl.set_value(str(data)[:60])

        with probe_grid(gap=12, min_column_width=340, class_="row"):
            with dg.Panel("300k point cloud", class_="case"):
                sel_scatter = dg.Scatter3D(
                    _SelFrame(), x="x", y="y", z="z",
                    on_pick=_on_sel_event,
                    class_="scalar-v1",
                )

            with dg.Panel("Controls", class_="controls-panel column-panel"):
                def enable_point() -> None:
                    sel_scatter.enable_point_picking()
                    _pick_lbl.set_value("Mode: point pick")

                def enable_rect() -> None:
                    sel_scatter.enable_rectangle_picking(on_select=_on_sel_event)
                    _pick_lbl.set_value("Mode: rect (drag to select)")

                def disable_pick() -> None:
                    sel_scatter.disable_picking()
                    _pick_lbl.set_value("Mode: none")

                def lod_on() -> None:
                    sel_scatter.set_lod(enabled=True, threshold=50_000, factor=8)
                    _lod_lbl.set_value("LOD: on (threshold=50k, factor=8)")

                def lod_off() -> None:
                    sel_scatter.set_lod(enabled=False)
                    _lod_lbl.set_value("LOD: off")

                with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
                    dg.Button("Point pick", class_="primary", on_click=enable_point)
                    dg.Button("Rect select", on_click=enable_rect)
                    dg.Button("Disable pick", on_click=disable_pick)
                    dg.Button("LOD on", on_click=lod_on)
                    dg.Button("LOD off", on_click=lod_off)
                    dg.Button("Fit", on_click=lambda: sel_scatter.fit())
                _lod_lbl
                _pick_lbl

    # ── Phase 6: Mesh and Statistical Overlays ────────────────────────────────
    with dg.Panel("Mesh Overlays", class_="controls-panel"):
        import numpy as _np6
        _rng6 = _np6.random.default_rng(7)
        _cluster_pts = _np6.concatenate([
            _rng6.multivariate_normal([0, 0, 0], _np6.eye(3) * 0.5, 150),
            _rng6.multivariate_normal([3, 0, 0], _np6.eye(3) * 0.4, 150),
            _rng6.multivariate_normal([0, 3, 0], _np6.eye(3) * 0.6, 150),
        ]).astype(_np6.float32)
        _cluster_lbls = _np6.array([0]*150 + [1]*150 + [2]*150)

        class _ClusterFrame:
            def __getitem__(self, k):
                return _cluster_pts[:, {"x": 0, "y": 1, "z": 2}[k]].tolist()

        _mesh_handles: list[int] = []

        with probe_grid(gap=12, min_column_width=340, class_="row"):
            with dg.Panel("Cluster cloud", class_="case"):
                mesh_scatter = dg.Scatter3D(
                    _ClusterFrame(), x="x", y="y", z="z",
                    scalars=(_cluster_lbls / 2).tolist(),
                    colormap="viridis",
                    class_="scalar-v1",
                )

            with dg.Panel("Mesh controls", class_="controls-panel column-panel"):
                def add_hulls() -> None:
                    try:
                        hs = mesh_scatter.add_cluster_hulls(
                            _cluster_pts, _cluster_lbls, opacity=0.25
                        )
                        _mesh_handles.extend(hs)
                    except ImportError as exc:
                        print(f"scipy not installed: {exc}")

                def add_ellipsoids() -> None:
                    hs = mesh_scatter.add_cluster_ellipsoids(
                        _cluster_pts, _cluster_lbls, opacity=0.2, n_std=2.0
                    )
                    _mesh_handles.extend(hs)

                def remove_last_mesh() -> None:
                    if _mesh_handles:
                        mesh_scatter.remove_mesh(_mesh_handles.pop())

                def clear_meshes() -> None:
                    mesh_scatter.clear_meshes()
                    _mesh_handles.clear()

                with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
                    dg.Button("Add hulls", class_="primary", on_click=add_hulls)
                    dg.Button("Add ellipsoids", on_click=add_ellipsoids)
                    dg.Button("Remove last", on_click=remove_last_mesh)
                    dg.Button("Clear meshes", on_click=clear_meshes)
                    dg.Button("Fit", on_click=lambda: mesh_scatter.fit())
                    dg.Button("Iso", on_click=lambda: mesh_scatter.view_isometric())


    # ── Phase 7: Export and Camera Linking ───────────────────────────────────
    with dg.Panel("Export & Camera", class_="controls-panel"):
        import numpy as _np7
        _rng7 = _np7.random.default_rng(42)
        _pts7 = _rng7.standard_normal((2000, 3)).astype(_np7.float32)

        class _Frame7:
            def __getitem__(self, k):
                return _pts7[:, {"x": 0, "y": 1, "z": 2}[k]].tolist()

        _export_status = dg.Label("Ready", class_="status-label")

        with probe_grid(gap=12, min_column_width=340, class_="row"):
            with dg.Panel("Left view", class_="case"):
                sc7a = dg.Scatter3D(_Frame7(), x="x", y="y", z="z", id="sc7a")

            with dg.Panel("Right view (linked)", class_="case"):
                sc7b = dg.Scatter3D(_Frame7(), x="x", y="y", z="z", id="sc7b")

        sc7a.link_cameras(sc7b)

        with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
            dg.Button("Flatten XY", on_click=lambda: (sc7a.flatten_view("xy"), sc7b.flatten_view("xy")))
            dg.Button("Flatten XZ", on_click=lambda: (sc7a.flatten_view("xz"), sc7b.flatten_view("xz")))
            dg.Button("Iso", on_click=lambda: (sc7a.view_isometric(), sc7b.view_isometric()))

        with dg.FlowLayout(class_="controls", gap=12, row_gap=8):
            def _do_screenshot() -> None:
                import os, pathlib
                try:
                    arr = sc7a.screenshot()
                    if arr is None:
                        _export_status.set_value("screenshot: None (not live?)")
                        return
                    out_path = pathlib.Path.home() / "dragongui_screenshot.png"
                    sc7a.save_png(str(out_path))
                    _export_status.set_value(f"saved {arr.shape[1]}×{arr.shape[0]} → {out_path}")
                except Exception as exc:
                    _export_status.set_value(f"error: {exc}")

            def _show_bounds() -> None:
                bounds = sc7a.get_view_bounds_2d()
                _export_status.set_value(f"2D bounds: {bounds}")

            dg.Button("Screenshot", class_="primary", on_click=_do_screenshot)
            dg.Button("View bounds", on_click=_show_bounds)
            dg.Button("Fit both", on_click=lambda: (sc7a.fit(), sc7b.fit()))

        _export_status


if __name__ == "__main__":
    print(app.run(win))
