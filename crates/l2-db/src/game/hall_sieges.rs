//! The tables of the conquerable clan hall sieges: `clanhall_siege_attackers`,
//! `clanhall_siege_guards`, `siegable_hall_flagwar_attackers(_members)` (Bandit Stronghold and
//! Wild Beast Reserve) and `rainbowsprings_attacker_list` (Rainbow Springs Chateau) (M7-CHSIEGE-01).
// Java: org/l2jmobius/gameserver/mechanics/siege/clanhalls/ClanHallSiegeEngine.java
//       (loadAttackers, saveAttackers, loadGuards)
// Java: dist/game/data/scripts/conquerablehalls/BanditStronghold/BanditStronghold.java
//       and WildBeastReserve.java (loadAttackers, loadAttackerMembers, saveClan, saveNpc,
//       saveMember, clearTables)
// Java: dist/game/data/scripts/conquerablehalls/RainbowSpringsChateau/RainbowSpringsChateau.java
//       (updateAttacker, loadAttackers, clearTables)
//
// Same layout as residences.rs: verbatim SQL, one method per statement. D-39: the Java game
// server reads and writes the same rows.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: ClanHallSiegeEngine.SQL_LOAD_ATTACKERS
const SELECT_ATTACKERS: &str = "SELECT attacker_id FROM clanhall_siege_attackers WHERE clanhall_id = ?";
// Java: ClanHallSiegeEngine.saveAttackers
const DELETE_ATTACKERS: &str = "DELETE FROM clanhall_siege_attackers WHERE clanhall_id = ?";
// Java: ClanHallSiegeEngine.SQL_SAVE_ATTACKERS
const INSERT_ATTACKER: &str = "INSERT INTO clanhall_siege_attackers VALUES (?,?)";
// Java: ClanHallSiegeEngine.SQL_LOAD_GUARDS (`SELECT *`, the columns Java reads)
const SELECT_GUARDS: &str =
    "SELECT npcId, x, y, z, heading, respawnDelay FROM clanhall_siege_guards WHERE clanHallId = ?";

// Java: BanditStronghold.SQL_LOAD_ATTACKERS
const SELECT_FLAG_WAR_CLANS: &str =
    "SELECT clan_id, flag, npc FROM siegable_hall_flagwar_attackers WHERE hall_id = ?";
// Java: BanditStronghold.SQL_LOAD_MEMEBERS
const SELECT_FLAG_WAR_MEMBERS: &str =
    "SELECT object_id FROM siegable_hall_flagwar_attackers_members WHERE clan_id = ?";
// Java: BanditStronghold.SQL_SAVE_CLAN
const INSERT_FLAG_WAR_CLAN: &str = "INSERT INTO siegable_hall_flagwar_attackers VALUES(?,?,?,?)";
// Java: BanditStronghold.SQL_SAVE_NPC
const UPDATE_FLAG_WAR_NPC: &str = "UPDATE siegable_hall_flagwar_attackers SET npc = ? WHERE clan_id = ?";
// Java: BanditStronghold.SQL_SAVE_ATTACKER
const INSERT_FLAG_WAR_MEMBER: &str = "INSERT INTO siegable_hall_flagwar_attackers_members VALUES (?,?,?)";
// Java: BanditStronghold.SQL_CLEAR_CLAN, SQL_CLEAR_CLAN_ATTACKERS
const DELETE_FLAG_WAR_CLANS: &str = "DELETE FROM siegable_hall_flagwar_attackers WHERE hall_id = ?";
const DELETE_FLAG_WAR_MEMBERS: &str = "DELETE FROM siegable_hall_flagwar_attackers_members WHERE hall_id = ?";

// Java: RainbowSpringsChateau.loadAttackers
const SELECT_RAINBOW: &str = "SELECT * FROM rainbowsprings_attacker_list";
// Java: RainbowSpringsChateau.updateAttacker
const INSERT_RAINBOW: &str = "INSERT INTO rainbowsprings_attacker_list VALUES (?,?)";
const DELETE_RAINBOW_CLAN: &str = "DELETE FROM rainbowsprings_attacker_list WHERE clanId = ?";
// Java: RainbowSpringsChateau.clearTables
const DELETE_RAINBOW: &str = "DELETE FROM rainbowsprings_attacker_list";

/// A `clanhall_siege_guards` row (the columns Java reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HallSiegeGuardRow {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Seconds.
    pub respawn_delay: i32,
}

#[derive(Debug, Clone, FromRow)]
struct HallSiegeGuardDbRow {
    #[sqlx(rename = "npcId")]
    npc_id: u16,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    #[sqlx(rename = "respawnDelay")]
    respawn_delay: i32,
}

impl From<HallSiegeGuardDbRow> for HallSiegeGuardRow {
    fn from(r: HallSiegeGuardDbRow) -> Self {
        Self {
            npc_id: i32::from(r.npc_id),
            x: r.x,
            y: r.y,
            z: r.z,
            heading: r.heading,
            respawn_delay: r.respawn_delay,
        }
    }
}

/// A `siegable_hall_flagwar_attackers` row: a registered clan, its flag NPC id and the NPC it
/// chose (0 for none).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagWarClanRow {
    pub clan_id: i32,
    pub flag: i32,
    pub npc: i32,
}

