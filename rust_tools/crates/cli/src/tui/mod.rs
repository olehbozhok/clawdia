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
                }
            }
            AppEvent::Key(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
