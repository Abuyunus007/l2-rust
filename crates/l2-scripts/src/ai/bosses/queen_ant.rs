//! Queen Ant, the Ant Nest grand boss: the row restored at start, the players moved out of the
//! nest when she spawns, the larva and the nurses that heal, the raid curse, the minion respawns
//! and the respawn cycle.
// Java: dist/game/data/scripts/ai/bosses/QueenAnt/QueenAnt.java
//
// The Java constructor is `on_load`; the fields are `QueenAntState` (`ctx.state`). Statuses: 0
// alive, 1 dead.
//
// Side by side (D-54, `super` module): with `CycleDatabaseWrites` off the constructor restores
// the boss only from an alive row, and `queen_unlock` reads the row again and brings the Queen back
// only when the row says alive.
//
// Not modelled: `unload` (D-16: no reload).

use l2_protocol_game::server::{PlaySound, SocialAction};
use l2_util::ObjectId;
use l2_world::{Intention, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};
use tracing::{info, warn};

use super::{EVENT_ROW_READ, date_time_string, row_lets_boss_run};

// Boss and minion identifiers.
const QUEEN_ANT_ID: i32 = 29001;
const LARVA_ID: i32 = 29002;
const NURSE_ID: i32 = 29003;
const GUARD_ID: i32 = 29004;
const ROYAL_ID: i32 = 29005;

// NPC identifiers array for event registration.
const MONITORED_NPC_IDS: [i32; 5] = [QUEEN_ANT_ID, LARVA_ID, NURSE_ID, GUARD_ID, ROYAL_ID];

// Nest and spawn coordinates.
const QUEEN_NEST_X: i32 = -21610;
const QUEEN_NEST_Y: i32 = 181594;
const QUEEN_NEST_Z: i32 = -5734;

const LARVA_SPAWN_X: i32 = -21600;
const LARVA_SPAWN_Y: i32 = 179482;
const LARVA_SPAWN_Z: i32 = -5846;

// Teleport destinations when the boss spawns.
const TELEPORT_LOCATION_ONE: (i32, i32, i32) = (-19480, 187344, -5600);
const TELEPORT_LOCATION_TWO: (i32, i32, i32) = (-17928, 180912, -5520);
const TELEPORT_LOCATION_THREE: (i32, i32, i32) = (-23808, 182368, -5600);

// Boss status flags.
const STATUS_ALIVE: i32 = 0;
const STATUS_DEAD: i32 = 1;

// Timed event identifiers.
const EVENT_HEAL: &str = "heal";
const EVENT_ACTION: &str = "action";
const EVENT_QUEEN_UNLOCK: &str = "queen_unlock";
const EVENT_DISTANCE_CHECK: &str = "DISTANCE_CHECK";

// Timers and behavior thresholds.
const HEAL_INTERVAL_MILLIS: i64 = 1000;
const ACTION_INTERVAL_MILLIS: i64 = 10_000;
const DISTANCE_CHECK_INTERVAL_MILLIS: i64 = 5000;
const RAID_CURSE_LEVEL_DIFFERENCE: i32 = 8;
const RAID_CURSE_CAST_TIME_MILLIS: i32 = 300;

// Respawn related constants.
const MILLIS_PER_HOUR: i64 = 3_600_000;
const ROYAL_RESPAWN_BASE_SECONDS: i32 = 280;
const ROYAL_RESPAWN_VARIATION_SECONDS: i32 = 40;
const NURSE_RESPAWN_DELAY_MILLIS: i32 = 10_000;

// Skill holders.
const NURSE_HEAL_SKILL_ONE: SkillKey = SkillKey { id: 4020, level: 1 };
const NURSE_HEAL_SKILL_TWO: SkillKey = SkillKey { id: 4024, level: 1 };
// `CommonSkill.RAID_CURSE` and `RAID_CURSE2`.
const RAID_CURSE: SkillKey = SkillKey { id: 4215, level: 1 };
const RAID_CURSE2: SkillKey = SkillKey { id: 4515, level: 1 };

// Sound identifiers.
const SOUND_SPAWN: &str = "BS01_A";
const SOUND_DEATH: &str = "BS02_D";

// Boss zone: `getZoneById(12012, BossZone.class)`.
const QUEEN_NEST_ZONE: (i32, &str) = (12012, "BossZone");

