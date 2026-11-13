---
name: wordcraft-vkr
description: Format VKR (thesis) documents via generic WordCraft Agent IR + a local layout profile. Use for ВКР layout, captions, headings 1 / 1.1 — not engine-specific VKR commands.
---

# WordCraft VKR (profile on generic IR)

VKR is a **scenario**, not an engine feature. Use generic MCP tools with a local profile.

## Always

1. Read local VKR rules (e.g. `ВКР-правила-письма.md`).
2. Prefer `wordcraft-live` (app `--control`).
3. Load thesis layout via `apply_layout` with profile from this skill folder (`profile.json`), not `apply_vkr_layout` (removed).
4. Prefer high-level tools: for large docs `export_ir` → edit `styles.json` / `sections/*.json` → `sync_ir`; else `get_ir`, `apply_ir_patch` (`id` or `replaceIn`), `insert_figure`, `insert_toc`, `apply_layout`, `ensure_heading`.
5. Work on a **copy** of the `.docx`. Styles live in the IR catalog (`styles.json`), not copied onto every paragraph.

## Profile

See `profile.json` next to this skill (margins 30/15/20/20 mm, TNR 14, first-line 35.45, caption rules). Call:

```json
{"profile": { …contents of profile.json… }}
```

via MCP `apply_layout`.

## Captions / figures

- `insert_figure` with `caption` / `captionLabel: "Рисунок"` (or English `Figure`).
- After layout, captions should be Caption style, centered, no first-line indent.
