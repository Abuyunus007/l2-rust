//! Monastery of Silence (instance 151), the Seven Signs quests 10294 to 10296.
// Java: dist/game/data/scripts/instances/SSQMonasteryOfSilence/SSQMonasteryOfSilence.java

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

type Loc = (i32, i32, i32);

// NPCs
const ELCADIA_INSTANCE: i32 = 32787;
const ERIS_EVIL_THOUGHTS: i32 = 32792;
const RELIC_GUARDIAN: i32 = 32803;
const RELIC_WATCHER1: i32 = 32804;
const RELIC_WATCHER2: i32 = 32805;
const RELIC_WATCHER3: i32 = 32806;
const RELIC_WATCHER4: i32 = 32807;
const ODD_GLOBE: i32 = 32815;
const TELEPORT_CONTROL_DEVICE1: i32 = 32817;
const TELEPORT_CONTROL_DEVICE2: i32 = 32818;
const TELEPORT_CONTROL_DEVICE3: i32 = 32819;
const TELEPORT_CONTROL_DEVICE4: i32 = 32820;
const TOMB_OF_THE_SAINTESS: i32 = 32843;

// Monsters
const TRAINEE_OF_REST: i32 = 27403;
const SUPPLICANT_OF_REST: i32 = 27404;
const ETIS_VAN_ETINA: i32 = 18949;
const SOLINAS_GUARDIAN_1: i32 = 18952;
const SOLINAS_GUARDIAN_2: i32 = 18953;
const SOLINAS_GUARDIAN_3: i32 = 18954;
const SOLINAS_GUARDIAN_4: i32 = 18955;
const GUARDIAN_OF_THE_TOMB_1: i32 = 18956;
const GUARDIAN_OF_THE_TOMB_2: i32 = 18957;
const GUARDIAN_OF_THE_TOMB_3: i32 = 18958;
const GUARDIAN_OF_THE_TOMB_4: i32 = 18959;

// Items
const SCROLL_OF_ABSTINENCE: i32 = 17228;
const SHIELD_OF_SACRIFICE: i32 = 17229;
const SWORD_OF_HOLY_SPIRIT: i32 = 17230;
const STAFF_OF_BLESSING: i32 = 17231;

// Skills
const BUFFS: [SkillKey; 3] = [
    SkillKey { id: 6725, level: 1 }, // Bless the Blood of Elcadia
    SkillKey { id: 6728, level: 1 }, // Recharge of Elcadia
    SkillKey { id: 6730, level: 1 }, // Greater Battle Heal of Elcadia
];

// Locations
const START_LOC: Loc = (120717, -86879, -3424);
const EXIT_LOC: Loc = (115983, -87351, -3397);
const CENTRAL_ROOM_LOC: Loc = (85794, -249788, -8320);
const SOUTH_WATCHERS_ROOM_LOC: Loc = (85798, -246566, -8320);
const WEST_WATCHERS_ROOM_LOC: Loc = (82531, -249405, -8320);
const EAST_WATCHERS_ROOM_LOC: Loc = (88665, -249784, -8320);
const NORTH_WATCHERS_ROOM_LOC: Loc = (85792, -252336, -8320);
const BACK_LOC: Loc = (120710, -86971, -3392);
const START_LOC_Q10295: Loc = (45545, -249423, -6788);
const CASKET_ROOM_LOC: Loc = (56033, -252944, -6792);
const SOLINAS_RESTING_PLACE_LOC: Loc = (55955, -250394, -6792);
const DIRECTORS_ROOM_LOC: Loc = (120717, -86879, -3424);
/// `(x, y, z, heading)`.
const GUARDIAN_OF_THE_TOMB_1_LOC: (i32, i32, i32, i32) = (55498, -252781, -6752, 0);
const GUARDIAN_OF_THE_TOMB_2_LOC: (i32, i32, i32, i32) = (55520, -252160, -6752, 0);
const GUARDIAN_OF_THE_TOMB_3_LOC: (i32, i32, i32, i32) = (56635, -252776, -6752, -32180);
const GUARDIAN_OF_THE_TOMB_4_LOC: (i32, i32, i32, i32) = (56672, -252156, -6754, 32252);
const SOLINAS_GUARDIAN_1_LOC: (i32, i32, i32, i32) = (45399, -253051, -6765, 16584);
const SOLINAS_GUARDIAN_2_LOC: (i32, i32, i32, i32) = (48736, -249632, -6768, -32628);
const SOLINAS_GUARDIAN_3_LOC: (i32, i32, i32, i32) = (45392, -246303, -6768, -16268);
const SOLINAS_GUARDIAN_4_LOC: (i32, i32, i32, i32) = (42016, -249648, -6764, 0);
const ELCADIA_LOC: Loc = (115927, -87005, -3392);
const SPACE_LOC: Loc = (76736, -241021, -10780);
const ETIS_VAN_ETINA_LOC: (i32, i32, i32, i32) = (76625, -240824, -10832, 0);
const SLAVE_SPAWN_1_LOC: [Loc; 4] = [
    (55680, -252832, -6752),
    (55825, -252792, -6752),
    (55687, -252718, -6752),
    (55824, -252679, -6752),
];
const SLAVE_SPAWN_2_LOC: [Loc; 4] = [
    (55672, -252099, -6751),
    (55810, -252262, -6752),
    (55824, -252112, -6752),
    (55669, -252227, -6752),
];
const SLAVE_SPAWN_3_LOC: [Loc; 4] = [
    (56480, -252833, -6751),
    (56481, -252725, -6752),
    (56368, -252787, -6752),
    (56368, -252669, -6752),
];
const SLAVE_SPAWN_4_LOC: [Loc; 4] = [
    (56463, -252225, -6751),
    (56469, -252108, -6752),
    (56336, -252168, -6752),
    (56336, -252288, -6752),
];

