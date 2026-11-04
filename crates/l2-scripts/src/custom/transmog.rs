//! Transmog (`Transmog.ini`): the NPC that changes the look of worn items to the look of items
//! the player has owned (the appearance collection).
// Off by default (`TransmogEnabled = False`): the NPC still stands in the script, its menu answers
// nothing useful because the collection is never filled (the item listener is only added with
// the switch on), exactly as in Java.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use l2_world::custom::transmog::transmog_body_part;
use l2_world::{Script, ScriptCtx, ScriptItem, ScriptNpc, ScriptSetup, World};

const NPC: i32 = 900_009;

/// Java `isValidSlot(slot)`.
fn is_valid_slot(slot: &str) -> bool {
    matches!(
        slot,
        "R_HAND"
            | "L_HAND"
            | "GLOVES"
            | "CHEST"
            | "LEGS"
            | "FEET"
            | "BACK"
            | "LR_HAND"
            | "FULL_ARMOR"
            | "HAIRALL"
            | "HAIR2"
            | "HAIR"
    )
}

/// Java `custom.Transmog.Transmog`.
pub struct Transmog;

impl Script for Transmog {
    fn name(&self) -> &'static str {
        "Transmog"
    }

    fn path(&self) -> &'static str {
        "custom/Transmog"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NPC]);
        s.add_talk_id(&[NPC]);
        s.add_first_talk_id(&[NPC]);
    }

    // The Java constructor adds the listeners only with `TransmogConfig.ENABLE_TRANSMOG`.
    fn setup_with(&self, s: &mut ScriptSetup, world: &World) {
        self.setup(s);
        if world.transmog_enabled() {
            s.set_on_player_item_add();
            s.set_on_enter_world();
            s.set_on_player_logout();
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if event == "menu" {
            return Some("900009-01.html".to_owned());
        } else if event.starts_with("rem") {
            // `event.split("!")`: an event with too few parts is an exception in Java.
            let parts: Vec<&str> = event.split('!').collect();
            let (slot, page) = (*parts.get(1)?, *parts.get(2)?);
            if is_valid_slot(slot) {
                match ctx.world_mut().transmog_unequip_body_slot(player, slot) {
                    Some(item) => {
                        let transmog_id = ctx.world().item_transmog_id(item);
                        if transmog_id > 0 {
                            let cost = i64::from(ctx.world().custom_script_settings().transmog.remove_cost);
                            if cost > 0 {
                                if ctx.world().adena_of(player) < cost {
                                    ctx.world_mut().transmog_equip(player, item);
                                    ctx.world().send(
                                        player,
                                        &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                                    );
                                    menu_page(ctx, &format!("{slot}!{page}"), player);
                                    return None;
                                }

                                ctx.world_mut().reduce_adena(player, cost, true);
                            }

                            ctx.world_mut().transmog_remove_item(item);
                            let name = transmog_name(ctx.world(), transmog_id);
                            ctx.world()
                                .send_message(player, &format!("You have removed your {name} transmog."));
                        }

                        ctx.world_mut().transmog_equip(player, item);
                        ctx.world_mut().broadcast_user_info(player);
                    }
                    None => {
                        ctx.world().send_message(player, "No item equipped in this slot.");
                    }
                }
            }

            menu_page(ctx, &format!("{slot}!{page}"), player);
        } else if event.starts_with("set") {
            let parts: Vec<&str> = event.split('!').collect();
            let (slot, page) = (*parts.get(1)?, *parts.get(2)?);
            // `Integer.parseInt(split[3])`.
            let transmog_id: i32 = parts.get(3)?.parse().ok()?;
            if is_valid_slot(slot) {
                let mask = World::transmog_slot_mask(slot);
                match ctx.world_mut().transmog_unequip_body_slot(player, slot) {
                    Some(item) => {
                        if let Some((name, _, _)) = ctx.world().transmog_item_info(transmog_id) {
                            if ctx
                                .world()
                                .transmog_collection(player, mask)
                                .contains(&transmog_id)
                            {
                                let cost =
                                    i64::from(ctx.world().custom_script_settings().transmog.apply_cost);
                                if cost > 0 {
                                    if ctx.world().adena_of(player) < cost {
                                        ctx.world_mut().transmog_equip(player, item);
                                        ctx.world().send(
                                            player,
                                            &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena),
                                        );
                                        menu_page(ctx, &format!("{slot}!{page}"), player);
                                        return None;
                                    }

                                    ctx.world_mut().reduce_adena(player, cost, true);
                                }

                                ctx.world_mut().transmog_set_item(item, transmog_id);
                                ctx.world().send_message(
                                    player,
                                    &format!("You have set your item transmog to {name}."),
                                );
                            }
                        }

                        ctx.world_mut().transmog_equip(player, item);
                        ctx.world_mut().broadcast_user_info(player);
                    }
                    None => {
                        ctx.world().send_message(player, "No item equipped in this slot.");
                    }
                }
            }

            menu_page(ctx, &format!("{slot}!{page}"), player);
        } else {
            return menu_page(ctx, event, player);
        }

        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }

    /// Java `onPlayerItemAdd`.
    fn on_player_item_add(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, item: ScriptItem) {
        ctx.world_mut().transmog_on_item_added(player, item.id);
    }

    /// Java `onPlayerLogin`.
    fn on_enter_world(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        ctx.world_mut().transmog_request_load(player);
    }

    /// Java `onPlayerLogout`.
    fn on_player_logout(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        ctx.world_mut().transmog_store_and_forget(player);
    }
}

