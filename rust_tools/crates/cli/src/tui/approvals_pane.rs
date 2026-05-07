use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use runtime::sessions::SessionId;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Ticket {
    pub id: String,
    pub session_id: SessionId,
    pub action_kind: String,
    pub args: serde_json::Value,
    pub reason: String,
    pub hint: Option<String>,
    pub expires_at: Instant,
}

#[derive(Debug, Default)]
pub struct ApprovalsState {
    pub pending: Vec<Ticket>,
    pub selected: usize,
    pub deny_modal_open: bool,
    pub deny_reason: String,
}

#[derive(Debug)]
pub enum ApprovalEvent {
    SetPending(Vec<Ticket>),
    SelectNext,
    SelectPrev,
    DenyModalClosed,
}

impl ApprovalsState {
    pub fn set_pending(&mut self, tickets: Vec<Ticket>) {
        self.pending = tickets;
        self.selected = self.selected.min(self.pending.len().saturating_sub(1));
    }

    pub fn select_next(&mut self) {
        if !self.pending.is_empty() && self.selected + 1 < self.pending.len() {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn selected_id(&self) -> Option<&str> {
        self.pending.get(self.selected).map(|t| t.id.as_str())
    }

    pub fn remaining_for(&self, id: &str, now: Instant) -> Duration {
        self.pending
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.expires_at.saturating_duration_since(now))
            .unwrap_or_default()
    }

    pub fn advance(&mut self, now: Instant) {
        self.pending.retain(|t| t.expires_at > now);
        self.selected = self.selected.min(self.pending.len().saturating_sub(1));
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ApprovalsState, focused: bool) {
    let border_style = if focused {
        Style::default()
    } else {
        Style::default().dim()
    };

    let block = Block::default()
        .title(" Approvals ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);

    if state.deny_modal_open {
        let modal_text = format!(
            "Reason for denial:\n{}\n\n[Enter] submit  [Esc] cancel",
            state.deny_reason
        );
        let para = Paragraph::new(modal_text).block(
            Block::default().title(" Deny ").borders(Borders::ALL),
        );
        frame.render_widget(para, area);
        return;
    }

    if state.pending.is_empty() {
        let para = Paragraph::new(Line::from(Span::raw("No pending approvals")))
            .block(block);
        frame.render_widget(para, area);
        return;
    }

    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(inner);

    let list_lines: Vec<Line> = state
        .pending
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let prefix = if i == state.selected { "▸ " } else { "  " };
            let style = if i == state.selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            Line::from(Span::styled(format!("{prefix}{}", t.id), style))
        })
        .collect();

    let list_para = Paragraph::new(list_lines).block(
        Block::default().title("Pending").borders(Borders::ALL),
    );
    frame.render_widget(list_para, chunks[0]);

    if let Some(ticket) = state.pending.get(state.selected) {
        let ttl = state.remaining_for(&ticket.id, Instant::now());
        let mins = ttl.as_secs() / 60;
        let secs = ttl.as_secs() % 60;
        let hint_line = ticket.hint.as_deref().map(|h| format!("\nHint: {h}")).unwrap_or_default();
        let detail_text = format!(
            "Action: {}\nSession: {}\nReason: {}\nArgs: {}{hint_line}\nTTL: {mins:02}:{secs:02}",
            ticket.action_kind,
            ticket.session_id.as_str(),
            ticket.reason,
            serde_json::to_string_pretty(&ticket.args).unwrap_or_default(),
        );
        let detail_para = Paragraph::new(detail_text).block(
            Block::default().title("Detail").borders(Borders::ALL),
        );
        frame.render_widget(detail_para, chunks[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

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
    fn selection_moves_within_bounds() {
        let mut s = ApprovalsState::default();
        s.set_pending(vec![
            ticket("a", Duration::from_secs(60)),
            ticket("b", Duration::from_secs(60)),
        ]);
        assert_eq!(s.selected_id(), Some("a"));
        s.select_next();
        assert_eq!(s.selected_id(), Some("b"));
        s.select_next();
        assert_eq!(s.selected_id(), Some("b"));
        s.select_prev();
        assert_eq!(s.selected_id(), Some("a"));
    }

    #[test]
    fn ttl_countdown_advances() {
        let mut s = ApprovalsState::default();
        s.set_pending(vec![ticket("a", Duration::from_secs(30))]);
        let t0 = Instant::now();
        let remaining = s.remaining_for(s.selected_id().unwrap(), t0);
        assert!(remaining <= Duration::from_secs(30));
        let later =
            s.remaining_for(s.selected_id().unwrap(), t0 + Duration::from_secs(10));
        assert!(later < remaining);
    }
}
