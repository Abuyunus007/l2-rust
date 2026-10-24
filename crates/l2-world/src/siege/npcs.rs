//! The NPCs of a castle siege: the control and flame towers, the headquarters of the attackers,
//! the artefact the attackers engrave, and the siege guards.
// Java: mechanics/siege/Siege.java (spawnControlTower, spawnFlameTower, removeTowers,
//       spawnSiegeGuard)
// Java: mechanics/siege/Castle.java (engrave, registerArtefact)
// Java: managers/SiegeGuardManager.java
// Java: entity/actor/Tower.java, entity/actor/instance/{ControlTower,FlameTower,Artefact,
//       SiegeFlag,Defender}.java, entity/actor/status/SiegeFlagStatus.java
// Java: dist/game/data/scripts/handlers/skill/effects/{HeadquarterCreate,TakeCastle}.java
// Java: managers/TerritoryWarManager.java (isAllyField)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{SiegeGuardRecord, SiegeNpc, SiegeTimer};
use crate::residence::{CastleId, ResidenceRef, ResidenceWrite, Writes};
use crate::target::distance_3d;
use crate::world::World;

/// `HeadquarterCreate.HQ_NPC_ID`.
pub const HQ_NPC_ID: i32 = 35062;
/// `SiegeFlag.ScheduleTalkTask`: 20 seconds between two warnings.
const FLAG_TALK_DELAY_MS: i64 = 20_000;

impl World {
    // ------------------------------------------------------------------ towers

    /// Spawns the towers of one kind (`new Spawn(id); spawn.setLocation(loc); doSpawn(false)`).
    fn siege_spawn_tower(&mut self, npc_id: i32, x: i32, y: i32, z: i32, kind: &str) -> Option<ObjectId> {
        let is_kind = self.npc_world.template(npc_id).is_some_and(|t| t.is_type(kind));
        if !is_kind {
            // Java: the cast to `ControlTower` / `FlameTower` throws.
            warn!("Siege: Cannot spawn {kind}! npc {npc_id}");
            return None;
        }
        self.spawn_npc_at(npc_id, x, y, z, 0)
    }

