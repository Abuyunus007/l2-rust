//! The secondary password: the lock on the character screen (L-04).
// Java: security/SecondaryPasswordAuth.java
// Java: data/xml/SecondaryAuthData.java (config/SecondaryAuth.xml)
// Java: network/clientpackets/{RequestEx2ndPasswordCheck,RequestEx2ndPasswordReq,
// Java: RequestEx2ndPasswordVerify}.java
// Java: network/clientpackets/CharacterSelect.java (the lock)
// Java: network/GameClient.java (setAccountName creates the `SecondaryPasswordAuth`)
//
// Everything here runs in the connection task before the world (like the character screen), so it
// may use the database (design: `docs/design/world-loop.md`). The rows are `account_gsdata`
// (`secauth_pwd` the hashed password, `secauth_wte` the wrong attempts in a row), shared with the
// Java game server (D-39).
//
// Left out: the `SendMail` to the login server after a temporary ban. Its opcode (0x09) is known
// to Java's login server and answered with nothing ("not implemented"), and so it is by ours.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use l2_db::{AccountDataRepository, VAR_PWD, VAR_WTE, crypt_secondary_password};
use l2_net::{Control, Encode};
use l2_protocol_game::client::{RequestEx2ndPasswordReq, RequestEx2ndPasswordVerify};
use l2_protocol_game::server::{Ex2ndPasswordAck, Ex2ndPasswordCheck, Ex2ndPasswordVerify, LeaveWorld};
use quick_xml::Reader;
use quick_xml::events::Event;
use tracing::{error, info, warn};

use super::BoxFuture;
use super::session::ClientSession;

/// Name inside the config directory (Java: `config/SecondaryAuth.xml`).
pub const SECONDARY_AUTH_FILE: &str = "SecondaryAuth.xml";

/// Java `SecondaryAuthData`: the settings of `SecondaryAuth.xml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryAuthData {
    /// `<enabled>`.
    pub enabled: bool,
    /// `<maxAttempts>`.
    pub max_attempts: i32,
    /// `<banTime>`, in minutes.
    pub ban_time: i32,
    /// `<recoveryLink>`.
    pub recovery_link: String,
    /// `<forbiddenPasswords>`.
    pub forbidden: HashSet<String>,
}

impl Default for SecondaryAuthData {
    /// The field defaults of the Java class, which a missing file keeps.
    fn default() -> Self {
        Self {
            enabled: false,
            max_attempts: 5,
            ban_time: 480,
            recovery_link: String::new(),
            forbidden: HashSet::new(),
        }
    }
}

impl SecondaryAuthData {
    /// Reads `SecondaryAuth.xml` from the config directory; without it the defaults stay
    /// (the feature is off).
    // Java: SecondaryAuthData.load
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(SECONDARY_AUTH_FILE);
        let data = match std::fs::read(&path) {
            Ok(bytes) => Self::parse(&String::from_utf8_lossy(&bytes)),
            Err(e) => {
                warn!("Could not read file {}: {e}", path.display());
                Self::default()
            }
        };
        info!(
            "SecondaryAuthData: Loaded {} forbidden passwords.",
            data.forbidden.len()
        );
        data
    }

    /// The elements `enabled`, `maxAttempts`, `banTime`, `recoveryLink` and `forbiddenPasswords`
    /// (with `password` children) of the `list` element. A number that does not parse ends the
    /// reading, like the `NumberFormatException` of Java's `parseDocument` (what was read
    /// before stays).
    // Java: SecondaryAuthData.parseDocument
    pub fn parse(xml: &str) -> Self {
        let mut data = Self::default();
        let mut reader = Reader::from_str(xml);
        // The element whose text is being collected, and the text so far.
        let mut open: Option<String> = None;
        let mut text = String::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(element)) => {
                    let name = element.name().as_ref().to_owned();
                    if ["enabled", "maxattempts", "bantime", "recoverylink", "password"]
                        .contains(&name.to_lowercase().as_str())
                    {
                        open = Some(name.to_lowercase());
                        text.clear();
                    }
                }
                Ok(Event::Text(t)) if open.is_some() => {
                    text.push_str(&t.xml10_content());
                }
                Ok(Event::End(_)) => {
                    let Some(name) = open.take() else {
                        continue;
                    };
                    match name.as_str() {
                        "enabled" => data.enabled = text.trim().eq_ignore_ascii_case("true"),
                        "maxattempts" => match text.trim().parse() {
                            Ok(value) => data.max_attempts = value,
                            Err(_) => return Self::failed(data),
                        },
                        "bantime" => match text.trim().parse() {
                            Ok(value) => data.ban_time = value,
                            Err(_) => return Self::failed(data),
                        },
                        "recoverylink" => data.recovery_link = text.clone(),
                        "password" => {
                            data.forbidden.insert(text.clone());
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(e) => {
                    warn!("Could not parse {SECONDARY_AUTH_FILE}: {e}");
                    break;
                }
            }
        }
        data
    }

    fn failed(data: Self) -> Self {
        warn!("Failed to load secondary auth data from xml.");
        data
    }

    /// Java `isForbiddenPassword`.
    pub fn is_forbidden(&self, password: &str) -> bool {
        self.forbidden.contains(password)
    }

    /// Java `validatePassword`: digits only, 6 to 8 characters, not on the forbidden list.
    // NOTE: Java's `Character.isDigit` also accepts the digits of other scripts; the client only
    // sends ASCII digits, so only those count here.
    pub fn is_valid_password(&self, password: &str) -> bool {
        if password.is_empty() || !password.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        let length = password.encode_utf16().count();
        if !(6..=8).contains(&length) {
            return false;
        }
        !self.is_forbidden(password)
    }
}