// NpcString
const ELCADIA_DIALOGS_Q010294: [NpcStringId; 3] = [
    NpcStringId::WeMustSearchHighAndLowInEveryRoomForTheReadingDeskThatContainsTheBookWeSeek,
    NpcStringId::RememberTheContentOfTheBooksThatYouFoundYouCanTTakeThemOutWithYou,
    NpcStringId::ItSeemsThatYouCannotRememberToTheRoomOfTheWatcherWhoFoundTheBook,
];
const ELCADIA_DIALOGS_Q010295: [NpcStringId; 3] = [
    NpcStringId::TheGuardianOfTheSealDoesnTSeemToGetInjuredAtAllUntilTheBarrierIsDestroyed,
    NpcStringId::TheDeviceLocatedInTheRoomInFrontOfTheGuardianOfTheSealIsDefinitelyTheBarrierThatControlsTheGuardianSPower,
    NpcStringId::ToRemoveTheBarrierYouMustFindTheRelicsThatFitTheBarrierAndActivateTheDevice,
];

// Misc
const TEMPLATE_ID: i32 = 151;

// Doors
const TOMB_DOOR: i32 = 21100018;
const DOORS: [i32; 16] = [
    21100014, 21100001, 21100006, 21100010, 21100003, 21100008, 21100012, 21100016, 21100002, 21100015,
    21100005, 21100004, 21100009, 21100007, 21100013, 21100011,
];
const FAKE_TOMB_DOORS: [i32; 4] = [21100101, 21100102, 21100103, 21100104];

// Movie ids (`Movie.SSQ2_*`).
const SSQ2_HOLY_BURIAL_GROUND_OPENING: i32 = 24;
const SSQ2_SOLINA_TOMB_OPENING: i32 = 26;
const SSQ2_SOLINA_TOMB_CLOSING: i32 = 27;
const SSQ2_BOSS_OPENING: i32 = 29;
const SSQ2_BOSS_CLOSING: i32 = 30;

// Quests it reads.
const Q10294: &str = "Q10294_SevenSignsToTheMonasteryOfSilence";
const Q10295: &str = "Q10295_SevenSignsSolinasTomb";
const Q10296: &str = "Q10296_SevenSignsOneWhoSeeksThePowerOfTheSeal";

/// What a Java NPC object keeps after it left the world: the instance it was in. The killed
/// Etis van Etina is still the NPC of a timer 63 seconds later (`getWorld(npc)`), long after it
/// decayed.
#[derive(Debug, Default)]
struct State {
    npc_instance: HashMap<ObjectId, i32>,
}

/// Java `instances.SSQMonasteryOfSilence.SSQMonasteryOfSilence`.
pub struct SSQMonasteryOfSilence;

