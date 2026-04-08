# Cedarling Authorization Integration Design

## Summary

Integrate Jans Cedarling as the default authorization engine for agent tool calls, replacing the current flat `permitted_actions` YAML lists with Cedar policies. This enables full attribute-based access control (ABAC) with typed context per action, policy annotations, and audit logging. A dual-mode system allows fallback to YAML-based authorization per agent when explicitly configured.

Additionally, introduce a Workflow Advisor — a separate crate with a clean API that scans MCP tool schemas and generates Cedar policies, agent configurations, and schema extensions interactively with human approval.

## Current State

### Authorization flow

`AuthzHook` (in `rust_tools/crates/runtime/src/authz_hook.rs`) implements `PromptHook`. On each tool call it checks `permitted_actions.contains(tool_name)` — a flat allow-list loaded from `agents.yaml`. Everything not listed is denied (deny-by-default).

### Authorization types

`rust_tools/crates/tools/src/authz.rs` defines `Principal`, `AuthorizationRequest`, `AuthorizationResult`, `AuditEntry`. The `AuthorizationRequest` already has `PolicyContext` with `delegation_record_id` and `requested_domain`.

### Agent configuration

`rust_tools/config/agents.yaml` defines orchestrator + sub-agents, each with `permitted_actions: [tool_name, ...]`.

### Limitations

- No conditional logic (cannot express "only if campaign is in X status")
- No typed context (domain extraction is hardcoded in Rust)
- No policy annotations or descriptions
- Adding a tool requires editing YAML, no validation against actual MCP tool schemas

## Design

### 1. Cedar Schema (`AgentPolicy` namespace)

```cedarschema
namespace AgentPolicy {
  entity Agent = {
    agent_type: String,
  };

  entity Tool = {
    tool_type: String,   // "mcp", "builtin", "sub_agent"
    domain: String,      // "campaign", "web", "chrome", "agent"
  };

  // Campaign entity — deferred to future work (requires scripted context enrichment)
  // entity Campaign = {
  //   status: String,
  //   verified_count: Long,
  // };

  // Actions — one per tool group, each with typed context
  // Context types are generated from MCP tool JSON schemas

  action "web_fetch" appliesTo {
    principal: [Agent],
    resource: [Tool],
    context: { requested_domain: String }
  };

  action "campaign_write" appliesTo {
    principal: [Agent],
    resource: [Tool],
    context: { campaign_id: String }
  };

  action "campaign_read" appliesTo {
    principal: [Agent],
    resource: [Tool],
    context: { campaign_id: String }
  };
}
```

Actions are grouped by tool domain, not one-per-tool. Each action has a typed context derived from MCP tool JSON schemas at generation time. The Workflow Advisor generates and updates this schema.

### 2. Policy Store Structure

```
rust_tools/config/policies/
├── metadata.json
├── schema.cedarschema
├── policies/
│   ├── orchestrator.cedar
│   ├── researcher.cedar
│   ├── verifier.cedar
│   └── copywriter.cedar
└── entities/
    ├── agents.json
    └── tools.json
```

- **One `.cedar` file per agent** — all policies for that agent in one place
- **Every policy has `@description` and `@tool` annotations**
- **`entities/`** — static entity data for agents and tools
- **`schema.cedarschema`** — generated from MCP tool schemas by the Advisor

### 3. Policy Examples

`policies/researcher.cedar`:

```cedar
@description("Researcher can search the web for marine conservation sources")
@tool("search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"search"
);

@description("Researcher can fetch page content from approved domains")
@tool("fetch_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"fetch_content"
) when {
  context.requested_domain == "fisheries.noaa.gov" ||
  context.requested_domain == "oceana.org" ||
  context.requested_domain == "worldwildlife.org"
};

@description("Researcher can use browser to bypass bot detection")
@tool("chrome_get_web_content")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"web_fetch",
  resource == AgentPolicy::Tool::"chrome_get_web_content"
);

@description("Researcher can add statements to campaigns")
@tool("doc_add_statement")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_write",
  resource == AgentPolicy::Tool::"doc_add_statement"
);

@description("Researcher can list campaigns")
@tool("doc_list")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list"
);

@description("Researcher can list statements to avoid duplicates")
@tool("doc_list_statements")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_list_statements"
);

@description("Researcher can check campaign status")
@tool("doc_status")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_status"
);

@description("Researcher can read campaign data")
@tool("doc_get")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"campaign_read",
  resource == AgentPolicy::Tool::"doc_get"
);
```

