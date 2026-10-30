//! Quest 621: Egg Delivery (Jeremy in the Town of Gludin area hands out five boiled eggs; Pulin,
//! Naff, Crocus, Kuber and Boelin each take one and give a price; Valentine pays for the last).
// Java: dist/game/data/scripts/quests/Q00621_EggDelivery/Q00621_EggDelivery.java
//
// Pattern: a delivery chain (`docs/design/scripts.md`, the delivery pattern), with a list of
// talkers whose step is `TALKERS.indexOf(npc) + 2`: each talker's page gives the price and the
// cond goes up by one. Jeremy pays out at cond 6 and Valentine at cond 7.

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
const VALENTINE: i32 = 31584;

// Items
const BOILED_EGG: i32 = 7195;
const EGG_PRICE: i32 = 7196;

// Misc
const MIN_LEVEL: i32 = 68;

// Reward
const QUICK_STEP_POTION: i32 = 734;
const SEALED_RING_OF_AURAKYRA: i32 = 6849;
const SEALED_SANDDRAGONS_EARING: i32 = 6847;
const SEALED_DRAGON_NECKLACE: i32 = 6851;

/// Java `TALKERS`: the talkers in order (cond 2 is the first one).
const TALKERS: [i32; 4] = [NAFF, CROCUS, KUBER, BOELIN];

/// Java `Q00621_EggDelivery`.
pub struct Q00621EggDelivery;

