# Clawdia

Clawdia Schiffer is a policy-governed AI activist agent that researches, verifies, and publishes anti-bottom-trawling campaign content. Built as an **MVP** to demonstrate agent-based workflows with authorization constraints.

## Status

This project is in early **MVP** stage. The current authorization model uses a simple deny-by-default permission list per agent. In the future, the [Cedarling](https://github.com/JanssenProject/jans/tree/main/jans-cedarling) authorization system will replace this with fine-grained Cedar policies governing what each agent can do, on which resources, and under what conditions.

## Architecture

Clawdia uses a multi-agent orchestration pattern:

- **Orchestrator** — coordinates the workflow: research, verify, write, publish
- **Researcher** — searches the web and adds cited statements to a campaign document
- **Verifier** — fact-checks each statement against its source and sets a verdict
- **Copywriter** — writes campaign content from verified statements

Agents communicate through a shared campaign document managed by the `campaign-doc` MCP server. Each agent is confined to a set of permitted tools (deny-by-default).

## Project Structure

```text
rust_tools/        CLI and agent runtime (Rust + rig framework)
mcp/campaign-doc/  Campaign document MCP server (Python + FastMCP)
```

## Quick Start

```bash
# Start the Chrome MCP bridge (optional, for browser-based content fetching)
# npx mcp-chrome-bridge --chrome-port=9222

# Run interactive chat
cd rust_tools
cp .env.example .env   # fill in DEEPSEEK_API_KEY
cargo run -- chat

# List available MCP tools
cargo run -- tools
cargo run -- tools --json

# List agent permissions
cargo run -- agents
```

## Future: Cedarling Authorization

The current MVP enforces agent permissions via static YAML config (`agents.yaml`). The planned integration with [Cedarling](https://github.com/JanssenProject/jans/tree/main/jans-cedarling) will provide:

- **Cedar policies** defining what each agent principal can do
- **Token-based identity** for agents and human approvers
- **Runtime policy evaluation** replacing the static permit list
- **Audit trail** with policy decision logs
- **Human-in-the-loop approval** for publishing, backed by OAuth tokens

## License

See [LICENSE](LICENSE).
