"""Python API for DragonGUI."""

from ._backend import BackendUnavailableError, backend_info, native_backend_available
from .app import App
from .components import ComponentCtx, ComponentInstance, StateSlot, component
from .dialogs import (
    FileDialog,
    open_file_dialog,
    open_files_dialog,
    pick_folder_dialog,
    save_file_dialog,
)
from .notifications import toast
from .runtime import ToastHandle
from .theme import Theme
from .vdom import Patch, ResourceRef, VNode
from .widgets import (
    Badge,
    Button,
    Checkbox,
    Collapsible,
    ColorPicker,
    DataFrameTable,
    Dropdown,
    FlowLayout,
    GridLayout,
    HLayout,
    Image,
    LED,
    Label,
    ContextMenu,
    Menu,
    MenuBar,
    MenuItem,
    Modal,
    NavItem,
    NumberInput,
    Page,
    Pages,
    Panel,
    ProgressBar,
    Scatter3D,
    ScatterFrameStream,
    ScatterHit,
    ScatterPick,
    ScatterPayload,
    ScatterStreamMetrics,
    ScrollArea,
    Separator,
    Sidebar,
    Slider,
    Spacer,
    StatusBar,
    Tab,
    Tabs,
    TableSelection,
    Tag,
    TextArea,
    TextInput,
    Tooltip,
    VLayout,
    Window,
    alert,
    confirm,
)


def link_cameras(*scatters: "Scatter3D") -> None:
    """Link two or more Scatter3D widgets so programmatic camera changes propagate between them."""
    for s in scatters:
        s.link_cameras(*(o for o in scatters if o is not s))


def unlink_cameras(*scatters: "Scatter3D") -> None:
    """Remove camera links between Scatter3D widgets."""
    for s in scatters:
        s.unlink_cameras(*(o for o in scatters if o is not s))


__all__ = [
    "App",
    "BackendUnavailableError",
    "ComponentCtx",
    "ComponentInstance",
    "FileDialog",
    "Patch",
    "ResourceRef",
    "StateSlot",
    "Theme",
    "ToastHandle",
    "VNode",
    "Badge",
    "Button",
    "Checkbox",
    "Collapsible",
    "ColorPicker",
    "DataFrameTable",
    "Dropdown",
    "FlowLayout",
    "GridLayout",
    "HLayout",
    "Image",
    "LED",
    "Label",
    "ContextMenu",
    "Menu",
    "MenuBar",
    "MenuItem",
    "Modal",
    "NavItem",
    "NumberInput",
    "Page",
    "Pages",
    "Panel",
    "ProgressBar",
    "Scatter3D",
    "ScatterFrameStream",
    "ScatterHit",
    "ScatterPick",
    "ScatterPayload",
    "ScatterStreamMetrics",
    "ScrollArea",
    "Separator",
    "Sidebar",
    "Slider",
    "Spacer",
    "StatusBar",
    "Tab",
    "Tabs",
    "TableSelection",
    "Tag",
    "TextArea",
    "TextInput",
    "Tooltip",
    "VLayout",
    "Window",
    "alert",
    "backend_info",
    "component",
    "confirm",
    "open_file_dialog",
    "open_files_dialog",
    "pick_folder_dialog",
    "save_file_dialog",
    "native_backend_available",
    "toast",
    "link_cameras",
    "unlink_cameras",
]

__version__ = "0.1.0"
