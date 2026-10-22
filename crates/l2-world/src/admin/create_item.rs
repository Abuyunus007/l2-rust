//! The item admin commands: `//itemcreate` (the menu), `//create_item`, `//create_coin`,
//! `//give_item_target`, `//give_item_to_all`, `//delete_item` and `//use_item`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminCreateItem.java
//
// Items are made by `World::inventory_add_item_by_id`, Java's `Inventory.addItem` (stacking, the
// `ADDED` / `MODIFIED` flag, the update packets; no overflow drop, no herb skip, as Java's admin
// commands get). The database rows follow the item list.
//
// `//create_item <id> <amount> <enchant>` sets the enchant level of an enchantable item, clamped to
// the highest level `EnchantItemGroupsData` has a chance for when `OverEnchantProtection` is on
// (`World::max_enchant_of`, M6-ENCHANT-01).
//
// # Left out, with the system that brings it
// - `//use_item` of an item that is not equippable runs its item handler (potions, scrolls, ...),
//   which is M5: nothing happens for those items here.
//
// `//delete_item` and `//use_item` end with `GMViewItemList` (the owner's item window) for the GM.
// `//give_item_target` and `//give_item_to_all` have `confirmDlg="true"` in `AdminCommands.xml`:
// typed as `//command` they wait for the answer of a `ConfirmDlg` (`admin/mod.rs`).

use l2_util::ObjectId;

use super::exp_sp::parse_long;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::inventory::is_equipable;
use crate::objects::ObjectKind;
use crate::player::ItemData;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// The admin item creation page (`AdminHtml.showAdminHtml(player, "itemcreation.htm")`).
const ITEM_CREATION_PAGE: &str = "itemcreation.htm";

/// Java `AdminCreateItem` registrations.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_itemcreate", admin_itemcreate),
        ("admin_create_item", admin_create_item),
        ("admin_create_coin", admin_create_coin),
        ("admin_give_item_target", admin_give_item_target),
        ("admin_give_item_to_all", admin_give_item_to_all),
        ("admin_delete_item", admin_delete_item),
        ("admin_use_item", admin_use_item),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// A `NumberFormatException` of a command argument that the handler catches (Java prints
/// "Specify a valid number.").
struct BadNumber;

/// Java `Integer.parseInt(text)`.
fn int(text: &str) -> Result<i32, BadNumber> {
    parse_int(text).map_err(|_| BadNumber)
}

/// Java `Long.parseLong(text)`.
fn long(text: &str) -> Result<i64, BadNumber> {
    parse_long(text).ok_or(BadNumber)
}

/// `Long.parseLong` where the `NumberFormatException` is not caught: the dispatcher reports it to
/// the GM. (`parse_int` already returns that exception for `Integer.parseInt`.)
fn long_or_exception(text: &str) -> Result<i64, JavaException> {
    parse_long(text).ok_or_else(|| JavaException::number_format(text))
}

/// Java `new StringTokenizer(val)`: the tokens in order.
fn tokens(text: &str) -> Vec<&str> {
    let mut st = Tokens::new(text);
    let mut words = Vec::new();
    while let Some(word) = st.next_token() {
        words.push(word);
    }
    words
}

/// Java `command.substring(begin)`. The handlers whose command word is shorter than the offset
/// (`admin_use_item` and `admin_delete_item`) throw Java's `StringIndexOutOfBoundsException`, which
/// the dispatcher reports.
fn arguments(command: &str, begin: usize) -> Result<&str, JavaException> {
    command.get(begin..).ok_or_else(|| {
        JavaException(format!(
            "java.lang.StringIndexOutOfBoundsException: begin {begin}, end {len}, length {len}",
            len = command.len()
        ))
    })
}

/// Java `AdminCreateItem.getCoinId(name)`: `0` for a name that is not a coin.
fn coin_id(name: &str) -> i32 {
    const COINS: &[(&str, i32)] = &[
        ("adena", 57),
        ("ancientadena", 5575),
        ("festivaladena", 6673),
        ("blueeva", 4355),
        ("goldeinhasad", 4356),
        ("silvershilen", 4357),
        ("bloodypaagrio", 4358),
        ("fantasyislecoin", 13067),
    ];
    COINS
        .iter()
        .find(|(coin, _)| name.eq_ignore_ascii_case(coin))
        .map_or(0, |(_, id)| *id)
}

