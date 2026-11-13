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
set WORDCRAFT_PROFILE_DIR=%CD%\profile
wordcraft.exe --control 17991
```

`wordcraft-live` MCP must use the **same** `WORDCRAFT_PROFILE_DIR` (and port) as the running exe. If the GUI was started from Downloads without that env, the key lands under that folder's `profile\`, while Cursor may still look in `%LOCALAPPDATA%\Programs\WordCraft-Agent-IR\profile` — then you get `no control key`. Prefer **Start Menu → WordCraft Agent IR** (`start-live.bat`), or point MCP `env.WORDCRAFT_PROFILE_DIR` at the profile next to the exe you launched.

Thesis-specific layout: local skill `profile.json` + `apply_layout` — not a built-in VKR engine command.

## File IR (large documents)

Prefer folder IR over shipping the whole document through MCP:

1. `export_ir` → `{stem}.wordcraft-ir/` (`styles.json`, `sections/*.json`, `assets/`)
2. Edit files (styles catalog separately; blocks only `style` + content + stable `id`)
3. `sync_ir` (live app keeps the viewport; optional folder watch debounces sync)

See `docs/agent-ir.md`.
