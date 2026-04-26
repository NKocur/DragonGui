"""Python API for DragonGUI."""

from ._backend import BackendUnavailableError, backend_info, native_backend_available
from .app import App
from .components import ComponentCtx, ComponentInstance, StateSlot, component
from .theme import Theme
from .vdom import Patch, ResourceRef, VNode
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
    Separator,
    Sidebar,
    Slider,
    Spacer,
    StatusBar,
    Tab,
    Tabs,
    TextInput,
    VLayout,
    Window,
)

__all__ = [
    "App",
    "BackendUnavailableError",
    "ComponentCtx",
    "ComponentInstance",
    "Patch",
    "ResourceRef",
    "StateSlot",
    "Theme",
    "VNode",
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
    "Separator",
    "Sidebar",
    "Slider",
    "Spacer",
    "StatusBar",
    "Tab",
    "Tabs",
    "TextInput",
    "VLayout",
    "Window",
    "backend_info",
    "component",
    "native_backend_available",
]

__version__ = "0.1.0"
