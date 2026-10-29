//! Quest 32: An Obvious Lie (Maximilian in the Town of Gludin sends a character to Gentler, Miki the Cat and the alligators).
// Java: dist/game/data/scripts/quests/Q00032_AnObviousLie/Q00032_AnObviousLie.java
//
// Pattern: a long chain of hand-overs, one kill count, and a reward choice (`docs/design/scripts.md`,
// "Quest patterns"): Gentler and Miki move the quest along, twenty medicinal herbs drop from the
// alligators of a random party member at cond 3 (`give_item_randomly`, limit 20), and the three
// ear rewards are chosen with the event words `cat`, `raccoon` and `rabbit`. One-time:
// `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MAXIMILIAN: i32 = 30120;
const GENTLER: i32 = 30094;
const MIKI_THE_CAT: i32 = 31706;

// Monster
const ALLIGATOR: i32 = 20135;

// Items: (id, count) as Java's `ItemHolder`.
const MAP_OF_GENTLER: i32 = 7165;
const MEDICINAL_HERB: (i32, i64) = (7166, 20);
const SPIRIT_ORE: (i32, i64) = (3031, 500);
const THREAD: (i32, i64) = (1868, 1000);
const SUEDE: (i32, i64) = (1866, 500);

// Misc
const MIN_LEVEL: i32 = 45;

/// Java `EARS`: the event word of each reward and its item.
fn ears_of(event: &str) -> Option<i32> {
    match event {
        "cat" => Some(6843),
        "raccoon" => Some(7680),
        "rabbit" => Some(7683),
        _ => None,
    }
}

/// Java `hasItem(player, holder)`: the player has at least the holder's count.
fn has_item(ctx: &ScriptCtx<'_>, player: ObjectId, (item, count): (i32, i64)) -> bool {
    ctx.get_quest_items_count(player, item) >= count
}

/// Java `hasAllItems(player, true, ...)`.
fn has_all_items(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[(i32, i64)]) -> bool {
    items.iter().all(|&item| has_item(ctx, player, item))
}

/// Java `takeAllItems(player, ...)`: takes all of them only when the player has all of them.
fn take_all_items(ctx: &mut ScriptCtx<'_>, player: ObjectId, items: &[(i32, i64)]) -> bool {
    if !has_all_items(ctx, player, items) {
        return false;
    }
    items
        .iter()
        .all(|&(item, count)| ctx.take_items(player, item, count))
}

/// Java `Q00032_AnObviousLie`.
pub struct Q00032AnObviousLie;

