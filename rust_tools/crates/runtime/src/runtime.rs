use crate::agents::AuditLog;
use crate::approvals::gateway::ApprovalGateway;
use crate::approvals::registry::{ActionRegistry, InMemoryActionRegistry};
use crate::authz_hook::AuthzBackend;
use crate::notifications::types::NotificationIdGenerator;
use crate::persistence::memory::{InMemoryInbox, InMemorySessionStore};
use crate::persistence::notifications::{InMemoryNotificationStore, NotificationStore};
use crate::persistence::tickets::{InMemoryTicketStore, TicketStore};
use crate::persistence::{Inbox, SessionStore};
use crate::sessions::Principal;
use crate::sub_agent::outcome::SubAgentOutcome;
use crate::sub_agent::registry::SubAgentRegistry;
use crate::sub_agent::spawn::{ChildRunner, SpawnCtx};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub struct Runtime {
    pub gateway: Arc<ApprovalGateway>,
    pub session_store: Arc<dyn SessionStore>,
    pub inbox: Arc<dyn Inbox>,
    pub ticket_store: Arc<dyn TicketStore>,
    pub notification_store: Arc<dyn NotificationStore>,
    pub notification_idgen: Arc<NotificationIdGenerator>,
    pub action_registry: Arc<dyn ActionRegistry>,
    pub spawn_ctx: Arc<SpawnCtx>,
    pub audit_log: Arc<AuditLog>,
    pub hmac_key: Vec<u8>,
    pub local_key_id: String,
    pub local_roles: Vec<String>,
    pub authz_backend: AuthzBackend,
}

struct StubChildRunner;

#[async_trait]
impl ChildRunner for StubChildRunner {
    async fn run(
        &self,
        _child: crate::sessions::SessionId,
        _prompt: String,
        _cancel: CancellationToken,
    ) -> SubAgentOutcome {
        SubAgentOutcome::Done {
            summary: "stub".into(),
            result: String::new(),
        }
    }
}

pub fn build_runtime(
    approver_hmac_key: &str,
    local_key_id: &str,
    local_roles: &str,
    authz_backend: AuthzBackend,
) -> (Runtime, CancellationToken, JoinSet<()>) {
    let cancel = CancellationToken::new();
    let set = JoinSet::new();

    let session_store: Arc<dyn SessionStore> = InMemorySessionStore::arc();
    let inbox: Arc<dyn Inbox> = InMemoryInbox::arc();
    let ticket_store = InMemoryTicketStore::new();
    let notification_store = InMemoryNotificationStore::new();
    let notification_idgen = Arc::new(NotificationIdGenerator::new());

    let hmac_key = approver_hmac_key.as_bytes().to_vec();
    let local_key_id = local_key_id.to_string();
    let local_roles: Vec<String> = local_roles.split(',').map(|s| s.trim().to_string()).collect();

    let gateway = Arc::new(ApprovalGateway::new(
        ticket_store.clone(),
        session_store.clone(),
        inbox.clone(),
        hmac_key.clone(),
    ));

    let action_registry: Arc<dyn ActionRegistry> = InMemoryActionRegistry::new();

    let spawn_ctx = Arc::new(SpawnCtx {
        sessions: session_store.clone(),
        inbox: inbox.clone(),
        registry: SubAgentRegistry::new(),
        runner: Arc::new(StubChildRunner),
        principal: Principal("anon".into()),
    });

    let audit_log = Arc::new(AuditLog::new());

    (
        Runtime {
            gateway,
            session_store,
            inbox,
            ticket_store,
            notification_store,
            notification_idgen,
            action_registry,
            spawn_ctx,
            audit_log,
            hmac_key,
            local_key_id,
            local_roles,
            authz_backend,
        },
        cancel,
        set,
    )
}
