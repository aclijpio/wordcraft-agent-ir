---
name: wordcraft-vkr
description: Format VKR (thesis) documents via generic WordCraft Agent IR + a local layout profile. Use for ВКР layout, captions, headings 1 / 1.1 — not engine-specific VKR commands.
---

# WordCraft VKR (profile on generic IR)

VKR is a **scenario**, not an engine feature. Use generic MCP tools with a local profile.

## One primary path (do not hybrid)

After bootstrap, **all structure for a thesis draft goes through file IR**:

`export_ir` → edit `styles.json` + `sections/*.json` + put PNGs in `assets/` → `sync_ir`

| Put in IR | Do **not** do when the IR folder exists |
|---|---|
| paragraphs, headings | `type_text` / many live inserts for bulk text |
| tables (`type:"table"`, `cells`, `header`) | live `insert_table` + `select_text` placeholders |
| pictures (`type:"picture"`, `asset:"assets/….png"`) | live `insert_figure` by caret / `select_text` |
| caption paragraphs (`style:"Caption"`) | hope caret landed after a live insert |

Live `insert_table` / `insert_figure` / `select_text` are **fallback only** (tiny doc, no IR folder yet). If you use them, pass explicit **`index`** (never rely on selection alone).

Do **not** round-trip IR through PowerShell `ConvertTo-Json`.

## Always

1. Read the local native rules first: `ВКР-правила-письма.md` on the user’s Desktop.
2. Prefer `wordcraft-live` (app `--control`). Match port / `WORDCRAFT_PROFILE_DIR` to the running exe (`start-live.bat`).
3. Work on a **copy** of the `.docx`.
4. `apply_layout` with `profile.json` from this skill folder.
5. Put Heading1/2 spacing and Caption (`italic:false`, size 14) into `styles.json`, then `sync_ir` (or `styles.modify` once after layout).
6. Prefer stable **`id`** over fragile `index` after export.
7. Bridge Normal between Heading1 and Heading2. Prefer Russian academic wording (rules §B4.1).
8. End: `update_toc` + `update_fields` + `save_document`.

## Profile

See `profile.json` next to this skill. Pass as `{"profile": {…}}` to `apply_layout`.

## Captions

Caption style: center (figures) / left (tables), no first-line indent; force non-italic after layout. Prefer Caption blocks in section JSON over live caption inserts.
