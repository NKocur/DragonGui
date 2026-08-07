"""Run XY's upstream correct-and-stable browser load/gesture benchmark.

The XY checkout is used only for its benchmark driver.  The chart package is
resolved from the active Python environment, so this measures the installed
wheel rather than checkout source.
"""

from __future__ import annotations

import argparse
import inspect
import json
import subprocess
import sys
import textwrap
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--xy-source", type=Path, required=True)
    parser.add_argument("--sizes", default="10000,100000,1000000,2500000,4000000,5000000")
    parser.add_argument("--arms", default="xy,xy-exact")
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--memory-gib", type=float, default=12)
    parser.add_argument("--chrome", required=True)
    parser.add_argument("--software", action="store_true")
    parser.add_argument(
        "--gestures",
        action="store_true",
        help="Enable upstream fixed-cadence wheel zoom and correct/stable settle phase.",
    )
    parser.add_argument(
        "--gesture-inputs",
        type=int,
        default=42,
        help="Fixed-schedule upstream zoom input count when --gestures is enabled.",
    )
    parser.add_argument(
        "--trusted-wheel",
        action="store_true",
        help="Deliver the gesture phase through CDP Input.dispatchMouseEvent.",
    )
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.trusted_wheel and not args.software:
        parser.error(
            "--trusted-wheel requires --software because XY 0.0.5 retains stale "
            "canvas geometry after this Windows hardware-path context recovery"
        )

    benchmark_dir = args.xy_source.resolve() / "benchmarks"
    if not (benchmark_dir / "bench_ux.py").exists():
        parser.error(f"XY benchmark checkout not found at {benchmark_dir}")
    sys.path.insert(0, str(benchmark_dir))
    import _ux_probe
    import bench_ux

    # Tornado can inherit a Windows MIME registry entry that serves `.js` as
    # text/plain. Chrome then rejects XY's module before `window.__arm` exists.
    # Route only XY's live host through a local header shim; benchmark code and
    # the installed XY package remain untouched.
    upstream_popen = subprocess.Popen
    compat_host = Path(__file__).with_name("_xy_live_host_compat.py").resolve()

    def popen_with_module_mime(command: object, *positional: object, **kwargs: object):
        if (
            isinstance(command, list)
            and len(command) > 1
            and Path(str(command[1])).name == "_ux_live_host.py"
        ):
            command = [command[0], str(compat_host), command[1], *command[2:]]
        return upstream_popen(command, *positional, **kwargs)

    bench_ux.subprocess.Popen = popen_with_module_mime

    upstream_engine_js = _ux_probe.engine_js

    def engine_js_with_startup_diagnostics(meta: dict[str, object]) -> str:
        """Retain browser startup errors when the upstream arm never readies."""
        source = upstream_engine_js(meta)
        diagnostics = r"""
window.__uxStartupErrors = [];
window.addEventListener("error", (event) => {
  window.__uxStartupErrors.push(String(event.error || event.message || "window error"));
});
window.addEventListener("unhandledrejection", (event) => {
  window.__uxStartupErrors.push(String(event.reason || "unhandled rejection"));
});
"""
        source = diagnostics + source.replace(
            'return fail("arm_never_ready");',
            'return fail("arm_never_ready", {'
            'errors: window.__uxStartupErrors.slice(-3), '
            'arm: !!window.__arm, view: !!window.__view, '
            'wireReady: !!window.__wireReady});',
        )
        source = source.replace(
            "JSON.stringify({ why, ...extra }).slice(0, 700)",
            "JSON.stringify({ why, ...extra }).slice(0, 4000)",
        )
        if not args.trusted_wheel:
            return source

        schedule_marker = "    const tFirstInput = performance.now();"
        loop_end_marker = "    rafOn = false;"
        settle_marker = '    stage("settle");'
        if source.count(schedule_marker) != 1 or source.count(loop_end_marker) != 1:
            raise RuntimeError("XY probe source no longer matches trusted-wheel adapter")
        loop_start = source.index(schedule_marker)
        loop_end = source.index(loop_end_marker, loop_start)
        trusted_loop = r"""    let tFirstInput = performance.now();
    const canvas = window.__view.canvas;
    window.__uxWheelDiag = {
      received: 0, trusted: 0, prevented: 0, cancelable: 0,
      queued: 0, setView: 0,
    };
    const originalQueueWheelZoom = window.__view._queueWheelZoom.bind(window.__view);
    window.__view._queueWheelZoom = (...values) => {
      window.__uxWheelDiag.queued++;
      window.__uxWheelDiag.lastQueueArgs = values;
      return originalQueueWheelZoom(...values);
    };
    const originalSetView = window.__view._setView.bind(window.__view);
    window.__view._setView = (...values) => {
      window.__uxWheelDiag.setView++;
      return originalSetView(...values);
    };
    window.__uxInteractionDiag = {
      dragMode: window.__view.dragMode,
      dragModeNoneRequested: window.__view._dragModeNoneRequested(),
      navigation: window.__view._interactionFlag("navigation", true),
      zoom: window.__view._interactionFlag("zoom", true),
      wheelZoom: window.__view._interactionFlag("wheel_zoom", true),
      interaction: window.__view.interaction,
      wheelHandlers: window.__view._listeners
        .filter((record) => record.type === "wheel" && record.target === canvas)
        .map((record) => String(record.handler).slice(0, 1200)),
    };
    canvas.addEventListener("wheel", (event) => {
      window.__uxWheelDiag.received++;
      if (event.isTrusted) window.__uxWheelDiag.trusted++;
      if (event.cancelable) window.__uxWheelDiag.cancelable++;
      window.__uxWheelDiag.lastEvent = {
        clientX: event.clientX,
        clientY: event.clientY,
        offsetX: event.offsetX,
        offsetY: event.offsetY,
        deltaX: event.deltaX,
        deltaY: event.deltaY,
        rect: (() => {
          const r = canvas.getBoundingClientRect();
          return {left: r.left, top: r.top, width: r.width, height: r.height};
        })(),
      };
      queueMicrotask(() => {
        if (event.defaultPrevented) window.__uxWheelDiag.prevented++;
      });
    }, {capture: true});
    const request = {
      targetX: cx,
      targetY: cy,
      inputs: M.gesture_inputs,
      cadenceMs: M.gesture_cadence_ms,
    };
    window.__uxTrustedWheelRequest = request;
    document.title = "UX_WHEEL_BATCH " + JSON.stringify(request);
    while (!window.__uxTrustedWheelAck || !window.__uxTrustedWheelAck.complete)
      await new Promise((resolve) => setTimeout(resolve, 1));
    tFirstInput = window.__uxTrustedWheelAck.startedAt;
    const tLastInput = window.__uxTrustedWheelAck.deliveredAt;
"""
        source = source[:loop_start] + trusted_loop + source[loop_end:]
        source = source.replace(
            settle_marker,
            "    result.trusted_wheel_max_schedule_lag_ms = "
            "window.__uxTrustedWheelAck.maxScheduleLagMs;\n"
            "    result.trusted_wheel_mean_schedule_lag_ms = "
            "window.__uxTrustedWheelAck.meanScheduleLagMs;\n"
            "    result.trusted_wheel_event_diag = window.__uxWheelDiag;\n"
            "    result.trusted_wheel_interaction_diag = window.__uxInteractionDiag;\n"
            "    result.trusted_wheel_hit_target = window.__uxTrustedWheelAck.hitTarget;\n"
            '    result.input_delivery = "cdp_trusted_mouse_wheel";\n'
            + settle_marker,
        )
        return source

    _ux_probe.engine_js = engine_js_with_startup_diagnostics

    if args.trusted_wheel:
        driver_source = textwrap.dedent(inspect.getsource(bench_ux.drive_browser))
        title_marker = '                if title.startswith("UX_BENCH "):'
        if driver_source.count(title_marker) != 1:
            raise RuntimeError("XY browser driver no longer matches trusted-wheel adapter")
        trusted_handler = '''                if title.startswith("UX_WHEEL_BATCH "):
                    request = json.loads(title[len("UX_WHEEL_BATCH ") :])
                    target_x = float(request["targetX"])
                    target_y = float(request["targetY"])
                    inputs = int(request["inputs"])
                    cadence_s = float(request["cadenceMs"]) / 1000.0
                    started_at = evaluate(
                        "window.__uxTrustedBatchStartedAt = performance.now()"
                    )
                    schedule_start = time.monotonic()
                    schedule_lags = []
                    x = 0.0
                    y = 0.0
                    for index in range(inputs):
                        target = schedule_start + index * cadence_s
                        wait_s = target - time.monotonic()
                        if wait_s > 0:
                            time.sleep(wait_s)
                        coords = evaluate(
                            "(() => { const view = window.__view; const d = view.view; "
                            "let fx = ("
                            + str(target_x)
                            + " - d.x0) / (d.x1 - d.x0); let fy = ("
                            + str(target_y)
                            + " - d.y0) / (d.y1 - d.y0); "
                            "fx = Math.min(0.98, Math.max(0.02, fx)); "
                            "fy = 1 - Math.min(0.98, Math.max(0.02, fy)); "
                            "const r = view.canvas.getBoundingClientRect(); "
                            "return {x: r.left + r.width * fx, "
                            "y: r.top + r.height * fy}; })()"
                        )
                        x = float(coords["x"])
                        y = float(coords["y"])
                        submitted = time.monotonic()
                        command(
                            "Input.dispatchMouseEvent",
                            {
                                "type": "mouseWheel",
                                "x": x,
                                "y": y,
                                "deltaX": 0,
                                "deltaY": -120,
                            },
                        )
                        schedule_lags.append((submitted - target) * 1000.0)
                    hit_target = evaluate(
                        "(() => { const e = document.elementFromPoint("
                        + str(x)
                        + ", "
                        + str(y)
                        + "); return e ? e.tagName + '.' + e.className : 'none'; })()"
                    )
                    max_lag = max(schedule_lags) if schedule_lags else 0.0
                    mean_lag = sum(schedule_lags) / len(schedule_lags) if schedule_lags else 0.0
                    evaluate(
                        "window.__uxTrustedWheelAck = {complete: true, startedAt: "
                        + str(float(started_at))
                        + ", deliveredAt: performance.now(), maxScheduleLagMs: "
                        + str(max_lag)
                        + ", meanScheduleLagMs: "
                        + str(mean_lag)
                        + ", hitTarget: "
                        + json.dumps(str(hit_target))
                        + "}"
                    )
                    continue
'''
        driver_source = driver_source.replace(title_marker, trusted_handler + title_marker)
        namespace: dict[str, object] = {}
        exec(
            compile(driver_source, str(Path(bench_ux.__file__).resolve()), "exec"),
            bench_ux.__dict__,
            namespace,
        )
        bench_ux.drive_browser = namespace["drive_browser"]

    # Historical load-only reproduction remains the default. DragonGUI now has
    # a real OS-input gate, so --gestures can retain upstream's fixed-cadence
    # wheel phase and its correct-and-ten-stable final stop rule.
    if args.gestures or args.trusted_wheel:
        _ux_probe.GESTURE_INPUTS = max(1, args.gesture_inputs)
    else:
        _ux_probe.GESTURE_INPUTS = 0
        _ux_probe.ZOOM_PROOF_FRACTION = 1.0
    forwarded = [
        "bench_ux.py",
        "--sizes",
        args.sizes,
        "--arms",
        args.arms,
        "--timeout",
        str(args.timeout),
        "--memory-gib",
        str(args.memory_gib),
        "--no-record",
        "--no-grid",
        "--chrome",
        args.chrome,
        "--out",
        str(args.out),
    ]
    if args.software:
        forwarded.append("--software")
    sys.argv = forwarded
    bench_ux.main()
    if args.out.exists():
        document = json.loads(args.out.read_text(encoding="utf-8"))
        document["input_delivery"] = (
            "cdp_trusted_mouse_wheel" if args.trusted_wheel else "javascript_wheel_event"
        )
        args.out.write_text(json.dumps(document, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