Entity file `entities/agents.json`:

```json
[
  {
    "uid": { "type": "AgentPolicy::Agent", "id": "orchestrator" },
    "attrs": { "agent_type": "orchestrator" },
    "parents": []
  },
  {
    "uid": { "type": "AgentPolicy::Agent", "id": "researcher" },
    "attrs": { "agent_type": "researcher" },
    "parents": []
  },
  {
    "uid": { "type": "AgentPolicy::Agent", "id": "verifier" },
    "attrs": { "agent_type": "verifier" },
    "parents": []
  },
  {
    "uid": { "type": "AgentPolicy::Agent", "id": "copywriter" },
    "attrs": { "agent_type": "copywriter" },
    "parents": []
  }
]
```

### 4. Dual-Mode AuthzHook

```rust
pub enum AuthzBackend {
    /// Flat list from agents.yaml — fallback mode
    Yaml,
    /// Cedar policy engine — default
    Cedar(Arc<Cedarling>),
}
```

`AuthzHook` keeps its existing `permitted_actions: Vec<String>` field. The `backend` enum determines which path `authorize()` takes:

- **`Yaml`** — existing logic: `permitted_actions.contains(tool_name)`
- **`Cedar`** — builds `RequestUnsigned` from principal (agent), resource (tool), action (tool group), context (extracted from args), calls `cedarling.authorize_unsigned()`

Selection in `agents.yaml`:

```yaml
orchestrator:
  name: orchestrator
  authz_mode: cedarling   # default if omitted
  preamble: ...

researcher:
  name: researcher
  authz_mode: yaml        # explicit fallback
  permitted_actions:
    - search
    - fetch_content
```

If `authz_mode` is not specified, Cedarling is used. If Cedarling mode is active but no policies match the agent, all tool calls are denied (deny-by-default).

### 5. Cedarling Initialization and Reload

**Initialization at startup:**

1. Runtime reads `agents.yaml` — determines `authz_mode` per agent
2. If any agent uses `cedarling` — loads policy store from `rust_tools/config/policies/`
3. Creates `Arc<Cedarling>` via `Cedarling::new(BootstrapConfig { ... })` with:
   - `PolicyStoreSource::Local(policy_store_path)` — path is configurable (see below)
   - `jwt_sig_validation: false` (no JWTs — using `authorize_unsigned`)
   - `log_level` from config
4. Each agent's `AuthzHook` receives `AuthzBackend::Cedar(arc.clone())` or `AuthzBackend::Yaml`

**Policy store path configuration:**

Configurable via `agents.yaml` top-level field or CLI arg, with sensible default:

```yaml
cedarling:
  policy_store_path: "./config/policies"  # default
  log_level: info
```

Or via CLI: `clawdia --policy-store ./my-policies run`

Or via env: `CLAWDIA_POLICY_STORE_PATH=./my-policies`

**Reload without process restart:**

1. Triggered by CLI command (`clawdia reload-policies`) or signal
2. Runtime stops current agent tasks
3. Creates new `Cedarling` instance from updated policy store
4. Replaces `Arc<Cedarling>` — old instance drops when last reference is freed
5. Resumes operation

### 6. Workflow Advisor Crate

**Location:** `rust_tools/crates/advisor/`

**Structure:**

```
advisor/
└── src/
    ├── lib.rs            # public API: AdvisorService
    ├── discovery.rs      # MCP tool schema scanning
    ├── generator.rs      # policy, entity, schema generation
    └── recommender.rs    # workflow analysis and agent/tool recommendations
```

**Public API (`lib.rs`):**

```rust
pub struct AdvisorService { ... }

impl AdvisorService {
    /// Connect to MCP servers and discover all available tools with schemas
    pub async fn discover_tools(&self) -> Result<Vec<ToolSchema>>;

    /// Given a workflow description, recommend agents and their tools
    pub fn recommend(&self, workflow: &str, tools: &[ToolSchema]) -> Recommendation;

    /// Generate Cedar policy files, entities, schema from a Recommendation
    pub fn generate(&self, recommendation: &Recommendation) -> GeneratedArtifacts;

    /// Preview changes as a diff
    pub fn preview(&self, artifacts: &GeneratedArtifacts) -> String;

    /// Apply artifacts to filesystem (after human approval)
    pub fn apply(&self, artifacts: &GeneratedArtifacts) -> Result<()>;
}
```

