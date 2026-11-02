//! `run/supervisor.pid`: written by the supervisor at start, removed at exit (G-01).
//!
//! The admin GUI reads it to avoid starting a second supervisor. Format: two lines, the process
//! id and the path of the executable. A file left behind by a crash or a kill is *stale*; it is
//! told apart from a live one by looking the pid up and comparing the executable.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// File name next to `supervisor.ini`.
pub const FILE_NAME: &str = "supervisor.pid";

/// The pid file that belongs to `supervisor.ini` at `ini_path`.
pub fn path_for(ini_path: &Path) -> PathBuf {
    ini_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(FILE_NAME)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidInfo {
    pub pid: u32,
    pub exe: String,
}

impl PidInfo {
    /// The running process.
    pub fn current() -> Self {
        Self {
            pid: std::process::id(),
            exe: std::env::current_exe()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
        }
    }

    pub fn render(&self) -> String {
        format!("{}\n{}\n", self.pid, self.exe)
    }

    /// `None` for anything that is not a pid on the first line (a half-written or foreign file).
    pub fn parse(text: &str) -> Option<Self> {
        let mut lines = text.lines();
        let pid = lines.next()?.trim().parse::<u32>().ok().filter(|p| *p != 0)?;
        let exe = lines.next().unwrap_or_default().trim().to_string();
        Some(Self { pid, exe })
    }

    /// True when a process with this pid exists and runs the recorded executable.
    pub fn is_alive(&self) -> bool {
        process_runs(self.pid, &self.exe)
    }
}

/// Reads the pid file; `None` when it is missing or unreadable.
pub fn read(path: &Path) -> Option<PidInfo> {
    PidInfo::parse(&std::fs::read_to_string(path).ok()?)
}

/// Removes the pid file when dropped, but only if it still names this process (a newer
/// supervisor may have replaced it).
#[derive(Debug)]
pub struct PidFileGuard {
    path: PathBuf,
    pid: u32,
}

impl PidFileGuard {
    /// Writes the pid file for the running process.
    pub fn create(path: &Path) -> io::Result<Self> {
        let info = PidInfo::current();
        std::fs::write(path, info.render())?;
        Ok(Self {
            path: path.to_path_buf(),
            pid: info.pid,
        })
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        if read(&self.path).is_some_and(|info| info.pid == self.pid) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn file_name_lower(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[cfg(windows)]
fn process_runs(pid: u32, exe: &str) -> bool {
    // `tasklist` prints `"image.exe","1234",...` for a match and an INFO line otherwise.
    let Ok(output) = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let Some(line) = text.lines().find(|l| l.starts_with('"')) else {
        return false;
    };
    let image = line
        .trim_start_matches('"')
        .split('"')
        .next()
        .unwrap_or_default()
        .to_lowercase();
    let wanted = file_name_lower(exe);
    // An unknown recorded exe (empty) only proves that the pid exists.
    wanted.is_empty() || image == wanted
}

#[cfg(not(windows))]
fn process_runs(pid: u32, exe: &str) -> bool {
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    if proc_dir.exists() {
        return match std::fs::read_link(proc_dir.join("exe")) {
            Ok(actual) => {
                let actual = actual
                    .to_string_lossy()
                    .trim_end_matches(" (deleted)")
                    .to_string();
                exe.is_empty() || file_name_lower(&actual) == file_name_lower(exe)
            }
            // Not ours to read (other user): the pid exists, which is all we can say.
            Err(_) => true,
        };
    }
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .is_ok_and(|o| o.status.success())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn render_and_parse_round_trip() {
        let info = PidInfo {
            pid: 4242,
            exe: "C:\\x\\l2-supervisor.exe".into(),
        };
        assert_eq!(PidInfo::parse(&info.render()), Some(info));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(PidInfo::parse(""), None);
        assert_eq!(PidInfo::parse("abc\nexe"), None);
        assert_eq!(PidInfo::parse("0\nexe"), None);
        assert_eq!(
            PidInfo::parse("17"),
            Some(PidInfo {
                pid: 17,
                exe: String::new()
            })
        );
    }

    #[test]
    fn path_sits_next_to_the_ini() {
        assert_eq!(
            path_for(Path::new("run/supervisor.ini")),
            Path::new("run").join(FILE_NAME)
        );
        assert_eq!(
            path_for(Path::new("supervisor.ini")),
            Path::new(".").join(FILE_NAME)
        );
    }

    #[test]
    fn this_process_is_alive_and_a_wrong_exe_is_not() {
        let me = PidInfo::current();
        assert!(me.is_alive());
        let impostor = PidInfo {
            pid: me.pid,
            exe: "definitely-not-this-test.exe".into(),
        };
        assert!(!impostor.is_alive());
    }

    #[test]
    fn a_pid_nobody_has_is_not_alive() {
        let ghost = PidInfo {
            pid: u32::MAX - 1,
            exe: String::new(),
        };
        assert!(!ghost.is_alive());
    }

    #[test]
    fn guard_writes_and_removes_its_own_file_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        {
            let _guard = PidFileGuard::create(&path).unwrap();
            assert_eq!(read(&path).unwrap().pid, std::process::id());
        }
        assert!(!path.exists());

        // A file taken over by another pid stays.
        let guard = PidFileGuard::create(&path).unwrap();
        std::fs::write(&path, "1\nother\n").unwrap();
        drop(guard);
        assert!(path.exists());
    }
}
