//! Baium, the emperor of the Tower of Insolence: the Angelic Vortex entry (Blooded Fabric), the
//! wake-up at the stone statue, the wake-up movie with Baium's gift, the archangels, the
//! three-slot hate model, the inactivity reset and the respawn cycle on the `GrandBossManager`
//! state.
// Java: dist/game/data/scripts/ai/bosses/Baium/Baium.java
//
// The Java fields are `BaiumState` (`ctx.state`); Baium's `NpcVariables` (`c_quest0..2`,
// `i_quest0..2`) are a map by NPC object id in it. The constructor is `on_load`. Statuses: 0 alive
// (the stone statue), 1 waiting (unused by this script), 2 in fight, 3 dead.
//
// Side by side (D-54, `super` module): with `CycleDatabaseWrites` off the constructor restores the
// boss only from an alive row (a dead row still waits for its respawn time), and `CLEAR_STATUS`
// reads the row again and brings the statue back only when the row says alive.
//
// Not modelled by the engine (the Java lines are ported, they do nothing yet): `onCreatureSee`
// is never fired (`CreatureSeeTaskManager`), the skill of a hit in `on_attack` is always `None`
// (so the hate is always `damage * 1000`), `isSummonSpawn` of the archangels, `unload` (D-16).

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, Earthquake, ExShowScreenMessage, PlaySound, SocialAction};
use l2_util::ObjectId;
use l2_world::mount::MountType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportTarget};
use tracing::info;

use super::{EVENT_ROW_READ, date_time_string, distance_3d, row_lets_boss_run};

// NPC definitions.
const BAIUM_NPC_ID: i32 = 29020; // GranbossBaium Npc
const BAIUM_STONE_NPC_ID: i32 = 29025; // GranbossBaium Stone Npc
const ANGELIC_VORTEX_NPC_ID: i32 = 31862; // Vortex Teleport Npc
const ARCHANGEL_NPC_ID: i32 = 29021; // Archangel Minion Npc
const TELEPORT_CUBE_NPC_ID: i32 = 31842; // Teleport Cube Exit

// Item definitions.
const BLOODED_FABRIC_ITEM_ID: i32 = 4295;

// Skill templates.
const SKILL_BAIUM_ATTACK: SkillKey = SkillKey { id: 4127, level: 1 }; // Baium: General Attack.
const SKILL_ENERGY_WAVE: SkillKey = SkillKey { id: 4128, level: 1 }; // Wind Of Force.
const SKILL_EARTHQUAKE: SkillKey = SkillKey { id: 4129, level: 1 }; // Earthquake.
const SKILL_THUNDERBOLT: SkillKey = SkillKey { id: 4130, level: 1 }; // Striking of Thunderbolt.
const SKILL_GROUP_HOLD: SkillKey = SkillKey { id: 4131, level: 1 }; // Stun.
const SKILL_SPEAR_ATTACK: SkillKey = SkillKey { id: 4132, level: 1 }; // Spear: Pound the Ground.
const SKILL_ANGEL_HEAL: SkillKey = SkillKey { id: 4133, level: 1 }; // Angel Heal.
const SKILL_HEAL_OF_BAIUM: SkillKey = SkillKey { id: 4135, level: 1 }; // Baium Heal.
const SKILL_BAIUM_PRESENT: SkillKey = SkillKey { id: 4136, level: 1 }; // Baium's Gift.
const SKILL_ANTI_STRIDER: SkillKey = SkillKey { id: 4258, level: 1 }; // Hinder Strider.

// Zone configuration and rooftop area: `getZoneById(70051, NoRestartZone.class)`.
const BAIUM_RIM_ZONE: (i32, &str) = (70051, "NoRestartZone");

// Status flags for Baium lifecycle.
const STATUS_ALIVE: i32 = 0;
const STATUS_WAITING: i32 = 1;
const STATUS_IN_FIGHT: i32 = 2;
const STATUS_DEAD: i32 = 3;

// Time configuration in milliseconds.
const CHECK_ATTACK_INTERVAL_MILLIS: i64 = 60000;
const INACTIVITY_LIMIT_MILLIS: i64 = 1800000;
const TELEPORT_CUBE_LIFETIME_MILLIS: i64 = 900000;
const SELECT_TARGET_INTERVAL_MILLIS: i64 = 5000;

// Distance configuration.
const PLAYER_PORT_RADIUS: f64 = 16000.0;

// Location constants for teleport and spawn points.
const LOCATION_BAIUM_GIFT: (i32, i32, i32) = (115910, 17337, 10105);
const LOCATION_BAIUM_SPAWN: (i32, i32, i32, i32) = (116033, 17447, 10107, 40188);
const LOCATION_TELEPORT_CUBE: (i32, i32, i32) = (115017, 15549, 10090);
const LOCATION_TELEPORT_IN: (i32, i32, i32) = (114077, 15882, 10078);
const TELEPORT_OUT_LOCATIONS: [(i32, i32, i32); 3] = [
    (108784, 16000, -4928),
    (113824, 10448, -5164),
    (115488, 22096, -5168),
];
const ARCHANGEL_SPAWN_LOCATIONS: [(i32, i32, i32, i32); 5] = [
    (115792, 16608, 10136, 0),
    (115168, 17200, 10136, 0),
    (115780, 15564, 10136, 13620),
    (114880, 16236, 10136, 5400),
    (114239, 17168, 10136, -1992),
];

// String identifiers for internal scripted events.
const EVENT_HTML_VORTEX_READY: &str = "31862-04.html";
const EVENT_ENTER: &str = "ENTER";
const EVENT_EXIT: &str = "EXIT";
const EVENT_WAKE_UP: &str = "WAKE_UP";
const EVENT_WAKEUP_ACTION: &str = "WAKEUP_ACTION";
const EVENT_MANAGE_EARTHQUAKE: &str = "MANAGE_EARTHQUAKE";
const EVENT_SOCIAL_ACTION: &str = "SOCIAL_ACTION";
const EVENT_PLAYER_PORT: &str = "PLAYER_PORT";
const EVENT_PLAYER_KILL: &str = "PLAYER_KILL";
const EVENT_SPAWN_ARCHANGELS: &str = "SPAWN_ARCHANGEL";
const EVENT_SELECT_TARGET: &str = "SELECT_TARGET";
const EVENT_CHECK_ATTACK: &str = "CHECK_ATTACK";
const EVENT_CLEAR_STATUS: &str = "CLEAR_STATUS";
const EVENT_CLEAR_ZONE: &str = "CLEAR_ZONE";
const EVENT_RESPAWN_BAIUM: &str = "RESPAWN_BAIUM";
const EVENT_ABORT_FIGHT: &str = "ABORT_FIGHT";
const EVENT_DESPAWN_MINIONS: &str = "DESPAWN_MINIONS";
const EVENT_MANAGE_SKILLS: &str = "MANAGE_SKILLS";

/// Java `CategoryType.CLERIC_GROUP` of `data/CategoryData.xml` (the categories are not a table
/// yet; `clan/academy.rs` does the same for its group).
const CLERIC_GROUP: [i32; 13] = [10, 15, 16, 17, 25, 29, 30, 38, 42, 43, 97, 98, 105];

