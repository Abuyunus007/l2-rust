//! The clan table: loading the clans, creating and destroying one, the delayed dissolution, the
//! clan ranks, the start-up alliance check, the shutdown save and the weekly leader change.
// Java: data/sql/ClanTable.java
// Java: entity/clan/Clan.java (restore*, store)
// Java: managers/DailyResetManager.java (clanLeaderApply)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use super::model::{
    COMPLETE_PRIVILEGES_MASK, Clan, ClanMember, ClanPrivilegesRef, SubPledge, calculate_pledge_class,
    is_alpha_numeric, java_len,
};
use super::{ClanRecord, ClanRow, ClanTimer, ClanWrite, java_int_days_ms};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// `ThreadPool.scheduleAtFixedRate(this::updateClanRanks, 1000, 1200000)`.
const UPDATE_RANKS_FIRST_MS: u64 = 1000;
const UPDATE_RANKS_PERIOD_MS: u64 = 1_200_000;
/// `scheduleRemoveClan`'s shortest delay (five minutes).
const REMOVE_CLAN_MIN_DELAY_MS: i64 = 300_000;
/// `setReputationScore`'s limits.
pub(crate) const MAX_REPUTATION: i32 = 100_000_000;

impl World {
    /// Java `ClanTable`'s constructor: every clan from the rows the game server read (`new
    /// Clan(clanId)` for each), the dissolutions that are due, the alliance check, the clan wars
    /// and the rank task. `wars` are the `clan_wars` rows `(clan1, clan2)`.
    pub fn load_clans(&mut self, records: Vec<ClanRecord>, wars: Vec<(String, String)>) {
        let now = unix_ms_now();
        let count = records.len();
        for record in records {
            let clan = self.restore_clan(record, now);
            let id = clan.id;
            let dissolving = clan.dissolving_expiry_time;
            self.clans.clans.insert(id, clan);
            if dissolving != 0 {
                self.schedule_remove_clan(id);
            }
        }
        info!("ClanTable: Restored {count} clans from the database.");
        self.alliance_check();
        self.restore_clan_wars(&wars);
        let due = self.tick + TimerWheel::ms_to_ticks(UPDATE_RANKS_FIRST_MS);
        self.timers
            .schedule_at(due, TimerKind::Clan(ClanTimer::UpdateRanks));
    }

