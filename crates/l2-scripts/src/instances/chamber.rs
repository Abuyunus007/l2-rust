//! Chambers of Delusion, the shared base: the six chambers (East, North, South, West, Square and
//! Tower) are this code with their own numbers ([`ChamberData`]).
// Java: dist/game/data/scripts/instances/ChambersOfDelusion/Chamber.java
//
// Java keeps `PartyInside`, `currentRoom` and the two scheduled tasks in the `InstanceWorld`
// parameters; here the first two are the script's state by instance id ([`ChamberWorld`]) and the
// tasks are quest timers whose event names carry the instance id (`banish;300000`).

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, Earthquake, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{PartyId, ScriptCtx, ScriptNpc, ScriptSetup};

// Items
const ENRIA: i32 = 4042;
const ASOFE: i32 = 4043;
const THONS: i32 = 4044;
const LEONARD: i32 = 9628;
const DELUSION_MARK: i32 = 15311;

// Skills
const SUCCESS_SKILL: SkillKey = SkillKey { id: 5758, level: 1 };
const FAIL_SKILL: SkillKey = SkillKey { id: 5376, level: 4 };

const ROOM_CHANGE_INTERVAL: i32 = 480; // 8 min
const ROOM_CHANGE_RANDOM_TIME: i32 = 120; // 2 min

// Instance restart time
const RESET_HOUR: i64 = 6;
const RESET_MIN: i64 = 30;

// Misc: `Chamber.class.getSimpleName() + "_return"`
const RETURN: &str = "Chamber_return";

/// The page folder of the six chambers.
pub(crate) const PATH: &str = "instances/ChambersOfDelusion";

/// What differs between the chambers (the arguments of Java's `Chamber` constructor and the
/// `ENTER_POINTS`).
pub(crate) struct ChamberData {
    pub(crate) instance_id: i32,
    pub(crate) entrance_gatekeeper: i32,
    pub(crate) room_gatekeeper_first: i32,
    pub(crate) room_gatekeeper_last: i32,
    pub(crate) aenkinel: i32,
    pub(crate) box_id: i32,
    /// `ROOM_ENTER_POINTS`; the last one is the raid room.
    pub(crate) enter_points: &'static [(i32, i32, i32)],
}

/// What Java keeps in the parameters of the `InstanceWorld` of a chamber.
#[derive(Debug, Default)]
pub(crate) struct ChamberState {
    worlds: HashMap<i32, ChamberWorld>,
}

#[derive(Debug, Default)]
struct ChamberWorld {
    /// `PartyInside`.
    party: Option<PartyId>,
    /// `currentRoom`.
    current_room: i32,
}

impl ChamberState {
    pub(crate) fn current_room(&self, instance_id: i32) -> i32 {
        self.worlds.get(&instance_id).map_or(0, |w| w.current_room)
    }

    pub(crate) fn party(&self, instance_id: i32) -> Option<PartyId> {
        self.worlds.get(&instance_id).and_then(|w| w.party)
    }
}

/// `Chamber(..)` constructor.
pub(crate) fn setup(d: &ChamberData, s: &mut ScriptSetup) {
    s.add_start_npc(&[d.entrance_gatekeeper]);
    s.add_talk_id(&[d.entrance_gatekeeper]);
    for i in d.room_gatekeeper_first..=d.room_gatekeeper_last {
        s.add_start_npc(&[i]);
        s.add_talk_id(&[i]);
    }

    s.add_kill_id(&[d.aenkinel]);
    s.add_attack_id(&[d.box_id]);
    s.add_spell_finished_id(&[d.box_id]);
    s.add_event_received_id(&[d.box_id]);
}

fn is_big_chamber(d: &ChamberData) -> bool {
    d.instance_id == 131 || d.instance_id == 132
}

fn is_boss_room(d: &ChamberData, ctx: &mut ScriptCtx<'_>, instance_id: i32) -> bool {
    let room = ctx.state::<ChamberState>().current_room(instance_id);
    usize::try_from(room).is_ok_and(|r| r + 1 == d.enter_points.len())
}

