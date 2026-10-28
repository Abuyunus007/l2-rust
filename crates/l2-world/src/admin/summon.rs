//! The summon admin commands: `//summon <id> [count]` (an item, or an NPC for ids from 1000000),
//! `//summon_info`, `//unsummon`, `//summon_setlvl <level>` and `//show_pet_inv`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSummon.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEditChar.java (admin_summon_info,
// gatherSummonInfo, admin_unsummon, admin_summon_setlvl, admin_show_pet_inv)
//
// # Left out, with the system that brings it
// - `//summon_setlvl` and `//show_pet_inv` on a pet: the pet's level table and inventory come with
//   M6-PET-01. A pet target does nothing yet; a servitor or other target gets Java's message.
// - The food and load lines of `//summon_info` for a pet: M6-PET-01 (shown as N/A).

use l2_util::ObjectId;
use tracing::warn;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::movement::Intention;
use crate::summon::{Summon, SummonKind};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminSummon` and the summon words of `AdminEditChar`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_summon", admin_summon),
        ("admin_summon_info", admin_summon_info),
        ("admin_unsummon", admin_unsummon),
        ("admin_summon_setlvl", admin_summon_setlvl),
        ("admin_show_pet_inv", admin_show_pet_inv),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The target when it is a summon (Java `target.isSummon()`).
fn target_summon(world: &World, gm: ObjectId) -> Option<ObjectId> {
    world.target_of(gm).filter(|t| world.is_summon(*t))
}

/// Java `AdminSummon.onCommand`: `//summon <id> [count]`. Ids below 1000000 are items (the
/// `admin_create_item` command), the others are NPCs spawned once (`admin_spawn_once`, id minus
/// 1000000). The inner command runs with its confirmation, as Java calls it with `useConfirm`.
fn admin_summon(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    if words.len() < 2 {
        world.send_sys_message(gm, "Usage: //summon <id> [count]");
        return Ok(());
    }
    // Java `Integer.parseInt(data[1])` and `Long.parseLong(data[2])`, both in one `try`.
    let parsed = parse_int(words[1]).ok().and_then(|id| {
        let count = match words.get(2) {
            Some(text) => text.parse::<i64>().ok()?,
            None => 1,
        };
        Some((id, count))
    });
    let Some((mut id, count)) = parsed else {
        world.send_sys_message(gm, "Incorrect format for command 'summon'");
        return Ok(());
    };

    let sub_command = if id < 1_000_000 {
        "admin_create_item"
    } else {
        world.send_sys_message(
            gm,
            "This is only a temporary spawn.  The mob(s) will NOT respawn.",
        );
        id -= 1_000_000;
        "admin_spawn_once"
    };
    if id > 0 && count > 0 {
        world.admin_command(gm, &format!("{sub_command} {id} {count}"), true);
    }
    Ok(())
}

/// Java `AdminEditChar.onCommand`, `admin_summon_info`: the summon of the target in `petinfo.htm`.
fn admin_summon_info(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(summon) = target_summon(world, gm) else {
        world.send_sys_message(gm, "Invalid target.");
        return Ok(());
    };
    gather_summon_info(world, summon, gm);
    Ok(())
}

/// Java `AdminEditChar.gatherSummonInfo(target, activeChar)`.
fn gather_summon_info(world: &mut World, summon: ObjectId, gm: ObjectId) {
    let Some(data) = world.summon(summon) else {
        return;
    };
    let owner = data.owner;
    let is_pet = data.kind == SummonKind::Pet;
    let name = data.name.clone();
    let exp = data.exp;
    let class = class_name(data);
    let owner_name = world.object_name(owner).unwrap_or_default();

    let level = world.stats(summon).map_or(0, |v| v.level());
    let (current_hp, current_mp) = world.core_of(summon).map_or((0.0, 0.0), |c| (c.cur_hp, c.cur_mp));
    let (max_hp, max_mp) = world.stats(summon).map_or((0, 0), |v| (v.max_hp(), v.max_mp()));
    // Java `Summon.getKarma()`: the owner's karma.
    let karma = world.karma_of_player(owner);
    let race = world
        .summon_template(summon)
        .map_or_else(String::new, |t| t.race.name().to_owned());
    // Java `target.getAI().getIntention().name()`.
    let ai = intention_name(world.intention_of(summon));

    // Java `asPlayer().getObjectId()`: the owner's id, for a pet's inventory link.
    let inventory = if is_pet {
        format!(" <a action=\"bypass admin_show_pet_inv {}\">view</a>", owner.0)
    } else {
        "none".to_owned()
    };
    // Only a pet has food and a load.
    let (food, load) = if is_pet {
        (
            format!("{}/{}", world.pet_current_fed(summon), world.pet_max_fed(summon)),
            format!(
                "{}/{}",
                world.pet_total_weight(owner),
                world.stats(summon).map_or(0, |v| v.max_load())
            ),
        )
    } else {
        ("N/A".to_owned(), "N/A".to_owned())
    };
    let owner_link = format!(" <a action=\"bypass -h admin_character_info {owner_name}\">{owner_name}</a>");
    let values = [
        ("%name%", if name.is_empty() { "N/A".to_owned() } else { name }),
        ("%level%", level.to_string()),
        ("%exp%", exp.to_string()),
        ("%owner%", owner_link),
        ("%class%", class.to_owned()),
        ("%ai%", ai.to_owned()),
        ("%hp%", format!("{}/{max_hp}", current_hp as i32)),
        ("%mp%", format!("{}/{max_mp}", current_mp as i32)),
        ("%karma%", karma.to_string()),
        ("%race%", race),
        ("%inv%", inventory),
        ("%food%", food),
        ("%load%", load),
    ];
    let mut html = world.admin_html_file(gm, "data/html/admin/petinfo.htm");
    for (pattern, value) in &values {
        html.replace(pattern, value);
    }
    world.send_admin_html(gm, &html);
}

