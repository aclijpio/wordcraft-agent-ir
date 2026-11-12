# Agent IR (generic)

> **Last updated:** 2026-11-12 · IR v2: layout profiles, structural patch, figures, TOC, tables. Thesis (VKR) rules live in a local skill profile, not in the engine.

Agents edit documents through structural IR and high-level `doc.*` / MCP tools.

## Live-first

```sh
cargo run --release -p wordcraft -- --control 17991
cargo run --release -p wordcraft-cli -- mcp --connect 127.0.0.1:17991
```

## IR (`doc.getIr` / `get_ir`)

Version **2**. Blocks: `paragraph`, `table`, `picture` (picture-only paras). Optional: `from`/`to`, `fields`, `pageBreaksOnly`, `kinds`.

Paragraph fields include `style`, `text`, `align`, `indentFirst`/`Left`/`Right`, `lineSpacing`, `pageBreakBefore`, `numbering`, `role` (`heading`/`caption`/`figureCaption`/`tableCaption`/`toc`), `pictures[]`.

## Patch (`doc.applyIrPatch` / `apply_ir_patch`)

Ops: `update`, `replace`, `replaceIn`, `insert`, `delete`, `move`, `setStyle`, `setCell`.

```json
{"ops":[
  {"op":"insert","index":0,"block":{"type":"paragraph","text":"Title","style":"Heading1"}},
  {"op":"replaceIn","index":2,"find":"old","with":"new"},
  {"op":"setCell","index":5,"row":0,"col":1,"text":"B1"}
]}
```

## Layout / styles

| Command / MCP | Role |
|---|---|
| `doc.applyLayout` / `apply_layout` | Page, margins, Normal, Caption, Headings from `profile` (default = neutral A4/Calibri 11) |
| `doc.ensureStyles` / `ensure_styles` | Upsert stylesheet entries |
| `doc.ensureHeading` / `ensure_heading` | Heading 1–N |
| `doc.clearStrayPageBreaks` | Clear non-Heading1 page breaks |
| `doc.applyBodyStyle` | Alias of `applyLayout` |

Thesis margins/fonts: pass a profile JSON from a local skill — do not hardcode in the engine.

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

## Templates

`doc.newFromTemplate` `{id|template|path}` — built-in sample ids or a `.docx` path / file under `packaging/agent-ir/templates/`.
