//! Quest 51: O'Fulle's Special Bait (O'Fulle in the Fishing Village; the fettered souls give the
//! lost bait).
// Java: dist/game/data/scripts/quests/Q00051_OFullesSpecialBait/Q00051_OFullesSpecialBait.java
//
// Pattern: a kill-count quest with a drop chance up to a limit (`docs/design/scripts.md`, "Quest
// patterns"): the fettered souls drop the lost bait with 33% to the party member at cond 1, and
// the hundredth moves that member on to cond 2. The icy air lures come back with the bait.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const OFULLE: i32 = 31572;

// Monster
const FETTERED_SOUL: i32 = 20552;

// Items
const LOST_BAIT: i32 = 7622;
const ICY_AIR_LURE: i32 = 7611;

// Misc
const MIN_LEVEL: i32 = 36;

/// Java `Q00051_OFullesSpecialBait`.
pub struct Q00051OFullesSpecialBait;

impl Script for Q00051OFullesSpecialBait {
    fn name(&self) -> &'static str {
        "Q00051_OFullesSpecialBait"
    }

    fn path(&self) -> &'static str {
        "quests/Q00051_OFullesSpecialBait"
    }

    fn id(&self) -> i32 {
        51
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[OFULLE]);
        s.add_talk_id(&[OFULLE]);
        s.add_kill_id(&[FETTERED_SOUL]);
        s.register_quest_items(&[LOST_BAIT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        let mut htmltext = Some(event.to_owned());
        match event {
            "31572-03.htm" => {
                qs.start_quest(ctx);
            }
            "31572-07.html" => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, LOST_BAIT) >= 100 {
                    htmltext = Some("31572-06.htm".to_owned());
                    ctx.give_items(player, ICY_AIR_LURE, 4);
                    qs.exit_quest(ctx, false, true);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(party_member) = ctx.get_random_party_member_cond(player, 1) else {
            return;
        };
        let qs = ctx.get_quest_state(party_member, false);
        if ctx.get_quest_items_count(player, LOST_BAIT) < 100 && ctx.get_random(100) < 33 {
            ctx.reward_items(player, LOST_BAIT, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
        if ctx.get_quest_items_count(player, LOST_BAIT) >= 100
            && let Some(qs) = qs
        {
            qs.set_cond(ctx, 2, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31572-01.htm"
                } else {
                    "31572-02.html"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31572-05.html"
                } else {
                    "31572-04.html"
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00051_OFullesSpecialBait";

    const NPCS: &str = r#"
        <npc id="31572" type="Folk" level="70" name="OFulle"/>
        <npc id="20552" type="Monster" level="40" name="Fettered Soul"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7611" type="EtcItem" name="Icy Air Lure"><set name="is_stackable" val="true"/></item>
        <item id="7622" type="EtcItem" name="Lost Bait"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00051OFullesSpecialBait);
        let pages = Pages::of(&Q00051OFullesSpecialBait)
            .page("31572-01.htm", &["31572-03.htm"])
            .page("31572-04.html", &["31572-07.html"])
            .plain(&[
                "31572-02.html",
                "31572-03.htm",
                "31572-05.html",
                "31572-06.htm",
                "31572-07.html",
            ])
            .npcs(&[OFULLE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ofulle = t.spawn(OFULLE, 20);
        (t, ofulle)
    }

    /// Calls a script event directly (for the branches whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00051OFullesSpecialBait.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills fettered souls until the player has 100 bait; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..2000 {
            let soul = t.spawn(FETTERED_SOUL, 80);
            t.kill(1, soul);
            if t.count(1, LOST_BAIT) >= 100 {
                return kills;
            }
        }
        panic!("no 100 bait after 2000 kills");
    }

    #[test]
    fn a_level_35_character_is_sent_away() {
        let (mut t, ofulle) = setup();
        t.enter(1, 35, vec![]);
        assert!(t.quest_page(1, ofulle).contains("31572-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_bait_then_the_four_lures() {
        let (mut t, ofulle) = setup();
        // Most of the bait are in the bag already, so the hunt is short.
        t.enter(1, 36, vec![item(1001, LOST_BAIT, 90)]);
        assert!(t.quest_page(1, ofulle).contains("31572-01.htm"));
        assert!(t.event(1, NAME, "31572-03.htm").contains("31572-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ofulle).contains("31572-05.html"));

        // The fettered souls give the bait; the hundredth sets cond 2.
        hunt(&mut t);
        assert_eq!(t.count(1, LOST_BAIT), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // OFulle takes the bait: four lures, the end (the bait go with the quest).
        assert!(t.quest_page(1, ofulle).contains("31572-04.html"));
        assert!(t.event(1, NAME, "31572-07.html").contains("31572-06.htm"));
        assert_eq!(t.count(1, ICY_AIR_LURE), 4);
        assert_eq!(t.count(1, LOST_BAIT), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, ofulle)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_reward_needs_the_hundred_bait() {
        let (mut t, ofulle) = setup();
        t.enter(1, 36, vec![]);
        t.quest_page(1, ofulle);
        t.event(1, NAME, "31572-03.htm");
        t.take_writes();

        // At cond 1 the event does nothing and its own name is the page.
        assert_eq!(direct(&mut t, "31572-07.html").as_deref(), Some("31572-07.html"));
        assert_eq!(t.count(1, ICY_AIR_LURE), 0);
        assert!(t.take_writes().is_empty());
    }
}
