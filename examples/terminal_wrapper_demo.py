"""DragonGUI terminal wrapper demo.

This demo embeds a PTY-backed terminal surface. On Windows, install pywinpty for
interactive tools such as Codex and Claude Code:

    py -m pip install pywinpty

The widget falls back to subprocess pipes when pywinpty is unavailable, which is
fine for simple commands but not for full terminal UIs.
"""

from __future__ import annotations

import os

import dragongui as dg


app = dg.App(theme=dg.Theme.dark())
app.stylesheet(
    """
    Window { background: #111418; }
    Panel.shell { padding: 10px; gap: 8px; }
    Terminal, HtmlReport { height: 620px; border-radius: 6px; }
    Label.hint { color: muted_text; font-size: 13px; }
    """
)

win = dg.Window("Terminal Wrapper", width=1180, height=780)

command = "powershell.exe" if os.name == "nt" else os.environ.get("SHELL", "bash")
with dg.Panel("Terminal", class_="shell"):
    dg.Label("Run Codex or Claude Code in a real terminal surface.", class_="hint")
    dg.Terminal(command, height=620)

app.run(win)
