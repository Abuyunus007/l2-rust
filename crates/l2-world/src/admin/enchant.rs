//! `//enchant`: the page that sets the enchant level of the items a character wears, and the
//! `//seteh` .. `//setbe` commands behind its buttons.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEnchant.java
//
// # Left out
// - The element buttons of the page (`admin_setlh` and the like) belong to `AdminElement`, not to
//   this handler; they are not registered here, so they answer "does not exist".

use l2_data::item::TYPE2_ACCESSORY;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// The page.
const ENCHANT_PAGE: &str = "enchant.htm";

/// Java `AdminEnchant.ADMIN_COMMANDS` with the paperdoll slot each one sets.
const SLOT_COMMANDS: [(&str, usize); 15] = [
    ("admin_seteh", paperdoll::HEAD),
    ("admin_setec", paperdoll::CHEST),
    ("admin_seteg", paperdoll::GLOVES),
    ("admin_setel", paperdoll::LEGS),
    ("admin_seteb", paperdoll::FEET),
    ("admin_setew", paperdoll::RHAND),
    ("admin_setes", paperdoll::LHAND),
    ("admin_setle", paperdoll::LEAR),
    ("admin_setre", paperdoll::REAR),
    ("admin_setlf", paperdoll::LFINGER),
    ("admin_setrf", paperdoll::RFINGER),
    ("admin_seten", paperdoll::NECK),
    ("admin_setun", paperdoll::UNDER),
    ("admin_setba", paperdoll::CLOAK),
    ("admin_setbe", paperdoll::BELT),
];

/// Java `AdminEnchant` registrations.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    handlers.register("admin_enchant", admin_enchant as AdminHandler);
    for (command, _) in SLOT_COMMANDS {
        handlers.register(command, admin_set_enchant as AdminHandler);
    }
}

/// The paperdoll slot of a command word (Java's `startsWith` chain).
fn slot_of(command: &str) -> Option<usize> {
    SLOT_COMMANDS
        .iter()
        .find(|(word, _)| command.starts_with(word))
        .map(|&(_, slot)| slot)
}

/// Java `command.substring(12)` (characters, not bytes): `None` is a
/// `StringIndexOutOfBoundsException`, a command shorter than 12 characters.
fn substring_12(command: &str) -> Option<String> {
    (command.chars().count() >= 12).then(|| command.chars().skip(12).collect())
}

/// Java `AdminEnchant.onCommand`, `admin_enchant`: the page.
fn admin_enchant(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.show_admin_html(gm, ENCHANT_PAGE);
    Ok(())
}

/// Java `AdminEnchant.onCommand`, the slot words: `<word> <level>`, then the page again.
fn admin_set_enchant(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if let Some(slot) = slot_of(command) {
        match substring_12(command) {
            None => world.send_sys_message(gm, "Please specify a new enchant value."),
            Some(text) => match parse_int(&text) {
                Err(_) => world.send_sys_message(gm, "Please specify a valid new enchant value."),
                // check value
                Ok(ench) if !(0..=65535).contains(&ench) => {
                    world.send_sys_message(gm, "You must set the enchant level to be between 0-65535.");
                }
                Ok(ench) => set_enchant(world, gm, ench, slot),
            },
        }
    }
    // Show the enchant menu after an action.
    world.show_admin_html(gm, ENCHANT_PAGE);
    Ok(())
}

/// Java `AdminEnchant.setEnchant(activeChar, ench, slot)`.
fn set_enchant(world: &mut World, gm: ObjectId, ench: i32, slot: usize) {
    // Get the target.
    let player = match world.target_of(gm) {
        Some(target) => world.acting_player(target),
        None => Some(gm),
    };
    let Some(player) = player else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return;
    };

    // Only attempt to enchant if there is a weapon equipped.
    let Some(object_id) = world.paperdoll_object(player, slot) else {
        return;
    };
    let Some(item) = world.inventory_item(player, object_id).cloned() else {
        return;
    };
    // `getLocationSlot() == slot`
    if usize::try_from(item.loc_data).ok() != Some(slot) {
        return;
    }
    let Some(template) = world
        .item_templates()
        .and_then(|templates| templates.get(item.item_id).cloned())
    else {
        return;
    };

    let cur_enchant = item.enchant_level;

    // Set enchant value.
    let mut enchant = ench;
    if world.settings.enchant.over_enchant_protection
        && !world.is_gm(player)
        && let Some(groups) = world.enchant.groups.clone()
    {
        let (kind, max) = if template.is_weapon() {
            ("weapon", groups.max_weapon_enchant())
        } else if template.type2 == TYPE2_ACCESSORY {
            ("accessory", groups.max_accessory_enchant())
        } else {
            ("armor", groups.max_armor_enchant())
        };
        if enchant > max {
            world.send_sys_message(gm, &format!("Maximum enchantment for {kind} items is {max}."));
            enchant = max;
        }
    }

    let mut changed = Vec::new();
    world.set_paperdoll_item(player, slot, None, &mut changed);
    world.set_item_enchant_level(player, object_id, enchant);
    world.equip_item(player, object_id, &mut changed);

    // Send packets.
    if let Some(entry) = world.modified_entry(player, object_id) {
        world.send_inventory_update(player, entry);
    }
    world.broadcast_user_info(player);

    // Information.
    let player_name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(
        gm,
        &format!(
            "Changed enchantment of {player_name}'s {} from {cur_enchant} to {enchant}.",
            template.name
        ),
    );
    world.send_message(
        player,
        &format!(
            "Admin has changed the enchantment of your {} from {cur_enchant} to {enchant}.",
            template.name
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_has_its_slot() {
        assert_eq!(slot_of("admin_seteh 6"), Some(paperdoll::HEAD));
        assert_eq!(slot_of("admin_setbe"), Some(paperdoll::BELT));
        assert_eq!(slot_of("admin_setle 4"), Some(paperdoll::LEAR));
        assert_eq!(slot_of("admin_other"), None);
        assert_eq!(SLOT_COMMANDS.len(), 15);
    }

    #[test]
    fn the_value_starts_at_the_twelfth_character() {
        // "admin_seteh" is 11 characters: no value at all.
        assert_eq!(substring_12("admin_seteh"), None);
        assert_eq!(substring_12("admin_seteh ").as_deref(), Some(""));
        assert_eq!(substring_12("admin_seteh 12").as_deref(), Some("12"));
        // Characters, not bytes.
        assert_eq!(substring_12("admin_setehé é").as_deref(), Some("é"));
    }
}
