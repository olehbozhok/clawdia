use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use std::collections::VecDeque;

use super::log_layer::LogLine;

#[derive(Debug, Default)]
pub struct LogState {
    pub lines: VecDeque<LogLine>,
    capacity: usize,
    pub scroll: usize,
}

impl LogState {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(cap),
            capacity: cap,
            scroll: 0,
        }
    }

    pub fn push(&mut self, line: LogLine) {
        if self.lines.len() >= self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
        self.scroll = self.lines.len().saturating_sub(1);
    }

    pub fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        let max = self.lines.len().saturating_sub(1);
        if self.scroll < max {
            self.scroll += 1;
        }
    }
}

fn severity_color(level: &tracing::Level) -> Color {
    match *level {
        tracing::Level::ERROR => Color::Red,
        tracing::Level::WARN => Color::Yellow,
        tracing::Level::INFO => Color::White,
        tracing::Level::DEBUG => Color::Gray,
        tracing::Level::TRACE => Color::DarkGray,
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &LogState, focused: bool) {
    let border_style = if focused {
        Style::default()
    } else {
        Style::default().dim()
    };

    let block = Block::default()
        .title(" Log ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);

    let visible_lines: Vec<Line> = state
        .lines
        .iter()
        .skip(state.scroll.saturating_sub(inner.height as usize))
        .take(inner.height as usize)
        .map(|l| {
            let color = severity_color(&l.level);
            let preview: String = l.message.chars().take(inner.width as usize).collect();
            Line::from(Span::styled(preview, Style::default().fg(color)))
        })
        .collect();

    let paragraph = Paragraph::new(visible_lines).block(block);
    frame.render_widget(paragraph, area);

    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None);
    let mut scrollbar_state = ScrollbarState::new(state.lines.len().saturating_sub(1))
        .position(state.scroll.min(state.lines.len().saturating_sub(1)));
    frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn renders_warn_with_yellow() {
        let mut state = LogState::with_capacity(8);
        state.push(LogLine {
            at: std::time::SystemTime::now(),
            level: tracing::Level::WARN,
            target: "net".to_string(),
            message: "slow upstream".to_string(),
        });
        let backend = TestBackend::new(40, 4);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, f.area(), &state, false))
            .unwrap();
        let buf = term.backend().buffer();
        let cell = buf.cell((1, 1)).unwrap();
        assert_eq!(cell.fg, ratatui::style::Color::Yellow);
    }
}
