//! Quest 249: Poisoned Plains of the Lizardmen (Mouen sends the player to Johnny, who has the
//! investigators' reports on the Plains of the Lizardmen).
// Java: dist/game/data/scripts/quests/Q00249_PoisonedPlainsOfTheLizardmen/Q00249_PoisonedPlainsOfTheLizardmen.java
//
// Pattern: a level-restricted, one-time quest with adena and exp reward and no items
// (`docs/design/scripts.md`, "Quest patterns"): `exit_quest(ctx, false, true)` at the reward.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MOUEN: i32 = 30196;
const JOHNNY: i32 = 32744;

/// Java `Q00249_PoisonedPlainsOfTheLizardmen`.
pub struct Q00249PoisonedPlainsOfTheLizardmen;

impl Script for Q00249PoisonedPlainsOfTheLizardmen {
    fn name(&self) -> &'static str {
        "Q00249_PoisonedPlainsOfTheLizardmen"
    }

    fn path(&self) -> &'static str {
        "quests/Q00249_PoisonedPlainsOfTheLizardmen"
    }

    fn id(&self) -> i32 {
        249
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MOUEN]);
        s.add_talk_id(&[MOUEN, JOHNNY]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let htmltext = Some(event.to_owned());
        let Some(qs) = ctx.get_quest_state(player, false) else {
            // NOTE(java-quirk): Java returns the event as the page when there is no state too.
            return htmltext;
        };
        // Java reads `npc.getId()` without a null check (a bypass always has the NPC).
        let npc = npc?;
        if npc.id == MOUEN {
            if event.eq_ignore_ascii_case("30196-03.htm") {
                qs.start_quest(ctx);
            }
        } else if npc.id == JOHNNY && event.eq_ignore_ascii_case("32744-03.htm") {
            ctx.give_adena(player, 83056, true);
            ctx.add_exp_and_sp(player, 477496, 58743);
            qs.exit_quest(ctx, false, true);
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == MOUEN {
            match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= 82 {
                        "30196-01.htm"
                    } else {
                        "30196-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30196-04.htm".to_owned();
                    }
                }
                COMPLETED => htmltext = "30196-05.htm".to_owned(),
                _ => {}
            }
        } else if npc.id == JOHNNY {
            // NOTE(java-quirk): see the test below, the cond of a finished quest stays set.
            if qs.is_cond(ctx, 1) {
                htmltext = "32744-01.htm".to_owned();
            } else if qs.is_completed(ctx) {
                htmltext = "32744-04.htm".to_owned();
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

    const NAME: &str = "Q00249_PoisonedPlainsOfTheLizardmen";

    const NPCS: &str = r#"
        <npc id="30196" type="Folk" level="70" name="Mouen"/>
        <npc id="32744" type="Folk" level="70" name="Johnny"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00249PoisonedPlainsOfTheLizardmen);
        let pages = Pages::of(&Q00249PoisonedPlainsOfTheLizardmen)
            .page("30196-01.htm", &["30196-02.htm"])
            .page("30196-02.htm", &["30196-03.htm"])
            .plain(&["30196-00.htm", "30196-03.htm", "30196-04.htm", "30196-05.htm"])
            .page("32744-01.htm", &["32744-02.htm"])
            .page("32744-02.htm", &["32744-03.htm"])
            .plain(&["32744-03.htm", "32744-04.htm"])
            .npcs(&[30196, 32744]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mouen = t.spawn(30196, 20);
        let johnny = t.spawn(32744, 40);
        (t, mouen, johnny)
    }

    #[test]
    fn a_character_below_level_82_is_sent_away() {
        let (mut t, mouen, _) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, mouen).contains("30196-00.htm"));
        // `getQuestState(player, true)` made a state, but nothing is written.
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_from_mouen_to_johnny_and_the_reward() {
        let (mut t, mouen, johnny) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, mouen).contains("30196-01.htm"));
        assert!(t.event(1, NAME, "30196-02.htm").contains("30196-02.htm"));
        assert!(t.event(1, NAME, "30196-03.htm").contains("30196-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Mouen waits for the answer; Johnny gives the reward.
        assert!(t.quest_page(1, mouen).contains("30196-04.htm"));
        assert!(t.quest_page(1, johnny).contains("32744-01.htm"));
        assert!(t.event(1, NAME, "32744-02.htm").contains("32744-02.htm"));
        assert!(t.event(1, NAME, "32744-03.htm").contains("32744-03.htm"));
        assert_eq!(t.count(1, 57), 83056);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );

        // NOTE(java-quirk): `exitQuest(false)` keeps `_cond` in memory (Java only clears `_vars`),
        // so Johnny still shows the cond 1 page until the character logs in again.
        assert!(t.quest_page(1, johnny).contains("32744-01.htm"));
        assert!(t.quest_page(1, mouen).contains("30196-05.htm"));
    }
}
