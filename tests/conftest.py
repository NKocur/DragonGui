from __future__ import annotations

import os

import pytest

from dragongui.widgets import _BuildContext


@pytest.fixture(autouse=True)
def reset_build_context() -> None:
    _BuildContext.stack = []
    _BuildContext.root = None


@pytest.fixture(autouse=True)
def force_dev_fallback(monkeypatch: pytest.MonkeyPatch) -> None:
    """Prevent tests from opening a native window by forcing dev-fallback mode.

    DRAGONGUI_DEV_FALLBACK=1 is respected by _backend.run_document() even when
    the native extension is built, so the event loop is never entered during
    the test suite.  This env var is also inherited by subprocesses spawned
    within tests (e.g. test_scatter_example_runs_from_source_tree).
    """
    monkeypatch.setenv("DRAGONGUI_DEV_FALLBACK", "1")
