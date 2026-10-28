//! Quest 109: In Search of the Nest (Mercenary Captain Pierce, the scout's corpse, Kahman).
// Java: dist/game/data/scripts/quests/Q00109_InSearchOfTheNest/Q00109_InSearchOfTheNest.java
//
// Pattern: a delivery chain with a found item (the scout's note) that is taken back at the
// third step; the states of the start NPC are read through the cond (`switch (getCond())`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PIERCE: i32 = 31553;
const SCOUTS_CORPSE: i32 = 32015;
const KAHMAN: i32 = 31554;

// Item
const SCOUTS_NOTE: i32 = 14858;

// Misc
const MIN_LEVEL: i32 = 81;

/// Java `Q00109_InSearchOfTheNest`.
pub struct Q00109InSearchOfTheNest;

impl Script for Q00109InSearchOfTheNest {
    fn name(&self) -> &'static str {
        "Q00109_InSearchOfTheNest"
    }

    fn path(&self) -> &'static str {
        "quests/Q00109_InSearchOfTheNest"
    }

    fn id(&self) -> i32 {
        109
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PIERCE]);
        s.add_talk_id(&[PIERCE, SCOUTS_CORPSE, KAHMAN]);
        s.register_quest_items(&[SCOUTS_NOTE]);
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

        match event {
            "31553-0.htm" => qs.start_quest(ctx),
            "32015-2.html" => {
                ctx.give_items(player, SCOUTS_NOTE, 1);
                qs.set_cond(ctx, 2, true);
            }
            "31553-3.html" => {
                ctx.take_items(player, SCOUTS_NOTE, -1);
                qs.set_cond(ctx, 3, true);
            }
            "31554-2.html" => {
                ctx.give_adena(player, 161_500, true);
                ctx.add_exp_and_sp(player, 701_500, 50_000);
                qs.exit_quest(ctx, false, true);
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            PIERCE => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "31553-0a.htm"
                    } else {
                        "31553-0b.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "31553-1.html".to_owned(),
                    2 => htmltext = "31553-2.html".to_owned(),
                    3 => htmltext = "31553-3a.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            SCOUTS_CORPSE => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "32015-1.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "32015-3.html".to_owned();
                    }
                }
            }
            KAHMAN => {
                if qs.is_started(ctx) && qs.is_cond(ctx, 3) {
                    htmltext = "31554-1.html".to_owned();
                }
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="31553" type="Folk" level="81" name="Mercenary Captain Pierce"/>
        <npc id="32015" type="Folk" level="81" name="Scout's Corpse"/>
        <npc id="31554" type="Folk" level="81" name="Kahman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14858" type="EtcItem" name="Scout's Note"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00109_InSearchOfTheNest";

    struct Npcs {
        pierce: Oid,
        corpse: Oid,
        kahman: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00109InSearchOfTheNest);
        let pages = Pages::of(&Q00109InSearchOfTheNest)
            .page("31553-0b.htm", &["31553-0.htm"])
            .plain(&["31553-0a.htm", "31553-0.htm", "31553-1.html", "31553-3a.html"])
            .page("31553-2.html", &["31553-3.html"])
            .plain(&["31553-3.html", "32015-3.html", "31554-1.html"])
            .page("32015-1.html", &["32015-2.html"])
            .plain(&["32015-2.html"])
            .page("31554-1.html", &["31554-2.html"])
            .plain(&["31554-2.html"])
            .npcs(&[PIERCE, SCOUTS_CORPSE, KAHMAN])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            pierce: t.spawn(PIERCE, 20),
            corpse: t.spawn(SCOUTS_CORPSE, 40),
            kahman: t.spawn(KAHMAN, 60),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_81_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, npcs.pierce).contains("31553-0a.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_corpse_and_kahman_have_nothing_to_say_before_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, npcs.corpse).contains("noquest"));
        assert!(t.quest_page(1, npcs.kahman).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_pierce_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, npcs.pierce).contains("31553-0b.htm"));

        // Accept: `startQuest()`.
        assert!(t.event(1, NAME, "31553-0.htm").contains("31553-0.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.quest_page(1, npcs.pierce).contains("31553-1.html"));

        // The corpse gives the note: cond 2.
        assert!(t.quest_page(1, npcs.corpse).contains("32015-1.html"));
        assert!(t.event(1, NAME, "32015-2.html").contains("32015-2.html"));
        assert_eq!(t.count(1, SCOUTS_NOTE), 1);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.corpse).contains("32015-3.html"));

        // Pierce takes the note back: cond 3.
        assert!(t.quest_page(1, npcs.pierce).contains("31553-2.html"));
        assert!(t.event(1, NAME, "31553-3.html").contains("31553-3.html"));
        assert_eq!(t.count(1, SCOUTS_NOTE), 0);
        assert_eq!(t.take_writes(), [update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.pierce).contains("31553-3a.html"));

        // Kahman's reward: adena, exp and sp, then `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.kahman).contains("31554-1.html"));
        assert!(t.event(1, NAME, "31554-2.html").contains("31554-2.html"));
        assert_eq!(t.count(1, 57), 161_500);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, npcs.pierce).contains("This quest has already been completed."));
    }
}
