//! Sailren, the Primeval Isle raid: Shilen's Stone Statue takes a party in with a Gazkh, three
//! Velociraptors, a Pterosaur and a Tyrannosaurus come first, then Sailren with his cinematic;
//! one hour after his death the lair opens again.
// Java: dist/game/data/scripts/ai/bosses/Sailren/Sailren.java
//
// The Java constructor is `setup` (the listeners) and `on_load` (the respawn row); the `static`
// fields are `SailrenState` (`ctx.state`). Java disables the script when the zone 70049 is not
// there: `enabled` does the same.
//
// Side by side (D-54): `SailrenRespawn` is a global variable the Java GS writes too. `ENTER`
// reads it again before it decides (`ctx.reread_global_variables`); a respawn time in the future
// that was not known before marks the raid dead. A raid this server killed or started itself keeps
// its own status.
//
// Not modelled: `unload` (D-16: no reload).

use l2_protocol_game::server::SpecialCamera;
use l2_util::ObjectId;
use l2_world::{NpcClass, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportWhereType, World};
use tracing::warn;

// NPC identifiers.
const STATUE_NPC_ID: i32 = 32109; // Shilen's Stone Statue.
const MOVIE_NPC_ID: i32 = 32110; // Invisible NPC used for camera.
const SAILREN_NPC_ID: i32 = 29065;
const VELOCIRAPTOR_NPC_ID: i32 = 22218;
const PTEROSAUR_NPC_ID: i32 = 22199;
const TREX_NPC_ID: i32 = 22217;
const TELEPORT_CUBE_NPC_ID: i32 = 32107;

// Item identifiers.
const GAZKH_ITEM_ID: i32 = 8784;

// Skill holders.
const CINEMATIC_ANIMATION_SKILL: SkillKey = SkillKey { id: 5090, level: 1 };

// Zone configuration.
const SAILREN_ZONE_ID: i32 = 70049;
const SAILREN_ZONE_TYPE: &str = "NoRestartZone";

// Location constants.
const LAIR_CENTER: (i32, i32, i32) = (27549, -6638, -2008);

const VELOCIRAPTOR_BASE_LOCATION: (i32, i32, i32) = (27313, -6766, -1975);
const VELOCIRAPTOR_SPAWN_RANDOM: i32 = 150;

const PTEROSAUR_SPAWN_LOCATION: (i32, i32, i32) = (27313, -6766, -1975);
const TREX_SPAWN_LOCATION: (i32, i32, i32) = (27313, -6766, -1975);

const TELEPORT_CUBE_SPAWN_LOCATION: (i32, i32, i32) = (27644, -6638, -2008);

const TELEPORT_STATUE_RADIUS: f64 = 1000.0;

// Time and delay configuration.
const RESPAWN_DELAY_MILLIS: i64 = 3_600_000; // 1 hour respawn, retail-like.
const MAX_FIGHT_DURATION_MILLIS: i64 = 3200 * 1000; // ~53 minutes, retail-like.
const INACTIVITY_LIMIT_MILLIS: i64 = 600_000; // 10 minutes without attacks.
const INACTIVITY_CHECK_INTERVAL_MILLIS: i64 = 120_000; // 2 minutes.
const INITIAL_VELOCIRAPTOR_DELAY_MILLIS: i64 = 60_000; // 1 minute after entry.
const TREX_TO_SAILREN_DELAY_MILLIS: i64 = 180_000; // 3 minutes after T-Rex death.
const TELEPORT_CUBE_LIFETIME_MILLIS: i64 = 300_000; // 5 minutes.
const SAILREN_INTRO_INVUL_DELAY_MILLIS: i64 = 24_600;
const CAMERA_STEP_ONE_DELAY_MILLIS: i64 = 4100;
const CAMERA_STEP_INTERVAL_MILLIS: i64 = 3000;
const CAMERA_STEP_FIVE_TO_SIX_DELAY_MILLIS: i64 = 7000;
const ANIMATION_INTERVAL_MILLIS: i64 = 2000;
const MOVIE_NPC_LIFETIME_MILLIS: i64 = 26_000;

// Event identifiers.
const EVENT_ENTER: &str = "ENTER";
const EVENT_EXIT: &str = "EXIT";
const EVENT_SPAWN_VELOCIRAPTORS: &str = "SPAWN_VELOCIRAPTORS";
const EVENT_SPAWN_SAILREN: &str = "SPAWN_SAILREN";
const EVENT_ANIMATION: &str = "ANIMATION";
const EVENT_CAMERA_STEP_ONE: &str = "CAMERA_1";
const EVENT_CAMERA_STEP_TWO: &str = "CAMERA_2";
const EVENT_CAMERA_STEP_THREE: &str = "CAMERA_3";
const EVENT_CAMERA_STEP_FOUR: &str = "CAMERA_4";
const EVENT_CAMERA_STEP_FIVE: &str = "CAMERA_5";
const EVENT_CAMERA_STEP_SIX: &str = "CAMERA_6";
const EVENT_ATTACK: &str = "ATTACK";
const EVENT_CLEAR_STATUS: &str = "CLEAR_STATUS";
const EVENT_TIMEOUT: &str = "TIME_OUT";
const EVENT_CHECK_INACTIVITY: &str = "CHECK_ATTACK";
/// Not a Java event: the continuation of the D-54 re-read `ENTER` asks for.
const EVENT_ENTER_ROW_READ: &str = "D54_ENTER_ROW_READ";

