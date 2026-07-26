from __future__ import annotations

from pathlib import Path


demo = Path(__file__).resolve().parents[2] / "examples" / "aurora_command_center_demo.py"
namespace = {
    "__file__": str(demo),
    "__name__": "__main__",
    "__package__": "examples",
}
exec(compile(demo.read_bytes(), str(demo), "exec"), namespace)
