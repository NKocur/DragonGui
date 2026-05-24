from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #10151d;
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
        gap: 12px;
        width: 100%;
        height: auto;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 380px;
        min-height: 350px;
        background: rgba(21, 30, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 10px;
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
        background: rgba(90, 169, 255, 0.12);
        border: 1px solid rgba(90, 169, 255, 0.34);
        border-radius: 8px;
        color: rgba(228, 241, 255, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    TreeView {
        width: 100%;
        gap: 0;
    }

    TreeNode {
        height: 28px;
        border-radius: 6px;
        color: rgba(246, 249, 255, 0.86);
        transition: background 120ms ease, border-color 120ms ease, color 120ms ease;
    }

    TreeNode:hover {
        background: rgba(90, 169, 255, 0.10);
        border-color: rgba(90, 169, 255, 0.24);
    }

    TreeNode:selected {
        background: rgba(90, 169, 255, 0.18);
        border-color: rgba(90, 169, 255, 0.62);
        color: white;
    }

    TreeNode::indicator {
        width: 14px;
        color: rgba(246, 249, 255, 0.58);
    }

    TreeNode:expanded::indicator,
    TreeNode:selected::indicator {
        color: #5aa9ff;
    }

    TreeNode::guide {
        background: rgba(246, 249, 255, 0.16);
        border-width: 1px;
    }

    TreeNode:disabled {
        color: rgba(246, 249, 255, 0.36);
        opacity: 0.70;
    }
    """
)

win = dg.Window("TreeView probe", width=980, height=620)

with dg.VLayout(class_="root"):
    dg.Label("TreeView", class_="title")
    status = dg.Label("Selected node: src/widgets.py", class_="status")

    def set_status(node_id: str) -> None:
        status.set_value(f"Selected node: {node_id}")

    with dg.HLayout(class_="grid"):
        with dg.Panel("Data-driven tree", class_="case"):
            dg.Label("Nested branches should expand, collapse, and keep one selected row.", class_="caption")
            dg.TreeView(
                [
                    {
                        "label": "src",
                        "id": "src",
                        "expanded": True,
                        "children": [
                            {"label": "app.py", "id": "src/app.py", "leaf": True},
                            {"label": "widgets.py", "id": "src/widgets.py", "leaf": True},
                            {
                                "label": "native",
                                "id": "src/native",
                                "expanded": True,
                                "children": [
                                    {"label": "runtime.rs", "id": "src/native/runtime.rs", "leaf": True},
                                    {"label": "layout.rs", "id": "src/native/layout.rs", "leaf": True},
                                ],
                            },
                        ],
                    },
                    {
                        "label": "tests",
                        "id": "tests",
                        "expanded": False,
                        "children": [
                            {"label": "test_python_api.py", "id": "tests/test_python_api.py", "leaf": True},
                        ],
                    },
                    {"label": "README.md", "id": "readme", "leaf": True, "disabled": True},
                ],
                selected="src/widgets.py",
                on_select=set_status,
            )

        with dg.Panel("Manual tree", class_="case"):
            dg.Label("Context-manager construction should wire nested TreeNode rows into the same TreeView.", class_="caption")
            with dg.TreeView(on_select=lambda node_id: set_status(f"manual/{node_id}")):
                with dg.TreeNode("Scene", node_id="scene", expanded=True):
                    dg.TreeNode("Camera", node_id="camera", leaf=True)
                    with dg.TreeNode("Lights", node_id="lights", expanded=True):
                        dg.TreeNode("Key", node_id="key-light", leaf=True, selected=True)
                        dg.TreeNode("Fill", node_id="fill-light", leaf=True)
                with dg.TreeNode("Materials", node_id="materials", expanded=False):
                    dg.TreeNode("Glass", node_id="glass", leaf=True)
                    dg.TreeNode("Metal", node_id="metal", leaf=True)

    dg.Label("PASS: tree rows, disclosure indicators, guides, selected, hover, focus, disabled, and nested layout render.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