impl Script for Q00032AnObviousLie {
    fn name(&self) -> &'static str {
        "Q00032_AnObviousLie"
    }

    fn path(&self) -> &'static str {
        "quests/Q00032_AnObviousLie"
    }

    fn id(&self) -> i32 {
        32
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MAXIMILIAN]);
        s.add_talk_id(&[MAXIMILIAN, GENTLER, MIKI_THE_CAT]);
        s.add_kill_id(&[ALLIGATOR]);
        s.register_quest_items(&[MAP_OF_GENTLER, MEDICINAL_HERB.0]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        match event {
            "30120-02.html" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    return Some(event.to_owned());
                }
            }
            "30094-02.html" => {
                if qs.is_cond(ctx, 1) {
                    ctx.give_items(player, MAP_OF_GENTLER, 1);
                    qs.set_cond(ctx, 2, true);
                    return Some(event.to_owned());
                }
            }
            "31706-02.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, MAP_OF_GENTLER) {
                    ctx.take_items(player, MAP_OF_GENTLER, -1);
                    qs.set_cond(ctx, 3, true);
                    return Some(event.to_owned());
                }
            }
            "30094-06.html" => {
                if qs.is_cond(ctx, 4) && has_item(ctx, player, MEDICINAL_HERB) {
                    ctx.take_items(player, MEDICINAL_HERB.0, MEDICINAL_HERB.1);
                    qs.set_cond(ctx, 5, true);
                    return Some(event.to_owned());
                }
            }
            "30094-09.html" => {
                if qs.is_cond(ctx, 5) && has_item(ctx, player, SPIRIT_ORE) {
                    ctx.take_items(player, SPIRIT_ORE.0, SPIRIT_ORE.1);
                    qs.set_cond(ctx, 6, true);
                    return Some(event.to_owned());
                }
            }
            "30094-12.html" => {
                if qs.is_cond(ctx, 7) {
                    qs.set_cond(ctx, 8, true);
                    return Some(event.to_owned());
                }
            }
            "30094-15.html" => return Some(event.to_owned()),
            "31706-05.html" => {
                if qs.is_cond(ctx, 6) {
                    qs.set_cond(ctx, 7, true);
                    return Some(event.to_owned());
                }
            }
            "cat" | "raccoon" | "rabbit" => {
                if let Some(ears) = ears_of(event)
                    && qs.is_cond(ctx, 8)
                    && take_all_items(ctx, player, &[THREAD, SUEDE])
                {
                    ctx.give_items(player, ears, 1);
                    qs.exit_quest(ctx, false, true);
                    return Some("30094-16.html".to_owned());
                }
                return Some("30094-17.html".to_owned());
            }
            _ => {}
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java: `getRandomPartyMemberState(killer, 3, 3, npc)`: the member at cond 3, the killer
        // three times as likely. The herb goes to that member, up to 20 herbs.
        if let Some(qs) = ctx.random_party_member_state(killer, 3, 3, Some(npc))
            && ctx.give_item_randomly(
                qs.player(),
                Some(npc),
                MEDICINAL_HERB.0,
                1,
                1,
                MEDICINAL_HERB.1,
                1.0,
                true,
            )
        {
            qs.set_cond(ctx, 4, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            MAXIMILIAN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30120-01.htm"
                    } else {
                        "30120-03.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30120-04.html".to_owned();
                    }
                }
                // Java: every other state (completed) gets the "already completed" message.
                _ => htmltext = ctx.get_already_completed_msg(player),
            },
            GENTLER => match qs.get_cond(ctx) {
                1 => htmltext = "30094-01.html".to_owned(),
                2 => htmltext = "30094-03.html".to_owned(),
                4 => {
                    htmltext = if has_item(ctx, player, MEDICINAL_HERB) {
                        "30094-04.html"
                    } else {
                        "30094-05.html"
                    }
                    .to_owned();
                }
                5 => {
                    htmltext = if has_item(ctx, player, SPIRIT_ORE) {
                        "30094-07.html"
                    } else {
                        "30094-08.html"
                    }
                    .to_owned();
                }
                6 => htmltext = "30094-10.html".to_owned(),
                7 => htmltext = "30094-11.html".to_owned(),
                8 => {
                    htmltext = if has_all_items(ctx, player, &[THREAD, SUEDE]) {
                        "30094-13.html"
                    } else {
                        "30094-14.html"
                    }
                    .to_owned();
                }
                _ => {}
            },
            MIKI_THE_CAT => match qs.get_cond(ctx) {
                2 => {
                    if ctx.has_quest_items(player, MAP_OF_GENTLER) {
                        htmltext = "31706-01.html".to_owned();
                    }
                }
                3..=5 => htmltext = "31706-03.html".to_owned(),
                6 => htmltext = "31706-04.html".to_owned(),
                7 => htmltext = "31706-06.html".to_owned(),
                _ => {}
            },
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00032_AnObviousLie";

    const NPCS: &str = r#"
        <npc id="30120" type="Folk" level="70" name="Maximilian"/>
        <npc id="30094" type="Folk" level="70" name="Gentler"/>
        <npc id="31706" type="Folk" level="70" name="Miki the Cat"/>
        <npc id="20135" type="Monster" level="20" name="Alligator"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7165" type="EtcItem" name="Map of Gentler"><set name="is_questitem" val="true"/></item>
        <item id="7166" type="EtcItem" name="Medicinal Herb"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="3031" type="EtcItem" name="Spirit Ore"><set name="is_stackable" val="true"/></item>
        <item id="1868" type="EtcItem" name="Thread"><set name="is_stackable" val="true"/></item>
        <item id="1866" type="EtcItem" name="Suede"><set name="is_stackable" val="true"/></item>
        <item id="6843" type="Armor" name="Cat Ears"><set name="bodypart" val="hair"/></item>
        <item id="7680" type="Armor" name="Raccoon Ears"><set name="bodypart" val="hair"/></item>"#;

    struct Npcs {
        maximilian: Oid,
        gentler: Oid,
        miki: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00032AnObviousLie);
        let pages = Pages::of(&Q00032AnObviousLie)
            .page("30120-01.htm", &["30120-02.html"])
            .page("30094-01.html", &["30094-02.html"])
            .page("31706-01.html", &["31706-02.html"])
            .page("30094-04.html", &["30094-06.html"])
            .page("30094-07.html", &["30094-09.html"])
            .page("30094-11.html", &["30094-12.html"])
            .page("30094-13.html", &["30094-15.html"])
            .page("31706-04.html", &["31706-05.html"])
            .page("30094-15.html", &["cat", "raccoon", "rabbit"])
            .plain(&[
                "30120-02.html",
                "30120-03.htm",
                "30120-04.html",
                "30094-02.html",
                "30094-03.html",
                "30094-05.html",
                "30094-06.html",
                "30094-08.html",
                "30094-09.html",
                "30094-10.html",
                "30094-12.html",
                "30094-14.html",
                "30094-16.html",
                "30094-17.html",
                "31706-02.html",
                "31706-03.html",
                "31706-05.html",
                "31706-06.html",
            ])
            .npcs(&[MAXIMILIAN, GENTLER, MIKI_THE_CAT]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            maximilian: t.spawn(MAXIMILIAN, 20),
            gentler: t.spawn(GENTLER, 40),
            miki: t.spawn(MIKI_THE_CAT, 60),
        };
        (t, npcs)
    }

    /// The items the player brings for the rest of the quest (ore, thread and suede).
    fn bag() -> Vec<l2_world::ItemData> {
        vec![
            item(100, SPIRIT_ORE.0, SPIRIT_ORE.1),
            item(101, THREAD.0, THREAD.1),
            item(102, SUEDE.0, SUEDE.1),
        ]
    }

    #[test]
    fn maximilian_wants_level_45() {
        let (mut t, npcs) = setup();
        t.enter(1, 44, vec![]);
        assert!(t.quest_page(1, npcs.maximilian).contains("30120-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_the_map_to_the_cat_ears() {
        let (mut t, npcs) = setup();
        t.enter(1, 45, bag());
        assert!(t.quest_page(1, npcs.maximilian).contains("30120-01.htm"));
        assert!(t.event(1, NAME, "30120-02.html").contains("30120-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.maximilian).contains("30120-04.html"));

        // Gentler gives the map (cond 2), Miki takes it (cond 3).
        assert!(t.quest_page(1, npcs.gentler).contains("30094-01.html"));
        assert!(t.event(1, NAME, "30094-02.html").contains("30094-02.html"));
        assert_eq!(t.count(1, MAP_OF_GENTLER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.miki).contains("31706-01.html"));
        assert!(t.event(1, NAME, "31706-02.html").contains("31706-02.html"));
        assert_eq!(t.count(1, MAP_OF_GENTLER), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.miki).contains("31706-03.html"));

        // Twenty medicinal herbs from the alligators of the player at cond 3 (cond 4 at the last).
        for _ in 0..19 {
            let alligator = t.spawn(ALLIGATOR, 60);
            t.kill(1, alligator);
        }
        assert_eq!(t.count(1, MEDICINAL_HERB.0), 19);
        assert!(t.take_writes().is_empty());
        let alligator = t.spawn(ALLIGATOR, 60);
        t.kill(1, alligator);
        assert_eq!(t.count(1, MEDICINAL_HERB.0), 20);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Gentler takes the herbs (cond 5) and then the ore (cond 6).
        assert!(t.quest_page(1, npcs.gentler).contains("30094-04.html"));
        assert!(t.event(1, NAME, "30094-06.html").contains("30094-06.html"));
        assert_eq!(t.count(1, MEDICINAL_HERB.0), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.gentler).contains("30094-07.html"));
        assert!(t.event(1, NAME, "30094-09.html").contains("30094-09.html"));
        assert_eq!(t.count(1, SPIRIT_ORE.0), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Miki (cond 7), Gentler (cond 8).
        assert!(t.quest_page(1, npcs.miki).contains("31706-04.html"));
        assert!(t.event(1, NAME, "31706-05.html").contains("31706-05.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);
        assert!(t.quest_page(1, npcs.gentler).contains("30094-11.html"));
        assert!(t.event(1, NAME, "30094-12.html").contains("30094-12.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);
        assert!(t.quest_page(1, npcs.gentler).contains("30094-13.html"));
        assert!(t.event(1, NAME, "30094-15.html").contains("30094-15.html"));

        // The raccoon ears, taking the thread and the suede; `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "raccoon").contains("30094-16.html"));
        assert_eq!((t.count(1, THREAD.0), t.count(1, SUEDE.0)), (0, 0));
        assert_eq!(t.count(1, 7680), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.maximilian)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn gentler_shows_the_first_page_and_the_ear_words_are_only_the_three() {
        let (mut t, npcs) = setup();
        t.enter(1, 45, vec![]);
        t.quest_page(1, npcs.maximilian);
        t.event(1, NAME, "30120-02.html");
        t.take_writes();
        // At cond 1 Gentler only shows the first page; the ears are asked for at cond 8.
        assert!(t.quest_page(1, npcs.gentler).contains("30094-01.html"));
        assert!(t.take_writes().is_empty());
        assert_eq!(ears_of("cat"), Some(6843));
        assert_eq!(ears_of("raccoon"), Some(7680));
        assert_eq!(ears_of("rabbit"), Some(7683));
        assert_eq!(ears_of("dog"), None);
    }
}
