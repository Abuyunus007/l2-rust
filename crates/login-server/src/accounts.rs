//! Account management behind the admin endpoint (G-04): search, create, password, access level,
//! ban, unban and delete.
//!
//! Java: `tools/AccountManager.java` is the spec for what each action writes; the password hash
//! is `LoginController.retriveAccountInfo`'s (SHA-1, Base64), see `l2_db::hash_password`.
//!
//! [`AccountStore`] is the database seam: [`LoginRepository`] in production, an in-memory map in
//! the tests (no test here touches a database).

use std::future::Future;

use l2_admin_api::{ACCOUNTS_PAGE_SIZE, AccountPage, AccountSummary, AccountsAction, AccountsResult};
use l2_db::{AccountListRow, DbError, LoginRepository, hash_password};
use tracing::warn;

/// `accounts.login` is `VARCHAR(45)`.
pub const MAX_LOGIN_CHARS: usize = 45;
/// The game client sends the password in a 16 byte field (`RequestAuthLogin`), so a longer one
/// could never log in.
pub const MAX_PASSWORD_BYTES: usize = 16;
/// `accounts.accessLevel` is a `TINYINT`.
pub const ACCESS_LEVEL_RANGE: std::ops::RangeInclusive<i32> = -128..=127;
/// What a ban writes.
pub const BANNED_LEVEL: i32 = -1;
/// What an unban writes: the "User" level.
pub const USER_LEVEL: i32 = 0;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AccountError {
    /// The request is wrong; the text is safe to show.
    #[error("{0}")]
    Invalid(String),
    #[error("account '{0}' already exists")]
    AlreadyExists(String),
    #[error("no account '{0}'")]
    NotFound(String),
    /// The database failed. The details go to the log, not to the client.
    #[error("database error")]
    Store,
}

impl From<DbError> for AccountError {
    fn from(e: DbError) -> Self {
        warn!("account database error: {e}");
        Self::Store
    }
}

/// The queries the Accounts view needs. `Send` futures so handlers can run on any worker.
pub trait AccountStore: Send + Sync + 'static {
    /// One page of accounts whose login starts with `prefix`, and the total match count.
    fn search(
        &self,
        prefix: &str,
        limit: u32,
        offset: u32,
    ) -> impl Future<Output = Result<(Vec<AccountSummary>, u64), AccountError>> + Send;
    fn access_level(&self, login: &str) -> impl Future<Output = Result<Option<i32>, AccountError>> + Send;
    fn last_ip(
        &self,
        login: &str,
    ) -> impl Future<Output = Result<Option<Option<String>>, AccountError>> + Send;
    /// Inserts a new row. `false` when the login exists.
    fn insert(
        &self,
        login: &str,
        password_hash: &str,
        access_level: i32,
    ) -> impl Future<Output = Result<bool, AccountError>> + Send;
    fn set_password(
        &self,
        login: &str,
        password_hash: &str,
    ) -> impl Future<Output = Result<(), AccountError>> + Send;
    fn set_access_level(
        &self,
        login: &str,
        access_level: i32,
    ) -> impl Future<Output = Result<(), AccountError>> + Send;
    /// Removes the `accounts` row only.
    fn delete(&self, login: &str) -> impl Future<Output = Result<(), AccountError>> + Send;
}

fn summary(row: AccountListRow) -> AccountSummary {
    AccountSummary {
        banned: row.access_level < 0,
        login: row.login,
        access_level: row.access_level,
        last_active_ms: i64::try_from(row.lastactive).unwrap_or(i64::MAX),
        last_server: row.last_server,
    }
}

impl AccountStore for LoginRepository {
    async fn search(
        &self,
        prefix: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<AccountSummary>, u64), AccountError> {
        let (rows, total) = self.search_accounts(prefix, limit, offset).await?;
        Ok((rows.into_iter().map(summary).collect(), total))
    }

    async fn access_level(&self, login: &str) -> Result<Option<i32>, AccountError> {
        Ok(LoginRepository::access_level(self, login).await?)
    }

    async fn last_ip(&self, login: &str) -> Result<Option<Option<String>>, AccountError> {
        Ok(LoginRepository::last_ip(self, login).await?)
    }