/// Java `getClass().getSimpleName()` of the summon: `Servitor` or `Pet`.
fn class_name(summon: &Summon) -> &'static str {
    match summon.kind {
        SummonKind::Servitor => "Servitor",
        SummonKind::Pet => "Pet",
    }
}

/// Java `Intention.name()`: the constant name (`IDLE`; the old `CtrlIntention` had `AI_INTENTION_`).
fn intention_name(intention: Intention) -> &'static str {
    match intention {
        Intention::Idle => "IDLE",
        Intention::Active => "ACTIVE",
        Intention::Rest => "REST",
        Intention::Attack => "ATTACK",
        Intention::Cast => "CAST",
        Intention::MoveTo => "MOVE_TO",
        Intention::Follow => "FOLLOW",
        Intention::PickUp => "PICK_UP",
        Intention::Interact => "INTERACT",
    }
}

/// Java `AdminEditChar.onCommand`, `admin_unsummon`: the target summon leaves its owner.
fn admin_unsummon(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    match target_summon(world, gm) {
        Some(summon) => {
            let owner = world.summon_owner(summon);
            world.unsummon(summon, owner);
        }
        None => world.send_sys_message(gm, "Usable only with Pets/Summons"),
    }
    Ok(())
}

/// Java `AdminEditChar.onCommand`, `admin_summon_setlvl <level>`: the pet's exp goes to the exp of
/// that level. Only pets have that; the pet level table is M6-PET-01.
fn admin_summon_setlvl(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let pet =
        target_summon(world, gm).filter(|s| world.summon(*s).is_some_and(|d| d.kind == SummonKind::Pet));
    if let Some(pet) = pet {
        // `Integer.parseInt(command.substring(20))`: a failure is only logged.
        match command.get(20..).map(parse_int) {
            Some(Ok(level)) => {
                let old_exp = world.summon(pet).map_or(0, |s| s.exp);
                let new_exp = world.pet_exp_for_level(pet, level);
                if old_exp > new_exp {
                    world.pet_remove_exp(pet, old_exp - new_exp);
                } else if old_exp < new_exp {
                    world.pet_add_exp(pet, new_exp - old_exp);
                }
            }
            _ => warn!("Problem with AdminEditChar: bad level in `{command}`"),
        }
    } else {
        world.send_sys_message(gm, "Usable only with Pets");
    }
    Ok(())
}

/// Java `AdminEditChar.onCommand`, `admin_show_pet_inv [objectId]`: the pet's inventory. The id is
/// a pet's object id (no fallback to the target when it is not a pet, as in Java).
fn admin_show_pet_inv(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let target = match command.get(19..).map(parse_int) {
        Some(Ok(id)) => {
            let pet = ObjectId(id);
            world.is_pet(pet).then_some(pet)
        }
        _ => world.target_of(gm),
    };
    match target.filter(|t| world.is_pet(*t)) {
        Some(pet) => send_pet_view_item_list(world, gm, pet),
        None => world.send_sys_message(gm, "Usable only with Pets"),
    }
    Ok(())
}

/// Java `new GMViewItemList(pet)`: the pet's name, `Pet.getInventoryLimit()` and the items of its
/// inventory.
fn send_pet_view_item_list(world: &World, gm: ObjectId, pet: ObjectId) {
    let Some(owner) = world.summon_owner(pet) else {
        return;
    };
    let player_name = world
        .summon(pet)
        .map(|s| s.name.clone())
        .filter(|n| !n.is_empty())
        .or_else(|| world.object_name(pet))
        .unwrap_or_default();
    let list = l2_protocol_game::server::GmViewItemList {
        player_name,
        limit: world.pet_inventory_limit(),
        items: world
            .pet_items(owner)
            .map(|item| world.update_entry(item, 0).item)
            .collect(),
    };
    world.send(gm, &list);
}
