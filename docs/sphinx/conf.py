from __future__ import annotations

import sys
import warnings
from importlib import metadata as importlib_metadata
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "python"))
warnings.filterwarnings(
    "ignore",
    message=r".*sphinx_autodoc_typehints.*set_application.*",
    category=DeprecationWarning,
)

project = "DragonGUI"
author = "DragonGUI contributors"
copyright = "2026, DragonGUI contributors"
try:
    release = importlib_metadata.version("dragongui")
except importlib_metadata.PackageNotFoundError:
    release = "1.0.0"

extensions = [
    "myst_parser",
    "sphinx.ext.autodoc",
    "sphinx.ext.autosummary",
    "sphinx.ext.napoleon",
    "sphinx_autodoc_typehints",
]

source_suffix = {
    ".md": "markdown",
    ".rst": "restructuredtext",
}
master_doc = "index"
exclude_patterns = ["README.md", "_build", "Thumbs.db", ".DS_Store"]

html_theme = "furo"
html_title = "DragonGUI Documentation"
html_static_path = ["_static"]

autosummary_generate = True
autodoc_member_order = "bysource"
autodoc_typehints = "description"
autodoc_typehints_format = "short"
autodoc_default_options = {
    "members": True,
    "undoc-members": False,
    "show-inheritance": True,
}

napoleon_google_docstring = True
napoleon_numpy_docstring = True
napoleon_include_init_with_doc = True

myst_enable_extensions = [
    "colon_fence",
    "deflist",
    "fieldlist",
    "substitution",
    "tasklist",
]
suppress_warnings = ["sphinx_autodoc_typehints.forward_reference"]