// Global variable key.
const GLOBAL_VAR_RESPAWN_TIME: &str = "SailrenRespawn";

// Status flags.
const STATUS_ALIVE: u8 = 0;
const STATUS_IN_FIGHT: u8 = 1;
const STATUS_DEAD: u8 = 2;

/// The static fields of the Java class.
#[derive(Debug)]
pub(crate) struct SailrenState {
    /// `_raidStatus`.
    pub(crate) raid_status: u8,
    /// `_killCount`.
    pub(crate) kill_count: i32,
    /// `_lastAttackTime`.
    pub(crate) last_attack_time: i64,
}

impl Default for SailrenState {
    fn default() -> Self {
        Self {
            raid_status: STATUS_ALIVE,
            kill_count: 0,
            last_attack_time: 0,
        }
    }
}

/// Java `ai.bosses.Sailren.Sailren`.
pub struct Sailren;

impl Script for Sailren {
    fn name(&self) -> &'static str {
        "Sailren"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Sailren"
    }

    /// Java: the zone must exist; otherwise the AI is disabled cleanly.
    fn enabled(&self, world: &World) -> bool {
        let found = world.zone_by_id(SAILREN_ZONE_ID, SAILREN_ZONE_TYPE).is_some();
        if !found {
            warn!("Sailren: Sailren zone {SAILREN_ZONE_ID} not found. AI disabled.");
        }
        found
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[STATUE_NPC_ID, TELEPORT_CUBE_NPC_ID]);
        s.add_talk_id(&[STATUE_NPC_ID, TELEPORT_CUBE_NPC_ID]);
        s.add_first_talk_id(&[STATUE_NPC_ID]);
        s.add_kill_id(&[VELOCIRAPTOR_NPC_ID, PTEROSAUR_NPC_ID, TREX_NPC_ID, SAILREN_NPC_ID]);
        s.add_attack_id(&[VELOCIRAPTOR_NPC_ID, PTEROSAUR_NPC_ID, TREX_NPC_ID, SAILREN_NPC_ID]);
    }

    /// The constructor's respawn row.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        sync_with_respawn_row(ctx);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "32109-01.html" | "32109-01a.html" | "32109-02a.html" | "32109-03a.html" => {
                return Some(event.to_owned());
            }
            EVENT_ENTER => {
                // D-54: the respawn row decides, so it is read again first.
                ctx.reread_global_variables(&[GLOBAL_VAR_RESPAWN_TIME], EVENT_ENTER_ROW_READ, npc, player);
            }
            EVENT_ENTER_ROW_READ => {
                sync_with_respawn_row(ctx);
                return handle_enter_event(ctx, npc, player);
            }
            EVENT_EXIT => {
                if let Some(player) = player {
                    ctx.world_mut()
                        .tele_to_location_where(player, TeleportWhereType::Town);
                }
            }
            EVENT_SPAWN_VELOCIRAPTORS => spawn_velociraptors(ctx),
            EVENT_SPAWN_SAILREN => spawn_sailren_with_cinematic(ctx),
            EVENT_ANIMATION => {
                if let Some(npc) = npc {
                    ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
                    ctx.world_mut().do_cast(npc.object_id, CINEMATIC_ANIMATION_SKILL);
                    ctx.start_quest_timer(EVENT_ANIMATION, ANIMATION_INTERVAL_MILLIS, Some(npc), None, false);
                }
            }
            EVENT_CAMERA_STEP_ONE => {
                if let Some(npc) = npc {
                    broadcast_to_zone(ctx, &camera(npc, 100, 180, 30, 3000, 20000, 0, 50, 1));
                    ctx.start_quest_timer(
                        EVENT_CAMERA_STEP_TWO,
                        CAMERA_STEP_INTERVAL_MILLIS,
                        Some(npc),
                        None,
                        false,
                    );
                }
            }
            EVENT_CAMERA_STEP_TWO => {
                if let Some(npc) = npc {
                    broadcast_to_zone(ctx, &camera(npc, 150, 270, 25, 3000, 20000, 0, 30, 1));
                    ctx.start_quest_timer(
                        EVENT_CAMERA_STEP_THREE,
                        CAMERA_STEP_INTERVAL_MILLIS,
                        Some(npc),
                        None,
                        false,
                    );
                }
            }
            EVENT_CAMERA_STEP_THREE => {
                if let Some(npc) = npc {
                    broadcast_to_zone(ctx, &camera(npc, 160, 360, 20, 3000, 20000, 10, 15, 1));
                    ctx.start_quest_timer(
                        EVENT_CAMERA_STEP_FOUR,
                        CAMERA_STEP_INTERVAL_MILLIS,
                        Some(npc),
                        None,
                        false,
                    );
                }
            }
            EVENT_CAMERA_STEP_FOUR => {
                if let Some(npc) = npc {
                    broadcast_to_zone(ctx, &camera(npc, 160, 450, 10, 3000, 20000, 0, 10, 1));
                    ctx.start_quest_timer(
                        EVENT_CAMERA_STEP_FIVE,
                        CAMERA_STEP_INTERVAL_MILLIS,
                        Some(npc),
                        None,
                        false,
                    );
                }
            }
            EVENT_CAMERA_STEP_FIVE => {
                if let Some(npc) = npc {
                    broadcast_to_zone(ctx, &camera(npc, 160, 560, 0, 3000, 20000, 0, 10, 1));
                    ctx.start_quest_timer(
                        EVENT_CAMERA_STEP_SIX,
                        CAMERA_STEP_FIVE_TO_SIX_DELAY_MILLIS,
                        Some(npc),
                        None,
                        false,
                    );
                }
            }
            EVENT_CAMERA_STEP_SIX => {
                if let Some(npc) = npc {
                    broadcast_to_zone(ctx, &camera(npc, 70, 560, 0, 500, 7000, -15, 20, 1));
                }
            }
            EVENT_ATTACK => {
                if let Some(npc) = npc
                    && ctx.world().npc(npc.object_id).is_some_and(|n| n.class.is_raid())
                {
                    ctx.world_mut().set_invul(npc.object_id, false);
                    ctx.set_immobilized(npc, false);
                }
            }
            EVENT_CLEAR_STATUS => {
                let st = ctx.state::<SailrenState>();
                st.raid_status = STATUS_ALIVE;
                st.kill_count = 0;
                st.last_attack_time = 0;
            }
            EVENT_TIMEOUT => handle_timeout(ctx),
            EVENT_CHECK_INACTIVITY => handle_inactivity_check(ctx),
            _ => {}
        }

        None
    }

    /// The only logic here is the last attack time that drives the inactivity timeout.
    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx
            .world()
            .is_character_in_zone(SAILREN_ZONE_ID, SAILREN_ZONE_TYPE, attacker)
        {
            ctx.state::<SailrenState>().last_attack_time = ctx.now_ms();
        }
    }

    /// The staged progression: three Velociraptors, the Pterosaur, the Tyrannosaurus, Sailren.
    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if !ctx
            .world()
            .is_character_in_zone(SAILREN_ZONE_ID, SAILREN_ZONE_TYPE, killer)
        {
            return;
        }

        match npc.id {
            SAILREN_NPC_ID => {
                ctx.state::<SailrenState>().raid_status = STATUS_DEAD;

                let (x, y, z) = TELEPORT_CUBE_SPAWN_LOCATION;
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

                let respawn_time = ctx.now_ms() + RESPAWN_DELAY_MILLIS;
                ctx.set_global_variable(GLOBAL_VAR_RESPAWN_TIME, &respawn_time.to_string());

                ctx.cancel_quest_timer(EVENT_CHECK_INACTIVITY, None, None);
                ctx.cancel_quest_timer(EVENT_TIMEOUT, None, None);

                ctx.start_quest_timer(EVENT_CLEAR_STATUS, RESPAWN_DELAY_MILLIS, None, None, false);
                ctx.start_quest_timer(EVENT_TIMEOUT, TELEPORT_CUBE_LIFETIME_MILLIS, None, None, false);
            }
            VELOCIRAPTOR_NPC_ID => {
                ctx.state::<SailrenState>().kill_count += 1;
                if ctx.state::<SailrenState>().kill_count == 3 {
                    let (x, y, z) = PTEROSAUR_SPAWN_LOCATION;
                    if let Some(pterosaur) = ctx.add_spawn(PTEROSAUR_NPC_ID, x, y, z, 0, false, 0, 0) {
                        ctx.add_attack_desire(pterosaur, killer, 999);
                    }
                    ctx.state::<SailrenState>().kill_count = 0;
                }
            }
            PTEROSAUR_NPC_ID => {
                let (x, y, z) = TREX_SPAWN_LOCATION;
                if let Some(trex) = ctx.add_spawn(TREX_NPC_ID, x, y, z, 0, false, 0, 0) {
                    ctx.add_attack_desire(trex, killer, 999);
                }
            }
            TREX_NPC_ID => {
                ctx.start_quest_timer(
                    EVENT_SPAWN_SAILREN,
                    TREX_TO_SAILREN_DELAY_MILLIS,
                    None,
                    None,
                    false,
                );
            }
            _ => {}
        }
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

