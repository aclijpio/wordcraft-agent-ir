# WordCraft Agent IR — Windows release

Fork of [storytold/wordcraft](https://github.com/storytold/wordcraft) with high-level agent IR tools for structural edits and VKR layout.

## What you get in the zip

| File | Purpose |
|---|---|
| `wordcraft-cli.exe` | Headless MCP + CLI (`doc.getIr`, `doc.applyVkrLayout`, …) |
| `docs/agent-ir.md` | IR shape, tools, live-first workflow |
| `skills/wordcraft-vkr/` | Cursor skill for thesis formatting |
| `mcp.json.example` | Cursor MCP template (headless + live) |

## Quick start (headless)

```bat
wordcraft-cli.exe mcp
```

Point Cursor MCP `wordcraft` at that binary (see `mcp.json.example`).

```bat
wordcraft-cli.exe run --file thesis.docx --cmd doc.applyVkrLayout --save out.docx
```

## Live window (no reopen)

1. Build or use a matching `wordcraft.exe` that includes `doc.*` (this fork).
2. Start with a writable profile and control port:

```bat
set WORDCRAFT_PROFILE_DIR=C:\path\to\writable-profile
wordcraft.exe --control 17991 C:\path\to\copy.docx
```

3. Cursor `wordcraft-live` → `mcp --connect 127.0.0.1:17991` with the same `WORDCRAFT_PROFILE_DIR`.

Installed ArtCraft MSI may lag this fork: if `doc.applyVkrLayout` is unknown, you are on an old engine.

## Do not

- Drive a document with headless save while the same file is open in another WordCraft — that forces a reopen.
- Write custom `.mjs`/`.ps1` bridges when live MCP works.
