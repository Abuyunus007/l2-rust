//! The `messages` table: the mails of the post box (header and flags; the attached items are
//! `items` rows with `loc = 'MAIL'`).
// Java: managers/MailManager.java (load, sendMessage, markAsReadInDb, markAsDeletedBySenderInDb,
// Java: markAsDeletedByReceiverInDb, removeAttachmentsInDb, deleteMessageInDb)
// Java: network/holders/MailMessage.java (the restoring constructor, getStatement)
//
// D-39: the Java game server shares the table. Every statement is Java's, verbatim, and the flags
// are written the way Java writes them: the strings `'true'` / `'false'` of the `enum` columns, and
// `sendBySystem` as the decimal text of the int.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: MailManager.load
const SELECT_ALL: &str = "SELECT * FROM messages ORDER BY expiration";
// Java: MailMessage.getStatement
const INSERT_MESSAGE: &str = "INSERT INTO messages (messageId, senderId, receiverId, subject, content, expiration, reqAdena, hasAttachments, isUnread, isDeletedBySender, isDeletedByReceiver, sendBySystem, isReturned) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
// Java: MailManager.markAsReadInDb
const MARK_READ: &str = "UPDATE messages SET isUnread = 'false' WHERE messageId = ?";
// Java: MailManager.markAsDeletedBySenderInDb
const MARK_DELETED_BY_SENDER: &str = "UPDATE messages SET isDeletedBySender = 'true' WHERE messageId = ?";
// Java: MailManager.markAsDeletedByReceiverInDb
const MARK_DELETED_BY_RECEIVER: &str =
    "UPDATE messages SET isDeletedByReceiver = 'true' WHERE messageId = ?";
// Java: MailManager.removeAttachmentsInDb
const REMOVE_ATTACHMENTS: &str = "UPDATE messages SET hasAttachments = 'false' WHERE messageId = ?";
// Java: MailManager.deleteMessageInDb
const DELETE_MESSAGE: &str = "DELETE FROM messages WHERE messageId = ?";

/// A mail as the table holds it (Java `MailMessage`'s stored fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRow {
    pub message_id: i32,
    /// The character id of the sender; -1 for a mail the system sent.
    pub sender_id: i32,
    pub receiver_id: i32,
    pub subject: String,
    pub content: String,
    /// Epoch milliseconds the mail is deleted at.
    pub expiration: i64,
    pub req_adena: i64,
    pub has_attachments: bool,
    pub is_unread: bool,
    pub is_deleted_by_sender: bool,
    pub is_deleted_by_receiver: bool,
    /// `MessageSenderType.ordinal()`.
    pub send_by_system: i32,
    pub is_returned: bool,
}

/// Database shape of [`MessageRow`]: the flags are `enum('true','false')` columns, the texts can be
/// NULL.
#[derive(Debug, Clone, FromRow)]
struct MessageDbRow {
    #[sqlx(rename = "messageId")]
    message_id: i32,
    #[sqlx(rename = "senderId")]
    sender_id: i32,
    #[sqlx(rename = "receiverId")]
    receiver_id: i32,
    subject: Option<String>,
    content: Option<String>,
    expiration: u64,
    #[sqlx(rename = "reqAdena")]
    req_adena: i64,
    #[sqlx(rename = "hasAttachments")]
    has_attachments: String,
    #[sqlx(rename = "isUnread")]
    is_unread: String,
    #[sqlx(rename = "isDeletedBySender")]
    is_deleted_by_sender: String,
    #[sqlx(rename = "isDeletedByReceiver")]
    is_deleted_by_receiver: String,
    #[sqlx(rename = "sendBySystem")]
    send_by_system: i8,
    #[sqlx(rename = "isReturned")]
    is_returned: String,
}

/// Java `Boolean.parseBoolean(String)`: true for "true" in any case, false for everything else.
fn parse_boolean(text: &str) -> bool {
    text.eq_ignore_ascii_case("true")
}

/// Java `String.valueOf(boolean)`.
fn boolean_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

