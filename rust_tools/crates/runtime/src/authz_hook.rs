use std::sync::Arc;

use rig::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig::completion::CompletionModel;

use tools::authz::{
    AuditEntry, AuthorizationDecision, AuthorizationRequest, AuthorizationResult, PolicyContext,
    Principal,
};

use crate::agents::AuditLog;
use crate::cedar_authz::CedarAuthz;

/// Which authorization backend to use for tool-call decisions.
#[derive(Clone)]
pub enum AuthzBackend {
    /// Flat list from agents.yaml — fallback mode
    Yaml,
    /// Cedar policy engine — default
    Cedar(Arc<CedarAuthz>),
}

/// A PromptHook that enforces deny-by-default tool authorization.
///
/// Dispatches to either the YAML flat-list or Cedar policy engine
/// depending on `backend`.
#[derive(Clone)]
pub struct AuthzHook {
    principal: Principal,
    permitted_actions: Vec<String>,
    sub_agent_tools: Vec<String>,
    audit_log: AuditLog,
    backend: AuthzBackend,
}

impl AuthzHook {
    pub fn new(
        principal: Principal,
        permitted_actions: Vec<String>,
        audit_log: AuditLog,
        backend: AuthzBackend,
    ) -> Self {
        Self {
            principal,
            permitted_actions,
            sub_agent_tools: Vec::new(),
            audit_log,
            backend,
        }
    }

    /// Register sub-agent tool names that are always allowed.
    /// Sub-agents enforce their own permissions via their own AuthzHook.
    pub fn with_sub_agent_tools(mut self, tools: Vec<String>) -> Self {
        self.sub_agent_tools = tools;
        self
    }

    pub fn authorize(&self, tool_name: &str, args: &str) -> AuthorizationResult {
        // deny-by-default: only explicitly permitted tools and sub-agent tools are allowed
        let is_permitted = self.permitted_actions.iter().any(|t| t == tool_name)
            || self.sub_agent_tools.iter().any(|t| t == tool_name);

        let result = if is_permitted {
            AuthorizationResult {
                decision: AuthorizationDecision::Allow,
                reason: None,
            }
        } else {
            AuthorizationResult {
                decision: AuthorizationDecision::Deny,
                reason: Some(format!(
                    "Tool '{tool_name}' not in permitted tools for {}",
                    self.principal.id
                )),
            }
        };

        self.record_audit(tool_name, args, &result);

        result
    }

    /// Record an authorization decision in the audit log.
    fn record_audit(&self, tool_name: &str, args: &str, result: &AuthorizationResult) {
        let request = AuthorizationRequest {
            principal: self.principal.clone(),
            action: tool_name.to_string(),
            resource: tool_name.to_string(),
            context: PolicyContext {
                delegation_record_id: self.principal.delegation_record_id,
                requested_domain: extract_domain(args),
            },
        };

        self.audit_log
            .append(AuditEntry::from_request(&request, result));
    }
}