fn room_point(d: &ChamberData, room: i32) -> (i32, i32, i32) {
    usize::try_from(room)
        .ok()
        .and_then(|r| d.enter_points.get(r).copied())
        .unwrap_or(d.enter_points[0])
}

/// `Chamber.checkConditions(player)`.
fn check_conditions(d: &ChamberData, world: &l2_world::World, player: ObjectId) -> bool {
    let Some(party) = world.party_of(player) else {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter),
        );
        return false;
    };

    if world.party_leader(party) != Some(player) {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    // `party.broadcastPacket(sm)`
    let broadcast = |sm: &SystemMessage| {
        for member in world.party_members(party) {
            world.send(member, sm);
        }
    };
    for member in world.party_members(party) {
        let name = world
            .player(member)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        if world.player(member).map_or(0, |p| p.core.level) < 80 {
            broadcast(
                &SystemMessage::new(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry)
                    .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }

        if !world.check_if_in_range(1000, player, member, true) {
            broadcast(
                &SystemMessage::new(
                    SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                )
                .with(SystemMessageParam::PlayerName(name)),
            );
            return false;
        }

        if is_big_chamber(d) {
            let reentertime = world.instance_time(member, d.instance_id);
            if world.instance_clock_ms() < reentertime {
                broadcast(
                    &SystemMessage::new(SystemMessageId::C1MayNotReEnterYet)
                        .with(SystemMessageParam::PlayerName(name)),
                );
                return false;
            }
        }
    }

    true
}

/// The re-entry time of `markRestriction`: today 06:30 on the server's clock with the seconds of
/// now, a day later when that is before now (D-48).
pub(crate) fn restriction_time(now_utc: i64, offset_ms: i64) -> i64 {
    const DAY_MS: i64 = 86_400_000;
    let local = now_utc + offset_ms;
    let day_start = local.div_euclid(DAY_MS) * DAY_MS;
    let mut reenter =
        day_start + RESET_HOUR * 3_600_000 + RESET_MIN * 60_000 + local.rem_euclid(60_000) - offset_ms;
    if reenter < now_utc {
        reenter += DAY_MS;
    }
    reenter
}

/// `markRestriction(world)`: `setInstanceTime` for every allowed player in the world, with the
/// message.
fn mark_restriction(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let now = ctx.world().instance_clock_ms();
    let offset = l2_util::time::local_offset_millis(now);
    ctx.world_mut()
        .set_reenter_time(instance_id, restriction_time(now, offset));
}

/// `changeRoom(world)`.
fn change_room(d: &ChamberData, ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let Some(party) = ctx.state::<ChamberState>().party(instance_id) else {
        return;
    };
    let Some(end_time) = ctx.world().instance(instance_id).map(|i| i.end_time_ms) else {
        return;
    };

    // Do nothing, if there are raid room of Sqare or Tower Chamber
    if is_big_chamber(d) && is_boss_room(d, ctx, instance_id) {
        return;
    }

    let last = i32::try_from(d.enter_points.len() - 1).unwrap_or(0);
    let current = ctx.state::<ChamberState>().current_room(instance_id);
    let mut new_room = current;
    // Teleport to raid room 10 min or lesser before instance end time for Tower and Square Chambers
    // ... or, 10% chance for teleport to raid room if not here already for the four small
    // chambers. Java's two branches both pick the raid room, so they are one condition here; the
    // random roll happens only when the first part is false, as in Java.
    if (is_big_chamber(d) && (end_time - ctx.world().instance_clock_ms()) < 600_000)
        || (!is_big_chamber(d) && !is_boss_room(d, ctx, instance_id) && ctx.get_random(100) < 10)
    {
        new_room = last;
    } else {
        // otherwise teleport to another room, except current
        while new_room == current {
            new_room = ctx.get_random(last + 1);
        }
    }

    let (x, y, z) = room_point(d, new_room);
    for member in ctx.world().party_members(party) {
        if instance_id == ctx.world().instance_id_of(member) {
            ctx.world_mut().script_set_intention_idle(member);
            ctx.teleport_player(member, x, y, z, instance_id, true);
        }
    }

    if let Some(w) = ctx.state::<ChamberState>().worlds.get_mut(&instance_id) {
        w.current_room = new_room;
    }

    // Do not schedule room change for Square and Tower Chambers, if raid room is reached
    if is_big_chamber(d) && is_boss_room(d, ctx, instance_id) {
        // Add 20 min to instance time if raid room is reached
        let remaining = end_time - ctx.world().instance_clock_ms();
        ctx.world_mut()
            .set_instance_duration(instance_id, (remaining + 1_200_000) as i32);
        let npcs: Vec<ObjectId> = ctx
            .world()
            .instance(instance_id)
            .map(|i| i.npcs.keys().copied().collect())
            .unwrap_or_default();
        for object_id in npcs {
            if ctx
                .world()
                .npc(object_id)
                .is_some_and(|n| n.npc_id == d.room_gatekeeper_last)
            {
                ctx.broadcast_npc_say(
                    ScriptNpc {
                        object_id,
                        id: d.room_gatekeeper_last,
                    },
                    ChatType::NpcGeneral,
                    NpcStringId::TwentyMinutesAreAddedToTheRemainingTimeInTheInstantZone,
                    &[],
                );
            }
        }
    } else {
        schedule_room_change(ctx, instance_id, false);
    }
}

/// `enter(world)`: the first entrance of a party.
fn enter(d: &ChamberData, ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let Some(party) = ctx.state::<ChamberState>().party(instance_id) else {
        return;
    };

    for member in ctx.world().party_members(party) {
        if ctx.has_quest_items(member, DELUSION_MARK) {
            ctx.take_items(member, DELUSION_MARK, -1);
        }

        if ctx.world().party_leader(party) == Some(member) {
            ctx.give_items(member, DELUSION_MARK, 1);
        }

        // Save location for teleport back into main hall
        if let Some((x, y, z)) = ctx.world().position_of(member) {
            ctx.set_player_variable(member, RETURN, &format!("{x};{y};{z}"));
        }

        ctx.world_mut().set_instance_id(member, instance_id);
        if let Some(w) = ctx.world_mut().instance_world_mut(instance_id) {
            w.add_allowed(member);
        }
    }

    change_room(d, ctx, instance_id);
}

/// `earthQuake(world)`.
fn earth_quake(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let Some(party) = ctx.state::<ChamberState>().party(instance_id) else {
        return;
    };

    for member in ctx.world().party_members(party) {
        if instance_id == ctx.world().instance_id_of(member) {
            if let Some((x, y, z)) = ctx.world().position_of(member) {
                ctx.world().send(
                    member,
                    &Earthquake {
                        x,
                        y,
                        z,
                        intensity: 20,
                        duration: 10,
                    },
                );
            }
        }
    }
}

/// `onEnterInstance(player, world, firstEntrance)`, run after `enterInstance` made or found the
/// world (the chambers' templates have no enter location and no buff removal, so the order
/// against the rest of `enterInstance` does not show).
fn on_enter_instance(
    d: &ChamberData,
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    instance_id: i32,
    first: bool,
) {
    if first {
        let party = ctx.world().party_of(player);
        ctx.state::<ChamberState>().worlds.insert(
            instance_id,
            ChamberWorld {
                party,
                current_room: 0,
            },
        );
        ctx.start_quest_timer(&format!("banish;{instance_id}"), 60_000, None, None, true);
        enter(d, ctx, instance_id);
    } else {
        let room = ctx.state::<ChamberState>().current_room(instance_id);
        let (x, y, z) = room_point(d, room);
        ctx.teleport_player(player, x, y, z, instance_id, true);
    }
}

/// `exitInstance(player)`.
fn exit_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    if ctx.world().player(player).is_none() || !ctx.world().is_in_instance(player) {
        return;
    }

    let instance_id = ctx.world().instance_id_of(player);
    let Some(instance) = ctx.world().instance(instance_id) else {
        return;
    };
    let mut ret = instance.exit_location;
    if let Some(return_point) = ctx.player_variable(player, RETURN) {
        let coords: Vec<&str> = return_point.split(';').collect();
        if coords.len() == 3 {
            if let (Ok(x), Ok(y), Ok(z)) = (
                coords[0].parse::<i32>(),
                coords[1].parse::<i32>(),
                coords[2].parse::<i32>(),
            ) {
                // NOTE(java-quirk): `ret` is the instance's own exit location, so this moves
                // the exit point of the instance for whoever leaves later without a variable.
                ret = Some((x, y, z));
                if let Some(instance) = ctx.world_mut().instance_mut(instance_id) {
                    instance.exit_location = ret;
                }
            }
        }
    }

    if let Some((x, y, z)) = ret {
        ctx.teleport_player(player, x, y, z, 0, true);
    }
    // `getPlayerWorld(player)`: the first world that allows him.
    let world = ctx.world().player_instance_world(player).map(|w| w.instance_id);
    if let Some(world) = world.and_then(|id| ctx.world_mut().instance_world_mut(id)) {
        world.remove_allowed(player);
    }
}

