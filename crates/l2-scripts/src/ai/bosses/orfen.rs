//! Orfen, the Pagan Temple grand boss: the row restored at start, the nest teleports by HP, the
//! paralysis pull, Raikel Leos minions and their six-minute respawns, Riba Iren heals and the
//! respawn cycle.
// Java: dist/game/data/scripts/ai/bosses/Orfen/Orfen.java
//
// The Java constructor is `on_load`; the static fields are `OrfenState` (`ctx.state`). Statuses: 0
// alive, 1 dead.
//
// Side by side (D-54, `super` module): with `CycleDatabaseWrites` off the constructor restores
// the boss only from an alive row, and `orfen_unlock` reads the row again and brings Orfen back
// only when the row says alive.
//
// Not modelled: `unload` (D-16: no reload).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, PlaySound};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportTarget};
use tracing::{info, warn};

use super::{EVENT_ROW_READ, date_time_string, row_lets_boss_run};

// NPC identifiers.
const ORFEN_NPC_ID: i32 = 29014;
const RAIKEL_LEOS_NPC_ID: i32 = 29016;
const RIBA_IREN_NPC_ID: i32 = 29018;

// Boss status flags.
const STATUS_ALIVE: i32 = 0;
const STATUS_DEAD: i32 = 1;

// Event identifiers.
const EVENT_ORFEN_UNLOCK: &str = "orfen_unlock";
const EVENT_CHECK_ORFEN_POSITION: &str = "check_orfen_pos";
const EVENT_CHECK_MINION_LOCATION: &str = "check_minion_loc";
const EVENT_DESPAWN_MINIONS: &str = "despawn_minions";
const EVENT_SPAWN_MINION: &str = "spawn_minion";

// Timing configuration.
const MILLIS_PER_HOUR: i64 = 3_600_000;
const CHECK_POSITION_INTERVAL_MS: i64 = 10_000;
const CHECK_MINION_INTERVAL_MS: i64 = 10_000;
const MINION_DESPAWN_DELAY_MS: i64 = 20_000;
const MINION_RESPAWN_DELAY_MS: i64 = 360_000;

// Distance thresholds.
const MINION_FOLLOW_RADIUS: i32 = 3000;
const TELEPORT_SKILL_MAX_RANGE: i32 = 1000;
const TELEPORT_SKILL_MIN_RANGE: i32 = 300;

// HP thresholds (fractions).
const ORFEN_TELEPORT_HP_FRACTION: f64 = 0.5;
const ORFEN_RETURN_HP_FRACTION: f64 = 0.95;
const RIBA_IREN_HEAL_HP_FRACTION: f64 = 0.5;

// Probabilities (1 / N).
const SKILL_SEE_REACTION_CHANCE: i32 = 5; // 1 / 5.
const ORFEN_ATTACK_TELEPORT_CHANCE: i32 = 10; // 1 / 10.
const RAIKEL_FACTION_SKILL_CHANCE: i32 = 20; // 1 / 20.
const RIBA_IREN_BASE_HEAL_CHANCE: i32 = 10; // getRandom(10) < chance.
const RIBA_IREN_ORFEN_HEAL_CHANCE: i32 = 9;

// Skill holders.
const PARALYSIS: SkillKey = SkillKey { id: 4064, level: 1 };
const BLOW: SkillKey = SkillKey { id: 4067, level: 4 };
const ORFEN_HEAL: SkillKey = SkillKey { id: 4516, level: 1 };

// Spawn locations (nest and roaming positions).
const ORFEN_SPAWN_LOCATIONS: [(i32, i32, i32); 4] = [
    (43728, 17220, -4342),
    (55024, 17368, -5412),
    (53504, 21248, -5486),
    (53248, 24576, -5262),
];

// Dialog lines.
const ORFEN_MESSAGES: [NpcStringId; 4] = [
    NpcStringId::S1StopKiddingYourselfAboutYourOwnPowerlessness,
    NpcStringId::S1ILlMakeYouFeelWhatTrueFearIs,
    NpcStringId::YouReReallyStupidToHaveChallengedMeS1GetReady,
    NpcStringId::S1DoYouThinkThatSGoingToWork,
];

/// The static fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct OrfenState {
    /// `MINIONS`.
    pub(crate) minions: Vec<ObjectId>,
    /// `_hasTeleported`.
    pub(crate) has_teleported: bool,
    /// `ZONE`: the boss zone that holds the first spawn location.
    pub(crate) zone: Option<i32>,
}

/// Java `ai.bosses.Orfen.Orfen`.
pub struct Orfen;

