//! The clan data: a clan, its members, sub pledges and rank privileges, and the clan part of a
//! player. Plain data with the small rules that only read it.
// Java: entity/clan/Clan.java (fields, getMaxNrOfMembers, getAvailablePledgeTypes,
//       getLeaderSubPledge, getSubPledgeMembersCount, SubPledge, RankPrivs)
// Java: entity/clan/ClanMember.java (fields, calculatePledgeClass)
// Java: entity/clan/ClanAccess.java, entity/clan/ClanPrivileges.java
//
// # Ids, not references (D-11)
// Java's `ClanMember` holds the `Player` while the member is online and reads the live values
// from it. Here a member keeps its stored values and `has_player` (Java `_player != null`); the
// world reads the player when it is set (`World::member_info`). A player holds its clan by id
// (`PlayerClanState::clan_id`).
//
// # Java's hash map order
// The members (`_members`), sub pledges (`_subPledges`) and rank privileges (`_privs`) are
// `ConcurrentHashMap<Integer, _>`s, and the packets list them in that map's order. The lists here
// keep insertion order and [`java_map_order`] gives Java's order (the table only grows, so the
// size it reached is kept with the list).

use l2_util::ObjectId;

/// Java `Clan.SUBUNIT_ACADEMY`.
pub const SUBUNIT_ACADEMY: i32 = -1;
/// Java `Clan.SUBUNIT_ROYAL1`.
pub const SUBUNIT_ROYAL1: i32 = 100;
/// Java `Clan.SUBUNIT_ROYAL2`.
pub const SUBUNIT_ROYAL2: i32 = 200;
/// Java `Clan.SUBUNIT_KNIGHT1`.
pub const SUBUNIT_KNIGHT1: i32 = 1001;
/// Java `Clan.SUBUNIT_KNIGHT2`.
pub const SUBUNIT_KNIGHT2: i32 = 1002;
/// Java `Clan.SUBUNIT_KNIGHT3`.
pub const SUBUNIT_KNIGHT3: i32 = 2001;
/// Java `Clan.SUBUNIT_KNIGHT4`.
pub const SUBUNIT_KNIGHT4: i32 = 2002;

/// Java `Clan.PENALTY_TYPE_CLAN_LEAVED`.
pub const PENALTY_TYPE_CLAN_LEAVED: i32 = 1;
/// Java `Clan.PENALTY_TYPE_CLAN_DISMISSED`.
pub const PENALTY_TYPE_CLAN_DISMISSED: i32 = 2;
/// Java `Clan.PENALTY_TYPE_DISMISS_CLAN`.
pub const PENALTY_TYPE_DISMISS_CLAN: i32 = 3;
/// Java `Clan.PENALTY_TYPE_DISSOLVE_ALLY`.
pub const PENALTY_TYPE_DISSOLVE_ALLY: i32 = 4;

/// Java `Clan.MAX_NOTICE_LENGTH`.
pub const MAX_NOTICE_LENGTH: usize = 8192;

/// The `sub_pledge_id` of a clan skill in `clan_skills` (Java `addNewSkill(skill)` uses `-2`).
pub const CLAN_SKILL_SUB_TYPE: i32 = -2;

/// Java `ClanAccess`: the privilege bits, `1 << ordinal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ClanAccess {
    None = 0,
    InviteMember,
    AssignTitle,
    AccessWarehouse,
    ModifyRanks,
    WarDeclaration,
    RemoveMember,
    ChangeCrest,
    DismissMentee,
    MemberFame,
    AccessAirship,
    HallOpenDoor,
    HallFunctions,
    HallAuction,
    HallBanish,
    HallManageFunctions,
    CastleOpenDoor,
    CastleManor,
    CastleSiege,
    CastleFunctions,
    CastleBanish,
    CastleVault,
    CastleMercenaries,
    CastleManageFunctions,
}

impl ClanAccess {
    /// Java `ClanAccess.values().length`.
    pub const COUNT: u32 = 24;

    /// Java `getMask()`: `1 << ordinal()`.
    pub const fn mask(self) -> i32 {
        1 << (self as u8)
    }
}

