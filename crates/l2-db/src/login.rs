//! Login server tables: `accounts`, `account_data`, `accounts_ipauth`, `gameservers`.
// Java: org/l2jmobius/loginserver/controller/LoginController.java (SQL constants),
// Java: org/l2jmobius/loginserver/data/GameServerTable.java,
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/{ChangePassword,RequestTempBan}.java
//
// Repository pattern: one method per Java statement, the SQL text copied verbatim into a
// constant. Methods only talk to the database. Decisions (is this IP allowed, is the
// password right, what to tell the game server) belong to the login flow code.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use num_bigint_dig::BigInt;
use sha1::{Digest, Sha1};
use sqlx::FromRow;

use crate::error::{DbError, Result};
use crate::pool::Pool;

// Java: LoginController.USER_INFO_SELECT
const USER_INFO_SELECT: &str = "SELECT login, password, IF(? > value OR value IS NULL, accessLevel, -1) AS accessLevel, lastServer FROM accounts LEFT JOIN (account_data) ON (account_data.account_name=accounts.login AND account_data.var=\"ban_temp\") WHERE login=?";
// Java: LoginController.AUTOCREATE_ACCOUNTS_INSERT
const AUTOCREATE_ACCOUNTS_INSERT: &str =
    "INSERT INTO accounts (login, password, lastactive, accessLevel, lastIP) values (?, ?, ?, ?, ?)";
// Java: LoginController.ACCOUNT_INFO_UPDATE
const ACCOUNT_INFO_UPDATE: &str = "UPDATE accounts SET lastactive = ?, lastIP = ? WHERE login = ?";
// Java: LoginController.ACCOUNT_LAST_SERVER_UPDATE
const ACCOUNT_LAST_SERVER_UPDATE: &str = "UPDATE accounts SET lastServer = ? WHERE login = ?";
// Java: LoginController.ACCOUNT_ACCESS_LEVEL_UPDATE
const ACCOUNT_ACCESS_LEVEL_UPDATE: &str = "UPDATE accounts SET accessLevel = ? WHERE login = ?";
// Java: LoginController.ACCOUNT_IPS_UPDATE
const ACCOUNT_IPS_UPDATE: &str =
    "UPDATE accounts SET pcIp = ?, hop1 = ?, hop2 = ?, hop3 = ?, hop4 = ? WHERE login = ?";
// Java: LoginController.ACCOUNT_IPAUTH_SELECT
const ACCOUNT_IPAUTH_SELECT: &str = "SELECT * FROM accounts_ipauth WHERE login = ?";
// Java: GameServerTable.loadRegisteredGameServers
const GAMESERVERS_SELECT: &str = "SELECT * FROM gameservers";
// Java: GameServerTable.registerServerOnDB
const GAMESERVERS_INSERT: &str = "INSERT INTO gameservers (hexid,server_id,host) values (?,?,?)";
// Java: ChangePassword.SELECT_PASSWORD_SQL
const SELECT_PASSWORD_SQL: &str = "SELECT password FROM accounts WHERE login=?";
// Java: ChangePassword.UPDATE_PASSWORD_MATCHING_SQL
const UPDATE_PASSWORD_MATCHING_SQL: &str = "UPDATE accounts SET password=? WHERE login=? AND password=?";
// Java: RequestTempBan.banUser
const TEMP_BAN_UPSERT: &str = "INSERT INTO account_data VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE value=?";
// Java: tools/AccountManager.searchAccount (the GUI tool lists with LIKE; paging from loadAccounts)
const ACCOUNTS_SEARCH: &str = "SELECT login, accessLevel, lastactive, lastServer FROM accounts WHERE login LIKE ? ORDER BY login LIMIT ? OFFSET ?";
// Java: tools/AccountManager.getTotalAccountCount, narrowed by the same LIKE
const ACCOUNTS_COUNT: &str = "SELECT COUNT(*) FROM accounts WHERE login LIKE ?";
// Java: tools/AccountManager.createAccount (existence check)
const ACCOUNT_EXISTS: &str = "SELECT COUNT(*) FROM accounts WHERE login = ?";
// Java: tools/AccountManager.createAccount
const ACCOUNT_INSERT: &str = "INSERT INTO accounts (login, password, accessLevel) VALUES (?, ?, ?)";
// Java: tools/AccountManager.updateAccount (the password part of its dynamic UPDATE)
const ACCOUNT_PASSWORD_UPDATE: &str = "UPDATE accounts SET password = ? WHERE login = ?";
// Java: tools/AccountManager.showAccountInfo (accessLevel)
const ACCOUNT_ACCESS_LEVEL_SELECT: &str = "SELECT accessLevel FROM accounts WHERE login = ?";
// Java: tools/AccountManager.showAccountInfo (lastIP)
const ACCOUNT_LAST_IP_SELECT: &str = "SELECT lastIP FROM accounts WHERE login = ?";
// Java: tools/AccountManager.deleteAccount
const ACCOUNT_DELETE: &str = "DELETE FROM accounts WHERE login = ?";
/// `account_data.var` of a temporary ban.
const BAN_TEMP_VAR: &str = "ban_temp";

