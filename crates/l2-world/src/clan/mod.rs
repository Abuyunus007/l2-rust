//! Clans: the clan table, members, sub pledges, ranks and privileges, clan level, reputation,
//! the clan notice and the clan window packets (M6-CLAN-01).
// Java: entity/clan/{Clan,ClanMember,ClanPrivileges,ClanAccess,ClanInfo}.java
// Java: data/sql/ClanTable.java
// Java: network/clientpackets/{RequestJoinPledge,RequestAnswerJoinPledge,RequestWithdrawalPledge,
//       RequestOustPledgeMember,RequestPledge*}.java
// Java: entity/actor/instance/VillageMaster.java (the clan commands of the village masters)
// Java: network/holders/RequestPartner.java
// Design: docs/design/m6-social.md
//
// # Shape
// The clans live in [`ClanWorld`] (`World::clans`), keyed by id like Java's `ClanTable`. They are
// loaded at startup (`World::load_clans`, from the rows the game server read, D-15) and changed
// only by the world. Every row Java writes goes out as `PersistCommand::Clan` with Java's own
// statement ([`ClanWrite`]); the clan columns of an online member's `characters` row are written
// by the normal store (`PlayerProfile::clan`, Java `storeCharBase`).
//
// # Side by side with the Java game server (D-39, D-49)
// Both servers keep the clans in memory and write the same rows. A change made on one server is
// seen by the other at its next start (design note). The jobs that write many rows on their own
// (the dissolution of a clan whose time ran out, the Wednesday leader change, the start-up
// alliance check and the shutdown save of every clan) run their database part only with
// `ClanMaintenanceDatabaseWrites = True` in `config/SideBySide.ini` (D-49); without it the world
// changes its own state and leaves the rows to the Java game server, and at shutdown only the
// clans this server changed are saved.
//
// # Left to other cards
// - Crests, alliances, clan wars (M6-CLAN-02): their packets reach `ClanInput::NotPorted`; the
//   ally and war columns and `clan_wars` are loaded.
// - Clan skills on the members and the academy graduation (M6-CLAN-03): the skills are loaded and
//   listed (`PledgeSkillList`), marked `// M6-CLAN-03:` where they act. The clan warehouse
//   (`Clan::warehouse`) is M6-WH-01, see `warehouse/`.
// - Castles, forts, clan halls, sieges (M7): their ids stay as loaded (fort and hall 0).
// - The `ClanMaster` village master script (scripts come last, D-05) that shows the pages with
//   the clan commands; the commands themselves are here (`village_master.rs`).

mod handlers;
mod level;
mod members;
pub mod model;
mod request;
mod table;
mod view;
mod village_master;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

use l2_util::ObjectId;

use crate::world::World;

pub use model::{
    COMPLETE_PRIVILEGES_MASK, Clan, ClanAccess, ClanMember, ClanPrivilegesRef, PlayerClanState, RankPrivs,
    SubPledge, calculate_pledge_class,
};
pub use request::{RequestPacket, RequestPartner};

/// The configuration values the clan code reads (`Player.ini`, `Feature.ini`, `Server.ini`,
/// `SideBySide.ini`).
#[derive(Clone)]
pub struct ClanSettings {
    /// `DaysBeforeJoinAClan`.
    pub alt_clan_join_days: i32,
    /// `DaysBeforeCreateAClan`.
    pub alt_clan_create_days: i32,
    /// `DaysToPassToDissolveAClan`.
    pub alt_clan_dissolve_days: i32,
    /// `AltClanLeaderInstantActivation`.
    pub alt_clan_leader_instant_activation: bool,
    /// `CreateRoyalGuardCost`.
    pub royal_guard_cost: i32,
    /// `CreateKnightUnitCost`.
    pub knight_unit_cost: i32,
    /// `ClanLevel6Cost` .. `ClanLevel11Cost` (index 0 is level 6).
    pub clan_level_costs: [i32; 6],
    /// `ClanLevel6Requirement` .. `ClanLevel11Requirement` (members).
    pub clan_level_requirements: [i32; 6],
    /// `ClanNameTemplate` (Java `CLAN_NAME_TEMPLATE_PATTERN.matcher(name).matches()`).
    pub clan_name_template: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    /// `ClanMaintenanceDatabaseWrites` of `SideBySide.ini` (D-49): the clan jobs that write rows
    /// on their own run their database part.
    pub maintenance_database_writes: bool,
}

