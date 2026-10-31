//! `l2-db-installer`: installs the login and/or game schema from `dist/db_installer/sql`.
// Java: org/l2jmobius/tools/DatabaseInstaller.java
//
// No database is contacted before the user presses Install (GUI) or confirms the questions
// (console). The password is never logged or written to a file.

use std::io::{self, IsTerminal};
use std::path::PathBuf;

use clap::Parser;
use l2_db_installer::console::ConsoleUi;
use l2_db_installer::gui;
use l2_db_installer::paths::{Roots, default_sql_dir, load_interface};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about = "Installs the L2JMobius login and game database schema")]
struct Cli {
    /// Ask the questions on the terminal instead of opening a window.
    #[arg(long)]
    console: bool,
    /// Folder with the `login` and `game` script folders
    /// (default: `../db_installer/sql` next to the exe, or `dist/db_installer/sql`).
    #[arg(long, value_name = "DIR")]
    sql: Option<PathBuf>,
}

/// A window needs a display on Linux; Windows and macOS always have one for a desktop user.
fn display_available() -> bool {
    if cfg!(any(target_os = "linux", target_os = "freebsd")) {
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
    } else {
        true
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();
    let cli = Cli::parse();
    let roots = Roots::detect();
    let sql_dir = cli
        .sql
        .or_else(|| default_sql_dir(&roots))
        // Not found: the install step reports "Directory not found" with this path.
        .unwrap_or_else(|| PathBuf::from("../db_installer/sql"));
    let interface = load_interface(&roots);

    if !cli.console && interface.enable_gui && display_available() {
        match gui::run(sql_dir.clone(), interface.dark_theme) {
            Ok(()) => return Ok(()),
            Err(e) => eprintln!("[WARNING] No window available ({e}); using the console."),
        }
    }

    if !io::stdin().is_terminal() {
        eprintln!("[INFO] Reading the answers from stdin (not a terminal).");
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut console = ConsoleUi::new(io::stdin().lock(), io::stdout());
    runtime.block_on(console.run_menu(sql_dir));
    Ok(())
}
