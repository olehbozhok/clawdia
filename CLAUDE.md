# Project Instructions

## Project Overview

Clawdia Schiffer is a policy-governed AI activist agent that researches, verifies, and publishes anti-bottom-trawling campaign content. It uses a multi-agent orchestration pattern (Orchestrator, Researcher, Verifier, Copywriter) communicating through a shared campaign document managed by the `campaign-doc` MCP server. Built with Rust (rig framework) for the agent runtime and Python (FastMCP) for the MCP server. Authorization is currently deny-by-default YAML config, with planned migration to Cedarling Cedar policies.

## Security

- NEVER read, display, or log the contents of `.env` files or any file containing secrets (API keys, passwords, tokens).
- NEVER include real secret values in code, comments, logs, or tool output.
- When creating `.env` files, always use placeholder values like `your-key-here`.
- If you need to reference environment variables, refer to them by name only (e.g. `DEEPSEEK_API_KEY`), never by value.

## Environment Files

- Always maintain an `.env.example` file alongside any `.env` file in the project.
- `.env.example` must contain all required environment variable names with placeholder values (e.g. `API_KEY=your-api-key-here`).
- When adding, removing, or renaming environment variables in code, immediately update `.env.example` to keep it in sync.
- When writing code that reads environment variables, validate that all required variables are set and non-empty at startup. Provide a clear error message listing any missing variables.
- All environment variables must be read in `main` (or the top-level entry point) and passed into functions as explicit parameters. Functions must never call `std::env::var` or equivalent directly — this keeps the source of configuration obvious and testable. In Rust CLI apps using clap, prefer `#[arg(env = "VAR_NAME")]` to read env vars declaratively in the CLI struct rather than manual `std::env::var` calls.

## Code Quality

- Follow best practices for the language being used (idiomatic Rust, Pythonic Python, etc.).
- Always evaluate whether a piece of functionality should be extracted into a separate module, function, or file. Consider separation of concerns, reusability, and readability.
- Keep functions and modules focused on a single responsibility.
- Prefer clear, descriptive naming over comments.
- Prefer named structs over tuples for return types and parameters when there are 2+ fields. Tuples like `(ServerSink, Vec<Tool>)` are opaque — use a struct with named fields instead.
- In Rust, never use `Box<dyn std::error::Error>` for error handling. Use `thiserror` for library error types and `anyhow::Result` for application-level code.
- Never use byte-index slicing (`&s[..n]`) on strings. This panics on multi-byte UTF-8 characters. Always use `.chars().take(n)` or `char_indices` for truncation.
- Any background async task (`tokio::spawn` with an unbounded loop, periodic sweep, watcher) **must** accept a `tokio_util::sync::CancellationToken` and exit on `cancel.cancelled()` via `tokio::select!`. Returning a bare `JoinHandle` without a cancel path forces callers to `abort()` and leaks in-flight work. Test the cancel path with a bounded `tokio::time::timeout`.

## Answering "which option is correct?"

- When the user asks "which is correct / right / better?", answer the question they asked — correctness, security, architectural soundness — not "which is cheapest to implement on top of the current code". These are different criteria and yield different answers.
- Before answering, **check authoritative sources**: any written plan, design doc, ADR, prior decision in CLAUDE.md, threat model, or invariants encoded in existing code/tests. These hold context that isn't visible from a local code reading.
- If the correct option diverges from the cheap/minimal option, surface BOTH explicitly: "correct = X (reason / invariant / source); cheaper alternative = Y (tradeoff Z); recommend X." Never silently substitute one criterion for another.
- A recommendation that contradicts the documented decision without acknowledging it is a bug — it changes the question the user thought they were asking.

## Deferring tasks

- Before deferring a plan task as "blocked on something later", split it into sub-parts and defer ONLY the parts actually blocked. A task that is "implement X struct + attach X to build_agent" should usually become "implement X (now) + attach X (deferred)", not "defer the whole thing".
- Cross-check against prior work in the same repo: if an analogous task was implemented as a self-contained struct + tests with the attach step deferred (e.g. `approval_*` tools in `approvals/tools.rs` while the wiring sits unwired in `agents.rs`), follow that precedent. Diverging from it without a reason creates inconsistent partial implementations that are harder to wire up later.
- When you defer, be explicit about the scope of the deferral (which sub-task, which plan owns the follow-up) — both in the commit message and in a doc comment on whatever you DID land. "Deferred to Plan 06" alone is not enough; say what specifically.
- **Surface every deferral to the user, before committing**, with a concrete reason that survives scrutiny. Acceptable reasons: missing dependency owned by another plan/PR, requires a destructive refactor outside scope, blocked on a decision the user has not made. Unacceptable: "feels like scope creep", "saves time", "I'll do it later". If you cannot state a reason that the user would accept on its own, do the work — do not defer.

## Agent Configuration (`rust_tools/config/agents.yaml`)

- Every sub-agent **must** have `doc_list` in `permitted_actions` so it can discover campaigns on its own.
- Every sub-agent preamble **must** instruct the agent to use the `campaign_id` provided in the prompt and NEVER invent IDs. Include a fallback: "if no campaign_id is provided, use `doc_list` to find it."
- The orchestrator preamble **must** instruct it to always pass `campaign_id` (and `statement_ids` when relevant) in the prompt when delegating to sub-agents.
- When adding a new MCP tool that sub-agents need, add it to `permitted_actions` of each relevant agent — not to the Rust agent runtime code.
- Agent behaviour problems (loops, wrong IDs, missing context) are fixed in `agents.yaml` preambles and `mcp/campaign-doc/` tools, not in Rust wrapper code (`rust_tools/crates/runtime/`).

## Validation

- Always use `uv run pyright` to validate Python code instead of running the app, when possible.
- Fix all pyright errors before considering the task complete.