/// D-54: the respawn row as it is now. A time in the future that this server did not know marks
/// the raid dead until then; a raid in a fight keeps its status. This is the constructor's
/// `storedRespawnTime` test as well.
fn sync_with_respawn_row(ctx: &mut ScriptCtx<'_>) {
    let stored_respawn_time = ctx.global_variables().get_long(GLOBAL_VAR_RESPAWN_TIME, 0);
    let remaining_delay = stored_respawn_time - ctx.now_ms();
    if remaining_delay > 0 {
        if ctx.state::<SailrenState>().raid_status == STATUS_ALIVE {
            ctx.state::<SailrenState>().raid_status = STATUS_DEAD;
            ctx.cancel_quest_timers(EVENT_CLEAR_STATUS);
            ctx.start_quest_timer(EVENT_CLEAR_STATUS, remaining_delay, None, None, false);
        }
    } else if ctx.state::<SailrenState>().raid_status == STATUS_DEAD
        && !ctx.has_quest_timer(EVENT_CLEAR_STATUS, None, None)
    {
        // Nothing is going to clear it.
        ctx.state::<SailrenState>().raid_status = STATUS_ALIVE;
    }
}

/// Party entry: checks the party, the status, the leader and the Gazkh, then takes the party in.
fn handle_enter_event(
    ctx: &mut ScriptCtx<'_>,
    npc: Option<ScriptNpc>,
    player: Option<ObjectId>,
) -> Option<String> {
    let party = player.and_then(|p| ctx.world().party_of(p));
    let raid_status = ctx.state::<SailrenState>().raid_status;

    let html = if let (Some(player), Some(party)) = (player, party) {
        if raid_status == STATUS_DEAD {
            "32109-04.html"
        } else if raid_status == STATUS_IN_FIGHT {
            "32109-05.html"
        } else if ctx.world().party_leader(party) != Some(player) {
            "32109-03.html"
        } else if !ctx.has_quest_items(player, GAZKH_ITEM_ID) {
            "32109-02.html"
        } else {
            // Valid entry: consume Gazkh and move the party inside.
            ctx.take_items(player, GAZKH_ITEM_ID, 1);
            let now = ctx.now_ms();
            {
                let st = ctx.state::<SailrenState>();
                st.raid_status = STATUS_IN_FIGHT;
                st.kill_count = 0;
                st.last_attack_time = now;
            }

            // Java: `npc` is dereferenced for every member; a null one throws.
            let npc = npc?;
            for member in ctx.world().party_members(party) {
                if crate::ai::areas::gracia::util::distance_3d(ctx, member, npc.object_id)
                    < TELEPORT_STATUE_RADIUS
                {
                    let (x, y, z) = LAIR_CENTER;
                    ctx.teleport_player(member, x, y, z, 0, false);
                }
            }

            ctx.start_quest_timer(
                EVENT_SPAWN_VELOCIRAPTORS,
                INITIAL_VELOCIRAPTOR_DELAY_MILLIS,
                None,
                None,
                false,
            );
            ctx.start_quest_timer(EVENT_TIMEOUT, MAX_FIGHT_DURATION_MILLIS, None, None, false);
            ctx.start_quest_timer(
                EVENT_CHECK_INACTIVITY,
                INACTIVITY_CHECK_INTERVAL_MILLIS,
                None,
                None,
                false,
            );
            return None;
        }
    } else {
        "32109-01.html"
    };

    Some(html.to_owned())
}