impl Script for Orfen {
    fn name(&self) -> &'static str {
        "Orfen"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Orfen"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_skill_see_id(&[ORFEN_NPC_ID]);
        s.add_faction_call_id(&[ORFEN_NPC_ID, RAIKEL_LEOS_NPC_ID, RIBA_IREN_NPC_ID]);
        s.add_attack_id(&[ORFEN_NPC_ID, RAIKEL_LEOS_NPC_ID, RIBA_IREN_NPC_ID]);
        s.add_kill_id(&[ORFEN_NPC_ID]);
    }

    /// The constructor's second half.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        ctx.state::<OrfenState>().has_teleported = false;
        let (x, y, z) = ORFEN_SPAWN_LOCATIONS[0];
        let zone = ctx.world().grand_boss_zone_at(x, y, z);
        ctx.state::<OrfenState>().zone = zone;

        let Some(info) = ctx.world().grand_boss_info(ORFEN_NPC_ID).cloned() else {
            // Java: `getStatSet` answers null and the constructor throws.
            warn!("Orfen: no grandboss_data row for {ORFEN_NPC_ID}, the script does not start.");
            return;
        };
        let status = ctx.world().grand_boss_status(ORFEN_NPC_ID).unwrap_or(-1);
        if status == STATUS_DEAD {
            // Load the unlock date and time for Orfen from DB.
            let remaining_millis = info.respawn_time - ctx.now_ms();

            // If Orfen is locked until a certain time, start unlock timer if time has not yet expired.
            if remaining_millis > 0 {
                ctx.start_quest_timer(EVENT_ORFEN_UNLOCK, remaining_millis, None, None, false);
            } else if !ctx.world().boss_writes_enabled() {
                // D-54: the respawn of a dead row waits for the other server's alive row.
                ctx.notify_event(EVENT_ORFEN_UNLOCK, None, None);
            } else {
                // Respawn time has already elapsed while the server was offline. Spawn Orfen immediately.
                spawn_orfen_at_random_nest(ctx);
            }
        } else {
            // D-54: with the switch off a row that is not alive is the other server's.
            if !row_lets_boss_run(ctx, ORFEN_NPC_ID) {
                info!("Orfen: the shared state row says {status}; the AI waits for an alive row (D-54).");
                return;
            }
            let Some(orfen) =
                ctx.add_spawn(ORFEN_NPC_ID, info.loc_x, info.loc_y, info.loc_z, info.heading, false, 0, 0)
            else {
                return;
            };
            ctx.world_mut()
                .set_current_hp_mp(orfen.object_id, info.current_hp, info.current_mp);
            spawn_boss(ctx, orfen);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            EVENT_ORFEN_UNLOCK => {
                // D-54: on the shared database the row decides; read it again first.
                if !ctx.world().boss_writes_enabled() {
                    ctx.world_mut()
                        .reread_grand_boss_for_script(ORFEN_NPC_ID, self.name(), EVENT_ROW_READ);
                    return None;
                }
                spawn_orfen_at_random_nest(ctx);
            }
            EVENT_ROW_READ => {
                // D-54: the answer of the read `orfen_unlock` asked for.
                match ctx.world().grand_boss_status(ORFEN_NPC_ID) {
                    Some(STATUS_ALIVE) => spawn_orfen_at_random_nest(ctx),
                    Some(STATUS_DEAD) => {
                        let respawn = ctx.world().grand_boss_info(ORFEN_NPC_ID).map_or(0, |i| i.respawn_time);
                        let remaining = respawn - ctx.now_ms();
                        if remaining > 0 {
                            ctx.start_quest_timer(EVENT_ORFEN_UNLOCK, remaining, None, None, false);
                        } else {
                            info!("Orfen: the shared state row says dead; Orfen stays away (D-54).");
                        }
                    }
                    status => info!("Orfen: the shared state row says {status:?}; Orfen stays away (D-54)."),
                }
            }
            EVENT_CHECK_ORFEN_POSITION => {
                let npc = npc?;
                let Some(zone) = ctx.state::<OrfenState>().zone else {
                    // Java: `ZONE` is null and the timer task throws.
                    warn!("Orfen: no boss zone holds the nest, the position check does nothing.");
                    return None;
                };
                let has_teleported = ctx.state::<OrfenState>().has_teleported;
                let inside = ctx
                    .world()
                    .is_inside_zone_by_position(zone, "BossZone", npc.object_id);
                let (current_hp, max_hp) = ctx.world().hp_and_max(npc.object_id).unwrap_or((0.0, 0.0));
                if (has_teleported && (current_hp > (max_hp * ORFEN_RETURN_HP_FRACTION)))
                    || (!inside && !has_teleported)
                {
                    let index = ctx.get_random(3) + 1;
                    set_spawn_point(ctx, npc, index as usize);
                    ctx.state::<OrfenState>().has_teleported = false;
                } else if has_teleported && !inside {
                    set_spawn_point(ctx, npc, 0);
                }
            }
            EVENT_CHECK_MINION_LOCATION => {
                let npc = npc?;
                let minions = ctx.state::<OrfenState>().minions.clone();
                for minion in minions {
                    if !ctx.is_inside_radius_2d(npc.object_id, minion, MINION_FOLLOW_RADIUS)
                        && let Some((x, y, z)) = ctx.world().position_of(npc.object_id)
                    {
                        ctx.world_mut()
                            .tele_to_location(minion, TeleportTarget::at(x, y, z).with_instance(0));
                        ctx.world_mut().clear_aggro_list(minion);
                    }
                }
            }
            EVENT_DESPAWN_MINIONS => {
                let minions = std::mem::take(&mut ctx.state::<OrfenState>().minions);
                for minion in minions {
                    ctx.world_mut().decay(minion);
                }
            }
            EVENT_SPAWN_MINION => {
                let npc = npc?;
                if let Some((x, y, z)) = ctx.world().position_of(npc.object_id)
                    && let Some(minion) = ctx.add_spawn(RAIKEL_LEOS_NPC_ID, x, y, z, 0, false, 0, 0)
                {
                    ctx.world_mut().set_is_raid_minion(minion.object_id, true);
                    ctx.state::<OrfenState>().minions.push(minion.object_id);
                }
            }
            _ => {}
        }

        None
    }

    /// Handles skills seen by Orfen, including the teleport reaction.
    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        _targets: &[ObjectId],
        is_summon: bool,
    ) {
        // `caster.getSummon()` can be null: Java then throws in `isInsideRadius2D`.
        let original_caster = if is_summon {
            let Some(summon) = ctx.summon_of(caster) else {
                return;
            };
            summon
        } else {
            caster
        };
        if ctx.world().skill_effect_point(skill).is_some_and(|point| point > 0)
            && (ctx.get_random(SKILL_SEE_REACTION_CHANCE) == 0)
            && ctx.is_inside_radius_2d(npc.object_id, original_caster, TELEPORT_SKILL_MAX_RANGE)
        {
            let message_index = ctx.get_random(4);
            let name = ctx.player_name(caster);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                ORFEN_MESSAGES[message_index as usize],
                &[&name],
            );

            tele_to_npc(ctx, npc, original_caster);
            ctx.world_mut().set_npc_target(npc.object_id, Some(original_caster));
            ctx.world_mut().do_cast(npc.object_id, PARALYSIS);
        }
    }

    /// Handles faction call logic for Raikel Leos and Riba Iren.
    fn on_faction_call(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caller: ScriptNpc,
        attacker: ObjectId,
        _is_summon: bool,
    ) {
        if ctx.world().is_casting_now(npc.object_id) {
            return;
        }

        let npc_id = npc.id;
        let caller_id = caller.id;
        if (npc_id == RAIKEL_LEOS_NPC_ID) && (ctx.get_random(RAIKEL_FACTION_SKILL_CHANCE) == 0) {
            ctx.world_mut().set_npc_target(npc.object_id, Some(attacker));
            ctx.world_mut().do_cast(npc.object_id, BLOW);
        } else if npc_id == RIBA_IREN_NPC_ID {
            let mut heal_chance = 1;
            if caller_id == ORFEN_NPC_ID {
                heal_chance = RIBA_IREN_ORFEN_HEAL_CHANCE;
            }

            let (caller_hp, caller_max_hp) = ctx.world().hp_and_max(caller.object_id).unwrap_or((0.0, 0.0));
            if (caller_id != RIBA_IREN_NPC_ID)
                && (caller_hp < (caller_max_hp / 2.0))
                && (ctx.get_random(RIBA_IREN_BASE_HEAL_CHANCE) < heal_chance)
            {
                ctx.world_mut().npc_set_intention_idle(npc.object_id);
                ctx.world_mut().set_npc_target(npc.object_id, Some(caller.object_id));
                ctx.world_mut().do_cast(npc.object_id, ORFEN_HEAL);
            }
        }
    }

    /// Handles attack events for Orfen and Riba Iren.
    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let npc_id = npc.id;
        let (current_hp, max_hp) = ctx.world().hp_and_max(npc.object_id).unwrap_or((0.0, 0.0));
        if npc_id == ORFEN_NPC_ID {
            let has_teleported = ctx.state::<OrfenState>().has_teleported;
            if !has_teleported && ((current_hp - f64::from(damage)) < (max_hp * ORFEN_TELEPORT_HP_FRACTION)) {
                ctx.state::<OrfenState>().has_teleported = true;
                set_spawn_point(ctx, npc, 0);
            } else if ctx.is_inside_radius_2d(npc.object_id, attacker, TELEPORT_SKILL_MAX_RANGE)
                && !ctx.is_inside_radius_2d(npc.object_id, attacker, TELEPORT_SKILL_MIN_RANGE)
                && (ctx.get_random(ORFEN_ATTACK_TELEPORT_CHANCE) == 0)
            {
                let message_index = ctx.get_random(3);
                let name = ctx.player_name(attacker);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    ORFEN_MESSAGES[message_index as usize],
                    &[&name],
                );

                tele_to_npc(ctx, npc, attacker);
                ctx.world_mut().set_npc_target(npc.object_id, Some(attacker));
                ctx.world_mut().do_cast(npc.object_id, PARALYSIS);
            }
        } else if (npc_id == RIBA_IREN_NPC_ID)
            && !ctx.world().is_casting_now(npc.object_id)
            && ((current_hp - f64::from(damage)) < (max_hp * RIBA_IREN_HEAL_HP_FRACTION))
        {
            ctx.world_mut().set_npc_target(npc.object_id, Some(attacker));
            ctx.world_mut().do_cast(npc.object_id, ORFEN_HEAL);
        }
    }

    /// Handles Orfen and Raikel Leos death, including respawn scheduling.
    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let npc_id = npc.id;
        if npc_id == ORFEN_NPC_ID {
            let at = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
            ctx.world()
                .broadcast_packet(npc.object_id, &npc_sound("BS02_D", npc.object_id, at));
            ctx.world_mut().set_grand_boss_status(ORFEN_NPC_ID, STATUS_DEAD);

            let grand_boss = &ctx.world().boss_settings().grand_boss;
            let base_interval_millis = i64::from(grand_boss.orfen_spawn_interval) * MILLIS_PER_HOUR;
            let random_range_millis = i64::from(grand_boss.orfen_spawn_random) * MILLIS_PER_HOUR;
            let respawn_time = base_interval_millis
                + ctx
                    .world_mut()
                    .rng()
                    .get_i64(-random_range_millis, random_range_millis);

            // Next respawn time.
            let next_respawn_time = ctx.now_ms() + respawn_time;
            info!("Orfen will respawn at: {}", date_time_string(next_respawn_time));

            ctx.start_quest_timer(EVENT_ORFEN_UNLOCK, respawn_time, None, None, false);

            // Save respawn time so that the info is maintained past reboots.
            if let Some(mut info) = ctx.world().grand_boss_info(ORFEN_NPC_ID).cloned() {
                info.respawn_time = next_respawn_time;
                ctx.world_mut().set_grand_boss_info(ORFEN_NPC_ID, info);
            }

            ctx.cancel_quest_timer(EVENT_CHECK_MINION_LOCATION, Some(npc), None);
            ctx.cancel_quest_timer(EVENT_CHECK_ORFEN_POSITION, Some(npc), None);
            ctx.start_quest_timer(EVENT_DESPAWN_MINIONS, MINION_DESPAWN_DELAY_MS, None, None, false);
            ctx.cancel_quest_timers(EVENT_SPAWN_MINION);
        } else if (ctx.world().grand_boss_status(ORFEN_NPC_ID) == Some(STATUS_ALIVE))
            && (npc_id == RAIKEL_LEOS_NPC_ID)
        {
            // NOTE(java-quirk): `setup` registers `addKillId(ORFEN_NPC_ID)` only, so this branch
            // never runs; and its timer would be bound to the corpse, which `Npc.onDecay()` stops
            // long before the six minutes. Ported as it is.
            ctx.state::<OrfenState>().minions.retain(|m| *m != npc.object_id);
            ctx.start_quest_timer(EVENT_SPAWN_MINION, MINION_RESPAWN_DELAY_MS, Some(npc), None, false);
        }
    }
}

