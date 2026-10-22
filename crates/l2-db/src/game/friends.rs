//! The `character_friends` table: the friend list (`relation` 0) and the block list (`relation` 1).
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (restoreFriendList, DELETE of the friend
// Java: rows in RequestFriendDel), entity/actor/holders/player/BlockList.java (loadList, updateInDB),
// Java: network/clientpackets/RequestAnswerFriendInvite.java (the INSERT of both rows)
//
// Same layout as characters.rs: verbatim SQL, one method per statement. The two reads also join
// `characters` for the name: Java gets the name from `CharInfoTable`, which holds every character
// name, so the join is the same answer without a second query per friend.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: Player.restoreFriendList (relation 0), BlockList.loadList (relation 1). The join is ours.
const SELECT_RELATION: &str = "SELECT cf.friendId, c.char_name FROM character_friends cf LEFT JOIN characters c ON c.charId = cf.friendId WHERE cf.charId=? AND cf.relation=? AND cf.friendId<>cf.charId";
// Java: RequestAnswerFriendInvite (both rows of a new friendship, relation 0 by default)
const INSERT_FRIENDS: &str = "INSERT INTO character_friends (charId, friendId) VALUES (?, ?), (?, ?)";
// Java: RequestFriendDel (both directions, whatever the relation)
const DELETE_FRIENDS: &str =
    "DELETE FROM character_friends WHERE (charId=? AND friendId=?) OR (charId=? AND friendId=?)";
// Java: BlockList.updateInDB (add)
const INSERT_BLOCK: &str = "INSERT INTO character_friends (charId, friendId, relation) VALUES (?, ?, 1)";
// Java: BlockList.updateInDB (remove)
const DELETE_BLOCK: &str = "DELETE FROM character_friends WHERE charId=? AND friendId=? AND relation=1";

/// The relation of a friend list row (Java `Player.restoreFriendList`).
pub const RELATION_FRIEND: i32 = 0;
/// The relation of a block list row (Java `BlockList.loadList`).
pub const RELATION_BLOCK: i32 = 1;

/// One row of a friend or block list: the other character and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendLink {
    /// `friendId`.
    pub friend_id: i32,
    /// `characters.char_name`, `None` when no character has that id.
    pub name: Option<String>,
}

/// Database shape of [`FriendLink`]. `INT UNSIGNED` ids are read as `u32`.
#[derive(Debug, Clone, FromRow)]
struct FriendDbRow {
    #[sqlx(rename = "friendId")]
    friend_id: u32,
    char_name: Option<String>,
}

/// Every query on `character_friends` that the friend list and the block list need.
#[derive(Debug, Clone)]
pub struct FriendRepository {
    pool: Pool,
}

impl FriendRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The friends of a character (relation 0), in the order the rows come back. A row whose id
    /// does not fit an `i32` is skipped.
    // Java: Player.restoreFriendList
    pub async fn friends_of(&self, char_id: i32) -> Result<Vec<FriendLink>> {
        self.links_of(char_id, RELATION_FRIEND).await
    }

    /// The block list of a character (relation 1).
    // Java: BlockList.loadList
    pub async fn block_list_of(&self, char_id: i32) -> Result<Vec<FriendLink>> {
        self.links_of(char_id, RELATION_BLOCK).await
    }

    async fn links_of(&self, char_id: i32, relation: i32) -> Result<Vec<FriendLink>> {
        let rows = sqlx::query_as::<_, FriendDbRow>(SELECT_RELATION)
            .bind(char_id)
            .bind(relation)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                Some(FriendLink {
                    friend_id: i32::try_from(r.friend_id).ok()?,
                    name: r.char_name,
                })
            })
            .collect())
    }

    /// Makes two characters friends: one row each way, relation 0.
    // Java: RequestAnswerFriendInvite (INSERT INTO character_friends (charId, friendId) VALUES ...)
    pub async fn add_friends(&self, first: i32, second: i32) -> Result<()> {
        sqlx::query(INSERT_FRIENDS)
            .bind(first)
            .bind(second)
            .bind(second)
            .bind(first)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Ends a friendship: both rows, whatever their relation (Java's `RequestFriendDel`).
    // Java: RequestFriendDel
    pub async fn remove_friends(&self, first: i32, second: i32) -> Result<()> {
        sqlx::query(DELETE_FRIENDS)
            .bind(first)
            .bind(second)
            .bind(second)
            .bind(first)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Adds a row to the owner's block list.
    // Java: BlockList.updateInDB(targetId, true)
    pub async fn add_block(&self, owner: i32, target: i32) -> Result<()> {
        sqlx::query(INSERT_BLOCK)
            .bind(owner)
            .bind(target)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes a row from the owner's block list.
    // Java: BlockList.updateInDB(targetId, false)
    pub async fn remove_block(&self, owner: i32, target: i32) -> Result<()> {
        sqlx::query(DELETE_BLOCK)
            .bind(owner)
            .bind(target)
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
        assert_eq!(placeholders(SELECT_RELATION), 2);
        assert_eq!(placeholders(INSERT_FRIENDS), 4);
        assert_eq!(placeholders(DELETE_FRIENDS), 4);
        assert_eq!(placeholders(INSERT_BLOCK), 2);
        assert_eq!(placeholders(DELETE_BLOCK), 2);
    }

    #[test]
    fn relations_are_javas() {
        assert_eq!(RELATION_FRIEND, 0);
        assert_eq!(RELATION_BLOCK, 1);
    }
}
