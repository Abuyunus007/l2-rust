//! `SwampZone` and `DamageZone`: the castle trap zones the flame towers switch on (M7-FU-03).
// Java: entity/zone/type/{SwampZone,DamageZone}.java
// Java: entity/actor/stat/PlayableStat.java (getRunSpeed, getWalkSpeed: the swamp's move bonus)
// Java: entity/actor/instance/FlameTower.java (enableZones), mechanics/siege/Castle.java
//       (getTrapUpgradeLevel)
//
// # How the trap level reaches the zones
// `castle_trap.xml` zones start disabled (`default_enabled = false`). A flame tower of upgrade level
// `n` switches its first `2 * n` zones on when the siege starts (`siege/npcs.rs`:
// `flame_tower_enable_zones`, `trap_zone_enabled`) and off when it ends. `ZoneType.isAffected`
// refuses a disabled zone, so nobody enters a trap that is off ([`World::zone_enabled`]); a player
// already inside stays in until he leaves, like in Java.
//
// # Where this differs from Java
// - NOTE: only players are tracked by the zones here (NPC zones are inert, `zones.rs`), so a
//   `DamageZone` hurts players only, which is what Java does too (`character.isPlayer()`).
// - NOTE(java-quirk): `ThreadPool.scheduleAtFixedRate(.., initialDelay, reuse)` takes milliseconds
//   (10 ms and 5000 ms by default); the world runs the first pass on the next 50 ms tick.

use l2_data::Zone;
use l2_data::stats::Stat;
use l2_protocol_game::server::OnEventTrigger;
use l2_util::ObjectId;
use tracing::debug;

use crate::residence::CastleId;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// `SwampZone._move_bonus` default.
const DEFAULT_MOVE_BONUS: f64 = 0.5;
/// `DamageZone` defaults: `_damageHPPerSec`, `_damageMPPerSec`, `_startTask`, `_reuseTask`.
const DEFAULT_DAMAGE_HP: i32 = 200;
const DEFAULT_DAMAGE_MP: i32 = 0;
const DEFAULT_START_MS: i64 = 10;
const DEFAULT_REUSE_MS: i64 = 5000;

/// The timers of the trap zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneTimer {
    /// `DamageZone.ApplyDamage` of the zone with this id.
    DamageZone(i32),
}

/// The parameters `DamageZone.setParameter` keeps, read from the zone's raw `<stat>` list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DamageParams {
    hp: i32,
    mp: i32,
    castle_id: i32,
    start_ms: i64,
    reuse_ms: i64,
}

impl DamageParams {
    fn of(zone: &Zone) -> Self {
        let mut p = Self {
            hp: DEFAULT_DAMAGE_HP,
            mp: DEFAULT_DAMAGE_MP,
            castle_id: 0,
            start_ms: DEFAULT_START_MS,
            reuse_ms: DEFAULT_REUSE_MS,
        };
        // A value `Integer.parseInt` rejects is skipped (Java's loader throws).
        for (name, value) in &zone.params {
            let Ok(int) = value.trim().parse::<i32>() else {
                continue;
            };
            match name.as_str() {
                "dmgHPSec" => p.hp = int,
                "dmgMPSec" => p.mp = int,
                "castleId" => p.castle_id = int,
                n if n.eq_ignore_ascii_case("initialDelay") => p.start_ms = i64::from(int),
                n if n.eq_ignore_ascii_case("reuse") => p.reuse_ms = i64::from(int),
                _ => {}
            }
        }
        p
    }
}

/// `castleId` of a trap zone (0 = none).
fn castle_param(zone: &Zone) -> i32 {
    zone.int_param("castleId").unwrap_or(0)
}

impl World {
    /// Whether a trap zone is on: the flame towers' switch, else `default_enabled`. `None` for a
    /// zone type the towers do not touch.
    pub(crate) fn trap_zone_override(&self, zone: &Zone) -> Option<bool> {
        if matches!(zone.zone_type.as_str(), "SwampZone" | "DamageZone") {
            self.trap_zone_enabled(zone.id)
        } else {
            None
        }
    }

    /// Java `ZoneType.isEnabled()` of a trap zone.
    fn trap_zone_is_on(&self, zone: &Zone) -> bool {
        self.trap_zone_override(zone).unwrap_or_else(|| {
            zone.param("default_enabled")
                .is_none_or(|v| v.eq_ignore_ascii_case("true"))
        })
    }

    /// `getCastle()` of a trap zone: the castle the datapack names, if the world has it.
    fn trap_castle(&self, zone: &Zone) -> Option<CastleId> {
        let id = castle_param(zone);
        (id > 0 && self.castle(CastleId(id)).is_some()).then_some(CastleId(id))
    }

    // ------------------------------------------------------------------ SwampZone

    /// `SwampZone.onEnter(creature)` for a player.
    pub(crate) fn swamp_zone_enter(&mut self, id: ObjectId, zone: &Zone) {
        if let Some(castle) = self.trap_castle(zone) {
            // "Castle zones active only during siege."
            if !self.is_castle_siege_in_progress(castle) {
                return;
            }
            // "Defenders not affected."
            if self.is_in_siege(id) && self.siege_state_of(id) == 2 {
                return;
            }
        }
        self.set_inside(id, ZoneId::Swamp, true);
        let event_id = zone.int_param("eventId").unwrap_or(0);
        if event_id > 0 {
            self.send(
                id,
                &OnEventTrigger {
                    emitter_id: event_id,
                    enabled: true,
                },
            );
        }
        self.broadcast_user_info(id);
    }

