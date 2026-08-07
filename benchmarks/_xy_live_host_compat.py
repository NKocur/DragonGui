"""Run XY's upstream live benchmark host with portable ES-module MIME types."""

from __future__ import annotations

import runpy
import sys
from pathlib import Path

import tornado.web


_set_extra_headers = tornado.web.StaticFileHandler.set_extra_headers


def _set_module_headers(self: tornado.web.StaticFileHandler, path: str) -> None:
    _set_extra_headers(self, path)
    if path.lower().endswith((".js", ".mjs")):
        self.set_header("Content-Type", "text/javascript; charset=UTF-8")


def main() -> None:
    if len(sys.argv) < 2:
        raise SystemExit("usage: _xy_live_host_compat.py UPSTREAM_HOST [ARGS...]")
    upstream_host = Path(sys.argv.pop(1)).resolve()
    tornado.web.StaticFileHandler.set_extra_headers = _set_module_headers
    runpy.run_path(str(upstream_host), run_name="__main__")


if __name__ == "__main__":
    main()
