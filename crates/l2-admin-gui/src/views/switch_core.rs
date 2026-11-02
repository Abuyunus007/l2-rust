//! The logic of the "Switch to Rust" assistant (M9-S01), without any drawing: which keys of which
//! `.ini` files change, the refusal while the Java game server is registered at the login server,
//! the backup gate, and the way back. `views/switch.rs` only calls these.
//!
//! The switch is "Rust replaces Java" (D-57): the Rust game server takes the Java server's port
//! (`GameserverPort = 7777`, optionally its id) and every `SideBySide.ini` switch goes to True
//! (D-49, D-51..D-54), so the Rust server does all global jobs the Java server did.
//!
//! The first forward step writes `switch-before.ini` into the run folder: the raw values the nine
//! keys had. "Back to side by side" writes exactly those back, so a round trip is byte-identical.
//! Nothing here touches a database or a server; every write goes through `l2_config::ini_edit`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use l2_admin_api::{BackupStatus, LoginStatus};
use l2_config::ini_edit::{self, IniDoc, IniEditError};

use super::setup_core::Layout;

/// The Rust-only switches of `config/SideBySide.ini` (D-49, D-51, D-52, D-53).
pub const SWITCH_KEYS: [&str; 7] = [
    "DailyResetDatabaseWrites",
    "ClanMaintenanceDatabaseWrites",
    "MailExpiryDatabaseWrites",
    "FishingChampionshipDatabaseWrites",
    "CycleDatabaseWrites",
    "OfflineTradeDatabaseWrites",
    "SiegeDatabaseWrites",
];
pub const PORT_KEY: &str = "GameserverPort";
pub const ID_KEY: &str = "RequestServerID";
/// The port and server id of the Java game server (Sieghardt), D-39.
pub const JAVA_PORT: u16 = 7777;
pub const JAVA_SERVER_ID: u8 = 2;
/// The port the Rust game server has side by side (D-39); "back" uses it without a snapshot.
pub const SIDE_BY_SIDE_PORT: u16 = 7778;
/// Where the values from before the switch are kept (in the run folder).
pub const SNAPSHOT_FILE: &str = "switch-before.ini";

const SIDE_BY_SIDE_FILE: &str = "SideBySide.ini";
const SERVER_FILE: &str = "Server.ini";

#[derive(Debug, thiserror::Error)]
pub enum SwitchError {
    #[error("{0}")]
    Refused(String),
    #[error("{file}: {source}")]
    Ini {
        file: &'static str,
        source: IniEditError,
    },
    #[error("{file}: {source}")]
    Io { file: String, source: io::Error },
}

/// Which game server id the Rust server has after the switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdChoice {
    /// Sieghardt's id (2): the `gameservers` row and `hexid.txt` of the Java server are used.
    TakeJava,
    /// Keep the Rust server's own id (3 in the side-by-side set-up).
    Keep,
}

/// One key of one file with the value now and the value after the change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub file: &'static str,
    pub key: &'static str,
    pub from: String,
    pub to: String,
}

/// The raw values the nine keys have in the files now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Current {
    /// `(key, raw value)` of [`SWITCH_KEYS`], in that order.
    pub switches: Vec<(&'static str, String)>,
    pub game_port: String,
    pub server_id: String,
}

impl Current {
    /// The Rust game server's id as the login server sees it now; `None` when not a number.
    pub fn server_id_number(&self) -> Option<u8> {
        self.server_id.trim().parse().ok()
    }

    pub fn all_switches_on(&self) -> bool {
        self.switches.iter().all(|(_, v)| is_true(v))
    }
}

fn is_true(raw: &str) -> bool {
    raw.trim().eq_ignore_ascii_case("true")
}

fn read_doc(path: &Path, file: &'static str) -> Result<IniDoc, SwitchError> {
    let bytes = fs::read(path).map_err(|source| SwitchError::Io {
        file: path.display().to_string(),
        source,
    })?;
    let text = String::from_utf8(bytes).map_err(|_| SwitchError::Ini {
        file,
        source: IniEditError::NotUtf8,
    })?;
    Ok(IniDoc::parse(&text))
}

