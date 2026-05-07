use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Pane;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalChoice {
    Approve,
    Deny { reason: String },
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    FocusNext,
    Quit,
    SelectNext,
    SelectPrev,
    ApprovalDecide(ApprovalChoice),
    OpenDenyModal,
    SubmitDenyReason(String),
    SubmitChat(String),
    InputChar(char),
    InputBackspace,
    None,
}

pub fn dispatch(pane: Pane, key: KeyEvent) -> Action {
    match (pane, key.code, key.modifiers) {
        // Global
        (_, KeyCode::Tab, _) => Action::FocusNext,
        (_, KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,

        // Chat pane
        (Pane::Chat, KeyCode::Enter, _) => {
            // Enter submits chat; actual text is handled by the event loop
            Action::SubmitChat(String::new())
        }
        (Pane::Chat, KeyCode::Char(c), KeyModifiers::NONE) => Action::InputChar(c),
        (Pane::Chat, KeyCode::Backspace, _) => Action::InputBackspace,

        // Approvals pane
        (Pane::Approvals, KeyCode::Char('j'), _) | (Pane::Approvals, KeyCode::Down, _) => {
            Action::SelectNext
        }
        (Pane::Approvals, KeyCode::Char('k'), _) | (Pane::Approvals, KeyCode::Up, _) => {
            Action::SelectPrev
        }
        (Pane::Approvals, KeyCode::Char('a'), _) => {
            Action::ApprovalDecide(ApprovalChoice::Approve)
        }
        (Pane::Approvals, KeyCode::Char('d'), _) => Action::OpenDenyModal,
        (Pane::Approvals, KeyCode::Char('s'), _) => Action::ApprovalDecide(ApprovalChoice::Skip),

        // Log pane
        (Pane::Log, KeyCode::Char('j'), _) | (Pane::Log, KeyCode::Down, _) => Action::SelectNext,
        (Pane::Log, KeyCode::Char('k'), _) | (Pane::Log, KeyCode::Up, _) => Action::SelectPrev,

        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(c: char, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), m)
    }

    #[test]
    fn approve_key_in_approvals_pane_emits_approve_action() {
        let ev = key('a', KeyModifiers::NONE);
        let act = dispatch(Pane::Approvals, ev);
        assert!(matches!(act, Action::ApprovalDecide(ApprovalChoice::Approve)));
    }

    #[test]
    fn ctrl_c_emits_quit_in_any_pane() {
        let ev = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(dispatch(Pane::Chat, ev), Action::Quit));
        assert!(matches!(dispatch(Pane::Log, ev), Action::Quit));
    }

    #[test]
    fn jk_navigation_in_approvals() {
        assert!(matches!(
            dispatch(Pane::Approvals, key('j', KeyModifiers::NONE)),
            Action::SelectNext
        ));
        assert!(matches!(
            dispatch(Pane::Approvals, key('k', KeyModifiers::NONE)),
            Action::SelectPrev
        ));
    }

    #[test]
    fn tab_focus_next() {
        let ev = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(dispatch(Pane::Chat, ev), Action::FocusNext);
        assert_eq!(dispatch(Pane::Approvals, ev), Action::FocusNext);
    }
}