/// Java `ClanPrivileges.getCompleteMask()`: every bit of [`ClanAccess`].
pub const COMPLETE_PRIVILEGES_MASK: i32 = (1 << ClanAccess::COUNT) - 1;

/// Java `ClanPrivileges.hasMinimumPrivileges(minimum)`.
pub const fn has_access(mask: i32, access: ClanAccess) -> bool {
    mask & access.mask() != 0
}

/// Which `ClanPrivileges` object a player holds.
///
/// NOTE(java-quirk): Java hands a member the clan's own `ClanPrivileges` object of its rank
/// (`setClanPrivileges(clan.getRankPrivs(grade))` at login and when joining), not a copy. Whatever
/// changes the player's privileges then changes the rank for everybody: `setNewLeader` calls
/// `enableAll()` on the new leader's object, so the rank the new leader had gets every privilege
/// (in memory, until the next restart). `Rank` keeps that sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClanPrivilegesRef {
    /// An object of the player's own (`new ClanPrivileges()`, `clone()`, `enableAll()` on a new
    /// object).
    Own(i32),
    /// The clan's `RankPrivs` object of this rank.
    Rank(i32),
}

impl Default for ClanPrivilegesRef {
    fn default() -> Self {
        Self::Own(0)
    }
}

/// The clan columns of a character (`characters`) and the clan values Java keeps on `Player`.
///
/// Loaded with the character and written by the normal store (Java `storeCharBase`); `None` in
/// `PlayerProfile::clan` means a snapshot that does not carry them (the store then keeps the
/// row's values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerClanState {
    /// `characters.clanid` (Java `_clanId`).
    pub clan_id: i32,
    /// `characters.clan_privs` (Java `_clanPrivileges`). A snapshot for the store always holds
    /// [`ClanPrivilegesRef::Own`] with the mask of the moment.
    pub privileges: ClanPrivilegesRef,
    /// `characters.wantspeace`.
    pub wants_peace: i32,
    /// `characters.power_grade`.
    pub power_grade: i32,
    /// `characters.subpledge` (Java `_pledgeType`).
    pub pledge_type: i32,
    /// `characters.lvl_joined_academy`.
    pub lvl_joined_academy: i32,
    /// `characters.apprentice`.
    pub apprentice: i32,
    /// `characters.sponsor`.
    pub sponsor: i32,
    /// `characters.clan_join_expiry_time` (epoch ms).
    pub clan_join_expiry_time: i64,
    /// `characters.clan_create_expiry_time` (epoch ms).
    pub clan_create_expiry_time: i64,
    /// Java `_pledgeClass` (the title above the CP bar). Not stored.
    pub pledge_class: i32,
}

impl PlayerClanState {
    /// The `clan_privs` value a store writes: the mask of an own object (a snapshot resolves the
    /// shared rank object first, see `World::store_snapshot`).
    pub fn stored_privileges(&self) -> i32 {
        match self.privileges {
            ClanPrivilegesRef::Own(mask) => mask,
            ClanPrivilegesRef::Rank(_) => 0,
        }
    }
}

/// One clan member (Java `ClanMember`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClanMember {
    pub object_id: ObjectId,
    pub name: String,
    pub title: String,
    pub power_grade: i32,
    pub level: i32,
    pub class_id: i32,
    pub female: bool,
    /// `getRace().ordinal()`.
    pub race: i32,
    pub pledge_type: i32,
    pub apprentice: i32,
    pub sponsor: i32,
    /// Java `_player != null`: the member's player is in the world and linked to it. While set,
    /// Java's getters read the player, not the fields above.
    pub has_player: bool,
}

/// A sub pledge (Java `Clan.SubPledge`): the academy, the royal guards, the knights.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubPledge {
    pub id: i32,
    pub name: String,
    pub leader_id: i32,
    /// `_subPledgeSkills` (a `HashMap<Integer, Skill>`): `(skill id, level)` in insertion order.
    pub skills: Vec<(i32, i32)>,
}

/// The privileges of a rank (Java `Clan.RankPrivs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankPrivs {
    pub rank: i32,
    /// Java `_party`, always 0 ("TODO: Find out what this stuff means").
    pub party: i32,
    pub privs: i32,
}

