//! Quest 14: Whereabouts of the Archaeologist (Liesel in the Dion Coliseum sends a level 74 character to a ghost).
// Java: dist/game/data/scripts/quests/Q00014_WhereaboutsOfTheArchaeologist/Q00014_WhereaboutsOfTheArchaeologist.java
//
// Pattern: a single delivery to one NPC (`docs/design/scripts.md`, "Quest patterns"): the start
// NPC gives the letter on the start event, and the ghost pays the reward items when the letter
// is in hand. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LIESEL: i32 = 31263;
const GHOST_OF_ADVENTURER: i32 = 31538;

// Item
const LETTER: i32 = 7253;

// Misc
const MIN_LEVEL: i32 = 74;

/// Java `Q00014_WhereaboutsOfTheArchaeologist`.
pub struct Q00014WhereaboutsOfTheArchaeologist;

impl Script for Q00014WhereaboutsOfTheArchaeologist {
    fn name(&self) -> &'static str {
        "Q00014_WhereaboutsOfTheArchaeologist"
    }

    fn path(&self) -> &'static str {
        "quests/Q00014_WhereaboutsOfTheArchaeologist"
    }

    fn id(&self) -> i32 {
        14
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LIESEL]);
        s.add_talk_id(&[LIESEL, GHOST_OF_ADVENTURER]);
        s.register_quest_items(&[LETTER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): without a quest state `onEvent` returns the event itself (not null),
        // so the event's page is shown anyway.
        let Some(player) = player else {
            return Some(event.to_owned());
        };
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        let mut htmltext = event.to_owned();
        match event {
            "31263-02.html" => {
                qs.start_quest(ctx);
                ctx.give_items(player, LETTER, 1);
            }
            "31538-01.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, LETTER) {
                    ctx.give_adena(player, 136_928, true);
                    ctx.add_exp_and_sp(player, 325_881, 32_524);
                    qs.exit_quest(ctx, false, true);
                } else {
                    htmltext = "31538-02.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let npc_id = npc.id;

        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc_id == LIESEL {
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "31263-01.html"
                    } else {
                        "31263-00.htm"
                    }
                    .to_owned();
                }
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    match npc_id {
                        LIESEL => htmltext = "31263-02.html".to_owned(),
                        GHOST_OF_ADVENTURER => htmltext = "31538-00.html".to_owned(),
                        _ => {}
                    }
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00014_WhereaboutsOfTheArchaeologist";

    const NPCS: &str = r#"
        <npc id="31263" type="Folk" level="70" name="Liesel"/>
        <npc id="31538" type="Folk" level="70" name="Ghost of an Adventurer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7253" type="EtcItem" name="Letter"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        liesel: Oid,
        ghost: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00014WhereaboutsOfTheArchaeologist);
        let pages = Pages::of(&Q00014WhereaboutsOfTheArchaeologist)
            .page("31263-00.htm", &["31263-02.html"])
            .page("31538-00.html", &["31538-01.html"])
            .plain(&["31263-01.html", "31263-02.html", "31538-01.html", "31538-02.html"])
            .npcs(&[LIESEL, GHOST_OF_ADVENTURER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            liesel: t.spawn(LIESEL, 20),
            ghost: t.spawn(GHOST_OF_ADVENTURER, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_74_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 73, vec![]);
        assert!(t.quest_page(1, npcs.liesel).contains("31263-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letter_from_liesel_to_the_ghost() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, npcs.liesel).contains("31263-00.htm"));
        assert!(t.event(1, NAME, "31263-02.html").contains("31263-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, LETTER), 1);

        // The ghost takes the letter: the adena and the exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.ghost).contains("31538-00.html"));
        assert!(t.event(1, NAME, "31538-01.html").contains("31538-01.html"));
        assert_eq!(t.count(1, 57), 136_928);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.liesel)
                .contains("This quest has already been completed.")
        );
    }
}
