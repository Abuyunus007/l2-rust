//! Finding the run folder, reading `supervisor.ini` and starting the supervisor (G-01, D-56).
//!
//! The decisions live in `l2_supervisor::launcher` (pure, tested there, and exercised against the
//! real binary by its integration test). This module is the GUI side:
//!
//! ```text
//! UI thread                                  short-lived thread
//! LaunchHandle::connect()  --UiRequest-->    (App) spawns resolve()  ---> Resolved
//! App::logic polls Resolved: state = ..., bridge.connect(target)
//! ```
//!
//! [`LaunchState`] is what the status bar and the connection view show about the supervisor.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use l2_supervisor::launcher::{
    self, ConnectionInfo, LaunchStatus, Problem, classify_connect_error, supervisor_exe_name,
};

use crate::bridge::Target;
use crate::settings::Settings;

/// After this many failed attempts at a supervisor we started ourselves, give up waiting and
/// say so (about 30 s with the bridge's backoff).
pub const STARTING_GIVE_UP_FAILURES: u32 = 6;

/// What the GUI knows about the supervisor process.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LaunchState {
    /// Nothing tried yet.
    #[default]
    Unknown,
    /// Looking for the run folder / starting it.
    Resolving,
    /// Something listens (or listened when we looked).
    Running,
    /// Started by us (or by someone else) and not listening yet.
    Starting,
    /// The user stopped it through the GUI.
    Stopped,
    Problem(Problem),
}

/// Requests from the views to the app (drained once per frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiRequest {
    /// Resolve the run folder, start the supervisor if needed, and (re)connect.
    Connect,
    /// Stop all servers and let the supervisor exit.
    StopSupervisor,
    /// Remember this folder as the run folder, then connect.
    UseRunFolder(String),
}

/// What views see and can ask for. Lives in [`crate::app::AppCtx`].
pub struct LaunchHandle {
    pub state: LaunchState,
    /// The run folder in use (where `supervisor.ini` was found).
    pub run_folder: Option<PathBuf>,
    /// The address connected to, from `supervisor.ini` or the override.
    pub url: Option<String>,
    /// `Listen` of the supervisor we resolved (to judge connection errors).
    pub listen: Option<SocketAddr>,
    /// A one-line message for the connection view (e.g. a failed stop).
    pub notice: Option<String>,
    requests: mpsc::Sender<UiRequest>,
}

impl LaunchHandle {
    pub fn new() -> (Self, mpsc::Receiver<UiRequest>) {
        let (requests, rx) = mpsc::channel();
        (
            Self {
                state: LaunchState::default(),
                run_folder: None,
                url: None,
                listen: None,
                notice: None,
                requests,
            },
            rx,
        )
    }

    /// A handle nobody listens to (tests that draw a view).
    #[cfg(test)]
    pub fn detached() -> Self {
        Self::new().0
    }

    pub fn connect(&self) {
        let _ = self.requests.send(UiRequest::Connect);
    }

    pub fn stop_supervisor(&self) {
        let _ = self.requests.send(UiRequest::StopSupervisor);
    }

    pub fn use_run_folder(&self, folder: String) {
        let _ = self.requests.send(UiRequest::UseRunFolder(folder));
    }
}

/// Everything [`resolve`] needs, copied out of the settings so it can run on another thread.
#[derive(Clone)]
pub struct ResolveInput {
    pub remembered: Option<PathBuf>,
    pub url_override: Option<String>,
    pub token_override: Option<String>,
    pub gui_exe: PathBuf,
    pub cwd: PathBuf,
}