/// A clan (Java `Clan`). The world keeps them in `ClanWorld`, keyed by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clan {
    pub id: i32,
    pub name: String,
    pub level: i32,
    pub castle_id: i32,
    /// Set by the fortress manager (M7); 0 until then.
    pub fort_id: i32,
    /// Set by the clan hall tables (M7); 0 until then.
    pub hideout_id: i32,
    pub blood_alliance_count: i32,
    pub blood_oath_count: i32,
    pub ally_id: i32,
    /// `null` in Java when there is none.
    pub ally_name: Option<String>,
    pub ally_penalty_expiry_time: i64,
    pub ally_penalty_type: i32,
    pub char_penalty_expiry_time: i64,
    pub dissolving_expiry_time: i64,
    pub crest_id: i32,
    pub crest_large_id: i32,
    pub ally_crest_id: i32,
    pub auction_bid_at: i32,
    pub reputation_score: i32,
    pub rank: i32,
    pub new_leader_id: i32,
    /// Java `_leader`: the leader's object id, `None` when the leader is not among the members
    /// (`getLeaderId()` is 0 then).
    pub leader: Option<ObjectId>,
    /// `_members` in insertion order.
    pub(crate) members: Vec<ClanMember>,
    /// The most members the map ever held (Java's table never shrinks).
    pub(crate) members_peak: usize,
    /// `_atWarWith`: the clans this clan declared war on.
    pub at_war_with: Vec<i32>,
    /// `_atWarAttackers`: the clans that declared war on this clan.
    pub at_war_attackers: Vec<i32>,
    /// `_skills`: `(skill id, level)` in insertion order.
    pub skills: Vec<(i32, i32)>,
    /// `_subPledgeSkills` (the skills of the main clan's squad, `sub_pledge_id = 0`).
    pub sub_pledge_skills: Vec<(i32, i32)>,
    /// `_privs` in insertion order.
    pub(crate) privs: Vec<RankPrivs>,
    /// `_subPledges` in insertion order.
    pub(crate) sub_pledges: Vec<SubPledge>,
    /// `_notice` (`null` until loaded or set).
    pub notice: Option<String>,
    pub notice_enabled: bool,
    /// Something `updateClanInDB` writes changed since the load (the reputation above all: Java
    /// writes it only with the next `updateClanInDB`, at the latest at shutdown).
    pub(crate) changed: bool,
}

impl Clan {
    /// Java `new Clan(clanId, clanName)`: a new clan with the nine ranks (`initializePrivs`).
    pub fn new(id: i32, name: &str) -> Self {
        Self {
            id,
            name: name.to_owned(),
            level: 0,
            castle_id: 0,
            fort_id: 0,
            hideout_id: 0,
            blood_alliance_count: 0,
            blood_oath_count: 0,
            ally_id: 0,
            ally_name: None,
            ally_penalty_expiry_time: 0,
            ally_penalty_type: 0,
            char_penalty_expiry_time: 0,
            dissolving_expiry_time: 0,
            crest_id: 0,
            crest_large_id: 0,
            ally_crest_id: 0,
            auction_bid_at: 0,
            reputation_score: 0,
            rank: 0,
            new_leader_id: 0,
            leader: None,
            members: Vec::new(),
            members_peak: 0,
            at_war_with: Vec::new(),
            at_war_attackers: Vec::new(),
            skills: Vec::new(),
            sub_pledge_skills: Vec::new(),
            // Java: initializePrivs, ranks 1 to 9 with no privileges.
            privs: (1..10)
                .map(|rank| RankPrivs {
                    rank,
                    party: 0,
                    privs: 0,
                })
                .collect(),
            sub_pledges: Vec::new(),
            notice: None,
            notice_enabled: false,
            changed: false,
        }
    }

    /// Java `getLeaderId()`.
    pub fn leader_id(&self) -> i32 {
        self.leader.map_or(0, |l| l.0)
    }

    /// The leader's member entry (Java `getLeader()`).
    pub fn leader_member(&self) -> Option<&ClanMember> {
        self.leader.and_then(|id| self.member(id))
    }

    /// Java `getClanMember(objectId)`.
    pub fn member(&self, id: ObjectId) -> Option<&ClanMember> {
        self.members.iter().find(|m| m.object_id == id)
    }

