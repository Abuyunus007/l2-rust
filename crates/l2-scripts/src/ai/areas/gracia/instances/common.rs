//! What the Gracia instance scripts share: Java has a copy of each of these methods in every
//! script (`teleportPlayer`, `broadCastPacket`, the entry checks, the tumor warp, the Ward of
//! Death, the end of an instance).
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/{HallOfErosionAttack,HallOfErosionDefence,
//       HallOfSufferingAttack,HallOfSufferingDefence,HeartInfinityAttack,HeartInfinityDefence,
//       SeedOfDestruction}/*.java
//
// # How the scripts keep Java's `InstanceWorld` subclasses
// Java's `HEAWorld`, `HSAWorld`, ... are fields on a subclass of `InstanceWorld`. Here the
// world's [`l2_world::instance::InstanceWorld`] keeps what every script reads (the allowed
// players, the status and the parameters such as `tag`, which `DestroyedTumors` reads), and
// the rest is a struct of the script in `ctx.state::<Worlds<T>>()`, by instance id. "The world
// is a `HEAWorld`" (`instanceof`) is "the script has state for that instance".
//
// Java's `ThreadPool.schedule(..)` tasks become quest timers without NPC or player whose name
// carries the instance id (and the NPC), so they outlive the NPC like Java's tasks do.

use std::collections::BTreeMap;

use l2_protocol_game::server::{ExShowScreenMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::teleport::TeleportTarget;
use l2_world::{ScriptCtx, ScriptNpc};

/// The script's part of each instance world, by instance id.
pub type Worlds<T> = BTreeMap<i32, T>;

/// An NPC and the place it had when the script last saw it (Java keeps the `Npc` object, whose
/// location stays readable after `deleteMe`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcAt {
    pub npc: ScriptNpc,
    pub at: (i32, i32, i32, i32),
}

impl NpcAt {
    /// The NPC's place now, else the last one known (Java `getLocation()`).
    pub fn location(&self, ctx: &ScriptCtx<'_>) -> (i32, i32, i32, i32) {
        location_of(ctx, self.npc.object_id).unwrap_or(self.at)
    }
}

/// `(x, y, z, heading)` of an object in the world.
pub fn location_of(ctx: &ScriptCtx<'_>, object: ObjectId) -> Option<(i32, i32, i32, i32)> {
    let (x, y, z) = ctx.world().position_of(object)?;
    Some((x, y, z, ctx.world().object_heading(object)))
}

/// Java `addSpawn(npcId, x, y, z, heading, false, 0, false, instanceId)`, remembering the place.
pub fn spawn(ctx: &mut ScriptCtx<'_>, npc_id: i32, (x, y, z, heading): (i32, i32, i32, i32), instance_id: i32) -> Option<NpcAt> {
    ctx.add_spawn(npc_id, x, y, z, heading, false, 0, instance_id)
        .map(|npc| NpcAt {
            npc,
            at: (x, y, z, heading),
        })
}

/// One `{npcId, x, y, z, heading, respawnDelay, count}` row of the room tables: `count` spawns
/// whose respawn is `respawnDelay` seconds (`setRespawnDelay`, `setAmount(1)`, `startRespawn`
/// above 0, else `stopRespawn`).
pub fn spawn_with_respawn(ctx: &mut ScriptCtx<'_>, row: &[i32; 7], instance_id: i32) -> Vec<NpcAt> {
    let mut spawned = Vec::new();
    for _ in 0..row[6] {
        let Some(npc) = spawn(ctx, row[0], (row[1], row[2], row[3], row[4]), instance_id) else {
            continue;
        };
        if let Some(index) = ctx.world().npc_spawn_index(npc.npc.object_id) {
            ctx.world_mut().set_spawn_respawn_delay(index, row[5]);
            if row[5] > 0 {
                ctx.world_mut().start_spawn_respawn(index);
            } else {
                ctx.world_mut().stop_spawn_respawn(index);
            }
        }
        spawned.push(npc);
    }
    spawned
}

/// The script's `teleportPlayer(player, coords, instanceId)`: `setInstanceId` and
/// `teleToLocation(x, y, z)`.
pub fn teleport_into(ctx: &mut ScriptCtx<'_>, player: ObjectId, (x, y, z): (i32, i32, i32), instance_id: i32) {
    ctx.world_mut()
        .tele_to_location(player, TeleportTarget::at(x, y, z).with_instance(instance_id));
}

/// `teleToLocation(loc, true)`: the place (with its heading and instance) and the random offset.
pub fn teleport_to_location(ctx: &mut ScriptCtx<'_>, player: ObjectId, (x, y, z, heading): (i32, i32, i32, i32), instance_id: i32) {
    let offset = ctx.world().max_offset_on_teleport();
    ctx.world_mut().tele_to_location(
        player,
        TeleportTarget::at(x, y, z)
            .with_heading(heading)
            .with_instance(instance_id)
            .with_random_offset(offset),
    );
}

/// `player.isOnline()`.
pub fn is_online(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world().player(player).is_some()
}

/// `world.getAllowed()` of an instance.
pub fn allowed(ctx: &ScriptCtx<'_>, instance_id: i32) -> Vec<ObjectId> {
    ctx.world()
        .instance_world(instance_id)
        .map(|w| w.allowed.clone())
        .unwrap_or_default()
}

/// The instance an object is in when the instance has a world (Java `getWorld(object)`).
pub fn world_of(ctx: &ScriptCtx<'_>, object: ObjectId) -> Option<i32> {
    ctx.world().instance_world_of(object).map(|w| w.instance_id)
}

/// The instance world a player is allowed in (Java `getPlayerWorld(player)`).
pub fn player_world(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<i32> {
    ctx.world().player_instance_world(player).map(|w| w.instance_id)
}

/// The script's `broadCastPacket(world, new ExShowScreenMessage(npcString, 2, 8000))`: the
/// allowed players that are online and inside.
pub fn screen_message(ctx: &mut ScriptCtx<'_>, instance_id: i32, npc_string: NpcStringId, params: &[&str]) {
    let packet = ExShowScreenMessage::npc_string(npc_string, 2, 8000, params);
    for player in allowed(ctx, instance_id) {
        if is_online(ctx, player) && ctx.world().instance_id_of(player) == instance_id {
            ctx.world().send(player, &packet);
        }
    }
}

/// A system message with the name of `name_of` (`sm.addPcName(member)`).
pub fn named_message(ctx: &ScriptCtx<'_>, id: SystemMessageId, name_of: ObjectId) -> SystemMessage {
    SystemMessage::new(id).with(SystemMessageParam::PlayerName(ctx.player_name(name_of)))
}

/// `new SystemMessage(id)` of a raw id (`SystemMessage(2097)`).
pub fn message_by_id(raw: i32) -> Option<SystemMessageId> {
    SystemMessageId::from_id(raw)
}

/// Java `CommandChannel.broadcastPacket(packet)`: every member of every party.
pub fn broadcast_to_channel(ctx: &ScriptCtx<'_>, player: ObjectId, packet: &SystemMessage) {
    if let Some(channel) = ctx.world().command_channel_of(player) {
        for member in ctx.world().command_channel_members(channel) {
            ctx.world().send(member, packet);
        }
    }
}

/// Java `Party.broadcastPacket(packet)`.
pub fn broadcast_to_party(ctx: &ScriptCtx<'_>, player: ObjectId, packet: &SystemMessage) {
    for member in ctx.world().party_members_of(player) {
        ctx.world().send(member, packet);
    }
}

/// `party.getCommandChannel().getMembers()` of the player's party (empty without one).
pub fn channel_members(ctx: &ScriptCtx<'_>, player: ObjectId) -> Vec<ObjectId> {
    ctx.world()
        .command_channel_of(player)
        .map(|c| ctx.world().command_channel_members(c))
        .unwrap_or_default()
}

/// `party.getLeader() == player`.
pub fn is_party_leader(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world().party_of(player).is_some() && ctx.world().is_party_leader(player)
}

/// `channel.getLeader() == player` of the player's party's channel.
pub fn is_channel_leader(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world()
        .command_channel_of(player)
        .is_some_and(|c| ctx.world().is_command_channel_leader(c, player))
}

/// The entry checks of the Halls of Erosion and the Heart of Infinity (one command channel led
/// by the player, `min..=max` members of level 75 to 85 within 1000 of him, none restricted):
/// the members' part, after the party and channel checks of each script.
pub fn check_channel_members(ctx: &mut ScriptCtx<'_>, player: ObjectId, template_id: i32, min: i32, max: i32, quest: Option<&str>) -> bool {
    let members = channel_members(ctx, player);
    let count = i32::try_from(members.len()).unwrap_or(i32::MAX);
    if count < min || count > max {
        // NOTE(java-quirk): the "level does not correspond" message, without the name it asks for.
        let sm = SystemMessage::new(SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry);
        broadcast_to_channel(ctx, player, &sm);
        return false;
    }

    let now = ctx.now_ms();
    for member in members {
        let level = ctx.player_level(member);
        if !(75..=85).contains(&level) {
            if let Some(id) = message_by_id(2097) {
                let sm = named_message(ctx, id, member);
                broadcast_to_channel(ctx, player, &sm);
            }
            return false;
        }

        if !ctx.check_if_in_range(1000, player, member, true) {
            if let Some(id) = message_by_id(2096) {
                let sm = named_message(ctx, id, member);
                broadcast_to_channel(ctx, player, &sm);
            }
            return false;
        }

        let reentertime = ctx.world().instance_time(member, template_id);
        if now < reentertime {
            if let Some(id) = message_by_id(2100) {
                let sm = named_message(ctx, id, member);
                broadcast_to_channel(ctx, player, &sm);
            }
            return false;
        }

        if let Some(quest) = quest {
            let cond_1 = ctx
                .player_quest_state(member, quest)
                .is_some_and(|qs| qs.is_cond(1));
            if !cond_1 {
                let sm = named_message(
                    ctx,
                    SystemMessageId::C1SQuestRequirementIsNotSufficientAndCannotBeEntered,
                    member,
                );
                broadcast_to_channel(ctx, player, &sm);
                return false;
            }
        }
    }

    true
}

/// The `isBossAttacked` block of `onAttack`: every allowed player (online ones only with
/// `online_only`) may enter again in `hours` hours and is told so.
pub fn restrict_entry(ctx: &mut ScriptCtx<'_>, instance_id: i32, template_id: i32, hours: i64, online_only: bool) {
    // `Calendar.getInstance(); reenter.add(Calendar.HOUR, INSTANCEPENALTY)`.
    let reenter = ctx.now_ms() + hours * 3_600_000;
    let sm = SystemMessage::new(
        SystemMessageId::InstantZoneS1SEntryHasBeenRestrictedYouCanCheckTheNextPossibleEntryTimeByUsingTheCommandInstancezone,
    )
    .with(SystemMessageParam::InstanceName(template_id));
    for player in allowed(ctx, instance_id) {
        if online_only && !is_online(ctx, player) {
            continue;
        }
        ctx.world_mut().set_instance_time(player, template_id, reenter);
        if is_online(ctx, player) {
            ctx.world().send(player, &sm);
        }
    }
}

/// `inst.setDuration(5 * 60000); inst.setEmptyDestroyTime(0)`.
pub fn expire_in_five_minutes(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    ctx.world_mut().set_instance_duration(instance_id, 5 * 60000);
    ctx.world_mut().set_instance_empty_destroy_time(instance_id, 0);
}

/// The end of a conquest or defence: `inst.removeNpcs()`, then five minutes (and five minutes
/// for an empty instance when players are inside).
pub fn conclude(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    if ctx.world().instance(instance_id).is_none() {
        return;
    }
    ctx.world_mut().remove_instance_npcs(instance_id);
    let empty = ctx
        .world()
        .instance(instance_id)
        .is_none_or(|i| i.players.is_empty());
    if empty {
        ctx.world_mut().set_instance_duration(instance_id, 5 * 60000);
    } else {
        ctx.world_mut().set_instance_duration(instance_id, 5 * 60000);
        ctx.world_mut().set_instance_empty_destroy_time(instance_id, 5 * 60000);
    }
}

/// The `warp` event of the Halls of Erosion and the Heart of Infinity: the leader pays a Soul
/// Stone Fragment (13797) and his party members within 500 go to a destroyed tumor at random.
/// `dead_tumors` is the world's list, with `dead_tumor` (the last one) added once more.
pub fn warp(ctx: &mut ScriptCtx<'_>, instance_id: i32, player: ObjectId, dead_tumors: &mut Vec<NpcAt>, dead_tumor: Option<NpcAt>) {
    if let Some(victim) = dead_tumor {
        dead_tumors.push(victim);
    }

    ctx.world_mut().destroy_item_by_item_id(player, 13797, 1, true);
    // NOTE(java-quirk): `getRandom(0)` of an empty list would throw; nothing happens here.
    if dead_tumors.is_empty() {
        return;
    }
    let index = ctx.get_random(i32::try_from(dead_tumors.len()).unwrap_or(i32::MAX));
    let Some(tumor) = dead_tumors.get(usize::try_from(index).unwrap_or(0)).copied() else {
        return;
    };
    let loc = tumor.location(ctx);
    screen_message(
        ctx,
        instance_id,
        NpcStringId::S1SPartyHasMovedToADifferentLocationThroughTheCrackInTheTumor,
        &[],
    );
    let tumor_instance = ctx.world().instance_id_of(tumor.npc.object_id);
    for member in ctx.world().party_members_of(player) {
        if ctx.distance_3d(member, player).is_some_and(|d| d <= 500.0) {
            teleport_to_location(ctx, member, loc, tumor_instance);
        }
    }
}

/// `partyMember.teleToLocation(x, y, z, true)` for the members of the player's party within
/// `radius` of him.
pub fn teleport_party_near(ctx: &mut ScriptCtx<'_>, player: ObjectId, radius: f64, (x, y, z): (i32, i32, i32)) {
    let offset = ctx.world().max_offset_on_teleport();
    for member in ctx.world().party_members_of(player) {
        if ctx.distance_3d(member, player).is_some_and(|d| d <= radius) {
            ctx.world_mut()
                .tele_to_location(member, TeleportTarget::at(x, y, z).with_random_offset(offset));
        }
    }
}

/// The Ward of Death (18668) of `onAggroRangeEnter`: one to four monsters where it stands, then
/// it dies (`doDie(npc)`) or goes (`deleteMe`).
pub fn ward_of_death(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, instance_id: i32, monsters: &[i32], die: bool) {
    let Some(loc) = location_of(ctx, npc.object_id) else {
        return;
    };
    // NOTE(java-quirk): `for (int i = 0; i < getRandom(1, 4); i++)` draws the bound again at
    // every round.
    let mut i = 0;
    while i < ctx.get_random_range(1, 4) {
        if let Some(id) = ctx.get_random_entry(monsters) {
            spawn(ctx, id, loc, instance_id);
        }
        i += 1;
    }

    if die {
        ctx.do_die(npc, npc.object_id);
    } else {
        ctx.delete_npc(npc);
    }
}

/// `npc.setCurrentHp(npc.getMaxHp() * ratio)`.
pub fn set_hp_ratio(ctx: &mut ScriptCtx<'_>, npc: ObjectId, ratio: f64) {
    if let Some((_, max)) = ctx.world().creature_hp(npc) {
        ctx.world_mut().set_current_hp(npc, max * ratio, true);
    }
}

/// `world.getParameters().getInt("tag", -1)` + `delta` into `tag`.
pub fn add_tag(ctx: &mut ScriptCtx<'_>, instance_id: i32, delta: i32) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        let tag = world
            .parameters
            .get("tag")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(-1);
        world
            .parameters
            .insert("tag".to_owned(), (tag + delta).to_string());
    }
}

/// `world.setParameter(name, value)`.
pub fn set_parameter(ctx: &mut ScriptCtx<'_>, instance_id: i32, name: &str, value: &str) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.parameters.insert(name.to_owned(), value.to_owned());
    }
}