impl Script for SSQMonasteryOfSilence {
    fn name(&self) -> &'static str {
        "SSQMonasteryOfSilence"
    }

    fn path(&self) -> &'static str {
        "instances/SSQMonasteryOfSilence"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[
            TELEPORT_CONTROL_DEVICE1,
            TELEPORT_CONTROL_DEVICE2,
            TELEPORT_CONTROL_DEVICE3,
            TELEPORT_CONTROL_DEVICE4,
        ]);
        s.add_kill_id(&[
            SOLINAS_GUARDIAN_1,
            SOLINAS_GUARDIAN_2,
            SOLINAS_GUARDIAN_3,
            SOLINAS_GUARDIAN_4,
            GUARDIAN_OF_THE_TOMB_1,
            GUARDIAN_OF_THE_TOMB_2,
            GUARDIAN_OF_THE_TOMB_3,
            GUARDIAN_OF_THE_TOMB_4,
            ETIS_VAN_ETINA,
        ]);
        s.add_spawn_id(&[ERIS_EVIL_THOUGHTS, TOMB_OF_THE_SAINTESS]);
        s.add_start_npc(&[
            ODD_GLOBE,
            TELEPORT_CONTROL_DEVICE1,
            TELEPORT_CONTROL_DEVICE2,
            TELEPORT_CONTROL_DEVICE3,
            TELEPORT_CONTROL_DEVICE4,
            ERIS_EVIL_THOUGHTS,
        ]);
        s.add_talk_id(&[
            ODD_GLOBE,
            ERIS_EVIL_THOUGHTS,
            RELIC_GUARDIAN,
            RELIC_WATCHER1,
            RELIC_WATCHER2,
            RELIC_WATCHER3,
            RELIC_WATCHER4,
            TELEPORT_CONTROL_DEVICE1,
            TELEPORT_CONTROL_DEVICE2,
            TELEPORT_CONTROL_DEVICE3,
            TELEPORT_CONTROL_DEVICE4,
            ERIS_EVIL_THOUGHTS,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // `InstanceManager.getWorld(npc)`: a null NPC is a NullPointerException.
        let npc = npc?;
        let instance_id = npc_instance(ctx, npc)?;
        match event {
            "TELE2" => {
                let player = player?;
                move_both(ctx, player, instance_id, CENTRAL_ROOM_LOC, true)?;
                ctx.start_quest_timer("START_MOVIE", 2000, Some(npc), Some(player), false);
            }
            "EXIT" => {
                let player = player?;
                ctx.cancel_quest_timer("FOLLOW", Some(npc), Some(player));
                ctx.cancel_quest_timer("DIALOG", Some(npc), Some(player));
                ctx.teleport_player(player, EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2, 0, true);
                let elcadia = elcadia(ctx, instance_id)?;
                ctx.delete_npc(elcadia);
            }
            "START_MOVIE" => {
                ctx.play_movie(player?, SSQ2_HOLY_BURIAL_GROUND_OPENING);
            }
            "BACK" => {
                move_both(ctx, player?, instance_id, BACK_LOC, true)?;
            }
            "EAST" => {
                move_both(ctx, player?, instance_id, EAST_WATCHERS_ROOM_LOC, true)?;
            }
            "WEST" => {
                move_both(ctx, player?, instance_id, WEST_WATCHERS_ROOM_LOC, true)?;
            }
            "NORTH" => {
                move_both(ctx, player?, instance_id, NORTH_WATCHERS_ROOM_LOC, true)?;
            }
            "SOUTH" => {
                move_both(ctx, player?, instance_id, SOUTH_WATCHERS_ROOM_LOC, true)?;
            }
            "CENTER" => {
                move_both(ctx, player?, instance_id, CENTRAL_ROOM_LOC, true)?;
            }
            "FOLLOW" => {
                let player = player?;
                // NOTE(java-quirk): `EXIT` cancels this timer with the NPC that was clicked, not
                // Elcadia, so it never stops in Java and goes on with a deleted Elcadia. Here it
                // ends when the NPC is gone: a deleted NPC does nothing visible.
                if !alive(ctx, npc) {
                    return None;
                }
                ctx.set_running(npc);
                ctx.world_mut().script_start_follow(npc.object_id, player);
                if ctx.world().is_in_combat(player)
                    && let Some(skill) = ctx.get_random_entry(&BUFFS)
                {
                    ctx.world_mut().do_cast(npc.object_id, skill);
                }
                ctx.start_quest_timer("FOLLOW", 5000, Some(npc), Some(player), false);
            }
            "DIALOG" => {
                let player = player?;
                // NOTE(java-quirk): see FOLLOW.
                if !alive(ctx, npc) {
                    return None;
                }
                let qs10294 = ctx
                    .player_quest_state(player, Q10294)
                    .is_some_and(|q| q.is_started());
                let qs10295 = ctx
                    .player_quest_state(player, Q10295)
                    .is_some_and(|q| q.is_memo_state(1));
                if qs10294 && let Some(text) = ctx.get_random_entry(&ELCADIA_DIALOGS_Q010294) {
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
                }
                if qs10295 && let Some(text) = ctx.get_random_entry(&ELCADIA_DIALOGS_Q010295) {
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
                }
                ctx.start_quest_timer("DIALOG", 10000, Some(npc), Some(player), false);
            }
            "ENTER_Q10295" => {
                let player = player?;
                move_both(ctx, player, instance_id, START_LOC_Q10295, true)?;
                ctx.start_quest_timer("START_MOVIE_Q10295", 2000, Some(npc), Some(player), false);
            }
            "START_MOVIE_Q10295" => {
                ctx.play_movie(player?, SSQ2_SOLINA_TOMB_OPENING);
            }
            "CASKET_ROOM" => {
                move_both(ctx, player?, instance_id, CASKET_ROOM_LOC, true)?;
            }
            "SOLINAS_RESTING_PLACE" => {
                move_both(ctx, player?, instance_id, SOLINAS_RESTING_PLACE_LOC, true)?;
            }
            "ERIS_OFFICE" => {
                move_both(ctx, player?, instance_id, START_LOC, true)?;
            }
            "OPEN_DOORS" => {
                for door_id in DOORS {
                    ctx.world_mut().instance_open_door(instance_id, door_id);
                }
            }
            "DIRECTORS_ROOM" => {
                move_both(ctx, player?, instance_id, DIRECTORS_ROOM_LOC, true)?;
            }
            // TODO(java): "Missing area debuff" in the four USE_ events.
            "USE_SCROLL" => {
                use_relic(
                    ctx,
                    player?,
                    instance_id,
                    SCROLL_OF_ABSTINENCE,
                    SOLINAS_GUARDIAN_1,
                    SOLINAS_GUARDIAN_1_LOC,
                );
            }
            "USE_SHIELD" => {
                use_relic(
                    ctx,
                    player?,
                    instance_id,
                    SHIELD_OF_SACRIFICE,
                    SOLINAS_GUARDIAN_2,
                    SOLINAS_GUARDIAN_2_LOC,
                );
            }
            "USE_SWORD" => {
                use_relic(
                    ctx,
                    player?,
                    instance_id,
                    SWORD_OF_HOLY_SPIRIT,
                    SOLINAS_GUARDIAN_3,
                    SOLINAS_GUARDIAN_3_LOC,
                );
            }
            "USE_STAFF" => {
                use_relic(
                    ctx,
                    player?,
                    instance_id,
                    STAFF_OF_BLESSING,
                    SOLINAS_GUARDIAN_4,
                    SOLINAS_GUARDIAN_4_LOC,
                );
            }
            "CLOSE_TOMB_DOORS" => {
                for door_id in FAKE_TOMB_DOORS {
                    ctx.world_mut().instance_close_door(instance_id, door_id);
                }
            }
            "TOMB_GUARDIAN_SPAWN" => {
                let player = player?;
                for door_id in FAKE_TOMB_DOORS {
                    ctx.world_mut().instance_open_door(instance_id, door_id);
                }

                spawn_at(
                    ctx,
                    GUARDIAN_OF_THE_TOMB_1,
                    GUARDIAN_OF_THE_TOMB_1_LOC,
                    instance_id,
                );
                for loc in SLAVE_SPAWN_1_LOC {
                    let slave = spawn_at(ctx, TRAINEE_OF_REST, (loc.0, loc.1, loc.2, 0), instance_id);
                    send_slave_at_player(ctx, slave, player);
                }

                spawn_at(
                    ctx,
                    GUARDIAN_OF_THE_TOMB_2,
                    GUARDIAN_OF_THE_TOMB_2_LOC,
                    instance_id,
                );
                for loc in SLAVE_SPAWN_2_LOC {
                    let slave = spawn_at(ctx, TRAINEE_OF_REST, (loc.0, loc.1, loc.2, 0), instance_id);
                    send_slave_at_player(ctx, slave, player);
                }

                spawn_at(
                    ctx,
                    GUARDIAN_OF_THE_TOMB_3,
                    GUARDIAN_OF_THE_TOMB_3_LOC,
                    instance_id,
                );
                for loc in SLAVE_SPAWN_3_LOC {
                    let slave = spawn_at(ctx, SUPPLICANT_OF_REST, (loc.0, loc.1, loc.2, 0), instance_id);
                    send_slave_at_player(ctx, slave, player);
                }

                spawn_at(
                    ctx,
                    GUARDIAN_OF_THE_TOMB_4,
                    GUARDIAN_OF_THE_TOMB_4_LOC,
                    instance_id,
                );
                for loc in SLAVE_SPAWN_4_LOC {
                    let slave = spawn_at(ctx, SUPPLICANT_OF_REST, (loc.0, loc.1, loc.2, 0), instance_id);
                    send_slave_at_player(ctx, slave, player);
                }

                return Some("32843-01.html".to_owned());
            }
            "START_MOVIE_Q10296" => {
                let player = player?;
                ctx.play_movie(player, SSQ2_BOSS_OPENING);
                ctx.start_quest_timer("TELEPORT_SPACE", 60000, Some(npc), Some(player), false);
                let elcadia = elcadia(ctx, instance_id)?;
                tele_npc(ctx, elcadia, ELCADIA_LOC, instance_id);
            }
            "TELEPORT_SPACE" => {
                let player = player?;
                move_both(ctx, player, instance_id, SPACE_LOC, true)?;
                spawn_at(ctx, ETIS_VAN_ETINA, ETIS_VAN_ETINA_LOC, instance_id);
            }
            "TELEPORT_TO_PLAYER" => {
                let player = player?;
                let elcadia = elcadia(ctx, instance_id)?;
                if let Some((x, y, z)) = ctx.world().position_of(player) {
                    tele_npc(ctx, elcadia, (x, y, z), instance_id);
                }
            }
            _ => {}
        }
        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(instance_id) = npc_instance(ctx, npc) else {
            return;
        };
        match npc.id {
            GUARDIAN_OF_THE_TOMB_1
            | GUARDIAN_OF_THE_TOMB_2
            | GUARDIAN_OF_THE_TOMB_3
            | GUARDIAN_OF_THE_TOMB_4 => {
                let dead = param_int(ctx, instance_id, "deadTombGuardianCount") + 1;
                set_param_int(ctx, instance_id, "deadTombGuardianCount", dead);
                if dead == 4 {
                    ctx.world_mut().instance_open_door(instance_id, TOMB_DOOR);
                    if ctx
                        .player_quest_state(killer, Q10295)
                        .is_some_and(|q| q.is_memo_state(2))
                        && let Some(qs) = ctx.quest_state_of(Q10295, killer, false)
                    {
                        qs.set_memo_state(ctx, 3);
                    }
                }
            }
            SOLINAS_GUARDIAN_1 | SOLINAS_GUARDIAN_2 | SOLINAS_GUARDIAN_3 | SOLINAS_GUARDIAN_4 => {
                let dead = param_int(ctx, instance_id, "deadSolinaGuardianCount") + 1;
                set_param_int(ctx, instance_id, "deadSolinaGuardianCount", dead);
                if dead == 4 {
                    ctx.play_movie(killer, SSQ2_SOLINA_TOMB_CLOSING);
                    if ctx
                        .player_quest_state(killer, Q10295)
                        .is_some_and(|q| q.is_memo_state(1))
                        && let Some(qs) = ctx.quest_state_of(Q10295, killer, false)
                    {
                        qs.set_memo_state(ctx, 2);
                    }
                }
            }
            ETIS_VAN_ETINA => {
                ctx.play_movie(killer, SSQ2_BOSS_CLOSING);
                // A world without Elcadia is a NullPointerException: the rest is skipped.
                let Some(elcadia) = elcadia(ctx, instance_id) else {
                    return;
                };
                tele_npc(ctx, elcadia, ELCADIA_LOC, instance_id);
                // The NPC of the timer: it decays long before the timer fires.
                ctx.state::<State>()
                    .npc_instance
                    .insert(npc.object_id, instance_id);
                ctx.start_quest_timer("TELEPORT_TO_PLAYER", 63000, Some(npc), Some(killer), false);
                if ctx
                    .player_quest_state(killer, Q10296)
                    .is_some_and(|q| q.is_memo_state(2))
                    && let Some(qs) = ctx.quest_state_of(Q10296, killer, false)
                {
                    qs.set_memo_state(ctx, 3);
                }
            }
            _ => {}
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        match npc.id {
            ERIS_EVIL_THOUGHTS => {
                ctx.start_quest_timer("OPEN_DOORS", 1000, Some(npc), None, false);
            }
            TOMB_OF_THE_SAINTESS => {
                ctx.start_quest_timer("CLOSE_TOMB_DOORS", 1000, Some(npc), None, false);
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        if npc.id == ODD_GLOBE {
            enter_instance(ctx, talker);
        }
        // `super.onTalk(npc, talker)`.
        None
    }
}

/// `InstanceManager.getWorld(npc)`: the instance of the NPC, with a script world.
fn npc_instance(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) -> Option<i32> {
    let mut instance_id = ctx.world().instance_id_of(npc.object_id);
    if instance_id == 0 {
        instance_id = ctx
            .state::<State>()
            .npc_instance
            .get(&npc.object_id)
            .copied()
            .unwrap_or(0);
    }
    ctx.world().instance_world(instance_id).map(|_| instance_id)
}

fn alive(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    ctx.world().npc(npc.object_id).is_some_and(|n| !n.decayed)
}

/// `world.getParameters().getObject("elcadia", Npc.class)`.
fn elcadia(ctx: &ScriptCtx<'_>, instance_id: i32) -> Option<ScriptNpc> {
    let id = ctx
        .world()
        .instance_world(instance_id)?
        .parameters
        .get("elcadia")?
        .parse()
        .ok()?;
    Some(ScriptNpc {
        object_id: ObjectId(id),
        id: ELCADIA_INSTANCE,
    })
}

/// `world.getParameters().getInt(name, 0)`.
fn param_int(ctx: &ScriptCtx<'_>, instance_id: i32, name: &str) -> i32 {
    ctx.world()
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get(name))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// `world.setParameter(name, value)`.
fn set_param_int(ctx: &mut ScriptCtx<'_>, instance_id: i32, name: &str, value: i32) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.parameters.insert(name.to_owned(), value.to_string());
    }
}

/// `elcadia.teleToLocation(x, y, z, 0, instanceId)`.
fn tele_npc(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, (x, y, z): Loc, instance_id: i32) {
    ctx.world_mut().tele_to_location(
        npc.object_id,
        TeleportTarget::at(x, y, z).with_instance(instance_id),
    );
}

/// `teleportPlayer(player, loc, id)` and `elcadia.teleToLocation(loc, id, 0)`. `None` when the
/// world has no Elcadia (a NullPointerException in Java, after the player was sent).
fn move_both(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    instance_id: i32,
    loc: Loc,
    offset: bool,
) -> Option<()> {
    ctx.teleport_player(player, loc.0, loc.1, loc.2, instance_id, offset);
    let elcadia = elcadia(ctx, instance_id)?;
    tele_npc(ctx, elcadia, loc, instance_id);
    Some(())
}

/// The four `USE_*` events: the relic is used up and its guardian appears.
fn use_relic(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    instance_id: i32,
    item: i32,
    guardian: i32,
    loc: (i32, i32, i32, i32),
) {
    if ctx.has_quest_items(player, item) {
        ctx.take_items(player, item, 1);
        spawn_at(ctx, guardian, loc, instance_id);
    }
}

/// `addSpawn(npcId, location, false, 0, false, instanceId)`.
fn spawn_at(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    (x, y, z, heading): (i32, i32, i32, i32),
    instance_id: i32,
) -> Option<ScriptNpc> {
    ctx.add_spawn(npc_id, x, y, z, heading, false, 0, instance_id)
}

/// Sends a freshly spawned tomb slave at the player. The template of a slave can be a Folk,
/// which is not attackable, so the aggression is only applied when it is.
fn send_slave_at_player(ctx: &mut ScriptCtx<'_>, slave: Option<ScriptNpc>, player: ObjectId) {
    let Some(slave) = slave else {
        return;
    };
    if ctx.world().is_attackable_npc(slave.object_id) {
        ctx.add_attack_desire(slave, player, 999);
    }
}

/// Java `enterInstance(talker, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let mut entered = None;
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance && let Some(instance_world) = world.instance_world_mut(instance_id) {
                instance_world.add_allowed(player);
            }
            // `teleportPlayer(player, START_LOC, id, false)`.
            world.tele_to_location(
                player,
                TeleportTarget::at(START_LOC.0, START_LOC.1, START_LOC.2).with_instance(instance_id),
            );
            entered = Some(instance_id);
        },
    );
    if let Some(instance_id) = entered {
        spawn_elcadia(ctx, player, instance_id);
    }
}