impl Default for ClanSettings {
    /// Java's defaults; the name template accepts everything (`.*`).
    fn default() -> Self {
        Self {
            alt_clan_join_days: 1,
            alt_clan_create_days: 10,
            alt_clan_dissolve_days: 7,
            alt_clan_leader_instant_activation: false,
            royal_guard_cost: 5000,
            knight_unit_cost: 10000,
            clan_level_costs: [5000, 10000, 20000, 40000, 40000, 75000],
            clan_level_requirements: [30, 50, 80, 120, 140, 170],
            clan_name_template: Arc::new(|_| true),
            maintenance_database_writes: false,
        }
    }
}

impl fmt::Debug for ClanSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClanSettings")
            .field("alt_clan_join_days", &self.alt_clan_join_days)
            .field("alt_clan_create_days", &self.alt_clan_create_days)
            .field("alt_clan_dissolve_days", &self.alt_clan_dissolve_days)
            .field(
                "alt_clan_leader_instant_activation",
                &self.alt_clan_leader_instant_activation,
            )
            .field("royal_guard_cost", &self.royal_guard_cost)
            .field("knight_unit_cost", &self.knight_unit_cost)
            .field("clan_level_costs", &self.clan_level_costs)
            .field("clan_level_requirements", &self.clan_level_requirements)
            .field("maintenance_database_writes", &self.maintenance_database_writes)
            .finish_non_exhaustive()
    }
}

/// The clan state of the world (Java `ClanTable` and the `RequestPartner` of every player).
#[derive(Debug, Default)]
pub struct ClanWorld {
    /// Java `ClanTable._clans`. A `BTreeMap` so every walk has a fixed order (Java walks a hash
    /// map; nothing visible depends on that order here).
    pub(crate) clans: BTreeMap<i32, Clan>,
    pub(crate) settings: ClanSettings,
    /// Java `Player.getRequest()` of the players that have one going.
    pub(crate) requests: HashMap<ObjectId, RequestPartner>,
}

/// The clan timers (one `TimerKind::Clan`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClanTimer {
    /// `ClanTable.scheduleRemoveClan`'s task.
    RemoveClan(i32),
    /// `ClanTable.updateClanRanks`, every 20 minutes.
    UpdateRanks,
    /// `RequestPartner.clear` 15 seconds after a request.
    RequestClear(ObjectId),
}

/// The clan packets of the client, decoded by the session (`ClientInput::Clan`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClanInput {
    /// `RequestJoinPledge`.
    JoinPledge { target: i32, pledge_type: i32 },
    /// `RequestAnswerJoinPledge`.
    AnswerJoinPledge { answer: i32 },
    /// `RequestWithdrawalPledge`.
    WithdrawalPledge,
    /// `RequestOustPledgeMember`.
    OustPledgeMember { target: String },
    /// `RequestPledgeMemberList`.
    PledgeMemberList,
    /// `RequestPledgeInfo`.
    PledgeInfo { clan_id: i32 },
    /// `RequestPledgeExtendedInfo` (Java does nothing with it).
    PledgeExtendedInfo,
    /// `RequestPledgePower`.
    PledgePower { rank: i32, action: i32, privs: i32 },
    /// `RequestPledgePowerGradeList`.
    PledgePowerGradeList,
    /// `RequestPledgeMemberPowerInfo`.
    PledgeMemberPowerInfo { player: String },
    /// `RequestPledgeMemberInfo`.
    PledgeMemberInfo { player: String },
    /// `RequestPledgeSetMemberPowerGrade`.
    PledgeSetMemberPowerGrade { member: String, power_grade: i32 },
    /// `RequestPledgeReorganizeMember`.
    PledgeReorganizeMember {
        is_member_selected: i32,
        member_name: String,
        new_pledge_type: i32,
        selected_member: String,
    },
    /// `RequestPledgeSetAcademyMaster`.
    PledgeSetAcademyMaster {
        set: i32,
        curr_player_name: String,
        target_player_name: String,
    },
    /// The crest, alliance and war packets: M6-CLAN-02. The name of the Java packet.
    NotPorted(&'static str),
}

