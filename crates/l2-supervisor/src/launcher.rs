//! Starting the supervisor on behalf of the admin GUI (G-01, D-56).
//!
//! The GUI finds `run/supervisor.ini`, reads `Listen` and `Token` from it and, when nobody
//! listens, starts `l2-supervisor` **detached**, so it keeps running when the GUI closes.
//! Everything that decides *what* to do is a pure function ([`decide`]) so each failure kind has a
//! unit test; the functions that touch the machine ([`inspect`], [`spawn_detached`]) are thin.
//!
//! ```text
//! find_run_folder -> read_connection -> probe (port, pid file, exe) -> decide
//!                                                       Connect | AlreadyStarting | Start | Report
//! ```

use std::fmt;
use std::fs::OpenOptions;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::config::{ConfigError, SupervisorConfig};
use crate::pidfile;

/// `supervisor.ini` inside the run folder.
pub const INI_NAME: &str = "supervisor.ini";
/// Under the run folder; the detached supervisor writes its output here.
pub const LOG_PATH: &str = "log/supervisor.log";
const PORT_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

/// File name of the supervisor binary next to the GUI.
pub fn supervisor_exe_name() -> &'static str {
    if cfg!(windows) {
        "l2-supervisor.exe"
    } else {
        "l2-supervisor"
    }
}

/// Why the supervisor cannot be reached or started, in words for the status bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// No `supervisor.ini` at this path (also: no run folder found at all).
    IniMissing(PathBuf),
    IniInvalid(String),
    TokenEmpty,
    /// Something listens on the port, but it does not speak the admin protocol.
    PortUsedByOther(SocketAddr),
    /// A supervisor listens on the port, but not the one of this run folder (an older package,
    /// a copy started elsewhere). The GUI offers to replace it.
    OtherSupervisor(SocketAddr),
    ExeMissing(PathBuf),
    SpawnFailed(String),
    /// Replacing the other supervisor failed (the reason in words).
    TakeOverFailed(String),
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Problem::IniMissing(path) => write!(f, "supervisor.ini is missing ({})", path.display()),
            Problem::IniInvalid(why) => write!(f, "supervisor.ini cannot be used: {why}"),
            Problem::TokenEmpty => write!(f, "Token is empty in supervisor.ini"),
            Problem::PortUsedByOther(addr) => write!(f, "port {} is used by another program", addr.port()),
            Problem::OtherSupervisor(addr) => write!(
                f,
                "port {} is used by another supervisor (not the one of this folder)",
                addr.port()
            ),
            Problem::ExeMissing(path) => write!(f, "{} is missing", path.display()),
            Problem::SpawnFailed(why) => write!(f, "could not start the supervisor: {why}"),
            Problem::TakeOverFailed(why) => write!(f, "could not replace the other supervisor: {why}"),
        }
    }
}

/// What the GUI needs from `supervisor.ini`.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectionInfo {
    pub listen: SocketAddr,
    pub token: String,
    /// The `Servers` order (Start all goes forward, Stop all backward).
    pub servers: Vec<String>,
}

// Hand-written so the token never ends up in a log line.
impl fmt::Debug for ConnectionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionInfo")
            .field("listen", &self.listen)
            .field("token", &"<redacted>")
            .field("servers", &self.servers)
            .finish()
    }
}

impl ConnectionInfo {
    /// `ws://host:port`; an unspecified listen address (`0.0.0.0`) is reached through loopback.
    pub fn url(&self) -> String {
        format!("ws://{}", connect_addr(self.listen))
    }
}

pub fn connect_addr(listen: SocketAddr) -> SocketAddr {
    if listen.ip().is_unspecified() {
        let ip = if listen.is_ipv4() {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        };
        SocketAddr::new(ip, listen.port())
    } else {
        listen
    }
}

