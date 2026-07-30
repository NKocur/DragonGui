from __future__ import annotations

import argparse
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REGISTRY_PATH = ROOT / "python" / "dragongui" / "widget_css_capabilities.json"
START_MARKER = "<!-- BEGIN GENERATED WIDGET CSS CAPABILITIES -->"
END_MARKER = "<!-- END GENERATED WIDGET CSS CAPABILITIES -->"
TARGETS = {
    ROOT / "docs" / "widgets.md": "## CSS Part Catalog",
    ROOT / "docs" / "widgets-reference.md": "## CSS Part Inventory",
    ROOT / "docs" / "css-styling.md": "Supported parts:",
    ROOT / "docs" / "css-capabilities-reference.md": "Supported parts:",
    ROOT / "docs" / "sphinx" / "styling.md": "Frequently used parts:",
}


def load_registry() -> dict[str, object]:
    return json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))


def render_table(registry: dict[str, object]) -> str:
    state_profiles = registry["state_profiles"]
    widgets = registry["widgets"]
    generated = registry["generated_content"]
    generated_parts = ", ".join(f"`::{part}`" for part in generated["parts"])
    lines = [
        START_MARKER,
        "",
        "_Generated from `python/dragongui/widget_css_capabilities.json`. Do not edit this table manually._",
        "",
        f"Global generated-content hooks: {generated_parts} ({generated['renderer']} renderer).",
        "",
        "| Widget | Supported states | Parts and renderer support |",
        "| --- | --- | --- |",
    ]
    for widget in sorted(widgets, key=lambda item: item["public_type"]):
        states = ", ".join(
            f"`:{state}`" for state in state_profiles[widget["states_profile"]]
        )
        parts = ", ".join(
            f"`{part}` ({renderer})"
            for renderer, renderer_parts in widget["parts"].items()
            for part in sorted(renderer_parts)
        )
        lines.append(f"| `{widget['public_type']}` | {states} | {parts} |")
    lines.extend(["", END_MARKER])
    return "\n".join(lines)


def replace_generated_table(source: str, anchor: str, generated: str) -> str:
    if START_MARKER in source:
        start = source.index(START_MARKER)
        end = source.index(END_MARKER, start) + len(END_MARKER)
        return source[:start] + generated + source[end:]

    anchor_index = source.index(anchor)
    table_start = source.index("| Widget | Parts |", anchor_index)
    table_end = table_start
    while table_end < len(source):
        next_line = source.find("\n", table_end)
        if next_line == -1:
            table_end = len(source)
            break
        following = next_line + 1
        if following >= len(source) or source[following] != "|":
            table_end = next_line
            break
        table_end = following
    return source[:table_start] + generated + source[table_end:]


def update_docs(*, check: bool) -> bool:
    generated = render_table(load_registry())
    clean = True
    for path, anchor in TARGETS.items():
        source = path.read_text(encoding="utf-8")
        updated = replace_generated_table(source, anchor, generated)
        if updated != source:
            clean = False
            if not check:
                path.write_text(updated, encoding="utf-8")
            else:
                print(f"out of date: {path.relative_to(ROOT)}")
    return clean


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate DragonGui widget CSS capability documentation."
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Report drift without writing documentation.",
    )
    args = parser.parse_args()
    clean = update_docs(check=args.check)
    return 0 if clean or not args.check else 1


if __name__ == "__main__":
    raise SystemExit(main())
