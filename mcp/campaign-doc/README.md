# campaign-doc MCP Server

MCP server for managing campaign documents: create campaigns, add statements with citations, verify them, attach media, write content, assemble, and publish.

## Prerequisites

- Python 3.12+
- [uv](https://docs.astral.sh/uv/getting-started/installation/)

## Installation

```bash
cd /path/to/mcp/campaign-doc
uv sync
```

## Running the server

The server uses **stdio** transport. Run it directly:

```bash
uv run --project /path/to/mcp/campaign-doc \
  python /path/to/mcp/campaign-doc/server.py
```

## Configuring as an MCP server

### Claude Code (`~/.claude/settings.json` or project `.mcp.json`)

```json
{
  "mcpServers": {
    "campaign-doc": {
      "command": "uv",
      "args": [
        "run",
        "--project",
        "/path/to/mcp/campaign-doc",
        "python",
        "/path/to/mcp/campaign-doc/server.py"
      ]
    }
  }
}
```

### Custom MCP client (YAML config)

```yaml
servers:
  campaign-doc:
    transport: stdio
    command: uv
    args:
      - "run"
      - "--project"
      - "/path/to/mcp/campaign-doc"
      - "python"
      - "/path/to/mcp/campaign-doc/server.py"
    env: {}
```

## Storage

By default campaigns are stored **in memory** and lost when the server stops.

To persist campaigns as JSON files, set a storage directory via CLI flag or env var:

```bash
# CLI flag
uv run --project /path/to/mcp/campaign-doc \
  python /path/to/mcp/campaign-doc/server.py --storage-dir /path/to/campaigns

# Environment variable
export CAMPAIGN_DOC_STORAGE_DIR=/path/to/campaigns
uv run --project /path/to/mcp/campaign-doc \
  python /path/to/mcp/campaign-doc/server.py
```

Each campaign is saved as `camp-N.json` in the specified directory. Existing campaigns are loaded on startup.

For MCP config, pass the flag in `args` or the env var in `env`:

```json
{
  "mcpServers": {
    "campaign-doc": {
      "command": "uv",
      "args": [
        "run", "--project", "/path/to/mcp/campaign-doc",
        "python", "/path/to/mcp/campaign-doc/server.py",
        "--storage-dir", "/path/to/campaigns"
      ]
    }
  }
}
```

## Available tools

| Tool | Description |
|---|---|
| `doc_create` | Create a new campaign document |
| `doc_add_statement` | Add a statement with source URL citation |
| `doc_set_verdict` | Set verification verdict: `verified`, `rejected`, `needs_revision` |
| `doc_add_media` | Add image/video media reference |
| `doc_write_content` | Write headline, body, Instagram caption, CTA |
| `doc_assemble` | Assemble package from verified statements only |
| `doc_list` | List all campaigns (ID, topic, status, statement count) |
| `doc_status` | Get lightweight campaign metadata (counts and readiness flags) |
| `doc_get` | Get full campaign state with all texts and URLs |
| `doc_publish_draft` | Save assembled campaign as draft |
| `doc_publish_live` | Publish campaign live (requires assembled package) |

## Workflow

1. `doc_create` — create a campaign with a topic
2. `doc_add_statement` (repeat) — add statements with source URLs
3. `doc_set_verdict` — verify or reject each statement
4. `doc_add_media` — attach images/videos
5. `doc_write_content` — write headline, body, caption, CTA
6. `doc_assemble` — assemble the final package (only verified statements included)
7. `doc_publish_draft` or `doc_publish_live` — publish

Use `doc_status` to check progress at any point, or `doc_get` for full details.

## Development

```bash
cd /path/to/mcp/campaign-doc
uv run pytest
uv run pyright
```
