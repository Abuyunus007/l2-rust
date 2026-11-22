//! The server folder: the programs, the login and game folders from `dist/`, the geodata, the
//! supervisor settings and the start scripts (the layout of `tools/package.py`).
//!
//! Run again on an existing folder it updates it: the programs, the datapack and the scripts are
//! replaced, but every settings file that is there already (`config/`, `banned_ip.cfg`,
//! `supervisor.ini`, `Backup.ini`, `hexid.txt`) is kept, so an update never loses a setting.
//! A shipped file that a newer `dist/` no longer has is removed (the list of the shipped files is
//! [`MANIFEST`]); files the owner added himself are never touched.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The list of the files copied from `dist/` (not the settings), one relative path per line.
pub const MANIFEST: &str = ".l2setup-files";

/// The programs of a server folder.
pub const PROGRAMS: [&str; 5] = [
    "login-server",
    "game-server",
    "l2-supervisor",
    "l2-admin-gui",
    "l2-db-installer",
];

/// The file name of a program on this system.
pub fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// The programs `release` (normally `target/release`) does not have yet.
pub fn missing_programs(release: &Path) -> Vec<&'static str> {
    PROGRAMS
        .into_iter()
        .filter(|p| !release.join(exe(p)).is_file())
        .collect()
}

/// What [`assemble`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// The folder was new.
    pub created: bool,
    /// Settings files that were there already and were kept, relative to the folder.
    pub kept: Vec<PathBuf>,
    /// Geodata files copied (files of the same size already there are skipped).
    pub geodata_copied: usize,
    /// Geodata files in the folder after the copy.
    pub geodata_total: usize,
    /// Files of an earlier `dist/` that this one no longer has, removed.
    pub removed: Vec<PathBuf>,
}

/// Builds or updates the server folder `out` from `release` (the built programs) and `dist`
/// (the repository's `dist/`), with the `*.l2j` files of `geodata` when given.
pub fn assemble(release: &Path, dist: &Path, out: &Path, geodata: Option<&Path>) -> io::Result<Report> {
    let mut report = Report {
        created: !out.exists(),
        ..Report::default()
    };
    let bin = out.join("bin");
    fs::create_dir_all(&bin)?;
    for program in PROGRAMS {
        let name = exe(program);
        fs::copy(release.join(&name), bin.join(&name))?;
    }

    let mut shipped = Vec::new();
    for tree in ["login", "game/config", "game/data", "db_installer"] {
        copy_tree(&dist.join(tree), &out.join(tree), out, &mut report, &mut shipped)?;
    }
    remove_unshipped(out, &shipped, &mut report)?;

    let geo_dir = out.join("game/data/geodata");
    fs::create_dir_all(&geo_dir)?;
    if let Some(geodata) = geodata {
        for entry in fs::read_dir(geodata)? {
            let path = entry?.path();
            if path.extension().is_none_or(|e| !e.eq_ignore_ascii_case("l2j")) {
                continue;
            }
            let Some(name) = path.file_name() else {
                continue;
            };
            let target = geo_dir.join(name);
            // A region file of the same size is the same file: an update does not copy
            // gigabytes again.
            if target
                .metadata()
                .is_ok_and(|m| Some(m.len()) == path.metadata().ok().map(|s| s.len()))
            {
                continue;
            }
            fs::copy(&path, &target)?;
            report.geodata_copied += 1;
        }
    }
    report.geodata_total = fs::read_dir(&geo_dir)?
        .filter_map(Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("l2j"))
        })
        .count();

    write_if_missing(&bin.join("supervisor.ini"), &supervisor_ini(), out, &mut report)?;
    write_if_missing(&bin.join("Backup.ini"), BACKUP_INI, out, &mut report)?;
    fs::write(out.join("README.txt"), readme())?;
    write_start_scripts(out)?;
    Ok(report)
}

/// A settings file: kept when it is there already. Everything below a `config` folder, and the
/// login server's banned IP list.
fn is_setting(relative: &Path) -> bool {
    relative.components().any(|c| c.as_os_str() == "config")
        || relative.file_name().is_some_and(|n| n == "banned_ip.cfg")
}

