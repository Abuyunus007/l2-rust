//! Quest 52: Willie's Special Bait (Willie in the Fishing Village; the tarlk basilisks give the
//! tarlk eye).
// Java: dist/game/data/scripts/quests/Q00052_WilliesSpecialBait/Q00052_WilliesSpecialBait.java
//
// Pattern: a kill-count quest with a drop chance up to a limit (`docs/design/scripts.md`, "Quest
// patterns"): the tarlk basilisks drop the tarlk eye with 33% to the party member at cond 1, and
// the hundredth moves that member on to cond 2. The earth fishing lures come back with the eyes.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const WILLIE: i32 = 31574;

// Monster
const TARLK_BASILISK: i32 = 20573;

// Items
const TARLK_EYE: i32 = 7623;
const EARTH_FISHING_LURE: i32 = 7612;

// Misc
const MIN_LEVEL: i32 = 48;

/// Java `Q00052_WilliesSpecialBait`.
pub struct Q00052WilliesSpecialBait;

impl Script for Q00052WilliesSpecialBait {
    fn name(&self) -> &'static str {
        "Q00052_WilliesSpecialBait"
    }

    fn path(&self) -> &'static str {
        "quests/Q00052_WilliesSpecialBait"
    }

    fn id(&self) -> i32 {
        52
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WILLIE]);
        s.add_talk_id(&[WILLIE]);
        s.add_kill_id(&[TARLK_BASILISK]);
        s.register_quest_items(&[TARLK_EYE]);
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
            "31574-03.htm" => {
                qs.start_quest(ctx);
            }
            "31574-07.html" => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, TARLK_EYE) >= 100 {
                    htmltext = Some("31574-06.htm".to_owned());
                    ctx.give_items(player, EARTH_FISHING_LURE, 4);
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
        if ctx.get_quest_items_count(player, TARLK_EYE) < 100 && ctx.get_random(100) < 33 {
            ctx.reward_items(player, TARLK_EYE, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
        if ctx.get_quest_items_count(player, TARLK_EYE) >= 100
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
                    "31574-01.htm"
                } else {
                    "31574-02.html"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31574-05.html"
                } else {
                    "31574-04.html"
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

    const NAME: &str = "Q00052_WilliesSpecialBait";

    const NPCS: &str = r#"
        <npc id="31574" type="Folk" level="70" name="Willie"/>
        <npc id="20573" type="Monster" level="40" name="Tarlk Basilisk"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7612" type="EtcItem" name="Earth Fishing Lure"><set name="is_stackable" val="true"/></item>
        <item id="7623" type="EtcItem" name="Tarlk Eye"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00052WilliesSpecialBait);
        let pages = Pages::of(&Q00052WilliesSpecialBait)
            .page("31574-01.htm", &["31574-03.htm"])
            .page("31574-04.html", &["31574-07.html"])
            .plain(&[
                "31574-02.html",
                "31574-03.htm",
                "31574-05.html",
                "31574-06.htm",
                "31574-07.html",
            ])
            .npcs(&[WILLIE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let willie = t.spawn(WILLIE, 20);
        (t, willie)
    }

    /// Calls a script event directly (for the branches whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00052WilliesSpecialBait.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills tarlk basilisks until the player has 100 eyes; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..2000 {
            let basilisk = t.spawn(TARLK_BASILISK, 80);
            t.kill(1, basilisk);
            if t.count(1, TARLK_EYE) >= 100 {
                return kills;
            }
        }
        panic!("no 100 eyes after 2000 kills");
    }

    #[test]
    fn a_level_47_character_is_sent_away() {
        let (mut t, willie) = setup();
        t.enter(1, 47, vec![]);
        assert!(t.quest_page(1, willie).contains("31574-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_eyes_then_the_four_lures() {
        let (mut t, willie) = setup();
        // Most of the eyes are in the bag already, so the hunt is short.
        t.enter(1, 48, vec![item(1001, TARLK_EYE, 90)]);
        assert!(t.quest_page(1, willie).contains("31574-01.htm"));
        assert!(t.event(1, NAME, "31574-03.htm").contains("31574-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, willie).contains("31574-05.html"));

        // The tarlk basilisks give the eyes; the hundredth sets cond 2.
        hunt(&mut t);
        assert_eq!(t.count(1, TARLK_EYE), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Willie takes the eyes: four lures, the end (the eyes go with the quest).
        assert!(t.quest_page(1, willie).contains("31574-04.html"));
        assert!(t.event(1, NAME, "31574-07.html").contains("31574-06.htm"));
        assert_eq!(t.count(1, EARTH_FISHING_LURE), 4);
        assert_eq!(t.count(1, TARLK_EYE), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, willie)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_reward_needs_the_hundred_eyes() {
        let (mut t, willie) = setup();
        t.enter(1, 48, vec![]);
        t.quest_page(1, willie);
        t.event(1, NAME, "31574-03.htm");
        t.take_writes();

        // At cond 1 the event does nothing and its own name is the page.
        assert_eq!(direct(&mut t, "31574-07.html").as_deref(), Some("31574-07.html"));
        assert_eq!(t.count(1, EARTH_FISHING_LURE), 0);
        assert!(t.take_writes().is_empty());
    }
}
