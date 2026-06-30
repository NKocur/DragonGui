@echo off
setlocal

rem Build DragonGUI's native wheel and copy the extracted extension into python\dragongui.
rem Optional first argument: full path to the Python executable to build against.

cd /d "%~dp0"

if "%~1"=="" (
    set "PYTHON_EXE=C:\Users\nkocur\AppData\Local\Microsoft\WindowsApps\python3.12.exe"
) else (
    set "PYTHON_EXE=%~1"
)

if not exist "%PYTHON_EXE%" (
    echo Python executable not found: %PYTHON_EXE%
    echo Falling back to python on PATH.
    set "PYTHON_EXE=python"
)

set "PYO3_PYTHON=%PYTHON_EXE%"

echo.
echo Building native wheel with:
echo   %PYTHON_EXE%
echo.

"%PYTHON_EXE%" -m maturin build --release
if errorlevel 1 goto :fail

set "WHEEL="
for /f "delims=" %%F in ('dir /b /a-d /o-d "native\target\wheels\dragongui-*.whl" 2^>nul') do (
    if not defined WHEEL set "WHEEL=native\target\wheels\%%F"
)

if not defined WHEEL (
    echo Could not find built wheel in native\target\wheels.
    goto :fail
)

for %%F in ("%WHEEL%") do set "WHEEL_NAME=%%~nxF"

echo.
echo Built wheel:
echo   %WHEEL%
echo.

if not exist "dist" mkdir "dist"
if not exist ".test-cache" mkdir ".test-cache"
if not exist ".test-cache\wheel_extract" mkdir ".test-cache\wheel_extract"

copy /Y "%WHEEL%" "dist\%WHEEL_NAME%"
if errorlevel 1 goto :fail

copy /Y "%WHEEL%" ".test-cache\dragongui-wheel.zip"
if errorlevel 1 goto :fail

powershell -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -LiteralPath '.test-cache\dragongui-wheel.zip' -DestinationPath '.test-cache\wheel_extract' -Force"
if errorlevel 1 goto :fail

if not exist ".test-cache\wheel_extract\dragongui\_dragongui.pyd" (
    echo Extracted wheel did not contain dragongui\_dragongui.pyd.
    goto :fail
)

copy /Y ".test-cache\wheel_extract\dragongui\_dragongui.pyd" "python\dragongui\_dragongui.pyd"
if errorlevel 1 (
    echo.
    echo Failed to copy python\dragongui\_dragongui.pyd.
    echo Close any running DragonGUI example windows and run this script again.
    goto :fail
)

echo.
echo Verifying native import:
"%PYTHON_EXE%" -c "import sys; sys.path.insert(0, 'python'); import dragongui._dragongui as native; print(native.__file__)"
if errorlevel 1 goto :fail

echo.
echo Done.
exit /b 0

:fail
echo.
echo Rebuild/copy failed.
exit /b 1

