#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Backups with a stand-in `mysqldump` (`backup-test-dump`): no database is ever contacted.

mod common;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use l2_admin_api::{AdminClient, Request, Response};
use l2_supervisor::Running;
use l2_supervisor::backup::{BackupConfig, BackupError, BackupService, run_backup};

use common::{config, eventually};

const FAKE_DUMP: &str = env!("CARGO_BIN_EXE_backup-test-dump");
const PASSWORD: &str = "fake-test-password";

fn now() -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2026, 11, 2)
        .unwrap()
        .and_hms_opt(3, 0, 0)
        .unwrap()
}

/// A folder with a `Database.ini` for `database` and a config pointing at the fake dump.
fn setup(dir: &Path, database: &str, keep: u32) -> BackupConfig {
    fs::write(
        dir.join("Database.ini"),
        format!(
            "URL = jdbc:mysql://dbhost:3399/{database}?useSSL=false\nLogin = gm\nPassword = {PASSWORD}\n"
        ),
    )
    .unwrap();
    BackupConfig {
        enabled: false,
        interval_hours: 24,
        keep,
        folder: dir.join("backups"),
        mysqldump_path: Some(PathBuf::from(FAKE_DUMP)),
        database_ini: dir.join("Database.ini"),
    }
}

fn gunzip(path: &Path) -> String {
    let mut text = String::new();
    flate2::read::GzDecoder::new(fs::File::open(path).unwrap())
        .read_to_string(&mut text)
        .unwrap();
    text
}

#[test]
fn backup_is_a_gzipped_dump_with_the_password_only_in_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let config = setup(dir.path(), "l2test", 7);
    let info = run_backup(&config, now()).unwrap();
    assert_eq!(info.name, "l2test-20261102-030000.sql.gz");
    let path = config.folder.join(&info.name);
    assert_eq!(info.size, fs::metadata(&path).unwrap().len());

    let dump = gunzip(&path);
    assert!(dump.contains("CREATE TABLE t (id int);"));
    // Command line: the options and the database, no password.
    let args = dump.lines().find(|l| l.starts_with("-- args:")).unwrap();
    assert_eq!(
        args,
        "-- args: --single-transaction --routines --events --host=dbhost --port=3399 --user=gm l2test"
    );
    assert!(!args.contains(PASSWORD));
    // Environment: it arrived there.
    assert!(dump.contains(&format!("-- MYSQL_PWD: {PASSWORD}")));
    // No half-written file stays.
    assert!(
        !fs::read_dir(&config.folder).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part"))
    );
}

#[test]
fn old_backups_are_rotated() {
    let dir = tempfile::tempdir().unwrap();
    let config = setup(dir.path(), "l2test", 2);
    fs::create_dir_all(&config.folder).unwrap();
    for day in ["20261029", "20261030", "20261031"] {
        fs::write(config.folder.join(format!("l2test-{day}-030000.sql.gz")), b"old").unwrap();
    }
    run_backup(&config, now()).unwrap();
    let mut names: Vec<String> = fs::read_dir(&config.folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["l2test-20261031-030000.sql.gz", "l2test-20261102-030000.sql.gz"]
    );
}

#[test]
fn a_failing_dump_leaves_no_file_and_no_password_in_the_error() {
    let dir = tempfile::tempdir().unwrap();
    let config = setup(dir.path(), "failing", 7);
    let err = run_backup(&config, now()).unwrap_err();
    assert!(matches!(err, BackupError::Dump { .. }), "{err}");
    let text = err.to_string();
    assert!(text.contains("Access denied"), "{text}");
    assert!(!text.contains(PASSWORD));
    assert!(fs::read_dir(&config.folder).unwrap().next().is_none());
}

#[test]
fn missing_database_ini_or_mysqldump_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = setup(dir.path(), "l2test", 7);
    config.mysqldump_path = Some(dir.path().join("no-such-mysqldump"));
    assert!(matches!(
        run_backup(&config, now()),
        Err(BackupError::NoMysqldump)
    ));
    config.database_ini = dir.path().join("missing.ini");
    assert!(matches!(run_backup(&config, now()), Err(BackupError::Config(_))));
}

#[tokio::test]
async fn admin_api_backup_now_list_and_state() {
    let dir = tempfile::tempdir().unwrap();
    let backup = setup(dir.path(), "l2test", 7);
    let folder = backup.folder.clone();
    let running = Running::start_with_backup(config(vec![]), backup).await.unwrap();
    let (client, _events) = AdminClient::connect(&format!("ws://{}", running.local_addr()), "secret")
        .await
        .unwrap();

    let Response::BackupState { state } = client.request(Request::BackupState).await.unwrap() else {
        panic!("expected backup_state");
    };
    assert!(!state.running && !state.enabled);
    assert!(state.last.is_none() && state.next_run_ms.is_none());
    let db = state.database.clone().unwrap();
    assert_eq!(
        (db.host.as_str(), db.port, db.user.as_str(), db.database.as_str()),
        ("dbhost", 3399, "gm", "l2test")
    );
    assert!(!format!("{state:?}").contains(PASSWORD));
    assert_eq!(
        client.request(Request::ListBackups).await.unwrap(),
        Response::Backups { backups: vec![] }
    );

    assert_eq!(client.request(Request::BackupNow).await.unwrap(), Response::Ok);
    let mut last = None;
    for _ in 0..400 {
        let Response::BackupState { state } = client.request(Request::BackupState).await.unwrap() else {
            panic!("expected backup_state");
        };
        if state.last.is_some() && !state.running {
            last = state.last;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let last = last.expect("the backup finishes");
    assert!(last.ok, "{}", last.message);
    let Response::Backups { backups } = client.request(Request::ListBackups).await.unwrap() else {
        panic!("expected backups");
    };
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0].name, last.message);
    assert!(backups[0].size > 0);
    assert!(folder.join(&backups[0].name).is_file());
    running.shutdown().await;
}

#[tokio::test]
async fn only_one_backup_runs_at_a_time() {
    let dir = tempfile::tempdir().unwrap();
    let service = BackupService::new(setup(dir.path(), "l2test", 7));
    service.start_now().unwrap();
    // The flag is set before the task starts, so the second request is refused at once.
    assert!(matches!(service.start_now(), Err(BackupError::AlreadyRunning)));
    eventually("the backup to finish", || !service.status().running).await;
    assert!(service.status().last.unwrap().ok);
    // After it, the next one is accepted again.
    let info = service.run_now().await.unwrap();
    assert!(info.name.ends_with(".sql.gz"));
}
