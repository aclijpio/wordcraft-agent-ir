@echo off
setlocal
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" -DesktopShortcut -UpdateCursorMcp %*
if errorlevel 1 (
  echo Install failed.
  pause
  exit /b 1
)
echo.
echo Open Start Menu -^> WordCraft Agent IR, or run start-live.bat
echo Reload Cursor MCP / Window after install.
pause