/// The fields of the Java class, and Baium's `NpcVariables`.
#[derive(Debug, Default)]
pub(crate) struct BaiumState {
    /// `_baiumInstance`.
    pub(crate) baium_instance: Option<ObjectId>,
    pub(crate) last_attack_time: i64,
    /// `npc.getVariables()`: `c_quest0..2` (the creature) and `i_quest0..2` (its aggro), by NPC.
    pub(crate) variables: HashMap<ObjectId, [(Option<ObjectId>, i32); 3]>,
}

/// Java `ai.bosses.Baium.Baium`.
pub struct Baium;

impl Script for Baium {
    fn name(&self) -> &'static str {
        "Baium"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Baium"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ANGELIC_VORTEX_NPC_ID]);
        s.add_talk_id(&[ANGELIC_VORTEX_NPC_ID, TELEPORT_CUBE_NPC_ID, BAIUM_STONE_NPC_ID]);
        s.add_start_npc(&[ANGELIC_VORTEX_NPC_ID, TELEPORT_CUBE_NPC_ID, BAIUM_STONE_NPC_ID]);
        s.add_attack_id(&[BAIUM_NPC_ID, ARCHANGEL_NPC_ID]);
        s.add_kill_id(&[BAIUM_NPC_ID]);
        s.add_spell_finished_id(&[BAIUM_NPC_ID]);
        s.add_creature_see_id(&[BAIUM_NPC_ID]);
    }

    /// The constructor's second half: restores the state from the boss manager.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let Some(boss_info) = ctx.world().grand_boss_info(BAIUM_NPC_ID).cloned() else {
            // Java: `getStatSet` answers null and the constructor throws.
            tracing::warn!("Baium: no grandboss_data row for {BAIUM_NPC_ID}, the script does not start.");
            return;
        };
        let stored_hp = boss_info.current_hp;
        let stored_mp = boss_info.current_mp;
        let stored_x = boss_info.loc_x;
        let stored_y = boss_info.loc_y;
        let stored_z = boss_info.loc_z;
        let stored_heading = boss_info.heading;
        let stored_respawn_time = boss_info.respawn_time;
        let current_status = get_status(ctx);
        // D-54: a waiting or fighting row belongs to the other server's fight.
        if current_status != STATUS_DEAD && !row_lets_boss_run(ctx, BAIUM_NPC_ID) {
            info!("Baium: the shared state row says {current_status}; the AI waits for an alive row (D-54).");
            return;
        }
        match current_status {
            STATUS_WAITING => {
                // NOTE(java-quirk): the status goes back to alive, but no statue is spawned until
                // the next start.
                set_status(ctx, STATUS_ALIVE);
            }
            STATUS_ALIVE => {
                spawn_stone(ctx);
            }
            STATUS_IN_FIGHT => {
                let restored = ctx.add_spawn(
                    BAIUM_NPC_ID,
                    stored_x,
                    stored_y,
                    stored_z,
                    stored_heading,
                    false,
                    0,
                    0,
                );
                ctx.state::<BaiumState>().baium_instance = restored.map(|b| b.object_id);
                if let Some(restored) = restored {
                    ctx.world_mut()
                        .set_current_hp_mp(restored.object_id, stored_hp, stored_mp);
                }
                let now = ctx.now_ms();
                ctx.state::<BaiumState>().last_attack_time = now;
                if let Some(restored) = restored {
                    ctx.world_mut().add_grand_boss(BAIUM_NPC_ID, restored.object_id);
                }
                spawn_archangels(ctx);
                ctx.start_quest_timer(
                    EVENT_CHECK_ATTACK,
                    CHECK_ATTACK_INTERVAL_MILLIS,
                    restored,
                    None,
                    false,
                );
            }
            STATUS_DEAD => {
                let remaining_delay = stored_respawn_time - ctx.now_ms();
                if remaining_delay > 0 {
                    ctx.start_quest_timer(EVENT_CLEAR_STATUS, remaining_delay, None, None, false);
                } else {
                    ctx.notify_event(EVENT_CLEAR_STATUS, None, None);
                }
            }
            _ => {}
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            // Static HTML.
            EVENT_HTML_VORTEX_READY => {
                return Some(EVENT_HTML_VORTEX_READY.to_owned());
            }
            // Player entry and exit handling.
            EVENT_ENTER => {
                let player = player?;
                let mut html_response = None;
                let baium_status = get_status(ctx);
                if baium_status == STATUS_DEAD {
                    html_response = Some("31862-03.html");
                } else if baium_status == STATUS_IN_FIGHT {
                    html_response = Some("31862-02.html");
                } else if !ctx.has_quest_items(player, BLOODED_FABRIC_ITEM_ID) {
                    html_response = Some("31862-01.html");
                } else {
                    ctx.take_items(player, BLOODED_FABRIC_ITEM_ID, 1);
                    let (x, y, z) = LOCATION_TELEPORT_IN;
                    tele_location(ctx, player, x, y, z);
                }
                return html_response.map(str::to_owned);
            }
            EVENT_EXIT => {
                let player = player?;
                let exit_destination = ctx.get_random_entry(&TELEPORT_OUT_LOCATIONS)?;
                let random_offset_x = ctx.get_random(100);
                let random_offset_y = ctx.get_random(100);
                ctx.world_mut().tele_to_location(
                    player,
                    TeleportTarget::at(
                        exit_destination.0 + random_offset_x,
                        exit_destination.1 + random_offset_y,
                        exit_destination.2,
                    ),
                );
            }
            // Wake-up flow, cinematic and initial actions.
            EVENT_WAKE_UP => {
                if get_status(ctx) == STATUS_ALIVE {
                    set_status(ctx, STATUS_IN_FIGHT);
                    let (x, y, z, heading) = LOCATION_BAIUM_SPAWN;
                    let baium = ctx.add_spawn(BAIUM_NPC_ID, x, y, z, heading, false, 0, 0);
                    ctx.state::<BaiumState>().baium_instance = baium.map(|b| b.object_id);
                    if let Some(baium) = baium {
                        ctx.world_mut().disable_core_ai(baium.object_id, true);
                        ctx.world_mut().set_random_walking(baium.object_id, false);
                        ctx.world_mut().add_grand_boss(BAIUM_NPC_ID, baium.object_id);
                    }
                    let now = ctx.now_ms();
                    ctx.state::<BaiumState>().last_attack_time = now;
                    ctx.start_quest_timer(EVENT_WAKEUP_ACTION, 50, baium, None, false);
                    ctx.start_quest_timer(EVENT_MANAGE_EARTHQUAKE, 2000, baium, None, false);
                    ctx.start_quest_timer(EVENT_SOCIAL_ACTION, 10000, baium, player, false);
                    ctx.start_quest_timer(
                        EVENT_CHECK_ATTACK,
                        CHECK_ATTACK_INTERVAL_MILLIS,
                        baium,
                        None,
                        false,
                    );
                    if let Some(npc) = npc {
                        ctx.world_mut().delete_npc(npc.object_id);
                    }
                }
            }
            EVENT_WAKEUP_ACTION => {
                if npc.is_some()
                    && let Some(baium) = alive_baium(ctx)
                {
                    broadcast_rim(
                        ctx,
                        &SocialAction {
                            object_id: baium.0,
                            action_id: 2,
                        },
                    );
                }
            }
            EVENT_MANAGE_EARTHQUAKE => {
                if let Some(npc) = npc
                    && let Some((x, y, z)) = ctx.world().position_of(npc.object_id)
                {
                    broadcast_rim(
                        ctx,
                        &Earthquake {
                            x,
                            y,
                            z,
                            intensity: 40,
                            duration: 10,
                        },
                    );
                    broadcast_rim(ctx, &PlaySound::file("BS02_A"));
                }
            }
            EVENT_SOCIAL_ACTION => {
                if let Some(npc) = npc {
                    broadcast_rim(
                        ctx,
                        &SocialAction {
                            object_id: npc.object_id.0,
                            action_id: 3,
                        },
                    );
                    ctx.start_quest_timer(EVENT_PLAYER_PORT, 6000, Some(npc), player, false);
                }
            }
            EVENT_PLAYER_PORT => {
                if let Some(npc) = npc {
                    if let Some(player) = player
                        && inside_radius_3d(ctx, player, npc.object_id, PLAYER_PORT_RADIUS)
                    {
                        let (x, y, z) = LOCATION_BAIUM_GIFT;
                        tele_location(ctx, player, x, y, z);
                        ctx.start_quest_timer(EVENT_PLAYER_KILL, 3000, Some(npc), Some(player), false);
                    } else {
                        let random_player = get_random_player(ctx, npc.object_id);
                        if let Some(random_player) = random_player {
                            let (x, y, z) = LOCATION_BAIUM_GIFT;
                            tele_location(ctx, random_player, x, y, z);
                            ctx.start_quest_timer(
                                EVENT_PLAYER_KILL,
                                3000,
                                Some(npc),
                                Some(random_player),
                                false,
                            );
                        } else {
                            ctx.start_quest_timer(EVENT_PLAYER_KILL, 3000, Some(npc), None, false);
                        }
                    }
                }
            }
            EVENT_PLAYER_KILL => {
                if let Some(npc) = npc
                    && let Some(player) = player
                    && inside_radius_3d(ctx, player, npc.object_id, PLAYER_PORT_RADIUS)
                {
                    broadcast_rim(
                        ctx,
                        &SocialAction {
                            object_id: npc.object_id.0,
                            action_id: 1,
                        },
                    );
                    let name = ctx.player_name(player);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::HowDareYouWakeMeNowYouShallDie,
                        &[&name],
                    );
                    ctx.world_mut().set_npc_target(npc.object_id, Some(player));
                    ctx.world_mut().do_cast(npc.object_id, SKILL_BAIUM_PRESENT);
                }
                let (zone_id, zone_type) = BAIUM_RIM_ZONE;
                for inside_player in ctx.world().zone_players_inside(zone_id, zone_type) {
                    if ctx.world().is_hero(inside_player)
                        && ctx.world().boss_settings().grand_boss.baium_recognize_hero
                    {
                        let name = ctx.player_name(inside_player);
                        broadcast_rim(
                            ctx,
                            &ExShowScreenMessage::npc_string(
                                NpcStringId::NotEvenTheGodsThemselvesCouldTouchMeButYouS1YouDareChallengeMeIgnorantMortal,
                                2,
                                4000,
                                &[&name],
                            ),
                        );
                        break;
                    }
                }
                ctx.start_quest_timer(EVENT_SPAWN_ARCHANGELS, 8000, npc, player, false);
            }
            // Archangel minion management.
            EVENT_SPAWN_ARCHANGELS => {
                let baium = alive_baium(ctx)?;
                if get_status(ctx) != STATUS_IN_FIGHT {
                    return None;
                }
                ctx.world_mut().disable_core_ai(baium, false);
                ctx.world_mut().set_random_walking(baium, true);
                spawn_archangels(ctx);
                // NOTE(java-quirk): `npc` is the timer's NPC; a timer without one makes Java's
                // `addAttackDesire(null, ..)` throw.
                let npc = npc?;
                if let Some(player) = player
                    && !ctx.world().is_dead(player)
                {
                    ctx.add_attack_desire(npc, player, 999);
                } else {
                    let random_target = get_random_player(ctx, npc.object_id);
                    if let Some(random_target) = random_target {
                        ctx.add_attack_desire(npc, random_target, 999);
                    }
                }
            }
            EVENT_SELECT_TARGET => {
                if let Some(npc) = npc {
                    let Some(baium) = alive_baium(ctx) else {
                        ctx.world_mut().delete_npc(npc.object_id);
                        return None;
                    };
                    let archangel = npc.object_id;
                    let (zone_id, zone_type) = BAIUM_RIM_ZONE;
                    let current_most_hated = ctx.world_mut().aggro_most_hated(archangel);
                    if let Some(most_hated) = current_most_hated
                        && ctx.world().player(most_hated).is_some()
                        && ctx
                            .world()
                            .is_inside_zone_by_position(zone_id, zone_type, most_hated)
                    {
                        if ctx.world().target_of(archangel) != Some(most_hated) {
                            ctx.world_mut().aggro_clear(archangel);
                        }
                        ctx.add_attack_desire(npc, most_hated, 999);
                    } else {
                        let candidate = ctx
                            .world()
                            .visible_objects_in_range(archangel, 1000)
                            .into_iter()
                            .find(|c| {
                                let world = ctx.world();
                                (world.player(*c).is_some() || world.is_summon(*c))
                                    && world.is_inside_zone_by_position(zone_id, zone_type, *c)
                                    && !world.is_dead(*c)
                            });
                        if let Some(candidate) = candidate {
                            if ctx.world().target_of(archangel) != Some(candidate) {
                                ctx.world_mut().aggro_clear(archangel);
                            }
                            ctx.add_attack_desire(npc, candidate, 999);
                        } else if inside_radius_3d(ctx, archangel, baium, 40.0) {
                            if ctx.world().target_of(archangel) != Some(baium) {
                                ctx.world_mut().aggro_clear(archangel);
                            }
                            ctx.add_attack_desire(npc, baium, 999);
                        } else {
                            ctx.world_mut().npc_set_intention_follow(archangel, baium);
                        }
                    }
                    ctx.start_quest_timer(
                        EVENT_SELECT_TARGET,
                        SELECT_TARGET_INTERVAL_MILLIS,
                        Some(npc),
                        None,
                        false,
                    );
                }
            }
            // Inactivity handling, self-heal checks and status reset.
            EVENT_CHECK_ATTACK => {
                let last_attack_time = ctx.state::<BaiumState>().last_attack_time;
                if let Some(npc) = npc {
                    if (last_attack_time + INACTIVITY_LIMIT_MILLIS) < ctx.now_ms() {
                        ctx.cancel_quest_timers(EVENT_SELECT_TARGET);
                        ctx.notify_event(EVENT_CLEAR_ZONE, None, None);
                        spawn_stone(ctx);
                        set_status(ctx, STATUS_ALIVE);
                    } else {
                        let heal_check_time = last_attack_time + 300000;
                        let (hp, max_hp) = hp_of(ctx, npc.object_id);
                        let heal_threshold_hp = max_hp * 0.75;
                        if (heal_check_time < ctx.now_ms()) && (hp < heal_threshold_hp) {
                            ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
                            ctx.world_mut().do_cast(npc.object_id, SKILL_HEAL_OF_BAIUM);
                        }
                        ctx.start_quest_timer(
                            EVENT_CHECK_ATTACK,
                            CHECK_ATTACK_INTERVAL_MILLIS,
                            Some(npc),
                            None,
                            false,
                        );
                    }
                }
            }
            EVENT_CLEAR_STATUS => {
                // D-54: on the shared database the row decides; read it again first.
                if !ctx.world().boss_writes_enabled() {
                    ctx.world_mut()
                        .reread_grand_boss_for_script(BAIUM_NPC_ID, self.name(), EVENT_ROW_READ);
                    return None;
                }
                set_status(ctx, STATUS_ALIVE);
                spawn_stone(ctx);
            }
            EVENT_ROW_READ => {
                // D-54: the answer of the read `CLEAR_STATUS` asked for.
                match get_status(ctx) {
                    STATUS_ALIVE => {
                        set_status(ctx, STATUS_ALIVE);
                        spawn_stone(ctx);
                    }
                    STATUS_DEAD => {
                        let respawn = ctx
                            .world()
                            .grand_boss_info(BAIUM_NPC_ID)
                            .map_or(0, |i| i.respawn_time);
                        let remaining_delay = respawn - ctx.now_ms();
                        if remaining_delay > 0 {
                            ctx.start_quest_timer(EVENT_CLEAR_STATUS, remaining_delay, None, None, false);
                        } else {
                            info!("Baium: the shared state row says dead; the statue stays away (D-54).");
                        }
                    }
                    status => {
                        info!("Baium: the shared state row says {status}; the statue stays away (D-54).");
                    }
                }
            }
            EVENT_CLEAR_ZONE => {
                let (zone_id, zone_type) = BAIUM_RIM_ZONE;
                for creature in ctx.world().zone_characters_inside(zone_id, zone_type) {
                    if ctx.world().is_npc(creature) {
                        ctx.world_mut().delete_npc(creature);
                    } else if ctx.world().player(creature).is_some() {
                        ctx.notify_event(EVENT_EXIT, None, Some(creature));
                    }
                }
            }
            // GM control utilities for Baium.
            EVENT_RESPAWN_BAIUM => {
                if get_status(ctx) == STATUS_DEAD {
                    set_respawn(ctx, 0);
                    ctx.cancel_quest_timer(EVENT_CLEAR_STATUS, None, None);
                    ctx.cancel_quest_timers(EVENT_CLEAR_ZONE);
                    ctx.notify_event(EVENT_CLEAR_STATUS, None, None);
                } else if let Some(player) = player {
                    ctx.world().send_message(
                        player,
                        &format!("{}: You cannot respawn Baium while Baium is alive!", self.name()),
                    );
                }
            }
            EVENT_ABORT_FIGHT => {
                if get_status(ctx) == STATUS_IN_FIGHT {
                    ctx.state::<BaiumState>().baium_instance = None;
                    ctx.notify_event(EVENT_CLEAR_ZONE, None, None);
                    ctx.notify_event(EVENT_CLEAR_STATUS, None, None);
                    if let Some(player) = player {
                        ctx.world()
                            .send_message(player, &format!("{}: Aborting fight!", self.name()));
                    }
                } else if let Some(player) = player {
                    ctx.world().send_message(
                        player,
                        &format!("{}: You cannot abort attack right now!", self.name()),
                    );
                }
                ctx.cancel_quest_timers(EVENT_CHECK_ATTACK);
                ctx.cancel_quest_timers(EVENT_SELECT_TARGET);
            }
            EVENT_DESPAWN_MINIONS => {
                if get_status(ctx) == STATUS_IN_FIGHT {
                    let (zone_id, zone_type) = BAIUM_RIM_ZONE;
                    for creature in ctx.world().zone_characters_inside(zone_id, zone_type) {
                        if ctx.script_npc(creature).is_some_and(|n| n.id == ARCHANGEL_NPC_ID) {
                            ctx.world_mut().delete_npc(creature);
                        }
                    }
                    if let Some(player) = player {
                        ctx.world().send_message(
                            player,
                            &format!("{}: All archangels has been deleted!", self.name()),
                        );
                    }
                } else if let Some(player) = player {
                    ctx.world().send_message(
                        player,
                        &format!("{}: You cannot despawn archangels right now!", self.name()),
                    );
                }
            }
            // Periodic Baium skill management.
            EVENT_MANAGE_SKILLS => {
                if let Some(npc) = npc {
                    manage_skills(ctx, npc);
                }
            }
            _ => {}
        }
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        skill: Option<SkillKey>,
    ) {
        let now = ctx.now_ms();
        ctx.state::<BaiumState>().last_attack_time = now;
        let npc_id = npc.id;
        let (zone_id, zone_type) = BAIUM_RIM_ZONE;
        if npc_id == BAIUM_NPC_ID {
            if (ctx.world().mount_type_of(attacker) == MountType::Strider)
                && !ctx.world().is_affected_by_skill(attacker, SKILL_ANTI_STRIDER.id)
                && !ctx
                    .world_mut()
                    .is_skill_disabled_of(npc.object_id, SKILL_ANTI_STRIDER)
            {
                ctx.world_mut().set_npc_target(npc.object_id, Some(attacker));
                ctx.world_mut().do_cast(npc.object_id, SKILL_ANTI_STRIDER);
            }
            let (hp, max_hp) = hp_of(ctx, npc.object_id);
            let health_ratio = hp / max_hp;
            if skill.is_none() {
                refresh_ai_params(ctx, attacker, npc, damage.wrapping_mul(1000));
            } else if health_ratio < 0.25 {
                refresh_ai_params(ctx, attacker, npc, (damage / 3).wrapping_mul(100));
            } else if health_ratio < 0.5 {
                refresh_ai_params(ctx, attacker, npc, damage.wrapping_mul(20));
            } else if health_ratio < 0.75 {
                refresh_ai_params(ctx, attacker, npc, damage.wrapping_mul(10));
            } else {
                refresh_ai_params(ctx, attacker, npc, (damage / 3).wrapping_mul(20));
            }
            manage_skills(ctx, npc);
        } else if npc_id == ARCHANGEL_NPC_ID {
            let archangel = npc.object_id;
            let most_hated = ctx.world_mut().aggro_most_hated(archangel);
            if (ctx.get_random(100) < 10)
                && ctx
                    .world_mut()
                    .check_do_cast_conditions_of(archangel, SKILL_SPEAR_ATTACK)
            {
                if let Some(most_hated) = most_hated
                    && distance_between(ctx, archangel, most_hated) < 1000.0
                    && ctx.world().is_character_in_zone(zone_id, zone_type, most_hated)
                {
                    ctx.world_mut().set_npc_target(archangel, Some(most_hated));
                    ctx.world_mut().do_cast(archangel, SKILL_SPEAR_ATTACK);
                } else if ctx.world().is_character_in_zone(zone_id, zone_type, attacker) {
                    ctx.world_mut().set_npc_target(archangel, Some(attacker));
                    ctx.world_mut().do_cast(archangel, SKILL_SPEAR_ATTACK);
                }
            }
            let (hp, max_hp) = hp_of(ctx, archangel);
            let archangel_hp_ratio = hp / max_hp;
            if (ctx.get_random(100) < 5)
                && (archangel_hp_ratio < 0.5)
                && ctx
                    .world_mut()
                    .check_do_cast_conditions_of(archangel, SKILL_ANGEL_HEAL)
            {
                ctx.world_mut().set_npc_target(archangel, Some(archangel));
                ctx.world_mut().do_cast(archangel, SKILL_ANGEL_HEAL);
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let (zone_id, zone_type) = BAIUM_RIM_ZONE;
        if !ctx.world().is_character_in_zone(zone_id, zone_type, killer) {
            // NOTE(java-quirk): a kill from outside the rim leaves the status "in fight".
            return;
        }
        set_status(ctx, STATUS_DEAD);
        let (x, y, z) = LOCATION_TELEPORT_CUBE;
        ctx.add_spawn(
            TELEPORT_CUBE_NPC_ID,
            x,
            y,
            z,
            0,
            false,
            TELEPORT_CUBE_LIFETIME_MILLIS,
            0,
        );
        broadcast_rim(ctx, &PlaySound::file("BS01_D"));
        let grand_boss = &ctx.world().boss_settings().grand_boss;
        let base_interval_millis = i64::from(grand_boss.baium_spawn_interval) * 3600000;
        let random_range_millis = i64::from(grand_boss.baium_spawn_random) * 3600000;
        let respawn_delay_millis = base_interval_millis
            + ctx
                .world_mut()
                .rng()
                .get_i64(-random_range_millis, random_range_millis);
        let next_respawn_time = ctx.now_ms() + respawn_delay_millis;
        info!("Baium will respawn at: {}", date_time_string(next_respawn_time));
        set_respawn(ctx, respawn_delay_millis);
        ctx.start_quest_timer(EVENT_CLEAR_STATUS, respawn_delay_millis, None, None, false);
        ctx.start_quest_timer(EVENT_CLEAR_ZONE, TELEPORT_CUBE_LIFETIME_MILLIS, None, None, false);
        ctx.cancel_quest_timer(EVENT_CHECK_ATTACK, Some(npc), None);
        ctx.cancel_quest_timers(EVENT_SELECT_TARGET);
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        let (zone_id, zone_type) = BAIUM_RIM_ZONE;
        if !ctx
            .world()
            .is_inside_zone_by_position(zone_id, zone_type, creature)
            || ctx
                .script_npc(creature)
                .is_some_and(|n| n.id == BAIUM_STONE_NPC_ID)
        {
            return;
        }
        let (hp, max_hp) = hp_of(ctx, npc.object_id);
        let health_ratio = hp / max_hp;
        // `creature.isInCategory(CLERIC_GROUP)`: the class id of a player (an NPC's id is never in
        // the group).
        let cleric = ctx
            .world()
            .player(creature)
            .is_some_and(|p| CLERIC_GROUP.contains(&p.data.class_id));
        if cleric {
            if health_ratio < 0.25 {
                refresh_ai_params_with(ctx, creature, npc, 10000, 10000);
            } else if health_ratio < 0.5 {
                refresh_ai_params_with(ctx, creature, npc, 10000, 6000);
            } else if health_ratio < 0.75 {
                refresh_ai_params_with(ctx, creature, npc, 10000, 3000);
            } else {
                refresh_ai_params_with(ctx, creature, npc, 10000, 2000);
            }
        } else {
            refresh_ai_params_with(ctx, creature, npc, 10000, 1000);
        }
        manage_skills(ctx, npc);
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        _skill: SkillKey,
    ) {
        ctx.start_quest_timer(EVENT_MANAGE_SKILLS, 1000, Some(npc), None, false);
        let (zone_id, zone_type) = BAIUM_RIM_ZONE;
        if let Some(baium) = alive_baium(ctx)
            && !ctx
                .world()
                .is_character_in_zone(zone_id, zone_type, npc.object_id)
        {
            let (x, y, z, heading) = LOCATION_BAIUM_SPAWN;
            ctx.world_mut().tele_to_location(
                baium,
                TeleportTarget::at(x, y, z).with_heading(heading).with_instance(0),
            );
        }
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

/// `_baiumInstance` while it is in the world and alive (`(_baiumInstance != null) &&
/// !_baiumInstance.isDead()`).
fn alive_baium(ctx: &mut ScriptCtx<'_>) -> Option<ObjectId> {
    let baium = ctx.state::<BaiumState>().baium_instance?;
    (ctx.world().npc(baium).is_some() && !ctx.world().is_dead(baium)).then_some(baium)
}

/// `addSpawn(BAIUM_STONE_NPC_ID, LOCATION_BAIUM_SPAWN, false, 0)`.
fn spawn_stone(ctx: &mut ScriptCtx<'_>) {
    let (x, y, z, heading) = LOCATION_BAIUM_SPAWN;
    ctx.add_spawn(BAIUM_STONE_NPC_ID, x, y, z, heading, false, 0, 0);
}

/// The five archangels, each with its `SELECT_TARGET` timer.
fn spawn_archangels(ctx: &mut ScriptCtx<'_>) {
    for (x, y, z, heading) in ARCHANGEL_SPAWN_LOCATIONS {
        let archangel_npc = ctx.add_spawn(ARCHANGEL_NPC_ID, x, y, z, heading, false, 0, 0);
        ctx.start_quest_timer(
            EVENT_SELECT_TARGET,
            SELECT_TARGET_INTERVAL_MILLIS,
            archangel_npc,
            None,
            false,
        );
    }
}

/// Updates Baium aggro using incoming damage as base value for both damage and aggro weight.
fn refresh_ai_params(ctx: &mut ScriptCtx<'_>, attacker: ObjectId, npc: ScriptNpc, damage: i32) {
    refresh_ai_params_with(ctx, attacker, npc, damage, damage);
}

/// Updates Baium aggro table using separate damage and aggro scores: three tracked attackers, the
/// one with the lowest weight is replaced.
fn refresh_ai_params_with(
    ctx: &mut ScriptCtx<'_>,
    attacker: ObjectId,
    npc: ScriptNpc,
    damage: i32,
    aggro: i32,
) {
    let new_aggro_value = damage.wrapping_add(ctx.get_random(3000));
    let aggro_threshold = aggro.wrapping_add(1000);
    let variables = ctx
        .state::<BaiumState>()
        .variables
        .entry(npc.object_id)
        .or_default();
    for slot in variables.iter_mut() {
        if slot.0 == Some(attacker) {
            if slot.1 < aggro_threshold {
                slot.1 = new_aggro_value;
            }
            return;
        }
    }
    let min_index = index_of_min_value(&[variables[0].1, variables[1].1, variables[2].1]);
    variables[min_index] = (Some(attacker), new_aggro_value);
}

/// Java `MathUtil.getIndexOfMinValue`: the first lowest.
fn index_of_min_value(values: &[i32; 3]) -> usize {
    let mut min_index = 0;
    for i in 1..values.len() {
        if values[i] < values[min_index] {
            min_index = i;
        }
    }
    min_index
}

/// Java `MathUtil.getIndexOfMaxValue`: the first highest.
fn index_of_max_value(values: &[i32; 3]) -> usize {
    let mut max_index = 0;
    for i in 1..values.len() {
        if values[i] > values[max_index] {
            max_index = i;
        }
    }
    max_index
}

fn get_status(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.world().grand_boss_status(BAIUM_NPC_ID).unwrap_or(-1)
}

fn set_status(ctx: &mut ScriptCtx<'_>, status: i32) {
    ctx.world_mut().set_grand_boss_status(BAIUM_NPC_ID, status);
}

fn set_respawn(ctx: &mut ScriptCtx<'_>, respawn_delay_millis: i64) {
    let at = ctx.now_ms() + respawn_delay_millis;
    ctx.world_mut().set_grand_boss_respawn_time(BAIUM_NPC_ID, at);
}

/// Selects and casts Baium skills based on HP ratio and aggro slots.
fn manage_skills(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let id = npc.object_id;
    if ctx.world().is_casting_now(id) || ctx.world().is_core_ai_disabled(id) || !ctx.world().is_in_combat(id)
    {
        return;
    }
    let mut variables = ctx
        .state::<BaiumState>()
        .variables
        .get(&id)
        .copied()
        .unwrap_or_default();
    for slot in &mut variables {
        let lost = match slot.0 {
            None => true,
            Some(attacker) => distance_between(ctx, id, attacker) > 9000.0 || ctx.world().is_dead(attacker),
        };
        if lost {
            slot.1 = 0;
        }
    }
    let max_index = index_of_max_value(&[variables[0].1, variables[1].1, variables[2].1]);
    let primary_target = variables[max_index].0;
    let current_aggro = variables[max_index].1;
    if (current_aggro > 0) && (ctx.get_random(100) < 70) {
        variables[max_index].1 = 500;
    }
    ctx.state::<BaiumState>().variables.insert(id, variables);
    let mut skill_to_cast = None;
    if let Some(primary_target) = primary_target
        && !ctx.world().is_dead(primary_target)
    {
        let (hp, max_hp) = hp_of(ctx, id);
        let health_ratio = hp / max_hp;
        skill_to_cast = Some(if health_ratio > 0.75 {
            if ctx.get_random(100) < 10 {
                SKILL_ENERGY_WAVE
            } else if ctx.get_random(100) < 10 {
                SKILL_EARTHQUAKE
            } else {
                SKILL_BAIUM_ATTACK
            }
        } else if health_ratio > 0.5 {
            if ctx.get_random(100) < 10 {
                SKILL_GROUP_HOLD
            } else if ctx.get_random(100) < 10 {
                SKILL_ENERGY_WAVE
            } else if ctx.get_random(100) < 10 {
                SKILL_EARTHQUAKE
            } else {
                SKILL_BAIUM_ATTACK
            }
        } else if health_ratio > 0.25 {
            if ctx.get_random(100) < 10 {
                SKILL_THUNDERBOLT
            } else if ctx.get_random(100) < 10 {
                SKILL_GROUP_HOLD
            } else if ctx.get_random(100) < 10 {
                SKILL_ENERGY_WAVE
            } else if ctx.get_random(100) < 10 {
                SKILL_EARTHQUAKE
            } else {
                SKILL_BAIUM_ATTACK
            }
        } else if ctx.get_random(100) < 10 {
            SKILL_THUNDERBOLT
        } else if ctx.get_random(100) < 10 {
            SKILL_GROUP_HOLD
        } else if ctx.get_random(100) < 10 {
            SKILL_ENERGY_WAVE
        } else if ctx.get_random(100) < 10 {
            SKILL_EARTHQUAKE
        } else {
            SKILL_BAIUM_ATTACK
        });
    }
    if let Some(skill_to_cast) = skill_to_cast
        && ctx.world_mut().check_do_cast_conditions_of(id, skill_to_cast)
    {
        ctx.world_mut().set_npc_target(id, primary_target);
        ctx.world_mut().do_cast(id, skill_to_cast);
    }
}

/// Gets a random valid player inside the Baium zone around the NPC: the first visible one in
/// 2000 that is in the rim and alive.
fn get_random_player(ctx: &ScriptCtx<'_>, npc: ObjectId) -> Option<ObjectId> {
    let (zone_id, zone_type) = BAIUM_RIM_ZONE;
    ctx.world()
        .visible_objects_in_range(npc, 2000)
        .into_iter()
        .find(|candidate| {
            let world = ctx.world();
            world.player(*candidate).is_some()
                && world.is_inside_zone_by_position(zone_id, zone_type, *candidate)
                && !world.is_dead(*candidate)
        })
}

// ------------------------------------------------------------------ world helpers

fn broadcast_rim(ctx: &ScriptCtx<'_>, packet: &impl l2_net::Encode) {
    let (zone_id, zone_type) = BAIUM_RIM_ZONE;
    ctx.world().broadcast_to_zone(zone_id, zone_type, packet);
}

/// `teleToLocation(new Location(x, y, z))`.
fn tele_location(ctx: &mut ScriptCtx<'_>, id: ObjectId, x: i32, y: i32, z: i32) {
    ctx.world_mut()
        .tele_to_location(id, TeleportTarget::at(x, y, z).with_instance(0));
}

fn hp_of(ctx: &ScriptCtx<'_>, id: ObjectId) -> (f64, f64) {
    ctx.world().hp_and_max(id).unwrap_or((0.0, 0.0))
}

fn distance_between(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    match (ctx.world().position_of(a), ctx.world().position_of(b)) {
        (Some(a), Some(b)) => distance_3d(a, b),
        _ => f64::MAX,
    }
}

/// Java `isInsideRadius3D(other, radius)`: `calculateDistance3D(other) < radius`.
fn inside_radius_3d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId, radius: f64) -> bool {
    distance_between(ctx, a, b) < radius
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{BossReadKind, BossReadThen, BossWrite, ScriptRegistry};

    use super::*;
    use crate::ai::bosses::test_support::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to};

    const DIR: &str = "data/scripts/ai/bosses/Baium";
    const ITEMS: &str = r#"<item id="4295" type="EtcItem" name="Blooded Fabric"><set name="is_stackable" val="true"/></item>"#;
    /// A place on the rooftop, inside the rim.
    const ON_ROOF: (i32, i32, i32) = (114500, 16000, 10070);

    fn creature_xml(id: i32, kind: &str, name: &str) -> String {
        format!(
            r#"<npc id="{id}" type="{kind}" level="75" name="{name}">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
    <corpseTime>13</corpseTime>
  </npc>"#
        )
    }

    fn npcs() -> String {
        let mut xml = creature_xml(BAIUM_NPC_ID, "GrandBoss", "Baium");
        xml += &creature_xml(ARCHANGEL_NPC_ID, "Monster", "Archangel");
        xml += r#"<npc id="29025" type="Folk" level="75" name="Baium"/>"#;
        xml += r#"<npc id="31862" type="Folk" level="70" name="Angelic Vortex"/>"#;
        xml += r#"<npc id="31842" type="Folk" level="70" name="Teleportation Cubic"/>"#;
        xml
    }

    fn setup_row(status: i32, respawn_time: i64, writes: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Baium);
        let vortex = r#"<html><body>31862<a action="bypass -h Script Baium ENTER">Go where the Angelic Vortex leads.</a></body></html>"#;
        let ask = r#"<a action="bypass Script Baium 31862-04.html">Ask about the stone statue.</a>"#;
        let pages: Vec<(String, String)> = [
            ("31862.html", vortex.to_owned()),
            ("31862-01.html", "<html><body>31862-01</body></html>".to_owned()),
            (
                "31862-02.html",
                format!("<html><body>31862-02{ask}</body></html>"),
            ),
            (
                "31862-03.html",
                format!("<html><body>31862-03{ask}</body></html>"),
            ),
            ("31862-04.html", "<html><body>31862-04</body></html>".to_owned()),
        ]
        .into_iter()
        .map(|(file, text)| (format!("{DIR}/{file}"), text))
        .collect();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        boss_test(
            registry,
            &npcs(),
            ITEMS,
            &pages,
            vec![row(BAIUM_NPC_ID, status, respawn_time, (115000, 16500, 10100))],
            writes,
        )
    }

    fn has_timer(t: &mut ScriptTest, name: &str, npc: Option<Oid>, player: Option<i32>) -> bool {
        let mut found = false;
        t.world.run_as_script("Baium", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            found = ctx.has_quest_timer(name, npc, player.map(Oid));
        });
        found
    }

    fn notify(t: &mut ScriptTest, event: &str, npc: Option<Oid>, player: Option<i32>) {
        t.world.run_as_script("Baium", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            ctx.notify_event(event, npc, player.map(Oid));
        });
    }

    fn status(t: &ScriptTest) -> Option<i32> {
        t.world.grand_boss_status(BAIUM_NPC_ID)
    }

    fn status_writes(t: &ScriptTest) -> Vec<i32> {
        grand_writes(&t.take_boss())
            .into_iter()
            .filter_map(|w| match w {
                BossWrite::GrandBossStatus { status, .. } => Some(status),
                _ => None,
            })
            .collect()
    }

    /// A player on the roof wakes Baium at the statue; the movie runs to the archangels.
    fn awake(t: &mut ScriptTest, player: i32) -> Oid {
        t.enter_with(player_at(player, ON_ROOF, vec![]));
        let stone = npcs_of(t, BAIUM_STONE_NPC_ID)[0];
        notify(t, EVENT_WAKE_UP, Some(stone), Some(player));
        run_ms(t, 27_100);
        appear(t, player);
        t.world.grand_boss_npc(BAIUM_NPC_ID).unwrap()
    }

    #[test]
    fn the_vortex_takes_the_blooded_fabric_and_sends_the_player_up() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        // The constructor: the statue sits where Baium will stand.
        let stone = npcs_of(&t, BAIUM_STONE_NPC_ID);
        assert_eq!(stone.len(), 1);
        let (x, y, _) = t.world.position_of(stone[0]).unwrap();
        assert_eq!((x, y), (LOCATION_BAIUM_SPAWN.0, LOCATION_BAIUM_SPAWN.1));

        let vortex = t.spawn(ANGELIC_VORTEX_NPC_ID, 20);
        t.enter(1, 80, vec![]);
        t.take();
        t.talk(1, vortex);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("31862"));
        t.bypass(1, "Script Baium ENTER");
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("31862-01"));

        t.enter(2, 80, vec![item(200, BLOODED_FABRIC_ITEM_ID, 2)]);
        t.talk(2, vortex);
        t.bypass(2, "Script Baium ENTER");
        appear(&mut t, 2);
        assert_eq!(t.count(2, BLOODED_FABRIC_ITEM_ID), 1);
        let (x, y, _) = t.world.position_of(Oid(2)).unwrap();
        assert_eq!((x, y), (LOCATION_TELEPORT_IN.0, LOCATION_TELEPORT_IN.1));
        assert!(
            t.world
                .zone_players_inside(70051, "NoRestartZone")
                .contains(&Oid(2))
        );
        // Nothing changes in the boss state.
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert!(status_writes(&t).is_empty());
    }

    #[test]
    fn the_wake_up_runs_the_movie_and_brings_the_archangels() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter_with(player_at(2, ON_ROOF, vec![]));
        t.take();
        t.take_boss();
        let stone = npcs_of(&t, BAIUM_STONE_NPC_ID)[0];
        notify(&mut t, EVENT_WAKE_UP, Some(stone), Some(2));
        assert_eq!(status(&t), Some(STATUS_IN_FIGHT));
        assert_eq!(status_writes(&t), [STATUS_IN_FIGHT]);
        assert!(t.world.npc(stone).is_none());
        let baium = t.world.grand_boss_npc(BAIUM_NPC_ID).unwrap();
        assert!(t.world.is_core_ai_disabled(baium));
        assert!(has_timer(&mut t, EVENT_CHECK_ATTACK, Some(baium), None));
        assert!(has_timer(&mut t, EVENT_SOCIAL_ACTION, Some(baium), Some(2)));

        run_ms(&mut t, 2_100);
        let names = names_to(&t.take(), 2);
        assert!(names.contains(&"SOCIAL_ACTION".to_owned()));
        assert!(names.contains(&"EARTHQUAKE".to_owned()));
        assert!(names.contains(&"PLAY_SOUND".to_owned()));

        // SOCIAL_ACTION at 10 s, PLAYER_PORT 6 s later: the waker stands before Baium.
        run_ms(&mut t, 14_000);
        appear(&mut t, 2);
        let (x, y, _) = t.world.position_of(Oid(2)).unwrap();
        assert_eq!((x, y), (LOCATION_BAIUM_GIFT.0, LOCATION_BAIUM_GIFT.1));
        // PLAYER_KILL 3 s later: "How dare you wake me!".
        run_ms(&mut t, 3_000);
        assert!(names_to(&t.take(), 2).contains(&"NPC_SAY".to_owned()));
        assert!(t.world.is_core_ai_disabled(baium));
        // SPAWN_ARCHANGEL 8 s later.
        run_ms(&mut t, 8_000);
        assert_eq!(npcs_of(&t, ARCHANGEL_NPC_ID).len(), 5);
        assert!(!t.world.is_core_ai_disabled(baium));
        for archangel in npcs_of(&t, ARCHANGEL_NPC_ID) {
            assert!(has_timer(&mut t, EVENT_SELECT_TARGET, Some(archangel), None));
        }
        // The vortex now says the fight runs.
        let vortex = t.spawn(ANGELIC_VORTEX_NPC_ID, 20);
        t.enter(3, 80, vec![item(300, BLOODED_FABRIC_ITEM_ID, 1)]);
        t.talk(3, vortex);
        t.take();
        t.bypass(3, "Script Baium ENTER");
        assert!(pages_to(&t.take(), 3).pop().unwrap().contains("31862-02"));
        assert_eq!(t.count(3, BLOODED_FABRIC_ITEM_ID), 1);
        t.bypass(3, "Script Baium 31862-04.html");
        assert!(pages_to(&t.take(), 3).pop().unwrap().contains("31862-04"));
    }

    #[test]
    fn the_death_on_the_roof_writes_the_state_and_the_gm_respawn_brings_the_statue() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        let baium = awake(&mut t, 2);
        t.take_boss();
        let before = now_ms();
        assert!(t.world.do_die(baium, Some(Oid(2))));
        run_ms(&mut t, 2_600);
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert_eq!(status_writes(&t), [STATUS_DEAD]);
        assert_eq!(npcs_of(&t, TELEPORT_CUBE_NPC_ID).len(), 1);
        assert!(has_timer(&mut t, EVENT_CLEAR_STATUS, None, None));
        assert!(has_timer(&mut t, EVENT_CLEAR_ZONE, None, None));
        for archangel in npcs_of(&t, ARCHANGEL_NPC_ID) {
            assert!(!has_timer(&mut t, EVENT_SELECT_TARGET, Some(archangel), None));
        }
        // 168 hours +- 48 (the Java defaults).
        let respawn = t.world.grand_boss_info(BAIUM_NPC_ID).unwrap().respawn_time;
        let hour = 3_600_000;
        assert!(respawn >= before + 120 * hour && respawn <= now_ms() + 216 * hour);

        notify(&mut t, EVENT_RESPAWN_BAIUM, None, Some(2));
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(npcs_of(&t, BAIUM_STONE_NPC_ID).len(), 1);
        assert!(!has_timer(&mut t, EVENT_CLEAR_STATUS, None, None));
        assert!(!has_timer(&mut t, EVENT_CLEAR_ZONE, None, None));
        assert_eq!(status_writes(&t), [STATUS_ALIVE]);
    }

    #[test]
    fn a_kill_from_outside_the_rim_changes_nothing() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        let baium = awake(&mut t, 2);
        t.enter(4, 80, vec![]);
        t.take_boss();
        assert!(t.world.do_die(baium, Some(Oid(4))));
        run_ms(&mut t, 2_600);
        assert_eq!(status(&t), Some(STATUS_IN_FIGHT));
        assert!(status_writes(&t).is_empty());
        assert!(!has_timer(&mut t, EVENT_CLEAR_STATUS, None, None));
    }

    #[test]
    fn half_an_hour_without_a_hit_clears_the_roof() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        let baium = awake(&mut t, 2);
        t.world.run_as_script("Baium", |ctx| {
            ctx.state::<BaiumState>().last_attack_time = 0;
        });
        t.take_boss();
        notify(&mut t, EVENT_CHECK_ATTACK, Some(baium), None);
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert_eq!(status_writes(&t), [STATUS_ALIVE]);
        assert!(t.world.npc(baium).is_none());
        assert!(npcs_of(&t, ARCHANGEL_NPC_ID).is_empty());
        assert_eq!(npcs_of(&t, BAIUM_STONE_NPC_ID).len(), 1);
        // The player goes down through `EXIT`: one of the three exits, up to 99 away.
        let (x, y, _) = t.world.position_of(Oid(2)).unwrap();
        assert!(
            TELEPORT_OUT_LOCATIONS
                .iter()
                .any(|(ox, oy, _)| (*ox..ox + 100).contains(&x) && (*oy..oy + 100).contains(&y))
        );
    }

    #[test]
    fn with_the_switch_off_nothing_is_written_and_the_row_decides_the_respawn() {
        let mut t = setup_row(STATUS_ALIVE, 0, false);
        let baium = awake(&mut t, 2);
        assert!(t.world.do_die(baium, Some(Oid(2))));
        run_ms(&mut t, 2_600);
        assert_eq!(status(&t), Some(STATUS_DEAD));
        assert!(grand_writes(&t.take_boss()).is_empty());

        notify(&mut t, EVENT_RESPAWN_BAIUM, None, Some(2));
        let then = BossReadThen::ScriptEvent {
            script: "Baium",
            event: EVENT_ROW_READ,
        };
        assert_eq!(
            rereads(&t.take_boss()),
            [(BossReadKind::GrandBoss(BAIUM_NPC_ID), then)]
        );
        assert!(npcs_of(&t, BAIUM_STONE_NPC_ID).is_empty());
        // The other server's Baium is in a fight: the statue stays away.
        answer(
            &mut t,
            then,
            BAIUM_NPC_ID,
            row(BAIUM_NPC_ID, STATUS_IN_FIGHT, 0, (1, 2, 3)),
        );
        assert!(npcs_of(&t, BAIUM_STONE_NPC_ID).is_empty());
        answer(
            &mut t,
            then,
            BAIUM_NPC_ID,
            row(BAIUM_NPC_ID, STATUS_ALIVE, 0, (1, 2, 3)),
        );
        assert_eq!(npcs_of(&t, BAIUM_STONE_NPC_ID).len(), 1);
        assert!(grand_writes(&t.take_boss()).is_empty());
    }

    #[test]
    fn the_constructor_restores_the_row() {
        // In fight, this server writes: Baium and the archangels are back.
        let mut t = setup_row(STATUS_IN_FIGHT, 0, true);
        let baium = t.world.grand_boss_npc(BAIUM_NPC_ID).unwrap();
        let (x, y, _) = t.world.position_of(baium).unwrap();
        assert_eq!((x, y), (115000, 16500));
        assert_eq!(npcs_of(&t, ARCHANGEL_NPC_ID).len(), 5);
        assert!(has_timer(&mut t, EVENT_CHECK_ATTACK, Some(baium), None));
        // D-54: on the shared database that is the other server's fight.
        let t = setup_row(STATUS_IN_FIGHT, 0, false);
        assert!(npcs_of(&t, BAIUM_NPC_ID).is_empty());
        assert!(npcs_of(&t, ARCHANGEL_NPC_ID).is_empty());
        // Waiting goes back to alive, without a statue (Java).
        let t = setup_row(STATUS_WAITING, 0, true);
        assert_eq!(status(&t), Some(STATUS_ALIVE));
        assert!(npcs_of(&t, BAIUM_STONE_NPC_ID).is_empty());
        // Dead, the time is over: the statue, or a read of the row first.
        let t = setup_row(STATUS_DEAD, now_ms() - 1000, true);
        assert_eq!(npcs_of(&t, BAIUM_STONE_NPC_ID).len(), 1);
        let t = setup_row(STATUS_DEAD, now_ms() - 1000, false);
        assert!(npcs_of(&t, BAIUM_STONE_NPC_ID).is_empty());
        assert_eq!(rereads(&t.take_boss()).len(), 1);
    }

    #[test]
    fn the_hate_slots_keep_three_attackers() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        let baium = awake(&mut t, 2);
        t.world.run_as_script("Baium", |ctx| {
            let npc = ctx.script_npc(baium).unwrap();
            for attacker in [10, 11, 12, 13] {
                refresh_ai_params_with(ctx, Oid(attacker), npc, 5000, 5000);
            }
            let slots = ctx.state::<BaiumState>().variables[&baium];
            // The fourth replaced the lowest of the first three.
            let ids: Vec<i32> = slots.iter().map(|s| s.0.unwrap().0).collect();
            assert!(ids.contains(&13));
            assert_eq!(ids.len(), 3);
            // A known attacker below its threshold gets the new value, nobody is replaced.
            refresh_ai_params_with(ctx, Oid(13), npc, 9000, 9000);
            let again = ctx.state::<BaiumState>().variables[&baium];
            let mine = again.iter().find(|s| s.0 == Some(Oid(13))).unwrap().1;
            assert!((9000..12000).contains(&mine));
        });
    }

    #[test]
    fn the_gm_events_answer_when_they_cannot_run() {
        let mut t = setup_row(STATUS_ALIVE, 0, true);
        t.enter(5, 80, vec![]);
        t.take();
        for event in [EVENT_RESPAWN_BAIUM, EVENT_ABORT_FIGHT, EVENT_DESPAWN_MINIONS] {
            notify(&mut t, event, None, Some(5));
            assert_eq!(names_to(&t.take(), 5), ["SYSTEM_MESSAGE"], "{event}");
        }
        assert_eq!(status(&t), Some(STATUS_ALIVE));
    }
}
