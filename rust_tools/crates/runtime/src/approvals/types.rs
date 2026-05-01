//! Canonical approval ticket id. Plan 04 expands the surrounding module with
//! `Ticket`, `TicketStore`, etc.

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TicketId(pub String);

impl std::fmt::Display for TicketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
