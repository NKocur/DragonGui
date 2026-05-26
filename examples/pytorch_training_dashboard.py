from __future__ import annotations

import math
import random
import sys
import threading
import time
from pathlib import Path
from typing import Any

if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except Exception:  # pragma: no cover - example fallback
    np = None

try:
    import torch
except Exception:  # pragma: no cover - PyTorch is optional for this demo
    torch = None


class SimpleFrame:
    def __init__(self, rows: list[dict[str, Any]], columns: tuple[str, ...] | None = None):
        self.rows = rows
        self.columns = columns or tuple(rows[0].keys() if rows else ("name", "value"))
        self.shape = (len(rows), len(self.columns))
        self.dtypes = tuple("str" for _ in self.columns)

    def __getitem__(self, name: str) -> list[Any]:
        return [row.get(name, "") for row in self.rows]

    def __getattr__(self, name: str) -> list[Any]:
        if name in self.columns:
            return self[name]
        raise AttributeError(name)


class MetricFrame:
    columns = ("step", "loss", "accuracy")
    dtypes = ("float32", "float32", "float32")

    def __init__(self, steps: list[float], losses: list[float], accuracies: list[float]):
        if np is not None:
            self.step = np.asarray(steps, dtype=np.float32)
            self.loss = np.asarray(losses, dtype=np.float32)
            self.accuracy = np.asarray(accuracies, dtype=np.float32)
        else:
            self.step = steps
            self.loss = losses
            self.accuracy = accuracies
        self.shape = (len(steps), 3)

    def __getitem__(self, name: str):
        return getattr(self, name)


def torch_status_rows() -> list[dict[str, str]]:
    if torch is None:
        return [
            {"property": "torch", "value": "not installed", "status": "mock mode"},
            {"property": "device", "value": "cpu", "status": "available"},
            {"property": "cuda", "value": "not available", "status": "mock mode"},
            {"property": "backend", "value": "eager", "status": "simulated"},
        ]

    cuda_available = bool(torch.cuda.is_available())
    device_name = "cpu"
    memory = "-"
    if cuda_available:
        device_name = torch.cuda.get_device_name(0)
        props = torch.cuda.get_device_properties(0)
        memory = f"{props.total_memory / 1024**3:.1f} GB"

    return [
        {"property": "torch", "value": torch.__version__, "status": "loaded"},
        {"property": "device", "value": device_name, "status": "ready"},
        {"property": "cuda", "value": str(cuda_available), "status": "available" if cuda_available else "cpu fallback"},
        {"property": "memory", "value": memory, "status": "reported"},
    ]


def layer_rows() -> list[dict[str, str]]:
    if torch is not None:
        model = torch.nn.Sequential(
            torch.nn.Flatten(),
            torch.nn.Linear(784, 256),
            torch.nn.ReLU(),
            torch.nn.Dropout(0.2),
            torch.nn.Linear(256, 10),
        )
        rows: list[dict[str, str]] = []
        for index, (name, module) in enumerate(model.named_children()):
            params = sum(parameter.numel() for parameter in module.parameters())
            rows.append(
                {
                    "idx": str(index),
                    "module": name or module.__class__.__name__,
                    "type": module.__class__.__name__,
                    "params": f"{params:,}",
                    "trainable": "yes" if params else "-",
                }
            )
        return rows

    return [
        {"idx": "0", "module": "features.0", "type": "Conv2d", "params": "448", "trainable": "yes"},
        {"idx": "1", "module": "features.1", "type": "BatchNorm2d", "params": "32", "trainable": "yes"},
        {"idx": "2", "module": "features.2", "type": "ReLU", "params": "0", "trainable": "-"},
        {"idx": "3", "module": "head.0", "type": "Linear", "params": "200,960", "trainable": "yes"},
        {"idx": "4", "module": "head.1", "type": "Linear", "params": "2,570", "trainable": "yes"},
    ]


def make_metric_frame() -> MetricFrame:
    steps = [0.0]
    losses = [2.35]
    accuracies = [0.08]
    return MetricFrame(steps, losses, accuracies)


