# Agent IR (generic)

> **Last updated:** 2026-11-13 · IR v2 (in-memory) + file IR v3 (`styles.json` + sections). Thesis (VKR) rules live in a local skill profile, not in the engine.

Agents edit documents through structural IR and high-level `doc.*` / MCP tools.

## Live-first

```sh
cargo run --release -p wordcraft -- --control 17991
cargo run --release -p wordcraft-cli -- mcp --connect 127.0.0.1:17991
```

## File IR v3 (preferred for large docs)

**Rule:** if the document is large (thesis / multi-section draft) or an IR folder already exists, edit **only** through that folder. Do not mix bulk `insert_table` / `insert_figure` / `select_text` with file IR — that is what breaks indices and drops pictures.

One folder per document:

- Saved doc: `{stem}.wordcraft-ir/` next to the `.docx`
- Unsaved: `{WORDCRAFT_PROFILE_DIR|settings}/ir/{documentId}/`

```
manifest.json
styles.json          # style catalog (classes), not per-paragraph formatting
sections/01-….json   # ordered blocks: id + type + style + content
assets/….png         # pictures
```

| Command / MCP | Role |
|---|---|
| `doc.exportIr` / `export_ir` | Assign stable ids (bookmarks `_WcIr_<id>`), write styles + sections + assets |
| `doc.syncIr` / `sync_ir` | Apply `styles.json` to the StyleSheet, rebuild body from section order |
| `doc.irPath` / `ir_path` | Absolute folder path |

| Content | In section JSON / assets | Live MCP (fallback only) |
|---|---|---|
| Text / headings | `type:"paragraph"` | `apply_ir_patch` / `type_text` for tiny edits |
| Table | `type:"table"`, `cells`, `header` | `insert_table` + **`index`** |
| Picture | `type:"picture"`, `asset:"assets/….png"` | `insert_figure` + **`index`** |
| Caption | paragraph `style:"Caption"` | `insert_caption` / figure `caption` |

Blocks reference a style by name (`"style":"Heading2"`). Do not copy font/indent onto every paragraph; change `styles.json` or the block's `style`. Rare overrides (`align`, `indentFirst`, …) may sit on one block as direct formatting.

Stable **`id`**: the document stays a sequence; `id` names a block inside that sequence so patches survive inserts/deletes. Prefer `"id"` over `"index"` in `apply_ir_patch` after export.

Live control / MCP apply uses keep-view (no scroll-to-caret). After export, the desktop app can watch the IR folder and debounce-sync.

## IR (`doc.getIr` / `get_ir`)

Version **2**. Blocks: `paragraph`, `table`, `picture`. Optional: `from`/`to`, `fields`, `pageBreaksOnly`, `kinds`. When bookmarks exist, blocks include `id`.

## Patch (`doc.applyIrPatch` / `apply_ir_patch`)

Ops: `update`, `replace`, `replaceIn`, `insert`, `delete`, `move`, `setStyle`, `setCell`. Address with `index` or `id`.

`replace` supports `paragraph` and `table` (`cells` / `values`, `header`, `style`). For large drafts edit `sections/*.json` (tables and picture assets) and `sync_ir`; do not fill via live `insert_table` / caret when the IR folder exists.

```json
{"ops":[
  {"op":"insert","index":0,"block":{"type":"paragraph","text":"Title","style":"Heading1"}},
  {"op":"replace","id":"b00a1f3c","block":{"type":"table","cells":[["A","B"],["1","2"]],"header":true}},
  {"op":"replaceIn","id":"b00a1f3c","find":"old","with":"new"},
  {"op":"setCell","index":5,"row":0,"col":1,"text":"B1"}
]}
```

## Layout / styles

| Command / MCP | Role |
|---|---|
| `doc.applyLayout` / `apply_layout` | Page, margins, Normal, Caption, Headings from `profile` |
| `doc.ensureStyles` / `ensure_styles` | Upsert stylesheet entries |
| `doc.ensureHeading` / `ensure_heading` | Heading 1–N |
| `doc.clearStrayPageBreaks` | Clear non-Heading1 page breaks |
| `doc.applyBodyStyle` | Alias of `applyLayout` |

Thesis margins/fonts: pass a profile JSON from a local skill — do not hardcode in the engine. After `apply_layout`, `export_ir` refreshes `styles.json` for the agent.

## Figures / TOC / refs

| Command / MCP | Wraps |
|---|---|
| `doc.insertFigure` / `insert_figure` | `insert.picture` + optional `references.caption` |
| `doc.insertCaption` / `insert_caption` | `references.caption` |
| `doc.insertToc` / `insert_toc` | `references.toc` |
| `doc.updateToc` / `update_toc` | `references.updateToc` |
| `doc.updateFields` / `update_fields` | `references.updateFields` |
| `doc.insertCrossRef` / `insert_cross_ref` | `insert.crossReference` |
| `doc.applyTableStyle` | Table `props.style` by body index |

Pictures in file IR: `type:"picture"` + `asset:"assets/….png"`. Tables: `type:"table"` + `cells`. TOC/numbering: styles + existing field/update commands after sync.

## Templates

`doc.newFromTemplate` `{id|template|path}` — built-in sample ids or a `.docx` path / file under `packaging/agent-ir/templates/`.
