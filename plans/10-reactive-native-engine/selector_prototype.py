"""Standalone selector prototype for R7 stylesheet research.

This file intentionally lives outside the DragonGUI runtime. It measures a
small selector model before any cascade/stylesheet code is considered for the
native renderer.
"""

from __future__ import annotations

from dataclasses import dataclass
import random
import time


PSEUDO_STATES = {"hover", "active", "focus", "disabled"}


@dataclass(frozen=True, slots=True)
class Selector:
    type_name: str | None = None
    id_name: str | None = None
    classes: tuple[str, ...] = ()
    pseudo: tuple[str, ...] = ()

    @property
    def specificity(self) -> tuple[int, int, int]:
        return (
            1 if self.id_name else 0,
            len(self.classes) + len(self.pseudo),
            1 if self.type_name else 0,
        )

    def matches(self, node: "NodeInfo") -> bool:
        if self.type_name is not None and self.type_name != node.type_name:
            return False
        if self.id_name is not None and self.id_name != node.id_name:
            return False
        if any(class_name not in node.classes for class_name in self.classes):
            return False
        if any(state not in node.states for state in self.pseudo):
            return False
        return True


@dataclass(frozen=True, slots=True)
class NodeInfo:
    type_name: str
    id_name: str
    classes: frozenset[str]
    states: frozenset[str]


def parse_selector(text: str) -> Selector:
    text = text.strip()
    if not text:
        raise ValueError("selector cannot be empty")

    type_name: str | None = None
    id_name: str | None = None
    classes: list[str] = []
    pseudo: list[str] = []

    idx = 0
    while idx < len(text):
        marker = text[idx]
        if marker in ".#:":
            idx += 1
            start = idx
            while idx < len(text) and (text[idx].isalnum() or text[idx] in "_-"):
                idx += 1
            name = text[start:idx]
            if not name:
                raise ValueError(f"empty selector segment in {text!r}")
            if marker == ".":
                classes.append(name)
            elif marker == "#":
                if id_name is not None:
                    raise ValueError(f"selector has multiple ids: {text!r}")
                id_name = name
            else:
                if name not in PSEUDO_STATES:
                    raise ValueError(f"unsupported pseudo-state: {name!r}")
                pseudo.append(name)
            continue

        start = idx
        while idx < len(text) and (text[idx].isalnum() or text[idx] in "_-"):
            idx += 1
        name = text[start:idx]
        if not name:
            raise ValueError(f"unsupported selector syntax near {text[idx:]!r}")
        if type_name is not None:
            raise ValueError(f"selector has multiple type names: {text!r}")
        type_name = name

    return Selector(
        type_name=type_name,
        id_name=id_name,
        classes=tuple(classes),
        pseudo=tuple(pseudo),
    )


def generate_nodes(count: int) -> list[NodeInfo]:
    rng = random.Random(7)
    types = ["button", "label", "panel", "dropdown", "text_input", "dataframe_table"]
    classes = ["primary", "secondary", "danger", "compact", "toolbar", "data"]
    states = ["hover", "active", "focus", "disabled"]
    nodes: list[NodeInfo] = []
    for idx in range(count):
        node_classes = frozenset(rng.sample(classes, rng.randrange(0, 3)))
        node_states = frozenset(state for state in states if rng.random() < 0.08)
        nodes.append(
            NodeInfo(
                type_name=rng.choice(types),
                id_name=f"node-{idx}",
                classes=node_classes,
                states=node_states,
            )
        )
    return nodes


def benchmark(node_count: int = 10_000, iterations: int = 50) -> dict[str, float | int]:
    selectors = [
        parse_selector("button.primary:hover"),
        parse_selector("panel.toolbar"),
        parse_selector(".danger"),
        parse_selector("dataframe_table.data:focus"),
        parse_selector("#node-9999"),
        parse_selector("text_input.compact"),
    ]
    nodes = generate_nodes(node_count)
    start = time.perf_counter()
    matches = 0
    for _ in range(iterations):
        for selector in selectors:
            matches += sum(1 for node in nodes if selector.matches(node))
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    checks = node_count * len(selectors) * iterations
    return {
        "nodes": node_count,
        "selectors": len(selectors),
        "iterations": iterations,
        "checks": checks,
        "matches": matches,
        "elapsed_ms": elapsed_ms,
        "checks_per_ms": checks / elapsed_ms if elapsed_ms > 0 else 0.0,
    }


def main() -> None:
    result = benchmark()
    for key, value in result.items():
        print(f"{key}: {value}")


if __name__ == "__main__":
    main()
