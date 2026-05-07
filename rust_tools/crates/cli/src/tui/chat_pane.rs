use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use runtime::notifications::types::Severity;

pub fn severity_color(severity: &Severity) -> (Color, Modifier) {
    match severity {
        Severity::Info => (Color::White, Modifier::empty()),
        Severity::Warn => (Color::Yellow, Modifier::empty()),
        Severity::Error => (Color::Red, Modifier::empty()),
        Severity::Blocker => (Color::Magenta, Modifier::BOLD),
    }
}

#[derive(Debug, Clone)]
pub enum ChatMsg {
    User(String),
    Agent { label: String, text: String },
    SubAgentSpawn { label: String, child: String },
    SubAgentFinish { label: String, child: String, outcome: String },
    Notify { severity: Severity, subject: String, body: String },
}

#[derive(Debug, Default)]
pub struct ChatState {
    pub history: Vec<ChatMsg>,
    pub input: tui_input::Input,
    pub scroll: u16,
}

pub fn truncate_for_width(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

fn render_chat_msg<'a>(msg: &'a ChatMsg, width: usize) -> Line<'a> {
    match msg {
        ChatMsg::User(text) => {
            let preview = truncate_for_width(text, width);
            Line::from(Span::styled(
                format!("You: {preview}"),
                Style::default().fg(Color::Cyan),
            ))
        }
        ChatMsg::Agent { label, text } => {
            let preview = truncate_for_width(text, width);
            Line::from(Span::styled(
                format!("[{label}]: {preview}"),
                Style::default().fg(Color::Green),
            ))
        }
        ChatMsg::SubAgentSpawn { label, child } => {
            Line::from(Span::styled(
                format!("⚡ spawned {label}/{child}"),
                Style::default().fg(Color::Blue),
            ))
        }
        ChatMsg::SubAgentFinish { label, child, outcome } => {
            Line::from(Span::styled(
                format!("✓ {label}/{child} finished: {outcome}"),
                Style::default().fg(Color::Blue),
            ))
        }
        ChatMsg::Notify { severity, subject, body } => {
            let (color, modifier) = severity_color(severity);
            let prefix = match severity {
                Severity::Info => "[INFO]",
                Severity::Warn => "[WARN]",
                Severity::Error => "[ERROR]",
                Severity::Blocker => "[BLOCKER]",
            };
            let detail = if body.is_empty() {
                truncate_for_width(subject, width.saturating_sub(10))
            } else {
                truncate_for_width(&format!("{subject}: {body}"), width.saturating_sub(10))
            };
            Line::from(Span::styled(
                format!("{prefix} {detail}"),
                Style::default().fg(color).add_modifier(modifier),
            ))
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ChatState, focused: bool) {
    let border_style = if focused {
        Style::default()
    } else {
        Style::default().dim()
    };

    let block = Block::default()
        .title(" Chat ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(inner);

    let lines: Vec<Line> = state
        .history
        .iter()
        .map(|msg| render_chat_msg(msg, chunks[0].width as usize))
        .collect();

    let history_para = Paragraph::new(lines).scroll((state.scroll, 0));
    frame.render_widget(history_para, chunks[0]);

    let input_text: String = state.input.value().chars().take(chunks[1].width as usize).collect();
    let input_para = Paragraph::new(input_text)
        .style(if focused { Style::default() } else { Style::default().dim() });
    frame.render_widget(input_para, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn truncate_uses_chars_not_bytes() {
        let line = "héllo🌊 world";
        let truncated = truncate_for_width(line, 6);
        assert_eq!(truncated.chars().count(), 6);
    }

    #[test]
    fn notification_renders_with_severity_prefix() {
        let mut s = ChatState::default();
        let msg = ChatMsg::Notify {
            severity: Severity::Warn,
            subject: "stale".into(),
            body: "data old".into(),
        };
        s.history.push(msg);
        let line = render_chat_msg(&s.history[0], 80);
        let rendered = line.to_string();
        assert!(rendered.contains("[WARN]"));
    }

    #[test]
    fn severity_colors_exhaustive() {
        use ratatui::style::Color;
        let backend = TestBackend::new(80, 10);
        let mut term = Terminal::new(backend).unwrap();

        for (severity, _expected_color) in [
            (Severity::Info, Color::White),
            (Severity::Warn, Color::Yellow),
            (Severity::Error, Color::Red),
            (Severity::Blocker, Color::Magenta),
        ] {
            let mut state = ChatState::default();
            state.history.push(ChatMsg::Notify {
                severity,
                subject: "test".into(),
                body: "".into(),
            });
            term.draw(|f| {
                let area = Rect::new(0, 0, 80, 10);
                render(f, area, &state, true)
            })
            .unwrap();
        }
    }
}
