//! Console mode: the questions on stdin, as Java's console mode.
// Java: org/l2jmobius/tools/DatabaseInstaller.java (`console`, `installDatabaseMenu`)
//
// Generic over the input and output so the tests can script a whole session.

use std::io::{BufRead, Write};
use std::path::PathBuf;

use sqlx::mysql::MySqlConnectOptions;

use crate::installer::{self, ErrorChoice, ExistsChoice, Level, Params, Ui};

pub const DEFAULT_HOST: &str = "localhost";
pub const DEFAULT_PORT: &str = "3306";
pub const DEFAULT_USER: &str = "root";
pub const DEFAULT_DB: &str = "l2jmobiush5";

pub struct ConsoleUi<R: BufRead, W: Write> {
    input: R,
    out: W,
    last_percent: usize,
}

impl<R: BufRead, W: Write> ConsoleUi<R, W> {
    pub fn new(input: R, out: W) -> Self {
        Self {
            input,
            out,
            last_percent: 0,
        }
    }

    pub fn into_output(self) -> W {
        self.out
    }

    fn say(&mut self, text: &str) {
        // A closed stdout is not worth stopping an installation for.
        let _ = writeln!(self.out, "{text}");
    }

    /// The next line, trimmed. `None` at the end of the input.
    fn read_line(&mut self) -> Option<String> {
        let _ = self.out.flush();
        let mut line = String::new();
        match self.input.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim().to_owned()),
        }
    }

    fn ask(&mut self, label: &str) -> Option<String> {
        let _ = write!(self.out, "{label}");
        self.read_line()
    }

    /// `label [default]: ` with the default used for an empty answer.
    fn ask_default(&mut self, label: &str, default: &str) -> Option<String> {
        let answer = self.ask(&format!("{label} [{default}]: "))?;
        Some(if answer.is_empty() {
            default.to_owned()
        } else {
            answer
        })
    }

    /// The main menu loop. Returns when the user exits or the input ends.
    pub async fn run_menu(&mut self, sql_dir: PathBuf) {
        self.say("=========================================================");
        self.say("       L2jMobius Development - Database Installer        ");
        self.say("             (Rust port of the Java installer)           ");
        self.say("=========================================================");
        loop {
            self.say("");
            self.say("=========================================================");
            self.say("                  AVAILABLE COMMANDS                     ");
            self.say("---------------------------------------------------------");
            self.say("  [1] Install Database");
            self.say("  [2] Exit");
            self.say("---------------------------------------------------------");
            let Some(choice) = self.ask("\nEnter your choice: ") else {
                return;
            };
            match choice.as_str() {
                "1" => self.install_menu(&sql_dir).await,
                "2" => {
                    self.say("\n[EXIT] Exiting the application. Goodbye!");
                    return;
                }
                _ => self.say("\n[WARNING] Invalid choice. Please select a valid option."),
            }
        }
    }

    async fn install_menu(&mut self, sql_dir: &std::path::Path) {
        self.say("=========================================================");
        self.say("             L2jMobius Database Installation             ");
        self.say("=========================================================");
        self.say("[1] Install Both Login and Game Databases");
        self.say("[2] Install Login Database");
        self.say("[3] Install Game Database");
        self.say("[4] Exit");
        let Some(choice) = self.ask("\nEnter your choice: ") else {
            return;
        };
        let (login, game) = match choice.as_str() {
            "1" => (true, true),
            "2" => (true, false),
            "3" => (false, true),
            "4" => {
                self.say("[INFO] Exiting installation...");
                return;
            }
            _ => {
                self.say("[ERROR] Invalid choice. Please select a valid option.");
                return;
            }
        };

        let Some(host) = self.ask_default("Enter MySQL Host", DEFAULT_HOST) else {
            return;
        };
        let Some(port) = self.ask_default("Enter MySQL Port", DEFAULT_PORT) else {
            return;
        };
        let Some(user) = self.ask_default("Enter MySQL Username", DEFAULT_USER) else {
            return;
        };
        // The password is echoed: there is no terminal-mode dependency (see the card report).
        let Some(password) = self.ask("Enter MySQL Password: ") else {
            return;
        };
        let Some(db_name) = self.ask_default("Enter Database Name", DEFAULT_DB) else {
            return;
        };
        let Ok(port) = port.parse::<u16>() else {
            self.say(&format!("[ERROR] Invalid port: {port}"));
            return;
        };

        let params = Params {
            options: MySqlConnectOptions::new()
                .host(&host)
                .port(port)
                .username(&user)
                .password(&password),
            db_name,
            sql_dir: sql_dir.to_path_buf(),
            login,
            game,
        };
        match installer::run(&params, self).await {
            Ok(summary) => {
                self.say(&format!(
                    "[INFO] Installation completed: {} files, {} statements ok, {} failed.",
                    summary.files, summary.statements_ok, summary.statements_failed
                ));
            }
            Err(e) => {
                self.say(&format!("[ERROR] {e}"));
                self.say("[ERROR] Installation failed. Please check the messages above.");
            }
        }
    }
}

impl<R: BufRead, W: Write> Ui for ConsoleUi<R, W> {
    fn log(&mut self, level: Level, message: &str) {
        let tag = match level {
            Level::Info => "[INFO]",
            Level::Success => "[OK]",
            Level::Error => "[ERROR]",
        };
        self.say(&format!("{tag} {message}"));
    }