/// `PlaySound(1, file, 1, objectId, x, y, z)`.
fn npc_sound(file: &str, object_id: ObjectId, (x, y, z): (i32, i32, i32)) -> PlaySound {
    PlaySound {
        unknown1: 1,
        sound_file: file.to_owned(),
        unknown3: 1,
        unknown4: object_id.0,
        unknown5: x,
        unknown6: y,
        unknown7: z,
        unknown8: 0,
    }
}

/// `originalCaster.teleToLocation(npc.getLocation())`.
fn tele_to_npc(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, who: ObjectId) {
    if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
        ctx.world_mut()
            .tele_to_location(who, TeleportTarget::at(x, y, z).with_instance(0));
    }
}

/// The nest Orfen comes back to: `getRandom(10)` below 4 / 7 picks location 1 / 2, else 3.
fn spawn_orfen_at_random_nest(ctx: &mut ScriptCtx<'_>) {
    let random_index = ctx.get_random(10);
    let (x, y, z) = if random_index < 4 {
        ORFEN_SPAWN_LOCATIONS[1]
    } else if random_index < 7 {
        ORFEN_SPAWN_LOCATIONS[2]
    } else {
        ORFEN_SPAWN_LOCATIONS[3]
    };

    let orfen = ctx.add_spawn(ORFEN_NPC_ID, x, y, z, 0, false, 0, 0);
    ctx.world_mut().set_grand_boss_status(ORFEN_NPC_ID, STATUS_ALIVE);
    if let Some(orfen) = orfen {
        spawn_boss(ctx, orfen);
    }
}

