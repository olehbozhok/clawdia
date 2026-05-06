# Agent Configuration (`rust_tools/config/agents.yaml`)

- Every sub-agent **must** have `doc_list` in `permitted_actions` so it can discover campaigns on its own.
- Every sub-agent preamble **must** instruct the agent to use the `campaign_id` provided in the prompt and NEVER invent IDs. Include a fallback: "if no campaign_id is provided, use `doc_list` to find it."
- The orchestrator preamble **must** instruct it to always pass `campaign_id` (and `statement_ids` when relevant) in the prompt when delegating to sub-agents.
- When adding a new MCP tool that sub-agents need, add it to `permitted_actions` of each relevant agent — not to the Rust agent runtime code.
- Agent behaviour problems (loops, wrong IDs, missing context) are fixed in `agents.yaml` preambles and `mcp/campaign-doc/` tools, not in Rust wrapper code (`rust_tools/crates/runtime/`).
