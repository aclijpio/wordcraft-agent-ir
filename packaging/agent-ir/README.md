# WordCraft Agent IR — Windows release

Generic agent document IR on top of WordCraft.

## Zip contents

| File | Purpose |
|---|---|
| `wordcraft-cli.exe` | Headless MCP + CLI |
| `wordcraft.exe` | GUI with `--control` for live MCP |
| `docs/agent-ir.md` | IR contract |
| `mcp.json.example` | Cursor MCP template |
| `templates/` | Optional custom `.docx` ids |

## Quick start

```bat
wordcraft-cli.exe mcp
wordcraft-cli.exe run --cmd "doc.applyLayout={}" --cmd "doc.getIr={}" --save out.docx
```

Live:

```bat
set WORDCRAFT_PROFILE_DIR=C:\path\to\writable-profile
wordcraft.exe --control 17991
wordcraft-cli.exe mcp --connect 127.0.0.1:17991
```

Thesis-specific layout: use a local skill profile with `apply_layout` — not a built-in VKR engine command.