/// A clan row write, with Java's statement (`PersistCommand::Clan`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClanWrite {
    /// `Clan.store()`: `INSERT_CLAN_DATA`.
    Insert(ClanRow),
    /// `Clan.updateClanInDB()`.
    Update(ClanUpdateRow),
    /// `Clan.changeLevel`: `UPDATE clan_data SET clan_level = ? WHERE clan_id = ?`.
    Level { clan_id: i32, level: i32 },
    /// `Clan.changeAllyCrest(crestId, true)`: `UPDATE clan_data SET ally_crest_id = ? WHERE
    /// clan_id = ?`.
    AllyCrestOfClan { clan_id: i32, crest_id: i32 },
    /// `Clan.removeMemberInDatabase` (three statements).
    RemoveMember {
        char_id: i32,
        clan_join_expiry_time: i64,
        clan_create_expiry_time: i64,
    },
    /// `UPDATE characters SET clan_privs = ? WHERE charId = ?` (`setNewLeader`).
    CharClanPrivs { char_id: i32, privs: i32 },
    /// `ClanMember.updatePledgeType`.
    CharSubPledge { char_id: i32, pledge_type: i32 },
    /// `ClanMember.updatePowerGrade`.
    CharPowerGrade { char_id: i32, power_grade: i32 },
    /// `ClanMember.saveApprenticeAndSponsor`.
    CharApprenticeSponsor {
        char_id: i32,
        apprentice: i32,
        sponsor: i32,
    },
    /// `Clan.createSubPledge`'s `INSERT INTO clan_subpledges`.
    InsertSubPledge {
        clan_id: i32,
        sub_pledge_id: i32,
        name: String,
        leader_id: i32,
    },
    /// `Clan.updateSubPledgeInDB`.
    UpdateSubPledge {
        clan_id: i32,
        sub_pledge_id: i32,
        name: String,
        leader_id: i32,
    },
    /// `Clan.setRankPrivs`: `REPLACE INTO clan_privs`.
    RankPrivs { clan_id: i32, rank: i32, privs: i32 },
    /// `Clan.storeNotice`.
    Notice {
        clan_id: i32,
        notice: String,
        enabled: bool,
    },
    /// `ClanTable.destroyClan`: the six `DELETE`s and, with a castle, its tax reset.
    Delete { clan_id: i32, castle_id: i32 },
}

/// The values of `INSERT_CLAN_DATA`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanRow {
    pub clan_id: i32,
    pub name: String,
    pub level: i32,
    pub castle_id: i32,
    pub blood_alliance_count: i32,
    pub blood_oath_count: i32,
    pub ally_id: i32,
    pub ally_name: Option<String>,
    pub leader_id: i32,
    pub crest_id: i32,
    pub crest_large_id: i32,
    pub ally_crest_id: i32,
    pub new_leader_id: i32,
}

/// The values of `updateClanInDB`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanUpdateRow {
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

/// One clan as the game server read it at startup (`new Clan(clanId)`'s queries).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClanRecord {
    pub clan_id: i32,
    /// The `clan_data` row; `None` when it vanished between the id scan and the read.
    pub data: Option<ClanDataRecord>,
    /// `SELECT ... FROM characters WHERE clanid=?`, in the database's order.
    pub members: Vec<ClanMemberRecord>,
    /// `clan_subpledges`: `(sub_pledge_id, name, leader_id)`.
    pub sub_pledges: Vec<(i32, String, i32)>,
    /// `clan_privs`: `(rank, party, privs)`.
    pub rank_privs: Vec<(i32, i32, i32)>,
    /// `clan_skills`: `(skill_id, skill_level, sub_pledge_id)`.
    pub skills: Vec<(i32, i32, i32)>,
    /// `clan_notices`: `(enabled text, notice)`.
    pub notices: Vec<(String, String)>,
    /// `items` with `loc = 'CLANWH'` and the clan as owner (Java `_warehouse.restore()`).
    pub warehouse: Vec<crate::player::ItemData>,
}