/// Three Velociraptors around the base location, with a small random offset.
fn spawn_velociraptors(ctx: &mut ScriptCtx<'_>) {
    for _ in 0..3 {
        let (bx, by, bz) = VELOCIRAPTOR_BASE_LOCATION;
        let spawn_x = bx + ctx.get_random(VELOCIRAPTOR_SPAWN_RANDOM);
        let spawn_y = by + ctx.get_random(VELOCIRAPTOR_SPAWN_RANDOM);
        ctx.add_spawn(VELOCIRAPTOR_NPC_ID, spawn_x, spawn_y, bz, 0, false, 0, 0);
    }
}

/// Sailren at the lair center with the introductory cinematic: invulnerable and immobilized until
/// it is over, a movie NPC as the camera pivot.
fn spawn_sailren_with_cinematic(ctx: &mut ScriptCtx<'_>) {
    let (x, y, z) = LAIR_CENTER;
    let sailren_npc = ctx.add_spawn(SAILREN_NPC_ID, x, y, z, 0, false, 0, 0);
    let Some(sailren) = sailren_npc.filter(|n| {
        ctx.world()
            .npc(n.object_id)
            .is_some_and(|s| s.class == NpcClass::RaidBoss)
    }) else {
        warn!("Sailren: Failed to spawn Sailren raid boss (ID: {SAILREN_NPC_ID}).");
        return;
    };

    let (sx, sy, sz) = ctx.world().position_of(sailren.object_id).unwrap_or((x, y, z));
    let Some(movie_npc) = ctx.add_spawn(
        MOVIE_NPC_ID,
        sx,
        sy,
        sz + 30,
        0,
        false,
        MOVIE_NPC_LIFETIME_MILLIS,
        0,
    ) else {
        warn!("Sailren: Failed to spawn Sailren cinematic NPC (ID: {MOVIE_NPC_ID}).");
        return;
    };

    ctx.world_mut().set_invul(sailren.object_id, true);
    ctx.set_immobilized(sailren, true);

    // Initial camera shot.
    broadcast_to_zone(ctx, &camera(movie_npc, 60, 110, 30, 4000, 20000, 0, 65, 1));

    // Start cinematic sequence.
    ctx.start_quest_timer(
        EVENT_ATTACK,
        SAILREN_INTRO_INVUL_DELAY_MILLIS,
        Some(sailren),
        None,
        false,
    );
    ctx.start_quest_timer(
        EVENT_ANIMATION,
        ANIMATION_INTERVAL_MILLIS,
        Some(movie_npc),
        None,
        false,
    );
    ctx.start_quest_timer(
        EVENT_CAMERA_STEP_ONE,
        CAMERA_STEP_ONE_DELAY_MILLIS,
        Some(movie_npc),
        None,
        false,
    );
}