/// The `account_gsdata` rows the secondary password reads and writes. [`DbSecondaryStore`] in the
/// server, a fake in tests. The errors are the messages Java logs.
pub trait SecondaryStore: Send + Sync + 'static {
    /// The `secauth_*` rows of an account: `(var, value)`.
    fn load<'a>(&'a self, account: &'a str) -> BoxFuture<'a, Result<Vec<(String, String)>, String>>;
    /// The first password.
    fn insert_password<'a>(&'a self, account: &'a str, hashed: &'a str) -> BoxFuture<'a, Result<(), String>>;
    /// A changed password.
    fn update_password<'a>(&'a self, account: &'a str, hashed: &'a str) -> BoxFuture<'a, Result<(), String>>;
    /// The wrong attempts in a row.
    fn insert_wrong_attempt<'a>(
        &'a self,
        account: &'a str,
        attempts: i32,
    ) -> BoxFuture<'a, Result<(), String>>;
}

/// [`SecondaryStore`] on the game database.
pub struct DbSecondaryStore(AccountDataRepository);

impl DbSecondaryStore {
    pub fn new(pool: l2_db::Pool) -> Self {
        Self(AccountDataRepository::new(pool))
    }
}

impl SecondaryStore for DbSecondaryStore {
    fn load<'a>(&'a self, account: &'a str) -> BoxFuture<'a, Result<Vec<(String, String)>, String>> {
        Box::pin(async move {
            self.0
                .secondary_auth_of(account)
                .await
                .map(|rows| rows.into_iter().map(|r| (r.var, r.value)).collect())
                .map_err(|e| e.to_string())
        })
    }

    fn insert_password<'a>(&'a self, account: &'a str, hashed: &'a str) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.0
                .insert_password(account, hashed)
                .await
                .map_err(|e| e.to_string())
        })
    }

    fn update_password<'a>(&'a self, account: &'a str, hashed: &'a str) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.0
                .update_password(account, hashed)
                .await
                .map_err(|e| e.to_string())
        })
    }

    fn insert_wrong_attempt<'a>(
        &'a self,
        account: &'a str,
        attempts: i32,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async move {
            self.0
                .insert_wrong_attempt(account, attempts)
                .await
                .map_err(|e| e.to_string())
        })
    }
}

/// The settings and the store every session shares.
pub struct SecondaryAuthService {
    pub data: SecondaryAuthData,
    pub store: Arc<dyn SecondaryStore>,
}

/// A store that is never asked (the feature is off).
struct NoStore;

