//! Logging: console plus rolling files in `./log/`, and the three side logs of Java (chat, item,
//! GM audit).
// Java: dist/game/log.cfg and GameServer constructor (`log/java%g.log`, `log/error%g.log`,
// 20 files kept). Same pattern as `login-server/src/logging.rs`.
// Java: log/handler/{ChatLogHandler,ItemLogHandler,GMAuditLogHandler}.java,
//       log/formatter/{ChatLogFormatter,ItemLogFormatter}.java, util/GMAudit.java
//
// Differences from Java, on purpose: `log.cfg` is a `java.util.logging` file and is not read.
// The level comes from `RUST_LOG` (default `info`). Files roll daily, the last 20 are kept.
// The error file holds ERROR events (Java's `ErrorLogHandler` keeps records with exceptions).
//
// The side logs have `useParentHandlers = false` in Java, so their lines never reach the console
// or the main game log here either:
// - target `chat` -> `log/chat.<date>.log`, lines `[dd MMM H:mm:ss] <message>` (ChatLogFormatter).
// - target `item` -> `log/item.<date>.log`, the same header (ItemLogFormatter).
// - target `gmaudit` -> `log/GMAudit/<gm name>.txt`, one file per GM, lines
//   `dd/MM/yyyy H:mm:ss>gm>action>target>params` (GMAudit.logAction). The message is
//   `gm>action>target>params`, the gm name is the text before the first `>`.
//   Java opens and appends the file for every line; so does this layer.
//   NOTE(java-quirk): Java's `log/gmaudit%g.log` handler never gets a line (`GMAudit` logs only
//   its own IO errors through that logger), so no such file is written.
// The item lines come from the item-process call sites, which are not ported yet (see the
// inventory modules); the file and the layer are ready for them.

use std::io::Write;
use std::path::{Path, PathBuf};

use l2_util::time::{local_offset_millis, now_millis};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};
use tracing_subscriber::filter::{FilterExt, LevelFilter, filter_fn};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

/// Name of the log directory (inside the server's root folder).
pub const LOG_DIR_NAME: &str = "log";
/// Daily log files are named `<prefix>.<date>.log`.
const MAIN_LOG_PREFIX: &str = "game";
const ERROR_LOG_PREFIX: &str = "game-error";
const CHAT_LOG_PREFIX: &str = "chat";
const ITEM_LOG_PREFIX: &str = "item";
const LOG_SUFFIX: &str = "log";
/// Java `FileHandler.count = 20`.
const KEEP_FILES: usize = 20;

/// The Java logger names (`Logger.getLogger("chat")` and so on).
pub const CHAT_TARGET: &str = "chat";
pub const ITEM_TARGET: &str = "item";
pub const GM_AUDIT_TARGET: &str = "gmaudit";
/// Java `GMAudit`: the folder of the per-GM files, under the log folder.
pub const GM_AUDIT_DIR_NAME: &str = "GMAudit";
/// Java `GMAudit.ILLEGAL_CHARACTERS`: replaced by `_` in a GM's file name.
const GM_AUDIT_ILLEGAL: [char; 15] = [
    '/', '\n', '\r', '\t', '\0', '\u{c}', '`', '?', '*', '\\', '<', '>', '|', '"', ':',
];

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

/// Installs the global logger: console, `log/game.<date>.log`, `log/game-error.<date>.log`, and
/// the chat, item and GM audit logs.
pub fn init(log_dir: &Path) -> Result<LogGuard, LoggingError> {
    let (subscriber, guard) = build(log_dir)?;
    subscriber
        .try_init()
        .map_err(|_| LoggingError::AlreadyInstalled)?;
    Ok(guard)
}

/// The subscriber `init` installs, without installing it (tests use it with `with_default`).
pub fn build(log_dir: &Path) -> Result<(impl Subscriber + Send + Sync + 'static, LogGuard), LoggingError> {
    // Java: mkdir the log folder (and `log/GMAudit` in the GMAudit static initializer).
    create_dir(log_dir)?;
    let gm_audit_dir = log_dir.join(GM_AUDIT_DIR_NAME);
    create_dir(&gm_audit_dir)?;
    let (main_writer, main_guard) = tracing_appender::non_blocking(appender(log_dir, MAIN_LOG_PREFIX)?);
    let (error_writer, error_guard) = tracing_appender::non_blocking(appender(log_dir, ERROR_LOG_PREFIX)?);
    let (chat_writer, chat_guard) = tracing_appender::non_blocking(appender(log_dir, CHAT_LOG_PREFIX)?);
    let (item_writer, item_guard) = tracing_appender::non_blocking(appender(log_dir, ITEM_LOG_PREFIX)?);

    let subscriber = tracing_subscriber::registry()
        .with(fmt::layer().with_filter(main_filter()))
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(main_writer)
                .with_filter(main_filter()),
        )
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(error_writer)
                .with_filter(LevelFilter::ERROR),
        )
        .with(side_layer(chat_writer, CHAT_TARGET))
        .with(side_layer(item_writer, ITEM_TARGET))
        .with(GmAuditLayer { dir: gm_audit_dir });
    Ok((
        subscriber,
        LogGuard {
            _guards: vec![main_guard, error_guard, chat_guard, item_guard],
        },
    ))
}