/// The fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct QueenAntState {
    /// `_queenInstance`.
    pub(crate) queen: Option<ObjectId>,
    /// `_larvaInstance`.
    pub(crate) larva: Option<ObjectId>,
    /// `_activeNurses`.
    pub(crate) nurses: Vec<ObjectId>,
}

/// Java `ai.bosses.QueenAnt.QueenAnt`.
pub struct QueenAnt;

impl Script for QueenAnt {
    fn name(&self) -> &'static str {
        "QueenAnt"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/QueenAnt"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&MONITORED_NPC_IDS);
        s.add_kill_id(&MONITORED_NPC_IDS);
        s.add_aggro_range_enter_id(&MONITORED_NPC_IDS);
        s.add_faction_call_id(&[NURSE_ID]);
    }

    /// The constructor's second half.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let Some(boss_info) = ctx.world().grand_boss_info(QUEEN_ANT_ID).cloned() else {
            // Java: `getStatSet` answers null and the constructor throws.
            warn!("QueenAnt: no grandboss_data row for {QUEEN_ANT_ID}, the script does not start.");
            return;
        };
        let current_status = ctx.world().grand_boss_status(QUEEN_ANT_ID).unwrap_or(-1);

        if current_status == STATUS_DEAD {
            let remaining_delay = boss_info.respawn_time - ctx.now_ms();
            if remaining_delay > 0 {
                ctx.start_quest_timer(EVENT_QUEEN_UNLOCK, remaining_delay, None, None, false);
            } else if !ctx.world().boss_writes_enabled() {
                // D-54: the respawn of a dead row waits for the other server's alive row.
                ctx.notify_event(EVENT_QUEEN_UNLOCK, None, None);
            } else {
                spawn_queen_at_nest(ctx);
            }
        } else {
            // D-54: with the switch off a row that is not alive is the other server's.
            if !row_lets_boss_run(ctx, QUEEN_ANT_ID) {
                info!(
                    "QueenAnt: the shared state row says {current_status}; the AI waits for an alive row (D-54)."
                );
                return;
            }
            let (mut loc_x, mut loc_y, mut loc_z) = (boss_info.loc_x, boss_info.loc_y, boss_info.loc_z);
            // Java: `_queenNestZone` is null without the zone and the constructor throws; the
            // default place is the safe reading.
            if !inside_nest(ctx, loc_x, loc_y, loc_z) {
                loc_x = QUEEN_NEST_X;
                loc_y = QUEEN_NEST_Y;
                loc_z = QUEEN_NEST_Z;
            }

            let Some(queen) =
                ctx.add_spawn(QUEEN_ANT_ID, loc_x, loc_y, loc_z, boss_info.heading, false, 0, 0)
            else {
                return;
            };
            ctx.world_mut()
                .set_current_hp_mp(queen.object_id, boss_info.current_hp, boss_info.current_mp);
            spawn_boss(ctx, queen);
        }
    }

    /// Handles scheduled events such as healing, social actions, respawn and distance checks.
    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            EVENT_HEAL => handle_heal(ctx),
            EVENT_ACTION => {
                if let Some(npc) = npc
                    && ctx.get_random(3) == 0
                {
                    let action_id = if ctx.get_random(2) == 0 { 3 } else { 4 };
                    ctx.world().broadcast_packet(
                        npc.object_id,
                        &SocialAction {
                            object_id: npc.object_id.0,
                            action_id,
                        },
                    );
                }
            }
            EVENT_QUEEN_UNLOCK => {
                // D-54: on the shared database the row decides; read it again first.
                if !ctx.world().boss_writes_enabled() {
                    ctx.world_mut()
                        .reread_grand_boss_for_script(QUEEN_ANT_ID, self.name(), EVENT_ROW_READ);
                    return None;
                }
                spawn_queen_at_nest(ctx);
            }
            EVENT_ROW_READ => {
                // D-54: the answer of the read `queen_unlock` asked for.
                match ctx.world().grand_boss_status(QUEEN_ANT_ID) {
                    Some(STATUS_ALIVE) => spawn_queen_at_nest(ctx),
                    Some(STATUS_DEAD) => {
                        let respawn = ctx
                            .world()
                            .grand_boss_info(QUEEN_ANT_ID)
                            .map_or(0, |i| i.respawn_time);
                        let remaining = respawn - ctx.now_ms();
                        if remaining > 0 {
                            ctx.start_quest_timer(EVENT_QUEEN_UNLOCK, remaining, None, None, false);
                        } else {
                            info!("QueenAnt: the shared state row says dead; the Queen stays away (D-54).");
                        }
                    }
                    status => {
                        info!("QueenAnt: the shared state row says {status:?}; the Queen stays away (D-54).")
                    }
                }
            }
            EVENT_DISTANCE_CHECK => {
                let queen = ctx.state::<QueenAntState>().queen;
                match queen {
                    None => ctx.cancel_quest_timers(EVENT_DISTANCE_CHECK),
                    Some(queen) if ctx.world().is_dead(queen) => {
                        ctx.cancel_quest_timers(EVENT_DISTANCE_CHECK)
                    }
                    Some(queen) => {
                        let (zone_id, zone_type) = QUEEN_NEST_ZONE;
                        if ctx.world().zone_by_id(zone_id, zone_type).is_some()
                            && !ctx.world().is_inside_zone_by_position(zone_id, zone_type, queen)
                        {
                            ctx.world_mut().clear_aggro_list(queen);
                            ctx.world_mut().npc_set_intention_move_to(
                                queen,
                                QUEEN_NEST_X,
                                QUEEN_NEST_Y,
                                QUEEN_NEST_Z,
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        None
    }

    /// Applies initial configuration when an NPC of this script spawns.
    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        match npc.id {
            LARVA_ID => {
                ctx.set_immobilized(npc, true);
                ctx.world_mut().set_mortal(npc.object_id, false);
                ctx.world_mut().set_is_raid_minion(npc.object_id, true);
            }
            NURSE_ID => {
                ctx.disable_core_ai(npc, true);
                ctx.world_mut().set_is_raid_minion(npc.object_id, true);
                ctx.state::<QueenAntState>().nurses.push(npc.object_id);
            }
            ROYAL_ID | GUARD_ID => {
                ctx.world_mut().set_is_raid_minion(npc.object_id, true);
            }
            QUEEN_ANT_ID => {
                ctx.cancel_quest_timer(EVENT_DISTANCE_CHECK, Some(npc), None);
                ctx.start_quest_timer(
                    EVENT_DISTANCE_CHECK,
                    DISTANCE_CHECK_INTERVAL_MILLIS,
                    Some(npc),
                    None,
                    true,
                );
            }
            _ => {}
        }
    }

    /// Handles faction calls, typically healing requests from allied NPCs.
    fn on_faction_call(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caller: ScriptNpc,
        _attacker: ObjectId,
        _is_summon: bool,
    ) {
        let (caller_hp, caller_max_hp) = ctx.world().hp_and_max(caller.object_id).unwrap_or((0.0, 0.0));
        if !ctx.world().is_casting_now(npc.object_id)
            && (ctx.world().intention_of(npc.object_id) != Intention::Cast)
            && (caller_hp < caller_max_hp)
        {
            ctx.world_mut()
                .set_npc_target(npc.object_id, Some(caller.object_id));
            ctx.world_mut()
                .attackable_use_magic(npc.object_id, NURSE_HEAL_SKILL_ONE);
        }
    }

    /// Handles raid curse logic when a player or summon enters aggro range.
    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        is_summon: bool,
    ) {
        if ctx.is_gm(player) && ctx.world().object(player).is_some_and(|e| e.invisible) {
            return;
        }

        let (attacker_is_mage, attacker_character) = if is_summon {
            (false, ctx.summon_of(player))
        } else {
            (ctx.is_mage_class(player), Some(player))
        };

        let Some(attacker_character) = attacker_character else {
            return;
        };

        let attacker_level = ctx.world().stats(attacker_character).map_or(0, |s| s.level());
        let npc_level = ctx.world().stats(npc.object_id).map_or(0, |s| s.level());
        if !ctx.world().raid_curse_disabled() && ((attacker_level - npc_level) > RAID_CURSE_LEVEL_DIFFERENCE)
        {
            let mut curse = None;

            if attacker_is_mage {
                if !ctx.world().is_muted(attacker_character) && (ctx.get_random(4) == 0) {
                    curse = Some(RAID_CURSE);
                }
            } else if !ctx.world().is_paralyzed(attacker_character) && (ctx.get_random(4) == 0) {
                curse = Some(RAID_CURSE2);
            }

            if let Some(curse) = curse {
                ctx.broadcast_magic_skill_use(
                    npc.object_id,
                    npc.object_id,
                    attacker_character,
                    curse.id,
                    curse.level,
                    RAID_CURSE_CAST_TIME_MILLIS,
                    0,
                );
                ctx.world_mut()
                    .apply_effects(npc.object_id, attacker_character, curse);
            }

            ctx.stop_hating(npc, attacker_character);
        }
    }

    /// Handles kill events for Queen Ant and her minions.
    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let npc_id = npc.id;
        if npc_id == QUEEN_ANT_ID {
            let at = ctx.world().position_of(npc.object_id).unwrap_or((0, 0, 0));
            ctx.world()
                .broadcast_packet(npc.object_id, &npc_sound(SOUND_DEATH, npc.object_id, at));
            ctx.world_mut().set_grand_boss_status(QUEEN_ANT_ID, STATUS_DEAD);

            let grand_boss = &ctx.world().boss_settings().grand_boss;
            let base_interval_millis = i64::from(grand_boss.queen_ant_spawn_interval) * MILLIS_PER_HOUR;
            let random_range_millis = i64::from(grand_boss.queen_ant_spawn_random) * MILLIS_PER_HOUR;
            let respawn_delay = base_interval_millis
                + ctx
                    .world_mut()
                    .rng()
                    .get_i64(-random_range_millis, random_range_millis);

            let next_respawn_time = ctx.now_ms() + respawn_delay;
            info!(
                "Queen Ant will respawn at: {}",
                date_time_string(next_respawn_time)
            );

            ctx.start_quest_timer(EVENT_QUEEN_UNLOCK, respawn_delay, None, None, false);
            ctx.cancel_quest_timer(EVENT_ACTION, Some(npc), None);
            ctx.cancel_quest_timer(EVENT_HEAL, None, None);

            if let Some(mut boss_info) = ctx.world().grand_boss_info(QUEEN_ANT_ID).cloned() {
                boss_info.respawn_time = next_respawn_time;
                ctx.world_mut().set_grand_boss_info(QUEEN_ANT_ID, boss_info);
            }

            let state = ctx.state::<QueenAntState>();
            state.nurses.clear();
            let larva = state.larva.take();
            state.queen = None;

            if let Some(larva) = larva {
                ctx.world_mut().delete_npc(larva);
            }

            ctx.cancel_quest_timers(EVENT_DISTANCE_CHECK);
        } else {
            let queen = ctx.state::<QueenAntState>().queen;
            if queen.is_some_and(|queen| !ctx.world().is_alike_dead(queen)) {
                if npc_id == ROYAL_ID {
                    if let Some(leader) = ctx.world().npc_leader(npc.object_id) {
                        let respawn_delay = (ROYAL_RESPAWN_BASE_SECONDS
                            + ctx.get_random(ROYAL_RESPAWN_VARIATION_SECONDS))
                            * 1000;
                        ctx.world_mut().minion_died(leader, npc.object_id, respawn_delay);
                    }
                } else if npc_id == NURSE_ID {
                    ctx.state::<QueenAntState>()
                        .nurses
                        .retain(|n| *n != npc.object_id);

                    if let Some(leader) = ctx.world().npc_leader(npc.object_id) {
                        ctx.world_mut()
                            .minion_died(leader, npc.object_id, NURSE_RESPAWN_DELAY_MILLIS);
                    }
                }
            }
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

/// `_queenNestZone.isInsideZone(x, y, z)`.
fn inside_nest(ctx: &ScriptCtx<'_>, x: i32, y: i32, z: i32) -> bool {
    let (zone_id, zone_type) = QUEEN_NEST_ZONE;
    ctx.world()
        .zone_by_id(zone_id, zone_type)
        .is_some_and(|zone| zone.is_inside_zone(x, y, z))
}

/// `queen_unlock` and the constructor of a dead row whose time is over: the Queen at the nest.
fn spawn_queen_at_nest(ctx: &mut ScriptCtx<'_>) {
    let queen = ctx.add_spawn(
        QUEEN_ANT_ID,
        QUEEN_NEST_X,
        QUEEN_NEST_Y,
        QUEEN_NEST_Z,
        0,
        false,
        0,
        0,
    );
    ctx.world_mut().set_grand_boss_status(QUEEN_ANT_ID, STATUS_ALIVE);
    if let Some(queen) = queen {
        spawn_boss(ctx, queen);
    }
}

/// Spawns the boss, teleports players out of the nest and starts periodic tasks.
fn spawn_boss(ctx: &mut ScriptCtx<'_>, boss: ScriptNpc) {
    ctx.world_mut().add_grand_boss(QUEEN_ANT_ID, boss.object_id);

    let (zone_id, zone_type) = QUEEN_NEST_ZONE;
    if ctx.world().zone_by_id(zone_id, zone_type).is_some() {
        let initial_roll = ctx.get_random(100);
        let (x, y, z) = if initial_roll < 33 {
            TELEPORT_LOCATION_ONE
        } else if ctx.get_random(100) < 50 {
            TELEPORT_LOCATION_TWO
        } else {
            TELEPORT_LOCATION_THREE
        };
        // `movePlayersTo(loc)`: every online player inside.
        for player in ctx.world().zone_players_inside(zone_id, zone_type) {
            ctx.world_mut()
                .tele_to_location(player, l2_world::TeleportTarget::at(x, y, z).with_instance(0));
        }
    }

    ctx.start_quest_timer(EVENT_ACTION, ACTION_INTERVAL_MILLIS, Some(boss), None, true);
    ctx.start_quest_timer(EVENT_HEAL, HEAL_INTERVAL_MILLIS, None, None, true);

    let at = ctx.world().position_of(boss.object_id).unwrap_or((0, 0, 0));
    ctx.world()
        .broadcast_packet(boss.object_id, &npc_sound(SOUND_SPAWN, boss.object_id, at));

    ctx.state::<QueenAntState>().queen = Some(boss.object_id);
    let heading = ctx.get_random(360);
    let larva = ctx.add_spawn(
        LARVA_ID,
        LARVA_SPAWN_X,
        LARVA_SPAWN_Y,
        LARVA_SPAWN_Z,
        heading,
        false,
        0,
        0,
    );
    ctx.state::<QueenAntState>().larva = larva.map(|l| l.object_id);
}

/// The `heal` timer: the nurses heal the larva first, then the queen.
fn handle_heal(ctx: &mut ScriptCtx<'_>) {
    let (queen, larva, nurses) = {
        let state = ctx.state::<QueenAntState>();
        (state.queen, state.larva, state.nurses.clone())
    };
    let needs_heal = |ctx: &ScriptCtx<'_>, id: Option<ObjectId>| {
        id.is_some_and(|id| ctx.world().hp_and_max(id).is_some_and(|(hp, max)| hp < max))
    };
    let larva_needs_heal = needs_heal(ctx, larva);
    let queen_needs_heal = needs_heal(ctx, queen);

    for nurse in nurses {
        if ctx.world().is_dead(nurse) || ctx.world().is_casting_now(nurse) {
            continue;
        }

        let nurse_not_casting = ctx.world().intention_of(nurse) != Intention::Cast;

        if larva_needs_heal {
            if (ctx.world().creature_target(nurse) != larva) || nurse_not_casting {
                ctx.world_mut().set_npc_target(nurse, larva);

                let heal_skill = if ctx.get_random_boolean() {
                    NURSE_HEAL_SKILL_ONE
                } else {
                    NURSE_HEAL_SKILL_TWO
                };
                ctx.world_mut().attackable_use_magic(nurse, heal_skill);
            }
            continue;
        }

        if queen_needs_heal {
            if ctx.world().npc_leader(nurse) == larva && larva.is_some() {
                continue;
            }

            if queen.is_some() && ((ctx.world().creature_target(nurse) != queen) || nurse_not_casting) {
                ctx.world_mut().set_npc_target(nurse, queen);
                ctx.world_mut().attackable_use_magic(nurse, NURSE_HEAL_SKILL_ONE);
            }
            continue;
        }

        if nurse_not_casting && ctx.world().creature_target(nurse).is_some() {
            ctx.world_mut().set_npc_target(nurse, None);
        }
    }
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
            r#"<npc id="29001" type="GrandBoss" level="40" name="Queen Ant" title="">
<parameters><minions name="Privates"><npc id="29003" count="2" respawnTime="10" weightPoint="1" /><npc id="29005" count="2" respawnTime="360" weightPoint="1" /></minions></parameters>
{MONSTER}<corpseTime>21</corpseTime></npc>
<npc id="29002" type="Monster" level="40" name="Queen Ant Larva">{MONSTER}</npc>
<npc id="29003" type="Monster" level="40" name="Nurse Ant">{MONSTER}</npc>
<npc id="29005" type="Monster" level="40" name="Royal Guard">{MONSTER}</npc>"#
        )
    }

    const NEST: (i32, i32, i32) = (QUEEN_NEST_X, QUEEN_NEST_Y, QUEEN_NEST_Z);

    fn setup_row(status: i32, respawn_time: i64, writes: bool, at: (i32, i32, i32)) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(QueenAnt);
        // The nest zone of `custom_boss.xml`.
        let zone = r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="Queen Ant Boss" id="12012" type="BossZone" shape="NPoly" minZ="-5947" maxZ="-5547">
		<node X="-21490" Y="178613" />
		<node X="-22258" Y="178961" />
		<node X="-22890" Y="182205" />
		<node X="-22770" Y="183129" />
		<node X="-21594" Y="183945" />
		<node X="-20570" Y="183381" />
		<node X="-20310" Y="182673" />
		<node X="-20762" Y="179129" />
	</zone>
</list>
"#;
        boss_test(
            registry,
            &npcs(),
            "",
            &[("data/zones/queen.xml", zone)],
            vec![row(QUEEN_ANT_ID, status, respawn_time, at)],
            writes,
        )
    }

    fn has_timer(t: &mut ScriptTest, name: &str, npc: Option<Oid>) -> bool {
        let mut found = false;
        t.world.run_as_script("QueenAnt", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            found = ctx.has_quest_timer(name, npc, None);
        });
        found
    }

    fn fire(t: &mut ScriptTest, event: &str, npc: Option<Oid>) {
        t.world.run_as_script("QueenAnt", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            ctx.notify_event(event, npc, None);
        });
    }

    fn status(t: &ScriptTest) -> Option<i32> {
        t.world.grand_boss_status(QUEEN_ANT_ID)
    }

    fn xy(t: &ScriptTest, id: Oid) -> (i32, i32) {
        let (x, y, _) = t.world.position_of(id).unwrap();
        (x, y)
    }

    fn alive(t: &ScriptTest, npc_id: i32) -> Vec<Oid> {
        npcs_of(t, npc_id)
            .into_iter()
            .filter(|m| t.world.object(*m).is_some())
            .collect()
    }

    fn state_of(t: &mut ScriptTest) -> (Option<Oid>, Option<Oid>, usize) {
        let mut out = (None, None, 0);
        t.world.run_as_script("QueenAnt", |ctx| {
            let s = ctx.state::<QueenAntState>();
            out = (s.queen, s.larva, s.nurses.len());
        });
        out
    }

    #[test]
    fn the_queen_spawns_with_her_larva_nurses_and_royal_guards() {
        let mut t = setup_row(STATUS_ALIVE, 0, true, NEST);
        let queen = t.world.grand_boss_npc(QUEEN_ANT_ID).unwrap();
        assert_eq!(xy(&t, queen), (NEST.0, NEST.1));
        let larva = alive(&t, LARVA_ID);
        assert_eq!(larva.len(), 1);
        assert_eq!(xy(&t, larva[0]), (LARVA_SPAWN_X, LARVA_SPAWN_Y));
        assert!(t.world.is_immobilized(larva[0]));
        assert!(!t.world.is_mortal(larva[0]));
        assert!(t.world.is_raid_minion(larva[0]));
        // The minions come from the template (`<minions>`): two nurses and two royal guards.
        assert_eq!(alive(&t, NURSE_ID).len(), 2);
        assert_eq!(alive(&t, ROYAL_ID).len(), 2);
        for royal in alive(&t, ROYAL_ID) {
            assert!(t.world.is_raid_minion(royal));
        }
        let (state_queen, state_larva, nurses) = state_of(&mut t);
        assert_eq!(state_queen, Some(queen));
        assert_eq!(state_larva, Some(larva[0]));
        assert_eq!(nurses, 2);
        assert!(has_timer(&mut t, EVENT_ACTION, Some(queen)));
        assert!(has_timer(&mut t, EVENT_HEAL, None));
        assert!(has_timer(&mut t, EVENT_DISTANCE_CHECK, Some(queen)));
        assert!(grand_writes(&t.take_boss()).is_empty());
    }

    #[test]
    fn a_row_outside_the_nest_puts_her_back_at_the_nest() {
        let t = setup_row(STATUS_ALIVE, 0, true, (0, 0, 0));
        let queen = t.world.grand_boss_npc(QUEEN_ANT_ID).unwrap();
        assert_eq!(xy(&t, queen), (NEST.0, NEST.1));
        // A row inside keeps its place.
        let t = setup_row(STATUS_ALIVE, 0, true, (-21600, 181000, -5734));
        let queen = t.world.grand_boss_npc(QUEEN_ANT_ID).unwrap();
        assert_eq!(xy(&t, queen), (-21600, 181000));
    }

    #[test]
    fn players_in_the_nest_are_moved_out_when_she_spawns() {
        let mut registry = ScriptRegistry::new();
        registry.add(QueenAnt);
        // A dead row whose time is over: the queen spawns after the player stands in the nest?
        // The player enters first, then the unlock event runs.
        let mut t = setup_row(STATUS_DEAD, now_ms() + 3_600_000, true, NEST);
        t.world.allow_boss_zone_entry(12012, Oid(1), 3600);
        t.enter_with(player_at(1, (-21600, 181000, -5734), vec![]));
        assert!(t.world.zone_players_inside(12012, "BossZone").contains(&Oid(1)));
        fire(&mut t, EVENT_QUEEN_UNLOCK, None);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert!(
            [
                TELEPORT_LOCATION_ONE,
                TELEPORT_LOCATION_TWO,
                TELEPORT_LOCATION_THREE
            ]
            .iter()
            .any(|(tx, ty, _)| (*tx, *ty) == (x, y)),
            "{x} {y}"
        );
        assert_eq!(status(&t), Some(STATUS_ALIVE));
    }

    #[test]
    fn with_the_switch_off_the_row_decides_the_unlock() {
        let mut t = setup_row(STATUS_DEAD, now_ms() - 1000, false, NEST);
        assert!(npcs_of(&t, QUEEN_ANT_ID).is_empty());
        let then = BossReadThen::ScriptEvent {
            script: "QueenAnt",
            event: EVENT_ROW_READ,
        };
        assert_eq!(
            rereads(&t.take_boss()),
            [(BossReadKind::GrandBoss(QUEEN_ANT_ID), then)]
        );
        answer(
            &mut t,
            then,
            QUEEN_ANT_ID,
            row(QUEEN_ANT_ID, STATUS_ALIVE, 0, NEST),
        );
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(npcs_of(&t, QUEEN_ANT_ID).len(), 1);
        assert!(grand_writes(&t.take_boss()).is_empty());

        let t = setup_row(STATUS_DEAD + 1, 0, false, NEST);
        assert!(npcs_of(&t, QUEEN_ANT_ID).is_empty());
    }

    #[test]
    fn a_dead_row_waits_for_its_time() {
        let mut t = setup_row(STATUS_DEAD, now_ms() + 3_600_000, true, NEST);
        assert!(npcs_of(&t, QUEEN_ANT_ID).is_empty());
        assert!(has_timer(&mut t, EVENT_QUEEN_UNLOCK, None));
        let t = setup_row(STATUS_DEAD, now_ms() - 10, true, NEST);
        assert_eq!(npcs_of(&t, QUEEN_ANT_ID).len(), 1);
        assert_eq!(status(&t), Some(STATUS_ALIVE));
    }

    #[test]
    fn the_queen_s_death_writes_the_row_and_the_larva_goes() {
        let mut t = setup_row(STATUS_ALIVE, 0, true, NEST);
        t.enter_with(player_at(1, (NEST.0 + 50, NEST.1, NEST.2), vec![]));
        let queen = t.world.grand_boss_npc(QUEEN_ANT_ID).unwrap();
        let larva = alive(&t, LARVA_ID)[0];
        t.take();
        t.take_boss();
        let before = now_ms();
        assert!(t.world.do_die(queen, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        assert!(names_to(&t.take(), 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert!(has_timer(&mut t, EVENT_QUEEN_UNLOCK, None));
        assert!(!has_timer(&mut t, EVENT_ACTION, Some(queen)));
        assert!(!has_timer(&mut t, EVENT_HEAL, None));
        assert!(!has_timer(&mut t, EVENT_DISTANCE_CHECK, Some(queen)));
        assert!(t.world.object(larva).is_none());
        assert_eq!(state_of(&mut t), (None, None, 0));
        let writes = grand_writes(&t.take_boss());
        assert_eq!(
            writes[0],
            BossWrite::GrandBossStatus {
                boss_id: QUEEN_ANT_ID,
                status: STATUS_DEAD
            }
        );
        let BossWrite::GrandBoss(GrandBossData { respawn_time, .. }) = writes[1].clone() else {
            panic!("{writes:?}");
        };
        let hour = 3_600_000;
        assert!(respawn_time >= before + 19 * hour && respawn_time <= now_ms() + 53 * hour);
    }

    #[test]
    fn a_dead_royal_guard_and_nurse_return_through_the_minion_list() {
        let mut t = setup_row(STATUS_ALIVE, 0, true, NEST);
        t.enter_with(player_at(1, (NEST.0 + 50, NEST.1, NEST.2), vec![]));
        let royal = alive(&t, ROYAL_ID)[0];
        let nurse = alive(&t, NURSE_ID)[0];
        assert!(t.world.do_die(royal, Some(Oid(1))));
        assert!(t.world.do_die(nurse, Some(Oid(1))));
        run_ms(&mut t, 2_600);
        assert_eq!(alive(&t, ROYAL_ID).len(), 1);
        assert_eq!(alive(&t, NURSE_ID).len(), 1);
        assert_eq!(state_of(&mut t).2, 1);
        // The nurse comes after 10 s; the royal guard after 280..320 s.
        run_ms(&mut t, 10_000);
        assert_eq!(alive(&t, NURSE_ID).len(), 2);
        assert_eq!(alive(&t, ROYAL_ID).len(), 1);
        run_ms(&mut t, 330_000);
        assert_eq!(alive(&t, ROYAL_ID).len(), 2);
    }

    #[test]
    fn the_distance_check_brings_a_stray_queen_home() {
        let mut t = setup_row(STATUS_ALIVE, 0, true, NEST);
        let queen = t.world.grand_boss_npc(QUEEN_ANT_ID).unwrap();
        t.world.tele_to_location(
            queen,
            l2_world::TeleportTarget::at(-10000, 181594, -5734).with_instance(0),
        );
        fire(&mut t, EVENT_DISTANCE_CHECK, Some(queen));
        assert!(t.world.is_moving(queen));
        // A dead queen stops the check.
        assert!(has_timer(&mut t, EVENT_DISTANCE_CHECK, Some(queen)));
        t.world
            .run_as_script("QueenAnt", |ctx| ctx.state::<QueenAntState>().queen = None);
        fire(&mut t, EVENT_DISTANCE_CHECK, Some(queen));
        assert!(!has_timer(&mut t, EVENT_DISTANCE_CHECK, Some(queen)));
    }

    #[test]
    fn the_nurses_target_the_hurt_larva_first_then_the_queen() {
        let mut t = setup_row(STATUS_ALIVE, 0, true, NEST);
        let queen = t.world.grand_boss_npc(QUEEN_ANT_ID).unwrap();
        let larva = alive(&t, LARVA_ID)[0];
        let nurses = alive(&t, NURSE_ID);
        // Nobody is hurt: the targets are cleared.
        t.world.set_npc_target(nurses[0], Some(queen));
        t.world.set_current_hp_mp(larva, 1000.0, 500.0);
        t.world.set_current_hp_mp(queen, 1000.0, 500.0);
        fire(&mut t, EVENT_HEAL, None);
        assert_eq!(t.world.creature_target(nurses[0]), None);
        // The queen is hurt.
        t.world.set_current_hp_mp(queen, 500.0, 500.0);
        fire(&mut t, EVENT_HEAL, None);
        assert_eq!(t.world.creature_target(nurses[0]), Some(queen));
        // The larva is hurt too: it comes first.
        t.world.set_current_hp_mp(larva, 500.0, 500.0);
        fire(&mut t, EVENT_HEAL, None);
        assert_eq!(t.world.creature_target(nurses[0]), Some(larva));
    }

    #[test]
    fn a_much_higher_level_player_entering_gets_the_curse_roll_and_loses_hate() {
        let mut t = setup_row(STATUS_ALIVE, 0, true, NEST);
        t.enter_with(player_at(1, (NEST.0 + 50, NEST.1, NEST.2), vec![]));
        let queen = t.world.grand_boss_npc(QUEEN_ANT_ID).unwrap();
        t.world.add_damage_hate(queen, Oid(1), 0, 100);
        let mut hated = (None, None);
        t.world.run_as_script("QueenAnt", |ctx| {
            let npc = ctx.script_npc(queen).unwrap();
            hated.0 = ctx.most_hated(npc);
            // Level 80 against 40: more than 8 levels.
            QueenAnt.on_aggro_range_enter(ctx, npc, Oid(1), false);
            hated.1 = ctx.most_hated(npc);
        });
        assert_eq!(hated, (Some(Oid(1)), None));
    }
}
