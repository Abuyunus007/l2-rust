//! The clan tables: `clan_data`, `clan_privs`, `clan_subpledges`, `clan_skills`,
//! `clan_notices`, `clan_wars` (read only here) and the clan columns of `characters`.
// Java: org/l2jmobius/gameserver/entity/clan/Clan.java (restore*, store, updateClanInDB,
//       removeMemberInDatabase, createSubPledge, updateSubPledgeInDB, setRankPrivs, storeNotice,
//       changeLevel, changeAllyCrest, setNewLeader)
// Java: org/l2jmobius/gameserver/entity/clan/ClanMember.java (updatePledgeType, updatePowerGrade,
//       saveApprenticeAndSponsor)
// Java: org/l2jmobius/gameserver/data/sql/ClanTable.java (constructor, destroyClan,
//       restoreClanWars)
// Java: network/GameClient.java (markToDeleteChar: the leader of a clan)
//
// Same layout as characters.rs: verbatim SQL, one method per statement. D-39: the Java game server
// reads and writes the same rows, so every statement is Java's own. Java runs each statement on
// its own (no transaction); so does this.

use sqlx::FromRow;

use crate::error::Result;
use crate::pool::Pool;

// Java: ClanTable constructor
const SELECT_CLAN_IDS: &str = "SELECT clan_id FROM clan_data";
// Java: Clan.SELECT_CLAN_DATA
const SELECT_CLAN_DATA: &str = "SELECT * FROM clan_data where clan_id=?";
// Java: Clan.restore (the members)
const SELECT_CLAN_MEMBERS: &str = "SELECT char_name,level,classid,charId,title,power_grade,subpledge,apprentice,sponsor,sex,race FROM characters WHERE clanid=?";
// Java: Clan.restoreSubPledges
const SELECT_SUB_PLEDGES: &str = "SELECT sub_pledge_id,name,leader_id FROM clan_subpledges WHERE clan_id=?";
// Java: Clan.restoreRankPrivs
const SELECT_RANK_PRIVS: &str = "SELECT privs,`rank`,party FROM clan_privs WHERE clan_id=?";
// Java: Clan.restoreSkills
const SELECT_CLAN_SKILLS: &str = "SELECT skill_id,skill_level,sub_pledge_id FROM clan_skills WHERE clan_id=?";
// Java: Clan.addNewSkill
const UPDATE_CLAN_SKILL: &str = "UPDATE clan_skills SET skill_level=? WHERE skill_id=? AND clan_id=?";
const INSERT_CLAN_SKILL: &str =
    "INSERT INTO clan_skills (clan_id,skill_id,skill_level,skill_name,sub_pledge_id) VALUES (?,?,?,?,?)";
// Java: Clan.restoreNotice
const SELECT_CLAN_NOTICE: &str = "SELECT enabled,notice FROM clan_notices WHERE clan_id=?";
// Java: ClanTable.restoreClanWars
const SELECT_CLAN_WARS: &str = "SELECT clan1, clan2 FROM clan_wars";

// Java: Clan.INSERT_CLAN_DATA
const INSERT_CLAN_DATA: &str = "INSERT INTO clan_data (clan_id,clan_name,clan_level,hasCastle,blood_alliance_count,blood_oath_count,ally_id,ally_name,leader_id,crest_id,crest_large_id,ally_crest_id,new_leader_id) values (?,?,?,?,?,?,?,?,?,?,?,?,?)";
// Java: Clan.updateClanInDB
const UPDATE_CLAN: &str = "UPDATE clan_data SET leader_id=?,ally_id=?,ally_name=?,reputation_score=?,ally_penalty_expiry_time=?,ally_penalty_type=?,char_penalty_expiry_time=?,dissolving_expiry_time=?,new_leader_id=? WHERE clan_id=?";
// Java: Clan.changeLevel
const UPDATE_CLAN_LEVEL: &str = "UPDATE clan_data SET clan_level = ? WHERE clan_id = ?";
// Java: Clan.changeAllyCrest(crestId, true)
const UPDATE_ALLY_CREST_OF_CLAN: &str = "UPDATE clan_data SET ally_crest_id = ? WHERE clan_id = ?";
// Java: Clan.removeMemberInDatabase (three statements)
const REMOVE_MEMBER: &str = "UPDATE characters SET clanid=0, title=?, clan_join_expiry_time=?, clan_create_expiry_time=?, clan_privs=0, wantspeace=0, subpledge=0, lvl_joined_academy=0, apprentice=0, sponsor=0 WHERE charId=?";
const CLEAR_APPRENTICE_OF: &str = "UPDATE characters SET apprentice=0 WHERE apprentice=?";
const CLEAR_SPONSOR_OF: &str = "UPDATE characters SET sponsor=0 WHERE sponsor=?";
// Java: Clan.setNewLeader (a leader who is offline)
const UPDATE_CHAR_CLAN_PRIVS: &str = "UPDATE characters SET clan_privs = ? WHERE charId = ?";
// Java: ClanMember.updatePledgeType
const UPDATE_CHAR_SUBPLEDGE: &str = "UPDATE characters SET subpledge=? WHERE charId=?";
// Java: ClanMember.updatePowerGrade
const UPDATE_CHAR_POWER_GRADE: &str = "UPDATE characters SET power_grade=? WHERE charId=?";
// Java: ClanMember.saveApprenticeAndSponsor
const UPDATE_CHAR_APPRENTICE_SPONSOR: &str = "UPDATE characters SET apprentice=?,sponsor=? WHERE charId=?";
// Java: Clan.createSubPledge
const INSERT_SUB_PLEDGE: &str =
    "INSERT INTO clan_subpledges (clan_id,sub_pledge_id,name,leader_id) values (?,?,?,?)";
