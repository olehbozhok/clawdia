//! Validates LLM-generated Cedar artifacts using the `cedar-policy` crate.

use anyhow::{Context, Result};
use cedar_policy::{PolicySet, Schema};

/// Compile a Cedar schema string. Returns the compiled schema for downstream
/// use in policy validation.
pub fn validate_schema(src: &str) -> Result<Schema> {
    Schema::from_cedarschema_str(src)
        .map(|(s, _warnings)| s)
        .context("invalid Cedar schema")
}

/// Parse a Cedar policy file (one or more policies) and validate it against
/// the schema. Returns `Ok(())` if all policies typecheck.
pub fn validate_policies(schema: &Schema, policies_src: &str) -> Result<()> {
    let policy_set: PolicySet = policies_src
        .parse()
        .context("failed to parse Cedar policies")?;

    let validator = cedar_policy::Validator::new(schema.clone());
    let result = validator.validate(&policy_set, cedar_policy::ValidationMode::Strict);

    if !result.validation_passed() {
        let errors: Vec<String> = result
            .validation_errors()
            .map(|e| e.to_string())
            .collect();
        anyhow::bail!("policy validation failed:\n{}", errors.join("\n"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SCHEMA: &str = r#"
namespace AgentPolicy {
  entity Agent = { agent_type: String };
  entity System;
  action "search" appliesTo {
    principal: [Agent],
    resource: [System],
    context: {}
  };
}
"#;

    const VALID_POLICY: &str = r#"
@description("researcher search")
permit(
  principal == AgentPolicy::Agent::"researcher",
  action == AgentPolicy::Action::"search",
  resource == AgentPolicy::System::"clawdia"
);
"#;

    #[test]
    fn valid_schema_compiles() {
        assert!(validate_schema(VALID_SCHEMA).is_ok());
    }

    #[test]
    fn broken_schema_fails() {
        assert!(validate_schema("namespace { broken").is_err());
    }

    #[test]
    fn valid_policy_passes() {
        let schema = validate_schema(VALID_SCHEMA).unwrap();
        assert!(validate_policies(&schema, VALID_POLICY).is_ok());
    }

    #[test]
    fn policy_referencing_unknown_action_fails() {
        let schema = validate_schema(VALID_SCHEMA).unwrap();
        let bad = r#"
permit(
  principal == AgentPolicy::Agent::"r",
  action == AgentPolicy::Action::"nonexistent",
  resource == AgentPolicy::System::"clawdia"
);
"#;
        assert!(validate_policies(&schema, bad).is_err());
    }

    #[test]
    fn malformed_policy_text_fails() {
        let schema = validate_schema(VALID_SCHEMA).unwrap();
        assert!(validate_policies(&schema, "not a policy").is_err());
    }
}
