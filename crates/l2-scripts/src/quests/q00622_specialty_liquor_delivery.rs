//! Quest 622: Specialty Liquor Delivery (Jeremy in the Town of Gludin area hands out five special
//! drinks; Boelin, Kuber, Crocus, Naff and Pulin each take one and give a price; Lietta pays).
// Java: dist/game/data/scripts/quests/Q00622_SpecialtyLiquorDelivery/Q00622_SpecialtyLiquorDelivery.java
//
// Pattern: the delivery chain of Q00621 with another order of the talkers (`TALKERS.indexOf(npc)
// + 2` is the cond at which each takes a drink). Boelin takes the first drink at cond 1, the
// reward is the Lietta page (its last band has no upper limit, as in Java).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const JEREMY: i32 = 31521;
const PULIN: i32 = 31543;
const NAFF: i32 = 31544;
const CROCUS: i32 = 31545;
const KUBER: i32 = 31546;
const BOELIN: i32 = 31547;
const LIETTA: i32 = 31267;

// Items
const SPECIAL_DRINK: i32 = 7197;
const SPECIAL_DRINK_PRICE: i32 = 7198;

// Rewards
const QUICK_STEP_POTION: i32 = 734;
const SEALED_RING_OF_AURAKYRA: i32 = 6849;
const SEALED_SANDDRAGONS_EARING: i32 = 6847;
const SEALED_DRAGON_NECKLACE: i32 = 6851;

// Misc
const MIN_LEVEL: i32 = 68;

/// Java `TALKERS`: the talkers in order (cond 2 is the first one).
const TALKERS: [i32; 4] = [KUBER, CROCUS, NAFF, PULIN];

/// Java `Q00622_SpecialtyLiquorDelivery`.
pub struct Q00622SpecialtyLiquorDelivery;

/// Java `TALKERS.indexOf(npc.getId()) + 2`: the cond at which the talker takes the drink.
fn talker_cond(npc_id: i32) -> i32 {
    TALKERS
        .iter()
        .position(|id| *id == npc_id)
        .map_or(0, |i| i as i32)
        + 2
}

