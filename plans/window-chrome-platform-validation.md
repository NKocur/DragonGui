# Window Chrome Platform Validation Matrix

**Parent plan:** [CSS and theming system improvement plan](css-theming-system-improvement-plan.md)
**Feature:** `Window(..., decorations="client")`
**Reference demo:** `examples/client_window_chrome_demo.py`
**Last updated:** 2026-07-30

## Purpose

This matrix defines the evidence required to finish Phase 8. Automated checks
cover retained-tree structure, CSS forwarding, focus order, keyboard routing,
Windows state transitions, viewport/DPI geometry, and safe macOS fallback
policy. The checks below cover the operating-system and compositor behavior
that cannot be proved faithfully by an in-process synthetic input driver.

Do not mark a platform cell passed from source inspection alone. Record the
operating-system version, display server, scale factor, result, and any artifact
or issue reference.

## Current Evidence

| Platform | Automated | Human interaction | Status |
|---|---:|---:|---|
| Windows | Viewport/DPI, pointer maximize/restore, keyboard traversal and activation, Alt+Space | Pending gestures listed below | In progress |
| macOS | Native-decoration fallback policy unit tested | Pending real host | In progress |
| Linux/X11 | Platform policy and retained behavior unit tested | Pending real host | In progress |
| Linux/Wayland | Platform policy and retained behavior unit tested | Pending real compositor | In progress |

Windows automated artifacts:

- `artifacts/visual_audit/phase8-windows-client-chrome/`
- `artifacts/visual_audit/phase8-windows-client-chrome-maximized/`
- `artifacts/visual_audit/phase8-windows-client-chrome-transitions/`
- `artifacts/visual_audit/phase8-windows-client-chrome-input-final/`

The 2026-07-30 input run passed all three registered states:

- Exact focus traversal through minimize, maximize, close, and application
  controls.
- Enter maximized and Space restored the retained maximize control, with Win32
  state assertions after both transitions.
- Alt+Space opened the native system menu and Escape closed it, with
  `GetGUIThreadInfo` assertions at both points.

## Common Setup

1. Build/install the local native extension for the Python interpreter used by
   the demo.
2. Run `python examples/client_window_chrome_demo.py`.
3. Confirm there is one retained DragonGUI titlebar and no duplicate native
   titlebar unless the platform intentionally uses the documented fallback.
4. Repeat the platform's DPI rows without changing the demo CSS.
5. For every failure, capture a screenshot or short recording and record the
   platform/version, scale, and exact gesture.

## Windows Matrix

Test on a supported Windows 10 or Windows 11 host.

| Check | 100% | 150% | 200% | Pass condition |
|---|---|---|---|---|
| Titlebar drag | Pending | Pending | Pending | Window follows the pointer without a jump and native snapping remains available |
| Titlebar double-click | Pending | Pending | Pending | First double-click maximizes; second restores to the previous bounds |
| Left/right/top/bottom resize | Pending | Pending | Pending | Cursor and resize direction match every edge; content relayouts without overlap |
| Four-corner resize | Pending | Pending | Pending | Cursor and resize direction match every corner |
| Maximized resize suppression | Pending | Pending | Pending | Resize cursors and drag-resize do not activate while maximized |
| Right-click titlebar menu | Pending | Pending | Pending | Native window menu opens at the pointer and its commands work |
| Minimize button | Pending | Pending | Pending | Window enters the taskbar and restores with content/state intact |
| Close button | Pending | Pending | Pending | One click closes the demo cleanly |
| Keyboard focus visibility | Pending | Pending | Pending | Focus indication remains visible on all retained controls |
| Alt+Space menu | Automated | Automated | Automated contract; 100% live | Native menu opens and Escape dismisses it |
| Maximize/restore control | Automated | Automated | Automated | Button action, glyph, tooltip, and `window-state` stay synchronized |

Also verify one multi-monitor transition between monitors with different scale
factors. After the move, titlebar hit regions, resize cursors, double-click
tolerance, and rendered geometry must use the destination monitor's DPI.

## macOS Matrix

DragonGUI currently falls back to native decorations on macOS because Winit
does not expose client edge resizing there.

| Check | 100% | 200%/Retina | Pass condition |
|---|---|---|---|
| Fallback selection | Pending | Pending | One native titlebar is present; the retained titlebar is absent |
| Application content preservation | Pending | Pending | No authored child disappears or shifts into the titlebar |
| Diagnostic metadata | Pending | Pending | Snapshot records requested client mode, effective native mode, and fallback reason |
| Native move/resize/minimize/maximize/close | Pending | Pending | Standard macOS controls retain normal behavior |
| Theme/layout below native frame | Pending | Pending | Client content is fully visible and correctly scaled |

## Linux/X11 Matrix

| Check | 100% | 150% or 200% | Pass condition |
|---|---|---|---|
| Retained titlebar | Pending | Pending | Exactly one themed titlebar is visible |
| Titlebar drag | Pending | Pending | Window manager move operation starts without jumping |
| Edge/corner resize | Pending | Pending | All eight directions work and show correct cursors |
| Double-click maximize/restore | Pending | Pending | State toggles and returns to prior bounds |
| Minimize/maximize/close | Pending | Pending | All retained controls invoke the expected window-manager action |
| Keyboard traversal/activation | Pending | Pending | Focus order and Enter/Space behavior match Windows |
| Alt+Space/right-click | Pending | Pending | Unsupported native menu behavior is harmless and documented; no crash or stuck input |

Record the desktop environment and window manager because decorations and
window actions vary between X11 window managers.

## Linux/Wayland Matrix

| Check | 100% | 150% or 200% | Pass condition |
|---|---|---|---|
| Retained titlebar | Pending | Pending | Exactly one themed titlebar is visible |
| Compositor drag | Pending | Pending | Move request is accepted and pointer interaction remains responsive |
| Compositor edge/corner resize | Pending | Pending | All supported directions work without stale cursor state |
| Double-click maximize/restore | Pending | Pending | State toggles and retained styling follows compositor state |
| Minimize | Pending | Pending | Minimize works; inability to programmatically un-minimize is not misreported as DragonGUI restore support |
| Maximize/close | Pending | Pending | Retained controls invoke the compositor actions |
| Keyboard traversal/activation | Pending | Pending | Focus order and Enter/Space behavior match the contract |
| Unsupported system menu | Pending | Pending | Alt+Space/right-click do not crash, hang, or leave input captured |

Record the compositor name and version. XWayland results do not count as a
native Wayland result.

## Completion Rule

Phase 8 platform validation is complete when:

- Every Windows human-interaction row passes at the listed DPI values.
- The macOS native-decoration fallback passes on a real Retina-capable host.
- X11 and native Wayland matrices pass on real sessions.
- Any platform limitation is reflected in public documentation and fails safe.
- The shared accessibility bridge work is completed separately under
  `plans/V5/03-general-app-completeness.md`.