// Java: Clan.updateSubPledgeInDB
const UPDATE_SUB_PLEDGE: &str =
    "UPDATE clan_subpledges SET leader_id=?, name=? WHERE clan_id=? AND sub_pledge_id=?";
// Java: Clan.setRankPrivs
const REPLACE_RANK_PRIVS: &str = "REPLACE INTO clan_privs (clan_id,`rank`,party,privs) VALUES (?,?,?,?)";
// Java: Clan.storeNotice
const STORE_NOTICE: &str = "INSERT INTO clan_notices (clan_id,notice,enabled) values (?,?,?) ON DUPLICATE KEY UPDATE notice=?,enabled=?";
// Java: ClanTable.destroyClan
const DELETE_CLAN_DATA: &str = "DELETE FROM clan_data WHERE clan_id=?";
const DELETE_CLAN_PRIVS: &str = "DELETE FROM clan_privs WHERE clan_id=?";
const DELETE_CLAN_SKILLS: &str = "DELETE FROM clan_skills WHERE clan_id=?";
const DELETE_CLAN_SUBPLEDGES: &str = "DELETE FROM clan_subpledges WHERE clan_id=?";
const DELETE_CLAN_WARS: &str = "DELETE FROM clan_wars WHERE clan1=? OR clan2=?";
const DELETE_CLAN_NOTICES: &str = "DELETE FROM clan_notices WHERE clan_id=?";
const RESET_CASTLE_TAX: &str = "UPDATE castle SET taxPercent = 0 WHERE id = ?";

/// The part of a `clan_data` row the character screen needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
struct ClanLeaderRow {
    /// `leader_id`: nullable in the schema, `getInt` reads NULL as 0.
    leader_id: Option<i32>,
}

/// One `clan_data` row (`SELECT *`). Nullable columns are `Option`; Java's `getInt`/`getString`
/// read NULL as 0 / `null`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ClanDataRow {
    pub clan_id: i32,
    pub clan_name: Option<String>,
    pub clan_level: Option<i32>,
    pub reputation_score: i32,
    #[sqlx(rename = "hasCastle")]
    pub has_castle: Option<i32>,
    pub blood_alliance_count: u16,
    pub blood_oath_count: u16,
    pub ally_id: Option<i32>,
    pub ally_name: Option<String>,
    pub leader_id: Option<i32>,
    pub crest_id: Option<i32>,
    pub crest_large_id: Option<i32>,
    pub ally_crest_id: Option<i32>,
    pub auction_bid_at: i32,
    pub ally_penalty_expiry_time: u64,
    pub ally_penalty_type: i8,
    pub char_penalty_expiry_time: u64,
    pub dissolving_expiry_time: u64,
    pub new_leader_id: u32,
}

/// One member as `Clan.restore` reads it from `characters` (`new ClanMember(clan, rs)`).
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ClanMemberRow {
    pub char_name: String,
    pub level: Option<u8>,
    pub classid: Option<u8>,
    #[sqlx(rename = "charId")]
    pub char_id: u32,
    pub title: Option<String>,
    pub power_grade: Option<u8>,
    pub subpledge: i16,
    pub apprentice: u32,
    pub sponsor: u32,
    pub sex: Option<u8>,
    pub race: Option<u8>,
}

/// One `clan_subpledges` row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ClanSubPledgeRow {
    pub sub_pledge_id: i32,
    pub name: Option<String>,
    pub leader_id: i32,
}

/// One `clan_privs` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct ClanPrivsRow {
    pub privs: i32,
    pub rank: i32,
    pub party: i32,
}