impl From<MessageDbRow> for MessageRow {
    fn from(r: MessageDbRow) -> Self {
        Self {
            message_id: r.message_id,
            sender_id: r.sender_id,
            receiver_id: r.receiver_id,
            // `getString` of a NULL column is null; the Rust side keeps an empty text.
            subject: r.subject.unwrap_or_default(),
            content: r.content.unwrap_or_default(),
            // Java `getLong`: the column is unsigned, a value above `i64::MAX` cannot happen.
            expiration: i64::try_from(r.expiration).unwrap_or(i64::MAX),
            req_adena: r.req_adena,
            has_attachments: parse_boolean(&r.has_attachments),
            is_unread: parse_boolean(&r.is_unread),
            is_deleted_by_sender: parse_boolean(&r.is_deleted_by_sender),
            is_deleted_by_receiver: parse_boolean(&r.is_deleted_by_receiver),
            send_by_system: i32::from(r.send_by_system),
            is_returned: parse_boolean(&r.is_returned),
        }
    }
}

/// Reads and writes `messages`.
#[derive(Clone)]
pub struct MessageRepository {
    pool: Pool,
}

impl MessageRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every mail, by expiration (Java `MailManager.load`).
    pub async fn load_all(&self) -> Result<Vec<MessageRow>> {
        let rows: Vec<MessageDbRow> = sqlx::query_as(SELECT_ALL).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(MessageRow::from).collect())
    }

    /// A new mail (Java `MailManager.sendMessage`).
    pub async fn insert(&self, message: &MessageRow) -> Result<()> {
        sqlx::query(INSERT_MESSAGE)
            .bind(message.message_id)
            .bind(message.sender_id)
            .bind(message.receiver_id)
            .bind(&message.subject)
            .bind(&message.content)
            .bind(message.expiration)
            .bind(message.req_adena)
            .bind(boolean_text(message.has_attachments))
            .bind(boolean_text(message.is_unread))
            .bind(boolean_text(message.is_deleted_by_sender))
            .bind(boolean_text(message.is_deleted_by_receiver))
            // Java `stmt.setString(12, String.valueOf(msg._sendBySystem))`.
            .bind(message.send_by_system.to_string())
            .bind(boolean_text(message.is_returned))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `markAsReadInDb`.
    pub async fn mark_read(&self, message_id: i32) -> Result<()> {
        sqlx::query(MARK_READ).bind(message_id).execute(&self.pool).await?;
        Ok(())
    }

    /// Java `markAsDeletedBySenderInDb`.
    pub async fn mark_deleted_by_sender(&self, message_id: i32) -> Result<()> {
        sqlx::query(MARK_DELETED_BY_SENDER)
            .bind(message_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `markAsDeletedByReceiverInDb`.
    pub async fn mark_deleted_by_receiver(&self, message_id: i32) -> Result<()> {
        sqlx::query(MARK_DELETED_BY_RECEIVER)
            .bind(message_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `removeAttachmentsInDb`.
    pub async fn remove_attachments(&self, message_id: i32) -> Result<()> {
        sqlx::query(REMOVE_ATTACHMENTS)
            .bind(message_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Java `deleteMessageInDb` (the database half).
    pub async fn delete(&self, message_id: i32) -> Result<()> {
        sqlx::query(DELETE_MESSAGE).bind(message_id).execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(INSERT_MESSAGE.matches('?').count(), 13);
        for sql in [
            MARK_READ,
            MARK_DELETED_BY_SENDER,
            MARK_DELETED_BY_RECEIVER,
            REMOVE_ATTACHMENTS,
            DELETE_MESSAGE,
        ] {
            assert_eq!(sql.matches('?').count(), 1, "{sql}");
        }
        assert_eq!(SELECT_ALL.matches('?').count(), 0);
    }

    #[test]
    fn flags_are_read_and_written_like_java() {
        // `Boolean.parseBoolean` ignores the case and takes everything else as false.
        assert!(parse_boolean("true"));
        assert!(parse_boolean("TRUE"));
        assert!(!parse_boolean("false"));
        assert!(!parse_boolean(""));
        assert!(!parse_boolean("1"));
        assert_eq!(boolean_text(true), "true");
        assert_eq!(boolean_text(false), "false");
    }
}
