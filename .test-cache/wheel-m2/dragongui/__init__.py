"""Python API for DragonGUI."""

from ._backend import BackendUnavailableError, backend_info, native_backend_available
from .app import App
from .widgets import (
    Button,
    Checkbox,
    DataFrameTable,
    Dropdown,
    HLayout,
    Label,
    Panel,
    Scatter3D,
    Slider,
    TextInput,
    VLayout,
    Window,
)

__all__ = [
    "App",
    "BackendUnavailableError",
    "Button",
    "Checkbox",
    "DataFrameTable",
    "Dropdown",
    "HLayout",
    "Label",
    "Panel",
    "Scatter3D",
    "Slider",
    "TextInput",
    "VLayout",
    "Window",
    "backend_info",
    "native_backend_available",
]

__version__ = "0.1.0"