    /// Java `new Clan(clanId)`: `initializePrivs`, then `restore` (the clan, its members, sub
    /// pledges, rank privileges, skills and notice).
    pub(crate) fn restore_clan(&self, record: ClanRecord, now: i64) -> Clan {
        let settings = &self.clans.settings;
        let mut clan = Clan::new(record.clan_id, "");
        if let Some(data) = record.data {
            // NOTE: a NULL `clan_name` is Java's `null` name; it is an empty name here.
            clan.name = data.name.unwrap_or_default();
            clan.level = data.level;
            clan.castle_id = data.castle_id;
            clan.blood_alliance_count = data.blood_alliance_count;
            clan.blood_oath_count = data.blood_oath_count;
            clan.ally_id = data.ally_id;
            clan.ally_name = data.ally_name;
            clan.ally_penalty_expiry_time = data.ally_penalty_expiry_time;
            clan.ally_penalty_type = data.ally_penalty_type;
            if clan.ally_penalty_expiry_time < now {
                clan.ally_penalty_expiry_time = 0;
                clan.ally_penalty_type = 0;
            }
            clan.char_penalty_expiry_time = data.char_penalty_expiry_time;
            if clan
                .char_penalty_expiry_time
                .wrapping_add(java_int_days_ms(settings.alt_clan_join_days))
                < now
            {
                clan.char_penalty_expiry_time = 0;
            }
            clan.dissolving_expiry_time = data.dissolving_expiry_time;
            clan.crest_id = data.crest_id;
            clan.crest_large_id = data.crest_large_id;
            clan.ally_crest_id = data.ally_crest_id;
            // `setReputationScore` (nobody is online to be told).
            clan.reputation_score = data.reputation_score.clamp(-MAX_REPUTATION, MAX_REPUTATION);
            clan.auction_bid_at = data.auction_bid_at;
            clan.new_leader_id = data.new_leader_id;
            for row in record.members {
                let member = ClanMember {
                    object_id: ObjectId(row.object_id),
                    name: row.name,
                    title: row.title,
                    power_grade: row.power_grade,
                    level: row.level,
                    class_id: row.class_id,
                    female: row.female,
                    race: row.race,
                    pledge_type: row.pledge_type,
                    apprentice: row.apprentice,
                    sponsor: row.sponsor,
                    has_player: false,
                };
                if member.object_id.0 == data.leader_id {
                    // `setLeader(member)`
                    clan.leader = Some(member.object_id);
                }
                clan.put_member(member);
            }
        }

        // restoreSubPledges
        for (id, name, leader_id) in record.sub_pledges {
            clan.put_sub_pledge(SubPledge {
                id,
                name,
                leader_id,
                skills: Vec::new(),
            });
        }

        // restoreRankPrivs
        for (rank, _party, privs) in record.rank_privs {
            if rank == -1 {
                continue;
            }
            match clan.rank_privs_mut(rank) {
                Some(rp) => rp.privs = privs,
                None => {
                    // NOTE(java-quirk): `_privs.get(rank)` is null for a rank outside 1 to 9: the
                    // NullPointerException ends the restore of the privileges.
                    warn!(
                        "Clan: Error restoring clan privs by rank: rank {rank} of clan {}",
                        clan.id
                    );
                    break;
                }
            }
        }

        // restoreSkills
        let table = self.skill_table();
        for (id, level, sub_type) in record.skills {
            if table.as_deref().is_some_and(|t| t.get(id, level).is_none()) {
                // NOTE(java-quirk): `SkillData.getSkill` is null: `skill.getId()` throws and the
                // remaining skills are not restored.
                warn!(
                    "Clan: Error restoring clan skills: skill {id} level {level} of clan {} does not exist",
                    clan.id
                );
                break;
            }
            match sub_type {
                -2 => {
                    put_skill(&mut clan.skills, id, level);
                }
                0 => {
                    put_skill(&mut clan.sub_pledge_skills, id, level);
                }
                _ => match clan.sub_pledge_mut(sub_type) {
                    Some(sub) => {
                        put_skill(&mut sub.skills, id, level);
                    }
                    None => info!(
                        "Clan: Missing subpledge {sub_type} for clan {}[{}], skill skipped.",
                        clan.name, clan.id
                    ),
                },
            }
        }

        // restoreNotice
        for (enabled, notice) in record.notices {
            // `Boolean.parseBoolean`
            clan.notice_enabled = enabled.eq_ignore_ascii_case("true");
            clan.notice = Some(notice);
        }
        clan
    }

    /// Java `ClanTable.restoreClanWars`.
    fn restore_clan_wars(&mut self, wars: &[(String, String)]) {
        for (clan1, clan2) in wars {
            // `rset.getInt` of the `VARCHAR` columns.
            let (Ok(id1), Ok(id2)) = (clan1.trim().parse::<i32>(), clan2.trim().parse::<i32>()) else {
                warn!("ClanTable: Error restoring clan wars data: '{clan1}', '{clan2}' are not clan ids.");
                return;
            };
            if self.clan(id1).is_some() && self.clan(id2).is_some() {
                if let Some(c) = self.clan_mut(id1)
                    && !c.at_war_with.contains(&id2)
                {
                    c.at_war_with.push(id2);
                }
                if let Some(c) = self.clan_mut(id2)
                    && !c.at_war_attackers.contains(&id1)
                {
                    c.at_war_attackers.push(id1);
                }
            } else {
                warn!(
                    "ClanTable: restorewars one of clans is null clan1:{} clan2:{}",
                    self.clan(id1)
                        .map_or_else(|| "null".to_owned(), |c| format!("{}[{}]", c.name, c.id)),
                    self.clan(id2)
                        .map_or_else(|| "null".to_owned(), |c| format!("{}[{}]", c.name, c.id))
                );
            }
        }
    }

