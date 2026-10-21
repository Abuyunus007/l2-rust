//! The PvP flag, karma, PK and PvP kill counts, the relation masks of `RelationChanged` and
//! `isAutoAttackable` between players (M6-PVP-01).
// Java: entity/actor/Player.java (updatePvPStatus (both), updatePvPFlag, startPvPFlag,
//       stopPvPFlag, stopPvpRegTask, onKillUpdatePvPKarma, increasePvpKills,
//       increasePkKillsAndKarma, updatePvpTitleAndColor, setKarma, setKarmaFlag, broadcastKarma,
//       broadcastTitleInfo, getRelation, isAutoAttackable)
// Java: entity/actor/Playable.java (checkIfPvP)
// Java: taskmanagers/PvpFlagTaskManager.java
// Java: managers/AntiFeedManager.java (check, setLastDeathTime)
// Java: mechanics/stats/Formulas.java (calculateKarmaGain)
// Java: network/serverpackets/{RelationChanged,NicknameChanged}.java
//
// # State
// The flag (`Player._pvpFlag`), the time it lasts to (`_pvpFlagLasts`) and the timer standing in
// for `PvpFlagTaskManager`'s set are one [`PvpState`] per player in `World::pvp`. Karma, PK and
// PvP kills are the player's own columns (`PlayerData::{karma, pk_kills, pvp_kills}`, which the
// store already writes as Java does).
//
// # `PvpFlagTaskManager`
// Java walks one global set every second. Here a flagged player has its own 1 s timer
// (`TimerKind::PvpFlag`) that does what one pass of the manager does for that player; the
// player enters the "set" in [`World::start_pvp_flag`] and leaves it in
// [`World::stop_pvp_reg_task`]. The only difference is the phase of the second (Java's pass is
// not aligned with the moment the player was added). The clock is the world's
// (`World::world_ms`), not `System.currentTimeMillis()`.
//
// # Relations
// [`relation_mask`] is `Player.getRelation` over plain inputs ([`RelationInput`]); the world fills
// the inputs from what exists. Clans, wars and sieges are other cards: until they exist their
// inputs are empty (`// M6-CLAN:`, `// M7:` in [`World::relation_input`]),
// so those bits stay 0 and the card that adds the state only has to fill the input.

use std::collections::HashMap;

use l2_protocol_game::server::visibility::{relation, status as attr};
use l2_protocol_game::server::{NicknameChanged, RelationChanged, RelationEntry, StatusUpdate};
use l2_util::ObjectId;

use crate::movement::Intention;
use crate::npc::NpcClass;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// `PvpFlagTaskManager` runs every second.
const PVP_FLAG_TASK_MS: u64 = 1000;
/// The flag blinks (value 2) during the last 20 seconds.
const PVP_FLAG_BLINK_MS: i64 = 20_000;

/// The `PvP.ini` values and the `Custom/PvpTitleColor.ini` ones this module reads. `Default` is
/// the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvpSettings {
    /// `PvpConfig.PVP_NORMAL_TIME` (`PvPVsNormalTime`), ms.
    pub pvp_normal_time_ms: i32,
    /// `PvpConfig.PVP_PVP_TIME` (`PvPVsPvPTime`), ms.
    pub pvp_pvp_time_ms: i32,
    /// `PvpConfig.FLAG_PLAYER_ON_RAID_ATTACK` (`FlagPlayerOnRaidAttack`).
    pub flag_player_on_raid_attack: bool,
    /// `PvpConfig.FLAG_PLAYER_ON_CHAMPION_ATTACK` (`FlagPlayerOnChampionAttack`).
    pub flag_player_on_champion_attack: bool,
    /// `PvpConfig.FLAG_PLAYER_ON_SUMMON_PET_RAID_CHAMPION_ATTACK`
    /// (`FlagPlayerOnSummonPetRaidChampionAttack`).
    pub flag_player_on_summon_pet_raid_champion_attack: bool,
    /// `PvpConfig.KARMA_AWARD_PK_KILL` (`AwardPKKillPVPPoint`).
    pub karma_award_pk_kill: bool,
    /// `PvpConfig.ANTIFEED_ENABLE` (`AntiFeedEnable`).
    pub antifeed_enable: bool,
    /// `PvpConfig.ANTIFEED_DUALBOX` (`AntiFeedDualbox`).
    pub antifeed_dualbox: bool,
    /// `PvpConfig.ANTIFEED_DISCONNECTED_AS_DUALBOX` (`AntiFeedDisconnectedAsDualbox`).
    pub antifeed_disconnected_as_dualbox: bool,
    /// `PvpConfig.ANTIFEED_INTERVAL` (`AntiFeedInterval` seconds, kept in ms).
    pub antifeed_interval_ms: i64,
    /// `FactionSystemConfig.FACTION_SYSTEM_ENABLED`: the PvP title colours give way to the
    /// faction ones. The factions themselves are not modelled.
    pub faction_system_enabled: bool,
    /// `PvpTitleColorConfig` (`Custom/PvpTitleColor.ini`).
    pub title_color: PvpTitleColorSettings,
}