/// `base64(sha1(utf8(password)))`, the format of `accounts.password`.
// Java: LoginController.retriveAccountInfo, ChangePassword (MessageDigest "SHA" + Base64 encoder)
pub fn hash_password(password: &str) -> String {
    STANDARD.encode(Sha1::digest(password.as_bytes()))
}

/// Server hex id as stored in `gameservers.hexid`: the signed big-endian bytes as a
/// `BigInteger` hex string (lowercase, `-` prefix when negative, no leading zeros).
// Java: GameServerTable.hexToString
pub fn hex_to_string(hex: &[u8]) -> Result<String> {
    // NOTE(java-quirk): Java's `new BigInteger(byte[])` throws on an empty array.
    if hex.is_empty() {
        return Err(DbError::InvalidHexId(String::new()));
    }
    Ok(BigInt::from_signed_bytes_be(hex).to_str_radix(16))
}

/// Inverse of [`hex_to_string`]: the minimal signed big-endian bytes (`BigInteger.toByteArray`).
// Java: GameServerTable.stringToHex
pub fn string_to_hex(text: &str) -> Result<Vec<u8>> {
    let invalid = || DbError::InvalidHexId(text.to_owned());
    // BigInteger(String, 16) accepts one optional sign followed by at least one hex digit.
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    let value = match text.strip_prefix('+') {
        Some(rest) => BigInt::parse_bytes(rest.as_bytes(), 16),
        None => BigInt::parse_bytes(text.as_bytes(), 16),
    };
    Ok(value.ok_or_else(invalid)?.to_signed_bytes_be())
}

/// Result of [`LoginRepository::account_info`].
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct AccountInfoRow {
    /// `accounts.login`.
    pub login: String,
    /// `accounts.password`: base64 SHA-1, NULL for accounts without a password.
    pub password: Option<String>,
    /// `accounts.accessLevel`, or -1 while a temporary ban is active.
    #[sqlx(rename = "accessLevel")]
    pub access_level: i32,
    /// `accounts.lastServer` (TINYINT, nullable).
    #[sqlx(rename = "lastServer")]
    pub last_server: Option<i32>,
}

impl AccountInfoRow {
    /// Java reads the column with `getInt`, which turns NULL into 0.
    pub fn last_server_or_zero(&self) -> i32 {
        self.last_server.unwrap_or(0)
    }
}

/// A row of the account list ([`LoginRepository::search_accounts`]).
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct AccountListRow {
    /// `accounts.login`.
    pub login: String,
    /// `accounts.accessLevel` (TINYINT; negative = banned).
    #[sqlx(rename = "accessLevel")]
    pub access_level: i32,
    /// `accounts.lastactive`: Unix time in milliseconds, 0 = never.
    pub lastactive: u64,
    /// `accounts.lastServer` (TINYINT, nullable).
    #[sqlx(rename = "lastServer")]
    pub last_server: Option<i32>,
}

/// Escapes `\`, `%` and `_` so `text` matches literally inside a `LIKE` pattern.
pub fn escape_like(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The five address columns written by [`LoginRepository::update_tracert`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TracertHops {
    /// `accounts.pcIp`.
    pub pc_ip: Option<String>,
    /// `accounts.hop1` .. `accounts.hop4`.
    pub hops: [Option<String>; 4],
}

/// `accounts_ipauth.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpAuthKind {
    /// `allow`: whitelist entry.
    Allow,
    /// `deny`: blacklist entry.
    Deny,
}

/// A row of `accounts_ipauth`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct IpAuthRow {
    /// `accounts_ipauth.login`.
    pub login: String,
    /// `accounts_ipauth.ip`: text, may not be a valid IPv4 address (the login flow checks).
    pub ip: String,
    /// `accounts_ipauth.type` as text (enum column, nullable, default `allow`).
    #[sqlx(rename = "type")]
    pub kind_text: Option<String>,
}