/// The end of the encounter: the status resets, the players go to town, the NPCs of the zone go.
fn handle_timeout(ctx: &mut ScriptCtx<'_>) {
    if ctx.state::<SailrenState>().raid_status == STATUS_IN_FIGHT {
        ctx.state::<SailrenState>().raid_status = STATUS_ALIVE;
    }

    for creature in ctx
        .world()
        .zone_characters_inside(SAILREN_ZONE_ID, SAILREN_ZONE_TYPE)
    {
        if ctx.world().player(creature).is_some() {
            ctx.world_mut()
                .tele_to_location_where(creature, TeleportWhereType::Town);
        } else if let Some(npc) = ctx.script_npc(creature) {
            ctx.delete_npc(npc);
        }
    }

    let st = ctx.state::<SailrenState>();
    st.kill_count = 0;
    st.last_attack_time = 0;
}

/// Without an attack for ten minutes the fight ends; otherwise the check comes again.
fn handle_inactivity_check(ctx: &mut ScriptCtx<'_>) {
    if ctx.state::<SailrenState>().raid_status != STATUS_IN_FIGHT {
        return;
    }

    if ctx
        .world()
        .zone_players_inside(SAILREN_ZONE_ID, SAILREN_ZONE_TYPE)
        .is_empty()
    {
        // No players inside: no reschedule.
        return;
    }

    if (ctx.state::<SailrenState>().last_attack_time + INACTIVITY_LIMIT_MILLIS) < ctx.now_ms() {
        ctx.cancel_quest_timer(EVENT_TIMEOUT, None, None);
        ctx.cancel_quest_timer(EVENT_CHECK_INACTIVITY, None, None);
        ctx.notify_event(EVENT_TIMEOUT, None, None);
    } else {
        ctx.start_quest_timer(
            EVENT_CHECK_INACTIVITY,
            INACTIVITY_CHECK_INTERVAL_MILLIS,
            None,
            None,
            false,
        );
    }
}

/// `SAILREN_ZONE.broadcastPacket(packet)`.
fn broadcast_to_zone(ctx: &ScriptCtx<'_>, packet: &impl l2_net::Encode) {
    ctx.world()
        .broadcast_to_zone(SAILREN_ZONE_ID, SAILREN_ZONE_TYPE, packet);
}

/// `new SpecialCamera(npc, force, angle1, angle2, time, 1500, duration, relYaw, relPitch,
/// isWide, 0, 0)`; the `range` of 1500 is not in the packet.
#[allow(clippy::too_many_arguments)]
fn camera(
    npc: ScriptNpc,
    force: i32,
    angle1: i32,
    angle2: i32,
    time: i32,
    duration: i32,
    rel_yaw: i32,
    rel_pitch: i32,
    is_wide: i32,
) -> SpecialCamera {
    SpecialCamera::new(
        npc.object_id.0,
        force,
        angle1,
        angle2,
        time,
        duration,
        rel_yaw,
        rel_pitch,
        is_wide,
        0,
        0,
    )
}

#[cfg(test)]
mod tests {
    use l2_world::{ScriptRegistry, ScriptSettings, WorldCommand};

    use super::*;
    use crate::testing::{ScriptTest, item};

    const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision>"#;

    fn npcs() -> String {
        format!(
            r#"<npc id="32109" type="Folk" level="70" name="Shilen's Stone Statue"/>
<npc id="32110" type="Folk" level="70" name="Camera"/>
<npc id="32107" type="Folk" level="70" name="Cube"/>
<npc id="29065" type="RaidBoss" level="80" name="Sailren">{STATS}</npc>
<npc id="22218" type="Monster" level="80" name="Velociraptor">{STATS}</npc>
<npc id="22199" type="Monster" level="80" name="Pterosaur">{STATS}</npc>
<npc id="22217" type="Monster" level="80" name="Tyrannosaurus">{STATS}</npc>"#
        )
    }

    const ITEMS: &str =
        r#"<item id="8784" type="EtcItem" name="Gazkh"><set name="is_questitem" val="true"/></item>"#;

