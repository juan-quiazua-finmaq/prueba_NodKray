//! Tracing setup (spec §90).
//!
//! All structured logs are appended to `~/.nodkray/logs/nodkray.log`. The
//! stderr layer is only attached with `--verbose`; stdout is never used for
//! logs so that `--json` can emit valid JSON exclusively.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter, Layer};

use crate::config::ConfigPaths;

/// Initialise logging. Returns the appender guard that must be kept alive for
/// the duration of the process. Safe to call more than once (later calls are
/// no-ops), which keeps unit tests from panicking.
pub fn init(paths: &ConfigPaths, verbose: bool) -> Option<WorkerGuard> {
    let filter = build_filter(verbose);

    match file_writer(paths) {
        Some((writer, guard)) => {
            let file_layer = fmt::layer()
                .with_writer(writer)
                .with_ansi(false)
                .with_target(true);
            let stderr_layer = fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .with_filter(filter_fn(move |_| verbose));

            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .with(stderr_layer)
                .try_init();
            Some(guard)
        }
        None => {
            // File logging unavailable: fall back to stderr only.
            let stderr_layer = fmt::layer().with_writer(std::io::stderr).with_ansi(false);
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .try_init();
            None
        }
    }
}

fn build_filter(verbose: bool) -> EnvFilter {
    let level = if verbose { "debug" } else { "info" };
    EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("nodkray={level},warn")))
}

fn file_writer(paths: &ConfigPaths) -> Option<(tracing_appender::non_blocking::NonBlocking, WorkerGuard)> {
    let logs_dir = paths.logs_dir();
    std::fs::create_dir_all(&logs_dir).ok()?;
    let appender = tracing_appender::rolling::never(&logs_dir, "nodkray.log");
    Some(tracing_appender::non_blocking(appender))
}

/// Path of the main log file, for messages and tests.
pub fn log_file(paths: &ConfigPaths) -> std::path::PathBuf {
    paths.logs_dir().join("nodkray.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_file_lives_under_logs_dir() {
        let paths = ConfigPaths::with_home("/tmp/c", "/tmp/d");
        assert_eq!(log_file(&paths), std::path::PathBuf::from("/tmp/d/logs/nodkray.log"));
    }

    #[test]
    fn init_is_idempotent_and_creates_log_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = ConfigPaths::with_home(tmp.path().join("config"), tmp.path().join("data"));
        let guard = init(&paths, false);
        assert!(paths.logs_dir().is_dir());
        // Second call must not panic.
        drop(guard);
        let _ = init(&paths, true);
    }
}
