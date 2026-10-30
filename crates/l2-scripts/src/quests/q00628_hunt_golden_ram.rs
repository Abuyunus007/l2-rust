//! Quest 628: Hunt of the Golden Ram Mercenary Force (Kahman in the Gludio area; the Stakato
//! monsters drop the splinter and the needle chitins, 100 of each, and the badges go up a rank).
// Java: dist/game/data/scripts/quests/Q00628_HuntGoldenRam/Q00628_HuntGoldenRam.java
//
// Pattern: three conds. The kills give a chitin only while the cond allows it (`ItemChanceHolder`
// count is the cond the chitin belongs to, and the drop is limited to 100), cond 2 and 3 are set
// by the talk and the badges, and the talk takes the items back. Java's `setCond(n)` has no sound.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KAHMAN: i32 = 31554;

// Items
const GOLDEN_RAM_BADGE_RECRUIT: i32 = 7246;
const GOLDEN_RAM_BADGE_SOLDIER: i32 = 7247;
const SPLINTER_STAKATO_CHITIN: i32 = 7248;
const NEEDLE_STAKATO_CHITIN: i32 = 7249;

// Misc
const REQUIRED_ITEM_COUNT: i64 = 100;
const MIN_LEVEL: i32 = 66;

/// Java `MOBS_DROP_CHANCES`: the chitin, its drop chance and the cond it is dropped at.
const MOBS_DROP_CHANCES: [(i32, i32, f64, i32); 10] = [
    (21508, SPLINTER_STAKATO_CHITIN, 0.500, 1), // splinter_stakato
    (21509, SPLINTER_STAKATO_CHITIN, 0.430, 1), // splinter_stakato_worker
    (21510, SPLINTER_STAKATO_CHITIN, 0.521, 1), // splinter_stakato_soldier
    (21511, SPLINTER_STAKATO_CHITIN, 0.575, 1), // splinter_stakato_drone
    (21512, SPLINTER_STAKATO_CHITIN, 0.746, 1), // splinter_stakato_drone_a
    (21513, NEEDLE_STAKATO_CHITIN, 0.500, 2),   // needle_stakato
    (21514, NEEDLE_STAKATO_CHITIN, 0.430, 2),   // needle_stakato_worker
    (21515, NEEDLE_STAKATO_CHITIN, 0.520, 2),   // needle_stakato_soldier
    (21516, NEEDLE_STAKATO_CHITIN, 0.531, 2),   // needle_stakato_drone
    (21517, NEEDLE_STAKATO_CHITIN, 0.744, 2),   // needle_stakato_drone_a
];

/// Java `Q00628_HuntGoldenRam`.
pub struct Q00628HuntGoldenRam;