impl ResolveInput {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            remembered: settings.remembered_run_folder(),
            url_override: settings.url_override(),
            token_override: settings.token_override().map(str::to_string),
            gui_exe: std::env::current_exe().unwrap_or_default(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

/// The answer of [`resolve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub run_folder: Option<PathBuf>,
    /// Where to connect; `None` when there is nothing to connect to.
    pub target: Option<Target>,
    pub listen: Option<SocketAddr>,
    pub state: LaunchState,
}

/// Blocking (a port probe, maybe a process spawn): call it on a short-lived thread.
///
/// * A typed address is a remote or hand-made setup: connect there, start nothing.
/// * Otherwise `supervisor.ini` gives address and token and the supervisor is started when nobody
///   listens. The file's token is used; a typed token only counts with a typed address.
pub fn resolve(input: &ResolveInput) -> Resolved {
    let run_folder = launcher::find_run_folder(input.remembered.as_deref(), &input.gui_exe, &input.cwd);

    if let Some(url) = &input.url_override {
        // The folder is only used for the token.
        let file_token = run_folder
            .as_ref()
            .and_then(|dir| launcher::read_connection(&dir.join(launcher::INI_NAME)).ok())
            .map(|info| info.token);
        let token = input.token_override.clone().or(file_token).unwrap_or_default();
        return if token.is_empty() {
            Resolved {
                run_folder,
                target: None,
                listen: None,
                state: LaunchState::Problem(Problem::TokenEmpty),
            }
        } else {
            Resolved {
                run_folder,
                target: Some(Target {
                    url: url.clone(),
                    token,
                }),
                listen: None,
                state: LaunchState::Running,
            }
        };
    }

    let Some(folder) = run_folder else {
        let first = launcher::run_folder_candidates(&input.gui_exe, &input.cwd)
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("run"));
        return Resolved {
            run_folder: None,
            target: None,
            listen: None,
            state: LaunchState::Problem(Problem::IniMissing(first.join(launcher::INI_NAME))),
        };
    };

    let exe = input
        .gui_exe
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(supervisor_exe_name());
    let launch = launcher::ensure_running(&folder, &exe);
    let state = match launch.status {
        LaunchStatus::Running => LaunchState::Running,
        LaunchStatus::Starting => LaunchState::Starting,
        LaunchStatus::Problem(problem) => LaunchState::Problem(problem),
    };
    let target = launch.connection.as_ref().and_then(|info| target_of(info, input));
    Resolved {
        run_folder: Some(folder),
        listen: launch.connection.map(|info| info.listen),
        target,
        state,
    }
}

/// For the local supervisor the file's token is the truth: a typed token is often one an older
/// GUI saved for another folder, and an empty file token makes the supervisor refuse everyone.
fn target_of(info: &ConnectionInfo, _input: &ResolveInput) -> Option<Target> {
    let token = info.token.clone();
    Some(Target {
        url: info.url(),
        token,
    })
}

/// What to do about a failed connection attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitingVerdict {
    Keep,
    Become(LaunchState),
    /// The supervisor we thought was running is gone: resolve again (which starts it).
    Relaunch,
}

/// Looks at the bridge's `Waiting` event. `alive` tells whether `supervisor.pid` names a live
/// supervisor (only asked when it matters).
pub fn judge_waiting(
    state: &LaunchState,
    failures: u32,
    error: &str,
    listen: Option<SocketAddr>,
    alive: impl Fn() -> bool,
) -> WaitingVerdict {
    match state {
        LaunchState::Running => {
            if let Some(listen) = listen {
                // Accepted but not a supervisor: someone else owns the port.
                if let Some(problem) = classify_connect_error(error, listen, false)
                    && !alive()
                {
                    return WaitingVerdict::Become(LaunchState::Problem(problem));
                }
            }
            if is_refused(error) {
                WaitingVerdict::Relaunch
            } else {
                WaitingVerdict::Keep
            }
        }
        LaunchState::Starting if failures >= STARTING_GIVE_UP_FAILURES && !alive() => {
            WaitingVerdict::Become(LaunchState::Problem(Problem::SpawnFailed(format!(
                "it did not start listening; see {}",
                launcher::LOG_PATH
            ))))
        }
        _ => WaitingVerdict::Keep,
    }
}

fn is_refused(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("refused") || lower.contains("os error 10061")
}