/// Java `spawnElcadia(player, world)`.
fn spawn_elcadia(ctx: &mut ScriptCtx<'_>, player: ObjectId, instance_id: i32) {
    if let Some(old) = elcadia(ctx, instance_id) {
        ctx.delete_npc(old);
    }
    let Some((x, y, z)) = ctx.world().position_of(player) else {
        return;
    };
    let Some(elcadia) = ctx.add_spawn(ELCADIA_INSTANCE, x, y, z, 0, false, 0, instance_id) else {
        return;
    };
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world
            .parameters
            .insert("elcadia".to_owned(), elcadia.object_id.0.to_string());
    }
    ctx.start_quest_timer("FOLLOW", 5000, Some(elcadia), Some(player), false);
    ctx.start_quest_timer("DIALOG", 10000, Some(elcadia), Some(player), false);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use l2_data::DoorTemplates;
    use l2_util::ObjectId as Oid;
    use l2_world::script::STARTED;
    use l2_world::{IdFactory, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32787" type="Folk" level="80" name="Elcadia"/>
        <npc id="32792" type="Folk" level="80" name="Eris Evil Thoughts"/>
        <npc id="32815" type="Folk" level="80" name="Odd Globe"/>
        <npc id="32843" type="Folk" level="80" name="Tomb of the Saintess"/>
        <npc id="27403" type="Monster" level="80" name="Trainee of Rest"/>
        <npc id="27404" type="Folk" level="80" name="Supplicant of Rest"/>
        <npc id="18949" type="Monster" level="80" name="Etis van Etina"/>
        <npc id="18952" type="Monster" level="80" name="Solina's Guardian 1"/>
        <npc id="18953" type="Monster" level="80" name="Solina's Guardian 2"/>
        <npc id="18954" type="Monster" level="80" name="Solina's Guardian 3"/>
        <npc id="18955" type="Monster" level="80" name="Solina's Guardian 4"/>
        <npc id="18956" type="Monster" level="80" name="Guardian of the Tomb 1"/>
        <npc id="18957" type="Monster" level="80" name="Guardian of the Tomb 2"/>
        <npc id="18958" type="Monster" level="80" name="Guardian of the Tomb 3"/>
        <npc id="18959" type="Monster" level="80" name="Guardian of the Tomb 4"/>"#;
    const ITEMS: &str = r#"
        <item id="17228" type="EtcItem" name="Scroll of Abstinence"><set name="is_questitem" val="true"/></item>"#;

    /// A stand-in for a quest this script reads or writes: only its name matters.
    struct Quest(&'static str);

    impl Script for Quest {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Standin"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn all_doors() -> Vec<i32> {
        DOORS
            .iter()
            .chain(&FAKE_TOMB_DOORS)
            .chain(&[TOMB_DOOR])
            .copied()
            .collect()
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        for name in [Q10294, Q10295, Q10296] {
            registry.add(Quest(name));
        }
        registry.add(SSQMonasteryOfSilence);
        let pages = Pages::of(&SSQMonasteryOfSilence)
            .plain(&["32843-01.html"])
            .npcs(&[]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        // The doors of the normal world that the instance template copies.
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        t.world.set_id_factory(Arc::clone(&ids));
        let doors: String = all_doors()
            .iter()
            .map(|id| {
                format!(
                    r#"<door id="{id}" name="door{id}" height="150" baseHpMax="1000" pos="0,0,0"
                    node1="0,0" node2="0,10" node3="10,10" node4="10,0" nodeZ="0"/>"#
                )
            })
            .collect();
        let templates = DoorTemplates::from_xml(&format!("<list>{doors}</list>")).unwrap();
        t.world.spawn_doors(&templates, &ids).unwrap();
        let listed: String = all_doors()
            .iter()
            .map(|id| format!(r#"<door id="{id}" />"#))
            .collect();
        let template =
            format!(r#"<exitPoint x="115983" y="-87351" z="-3397" /><doorlist>{listed}</doorlist>"#);
        t.world.set_instance_data(instance_data(
            &[(151, "Monastery of Silence")],
            &[(151, &template)],
        ));
        let globe = t.spawn(ODD_GLOBE, 20);
        (t, globe)
    }

    fn set_memo(t: &mut ScriptTest, quest: &'static str, memo: i32) {
        t.world.run_as_script(quest, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, STARTED);
            qs.set_memo_state(ctx, memo);
        });
        t.take_writes();
    }

    fn memo(t: &mut ScriptTest, quest: &'static str) -> i32 {
        let mut memo = -1;
        t.world.run_as_script(quest, |ctx| {
            memo = ctx
                .player_quest_state(Oid(1), quest)
                .map_or(-1, |q| q.memo_state());
        });
        memo
    }

    /// Player 1 talks to the globe and is inside with Elcadia.
    fn inside(t: &mut ScriptTest, globe: Oid) -> i32 {
        t.enter(1, 80, vec![]);
        t.world.run_as_script("SSQMonasteryOfSilence", |ctx| {
            let npc = ScriptNpc {
                object_id: globe,
                id: ODD_GLOBE,
            };
            assert_eq!(SSQMonasteryOfSilence.on_talk(ctx, npc, Oid(1)), None);
        });
        t.ticks(60);
        t.world.instance_id_of(Oid(1))
    }

    /// An NPC of the instance (a clicked one, the killed one).
    fn spawn_in(t: &mut ScriptTest, instance_id: i32, npc_id: i32) -> ScriptNpc {
        let object_id = t
            .world
            .script_add_spawn(
                npc_id,
                (START_LOC.0, START_LOC.1, START_LOC.2),
                0,
                false,
                0,
                instance_id,
            )
            .unwrap();
        ScriptNpc {
            object_id,
            id: npc_id,
        }
    }

    fn event(t: &mut ScriptTest, name: &str, npc: ScriptNpc) -> Option<String> {
        let mut result = None;
        t.world.run_as_script("SSQMonasteryOfSilence", |ctx| {
            result = SSQMonasteryOfSilence.on_event(ctx, name, Some(npc), Some(Oid(1)));
        });
        t.ticks(60);
        result
    }

    fn kill(t: &mut ScriptTest, npc: ScriptNpc) {
        t.world.run_as_script("SSQMonasteryOfSilence", |ctx| {
            SSQMonasteryOfSilence.on_kill(ctx, npc, Oid(1), false);
        });
    }

    fn elcadia_of(t: &ScriptTest) -> Oid {
        let ids = t.npcs_of(ELCADIA_INSTANCE);
        assert_eq!(ids.len(), 1);
        ids[0]
    }

    fn param_int_of(t: &ScriptTest, instance_id: i32, name: &str) -> i32 {
        t.world
            .instance_world(instance_id)
            .and_then(|w| w.parameters.get(name))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    #[test]
    fn the_globe_makes_the_instance_and_a_following_elcadia() {
        let (mut t, globe) = setup();
        let instance_id = inside(&mut t, globe);
        assert_eq!(instance_id, 300_000);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (151, vec![Oid(1)]));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (START_LOC.0, START_LOC.1));
        let elcadia = elcadia_of(&t);
        assert_eq!(t.world.instance_id_of(elcadia), instance_id);
        // FOLLOW after 5 s, DIALOG after 10 s; FOLLOW makes her follow and goes on.
        assert_eq!(t.world.quest_timer_count(), 2);
        t.wait_ms(5_100);
        assert!(t.world.is_intention_follow(elcadia));
        assert_eq!(t.world.quest_timer_count(), 2);
    }

    #[test]
    fn the_scene_events_move_player_and_elcadia_together() {
        let (mut t, globe) = setup();
        let instance_id = inside(&mut t, globe);
        let elcadia = elcadia_of(&t);
        let eris = spawn_in(&mut t, instance_id, ERIS_EVIL_THOUGHTS);
        for (name, loc) in [
            ("EAST", EAST_WATCHERS_ROOM_LOC),
            ("WEST", WEST_WATCHERS_ROOM_LOC),
            ("NORTH", NORTH_WATCHERS_ROOM_LOC),
            ("SOUTH", SOUTH_WATCHERS_ROOM_LOC),
            ("CENTER", CENTRAL_ROOM_LOC),
            ("BACK", BACK_LOC),
            ("CASKET_ROOM", CASKET_ROOM_LOC),
            ("SOLINAS_RESTING_PLACE", SOLINAS_RESTING_PLACE_LOC),
            ("ERIS_OFFICE", START_LOC),
            ("DIRECTORS_ROOM", DIRECTORS_ROOM_LOC),
        ] {
            assert_eq!(event(&mut t, name, eris), None);
            // She is at the spot (she may have started to walk to the player since).
            let (x, y, _) = t.world.position_of(elcadia).unwrap();
            assert!((x - loc.0).abs() <= 50 && (y - loc.1).abs() <= 50, "{name}");
            let offset = t.world.max_offset_on_teleport();
            let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
            assert!(
                (x - loc.0).abs() <= offset && (y - loc.1).abs() <= offset,
                "{name}"
            );
        }
        // TELE2 also starts the movie timer (two seconds).
        let before = t.world.quest_timer_count();
        t.world.run_as_script("SSQMonasteryOfSilence", |ctx| {
            SSQMonasteryOfSilence.on_event(ctx, "TELE2", Some(eris), Some(Oid(1)));
        });
        assert_eq!(t.world.quest_timer_count(), before + 1);
    }

    #[test]
    fn exit_takes_the_player_out_and_elcadia_and_her_timers_end() {
        let (mut t, globe) = setup();
        let instance_id = inside(&mut t, globe);
        let elcadia = elcadia_of(&t);
        let eris = spawn_in(&mut t, instance_id, ERIS_EVIL_THOUGHTS);
        event(&mut t, "EXIT", eris);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert!(!t.in_world(elcadia));
        // NOTE(java-quirk): Java never cancels FOLLOW and DIALOG; they end here with the NPC.
        t.wait_ms(11_000);
        assert_eq!(t.world.quest_timer_count(), 0);
    }

    #[test]
    fn eris_opens_the_sixteen_doors_and_the_tomb_closes_its_four() {
        let (mut t, globe) = setup();
        let instance_id = inside(&mut t, globe);
        let eris = spawn_in(&mut t, instance_id, ERIS_EVIL_THOUGHTS);
        t.world.run_as_script("SSQMonasteryOfSilence", |ctx| {
            SSQMonasteryOfSilence.on_spawn(ctx, eris);
        });
        t.wait_ms(1_100);
        assert!(
            DOORS
                .iter()
                .all(|d| t.world.is_instance_door_open(instance_id, *d))
        );

        let tomb = spawn_in(&mut t, instance_id, TOMB_OF_THE_SAINTESS);
        for d in FAKE_TOMB_DOORS {
            t.world.instance_open_door(instance_id, d);
        }
        t.world.run_as_script("SSQMonasteryOfSilence", |ctx| {
            SSQMonasteryOfSilence.on_spawn(ctx, tomb);
        });
        t.wait_ms(1_100);
        assert!(
            FAKE_TOMB_DOORS
                .iter()
                .all(|d| !t.world.is_instance_door_open(instance_id, *d))
        );

        // The tomb: doors open, four guardians and sixteen slaves.
        assert_eq!(
            event(&mut t, "TOMB_GUARDIAN_SPAWN", tomb).as_deref(),
            Some("32843-01.html")
        );
        assert!(
            FAKE_TOMB_DOORS
                .iter()
                .all(|d| t.world.is_instance_door_open(instance_id, *d))
        );
        for guardian in [
            GUARDIAN_OF_THE_TOMB_1,
            GUARDIAN_OF_THE_TOMB_2,
            GUARDIAN_OF_THE_TOMB_3,
            GUARDIAN_OF_THE_TOMB_4,
        ] {
            assert_eq!(t.npcs_of(guardian).len(), 1);
        }
        assert_eq!(t.npcs_of(TRAINEE_OF_REST).len(), 8);
        assert_eq!(t.npcs_of(SUPPLICANT_OF_REST).len(), 8);
    }

    #[test]
    fn the_four_tomb_guardians_open_the_tomb_door_and_advance_the_quest() {
        let (mut t, globe) = setup();
        let instance_id = inside(&mut t, globe);
        set_memo(&mut t, Q10295, 2);
        for (i, guardian) in [
            GUARDIAN_OF_THE_TOMB_1,
            GUARDIAN_OF_THE_TOMB_2,
            GUARDIAN_OF_THE_TOMB_3,
            GUARDIAN_OF_THE_TOMB_4,
        ]
        .into_iter()
        .enumerate()
        {
            let npc = spawn_in(&mut t, instance_id, guardian);
            kill(&mut t, npc);
            assert_eq!(t.world.is_instance_door_open(instance_id, TOMB_DOOR), i == 3);
            assert_eq!(memo(&mut t, Q10295), if i == 3 { 3 } else { 2 });
        }
    }

    #[test]
    fn the_four_solina_guardians_end_with_a_movie_and_memo_2() {
        let (mut t, globe) = setup();
        let instance_id = inside(&mut t, globe);
        set_memo(&mut t, Q10295, 1);
        for guardian in [
            SOLINAS_GUARDIAN_1,
            SOLINAS_GUARDIAN_2,
            SOLINAS_GUARDIAN_3,
            SOLINAS_GUARDIAN_4,
        ] {
            let npc = spawn_in(&mut t, instance_id, guardian);
            kill(&mut t, npc);
        }
        assert_eq!(memo(&mut t, Q10295), 2);
        assert_eq!(param_int_of(&t, instance_id, "deadSolinaGuardianCount"), 4);
    }

    #[test]
    fn a_scroll_is_used_up_for_a_guardian() {
        let (mut t, globe) = setup();
        let instance_id = inside(&mut t, globe);
        let eris = spawn_in(&mut t, instance_id, ERIS_EVIL_THOUGHTS);
        // Without the scroll nothing happens.
        event(&mut t, "USE_SCROLL", eris);
        assert!(t.npcs_of(SOLINAS_GUARDIAN_1).is_empty());
        t.world
            .run_as_script(Q10294, |ctx| ctx.give_items(Oid(1), SCROLL_OF_ABSTINENCE, 1));
        event(&mut t, "USE_SCROLL", eris);
        assert_eq!(t.count(1, SCROLL_OF_ABSTINENCE), 0);
        assert_eq!(t.npcs_of(SOLINAS_GUARDIAN_1).len(), 1);
    }

    #[test]
    fn etis_van_etina_ends_the_quest_step_and_elcadia_comes_back_to_the_player() {
        let (mut t, globe) = setup();
        let instance_id = inside(&mut t, globe);
        let elcadia = elcadia_of(&t);
        set_memo(&mut t, Q10296, 2);
        let eris = spawn_in(&mut t, instance_id, ERIS_EVIL_THOUGHTS);
        // The boss scene: the movie, then the boss in space after a minute.
        event(&mut t, "START_MOVIE_Q10296", eris);
        let (x, y, _) = t.world.position_of(elcadia).unwrap();
        assert_eq!((x, y), (ELCADIA_LOC.0, ELCADIA_LOC.1));
        t.wait_ms(60_100);
        assert_eq!(t.npcs_of(ETIS_VAN_ETINA).len(), 1);
        let etis = t.npcs_of(ETIS_VAN_ETINA)[0];
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        let offset = t.world.max_offset_on_teleport();
        assert!((x - SPACE_LOC.0).abs() <= offset && (y - SPACE_LOC.1).abs() <= offset);

        // Its death: memo 3, and 63 s later Elcadia is at the player although the boss is gone.
        let npc = ScriptNpc {
            object_id: etis,
            id: ETIS_VAN_ETINA,
        };
        kill(&mut t, npc);
        assert_eq!(memo(&mut t, Q10296), 3);
        t.world.decay(etis);
        t.world.tele_to_location(
            Oid(1),
            TeleportTarget::at(START_LOC.0, START_LOC.1, START_LOC.2).with_instance(instance_id),
        );
        t.ticks(60);
        t.wait_ms(63_100);
        let (x, y, _) = t.world.position_of(elcadia).unwrap();
        assert_eq!((x, y), (START_LOC.0, START_LOC.1));
    }
}