#[derive(Debug, Clone, FromRow)]
struct FlagWarClanDbRow {
    clan_id: u32,
    flag: u32,
    npc: u32,
}

impl From<FlagWarClanDbRow> for FlagWarClanRow {
    fn from(r: FlagWarClanDbRow) -> Self {
        // Java `getInt` of an unsigned column wraps like `as`.
        Self {
            clan_id: r.clan_id as i32,
            flag: r.flag as i32,
            npc: r.npc as i32,
        }
    }
}

/// A `rainbowsprings_attacker_list` row (NULL reads as 0, like Java's `getInt` / `getLong`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RainbowAttackerRow {
    pub clan_id: i32,
    pub war_decrees_count: i64,
}

#[derive(Debug, Clone, FromRow)]
struct RainbowAttackerDbRow {
    #[sqlx(rename = "clanId")]
    clan_id: Option<i32>,
    war_decrees_count: Option<f64>,
}

impl From<RainbowAttackerDbRow> for RainbowAttackerRow {
    fn from(r: RainbowAttackerDbRow) -> Self {
        Self {
            clan_id: r.clan_id.unwrap_or(0),
            war_decrees_count: r.war_decrees_count.map_or(0, |v| v as i64),
        }
    }
}

/// Every query on the conquerable hall siege tables.
#[derive(Debug, Clone)]
pub struct HallSiegeRepository {
    pool: Pool,
}

impl HallSiegeRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    // Java: ClanHallSiegeEngine.loadAttackers
    pub async fn attackers(&self, hall_id: i32) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_ATTACKERS)
            .bind(hall_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    /// Replaces the attackers of the hall: Java deletes its rows and inserts one per clan, on one
    /// connection (`saveAttackers`). Not a transaction, like Java.
    // Java: ClanHallSiegeEngine.saveAttackers
    pub async fn save_attackers(&self, hall_id: i32, clans: &[i32]) -> Result<()> {
        sqlx::query(DELETE_ATTACKERS)
            .bind(hall_id)
            .execute(&self.pool)
            .await?;
        for clan in clans {
            sqlx::query(INSERT_ATTACKER)
                .bind(hall_id)
                .bind(*clan)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    // Java: ClanHallSiegeEngine.loadGuards
    pub async fn guards(&self, hall_id: i32) -> Result<Vec<HallSiegeGuardRow>> {
        let rows = sqlx::query_as::<_, HallSiegeGuardDbRow>(SELECT_GUARDS)
            .bind(hall_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(HallSiegeGuardRow::from).collect())
    }

    // Java: BanditStronghold.loadAttackers
    pub async fn flag_war_clans(&self, hall_id: i32) -> Result<Vec<FlagWarClanRow>> {
        let rows = sqlx::query_as::<_, FlagWarClanDbRow>(SELECT_FLAG_WAR_CLANS)
            .bind(hall_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(FlagWarClanRow::from).collect())
    }

    // Java: BanditStronghold.loadAttackerMembers
    pub async fn flag_war_members(&self, clan_id: i32) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, u32>(SELECT_FLAG_WAR_MEMBERS)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids.into_iter().map(|id| id as i32).collect())
    }

    // Java: BanditStronghold.saveClan
    pub async fn insert_flag_war_clan(&self, hall_id: i32, flag: i32, npc: i32, clan_id: i32) -> Result<()> {
        sqlx::query(INSERT_FLAG_WAR_CLAN)
            .bind(hall_id)
            .bind(flag)
            .bind(npc)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: BanditStronghold.saveNpc
    pub async fn update_flag_war_npc(&self, npc: i32, clan_id: i32) -> Result<()> {
        sqlx::query(UPDATE_FLAG_WAR_NPC)
            .bind(npc)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: BanditStronghold.saveMember
    pub async fn insert_flag_war_member(&self, hall_id: i32, clan_id: i32, object_id: i32) -> Result<()> {
        sqlx::query(INSERT_FLAG_WAR_MEMBER)
            .bind(hall_id)
            .bind(clan_id)
            .bind(object_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: BanditStronghold.clearTables
    pub async fn clear_flag_war(&self, hall_id: i32) -> Result<()> {
        sqlx::query(DELETE_FLAG_WAR_CLANS)
            .bind(hall_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(DELETE_FLAG_WAR_MEMBERS)
            .bind(hall_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: RainbowSpringsChateau.loadAttackers
    pub async fn rainbow_attackers(&self) -> Result<Vec<RainbowAttackerRow>> {
        let rows = sqlx::query_as::<_, RainbowAttackerDbRow>(SELECT_RAINBOW)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(RainbowAttackerRow::from).collect())
    }

    // Java: RainbowSpringsChateau.updateAttacker (a new registration)
    pub async fn insert_rainbow_attacker(&self, clan_id: i32, war_decrees_count: i64) -> Result<()> {
        sqlx::query(INSERT_RAINBOW)
            .bind(clan_id)
            .bind(war_decrees_count)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: RainbowSpringsChateau.updateAttacker (a cancelled one)
    pub async fn delete_rainbow_attacker(&self, clan_id: i32) -> Result<()> {
        sqlx::query(DELETE_RAINBOW_CLAN)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: RainbowSpringsChateau.clearTables
    pub async fn clear_rainbow_attackers(&self) -> Result<()> {
        sqlx::query(DELETE_RAINBOW).execute(&self.pool).await?;
        Ok(())
    }
}