fn create_dir(path: &Path) -> Result<(), LoggingError> {
    std::fs::create_dir_all(path).map_err(|source| LoggingError::CreateDir {
        path: path.display().to_string(),
        source,
    })
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

/// The console and the main game log: the level filter, without the Java side logs.
fn main_filter<S: Subscriber>() -> impl tracing_subscriber::layer::Filter<S> + Send + Sync + 'static {
    env_filter().and(filter_fn(|meta| !is_side_log(meta.target())))
}

fn is_side_log(target: &str) -> bool {
    matches!(target, CHAT_TARGET | ITEM_TARGET | GM_AUDIT_TARGET)
}

/// A Java side logger that writes its lines to one file: INFO and above (`level = INFO`), only
/// its own target.
fn side_layer<S>(writer: NonBlocking, target: &'static str) -> impl Layer<S> + Send + Sync + 'static
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fmt::layer()
        .event_format(JavaLineFormat)
        .with_writer(writer)
        .with_filter(filter_fn(move |meta| {
            meta.target() == target && *meta.level() <= Level::INFO
        }))
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

// ---- Java side-log formats ---------------------------------------------------------------------

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Calendar fields of a local wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Civil {
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

/// Splits local wall-clock milliseconds (see `l2_util::time::local_now_millis`) into calendar
/// fields. Days come from the civil-calendar algorithm of Howard Hinnant (proleptic Gregorian).
fn civil(local_millis: i64) -> Civil {
    let secs = local_millis.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    Civil {
        year,
        month: u32::try_from(month).unwrap_or(1),
        day: u32::try_from(day).unwrap_or(1),
        hour: u32::try_from(rem / 3600).unwrap_or(0),
        minute: u32::try_from(rem % 3600 / 60).unwrap_or(0),
        second: u32::try_from(rem % 60).unwrap_or(0),
    }
}

/// `dd MMM H:mm:ss` of the chat and item logs (`ChatLogFormatter`), in the local zone.
fn chat_time(local_millis: i64) -> String {
    let c = civil(local_millis);
    let month = MONTHS[usize::try_from(c.month.saturating_sub(1)).unwrap_or(0) % 12];
    format!("{:02} {month} {}:{:02}:{:02}", c.day, c.hour, c.minute, c.second)
}

/// `dd/MM/yyyy H:mm:ss` of the GM audit lines (`GMAudit.DATE_FORMAT`), in the local zone.
fn gm_audit_time(local_millis: i64) -> String {
    let c = civil(local_millis);
    format!(
        "{:02}/{:02}/{:04} {}:{:02}:{:02}",
        c.day, c.month, c.year, c.hour, c.minute, c.second
    )
}

/// Local wall-clock milliseconds of now (`LocalDateTime.now()` / `Instant.atZone(systemDefault)`).
fn local_now() -> i64 {
    let now = now_millis();
    now + local_offset_millis(now)
}

/// Java `GMAudit.sanitizeFileName`: each illegal character becomes `_`.
fn sanitize_file_name(name: &str) -> String {
    name.chars()
        .map(|c| if GM_AUDIT_ILLEGAL.contains(&c) { '_' } else { c })
        .collect()
}

/// `[dd MMM H:mm:ss] <message>`, one line (Java `ChatLogFormatter` / `ItemLogFormatter`).
struct JavaLineFormat;

impl<S, N> FormatEvent<S, N> for JavaLineFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        write!(writer, "[{}] ", chat_time(local_now()))?;
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

/// Java `GMAudit.logAction`: appends `dd/MM/yyyy H:mm:ss>` and the message to the GM's own file.
/// It is a layer, not a call, so the admin and item code only emit `tracing` events.
struct GmAuditLayer {
    dir: PathBuf,
}

impl<S: Subscriber> Layer<S> for GmAuditLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() != GM_AUDIT_TARGET {
            return;
        }
        let mut message = MessageText::default();
        event.record(&mut message);
        let Some(text) = message.0 else {
            return;
        };
        // The gm name is the first field. Java passes the name in brackets as the file name
        // (`name [objectId]` -> `name [objectId].txt`).
        let gm = text.split('>').next().unwrap_or_default();
        let path = self.dir.join(format!("{}.txt", sanitize_file_name(gm)));
        let line = format!("{}>{text}\n", gm_audit_time(local_now()));
        if let Err(error) = append_line(&path, &line) {
            tracing::error!("Could not save GMAudit log for GM {gm}: {error}");
        }
    }
}

fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())
}

/// Reads the text of the `message` field of an event.
#[derive(Default)]
struct MessageText(Option<String>);

impl Visit for MessageText {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            // `fmt::Arguments` debug-prints as its text.
            self.0 = Some(format!("{value:?}"));
        }
    }
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
    fn chat_and_item_lines_go_to_their_own_files_only() {
        let root = tempfile::tempdir().unwrap();
        let log_dir = root.path().join(LOG_DIR_NAME);
        let (subscriber, guard) = build(&log_dir).unwrap();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "chat", "GENERAL [Player:Bob[268435460]] hello there");
            tracing::info!(target: "item", "DELETE:Reference, item 268435461:Adena(5), Player:Bob[268435460]");
            tracing::info!("Loaded 3 things.");
        });
        drop(guard);

        let chat = read_logs(&log_dir, "chat");
        let line = chat.lines().next().unwrap_or_default();
        // `[dd MMM H:mm:ss] ` then the message, like `ChatLogFormatter`.
        assert!(line.starts_with('['), "{chat}");
        assert!(
            line.ends_with("] GENERAL [Player:Bob[268435460]] hello there"),
            "{chat}"
        );
        let item = read_logs(&log_dir, "item");
        assert!(item.starts_with('['), "{item}");
        assert!(
            item.contains("DELETE:Reference, item 268435461:Adena(5)"),
            "{item}"
        );

        let main = read_logs(&log_dir, "game");
        assert!(main.contains("Loaded 3 things."), "{main}");
        assert!(!main.contains("GENERAL"), "{main}");
        assert!(!main.contains("DELETE:Reference"), "{main}");
    }

    #[test]
    fn gm_audit_lines_go_to_one_file_per_gm() {
        let root = tempfile::tempdir().unwrap();
        let log_dir = root.path().join(LOG_DIR_NAME);
        let (subscriber, guard) = build(&log_dir).unwrap();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "gmaudit", "Admin Name [77]>//heal>Target (5)>");
            tracing::info!(target: "gmaudit", "Admin Name [77]>//kick>Other (6)>reason");
            tracing::info!(target: "gmaudit", "bad/name:1>//x>y>");
        });
        drop(guard);

        let gm_dir = log_dir.join(GM_AUDIT_DIR_NAME);
        let text = std::fs::read_to_string(gm_dir.join("Admin Name [77].txt")).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "{text}");
        // `dd/MM/yyyy H:mm:ss>` then the logged text, like `GMAudit.logAction`.
        let (stamp, rest) = lines[0].split_once('>').unwrap();
        assert_eq!(rest, "Admin Name [77]>//heal>Target (5)>");
        assert_eq!(stamp.matches('/').count(), 2, "{stamp}");
        assert_eq!(stamp.matches(':').count(), 2, "{stamp}");
        assert!(
            lines[1].ends_with(">Admin Name [77]>//kick>Other (6)>reason"),
            "{text}"
        );

        // Java replaces the illegal characters of the name with `_`.
        let sanitized = std::fs::read_to_string(gm_dir.join("bad_name_1.txt")).unwrap();
        assert!(sanitized.contains(">bad/name:1>//x>y>"), "{sanitized}");

        let main = read_logs(&log_dir, "game");
        assert!(!main.contains("//heal"), "{main}");
    }

    #[test]
    fn sanitize_replaces_every_java_illegal_character() {
        assert_eq!(sanitize_file_name("a/b\\c:d?e*f"), "a_b_c_d_e_f");
        assert_eq!(sanitize_file_name("Name [12]"), "Name [12]");
    }

    #[test]
    fn civil_time_matches_known_instants() {
        // 1970-01-01T00:00:00
        assert_eq!(
            civil(0),
            Civil {
                year: 1970,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0
            }
        );
        // 2001-09-09T01:46:40Z (1_000_000_000 seconds)
        assert_eq!(
            civil(1_000_000_000_000),
            Civil {
                year: 2001,
                month: 9,
                day: 9,
                hour: 1,
                minute: 46,
                second: 40
            }
        );
        assert_eq!(chat_time(1_000_000_000_000), "09 Sep 1:46:40");
        assert_eq!(gm_audit_time(1_000_000_000_000), "09/09/2001 1:46:40");
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
