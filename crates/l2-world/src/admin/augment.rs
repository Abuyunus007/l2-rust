//! `//augment <item id> <option id>`: the equipped item of the target (or the GM) gets the
//! augmentation option, replacing any it had.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminAugment.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::enter::paperdoll_items;
use crate::objects::ObjectKind;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminAugment.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    handlers.register("admin_augment", admin_augment as AdminHandler);
}

/// Java `AdminAugment.onCommand`. Every failure tells the GM why and changes nothing.
fn admin_augment(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // Java `command.trim().split(" ")`: the trim is the whole-text trim of Java (up to U+0020).
    let trimmed = command.trim_matches(|c: char| c <= ' ');
    let split = java_split_space(trimmed);
    if split.len() < 3 {
        world.send_sys_message(gm, "Usage: //augment <item id> <option id>");
        return Ok(());
    }
    let target = world.target_of(gm).unwrap_or(gm);
    if world.objects.get(&target).map(|e| e.kind) != Some(ObjectKind::Player) {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    }
    let Ok(item_id) = parse_int(split[1]) else {
        world.send_sys_message(gm, &format!("No item with id {}", split[1]));
        return Ok(());
    };
    // Java `getPaperdollItemByItemId(itemId)`: the first equipped item with that template.
    let equipped = world.players.get(&target).and_then(|p| {
        paperdoll_items(&p.data)
            .into_iter()
            .flatten()
            .find(|item| item.item_id == item_id)
            .map(|item| item.object_id)
    });
    let Some(item) = equipped else {
        world.send_sys_message(gm, &format!("No equipped item with id {}", split[1]));
        return Ok(());
    };
    let Ok(option_id) = parse_int(split[2]) else {
        world.send_sys_message(gm, &format!("No option with id {}", split[2]));
        return Ok(());
    };
    // Java: `removeAugmentation()` when augmented, then `setAugmentation(new Augmentation(id))`.
    if world.is_item_augmented(item) {
        world.set_item_augmentation(item, None);
    }
    world.set_item_augmentation(item, Some(option_id));
    world.send_item_list(target, false);
    Ok(())
}