/// `ItemData.getInstance().getTemplate(transmogId).getName()`.
fn transmog_name(world: &World, transmog_id: i32) -> String {
    world
        .transmog_item_info(transmog_id)
        .map(|(name, _, _)| name)
        .unwrap_or_default()
}

/// The last branch of Java's `onEvent`: the menu page of a slot, sent on the community board.
/// `Some("900009-02.html")` when the collection of the slot is empty.
fn menu_page(ctx: &mut ScriptCtx<'_>, event: &str, player: ObjectId) -> Option<String> {
    let split: Vec<&str> = event.split('!').collect();
    let slot = split[0];
    let mask = transmog_body_part(slot).mask();
    let item_ids = ctx.world().transmog_collection(player, mask);
    if item_ids.is_empty() {
        return Some("900009-02.html".to_owned());
    }

    // `split.length > 1 ? Integer.parseInt(split[1]) : 1`.
    let page: i32 = match split.get(1) {
        Some(text) => text.parse().ok()?,
        None => 1,
    };
    let mut total = 0;
    let mut counter = 0;
    let pages = i32::try_from(item_ids.len().div_ceil(16)).unwrap_or(i32::MAX);
    let max_item = page * 16;
    let min_item = max_item - 16;
    let mut content = ctx.get_htm(player, "900009-03.html").unwrap_or_default();
    for item_id in &item_ids {
        if let Some((name, icon, display_id)) = ctx.world().transmog_item_info(*item_id) {
            total += 1;
            if total < min_item || total > max_item {
                continue;
            }

            counter += 1;
            let icon = icon.unwrap_or_else(|| "null".to_owned());
            content = content.replace(
                &format!("%{counter}%"),
                &format!(
                    "<center><img src=\"{icon}\" width=32 height=32><br><a action=\"bypass Script Transmog set!{slot}!{page}!{display_id}\">{name}</a></center>"
                ),
            );
        }
    }

    for i in 1..17 {
        content = content.replace(&format!("%{i}%"), "");
    }

    content = content.replace("%title%", &format!("Transmog ({page} of {pages})"));
    content = content.replace(
        "%previous%",
        &format!(
            "<a action=\"bypass Script Transmog {slot}!{}\">Previous</a>",
            (page - 1).max(1)
        ),
    );
    content = content.replace(
        "%next%",
        &format!(
            "<a action=\"bypass Script Transmog {slot}!{}\">Next</a>",
            pages.min(page + 1)
        ),
    );
    content = content.replace(
        "%remove%",
        &format!("<a action=\"bypass Script Transmog rem!{slot}!{page}\">Remove Transmog</a>"),
    );

    ctx.world_mut().transmog_send_page(player, &content);
    None
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::custom::transmog::TransmogLoad;
    use l2_world::custom::{CustomScriptSettings, TransmogSettings};
    use l2_world::{PersistCommand, WorldCommand};

    use super::*;
    use crate::testing::{ScriptTest, item, names_to};

    const NPCS: &str = r#"<npc id="900009" type="Folk" level="1" name="Zumzi"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="100" type="Armor" name="Red Robe"><set name="bodypart" val="chest"/><set name="icon" val="icon.red"/></item>
        <item id="101" type="Armor" name="Blue Robe"><set name="bodypart" val="chest"/><set name="icon" val="icon.blue"/></item>
        <item id="102" type="Armor" name="Gold Robe"><set name="bodypart" val="chest"/><set name="icon" val="icon.gold"/></item>
        <item id="900" type="EtcItem" name="Scroll"/>"#;
    const DIR: &str = "data/scripts/custom/Transmog";
    const PLAYER: i32 = 1;
    const WORN: i32 = 500;

    fn settings(enabled: bool, apply_cost: i32, remove_cost: i32) -> CustomScriptSettings {
        CustomScriptSettings {
            transmog: TransmogSettings {
                enabled,
                apply_cost,
                remove_cost,
                banned_item_ids: [102].into_iter().collect(),
                ..TransmogSettings::default()
            },
            ..CustomScriptSettings::default()
        }
    }

    fn scene(enabled: bool, apply_cost: i32, remove_cost: i32) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Transmog);
        let pages: Vec<(String, String)> = [
            (
                "900009-03.html",
                "<html>%title%<br>%1%%2%%3%<br>%previous% %next% %remove%</html>",
            ),
            ("900009-01.html", "<html>menu</html>"),
            ("900009-02.html", "<html>empty</html>"),
        ]
        .iter()
        .map(|(n, t)| (format!("{DIR}/{n}"), (*t).to_owned()))
        .collect();
        let refs: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new_with(registry, NPCS, ITEMS, &refs, |world, _| {
            world.set_custom_script_settings(settings(enabled, apply_cost, remove_cost));
        });
        let mut worn = item(WORN, 100, 1);
        worn.loc = "PAPERDOLL".to_owned();
        worn.loc_data = 6;
        t.enter(PLAYER, 40, vec![worn, item(501, 57, 10_000)]);
        t.take();
        t
    }

    fn event(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut answer = None;
        t.world.run_as_script("Transmog", |ctx| {
            answer = Transmog.on_event(ctx, event, None, Some(ObjectId(PLAYER)));
        });
        answer
    }

    fn commands(t: &ScriptTest) -> Vec<PersistCommand> {
        t.take_boss()
    }

    #[test]
    fn switched_off_nothing_is_collected_and_nothing_is_asked_of_the_database() {
        let mut t = scene(false, 0, 0);
        t.ticks(3);
        assert!(commands(&t).is_empty(), "no read at the login");
        t.world
            .run_as_script("Transmog", |ctx| ctx.give_items(ObjectId(PLAYER), 101, 1));
        assert!(t.world.transmog_collection(ObjectId(PLAYER), 0x400).is_empty());
        // The menu answers: the pages of an empty collection.
        assert_eq!(event(&mut t, "menu").as_deref(), Some("900009-01.html"));
        assert_eq!(event(&mut t, "CHEST").as_deref(), Some("900009-02.html"));
        // No look is changed, whatever the id.
        assert_eq!(t.world.transmog_display_id(ObjectId(WORN)), None);
    }

    #[test]
    fn the_login_asks_for_the_rows_and_the_answer_fills_the_collection() {
        let mut t = scene(true, 0, 0);
        t.enter(2, 40, vec![item(600, 100, 1)]);
        t.ticks(3);
        let asked = commands(&t);
        assert!(
            asked.iter().any(|c| matches!(c,
                PersistCommand::TransmogLoadRequest { owner, item_object_ids, .. }
                    if owner == "2" && item_object_ids == &[600])),
            "{asked:?}"
        );
        t.world.handle(WorldCommand::TransmogLoaded {
            player: ObjectId(2),
            result: Ok(TransmogLoad {
                collection: vec![101, 102, 900, 4_000_000],
                items: vec![(600, 101)],
            }),
        });
        // Ids without a template are dropped; the rest is listed by body part.
        assert_eq!(t.world.transmog_collection(ObjectId(2), 0x400), [101, 102]);
        // The item shows the look of 101.
        assert_eq!(t.world.transmog_display_id(ObjectId(600)), Some(101));
    }

    #[test]
    fn new_items_join_the_collection_once_banned_and_etc_items_do_not() {
        let mut t = scene(true, 0, 0);
        t.world.run_as_script("Transmog", |ctx| {
            ctx.give_items(ObjectId(PLAYER), 101, 1);
            ctx.give_items(ObjectId(PLAYER), 102, 1); // banned
            ctx.give_items(ObjectId(PLAYER), 900, 1); // an etc item
            ctx.give_items(ObjectId(PLAYER), 101, 1); // again
        });
        assert_eq!(t.world.transmog_collection(ObjectId(PLAYER), 0x400), [101]);
        // One whisper "[Transmog]" for the one new look.
        let out = t.take();
        assert_eq!(names_to(&out, PLAYER).iter().filter(|n| *n == "SAY2").count(), 1);
    }

    #[test]
    fn the_menu_lists_the_looks_and_set_and_remove_change_the_worn_item() {
        let mut t = scene(true, 50, 20);
        t.world.run_as_script("Transmog", |ctx| {
            ctx.give_items(ObjectId(PLAYER), 101, 1);
        });
        t.take();
        // The menu page of the chest slot goes to the community board.
        assert_eq!(event(&mut t, "CHEST!1"), None);
        assert!(names_to(&t.take(), PLAYER).iter().any(|n| n == "SHOW_BOARD"));

        // Set: the worn chest item takes the look of 101 for 50 adena; one row is written.
        commands(&t);
        assert_eq!(event(&mut t, "set!CHEST!1!101"), None);
        assert_eq!(t.world.item_transmog_id(ObjectId(WORN)), 101);
        assert_eq!(t.world.transmog_display_id(ObjectId(WORN)), Some(101));
        assert_eq!(t.count(PLAYER, 57), 9_950);
        assert!(commands(&t).iter().any(|c| matches!(
            c,
            PersistCommand::ItemTransmog {
                object_id: WORN,
                transmog_id: Some(101)
            }
        )));
        // It is worn again.
        assert!(t.world.paperdoll_object(ObjectId(PLAYER), 6).is_some());

        // A look that is not in the collection changes nothing and costs nothing.
        assert_eq!(event(&mut t, "set!CHEST!1!102"), None);
        assert_eq!(t.world.item_transmog_id(ObjectId(WORN)), 101);
        assert_eq!(t.count(PLAYER, 57), 9_950);

        // Remove: 20 adena, the row is deleted.
        assert_eq!(event(&mut t, "rem!CHEST!1"), None);
        assert_eq!(t.world.item_transmog_id(ObjectId(WORN)), 0);
        assert_eq!(t.count(PLAYER, 57), 9_930);
        assert!(commands(&t).iter().any(|c| matches!(
            c,
            PersistCommand::ItemTransmog {
                object_id: WORN,
                transmog_id: None
            }
        )));
    }

    #[test]
    fn without_the_adena_the_item_stays_as_it_was() {
        let mut t = scene(true, 20_000, 0);
        t.world
            .run_as_script("Transmog", |ctx| ctx.give_items(ObjectId(PLAYER), 101, 1));
        commands(&t);
        assert_eq!(event(&mut t, "set!CHEST!1!101"), None);
        assert_eq!(t.world.item_transmog_id(ObjectId(WORN)), 0);
        assert_eq!(t.count(PLAYER, 57), 10_000);
        assert!(
            commands(&t)
                .iter()
                .all(|c| !matches!(c, PersistCommand::ItemTransmog { .. }))
        );
        assert!(
            t.world.paperdoll_object(ObjectId(PLAYER), 6).is_some(),
            "worn again"
        );
    }

    #[test]
    fn the_collection_is_stored_when_the_player_leaves() {
        let mut t = scene(true, 0, 0);
        t.world
            .run_as_script("Transmog", |ctx| ctx.give_items(ObjectId(PLAYER), 101, 1));
        commands(&t);
        t.world
            .run_as_script("Transmog", |ctx| Transmog.on_player_logout(ctx, ObjectId(PLAYER)));
        let stored = commands(&t);
        assert!(
            stored.iter().any(|c| matches!(c,
                PersistCommand::TransmogSaveCollection { owner, item_ids } if owner == "1" && item_ids == &[101])),
            "{stored:?}"
        );
        assert!(t.world.transmog_collection(ObjectId(PLAYER), 0x400).is_empty());
    }
}
