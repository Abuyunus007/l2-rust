//! The element admin commands: `//setlh`, `//setlc`, `//setlg`, `//setlb`, `//setll`, `//setlw` and
//! `//setls` set the element of the head, chest, gloves, feet, legs, right hand and left hand
//! piece the target wears (`<element> <value>`, element `fire|water|wind|earth|holy|dark|none`).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminElement.java,
// Java: entity/item/holders/Elementals.java (getElementId, getElementName, toString)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::enter::paperdoll_items;
use crate::objects::ObjectKind;
use crate::world::World;

type Outcome = Result<(), JavaException>;

// Java: `Inventory.PAPERDOLL_*` (the slot index of each body part).
const PAPERDOLL_HEAD: usize = 1;
const PAPERDOLL_RHAND: usize = 5;
const PAPERDOLL_CHEST: usize = 6;
const PAPERDOLL_LHAND: usize = 7;
const PAPERDOLL_GLOVES: usize = 10;
const PAPERDOLL_LEGS: usize = 11;
const PAPERDOLL_FEET: usize = 12;

const USAGE_RANGE: &str = "Usage: //setlh/setlc/setlg/setlb/setll/setlw/setls <element> <value>[0-450]";
const USAGE_FORMAT: &str = "Usage: //setlh/setlc/setlg/setlb/setll/setlw/setls <element>[0-5] <value>[0-450]";

/// Java `AdminElement.ADMIN_COMMANDS`, one handler for the seven words.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let handler: AdminHandler = admin_element;
    for word in [
        "admin_setlh",
        "admin_setlc",
        "admin_setlg",
        "admin_setlb",
        "admin_setll",
        "admin_setlw",
        "admin_setls",
    ] {
        handlers.register(word, handler);
    }
}

/// Java `Elementals.getElementId(name)`: the element by its name, `-1` for anything else.
fn element_id(name: &str) -> i32 {
    match name.to_lowercase().as_str() {
        "fire" => 0,
        "water" => 1,
        "wind" => 2,
        "earth" => 3,
        "holy" => 4,
        "dark" => 5,
        _ => -1,
    }
}

/// Java `Elementals.getElementName(element)`.
fn element_name(element: i32) -> &'static str {
    match element {
        0 => "Fire",
        1 => "Water",
        2 => "Wind",
        3 => "Earth",
        5 => "Dark",
        4 => "Holy",
        _ => "None",
    }
}

/// Java `AdminElement.onCommand`: the body part comes from the command word; `<element> <value>`
/// are checked before the item changes.
fn admin_element(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let slot = if command.starts_with("admin_setlh") {
        PAPERDOLL_HEAD
    } else if command.starts_with("admin_setlc") {
        PAPERDOLL_CHEST
    } else if command.starts_with("admin_setlg") {
        PAPERDOLL_GLOVES
    } else if command.starts_with("admin_setlb") {
        PAPERDOLL_FEET
    } else if command.starts_with("admin_setll") {
        PAPERDOLL_LEGS
    } else if command.starts_with("admin_setlw") {
        PAPERDOLL_RHAND
    } else if command.starts_with("admin_setls") {
        PAPERDOLL_LHAND
    } else {
        return Ok(());
    };
    let args = java_split_space(command);
    // Java: `getElementId(args[1])` and `parseInt(args[2])` in one `try`; a missing word or a bad
    // number is the usage text.
    let parsed = match (args.get(1), args.get(2)) {
        (Some(name), Some(value)) => parse_int(value).ok().map(|v| (element_id(name), v)),
        _ => None,
    };
    let Some((element, value)) = parsed else {
        world.send_sys_message(gm, USAGE_FORMAT);
        return Ok(());
    };
    if !(-1..=5).contains(&element) || !(0..=450).contains(&value) {
        world.send_sys_message(gm, USAGE_RANGE);
        return Ok(());
    }
    set_element(world, gm, element, value, slot);
    Ok(())
}

/// Java `AdminElement.setElement(activeChar, type, value, armorType)`.
fn set_element(world: &mut World, gm: ObjectId, element: i32, value: i32, slot: usize) {
    let target = world.target_of(gm).unwrap_or(gm);
    if world.objects.get(&target).map(|e| e.kind) != Some(ObjectKind::Player) {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return;
    }
    // Java `getPaperdollItem(armorType)` with `getLocationSlot() == armorType`.
    let worn = world.players.get(&target).and_then(|p| {
        paperdoll_items(&p.data)
            .get(slot)
            .copied()
            .flatten()
            .map(|item| (item.object_id, item.item_id))
    });
    let Some((item, item_id)) = worn else {
        return;
    };
    let name = world.data.item_name(item_id).unwrap_or_default();
    let before = elemental_text(world, target, item, element);
    world.unequip_item_in_slot_and_record(target, slot);
    if element == -1 {
        world.clear_item_elemental(item, -1);
    } else {
        world.set_item_elemental(item, element, value);
    }
    let mut changed = world.equip_item_and_record(target, item);
    changed.push(item);
    world.send_changed_items(target, &changed);
    let after = elemental_text(world, target, item, element);

    let player_name = world.object_name(target).unwrap_or_default();
    let gm_name = world.object_name(gm).unwrap_or_default();
    world.send_sys_message(
        gm,
        &format!("Changed elemental power of {player_name}'s {name} from {before} to {after}."),
    );
    if target != gm {
        world.send_message(
            target,
            &format!("{gm_name} has changed the elemental power of your {name} from {before} to {after}."),
        );
    }
}

/// Java `Elementals.toString()` of the element of an item (`Fire +30`), or `None`. Java asks
/// `getElemental(type)` for the element; with type -1 there is never one.
fn elemental_text(world: &World, player: ObjectId, item: ObjectId, element: i32) -> String {
    if element == -1 {
        return "None".to_owned();
    }
    let found = world.players.get(&player).and_then(|p| {
        paperdoll_items(&p.data)
            .into_iter()
            .flatten()
            .find(|i| i.object_id == item)
            .and_then(|i| i.elemental(element))
    });
    match found {
        Some(e) => format!("{} +{}", element_name(e.element), e.value),
        None => "None".to_owned(),
    }
}