def main() -> None:
    app = dg.App()
    app.stylesheet(
        """
        Window {
            background: #050509;
            color: #ffdca8;
            font-size: 13px;
        }

        Panel {
            background: #08080f;
            border: 2px solid #ff9f1c;
            border-radius: 26px;
            padding: 14px;
            gap: 10px;
        }

        Panel.header {
            background: #050509;
            border-color: #ff9f1c;
            border-radius: 32px;
            padding: 10px 14px;
            min-height: 66px;
        }

        Panel.metric-card {
            background: #0b0b14;
            border-color: #c88cff;
            min-height: 98px;
            padding: 16px;
        }

        Label.title {
            font-size: 21px;
            font-weight: 800;
            color: #ffcc66;
            height: 28px;
        }

        Label.section-title {
            font-size: 14px;
            font-weight: 800;
            color: #ff9f1c;
            height: 20px;
        }

        Label.metric-value {
            font-size: 26px;
            font-weight: 800;
            color: #ffcc66;
            height: 34px;
        }

        Label.metric-caption {
            color: #c88cff;
            font-weight: 700;
            height: 18px;
        }

        Label.muted {
            color: #99ccff;
        }

        Label.lcars-tab-wide {
            background: #ff9f1c;
            color: #050509;
            border-radius: 999px;
            font-weight: 800;
            height: 28px;
            width: 132px;
            text-align: center;
        }

        Label.lcars-tab {
            background: #c88cff;
            color: #050509;
            border-radius: 999px;
            font-weight: 800;
            height: 28px;
            width: 76px;
            text-align: center;
        }

        Label.lcars-tab-blue {
            background: #99ccff;
            color: #050509;
            border-radius: 999px;
            font-weight: 800;
            height: 28px;
            width: 86px;
            text-align: center;
        }

        Button {
            background: #ff9f1c;
            color: #050509;
            border: 0px solid #ff9f1c;
            border-radius: 999px;
            padding: 8px 14px;
            min-height: 36px;
            text-align: center;
            font-weight: 800;
        }

        Button:hover {
            background: #ffcc66;
            color: #050509;
        }

        Button.primary {
            background: #ffcc66;
            color: #050509;
        }

        Button.warning {
            background: #c88cff;
            color: #050509;
        }

        Button.danger {
            background: #ff6f91;
            color: #050509;
        }

        Badge {
            background: #ff9f1c;
            color: #050509;
            border: 0px solid #ff9f1c;
            border-radius: 999px;
            padding: 4px 11px;
            min-height: 24px;
            font-weight: 800;
        }

        Badge.good {
            background: #99ccff;
            color: #050509;
        }

        Badge.warn {
            background: #c88cff;
            color: #050509;
        }

        TextInput, NumberInput, Dropdown {
            background: #050509;
            color: #ffcc66;
            border: 2px solid #c88cff;
            border-radius: 999px;
            padding: 7px 12px;
            min-height: 34px;
        }

        TextInput:focus, NumberInput:focus, Dropdown:focus {
            border-color: #ffcc66;
        }

        Checkbox {
            color: #ffdca8;
            accent: #ff9f1c;
        }

        ProgressBar {
            background: #050509;
            border: 2px solid #ff9f1c;
            border-radius: 999px;
            color: #050509;
            min-height: 20px;
        }

        ProgressBar::track {
            background: #161621;
            border-radius: 999px;
        }

        ProgressBar::fill {
            background: #ff9f1c;
            border-radius: 999px;
        }

        ProgressBar::label {
            color: #050509;
            font-weight: 800;
        }

        DataFrameTable {
            background: #050509;
            border: 2px solid #c88cff;
            border-radius: 18px;
            color: #ffdca8;
        }

        DataFrameTable::header {
            background: #ff9f1c;
            color: #050509;
            font-weight: 800;
        }

        DataFrameTable::row {
            color: #ffdca8;
        }

        DataFrameTable::row-selected {
            background: #c88cff;
            color: #050509;
        }

        DataFrameTable::grid-line {
            background: #3d2a4f;
        }

        LogView {
            background: #050509;
            border: 2px solid #99ccff;
            border-radius: 18px;
            color: #ffcc66;
        }

        LinePlot {
            background: #050509;
            border: 2px solid #99ccff;
            border-radius: 18px;
            padding: 8px;
        }

        HLayout.toolbar {
            gap: 8px;
            align-items: center;
        }

        HLayout.metrics {
            gap: 10px;
        }

        """
    )

    state = {
        "running": False,
        "step": 0,
        "epoch": 0,
        "max_steps": 240,
        "loss": 2.35,
        "accuracy": 0.08,
        "throughput": 0.0,
    }

    stop_event = threading.Event()
    worker_thread: threading.Thread | None = None

    win = dg.Window(
        "PyTorch Training Interface",
        width=1280,
        height=820,
        style={"overflow_y": "auto", "overflow_x": "hidden"},
    )

    with dg.VLayout(style={"gap": 12, "padding": 12}):
        with dg.Panel(class_="header"):
            with dg.HLayout(class_="toolbar"):
                dg.Label("LCARS 741", class_="lcars-tab-wide")
                dg.Label("OPS", class_="lcars-tab")
                dg.Label("TRAIN", class_="lcars-tab-blue")
                dg.Label("PyTorch Training Interface", class_="title")
                dg.Spacer()
                runtime_badge = dg.Badge("torch loaded" if torch is not None else "mock mode", level="success" if torch else "warning")
                device_value = torch_status_rows()[1]["value"]
                dg.Badge(device_value, level="success" if torch is not None else "warning")

        with dg.HLayout(style={"gap": 12, "flex_grow": 1}):
            with dg.Panel(width=300, style={"gap": 11}):
                dg.Label("Run Setup", class_="section-title")
                run_name = dg.TextInput("resnet-experiment-001", placeholder="Run name")
                model_select = dg.Dropdown(
                    ["MLP classifier", "ResNet18", "Transformer encoder", "UNet segmenter"],
                    value="MLP classifier",
                )
                dataset_select = dg.Dropdown(["MNIST", "CIFAR-10", "ImageNet subset", "Synthetic batches"], value="MNIST")
                precision_select = dg.Dropdown(["fp32", "amp fp16", "bf16"], value="amp fp16")

                dg.Label("Hyperparameters", class_="section-title")
                lr_input = dg.NumberInput(0.001, min=0.00001, max=1.0, step=0.0001)
                batch_input = dg.NumberInput(128, min=1, max=4096, step=1)
                accum_input = dg.NumberInput(1, min=1, max=32, step=1)
                amp_toggle = dg.Checkbox("Enable AMP", checked=True)

                with dg.HLayout(class_="toolbar"):
                    start_button = dg.Button("Start", class_="primary")
                    pause_button = dg.Button("Pause", class_="warning")
                    reset_button = dg.Button("Reset", class_="danger")

                dg.Label("Device", class_="section-title")
                device_table = dg.DataFrameTable(
                    SimpleFrame(torch_status_rows(), ("property", "value", "status")),
                    page_size=4,
                    sample_rows=4,
                    style={"height": 178},
                )

            with dg.VLayout(style={"gap": 12, "flex_grow": 1}):
                with dg.HLayout(class_="metrics"):
                    with dg.Panel(class_="metric-card", style={"flex_grow": 1}):
                        dg.Label("Loss", class_="metric-caption")
                        loss_label = dg.Label("2.350", class_="metric-value")
                    with dg.Panel(class_="metric-card", style={"flex_grow": 1}):
                        dg.Label("Accuracy", class_="metric-caption")
                        accuracy_label = dg.Label("8.0%", class_="metric-value")
                    with dg.Panel(class_="metric-card", style={"flex_grow": 1}):
                        dg.Label("Throughput", class_="metric-caption")
                        throughput_label = dg.Label("0 samples/s", class_="metric-value")

                with dg.Panel(style={"flex_grow": 1, "gap": 8}):
                    with dg.HLayout(class_="toolbar"):
                        dg.Label("Metrics Stream", class_="section-title")
                        dg.Spacer()
                        status_badge = dg.Badge("idle")
                    if np is not None:
                        metrics_plot = dg.LinePlot(
                            make_metric_frame(),
                            x="step",
                            y=["loss", "accuracy"],
                            labels=["loss", "accuracy"],
                            colors=["#ff9f1c", "#99ccff"],
                            show_legend=True,
                            show_toolbar=True,
                            line_width=2.2,
                            max_points=360,
                            style={"height": 300},
                        )
                    else:
                        metrics_plot = None
                        dg.Label("NumPy is not installed, so the live metric plot is disabled.", class_="muted")

                with dg.HLayout(style={"gap": 12, "flex_grow": 1}):
                    with dg.Panel(style={"flex_grow": 1, "gap": 8}):
                        dg.Label("Training Log", class_="section-title")
                        log_view = dg.LogView(
                            [
                                "ready: configure model and start a training run",
                                f"runtime: {'torch ' + torch.__version__ if torch is not None else 'mock PyTorch session'}",
                            ],
                            rows=11,
                            follow=True,
                            max_lines=300,
                        )
                    with dg.Panel(width=430, style={"gap": 8}):
                        dg.Label("Model Structure", class_="section-title")
                        layer_table = dg.DataFrameTable(
                            SimpleFrame(layer_rows(), ("idx", "module", "type", "params", "trainable")),
                            page_size=8,
                            sample_rows=8,
                            style={"height": 248},
                        )

            with dg.Panel(width=280, style={"gap": 11}):
                dg.Label("Progress", class_="section-title")
                epoch_progress = dg.ProgressBar(0.0, label="epoch", show_value=True)
                run_progress = dg.ProgressBar(0.0, label="run", show_value=True)

                dg.Label("Run Queue", class_="section-title")
                queue_table = dg.DataFrameTable(
                    SimpleFrame(
                        [
                            {"run": "baseline", "model": "MLP", "state": "done"},
                            {"run": "augmented", "model": "ResNet18", "state": "queued"},
                            {"run": "lr-sweep", "model": "Transformer", "state": "queued"},
                            {"run": "cpu-check", "model": "UNet", "state": "paused"},
                        ],
                        ("run", "model", "state"),
                    ),
                    page_size=5,
                    sample_rows=5,
                    style={"height": 190},
                )

                dg.Label("Selected Run", class_="section-title")
                selected_run_label = dg.Label("resnet-experiment-001", class_="muted")
                dg.Label("Optimizer: AdamW")
                dg.Label("Scheduler: cosine warmup")
                dg.Label("Loss: cross entropy")

    def update_labels() -> None:
        loss_label.set_value(f"{state['loss']:.3f}")
        accuracy_label.set_value(f"{state['accuracy'] * 100:.1f}%")
        throughput_label.set_value(f"{state['throughput']:.0f} samples/s")
        epoch_progress.set_value((state["step"] % 60) / 60.0)
        run_progress.set_value(min(1.0, state["step"] / state["max_steps"]))
        selected_run_label.set_value(run_name.value)
        if state["running"]:
            status_badge.set_value("training")
            status_badge.set_level("success")
            runtime_badge.set_value("torch active" if torch is not None else "mock active")
        else:
            status_badge.set_value("idle")
            status_badge.set_level("neutral")
            runtime_badge.set_value("torch loaded" if torch is not None else "mock mode")

    def append_metric(step: int, loss: float, accuracy: float) -> None:
        if metrics_plot is None or np is None:
            return
        xs = np.asarray([step], dtype=np.float32)
        metrics_plot.append_points(xs, np.asarray([loss], dtype=np.float32), series="loss", max_points=360)
        metrics_plot.append_points(xs, np.asarray([accuracy], dtype=np.float32), series="accuracy", max_points=360)

    def training_tick() -> None:
        if not state["running"]:
            return
        state["step"] += 1
        step = state["step"]
        state["epoch"] = step // 60
        decay = math.exp(-step / 115.0)
        noise = random.uniform(-0.018, 0.018)
        state["loss"] = max(0.12, 2.35 * decay + 0.15 + noise)
        state["accuracy"] = min(0.985, 0.08 + 0.88 * (1.0 - decay) + random.uniform(-0.006, 0.006))
        state["throughput"] = float(batch_input.value) * random.uniform(8.5, 11.5)
        append_metric(step, state["loss"], state["accuracy"])
        if step % 10 == 0:
            log_view.append_line(
                f"step={step:04d} epoch={state['epoch']:02d} loss={state['loss']:.4f} "
                f"acc={state['accuracy'] * 100:.2f}% lr={float(lr_input.value):.5f}"
            )
        if step >= state["max_steps"]:
            state["running"] = False
            stop_event.set()
            log_view.append_line("run complete: final checkpoint recorded")
        update_labels()

    def worker() -> None:
        while not stop_event.wait(0.08):
            try:
                app.call_soon_threadsafe(training_tick)
            except RuntimeError:
                break

    def start_run() -> None:
        nonlocal worker_thread
        if state["running"]:
            return
        state["running"] = True
        stop_event.clear()
        log_view.append_line(
            f"start: {run_name.value} model={model_select.value} dataset={dataset_select.value} "
            f"precision={precision_select.value} amp={amp_toggle.checked}"
        )
        if worker_thread is None or not worker_thread.is_alive():
            worker_thread = threading.Thread(target=worker, daemon=True)
            worker_thread.start()
        update_labels()

    def pause_run() -> None:
        state["running"] = False
        stop_event.set()
        log_view.append_line("pause: training loop stopped")
        update_labels()

    def reset_run() -> None:
        state.update({"running": False, "step": 0, "epoch": 0, "loss": 2.35, "accuracy": 0.08, "throughput": 0.0})
        stop_event.set()
        if metrics_plot is not None and np is not None:
            metrics_plot.clear()
            append_metric(0, state["loss"], state["accuracy"])
        log_view.clear()
        log_view.append_line("reset: metrics, progress, and log cleared")
        update_labels()

    start_button.on_click = start_run
    pause_button.on_click = pause_run
    reset_button.on_click = reset_run
    update_labels()

    try:
        print(app.run(win))
    finally:
        state["running"] = False
        stop_event.set()


if __name__ == "__main__":
    main()
