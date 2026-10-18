//! `login-server [--root DIR] [--config-dir DIR] [--data-dir DIR]`
//!
//! Start it from a copy of `dist/login` (or give `--root`). Runs until Ctrl+C or SIGTERM
//! (exit code 0). Exit code 2 asks the wrapper (start script, supervisor) to start it again,
//! exit code 1 is a fatal startup error.
// Java: org/l2jmobius/loginserver/LoginServer.java (main)

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use login_server::state::ExitReason;
use login_server::{Paths, Server, logging};
use tracing::error;

/// L2 login server.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Folder with the `.ini` files, relative to `--root`.
    #[arg(long, default_value = "./config")]
    config_dir: PathBuf,
    /// Folder with `servername.xml`, relative to `--root`.
    #[arg(long, default_value = "./data")]
    data_dir: PathBuf,
    /// The server's working folder, like the Java `dist/login` folder: `log/` and
    /// `banned_ip.cfg` live here.
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let paths = Paths::resolve(&cli.root, &cli.config_dir, &cli.data_dir);

    // Held until the end of `main` so the buffered log lines are flushed before the exit.
    let _log_guard = match logging::init(&paths.log_dir) {
        Ok(guard) => guard,
        Err(error) => {
            // The logger is not there yet, so this is the one place that prints directly.
            eprintln!("FATAL: cannot start logging: {error}");
            return ExitCode::from(ExitReason::Fatal.code());
        }
    };

    let server = match Server::start(&paths).await {
        Ok(server) => server,
        Err(error) => {
            error!("FATAL: {error}");
            return ExitCode::from(ExitReason::Fatal.code());
        }
    };
    let reason = server.run_until(wait_for_stop_signal()).await;
    ExitCode::from(reason.code())
}

#[cfg(unix)]
async fn wait_for_stop_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let Ok(mut terminate) = signal(SignalKind::terminate()) else {
        let _ = tokio::signal::ctrl_c().await;
        return;
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_stop_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_work_from_a_dist_login_copy() {
        let cli = Cli::try_parse_from(["login-server"]).unwrap();
        assert_eq!(cli.config_dir, PathBuf::from("./config"));
        assert_eq!(cli.data_dir, PathBuf::from("./data"));
        assert_eq!(cli.root, PathBuf::from("."));
    }

    #[test]
    fn all_options_can_be_given() {
        let cli = Cli::try_parse_from([
            "login-server",
            "--root",
            "/srv/login",
            "--config-dir",
            "cfg",
            "--data-dir",
            "dat",
        ])
        .unwrap();
        assert_eq!(cli.root, PathBuf::from("/srv/login"));
        assert_eq!(cli.config_dir, PathBuf::from("cfg"));
        assert_eq!(cli.data_dir, PathBuf::from("dat"));
    }

    #[test]
    fn unknown_options_are_rejected() {
        assert!(Cli::try_parse_from(["login-server", "--nope"]).is_err());
    }
}
