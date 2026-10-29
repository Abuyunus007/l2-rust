//! `Hellbound/Instances`: the Demon Prince's and Ranku's floors of the Tower of Infinitum and the
//! Urban Area.

pub mod demon_prince_floor;
pub mod ranku_floor;
pub mod urban_area;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::teleport::TeleportTarget;
use l2_world::{ScriptCtx, ScriptRegistry, World};

/// Registers every script of the folder in Java's `HellboundLoader` order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(demon_prince_floor::DemonPrinceFloor);
    r.add(urban_area::UrbanArea);
    r.add(ranku_floor::RankuFloor);
}

/// Java `LocationUtil.checkIfInRange(range, a, b, true)`.
pub(crate) fn in_range_3d(world: &World, range: i32, a: ObjectId, b: ObjectId) -> bool {
    match (world.position_of(a), world.position_of(b)) {
        (Some(a), Some(b)) => {
            let (dx, dy, dz) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1), f64::from(a.2 - b.2));
            (dx * dx + dy * dy + dz * dz).sqrt() <= f64::from(range)
        }
        _ => false,
    }
}

/// Java `party.broadcastPacket(new SystemMessage(id).addPcName(member))`.
pub(crate) fn party_message(world: &World, party_of: ObjectId, id: SystemMessageId, member: ObjectId) {
    let name = world
        .player(member)
        .map(|p| p.data.name.clone())
        .unwrap_or_default();
    let message = SystemMessage::new(id).with(SystemMessageParam::PlayerName(name));
    for to in world.party_members_of(party_of) {
        world.send(to, &message);
    }
}

/// The `checkConditions(player)` of the two floors (they differ in the template, the item and the
/// reach only): a party leader with a party that is near, high enough, free to enter and holds the
/// Seal Breaker.
// NOTE(java-quirk): `getPlayerWorld(player)` is asked for the leader, once per member.
pub(crate) fn check_floor_conditions(
    world: &mut World,
    player: ObjectId,
    template_id: i32,
    min_level: i32,
    seal_breaker: i32,
    now_ms: i64,
) -> bool {
    if world.script_is_gm(player) {
        return true;
    }

    if !world.is_party_leader(player) {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    for member in world.party_members_of(player) {
        let level = world.player(member).map_or(0, |p| p.core.level);
        if level < min_level {
            party_message(
                world,
                player,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                member,
            );
            return false;
        }

        if !in_range_3d(world, 500, player, member) {
            party_message(
                world,
                player,
                SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                member,
            );
            return false;
        }

        if world.player_instance_world(player).is_some() {
            party_message(
                world,
                player,
                SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
                member,
            );
            return false;
        }

        if now_ms < world.instance_time(member, template_id) {
            party_message(world, player, SystemMessageId::C1MayNotReEnterYet, member);
            return false;
        }

        let has_seal_breaker = world.player(member).is_some_and(|p| {
            p.data.items.iter().any(|i| {
                i.item_id == seal_breaker && (i.loc == "INVENTORY" || i.loc == "PAPERDOLL") && i.count >= 1
            })
        });
        if !has_seal_breaker {
            party_message(
                world,
                player,
                SystemMessageId::C1SQuestRequirementIsNotSufficientAndCannotBeEntered,
                member,
            );
            return false;
        }
    }

    true
}

/// The `onEnterInstance(player, world, firstEntrance)` of the two floors.
pub(crate) fn enter_floor(
    world: &mut World,
    player: ObjectId,
    instance_id: i32,
    first_entrance: bool,
    (x, y, z): (i32, i32, i32),
    seal_breaker: i32,
) {
    let teleport = |world: &mut World, who: ObjectId| {
        world.tele_to_location(who, TeleportTarget::at(x, y, z).with_instance(instance_id));
    };
    if first_entrance {
        // `party == null`: just the player.
        let members = world.party_members_of(player);
        for member in members {
            teleport(world, member);
            world.destroy_item_by_item_id(member, seal_breaker, 1, true);
            if let Some(instance_world) = world.instance_world_mut(instance_id) {
                instance_world.add_allowed(member);
            }
        }
    } else {
        teleport(world, player);
    }
}

/// The `onTalk` of the two floors' gatekeeper: `None` and the entrance, or the page that says why
/// not.
pub(crate) fn gatekeeper_talk(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    template_id: i32,
    min_level: i32,
    seal_breaker: i32,
    entry: (i32, i32, i32),
) -> Option<String> {
    let mut htmltext = None;
    if !ctx.is_gm(player) {
        if ctx.world().party_of(player).is_none() {
            htmltext = Some("gk-noparty.htm");
        } else if !ctx.world().is_party_leader(player) {
            htmltext = Some("gk-noleader.htm");
        }
    }

    if htmltext.is_none() {
        let now = ctx.now_ms();
        ctx.world_mut().enter_instance(
            player,
            template_id,
            move |world, player| {
                check_floor_conditions(world, player, template_id, min_level, seal_breaker, now)
            },
            move |world, player, instance_id, first| {
                enter_floor(world, player, instance_id, first, entry, seal_breaker);
            },
        );
    }

    htmltext.map(str::to_owned)
}

/// The `onKill` of the two bosses: the exit is set, the instance ends after the default time and
/// the cube appears.
pub(crate) fn boss_killed(
    ctx: &mut ScriptCtx<'_>,
    npc: l2_world::ScriptNpc,
    exit: (i32, i32, i32),
    cube: (i32, i32, i32, i32),
    cube_id: i32,
) {
    let instance_id = ctx.world().instance_id_of(npc.object_id);
    if instance_id > 0 {
        if let Some(instance) = ctx.world_mut().instance_mut(instance_id) {
            instance.exit_location = Some(exit);
        }
        ctx.world_mut().finish_instance(instance_id);
        ctx.add_spawn(cube_id, cube.0, cube.1, cube.2, cube.3, false, 0, instance_id);
    }
}