fn value_of(doc: &IniDoc, key: &str, file: &'static str) -> Result<String, SwitchError> {
    // The last occurrence wins, like `ini_edit::set_value` and the servers.
    doc.entries()
        .into_iter()
        .rev()
        .find(|e| e.key == key)
        .map(|e| e.value)
        .ok_or_else(|| SwitchError::Ini {
            file,
            source: IniEditError::KeyNotFound(key.to_owned()),
        })
}

/// Reads the nine keys from `SideBySide.ini` and `Server.ini` of the game server.
pub fn read_current(layout: &Layout) -> Result<Current, SwitchError> {
    let sbs = read_doc(&layout.game_config(SIDE_BY_SIDE_FILE), SIDE_BY_SIDE_FILE)?;
    let server = read_doc(&layout.game_config(SERVER_FILE), SERVER_FILE)?;
    let mut switches = Vec::new();
    for key in SWITCH_KEYS {
        switches.push((key, value_of(&sbs, key, SIDE_BY_SIDE_FILE)?));
    }
    Ok(Current {
        switches,
        game_port: value_of(&server, PORT_KEY, SERVER_FILE)?,
        server_id: value_of(&server, ID_KEY, SERVER_FILE)?,
    })
}

/// What the switch will change (only keys whose value differs), for the assistant to show.
pub fn plan_forward(current: &Current, choice: IdChoice) -> Vec<Change> {
    let mut out = Vec::new();
    for (key, from) in &current.switches {
        if !is_true(from) {
            out.push(Change {
                file: SIDE_BY_SIDE_FILE,
                key,
                from: from.clone(),
                to: "True".to_owned(),
            });
        }
    }
    if current.game_port.trim() != JAVA_PORT.to_string() {
        out.push(Change {
            file: SERVER_FILE,
            key: PORT_KEY,
            from: current.game_port.clone(),
            to: JAVA_PORT.to_string(),
        });
    }
    if choice == IdChoice::TakeJava && current.server_id.trim() != JAVA_SERVER_ID.to_string() {
        out.push(Change {
            file: SERVER_FILE,
            key: ID_KEY,
            from: current.server_id.clone(),
            to: JAVA_SERVER_ID.to_string(),
        });
    }
    out
}

/// The result of waiting for a backup the assistant started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupOutcome {
    Running,
    Ok(String),
    Failed(String),
}

/// Reads a `BackupState` answer. `started_ms` is the clock time of the click: a result older than
/// that is the previous backup and does not count (so a stale "ok" never passes the gate).
pub fn backup_outcome(status: &BackupStatus, started_ms: i64) -> BackupOutcome {
    if status.running {
        return BackupOutcome::Running;
    }
    match &status.last {
        Some(last) if last.finished_ms >= started_ms => {
            if last.ok {
                BackupOutcome::Ok(last.message.clone())
            } else {
                BackupOutcome::Failed(last.message.clone())
            }
        }
        // The supervisor has not started it yet (or the request was lost).
        _ => BackupOutcome::Running,
    }
}

/// `Some(reason)` while a game server other than the Rust one is registered at the login server.
/// `rust_id` is the Rust server's `RequestServerID` before the switch. `None` status (the login
/// server did not answer) refuses too: nothing is changed without knowing.
pub fn java_registered(status: Option<&LoginStatus>, rust_id: Option<u8>) -> Option<String> {
    let Some(status) = status else {
        return Some("The login server's server list is not known yet; ask it again.".to_owned());
    };
    let others: Vec<String> = status
        .game_servers
        .iter()
        .filter(|s| s.authed && Some(s.id) != rust_id)
        .map(|s| format!("{} (id {})", s.name, s.id))
        .collect();
    if others.is_empty() {
        None
    } else {
        Some(format!(
            "A game server is still registered at the login server: {}. Stop the Java game server first.",
            others.join(", ")
        ))
    }
}

/// What the assistant knows before it may write.
#[derive(Debug, Clone, Copy)]
pub struct Gate<'a> {
    pub backup_ok: bool,
    pub login: Option<&'a LoginStatus>,
}