/// Reads `Listen`, `Token` and the server order. A missing or unreadable file is the way this
/// fails; an empty token is fine on a local `Listen` (no token needed there, D-56).
pub fn read_connection(ini_path: &Path) -> Result<ConnectionInfo, Problem> {
    let config = match SupervisorConfig::load(ini_path) {
        Ok(config) => config,
        Err(ConfigError::Read { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
            return Err(Problem::IniMissing(ini_path.to_path_buf()));
        }
        Err(e) => return Err(Problem::IniInvalid(e.to_string())),
    };
    if config.token.is_empty() && !config.listen.ip().is_loopback() {
        return Err(Problem::TokenEmpty);
    }
    Ok(ConnectionInfo {
        listen: config.listen,
        token: config.token,
        servers: config.servers.into_iter().map(|s| s.name).collect(),
    })
}

/// Folders that may hold `supervisor.ini`, best first: the repo the GUI exe belongs to
/// (`<repo>/target/<profile>/gui` gives `<repo>/run`), `./run`, the exe's own folder.
pub fn run_folder_candidates(gui_exe: &Path, cwd: &Path) -> Vec<PathBuf> {
    let exe_dir = gui_exe.parent().unwrap_or_else(|| Path::new("."));
    let mut list = Vec::new();
    if let Some(repo) = exe_dir.parent().and_then(Path::parent) {
        list.push(repo.join("run"));
    }
    list.push(cwd.join("run"));
    list.push(exe_dir.to_path_buf());
    list
}

/// A `supervisor.ini` next to the GUI exe wins (a release package: the GUI belongs to the
/// servers it ships with, whatever another copy of the GUI remembered). Then the remembered folder
/// if it still has a `supervisor.ini`, then the first candidate that has one.
pub fn find_run_folder(remembered: Option<&Path>, gui_exe: &Path, cwd: &Path) -> Option<PathBuf> {
    let has_ini = |dir: &Path| dir.join(INI_NAME).is_file();
    let exe_dir = gui_exe.parent().filter(|dir| !dir.as_os_str().is_empty());
    if let Some(dir) = exe_dir.filter(|dir| has_ini(dir)) {
        return Some(dir.to_path_buf());
    }
    remembered
        .filter(|dir| !dir.as_os_str().is_empty() && has_ini(dir))
        .map(Path::to_path_buf)
        .or_else(|| {
            run_folder_candidates(gui_exe, cwd)
                .into_iter()
                .find(|dir| has_ini(dir))
        })
}

/// What the machine looks like right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Something accepts TCP connections on `Listen`.
    pub port_open: bool,
    /// `supervisor.pid` names a live supervisor process.
    pub pid_alive: bool,
    /// The `l2-supervisor` next to the GUI, if it exists.
    pub exe: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Someone listens: connect (the handshake tells whether it is a supervisor).
    Connect,
    /// A supervisor process exists but does not listen yet: just keep connecting.
    AlreadyStarting,
    /// Nobody listens and no supervisor runs: start this binary.
    Start(PathBuf),
    /// Nothing to connect to and nothing we can start.
    Report(Problem),
}

/// The choice between connect, start and report. Never starts a second supervisor: a live pid
/// file or an open port means "connect".
pub fn decide(ini: &Result<ConnectionInfo, Problem>, exe_path: &Path, probe: &Probe) -> Decision {
    if let Err(problem) = ini {
        return Decision::Report(problem.clone());
    }
    if probe.port_open {
        // Our own supervisor writes `supervisor.pid` once it listens; a listener without it is
        // someone else's supervisor (or another program), never ours to control.
        return if probe.pid_alive {
            Decision::Connect
        } else {
            Decision::Report(Problem::OtherSupervisor(
                ini.as_ref()
                    .map_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0)), |info| info.listen),
            ))
        };
    }
    if probe.pid_alive {
        return Decision::AlreadyStarting;
    }
    match &probe.exe {
        Some(exe) => Decision::Start(exe.clone()),
        None => Decision::Report(Problem::ExeMissing(exe_path.to_path_buf())),
    }
}

/// Maps a failed connection attempt to a problem. A refused connection is the normal "nobody is
/// there yet"; a connection that is accepted but fails the handshake means another program owns
/// the port, unless our own supervisor is alive (then it is still starting up).
pub fn classify_connect_error(error: &str, listen: SocketAddr, supervisor_alive: bool) -> Option<Problem> {
    let lower = error.to_lowercase();
    let refused = lower.contains("refused")
        || lower.contains("os error 10061")
        || (lower.contains("timed out") && !lower.contains("handshake"));
    if refused || supervisor_alive {
        return None;
    }
    if lower.contains("rejected") || lower.contains("invalid token") {
        return Some(Problem::OtherSupervisor(listen));
    }
    if lower.contains("websocket")
        || lower.contains("protocol")
        || lower.contains("closed")
        || lower.contains("invalid json")
    {
        return Some(Problem::PortUsedByOther(listen));
    }
    None
}

/// TCP connect probe. Port 0 (tests that let the OS choose) is never "open".
pub fn port_open(listen: SocketAddr) -> bool {
    if listen.port() == 0 {
        return false;
    }
    TcpStream::connect_timeout(&connect_addr(listen), PORT_PROBE_TIMEOUT).is_ok()
}