/// The `clan_data` columns (`getInt` of NULL is 0, `getString` of NULL is `None`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClanDataRecord {
    pub name: Option<String>,
    pub level: i32,
    pub reputation_score: i32,
    pub castle_id: i32,
    pub blood_alliance_count: i32,
    pub blood_oath_count: i32,
    pub ally_id: i32,
    pub ally_name: Option<String>,
    pub leader_id: i32,
    pub crest_id: i32,
    pub crest_large_id: i32,
    pub ally_crest_id: i32,
    pub auction_bid_at: i32,
    pub ally_penalty_expiry_time: i64,
    pub ally_penalty_type: i32,
    pub char_penalty_expiry_time: i64,
    pub dissolving_expiry_time: i64,
    pub new_leader_id: i32,
}

/// One member row (`new ClanMember(clan, resultSet)`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClanMemberRecord {
    pub object_id: i32,
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    pub title: String,
    pub power_grade: i32,
    pub pledge_type: i32,
    pub apprentice: i32,
    pub sponsor: i32,
    pub female: bool,
    pub race: i32,
}

/// What `ClanMember` answers for a member: the player's live values while it is linked, the
/// stored ones otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemberInfo {
    pub object_id: ObjectId,
    pub name: String,
    pub level: i32,
    pub class_id: i32,
    pub title: String,
    pub pledge_type: i32,
    pub power_grade: i32,
    pub female: bool,
    pub race: i32,
    pub apprentice: i32,
    pub sponsor: i32,
    /// Java `isOnline()`.
    pub online: bool,
}

impl World {
    /// Installs the clan configuration. Call before the clans are loaded.
    pub fn set_clan_settings(&mut self, settings: ClanSettings) {
        self.clans.settings = settings;
    }

    /// A clan by id (Java `ClanTable.getClan`).
    pub fn clan(&self, clan_id: i32) -> Option<&Clan> {
        self.clans.clans.get(&clan_id)
    }

    pub(crate) fn clan_mut(&mut self, clan_id: i32) -> Option<&mut Clan> {
        self.clans.clans.get_mut(&clan_id)
    }

    /// Every clan (Java `ClanTable.getClans`), by id.
    pub fn clans(&self) -> impl Iterator<Item = &Clan> {
        self.clans.clans.values()
    }

    /// Java `ClanTable.getClanByName`: ignoring case.
    pub fn clan_by_name(&self, name: &str) -> Option<&Clan> {
        self.clans
            .clans
            .values()
            .find(|c| model::java_equals_ignore_case(&c.name, name))
    }

    /// The clan columns of a player (all zero without a clan or a loaded state).
    pub fn player_clan_state(&self, id: ObjectId) -> PlayerClanState {
        self.players
            .get(&id)
            .and_then(|p| p.data.profile.clan)
            .unwrap_or_default()
    }

    pub(crate) fn player_clan_state_mut(&mut self, id: ObjectId) -> Option<&mut PlayerClanState> {
        let player = self.players.get_mut(&id)?;
        Some(player.data.profile.clan.get_or_insert_with(Default::default))
    }

    /// Java `Player.getClanId()`.
    pub fn clan_id_of(&self, id: ObjectId) -> i32 {
        self.player_clan_state(id).clan_id
    }

    /// Java `Player.getClan()`: the clan of an online player.
    pub fn clan_of(&self, id: ObjectId) -> Option<&Clan> {
        match self.clan_id_of(id) {
            0 => None,
            clan_id => self.clan(clan_id),
        }
    }

    /// Java `Player.isClanLeader()`.
    pub fn is_clan_leader(&self, id: ObjectId) -> bool {
        self.clan_of(id).is_some_and(|c| c.leader == Some(id))
    }