pub(crate) fn on_event(
    d: &ChamberData,
    ctx: &mut ScriptCtx<'_>,
    event: &str,
    npc: Option<ScriptNpc>,
    player: Option<ObjectId>,
) -> Option<String> {
    // The scheduled tasks (`BanishTask`, `ChangeRoomTask`).
    if let Some((task, id)) = event.split_once(';') {
        if let Ok(instance_id) = id.parse::<i32>() {
            match task {
                "banish" => banish_task(ctx, instance_id),
                "roomChange" => {
                    earth_quake(ctx, instance_id);
                    // `Thread.sleep(5000)`
                    ctx.start_quest_timer(&format!("roomChange2;{instance_id}"), 5000, None, None, false);
                }
                "roomChange2" => change_room(d, ctx, instance_id),
                _ => {}
            }
        }
        return None;
    }

    let mut htmltext = Some(String::new());
    // `InstanceManager.getWorld(npc)`; Java throws on a null NPC.
    let npc = npc?;
    let world = ctx
        .world()
        .instance_world_of(npc.object_id)
        .map(|w| w.instance_id);
    if let (Some(player), Some(instance_id)) = (player, world) {
        if npc.id >= d.room_gatekeeper_first && npc.id <= d.room_gatekeeper_last {
            let party = ctx.world().party_of(player);
            let leader = party.and_then(|p| ctx.world().party_leader(p));
            match event {
                "next_room" => {
                    if party.is_none() {
                        htmltext = Some(page(ctx, player, "no_party.html"));
                    } else if leader != Some(player) {
                        htmltext = Some(page(ctx, player, "no_leader.html"));
                    } else if ctx.has_quest_items(player, DELUSION_MARK) {
                        ctx.take_items(player, DELUSION_MARK, 1);
                        stop_room_change_task(ctx, instance_id);
                        change_room(d, ctx, instance_id);
                    } else {
                        htmltext = Some(page(ctx, player, "no_item.html"));
                    }
                }
                "go_out" => {
                    if let Some(party) = party {
                        if leader != Some(player) {
                            htmltext = Some(page(ctx, player, "no_leader.html"));
                        } else {
                            stop_room_change_task(ctx, instance_id);
                            stop_banish_task(ctx, instance_id);

                            for member in ctx.world().party_members(party) {
                                exit_instance(ctx, member);
                            }

                            ctx.world_mut().set_instance_empty_destroy_time(instance_id, 0);
                        }
                    } else {
                        htmltext = Some(page(ctx, player, "no_party.html"));
                    }
                }
                "look_party" => {
                    if party.is_some() && party == ctx.state::<ChamberState>().party(instance_id) {
                        let room = ctx.state::<ChamberState>().current_room(instance_id);
                        let (x, y, z) = room_point(d, room);
                        ctx.teleport_player(player, x, y, z, instance_id, false);
                    }
                }
                _ => {}
            }
        }
    }

    htmltext
}