/// The status of an instance world (`world.getStatus()`).
pub fn status(ctx: &ScriptCtx<'_>, instance_id: i32) -> i32 {
    ctx.world()
        .instance_world(instance_id)
        .map_or(0, |w| w.status())
}

/// `world.setStatus(value)`.
pub fn set_status(ctx: &mut ScriptCtx<'_>, instance_id: i32, value: i32) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.set_status(value);
    }
}

/// `world.incStatus()`.
pub fn inc_status(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.inc_status();
    }
}

/// `world.addAllowed(player)`.
pub fn add_allowed(ctx: &mut ScriptCtx<'_>, instance_id: i32, player: ObjectId) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.add_allowed(player);
    }
}

/// `new XWorld(); world.setInstance(createDynamicInstance(templateId)); addWorld(world)`: the
/// new instance's id.
pub fn create_world(ctx: &mut ScriptCtx<'_>, template_id: i32) -> i32 {
    let instance_id = ctx.world_mut().create_dynamic_instance(template_id);
    // `world.getTemplateId()` is the instance's template; without the template file the instance
    // has none and the world still is of this script.
    let template = ctx
        .world()
        .instance(instance_id)
        .map(|i| i.template_id)
        .filter(|t| *t > 0)
        .unwrap_or(template_id);
    ctx.world_mut()
        .add_instance_world(l2_world::instance::InstanceWorld::new(instance_id, template));
    instance_id
}

/// The `newQuestState(player)` of the scripts that keep a quest state for nothing.
pub fn new_quest_state(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    ctx.get_quest_state(player, true);
}
