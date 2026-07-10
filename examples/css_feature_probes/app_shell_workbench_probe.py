from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#74ddb0", radius=7, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #10141b;
        color: rgba(246, 249, 255, 0.94);
        padding: 0;
        gap: 0;
        font-size: 14px;
    }

    AppShell.dashboard {
        background: #10141b;
    }

    Sidebar.audit-sidebar {
        background: rgba(18, 25, 35, 0.98);
        border-right: 1px solid rgba(255, 255, 255, 0.14);
        padding: 14px;
        gap: 10px;
    }

    .audit-body,
    .audit-main {
        padding: 14px;
        gap: 12px;
    }

    .audit-body {
        flex: 1;
        height: 0;
    }

    .audit-workbench {
        background: #111827;
        flex: 1;
        min-width: 0;
        height: 100%;
        padding: 0;
        gap: 0;
    }

    .audit-main {
        height: 292px;
        flex-grow: 0;
        flex-shrink: 0;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.11);
        border-radius: 8px;
    }

    HLayout.summary-grid {
        width: 100%;
        gap: 10px;
    }

    Panel.card {
        flex: 1;
        min-width: 0;
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
        padding: 12px;
        gap: 8px;
    }

    Panel.fixed-card {
        width: 170px;
        flex-shrink: 0;
    }

    Panel.message {
        width: 100%;
        background: rgba(116, 221, 176, 0.09);
        border: 1px solid rgba(116, 221, 176, 0.24);
        border-radius: 8px;
        padding: 10px;
    }

    .audit-status {
        background: rgba(5, 9, 15, 0.92);
        border-top: 1px solid rgba(255, 255, 255, 0.14);
        padding: 0 12px;
        gap: 10px;
        align-items: center;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
        line-height: 1.08;
    }

    Label.sidebar-title {
        color: white;
        font-weight: 850;
    }

    Label.caption,
    Label.metric-caption {
        color: rgba(246, 249, 255, 0.68);
        line-height: 1.14;
    }

    Label.metric {
        color: #ffffff;
        font-size: 22px;
        font-weight: 850;
    }

    Label.status-left {
        color: rgba(246, 249, 255, 0.86);
        flex: 1;
        min-width: 0;
    }

    Badge.ok {
        background: rgba(116, 221, 176, 0.18);
        border: 1px solid rgba(116, 221, 176, 0.36);
        color: #dffff0;
    }

    Button.nav {
        width: 100%;
        justify-content: flex-start;
    }

    @media (max-width: 520px) {
        Sidebar.audit-sidebar {
            width: 88px;
            padding: 8px;
            gap: 8px;
        }

        .audit-body,
        .audit-main {
            padding: 8px;
            gap: 8px;
        }

        .audit-main {
            height: 250px;
        }

        .audit-status {
            padding: 0 8px;
            gap: 6px;
        }

        Badge.ok {
            padding: 4px 6px;
        }

        HLayout.summary-grid {
            flex-direction: column;
            gap: 7px;
        }

        Panel.card,
        Panel.fixed-card {
            width: 100%;
            padding: 8px;
            gap: 5px;
        }

        Label.title {
            font-size: 16px;
            height: 38px;
        }

        Label.metric {
            font-size: 18px;
        }

        Panel.card Label.caption {
            text-overflow: ellipsis;
            height: 36px;
        }
    }
    """
)


with dg.Window(title="App Shell Workbench Probe", width=1040, height=720) as window:
    with dg.AppShell(class_="dashboard"):
        with dg.Sidebar(title="Audit", width=190, class_="audit-sidebar"):
            dg.Label("Shell", class_="sidebar-title")
            dg.Button("Overview", class_="nav")
            dg.Button("Metrics", class_="nav")
            dg.Button("Settings", class_="nav")
            dg.Spacer(height=8)
            dg.Badge("online", class_="ok")

        with dg.WorkbenchLayout(class_="audit-workbench", gap=0, padding=0):
            with dg.Body(scroll="y", gap=14, class_="audit-body"):
                dg.Label("AppShell + Body + WorkbenchLayout", class_="title")
                dg.Label(
                    "Bounded dashboard shell with a fixed sidebar, flexible scroll-owning body, "
                    "workbench main region, and fixed-height status bar.",
                    class_="caption",
                )

                with dg.HLayout(class_="summary-grid"):
                    with dg.Panel(class_="card fixed-card"):
                        dg.Label("Open", class_="metric-caption")
                        dg.Label("12", class_="metric")
                    with dg.Panel(class_="card"):
                        dg.Label("Queue", class_="metric-caption")
                        dg.Label("Responsive body should shrink without clipping", class_="caption")
                    with dg.Panel(class_="card"):
                        dg.Label("Owner", class_="metric-caption")
                        dg.Label("WorkbenchMain owns vertical overflow", class_="caption")

                with dg.WorkbenchMain(
                    scroll="y",
                    gap=10,
                    class_="audit-main",
                    style={"height": 292, "flex_grow": 0, "flex_shrink": 0},
                ):
                    for index in range(1, 7):
                        with dg.Panel(class_="message"):
                            dg.Label(f"Workbench row {index}", class_="sidebar-title")
                            dg.Label(
                                "This row checks nested scroll ownership, flexible width, and "
                                "text wrapping inside WorkbenchMain.",
                                class_="caption",
                            )

            with dg.StatusBar(height=34, class_="audit-status"):
                dg.Label("Ready", class_="status-left")
                dg.Badge("3 panels", class_="ok")
                dg.Badge("bar", class_="ok")


app.run(window)
