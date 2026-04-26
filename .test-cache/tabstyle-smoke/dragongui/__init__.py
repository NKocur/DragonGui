"""Python API for DragonGUI."""

from ._backend import BackendUnavailableError, backend_info, native_backend_available
from .app import App
from .theme import Theme
from .widgets import (
    Button,
    Checkbox,
    DataFrameTable,
    Dropdown,
    HLayout,
    Label,
    NavItem,
    Page,
    Pages,
    Panel,
    Scatter3D,
    Sidebar,
    Slider,
    Tab,
    Tabs,
    TextInput,
    VLayout,
    Window,
)

__all__ = [
    "App",
    "BackendUnavailableError",
    "Theme",
    "Button",
    "Checkbox",
    "DataFrameTable",
    "Dropdown",
    "HLayout",
    "Label",
    "NavItem",
    "Page",
    "Pages",
    "Panel",
    "Scatter3D",
    "Sidebar",
    "Slider",
    "Tab",
    "Tabs",
    "TextInput",
    "VLayout",
    "Window",
    "backend_info",
    "native_backend_available",
]

__version__ = "0.1.0"
