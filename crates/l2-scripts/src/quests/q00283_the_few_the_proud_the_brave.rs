//! Quest 283: The Few, The Proud, The Brave (Perwan in the Hunters Village buys the crimson spider claws).
// Java: dist/game/data/scripts/quests/Q00283_TheFewTheProudTheBrave/Q00283_TheFewTheProudTheBrave.java
//
// Pattern: a party drop quest without a condition (`docs/design/scripts.md`, "Quest patterns"): a
// random started party member gets a crimson spider claw from the crimson spiders with a drop
// chance; Perwan pays per claw, with a bonus from 10 claws, moves the Newbie Guide's mission on
// and the player can leave the quest with `exitQuest(true)`.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const PERWAN: i32 = 32133;

// Item
const CRIMSON_SPIDER_CLAW: i32 = 9747;

// Monster
const CRIMSON_SPIDER: i32 = 22244;

// Misc
const CLAW_PRICE: i64 = 45;
const BONUS: i64 = 2187;
const MIN_LEVEL: i32 = 15;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00283_TheFewTheProudTheBrave`.
pub struct Q00283TheFewTheProudTheBrave;

impl Script for Q00283TheFewTheProudTheBrave {
    fn name(&self) -> &'static str {
        "Q00283_TheFewTheProudTheBrave"
    }

    fn path(&self) -> &'static str {
        "quests/Q00283_TheFewTheProudTheBrave"
    }

    fn id(&self) -> i32 {
        283
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[CRIMSON_SPIDER]);
        s.add_start_npc(&[PERWAN]);
        s.add_talk_id(&[PERWAN]);
        s.register_quest_items(&[CRIMSON_SPIDER_CLAW]);
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
            "32133-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32133-06.html" => htmltext = Some(event.to_owned()),
            "32133-08.html" => {
                if ctx.has_quest_items(player, CRIMSON_SPIDER_CLAW) {
                    let claws = ctx.get_quest_items_count(player, CRIMSON_SPIDER_CLAW);
                    let bonus = if claws >= 10 { BONUS } else { 0 };
                    ctx.give_adena(player, claws * CLAW_PRICE + bonus, true);
                    ctx.take_items(player, CRIMSON_SPIDER_CLAW, -1);

                    // Newbie Guide: only when `ai/others/NewbieGuide` is installed.
                    if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                        if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 100_000_000) / 10_000_000
                            != 1
                        {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 10_000_000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        }
                        guide.set_state(ctx, COMPLETED);
                    }

                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("32133-07.html".to_owned());
                }
            }
            "32133-09.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.random_party_member_state(killer, -1, 3, Some(npc)).is_some() {
            // Limit 0: no limit on the claws.
            ctx.give_item_randomly(killer, Some(npc), CRIMSON_SPIDER_CLAW, 1, 1, 0, 0.6, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(talker) >= MIN_LEVEL {
                    "32133-01.htm"
                } else {
                    "32133-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if ctx.has_quest_items(talker, CRIMSON_SPIDER_CLAW) {
                    "32133-04.html"
                } else {
                    "32133-05.html"
                }
                .to_owned();
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00283_TheFewTheProudTheBrave";

    const NPCS: &str = r#"
        <npc id="32133" type="Folk" level="70" name="Perwan"/>
        <npc id="22244" type="Monster" level="40" name="Crimson Spider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9747" type="EtcItem" name="Crimson Spider Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// Java `ai.others.NewbieGuide` as far as this quest needs it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            "NewbieGuide"
        }
        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Q00283TheFewTheProudTheBrave);
        let pages = Pages::of(&Q00283TheFewTheProudTheBrave)
            .page("32133-01.htm", &["32133-03.htm"])
            .page("32133-03.htm", &[])
            .page("32133-04.html", &["32133-08.html"])
            .page("32133-08.html", &["32133-06.html", "32133-09.html"])
            .plain(&[
                "32133-02.htm",
                "32133-05.html",
                "32133-06.html",
                "32133-07.html",
                "32133-09.html",
            ])
            .npcs(&[PERWAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let perwan = t.spawn(PERWAN, 20);
        (t, perwan)
    }

    /// Kills crimson spiders until `claws` claws are in the inventory; returns the kills.
    fn hunt(t: &mut ScriptTest, claws: i64) -> usize {
        for kills in 1..400 {
            if t.count(1, CRIMSON_SPIDER_CLAW) >= claws {
                return kills;
            }
            let spider = t.spawn(CRIMSON_SPIDER, 60);
            t.kill(1, spider);
        }
        panic!("no {claws} claws after 400 kills");
    }

    #[test]
    fn a_level_fourteen_character_is_sent_away() {
        let (mut t, perwan) = setup();
        t.enter_as(1, 14, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, perwan).contains("32133-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn claws_for_adena_with_the_bonus_then_the_newbie_guide_and_the_exit() {
        let (mut t, perwan) = setup();
        t.enter_as(1, 15, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, perwan).contains("32133-01.htm"));
        assert!(t.event(1, NAME, "32133-03.htm").contains("32133-03.htm"));
        // `startQuest()` sets cond 1 and the state.
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, perwan).contains("32133-05.html"));

        // Claws come with a chance per kill; nothing is written.
        hunt(&mut t, 10);
        assert!(t.take_writes().is_empty());

        // Perwan pays 45 per claw and 2187 more from 10 claws; the claws go, the Newbie Guide's
        // mission moves on.
        let claws = t.count(1, CRIMSON_SPIDER_CLAW);
        assert!(t.quest_page(1, perwan).contains("32133-04.html"));
        t.bypass(1, &format!("Script {NAME} 32133-08.html"));
        let out = t.take();
        assert!(
            pages_to(&out, 1)
                .pop()
                .unwrap_or_default()
                .contains("32133-08.html")
        );
        assert!(names_to(&out, 1).contains(&"EX_SHOW_SCREEN_MESSAGE".to_owned()));
        assert_eq!(t.count(1, 57), claws * 45 + 2187);
        assert_eq!(t.count(1, CRIMSON_SPIDER_CLAW), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NEWBIE_GUIDE, "NRmemo", "41"),
                rows::insert(NEWBIE_GUIDE, "NRmemoState41", "100000"),
            ]
        );

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "32133-09.html").contains("32133-09.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