/// One `clan_skills` row (the columns `Clan.restoreSkills` reads).
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct ClanSkillRow {
    pub skill_id: i32,
    pub skill_level: i32,
    pub sub_pledge_id: i32,
}

/// One `clan_notices` row: `enabled` is the enum text `'true'`/`'false'`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ClanNoticeRow {
    pub enabled: String,
    pub notice: String,
}

/// One `clan_wars` row. The columns are `VARCHAR` in the schema; Java reads them with `getInt`.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct ClanWarRow {
    pub clan1: String,
    pub clan2: String,
}

/// The values `Clan.store` binds (`INSERT_CLAN_DATA`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewClan {
    pub clan_id: i32,
    pub clan_name: String,
    pub clan_level: i32,
    pub castle_id: i32,
    pub blood_alliance_count: i32,
    pub blood_oath_count: i32,
    pub ally_id: i32,
    /// `setString(null)` writes NULL.
    pub ally_name: Option<String>,
    pub leader_id: i32,
    pub crest_id: i32,
    pub crest_large_id: i32,
    pub ally_crest_id: i32,
    pub new_leader_id: i32,
}

/// The values `Clan.updateClanInDB` binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanUpdate {
    pub clan_id: i32,
    pub leader_id: i32,
    pub ally_id: i32,
    pub ally_name: Option<String>,
    pub reputation_score: i32,
    pub ally_penalty_expiry_time: i64,
    pub ally_penalty_type: i32,
    pub char_penalty_expiry_time: i64,
    pub dissolving_expiry_time: i64,
    pub new_leader_id: i32,
}

/// Every query on the clan tables.
#[derive(Debug, Clone)]
pub struct ClanRepository {
    pool: Pool,
}

