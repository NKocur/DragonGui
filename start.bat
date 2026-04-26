@echo off
setlocal

set "ROOT=%~dp0"
set "PYTHONPATH=%ROOT%python;%PYTHONPATH%"
set "PYTHONDONTWRITEBYTECODE=1"

if defined PYTHON_EXE (
    set "PYTHON_CMD=%PYTHON_EXE%"
) else (
    set "PYTHON_CMD=python"
)

rem -------------------------------------------------------------------------
rem Copy the compiled extension into the source tree so PYTHONPATH=python
rem can find it via a relative import.  Prefer release over debug.
rem -------------------------------------------------------------------------
set "EXT_DEST=%ROOT%python\dragongui\_dragongui.pyd"
set "EXT_RELEASE=%ROOT%native\target\x86_64-pc-windows-gnu\release\_dragongui.dll"
set "EXT_DEBUG=%ROOT%native\target\x86_64-pc-windows-gnu\debug\_dragongui.dll"

if exist "%EXT_RELEASE%" (
    copy /Y "%EXT_RELEASE%" "%EXT_DEST%" >nul
) else if exist "%EXT_DEBUG%" (
    copy /Y "%EXT_DEBUG%" "%EXT_DEST%" >nul
) else (
    rem No native build found — run in Python-only dev fallback mode.
    rem Build first with: python -m maturin build --target x86_64-pc-windows-gnu
    set "DRAGONGUI_DEV_FALLBACK=1"
)

"%PYTHON_CMD%" "%ROOT%examples\scatter_tool.py" %*
exit /b %ERRORLEVEL%