/// Writes the switch: all seven `SideBySide.ini` switches True, `GameserverPort = 7777` and (with
/// [`IdChoice::TakeJava`]) `RequestServerID = 2`. Refuses without a good backup or while the Java
/// game server is registered. Returns the number of keys changed.
pub fn apply_forward(layout: &Layout, choice: IdChoice, gate: Gate<'_>) -> Result<usize, SwitchError> {
    if !gate.backup_ok {
        return Err(SwitchError::Refused("Run the backup first.".to_owned()));
    }
    let current = read_current(layout)?;
    if let Some(why) = java_registered(gate.login, current.server_id_number()) {
        return Err(SwitchError::Refused(why));
    }
    save_snapshot(layout, &current)?;
    let changes = plan_forward(&current, choice);
    write_changes(layout, &changes)
}

/// "Back to side by side": writes the values from before the switch. Without a snapshot (the
/// assistant never ran) it sets the switches to False and the port to 7778 and keeps the id.
pub fn apply_back(layout: &Layout) -> Result<usize, SwitchError> {
    let current = read_current(layout)?;
    let target = match load_snapshot(layout) {
        Some(s) => s,
        None => Current {
            switches: current
                .switches
                .iter()
                .map(|(k, _)| (*k, "False".to_owned()))
                .collect(),
            game_port: SIDE_BY_SIDE_PORT.to_string(),
            server_id: current.server_id.clone(),
        },
    };
    let mut changes = Vec::new();
    for ((key, now), (_, to)) in current.switches.iter().zip(&target.switches) {
        changes.push(Change {
            file: SIDE_BY_SIDE_FILE,
            key,
            from: now.clone(),
            to: to.clone(),
        });
    }
    changes.push(Change {
        file: SERVER_FILE,
        key: PORT_KEY,
        from: current.game_port.clone(),
        to: target.game_port,
    });
    changes.push(Change {
        file: SERVER_FILE,
        key: ID_KEY,
        from: current.server_id.clone(),
        to: target.server_id,
    });
    let n = write_changes(layout, &changes)?;
    let _ = fs::remove_file(snapshot_path(layout));
    Ok(n)
}

/// Whether "Back to side by side" has values to restore.
pub fn has_snapshot(layout: &Layout) -> bool {
    snapshot_path(layout).is_file()
}

fn write_changes(layout: &Layout, changes: &[Change]) -> Result<usize, SwitchError> {
    let mut total = 0;
    for file in [SIDE_BY_SIDE_FILE, SERVER_FILE] {
        let list: Vec<(String, String)> = changes
            .iter()
            .filter(|c| c.file == file)
            .map(|c| (c.key.to_owned(), c.to.clone()))
            .collect();
        if list.is_empty() {
            continue;
        }
        total += ini_edit::save_changes(&layout.game_config(file), &list)
            .map_err(|source| SwitchError::Ini { file, source })?;
    }
    Ok(total)
}

fn snapshot_path(layout: &Layout) -> PathBuf {
    layout.run_folder.join(SNAPSHOT_FILE)
}

/// Keeps the first snapshot: pressing the switch twice must not overwrite the original values with
/// the switched ones.
fn save_snapshot(layout: &Layout, current: &Current) -> Result<(), SwitchError> {
    let path = snapshot_path(layout);
    if path.exists() {
        return Ok(());
    }
    let mut text =
        String::from("# Values before \"Switch to Rust\"; \"Back to side by side\" writes them back.\n");
    for (key, value) in &current.switches {
        text.push_str(&format!("{key} = {value}\n"));
    }
    text.push_str(&format!(
        "{PORT_KEY} = {}\n{ID_KEY} = {}\n",
        current.game_port, current.server_id
    ));
    ini_edit::write_atomic(&path, &text).map_err(|source| SwitchError::Ini {
        file: SNAPSHOT_FILE,
        source,
    })
}

