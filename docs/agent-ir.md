# Agent IR and high-level apply

> **Last updated:** 2026-11-12 · Live-first: prefer the open app (`--control`) so edits show without reopening; slim IR filters; `doc.applyVkrLayout` one-shot.

Agents edit long documents through structural patches and a few bulk style ops, instead of dozens of low-level `execute` calls.

## Live-first (see changes without reopen)

Drive the **running app** so `engine.execute` mutates the same `Session` the window paints:

```sh
cargo run --release -p wordcraft -- --control 17890
cargo run --release -p wordcraft-cli -- mcp --connect 127.0.0.1:17890
```

Then MCP tools (`get_ir`, `apply_vkr_layout`, …) update the open document on screen. Do not use a separate headless session to edit a file that is already open in the app — that is a second copy; saving it forces a manual reopen.

Headless remains for offline batch / CI:

```sh
cargo run --release -p wordcraft-cli -- mcp
```

CLI:

```sh
cargo run --release -p wordcraft-cli -- run --file in.docx \
  --cmd doc.applyVkrLayout \
  --cmd 'doc.getIr={"pageBreaksOnly":true}' \
  --save out.docx
```

Installed ArtCraft MSI picks up these tools only after this code is in that build. Until then use workspace release binaries.

## IR (`doc.getIr` / MCP `get_ir`)

Version 1, derived from inspect — not a second document model.

Optional params:

| Param | Effect |
|---|---|
| `from` / `to` | Inclusive body index range |
| `fields` | Keep only these keys on each block (always keeps `index`, `type`) |
| `pageBreaksOnly` | Only paragraphs with `pageBreakBefore` or a Heading* style |

```json
{
  "version": 1,
  "pages": 12,
  "sections": [{"endBlock": 40, "margins": {"top": 56.7, "bottom": 56.7, "left": 85.05, "right": 42.55}, "page": {"w": 595.28, "h": 841.89}}],
  "blocks": [
    {
      "index": 0,
      "type": "paragraph",
      "style": "Heading1",
      "text": "Введение",
      "align": "left",
      "indentFirst": 35.45,
      "lineSpacing": 1.5,
      "pageBreakBefore": true,
      "numbering": {"num": 1, "level": 0},
      "listLevel": 0
    }
  ]
}
```

Block kinds: `paragraph`, `table` (cells as text only in v1). Patch identity is body `index` (stable until insert/delete).

## Patch example (`doc.applyIrPatch` / MCP `apply_ir_patch`)

```json
{
  "ops": [
    {"op": "update", "index": 0, "set": {"style": "Heading1", "pageBreakBefore": true, "text": "1 Постановка задачи"}},
    {"op": "update", "index": 3, "set": {"style": "Heading2", "pageBreakBefore": false}},
    {"op": "update", "index": 4, "set": {"align": "justify", "indentFirst": 35.45, "lineSpacing": 1.5, "pageBreakBefore": false}}
  ]
}
```

`replace` takes a full paragraph `block` (v1: paragraphs only).

Targeted word/sentence edit (keeps the rest of the paragraph):

```json
{"ops":[{"op":"replaceIn","index":17,"find":"заявками","with":"заказами","occurrence":1}]}
```

`update.set` may also include `indentLeft` / `indentRight` and `replaceIn`. Reply is a fresh IR plus `changed`. IR paragraphs may include `role` (`figureCaption` / `tableCaption` / `heading`).

`doc.applyVkrLayout` normalizes captions (`Рисунок…` centered, no first-line indent; `Таблица…` left, no first-line indent) and clears leftover left indents on Normal/headings so text does not stay shifted right after figures.

## High-level commands / MCP tools

| Engine command | MCP tool | Effect |
|---|---|---|
| `doc.getIr` | `get_ir` | Export IR (`from`/`to`/`fields`/`pageBreaksOnly`) |
| `doc.applyIrPatch` | `apply_ir_patch` | Structural patch by index |
| `doc.applyBodyStyle` | `apply_body_style` | A4; margins 30/15/20/20 mm; Normal = TNR 14, 1.5, justify, firstLine 35.45 pt; clear `pageBreakBefore` on non-Heading1 |
| `doc.ensureHeading` | `ensure_heading` | Heading1–N (default 3): TNR 14 bold black; multilevel `%1` / `%1.%2`; `pageBreakBefore` only on Heading1 |
| `doc.clearStrayPageBreaks` | `clear_stray_page_breaks` | Clear direct `pageBreakBefore` except Heading1 |
| `doc.applyVkrLayout` | `apply_vkr_layout` | One undo: clear stray breaks + body style + ensure headings |

Existing `execute` / `inspect_document` stay unchanged.

## Smoke (thesis-style, live)

1. Copy the `.docx` and open it in the workspace-built app (`--control 17890`).
2. `apply_vkr_layout`.
3. `get_ir` (`pageBreaksOnly` or full): Heading1/Heading2; body without stray breaks; `pages` not ≈ one per paragraph.
4. Confirm in the same window (`screenshot` / look at the app); then `save_document`.