    pub(crate) fn member_mut(&mut self, id: ObjectId) -> Option<&mut ClanMember> {
        self.members.iter_mut().find(|m| m.object_id == id)
    }

    /// Java `isMember(id)`.
    pub fn is_member(&self, id: i32) -> bool {
        id != 0 && self.members.iter().any(|m| m.object_id.0 == id)
    }

    /// Java `getMembersCount()`.
    pub fn members_count(&self) -> usize {
        self.members.len()
    }

    /// Java `getMembers()` in the map's iteration order.
    pub fn members(&self) -> Vec<&ClanMember> {
        let keys: Vec<i32> = self.members.iter().map(|m| m.object_id.0).collect();
        java_map_order(&keys, self.members_peak)
            .into_iter()
            .filter_map(|i| self.members.get(i))
            .collect()
    }

    /// Java `_members.put(id, member)`: a known id keeps its place in the map.
    pub(crate) fn put_member(&mut self, member: ClanMember) {
        match self.members.iter_mut().find(|m| m.object_id == member.object_id) {
            Some(existing) => *existing = member,
            None => {
                self.members.push(member);
                self.members_peak = self.members_peak.max(self.members.len());
            }
        }
    }

    /// Java `_members.remove(id)`.
    pub(crate) fn take_member(&mut self, id: ObjectId) -> Option<ClanMember> {
        let index = self.members.iter().position(|m| m.object_id == id)?;
        Some(self.members.remove(index))
    }

    /// Java `getSubPledgeMembersCount(pledgeType)`, with the stored pledge types (the world's
    /// version reads the online members' players).
    pub fn sub_pledge_members_count(&self, pledge_type: i32) -> usize {
        self.members
            .iter()
            .filter(|m| m.pledge_type == pledge_type)
            .count()
    }

    /// Java `getMaxNrOfMembers(pledgeType)`.
    pub fn max_members(&self, pledge_type: i32) -> usize {
        max_members(self.level, pledge_type)
    }

    /// Java `getSubPledge(pledgeType)`.
    pub fn sub_pledge(&self, pledge_type: i32) -> Option<&SubPledge> {
        self.sub_pledges.iter().find(|s| s.id == pledge_type)
    }

    pub(crate) fn sub_pledge_mut(&mut self, pledge_type: i32) -> Option<&mut SubPledge> {
        self.sub_pledges.iter_mut().find(|s| s.id == pledge_type)
    }

    /// Java `getSubPledge(pledgeName)`: the first one in map order whose name matches, ignoring
    /// case.
    pub fn sub_pledge_by_name(&self, name: &str) -> Option<&SubPledge> {
        self.all_sub_pledges()
            .into_iter()
            .find(|s| java_equals_ignore_case(&s.name, name))
    }

    /// Java `getAllSubPledges()` in the map's order.
    pub fn all_sub_pledges(&self) -> Vec<&SubPledge> {
        let keys: Vec<i32> = self.sub_pledges.iter().map(|s| s.id).collect();
        java_map_order(&keys, keys.len())
            .into_iter()
            .filter_map(|i| self.sub_pledges.get(i))
            .collect()
    }

    /// Java `_subPledges.put(type, pledge)`.
    pub(crate) fn put_sub_pledge(&mut self, pledge: SubPledge) {
        match self.sub_pledges.iter_mut().find(|s| s.id == pledge.id) {
            Some(existing) => *existing = pledge,
            None => self.sub_pledges.push(pledge),
        }
    }

    /// Java `getAvailablePledgeTypes(pledgeType)`: the next free unit of the kind asked for, 0
    /// when there is none.
    pub fn available_pledge_types(&self, pledge_type: i32) -> i32 {
        if self.sub_pledge(pledge_type).is_some() {
            return match pledge_type {
                SUBUNIT_ACADEMY | SUBUNIT_ROYAL2 | SUBUNIT_KNIGHT4 => 0,
                SUBUNIT_ROYAL1 => self.available_pledge_types(SUBUNIT_ROYAL2),
                SUBUNIT_KNIGHT1 => self.available_pledge_types(SUBUNIT_KNIGHT2),
                SUBUNIT_KNIGHT2 => self.available_pledge_types(SUBUNIT_KNIGHT3),
                SUBUNIT_KNIGHT3 => self.available_pledge_types(SUBUNIT_KNIGHT4),
                _ => pledge_type,
            };
        }
        pledge_type
    }