    /// Java `ClanTable.allianceCheck`: a clan in an alliance whose leading clan is gone leaves it.
    /// D-49: the rows are left to the Java game server without `ClanMaintenanceDatabaseWrites`.
    fn alliance_check(&mut self) {
        let write = self.clans.settings.maintenance_database_writes;
        let ids: Vec<i32> = self.clans.clans.keys().copied().collect();
        for id in ids {
            let Some(clan) = self.clan(id) else {
                continue;
            };
            let ally_id = clan.ally_id;
            if ally_id == 0 || id == ally_id || self.clans.clans.contains_key(&ally_id) {
                continue;
            }
            if let Some(clan) = self.clan_mut(id) {
                clan.ally_id = 0;
                clan.ally_name = None;
                // `changeAllyCrest(0, true)`: nobody is online to see it.
                clan.ally_crest_id = 0;
            }
            if write {
                self.clan_write(ClanWrite::AllyCrestOfClan {
                    clan_id: id,
                    crest_id: 0,
                });
                self.update_clan_in_db(id);
            }
            info!("ClanTable: Removed alliance from clan: {}", self.clan_display(id));
        }
    }

    /// Java `Clan.toString()`: `name[id]`.
    pub(crate) fn clan_display(&self, clan_id: i32) -> String {
        self.clan(clan_id)
            .map_or_else(|| format!("null[{clan_id}]"), |c| format!("{}[{}]", c.name, c.id))
    }

    /// Java `ClanTable.getClanRank(clan)`.
    pub fn clan_rank(&self, clan_id: i32) -> i32 {
        let Some(clan) = self.clan(clan_id) else {
            return 0;
        };
        if clan.level < 3 {
            return 0;
        }
        let mut rank = 1;
        for c in self.clans.clans.values() {
            if c.id != clan.id
                && (clan.level < c.level
                    || (clan.level == c.level && clan.reputation_score <= c.reputation_score))
            {
                rank += 1;
            }
        }
        rank
    }

    /// Java `ClanTable.updateClanRanks`, then the next run 20 minutes later.
    pub(crate) fn on_update_clan_ranks_timer(&mut self) {
        let ranks: Vec<(i32, i32)> = self
            .clans
            .clans
            .keys()
            .map(|id| (*id, self.clan_rank(*id)))
            .collect();
        for (id, rank) in ranks {
            if let Some(c) = self.clan_mut(id) {
                c.rank = rank;
            }
        }
        let due = self.tick + TimerWheel::ms_to_ticks(UPDATE_RANKS_PERIOD_MS);
        self.timers
            .schedule_at(due, TimerKind::Clan(ClanTimer::UpdateRanks));
    }

    /// Java `ClanTable.getClanAllies(allianceId)`: the clans of the alliance (none for 0).
    pub fn clan_allies(&self, alliance_id: i32) -> Vec<i32> {
        if alliance_id == 0 {
            return Vec::new();
        }
        self.clans
            .clans
            .values()
            .filter(|c| c.ally_id == alliance_id)
            .map(|c| c.id)
            .collect()
    }

    /// Java `ClanTable.isAllyExists(allyName)`.
    pub fn is_ally_exists(&self, ally_name: &str) -> bool {
        self.clans.clans.values().any(|c| {
            c.ally_name
                .as_deref()
                .is_some_and(|n| super::model::java_equals_ignore_case(n, ally_name))
        })
    }

    // ------------------------------------------------------------------ create

