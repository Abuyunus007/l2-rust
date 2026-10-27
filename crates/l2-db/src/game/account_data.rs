//! The `account_gsdata` table: per account values of the game server. The secondary password
//! (`secauth_pwd`) and its wrong attempts (`secauth_wte`) live here.
// Java: org/l2jmobius/gameserver/security/SecondaryPasswordAuth.java (SELECT_PASSWORD,
// Java: INSERT_PASSWORD, UPDATE_PASSWORD, INSERT_ATTEMPT)

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha1::{Digest, Sha1};
use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

/// Java `SecondaryPasswordAuth.VAR_PWD`.
pub const VAR_PWD: &str = "secauth_pwd";
/// Java `SecondaryPasswordAuth.VAR_WTE`.
pub const VAR_WTE: &str = "secauth_wte";

// Java: SecondaryPasswordAuth.SELECT_PASSWORD
const SELECT_PASSWORD: &str =
    "SELECT var, value FROM account_gsdata WHERE account_name=? AND var LIKE 'secauth_%'";
// Java: SecondaryPasswordAuth.INSERT_PASSWORD
const INSERT_PASSWORD: &str = "INSERT INTO account_gsdata VALUES (?, ?, ?)";
// Java: SecondaryPasswordAuth.UPDATE_PASSWORD
const UPDATE_PASSWORD: &str = "UPDATE account_gsdata SET value=? WHERE account_name=? AND var=?";
// Java: SecondaryPasswordAuth.INSERT_ATTEMPT
const INSERT_ATTEMPT: &str = "INSERT INTO account_gsdata VALUES (?, ?, ?) ON DUPLICATE KEY UPDATE value=?";

/// `SHA-1` of a byte string. Java `MessageDigest.getInstance("SHA")`; the game server has no
/// hash crate of its own (`GameGuardReply` needs this one).
pub fn sha1_digest(data: &[u8]) -> [u8; 20] {
    Sha1::digest(data).into()
}

/// The stored form of a secondary password: `base64(sha1(utf8(password)))`.
// Java: SecondaryPasswordAuth.cryptPassword
pub fn crypt_secondary_password(password: &str) -> String {
    STANDARD.encode(Sha1::digest(password.as_bytes()))
}

/// One `secauth_*` row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct AccountDataRow {
    pub var: String,
    pub value: String,
}

/// The queries on `account_gsdata` that the secondary password needs.
#[derive(Debug, Clone)]
pub struct AccountDataRepository {
    pool: Pool,
}

impl AccountDataRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The `secauth_*` rows of an account, in the order the rows come back.
    // Java: SecondaryPasswordAuth.loadPassword
    pub async fn secondary_auth_of(&self, account: &str) -> Result<Vec<AccountDataRow>> {
        let rows = sqlx::query_as::<_, AccountDataRow>(SELECT_PASSWORD)
            .bind(account)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// The first password of an account (`secauth_pwd`, the already hashed value).
    // Java: SecondaryPasswordAuth.savePassword
    pub async fn insert_password(&self, account: &str, hashed: &str) -> Result<()> {
        sqlx::query(INSERT_PASSWORD)
            .bind(account)
            .bind(VAR_PWD)
            .bind(hashed)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A new password for an account that has one.
    // Java: SecondaryPasswordAuth.changePassword
    pub async fn update_password(&self, account: &str, hashed: &str) -> Result<()> {
        sqlx::query(UPDATE_PASSWORD)
            .bind(hashed)
            .bind(account)
            .bind(VAR_PWD)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The wrong attempts in a row (`secauth_wte`).
    // Java: SecondaryPasswordAuth.insertWrongAttempt
    pub async fn insert_wrong_attempt(&self, account: &str, attempts: i32) -> Result<()> {
        let value = attempts.to_string();
        sqlx::query(INSERT_ATTEMPT)
            .bind(account)
            .bind(VAR_WTE)
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

    fn placeholders(sql: &str) -> usize {
        sql.matches('?').count()
    }

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(placeholders(SELECT_PASSWORD), 1);
        assert_eq!(placeholders(INSERT_PASSWORD), 3);
        assert_eq!(placeholders(UPDATE_PASSWORD), 3);
        assert_eq!(placeholders(INSERT_ATTEMPT), 4);
    }

    #[test]
    fn the_password_is_base64_sha1() {
        // `echo -n 123457 | openssl sha1 -binary | base64`
        assert_eq!(crypt_secondary_password("123457"), "kI9wTMqt/YanRAfSNMe94w8nRP4=");
        assert_eq!(
            sha1_digest(b"abc"),
            [
                0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50, 0xc2,
                0x6c, 0x9c, 0xd0, 0xd8, 0x9d
            ]
        );
    }
}
