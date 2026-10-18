//! Logging: console plus rolling files in `./log/`.
// Java: dist/game/log.cfg and GameServer constructor (`log/java%g.log`, `log/error%g.log`,
// 20 files kept). Same pattern as `login-server/src/logging.rs`.
//
// Differences from Java, on purpose: `log.cfg` is a `java.util.logging` file and is not read.
// The level comes from `RUST_LOG` (default `info`). Files roll daily, the last 20 are kept.
// The error file holds ERROR events (Java's `ErrorLogHandler` keeps records with exceptions).
// TODO(later): the separate chat / item / GM-audit logs of D-25 come with the cards that
// produce those events.

use std::path::Path;

use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

/// Name of the log directory (inside the server's root folder).
pub const LOG_DIR_NAME: &str = "log";
/// Daily log files are named `<prefix>.<date>.log`.
const MAIN_LOG_PREFIX: &str = "game";
const ERROR_LOG_PREFIX: &str = "game-error";
const LOG_SUFFIX: &str = "log";
/// Java `FileHandler.count = 20`.
const KEEP_FILES: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("cannot create log directory {path}: {source}")]
    CreateDir { path: String, source: std::io::Error },
    #[error("cannot open log file in {path}: {source}")]
    OpenFile { path: String, source: InitError },
    #[error("a logger is already installed")]
    AlreadyInstalled,
}

/// Keeps the file writers flushing. Drop it last.
#[must_use = "dropping the guard stops the file logging"]
pub struct LogGuard {
    _guards: Vec<WorkerGuard>,
}

/// Installs the global logger: console, `log/game.<date>.log` and `log/game-error.<date>.log`.
pub fn init(log_dir: &Path) -> Result<LogGuard, LoggingError> {
    let (subscriber, guard) = build(log_dir)?;
    subscriber
        .try_init()
        .map_err(|_| LoggingError::AlreadyInstalled)?;
    Ok(guard)
}

/// The subscriber `init` installs, without installing it (tests use it with `with_default`).
pub fn build(log_dir: &Path) -> Result<(impl Subscriber + Send + Sync + 'static, LogGuard), LoggingError> {
    // Java: mkdir the log folder. Here the file appender needs it.
    std::fs::create_dir_all(log_dir).map_err(|source| LoggingError::CreateDir {
        path: log_dir.display().to_string(),
        source,
    })?;
    let (main_writer, main_guard) = tracing_appender::non_blocking(appender(log_dir, MAIN_LOG_PREFIX)?);
    let (error_writer, error_guard) = tracing_appender::non_blocking(appender(log_dir, ERROR_LOG_PREFIX)?);

    let subscriber = tracing_subscriber::registry()
        .with(fmt::layer().with_filter(env_filter()))
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(main_writer)
                .with_filter(env_filter()),
        )
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(error_writer)
                .with_filter(LevelFilter::ERROR),
        );
    Ok((
        subscriber,
        LogGuard {
            _guards: vec![main_guard, error_guard],
        },
    ))
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

fn appender(log_dir: &Path, prefix: &str) -> Result<RollingFileAppender, LoggingError> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(prefix)
        .filename_suffix(LOG_SUFFIX)
        .max_log_files(KEEP_FILES)
        .build(log_dir)
        .map_err(|source| LoggingError::OpenFile {
            path: log_dir.display().to_string(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_logs(dir: &Path, prefix: &str) -> String {
        let mut text = String::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            // `game.` must not match `game-error.`.
            if name.starts_with(&format!("{prefix}.")) && name.ends_with(".log") {
                text.push_str(&std::fs::read_to_string(path).unwrap());
            }
        }
        text
    }

    #[test]
    fn writes_console_and_both_files_and_creates_the_directory() {
        let root = tempfile::tempdir().unwrap();
        let log_dir = root.path().join(LOG_DIR_NAME).join("nested");
        let (subscriber, guard) = build(&log_dir).unwrap();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("Loaded 3 things.");
            tracing::error!("something broke");
        });
        drop(guard); // flushes

        let main = read_logs(&log_dir, "game");
        assert!(main.contains("Loaded 3 things."), "{main}");
        assert!(main.contains("something broke"), "{main}");
        let errors = read_logs(&log_dir, "game-error");
        assert!(errors.contains("something broke"), "{errors}");
        assert!(!errors.contains("3 things"), "{errors}");
    }

    #[test]
    fn unusable_directory_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("a-file");
        std::fs::write(&file, "x").unwrap();
        assert!(matches!(
            build(&file.join("log")),
            Err(LoggingError::CreateDir { .. })
        ));
    }
}
