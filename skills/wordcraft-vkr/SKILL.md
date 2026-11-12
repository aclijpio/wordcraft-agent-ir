---
name: wordcraft-vkr
description: Edit VKR (thesis) documents in WordCraft via agent IR and high-level MCP tools. Use when formatting or revising a ВКР, applying методичка layout, fixing pageBreakBefore sprawl, or numbering headings 1 / 1.1.
---

# WordCraft VKR

## Always

1. Read the local VKR rules file (e.g. `ВКР-правила-письма.md`) before layout or style edits.
2. **Prefer `wordcraft-live`** (app with `--control`). Edits hit the open window — no reopen to preview.
3. Prefer high-level tools over low-level `execute`:
   - `apply_vkr_layout` — one shot: body/margins, headings, captions (Рисунок/Таблица without first-line/left indent leak)
   - `get_ir` — structure (`role`, `indentLeft`/`indentFirst`); use `pageBreaksOnly` / `from`+`to` / `fields`
   - `apply_ir_patch` — paragraph updates; for a word/sentence use `op:"replaceIn"` with `find`/`with`/`occurrence` (do not rewrite whole `text`)
   - `apply_body_style` / `ensure_heading` / `clear_stray_page_breaks` — when not using the one-shot
4. Use low-level `execute` only as fallback (tables, TOC, pictures, missing IR field).
5. Work on a **copy** of the thesis `.docx`, never the only original.
6. Never invent `.mjs`/`.ps1` bridges when live MCP can talk to the app.

## Hard rules

- Never leave `pageBreakBefore: true` on Normal / body paragraphs.
- `pageBreakBefore` only for Heading1 section starts (ВВЕДЕНИЕ, 1 …, ЗАКЛЮЧЕНИЕ, СПИСОК…, ПРИЛОЖЕНИЕ…).
- Heading2/3 do **not** start a new page.
- First-line indent ≈ 35.45 pt — not spaces or Tab.
- Body: Times New Roman 14, line spacing 1.5 (multiple), justify; bold mainly in headings.

## Typical sequence (live)

1. App from this fork with `doc.*` (`wordcraft.exe --control 17991`), MCP `wordcraft-live` connected to the same port and `WORDCRAFT_PROFILE_DIR`.
2. `list_documents` — if the copy is not open: `open_document` on the copy path.
3. `get_ir` with `pageBreaksOnly: true` — note `pages` and stray breaks.
4. `apply_vkr_layout` (or the three separate tools).
5. Fix headings / text with `apply_ir_patch` / `type_text` as needed.
6. `get_ir` / `render_page` / `screenshot` — verify in the **same** window; do not ask the user to reopen.
7. `save_document` when done.

## MCP backends

- **Default:** `wordcraft-live` — live session in the open app.
- **Headless `wordcraft`:** offline batch / CI only.
- ArtCraft MSI may lag this fork — if `doc.*` is unknown, use the release `wordcraft-cli.exe` / built `wordcraft.exe` from this repository.

Details: [docs/agent-ir.md](../../docs/agent-ir.md) and [packaging/agent-ir/README.md](../../packaging/agent-ir/README.md).