    /// Java `getLeaderSubPledge(leaderId)`: the unit this member leads, 0 for none (the last one
    /// in map order when it leads several).
    pub fn leader_sub_pledge(&self, leader_id: i32) -> i32 {
        let mut id = 0;
        for sp in self.all_sub_pledges() {
            if sp.leader_id == 0 {
                continue;
            }
            if sp.leader_id == leader_id {
                id = sp.id;
            }
        }
        id
    }

    /// Java `getRankPrivs(rank).getMask()`: 0 for a rank the clan does not have (Java hands out a
    /// new empty object then).
    pub fn rank_privs(&self, rank: i32) -> i32 {
        self.privs.iter().find(|p| p.rank == rank).map_or(0, |p| p.privs)
    }

    /// Java `_privs.get(rank) != null`.
    pub fn has_rank(&self, rank: i32) -> bool {
        self.privs.iter().any(|p| p.rank == rank)
    }

    pub(crate) fn rank_privs_mut(&mut self, rank: i32) -> Option<&mut RankPrivs> {
        self.privs.iter_mut().find(|p| p.rank == rank)
    }

    /// Java `getAllRankPrivs()` in the map's order.
    pub fn all_rank_privs(&self) -> Vec<RankPrivs> {
        let keys: Vec<i32> = self.privs.iter().map(|p| p.rank).collect();
        java_map_order(&keys, keys.len())
            .into_iter()
            .filter_map(|i| self.privs.get(i).copied())
            .collect()
    }

    /// Java `_privs.put(rank, new RankPrivs(rank, 0, privs))`.
    pub(crate) fn put_rank_privs(&mut self, rank: i32, privs: i32) {
        match self.rank_privs_mut(rank) {
            Some(existing) => existing.privs = privs,
            None => self.privs.push(RankPrivs {
                rank,
                party: 0,
                privs,
            }),
        }
    }

    /// Java `isAtWar()`.
    pub fn is_at_war(&self) -> bool {
        !self.at_war_with.is_empty()
    }

    /// Java `isAtWarWith(id)`.
    pub fn is_at_war_with(&self, id: i32) -> bool {
        self.at_war_with.contains(&id)
    }

    /// Java `getAllSkills()` in the map's order: `(id, level)`.
    pub fn all_skills(&self) -> Vec<(i32, i32)> {
        let keys: Vec<i32> = self.skills.iter().map(|s| s.0).collect();
        java_map_order(&keys, keys.len())
            .into_iter()
            .filter_map(|i| self.skills.get(i).copied())
            .collect()
    }

    /// Java `getAllSubSkills()`: the squad skills of the main clan, then those of each sub pledge
    /// (map order): `(sub type, id, level)`.
    pub fn all_sub_skills(&self) -> Vec<(i32, i32, i32)> {
        let mut list = Vec::new();
        let keys: Vec<i32> = self.sub_pledge_skills.iter().map(|s| s.0).collect();
        for i in java_map_order(&keys, keys.len()) {
            if let Some((id, level)) = self.sub_pledge_skills.get(i) {
                list.push((0, *id, *level));
            }
        }
        for sub in self.all_sub_pledges() {
            // `SubPledge._subPledgeSkills` is a `HashMap`: the same bucket order for a small one.
            let keys: Vec<i32> = sub.skills.iter().map(|s| s.0).collect();
            for i in java_hash_map_order(&keys) {
                if let Some((id, level)) = sub.skills.get(i) {
                    list.push((sub.id, *id, *level));
                }
            }
        }
        list
    }

    /// Java `getNotice()`: empty without a notice, empty when it looks like a bypass exploit,
    /// otherwise the text without tags.
    pub fn notice_text(&self) -> String {
        let Some(notice) = &self.notice else {
            return String::new();
        };
        // Bypass exploit check.
        let text = notice.to_lowercase();
        if text.contains("action") && text.contains("bypass") {
            return String::new();
        }
        // Returns text without tags (`replaceAll("<.*?>", "")`: `.` does not match a line end).
        strip_tags(notice)
    }
}