impl Script for Q00621EggDelivery {
    fn name(&self) -> &'static str {
        "Q00621_EggDelivery"
    }

    fn path(&self) -> &'static str {
        "quests/Q00621_EggDelivery"
    }

    fn id(&self) -> i32 {
        621
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JEREMY]);
        s.add_talk_id(&[JEREMY, PULIN, VALENTINE]);
        s.add_talk_id(&TALKERS);
        s.register_quest_items(&[BOILED_EGG, EGG_PRICE]);
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
                    ctx.give_items(player, BOILED_EGG, 5);
                    htmltext = Some(event.to_owned());
                }
            }
            "31521-06.html" => {
                if qs.is_cond(ctx, 6) {
                    if ctx.get_quest_items_count(player, EGG_PRICE) >= 5 {
                        qs.set_cond(ctx, 7, true);
                        ctx.take_items(player, EGG_PRICE, -1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31521-07.html".to_owned());
                    }
                }
            }
            "31543-02.html" => {
                if qs.is_cond(ctx, 1) {
                    if ctx.has_quest_items(player, BOILED_EGG) {
                        qs.set_cond(ctx, 2, true);
                        ctx.take_items(player, BOILED_EGG, 1);
                        ctx.give_items(player, EGG_PRICE, 1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("31543-03.html".to_owned());
                    }
                }
            }
            "31544-02.html" | "31545-02.html" | "31546-02.html" | "31547-02.html" => {
                let Some(npc) = npc else {
                    return None;
                };
                if TALKERS.contains(&npc.id) && qs.is_cond(ctx, talker_cond(npc.id)) {
                    if ctx.has_quest_items(player, BOILED_EGG) {
                        let next = qs.get_cond(ctx) + 1;
                        qs.set_cond(ctx, next, true);
                        ctx.take_items(player, BOILED_EGG, 1);
                        ctx.give_items(player, EGG_PRICE, 1);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some(format!("{}-03.html", npc.id));
                    }
                }
            }
            "31584-02.html" => {
                if qs.is_cond(ctx, 7) {
                    let rnd = ctx.get_random(1000);
                    if rnd < 800 {
                        ctx.reward_items(player, QUICK_STEP_POTION, 1);
                        ctx.give_adena(player, 18_800, true);
                    } else if rnd < 880 {
                        ctx.reward_items(player, SEALED_RING_OF_AURAKYRA, 1);
                    } else if rnd < 960 {
                        ctx.reward_items(player, SEALED_SANDDRAGONS_EARING, 1);
                    } else if rnd < 1000 {
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
                            if ctx.has_quest_items(player, EGG_PRICE) {
                                htmltext = "31521-05.html".to_owned();
                            }
                        }
                        7 => {
                            if !ctx.has_quest_items(player, BOILED_EGG) {
                                htmltext = "31521-08.html".to_owned();
                            }
                        }
                        _ => {}
                    }
                } else if state == COMPLETED {
                    htmltext = ctx.get_already_completed_msg(player);
                }
            }
            PULIN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => {
                            if ctx.get_quest_items_count(player, BOILED_EGG) >= 5 {
                                htmltext = "31543-01.html".to_owned();
                            }
                        }
                        2 => htmltext = "31543-04.html".to_owned(),
                        _ => {}
                    }
                }
            }
            NAFF | CROCUS | KUBER | BOELIN => {
                if qs.is_started(ctx) {
                    let cond = talker_cond(npc.id);
                    if qs.is_cond(ctx, cond) && ctx.has_quest_items(player, EGG_PRICE) {
                        htmltext = format!("{}-01.html", npc.id);
                    } else if qs.is_cond(ctx, cond + 1) {
                        htmltext = format!("{}-04.html", npc.id);
                    }
                }
            }
            VALENTINE => {
                if qs.is_started(ctx) && qs.is_cond(ctx, 7) {
                    htmltext = "31584-01.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `TALKERS.indexOf(npc.getId()) + 2`: the cond at which the talker takes the egg.
fn talker_cond(npc_id: i32) -> i32 {
    TALKERS
        .iter()
        .position(|id| *id == npc_id)
        .map_or(0, |i| i as i32)
        + 2
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00621_EggDelivery";

    const NPCS: &str = r#"
        <npc id="31521" type="Folk" level="70" name="Jeremy"/>
        <npc id="31543" type="Folk" level="70" name="Pulin"/>
        <npc id="31544" type="Folk" level="70" name="Naff"/>
        <npc id="31545" type="Folk" level="70" name="Crocus"/>
        <npc id="31546" type="Folk" level="70" name="Kuber"/>
        <npc id="31547" type="Folk" level="70" name="Boelin"/>
        <npc id="31584" type="Folk" level="70" name="Valentine"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7195" type="EtcItem" name="Boiled Egg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7196" type="EtcItem" name="Egg Price"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="734" type="EtcItem" name="Quick Step Potion"><set name="is_stackable" val="true"/></item>
        <item id="6849" type="Armor" name="Ring of Aurakyra"><set name="bodypart" val="rfinger"/></item>
        <item id="6847" type="Armor" name="Sanddragon's Earing"><set name="bodypart" val="lfinger"/></item>
        <item id="6851" type="Armor" name="Dragon Necklace"><set name="bodypart" val="neck"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00621EggDelivery);
        let mut plain = vec![
            "31521-02.htm",
            "31521-04.html",
            "31521-07.html",
            "31521-08.html",
            "31543-03.html",
            "31543-04.html",
            "31584-01.html",
            "31584-02.html",
        ];
        plain.extend(["31544-03.html", "31545-03.html", "31546-03.html", "31547-03.html"]);
        let pages = Pages::of(&Q00621EggDelivery)
            .page("31521-01.htm", &["31521-03.htm"])
            .page("31521-05.html", &["31521-06.html"])
            .plain(&plain)
            .plain(&["31521-03.htm", "31521-06.html"])
            .page("31543-01.html", &["31543-02.html"])
            .page("31543-02.html", &[])
            .page("31544-01.html", &["31544-02.html"])
            .page("31545-01.html", &["31545-02.html"])
            .page("31546-01.html", &["31546-02.html"])
            .page("31547-01.html", &["31547-02.html"])
            .plain(&["31544-02.html", "31545-02.html", "31546-02.html", "31547-02.html"])
            .page("31584-01.html", &["31584-02.html"])
            .npcs(&[JEREMY, PULIN, NAFF, CROCUS, KUBER, BOELIN, VALENTINE]);
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
    fn the_eggs_go_round_the_talkers_to_valentine_who_ends_it() {
        let (mut t, jeremy) = setup();
        t.enter(1, 68, vec![]);
        let npc = |t: &mut ScriptTest, id: i32| t.spawn(id, 30);
        let pulin = npc(&mut t, PULIN);
        let naff = npc(&mut t, NAFF);
        let crocus = npc(&mut t, CROCUS);
        let kuber = npc(&mut t, KUBER);
        let boelin = npc(&mut t, BOELIN);
        let valentine = npc(&mut t, VALENTINE);

        assert!(t.quest_page(1, jeremy).contains("31521-01.htm"));
        assert!(t.event(1, NAME, "31521-03.htm").contains("31521-03.htm"));
        assert_eq!(t.count(1, BOILED_EGG), 5);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Pulin at cond 1: one egg for a price, cond 2.
        assert!(t.quest_page(1, pulin).contains("31543-01.html"));
        assert!(t.event(1, NAME, "31543-02.html").contains("31543-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Naff, Crocus, Kuber, Boelin each take one egg and move the cond on.
        for (npc, page, cond) in [
            (naff, "31544", "3"),
            (crocus, "31545", "4"),
            (kuber, "31546", "5"),
            (boelin, "31547", "6"),
        ] {
            assert!(t.quest_page(1, npc).contains(&format!("{page}-01.html")));
            assert!(
                t.event(1, NAME, &format!("{page}-02.html"))
                    .contains(&format!("{page}-02.html"))
            );
            assert_eq!(t.take_writes(), [rows::update(NAME, "cond", cond)]);
        }
        assert_eq!((t.count(1, BOILED_EGG), t.count(1, EGG_PRICE)), (0, 5));

        // Jeremy pays the five prices at cond 6 (cond 7 and the prices taken).
        assert!(t.quest_page(1, jeremy).contains("31521-05.html"));
        assert!(t.event(1, NAME, "31521-06.html").contains("31521-06.html"));
        assert_eq!(t.count(1, EGG_PRICE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Valentine ends it with a reward.
        assert!(t.quest_page(1, valentine).contains("31584-01.html"));
        assert!(t.event(1, NAME, "31584-02.html").contains("31584-02.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
