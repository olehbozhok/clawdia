use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use runtime::approvals::outcome::{ApprovalOutcome, ApproverIdentity, ApproverKind};
use runtime::approvals::types::{Ticket as RuntimeTicket, TicketId, TicketStatus};
use runtime::persistence::notifications::NotificationStore;
use runtime::persistence::{Inbox, SessionStore};
use runtime::sessions::SessionId;
use tokio_util::sync::CancellationToken;

use super::AppEvent;
use super::approvals_pane::{ApprovalEvent, Ticket};
use super::chat_pane::ChatMsg;
use super::keymap::ApprovalChoice;

#[derive(Clone)]
pub struct RuntimeGlue {
    pub gateway: Arc<runtime::approvals::gateway::ApprovalGateway>,
    pub sessions: Arc<dyn SessionStore>,
    pub inbox: Arc<dyn Inbox>,
    pub notifications: Arc<dyn NotificationStore>,
    pub local_key_id: String,
    pub local_roles: Vec<String>,
    pub root_session: SessionId,
}

fn to_tui_ticket(rt: &RuntimeTicket) -> Ticket {
    Ticket {
        id: rt.id.0.clone(),
        session_id: rt.session_id.clone(),
        action_kind: rt.action_kind.clone(),
        args: rt.args.clone(),
        reason: rt.reason.clone(),
        hint: rt.hint.clone(),
        expires_at: rt.expires_at,
    }
}

impl RuntimeGlue {
    pub fn spawn(
        self,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut notif_rx = self.notifications.subscribe();
            let mut poll_ticker = tokio::time::interval(std::time::Duration::from_millis(500));
            let mut known_sessions: HashSet<String> = HashSet::new();
            let mut last_tickets: Vec<String> = Vec::new();

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    notif = notif_rx.recv() => {
                        match notif {
                            Ok(n) => {
                                let _ = tx.send(AppEvent::Chat(ChatMsg::Notify {
                                    severity: n.severity,
                                    subject: n.subject,
                                    body: n.body,
                                }));
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                tracing::warn!("notifications: skipped {n} lagged messages");
                            }
                            Err(_) => break,
                        }
                    }
                    _ = poll_ticker.tick() => {
                        // Poll pending tickets for root session
                        if let Ok(all) = self.gateway.ticket_store().list_by_session(&self.root_session) {
                            let pending: Vec<Ticket> = all.iter()
                                .filter(|t| t.status == TicketStatus::Pending)
                                .filter(|t| t.expires_at > Instant::now())
                                .map(to_tui_ticket)
                                .collect();
                            let ids: Vec<String> = pending.iter().map(|t| t.id.clone()).collect();
                            if ids != last_tickets {
                                last_tickets = ids;
                                let _ = tx.send(AppEvent::Approval(ApprovalEvent::SetPending(pending)));
                            }
                        }

                        // Poll active sessions to detect spawn/finish
                        if let Ok(active) = self.sessions.list_active().await {
                            let current: HashSet<String> = active.iter()
                                .map(|s| s.id.as_str().to_string())
                                .collect();
                            for id in &current {
                                if !known_sessions.contains(id) && id != self.root_session.as_str() {
                                    let _ = tx.send(AppEvent::Chat(ChatMsg::SubAgentSpawn {
                                        label: "sub-agent".into(),
                                        child: id.clone(),
                                    }));
                                }
                            }
                            for id in &known_sessions {
                                if !current.contains(id) {
                                    let _ = tx.send(AppEvent::Chat(ChatMsg::SubAgentFinish {
                                        label: "sub-agent".into(),
                                        child: id.clone(),
                                        outcome: "done".into(),
                                    }));
                                }
                            }
                            known_sessions = current;
                        }
                    }
                }
            }
        })
    }

    /// Best-effort cancel of the root session on shutdown.
    pub async fn shutdown(&self) {
        let _ = runtime::sessions::cancel_session(
            self.sessions.as_ref(),
            self.inbox.as_ref(),
            &self.root_session,
        )
        .await;
    }

    pub async fn approve(
        &self,
        ticket_id: &TicketId,
        choice: ApprovalChoice,
    ) -> anyhow::Result<()> {
        let outcome = match choice {
            ApprovalChoice::Approve => ApprovalOutcome::Approved,
            ApprovalChoice::Deny { reason } => ApprovalOutcome::Denied { reason },
            ApprovalChoice::Skip => return Ok(()),
        };
        let identity = ApproverIdentity {
            kind: ApproverKind::LocalKey {
                key_id: self.local_key_id.clone(),
            },
            roles: self.local_roles.clone(),
        };
        Ok(self
            .gateway
            .decide_local(ticket_id, outcome, identity)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime::approvals::gateway::ApprovalGateway;
    use runtime::persistence::memory::{InMemoryInbox, InMemorySessionStore};
    use runtime::persistence::tickets::InMemoryTicketStore;
    use runtime::sessions::Principal;
    use std::sync::Arc;

    #[tokio::test]
    async fn glue_spawn_forwards_notification() {
        let gateway = Arc::new(ApprovalGateway::new(
            InMemoryTicketStore::new(),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryInbox::new()),
            vec![],
        ));
        let sessions: Arc<dyn SessionStore> = Arc::new(InMemorySessionStore::new());
        let notifications = runtime::persistence::notifications::InMemoryNotificationStore::new();
        let root = sessions
            .create_root("orch".into(), Principal("anon".into()), None)
            .await
            .unwrap();

        let root_clone = root.clone();
        let inbox: Arc<dyn Inbox> = Arc::new(InMemoryInbox::new());
        let glue = RuntimeGlue {
            gateway,
            sessions,
            inbox,
            notifications: notifications.clone(),
            local_key_id: "local".into(),
            local_roles: vec!["campaign_owner".into()],
            root_session: root,
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let cancel = CancellationToken::new();
        let _handle = glue.spawn(tx.clone(), cancel.clone());

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let n = runtime::notifications::types::Notification {
            id: runtime::notifications::types::NotificationId("n_1".into()),
            severity: runtime::notifications::types::Severity::Warn,
            subject: "test".into(),
            body: "body".into(),
            refs: runtime::notifications::types::NotificationRefs::default(),
            emitted_by: root_clone,
            emitted_at: std::time::SystemTime::now(),
        };
        notifications.record(n).unwrap();

        let result = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while let Some(event) = rx.recv().await {
                if matches!(event, AppEvent::Chat(ChatMsg::Notify { .. })) {
                    return true;
                }
            }
            false
        })
        .await;

        cancel.cancel();
        assert!(result.unwrap_or(false), "expected notification event");
    }
}