/// Java `AdminCreateItem.createItem(activeChar, target, id, num)`, and the overload with an
/// enchant level (`enchant` is `Some` for the three-number form of `//create_item`).
fn create_item(world: &mut World, gm: ObjectId, target: ObjectId, id: i32, num: i64, enchant: Option<i32>) {
    let templates = world.item_templates();
    let Some(template) = templates.as_deref().and_then(|t| t.get(id)) else {
        world.send_sys_message(gm, "This item doesn't exist.");
        return;
    };
    if num > 10 && !template.stackable {
        world.send_sys_message(gm, "This item does not stack - Creation aborted.");
        return;
    }
    let name = template.name.clone();

    let added = world.inventory_add_item_by_id(target, id, num);
    if let Some(enchant) = enchant {
        // `(item != null) && item.isEnchantable() && (enchant > 0)`
        match added
            .as_ref()
            .filter(|added| enchant > 0 && world.item_is_enchantable(&added.item, template))
        {
            Some(added) => {
                let level = if world.settings.enchant.over_enchant_protection {
                    enchant.min(world.max_enchant_of(template))
                } else {
                    enchant
                };
                let object_id = added.item.object_id;
                // `setEnchantLevel` only marks the item dirty: the next store writes it.
                world.set_item_enchant_level(target, object_id, level);
            }
            None => world.send_message(gm, "This item is not enchantable, no enchant applied."),
        }
    }
    if gm != target {
        world.send_message(target, &format!("Admin spawned {num} {name} in your inventory."));
    }
    world.send_item_list(target, false);
    let target_name = world.object_name(target).unwrap_or_default();
    world.send_sys_message(
        gm,
        &format!("You have spawned {num} {name}({id}) in {target_name} inventory."),
    );
}

/// Java `AdminCreateItem.onCommand`, `admin_itemcreate`: the creation page.
fn admin_itemcreate(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.show_admin_html(gm, ITEM_CREATION_PAGE);
    Ok(())
}

/// Java `admin_create_item`: `<itemId> [amount] [enchant]`. The page is shown after every branch.
fn admin_create_item(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if create_item_command(world, gm, command).is_err() {
        world.send_sys_message(gm, "Specify a valid number.");
    }
    world.show_admin_html(gm, ITEM_CREATION_PAGE);
    Ok(())
}

fn create_item_command(world: &mut World, gm: ObjectId, command: &str) -> Result<(), BadNumber> {
    // The command word is 17 characters, so the substring always exists.
    let words = tokens(command.get(17..).unwrap_or_default());
    match words.as_slice() {
        [] => {}
        [id] => create_item(world, gm, gm, int(id)?, 1, None),
        [id, num, rest @ ..] => {
            let (id, num) = (int(id)?, long(num)?);
            match rest.first() {
                None => create_item(world, gm, gm, id, num, None),
                Some(enchant) => create_item(world, gm, gm, id, num, Some(int(enchant)?)),
            }
        }
    }
    Ok(())
}

/// Java `admin_create_coin`: `<name> [amount]`, the coin names of `getCoinId`.
fn admin_create_coin(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if create_coin_command(world, gm, command).is_err() {
        world.send_sys_message(gm, "Specify a valid number.");
    }
    world.show_admin_html(gm, ITEM_CREATION_PAGE);
    Ok(())
}

fn create_coin_command(world: &mut World, gm: ObjectId, command: &str) -> Result<(), BadNumber> {
    // The command word is 17 characters, so the substring always exists.
    let words = tokens(command.get(17..).unwrap_or_default());
    match words.as_slice() {
        [name, num] => {
            // An unknown coin name makes no item and no message.
            let id = coin_id(name);
            if id > 0 {
                create_item(world, gm, gm, id, long(num)?, None);
            }
        }
        [name] => create_item(world, gm, gm, coin_id(name), 1, None),
        _ => {}
    }
    Ok(())
}

/// Java `admin_give_item_target`: `<itemId> [amount]` to the targeted player.
fn admin_give_item_target(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(target) = world.target_of(gm).filter(|t| world.is_player(*t)) else {
        // Java returns before the page is shown.
        world.send_sys_message(gm, "Invalid target.");
        return Ok(());
    };
    if give_item_target_command(world, gm, target, command).is_err() {
        world.send_sys_message(gm, "Specify a valid number.");
    }
    world.show_admin_html(gm, ITEM_CREATION_PAGE);
    Ok(())
}

fn give_item_target_command(
    world: &mut World,
    gm: ObjectId,
    target: ObjectId,
    command: &str,
) -> Result<(), BadNumber> {
    // The command word is 22 characters, so the substring always exists.
    let words = tokens(command.get(22..).unwrap_or_default());
    match words.as_slice() {
        [id, num] => create_item(world, gm, target, int(id)?, long(num)?, None),
        [id] => create_item(world, gm, target, int(id)?, 1, None),
        _ => {}
    }
    Ok(())
}

