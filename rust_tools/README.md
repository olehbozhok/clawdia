# rust_tools

Agent runtime and CLI for Clawdia Schiffer — built with Rust and the [rig](https://github.com/0xPlaygrounds/rig) agent framework.

## Status

**MVP** — the agent orchestration and tool authorization work end-to-end, but the authorization model is a static permit list. Future versions will integrate [Cedarling](https://github.com/JanssenProject/jans/tree/main/jans-cedarling) for Cedar-policy-based runtime authorization.

## Structure

```text
crates/
  cli/        CLI entry point (chat, tools, agents commands)
  runtime/    Agent building, MCP connection, authorization hook
  tools/      Authorization types, audit log, approval records
  ui/         Terminal UI utilities
config/
  agents.yaml        Agent definitions and permitted actions
  mcp_servers.yaml   MCP server connection config
```

## Environment Variables

Copy `.env.example` to `.env` and fill in the values:

```bash
cp .env.example .env
```

| Variable           | Description                                  | Default                  |
| ------------------ | -------------------------------------------- | ------------------------ |
| `DEEPSEEK_API_KEY` | DeepSeek API key (required)                  | —                        |
| `DEEPSEEK_MODEL`   | Model name                                   | `deepseek-chat`          |
| `MCP_CONFIG`       | Path to MCP servers config                   | `config/mcp_servers.yaml`|
| `AGENTS_CONFIG`    | Path to agents config                        | `config/agents.yaml`     |
| `LOG_LEVEL`        | Log level (trace/debug/info/warn/error/off)  | `info`                   |

## Usage

### TUI (interactive chat)

Three-pane terminal UI with ratatui:

- **Chat** — agent transcript, user input, notifications
- **Approvals** — pending approval tickets with approve/deny/skip
- **Log** — runtime tracing events with severity colors

Keybindings: `Tab` to cycle panes, `j`/`k` to navigate lists, `a`/`d`/`s` to approve/deny/skip in Approvals pane.

```bash
# Interactive chat with the orchestrator (TUI)
cargo run -- chat
```

### CLI commands

```bash
# List MCP tools (human-readable)
cargo run -- tools

# List MCP tools with full JSON schemas
cargo run -- tools --json

# List agents and their permissions
cargo run -- agents

# Set log level (trace, debug, info, warn, error, off)
cargo run -- --log-level warn chat
```

## Configuration

### `config/agents.yaml`

Defines the orchestrator and sub-agents. Each agent has:

- **preamble** — system prompt with behavioral instructions
- **permitted_actions** — deny-by-default list of allowed MCP tool names

### `config/mcp_servers.yaml`

Defines MCP server connections (stdio or HTTP transport).

## Authorization Model

Currently, each agent has a static list of permitted tool names. The `AuthzHook` intercepts every tool call and checks it against this list. Unauthorized calls are denied and logged to an audit trail.

### Future: Cedarling

The static permit list will be replaced by [Cedarling](https://github.com/JanssenProject/jans/tree/main/jans-cedarling), which evaluates Cedar policies at runtime. This will enable:

- Fine-grained policies (e.g. "researcher can add statements only based on policies)
- Token-based agent identity (OAuth2/OIDC)
- Human approval workflows for publishing (backed by `id_token` verification)
- Domain-level access control (e.g. restrict which source domains agents can fetch)