    /// Java `Player.getAllyId()`.
    pub fn ally_id_of(&self, id: ObjectId) -> i32 {
        self.clan_of(id).map_or(0, |c| c.ally_id)
    }

    /// Java `Player.getClanCrestId()`.
    pub fn clan_crest_id_of(&self, id: ObjectId) -> i32 {
        self.clan_of(id).map_or(0, |c| c.crest_id)
    }

    /// Java `Player.getClanCrestLargeId()`: only a clan with a castle or a clan hall shows it.
    pub fn clan_crest_large_id_of(&self, id: ObjectId) -> i32 {
        self.clan_of(id)
            .filter(|c| c.castle_id != 0 || c.hideout_id != 0)
            .map_or(0, |c| c.crest_large_id)
    }

    /// Java `Player.getAllyCrestId()`.
    pub fn ally_crest_id_of(&self, id: ObjectId) -> i32 {
        match self.clan_of(id) {
            Some(c) if c.ally_id != 0 => c.ally_crest_id,
            _ => 0,
        }
    }

    /// Java `Player.getClanPrivileges().getMask()`.
    pub fn clan_privileges_of(&self, id: ObjectId) -> i32 {
        let state = self.player_clan_state(id);
        self.resolve_privileges(state.clan_id, state.privileges)
    }

    /// The mask of a privileges object: an own mask, or the clan's rank (0 when the clan or the
    /// rank is gone).
    pub(crate) fn resolve_privileges(&self, clan_id: i32, privileges: ClanPrivilegesRef) -> i32 {
        match privileges {
            ClanPrivilegesRef::Own(mask) => mask,
            ClanPrivilegesRef::Rank(rank) => self.clan(clan_id).map_or(0, |c| c.rank_privs(rank)),
        }
    }

    /// Java `Player.hasAccess(access)`.
    pub fn has_clan_access(&self, id: ObjectId, access: ClanAccess) -> bool {
        model::has_access(self.clan_privileges_of(id), access)
    }

    /// Java `getClanPrivileges().setMask(mask)` (also `enableAll`, `disableAll`): a shared rank
    /// object changes for everybody holding it (see [`ClanPrivilegesRef`]).
    pub(crate) fn set_player_privileges_mask(&mut self, id: ObjectId, mask: i32) {
        let state = self.player_clan_state(id);
        match state.privileges {
            ClanPrivilegesRef::Rank(rank) => {
                if let Some(rp) = self.clan_mut(state.clan_id).and_then(|c| c.rank_privs_mut(rank)) {
                    rp.privs = mask;
                }
            }
            ClanPrivilegesRef::Own(_) => {
                if let Some(s) = self.player_clan_state_mut(id) {
                    s.privileges = ClanPrivilegesRef::Own(mask);
                }
            }
        }
    }

    /// Java `Player.isAtWarWith(target)`: both in a clan, neither in the academy, and the
    /// player's clan declared war on the target's.
    pub fn is_at_war_with(&self, player: ObjectId, target: ObjectId) -> bool {
        match (self.clan_of(player), self.clan_of(target)) {
            (Some(mine), Some(theirs))
                if !self.is_academy_member(player) && !self.is_academy_member(target) =>
            {
                mine.is_at_war_with(theirs.id)
            }
            _ => false,
        }
    }

    /// Java `Player.isInClanWith(target)`.
    pub fn is_in_clan_with(&self, player: ObjectId, target: ObjectId) -> bool {
        let (mine, theirs) = (self.clan_id_of(player), self.clan_id_of(target));
        mine != 0 && theirs != 0 && mine == theirs
    }

    /// Java `Player.isInAllyWith(target)`.
    pub fn is_in_ally_with(&self, player: ObjectId, target: ObjectId) -> bool {
        let (mine, theirs) = (self.ally_id_of(player), self.ally_id_of(target));
        mine != 0 && theirs != 0 && mine == theirs
    }

    /// Java `Player.getPledgeClass()`.
    pub fn pledge_class_of(&self, id: ObjectId) -> i32 {
        self.player_clan_state(id).pledge_class
    }

