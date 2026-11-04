//! The startup scan of used object ids, read-only.

use crate::error::Result;
use crate::pool::Pool;

// `INT UNSIGNED`, the other columns are signed `INT`.
const SELECT_CHARACTER_IDS: &str = "SELECT charId FROM characters";
const SELECT_ITEM_IDS: &str = "SELECT object_id FROM items";
const SELECT_CLAN_IDS: &str = "SELECT clan_id FROM clan_data";
const SELECT_GROUND_ITEM_IDS: &str = "SELECT object_id FROM itemsonground";
const SELECT_MESSAGE_IDS: &str = "SELECT messageId FROM messages";

/// The used object ids found by [`IdRepository::used_object_ids`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsedIds {
    /// Ids inside the requested range, sorted, without duplicates (an id may be in two tables).
    pub ids: Vec<i32>,
    /// How many rows had an id outside the range. For Java's own range these are the ids Java
    /// created; Java warns about them, we only count them.
    pub outside_range: u64,
}

/// Read-only access to the id columns of the five tables Java's IdManager scans.
#[derive(Clone)]
pub struct IdRepository {
    pool: Pool,
}

impl IdRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Reads the id column of `characters`, `items`, `clan_data`, `itemsonground` and `messages`
    /// and keeps the ids in `first..=last`.
    ///
    /// Pass the Rust range (`l2_world::RUST_FIRST_OBJECT_ID..=RUST_LAST_OBJECT_ID`) to build the
    /// allocator. Any error aborts the scan: Java logs and carries on with a table missing, but
    /// an incomplete scan could hand out an id that is already in use, so the caller must stop.
    pub async fn used_object_ids(&self, first: i32, last: i32) -> Result<UsedIds> {
        let mut ids = Vec::new();
        let mut outside_range = 0u64;
        let mut keep = |id: i64| {
            if id >= i64::from(first) && id <= i64::from(last) {
                // The range checks above bound it to `i32`.
                ids.push(id as i32);
            } else {
                outside_range += 1;
            }
        };

        // Java reads every column with `getInt`. An unsigned value above `i32::MAX` is a
        // negative int there, so it is out of range for any non-negative range either way.
        for id in sqlx::query_scalar::<_, u32>(SELECT_CHARACTER_IDS)
            .fetch_all(&self.pool)
            .await?
        {
            keep(i64::from(id));
        }
        for sql in [
            SELECT_ITEM_IDS,
            SELECT_CLAN_IDS,
            SELECT_GROUND_ITEM_IDS,
            SELECT_MESSAGE_IDS,
        ] {
            for id in sqlx::query_scalar::<_, i32>(sql).fetch_all(&self.pool).await? {
                keep(i64::from(id));
            }
        }

        ids.sort_unstable();
        ids.dedup();
        Ok(UsedIds { ids, outside_range })
    }
}
