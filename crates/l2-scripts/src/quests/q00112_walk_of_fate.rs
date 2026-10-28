//! Quest 112: Walk of Fate (Seer Livina in the village, Karuda in the Crypts of Disgrace).
// Java: dist/game/data/scripts/quests/Q00112_WalkOfFate/Q00112_WalkOfFate.java
//
// Pattern: a start NPC that sends the player to a second NPC, a reward with adena, exp and an
// item. Unlike Q00121 and Q00122, `onEvent` returns null for a player without the quest or below
// the level, and answers only the events it handles (`htmltext` stays null otherwise).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LIVINA: i32 = 30572;
const KARUDA: i32 = 32017;

// Item
const SCROLL_ENCHANT_ARMOR_D_GRADE: i32 = 956;

// Misc
const MIN_LEVEL: i32 = 20;

/// Java `Q00112_WalkOfFate`.
pub struct Q00112WalkOfFate;

impl Script for Q00112WalkOfFate {
    fn name(&self) -> &'static str {
        "Q00112_WalkOfFate"
    }

    fn path(&self) -> &'static str {
        "quests/Q00112_WalkOfFate"
    }

    fn id(&self) -> i32 {
        112
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LIVINA]);
        s.add_talk_id(&[LIVINA, KARUDA]);
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
        if ctx.player_level(player) < MIN_LEVEL {
            return None;
        }

        let mut htmltext = None;
        match event {
            "30572-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32017-02.html" => {
                ctx.give_adena(player, 22_308, true);
                ctx.add_exp_and_sp(player, 112_876, 5774);
                ctx.give_items(player, SCROLL_ENCHANT_ARMOR_D_GRADE, 1);
                qs.exit_quest(ctx, false, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30572-03.html"
                } else {
                    "30572-01.htm"
                }
                .to_owned();
            }
            STARTED => match npc.id {
                LIVINA => htmltext = "30572-05.html".to_owned(),
                KARUDA => htmltext = "32017-01.html".to_owned(),
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="30572" type="Folk" level="20" name="Seer Livina"/>
        <npc id="32017" type="Folk" level="20" name="Karuda"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="956" type="EtcItem" name="Scroll: Enchant Armor (D-Grade)"/>"#;

    const NAME: &str = "Q00112_WalkOfFate";

    struct Npcs {
        livina: Oid,
        karuda: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00112WalkOfFate);
        let pages = Pages::of(&Q00112WalkOfFate)
            .page("30572-01.htm", &["30572-04.htm"])
            .plain(&["30572-03.html", "30572-04.htm", "30572-05.html"])
            .page("32017-01.html", &["32017-02.html"])
            .plain(&["32017-02.html"])
            .npcs(&[LIVINA, KARUDA])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            livina: t.spawn(LIVINA, 20),
            karuda: t.spawn(KARUDA, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_20_is_sent_away_and_cannot_start() {
        let (mut t, npcs) = setup();
        t.enter(1, 19, vec![]);
        assert!(t.quest_page(1, npcs.livina).contains("30572-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn karuda_has_nothing_to_say_before_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, npcs.karuda).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_livina_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, npcs.livina).contains("30572-01.htm"));

        // Accept: `startQuest()`.
        assert!(t.event(1, NAME, "30572-04.htm").contains("30572-04.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );

        // Livina repeats her line while the quest runs; Karuda is the next step.
        assert!(t.quest_page(1, npcs.livina).contains("30572-05.html"));
        assert!(t.quest_page(1, npcs.karuda).contains("32017-01.html"));

        // Karuda's reward: adena, exp and sp, the scroll, then `exitQuest(false, true)`.
        assert!(t.event(1, NAME, "32017-02.html").contains("32017-02.html"));
        assert_eq!(t.count(1, 57), 22_308);
        assert_eq!(t.count(1, SCROLL_ENCHANT_ARMOR_D_GRADE), 1);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: the "already completed" page.
        assert!(t.quest_page(1, npcs.livina).contains("This quest has already been completed."));
    }
}