/// `getHtm(player, "data/scripts/instances/ChambersOfDelusion/<file>")`.
fn page(ctx: &ScriptCtx<'_>, player: ObjectId, file: &str) -> String {
    ctx.get_htm(player, &format!("data/scripts/{PATH}/{file}"))
        .unwrap_or_default()
}

pub(crate) fn on_attack(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, attacker: ObjectId) {
    if !ctx.is_npc_busy(npc) && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc) / 10) {
        ctx.set_npc_busy(npc, true);
        if ctx.get_random(100) < 25 {
            // 25% chance to reward
            if ctx.get_random(100) < 33 {
                drop_item(ctx, npc, attacker, ENRIA, 3);
            }

            if ctx.get_random(100) < 50 {
                drop_item(ctx, npc, attacker, THONS, 4);
            }

            if ctx.get_random(100) < 50 {
                drop_item(ctx, npc, attacker, ASOFE, 4);
            }

            if ctx.get_random(100) < 16 {
                drop_item(ctx, npc, attacker, LEONARD, 2);
            }

            ctx.world_mut()
                .broadcast_npc_event(npc.object_id, "SCE_LUCKY", 2000, None);
            ctx.world_mut().do_cast(npc.object_id, SUCCESS_SKILL);
        } else {
            ctx.world_mut()
                .broadcast_npc_event(npc.object_id, "SCE_DREAM_FIRE_IN_THE_HOLE", 2000, None);
        }
    }
}

