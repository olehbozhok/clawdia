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
- **Follow the plan exactly. The plan is a specification, not a suggestion.** If a plan specifies an approach (e.g. a specific library, data structure, or API), use that approach. Treat each code block in the plan as authoritative unless you have tested it and confirmed it does not compile — in which case stop and ask, do not silently substitute.
- **Re-read this file, the current plan, and `docs/agent/*.md` at the start of every work session and after any interruption.** The rules here are load-bearing — forgetting them leads to costly rework.
- **Flag every intentional deviation** from a plan or design doc in your response, with the reason. Do not assume small deviations are acceptable.
- **When you hit friction** (an API doesn't work as expected, a dependency is missing, a test fails): stop, investigate the correct solution, and report the issue. Do not "make it work with what you have" by substituting a simpler approach that the plan didn't authorize.
- **Do not ship incomplete features.** If you commit code and know a feature is partially implemented (e.g. a keybinding that maps to an action but the event loop ignores it, a struct field that nothing reads, a tool that is declared but not wired), it is not done. Finish it or do not commit it.
- **Do not redesign the architecture.** The plan specifies why things are where they are. If you think a field is redundant (e.g. `RuntimeGlue.hmac_key` when the gateway already has it), the plan has a reason — follow it. You can flag the redundancy in review, but implement what is specified.
- **Enumerate all sub-tasks before starting a plan task.** For every task, list every file change, every integration point, every test case — before writing any code. A task like "Wire resolvers at call sites" must be decomposed into individual sub-tasks (e.g. "add RuntimeConfig param to build_agent", "wire resolve_session_ttl into session creation", etc.) and each must be checked off independently.
- **Cross-check against the plan's checklist after "completing" a task.** Re-read the plan's task definition, verify every checkbox is accounted for, and confirm no sub-task was silently skipped. Green tests are necessary but not sufficient — they only prove the implemented subset works, not that the task is fully done.
- **When the plan references an undefined or underspecified type** (e.g. `AgentsMap` with no definition, or a struct that doesn't match the real data format), stop and ask. Do not improvise a substitute and build the rest of the system on top of it — this creates plan drift from the first deviation.

## Detailed Rules

## Known Failure Patterns (learned from experience)

These are specific traps this project has fallen into before. Learn to recognize them:

- **"Good enough" substitution.** When a planned library or component is harder to use than expected, the temptation is to replace it with something simpler (e.g. `String` instead of `tui_input::Input`). Don't. Figure out the correct API or ask. Planned dependencies were chosen for a reason.
- **Dead action arms.** Declaring an `Action` variant in `keymap.rs` and a keybinding for it, but never handling it in the event loop (`commands.rs`). This includes partial implementations where the keypress is dispatched but the result is silently ignored. Every dispatch arm must have a handler.
- **Skipping tests because "it's just plumbing."** Event loop integration, reducer correctness, and state transitions must all be tested. If a test is awkward to write, that's a sign the architecture is wrong.
- **"I know better than the plan" refactors.** Removing a field from a struct because "it's redundant" or restructuring module layout "for clarity" without the plan authorizing it. The plan sets the architecture; code follows.
- **Plan drift.** Making the first task deviate from the plan (e.g. `String` instead of `tui_input::Input`), then building the rest of the system on top of that deviation. By the end, the gap is large and hard to undo. The first deviation is the most important to catch.
- **Silent schema adaptation.** When the plan references an undefined type (e.g. `AgentsMap` with no definition) or a struct that doesn't match real data formats, the temptation is to improvise a substitute and keep building. Don't — stop and ask how to reconcile the plan's schema with reality. Building on top of an improvised foundation guarantees plan drift from the first step.
- **Partial integration / incomplete wiring.** A task that specifies N integration points (e.g. "wire resolvers into agent/session/spawn/approval") is not done when M < N are wired. Each unwired point is a dead code path. Before marking a task complete, enumerate every wire point and verify each has a call site. If a wire point is genuinely blocked, defer it explicitly with scope + reason per `docs/agent/deferring.md`.
- **Green-tests false completion.** Passing tests prove only that the implemented subset works — they do NOT prove the task is fully done. A resolver can have 18 green tests covering its logic while 2 of 4 call sites remain unwired. Always cross-check test coverage against the plan's requirements, not just against the code you wrote.
- **Skipping the pre-flight ritual.** The rule says "re-read CLAUDE.md, the plan, and docs/agent/*.md at start of every session." Skipping this leads to forgotten rules, undetected plan drift, and violations that compound. This is not optional overhead — it is the mechanism that keeps the implementation aligned with the design. Make it the very first action after receiving a task.

## Detailed Rules

See `docs/agent/*.md` for detailed guidelines:

- `docs/agent/env.md` — Environment files
- `docs/agent/code-quality.md` — Code quality standards
- `docs/agent/decisions.md` — Answering "which option is correct?"
- `docs/agent/deferring.md` — Deferring tasks
- `docs/agent/agent-config.md` — Agent configuration (agents.yaml)
- `docs/agent/validation.md` — Validation commands
- `docs/agent/adding-to-claude.md` — Adding information to CLAUDE.md or AGENTS.md