/// Updates Orfen spawn point and teleports it to a predefined location index.
fn set_spawn_point(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, index: usize) {
    let (x, y, z) = ORFEN_SPAWN_LOCATIONS[index];
    ctx.world_mut().clear_aggro_list(npc.object_id);
    ctx.world_mut().npc_set_intention_idle(npc.object_id);
    ctx.world_mut().set_npc_spawn_point(npc.object_id, x, y, z);
    ctx.world_mut()
        .tele_to_location(npc.object_id, TeleportTarget::at(x, y, z).with_instance(0));
}

/// Registers Orfen in the boss manager, plays spawn sound and spawns minions.
fn spawn_boss(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    ctx.world_mut().add_grand_boss(ORFEN_NPC_ID, npc.object_id);
    let at = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
    ctx.world()
        .broadcast_packet(npc.object_id, &npc_sound("BS01_A", npc.object_id, at));

    ctx.start_quest_timer(
        EVENT_CHECK_ORFEN_POSITION,
        CHECK_POSITION_INTERVAL_MS,
        Some(npc),
        None,
        true,
    );

    // Spawn minions.
    let (boss_x, boss_y, boss_z) = at;
    for (dx, dy) in [(100, 100), (100, -100), (-100, 100), (-100, -100)] {
        if let Some(minion) = ctx.add_spawn(RAIKEL_LEOS_NPC_ID, boss_x + dx, boss_y + dy, boss_z, 0, false, 0, 0) {
            ctx.world_mut().set_is_raid_minion(minion.object_id, true);
            ctx.state::<OrfenState>().minions.push(minion.object_id);
        }
    }

    ctx.start_quest_timer(
        EVENT_CHECK_MINION_LOCATION,
        CHECK_MINION_INTERVAL_MS,
        Some(npc),
        None,
        true,
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{BossReadKind, BossReadThen, BossWrite, GrandBossData, ScriptRegistry};

    use super::*;
    use crate::ai::bosses::test_support::*;
    use crate::testing::{ScriptTest, names_to};

    const MONSTER: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision>"#;

    fn npcs() -> String {
        format!(
            r#"<npc id="29014" type="GrandBoss" level="87" name="Orfen" title="Harpy">{MONSTER}<corpseTime>21</corpseTime></npc>
<npc id="29016" type="Monster" level="79" name="Raikel Leos">{MONSTER}</npc>
<npc id="29018" type="Monster" level="79" name="Riba Iren">{MONSTER}</npc>"#
        )
    }

    /// The nest (`Orfen Boss` zone shape: a square around the first spawn location).
    const ORFEN_ZONE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="orfen_nest" id="12013" type="BossZone" shape="NPoly" minZ="-5000" maxZ="-3500">
		<stat name="InvadeTime" val="1800000" />
		<node X="42000" Y="15000" />
		<node X="46000" Y="15000" />
		<node X="46000" Y="19000" />
		<node X="42000" Y="19000" />
	</zone>
</list>
"#;

    fn setup_row(status: i32, respawn_time: i64, writes: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Orfen);
        boss_test(
            registry,
            &npcs(),
            "",
            &[("data/zones/orfen.xml", ORFEN_ZONE)],
            vec![row(ORFEN_NPC_ID, status, respawn_time, ORFEN_SPAWN_LOCATIONS[0])],
            writes,
        )
    }

    fn has_timer(t: &mut ScriptTest, name: &str, npc: Option<Oid>) -> bool {
        let mut found = false;
        t.world.run_as_script("Orfen", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            found = ctx.has_quest_timer(name, npc, None);
        });
        found
    }

    fn fire(t: &mut ScriptTest, event: &str, npc: Option<Oid>) {
        t.world.run_as_script("Orfen", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            ctx.notify_event(event, npc, None);
        });
    }

    fn status(t: &ScriptTest) -> Option<i32> {
        t.world.grand_boss_status(ORFEN_NPC_ID)
    }

    fn xy(t: &ScriptTest, id: Oid) -> (i32, i32) {
        let (x, y, _) = t.world.position_of(id).unwrap();
        (x, y)
    }

    fn live_minions(t: &ScriptTest) -> Vec<Oid> {
        npcs_of(t, RAIKEL_LEOS_NPC_ID)
            .into_iter()
            .filter(|m| t.world.object(*m).is_some())
            .collect()
    }

    fn attack(t: &mut ScriptTest, npc: Oid, attacker: i32, damage: i32) {
        t.world.run_as_script("Orfen", |ctx| {
            let npc = ctx.script_npc(npc).unwrap();
            Orfen.on_attack(ctx, npc, Oid(attacker), damage, false, None);
        });
    }

    #[test]
    fn an_alive_row_restores_orfen_and_four_minions() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        let orfen = t.world.grand_boss_npc(ORFEN_NPC_ID).unwrap();
        assert_eq!(xy(&t, orfen), (ORFEN_SPAWN_LOCATIONS[0].0, ORFEN_SPAWN_LOCATIONS[0].1));
        let (ox, oy) = xy(&t, orfen);
        let mut places: Vec<(i32, i32)> = live_minions(&t).into_iter().map(|m| xy(&t, m)).collect();
        places.sort_unstable();
        let mut expected = vec![
            (ox + 100, oy + 100),
            (ox + 100, oy - 100),
            (ox - 100, oy + 100),
            (ox - 100, oy - 100),
        ];
        expected.sort_unstable();
        assert_eq!(places, expected);
        for m in live_minions(&t) {
            assert!(t.world.is_raid_minion(m));
        }
        assert!(has_timer(&mut t, EVENT_CHECK_ORFEN_POSITION, Some(orfen)));
        assert!(has_timer(&mut t, EVENT_CHECK_MINION_LOCATION, Some(orfen)));
        assert!(grand_writes(&t.take_boss()).is_empty());
    }

    #[test]
    fn a_dead_row_waits_or_spawns_orfen_at_a_roaming_nest() {
        let mut t = setup_row(STATUS_DEAD, now_ms() + 3_600_000, true);
        assert!(npcs_of(&t, ORFEN_NPC_ID).is_empty());
        assert!(has_timer(&mut t, EVENT_ORFEN_UNLOCK, None));

        let t = setup_row(STATUS_DEAD, now_ms() - 1000, true);
        let orfen = t.world.grand_boss_npc(ORFEN_NPC_ID).unwrap();
        let at = xy(&t, orfen);
        assert!(
            ORFEN_SPAWN_LOCATIONS[1..]
                .iter()
                .any(|(x, y, _)| (*x, *y) == at),
            "{at:?}"
        );
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(live_minions(&t).len(), 4);
    }

    #[test]
    fn with_the_switch_off_the_row_decides_the_unlock() {
        let mut t = setup_row(STATUS_DEAD, now_ms() - 1000, false);
        assert!(npcs_of(&t, ORFEN_NPC_ID).is_empty());
        let then = BossReadThen::ScriptEvent {
            script: "Orfen",
            event: EVENT_ROW_READ,
        };
        assert_eq!(
            rereads(&t.take_boss()),
            [(BossReadKind::GrandBoss(ORFEN_NPC_ID), then)]
        );
        answer(
            &mut t,
            then,
            ORFEN_NPC_ID,
            row(ORFEN_NPC_ID, STATUS_ALIVE, 0, ORFEN_SPAWN_LOCATIONS[0]),
        );
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(npcs_of(&t, ORFEN_NPC_ID).len(), 1);
        assert!(grand_writes(&t.take_boss()).is_empty());

        // The row stays dead: Orfen stays away.
        let mut t = setup_row(STATUS_DEAD, now_ms() - 1000, false);
        t.take_boss();
        answer(
            &mut t,
            then,
            ORFEN_NPC_ID,
            row(ORFEN_NPC_ID, STATUS_DEAD, now_ms() - 5, ORFEN_SPAWN_LOCATIONS[0]),
        );
        assert!(npcs_of(&t, ORFEN_NPC_ID).is_empty());

        let t = setup_row(STATUS_DEAD + 1, 0, false);
        assert!(npcs_of(&t, ORFEN_NPC_ID).is_empty());
    }

    #[test]
    fn below_half_hp_orfen_goes_to_its_lair_and_returns_when_healed() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (43800, 17300, -4342), vec![]));
        let orfen = t.world.grand_boss_npc(ORFEN_NPC_ID).unwrap();
        // The nest is the lair of location 0: 510 - 20 < 500 sends it back there at once.
        t.world.set_current_hp_mp(orfen, 510.0, 500.0);
        assert_eq!(t.world.hp_and_max(orfen).unwrap().1, 1000.0);
        attack(&mut t, orfen, 1, 20);
        assert!(has_teleported(&mut t));
        assert_eq!(xy(&t, orfen), (ORFEN_SPAWN_LOCATIONS[0].0, ORFEN_SPAWN_LOCATIONS[0].1));
        // Inside the nest and hurt: nothing moves it.
        fire(&mut t, EVENT_CHECK_ORFEN_POSITION, Some(orfen));
        assert!(has_teleported(&mut t));
        // Healed over 95%: it goes to a roaming nest and may be hurt again.
        t.world.set_current_hp_mp(orfen, 960.0, 500.0);
        fire(&mut t, EVENT_CHECK_ORFEN_POSITION, Some(orfen));
        assert!(!has_teleported(&mut t));
        let at = xy(&t, orfen);
        assert!(ORFEN_SPAWN_LOCATIONS[1..].iter().any(|(x, y, _)| (*x, *y) == at), "{at:?}");

        // Teleported, hurt and outside the nest: back to the lair (location 0).
        t.world.set_current_hp_mp(orfen, 400.0, 500.0);
        t.world.run_as_script("Orfen", |ctx| ctx.state::<OrfenState>().has_teleported = true);
        fire(&mut t, EVENT_CHECK_ORFEN_POSITION, Some(orfen));
        assert_eq!(xy(&t, orfen), (ORFEN_SPAWN_LOCATIONS[0].0, ORFEN_SPAWN_LOCATIONS[0].1));
        // Not teleported and outside the nest: a roaming nest and `_hasTeleported` stays false.
        t.world.run_as_script("Orfen", |ctx| ctx.state::<OrfenState>().has_teleported = false);
        t.world
            .tele_to_location(orfen, TeleportTarget::at(50000, 17220, -4342).with_instance(0));
        fire(&mut t, EVENT_CHECK_ORFEN_POSITION, Some(orfen));
        let at = xy(&t, orfen);
        assert!(ORFEN_SPAWN_LOCATIONS[1..].iter().any(|(x, y, _)| (*x, *y) == at), "{at:?}");
    }

    fn has_teleported(t: &mut ScriptTest) -> bool {
        let mut value = false;
        t.world.run_as_script("Orfen", |ctx| value = ctx.state::<OrfenState>().has_teleported);
        value
    }

    #[test]
    fn a_hit_from_afar_can_pull_the_attacker_to_orfen() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (43728 + 600, 17220, -4342), vec![]));
        let orfen = t.world.grand_boss_npc(ORFEN_NPC_ID).unwrap();
        t.take();
        let mut pulled = false;
        for _ in 0..200 {
            attack(&mut t, orfen, 1, 1);
            if xy(&t, Oid(1)) != (43728 + 600, 17220) {
                pulled = true;
                break;
            }
        }
        assert!(pulled);
        assert_eq!(xy(&t, Oid(1)), (ORFEN_SPAWN_LOCATIONS[0].0, ORFEN_SPAWN_LOCATIONS[0].1));
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"NPC_SAY".to_owned()));
        assert!(!has_teleported(&mut t));
    }

    #[test]
    fn a_skill_without_effect_point_data_does_not_move_the_caster() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (43728 + 600, 17220, -4342), vec![]));
        let orfen = t.world.grand_boss_npc(ORFEN_NPC_ID).unwrap();
        for _ in 0..50 {
            t.world.run_as_script("Orfen", |ctx| {
                let npc = ctx.script_npc(orfen).unwrap();
                Orfen.on_skill_see(ctx, npc, Oid(1), SkillKey::new(1, 1), &[], false);
            });
        }
        // The test world has no skill table: `getEffectPoint()` is unknown, nobody moves.
        assert_eq!(xy(&t, Oid(1)), (43728 + 600, 17220));
    }

    #[test]
    fn orfen_death_writes_the_row_and_sends_the_minions_away() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (43800, 17300, -4342), vec![]));
        let orfen = t.world.grand_boss_npc(ORFEN_NPC_ID).unwrap();
        t.take();
        t.take_boss();
        let before = now_ms();
        // A pending minion respawn timer is cancelled by name when Orfen dies.
        let minion = live_minions(&t)[0];
        t.world.run_as_script("Orfen", |ctx| {
            let npc = ctx.script_npc(minion);
            ctx.start_quest_timer(EVENT_SPAWN_MINION, MINION_RESPAWN_DELAY_MS, npc, None, false);
        });
        assert!(has_timer(&mut t, EVENT_SPAWN_MINION, Some(minion)));

        assert!(t.world.do_die(orfen, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert!(has_timer(&mut t, EVENT_ORFEN_UNLOCK, None));
        assert!(has_timer(&mut t, EVENT_DESPAWN_MINIONS, None));
        assert!(!has_timer(&mut t, EVENT_CHECK_ORFEN_POSITION, Some(orfen)));
        assert!(!has_timer(&mut t, EVENT_CHECK_MINION_LOCATION, Some(orfen)));
        assert!(!has_timer(&mut t, EVENT_SPAWN_MINION, Some(minion)));
        let writes = grand_writes(&t.take_boss());
        assert_eq!(
            writes[0],
            BossWrite::GrandBossStatus {
                boss_id: ORFEN_NPC_ID,
                status: STATUS_DEAD
            }
        );
        let BossWrite::GrandBoss(GrandBossData { respawn_time, .. }) = writes[1].clone() else {
            panic!("{writes:?}");
        };
        let hour = 3_600_000;
        assert!(respawn_time >= before + 28 * hour && respawn_time <= now_ms() + 68 * hour);

        run_ms(&mut t, 20_000);
        assert!(live_minions(&t).is_empty());
    }

    #[test]
    fn raikel_leos_kills_never_reach_the_script() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (43800, 17300, -4342), vec![]));
        let minion = live_minions(&t)[0];
        let at = xy(&t, minion);
        assert!(t.world.do_die(minion, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        // NOTE(java-quirk): Java registers `addKillId(ORFEN_NPC_ID)` only, so `onKill` never sees
        // a Raikel Leos and no respawn timer starts.
        assert!(!has_timer(&mut t, EVENT_SPAWN_MINION, Some(minion)));
        assert_eq!(live_minions(&t).len(), 3);
        // The event itself, fired with a corpse, spawns one where it fell.
        fire(&mut t, EVENT_SPAWN_MINION, Some(minion));
        let now = live_minions(&t);
        assert_eq!(now.len(), 4);
        assert!(now.iter().any(|m| *m != minion && xy(&t, *m) == at));
    }

    #[test]
    fn a_minion_that_strayed_is_brought_back_to_orfen() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        let orfen = t.world.grand_boss_npc(ORFEN_NPC_ID).unwrap();
        let minion = live_minions(&t)[0];
        t.world
            .tele_to_location(minion, TeleportTarget::at(43728 + 5000, 17220, -4342).with_instance(0));
        assert_eq!(xy(&t, minion).0, 43728 + 5000);
        fire(&mut t, EVENT_CHECK_MINION_LOCATION, Some(orfen));
        assert_eq!(xy(&t, minion), xy(&t, orfen));
    }

    #[test]
    fn riba_iren_heals_the_hurt_orfen() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(1, (43800, 17300, -4342), vec![]));
        let orfen = t.world.grand_boss_npc(ORFEN_NPC_ID).unwrap();
        let riba = t
            .world
            .script_add_spawn(RIBA_IREN_NPC_ID, (43900, 17300, -4342), 0, false, 0, 0)
            .unwrap();
        t.world.set_current_hp_mp(orfen, 300.0, 500.0);
        let mut targeted = false;
        for _ in 0..30 {
            t.world.run_as_script("Orfen", |ctx| {
                let npc = ctx.script_npc(riba).unwrap();
                let caller = ctx.script_npc(orfen).unwrap();
                Orfen.on_faction_call(ctx, npc, caller, Oid(1), false);
            });
            if t.world.creature_target(riba) == Some(orfen) {
                targeted = true;
                break;
            }
        }
        assert!(targeted);
    }
}