impl IpAuthRow {
    /// The type as an enum; `None` for NULL (Java would throw there) or an unknown value.
    pub fn kind(&self) -> Option<IpAuthKind> {
        match self.kind_text.as_deref() {
            Some("allow") => Some(IpAuthKind::Allow),
            Some("deny") => Some(IpAuthKind::Deny),
            _ => None,
        }
    }
}

/// A row of `gameservers`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct GameServerRow {
    /// `server_id`.
    pub server_id: i32,
    /// `hexid` in `BigInteger` hex notation; decode with [`string_to_hex`].
    pub hexid: String,
    /// `host`: the external host the server registered with, may be empty.
    pub host: String,
}

/// Outcome of [`LoginRepository::change_password`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangePasswordResult {
    /// The current hash matched and the new one is stored.
    Changed,
    /// No row was updated although the stored hash equals the current one.
    UpdateFailed,
    /// The current password is wrong (or the account does not exist).
    CurrentMismatch,
}

/// Every query the Java login server runs.
#[derive(Debug, Clone)]
pub struct LoginRepository {
    pool: Pool,
}

impl LoginRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The account with its effective access level, or `None` if the login does not exist.
    ///
    /// `now_ms` is the current time in milliseconds. While `account_data.ban_temp` holds a
    /// later time, `access_level` comes back as -1.
    // Java: LoginController.USER_INFO_SELECT
    pub async fn account_info(&self, login: &str, now_ms: i64) -> Result<Option<AccountInfoRow>> {
        // NOTE(java-quirk): Java binds the time as a string (`Long.toString`) and
        // `account_data.value` is a VARCHAR, so the SQL compares text, not numbers. It is only
        // right while both have the same number of digits (true for millisecond times until
        // the year 2286). Kept as is.
        let row = sqlx::query_as::<_, AccountInfoRow>(USER_INFO_SELECT)
            .bind(now_ms.to_string())
            .bind(login)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Inserts a new account (auto-create). `password_hash` comes from [`hash_password`].
    // Java: LoginController.AUTOCREATE_ACCOUNTS_INSERT
    pub async fn create_account(
        &self,
        login: &str,
        password_hash: &str,
        now_ms: i64,
        access_level: i32,
        ip: &str,
    ) -> Result<()> {
        sqlx::query(AUTOCREATE_ACCOUNTS_INSERT)
            .bind(login)
            .bind(password_hash)
            .bind(now_ms)
            .bind(access_level)
            .bind(ip)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Sets `lastactive` and `lastIP` after a successful login.
    // Java: LoginController.ACCOUNT_INFO_UPDATE
    pub async fn update_last_active(&self, login: &str, now_ms: i64, ip: &str) -> Result<()> {
        sqlx::query(ACCOUNT_INFO_UPDATE)
            .bind(now_ms)
            .bind(ip)
            .bind(login)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Remembers the last game server the account logged into.
    // Java: LoginController.ACCOUNT_LAST_SERVER_UPDATE
    pub async fn update_last_server(&self, login: &str, server_id: i32) -> Result<()> {
        sqlx::query(ACCOUNT_LAST_SERVER_UPDATE)
            .bind(server_id)
            .bind(login)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Sets `accessLevel` (negative = banned).
    // Java: LoginController.ACCOUNT_ACCESS_LEVEL_UPDATE
    pub async fn update_access_level(&self, login: &str, access_level: i32) -> Result<()> {
        sqlx::query(ACCOUNT_ACCESS_LEVEL_UPDATE)
            .bind(access_level)
            .bind(login)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Stores the client's traceroute addresses.
    // Java: LoginController.ACCOUNT_IPS_UPDATE
    pub async fn update_tracert(&self, login: &str, hops: &TracertHops) -> Result<()> {
        sqlx::query(ACCOUNT_IPS_UPDATE)
            .bind(hops.pc_ip.as_deref())
            .bind(hops.hops[0].as_deref())
            .bind(hops.hops[1].as_deref())
            .bind(hops.hops[2].as_deref())
            .bind(hops.hops[3].as_deref())
            .bind(login)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The IP allow/deny entries of an account.
    // Java: LoginController.ACCOUNT_IPAUTH_SELECT
    pub async fn ip_auth(&self, login: &str) -> Result<Vec<IpAuthRow>> {
        let rows = sqlx::query_as::<_, IpAuthRow>(ACCOUNT_IPAUTH_SELECT)
            .bind(login)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// All registered game servers.
    // Java: GameServerTable.loadRegisteredGameServers
    pub async fn game_servers(&self) -> Result<Vec<GameServerRow>> {
        let rows = sqlx::query_as::<_, GameServerRow>(GAMESERVERS_SELECT)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Registers a game server. `hexid` is the text form (see [`hex_to_string`]). Fails when
    /// the id is already registered (primary key), like the Java insert.
    // Java: GameServerTable.registerServerOnDB
    pub async fn register_game_server(&self, server_id: i32, hexid: &str, host: &str) -> Result<()> {
        sqlx::query(GAMESERVERS_INSERT)
            .bind(hexid)
            .bind(server_id)
            .bind(host)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Changes the password if `current_hash` matches. Both hashes come from [`hash_password`].
    // Java: ChangePassword (UPDATE_PASSWORD_MATCHING_SQL, then SELECT_PASSWORD_SQL on 0 rows)
    pub async fn change_password(
        &self,
        login: &str,
        current_hash: &str,
        new_hash: &str,
    ) -> Result<ChangePasswordResult> {
        // The connection reports matched rows (CLIENT_FOUND_ROWS), like Connector/J's default,
        // so setting the same password again still counts as changed.
        let updated = sqlx::query(UPDATE_PASSWORD_MATCHING_SQL)
            .bind(new_hash)
            .bind(login)
            .bind(current_hash)
            .execute(&self.pool)
            .await?
            .rows_affected();
        if updated > 0 {
            return Ok(ChangePasswordResult::Changed);
        }

        let stored: Option<Option<String>> = sqlx::query_scalar(SELECT_PASSWORD_SQL)
            .bind(login)
            .fetch_optional(&self.pool)
            .await?;
        Ok(if stored.flatten().as_deref() == Some(current_hash) {
            ChangePasswordResult::UpdateFailed
        } else {
            ChangePasswordResult::CurrentMismatch
        })
    }

    /// One page of accounts whose login starts with `prefix` (literal text, not a pattern),
    /// ordered by login, and the number of matching accounts in total.
    // Java: tools/AccountManager.searchAccount / loadAccounts
    pub async fn search_accounts(
        &self,
        prefix: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<AccountListRow>, u64)> {
        let pattern = format!("{}%", escape_like(prefix));
        let rows = sqlx::query_as::<_, AccountListRow>(ACCOUNTS_SEARCH)
            .bind(&pattern)
            .bind(i64::from(limit))
            .bind(i64::from(offset))
            .fetch_all(&self.pool)
            .await?;
        let total: i64 = sqlx::query_scalar(ACCOUNTS_COUNT)
            .bind(&pattern)
            .fetch_one(&self.pool)
            .await?;
        Ok((rows, u64::try_from(total).unwrap_or(0)))
    }

    /// Whether an account with exactly this login exists.
    // Java: tools/AccountManager.createAccount (SELECT COUNT(*))
    pub async fn account_exists(&self, login: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(ACCOUNT_EXISTS)
            .bind(login)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    /// Inserts an account the way the AccountManager tool does (other columns keep their
    /// defaults). Fails on an existing login (primary key).
    // Java: tools/AccountManager.createAccount
    pub async fn insert_account(&self, login: &str, password_hash: &str, access_level: i32) -> Result<()> {
        sqlx::query(ACCOUNT_INSERT)
            .bind(login)
            .bind(password_hash)
            .bind(access_level)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Sets the password hash without asking for the old one (admin tool).
    // Java: tools/AccountManager.updateAccount
    pub async fn set_password(&self, login: &str, password_hash: &str) -> Result<()> {
        sqlx::query(ACCOUNT_PASSWORD_UPDATE)
            .bind(password_hash)
            .bind(login)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `accounts.accessLevel` as stored (a temporary ban is not applied), `None` = no such account.
    // Java: tools/AccountManager.showAccountInfo
    pub async fn access_level(&self, login: &str) -> Result<Option<i32>> {
        let level: Option<i32> = sqlx::query_scalar(ACCOUNT_ACCESS_LEVEL_SELECT)
            .bind(login)
            .fetch_optional(&self.pool)
            .await?;
        Ok(level)
    }

    /// `accounts.lastIP`: outer `None` = no such account, inner `None` = never logged in.
    // Java: tools/AccountManager.showAccountInfo
    pub async fn last_ip(&self, login: &str) -> Result<Option<Option<String>>> {
        let ip: Option<Option<String>> = sqlx::query_scalar(ACCOUNT_LAST_IP_SELECT)
            .bind(login)
            .fetch_optional(&self.pool)
            .await?;
        Ok(ip)
    }

    /// Deletes the account row. Like the Java tool it touches nothing else: the characters and
    /// the `account_data` rows stay behind.
    // Java: tools/AccountManager.deleteAccount
    pub async fn delete_account(&self, login: &str) -> Result<()> {
        sqlx::query(ACCOUNT_DELETE)
            .bind(login)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Bans an account until `ban_time_ms` (`account_data.ban_temp`), replacing an older ban.
    // Java: RequestTempBan.banUser
    pub async fn temp_ban(&self, login: &str, ban_time_ms: i64) -> Result<()> {
        let value = ban_time_ms.to_string();
        sqlx::query(TEMP_BAN_UPSERT)
            .bind(login)
            .bind(BAN_TEMP_VAR)
            .bind(&value)
            .bind(&value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hand-verified with `echo -n <pw> | openssl sha1 -binary | base64`.
    #[test]
    fn hash_password_known_values() {
        assert_eq!(hash_password("password"), "W6ph5Mm5Pz8GgiULbPgzG37mj9g=");
        assert_eq!(hash_password(""), "2jmj7l5rSw0yVb/vlWAYkK/YBwk=");
        assert_eq!(hash_password("abc"), "qZk+NkcGgWq6PiVxeFDCbJzQ2J0=");
    }

    #[test]
    fn like_patterns_are_escaped() {
        assert_eq!(escape_like("a_b%c\\d"), "a\\_b\\%c\\\\d");
        assert_eq!(escape_like("plain"), "plain");
    }

    #[test]
    fn hash_password_hashes_utf8_bytes() {
        assert_eq!(
            hash_password("p\u{e4}ssw\u{f6}rd"),
            "9Rfd8dMqES/xrVXGbRsSyzjn6Pc="
        );
    }

    #[test]
    fn hexid_matches_biginteger_to_byte_array() {
        // BigInteger.toByteArray() is minimal two's complement: positive values whose top bit
        // is set get a leading 0x00.
        assert_eq!(string_to_hex("ff").unwrap(), [0x00, 0xff]);
        assert_eq!(string_to_hex("7f").unwrap(), [0x7f]);
        assert_eq!(string_to_hex("80").unwrap(), [0x00, 0x80]);
        assert_eq!(string_to_hex("-80").unwrap(), [0x80]);
        assert_eq!(string_to_hex("-81").unwrap(), [0xff, 0x7f]);
        assert_eq!(string_to_hex("-1").unwrap(), [0xff]);
        assert_eq!(string_to_hex("0").unwrap(), [0x00]);
        assert_eq!(string_to_hex("+1A").unwrap(), [0x1a]);
    }

    #[test]
    fn hexid_to_string_is_signed_lowercase() {
        assert_eq!(hex_to_string(&[0x00, 0xff]).unwrap(), "ff");
        assert_eq!(hex_to_string(&[0x80]).unwrap(), "-80");
        assert_eq!(hex_to_string(&[0xff]).unwrap(), "-1");
        assert_eq!(hex_to_string(&[0x00, 0x00, 0x01]).unwrap(), "1");
        assert_eq!(hex_to_string(&[0x00]).unwrap(), "0");
    }

    #[test]
    fn hexid_round_trips_the_shipped_default() {
        // The row `gameservers.sql` ships with.
        let text = "-2ad66b3f483c22be097019f55c8abdf0";
        let bytes = string_to_hex(text).unwrap();
        assert_eq!(bytes.len(), 16);
        assert_eq!(bytes[0] & 0x80, 0x80, "negative, so the top bit is set");
        assert_eq!(hex_to_string(&bytes).unwrap(), text);
    }

    #[test]
    fn hexid_rejects_what_java_rejects() {
        for bad in ["", "-", "+", "xyz", "12 3", "--1", "0x10", "1-"] {
            assert!(
                matches!(string_to_hex(bad), Err(DbError::InvalidHexId(_))),
                "{bad:?}"
            );
        }
        assert!(matches!(hex_to_string(&[]), Err(DbError::InvalidHexId(_))));
    }

    #[test]
    fn ip_auth_kind_parses_enum_text() {
        let row = |t: Option<&str>| IpAuthRow {
            login: "a".into(),
            ip: "1.2.3.4".into(),
            kind_text: t.map(str::to_owned),
        };
        assert_eq!(row(Some("allow")).kind(), Some(IpAuthKind::Allow));
        assert_eq!(row(Some("deny")).kind(), Some(IpAuthKind::Deny));
        assert_eq!(row(None).kind(), None);
        assert_eq!(row(Some("other")).kind(), None);
    }
}