impl ClanRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The leader's character id of the clan, or `None` when no such clan exists
    /// (`ClanTable.getClan` returns `null`).
    // Java: Clan.restore (leader_id), ClanTable.getClan
    pub async fn leader_id(&self, clan_id: i32) -> Result<Option<i32>> {
        let row = sqlx::query_as::<_, ClanLeaderRow>(SELECT_CLAN_DATA)
            .bind(clan_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.leader_id.unwrap_or(0)))
    }

    // ------------------------------------------------------------------ reads (startup)

    /// Every clan id, in the order the database returns them.
    // Java: ClanTable constructor ("SELECT clan_id FROM clan_data")
    pub async fn clan_ids(&self) -> Result<Vec<i32>> {
        let ids = sqlx::query_scalar::<_, i32>(SELECT_CLAN_IDS)
            .fetch_all(&self.pool)
            .await?;
        Ok(ids)
    }

    /// The `clan_data` row of a clan.
    // Java: Clan.restore (SELECT_CLAN_DATA)
    pub async fn clan_data(&self, clan_id: i32) -> Result<Option<ClanDataRow>> {
        let row = sqlx::query_as::<_, ClanDataRow>(SELECT_CLAN_DATA)
            .bind(clan_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// The characters whose `clanid` is the clan.
    // Java: Clan.restore (the member query)
    pub async fn clan_members(&self, clan_id: i32) -> Result<Vec<ClanMemberRow>> {
        let rows = sqlx::query_as::<_, ClanMemberRow>(SELECT_CLAN_MEMBERS)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // Java: Clan.restoreSubPledges
    pub async fn sub_pledges(&self, clan_id: i32) -> Result<Vec<ClanSubPledgeRow>> {
        let rows = sqlx::query_as::<_, ClanSubPledgeRow>(SELECT_SUB_PLEDGES)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // Java: Clan.restoreRankPrivs
    pub async fn rank_privs(&self, clan_id: i32) -> Result<Vec<ClanPrivsRow>> {
        let rows = sqlx::query_as::<_, ClanPrivsRow>(SELECT_RANK_PRIVS)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // Java: Clan.restoreSkills
    pub async fn clan_skills(&self, clan_id: i32) -> Result<Vec<ClanSkillRow>> {
        let rows = sqlx::query_as::<_, ClanSkillRow>(SELECT_CLAN_SKILLS)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// The notice rows of a clan (the primary key makes it at most one; Java loops and keeps the
    /// last).
    // Java: Clan.restoreNotice
    pub async fn clan_notices(&self, clan_id: i32) -> Result<Vec<ClanNoticeRow>> {
        let rows = sqlx::query_as::<_, ClanNoticeRow>(SELECT_CLAN_NOTICE)
            .bind(clan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // Java: ClanTable.restoreClanWars
    pub async fn clan_wars(&self) -> Result<Vec<ClanWarRow>> {
        let rows = sqlx::query_as::<_, ClanWarRow>(SELECT_CLAN_WARS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // ------------------------------------------------------------------ writes

    // Java: Clan.store (INSERT_CLAN_DATA)
    pub async fn insert_clan(&self, clan: &NewClan) -> Result<()> {
        sqlx::query(INSERT_CLAN_DATA)
            .bind(clan.clan_id)
            .bind(&clan.clan_name)
            .bind(clan.clan_level)
            .bind(clan.castle_id)
            .bind(clan.blood_alliance_count)
            .bind(clan.blood_oath_count)
            .bind(clan.ally_id)
            .bind(&clan.ally_name)
            .bind(clan.leader_id)
            .bind(clan.crest_id)
            .bind(clan.crest_large_id)
            .bind(clan.ally_crest_id)
            .bind(clan.new_leader_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Clan.updateClanInDB
    pub async fn update_clan(&self, clan: &ClanUpdate) -> Result<()> {
        sqlx::query(UPDATE_CLAN)
            .bind(clan.leader_id)
            .bind(clan.ally_id)
            .bind(&clan.ally_name)
            .bind(clan.reputation_score)
            .bind(clan.ally_penalty_expiry_time)
            .bind(clan.ally_penalty_type)
            .bind(clan.char_penalty_expiry_time)
            .bind(clan.dissolving_expiry_time)
            .bind(clan.new_leader_id)
            .bind(clan.clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Clan.changeLevel
    pub async fn update_clan_level(&self, clan_id: i32, level: i32) -> Result<()> {
        sqlx::query(UPDATE_CLAN_LEVEL)
            .bind(level)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Clan.changeAllyCrest(crestId, onlyThisClan = true)
    pub async fn update_ally_crest_of_clan(&self, clan_id: i32, crest_id: i32) -> Result<()> {
        sqlx::query(UPDATE_ALLY_CREST_OF_CLAN)
            .bind(crest_id)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The member leaves the clan in the database, and nobody keeps it as apprentice or sponsor.
    // Java: Clan.removeMemberInDatabase
    pub async fn remove_member(
        &self,
        char_id: i32,
        clan_join_expiry_time: i64,
        clan_create_expiry_time: i64,
    ) -> Result<()> {
        // Remove clan member.
        sqlx::query(REMOVE_MEMBER)
            .bind("")
            .bind(clan_join_expiry_time)
            .bind(clan_create_expiry_time)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        // Remove apprentice.
        sqlx::query(CLEAR_APPRENTICE_OF)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        // Remove sponsor.
        sqlx::query(CLEAR_SPONSOR_OF)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Clan.setNewLeader ("UPDATE characters SET clan_privs = ? WHERE charId = ?")
    pub async fn update_char_clan_privs(&self, char_id: i32, privs: i32) -> Result<()> {
        sqlx::query(UPDATE_CHAR_CLAN_PRIVS)
            .bind(privs)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanMember.updatePledgeType (`setLong`)
    pub async fn update_char_subpledge(&self, char_id: i32, pledge_type: i32) -> Result<()> {
        sqlx::query(UPDATE_CHAR_SUBPLEDGE)
            .bind(i64::from(pledge_type))
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanMember.updatePowerGrade (`setLong`)
    pub async fn update_char_power_grade(&self, char_id: i32, power_grade: i32) -> Result<()> {
        sqlx::query(UPDATE_CHAR_POWER_GRADE)
            .bind(i64::from(power_grade))
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: ClanMember.saveApprenticeAndSponsor
    pub async fn update_char_apprentice_sponsor(
        &self,
        char_id: i32,
        apprentice: i32,
        sponsor: i32,
    ) -> Result<()> {
        sqlx::query(UPDATE_CHAR_APPRENTICE_SPONSOR)
            .bind(apprentice)
            .bind(sponsor)
            .bind(char_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A clan that already had the skill: only the level changes. Java's statement does not look at
    /// `sub_pledge_id`, so a skill id that two squads of the clan share changes in all of them.
    // Java: Clan.addNewSkill
    pub async fn update_clan_skill(&self, clan_id: i32, skill_id: i32, skill_level: i32) -> Result<()> {
        sqlx::query(UPDATE_CLAN_SKILL)
            .bind(skill_level)
            .bind(skill_id)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `sub_pledge_id` is -2 for a clan skill, 0 for a skill of the main squad.
    // Java: Clan.addNewSkill
    pub async fn insert_clan_skill(
        &self,
        clan_id: i32,
        skill_id: i32,
        skill_level: i32,
        skill_name: &str,
        sub_pledge_id: i32,
    ) -> Result<()> {
        sqlx::query(INSERT_CLAN_SKILL)
            .bind(clan_id)
            .bind(skill_id)
            .bind(skill_level)
            .bind(skill_name)
            .bind(sub_pledge_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Clan.createSubPledge
    pub async fn insert_sub_pledge(
        &self,
        clan_id: i32,
        sub_pledge_id: i32,
        name: &str,
        leader_id: i32,
    ) -> Result<()> {
        sqlx::query(INSERT_SUB_PLEDGE)
            .bind(clan_id)
            .bind(sub_pledge_id)
            .bind(name)
            .bind(leader_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Clan.updateSubPledgeInDB
    pub async fn update_sub_pledge(
        &self,
        clan_id: i32,
        sub_pledge_id: i32,
        name: &str,
        leader_id: i32,
    ) -> Result<()> {
        sqlx::query(UPDATE_SUB_PLEDGE)
            .bind(leader_id)
            .bind(name)
            .bind(clan_id)
            .bind(sub_pledge_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `party` is always 0 in Java.
    // Java: Clan.setRankPrivs
    pub async fn replace_rank_privs(&self, clan_id: i32, rank: i32, privs: i32) -> Result<()> {
        sqlx::query(REPLACE_RANK_PRIVS)
            .bind(clan_id)
            .bind(rank)
            .bind(0)
            .bind(privs)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Java: Clan.storeNotice
    pub async fn store_notice(&self, clan_id: i32, notice: &str, enabled: bool) -> Result<()> {
        let enabled = if enabled { "true" } else { "false" };
        sqlx::query(STORE_NOTICE)
            .bind(clan_id)
            .bind(notice)
            .bind(enabled)
            .bind(notice)
            .bind(enabled)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The rows `ClanTable.destroyClan` deletes, and the castle tax reset when the clan had a
    /// castle (`castle_id != 0`).
    // Java: ClanTable.destroyClan
    pub async fn delete_clan(&self, clan_id: i32, castle_id: i32) -> Result<()> {
        for statement in [
            DELETE_CLAN_DATA,
            DELETE_CLAN_PRIVS,
            DELETE_CLAN_SKILLS,
            DELETE_CLAN_SUBPLEDGES,
        ] {
            sqlx::query(statement).bind(clan_id).execute(&self.pool).await?;
        }
        sqlx::query(DELETE_CLAN_WARS)
            .bind(clan_id)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(DELETE_CLAN_NOTICES)
            .bind(clan_id)
            .execute(&self.pool)
            .await?;
        if castle_id != 0 {
            sqlx::query(RESET_CASTLE_TAX)
                .bind(castle_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        let count = |s: &str| s.matches('?').count();
        assert_eq!(count(SELECT_CLAN_IDS), 0);
        assert_eq!(count(SELECT_CLAN_DATA), 1);
        assert_eq!(count(SELECT_CLAN_MEMBERS), 1);
        assert_eq!(count(SELECT_SUB_PLEDGES), 1);
        assert_eq!(count(SELECT_RANK_PRIVS), 1);
        assert_eq!(count(SELECT_CLAN_SKILLS), 1);
        assert_eq!(count(SELECT_CLAN_NOTICE), 1);
        assert_eq!(count(SELECT_CLAN_WARS), 0);
        assert_eq!(count(INSERT_CLAN_DATA), 13);
        assert_eq!(count(UPDATE_CLAN), 10);
        assert_eq!(count(UPDATE_CLAN_LEVEL), 2);
        assert_eq!(count(UPDATE_ALLY_CREST_OF_CLAN), 2);
        assert_eq!(count(REMOVE_MEMBER), 4);
        assert_eq!(count(CLEAR_APPRENTICE_OF), 1);
        assert_eq!(count(CLEAR_SPONSOR_OF), 1);
        assert_eq!(count(UPDATE_CHAR_CLAN_PRIVS), 2);
        assert_eq!(count(UPDATE_CHAR_SUBPLEDGE), 2);
        assert_eq!(count(UPDATE_CHAR_POWER_GRADE), 2);
        assert_eq!(count(UPDATE_CHAR_APPRENTICE_SPONSOR), 3);
        assert_eq!(count(INSERT_SUB_PLEDGE), 4);
        assert_eq!(count(UPDATE_SUB_PLEDGE), 4);
        assert_eq!(count(REPLACE_RANK_PRIVS), 4);
        assert_eq!(count(STORE_NOTICE), 5);
        assert_eq!(count(DELETE_CLAN_WARS), 2);
        assert_eq!(count(RESET_CASTLE_TAX), 1);
    }
}