fn drop_item(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, item_id: i32, count: i64) {
    ctx.world_mut()
        .npc_drop_item(npc.object_id, Some(player), item_id, count);
}

pub(crate) fn on_event_received(
    ctx: &mut ScriptCtx<'_>,
    event_name: &str,
    receiver: ScriptNpc,
) -> Option<String> {
    match event_name {
        "SCE_LUCKY" => {
            ctx.set_npc_busy(receiver, true);
            ctx.world_mut().do_cast(receiver.object_id, SUCCESS_SKILL);
        }
        "SCE_DREAM_FIRE_IN_THE_HOLE" => {
            ctx.set_npc_busy(receiver, true);
            ctx.world_mut().do_cast(receiver.object_id, FAIL_SKILL);
        }
        _ => {}
    }

    None
}

pub(crate) fn on_kill(d: &ChamberData, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    // `getPlayerWorld(player)`
    let Some(instance_id) = ctx.world().player_instance_world(player).map(|w| w.instance_id) else {
        return;
    };
    let Some(end_time) = ctx.world().instance(instance_id).map(|i| i.end_time_ms) else {
        return;
    };

    if is_big_chamber(d) {
        mark_restriction(ctx, instance_id); // Set reenter restriction
        if (end_time - ctx.world().instance_clock_ms()) > 300_000 {
            ctx.world_mut().set_instance_duration(instance_id, 300_000); // Finish instance in 5 minutes
        }
    } else {
        stop_room_change_task(ctx, instance_id);
        schedule_room_change(ctx, instance_id, true);
    }

    ctx.world_mut().spawn_instance_group(instance_id, "boxes");
}

pub(crate) fn on_spell_finished(
    d: &ChamberData,
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    player: Option<ObjectId>,
    skill: SkillKey,
) {
    if npc.id == d.box_id && (skill.id == 5376 || skill.id == 5758) && !ctx.is_dead(npc.object_id) {
        ctx.world_mut().do_die(npc.object_id, player);
    }
}

pub(crate) fn on_talk(
    d: &ChamberData,
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    player: ObjectId,
) -> Option<String> {
    let _ = ctx.get_quest_state(player, true);

    if npc.id == d.entrance_gatekeeper {
        let mut entered: Option<(i32, bool)> = None;
        ctx.world_mut().enter_instance(
            player,
            d.instance_id,
            |world, player| check_conditions(d, world, player),
            |_, _, instance_id, first| entered = Some((instance_id, first)),
        );
        if let Some((instance_id, first)) = entered {
            on_enter_instance(d, ctx, player, instance_id, first);
        }
    }

    Some(String::new())
}