    /// Java `Siege.spawnControlTower()`.
    pub(crate) fn siege_spawn_control_tower(&mut self, id: CastleId) {
        let towers = self
            .sieges
            .settings
            .castles
            .get(&id.0)
            .map(|c| c.control_towers.clone())
            .unwrap_or_default();
        for ts in towers {
            if let Some(npc) = self.siege_spawn_tower(ts.npc_id, ts.x, ts.y, ts.z, "ControlTower") {
                self.sieges.npcs.insert(
                    npc,
                    SiegeNpc::ControlTower {
                        castle: id,
                        guards: Vec::new(),
                    },
                );
                if let Some(siege) = self.castle_siege_mut(id) {
                    siege.control_towers.push(npc);
                }
            }
        }
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.control_tower_count = i32::try_from(siege.control_towers.len()).unwrap_or(i32::MAX);
        }
    }

    /// Java `Siege.spawnFlameTower()`: the tower switches on its trap zones (`setZoneList`).
    pub(crate) fn siege_spawn_flame_tower(&mut self, id: CastleId) {
        let towers = self
            .sieges
            .settings
            .castles
            .get(&id.0)
            .map(|c| c.flame_towers.clone())
            .unwrap_or_default();
        for ts in towers {
            if let Some(npc) = self.siege_spawn_tower(ts.npc_id, ts.x, ts.y, ts.z, "FlameTower") {
                self.sieges.npcs.insert(
                    npc,
                    SiegeNpc::FlameTower {
                        castle: id,
                        upgrade_level: ts.upgrade_level,
                        zone_list: ts.zone_list.clone(),
                    },
                );
                self.flame_tower_enable_zones(npc, true);
                if let Some(siege) = self.castle_siege_mut(id) {
                    siege.flame_towers.push(npc);
                }
            }
        }
    }

    /// Java `Siege.removeTowers()`: the flame towers (their trap zones go off), then the control
    /// towers.
    pub(crate) fn siege_remove_towers(&mut self, id: CastleId) {
        let Some(siege) = self.castle_siege_mut(id) else {
            return;
        };
        let flame = std::mem::take(&mut siege.flame_towers);
        let control = std::mem::take(&mut siege.control_towers);
        for npc in flame.into_iter().chain(control) {
            self.flame_tower_enable_zones(npc, false);
            self.delete_npc(npc);
            self.sieges.npcs.remove(&npc);
        }
    }

    /// Java `FlameTower.enableZones(value)`: the first `2 * upgradeLevel` trap zones.
    fn flame_tower_enable_zones(&mut self, npc: ObjectId, value: bool) {
        let Some(SiegeNpc::FlameTower {
            upgrade_level,
            zone_list,
            ..
        }) = self.sieges.npcs.get(&npc)
        else {
            return;
        };
        if *upgrade_level == 0 {
            return;
        }
        let max = usize::try_from(*upgrade_level * 2).unwrap_or(0);
        // NOTE(java-quirk): Java throws when the tower has fewer zones than that.
        let zones: Vec<i32> = zone_list.iter().take(max).copied().collect();
        for zone in zones {
            self.sieges.trap_zones.insert(zone, value);
        }
    }

    /// Whether a castle trap zone (`castle_trap.xml`) was switched on or off by a flame tower;
    /// `None` when no tower touched it (the zone's `default_enabled` applies).
    // FOLLOW-UP(zones): `SwampZone` and `DamageZone` are not ported (`zones.rs`), so nothing reads
    // this yet.
    pub fn trap_zone_enabled(&self, zone_id: i32) -> Option<bool> {
        self.sieges.trap_zones.get(&zone_id).copied()
    }

    /// Java `Castle.setTrapUpgrade(towerIndex, level, save)`'s memory part.
    pub(crate) fn set_trap_upgrade_level_in_memory(&mut self, id: CastleId, tower_index: i32, level: i32) {
        let Some(tower) = usize::try_from(tower_index).ok().and_then(|i| {
            self.sieges
                .settings
                .castles
                .get_mut(&id.0)
                .and_then(|c| c.flame_towers.get_mut(i))
        }) else {
            // NOTE(java-quirk): Java's `List.get` throws for an index past the towers.
            warn!("Exception: loadTrapUpgrade(): no flame tower {tower_index} for {id}");
            return;
        };
        tower.upgrade_level = level;
    }

    /// Java `Castle.getTrapUpgradeLevel(towerIndex)`.
    pub fn castle_trap_upgrade_level(&self, id: CastleId, tower_index: usize) -> i32 {
        self.sieges
            .settings
            .castles
            .get(&id.0)
            .and_then(|c| c.flame_towers.get(tower_index))
            .map_or(0, |t| t.upgrade_level)
    }

    /// Java `Castle.setTrapUpgrade(towerIndex, level, save)` (the chamberlain's trap upgrade, a
    /// player's purchase: written).
    pub fn castle_set_trap_upgrade(&mut self, id: CastleId, tower_index: i32, level: i32, save: bool) {
        if save {
            self.write_residence(
                Writes::Player,
                ResidenceWrite::CastleTrapUpgrade {
                    castle_id: id.0,
                    tower_index,
                    level,
                },
            );
        }
        self.set_trap_upgrade_level_in_memory(id, tower_index, level);
    }

    /// Java `Castle.removeTrapUpgrade()`.
    pub(crate) fn castle_remove_trap_upgrade(&mut self, id: CastleId) {
        if let Some(config) = self.sieges.settings.castles.get_mut(&id.0) {
            for tower in &mut config.flame_towers {
                tower.upgrade_level = 0;
            }
        }
        self.siege_job_write(ResidenceWrite::CastleTrapUpgradesDelete { castle_id: id.0 });
    }

    // ------------------------------------------------------------------ siege NPC rules

    /// `canBeAttacked()` of the siege NPCs (`Tower`, `Artefact`, `SiegeFlag`); `None` for any
    /// other NPC.
    pub(crate) fn siege_npc_can_be_attacked(&self, npc: ObjectId) -> Option<bool> {
        if self.npc_template(npc).is_some_and(|t| t.is_type("Artefact")) {
            return Some(false);
        }
        match self.sieges.npcs.get(&npc)? {
            // Attackable during siege by attacker only.
            SiegeNpc::ControlTower { castle, .. } | SiegeNpc::FlameTower { castle, .. } => {
                Some(castle.0 > 0 && self.is_castle_siege_in_progress(*castle))
            }
            SiegeNpc::Flag { .. } => Some(!self.is_invul(npc)),
            SiegeNpc::Guard { .. } => None,
        }
    }

    /// `isAutoAttackable(attacker)` of the siege NPCs (`Tower`, `Artefact`, `SiegeFlag`,
    /// `Defender`); `None` for any other NPC.
    pub(crate) fn siege_npc_auto_attackable(&self, npc: ObjectId, attacker: ObjectId) -> Option<bool> {
        let template = self.npc_template(npc)?;
        if template.is_type("Artefact") {
            return Some(false);
        }
        if template.is_type("Defender") {
            return Some(self.defender_auto_attackable(npc, attacker));
        }
        match self.sieges.npcs.get(&npc)? {
            SiegeNpc::ControlTower { castle, .. } | SiegeNpc::FlameTower { castle, .. } => Some(
                self.is_player(attacker)
                    && castle.0 > 0
                    && self.castle_siege(*castle).is_some_and(|s| {
                        s.in_progress && s.is_attacker(self.clan_of(attacker).map(|c| c.id))
                    }),
            ),
            SiegeNpc::Flag { .. } => Some(!self.is_invul(npc)),
            SiegeNpc::Guard { .. } => None,
        }
    }

    /// Java `Defender.isAutoAttackable(attacker)`: during an active siege of its castle (fortress,
    /// hall) anybody but the defenders of that siege.
    pub(crate) fn defender_auto_attackable(&self, npc: ObjectId, attacker: ObjectId) -> bool {
        // Attackable during siege by all except defenders.
        if !self.is_playable(attacker) {
            return false;
        }
        let Some(active_siege_id) = self.defender_active_siege_id(npc) else {
            return false;
        };
        let Some(player) = self.acting_player(attacker) else {
            return false;
        };
        let state = self.siege_state_of(player);
        // Check if player is an enemy of this defender npc.
        (state == 2 && !self.is_registered_on_this_siege_field(player, active_siege_id))
            || (state == 1 && !self.is_ally_field(player, active_siege_id))
            || state == 0
    }

    /// The `activeSiegeId` of `Defender`: the castle (fortress, hall) it was spawned in, while its
    /// siege zone is active. `_castle` is `CastleManager.getCastle(x, y, z)` at `onSpawn`.
    pub(crate) fn defender_active_siege_id(&self, npc: ObjectId) -> Option<i32> {
        let (x, y, z) = self
            .npcs
            .get(&npc)
            .map(|n| n.spawn_point)
            .filter(|p| *p != (0, 0, 0))
            .or_else(|| self.position_of(npc))?;
        // M7-FORT-01 / M7-CHSIEGE-01: `_fort` and `_hall` (they come first in Java).
        let castle = self.castle_at(x, y, z)?;
        let zone = self.castle(castle)?.siege_zone?;
        self.is_siege_zone_active(zone).then_some(castle.0)
    }

    /// `Defender.addDamageHate`'s siege rule: no hate from a registered defender's harmless act.
    pub(crate) fn defender_refuses_hate(
        &self,
        npc: ObjectId,
        attacker: ObjectId,
        damage: i64,
        aggro: i64,
    ) -> bool {
        if !(damage == 0 && aggro <= 1 && self.is_playable(attacker)) {
            return false;
        }
        let Some(player) = self.acting_player(attacker) else {
            return false;
        };
        let Some(active_siege_id) = self.defender_active_siege_id(npc) else {
            return false;
        };
        // Do not add hate on defenders.
        self.siege_state_of(player) == 2 && self.is_registered_on_this_siege_field(player, active_siege_id)
    }

    /// Java `TerritoryWarManager.isAllyField(player, fieldId)`.
    // M7-TW-01: the territory's fortress (`fieldId > 100`).
    pub fn is_ally_field(&self, player: ObjectId, field_id: i32) -> bool {
        let side = self.siege_side_of(player);
        side != 0 && side - 80 == field_id
    }

    /// The clicks of `Tower.onAction` and `SiegeFlag.onAction`: `true` when the NPC is one of them
    /// and the click is handled.
    pub(crate) fn siege_npc_click(&mut self, player: ObjectId, npc: ObjectId, interact: bool) -> bool {
        let tower = match self.sieges.npcs.get(&npc) {
            Some(SiegeNpc::ControlTower { .. } | SiegeNpc::FlameTower { .. }) => true,
            Some(SiegeNpc::Flag { .. }) => false,
            _ => return false,
        };
        if self.target_of(player) != Some(npc) {
            // Set the target of the Player player.
            self.set_target(player, Some(npc));
        } else if interact {
            let dz = match (self.position_of(player), self.position_of(npc)) {
                (Some(p), Some(n)) => (p.2 - n.2).abs(),
                _ => i32::MAX,
            };
            let attackable = self.siege_npc_auto_attackable(npc, player).unwrap_or(false);
            if attackable && dz < 100 && (!tower || self.can_see_target(player, npc)) {
                self.set_intention_attack(player, npc);
            }
        }
        // The `ActionFailed` of `Tower.onAction` (and of the flag that is not attacked) is the
        // one every click ends with (`on_action_object`).
        true
    }

    // ------------------------------------------------------------------ the headquarters

    /// Java `HeadquarterCreate.onStart` + `new SiegeFlag(player, template, advanced, false)`: the
    /// clan leader's headquarters stands 50 above him, with the clan's name as its title.
    pub fn siege_create_headquarters(&mut self, player: ObjectId, advanced: bool) {
        if !self.is_clan_leader(player) {
            return;
        }
        let Some(clan_id) = self.clan_of(player).map(|c| c.id) else {
            return;
        };
        let Some((x, y, z)) = self.position_of(player) else {
            return;
        };
        // M7-TW-01: a flag of Territory War (`isTWInProgress`) belongs to no siege.
        // `SiegeManager.getSiege`, then `FortSiegeManager.getSiege`, then `CHSiegeManager`.
        // M7-FORT-01 / M7-CHSIEGE-01: the fortress and hall sieges.
        let Some(castle) = self.siege_at(x, y, z) else {
            // Java: "SiegeFlag: Initialization failed." (a NullPointerException).
            warn!("SiegeFlag: Initialization failed.");
            return;
        };
        let siege = ResidenceRef::Castle(castle);
        if self.siege_attacker_clan_mut(siege, clan_id).is_none() {
            warn!("SiegeFlag: Cannot find siege clan.");
            return;
        }
        let heading = self.heading_of(player);
        let Some(flag) = self.spawn_npc_at(HQ_NPC_ID, x, y, z + 50, heading) else {
            return;
        };
        if let Some(sc) = self.siege_attacker_clan_mut(siege, clan_id) {
            sc.flags.push(flag);
        }
        self.sieges.npcs.insert(
            flag,
            SiegeNpc::Flag {
                clan_id,
                siege: Some(siege),
                advanced,
                can_talk: true,
                invul: false,
            },
        );
        self.set_invul(flag, false);
        let name = self.clan(clan_id).map(|c| c.name.clone()).unwrap_or_default();
        self.sieges.npc_titles.insert(flag, name);
        // The title changed after the spawn sent the NPC: show it again.
        let viewers = self.visible_players(flag);
        for viewer in viewers {
            self.send_npc_info(flag, viewer);
        }
    }

    /// Java `Formulas.calcSiegeRegenModifier(player)`: 1.5 within 200 of the first headquarters
    /// of the player's clan in the castle siege he stands in, else 0.
    pub fn siege_regen_modifier(&self, player: ObjectId) -> f64 {
        let Some(clan) = self.clan_of(player).map(|c| c.id) else {
            return 0.0;
        };
        let Some(siege) = self.siege_of(player).and_then(|c| self.castle_siege(c)) else {
            return 0.0;
        };
        match siege.attacker_clan(clan).and_then(|sc| sc.flags.first()) {
            Some(flag) if self.check_if_in_range(200, player, *flag, true) => 1.5,
            _ => 0.0,
        }
    }

    /// A title the siege gave an NPC (Java `Npc.setTitle`, the headquarters' clan name).
    pub fn npc_custom_title(&self, npc: ObjectId) -> Option<&str> {
        self.sieges.npc_titles.get(&npc).map(String::as_str)
    }

    /// `SiegeFlagStatus.reduceHp`: an advanced headquarters takes half the damage first, then the
    /// damage again. `true` when `npc` is a headquarters (the caller does nothing more).
    // NOTE(java-quirk): "double HP" in the skill's text, but Java takes 1.5 times the damage.
    pub(crate) fn siege_flag_reduce_hp(
        &mut self,
        npc: ObjectId,
        amount: f64,
        attacker: Option<ObjectId>,
    ) -> bool {
        let Some(SiegeNpc::Flag { advanced, .. }) = self.sieges.npcs.get(&npc) else {
            return false;
        };
        if *advanced {
            self.reduce_hp(npc, amount / 2.0, attacker);
        }
        self.reduce_hp(npc, amount, attacker);
        self.siege_flag_after_damage(npc);
        true
    }

    /// The rest of `SiegeFlag.reduceCurrentHp`: while its siege runs, the headquarters warns its
    /// clan ("Your base is being attacked."), then waits 20 seconds.
    fn siege_flag_after_damage(&mut self, npc: ObjectId) {
        let Some(SiegeNpc::Flag {
            clan_id, can_talk, ..
        }) = self.sieges.npcs.get(&npc).cloned()
        else {
            return;
        };
        if !can_talk {
            return;
        }
        // `getCastle()` / `getFort()` / `getConquerableHall()` of the flag's position.
        let in_siege = self
            .npc_castle(npc)
            .is_some_and(|c| self.is_castle_siege_in_progress(c));
        if !in_siege || self.clan(clan_id).is_none() {
            return;
        }
        self.broadcast_to_clan(
            clan_id,
            &SystemMessage::new(SystemMessageId::YourBaseIsBeingAttacked),
        );
        if let Some(SiegeNpc::Flag { can_talk, .. }) = self.sieges.npcs.get_mut(&npc) {
            *can_talk = false;
        }
        self.schedule_siege_timer(SiegeTimer::FlagTalk(npc), FLAG_TALK_DELAY_MS);
    }

    /// `ScheduleTalkTask.run()`.
    pub(crate) fn on_flag_talk_timer(&mut self, npc: ObjectId) {
        if let Some(SiegeNpc::Flag { can_talk, .. }) = self.sieges.npcs.get_mut(&npc) {
            *can_talk = true;
        }
    }

    // ------------------------------------------------------------------ deaths

    /// The `doDie` overrides of the siege NPCs, after the death: a control tower counts (and
    /// stops the respawn of its guards), a flame tower switches its traps off, a headquarters
    /// leaves its clan and vanishes.
    pub(crate) fn siege_npc_do_die(&mut self, npc: ObjectId) {
        match self.sieges.npcs.get(&npc).cloned() {
            Some(SiegeNpc::ControlTower { castle, guards }) => {
                if self.is_castle_siege_in_progress(castle) {
                    self.siege_killed_ct(castle);
                    for spawn in guards {
                        self.stop_respawn(spawn);
                    }
                    if let Some(SiegeNpc::ControlTower { guards, .. }) = self.sieges.npcs.get_mut(&npc) {
                        guards.clear();
                    }
                }
            }
            Some(SiegeNpc::FlameTower { .. }) => self.flame_tower_enable_zones(npc, false),
            Some(SiegeNpc::Flag { clan_id, siege, .. }) => {
                // M7-TW-01: `TerritoryWarManager.removeClanFlag(clan)` of a flag without a siege.
                if let Some(siege) = siege
                    && let Some(sc) = self.siege_attacker_clan_mut(siege, clan_id)
                {
                    sc.flags.retain(|f| *f != npc);
                    self.delete_npc(npc);
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ the artefact

    /// Java `TakeCastle.onStart` + `Castle.engrave(clan, target)`: the engraver's clan takes the
    /// castle of the place he stands, when the target is that castle's artefact.
    pub fn castle_engrave(&mut self, player: ObjectId, target: ObjectId) {
        if !self.is_player(player) {
            return;
        }
        // `CastleManager.getCastle(effector)`.
        let Some(castle) = self
            .position_of(player)
            .and_then(|(x, y, z)| self.castle_at(x, y, z))
        else {
            return;
        };
        // `_artefacts.contains(target)`: the artefacts register with their castle at spawn.
        let is_artefact = self.npc_template(target).is_some_and(|t| t.is_type("Artefact"))
            && self.npc_castle(target) == Some(castle);
        if !is_artefact {
            return;
        }
        let Some((clan_id, clan_name)) = self.clan_of(player).map(|c| (c.id, c.name.clone())) else {
            return;
        };
        self.castle_set_owner(castle, Some(clan_id));
        self.broadcast_to_all_online_players(
            &SystemMessage::new(SystemMessageId::ClanS1HasSuccessfullyEngravedTheHolyArtifact)
                .with(SystemMessageParam::Text(clan_name)),
        );
    }

    // ------------------------------------------------------------------ SiegeGuardManager

    /// Java `SiegeGuardManager.spawnSiegeGuard()` and the guard registration of
    /// `Siege.spawnSiegeGuard()`: the hired mercenaries of an owned castle (at most
    /// `MaxMercenaries`), else the castle's own guards; each guard joins the nearest control
    /// tower.
    pub(crate) fn siege_spawn_siege_guard(&mut self, id: CastleId) {
        let hired_max = self.merc_max_allowed(id.0);
        let is_hired = self.castle(id).is_some_and(|c| c.owner_id > 0);
        // loadSiegeGuard.
        let rows: Vec<SiegeGuardRecord> = self
            .sieges
            .guard_rows
            .iter()
            .filter(|r| r.castle_id == id.0 && r.is_hired == is_hired)
            .copied()
            .collect();
        let mut spawns = Vec::new();
        for row in &rows {
            spawns.push((self.add_guard_spawn(row), row));
        }
        let mut hired_count = 0;
        for (index, row) in &spawns {
            // `spawn.init()`.
            let created = self.init_runtime_spawn(*index);
            for npc in &created {
                self.sieges.npcs.insert(*npc, SiegeNpc::Guard { castle: id });
            }
            if is_hired {
                self.stop_respawn(*index);
                hired_count += 1;
                // NOTE(java-quirk): the mercenary past the limit has spawned already.
                if hired_count > hired_max {
                    break;
                }
            } else if row.respawn_delay == 0 {
                self.stop_respawn(*index);
            }
        }
        let indices: Vec<usize> = spawns.iter().map(|(i, _)| *i).collect();
        // Register guard to the closest Control Tower. When CT dies, so do all the guards that it
        // controls.
        let towers: Vec<(ObjectId, (i32, i32, i32))> = self
            .castle_siege(id)
            .map(|s| s.control_towers.clone())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|t| self.position_of(t).map(|p| (t, p)))
            .collect();
        for (index, row) in &spawns {
            let mut closest: Option<ObjectId> = None;
            let mut closest_distance = f64::from(i32::MAX);
            for (tower, pos) in &towers {
                let distance = distance_3d(*pos, (row.x, row.y, row.z));
                if distance < closest_distance {
                    closest = Some(*tower);
                    closest_distance = distance;
                }
            }
            if let Some(tower) = closest
                && let Some(SiegeNpc::ControlTower { guards, .. }) = self.sieges.npcs.get_mut(&tower)
            {
                guards.push(*index);
            }
        }
        if let Some(siege) = self.castle_siege_mut(id) {
            siege.guard_spawns = indices;
        }
    }

    /// A guard row as a spawn (`new Spawn(npcId)`, amount 1, the position and heading,
    /// `setRespawnDelay(respawnDelay)`).
    fn add_guard_spawn(&mut self, row: &SiegeGuardRecord) -> usize {
        let delay_ms = if row.respawn_delay == 0 {
            0
        } else {
            row.respawn_delay.max(1).saturating_mul(1000)
        };
        self.add_runtime_spawn(l2_data::spawn::NpcSpawn {
            npc_id: row.npc_id,
            amount: 1,
            location: l2_data::Location {
                x: row.x,
                y: row.y,
                z: row.z,
            },
            heading: row.heading,
            respawn_min_ms: delay_ms,
            respawn_max_ms: delay_ms,
            // `init()`: `_doRespawn = _respawnMinDelay > 0`.
            respawn_enabled: delay_ms > 0,
            chase_range: 0,
            name: None,
            territory: None,
            period_of_day: None,
            ai_data: None,
            group: None,
            source: "castle_siege_guards".to_owned(),
        })
    }

    /// Java `SiegeGuardManager.unspawnSiegeGuard()`.
    pub(crate) fn siege_unspawn_siege_guard(&mut self, id: CastleId) {
        let spawns = self
            .castle_siege_mut(id)
            .map(|s| std::mem::take(&mut s.guard_spawns))
            .unwrap_or_default();
        for index in spawns {
            if let Some(npc) = self.last_spawn(index) {
                self.stop_respawn(index);
                self.delete_npc(npc);
                self.sieges.npcs.remove(&npc);
            }
        }
    }

    /// Java `SiegeGuardManager.saveSiegeGuard(x, y, z, heading, npcId, isHire)`: a hired
    /// mercenary has no respawn, a guard 600 s. What a castle lord or a GM does himself, so it
    /// is written like a player's action.
    fn siege_save_siege_guard(
        &mut self,
        id: CastleId,
        (x, y, z): (i32, i32, i32),
        heading: i32,
        npc_id: i32,
        hired: bool,
    ) {
        let row = SiegeGuardRecord {
            castle_id: id.0,
            npc_id,
            x,
            y,
            z,
            heading,
            respawn_delay: if hired { 0 } else { 600 },
            is_hired: hired,
        };
        self.sieges.guard_rows.push(row);
        self.write_residence(Writes::Player, ResidenceWrite::SiegeGuardInsert(row));
    }

    /// Java `SiegeGuardManager.addSiegeGuard(player, npcId)` (`//add_guard`).
    pub fn siege_add_siege_guard(&mut self, id: CastleId, player: ObjectId, npc_id: i32) {
        let Some((x, y, z)) = self.position_of(player) else {
            return;
        };
        let heading = self.heading_of(player);
        self.siege_save_siege_guard(id, (x, y, z), heading, npc_id, false);
    }

    /// Java `SiegeGuardManager.hireMerc(x, y, z, heading, npcId)`.
    pub(crate) fn siege_hire_merc(
        &mut self,
        id: CastleId,
        x: i32,
        y: i32,
        z: i32,
        heading: i32,
        npc_id: i32,
    ) {
        self.siege_save_siege_guard(id, (x, y, z), heading, npc_id, true);
    }

    /// Java `SiegeGuardManager.removeMerc(npcId, x, y, z)`: the castle lord picked a ticket up.
    pub(crate) fn siege_remove_merc(&mut self, npc_id: i32, x: i32, y: i32, z: i32) {
        self.sieges
            .guard_rows
            .retain(|r| !(r.npc_id == npc_id && r.x == x && r.y == y && r.z == z && r.is_hired));
        self.write_residence(
            Writes::Player,
            ResidenceWrite::SiegeGuardDeleteHired { npc_id, x, y, z },
        );
    }

    /// Java `SiegeGuardManager.removeMercs()`: every hired mercenary of the castle (a siege job).
    pub(crate) fn siege_remove_mercs(&mut self, id: CastleId) {
        self.sieges
            .guard_rows
            .retain(|r| !(r.castle_id == id.0 && r.is_hired));
        self.siege_job_write(ResidenceWrite::SiegeGuardsDeleteHired { castle_id: id.0 });
    }
}
