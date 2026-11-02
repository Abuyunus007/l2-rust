//! The installation on its own thread, for any egui window (the installer's own and the admin GUI's
//! setup wizard).
// Java: org/l2jmobius/tools/DatabaseInstaller.java (`installDatabase`)
//
// The worker runs [`installer::run`] on a small tokio runtime and talks to the window through a
// channel of [`Event`]s. Its questions (database exists, statement failed) are events that carry a
// reply channel; the worker blocks until the window answers. Nothing here touches a database
// before [`spawn_install`] / [`spawn_test`] is called.

use std::io;
use std::sync::mpsc::{self, Receiver, Sender};

use sqlx::mysql::MySqlConnectOptions;

use crate::installer::{self, DatabaseState, ErrorChoice, ExistsChoice, Level, Params, Ui};

/// What the worker tells the window.
pub enum Event {
    Log(Level, String),
    /// Overall progress, 0.0 to 1.0.
    Progress(f32),
    AskExists {
        db_name: String,
        reply: Sender<ExistsChoice>,
    },
    AskError {
        error: String,
        reply: Sender<ErrorChoice>,
    },
    /// Last event; the channel closes after it.
    Done,
}

/// The [`Ui`] of the worker thread: forwards everything to the window.
struct ChannelUi {
    tx: Sender<Event>,
    ctx: egui::Context,
}

impl ChannelUi {
    fn send(&self, event: Event) {
        // The window may be gone; the questions below then answer Cancel/Abort.
        let _ = self.tx.send(event);
        self.ctx.request_repaint();
    }
}

impl Ui for ChannelUi {
    fn log(&mut self, level: Level, message: &str) {
        self.send(Event::Log(level, message.to_owned()));
    }

    fn progress(&mut self, done: usize, total: usize) {
        let fraction = if total == 0 {
            1.0
        } else {
            done as f32 / total as f32
        };
        self.send(Event::Progress(fraction));
    }

    fn ask_exists(&mut self, db_name: &str) -> ExistsChoice {
        let (reply, answer) = mpsc::channel();
        self.send(Event::AskExists {
            db_name: db_name.to_owned(),
            reply,
        });
        answer.recv().unwrap_or(ExistsChoice::Cancel)
    }

    fn ask_error(&mut self, _sql: &str, error: &str) -> ErrorChoice {
        let (reply, answer) = mpsc::channel();
        self.send(Event::AskError {
            error: error.to_owned(),
            reply,
        });
        answer.recv().unwrap_or(ErrorChoice::Abort)
    }
}

/// Starts the installation on a new thread. The caller polls the receiver once per frame.
pub fn spawn_install(params: Params, ctx: egui::Context) -> io::Result<Receiver<Event>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("db-installer".to_owned())
        .spawn(move || worker(params, tx, ctx))?;
    Ok(rx)
}

fn worker(params: Params, tx: Sender<Event>, ctx: egui::Context) {
    let mut ui = ChannelUi { tx, ctx: ctx.clone() };
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build();
    match runtime {
        Ok(rt) => match rt.block_on(installer::run(&params, &mut ui)) {
            Ok(summary) => {
                let what = match (params.login, params.game) {
                    (true, true) => "Login and Game databases are installed.",
                    (true, false) => "Login database is installed.",
                    _ => "Game database is installed.",
                };
                ui.log(Level::Success, what);
                ui.log(
                    Level::Info,
                    &format!(
                        "{} files, {} statements ok, {} failed.",
                        summary.files, summary.statements_ok, summary.statements_failed
                    ),
                );
            }
            Err(e) => {
                ui.log(Level::Error, &e.to_string());
                ui.log(
                    Level::Error,
                    "Installation failed. Please check the messages above.",
                );
            }
        },
        Err(e) => ui.log(Level::Error, &format!("Cannot start the runtime: {e}")),
    }
    ui.send(Event::Done);
}

/// The answer of [`spawn_test`].
pub type TestResult = Result<DatabaseState, String>;

/// Runs [`installer::test_connection`] on a new thread (one connection, closed again).
pub fn spawn_test(
    options: MySqlConnectOptions,
    db_name: String,
    ctx: egui::Context,
) -> io::Result<Receiver<TestResult>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("db-test".to_owned())
        .spawn(move || {
            let result = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt.block_on(installer::test_connection(&options, &db_name)),
                Err(e) => Err(format!("Cannot start the runtime: {e}")),
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        })?;
    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_ui_questions_cancel_when_the_window_is_gone() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let mut ui = ChannelUi {
            tx,
            ctx: egui::Context::default(),
        };
        assert_eq!(ui.ask_exists("db"), ExistsChoice::Cancel);
        assert_eq!(ui.ask_error("s", "e"), ErrorChoice::Abort);
    }
}
