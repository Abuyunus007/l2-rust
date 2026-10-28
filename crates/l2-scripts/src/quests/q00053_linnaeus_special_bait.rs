//! Quest 53: Linnaeus's Special Bait (Linnaeus in the Fishing Village; the crimson drakes give the
//! crimson drake heart).
// Java: dist/game/data/scripts/quests/Q00053_LinnaeusSpecialBait/Q00053_LinnaeusSpecialBait.java
//
// Pattern: a kill-count quest with a drop chance up to a limit (`docs/design/scripts.md`, "Quest
// patterns"): the crimson drakes drop the heart with 33% to the party member at cond 1, and the
// hundredth heart moves that member on to cond 2. The flaming fishing lures come back with the
// hearts. Unlike Q00050 the reward page is the event itself, and a refusal shows page 5.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const LINNAEUS: i32 = 31577;

// Monster
const CRIMSON_DRAKE: i32 = 20670;

// Items
const CRIMSON_DRAKE_HEART: i32 = 7624;
const FLAMING_FISHING_LURE: i32 = 7613;

/// Java `Q00053_LinnaeusSpecialBait`.
pub struct Q00053LinnaeusSpecialBait;

impl Script for Q00053LinnaeusSpecialBait {
    fn name(&self) -> &'static str {
        "Q00053_LinnaeusSpecialBait"
    }

    fn path(&self) -> &'static str {
        "quests/Q00053_LinnaeusSpecialBait"
    }

    fn id(&self) -> i32 {
        53
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LINNAEUS]);
        s.add_talk_id(&[LINNAEUS]);
        s.add_kill_id(&[CRIMSON_DRAKE]);
        s.register_quest_items(&[CRIMSON_DRAKE_HEART]);
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
            "31577-1.htm" => {
                qs.start_quest(ctx);
            }
            "31577-3.htm" => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, CRIMSON_DRAKE_HEART) >= 100 {
                    // The page stays the event itself (`htmltext = event`).
                    ctx.give_items(player, FLAMING_FISHING_LURE, 4);
                    qs.exit_quest(ctx, false, true);
                } else {
                    htmltext = Some("31577-5.html".to_owned());
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
        if ctx.get_quest_items_count(player, CRIMSON_DRAKE_HEART) < 100 && ctx.get_random(100) < 33 {
            ctx.reward_items(player, CRIMSON_DRAKE_HEART, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
        if ctx.get_quest_items_count(player, CRIMSON_DRAKE_HEART) >= 100
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
                htmltext = if ctx.player_level(player) > 59 {
                    "31577-0.htm"
                } else {
                    "31577-0a.html"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 1) {
                    "31577-4.html"
                } else {
                    "31577-2.html"
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

    const NAME: &str = "Q00053_LinnaeusSpecialBait";

    const NPCS: &str = r#"
        <npc id="31577" type="Folk" level="70" name="Linnaeus"/>
        <npc id="20670" type="Monster" level="40" name="Crimson Drake"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7613" type="EtcItem" name="Flaming Fishing Lure"><set name="is_stackable" val="true"/></item>
        <item id="7624" type="EtcItem" name="Crimson Drake Heart"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00053LinnaeusSpecialBait);
        let pages = Pages::of(&Q00053LinnaeusSpecialBait)
            .page("31577-0.htm", &["31577-1.htm"])
            .page("31577-2.html", &["31577-3.htm"])
            .plain(&[
                "31577-0a.html",
                "31577-1.htm",
                "31577-3.htm",
                "31577-4.html",
                "31577-5.html",
            ])
            .npcs(&[LINNAEUS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let linnaeus = t.spawn(LINNAEUS, 20);
        (t, linnaeus)
    }

    /// Calls a script event directly (for the branches whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00053LinnaeusSpecialBait.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills crimson drakes until the player has 100 hearts; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..2000 {
            let drake = t.spawn(CRIMSON_DRAKE, 80);
            t.kill(1, drake);
            if t.count(1, CRIMSON_DRAKE_HEART) >= 100 {
                return kills;
            }
        }
        panic!("no 100 hearts after 2000 kills");
    }

    #[test]
    fn a_level_59_character_is_sent_away() {
        let (mut t, linnaeus) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, linnaeus).contains("31577-0a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hearts_then_the_four_lures() {
        let (mut t, linnaeus) = setup();
        // Most of the hearts are in the bag already, so the hunt is short.
        t.enter(1, 60, vec![item(1001, CRIMSON_DRAKE_HEART, 90)]);
        assert!(t.quest_page(1, linnaeus).contains("31577-0.htm"));
        assert!(t.event(1, NAME, "31577-1.htm").contains("31577-1.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, linnaeus).contains("31577-4.html"));

        // The crimson drakes give the hearts; the hundredth sets cond 2.
        hunt(&mut t);
        assert_eq!(t.count(1, CRIMSON_DRAKE_HEART), 100);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Linnaeus takes the hearts: four lures, the end; the page is the event itself.
        assert!(t.quest_page(1, linnaeus).contains("31577-2.html"));
        assert!(t.event(1, NAME, "31577-3.htm").contains("31577-3.htm"));
        assert_eq!(t.count(1, FLAMING_FISHING_LURE), 4);
        assert_eq!(t.count(1, CRIMSON_DRAKE_HEART), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, linnaeus)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_reward_needs_the_hundred_hearts() {
        let (mut t, linnaeus) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, linnaeus);
        t.event(1, NAME, "31577-1.htm");
        t.take_writes();

        // At cond 1 the refusal page 5, nothing given.
        assert_eq!(direct(&mut t, "31577-3.htm").as_deref(), Some("31577-5.html"));
        assert_eq!(t.count(1, FLAMING_FISHING_LURE), 0);
        assert!(t.take_writes().is_empty());
    }
}
