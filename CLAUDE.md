# Project Instructions

## Project Overview

Clawdia Schiffer is a policy-governed AI activist agent that researches, verifies, and publishes anti-bottom-trawling campaign content. It uses a multi-agent orchestration pattern (Orchestrator, Researcher, Verifier, Copywriter) communicating through a shared campaign document managed by the `campaign-doc` MCP server. Built with Rust (rig framework) for the agent runtime and Python (FastMCP) for the MCP server. Authorization is currently deny-by-default YAML config, with planned migration to Cedarling Cedar policies.

## Security

- NEVER read, display, or log the contents of `.env` files or any file containing secrets (API keys, passwords, tokens).
- NEVER include real secret values in code, comments, logs, or tool output.
- When creating `.env` files, always use placeholder values like `your-key-here`.
- If you need to reference environment variables, refer to them by name only (e.g. `DEEPSEEK_API_KEY`), never by value.

## Detailed Rules

See `docs/agent/*.md` for detailed guidelines:

- `docs/agent/env.md` — Environment files
- `docs/agent/code-quality.md` — Code quality standards
- `docs/agent/decisions.md` — Answering "which option is correct?"
- `docs/agent/deferring.md` — Deferring tasks
- `docs/agent/agent-config.md` — Agent configuration (agents.yaml)
- `docs/agent/validation.md` — Validation commands
- `docs/agent/adding-to-claude.md` — Adding information to CLAUDE.md or AGENTS.md
