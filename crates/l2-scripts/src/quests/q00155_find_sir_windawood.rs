//! Quest 155: Find Sir Windawood (Abellos sends the player to Sir Collin Windawood with a letter).
// Java: dist/game/data/scripts/quests/Q00155_FindSirWindawood/Q00155_FindSirWindawood.java
//
// Pattern: a delivery chain, the start NPC hands out the letter, the other NPC takes it and
// gives a potion and ends the quest (one-time, `exit_quest(ctx, false, true)`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ABELLOS: i32 = 30042;
const SIR_COLLIN_WINDAWOOD: i32 = 30311;

// Items
const OFFICIAL_LETTER: i32 = 1019;
const HASTE_POTION: i32 = 734;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00155_FindSirWindawood`.
pub struct Q00155FindSirWindawood;

impl Script for Q00155FindSirWindawood {
    fn name(&self) -> &'static str {
        "Q00155_FindSirWindawood"
    }

    fn path(&self) -> &'static str {
        "quests/Q00155_FindSirWindawood"
    }

    fn id(&self) -> i32 {
        155
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ABELLOS]);
        s.add_talk_id(&[ABELLOS, SIR_COLLIN_WINDAWOOD]);
        s.register_quest_items(&[OFFICIAL_LETTER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && event.eq_ignore_ascii_case("30042-03.htm")
        {
            qs.start_quest(ctx);
            ctx.give_items(player, OFFICIAL_LETTER, 1);
            return Some(event.to_owned());
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ABELLOS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30042-02.htm".to_owned()
                    } else {
                        "30042-01.htm".to_owned()
                    };
                }
                STARTED => htmltext = "30042-04.html".to_owned(),
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            SIR_COLLIN_WINDAWOOD => {
                if qs.is_started(ctx) && ctx.has_quest_items(player, OFFICIAL_LETTER) {
                    ctx.give_items(player, HASTE_POTION, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30311-01.html".to_owned();
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

    const NAME: &str = "Q00155_FindSirWindawood";

    const NPCS: &str = r#"
        <npc id="30042" type="Folk" level="1" name="Abellos"/>
        <npc id="30311" type="Folk" level="1" name="Sir Collin Windawood"/>"#;

    const ITEMS: &str = r#"
        <item id="1019" type="EtcItem" name="Official Letter"><set name="is_questitem" val="true"/></item>
        <item id="734" type="EtcItem" name="Haste Potion"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        abellos: Oid,
        windawood: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00155FindSirWindawood);
        let pages = Pages::of(&Q00155FindSirWindawood)
            .page("30042-02.htm", &["30042-03.htm"])
            .plain(&["30042-01.htm", "30042-03.htm", "30042-04.html", "30311-01.html"])
            .npcs(&[ABELLOS, SIR_COLLIN_WINDAWOOD]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            abellos: t.spawn(ABELLOS, 20),
            windawood: t.spawn(SIR_COLLIN_WINDAWOOD, 40),
        };
        (t, npcs)
    }

    #[test]
    fn a_level_two_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 2, vec![]);
        assert!(t.quest_page(1, npcs.abellos).contains("30042-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn letter_to_windawood_gives_a_potion_and_ends_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 3, vec![]);
        assert!(t.quest_page(1, npcs.abellos).contains("30042-02.htm"));
        assert!(t.event(1, NAME, "30042-03.htm").contains("30042-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, OFFICIAL_LETTER), 1);

        // Abellos again while the quest runs.
        assert!(t.quest_page(1, npcs.abellos).contains("30042-04.html"));
        assert!(t.take_writes().is_empty());

        // Windawood takes the letter: a potion, `exitQuest(false, true)` with the finish sound.
        assert!(t.quest_page(1, npcs.windawood).contains("30311-01.html"));
        assert_eq!(t.count(1, HASTE_POTION), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.abellos)
                .contains("This quest has already been completed.")
        );
    }
}