impl SecondaryStore for NoStore {
    fn load<'a>(&'a self, _account: &'a str) -> BoxFuture<'a, Result<Vec<(String, String)>, String>> {
        Box::pin(async { Ok(Vec::new()) })
    }
    fn insert_password<'a>(
        &'a self,
        _account: &'a str,
        _hashed: &'a str,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
    fn update_password<'a>(
        &'a self,
        _account: &'a str,
        _hashed: &'a str,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
    fn insert_wrong_attempt<'a>(
        &'a self,
        _account: &'a str,
        _attempts: i32,
    ) -> BoxFuture<'a, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
}

impl SecondaryAuthService {
    /// The feature is off: no session has a lock.
    pub fn disabled() -> Self {
        Self {
            data: SecondaryAuthData::default(),
            store: Arc::new(NoStore),
        }
    }
}

/// Java `SecondaryPasswordAuth`: the state of one client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SecondaryAuth {
    /// `_password`: the hashed password, `None` when the account has none.
    pub password: Option<String>,
    /// `_wrongAttempts`.
    pub wrong_attempts: i32,
    /// `_authed`.
    pub authed: bool,
}

impl SecondaryAuth {
    /// Java `loadPassword`. A failed read is logged; what was read before stays.
    pub async fn load(store: &dyn SecondaryStore, account: &str) -> Self {
        let mut auth = Self::default();
        match store.load(account).await {
            Ok(rows) => {
                for (var, value) in rows {
                    if var == VAR_PWD {
                        auth.password = Some(value);
                    } else if var == VAR_WTE {
                        match value.trim().parse() {
                            Ok(attempts) => auth.wrong_attempts = attempts,
                            Err(e) => {
                                error!("Error while reading password: {e}");
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => error!("Error while reading password: {e}"),
        }
        auth
    }

    /// Java `passwordExist`.
    pub fn password_exists(&self) -> bool {
        self.password.is_some()
    }

    /// Java `openDialog`: the window the client shows.
    pub fn dialog(&self) -> Ex2ndPasswordCheck {
        Ex2ndPasswordCheck {
            window_type: if self.password_exists() {
                Ex2ndPasswordCheck::PASSWORD_PROMPT
            } else {
                Ex2ndPasswordCheck::PASSWORD_NEW
            },
        }
    }
}

impl ClientSession {
    fn secondary_enabled(&self) -> bool {
        self.ctx.secondary.data.enabled
    }

    /// Java `GameClient.setAccountName`: with the feature on, the account's password is read.
    pub(super) async fn load_secondary_auth(&mut self) {
        if !self.secondary_enabled() {
            return;
        }
        let Some(account) = self.account.clone() else {
            return;
        };
        let ctx = Arc::clone(&self.ctx);
        self.secondary = Some(SecondaryAuth::load(&*ctx.secondary.store, &account).await);
    }

    /// Java `CharacterSelect`: `SecondaryAuthData.isEnabled() && !isAuthed()` opens the dialog and
    /// stops. `true` when the selection must stop.
    pub(super) fn secondary_blocks_select(&mut self, ctl: &mut Control<'_>) -> bool {
        if !self.secondary_enabled() {
            return false;
        }
        match &self.secondary {
            Some(auth) if auth.authed => false,
            Some(auth) => {
                ctl.send(auth.dialog().to_bytes());
                true
            }
            // Cannot happen: the state is created when the account logs in.
            None => true,
        }
    }

    /// Java `RequestEx2ndPasswordCheck.runImpl`.
    pub(super) fn on_2nd_password_check(&mut self, ctl: &mut Control<'_>) {
        let authed = self.secondary.as_ref().is_some_and(|a| a.authed);
        if !self.secondary_enabled() || authed {
            ctl.send(
                Ex2ndPasswordCheck {
                    window_type: Ex2ndPasswordCheck::PASSWORD_OK,
                }
                .to_bytes(),
            );
            return;
        }
        if let Some(auth) = &self.secondary {
            ctl.send(auth.dialog().to_bytes());
        }
    }

    /// Java `RequestEx2ndPasswordReq.runImpl`.
    pub(super) async fn on_2nd_password_req(
        &mut self,
        packet: RequestEx2ndPasswordReq,
        ctl: &mut Control<'_>,
    ) {
        if !self.secondary_enabled() {
            return;
        }
        let exists = self
            .secondary
            .as_ref()
            .is_some_and(SecondaryAuth::password_exists);
        let success = if packet.change_pass == 0 && !exists {
            self.save_secondary_password(&packet.password, ctl).await
        } else if packet.change_pass == 2 && exists {
            let new_password = packet.new_password.as_deref().unwrap_or("");
            self.change_secondary_password(&packet.password, new_password, ctl)
                .await
        } else {
            false
        };
        if success {
            ctl.send(
                Ex2ndPasswordAck {
                    response: Ex2ndPasswordAck::SUCCESS,
                }
                .to_bytes(),
            );
        }
    }

    /// Java `RequestEx2ndPasswordVerify.runImpl`.
    pub(super) async fn on_2nd_password_verify(
        &mut self,
        packet: RequestEx2ndPasswordVerify,
        ctl: &mut Control<'_>,
    ) {
        if !self.secondary_enabled() {
            return;
        }
        self.check_secondary_password(&packet.password, false, ctl).await;
    }

    /// Java `savePassword`.
    async fn save_secondary_password(&mut self, value: &str, ctl: &mut Control<'_>) -> bool {
        let ctx = Arc::clone(&self.ctx);
        let Some(account) = self.account.clone() else {
            return false;
        };
        if self
            .secondary
            .as_ref()
            .is_some_and(SecondaryAuth::password_exists)
        {
            warn!("[SecondaryPasswordAuth]{account} forced savePassword");
            ctl.send_and_close(LeaveWorld.to_bytes());
            return false;
        }
        if !ctx.secondary.data.is_valid_password(value) {
            ctl.send(
                Ex2ndPasswordAck {
                    response: Ex2ndPasswordAck::WRONG_PATTERN,
                }
                .to_bytes(),
            );
            return false;
        }
        let hashed = crypt_secondary_password(value);
        if let Err(e) = ctx.secondary.store.insert_password(&account, &hashed).await {
            error!("Error while writing password: {e}");
            return false;
        }
        if let Some(auth) = &mut self.secondary {
            auth.password = Some(hashed);
        }
        true
    }

    /// Java `changePassword`.
    async fn change_secondary_password(
        &mut self,
        old_password: &str,
        new_password: &str,
        ctl: &mut Control<'_>,
    ) -> bool {
        let ctx = Arc::clone(&self.ctx);
        let Some(account) = self.account.clone() else {
            return false;
        };
        if !self
            .secondary
            .as_ref()
            .is_some_and(SecondaryAuth::password_exists)
        {
            warn!("[SecondaryPasswordAuth]{account} forced changePassword");
            ctl.send_and_close(LeaveWorld.to_bytes());
            return false;
        }
        if !self.check_secondary_password(old_password, true, ctl).await {
            return false;
        }
        if !ctx.secondary.data.is_valid_password(new_password) {
            ctl.send(
                Ex2ndPasswordAck {
                    response: Ex2ndPasswordAck::WRONG_PATTERN,
                }
                .to_bytes(),
            );
            return false;
        }
        let hashed = crypt_secondary_password(new_password);
        if let Err(e) = ctx.secondary.store.update_password(&account, &hashed).await {
            error!("Error while reading password: {e}");
            return false;
        }
        if let Some(auth) = &mut self.secondary {
            auth.password = Some(hashed);
            auth.authed = false;
        }
        true
    }

    /// Java `checkPassword(value, skipAuth)`.
    async fn check_secondary_password(
        &mut self,
        value: &str,
        skip_auth: bool,
        ctl: &mut Control<'_>,
    ) -> bool {
        let ctx = Arc::clone(&self.ctx);
        let Some(account) = self.account.clone() else {
            return false;
        };
        let hashed = crypt_secondary_password(value);
        let Some(auth) = &mut self.secondary else {
            return false;
        };
        if auth.password.as_deref() != Some(hashed.as_str()) {
            auth.wrong_attempts += 1;
            let attempts = auth.wrong_attempts;
            let max = ctx.secondary.data.max_attempts;
            if attempts < max {
                ctl.send(
                    Ex2ndPasswordVerify {
                        mode: Ex2ndPasswordVerify::PASSWORD_WRONG,
                        wrong_attempts: attempts,
                    }
                    .to_bytes(),
                );
                Self::log_attempt(&ctx, &account, attempts).await;
            } else {
                ctx.gate
                    .temp_ban(
                        &account,
                        &self.ip.to_string(),
                        i64::from(ctx.secondary.data.ban_time),
                    )
                    .await;
                warn!(
                    "{account} - ({}) has inputted the wrong password {attempts} times in row.",
                    self.ip
                );
                Self::log_attempt(&ctx, &account, 0).await;
                ctl.send_and_close(
                    Ex2ndPasswordVerify {
                        mode: Ex2ndPasswordVerify::PASSWORD_BAN,
                        wrong_attempts: max,
                    }
                    .to_bytes(),
                );
            }
            return false;
        }
        if !skip_auth {
            auth.authed = true;
            // NOTE(java-quirk): the count sent is the one before the reset below, and the
            // in-memory count is never reset (only the row is).
            ctl.send(
                Ex2ndPasswordVerify {
                    mode: Ex2ndPasswordVerify::PASSWORD_OK,
                    wrong_attempts: auth.wrong_attempts,
                }
                .to_bytes(),
            );
        }
        Self::log_attempt(&ctx, &account, 0).await;
        true
    }

    /// Java `insertWrongAttempt`: a failed write is logged and goes on.
    async fn log_attempt(ctx: &super::ClientContext, account: &str, attempts: i32) {
        if let Err(e) = ctx.secondary.store.insert_wrong_attempt(account, attempts).await {
            error!("Error while writing wrong attempts: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../data/xsd/SecondaryAuth.xsd">
	<enabled>true</enabled> <!-- Enable Secondary Authentication on Character Select -->
	<maxAttempts>3</maxAttempts>
	<banTime>10</banTime>
	<recoveryLink>http://example.invalid/recover</recoveryLink>
	<forbiddenPasswords>
		<!-- Client checks for the entries below -->
		<password>000000</password>
		<password>123456</password>
	</forbiddenPasswords>
</list>"#;

    #[test]
    fn the_file_is_read_like_java() {
        let data = SecondaryAuthData::parse(SHIPPED);
        assert!(data.enabled);
        assert_eq!((data.max_attempts, data.ban_time), (3, 10));
        assert_eq!(data.recovery_link, "http://example.invalid/recover");
        assert_eq!(data.forbidden.len(), 2);
        assert!(data.is_forbidden("123456") && !data.is_forbidden("123457"));
    }

    #[test]
    fn a_missing_file_keeps_the_defaults_and_a_bad_number_stops_the_reading() {
        let data = SecondaryAuthData::default();
        assert!(!data.enabled);
        assert_eq!((data.max_attempts, data.ban_time), (5, 480));
        let broken = "<list><enabled>true</enabled><maxAttempts>x</maxAttempts><banTime>1</banTime></list>";
        let data = SecondaryAuthData::parse(broken);
        // `enabled` was read before the bad number; `banTime` after it is lost.
        assert!(data.enabled);
        assert_eq!(data.ban_time, 480);
    }

    #[test]
    fn a_password_is_six_to_eight_digits_and_not_forbidden() {
        let data = SecondaryAuthData::parse(SHIPPED);
        for good in ["123457", "1234567", "12345679"] {
            assert!(data.is_valid_password(good), "{good}");
        }
        for bad in ["", "12345", "123456789", "12345a", "123456", "000000", "12 456"] {
            assert!(!data.is_valid_password(bad), "{bad}");
        }
    }

    #[test]
    fn the_dialog_follows_the_password() {
        let none = SecondaryAuth::default();
        assert_eq!(none.dialog().window_type, Ex2ndPasswordCheck::PASSWORD_NEW);
        let some = SecondaryAuth {
            password: Some("x".into()),
            ..SecondaryAuth::default()
        };
        assert_eq!(some.dialog().window_type, Ex2ndPasswordCheck::PASSWORD_PROMPT);
    }
}