**Key behaviors:**

- **Discovery:** connects to all configured MCP servers, calls `list_tools`, collects tool names + JSON schemas. This is the only "hardcoded" permission — advisor can always list tools.
- **Generation:** creates `.cedar` files with `@description` and `@tool` annotations, updates `schema.cedarschema` with context types derived from tool JSON schemas, generates entity JSON files.
- **Human-in-the-loop:** `generate()` produces artifacts in memory, `preview()` shows a diff, `apply()` writes to disk only after human confirms.

**CLI integration** (`crates/cli/`): thin wrapper — `clawdia advisor` subcommand.

**Future:** same `AdvisorService` API is used by web server endpoint.

### 7. Policy-Aware System Prompts

When an agent is launched, the runtime reads all Cedar policies for that agent, extracts `@description` and `@tool` annotations, and appends a permissions summary to the agent's system prompt. This lets the LLM understand its constraints before making tool calls.

Example addition to researcher's system prompt:

```
## Your permissions

You are authorized to use the following tools:
- search — Search the web for marine conservation sources
- fetch_content — Fetch page content from approved domains (fisheries.noaa.gov, oceana.org, worldwildlife.org)
- chrome_get_web_content — Use browser to bypass bot detection
- doc_add_statement — Add statements to campaigns
- doc_list — List campaigns
- doc_list_statements — List statements to avoid duplicates
- doc_status — Check campaign status
- doc_get — Read campaign data

Any tool call outside this list will be denied.
```

The runtime builds this by iterating policies where `principal == AgentPolicy::Agent::"<agent_name>"`, collecting `@tool` and `@description` values. For policies with `when` clauses, the description should mention the constraint (e.g. "from approved domains").

This replaces the implicit knowledge from `permitted_actions` — the LLM gets explicit, human-readable context about what it can and cannot do.

### 8. Action-to-Tool Mapping

The runtime needs to know which Cedar action corresponds to a given tool. This mapping is derived from the `@tool` annotations in policy files and stored as a lookup:

```
tool_name → Cedar action
```

Example:
- `search` → `AgentPolicy::Action::"web_fetch"`
- `fetch_content` → `AgentPolicy::Action::"web_fetch"`
- `doc_add_statement` → `AgentPolicy::Action::"campaign_write"`
- `doc_list` → `AgentPolicy::Action::"campaign_read"`

This mapping is generated by the Advisor as `rust_tools/config/policies/tool_action_map.json`:

```json
{
  "search": "web_fetch",
  "fetch_content": "web_fetch",
  "chrome_get_web_content": "web_fetch",
  "doc_add_statement": "campaign_write",
  "doc_list": "campaign_read",
  "doc_list_statements": "campaign_read",
  "doc_create": "campaign_write",
  "doc_get": "campaign_read"
}
```

Loaded at startup alongside the policy store. When `AuthzHook` receives a tool call, it looks up the action from this mapping, then constructs the full Cedar action name (`AgentPolicy::Action::"<action>"`).

If a tool has no mapping entry, the authorization request is denied (deny-by-default).

### 8. Context Extraction

For the first iteration, context is extracted from tool call `args` JSON:

- **`requested_domain`** — extracted from `url`, `domain`, or `uri` fields (existing `extract_domain` logic in `authz_hook.rs`)
- **`campaign_id`** — extracted from `campaign_id` field in args

Context fields are optional. If a policy checks `context.requested_domain` but the tool call args don't contain a URL, the condition fails and the policy doesn't match (deny by default unless another policy permits).

## Future Work (TODO)

- **Scripted context enrichment** — allow LLM-generated scripts (in a sandboxed scripting language) to define how context is populated per tool call, enabling richer authz checks like campaign status, verified statement counts, etc. without hardcoding context extraction in Rust.
- **Web server integration** — expose Advisor API and policy reload via HTTP endpoints.
- **Policy validation** — Advisor validates generated policies against schema before writing.

## Dependencies

- `cedarling` crate from `jans-cedarling/cedarling` (Rust, in-process)
- Existing: `rig`, `tools`, `serde`, `tokio`
