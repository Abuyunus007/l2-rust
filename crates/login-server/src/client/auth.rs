//! Account checks of a login attempt, in Java's order.
//!
//! Java: `loginserver/controller/LoginController.java` (`retriveAccountInfo`,
//! `tryCheckinAccount`, `canCheckin`, `isValidIPAddress`) and `AccountInfo.java`.

use l2_db::{IpAuthKind, hash_password};
use l2_util::time::now_millis;
use tracing::{info, warn};

use crate::state::LoginState;

/// The account as the login flow sees it. Java: `AccountInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountInfo {
    /// Lower case, like Java's `AccountInfo` constructor.
    pub login: String,
    pub access_level: i32,
    pub last_server: i32,
}

/// Java: `LoginResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginResult {
    AuthSuccess,
    InvalidPassword,
    AccountBanned,
    AlreadyOnLs,
    AlreadyOnGs,
}

/// Looks the account up and checks the password; creates it when `AutoCreateAccounts` is on.
/// `None` means "wrong user or password" for the client. Every failure is counted per address
/// (IP ban after `LoginTryBeforeBan`), except database errors, like Java.
// Java: LoginController.retriveAccountInfo (both overloads)
pub async fn retrieve_account_info(
    state: &LoginState,
    address: &str,
    login: &str,
    password: &str,
) -> Option<AccountInfo> {
    let hash = hash_password(password);
    let repository = &state.repository;
    let mut auto_create = true;
    loop {
        let row = match repository.account_info(login, now_millis()).await {
            Ok(row) => row,
            Err(e) => {
                warn!("Exception while retriving account info for '{login}'! {e}");
                return None;
            }
        };
        if let Some(row) = row {
            // NOTE(java-quirk): Java's AccountInfo constructor throws for a NULL or empty
            // password hash or login; the exception is caught and the login fails without
            // counting an attempt.
            let (Some(stored), false) = (row.password.as_deref(), row.login.is_empty()) else {
                warn!("Exception while retriving account info for '{login}'! (empty login or password hash)");
                return None;
            };
            if stored.is_empty() {
                warn!("Exception while retriving account info for '{login}'! (empty password hash)");
                return None;
            }
            if stored != hash {
                // Not in Java: without this line a failed login leaves no trace for the admin.
                // Never log the password itself; its length helps spot truncated input.
                info!(
                    "Login failed for '{login}' from {address}: wrong password ({} characters received, attempt {} of {}).",
                    password.chars().count(),
                    state.failed_logins(address) + 1,
                    state.config.login.login_try_before_ban
                );
                state.record_failed_login(address);
                return None;
            }
            state.clear_failed_logins(address);
            return Some(AccountInfo {
                login: account_key(&row.login),
                access_level: row.access_level,
                last_server: row.last_server_or_zero(),
            });
        }

        if !auto_create || !state.config.login.auto_create_accounts {
            // Account does not exist and auto create is off.
            info!("Login failed for '{login}' from {address}: no such account (AutoCreateAccounts is off).");
            state.record_failed_login(address);
            return None;
        }
        if let Err(e) = repository
            .create_account(login, &hash, now_millis(), 0, address)
            .await
        {
            warn!("Exception while auto creating account for '{login}'! {e}");
            return None;
        }
        info!("Auto created account '{login}'.");
        auto_create = false;
    }
}

/// Java `isValidIPAddress`: four dot-separated parts, each 0..=255.
///
/// NOTE(java-quirk): a part that isn't a number makes Java's `Integer.parseInt` throw, which
/// aborts the whole `canCheckin` ("Could not finish login process!"), so such a row in
/// `accounts_ipauth` blocks the account. `None` reproduces that.
pub fn is_valid_ip(ip: &str) -> Option<bool> {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return Some(false);
    }
    for part in parts {
        let value: i32 = part.parse().ok()?;
        if !(0..=255).contains(&value) {
            return Some(false);
        }
    }
    Some(true)
}

/// IP allow/deny lists, then `lastactive`/`lastIP` update. `false` makes the login fail with
/// "wrong user or password".
// Java: LoginController.canCheckin
pub async fn can_check_in(state: &LoginState, address: &str, info: &AccountInfo) -> bool {
    let rows = match state.repository.ip_auth(&info.login).await {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Could not finish login process! {e}");
            return false;
        }
    };
    let mut allow = Vec::new();
    let mut deny = Vec::new();
    for row in &rows {
        match is_valid_ip(&row.ip) {
            Some(true) => {}
            Some(false) => continue,
            None => {
                warn!(
                    "Could not finish login process! (invalid address '{}' in accounts_ipauth)",
                    row.ip
                );
                return false;
            }
        }
        match row.kind() {
            Some(IpAuthKind::Allow) => allow.push(row.ip.as_str()),
            Some(IpAuthKind::Deny) => deny.push(row.ip.as_str()),
            // NOTE(java-quirk): a NULL type throws in Java (`type.equals`), aborting the login.
            None if row.kind_text.is_none() => {
                warn!("Could not finish login process! (NULL type in accounts_ipauth)");
                return false;
            }
            None => {}
        }
    }
    if !allow.is_empty() && !allow.contains(&address) {
        warn!(
            "Account checkin attemp from address({address}) not present on whitelist for account '{}'.",
            info.login
        );
        return false;
    }
    if deny.contains(&address) {
        warn!(
            "Account checkin attemp from address({address}) on blacklist for account '{}'.",
            info.login
        );
        return false;
    }
    if let Err(e) = state
        .repository
        .update_last_active(&info.login, now_millis(), address)
        .await
    {
        warn!("Could not finish login process! {e}");
        return false;
    }
    true
}

/// The account name as the rest of the login server keys it: the `login` column, lower-cased
/// (Java `AccountInfo`) and with control characters around it removed.
///
/// Deliberate deviation: Java keeps the column value as it is. A row whose name was stored with
/// trailing NUL bytes is still found by the lookup (MariaDB's collations ignore U+0000), but the
/// game server later asks for the account by the name the client typed, so with the NULs kept
/// `PlayerAuthRequest` never matches ("Session key incorrect") on the Java GS and the Rust GS.
fn account_key(login: &str) -> String {
    login.trim_matches(|c: char| c <= ' ').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_validation_like_java() {
        assert_eq!(is_valid_ip("127.0.0.1"), Some(true));
        assert_eq!(is_valid_ip("255.255.255.255"), Some(true));
        assert_eq!(is_valid_ip("256.0.0.1"), Some(false));
        assert_eq!(is_valid_ip("-1.0.0.1"), Some(false));
        assert_eq!(is_valid_ip("1.2.3"), Some(false));
        assert_eq!(is_valid_ip("1.2.3.4.5"), Some(false));
        assert_eq!(is_valid_ip("+1.2.3.4"), Some(true)); // Java parseInt accepts a leading '+'
        assert_eq!(is_valid_ip("a.b.c.d"), None);
        assert_eq!(is_valid_ip("1..3.4"), None);
    }

    #[test]
    fn account_keys_drop_nul_padding_and_case() {
        assert_eq!(account_key("Yunus"), "yunus");
        assert_eq!(account_key("abc    "), "abc");
        assert_eq!(account_key(" abc "), "abc");
    }
}
