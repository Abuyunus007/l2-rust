//! The `random_spawn` and `random_spawn_loc` tables: the groups of `AutoSpawnHandler` (the
//! Mammon NPCs, the Seven Signs preachers and crests, ...) and their places. Read only.
// Java: org/l2jmobius/gameserver/entity/spawns/AutoSpawnHandler.java (restoreSpawnData)

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: AutoSpawnHandler.restoreSpawnData
const SELECT_GROUPS: &str = "SELECT * FROM random_spawn ORDER BY groupId ASC";
const SELECT_LOCATIONS: &str = "SELECT * FROM random_spawn_loc WHERE groupId=?";

/// One `random_spawn` row with its `random_spawn_loc` rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomSpawnRow {
    pub group_id: i32,
    pub npc_id: i32,
    pub count: i32,
    pub initial_delay: i32,
    pub respawn_delay: i32,
    pub despawn_delay: i32,
    /// `Boolean.parseBoolean(rs.getString("broadcastSpawn"))`.
    pub broadcast_spawn: bool,
    /// `Boolean.parseBoolean(rs.getString("randomSpawn"))`.
    pub random_spawn: bool,
    /// `(x, y, z, heading)` in the database's order.
    pub locations: Vec<(i32, i32, i32, i32)>,
}

#[derive(Debug, FromRow)]
struct GroupDbRow {
    #[sqlx(rename = "groupId")]
    group_id: u8,
    #[sqlx(rename = "npcId")]
    npc_id: u16,
    count: u8,
    #[sqlx(rename = "initialDelay")]
    initial_delay: i32,
    #[sqlx(rename = "respawnDelay")]
    respawn_delay: i32,
    #[sqlx(rename = "despawnDelay")]
    despawn_delay: i32,
    #[sqlx(rename = "broadcastSpawn")]
    broadcast_spawn: String,
    #[sqlx(rename = "randomSpawn")]
    random_spawn: String,
}

#[derive(Debug, FromRow)]
struct LocationDbRow {
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
}

/// Every query on the auto spawn tables.
#[derive(Debug, Clone)]
pub struct RandomSpawnRepository {
    pool: Pool,
}

impl RandomSpawnRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Every group by `groupId`, each with its places.
    pub async fn load_all(&self) -> Result<Vec<RandomSpawnRow>> {
        let groups = sqlx::query_as::<_, GroupDbRow>(SELECT_GROUPS)
            .fetch_all(&self.pool)
            .await?;
        let mut out = Vec::with_capacity(groups.len());
        for g in groups {
            let locations = sqlx::query_as::<_, LocationDbRow>(SELECT_LOCATIONS)
                .bind(g.group_id)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|l| (l.x, l.y, l.z, l.heading))
                .collect();
            out.push(RandomSpawnRow {
                group_id: i32::from(g.group_id),
                npc_id: i32::from(g.npc_id),
                count: i32::from(g.count),
                initial_delay: g.initial_delay,
                respawn_delay: g.respawn_delay,
                despawn_delay: g.despawn_delay,
                broadcast_spawn: g.broadcast_spawn.eq_ignore_ascii_case("true"),
                random_spawn: g.random_spawn.eq_ignore_ascii_case("true"),
                locations,
            });
        }
        Ok(out)
    }
}