/// Java `admin_give_item_to_all`: `<itemId> [amount]` to every online player but the GM. Not a
/// number the handler catches: a bad number is reported to the GM by the dispatcher.
fn admin_give_item_to_all(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // The command word is 22 characters, so the substring always exists.
    let words = tokens(command.get(22..).unwrap_or_default());
    let (id, num) = match words.as_slice() {
        [id, num] => (parse_int(id)?, long_or_exception(num)?),
        [id] => (parse_int(id)?, 1),
        _ => (0, 0),
    };

    let templates = world.item_templates();
    let Some(template) = templates.as_deref().and_then(|t| t.get(id)) else {
        world.send_sys_message(gm, "This item doesn't exist.");
        return Ok(());
    };
    if num > 10 && !template.stackable {
        world.send_sys_message(gm, "This item does not stack - Creation aborted.");
        return Ok(());
    }
    let name = template.name.clone();

    // Every player in the world is online with a client (a player without one is not in it).
    let mut players: Vec<ObjectId> = world.players.keys().copied().filter(|p| *p != gm).collect();
    players.sort_by_key(|p| p.0);
    let counter = players.len();
    for player in players {
        world.inventory_add_item_by_id(player, id, num);
        world.send_message(player, &format!("Admin spawned {num} {name} in your inventory."));
    }
    world.send_message(gm, &format!("{counter} players rewarded with {name}"));
    Ok(())
}

/// What `(Item) World.findObject(objectId)` gives the item commands.
enum Found {
    /// An item of a character's item list, and the character (`getOwnerId()`).
    Owned { owner: ObjectId, item: ItemData },
    /// A dropped item on the ground: its owner id is 0.
    Unowned,
}

/// Java `(Item) World.findObject(objectId)`: a `NullPointerException` in the command when nothing
/// has the id, a `ClassCastException` when the object is not an item.
// NOTE: the JVM's helpful messages name the expressions and classes; only the exception class is
// reproduced here (as in `combat.rs`).
fn find_item(world: &World, object_id: ObjectId) -> Result<Found, JavaException> {
    let owned = world.players.iter().find_map(|(owner, player)| {
        player
            .data
            .items
            .iter()
            .find(|i| i.object_id == object_id)
            .map(|item| Found::Owned {
                owner: *owner,
                item: item.clone(),
            })
    });
    if let Some(found) = owned {
        return Ok(found);
    }
    match world.objects.get(&object_id).map(|e| e.kind) {
        Some(ObjectKind::Item) => Ok(Found::Unowned),
        Some(_) => Err(JavaException("java.lang.ClassCastException".to_owned())),
        None => Err(JavaException("java.lang.NullPointerException".to_owned())),
    }
}

/// Java `activeChar.sendPacket(new GMViewItemList(player))`: the item window of `owner` for the GM.
fn send_gm_view_item_list(world: &World, gm: ObjectId, owner: ObjectId) {
    let Some(limit) = world.stats(owner).map(|v| v.inventory_limit()) else {
        return;
    };
    let Some(player) = world.players.get(&owner) else {
        return;
    };
    let list =
        crate::enter::gm_view_item_list(&player.data, limit, &*world.data, crate::world::unix_ms_now());
    world.send(gm, &list);
}

/// Java `admin_delete_item <objectId> [amount]`: destroys the amount (all of it when 0) of an
/// item of an online player.
fn admin_delete_item(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = tokens(arguments(command, 18)?);
    let (object_id, count) = match words.as_slice() {
        [id, num] => (parse_int(id)?, long_or_exception(num)?),
        [id] => (parse_int(id)?, 1),
        _ => (0, 0),
    };
    match find_item(world, ObjectId(object_id))? {
        Found::Owned { owner, item } => {
            let count = if count == 0 { item.count } else { count };
            world.destroy_inventory_item(owner, item.object_id, count);
            send_gm_view_item_list(world, gm, owner);
            world.send_sys_message(gm, "Item deleted.");
        }
        Found::Unowned => world.send_sys_message(gm, "Item doesn't have owner."),
    }
    Ok(())
}

/// Java `admin_use_item <objectId>`: equips or unequips an equippable item of an online player.
fn admin_use_item(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let object_id = parse_int(arguments(command, 15)?)?;
    match find_item(world, ObjectId(object_id))? {
        Found::Owned { owner, item } => {
            let templates = world.item_templates();
            let equipable = templates
                .as_deref()
                .and_then(|t| t.get(item.item_id))
                .is_some_and(is_equipable);
            if equipable {
                world.use_equippable_item(owner, item.object_id, false);
            }
            // M5: the item handler of an item that is not equippable (`ItemHandler`) is not
            // ported.
            send_gm_view_item_list(world, gm, owner);
        }
        Found::Unowned => world.send_sys_message(gm, "Item doesn't have owner."),
    }
    Ok(())
}