    /// Java `Player.isAcademyMember()`.
    pub fn is_academy_member(&self, id: ObjectId) -> bool {
        self.player_clan_state(id).lvl_joined_academy > 0
    }

    /// Java `ClanMember` getters for one member.
    pub(crate) fn member_info(&self, member: &ClanMember) -> MemberInfo {
        let player = member
            .has_player
            .then(|| self.players.get(&member.object_id))
            .flatten();
        match player {
            Some(p) => {
                let state = p.data.profile.clan.unwrap_or_default();
                MemberInfo {
                    object_id: member.object_id,
                    name: p.data.name.clone(),
                    level: p.core.level,
                    class_id: p.data.class_id,
                    title: p.data.title.clone(),
                    pledge_type: state.pledge_type,
                    power_grade: state.power_grade,
                    female: p.data.sex != 0,
                    race: p.data.race,
                    apprentice: state.apprentice,
                    sponsor: state.sponsor,
                    // `_player.isOnline() && !_player.isInOfflineMode()` (no offline shops yet).
                    online: true,
                }
            }
            None => MemberInfo {
                object_id: member.object_id,
                name: member.name.clone(),
                level: member.level,
                class_id: member.class_id,
                title: member.title.clone(),
                pledge_type: member.pledge_type,
                power_grade: member.power_grade,
                female: member.female,
                race: member.race,
                apprentice: member.apprentice,
                sponsor: member.sponsor,
                online: false,
            },
        }
    }

    /// Java `Clan.getClanMember(name)`: the first member in map order whose name is `name`
    /// (case sensitive).
    pub(crate) fn clan_member_by_name(&self, clan_id: i32, name: &str) -> Option<MemberInfo> {
        let clan = self.clan(clan_id)?;
        clan.members()
            .into_iter()
            .map(|m| self.member_info(m))
            .find(|m| m.name == name)
    }

    /// Java `Clan.getClanMember(objectId)`.
    pub(crate) fn clan_member_info(&self, clan_id: i32, id: ObjectId) -> Option<MemberInfo> {
        let clan = self.clan(clan_id)?;
        clan.member(id).map(|m| self.member_info(m))
    }

    /// Java `Clan.getMembers()` with each member's answers, in map order.
    pub(crate) fn clan_members_info(&self, clan_id: i32) -> Vec<MemberInfo> {
        self.clan(clan_id)
            .map(|c| c.members().into_iter().map(|m| self.member_info(m)).collect())
            .unwrap_or_default()
    }

    /// Java `Clan.getOnlineMembers(exclude)` (map order).
    pub(crate) fn clan_online_members(&self, clan_id: i32, exclude: i32) -> Vec<ObjectId> {
        self.clan_members_info(clan_id)
            .into_iter()
            .filter(|m| m.online && m.object_id.0 != exclude)
            .map(|m| m.object_id)
            .collect()
    }

    /// Java `Clan.getOnlineMembersCount()`.
    pub fn clan_online_members_count(&self, clan_id: i32) -> usize {
        self.clan_online_members(clan_id, 0).len()
    }

    /// Java `Clan.getSubPledgeMembersCount(pledgeType)` (an online member's own pledge type).
    pub(crate) fn sub_pledge_members_count(&self, clan_id: i32, pledge_type: i32) -> usize {
        self.clan_members_info(clan_id)
            .iter()
            .filter(|m| m.pledge_type == pledge_type)
            .count()
    }

    /// Java `Clan.getLeaderName()`: empty (with Java's warning) when the leader is not a member.
    pub(crate) fn clan_leader_name(&self, clan_id: i32) -> String {
        let Some(clan) = self.clan(clan_id) else {
            return String::new();
        };
        match clan.leader_member() {
            Some(m) => self.member_info(m).name,
            None => {
                tracing::warn!("Clan: Clan {} without clan leader!", clan.name);
                String::new()
            }
        }
    }