/// Copies `from` into `to`: new files always, existing ones only when they are not settings.
/// `shipped` gets the relative path (with `/`) of every file that is not a setting.
fn copy_tree(
    from: &Path,
    to: &Path,
    out: &Path,
    report: &mut Report,
    shipped: &mut Vec<String>,
) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&source, &target, out, report, shipped)?;
            continue;
        }
        let relative = target.strip_prefix(out).unwrap_or(&target).to_path_buf();
        if is_setting(&relative) {
            if target.exists() {
                report.kept.push(relative);
                continue;
            }
        } else {
            shipped.push(slash_path(&relative));
        }
        fs::copy(&source, &target)?;
    }
    Ok(())
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Removes the files the last setup shipped that `shipped` (this setup's) no longer has, then
/// writes the new list. Settings and geodata are never removed.
fn remove_unshipped(out: &Path, shipped: &[String], report: &mut Report) -> io::Result<()> {
    let list = out.join(MANIFEST);
    let before = fs::read_to_string(&list).unwrap_or_default();
    for line in before.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let relative = PathBuf::from(line);
        if shipped.iter().any(|s| s == line)
            || is_setting(&relative)
            || line.starts_with("game/data/geodata/")
            || relative.components().any(|c| c.as_os_str() == "..")
        {
            continue;
        }
        let path = out.join(&relative);
        if path.is_file() {
            fs::remove_file(&path)?;
            report.removed.push(relative);
        }
    }
    fs::write(list, shipped.join("\n") + "\n")
}

fn write_if_missing(path: &Path, text: &str, out: &Path, report: &mut Report) -> io::Result<()> {
    if path.exists() {
        report
            .kept
            .push(path.strip_prefix(out).unwrap_or(path).to_path_buf());
        return Ok(());
    }
    fs::write(path, text)
}

/// The start scripts of this system: `.cmd` on Windows, `.sh` elsewhere.
fn write_start_scripts(out: &Path) -> io::Result<()> {
    if cfg!(windows) {
        fs::write(
            out.join("Start-AdminGUI.cmd"),
            "@echo off\r\ncd /d \"%~dp0bin\"\r\nstart \"\" \"l2-admin-gui.exe\"\r\n",
        )?;
        fs::write(
            out.join("Start-Servers.cmd"),
            "@echo off\r\ncd /d \"%~dp0bin\"\r\n\"l2-supervisor.exe\" supervisor.ini\r\nif errorlevel 1 pause\r\n",
        )?;
        fs::write(
            out.join("Install-Database.cmd"),
            "@echo off\r\ncd /d \"%~dp0bin\"\r\n\"l2-db-installer.exe\" --sql \"%~dp0db_installer\\sql\"\r\nif errorlevel 1 pause\r\n",
        )?;
    } else {
        write_shell(
            &out.join("start-admin-gui.sh"),
            "#!/bin/sh\ncd \"$(dirname \"$0\")/bin\" && exec ./l2-admin-gui\n",
        )?;
        write_shell(
            &out.join("start-servers.sh"),
            "#!/bin/sh\ncd \"$(dirname \"$0\")/bin\" && exec ./l2-supervisor supervisor.ini\n",
        )?;
        write_shell(
            &out.join("install-database.sh"),
            "#!/bin/sh\ncd \"$(dirname \"$0\")/bin\" && exec ./l2-db-installer --sql ../db_installer/sql \"$@\"\n",
        )?;
    }
    Ok(())
}

