@echo off
setlocal
cd /d "%~dp0\.."
set "PYTHONPATH=%CD%\python;%PYTHONPATH%"
py -3 examples\terminal_wrapper_demo.py
echo.
echo Probe exited with code %ERRORLEVEL%.
pause
exit /b %ERRORLEVEL%