    /// `SwampZone.onExit(creature)` for a player.
    pub(crate) fn swamp_zone_exit(&mut self, id: ObjectId, zone: &Zone) {
        // "Don't broadcast info if not needed."
        if !self.is_inside_zone(id, ZoneId::Swamp) {
            return;
        }
        self.set_inside(id, ZoneId::Swamp, false);
        let event_id = zone.int_param("eventId").unwrap_or(0);
        if event_id > 0 {
            self.send(
                id,
                &OnEventTrigger {
                    emitter_id: event_id,
                    enabled: false,
                },
            );
        }
        if !self.is_teleporting(id) {
            self.broadcast_user_info(id);
        }
    }

    /// `ZoneManager.getZone(playable, SwampZone.class).getMoveBonus()` for a player inside a swamp
    /// (`PlayableStat.getRunSpeed` and `getWalkSpeed` multiply by it); `None` outside one.
    pub(crate) fn swamp_move_bonus(&self, id: ObjectId) -> Option<f64> {
        let player = self.players.get(&id)?;
        if !player.zones.is_inside(ZoneId::Swamp) {
            return None;
        }
        let zones = self.zoning.zones.as_ref()?;
        let (x, y, z) = (player.core.x, player.core.y, player.core.z);
        let zone = zones.zone_at(x, y, z, "SwampZone")?;
        Some(
            zone.param("move_bonus")
                .and_then(|v| v.trim().parse::<f64>().ok())
                .unwrap_or(DEFAULT_MOVE_BONUS),
        )
    }

    // ------------------------------------------------------------------ DamageZone

    /// The players a zone holds (Java `getCharactersInside()`, players only), by object id.
    fn zone_players(&self, zone: &Zone) -> Vec<ObjectId> {
        let mut inside: Vec<ObjectId> = self
            .players
            .iter()
            .filter(|(_, p)| {
                p.zones
                    .members()
                    .iter()
                    .any(|(id, ty)| *id == zone.id && *ty == zone.zone_type)
            })
            .map(|(id, _)| *id)
            .collect();
        inside.sort();
        inside
    }

    fn schedule_zone_timer(&mut self, timer: ZoneTimer, delay_ms: i64) -> TimerId {
        let delay = u64::try_from(delay_ms.max(0)).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        self.timers.schedule_at(due, TimerKind::Zone(timer))
    }

    /// `DamageZone.onEnter(creature)` for a player: the damage task starts with the first one.
    pub(crate) fn damage_zone_enter(&mut self, id: ObjectId, zone: &Zone) {
        if self.zoning.damage_tasks.contains_key(&zone.id) {
            return;
        }
        let p = DamageParams::of(zone);
        if p.hp == 0 && p.mp == 0 {
            return;
        }
        if self.trap_castle(zone).is_some() {
            let castle = CastleId(p.castle_id);
            // "Siege and no defender": a castle zone hurts the attackers during the siege only.
            if !(self.is_castle_siege_in_progress(castle) && self.siege_state_of(id) != 2) {
                return;
            }
        }
        let timer = self.schedule_zone_timer(ZoneTimer::DamageZone(zone.id), p.start_ms);
        self.zoning.damage_tasks.insert(zone.id, timer);
    }

    /// `DamageZone.onExit(creature)`: the task stops with the last one.
    pub(crate) fn damage_zone_exit(&mut self, zone: &Zone) {
        if self.zone_players(zone).is_empty()
            && let Some(timer) = self.zoning.damage_tasks.remove(&zone.id)
        {
            self.timers.cancel(timer);
        }
    }

    /// The zone timers.
    pub(crate) fn on_zone_timer(&mut self, timer: ZoneTimer) {
        match timer {
            ZoneTimer::DamageZone(zone_id) => self.on_damage_zone_run(zone_id),
        }
    }

    /// `DamageZone.ApplyDamage.run()`.
    fn on_damage_zone_run(&mut self, zone_id: i32) {
        // The timer that fired is spent; the task goes on only by scheduling the next pass.
        self.zoning.damage_tasks.remove(&zone_id);
        let Some(zone) = self
            .zoning
            .zones
            .as_ref()
            .and_then(|z| z.get_of_type(zone_id, "DamageZone"))
            .cloned()
        else {
            return;
        };
        let inside = self.zone_players(&zone);
        if inside.is_empty() {
            return;
        }
        let p = DamageParams::of(&zone);
        // NOTE(java-quirk): a disabled zone keeps its task and only skips the pass.
        if !self.trap_zone_is_on(&zone) {
            self.reschedule_damage_zone(zone_id, p);
            return;
        }
        let mut siege = false;
        if let Some(castle) = self.trap_castle(&zone) {
            siege = self.is_castle_siege_in_progress(castle);
            // "Castle zones active only during siege."
            if !siege {
                return;
            }
        }
        for player in inside {
            if self.is_dead(player) {
                continue;
            }
            // "During siege defenders not affected."
            if siege && self.is_in_siege(player) && self.siege_state_of(player) == 2 {
                continue;
            }
            let vuln = self
                .stats(player)
                .map_or(0.0, |v| v.calc(Stat::DamageZoneVuln, 0.0));
            let multiplier = 1.0 + vuln / 100.0;
            if p.hp != 0 {
                self.reduce_current_hp(player, f64::from(p.hp) * multiplier, Some(player));
            }
            if p.mp != 0 {
                self.reduce_mp(player, f64::from(p.mp) * multiplier);
            }
        }
        debug!("DamageZone {zone_id}: pass done");
        self.reschedule_damage_zone(zone_id, p);
    }

    fn reschedule_damage_zone(&mut self, zone_id: i32, p: DamageParams) {
        // A period of 0 would be an `IllegalArgumentException` in Java; one tick is the floor.
        let timer = self.schedule_zone_timer(ZoneTimer::DamageZone(zone_id), p.reuse_ms.max(1));
        self.zoning.damage_tasks.insert(zone_id, timer);
    }
}
