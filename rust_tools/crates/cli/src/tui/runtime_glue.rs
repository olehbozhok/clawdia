use runtime::approvals::outcome::{ApprovalOutcome, ApproverIdentity, ApproverKind};
use runtime::approvals::types::TicketId;
use runtime::persistence::notifications::NotificationStore;
use runtime::persistence::SessionStore;
use runtime::sessions::SessionId;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::chat_pane::ChatMsg;
use super::keymap::ApprovalChoice;
use super::AppEvent;

#[derive(Clone)]
pub struct RuntimeGlue {
    pub gateway: Arc<runtime::approvals::gateway::ApprovalGateway>,
    pub sessions: Arc<dyn SessionStore>,
    pub notifications: Arc<dyn NotificationStore>,
    pub hmac_key: Arc<Vec<u8>>,
    pub local_key_id: String,
    pub local_roles: Vec<String>,
    pub root_session: SessionId,
}

impl RuntimeGlue {
    pub fn spawn(
        self,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut notif_rx = self.notifications.subscribe();
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
                }
            }
        })
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
        Ok(self.gateway.decide_local(ticket_id, outcome, identity).await?)
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
        let glue = RuntimeGlue {
            gateway,
            sessions,
            notifications: notifications.clone(),
            hmac_key: Arc::new(vec![]),
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

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            async {
                while let Some(event) = rx.recv().await {
                    if matches!(event, AppEvent::Chat(ChatMsg::Notify { .. })) {
                        return true;
                    }
                }
                false
            },
        )
        .await;

        cancel.cancel();
        assert!(result.unwrap_or(false), "expected notification event");
    }
}
