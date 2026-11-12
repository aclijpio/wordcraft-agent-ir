# WordCraft Agent IR — Windows release

Generic agent document IR on top of WordCraft.

## Install (recommended)

1. Unpack the zip anywhere.
2. Double-click **`install.bat`**  
   (or: `powershell -ExecutionPolicy Bypass -File install.ps1 -DesktopShortcut -UpdateCursorMcp`)

This copies the app to:

`%LOCALAPPDATA%\Programs\WordCraft-Agent-IR`

and creates:

- Start Menu → **WordCraft Agent IR**
- Desktop shortcut (with `install.bat`)
- `start-live.bat` (GUI + control port **17991**)
- `cursor-mcp-snippet.json`
- updates Cursor `%USERPROFILE%\.cursor\mcp.json` for `wordcraft` / `wordcraft-live` when `-UpdateCursorMcp` is used

Uninstall:

```bat
powershell -ExecutionPolicy Bypass -File "%LOCALAPPDATA%\Programs\WordCraft-Agent-IR\uninstall.ps1"
```

Then reload Cursor MCP / Window.

## Zip contents

| File | Purpose |
|---|---|
| `install.bat` / `install.ps1` | Windows install (no admin) |
| `wordcraft-cli.exe` | Headless MCP + CLI |
| `wordcraft.exe` | GUI with `--control` for live MCP |
| `docs/agent-ir.md` | IR contract |
| `mcp.json.example` | Cursor MCP template |
| `templates/` | Optional custom `.docx` ids |

## Manual run (without install)

```bat
wordcraft-cli.exe mcp
set WORDCRAFT_PROFILE_DIR=%CD%\profile
wordcraft.exe --control 17991
```

Thesis-specific layout: local skill `profile.json` + `apply_layout` — not a built-in VKR engine command.