impl Script for Q00622SpecialtyLiquorDelivery {
    fn name(&self) -> &'static str {
        "Q00622_SpecialtyLiquorDelivery"
    }

    fn path(&self) -> &'static str {
        "quests/Q00622_SpecialtyLiquorDelivery"
    }

    fn id(&self) -> i32 {
        622
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JEREMY]);
        s.add_talk_id(&[JEREMY, BOELIN, LIETTA]);
        s.add_talk_id(&TALKERS);
        s.register_quest_items(&[SPECIAL_DRINK, SPECIAL_DRINK_PRICE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = None;
        match event {
            "31521-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    ctx.give_items(player, SPECIAL_DRINK, 5);
                    htmltext = Some(event.to_owned());
                }
            }
            "31521-06.html" => {
                if qs.is_cond(ctx, 6) {
                    if ctx.get_quest_items_count(player, SPECIAL_DRINK_PRICE) >= 5 {
                        qs.set_cond(ctx, 7, true);
                        // Java `takeItems(player, -1, SPECIAL_DRINK_PRICE)`: the ids overload.
                        ctx.take_items_many(player, -1, &[SPECIAL_DRINK_PRICE]);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31521-07.html".to_owned());
                    }
                }
            }
            "31547-02.html" => {
                if qs.is_cond(ctx, 1) {
                    if ctx.has_quest_items(player, SPECIAL_DRINK) {
                        qs.set_cond(ctx, 2, true);
                        ctx.take_items(player, SPECIAL_DRINK, 1);
                        ctx.give_items(player, SPECIAL_DRINK_PRICE, 1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31547-03.html".to_owned());
                    }
                }
            }
            "31543-02.html" | "31544-02.html" | "31545-02.html" | "31546-02.html" => {
                let npc = npc?;
                if TALKERS.contains(&npc.id) && qs.is_cond(ctx, talker_cond(npc.id)) {
                    if ctx.has_quest_items(player, SPECIAL_DRINK) {
                        let next = qs.get_cond(ctx) + 1;
                        qs.set_cond(ctx, next, true);
                        ctx.take_items(player, SPECIAL_DRINK, 1);
                        ctx.give_items(player, SPECIAL_DRINK_PRICE, 1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some(format!("{}-03.html", npc.id));
                    }
                }
            }
            "31267-02.html" => {
                if qs.is_cond(ctx, 7) {
                    let rnd = ctx.get_random(1000);
                    if rnd < 800 {
                        ctx.reward_items(player, QUICK_STEP_POTION, 1);
                        ctx.give_adena(player, 18_800, true);
                    } else if rnd < 880 {
                        ctx.reward_items(player, SEALED_RING_OF_AURAKYRA, 1);
                    } else if rnd < 960 {
                        ctx.reward_items(player, SEALED_SANDDRAGONS_EARING, 1);
                    } else {
                        ctx.reward_items(player, SEALED_DRAGON_NECKLACE, 1);
                    }
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            JEREMY => {
                let state = qs.state(ctx);
                if state == CREATED {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31521-01.htm".to_owned()
                    } else {
                        "31521-02.htm".to_owned()
                    };
                } else if state == STARTED {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "31521-04.html".to_owned(),
                        6 => {
                            if ctx.has_quest_items(player, SPECIAL_DRINK_PRICE) {
                                htmltext = "31521-05.html".to_owned();
                            }
                        }
                        7 => {
                            if !ctx.has_quest_items(player, SPECIAL_DRINK) {
                                htmltext = "31521-08.html".to_owned();
                            }
                        }
                        _ => {}
                    }
                } else if state == COMPLETED {
                    htmltext = ctx.get_already_completed_msg(player);
                }
            }
            BOELIN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => {
                            if ctx.get_quest_items_count(player, SPECIAL_DRINK) >= 5 {
                                htmltext = "31547-01.html".to_owned();
                            }
                        }
                        2 => htmltext = "31547-04.html".to_owned(),
                        _ => {}
                    }
                }
            }
            KUBER | CROCUS | NAFF | PULIN => {
                if qs.is_started(ctx) {
                    let cond = talker_cond(npc.id);
                    if qs.is_cond(ctx, cond) && ctx.has_quest_items(player, SPECIAL_DRINK_PRICE) {
                        htmltext = format!("{}-01.html", npc.id);
                    } else if qs.is_cond(ctx, cond + 1) {
                        htmltext = format!("{}-04.html", npc.id);
                    }
                }
            }
            LIETTA => {
                if qs.is_started(ctx) && qs.is_cond(ctx, 7) {
                    htmltext = "31267-01.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00622_SpecialtyLiquorDelivery";

    const NPCS: &str = r#"
        <npc id="31521" type="Folk" level="70" name="Jeremy"/>
        <npc id="31543" type="Folk" level="70" name="Pulin"/>
        <npc id="31544" type="Folk" level="70" name="Naff"/>
        <npc id="31545" type="Folk" level="70" name="Crocus"/>
        <npc id="31546" type="Folk" level="70" name="Kuber"/>
        <npc id="31547" type="Folk" level="70" name="Boelin"/>
        <npc id="31267" type="Folk" level="70" name="Lietta"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7197" type="EtcItem" name="Special Drink"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7198" type="EtcItem" name="Special Drink Price"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="734" type="EtcItem" name="Quick Step Potion"><set name="is_stackable" val="true"/></item>
        <item id="6849" type="Armor" name="Ring of Aurakyra"><set name="bodypart" val="rfinger"/></item>
        <item id="6847" type="Armor" name="Sanddragon's Earing"><set name="bodypart" val="lfinger"/></item>
        <item id="6851" type="Armor" name="Dragon Necklace"><set name="bodypart" val="neck"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00622SpecialtyLiquorDelivery);
        let pages = Pages::of(&Q00622SpecialtyLiquorDelivery)
            .page("31521-01.htm", &["31521-03.htm"])
            .plain(&[
                "31521-02.htm",
                "31521-03.htm",
                "31521-04.html",
                "31521-05.html",
                "31521-07.html",
                "31521-08.html",
            ])
            .page("31521-05.html", &["31521-06.html"])
            .plain(&["31521-06.html"])
            .page("31547-01.html", &["31547-02.html"])
            .plain(&["31547-02.html", "31547-03.html"])
            .page("31546-01.html", &["31546-02.html"])
            .page("31545-01.html", &["31545-02.html"])
            .page("31544-01.html", &["31544-02.html"])
            .page("31543-01.html", &["31543-02.html"])
            .plain(&["31546-02.html", "31545-02.html", "31544-02.html", "31543-02.html"])
            .page("31267-01.html", &["31267-02.html"])
            .plain(&["31267-02.html"])
            .npcs(&[JEREMY, PULIN, NAFF, CROCUS, KUBER, BOELIN, LIETTA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let jeremy = t.spawn(JEREMY, 20);
        (t, jeremy)
    }

    #[test]
    fn a_character_below_level_68_is_sent_away() {
        let (mut t, jeremy) = setup();
        t.enter(1, 67, vec![]);
        assert!(t.quest_page(1, jeremy).contains("31521-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_drinks_go_round_the_talkers_to_lietta_who_ends_it() {
        let (mut t, jeremy) = setup();
        t.enter(1, 68, vec![]);
        let boelin = t.spawn(BOELIN, 30);
        let kuber = t.spawn(KUBER, 30);
        let crocus = t.spawn(CROCUS, 30);
        let naff = t.spawn(NAFF, 30);
        let pulin = t.spawn(PULIN, 30);
        let lietta = t.spawn(LIETTA, 30);

        assert!(t.quest_page(1, jeremy).contains("31521-01.htm"));
        assert!(t.event(1, NAME, "31521-03.htm").contains("31521-03.htm"));
        assert_eq!(t.count(1, SPECIAL_DRINK), 5);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Boelin at cond 1 takes the first drink: cond 2.
        assert!(t.quest_page(1, boelin).contains("31547-01.html"));
        assert!(t.event(1, NAME, "31547-02.html").contains("31547-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Kuber, Crocus, Naff and Pulin each take one drink and move the cond on.
        for (npc, page, cond) in [
            (kuber, "31546", "3"),
            (crocus, "31545", "4"),
            (naff, "31544", "5"),
            (pulin, "31543", "6"),
        ] {
            assert!(t.quest_page(1, npc).contains(&format!("{page}-01.html")));
            assert!(
                t.event(1, NAME, &format!("{page}-02.html"))
                    .contains(&format!("{page}-02.html"))
            );
            assert_eq!(t.take_writes(), [rows::update(NAME, "cond", cond)]);
        }
        assert_eq!(
            (t.count(1, SPECIAL_DRINK), t.count(1, SPECIAL_DRINK_PRICE)),
            (0, 5)
        );

        // Jeremy pays at cond 6, Lietta gives the reward and the quest ends.
        assert!(t.quest_page(1, jeremy).contains("31521-05.html"));
        assert!(t.event(1, NAME, "31521-06.html").contains("31521-06.html"));
        assert_eq!(t.count(1, SPECIAL_DRINK_PRICE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        assert!(t.quest_page(1, lietta).contains("31267-01.html"));
        assert!(t.event(1, NAME, "31267-02.html").contains("31267-02.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
