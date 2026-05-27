from __future__ import annotations

import platform
import sys

import dragongui


def main() -> None:
    print("python:", sys.version.replace("\n", " "))
    print("machine:", platform.machine())
    print("dragongui:", dragongui.__file__)
    print("native_backend_available:", dragongui.native_backend_available())
    print("backend:", dragongui.backend_info())


if __name__ == "__main__":
    main()
