use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{prelude::*, EnvFilter};

pub struct AppenderGuard {
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

impl AppenderGuard {
    fn new(guard: tracing_appender::non_blocking::WorkerGuard) -> Self {
        Self { _guard: guard }
    }
}

pub fn init_tracing(
    log_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::log_layer::LogLine>,
    log_dir: &std::path::Path,
) -> anyhow::Result<AppenderGuard> {
    std::fs::create_dir_all(log_dir)?;
    let file_appender = rolling::daily(log_dir, "runtime.log");
    let (file_nb, guard) = non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(crate::tui::log_layer::LogLayer::new(log_tx))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_nb)
                .with_ansi(false),
        );

    registry.try_init().map_err(|e| anyhow::anyhow!("tracing init: {e}"))?;
    Ok(AppenderGuard::new(guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_tracing_writes_to_file_and_channel() {
        let dir = tempdir().unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let _guard = init_tracing(tx.clone(), dir.path()).unwrap();

        tracing::warn!("test message");

        // Check channel received it
        let line = rx.try_recv().expect("expected log line");
        assert!(line.message.contains("test message"));

        // Check file was created
        let entries = std::fs::read_dir(dir.path()).unwrap();
        assert!(entries.count() > 0);
    }
}