/// `scheduleRoomChange(world, bossRoom)`.
fn schedule_room_change(ctx: &mut ScriptCtx<'_>, instance_id: i32, boss_room: bool) {
    let Some(end_time) = ctx.world().instance(instance_id).map(|i| i.end_time_ms) else {
        return;
    };
    let next_interval = if boss_room {
        60_000
    } else {
        i64::from(ROOM_CHANGE_INTERVAL + ctx.get_random(ROOM_CHANGE_RANDOM_TIME)) * 1000
    };

    // Schedule next room change only if remaining time is enough
    if (end_time - ctx.world().instance_clock_ms()) > next_interval {
        ctx.start_quest_timer(
            &format!("roomChange;{instance_id}"),
            next_interval - 5000,
            None,
            None,
            false,
        );
    }
}

fn stop_banish_task(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    ctx.cancel_quest_timer(&format!("banish;{instance_id}"), None, None);
}

fn stop_room_change_task(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    ctx.cancel_quest_timer(&format!("roomChange;{instance_id}"), None, None);
    // The task sleeps 5 s after the earthquake; cancelling it interrupts the sleep.
    ctx.cancel_quest_timer(&format!("roomChange2;{instance_id}"), None, None);
}

/// `BanishTask.run()`.
fn banish_task(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let now = ctx.world().instance_clock_ms();
    let remaining = ctx.world().instance(instance_id).map(|i| i.end_time_ms - now);
    if remaining.is_none_or(|r| r < 60_000) {
        stop_banish_task(ctx, instance_id);
    } else {
        let players: Vec<ObjectId> = ctx
            .world()
            .instance(instance_id)
            .map(|i| i.players.iter().copied().collect())
            .unwrap_or_default();
        let party = ctx.state::<ChamberState>().party(instance_id);
        for pl in players {
            if ctx.world().player(pl).is_some() {
                let pl_party = ctx.world().party_of(pl);
                if party.is_none() || !ctx.world().is_in_party(pl) || party != pl_party {
                    exit_instance(ctx, pl);
                }
            }
        }
    }
}

/// Writes the `impl Script` of one chamber:
/// `chamber_script!(ChamberOfDelusionEast, "ChamberOfDelusionEast", DATA);`.
macro_rules! chamber_script {
    ($ty:ident, $name:literal, $data:expr) => {
        impl l2_world::Script for $ty {
            fn name(&self) -> &'static str {
                $name
            }

            fn path(&self) -> &'static str {
                $crate::instances::chamber::PATH
            }

            fn setup(&self, s: &mut l2_world::ScriptSetup) {
                $crate::instances::chamber::setup(&$data, s);
            }

            fn on_event(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                event: &str,
                npc: Option<l2_world::ScriptNpc>,
                player: Option<l2_util::ObjectId>,
            ) -> Option<String> {
                $crate::instances::chamber::on_event(&$data, ctx, event, npc, player)
            }

            fn on_attack(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                attacker: l2_util::ObjectId,
                _damage: i32,
                _is_summon: bool,
                _skill: Option<l2_world::effects::SkillKey>,
            ) {
                $crate::instances::chamber::on_attack(ctx, npc, attacker);
            }

            fn on_event_received(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                event_name: &str,
                _sender: l2_world::ScriptNpc,
                receiver: l2_world::ScriptNpc,
                _reference: Option<l2_util::ObjectId>,
            ) -> Option<String> {
                $crate::instances::chamber::on_event_received(ctx, event_name, receiver)
            }

            fn on_kill(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                _npc: l2_world::ScriptNpc,
                killer: l2_util::ObjectId,
                _is_summon: bool,
            ) {
                $crate::instances::chamber::on_kill(&$data, ctx, killer);
            }

            fn on_spell_finished(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                player: Option<l2_util::ObjectId>,
                skill: l2_world::effects::SkillKey,
            ) {
                $crate::instances::chamber::on_spell_finished(&$data, ctx, npc, player, skill);
            }

            fn on_talk(
                &self,
                ctx: &mut l2_world::ScriptCtx<'_>,
                npc: l2_world::ScriptNpc,
                player: l2_util::ObjectId,
            ) -> Option<String> {
                $crate::instances::chamber::on_talk(&$data, ctx, npc, player)
            }
        }
    };
}
pub(crate) use chamber_script;