/// Gathers the [`Probe`] for a run folder.
pub fn inspect(run_folder: &Path, listen: SocketAddr, supervisor_exe: &Path) -> Probe {
    let pid_alive =
        pidfile::read(&pidfile::path_for(&run_folder.join(INI_NAME))).is_some_and(|info| info.is_alive());
    Probe {
        port_open: port_open(listen),
        pid_alive,
        exe: supervisor_exe.is_file().then(|| supervisor_exe.to_path_buf()),
    }
}

/// How far [`ensure_running`] got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchStatus {
    /// Something already listens.
    Running,
    /// A supervisor was started (now or earlier) and is not listening yet.
    Starting,
    Problem(Problem),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    /// `Some` whenever `supervisor.ini` could be read, even if starting failed.
    pub connection: Option<ConnectionInfo>,
    pub status: LaunchStatus,
}

/// Reads `<run_folder>/supervisor.ini`, starts the supervisor when nobody listens, and says
/// what happened. Blocking but quick (a port probe and a process spawn): call it off the UI thread.
pub fn ensure_running(run_folder: &Path, supervisor_exe: &Path) -> Launch {
    let ini_path = run_folder.join(INI_NAME);
    let ini = read_connection(&ini_path);
    let probe = match &ini {
        Ok(info) => inspect(run_folder, info.listen, supervisor_exe),
        Err(_) => Probe {
            port_open: false,
            pid_alive: false,
            exe: None,
        },
    };
    let connection = ini.as_ref().ok().cloned();
    let status = match decide(&ini, supervisor_exe, &probe) {
        Decision::Connect => LaunchStatus::Running,
        Decision::AlreadyStarting => LaunchStatus::Starting,
        Decision::Report(problem) => LaunchStatus::Problem(problem),
        Decision::Start(exe) => match spawn_detached(&exe, run_folder) {
            Ok(pid) => {
                tracing::info!("started the supervisor (pid {pid})");
                LaunchStatus::Starting
            }
            Err(e) => LaunchStatus::Problem(Problem::SpawnFailed(e.to_string())),
        },
    };
    Launch { connection, status }
}