impl Default for PvpSettings {
    fn default() -> Self {
        Self {
            pvp_normal_time_ms: 120_000,
            pvp_pvp_time_ms: 60_000,
            flag_player_on_raid_attack: true,
            flag_player_on_champion_attack: true,
            flag_player_on_summon_pet_raid_champion_attack: true,
            karma_award_pk_kill: false,
            antifeed_enable: false,
            antifeed_dualbox: true,
            antifeed_disconnected_as_dualbox: true,
            antifeed_interval_ms: 120_000,
            faction_system_enabled: false,
            title_color: PvpTitleColorSettings::default(),
        }
    }
}

/// `PvpTitleColorConfig`: five tiers of PvP kills, each with a title and a colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvpTitleColorSettings {
    /// `PVP_COLOR_SYSTEM_ENABLED` (`EnablePvPColorSystem`).
    pub enabled: bool,
    /// `PVP_AMOUNT1..5` (`PvpAmount1..5`).
    pub amounts: [i32; 5],
    /// `NAME_COLOR_FOR_PVP_AMOUNT1..5` (`ColorForAmount1..5`, `Integer.decode("0x" + text)`).
    pub colors: [i32; 5],
    /// `TITLE_FOR_PVP_AMOUNT1..5` (`PvPTitleForAmount1..5`).
    pub titles: [String; 5],
}

impl Default for PvpTitleColorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            amounts: [500, 1000, 1500, 2500, 5000],
            colors: [0x00FF00; 5],
            titles: std::array::from_fn(|_| "Title".to_string()),
        }
    }
}

/// One player's flag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PvpState {
    /// `Player._pvpFlag`: 0 none, 1 flagged, 2 flagged and about to end (the name blinks).
    flag: i32,
    /// `Player._pvpFlagLasts`, in `World::world_ms`.
    lasts_ms: i64,
    /// The player's entry in `PvpFlagTaskManager`: its next pass.
    timer: Option<TimerId>,
}

/// The world's PvP tables.
#[derive(Debug, Default)]
pub(crate) struct PvpWorld {
    players: HashMap<ObjectId, PvpState>,
    /// `AntiFeedManager._lastDeathTimes` (world ms). Never forgotten, like Java's map.
    last_death_ms: HashMap<ObjectId, i64>,
}

// ------------------------------------------------------------------ formulas

/// Java `Formulas.calculateKarmaGain(pkCount, isSummon)`: the karma a killer gets for a kill.
// NOTE(java-quirk): for a summon the first result is overwritten by the `pkCount` branches below
// it, so only the cap of 10800 (before them) and `pkCount >= 180` (43200) are ever reached
// through the summon formula.
pub fn calculate_karma_gain(pk_count: i32, is_summon: bool) -> i32 {
    let pk = f64::from(pk_count);
    let mut result = 43_200;
    if is_summon {
        result = ((((pk * 0.375) + 1.0) * 60.0) * 4.0) as i32 - 150;
        if result > 10_800 {
            return 10_800;
        }
    }
    if pk_count < 99 {
        result = ((((pk * 0.5) + 1.0) * 60.0) * 12.0) as i32;
    } else if pk_count < 180 {
        result = ((((pk * 0.125) + 37.75) * 60.0) * 12.0) as i32;
    }
    result
}

// ------------------------------------------------------------------ relations

/// A clan war between the subject's clan and the viewer's (`Player.getRelation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClanWar {
    /// The viewer's clan declared war on the subject's, not the other way round.
    OneSided,
    /// Both clans are at war with each other.
    Mutual,
}

/// What `Player.getRelation(target)` reads, for the subject (`this`) and the viewer (`target`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelationInput {
    /// `_clan != null`.
    pub has_clan: bool,
    /// `getClan() == target.getClan()`.
    pub same_clan: bool,
    /// `getAllyId() != 0`.
    pub has_ally: bool,
    /// `isClanLeader()`.
    pub is_clan_leader: bool,
    /// The subject's position in its party's member list, when it and the viewer are in the same
    /// party (`getParty() == target.getParty()`).
    pub party_index: Option<usize>,
    /// `_siegeState`.
    pub siege_state: i32,
    /// `TerritoryWarManager.getRegisteredTerritoryId(this) != 0`.
    pub territory_war: bool,
    /// `getSiegeState() != target.getSiegeState()`.
    pub siege_state_differs: bool,
    /// The war between the clans, when the viewer's clan is at war with the subject's and neither
    /// is an academy member.
    pub clan_war: Option<ClanWar>,
}