    async fn insert(
        &self,
        login: &str,
        password_hash: &str,
        access_level: i32,
    ) -> Result<bool, AccountError> {
        match self.insert_account(login, password_hash, access_level).await {
            Ok(()) => Ok(true),
            Err(e) if e.is_duplicate_key() => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    async fn set_password(&self, login: &str, password_hash: &str) -> Result<(), AccountError> {
        Ok(LoginRepository::set_password(self, login, password_hash).await?)
    }

    async fn set_access_level(&self, login: &str, access_level: i32) -> Result<(), AccountError> {
        Ok(self.update_access_level(login, access_level).await?)
    }

    async fn delete(&self, login: &str) -> Result<(), AccountError> {
        Ok(self.delete_account(login).await?)
    }
}

/// The login as the Java tool takes it: trimmed, not empty. Also at most 45 characters and
/// without control characters (the column size; the client could not type them).
pub fn clean_login(raw: &str) -> Result<String, AccountError> {
    let login = raw.trim();
    if login.is_empty() {
        return Err(AccountError::Invalid("the login cannot be empty".into()));
    }
    if login.chars().count() > MAX_LOGIN_CHARS {
        return Err(AccountError::Invalid(format!(
            "the login is longer than {MAX_LOGIN_CHARS} characters"
        )));
    }
    if login.chars().any(char::is_control) {
        return Err(AccountError::Invalid(
            "the login contains control characters".into(),
        ));
    }
    Ok(login.to_owned())
}

/// The password as the Java tool takes it: trimmed, not empty, and short enough for the client.
pub fn clean_password(raw: &str) -> Result<String, AccountError> {
    let password = raw.trim();
    if password.is_empty() {
        return Err(AccountError::Invalid("the password cannot be empty".into()));
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(AccountError::Invalid(format!(
            "the password is longer than {MAX_PASSWORD_BYTES} bytes (the client cannot send more)"
        )));
    }
    Ok(password.to_owned())
}

fn check_level(level: i32) -> Result<i32, AccountError> {
    if ACCESS_LEVEL_RANGE.contains(&level) {
        Ok(level)
    } else {
        Err(AccountError::Invalid(format!(
            "the access level must be between {} and {}",
            ACCESS_LEVEL_RANGE.start(),
            ACCESS_LEVEL_RANGE.end()
        )))
    }
}

/// Runs [`AccountsAction`]s against a store.
pub struct AccountsService<S> {
    store: S,
}

impl<S: AccountStore> AccountsService<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    async fn require(&self, login: &str) -> Result<i32, AccountError> {
        self.store
            .access_level(login)
            .await?
            .ok_or_else(|| AccountError::NotFound(login.to_owned()))
    }

    fn done(login: String, access_level: Option<i32>) -> AccountsResult {
        AccountsResult::Done { login, access_level }
    }

    /// Passwords are only ever hashed here; they are not logged and not returned.
    pub async fn run(&self, action: AccountsAction) -> Result<AccountsResult, AccountError> {
        match action {
            AccountsAction::Search { prefix, page } => {
                let offset = u64::from(page) * u64::from(ACCOUNTS_PAGE_SIZE);
                let offset = u32::try_from(offset)
                    .map_err(|_| AccountError::Invalid("the page number is too large".into()))?;
                let (accounts, total) = self
                    .store
                    .search(prefix.trim(), ACCOUNTS_PAGE_SIZE, offset)
                    .await?;
                Ok(AccountsResult::Page(AccountPage {
                    page,
                    page_size: ACCOUNTS_PAGE_SIZE,
                    total,
                    accounts,
                }))
            }
            AccountsAction::LastIp { login } => {
                let login = clean_login(&login)?;
                let ip = self
                    .store
                    .last_ip(&login)
                    .await?
                    .ok_or_else(|| AccountError::NotFound(login.clone()))?;
                Ok(AccountsResult::LastIp { login, ip })
            }
            AccountsAction::Create {
                login,
                password,
                access_level,
            } => {
                let login = clean_login(&login)?;
                let password = clean_password(&password)?;
                let level = check_level(access_level)?;
                // Java checks with SELECT COUNT(*) first; the insert would fail on the key anyway.
                if self.store.access_level(&login).await?.is_some()
                    || !self
                        .store
                        .insert(&login, &hash_password(&password), level)
                        .await?
                {
                    return Err(AccountError::AlreadyExists(login));
                }
                Ok(Self::done(login, Some(level)))
            }
            AccountsAction::SetPassword { login, password } => {
                let login = clean_login(&login)?;
                let password = clean_password(&password)?;
                self.require(&login).await?;
                self.store.set_password(&login, &hash_password(&password)).await?;
                Ok(Self::done(login, None))
            }
            AccountsAction::SetAccessLevel { login, access_level } => {
                let login = clean_login(&login)?;
                let level = check_level(access_level)?;
                self.require(&login).await?;
                self.store.set_access_level(&login, level).await?;
                Ok(Self::done(login, Some(level)))
            }
            AccountsAction::Ban { login } => {
                let login = clean_login(&login)?;
                self.require(&login).await?;
                self.store.set_access_level(&login, BANNED_LEVEL).await?;
                Ok(Self::done(login, Some(BANNED_LEVEL)))
            }
            AccountsAction::Unban { login } => {
                let login = clean_login(&login)?;
                // Only banned accounts: unbanning a GM must not demote it to "User".
                if self.require(&login).await? >= 0 {
                    return Err(AccountError::Invalid(format!("account '{login}' is not banned")));
                }
                self.store.set_access_level(&login, USER_LEVEL).await?;
                Ok(Self::done(login, Some(USER_LEVEL)))
            }
            AccountsAction::Delete { login } => {
                let login = clean_login(&login)?;
                self.require(&login).await?;
                // NOTE(java-quirk): AccountManager runs only `DELETE FROM accounts WHERE login = ?`.
                // Characters, `account_data` (temporary bans) and `accounts_ipauth` stay behind.
                self.store.delete(&login).await?;
                Ok(Self::done(login, None))
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod memory {
    //! The store the tests use: a map, no database.

    use std::collections::BTreeMap;
    use std::sync::{Mutex, PoisonError};

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct MemoryRow {
        pub password_hash: String,
        pub access_level: i32,
        pub last_active_ms: u64,
        pub last_server: Option<i32>,
        pub last_ip: Option<String>,
    }

    #[derive(Default)]
    pub struct MemoryAccountStore {
        rows: Mutex<BTreeMap<String, MemoryRow>>,
    }

    impl MemoryAccountStore {
        pub fn row(&self, login: &str) -> Option<MemoryRow> {
            self.rows
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(login)
                .cloned()
        }

        pub fn put(&self, login: &str, row: MemoryRow) {
            self.rows
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(login.to_owned(), row);
        }

        pub fn len(&self) -> usize {
            self.rows.lock().unwrap_or_else(PoisonError::into_inner).len()
        }
    }

    impl AccountStore for MemoryAccountStore {
        async fn search(
            &self,
            prefix: &str,
            limit: u32,
            offset: u32,
        ) -> Result<(Vec<AccountSummary>, u64), AccountError> {
            let rows = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
            // The SQL collation is case-insensitive, so the prefix match is too.
            let wanted = prefix.to_lowercase();
            let matching: Vec<_> = rows
                .iter()
                .filter(|(login, _)| login.to_lowercase().starts_with(&wanted))
                .collect();
            let total = matching.len() as u64;
            let page = matching
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .map(|(login, row)| {
                    summary(AccountListRow {
                        login: login.clone(),
                        access_level: row.access_level,
                        lastactive: row.last_active_ms,
                        last_server: row.last_server,
                    })
                })
                .collect();
            Ok((page, total))
        }

        async fn access_level(&self, login: &str) -> Result<Option<i32>, AccountError> {
            Ok(self.row(login).map(|r| r.access_level))
        }

        async fn last_ip(&self, login: &str) -> Result<Option<Option<String>>, AccountError> {
            Ok(self.row(login).map(|r| r.last_ip))
        }

        async fn insert(
            &self,
            login: &str,
            password_hash: &str,
            access_level: i32,
        ) -> Result<bool, AccountError> {
            let mut rows = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
            if rows.contains_key(login) {
                return Ok(false);
            }
            // Columns the Java insert leaves out: lastactive 0, lastServer 1 (column default).
            rows.insert(
                login.to_owned(),
                MemoryRow {
                    password_hash: password_hash.to_owned(),
                    access_level,
                    last_active_ms: 0,
                    last_server: Some(1),
                    last_ip: None,
                },
            );
            Ok(true)
        }

        async fn set_password(&self, login: &str, password_hash: &str) -> Result<(), AccountError> {
            if let Some(row) = self
                .rows
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_mut(login)
            {
                row.password_hash = password_hash.to_owned();
            }
            Ok(())
        }

        async fn set_access_level(&self, login: &str, access_level: i32) -> Result<(), AccountError> {
            if let Some(row) = self
                .rows
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get_mut(login)
            {
                row.access_level = access_level;
            }
            Ok(())
        }

        async fn delete(&self, login: &str) -> Result<(), AccountError> {
            self.rows
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(login);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::memory::{MemoryAccountStore, MemoryRow};
    use super::*;

    fn service() -> AccountsService<MemoryAccountStore> {
        AccountsService::new(MemoryAccountStore::default())
    }

    fn create(login: &str, password: &str, level: i32) -> AccountsAction {
        AccountsAction::Create {
            login: login.into(),
            password: password.into(),
            access_level: level,
        }
    }

    fn login(name: &str) -> String {
        name.to_owned()
    }

    #[tokio::test]
    async fn create_stores_the_java_hash() {
        let s = service();
        let done = s.run(create("  alice ", "password", 0)).await.unwrap();
        assert_eq!(
            done,
            AccountsResult::Done {
                login: "alice".into(),
                access_level: Some(0)
            }
        );
        let row = s.store.row("alice").unwrap();
        // SHA-1 + Base64 of "password", as `MessageDigest.getInstance("SHA")` gives (checked
        // with openssl; java-vectors has no password vector).
        assert_eq!(row.password_hash, "W6ph5Mm5Pz8GgiULbPgzG37mj9g=");
        assert_eq!(row.access_level, 0);
    }

    #[tokio::test]
    async fn create_rejects_bad_input_and_duplicates() {
        let s = service();
        for (name, pw, level) in [
            ("", "x", 0),
            ("   ", "x", 0),
            ("a", "", 0),
            ("a", "   ", 0),
            ("a", "12345678901234567", 0),
            ("a\nb", "x", 0),
            ("a", "x", 128),
            ("a", "x", -129),
        ] {
            assert!(
                matches!(
                    s.run(create(name, pw, level)).await,
                    Err(AccountError::Invalid(_))
                ),
                "{name:?} {pw:?} {level}"
            );
        }
        assert!(matches!(
            s.run(create(&"n".repeat(46), "x", 0)).await,
            Err(AccountError::Invalid(_))
        ));
        s.run(create(&"n".repeat(45), "1234567890123456", 127))
            .await
            .unwrap();
        assert!(matches!(
            s.run(create(&"n".repeat(45), "x", 0)).await,
            Err(AccountError::AlreadyExists(_))
        ));
        assert_eq!(s.store.len(), 1);
    }

    #[tokio::test]
    async fn errors_never_contain_the_password() {
        let s = service();
        let err = s
            .run(create("a", "this-password-is-far-too-long", 0))
            .await
            .unwrap_err();
        assert!(!err.to_string().contains("this-password"));
    }

    #[tokio::test]
    async fn password_change_replaces_only_the_hash() {
        let s = service();
        s.run(create("alice", "old", 30)).await.unwrap();
        s.run(AccountsAction::SetPassword {
            login: login("alice"),
            password: "password".into(),
        })
        .await
        .unwrap();
        let row = s.store.row("alice").unwrap();
        assert_eq!(row.password_hash, "W6ph5Mm5Pz8GgiULbPgzG37mj9g=");
        assert_eq!(row.access_level, 30);
        let missing = s
            .run(AccountsAction::SetPassword {
                login: login("nobody"),
                password: "x".into(),
            })
            .await;
        assert_eq!(missing, Err(AccountError::NotFound("nobody".into())));
    }

    #[tokio::test]
    async fn access_level_ban_and_unban() {
        let s = service();
        s.run(create("alice", "x", 0)).await.unwrap();
        let set = |level| AccountsAction::SetAccessLevel {
            login: login("alice"),
            access_level: level,
        };
        s.run(set(100)).await.unwrap();
        assert_eq!(s.store.row("alice").unwrap().access_level, 100);
        assert!(matches!(s.run(set(1000)).await, Err(AccountError::Invalid(_))));

        // A GM cannot be "unbanned" down to User.
        let unban = AccountsAction::Unban {
            login: login("alice"),
        };
        assert!(matches!(
            s.run(unban.clone()).await,
            Err(AccountError::Invalid(_))
        ));
        assert_eq!(s.store.row("alice").unwrap().access_level, 100);

        let banned = s
            .run(AccountsAction::Ban {
                login: login("alice"),
            })
            .await
            .unwrap();
        assert_eq!(
            banned,
            AccountsResult::Done {
                login: "alice".into(),
                access_level: Some(-1)
            }
        );
        assert_eq!(s.store.row("alice").unwrap().access_level, -1);

        s.run(unban).await.unwrap();
        assert_eq!(s.store.row("alice").unwrap().access_level, 0);

        assert_eq!(
            s.run(AccountsAction::Ban {
                login: login("nobody")
            })
            .await,
            Err(AccountError::NotFound("nobody".into()))
        );
    }

    #[tokio::test]
    async fn delete_removes_only_that_row() {
        let s = service();
        s.run(create("alice", "x", 0)).await.unwrap();
        s.run(create("bob", "x", 0)).await.unwrap();
        s.run(AccountsAction::Delete {
            login: login("alice"),
        })
        .await
        .unwrap();
        assert!(s.store.row("alice").is_none());
        assert!(s.store.row("bob").is_some());
        assert_eq!(
            s.run(AccountsAction::Delete {
                login: login("alice")
            })
            .await,
            Err(AccountError::NotFound("alice".into()))
        );
    }

    #[tokio::test]
    async fn last_ip_is_on_demand() {
        let s = service();
        s.store.put(
            "alice",
            MemoryRow {
                password_hash: "h".into(),
                access_level: 0,
                last_active_ms: 5,
                last_server: Some(2),
                last_ip: Some("10.0.0.7".into()),
            },
        );
        s.run(create("bob", "x", 0)).await.unwrap();
        let ip = |name: &str| AccountsAction::LastIp { login: name.into() };
        assert_eq!(
            s.run(ip("alice")).await.unwrap(),
            AccountsResult::LastIp {
                login: "alice".into(),
                ip: Some("10.0.0.7".into())
            }
        );
        assert_eq!(
            s.run(ip("bob")).await.unwrap(),
            AccountsResult::LastIp {
                login: "bob".into(),
                ip: None
            }
        );
        assert!(matches!(s.run(ip("zed")).await, Err(AccountError::NotFound(_))));
    }

    #[tokio::test]
    async fn search_pages_by_fifty_and_filters_by_prefix() {
        let s = service();
        for i in 0..120 {
            s.run(create(&format!("user{i:03}"), "x", 0)).await.unwrap();
        }
        s.run(create("zed", "x", -1)).await.unwrap();
        let search = |prefix: &str, page| AccountsAction::Search {
            prefix: prefix.into(),
            page,
        };
        let page = |result| match result {
            AccountsResult::Page(p) => p,
            other => panic!("{other:?}"),
        };
        let p0 = page(s.run(search("", 0)).await.unwrap());
        assert_eq!(
            (p0.page, p0.page_size, p0.total, p0.accounts.len()),
            (0, 50, 121, 50)
        );
        assert_eq!(p0.accounts[0].login, "user000");
        let p2 = page(s.run(search("", 2)).await.unwrap());
        assert_eq!(p2.accounts.len(), 21);
        assert_eq!(p2.accounts[20].login, "zed");
        assert!(p2.accounts[20].banned);
        let p3 = page(s.run(search("", 3)).await.unwrap());
        assert!(p3.accounts.is_empty());
        assert_eq!(p3.total, 121);
        let users = page(s.run(search(" USER01 ", 0)).await.unwrap());
        assert_eq!(users.total, 10);
        assert!(matches!(
            s.run(search("", u32::MAX)).await,
            Err(AccountError::Invalid(_))
        ));
    }
}
