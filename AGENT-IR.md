# WordCraft Agent IR

Fork of [storytold/wordcraft](https://github.com/storytold/wordcraft) with a **generic Agent IR** layer: layout profiles, structural patches, figures, TOC, tables, and templates over MCP/CLI.

## Features

- `doc.getIr` (v2) / MCP `get_ir`
- `doc.applyIrPatch` — update / replaceIn / insert / delete / move / setStyle / setCell
- `doc.applyLayout` / `ensureStyles` / `ensureHeading`
- `doc.insertFigure`, `insertToc`, `updateFields`, `insertCrossRef`, …
- Cursor-oriented docs: [`docs/agent-ir.md`](docs/agent-ir.md)
- Windows pack: [`packaging/agent-ir/README.md`](packaging/agent-ir/README.md)

Domain rules (e.g. thesis layout) belong in a **local skill + profile JSON** that calls `apply_layout`, not in the engine.

## Download

[GitHub Releases](https://github.com/aclijpio/wordcraft-agent-ir/releases)

## Licence

Upstream WordCraft: MIT OR Apache-2.0. Agent IR changes are additive.