/// Java `Player.getRelation(target)` (without the Handy's Block Checker arena, an event).
pub fn relation_mask(input: &RelationInput) -> i32 {
    let mut result = 0;
    if input.has_clan {
        result |= relation::CLAN_MEMBER;
        if input.same_clan {
            result |= relation::CLAN_MATE;
        }
        if input.has_ally {
            result |= relation::ALLY_MEMBER;
        }
    }
    if input.is_clan_leader {
        result |= relation::LEADER;
    }
    if let Some(index) = input.party_index {
        result |= relation::HAS_PARTY;
        result |= match index {
            0 => relation::PARTYLEADER,
            1 => relation::PARTY4,
            2 => relation::PARTY3 + relation::PARTY2 + relation::PARTY1,
            3 => relation::PARTY3 + relation::PARTY2,
            4 => relation::PARTY3 + relation::PARTY1,
            5 => relation::PARTY3,
            6 => relation::PARTY2 + relation::PARTY1,
            7 => relation::PARTY2,
            8 => relation::PARTY1,
            _ => 0,
        };
    }
    if input.siege_state != 0 {
        if input.territory_war {
            result |= relation::TERRITORY_WAR;
        } else {
            result |= relation::INSIEGE;
            if input.siege_state_differs {
                result |= relation::ENEMY;
            } else {
                result |= relation::ALLY;
            }
            if input.siege_state == 1 {
                result |= relation::ATTACKER;
            }
        }
    }
    if let Some(war) = input.clan_war {
        result |= relation::ONE_SIDED_WAR;
        if war == ClanWar::Mutual {
            result |= relation::MUTUAL_WAR;
        }
    }
    result
}

impl World {
    // -------------------------------------------------------------- the flag

    /// Java `Player.getPvpFlag()`.
    pub fn pvp_flag_of(&self, id: ObjectId) -> i32 {
        self.pvp.players.get(&id).map_or(0, |s| s.flag)
    }

    /// Java `Player.getPvpFlagLasts()` (world ms).
    pub fn pvp_flag_lasts(&self, id: ObjectId) -> i64 {
        self.pvp.players.get(&id).map_or(0, |s| s.lasts_ms)
    }

    /// Java `Player.setPvpFlagLasts(time)`.
    pub fn set_pvp_flag_lasts(&mut self, id: ObjectId, time_ms: i64) {
        if self.players.contains_key(&id) {
            self.pvp.players.entry(id).or_default().lasts_ms = time_ms;
        }
    }

    /// Java `Player.startPvPFlag()`: flag 1 and into `PvpFlagTaskManager`.
    pub fn start_pvp_flag(&mut self, id: ObjectId) {
        self.update_pvp_flag(id, 1);
        self.pvp_flag_task_add(id);
    }

    /// Java `Player.stopPvpRegTask()`: out of `PvpFlagTaskManager`.
    pub fn stop_pvp_reg_task(&mut self, id: ObjectId) {
        if let Some(timer) = self.pvp.players.get_mut(&id).and_then(|s| s.timer.take()) {
            self.timers.cancel(timer);
        }
    }

    /// Java `Player.stopPvPFlag()`.
    pub fn stop_pvp_flag(&mut self, id: ObjectId) {
        self.stop_pvp_reg_task(id);
        self.update_pvp_flag(id, 0);
    }

    /// Java `Player.updatePvPFlag(value)`: the player gets its `UserInfo` and everybody who sees
    /// it a `RelationChanged` with the new flag.
    pub fn update_pvp_flag(&mut self, id: ObjectId, value: i32) {
        if !self.players.contains_key(&id) || self.pvp_flag_of(id) == value {
            return;
        }
        self.pvp.players.entry(id).or_default().flag = value;
        self.send_user_info(id);
        // If this player has a pet update the pets pvp flag as well: no summons yet (M6-SUMMON).
        self.send_relation_to_visible_players(id);
    }

    /// `PvpFlagTaskManager.add(player)`: a set, so a player already in it keeps its timer.
    fn pvp_flag_task_add(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        if self.pvp.players.get(&id).is_some_and(|s| s.timer.is_some()) {
            return;
        }
        self.schedule_pvp_flag_pass(id);
    }

