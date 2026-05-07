# Project Instructions

## Project Overview

Clawdia Schiffer is a policy-governed AI activist agent that researches, verifies, and publishes anti-bottom-trawling campaign content. It uses a multi-agent orchestration pattern (Orchestrator, Researcher, Verifier, Copywriter) communicating through a shared campaign document managed by the `campaign-doc` MCP server. Built with Rust (rig framework) for the agent runtime and Python (FastMCP) for the MCP server. Authorization is currently deny-by-default YAML config, with planned migration to Cedarling Cedar policies.

## Security

- NEVER read, display, or log the contents of `.env` files or any file containing secrets (API keys, passwords, tokens).
- NEVER include real secret values in code, comments, logs, or tool output.
- When creating `.env` files, always use placeholder values like `your-key-here`.
- If you need to reference environment variables, refer to them by name only (e.g. `DEEPSEEK_API_KEY`), never by value.

## Behaviour

- **This is a real production application.** Every feature, test, error path, and edge case must be fully implemented according to the plan and design doc. Cutting corners — skipping features, substituting simpler implementations without justification, leaving dead code paths, or deferring functionality without explicit approval — is not acceptable.
- **Follow the plan exactly.** If a plan specifies an approach (e.g. a specific library, data structure, or API), use that approach. If you believe a deviation is necessary, stop and explain the trade-off before proceeding. Do not silently substitute.
- **Re-read this file and `docs/agent/*.md` periodically**, especially at the start of a work session and after any interruption. The rules here are load-bearing — forgetting them leads to costly rework.
- **Flag every intentional deviation** from a plan or design doc in your response, with the reason. Do not assume small deviations are acceptable.

## Detailed Rules

See `docs/agent/*.md` for detailed guidelines:

- `docs/agent/env.md` — Environment files
- `docs/agent/code-quality.md` — Code quality standards
- `docs/agent/decisions.md` — Answering "which option is correct?"
- `docs/agent/deferring.md` — Deferring tasks
- `docs/agent/agent-config.md` — Agent configuration (agents.yaml)
- `docs/agent/validation.md` — Validation commands
- `docs/agent/adding-to-claude.md` — Adding information to CLAUDE.md or AGENTS.md
