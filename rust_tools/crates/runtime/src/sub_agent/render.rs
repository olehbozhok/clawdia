//! Render `SubAgentFinished` system messages for LLM context (design §16).

use crate::approvals::types::TicketId;
use crate::inbox::SystemMsg;
use crate::sub_agent::outcome::{AbandonReason, FailureKind, SubAgentOutcome};

pub fn render_for_llm(msg: &SystemMsg) -> String {
    match msg {
        SystemMsg::SubAgentFinished {
            child_session_id,
            agent_label,
            outcome,
            ..
        } => {
            let header = format!("child <{}/{}>", agent_label, child_session_id.as_str());
            match outcome {
                SubAgentOutcome::Done { summary, result: _ } => {
                    format!("{header} DONE: {summary}")
                }
                SubAgentOutcome::Failed {
                    kind,
                    message,
                    suggested_action,
                } => {
                    let kind_str = render_kind(kind);
                    let mut out = format!("{header} FAILED: {kind_str}\n  message: {message:?}");
                    if let Some(s) = suggested_action {
                        out.push_str(&format!("\n  suggested: {s:?}"));
                    }
                    out
                }
                SubAgentOutcome::Abandoned { reason } => {
                    format!("{header} ABANDONED: {}", render_abandon(reason))
                }
                SubAgentOutcome::Cancelled => format!("{header} CANCELLED"),
            }
        }
        SystemMsg::ApprovalDecided { .. }
        | SystemMsg::UserMessage { .. }
        | SystemMsg::DeadlineWarning { .. } => crate::inbox::render(msg),
    }
}

fn render_kind(k: &FailureKind) -> String {
    match k {
        FailureKind::MissingTool { name } => format!("MissingTool({name:?})"),
        FailureKind::InsufficientPermission { action } => {
            format!("InsufficientPermission({action:?})")
        }
        FailureKind::AmbiguousRequest { question } => format!("AmbiguousRequest({question:?})"),
        FailureKind::ExternalError { source } => format!("ExternalError({source:?})"),
        FailureKind::HumanInputRequired { ticket_id } => {
            let TicketId(id) = ticket_id;
            format!("HumanInputRequired({id:?})")
        }
        FailureKind::Unknown => "Unknown".to_string(),
    }
}

fn render_abandon(r: &AbandonReason) -> &'static str {
    match r {
        AbandonReason::Ttl => "Ttl",
        AbandonReason::ParentCancel => "ParentCancel",
        AbandonReason::DenialTerminal => "DenialTerminal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::SessionId;
    use crate::sub_agent::outcome::FailureKind;

    #[test]
    fn render_failed_missing_tool_matches_design_example() {
        let msg = SystemMsg::SubAgentFinished {
            child_session_id: SessionId::for_test("sid_42"),
            agent_label: "agent_researcher".into(),
            outcome: SubAgentOutcome::Failed {
                kind: FailureKind::MissingTool {
                    name: "fetch_pdf".into(),
                },
                message: "Source is a PDF; no extraction tool available.".into(),
                suggested_action: Some("Skip this source and use the HTML alternative.".into()),
            },
            finished_at: std::time::Instant::now(),
        };
        let s = render_for_llm(&msg);
        assert_eq!(
            s,
            r#"child <agent_researcher/sid_42> FAILED: MissingTool("fetch_pdf")
  message: "Source is a PDF; no extraction tool available."
  suggested: "Skip this source and use the HTML alternative.""#
        );
    }

    #[test]
    fn render_done_includes_summary() {
        let msg = SystemMsg::SubAgentFinished {
            child_session_id: SessionId::for_test("sid_7"),
            agent_label: "agent_verifier".into(),
            outcome: SubAgentOutcome::Done {
                summary: "ok".into(),
                result: "{}".into(),
            },
            finished_at: std::time::Instant::now(),
        };
        let s = render_for_llm(&msg);
        assert!(s.contains("DONE"));
        assert!(s.contains("ok"));
    }
}