/// Text for the status bar: `Supervisor: running / starting / not found (path)` plus the reason.
pub fn status_text(state: &LaunchState, connected: bool) -> String {
    if connected {
        return "Supervisor: running".to_string();
    }
    match state {
        LaunchState::Unknown => "Supervisor: not checked yet".to_string(),
        LaunchState::Resolving | LaunchState::Starting => "Supervisor: starting".to_string(),
        LaunchState::Running => "Supervisor: running, not connected".to_string(),
        LaunchState::Stopped => "Supervisor: stopped".to_string(),
        LaunchState::Problem(Problem::IniMissing(path)) => {
            format!(
                "Supervisor: not found ({}): supervisor.ini is missing",
                path.display()
            )
        }
        LaunchState::Problem(Problem::ExeMissing(path)) => {
            format!("Supervisor: not found ({})", path.display())
        }
        LaunchState::Problem(problem) => format!("Supervisor: not running: {problem}"),
    }
}

/// The hint under the status when there is no `supervisor.ini` yet (G-02 adds the wizard).
pub const SETUP_HINT: &str = "Run the setup wizard. Until it exists: copy crates/l2-supervisor/supervisor.example.ini to \
     run/supervisor.ini and set Token (README, section \"Supervisor\").";

/// True when [`SETUP_HINT`] applies.
pub fn needs_setup(state: &LaunchState) -> bool {
    matches!(
        state,
        LaunchState::Problem(Problem::IniMissing(_) | Problem::TokenEmpty)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const LISTEN: &str = "127.0.0.1:7900";

    fn listen() -> Option<SocketAddr> {
        Some(LISTEN.parse().unwrap())
    }

    fn input(dir: &Path) -> ResolveInput {
        ResolveInput {
            remembered: Some(dir.to_path_buf()),
            url_override: None,
            token_override: None,
            // No l2-supervisor next to it, so nothing can ever be spawned from these tests.
            gui_exe: dir.join("no-such-dir").join("gui"),
            cwd: dir.join("nowhere"),
        }
    }

    #[test]
    fn no_ini_anywhere_asks_for_setup() {
        let dir = tempfile::tempdir().unwrap();
        let mut input = input(dir.path());
        input.remembered = None;
        let resolved = resolve(&input);
        assert_eq!(resolved.target, None);
        assert!(matches!(
            resolved.state,
            LaunchState::Problem(Problem::IniMissing(_))
        ));
        assert!(needs_setup(&resolved.state));
    }

    #[test]
    fn an_empty_token_uses_the_generated_token_file() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("supervisor.ini"),
            format!("Listen = 127.0.0.1:{port}\nToken =\n"),
        )
        .unwrap();
        let resolved = resolve(&input(dir.path()));
        assert_eq!(resolved.run_folder.as_deref(), Some(dir.path()));
        assert_eq!(resolved.state, LaunchState::Running);
        let file = std::fs::read_to_string(dir.path().join("supervisor.token")).unwrap();
        assert_eq!(resolved.target.unwrap().token, file);
    }

    #[test]
    fn the_exe_next_to_the_gui_is_required_to_start() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("supervisor.ini"),
            "Listen = 127.0.0.1:0\nToken = abc\n",
        )
        .unwrap();
        let resolved = resolve(&input(dir.path()));
        assert!(matches!(
            resolved.state,
            LaunchState::Problem(Problem::ExeMissing(_))
        ));
        // The connection details are still known, so a manual start can be connected to.
        assert_eq!(resolved.target.unwrap().token, "abc");
    }

    #[test]
    fn the_file_gives_url_and_token_and_a_typed_token_is_ignored() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("supervisor.ini"),
            format!("Listen = 127.0.0.1:{port}\nToken = from-file\n"),
        )
        .unwrap();

        let resolved = resolve(&input(dir.path()));
        assert_eq!(resolved.state, LaunchState::Running, "the port is open: no start");
        let target = resolved.target.unwrap();
        assert_eq!(target.url, format!("ws://127.0.0.1:{port}"));
        assert_eq!(target.token, "from-file");

        let mut typed = input(dir.path());
        typed.token_override = Some("typed".into());
        assert_eq!(
            resolve(&typed).target.unwrap().token,
            "from-file",
            "a stale typed token is ignored"
        );
    }

    #[test]
    fn a_typed_address_connects_without_starting_anything() {
        let dir = tempfile::tempdir().unwrap();
        let mut remote = input(dir.path());
        remote.url_override = Some("ws://10.0.0.5:7900".into());

        // No token anywhere: reported.
        assert_eq!(resolve(&remote).state, LaunchState::Problem(Problem::TokenEmpty));

        remote.token_override = Some("t".into());
        let resolved = resolve(&remote);
        assert_eq!(resolved.state, LaunchState::Running);
        assert_eq!(resolved.target.unwrap().url, "ws://10.0.0.5:7900");

        // The file's token is used when none was typed.
        std::fs::write(dir.path().join("supervisor.ini"), "Token = file\n").unwrap();
        remote.token_override = None;
        assert_eq!(resolve(&remote).target.unwrap().token, "file");
    }

    #[test]
    fn a_vanished_supervisor_is_launched_again() {
        let refused = "i/o error: Connection refused (os error 111)";
        assert_eq!(
            judge_waiting(&LaunchState::Running, 1, refused, listen(), || false),
            WaitingVerdict::Relaunch
        );
        // A supervisor we started is just slow: keep waiting.
        assert_eq!(
            judge_waiting(&LaunchState::Starting, 1, refused, listen(), || true),
            WaitingVerdict::Keep
        );
    }

    #[test]
    fn another_program_on_the_port_is_reported() {
        let addr = listen().unwrap();
        assert_eq!(
            judge_waiting(
                &LaunchState::Running,
                1,
                "websocket error: HTTP error: 400 Bad Request",
                listen(),
                || false
            ),
            WaitingVerdict::Become(LaunchState::Problem(Problem::PortUsedByOther(addr)))
        );
        // ... but not while our own supervisor is alive.
        assert_eq!(
            judge_waiting(
                &LaunchState::Running,
                1,
                "websocket error: HTTP error: 400 Bad Request",
                listen(),
                || true
            ),
            WaitingVerdict::Keep
        );
    }

    #[test]
    fn a_supervisor_that_never_listens_ends_in_a_problem() {
        let refused = "Connection refused";
        assert_eq!(
            judge_waiting(
                &LaunchState::Starting,
                STARTING_GIVE_UP_FAILURES - 1,
                refused,
                listen(),
                || false
            ),
            WaitingVerdict::Keep
        );
        assert!(matches!(
            judge_waiting(
                &LaunchState::Starting,
                STARTING_GIVE_UP_FAILURES,
                refused,
                listen(),
                || false
            ),
            WaitingVerdict::Become(LaunchState::Problem(Problem::SpawnFailed(_)))
        ));
        // Alive but slow: no verdict yet.
        assert_eq!(
            judge_waiting(&LaunchState::Starting, 50, refused, listen(), || true),
            WaitingVerdict::Keep
        );
    }

    #[test]
    fn status_text_names_the_reason() {
        assert_eq!(status_text(&LaunchState::Stopped, true), "Supervisor: running");
        assert_eq!(status_text(&LaunchState::Starting, false), "Supervisor: starting");
        assert_eq!(
            status_text(&LaunchState::Problem(Problem::TokenEmpty), false),
            "Supervisor: not running: Token is empty in supervisor.ini"
        );
        assert_eq!(
            status_text(
                &LaunchState::Problem(Problem::IniMissing(PathBuf::from("run/supervisor.ini"))),
                false
            ),
            "Supervisor: not found (run/supervisor.ini): supervisor.ini is missing"
        );
        assert_eq!(
            status_text(
                &LaunchState::Problem(Problem::ExeMissing(PathBuf::from("x/l2-supervisor"))),
                false
            ),
            "Supervisor: not found (x/l2-supervisor)"
        );
        let addr = listen().unwrap();
        assert_eq!(
            status_text(&LaunchState::Problem(Problem::PortUsedByOther(addr)), false),
            "Supervisor: not running: port 7900 is used by another program"
        );
    }

    #[test]
    fn handle_requests_reach_the_app() {
        let (handle, rx) = LaunchHandle::new();
        handle.connect();
        handle.stop_supervisor();
        handle.use_run_folder("somewhere".into());
        let got: Vec<UiRequest> = rx.try_iter().collect();
        assert_eq!(
            got,
            [
                UiRequest::Connect,
                UiRequest::StopSupervisor,
                UiRequest::UseRunFolder("somewhere".into())
            ]
        );
    }
}