fn load_snapshot(layout: &Layout) -> Option<Current> {
    let text = fs::read_to_string(snapshot_path(layout)).ok()?;
    let doc = IniDoc::parse(&text);
    let get = |key: &str| value_of(&doc, key, SNAPSHOT_FILE).ok();
    let mut switches = Vec::new();
    for key in SWITCH_KEYS {
        switches.push((key, get(key)?));
    }
    Some(Current {
        switches,
        game_port: get(PORT_KEY)?,
        server_id: get(ID_KEY)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_admin_api::{BackupResult, GameServerStatus};

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A run folder built from the real `dist/` files (a temp copy; nothing outside is touched)
    /// with the side-by-side values of D-39: port 7778, id 3. CRLF in SideBySide.ini on purpose.
    fn layout_from_dist() -> Option<(tempfile::TempDir, Layout)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cfg = root.join("dist/game/config");
        if !cfg.join("SideBySide.ini").is_file() {
            println!("skipped: dist/ not found");
            return None;
        }
        let tmp = tempfile::tempdir().unwrap();
        let mut layout = Layout::repo(&root);
        layout.run_folder = tmp.path().to_path_buf();
        layout.game_dir = tmp.path().join("game");
        let sbs = fs::read_to_string(cfg.join("SideBySide.ini")).unwrap();
        // Mixed: one switch already on, one in lower case.
        let sbs = sbs
            .replace(
                "MailExpiryDatabaseWrites = False",
                "MailExpiryDatabaseWrites = True",
            )
            .replace("SiegeDatabaseWrites = False", "SiegeDatabaseWrites = false")
            .replace("\r\n", "\n")
            .replace('\n', "\r\n");
        write(&layout.game_config("SideBySide.ini"), &sbs);
        let server = fs::read_to_string(cfg.join("Server.ini")).unwrap();
        let server = server
            .replace("GameserverPort = 7777", "GameserverPort = 7778")
            .replace("RequestServerID = 1", "RequestServerID = 3");
        write(&layout.game_config("Server.ini"), &server);
        Some((tmp, layout))
    }

    fn server(id: u8, name: &str, authed: bool) -> GameServerStatus {
        GameServerStatus {
            id,
            name: name.to_owned(),
            authed,
            online: 0,
            max_players: 100,
            status: "Auto".to_owned(),
        }
    }

    fn login(servers: Vec<GameServerStatus>) -> LoginStatus {
        LoginStatus {
            game_servers: servers,
            authed_clients: 0,
        }
    }

    fn files(layout: &Layout) -> (Vec<u8>, Vec<u8>) {
        (
            fs::read(layout.game_config("SideBySide.ini")).unwrap(),
            fs::read(layout.game_config("Server.ini")).unwrap(),
        )
    }

    fn gate(login: Option<&LoginStatus>, backup_ok: bool) -> Gate<'_> {
        Gate { backup_ok, login }
    }

    #[test]
    fn forward_sets_everything_and_back_is_byte_identical() {
        let Some((_tmp, layout)) = layout_from_dist() else {
            return;
        };
        let before = files(&layout);
        let ok_login = login(vec![server(3, "Kain", true)]);
        let planned = plan_forward(&read_current(&layout).unwrap(), IdChoice::TakeJava);
        // 6 switches off (one is on already, one is lower case "false"), port 7778, id 3.
        assert_eq!(planned.len(), 6 + 1 + 1);
        let n = apply_forward(&layout, IdChoice::TakeJava, gate(Some(&ok_login), true)).unwrap();
        assert_eq!(n, planned.len());
        let now = read_current(&layout).unwrap();
        assert!(now.all_switches_on());
        assert_eq!((now.game_port.as_str(), now.server_id.as_str()), ("7777", "2"));
        assert!(has_snapshot(&layout));
        // Only the values changed: the line endings stay.
        let (sbs, _) = files(&layout);
        assert!(String::from_utf8(sbs).unwrap().contains("\r\n"));

        // Pressing it twice keeps the first snapshot; the id is already 2 so nothing changes.
        assert_eq!(
            apply_forward(&layout, IdChoice::TakeJava, gate(Some(&login(vec![])), true)).unwrap(),
            0
        );

        apply_back(&layout).unwrap();
        assert_eq!(files(&layout), before);
        assert!(!has_snapshot(&layout));
    }

    #[test]
    fn keep_choice_leaves_the_server_id() {
        let Some((_tmp, layout)) = layout_from_dist() else {
            return;
        };
        let l = login(vec![]);
        apply_forward(&layout, IdChoice::Keep, gate(Some(&l), true)).unwrap();
        let now = read_current(&layout).unwrap();
        assert_eq!((now.game_port.as_str(), now.server_id.as_str()), ("7777", "3"));
    }

    #[test]
    fn back_without_a_snapshot_goes_to_side_by_side_defaults() {
        let Some((_tmp, layout)) = layout_from_dist() else {
            return;
        };
        let plan = plan_forward(&read_current(&layout).unwrap(), IdChoice::Keep);
        write_changes(&layout, &plan).unwrap();
        apply_back(&layout).unwrap();
        let now = read_current(&layout).unwrap();
        assert!(now.switches.iter().all(|(_, v)| v == "False"));
        assert_eq!(now.game_port, "7778");
    }

    #[test]
    fn refuses_while_a_java_server_is_registered_and_writes_nothing() {
        let Some((_tmp, layout)) = layout_from_dist() else {
            return;
        };
        let before = files(&layout);
        // Java (id 2) registered, Rust (id 3) too.
        let l = login(vec![server(2, "Sieghardt", true), server(3, "Kain", true)]);
        let err = apply_forward(&layout, IdChoice::TakeJava, gate(Some(&l), true)).unwrap_err();
        assert!(
            matches!(err, SwitchError::Refused(ref m) if m.contains("Sieghardt")),
            "{err}"
        );
        assert_eq!(files(&layout), before);
        assert!(!has_snapshot(&layout));
        // A server in the list that is not connected does not count.
        let l = login(vec![server(2, "Sieghardt", false), server(3, "Kain", true)]);
        assert!(apply_forward(&layout, IdChoice::TakeJava, gate(Some(&l), true)).is_ok());
    }

    #[test]
    fn refuses_without_a_backup_or_without_a_login_answer() {
        let Some((_tmp, layout)) = layout_from_dist() else {
            return;
        };
        let before = files(&layout);
        let l = login(vec![]);
        assert!(matches!(
            apply_forward(&layout, IdChoice::Keep, gate(Some(&l), false)),
            Err(SwitchError::Refused(_))
        ));
        assert!(matches!(
            apply_forward(&layout, IdChoice::Keep, gate(None, true)),
            Err(SwitchError::Refused(_))
        ));
        assert_eq!(files(&layout), before);
    }

    #[test]
    fn backup_outcome_ignores_an_older_result() {
        let mut s = BackupStatus {
            enabled: false,
            running: false,
            interval_hours: 24,
            keep: 7,
            folder: String::new(),
            database: None,
            last: Some(BackupResult {
                ok: true,
                message: "old.sql".to_owned(),
                finished_ms: 100,
            }),
            next_run_ms: None,
        };
        assert_eq!(backup_outcome(&s, 200), BackupOutcome::Running);
        s.last = Some(BackupResult {
            ok: true,
            message: "new.sql".to_owned(),
            finished_ms: 300,
        });
        assert_eq!(backup_outcome(&s, 200), BackupOutcome::Ok("new.sql".to_owned()));
        s.last = Some(BackupResult {
            ok: false,
            message: "mysqldump missing".to_owned(),
            finished_ms: 300,
        });
        assert!(matches!(backup_outcome(&s, 200), BackupOutcome::Failed(_)));
        s.running = true;
        assert_eq!(backup_outcome(&s, 200), BackupOutcome::Running);
    }

    #[test]
    fn java_registered_matches_the_rust_id_only() {
        let l = login(vec![server(3, "Kain", true)]);
        assert!(java_registered(Some(&l), Some(3)).is_none());
        assert!(java_registered(Some(&l), Some(2)).is_some());
        assert!(java_registered(None, Some(3)).is_some());
    }
}
