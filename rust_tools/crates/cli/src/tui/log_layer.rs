use tokio::sync::mpsc::UnboundedSender;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

#[derive(Debug, Clone)]
pub struct LogLine {
    pub at: std::time::SystemTime,
    pub level: Level,
    pub target: String,
    pub message: String,
}

pub struct LogLayer {
    tx: UnboundedSender<LogLine>,
}

impl LogLayer {
    pub fn new(tx: UnboundedSender<LogLine>) -> Self {
        Self { tx }
    }
}

impl<S: Subscriber> Layer<S> for LogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let meta = event.metadata();
        let _ = self.tx.send(LogLine {
            at: std::time::SystemTime::now(),
            level: *meta.level(),
            target: meta.target().to_string(),
            message: visitor.0,
        });
    }
}

#[derive(Default)]
struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}").trim_matches('"').to_string();
        } else {
            use std::fmt::Write;
            let _ = write!(self.0, " {}={:?}", field.name(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use tracing::info;
    use tracing_subscriber::prelude::*;

    #[tokio::test]
    async fn forwards_info_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let layer = LogLayer::new(tx);
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            info!(target: "test", "hello {}", "world");
        });
        let line = rx.recv().await.expect("a line");
        assert_eq!(line.level, Level::INFO);
        assert!(line.message.contains("hello world"));
        assert_eq!(line.target, "test");
    }
}