    /// The clan timers.
    pub(crate) fn on_clan_timer(&mut self, timer: ClanTimer) {
        match timer {
            ClanTimer::RemoveClan(clan_id) => self.on_remove_clan_timer(clan_id),
            ClanTimer::UpdateRanks => self.on_update_clan_ranks_timer(),
            ClanTimer::RequestClear(player) => self.clear_request(player),
        }
    }

    /// The clan packets of a player in the world.
    pub(crate) fn on_clan_input(&mut self, id: ObjectId, input: ClanInput) {
        match input {
            ClanInput::JoinPledge { target, pledge_type } => {
                self.on_request_join_pledge(id, target, pledge_type)
            }
            ClanInput::AnswerJoinPledge { answer } => self.on_request_answer_join_pledge(id, answer),
            ClanInput::WithdrawalPledge => self.on_request_withdrawal_pledge(id),
            ClanInput::OustPledgeMember { target } => self.on_request_oust_pledge_member(id, &target),
            ClanInput::PledgeMemberList => self.on_request_pledge_member_list(id),
            ClanInput::PledgeInfo { clan_id } => self.on_request_pledge_info(id, clan_id),
            // Java reads the name and does nothing.
            ClanInput::PledgeExtendedInfo => {}
            ClanInput::PledgePower { rank, action, privs } => {
                self.on_request_pledge_power(id, rank, action, privs)
            }
            ClanInput::PledgePowerGradeList => self.on_request_pledge_power_grade_list(id),
            ClanInput::PledgeMemberPowerInfo { player } => {
                self.on_request_pledge_member_power_info(id, &player)
            }
            ClanInput::PledgeMemberInfo { player } => self.on_request_pledge_member_info(id, &player),
            ClanInput::PledgeSetMemberPowerGrade { member, power_grade } => {
                self.on_request_pledge_set_member_power_grade(id, &member, power_grade)
            }
            ClanInput::PledgeReorganizeMember {
                is_member_selected,
                member_name,
                new_pledge_type,
                selected_member,
            } => self.on_request_pledge_reorganize_member(
                id,
                is_member_selected,
                &member_name,
                new_pledge_type,
                &selected_member,
            ),
            ClanInput::PledgeSetAcademyMaster {
                set,
                curr_player_name,
                target_player_name,
            } => self.on_request_pledge_set_academy_master(id, set, &curr_player_name, &target_player_name),
            ClanInput::NotPorted(packet) => {
                tracing::debug!("{id}: {packet} is not ported yet (M6-CLAN-02).");
            }
        }
    }

    /// Hands a clan row write to the persistence task.
    pub(crate) fn clan_write(&mut self, write: ClanWrite) {
        self.outbox.persist(crate::outbox::PersistCommand::Clan(write));
    }

    /// Java `Clan.updateClanInDB()`.
    pub(crate) fn update_clan_in_db(&mut self, clan_id: i32) {
        let Some(clan) = self.clan_mut(clan_id) else {
            return;
        };
        clan.changed = false;
        let row = ClanUpdateRow {
            clan_id: clan.id,
            leader_id: clan.leader_id(),
            ally_id: clan.ally_id,
            ally_name: clan.ally_name.clone(),
            reputation_score: clan.reputation_score,
            ally_penalty_expiry_time: clan.ally_penalty_expiry_time,
            ally_penalty_type: clan.ally_penalty_type,
            char_penalty_expiry_time: clan.char_penalty_expiry_time,
            dissolving_expiry_time: clan.dissolving_expiry_time,
            new_leader_id: clan.new_leader_id,
        };
        self.clan_write(ClanWrite::Update(row));
    }
}

/// Java `int days * 86400000`: an int product, so it wraps for 25 days and more.
pub(crate) fn java_int_days_ms(days: i32) -> i64 {
    // NOTE(java-quirk): `PlayerConfig.ALT_CLAN_..._DAYS * 86400000` is computed in `int`.
    i64::from(days.wrapping_mul(86_400_000))
}

/// Java `TimeUnit.DAYS.toMillis(days)` (in `long`).
pub(crate) fn days_ms(days: i32) -> i64 {
    i64::from(days).saturating_mul(86_400_000)
}
