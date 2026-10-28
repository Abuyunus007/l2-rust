//! Quest 50: Lanosco's Special Bait (Lanosco in the Fishing Village; the singing winds give the
//! essence of wind).
// Java: dist/game/data/scripts/quests/Q00050_LanoscosSpecialBait/Q00050_LanoscosSpecialBait.java
//
// Pattern: a kill-count quest with a drop chance up to a limit (`docs/design/scripts.md`, "Quest
// patterns"): the singing winds drop the essence with 33% to the party member at cond 1, and the
// hundredth essence moves that member on to cond 2. The fishing lures come back with the essence.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const LANOSCO: i32 = 31570;

// Monster
const SINGING_WIND: i32 = 21026;

// Items
const ESSENCE_OF_WIND: i32 = 7621;
const WIND_FISHING_LURE: i32 = 7610;

// Misc
const MIN_LEVEL: i32 = 27;

/// Java `Q00050_LanoscosSpecialBait`.
pub struct Q00050LanoscosSpecialBait;

impl Script for Q00050LanoscosSpecialBait {
    fn name(&self) -> &'static str {
        "Q00050_LanoscosSpecialBait"
    }

    fn path(&self) -> &'static str {
        "quests/Q00050_LanoscosSpecialBait"
    }

    fn id(&self) -> i32 {
        50
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LANOSCO]);
        s.add_talk_id(&[LANOSCO]);
        s.add_kill_id(&[SINGING_WIND]);
        s.register_quest_items(&[ESSENCE_OF_WIND]);
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
            "31570-03.htm" => {
                qs.start_quest(ctx);
            }
            "31570-07.html" => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, ESSENCE_OF_WIND) >= 100 {
                    htmltext = Some("31570-06.htm".to_owned());
                    ctx.give_items(player, WIND_FISHING_LURE, 4);
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
        if ctx.get_quest_items_count(player, ESSENCE_OF_WIND) < 100 && ctx.get_random(100) < 33 {
            ctx.reward_items(player, ESSENCE_OF_WIND, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
        if ctx.get_quest_items_count(player, ESSENCE_OF_WIND) >= 100
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
                    "31570-01.htm"
                } else {
                    "31570-02.html"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31570-05.html"
                } else {
                    "31570-04.html"
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

    const NAME: &str = "Q00050_LanoscosSpecialBait";

    const NPCS: &str = r#"
        <npc id="31570" type="Folk" level="70" name="Lanosco"/>
        <npc id="21026" type="Monster" level="40" name="Singing Wind"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7610" type="EtcItem" name="Wind Fishing Lure"><set name="is_stackable" val="true"/></item>
        <item id="7621" type="EtcItem" name="Essence of Wind"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00050LanoscosSpecialBait);
        let pages = Pages::of(&Q00050LanoscosSpecialBait)
            .page("31570-01.htm", &["31570-03.htm"])
            .page("31570-04.html", &["31570-07.html"])
            .plain(&[
                "31570-02.html",
                "31570-03.htm",
                "31570-05.html",
                "31570-06.htm",
                "31570-07.html",
            ])
            .npcs(&[LANOSCO]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lanosco = t.spawn(LANOSCO, 20);
        (t, lanosco)
    }

    /// Calls a script event directly (for the branches whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00050LanoscosSpecialBait.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills singing winds until the player has 100 essences; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..2000 {
            let wind = t.spawn(SINGING_WIND, 80);
            t.kill(1, wind);
            if t.count(1, ESSENCE_OF_WIND) >= 100 {
                return kills;
            }
        }
        panic!("no 100 essences after 2000 kills");
    }

    #[test]
    fn a_level_26_character_is_sent_away() {
        let (mut t, lanosco) = setup();
        t.enter(1, 26, vec![]);
        assert!(t.quest_page(1, lanosco).contains("31570-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_essences_then_the_four_lures() {
        let (mut t, lanosco) = setup();
        // Most of the essences are in the bag already, so the hunt is short.
        t.enter(1, 27, vec![item(1001, ESSENCE_OF_WIND, 90)]);
        assert!(t.quest_page(1, lanosco).contains("31570-01.htm"));
        assert!(t.event(1, NAME, "31570-03.htm").contains("31570-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, lanosco).contains("31570-05.html"));

        // The singing winds give the essences; the hundredth sets cond 2.
        hunt(&mut t);
        assert_eq!(t.count(1, ESSENCE_OF_WIND), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Lanosco takes the essences: four lures, the end (the essences go with the quest).
        assert!(t.quest_page(1, lanosco).contains("31570-04.html"));
        assert!(t.event(1, NAME, "31570-07.html").contains("31570-06.htm"));
        assert_eq!(t.count(1, WIND_FISHING_LURE), 4);
        assert_eq!(t.count(1, ESSENCE_OF_WIND), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, lanosco)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_reward_needs_the_hundred_essences() {
        let (mut t, lanosco) = setup();
        t.enter(1, 27, vec![]);
        t.quest_page(1, lanosco);
        t.event(1, NAME, "31570-03.htm");
        t.take_writes();

        // At cond 1 the event does nothing and its own name is the page.
        assert_eq!(direct(&mut t, "31570-07.html").as_deref(), Some("31570-07.html"));
        assert_eq!(t.count(1, WIND_FISHING_LURE), 0);
        assert!(t.take_writes().is_empty());
    }
}