    /// Java `ClanTable.createClan(player, clanName)`. The new clan's id, `None` when a check
    /// failed (with Java's message).
    pub fn create_clan(&mut self, player: ObjectId, clan_name: &str) -> Option<i32> {
        let p = self.players.get(&player)?;
        let level = p.core.level;
        let state = self.player_clan_state(player);
        if 10 > level {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotMeetTheCriteriaInOrderToCreateAClan),
            );
            return None;
        }
        if state.clan_id != 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveFailedToCreateAClan),
            );
            return None;
        }
        if unix_ms_now() < state.clan_create_expiry_time {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouMustWait10DaysBeforeCreatingANewClan),
            );
            return None;
        }
        if !is_alpha_numeric(clan_name) || 2 > java_len(clan_name) {
            self.send(player, &SystemMessage::new(SystemMessageId::ClanNameIsInvalid));
            return None;
        }
        if 16 < java_len(clan_name) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ClanNameSLengthIsIncorrect),
            );
            return None;
        }
        if self.clan_by_name(clan_name).is_some() {
            // Clan name is already taken.
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::S1AlreadyExists)
                    .with(SystemMessageParam::Text(clan_name.to_owned())),
            );
            return None;
        }

        // `IdManager.getInstance().getNextId()`
        let clan_id = self.allocate_clan_id()?;
        let mut clan = Clan::new(clan_id, clan_name);
        // `new ClanMember(clan, player)`, `clan.setLeader(leader)`, `leader.setPlayer(player)`.
        let member = self.member_of_player(player)?;
        clan.leader = Some(member.object_id);
        clan.put_member(member);
        // `setPlayer` -> `clan.addSkillEffects(player)`: a new clan has no skills.
        let row = ClanRow {
            clan_id,
            name: clan.name.clone(),
            level: clan.level,
            castle_id: clan.castle_id,
            blood_alliance_count: clan.blood_alliance_count,
            blood_oath_count: clan.blood_oath_count,
            ally_id: clan.ally_id,
            ally_name: clan.ally_name.clone(),
            leader_id: clan.leader_id(),
            crest_id: clan.crest_id,
            crest_large_id: clan.crest_large_id,
            ally_crest_id: clan.ally_crest_id,
            new_leader_id: clan.new_leader_id,
        };
        self.clans.clans.insert(clan_id, clan);
        // `clan.store()`
        self.clan_write(ClanWrite::Insert(row));
        // `player.setClan(clan)`
        if let Some(s) = self.player_clan_state_mut(player) {
            s.clan_id = clan_id;
            s.keep_stored = false;
        }
        self.set_pledge_class(player);
        // `new ClanPrivileges()` + `enableAll()`: an object of the player's own.
        if let Some(s) = self.player_clan_state_mut(player) {
            s.privileges = ClanPrivilegesRef::Own(COMPLETE_PRIVILEGES_MASK);
        }

        // "should be update packet only"
        if let Some(clan) = self.clan(clan_id) {
            let update = self.pledge_show_info_update(clan);
            self.send(player, &update);
        }
        self.send_pledge_show_member_list_all(clan_id, player);
        self.send_user_info(player);
        if let Some(update) = self.pledge_member_update_of_player(player) {
            self.send(player, &update);
        }
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YourClanHasBeenCreated),
        );
        Some(clan_id)
    }

    /// Java `IdManager.getNextId()` for a clan (D-39: the Rust range).
    fn allocate_clan_id(&self) -> Option<i32> {
        let Some(ids) = self.npc_world.ids.as_ref() else {
            warn!("No id factory installed: a new clan cannot be created.");
            return None;
        };
        match ids.allocate() {
            Ok(id) => Some(id.0),
            Err(e) => {
                warn!("No object id for a new clan: {e}");
                None
            }
        }
    }

    /// Java `new ClanMember(clan, player)` with `setPlayer(player)`.
    pub(crate) fn member_of_player(&self, player: ObjectId) -> Option<ClanMember> {
        let p = self.players.get(&player)?;
        let state = p.data.profile.clan.unwrap_or_default();
        Some(ClanMember {
            object_id: player,
            name: p.data.name.clone(),
            title: p.data.title.clone(),
            power_grade: state.power_grade,
            level: p.core.level,
            class_id: p.data.class_id,
            female: p.data.sex != 0,
            race: p.data.race,
            pledge_type: state.pledge_type,
            apprentice: 0,
            sponsor: 0,
            has_player: true,
        })
    }

    /// Java `player.setPledgeClass(ClanMember.calculatePledgeClass(player))`, which also checks
    /// the worn items (`checkItemRestriction`).
    pub(crate) fn set_pledge_class(&mut self, player: ObjectId) {
        let state = self.player_clan_state(player);
        let noble = self.players.get(&player).is_some_and(|p| p.data.profile.noble);
        // `isHero()`: the heroes table is not loaded (Olympiad, M8).
        let pledge_class =
            calculate_pledge_class(self.clan_of(player), player.0, state.pledge_type, noble, false);
        if let Some(s) = self.player_clan_state_mut(player) {
            s.pledge_class = pledge_class;
        }
        let mut changed = Vec::new();
        self.check_item_restriction(player, &mut changed);
    }

    // ------------------------------------------------------------------ destroy

    /// Java `ClanTable.scheduleRemoveClan(clanId)`: the clan goes when its dissolving time ran
    /// out, five minutes at the soonest.
    ///
    /// NOTE(java-quirk): a timer is never cancelled. A clan that recovered and asked again is
    /// destroyed when the first timer fires (the time is set again by then).
    pub(crate) fn schedule_remove_clan(&mut self, clan_id: i32) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        let delay = (clan.dissolving_expiry_time - unix_ms_now()).max(REMOVE_CLAN_MIN_DELAY_MS);
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay).unwrap_or(u64::MAX));
        self.timers
            .schedule_at(due, TimerKind::Clan(ClanTimer::RemoveClan(clan_id)));
    }

    /// The task of `scheduleRemoveClan`.
    pub(crate) fn on_remove_clan_timer(&mut self, clan_id: i32) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        if clan.dissolving_expiry_time != 0 {
            // D-49: the rows of a scheduled dissolution are left to the Java game server unless
            // `ClanMaintenanceDatabaseWrites` is on.
            let write = self.clans.settings.maintenance_database_writes;
            self.destroy_clan(clan_id, write);
        }
    }

    /// Java `ClanTable.destroyClan(clanId)`. `write_db` false keeps every row as it is (D-49):
    /// the clan goes from this world and its online members leave it; their own rows follow with
    /// their next store.
    pub fn destroy_clan(&mut self, clan_id: i32, write_db: bool) {
        let Some(clan) = self.clan(clan_id) else {
            return;
        };
        self.broadcast_to_clan(clan_id, &SystemMessage::new(SystemMessageId::ClanHasDispersed));
        let castle_id = clan.castle_id;
        // Sieges, fortress sieges, siegable halls and the clan hall auction: M7.
        // M6-CLAN-03: `clan.getWarehouse().destroyAllItems(...)`.
        let members: Vec<ObjectId> = clan.members().iter().map(|m| m.object_id).collect();
        for member in members {
            self.remove_clan_member(clan_id, member, 0, write_db);
        }
        let leader = self
            .clans
            .clans
            .remove(&clan_id)
            .and_then(|c| c.leader_member().map(|m| m.name.clone()));
        // `IdManager.releaseId(clanId)`: only an id of the Rust range goes back.
        if let Some(ids) = self.npc_world.ids.as_ref()
            && ids.contains(ObjectId(clan_id))
        {
            ids.release(ObjectId(clan_id));
        }
        if write_db {
            self.clan_write(ClanWrite::Delete { clan_id, castle_id });
        }
        // Fortress and siegable hall owners: M7.
        info!(
            "ClanTable: Clan {clan_id} destroyed (leader {}).",
            leader.unwrap_or_default()
        );
    }

    // ------------------------------------------------------------------ shutdown, weekly

    /// Java `ClanTable.shutdown()`: `updateClanInDB` of every clan. D-49: without
    /// `ClanMaintenanceDatabaseWrites` only the clans this server changed are written.
    pub(crate) fn clan_table_shutdown(&mut self) {
        let all = self.clans.settings.maintenance_database_writes;
        let ids: Vec<i32> = self
            .clans
            .clans
            .values()
            .filter(|c| all || c.changed)
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.update_clan_in_db(id);
        }
    }

    /// Java `DailyResetManager.clanLeaderApply` (the Wednesday part of the daily reset): the
    /// leader changes asked at a village master happen. D-49: a job of every clan, so it runs only
    /// with `ClanMaintenanceDatabaseWrites` (the Java game server applies them otherwise; this
    /// server sees the new leaders at its next start).
    pub fn clan_leader_apply(&mut self) {
        if !self.clans.settings.maintenance_database_writes {
            return;
        }
        let ids: Vec<i32> = self.clans.clans.keys().copied().collect();
        for id in ids {
            let Some(new_leader) = self.clan(id).map(|c| c.new_leader_id) else {
                continue;
            };
            if new_leader == 0 {
                continue;
            }
            if self
                .clan(id)
                .and_then(|c| c.member(ObjectId(new_leader)))
                .is_none()
            {
                continue;
            }
            self.set_new_leader(id, ObjectId(new_leader));
        }
        info!("Clan leaders have been updated.");
    }
}

/// `Map.put(skill.getId(), skill)`: a known id keeps its place.
pub(crate) fn put_skill(skills: &mut Vec<(i32, i32)>, id: i32, level: i32) -> Option<(i32, i32)> {
    match skills.iter_mut().find(|s| s.0 == id) {
        Some(existing) => {
            let old = *existing;
            existing.1 = level;
            Some(old)
        }
        None => {
            skills.push((id, level));
            None
        }
    }
}