    fn schedule_pvp_flag_pass(&mut self, id: ObjectId) {
        let due = self.tick + TimerWheel::ms_to_ticks(PVP_FLAG_TASK_MS);
        let timer = self.timers.schedule_at(due, TimerKind::PvpFlag(id));
        self.pvp.players.entry(id).or_default().timer = Some(timer);
    }

    /// One pass of `PvpFlagTaskManager.run` over one player.
    pub(crate) fn on_pvp_flag_timer(&mut self, id: ObjectId) {
        let Some(state) = self.pvp.players.get_mut(&id) else {
            return;
        };
        state.timer = None;
        let lasts = state.lasts_ms;
        let now = self.world_ms();
        if now > lasts {
            self.stop_pvp_flag(id);
        } else if now > lasts - PVP_FLAG_BLINK_MS {
            self.update_pvp_flag(id, 2);
            self.schedule_pvp_flag_pass(id);
        } else {
            self.update_pvp_flag(id, 1);
            self.schedule_pvp_flag_pass(id);
        }
    }

    /// The player is gone (`Player.stopAllTimers` -> `stopPvpRegTask`, and the object with it).
    pub(crate) fn forget_pvp(&mut self, id: ObjectId) {
        self.stop_pvp_reg_task(id);
        self.pvp.players.remove(&id);
    }

    /// Java `Player.updatePvPStatus()`: flag the player for the normal time unless it stands in a
    /// PvP zone.
    pub fn update_pvp_status(&mut self, id: ObjectId) {
        if self.is_inside_zone(id, ZoneId::Pvp) {
            return;
        }
        let lasts = self.world_ms() + i64::from(self.settings.pvp.pvp_normal_time_ms);
        self.set_pvp_flag_lasts(id, lasts);
        if self.pvp_flag_of(id) == 0 {
            self.start_pvp_flag(id);
        }
    }

    /// The player a creature acts for (Java `asPlayer()`): the player itself.
    // M6-SUMMON: a servitor or pet gives its owner.
    fn acting_player_of(&self, id: ObjectId) -> Option<ObjectId> {
        self.players.contains_key(&id).then_some(id)
    }

    /// Java `Player.updatePvPStatus(target)`: attacking another player outside a PvP zone flags the
    /// attacker, for the longer time if the target is flagged too.
    pub fn update_pvp_status_target(&mut self, id: ObjectId, target: ObjectId) {
        let Some(target_player) = self.acting_player_of(target) else {
            return;
        };
        if id == target_player {
            return;
        }
        // The faction system (good against evil does not flag): not modelled.
        // Fighting the opponent of one's own duel does not flag.
        if self.is_in_duel(id) && self.duel_id_of(target_player) == self.duel_id_of(id) {
            return;
        }
        let both_in_pvp_zone =
            self.is_inside_zone(id, ZoneId::Pvp) && self.is_inside_zone(target, ZoneId::Pvp);
        if !both_in_pvp_zone && self.karma_of_player(target_player) == 0 {
            let time = if self.check_if_pvp(id, target_player) {
                self.settings.pvp.pvp_pvp_time_ms
            } else {
                self.settings.pvp.pvp_normal_time_ms
            };
            let lasts = self.world_ms() + i64::from(time);
            self.set_pvp_flag_lasts(id, lasts);
            if self.pvp_flag_of(id) == 0 {
                self.start_pvp_flag(id);
            }
        }
    }

    /// Java `Playable.checkIfPvP(target)`: both players without karma and the target flagged.
    pub fn check_if_pvp(&self, this: ObjectId, target: ObjectId) -> bool {
        if this == target {
            return false;
        }
        // The target must be playable; the active player is `this` (or a summon's owner).
        let (Some(player), Some(target_player)) =
            (self.acting_player_of(this), self.acting_player_of(target))
        else {
            return false;
        };
        if self.karma_of_player(player) != 0 {
            return false;
        }
        if target_player == player {
            return false;
        }
        if self.karma_of_player(target_player) != 0 {
            return false;
        }
        self.pvp_flag_of(target_player) != 0
    }