/// Java `getMaxNrOfMembers(pledgeType)` for a clan of `level`.
pub fn max_members(level: i32, pledge_type: i32) -> usize {
    match pledge_type {
        0 => match level {
            3 => 30,
            2 => 20,
            1 => 15,
            0 => 10,
            _ => 40,
        },
        -1 => 20,
        100 | 200 => match level {
            11 => 30,
            _ => 20,
        },
        1001 | 1002 | 2001 | 2002 => match level {
            9..=11 => 25,
            _ => 10,
        },
        _ => 0,
    }
}

/// Java `ClanMember.calculatePledgeClass(player)` for a player with this clan state.
/// `clan` is `None` without a clan.
pub fn calculate_pledge_class(
    clan: Option<&Clan>,
    object_id: i32,
    pledge_type: i32,
    noble: bool,
    hero: bool,
) -> i32 {
    let mut pledge_class = 0;
    if let Some(clan) = clan {
        let leader = clan.leader_id() == object_id && object_id != 0;
        // The member's own unit, then the unit it leads: `(academy, royal, knight, main member,
        // royal captain, knight captain, leader)` per level.
        let table =
            |academy, royal, knight, member, royal_captain, knight_captain, leader_class| match pledge_type {
                -1 => academy,
                100 | 200 => royal,
                1001 | 1002 | 2001 | 2002 => knight,
                0 => {
                    if leader {
                        leader_class
                    } else {
                        match clan.leader_sub_pledge(object_id) {
                            100 | 200 => royal_captain,
                            1001 | 1002 | 2001 | 2002 => knight_captain,
                            _ => member,
                        }
                    }
                }
                _ => 0,
            };
        pledge_class = match clan.level {
            4 => {
                if leader {
                    3
                } else {
                    0
                }
            }
            5 => {
                if leader {
                    4
                } else {
                    2
                }
            }
            // NOTE(java-quirk): at level 6 a knight (no knights below level 7) is class 0, and the
            // captain of a knight order counts as a plain member (3).
            6 => table(1, 2, 0, 3, 4, 3, 5),
            7 => table(1, 3, 2, 4, 6, 5, 7),
            8 => table(1, 4, 3, 5, 7, 6, 8),
            9 => table(1, 5, 4, 6, 8, 7, 9),
            10 => table(1, 6, 5, 7, 9, 8, 10),
            11 => table(1, 7, 6, 8, 10, 9, 11),
            _ => 1,
        };
    }
    if noble && pledge_class < 5 {
        pledge_class = 5;
    }
    if hero && pledge_class < 8 {
        pledge_class = 8;
    }
    pledge_class
}

/// The iteration order of a `ConcurrentHashMap<Integer, _>` whose keys are `keys` (insertion
/// order, no duplicates) and that once held `peak` entries: by bucket `spread(h) & (n - 1)`,
/// then insertion order. Returns indices into `keys`.
// Java: the table starts at 16 and doubles when the count reaches `n - n/4`; it never shrinks.
// A resize can reorder the entries of one bucket (`lastRun`); ignored, like in `skills/reuse.rs`.
pub fn java_map_order(keys: &[i32], peak: usize) -> Vec<usize> {
    let mut capacity: usize = 16;
    while peak.max(keys.len()) >= capacity - capacity / 4 {
        capacity *= 2;
    }
    bucket_order(keys, capacity, true)
}

/// The iteration order of a `HashMap<Integer, _>` holding `keys` (resized at `0.75 n`).
pub fn java_hash_map_order(keys: &[i32]) -> Vec<usize> {
    let mut capacity: usize = 16;
    while keys.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    bucket_order(keys, capacity, false)
}

fn bucket_order(keys: &[i32], capacity: usize, concurrent: bool) -> Vec<usize> {
    let bucket = |k: i32| {
        let h = k as u32;
        let spread = h ^ (h >> 16);
        // `ConcurrentHashMap.spread` also clears the sign bit (`& HASH_BITS`).
        let spread = if concurrent { spread & 0x7fff_ffff } else { spread };
        (spread as usize) & (capacity - 1)
    };
    let mut indexed: Vec<(usize, usize)> = keys.iter().enumerate().map(|(i, &k)| (bucket(k), i)).collect();
    indexed.sort_unstable();
    indexed.into_iter().map(|(_, i)| i).collect()
}

