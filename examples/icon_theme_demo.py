"""Application icon-theme override example."""

import dragongui as dg


SEARCH_ICON = dg.IconResource(
    [
        dg.IconStroke(
            [
                (5, 10),
                (7, 6),
                (11, 4),
                (15, 6),
                (17, 10),
                (15, 14),
                (11, 16),
                (7, 14),
            ],
            closed=True,
        ),
        dg.IconStroke([(15, 14), (21, 20)]),
    ]
)
ALT_SEARCH_ICON = dg.IconResource(
    [
        dg.IconStroke([(4, 12), (20, 12)]),
        dg.IconStroke([(12, 4), (12, 20)]),
    ]
)
SAVE_ICON = dg.IconResource(
    [
        dg.IconStroke([(4, 3), (20, 3), (20, 21), (4, 21)], closed=True),
        dg.IconStroke([(8, 3), (8, 10), (16, 10), (16, 3)]),
        dg.IconStroke([(8, 16), (16, 16)]),
    ],
    stroke_width=1.6,
)

app = dg.App(title="DragonGUI icon theme")
app.set_icon_theme({"search": SEARCH_ICON, "save": SAVE_ICON, "run": "play"})

window = dg.Window("Icon Theme", width=620, height=320)
with dg.VLayout(parent=window, style={"padding": 20, "gap": 14}):
    dg.Label("Application-owned monochrome icon geometry")
    with dg.Toolbar():
        search_button = dg.IconButton("search", tooltip="Custom search")
        dg.IconButton("save", tooltip="Custom save")
        dg.IconButton("run", tooltip="Built-in alias")
        dg.IconButton("settings", tooltip="Built-in fallback")

    alternate = [False]
    showing_search = [True]

    def swap_theme() -> None:
        alternate[0] = not alternate[0]
        app.set_icon_theme(
            {
                "search": ALT_SEARCH_ICON if alternate[0] else SEARCH_ICON,
                "save": SAVE_ICON,
                "run": "play",
            }
        )

    def change_identity() -> None:
        showing_search[0] = not showing_search[0]
        search_button.set_icon("search" if showing_search[0] else "help")

    with dg.HLayout(style={"gap": 8}):
        dg.SmallButton("Swap live theme", id="swap-icon-theme", on_click=swap_theme)
        dg.SmallButton("Change live icon", id="change-icon-identity", on_click=change_identity)

    with dg.Panel("Theming contract"):
        dg.Label(
            "Geometry comes from App.set_icon_theme(); color, interaction states, "
            "size, and spacing remain owned by IconButton CSS."
        )

app.run(window)