fn write_shell(path: &Path, text: &str) -> io::Result<()> {
    fs::write(path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// `bin/supervisor.ini`: no token (D-56). The servers start with the supervisor, so the start
/// script without the GUI starts them too (the GUI shows them running).
pub fn supervisor_ini() -> String {
    let autostart = "True";
    let login = exe("login-server");
    let game = exe("game-server");
    format!(
        "# Supervisor settings of this server folder. The admin GUI reads Listen and Token from here.\n\
# Relative paths are relative to this file's folder (bin/).\n\
# Token and AdminToken stay empty: on this machine the GUI needs none, and the supervisor hands\n\
# the servers theirs, so nothing has to be typed (D-56).\n\
Listen = 127.0.0.1:7900\n\
Token =\n\
LogBufferLines = 10000\n\
StopTimeoutSecs = 30\n\
RestartDelaySecs = 10\n\
Servers = login, game\n\
\n\
Server.login.Command = ./{login}\n\
Server.login.Args =\n\
Server.login.WorkingDir = ../login\n\
Server.login.AdminEndpoint = 127.0.0.1:7901\n\
Server.login.AdminToken =\n\
Server.login.Autostart = {autostart}\n\
Server.login.RestartOnExitCode = 2\n\
\n\
Server.game.Command = ./{game}\n\
Server.game.Args = --root .\n\
Server.game.WorkingDir = ../game\n\
Server.game.AdminEndpoint = 127.0.0.1:7902\n\
Server.game.AdminToken =\n\
Server.game.Autostart = {autostart}\n\
Server.game.RestartOnExitCode = 2\n"
    )
}

/// `bin/Backup.ini`: the scheduled backups, off until the owner turns them on.
pub const BACKUP_INI: &str = "# Database backups of the supervisor. Relative paths are relative to this file's folder (bin/).\n\
# A backup is mysqldump --single-transaction --routines --events <db>, written gzip-compressed to\n\
# <Folder>/<db>-YYYYMMDD-HHMMSS.sql.gz. The password comes from Database.ini and is never logged.\n\
# \"Backup now\" in the admin GUI (Backups) works whatever Enabled says.\n\
Enabled = False\n\
IntervalHours = 24\n\
Keep = 7\n\
Folder = backups\n\
# Empty = look on the PATH, then in the usual MariaDB/MySQL/XAMPP folders.\n\
MysqldumpPath =\n\
DatabaseIni = ../login/config/Database.ini\n";

fn readme() -> String {
    let (gui, servers, install) = if cfg!(windows) {
        ("Start-AdminGUI.cmd", "Start-Servers.cmd", "Install-Database.cmd")
    } else {
        ("start-admin-gui.sh", "start-servers.sh", "install-database.sh")
    };
    format!(
        "L2 Rust server (High Five)\n\
==========================\n\
\n\
Made by the setup of the repository (setup.cmd / setup.sh, which runs l2-setup). Run the setup\n\
again to update this folder: programs and datapack are replaced, your settings are kept.\n\
\n\
What is in here\n\
  bin/           the programs and supervisor.ini, Backup.ini\n\
  login/         login server settings (config/) and data\n\
  game/          game server settings (config/) and the datapack (data/, geodata in data/geodata)\n\
  db_installer/  the database schema (sql/)\n\
  backups/       the database backup made before a migration (if any)\n\
\n\
Start\n\
  {gui}   the admin GUI: \"Start all\" in the Servers view (login first, then game)\n\
  {servers}   the servers without the GUI (the supervisor; Ctrl+C stops them cleanly)\n\
  {install}   the database installer (only for a new, empty database)\n\
\n\
Client: point the client's l2.ini / hosts entry at this machine, port 2106.\n\
Ports: login 2106 (clients), 9014 (game servers); game 7777.\n\
This server runs all global jobs itself: never run a Java game server on the same database.\n\
Logs: login/log, game/log, bin/log.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A release folder with every program, a small `dist/`, a geodata folder.
    fn sources(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let release = root.join("release");
        for p in PROGRAMS {
            write(&release.join(exe(p)), p);
        }
        let dist = root.join("dist");
        write(&dist.join("login/config/Database.ini"), "Login = root\n");
        write(&dist.join("login/banned_ip.cfg"), "# none\n");
        write(&dist.join("login/data/servername.xml"), "<list/>");
        write(&dist.join("game/config/L2R.ini"), "RiftEnabled = true\n");
        write(&dist.join("game/data/stats/npcs/a.xml"), "<list/>");
        write(
            &dist.join("db_installer/sql/game/characters.sql"),
            "CREATE TABLE x;",
        );
        let geo = root.join("geo");
        write(&geo.join("20_20.l2j"), "geo");
        write(&geo.join("readme.txt"), "not geodata");
        (release, dist, geo)
    }

    #[test]
    fn a_new_folder_gets_programs_data_settings_geodata_and_scripts() {
        let root = tempfile::tempdir().unwrap();
        let (release, dist, geo) = sources(root.path());
        let out = root.path().join("server");
        let report = assemble(&release, &dist, &out, Some(&geo)).unwrap();
        assert!(report.created);
        assert!(report.kept.is_empty());
        assert_eq!((report.geodata_copied, report.geodata_total), (1, 1));
        for p in PROGRAMS {
            assert!(out.join("bin").join(exe(p)).is_file(), "{p}");
        }
        assert!(out.join("login/config/Database.ini").is_file());
        assert!(out.join("game/config/L2R.ini").is_file());
        assert!(out.join("game/data/stats/npcs/a.xml").is_file());
        assert!(out.join("db_installer/sql/game/characters.sql").is_file());
        assert!(out.join("game/data/geodata/20_20.l2j").is_file());
        assert!(!out.join("game/data/geodata/readme.txt").exists());
        let supervisor = fs::read_to_string(out.join("bin/supervisor.ini")).unwrap();
        assert!(supervisor.contains(&format!("Server.game.Command = ./{}", exe("game-server"))));
        assert!(supervisor.contains("Token =\n"));
        assert!(out.join("bin/Backup.ini").is_file() && out.join("README.txt").is_file());
        let script = if cfg!(windows) {
            "Start-Servers.cmd"
        } else {
            "start-servers.sh"
        };
        assert!(out.join(script).is_file());
    }

    #[test]
    fn an_update_keeps_the_settings_and_replaces_the_rest() {
        let root = tempfile::tempdir().unwrap();
        let (release, dist, geo) = sources(root.path());
        let out = root.path().join("server");
        assemble(&release, &dist, &out, Some(&geo)).unwrap();
        // The owner's settings, and an older program and datapack file.
        write(&out.join("login/config/Database.ini"), "Login = owner\n");
        write(&out.join("login/banned_ip.cfg"), "1.2.3.4\n");
        write(&out.join("game/config/L2R.ini"), "RiftEnabled = false\n");
        write(&out.join("bin/supervisor.ini"), "Listen = 127.0.0.1:7999\n");
        write(&out.join("game/data/stats/npcs/a.xml"), "old");
        write(&out.join(format!("bin/{}", exe("game-server"))), "old");
        let report = assemble(&release, &dist, &out, Some(&geo)).unwrap();
        assert!(!report.created);
        let read = |p: &str| fs::read_to_string(out.join(p)).unwrap();
        assert_eq!(read("login/config/Database.ini"), "Login = owner\n");
        assert_eq!(read("login/banned_ip.cfg"), "1.2.3.4\n");
        assert_eq!(read("game/config/L2R.ini"), "RiftEnabled = false\n");
        assert_eq!(read("bin/supervisor.ini"), "Listen = 127.0.0.1:7999\n");
        assert_eq!(read("game/data/stats/npcs/a.xml"), "<list/>");
        assert_eq!(read(&format!("bin/{}", exe("game-server"))), "game-server");
        assert!(
            report
                .kept
                .contains(&PathBuf::from("login").join("config").join("Database.ini"))
        );
        // The same geodata is not copied again.
        assert_eq!((report.geodata_copied, report.geodata_total), (0, 1));
        // A new settings file of a newer dist arrives.
        write(&dist.join("game/config/New.ini"), "X = 1\n");
        assemble(&release, &dist, &out, None).unwrap();
        assert_eq!(read("game/config/New.ini"), "X = 1\n");
        // A datapack file the newer dist renamed goes; the owner's own file and a setting stay.
        write(&out.join("game/data/stats/npcs/mine.xml"), "<list/>");
        fs::rename(
            dist.join("game/data/stats/npcs/a.xml"),
            dist.join("game/data/stats/npcs/b.xml"),
        )
        .unwrap();
        fs::remove_file(dist.join("game/config/New.ini")).unwrap();
        let report = assemble(&release, &dist, &out, None).unwrap();
        assert!(!out.join("game/data/stats/npcs/a.xml").exists());
        assert!(out.join("game/data/stats/npcs/b.xml").is_file());
        assert!(out.join("game/data/stats/npcs/mine.xml").is_file());
        assert!(out.join("game/config/New.ini").is_file());
        assert!(out.join("game/data/geodata/20_20.l2j").is_file());
        assert_eq!(report.removed, [PathBuf::from("game/data/stats/npcs/a.xml")]);
    }

    #[test]
    fn missing_programs_are_named() {
        let root = tempfile::tempdir().unwrap();
        let (release, _, _) = sources(root.path());
        assert!(missing_programs(&release).is_empty());
        fs::remove_file(release.join(exe("game-server"))).unwrap();
        assert_eq!(missing_programs(&release), vec!["game-server"]);
    }
}
