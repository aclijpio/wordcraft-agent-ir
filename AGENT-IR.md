# WordCraft Agent IR

This repository is a fork of [storytold/wordcraft](https://github.com/storytold/wordcraft) with an **agent IR** layer: structural document patches and high-level VKR layout commands over MCP/CLI.

## Features

- `doc.getIr` / MCP `get_ir` — slim IR (`from`/`to`/`fields`/`pageBreaksOnly`)
- `doc.applyIrPatch` / `apply_ir_patch` — update/replace paragraphs by body index
- `doc.applyBodyStyle`, `doc.ensureHeading`, `doc.clearStrayPageBreaks`
- `doc.applyVkrLayout` / `apply_vkr_layout` — one-shot thesis layout
- Cursor skill: [`skills/wordcraft-vkr`](skills/wordcraft-vkr)
- Docs: [`docs/agent-ir.md`](docs/agent-ir.md)
- Windows release pack notes: [`packaging/agent-ir/README.md`](packaging/agent-ir/README.md)

## Download

See [GitHub Releases](https://github.com/aclijpio/wordcraft-agent-ir/releases) for a Windows zip with `wordcraft-cli.exe` and examples.

## Upstream

WordCraft remains MIT OR Apache-2.0. Agent IR changes are additive; ordinary MCP tools stay compatible.
