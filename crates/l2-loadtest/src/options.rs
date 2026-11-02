//! The command line.

use std::path::PathBuf;

use clap::Parser;

/// The real login server port (Java `LoginPort`). Aimed at without the explicit flag, the tool
/// refuses to run: a load test must never hit the owner's server.
pub const REAL_LOGIN_PORT: u16 = 2106;

/// Why the options are refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OptionsError {
    #[error(
        "--login points at port {REAL_LOGIN_PORT}, the real login server. Run the load test against a test setup only \
         (its own database and ports). If this really is a test server, add --i-know-this-is-a-test-server"
    )]
    RealPort,
    #[error("--login must be host:port, got '{0}'")]
    BadLogin(String),
    #[error("--count must be at least 1")]
    NoBots,
    #[error("--ramp must be a number above 0")]
    BadRamp,
    #[error("--duration must be at least 1 second")]
    NoDuration,
    #[error("--accounts must be 1 to 10 letters or digits, got '{0}'")]
    BadPrefix(String),
    #[error("--sample-interval must be at least 1 second")]
    BadInterval,
}

#[derive(Debug, Clone, Parser)]
#[command(
    name = "l2-loadtest",
    about = "Logs in N bots through the login and game server, plays a simple loop and measures the server. \
             For test setups only."
)]
pub struct Options {
    /// The login server, host:port.
    #[arg(long)]
    pub login: String,
    /// How many bots.
    #[arg(long, default_value_t = 20)]
    pub count: u32,
    /// Bots started per second.
    #[arg(long, default_value_t = 10.0)]
    pub ramp: f64,
    /// Seconds the run lasts, counted from the start of the first bot (the ramp is part of it).
    #[arg(long, default_value_t = 60)]
    pub duration: u64,
    /// Account name prefix: the accounts are `prefix0001`, `prefix0002`, ... (the login server
    /// creates them with `AutoCreateAccounts`). Letters and digits only: the characters get the
    /// same names.
    #[arg(long, default_value = "lt")]
    pub accounts: String,
    /// The password of every bot account. A throwaway value, never a real one.
    #[arg(long, default_value = "loadtest")]
    pub password: String,
    /// Writes the summary table to this file and the samples to `<file>.csv`.
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// The game server's admin endpoint, host:port. Without it only our own numbers are sampled.
    #[arg(long)]
    pub admin: Option<String>,
    /// The admin token (empty: a loopback endpoint of a supervisor setup needs none, D-56).
    /// Falls back to the `L2_ADMIN_TOKEN` environment variable.
    #[arg(long, default_value = "")]
    pub admin_token: String,
    /// Seconds between two samples.
    #[arg(long, default_value_t = 5)]
    pub sample_interval: u64,
    /// Seed of the bots' random choices (each bot derives its own).
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// Needed to aim at port 2106.
    #[arg(long)]
    pub i_know_this_is_a_test_server: bool,
}

impl Options {
    /// Checks the options. Runs before anything connects.
    pub fn validate(&self) -> Result<(), OptionsError> {
        let port = self
            .login
            .rsplit_once(':')
            .and_then(|(host, port)| (!host.is_empty()).then_some(port))
            .and_then(|port| port.parse::<u16>().ok())
            .ok_or_else(|| OptionsError::BadLogin(self.login.clone()))?;
        if port == REAL_LOGIN_PORT && !self.i_know_this_is_a_test_server {
            return Err(OptionsError::RealPort);
        }
        if self.count == 0 {
            return Err(OptionsError::NoBots);
        }
        if !(self.ramp.is_finite() && self.ramp > 0.0) {
            return Err(OptionsError::BadRamp);
        }
        if self.duration == 0 {
            return Err(OptionsError::NoDuration);
        }
        if self.accounts.is_empty()
            || self.accounts.len() > 10
            || !self.accounts.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return Err(OptionsError::BadPrefix(self.accounts.clone()));
        }
        if self.sample_interval == 0 {
            return Err(OptionsError::BadInterval);
        }
        Ok(())
    }

    /// The admin token to use: the option, else the environment.
    pub fn effective_admin_token(&self) -> String {
        if self.admin_token.is_empty() {
            std::env::var(l2_admin_api::ADMIN_TOKEN_ENV).unwrap_or_default()
        } else {
            self.admin_token.clone()
        }
    }
}

/// The account (and character) name of bot `index` (from 0): `prefix0001`, `prefix0002`, ...
pub fn account_name(prefix: &str, index: u32) -> String {
    format!("{prefix}{:04}", index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(login: &str) -> Options {
        Options::parse_from(["l2-loadtest", "--login", login])
    }

    #[test]
    fn defaults_are_valid() {
        let o = options("127.0.0.1:12345");
        assert_eq!((o.count, o.duration, o.accounts.as_str()), (20, 60, "lt"));
        assert_eq!(o.validate(), Ok(()));
    }

    #[test]
    fn refuses_the_real_login_port_without_the_flag() {
        assert_eq!(options("127.0.0.1:2106").validate(), Err(OptionsError::RealPort));
        assert_eq!(options("some.host:2106").validate(), Err(OptionsError::RealPort));
        let o = Options::parse_from([
            "l2-loadtest",
            "--login",
            "127.0.0.1:2106",
            "--i-know-this-is-a-test-server",
        ]);
        assert_eq!(o.validate(), Ok(()));
    }

    #[test]
    fn refuses_nonsense() {
        assert!(matches!(
            options("nohost").validate(),
            Err(OptionsError::BadLogin(_))
        ));
        assert!(matches!(
            options(":2106").validate(),
            Err(OptionsError::BadLogin(_))
        ));
        assert!(matches!(
            options("h:99999").validate(),
            Err(OptionsError::BadLogin(_))
        ));
        let mut o = options("h:1");
        o.count = 0;
        assert_eq!(o.validate(), Err(OptionsError::NoBots));
        let mut o = options("h:1");
        o.ramp = 0.0;
        assert_eq!(o.validate(), Err(OptionsError::BadRamp));
        o.ramp = f64::NAN;
        assert_eq!(o.validate(), Err(OptionsError::BadRamp));
        let mut o = options("h:1");
        o.accounts = "a_b".into();
        assert!(matches!(o.validate(), Err(OptionsError::BadPrefix(_))));
        o.accounts = "waytoolongprefix".into();
        assert!(matches!(o.validate(), Err(OptionsError::BadPrefix(_))));
    }

    #[test]
    fn account_names_count_from_one() {
        assert_eq!(account_name("lt", 0), "lt0001");
        assert_eq!(account_name("lt", 1999), "lt2000");
        assert_eq!(account_name("lt", 9999), "lt10000");
    }
}