impl<M: CompletionModel> PromptHook<M> for AuthzHook {
    async fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        args: &str,
    ) -> ToolCallHookAction {
        // Sub-agent tools are always allowed (they enforce their own permissions)
        if self.sub_agent_tools.iter().any(|t| t == tool_name) {
            let result = AuthorizationResult {
                decision: AuthorizationDecision::Allow,
                reason: None,
            };
            self.record_audit(tool_name, args, &result);
            return ToolCallHookAction::cont();
        }

        let result = match &self.backend {
            AuthzBackend::Yaml => self.authorize(tool_name, args),
            AuthzBackend::Cedar(cedar) => {
                let parsed_args: serde_json::Value =
                    serde_json::from_str(args).unwrap_or(serde_json::Value::Null);
                let result = cedar
                    .authorize(&self.principal.id, tool_name, &parsed_args)
                    .await;
                self.record_audit(tool_name, args, &result);
                result
            }
        };

        match result.decision {
            AuthorizationDecision::Allow => ToolCallHookAction::cont(),
            AuthorizationDecision::Deny => {
                let reason = result
                    .reason
                    .unwrap_or_else(|| "denied by policy".to_string());
                ToolCallHookAction::skip(reason)
            }
        }
    }

    async fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        _args: &str,
        result: &str,
    ) -> HookAction {
        let preview = truncate_str(result, 100);
        println!(
            "  │  📎 [{principal}] tool {tool_name} returned: {preview}",
            principal = self.principal.id,
            preview = preview.replace('\n', " "),
        );
        HookAction::cont()
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let truncated: String = s.chars().take(max_chars).collect();
    let truncated = truncated.replace('\n', " ");
    if s.chars().count() > max_chars {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn extract_domain(args: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(args).ok()?;
    v.get("url")
        .or_else(|| v.get("domain"))
        .or_else(|| v.get("uri"))
        .and_then(|v| v.as_str())
        .and_then(|s| url_domain(s).or_else(|| Some(s.to_string())))
}

fn url_domain(s: &str) -> Option<String> {
    let after_scheme = s.strip_prefix("https://").or(s.strip_prefix("http://"))?;
    Some(after_scheme.split('/').next()?.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::AuditLog;
    use tools::authz::{AuthorizationDecision, Principal};

    fn make_hook(name: &str, permitted: &[&str]) -> AuthzHook {
        let principal = Principal {
            id: name.into(),
            principal_type: name.into(),
            delegation_record_id: None,
        };
        AuthzHook::new(
            principal,
            permitted.iter().map(|s| s.to_string()).collect(),
            AuditLog::new(),
            AuthzBackend::Yaml,
        )
    }

    // ── deny-by-default ──

    #[test]
    fn empty_permitted_actions_denies_everything() {
        let hook = make_hook("orchestrator", &[]);
        let result = hook.authorize("read_file", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Deny);
    }

    #[test]
    fn permitted_action_is_allowed() {
        let hook = make_hook("orchestrator", &["read_file"]);
        let result = hook.authorize("read_file", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Allow);
        assert!(result.reason.is_none());
    }

    #[test]
    fn unpermitted_action_is_denied() {
        let hook = make_hook("orchestrator", &["read_file"]);
        let result = hook.authorize("write_file", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Deny);
        assert!(result.reason.unwrap().contains("write_file"));
    }

    // ── sub-agent delegation ──

    fn make_hook_with_sub_agents(
        name: &str,
        permitted: &[&str],
        sub_agents: &[&str],
    ) -> AuthzHook {
        make_hook(name, permitted).with_sub_agent_tools(
            sub_agents.iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn registered_sub_agent_tool_is_allowed() {
        let hook = make_hook_with_sub_agents(
            "orchestrator",
            &[],
            &["agent_researcher", "agent_verifier"],
        );
        assert_eq!(
            hook.authorize("agent_researcher", "{}").decision,
            AuthorizationDecision::Allow
        );
        assert_eq!(
            hook.authorize("agent_verifier", "{}").decision,
            AuthorizationDecision::Allow
        );
    }

    #[test]
    fn unregistered_sub_agent_tool_is_denied() {
        let hook = make_hook_with_sub_agents(
            "orchestrator",
            &["doc_create"],
            &["agent_researcher"],
        );
        assert_eq!(
            hook.authorize("agent_copywriter", "{}").decision,
            AuthorizationDecision::Deny
        );
    }

    #[test]
    fn sub_agent_tools_do_not_bypass_regular_tool_deny() {
        let hook = make_hook_with_sub_agents(
            "orchestrator",
            &[],
            &["agent_researcher"],
        );
        assert_eq!(
            hook.authorize("doc_create", "{}").decision,
            AuthorizationDecision::Deny
        );
    }

    #[test]
    fn sub_agent_with_no_tools_cannot_call_anything() {
        let hook = make_hook("researcher", &[]);
        let result = hook.authorize("read_file", "{}");
        assert_eq!(result.decision, AuthorizationDecision::Deny);
    }

    #[test]
    fn sub_agent_confined_to_permitted_tools() {
        let hook = make_hook("researcher", &["search_sources", "fetch_article"]);

        assert_eq!(
            hook.authorize("search_sources", "{}").decision,
            AuthorizationDecision::Allow
        );
        assert_eq!(
            hook.authorize("fetch_article", "{}").decision,
            AuthorizationDecision::Allow
        );
        assert_eq!(
            hook.authorize("write_file", "{}").decision,
            AuthorizationDecision::Deny
        );
        assert_eq!(
            hook.authorize("publish_instagram_live", "{}").decision,
            AuthorizationDecision::Deny
        );
    }

    // ── audit log ──

    #[test]
    fn authorize_appends_to_audit_log() {
        let audit_log = AuditLog::new();
        let principal = Principal {
            id: "orchestrator".into(),
            principal_type: "orchestrator".into(),
            delegation_record_id: None,
        };
        let hook = AuthzHook::new(principal, vec!["read_file".into()], audit_log.clone(), AuthzBackend::Yaml);

        hook.authorize("read_file", "{}");
        hook.authorize("write_file", "{}");

        let entries = audit_log.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].decision, AuthorizationDecision::Allow);
        assert_eq!(entries[0].action, "read_file");
        assert_eq!(entries[1].decision, AuthorizationDecision::Deny);
        assert_eq!(entries[1].action, "write_file");
    }

    #[test]
    fn audit_log_records_principal_identity() {
        let audit_log = AuditLog::new();
        let principal = Principal {
            id: "researcher".into(),
            principal_type: "researcher".into(),
            delegation_record_id: None,
        };
        let hook = AuthzHook::new(principal, vec![], audit_log.clone(), AuthzBackend::Yaml);

        hook.authorize("some_tool", "{}");

        let entry = &audit_log.entries()[0];
        assert_eq!(entry.principal_id, "researcher");
        assert_eq!(entry.principal_type, "researcher");
    }

    // ── domain extraction ──

    #[test]
    fn extract_domain_from_url_arg() {
        let domain = extract_domain(r#"{"url": "https://fisheries.noaa.gov/article/123"}"#);
        assert_eq!(domain.as_deref(), Some("fisheries.noaa.gov"));
    }

    #[test]
    fn extract_domain_from_domain_arg() {
        let domain = extract_domain(r#"{"domain": "oceana.org"}"#);
        assert_eq!(domain.as_deref(), Some("oceana.org"));
    }

    #[test]
    fn extract_domain_returns_none_for_empty_args() {
        assert_eq!(extract_domain("{}"), None);
    }

    #[test]
    fn extract_domain_returns_none_for_invalid_json() {
        assert_eq!(extract_domain("not json"), None);
    }

    // ── truncation (UTF-8 safe) ──

    #[test]
    fn truncate_ascii() {
        assert_eq!(truncate_str("hello world", 5), "hello…");
    }

    #[test]
    fn truncate_no_op_when_short() {
        assert_eq!(truncate_str("hi", 10), "hi");
    }

    #[test]
    fn truncate_utf8_does_not_panic() {
        let ukrainian = "Донне тралення знищує екосистеми морського дна";
        let result = truncate_str(ukrainian, 10);
        assert_eq!(result, "Донне трал…");
    }

    #[test]
    fn truncate_replaces_newlines() {
        assert_eq!(truncate_str("line1\nline2\nline3", 100), "line1 line2 line3");
    }
}