/// Java `String.equalsIgnoreCase`.
pub(crate) fn java_equals_ignore_case(a: &str, b: &str) -> bool {
    a.chars().count() == b.chars().count()
        && a.chars().zip(b.chars()).all(|(x, y)| {
            x == y || x.to_uppercase().eq(y.to_uppercase()) || x.to_lowercase().eq(y.to_lowercase())
        })
}

/// `text.replaceAll("<.*?>", "")`: every shortest `<...>` on one line goes.
fn strip_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        let after = &rest[start + 1..];
        // `.` stops at a line terminator, so a tag never spans lines.
        let line_end = after.find(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}']);
        match after.find('>') {
            Some(end) if line_end.is_none_or(|le| end < le) => {
                out.push_str(&rest[..start]);
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&rest[..=start]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Java `StringUtil.isAlphaNumeric(text)`: not empty and every UTF-16 unit a letter or a digit
/// (same as the character screen's `is_alpha_numeric`).
///
/// NOTE: Java asks `Character.isLetterOrDigit(char)`; Rust's `is_alphabetic`/`is_numeric` agree
/// for the characters players type, and differ only for rare marks and number symbols.
pub(crate) fn is_alpha_numeric(text: &str) -> bool {
    !text.is_empty()
        && text.encode_utf16().all(|unit| {
            // A surrogate half is not a letter on its own in Java.
            char::from_u32(u32::from(unit)).is_some_and(|c| c.is_alphabetic() || c.is_numeric())
        })
}

/// Java `String.length()`: UTF-16 code units.
pub(crate) fn java_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clan_at(level: i32) -> Clan {
        let mut clan = Clan::new(1, "Clan");
        clan.level = level;
        clan.leader = Some(ObjectId(10));
        clan.put_sub_pledge(SubPledge {
            id: SUBUNIT_ROYAL1,
            name: "Royal".into(),
            leader_id: 20,
            skills: Vec::new(),
        });
        clan.put_sub_pledge(SubPledge {
            id: SUBUNIT_KNIGHT1,
            name: "Knight".into(),
            leader_id: 30,
            skills: Vec::new(),
        });
        clan
    }

    #[test]
    fn the_complete_mask_has_24_bits() {
        assert_eq!(COMPLETE_PRIVILEGES_MASK, 0x00FF_FFFF);
        assert_eq!(ClanAccess::InviteMember.mask(), 2);
        assert_eq!(ClanAccess::RemoveMember.mask(), 64);
        assert_eq!(ClanAccess::CastleManageFunctions.mask(), 1 << 23);
    }

    #[test]
    fn pledge_classes_follow_calculate_pledge_class() {
        // Level 3 and below: everybody is 1.
        assert_eq!(calculate_pledge_class(Some(&clan_at(3)), 10, 0, false, false), 1);
        // Level 4: the leader 3, the others 0; level 5: 4 and 2.
        assert_eq!(calculate_pledge_class(Some(&clan_at(4)), 10, 0, false, false), 3);
        assert_eq!(calculate_pledge_class(Some(&clan_at(4)), 11, 0, false, false), 0);
        assert_eq!(calculate_pledge_class(Some(&clan_at(5)), 10, 0, false, false), 4);
        assert_eq!(calculate_pledge_class(Some(&clan_at(5)), 11, 0, false, false), 2);
        // Level 8: leader, royal captain, knight captain, member, royal, knight, academy.
        let clan = clan_at(8);
        assert_eq!(calculate_pledge_class(Some(&clan), 10, 0, false, false), 8);
        assert_eq!(calculate_pledge_class(Some(&clan), 20, 0, false, false), 7);
        assert_eq!(calculate_pledge_class(Some(&clan), 30, 0, false, false), 6);
        assert_eq!(calculate_pledge_class(Some(&clan), 40, 0, false, false), 5);
        assert_eq!(calculate_pledge_class(Some(&clan), 40, 100, false, false), 4);
        assert_eq!(calculate_pledge_class(Some(&clan), 40, 1001, false, false), 3);
        assert_eq!(calculate_pledge_class(Some(&clan), 40, -1, false, false), 1);
        // Level 6 quirk: a knight is 0, a knight captain a plain member.
        let clan = clan_at(6);
        assert_eq!(calculate_pledge_class(Some(&clan), 40, 1001, false, false), 0);
        assert_eq!(calculate_pledge_class(Some(&clan), 30, 0, false, false), 3);
        // Nobles and heroes have a floor, also without a clan.
        assert_eq!(calculate_pledge_class(None, 40, 0, true, false), 5);
        assert_eq!(calculate_pledge_class(None, 40, 0, false, true), 8);
        assert_eq!(calculate_pledge_class(None, 40, 0, false, false), 0);
    }

    #[test]
    fn member_limits_follow_get_max_nr_of_members() {
        assert_eq!(max_members(0, 0), 10);
        assert_eq!(max_members(3, 0), 30);
        assert_eq!(max_members(4, 0), 40);
        assert_eq!(max_members(5, -1), 20);
        assert_eq!(max_members(11, 100), 30);
        assert_eq!(max_members(10, 200), 20);
        assert_eq!(max_members(9, 2002), 25);
        assert_eq!(max_members(8, 1001), 10);
        assert_eq!(max_members(8, 7), 0);
    }

    #[test]
    fn available_pledge_types_walk_to_the_next_free_unit() {
        let mut clan = Clan::new(1, "C");
        assert_eq!(clan.available_pledge_types(SUBUNIT_ROYAL1), SUBUNIT_ROYAL1);
        for id in [SUBUNIT_ROYAL1, SUBUNIT_KNIGHT1, SUBUNIT_KNIGHT2] {
            clan.put_sub_pledge(SubPledge {
                id,
                name: id.to_string(),
                leader_id: 0,
                skills: Vec::new(),
            });
        }
        assert_eq!(clan.available_pledge_types(SUBUNIT_ROYAL1), SUBUNIT_ROYAL2);
        assert_eq!(clan.available_pledge_types(SUBUNIT_KNIGHT1), SUBUNIT_KNIGHT3);
        clan.put_sub_pledge(SubPledge {
            id: SUBUNIT_ACADEMY,
            name: "A".into(),
            leader_id: 0,
            skills: Vec::new(),
        });
        assert_eq!(clan.available_pledge_types(SUBUNIT_ACADEMY), 0);
    }

    #[test]
    fn sub_pledges_come_in_concurrent_hash_map_order() {
        let mut clan = Clan::new(1, "C");
        for id in [100, 200, 1001, 1002, 2001, 2002, -1] {
            clan.put_sub_pledge(SubPledge {
                id,
                name: id.to_string(),
                leader_id: 0,
                skills: Vec::new(),
            });
        }
        let order: Vec<i32> = clan.all_sub_pledges().iter().map(|s| s.id).collect();
        // Buckets of 16: -1 -> 0 (the sign bit is cleared), 2001 -> 1, 2002 -> 2, 100 -> 4,
        // 200 -> 8, 1001 -> 9, 1002 -> 10.
        assert_eq!(order, [-1, 2001, 2002, 100, 200, 1001, 1002]);
    }

    #[test]
    fn the_notice_loses_its_tags_and_bypass_links() {
        let mut clan = Clan::new(1, "C");
        assert_eq!(clan.notice_text(), "");
        clan.notice = Some("<font color=\"LEVEL\">Hi</font> all <b".into());
        assert_eq!(clan.notice_text(), "Hi all <b");
        clan.notice = Some("<a action=\"bypass -h x\">x</a>".into());
        assert_eq!(clan.notice_text(), "");
        clan.notice = Some("a <b\n> c".into());
        assert_eq!(clan.notice_text(), "a <b\n> c");
    }

    #[test]
    fn names_compare_like_java() {
        assert!(java_equals_ignore_case("Royal", "rOYAL"));
        assert!(!java_equals_ignore_case("Royal", "Roya"));
        assert!(is_alpha_numeric("Clan42"));
        assert!(!is_alpha_numeric("Clan 42"));
        assert!(!is_alpha_numeric(""));
        assert_eq!(java_len("ab"), 2);
    }
}