/// Starts `exe` on `<run_folder>/supervisor.ini` so that it outlives the caller (D-56):
/// Windows: no console window and detached from our console; Unix: its own process group, so a
/// Ctrl+C or a closing terminal does not reach it. Output goes to `<run_folder>/log/supervisor.log`.
/// Returns the pid.
pub fn spawn_detached(exe: &Path, run_folder: &Path) -> io::Result<u32> {
    let run_folder = std::path::absolute(run_folder)?;
    let log_path = run_folder.join(LOG_PATH);
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let log = OpenOptions::new().create(true).append(true).open(&log_path)?;

    let mut command = Command::new(std::path::absolute(exe)?);
    command
        .arg(run_folder.join(INI_NAME))
        .current_dir(&run_folder)
        .env("NO_COLOR", "1") // no escape codes in the log file
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    detach(&mut command);

    let mut child = command.spawn()?;
    let pid = child.id();
    // Reap it when it ends (no zombie on Unix while the GUI lives); if the GUI exits first the
    // supervisor simply carries on.
    std::thread::Builder::new()
        .name("supervisor-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        })?;
    Ok(pid)
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    // `setsid` would need `unsafe` (pre_exec); a process group of its own is enough to
    // survive the terminal's Ctrl+C.
    command.process_group(0);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const LISTEN: &str = "127.0.0.1:7900";

    fn info() -> ConnectionInfo {
        ConnectionInfo {
            listen: LISTEN.parse().unwrap(),
            token: "t".into(),
            servers: vec!["login".into(), "game".into()],
        }
    }

    fn write_ini(dir: &Path, text: &str) -> PathBuf {
        let path = dir.join(INI_NAME);
        std::fs::write(&path, text).unwrap();
        path
    }

    // ---- run folder -------------------------------------------------------------------------

    #[test]
    fn candidates_follow_the_documented_order() {
        let list = run_folder_candidates(Path::new("/repo/target/release/gui"), Path::new("/work"));
        assert_eq!(
            list,
            vec![
                PathBuf::from("/repo/run"),
                PathBuf::from("/work/run"),
                PathBuf::from("/repo/target/release"),
            ]
        );
    }

    #[test]
    fn finds_the_first_folder_with_an_ini_and_prefers_the_remembered_one() {
        let root = tempfile::tempdir().unwrap();
        let exe = root.path().join("repo/target/release/gui");
        let repo_run = root.path().join("repo/run");
        let cwd_run = root.path().join("work/run");
        std::fs::create_dir_all(&repo_run).unwrap();
        std::fs::create_dir_all(&cwd_run).unwrap();
        let cwd = root.path().join("work");

        // Nothing has an ini yet.
        assert_eq!(find_run_folder(None, &exe, &cwd), None);

        write_ini(&cwd_run, "");
        assert_eq!(find_run_folder(None, &exe, &cwd), Some(cwd_run.clone()));

        // The repo's run folder comes first once it has an ini.
        write_ini(&repo_run, "");
        assert_eq!(find_run_folder(None, &exe, &cwd), Some(repo_run.clone()));

        // A remembered folder wins, a remembered folder without an ini is ignored.
        assert_eq!(find_run_folder(Some(&cwd_run), &exe, &cwd), Some(cwd_run.clone()));
        let empty = root.path().join("nothing");
        assert_eq!(find_run_folder(Some(&empty), &exe, &cwd), Some(repo_run.clone()));
        assert_eq!(find_run_folder(Some(Path::new("")), &exe, &cwd), Some(repo_run));
    }

    #[test]
    fn an_ini_next_to_the_exe_wins_over_everything() {
        let root = tempfile::tempdir().unwrap();
        let exe_dir = root.path().join("bundle");
        std::fs::create_dir_all(&exe_dir).unwrap();
        write_ini(&exe_dir, "");
        let found = find_run_folder(None, &exe_dir.join("gui"), &root.path().join("elsewhere"));
        assert_eq!(found, Some(exe_dir.clone()));

        // A package's GUI ignores the folder another copy of the GUI remembered.
        let other = root.path().join("repo/run");
        std::fs::create_dir_all(&other).unwrap();
        write_ini(&other, "");
        let found = find_run_folder(Some(&other), &exe_dir.join("gui"), &root.path().join("elsewhere"));
        assert_eq!(found, Some(exe_dir));
    }

    // ---- supervisor.ini ---------------------------------------------------------------------

    #[test]
    fn reads_listen_token_and_server_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_ini(
            dir.path(),
            "Listen = 127.0.0.1:7123\nToken = abc\nServers = login, game\n\
             Server.login.Command = x\nServer.game.Command = y\n",
        );
        let info = read_connection(&path).unwrap();
        assert_eq!(info.listen.to_string(), "127.0.0.1:7123");
        assert_eq!(info.token, "abc");
        assert_eq!(info.servers, ["login", "game"]);
        assert_eq!(info.url(), "ws://127.0.0.1:7123");
    }

    #[test]
    fn listen_defaults_and_wildcard_connects_through_loopback() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_ini(dir.path(), "Token = abc\n");
        assert_eq!(read_connection(&path).unwrap().url(), "ws://127.0.0.1:7900");
        let path = write_ini(dir.path(), "Listen = 0.0.0.0:7000\nToken = abc\n");
        assert_eq!(read_connection(&path).unwrap().url(), "ws://127.0.0.1:7000");
    }

    #[test]
    fn missing_and_broken_files_are_told_apart_and_an_empty_local_token_is_fine() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(INI_NAME);
        assert_eq!(read_connection(&path), Err(Problem::IniMissing(path.clone())));

        write_ini(dir.path(), "Listen = 127.0.0.1:7900\nToken =\n");
        assert_eq!(
            read_connection(&path).unwrap().token,
            "",
            "a local Listen needs no token"
        );

        write_ini(dir.path(), "Listen = 0.0.0.0:7900\nToken =\n");
        assert_eq!(read_connection(&path), Err(Problem::TokenEmpty));

        write_ini(dir.path(), "Listen = nonsense\nToken = abc\n");
        assert!(matches!(read_connection(&path), Err(Problem::IniInvalid(_))));
    }

    #[test]
    fn debug_output_hides_the_token() {
        let mut shown = info();
        shown.token = "hunter2".into();
        assert!(!format!("{shown:?}").contains("hunter2"));
    }

    // ---- decide -----------------------------------------------------------------------------

    fn probe(port_open: bool, pid_alive: bool, exe: bool) -> Probe {
        Probe {
            port_open,
            pid_alive,
            exe: exe.then(|| PathBuf::from("/bin/l2-supervisor")),
        }
    }

    fn decide_with(ini: Result<ConnectionInfo, Problem>, probe: Probe) -> Decision {
        decide(&ini, Path::new("/bin/l2-supervisor"), &probe)
    }

    #[test]
    fn an_open_port_is_ours_only_with_a_live_pid_file() {
        assert_eq!(
            decide_with(Ok(info()), probe(true, true, false)),
            Decision::Connect
        );
        assert_eq!(
            decide_with(Ok(info()), probe(true, true, true)),
            Decision::Connect
        );
        // Someone else's supervisor (an older package): reported, never controlled.
        assert_eq!(
            decide_with(Ok(info()), probe(true, false, true)),
            Decision::Report(Problem::OtherSupervisor(info().listen))
        );
    }

    #[test]
    fn a_live_supervisor_without_a_port_is_never_started_twice() {
        assert_eq!(
            decide_with(Ok(info()), probe(false, true, true)),
            Decision::AlreadyStarting
        );
    }

    #[test]
    fn nobody_there_means_start() {
        assert_eq!(
            decide_with(Ok(info()), probe(false, false, true)),
            Decision::Start(PathBuf::from("/bin/l2-supervisor"))
        );
    }

    #[test]
    fn every_failure_kind_is_reported() {
        let missing = PathBuf::from("run/supervisor.ini");
        for problem in [
            Problem::IniMissing(missing),
            Problem::TokenEmpty,
            Problem::IniInvalid("bad".into()),
        ] {
            assert_eq!(
                decide_with(Err(problem.clone()), probe(true, true, true)),
                Decision::Report(problem)
            );
        }
        assert_eq!(
            decide_with(Ok(info()), probe(false, false, false)),
            Decision::Report(Problem::ExeMissing(PathBuf::from("/bin/l2-supervisor")))
        );
    }

    #[test]
    fn problems_read_as_plain_words() {
        let addr: SocketAddr = LISTEN.parse().unwrap();
        assert_eq!(
            Problem::TokenEmpty.to_string(),
            "Token is empty in supervisor.ini"
        );
        assert_eq!(
            Problem::PortUsedByOther(addr).to_string(),
            "port 7900 is used by another program"
        );
        assert!(
            Problem::IniMissing(PathBuf::from("run/supervisor.ini"))
                .to_string()
                .starts_with("supervisor.ini is missing")
        );
        assert!(
            Problem::ExeMissing(PathBuf::from("x"))
                .to_string()
                .ends_with("is missing")
        );
    }

    #[test]
    fn connect_errors_are_classified() {
        let addr: SocketAddr = LISTEN.parse().unwrap();
        // Nobody listens yet: fine, keep trying.
        assert_eq!(
            classify_connect_error("i/o error: Connection refused (os error 111)", addr, false),
            None
        );
        assert_eq!(
            classify_connect_error(
                "i/o error: No connection could be made (os error 10061)",
                addr,
                false
            ),
            None
        );
        // Something answers but is not a supervisor.
        assert_eq!(
            classify_connect_error("websocket error: HTTP error: 400 Bad Request", addr, false),
            Some(Problem::PortUsedByOther(addr))
        );
        assert_eq!(
            classify_connect_error("protocol error: expected welcome", addr, false),
            Some(Problem::PortUsedByOther(addr))
        );
        // Our own supervisor is still starting: not another program.
        assert_eq!(classify_connect_error("connection closed", addr, true), None);
    }

    // ---- ensure_running (no process is spawned here) ---------------------------------------

    #[test]
    fn ensure_running_reports_a_missing_ini_and_a_missing_exe() {
        let dir = tempfile::tempdir().unwrap();
        let launch = ensure_running(dir.path(), Path::new("no/such/l2-supervisor"));
        assert_eq!(launch.connection, None);
        assert_eq!(
            launch.status,
            LaunchStatus::Problem(Problem::IniMissing(dir.path().join(INI_NAME)))
        );

        // Port 0 is never open, no pid file, no exe: reported, nothing spawned.
        write_ini(dir.path(), "Listen = 127.0.0.1:0\nToken = abc\n");
        let launch = ensure_running(dir.path(), &dir.path().join("no-such-exe"));
        assert!(launch.connection.is_some());
        assert!(matches!(
            launch.status,
            LaunchStatus::Problem(Problem::ExeMissing(_))
        ));
    }

    #[test]
    fn ensure_running_connects_to_an_open_port_without_spawning() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let dir = tempfile::tempdir().unwrap();
        let ini = write_ini(dir.path(), &format!("Listen = 127.0.0.1:{port}\nToken = abc\n"));
        // Without our pid file the listener is someone else's supervisor.
        let launch = ensure_running(dir.path(), &dir.path().join("no-such-exe"));
        assert!(matches!(
            launch.status,
            LaunchStatus::Problem(Problem::OtherSupervisor(_))
        ));
        // With it (this test process stands in for the supervisor): connect, start nothing.
        let _pid = pidfile::PidFileGuard::create(&pidfile::path_for(&ini)).unwrap();
        let launch = ensure_running(dir.path(), &dir.path().join("no-such-exe"));
        assert_eq!(launch.status, LaunchStatus::Running);
    }
}
