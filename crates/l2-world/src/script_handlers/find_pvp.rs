//! The `FindPvP` NPC bypass: the NPC sends the player to the busiest flagged player.
// Java: dist/game/data/scripts/handlers/bypass/npc/FindPvP.java
//
// Not modelled: `player.setSpawnProtection(true)` (the world has no spawn protection yet, see
// `status.rs`).

use std::collections::BTreeMap;

use l2_protocol_game::server::{CreatureSay, SayBody, SenderField};
use l2_util::ObjectId;

use crate::bypass::BypassHandlers;
use l2_protocol_game::server::ChatType;
use crate::teleport::TeleportTarget;
use crate::world::World;
use crate::zones::ZoneId;

/// Registers the handler of this module (called by `BypassHandlers::default`).
pub(crate) fn register(handlers: &mut BypassHandlers) {
    handlers.register("FindPvP", find_pvp_handler);
}

/// Java `FindPvP.onCommand`.
fn find_pvp_handler(world: &mut World, player: ObjectId, _command: &str, target: Option<ObjectId>) -> bool {
    let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) else {
        return false;
    };
    if !world.script_handlers.settings.enable_find_pvp {
        return false;
    }
    let npc_name = world
        .npcs
        .get(&npc)
        .and_then(|n| world.npc_world.template(n.npc_id))
        .map(|t| t.name.clone())
        .unwrap_or_default();

    // NOTE: Java walks `World.getPlayers()`, a hash map; the world walks the object ids in order
    // so the same input gives the same output (D-13).
    let mut ids: Vec<ObjectId> = world.players.keys().copied().collect();
    ids.sort_unstable();
    let mut most_pvp: Option<ObjectId> = None;
    let mut max = -1_i32;
    for plr in ids {
        if world.pvp_flag_of(plr) == 0
            || world.instance_id_of(plr) != 0
            || world.is_gm(plr)
            || world.is_inside_zone(plr, ZoneId::Peace)
            || world.is_inside_zone(plr, ZoneId::Siege)
            || world.is_inside_zone(plr, ZoneId::NoSummonFriend)
        {
            continue;
        }
        let count = world
            .visible_players(plr)
            .into_iter()
            .filter(|pl| world.pvp_flag_of(*pl) > 0 && !world.is_inside_zone(*pl, ZoneId::Peace))
            .count() as i32;
        if count > max {
            max = count;
            most_pvp = Some(plr);
        }
    }

    let whisper = |text: &str| CreatureSay {
        sender_object_id: 0,
        chat_type: ChatType::Whisper,
        sender: SenderField::Name(npc_name.clone()),
        message_id: -1,
        body: SayBody::Text(text.to_owned()),
    };
    let Some(most_pvp) = most_pvp else {
        world.send(
            player,
            &whisper("Sorry, I can't find anyone in flag status right now."),
        );
        return false;
    };

    // Check if the player's clan is already outnumbering the PvP.
    if world.clan_of(player).is_some() {
        let ally_key = |world: &World, id: ObjectId| match world.ally_id_of(id) {
            0 => world.clan_id_of(id),
            ally => ally,
        };
        let mut numbers: BTreeMap<i32, i32> = BTreeMap::new();
        let own = ally_key(world, player);
        numbers.insert(own, 1);
        for known in world.visible_players(most_pvp) {
            let key = ally_key(world, known);
            if key != 0 {
                *numbers.entry(key).or_insert(0) += 1;
            }
        }
        let mut biggest_ally_id = 0;
        let mut biggest_amount = 2;
        for (key, amount) in &numbers {
            if *amount > biggest_amount {
                biggest_ally_id = *key;
                biggest_amount = *amount;
            }
        }
        if biggest_ally_id == own {
            world.send(
                player,
                &whisper(
                    "Sorry, your clan/ally is outnumbering the place already so you can't move there.",
                ),
            );
            return true;
        }
    }

    let Some((x, y, z)) = world.position_of(most_pvp) else {
        return false;
    };
    let dx = world.rng().below(300);
    let dy = world.rng().below(300);
    world.tele_to_location(player, TeleportTarget::at(x + dx - 150, y + dy - 150, z));
    if !world.is_gm(player) {
        let lasts = world.world_ms() + i64::from(world.settings.pvp.pvp_pvp_time_ms);
        world.set_pvp_flag_lasts(player, lasts);
        world.start_pvp_flag(player);
    }
    false
}
