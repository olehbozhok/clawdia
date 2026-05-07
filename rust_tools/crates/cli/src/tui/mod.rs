#![allow(dead_code)]

pub mod approvals_pane;
pub mod chat_pane;
pub mod keymap;
pub mod log_layer;
pub mod log_pane;
pub mod runtime_glue;
pub mod tracing_init;

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Chat,
    Approvals,
    Log,
}

#[derive(Debug)]
pub struct App {
    pub focus: Pane,
    pub chat: chat_pane::ChatState,
    pub log: log_pane::LogState,
    pub approvals: approvals_pane::ApprovalsState,
    pub quitting: bool,
}

#[derive(Debug)]
pub enum AppEvent {
    FocusNext,
    Key(crossterm::event::KeyEvent),
    Tick(Instant),
    Log(log_layer::LogLine),
    Chat(chat_pane::ChatMsg),
    Approval(approvals_pane::ApprovalEvent),
    Quit,
}

impl App {
    pub fn new() -> Self {
        Self {
            focus: Pane::Chat,
            chat: chat_pane::ChatState::default(),
            log: log_pane::LogState::with_capacity(1000),
            approvals: approvals_pane::ApprovalsState::default(),
            quitting: false,
        }
    }

    pub fn reduce(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::FocusNext => {
                self.focus = match self.focus {
                    Pane::Chat => Pane::Approvals,
                    Pane::Approvals => Pane::Log,
                    Pane::Log => Pane::Chat,
                };
            }
            AppEvent::Quit => {
                self.quitting = true;
            }
            AppEvent::Tick(now) => {
                self.approvals.advance(now);
            }
            AppEvent::Log(line) => {
                self.log.push(line);
            }
            AppEvent::Chat(msg) => {
                self.chat.history.push(msg);
            }
            AppEvent::Approval(ev) => {
                match ev {
                    approvals_pane::ApprovalEvent::SetPending(tickets) => {
                        self.approvals.set_pending(tickets);
                    }
                    approvals_pane::ApprovalEvent::SelectNext => {
                        self.approvals.select_next();
                    }
                    approvals_pane::ApprovalEvent::SelectPrev => {
                        self.approvals.select_prev();
                    }
                    approvals_pane::ApprovalEvent::DenyModalClosed => {
                        self.approvals.deny_modal_open = false;
                        self.approvals.deny_reason.clear();
                    }
                }
            }
            AppEvent::Key(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::approvals_pane::{ApprovalEvent, Ticket};
    use crate::tui::chat_pane::ChatMsg;
    use runtime::notifications::types::Severity;
    use runtime::sessions::SessionId;
    use std::time::Duration;

    #[test]
    fn focus_next_cycles_three_panes() {
        let mut app = App::new();
        assert_eq!(app.focus, Pane::Chat);
        app.reduce(AppEvent::FocusNext);
        assert_eq!(app.focus, Pane::Approvals);
        app.reduce(AppEvent::FocusNext);
        assert_eq!(app.focus, Pane::Log);
        app.reduce(AppEvent::FocusNext);
        assert_eq!(app.focus, Pane::Chat);
    }

    #[test]
    fn quit_sets_quitting_flag() {
        let mut app = App::new();
        assert!(!app.quitting);
        app.reduce(AppEvent::Quit);
        assert!(app.quitting);
    }

    #[test]
    fn tick_advances_approvals() {
        let mut app = App::new();
        let now = Instant::now();
        app.reduce(AppEvent::Tick(now));
    }

    #[test]
    fn log_event_pushes_to_log_state() {
        let mut app = App::new();
        let line = log_layer::LogLine {
            at: std::time::SystemTime::now(),
            level: tracing::Level::INFO,
            target: "test".to_string(),
            message: "hello".to_string(),
        };
        app.reduce(AppEvent::Log(line));
        assert_eq!(app.log.lines.len(), 1);
    }

    fn ticket(id: &str, ttl: Duration) -> Ticket {
        Ticket {
            id: id.into(),
            session_id: SessionId::from_string("s_test1".to_string()).unwrap(),
            action_kind: "tool.doc_publish_live".into(),
            args: serde_json::json!({"draft_id": "d1"}),
            reason: "publish manifest".into(),
            hint: None,
            expires_at: Instant::now() + ttl,
        }
    }

    #[test]
    fn spawn_notify_approve_consume_flow() {
        let mut app = App::new();
        app.reduce(AppEvent::Chat(ChatMsg::SubAgentSpawn {
            label: "researcher".into(),
            child: "sid_42".into(),
        }));
        assert_eq!(app.chat.history.len(), 1);

        app.reduce(AppEvent::Chat(ChatMsg::Notify {
            severity: Severity::Warn,
            subject: "source flaky".into(),
            body: "retrying".into(),
        }));
        assert_eq!(app.chat.history.len(), 2);

        let t = ticket("tk_1", Duration::from_secs(60));
        app.reduce(AppEvent::Approval(ApprovalEvent::SetPending(vec![
            t.clone(),
        ])));
        assert_eq!(app.approvals.selected_id(), Some("tk_1"));

        app.reduce(AppEvent::Approval(ApprovalEvent::SetPending(vec![])));
        assert_eq!(app.approvals.selected_id(), None);

        app.reduce(AppEvent::Chat(ChatMsg::SubAgentFinish {
            label: "researcher".into(),
            child: "sid_42".into(),
            outcome: "Done".into(),
        }));
        assert_eq!(app.chat.history.len(), 3);
    }
}