    fn progress(&mut self, done: usize, total: usize) {
        let percent = (done * 100).checked_div(total).unwrap_or(100);
        // Every ten percent is enough for a terminal.
        if percent / 10 != self.last_percent / 10 || done == 0 {
            self.say(&format!("[INFO] Progress: {percent}%"));
        }
        self.last_percent = percent;
    }

    fn ask_exists(&mut self, db_name: &str) -> ExistsChoice {
        self.say(&format!("[WARNING] Database '{db_name}' already exists."));
        self.say("  [R] Reset: DROP and CREATE it again. This DELETES ALL DATA in the database.");
        self.say("  [K] Keep: install into it (the scripts still DROP and recreate their tables).");
        self.say("  [C] Cancel");
        let Some(answer) = self.ask("Your choice [C]: ") else {
            return ExistsChoice::Cancel;
        };
        match answer.to_lowercase().as_str() {
            "r" | "reset" => {
                let prompt = format!("Type 'yes' to delete everything in '{db_name}': ");
                match self.ask(&prompt) {
                    Some(confirm) if confirm.eq_ignore_ascii_case("yes") => ExistsChoice::Reset,
                    _ => ExistsChoice::Cancel,
                }
            }
            "k" | "keep" => ExistsChoice::Keep,
            _ => ExistsChoice::Cancel,
        }
    }

    fn ask_error(&mut self, _sql: &str, error: &str) -> ErrorChoice {
        self.say(&format!("[ERROR] MySQL Error: {error}"));
        let Some(answer) = self.ask("[C]ontinue or [A]bort? [C]: ") else {
            return ErrorChoice::Abort;
        };
        match answer.to_lowercase().as_str() {
            "a" | "abort" => ErrorChoice::Abort,
            _ => ErrorChoice::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn ui(input: &str) -> ConsoleUi<Cursor<Vec<u8>>, Vec<u8>> {
        ConsoleUi::new(Cursor::new(input.as_bytes().to_vec()), Vec::new())
    }

    fn output(ui: ConsoleUi<Cursor<Vec<u8>>, Vec<u8>>) -> String {
        String::from_utf8(ui.into_output()).unwrap()
    }

    #[test]
    fn reset_needs_an_explicit_yes() {
        assert_eq!(ui("r\nyes\n").ask_exists("db"), ExistsChoice::Reset);
        assert_eq!(ui("R\nYES\n").ask_exists("db"), ExistsChoice::Reset);
        assert_eq!(ui("r\nno\n").ask_exists("db"), ExistsChoice::Cancel);
        assert_eq!(ui("r\n").ask_exists("db"), ExistsChoice::Cancel);
    }

    #[test]
    fn the_reset_question_names_the_database() {
        let mut u = ui("c\n");
        assert_eq!(u.ask_exists("my_game_db"), ExistsChoice::Cancel);
        assert!(output(u).contains("'my_game_db' already exists"));
    }

    #[test]
    fn keep_cancel_and_default() {
        assert_eq!(ui("k\n").ask_exists("db"), ExistsChoice::Keep);
        assert_eq!(ui("c\n").ask_exists("db"), ExistsChoice::Cancel);
        assert_eq!(ui("\n").ask_exists("db"), ExistsChoice::Cancel);
        assert_eq!(ui("").ask_exists("db"), ExistsChoice::Cancel);
    }

    #[test]
    fn error_choice_defaults_to_continue_and_aborts_at_end_of_input() {
        assert_eq!(ui("\n").ask_error("s", "e"), ErrorChoice::Continue);
        assert_eq!(ui("c\n").ask_error("s", "e"), ErrorChoice::Continue);
        assert_eq!(ui("a\n").ask_error("s", "e"), ErrorChoice::Abort);
        assert_eq!(ui("").ask_error("s", "e"), ErrorChoice::Abort);
    }

    #[test]
    fn defaults_fill_empty_answers() {
        let mut u = ui("\n  \nvalue\n");
        assert_eq!(u.ask_default("Host", "localhost").unwrap(), "localhost");
        assert_eq!(u.ask_default("Host", "localhost").unwrap(), "localhost");
        assert_eq!(u.ask_default("Host", "localhost").unwrap(), "value");
        assert!(u.ask_default("Host", "localhost").is_none());
    }

    #[tokio::test]
    async fn menu_exits_on_2_on_end_of_input_and_rejects_garbage() {
        let mut u = ui("x\n2\n");
        u.run_menu(PathBuf::from("unused")).await;
        let text = output(u);
        assert!(text.contains("Invalid choice"));
        assert!(text.contains("Goodbye"));

        let mut u = ui("");
        u.run_menu(PathBuf::from("unused")).await;
    }

    #[tokio::test]
    async fn an_invalid_port_stops_before_any_connection() {
        // Menu 1 (install), 1 (both), host, port "abc".
        let mut u = ui("1\n1\nlocalhost\nabc\nroot\npw\ndb\n2\n");
        u.run_menu(PathBuf::from("unused")).await;
        assert!(output(u).contains("Invalid port: abc"));
    }

    #[tokio::test]
    async fn a_missing_sql_folder_stops_before_any_connection() {
        let mut u = ui("1\n2\n\n\n\n\n\n2\n");
        u.run_menu(PathBuf::from("this-folder-does-not-exist")).await;
        let text = output(u);
        assert!(text.contains("Directory not found"));
        assert!(!text.contains("Connected."));
    }

    #[test]
    fn progress_prints_every_ten_percent() {
        let mut u = ui("");
        for done in 0..=100 {
            u.progress(done, 100);
        }
        let lines = output(u).lines().count();
        assert_eq!(lines, 11);
    }
}
