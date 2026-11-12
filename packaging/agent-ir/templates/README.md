# Agent templates

Built-in ids for `doc.newFromTemplate` / MCP `new_document`:

| id | Source |
|---|---|
| `blank` / `agent-blank` | `file.new` blank |
| `sample` | sample document |
| `letter` / `agent-letter` | letter sample |
| `resume` | resume sample |
| `report` / `agent-report` | report sample |

Custom `.docx` files placed here as `{id}.docx` are opened via `file.newFromTemplate` when `id` is requested. Keep assets original / CC0 only.