impl Script for Q00628HuntGoldenRam {
    fn name(&self) -> &'static str {
        "Q00628_HuntGoldenRam"
    }

    fn path(&self) -> &'static str {
        "quests/Q00628_HuntGoldenRam"
    }

    fn id(&self) -> i32 {
        628
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KAHMAN]);
        s.add_talk_id(&[KAHMAN]);
        let mobs: Vec<i32> = MOBS_DROP_CHANCES.iter().map(|(id, _, _, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SPLINTER_STAKATO_CHITIN, NEEDLE_STAKATO_CHITIN]);
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
        let mut htmltext = None;
        match event {
            "accept" => {
                if qs.is_created(ctx) && ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    if ctx.has_quest_items(player, GOLDEN_RAM_BADGE_SOLDIER) {
                        qs.set_cond(ctx, 3, false);
                        htmltext = Some("31554-05.htm".to_owned());
                    } else if ctx.has_quest_items(player, GOLDEN_RAM_BADGE_RECRUIT) {
                        qs.set_cond(ctx, 2, false);
                        htmltext = Some("31554-04.htm".to_owned());
                    } else {
                        htmltext = Some("31554-03.htm".to_owned());
                    }
                }
            }
            "31554-08.html" => {
                if ctx.get_quest_items_count(player, SPLINTER_STAKATO_CHITIN) >= REQUIRED_ITEM_COUNT {
                    ctx.give_items(player, GOLDEN_RAM_BADGE_RECRUIT, 1);
                    ctx.take_items(player, SPLINTER_STAKATO_CHITIN, -1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "31554-12.html" | "31554-13.html" => {
                if qs.is_started(ctx) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31554-14.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, -1, 1, npc)`.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 1, Some(npc)) else {
            return;
        };
        if qs.is_cond(ctx, 3) {
            return;
        }
        let Some(&(_, item, chance, cond)) = MOBS_DROP_CHANCES.iter().find(|(id, _, _, _)| *id == npc.id)
        else {
            return;
        };
        if cond <= qs.get_cond(ctx) {
            ctx.give_item_randomly(
                qs.player(),
                Some(npc),
                item,
                1,
                1,
                REQUIRED_ITEM_COUNT,
                chance,
                true,
            );
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "31554-01.htm".to_owned()
            } else {
                "31554-02.htm".to_owned()
            };
        } else if state == STARTED {
            let splinter = ctx.get_quest_items_count(player, SPLINTER_STAKATO_CHITIN);
            let needle = ctx.get_quest_items_count(player, NEEDLE_STAKATO_CHITIN);
            match qs.get_cond(ctx) {
                1 => {
                    htmltext = if splinter >= REQUIRED_ITEM_COUNT {
                        "31554-07.html".to_owned()
                    } else {
                        "31554-06.html".to_owned()
                    };
                }
                2 => {
                    if ctx.has_quest_items(player, GOLDEN_RAM_BADGE_RECRUIT) {
                        if splinter >= REQUIRED_ITEM_COUNT && needle >= REQUIRED_ITEM_COUNT {
                            ctx.take_items(player, GOLDEN_RAM_BADGE_RECRUIT, -1);
                            ctx.take_items(player, SPLINTER_STAKATO_CHITIN, -1);
                            ctx.take_items(player, NEEDLE_STAKATO_CHITIN, -1);
                            ctx.give_items(player, GOLDEN_RAM_BADGE_SOLDIER, 1);
                            qs.set_cond(ctx, 3, true);
                            htmltext = "31554-10.html".to_owned();
                        } else {
                            htmltext = "31554-09.html".to_owned();
                        }
                    } else {
                        qs.set_cond(ctx, 1, false);
                        htmltext = if splinter >= REQUIRED_ITEM_COUNT {
                            "31554-07.html".to_owned()
                        } else {
                            "31554-06.html".to_owned()
                        };
                    }
                }
                3 => {
                    if ctx.has_quest_items(player, GOLDEN_RAM_BADGE_SOLDIER) {
                        htmltext = "31554-11.html".to_owned();
                    } else {
                        qs.set_cond(ctx, 1, false);
                        htmltext = if splinter >= REQUIRED_ITEM_COUNT {
                            "31554-07.html".to_owned()
                        } else {
                            "31554-06.html".to_owned()
                        };
                    }
                }
                _ => {}
            }
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

    const NAME: &str = "Q00628_HuntGoldenRam";

    const NPCS: &str = r#"
        <npc id="31554" type="Folk" level="70" name="Kahman"/>
        <npc id="21508" type="Monster" level="60" name="Splinter Stakato"/>
        <npc id="21513" type="Monster" level="60" name="Needle Stakato"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7246" type="EtcItem" name="Golden Ram Badge - Recruit"><set name="is_questitem" val="true"/></item>
        <item id="7247" type="EtcItem" name="Golden Ram Badge - Soldier"><set name="is_questitem" val="true"/></item>
        <item id="7248" type="EtcItem" name="Splinter Stakato Chitin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="7249" type="EtcItem" name="Needle Stakato Chitin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00628HuntGoldenRam);
        let pages = Pages::of(&Q00628HuntGoldenRam)
            .page("31554-01.htm", &["accept"])
            .plain(&[
                "31554-02.htm",
                "31554-03.htm",
                "31554-06.html",
                "31554-09.html",
                "31554-10.html",
            ])
            .page("31554-07.html", &["31554-08.html"])
            .plain(&["31554-08.html"])
            .page("31554-11.html", &["31554-12.html", "31554-13.html"])
            .page("31554-12.html", &["31554-14.html"])
            .plain(&["31554-13.html", "31554-14.html"])
            .npcs(&[KAHMAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kahman = t.spawn(KAHMAN, 20);
        (t, kahman)
    }

    /// Kills `mob` next to the player until the player has `target` of `item`.
    fn kill_until(t: &mut ScriptTest, mob: i32, item: i32, target: i64) {
        let mut kills = 0;
        while t.count(1, item) < target {
            let npc = t.spawn(mob, 40);
            t.kill(1, npc);
            kills += 1;
            assert!(kills < 200, "no {item} dropped");
        }
    }

    #[test]
    fn a_character_below_level_66_is_sent_away() {
        let (mut t, kahman) = setup();
        t.enter(1, 65, vec![]);
        assert!(t.quest_page(1, kahman).contains("31554-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn splinters_then_needles_make_the_soldier_badge_and_the_quest_ends() {
        let (mut t, kahman) = setup();
        t.enter(1, 66, vec![]);
        assert!(t.quest_page(1, kahman).contains("31554-01.htm"));
        assert!(t.event(1, NAME, "accept").contains("31554-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Splinters to 100: the talk offers the recruit badge (cond 2 with the sound).
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SPLINTER_STAKATO_CHITIN, 99);
        });
        t.take();
        kill_until(&mut t, 21508, SPLINTER_STAKATO_CHITIN, 100);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, kahman).contains("31554-07.html"));
        assert!(t.event(1, NAME, "31554-08.html").contains("31554-08.html"));
        assert_eq!(t.count(1, GOLDEN_RAM_BADGE_RECRUIT), 1);
        assert_eq!(t.count(1, SPLINTER_STAKATO_CHITIN), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Cond 2 without the chitins: the wait page. The splinters went to the talk, so the
        // splinter drop (cond 1 chitin, still dropped at cond 2) makes 100 again.
        assert!(t.quest_page(1, kahman).contains("31554-09.html"));
        assert!(t.take_writes().is_empty());
        kill_until(&mut t, 21508, SPLINTER_STAKATO_CHITIN, 100);
        assert!(t.take_writes().is_empty());

        // Needles to 100 by the kills (the talk wants both): the soldier badge and cond 3.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), NEEDLE_STAKATO_CHITIN, 99);
        });
        t.take();
        kill_until(&mut t, 21513, NEEDLE_STAKATO_CHITIN, 100);
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, kahman).contains("31554-10.html"));
        assert_eq!(t.count(1, GOLDEN_RAM_BADGE_SOLDIER), 1);
        assert_eq!(
            (
                t.count(1, NEEDLE_STAKATO_CHITIN),
                t.count(1, GOLDEN_RAM_BADGE_RECRUIT)
            ),
            (0, 0)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Cond 3 with the soldier badge: the page with the exits, then the quest ends.
        assert!(t.quest_page(1, kahman).contains("31554-11.html"));
        assert!(t.event(1, NAME, "31554-12.html").contains("31554-12.html"));
        assert!(t.event(1, NAME, "31554-14.html").contains("31554-14.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
