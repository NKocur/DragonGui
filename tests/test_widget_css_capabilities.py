from __future__ import annotations

from tools.generate_widget_css_capabilities import update_docs


def test_generated_widget_css_capability_docs_are_current() -> None:
    assert update_docs(check=True)