    pub(crate) fn karma_of_player(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.data.karma)
    }

    // -------------------------------------------------------------- karma, PK, PvP kills

    /// Java `Player.setKarma(value)`: never negative; guards wake when the player turns chaotic;
    /// when the karma ends everybody around gets the new relation; then `broadcastKarma()`.
    // ON_PLAYER_KARMA_CHANGED: script event (D-05).
    pub fn set_karma(&mut self, id: ObjectId, value: i32) {
        let karma = value.max(0);
        let old = self.karma_of_player(id);
        if old == 0 && karma > 0 {
            self.wake_idle_guards_around(id);
        } else if old > 0 && karma == 0 {
            // Send a Server->Client StatusUpdate packet with Karma and PvP Flag to the Player and
            // all Player to inform (broadcast).
            self.set_karma_flag(id);
        }
        // NOTE(java-quirk): `setKarmaFlag()` runs before `super.setKarma(karma)`, so its packets
        // still carry the old karma (the `UserInfo` and the `RelationChanged` of the last
        // karma point); `broadcastKarma()` below follows with the new one.
        if let Some(player) = self.players.get_mut(&id) {
            player.data.karma = karma;
        }
        self.broadcast_karma(id);
    }

    /// The `Guard` loop of `setKarma`: guards standing idle around a player who just got karma
    /// start looking for it.
    fn wake_idle_guards_around(&mut self, id: ObjectId) {
        let guards: Vec<ObjectId> = self
            .visible_objects(id)
            .into_iter()
            .filter(|o| self.is_guard(*o))
            .collect();
        for guard in guards {
            if self.has_ai(guard) && self.intention_of(guard) == Intention::Idle {
                self.set_intention_active(guard);
            }
        }
    }

    /// Java `Player.setKarmaFlag()`: `UserInfo` for the player, the relation for everybody around.
    pub(crate) fn set_karma_flag(&mut self, id: ObjectId) {
        self.send_user_info(id);
        self.send_relation_to_visible_players(id);
    }

    /// Java `Player.broadcastKarma()`: a `StatusUpdate` with the karma for the player, the relation
    /// for everybody around.
    pub(crate) fn broadcast_karma(&mut self, id: ObjectId) {
        let mut update = StatusUpdate::new(id.0);
        update.add_attribute(attr::KARMA, self.karma_of_player(id));
        self.send(id, &update);
        self.send_relation_to_visible_players(id);
    }

    /// `World.forEachVisibleObject(this, Player.class, p -> p.sendPacket(new RelationChanged(this,
    /// getRelation(p), isAutoAttackable(p))))`: no invisibility check, as in Java.
    fn send_relation_to_visible_players(&self, subject: ObjectId) {
        for viewer in self.visible_players(subject) {
            self.send(
                viewer,
                &RelationChanged::single(self.relation_entry(subject, viewer)),
            );
        }
    }

    /// The entry `new RelationChanged(subject, getRelation(viewer), isAutoAttackable(viewer))`
    /// writes.
    pub(crate) fn relation_entry(&self, subject: ObjectId, viewer: ObjectId) -> RelationEntry {
        RelationEntry {
            object_id: subject.0,
            relation: self.relation(subject, viewer),
            auto_attackable: self.is_auto_attackable(subject, viewer),
            karma: self.karma_of_player(subject),
            pvp_flag: self.pvp_flag_of(subject),
        }
    }

    /// Java `Player.onKillUpdatePvPKarma(target)`: what a kill of another player is worth to the
    /// killer, a PvP kill or a PK with karma.
    pub fn on_kill_update_pvp_karma(&mut self, killer: ObjectId, target: ObjectId) {
        // Avoid nulls && check if player != killedPlayer.
        let Some(killed) = self.acting_player_of(target) else {
            return;
        };
        if killed == killer {
            return;
        }
        // Cursed weapons progress: nobody has one (M8).
        // M8: olympiad (`isInOlympiadMode()` of either).
        // Duel support.
        // NOTE(java-quirk): both only have to be in *a* duel, not the same one.
        if self.is_in_duel(killer) && self.is_in_duel(killed) {
            return;
        }
        // If both players are in SIEGE zone just increase siege kills/deaths.
        if self.is_inside_zone(killer, ZoneId::Siege) && self.is_inside_zone(killed, ZoneId::Siege) {
            // M7: `_clan.addSiegeKill()` / `targetClan.addSiegeDeath()` unless `isSiegeFriend`.
            return;
        }
        // Do nothing when in PVP zone.
        if self.is_inside_zone(killer, ZoneId::Pvp) || self.is_inside_zone(killed, ZoneId::Pvp) {
            return;
        }
        // Check if it's pvp.
        if self.check_if_pvp(killer, target) && self.pvp_flag_of(killed) != 0 {
            self.increase_pvp_kills(killer, target);
            return;
        }
        // Target player doesn't have pvp flag set.
        // M6-CLAN: a war both ways between the clans is a PvP kill (`increasePvpKills`).
        // 'No war' or 'One way war' -> 'Normal PK'.
        if self.karma_of_player(killed) > 0 {
            // Target player has karma.
            if self.settings.pvp.karma_award_pk_kill {
                self.increase_pvp_kills(killer, target);
            }
        } else if self.pvp_flag_of(killed) == 0 {
            // Target player doesn't have karma. The faction system (`_isGood`/`_isEvil`) is not
            // modelled.
            self.increase_pk_kills_and_karma(killer, target);
            // Unequip adventurer items.
            let mut changed = Vec::new();
            self.check_item_restriction(killer, &mut changed);
        }
    }

    /// Java `Player.increasePvpKills(target)`.
    pub(crate) fn increase_pvp_kills(&mut self, killer: ObjectId, target: ObjectId) {
        if !(self.is_player(target) && self.anti_feed_check(killer, target)) {
            return;
        }
        if let Some(player) = self.players.get_mut(&killer) {
            player.data.pvp_kills = player.data.pvp_kills.wrapping_add(1);
        }
        self.update_pvp_title_and_color(killer, true);
        // Send a Server->Client UserInfo packet to attacker with its Karma and PK Counter.
        self.send_user_info(killer);
    }

    /// Java `Player.increasePkKillsAndKarma(target)`.
    pub(crate) fn increase_pk_kills_and_karma(&mut self, killer: ObjectId, target: ObjectId) {
        // Only playables can increase karma/pk.
        if !self.players.contains_key(&target) {
            return;
        }
        // Calculate new karma (before the pk count is increased).
        let pk_kills = self.players.get(&killer).map_or(0, |p| p.data.pk_kills);
        let gain = calculate_karma_gain(pk_kills, false);
        self.set_karma(killer, self.karma_of_player(killer).wrapping_add(gain));
        // PK Points are increased only if you kill a player.
        if self.is_player(target)
            && let Some(player) = self.players.get_mut(&killer)
        {
            player.data.pk_kills = player.data.pk_kills.wrapping_add(1);
        }
        // Update player's UI.
        self.send_user_info(killer);
    }

    /// Java `Player.updatePvpTitleAndColor(broadcastInfo)`: the custom title and colour for the
    /// number of PvP kills.
    pub(crate) fn update_pvp_title_and_color(&mut self, id: ObjectId, broadcast_info: bool) {
        let settings = &self.settings.pvp;
        // The faction system uses title colours.
        if !settings.title_color.enabled || settings.faction_system_enabled {
            return;
        }
        let Some(kills) = self.players.get(&id).map(|p| p.data.pvp_kills) else {
            return;
        };
        let a = &settings.title_color.amounts;
        // Java's if/else chain, in its order.
        let tier = if kills >= a[0] && kills < a[1] {
            Some(0)
        } else if kills >= a[1] && kills < a[2] {
            Some(1)
        } else if kills >= a[2] && kills < a[3] {
            Some(2)
        } else if kills >= a[3] && kills < a[4] {
            Some(3)
        } else if kills >= a[4] {
            Some(4)
        } else {
            None
        };
        let Some(tier) = tier else {
            return;
        };
        let title = format!("\u{00AE} {} \u{00AE}", settings.title_color.titles[tier]);
        let color = settings.title_color.colors[tier];
        if let Some(player) = self.players.get_mut(&id) {
            player.data.title = title;
            player.data.profile.title_color = color;
        }
        if broadcast_info {
            self.broadcast_title_info(id);
        }
    }

    /// Java `Player.broadcastTitleInfo()`: `UserInfo`, then the new title to everybody who sees the
    /// player.
    fn broadcast_title_info(&mut self, id: ObjectId) {
        self.send_user_info(id);
        let Some(title) = self.players.get(&id).map(|p| p.data.title.clone()) else {
            return;
        };
        self.broadcast_with_self(
            id,
            &NicknameChanged {
                object_id: id.0,
                title,
            },
        );
    }

    // -------------------------------------------------------------- anti feed

    /// Java `AntiFeedManager.setLastDeathTime(objectId)`.
    pub(crate) fn set_last_death_time(&mut self, id: ObjectId) {
        let now = self.world_ms();
        self.pvp.last_death_ms.insert(id, now);
    }

    /// Java `AntiFeedManager.check(attacker, target)`: is this kill one that counts (not feeding).
    pub(crate) fn anti_feed_check(&self, attacker: ObjectId, target: ObjectId) -> bool {
        let settings = &self.settings.pvp;
        if !settings.antifeed_enable {
            return true;
        }
        if !self.players.contains_key(&target) {
            return false;
        }
        // Players in offline mode should't be valid targets: no connection is a detached client.
        if !self.sinks.contains_key(&target) {
            return false;
        }
        if settings.antifeed_interval_ms > 0
            && let Some(died) = self.pvp.last_death_ms.get(&target)
            && self.world_ms() - died < settings.antifeed_interval_ms
        {
            return false;
        }
        if settings.antifeed_dualbox {
            if !self.players.contains_key(&attacker) {
                return false;
            }
            let ip = |id: ObjectId| {
                self.sinks
                    .contains_key(&id)
                    .then(|| self.players.get(&id).and_then(|p| p.client_ip))
                    .flatten()
            };
            return match (ip(target), ip(attacker)) {
                (Some(target_ip), Some(attacker_ip)) => target_ip != attacker_ip,
                // Unable to check ip address.
                _ => !settings.antifeed_disconnected_as_dualbox,
            };
        }
        true
    }

    // -------------------------------------------------------------- relation, auto attackable

    /// Java `Player.getRelation(target)`: the mask `RelationChanged` carries for `subject` as
    /// `viewer` sees it.
    pub fn relation(&self, subject: ObjectId, viewer: ObjectId) -> i32 {
        relation_mask(&self.relation_input(subject, viewer))
    }

    /// The inputs of [`relation_mask`] from the world.
    fn relation_input(&self, subject: ObjectId, viewer: ObjectId) -> RelationInput {
        // `getParty() == target.getParty()`: the subject's place in the party it shares with the
        // viewer.
        let party_index = self
            .party_of(subject)
            .filter(|party| Some(*party) == self.party_of(viewer))
            .and_then(|party| self.party_members(party).iter().position(|m| *m == subject));
        // M6-CLAN: `has_clan`, `same_clan`, `has_ally`, `is_clan_leader` and, with M6-CLAN-02, the
        //          war between the clans.
        // M7: `siege_state`, `siege_state_differs` and the territory wars.
        // The Handy's Block Checker arena (an event) adds INSIEGE/ENEMY/ALLY/ATTACKER: M8.
        RelationInput {
            party_index,
            ..RelationInput::default()
        }
    }

    /// Java `Player.isAutoAttackable(attacker)`: can `attacker` (a player or an NPC) hit the player
    /// `subject` without holding Ctrl.
    pub fn is_auto_attackable(&self, subject: ObjectId, attacker: ObjectId) -> bool {
        if !self.players.contains_key(&subject) {
            return false;
        }
        // Invisible GM players should not be attackable.
        if self.objects.get(&subject).is_some_and(|e| e.invisible) && self.is_gm(subject) {
            return false;
        }
        // Check if the attacker isn't the Player Pet. M6-SUMMON: `attacker == _summon`.
        if attacker == subject {
            return false;
        }
        let attacker_class = self.npcs.get(&attacker).map(|n| n.class);
        // Friendly mobs do not attack players.
        if attacker_class == Some(NpcClass::FriendlyMob) {
            return false;
        }
        // Check if the attacker is a Monster.
        if attacker_class.is_some_and(NpcClass::is_monster) {
            return true;
        }
        let attacker_is_player = self.players.contains_key(&attacker);
        // Both in the same duel, which is still going on (`DUELSTATE_DUELLING`).
        // M6-SUMMON: `attacker.isPlayable()` also holds for a summon (its owner's duel id).
        if attacker_is_player
            && self.duel_state_of(subject) == crate::duel::DuelState::Duelling
            && self.duel_id_of(subject) == self.duel_id_of(attacker)
        {
            return true;
        }
        // The attacker is in the subject's party (party checks go before the olympiad ones, so
        // party members do not auto attack in the olympiad).
        if self.is_in_party_with(subject, attacker) {
            return false;
        }
        // M8: the attacker is in the olympiad (`isInOlympiadMode`) and the game started.
        // Events (`isOnEvent`): not modelled.
        // Check if the attacker is a Playable.
        if attacker_is_player {
            if self.is_inside_zone(subject, ZoneId::Peace) || self.is_inside_zone(subject, ZoneId::NoPvp) {
                return false;
            }
            // M6-CLAN / M7: different clans in a siege (defenders and attackers of the same side
            // do not hit each other), and clans at war (`getWantsPeace`, academy members).
            // Check if the Player is in an arena, but NOT siege zone. NOTE: This check comes
            // before clan/ally checks, but after party checks. This is done because in arenas,
            // clan/ally members can autoattack if they are not in party.
            let in_siege =
                self.is_inside_zone(subject, ZoneId::Siege) && self.is_inside_zone(attacker, ZoneId::Siege);
            if self.is_inside_zone(subject, ZoneId::Pvp)
                && self.is_inside_zone(attacker, ZoneId::Pvp)
                && !in_siege
            {
                return true;
            }
            // M6-CLAN: the attacker is a member of the subject's clan, or in its alliance.
            // Now check again if the Player is in pvp zone, but this time at siege PvP zone,
            // applying clan/ally checks.
            if self.is_inside_zone(subject, ZoneId::Pvp)
                && self.is_inside_zone(attacker, ZoneId::Pvp)
                && in_siege
            {
                return true;
            }
            // The faction system (good against evil): not modelled.
        }
        // M7: `attacker instanceof Defender && _clan != null` (the siege's attackers).
        if self.is_guard(attacker) {
            // The faction guards: not modelled. Guards attack only PK players.
            return self.karma_of_player(subject) > 0;
        }
        // Check if the Player has Karma.
        self.karma_of_player(subject) > 0 || self.pvp_flag_of(subject) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spot values worked out from the formula, so a broken vector file cannot hide a mistake.
    #[test]
    fn karma_gain_by_hand() {
        // (0 * 0.5 + 1) * 60 * 12
        assert_eq!(calculate_karma_gain(0, false), 720);
        assert_eq!(calculate_karma_gain(98, false), 36_000);
        // (99 * 0.125 + 37.75) * 60 * 12 = 36090
        assert_eq!(calculate_karma_gain(99, false), 36_090);
        assert_eq!(calculate_karma_gain(179, false), 43_290);
        assert_eq!(calculate_karma_gain(180, false), 43_200);
        // The summon formula is overwritten below 180 (see the quirk).
        assert_eq!(calculate_karma_gain(3, true), calculate_karma_gain(3, false));
        assert_eq!(calculate_karma_gain(1000, true), 10_800);
    }

    #[test]
    fn relation_bits_of_a_party_follow_javas_table() {
        let at = |i| {
            relation_mask(&RelationInput {
                party_index: Some(i),
                ..RelationInput::default()
            })
        };
        let has = relation::HAS_PARTY;
        assert_eq!(at(0), has | 0x10);
        assert_eq!(at(1), has | 0x08);
        assert_eq!(at(2), has | 0x07);
        assert_eq!(at(3), has | 0x06);
        assert_eq!(at(4), has | 0x05);
        assert_eq!(at(5), has | 0x04);
        assert_eq!(at(6), has | 0x03);
        assert_eq!(at(7), has | 0x02);
        assert_eq!(at(8), has | 0x01);
        assert_eq!(at(9), has);
    }

    #[test]
    fn relation_bits_of_clans_sieges_and_wars() {
        assert_eq!(relation_mask(&RelationInput::default()), 0);
        let clan = RelationInput {
            has_clan: true,
            ..RelationInput::default()
        };
        assert_eq!(relation_mask(&clan), 0x40);
        let mate = RelationInput {
            same_clan: true,
            has_ally: true,
            is_clan_leader: true,
            ..clan
        };
        assert_eq!(relation_mask(&mate), 0x40 | 0x100 | 0x10000 | 0x80);
        // A clan leader without a clan cannot happen in Java; the leader bit is independent.
        // Siege: attacker side (state 1) against a viewer on another side.
        let attacker = RelationInput {
            siege_state: 1,
            siege_state_differs: true,
            ..RelationInput::default()
        };
        assert_eq!(relation_mask(&attacker), 0x200 | 0x1000 | 0x400);
        // Defender (state 2) on the viewer's side.
        let defender = RelationInput {
            siege_state: 2,
            ..RelationInput::default()
        };
        assert_eq!(relation_mask(&defender), 0x200 | 0x800);
        let territory = RelationInput {
            siege_state: 2,
            territory_war: true,
            ..RelationInput::default()
        };
        assert_eq!(relation_mask(&territory), 0x80000);
        let one = RelationInput {
            clan_war: Some(ClanWar::OneSided),
            ..RelationInput::default()
        };
        assert_eq!(relation_mask(&one), 0x8000);
        let both = RelationInput {
            clan_war: Some(ClanWar::Mutual),
            ..RelationInput::default()
        };
        assert_eq!(relation_mask(&both), 0x8000 | 0x4000);
    }

    #[test]
    fn defaults_are_javas() {
        let s = PvpSettings::default();
        assert_eq!((s.pvp_normal_time_ms, s.pvp_pvp_time_ms), (120_000, 60_000));
        assert!(s.flag_player_on_raid_attack && s.flag_player_on_champion_attack);
        assert!(!s.title_color.enabled);
        assert_eq!(s.title_color.colors, [0x00FF00; 5]);
    }
}
