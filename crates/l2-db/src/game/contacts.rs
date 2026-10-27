//! The `character_contacts` table: the mail contact list.
// Java: org/l2jmobius/gameserver/entity/actor/holders/player/ContactList.java (QUERY_ADD,
// Java: QUERY_REMOVE, QUERY_LOAD)
//
// The read also joins `characters` for the name: Java gets it from `CharInfoTable`, which holds
// every character name (same reasoning as friends.rs).

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: ContactList.QUERY_LOAD. The join is ours.
const SELECT_CONTACTS: &str = "SELECT cc.contactId, c.char_name FROM character_contacts cc LEFT JOIN characters c ON c.charId = cc.contactId WHERE cc.charId=?";
// Java: ContactList.QUERY_ADD
const REPLACE_CONTACT: &str = "REPLACE INTO character_contacts (charId, contactId) VALUES (?, ?)";
// Java: ContactList.QUERY_REMOVE
const DELETE_CONTACT: &str = "DELETE FROM character_contacts WHERE charId = ? and contactId = ?";

/// One row of a contact list: the contact and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactLink {
    /// `contactId`.
    pub contact_id: i32,
    /// `characters.char_name`, `None` when no character has that id.
    pub name: Option<String>,
}

/// Database shape of [`ContactLink`]. `INT UNSIGNED` ids are read as `u32`.
#[derive(Debug, Clone, FromRow)]
struct ContactDbRow {
    #[sqlx(rename = "contactId")]
    contact_id: u32,
    char_name: Option<String>,
}

/// The queries on `character_contacts`.
#[derive(Debug, Clone)]
pub struct ContactRepository {
    pool: Pool,
}

impl ContactRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The contacts of a character, in the order the rows come back. A row whose id does not fit
    /// an `i32` is skipped.
    // Java: ContactList.restore
    pub async fn contacts_of(&self, char_id: i32) -> Result<Vec<ContactLink>> {
        let rows = sqlx::query_as::<_, ContactDbRow>(SELECT_CONTACTS)
            .bind(char_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                Some(ContactLink {
                    contact_id: i32::try_from(r.contact_id).ok()?,
                    name: r.char_name,
                })
            })
            .collect())
    }

    /// A contact added (`REPLACE`: adding one that exists is not an error).
    // Java: ContactList.add
    pub async fn add_contact(&self, char_id: i32, contact_id: i32) -> Result<()> {
        sqlx::query(REPLACE_CONTACT)
            .bind(char_id)
            .bind(contact_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A contact removed.
    // Java: ContactList.remove
    pub async fn remove_contact(&self, char_id: i32, contact_id: i32) -> Result<()> {
        sqlx::query(DELETE_CONTACT)
            .bind(char_id)
            .bind(contact_id)
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
        assert_eq!(placeholders(SELECT_CONTACTS), 1);
        assert_eq!(placeholders(REPLACE_CONTACT), 2);
        assert_eq!(placeholders(DELETE_CONTACT), 2);
    }
}