    /// `data/zones/no_restart.xml`, zone 70049: a square around the lair (the real shape is
    /// larger).
    const ZONES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="sailen_no_restart" id="70049" type="NoRestartZone" shape="NPoly" minZ="-2560" maxZ="440">
		<stat name="restartTime" val="0" />
		<node X="25000" Y="-9000" />
		<node X="30000" Y="-9000" />
		<node X="30000" Y="-4000" />
		<node X="25000" Y="-4000" />
	</zone>
</list>
"#;

    const PAGES: [(&str, &str); 2] = [
        ("data/zones/sailren.xml", ZONES),
        (
            "data/scripts/ai/bosses/Sailren/32109.html",
            "<html><body>Statue</body></html>",
        ),
    ];

    fn world_with(zones: bool, writes: bool, rows: Vec<(String, Option<String>)>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Sailren);
        let pages: &[(&str, &str)] = if zones { &PAGES } else { &PAGES[1..] };
        ScriptTest::new_with(registry, &npcs(), ITEMS, pages, move |world, root| {
            world.set_zone_data(
                std::sync::Arc::new(l2_data::Zones::load(root)),
                std::sync::Arc::new(l2_data::MapRegions::load(root)),
            );
            world.set_script_settings(ScriptSettings {
                cycle_database_writes: writes,
                ..ScriptSettings::default()
            });
            world.set_global_variables(rows);
        })
    }

    fn world() -> ScriptTest {
        world_with(true, true, vec![])
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap())
            .unwrap()
    }

    fn st<R>(t: &mut ScriptTest, f: impl FnOnce(&mut SailrenState) -> R) -> R {
        let mut out = None;
        t.world
            .run_as_script("Sailren", |ctx| out = Some(f(ctx.state::<SailrenState>())));
        out.unwrap()
    }

    fn event(t: &mut ScriptTest, event: &str, player: Option<i32>, npc: Option<ObjectId>) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("Sailren", |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            out = Sailren.on_event(ctx, event, npc, player.map(ObjectId));
        });
        out
    }

    fn has_timer(t: &mut ScriptTest, name: &str) -> bool {
        let mut found = false;
        t.world
            .run_as_script("Sailren", |ctx| found = ctx.has_quest_timer(name, None, None));
        found
    }

    /// A player standing next to the statue in the lair's neighbourhood.
    fn enter(t: &mut ScriptTest, id: i32, gazkh: bool) {
        let items = if gazkh {
            vec![item(900_000 + id, GAZKH_ITEM_ID, 1)]
        } else {
            vec![]
        };
        t.enter(id, 80, items);
    }

    /// Two characters, 1 is the leader of the party.
    fn party_of_two(t: &mut ScriptTest, gazkh: bool) -> ObjectId {
        enter(t, 1, gazkh);
        enter(t, 2, false);
        t.party(1, 2);
        t.spawn(STATUE_NPC_ID, 10)
    }

    #[test]
    fn without_the_zone_the_script_is_not_installed() {
        let t = world_with(false, true, vec![]);
        assert!(!t.world.has_script("Sailren"));
        let t = world();
        assert!(t.world.has_script("Sailren"));
    }

    #[test]
    fn the_statue_page_and_the_html_events() {
        let mut t = world();
        let statue = t.spawn(STATUE_NPC_ID, 10);
        let mut page = None;
        t.world.run_as_script("Sailren", |ctx| {
            let npc = ctx.script_npc(statue).unwrap();
            page = Sailren.on_first_talk(ctx, npc, ObjectId(1));
        });
        assert_eq!(page.as_deref(), Some("32109.html"));
        for e in [
            "32109-01.html",
            "32109-01a.html",
            "32109-02a.html",
            "32109-03a.html",
        ] {
            assert_eq!(event(&mut t, e, Some(1), Some(statue)).as_deref(), Some(e));
        }
    }

    #[test]
    fn the_entry_refuses_by_party_status_leader_and_gazkh() {
        let mut t = world();
        let statue = party_of_two(&mut t, false);
        // The leader has no Gazkh.
        let answer = {
            let mut out = None;
            t.world.run_as_script("Sailren", |ctx| {
                let npc = ctx.script_npc(statue);
                out = handle_enter_event(ctx, npc, Some(ObjectId(1)));
            });
            out
        };
        assert_eq!(answer.as_deref(), Some("32109-02.html"));
        let answer = {
            let mut out = None;
            t.world.run_as_script("Sailren", |ctx| {
                let npc = ctx.script_npc(statue);
                out = handle_enter_event(ctx, npc, Some(ObjectId(2)));
            });
            out
        };
        assert_eq!(answer.as_deref(), Some("32109-03.html"));

        // Not in a party at all.
        enter(&mut t, 3, true);
        let answer = {
            let mut out = None;
            t.world.run_as_script("Sailren", |ctx| {
                let npc = ctx.script_npc(statue);
                out = handle_enter_event(ctx, npc, Some(ObjectId(3)));
            });
            out
        };
        assert_eq!(answer.as_deref(), Some("32109-01.html"));

        // A raid in a fight or dead.
        st(&mut t, |s| s.raid_status = STATUS_IN_FIGHT);
        let mut out = None;
        t.world.run_as_script("Sailren", |ctx| {
            let npc = ctx.script_npc(statue);
            out = handle_enter_event(ctx, npc, Some(ObjectId(1)));
        });
        assert_eq!(out.as_deref(), Some("32109-05.html"));
        st(&mut t, |s| s.raid_status = STATUS_DEAD);
        let mut out = None;
        t.world.run_as_script("Sailren", |ctx| {
            let npc = ctx.script_npc(statue);
            out = handle_enter_event(ctx, npc, Some(ObjectId(1)));
        });
        assert_eq!(out.as_deref(), Some("32109-04.html"));
    }

    #[test]
    fn the_leader_with_a_gazkh_takes_the_party_in_and_the_timers_start() {
        let mut t = world();
        let statue = party_of_two(&mut t, true);
        event(&mut t, EVENT_ENTER, Some(1), Some(statue));
        t.ticks(2);
        assert_eq!(st(&mut t, |s| s.raid_status), STATUS_IN_FIGHT);
        assert_eq!(t.count(1, GAZKH_ITEM_ID), 0);
        t.ticks(3);
        t.input(1, l2_world::ClientInput::Appearing);
        t.input(2, l2_world::ClientInput::Appearing);
        for id in [1, 2] {
            let (x, y, _) = t.world.position_of(ObjectId(id)).unwrap();
            assert!((x - 27549).abs() < 400 && (y + 6638).abs() < 400, "{id}: {x},{y}");
        }
        assert!(has_timer(&mut t, EVENT_SPAWN_VELOCIRAPTORS));
        assert!(has_timer(&mut t, EVENT_TIMEOUT));
        assert!(has_timer(&mut t, EVENT_CHECK_INACTIVITY));
        // A second go is refused: the fight is on.
        let mut out = None;
        t.world.run_as_script("Sailren", |ctx| {
            let npc = ctx.script_npc(statue);
            out = handle_enter_event(ctx, npc, Some(ObjectId(1)));
        });
        assert_eq!(out.as_deref(), Some("32109-05.html"));
    }

    #[test]
    fn the_waves_follow_each_other_to_sailren_s_cinematic() {
        let mut t = world();
        enter(&mut t, 1, false);
        t.world.tele_to_location(
            ObjectId(1),
            l2_world::TeleportTarget::at(27549, -6638, -2008).with_instance(0),
        );
        t.input(1, l2_world::ClientInput::Appearing);
        // Three raptors, then the Pterosaur, then the T-Rex, then Sailren.
        event(&mut t, EVENT_SPAWN_VELOCIRAPTORS, None, None);
        let raptors = t.npcs_of(VELOCIRAPTOR_NPC_ID);
        assert_eq!(raptors.len(), 3);
        for raptor in &raptors {
            let (x, y, _) = t.world.position_of(*raptor).unwrap();
            assert!((27313..27313 + 150).contains(&x) && (-6766..-6766 + 150).contains(&y));
        }
        for raptor in raptors {
            t.kill(1, raptor);
        }
        let pterosaur = t.npcs_of(PTEROSAUR_NPC_ID);
        assert_eq!(pterosaur.len(), 1);
        assert_eq!(st(&mut t, |s| s.kill_count), 0);
        t.kill(1, pterosaur[0]);
        let trex = t.npcs_of(TREX_NPC_ID);
        assert_eq!(trex.len(), 1);
        t.kill(1, trex[0]);
        assert!(has_timer(&mut t, EVENT_SPAWN_SAILREN));

        event(&mut t, EVENT_SPAWN_SAILREN, None, None);
        let sailren = t.npcs_of(SAILREN_NPC_ID);
        assert_eq!(sailren.len(), 1);
        assert_eq!(t.npcs_of(MOVIE_NPC_ID).len(), 1);
        assert!(t.world.is_invul(sailren[0]));
        let movie = t.npcs_of(MOVIE_NPC_ID)[0];
        t.world.run_as_script("Sailren", |ctx| {
            let movie = ctx.script_npc(movie).unwrap();
            assert!(ctx.has_quest_timer(EVENT_CAMERA_STEP_ONE, Some(movie), None));
            assert!(ctx.has_quest_timer(EVENT_ANIMATION, Some(movie), None));
        });
        // The cinematic ends and he can be hurt.
        t.wait_ms(24_700);
        assert!(!t.world.is_invul(sailren[0]));
        // The camera NPC goes away.
        t.wait_ms(2_000);
        assert!(t.npcs_of(MOVIE_NPC_ID).is_empty());
    }

    #[test]
    fn sailren_s_death_stores_the_respawn_and_opens_the_cube() {
        let mut t = world();
        enter(&mut t, 1, false);
        t.world.tele_to_location(
            ObjectId(1),
            l2_world::TeleportTarget::at(27549, -6638, -2008).with_instance(0),
        );
        t.input(1, l2_world::ClientInput::Appearing);
        event(&mut t, EVENT_SPAWN_SAILREN, None, None);
        let sailren = t.npcs_of(SAILREN_NPC_ID)[0];
        t.world.set_invul(sailren, false);
        let before = now_ms();
        t.kill(1, sailren);
        assert_eq!(st(&mut t, |s| s.raid_status), STATUS_DEAD);
        let stored = t.world.global_variables().get_long(GLOBAL_VAR_RESPAWN_TIME, 0);
        assert!(stored >= before + RESPAWN_DELAY_MILLIS && stored <= now_ms() + RESPAWN_DELAY_MILLIS);
        assert_eq!(t.npcs_of(TELEPORT_CUBE_NPC_ID).len(), 1);
        assert!(has_timer(&mut t, EVENT_CLEAR_STATUS));
        // The timeout five minutes later sends the player to town and removes the NPCs.
        assert!(has_timer(&mut t, EVENT_TIMEOUT));
        event(&mut t, EVENT_TIMEOUT, None, None);
        assert!(t.npcs_of(TELEPORT_CUBE_NPC_ID).is_empty());
        // The clear brings the lair back.
        event(&mut t, EVENT_CLEAR_STATUS, None, None);
        assert_eq!(st(&mut t, |s| s.raid_status), STATUS_ALIVE);
    }

    #[test]
    fn the_inactivity_check_ends_a_fight_nobody_attacks() {
        let mut t = world();
        enter(&mut t, 1, false);
        t.world.tele_to_location(
            ObjectId(1),
            l2_world::TeleportTarget::at(27549, -6638, -2008).with_instance(0),
        );
        t.input(1, l2_world::ClientInput::Appearing);
        st(&mut t, |s| {
            s.raid_status = STATUS_IN_FIGHT;
            s.last_attack_time = now_ms();
        });
        // A recent attack: the check comes again.
        event(&mut t, EVENT_CHECK_INACTIVITY, None, None);
        assert!(has_timer(&mut t, EVENT_CHECK_INACTIVITY));
        // Ten minutes without one: the timeout runs.
        st(&mut t, |s| {
            s.last_attack_time = now_ms() - INACTIVITY_LIMIT_MILLIS - 1000
        });
        event(&mut t, EVENT_CHECK_INACTIVITY, None, None);
        assert_eq!(st(&mut t, |s| s.raid_status), STATUS_ALIVE);
        assert!(!has_timer(&mut t, EVENT_CHECK_INACTIVITY));
    }

    #[test]
    fn an_attack_inside_the_zone_refreshes_the_clock() {
        let mut t = world();
        enter(&mut t, 1, false);
        t.world.tele_to_location(
            ObjectId(1),
            l2_world::TeleportTarget::at(27549, -6638, -2008).with_instance(0),
        );
        t.input(1, l2_world::ClientInput::Appearing);
        let raptor = t.spawn(VELOCIRAPTOR_NPC_ID, 10);
        t.world.run_as_script("Sailren", |ctx| {
            let npc = ctx.script_npc(raptor).unwrap();
            Sailren.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        assert!(st(&mut t, |s| s.last_attack_time) > 0);
    }

    #[test]
    fn a_future_respawn_row_marks_the_raid_dead_at_start() {
        let mut t = world_with(
            true,
            true,
            vec![(
                GLOBAL_VAR_RESPAWN_TIME.to_owned(),
                Some((now_ms() + 600_000).to_string()),
            )],
        );
        assert_eq!(st(&mut t, |s| s.raid_status), STATUS_DEAD);
        assert!(has_timer(&mut t, EVENT_CLEAR_STATUS));
        let mut t = world_with(
            true,
            true,
            vec![(
                GLOBAL_VAR_RESPAWN_TIME.to_owned(),
                Some((now_ms() - 600_000).to_string()),
            )],
        );
        assert_eq!(st(&mut t, |s| s.raid_status), STATUS_ALIVE);
    }

    #[test]
    fn with_the_switch_off_the_entry_reads_the_row_first() {
        let mut t = world_with(true, false, vec![]);
        let statue = party_of_two(&mut t, true);
        event(&mut t, EVENT_ENTER, Some(1), Some(statue));
        assert_eq!(t.world.script_rereads_waiting(), 1);
        assert_eq!(st(&mut t, |s| s.raid_status), STATUS_ALIVE);
        // The Java GS killed Sailren a moment ago: the row says so.
        t.world.handle(WorldCommand::GlobalVariablesReread {
            token: 1,
            result: Ok(vec![(
                GLOBAL_VAR_RESPAWN_TIME.to_owned(),
                Some((now_ms() + 3_000_000).to_string()),
            )]),
        });
        assert_eq!(st(&mut t, |s| s.raid_status), STATUS_DEAD);
        assert_eq!(t.count(1, GAZKH_ITEM_ID), 1);
        assert!(has_timer(&mut t, EVENT_CLEAR_STATUS));
    }
}
