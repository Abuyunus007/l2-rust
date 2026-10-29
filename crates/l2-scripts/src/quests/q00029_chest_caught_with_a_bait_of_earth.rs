//! Quest 29: Chest Caught with a Bait of Earth (Willie in the Fishing Village, the bait quest of the earth).
// Java: dist/game/data/scripts/quests/Q00029_ChestCaughtWithABaitOfEarth/Q00029_ChestCaughtWithABaitOfEarth.java
//
// Pattern: a fishing chest hand-over (`docs/design/scripts.md`, "Quest patterns"): Willie starts
// the quest once Willie's Special Bait (Q00052) is done, takes the purple treasure box for the
// small glass box, and Anabel pays the plated leather gloves. One-time: `exit_quest(ctx, false, true)`.
//
// NOTE(java-quirk): an event that is not handled is still shown back as its page (the Java
// `htmltext = event` default), so the quest answers every event it gets.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WILLIE: i32 = 31574;
const ANABEL: i32 = 30909;

// Items
const PURPLE_TREASURE_BOX: i32 = 6507;
const SMALL_GLASS_BOX: i32 = 7627;
const PLATED_LEATHER_GLOVES: i32 = 2455;

// Misc
const MIN_LEVEL: i32 = 48;
/// Java `Q00052_WilliesSpecialBait.class.getSimpleName()`: the quest that comes before this one.
const WILLIES_SPECIAL_BAIT: &str = "Q00052_WilliesSpecialBait";

/// Java `Q00029_ChestCaughtWithABaitOfEarth`.
pub struct Q00029ChestCaughtWithABaitOfEarth;

impl Script for Q00029ChestCaughtWithABaitOfEarth {
    fn name(&self) -> &'static str {
        "Q00029_ChestCaughtWithABaitOfEarth"
    }

    fn path(&self) -> &'static str {
        "quests/Q00029_ChestCaughtWithABaitOfEarth"
    }

    fn id(&self) -> i32 {
        29
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WILLIE]);
        s.add_talk_id(&[WILLIE, ANABEL]);
        s.register_quest_items(&[SMALL_GLASS_BOX]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // Java: `htmltext = event` for every case, also when there is no quest state.
        let Some(player) = player else {
            return Some(event.to_owned());
        };
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        let mut htmltext = event.to_owned();
        match event {
            "31574-04.htm" => qs.start_quest(ctx),
            "31574-08.htm" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, PURPLE_TREASURE_BOX) {
                    ctx.give_items(player, SMALL_GLASS_BOX, 1);
                    ctx.take_items(player, PURPLE_TREASURE_BOX, -1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = "31574-07.htm".to_owned();
                }
            }
            "30909-03.htm" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, SMALL_GLASS_BOX) {
                    ctx.give_items(player, PLATED_LEATHER_GLOVES, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30909-02.htm".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                // Java: `player.getQuestState(Q00052...)` completed and level 48 (on Willie only).
                if npc.id == WILLIE {
                    htmltext = "31574-02.htm".to_owned();
                    let bait_done = ctx
                        .player_quest_state(player, WILLIES_SPECIAL_BAIT)
                        .is_some_and(|q| q.is_completed());
                    if ctx.player_level(player) >= MIN_LEVEL && bait_done {
                        htmltext = "31574-01.htm".to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                WILLIE => match qs.get_cond(ctx) {
                    1 => {
                        htmltext = if ctx.has_quest_items(player, PURPLE_TREASURE_BOX) {
                            "31574-05.htm"
                        } else {
                            "31574-06.htm"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "31574-09.htm".to_owned(),
                    _ => {}
                },
                ANABEL => {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "30909-01.htm".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00029_ChestCaughtWithABaitOfEarth";

    const NPCS: &str = r#"
        <npc id="31574" type="Folk" level="70" name="Willie"/>
        <npc id="30909" type="Folk" level="70" name="Anabel"/>"#;

    const ITEMS: &str = r#"
        <item id="6507" type="EtcItem" name="Purple Treasure Chest"><set name="is_questitem" val="true"/></item>
        <item id="7627" type="EtcItem" name="Small Glass Box"><set name="is_questitem" val="true"/></item>
        <item id="2455" type="Armor" name="Plated Leather Gloves"><set name="bodypart" val="gloves"/></item>"#;

    /// Java `Q00052_WilliesSpecialBait` as far as this quest needs it (a stand-in state).
    struct WilliesSpecialBait;

    impl Script for WilliesSpecialBait {
        fn name(&self) -> &'static str {
            "Q00052_WilliesSpecialBait"
        }
        fn path(&self) -> &'static str {
            "quests/Q00052_WilliesSpecialBait"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        willie: Oid,
        anabel: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(WilliesSpecialBait);
        registry.add(Q00029ChestCaughtWithABaitOfEarth);
        let pages = Pages::of(&Q00029ChestCaughtWithABaitOfEarth)
            .page("31574-01.htm", &["31574-04.htm"])
            .page("31574-05.htm", &["31574-08.htm"])
            .page("30909-01.htm", &["30909-03.htm"])
            .plain(&[
                "31574-02.htm",
                "31574-04.htm",
                "31574-06.htm",
                "31574-07.htm",
                "31574-09.htm",
                "30909-02.htm",
                "30909-03.htm",
            ])
            .npcs(&[WILLIE, ANABEL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            willie: t.spawn(WILLIE, 20),
            anabel: t.spawn(ANABEL, 40),
        };
        (t, npcs)
    }

    /// Willie's Special Bait done (its state completed), through its own exit path.
    fn complete_bait(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(WILLIES_SPECIAL_BAIT, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
        });
        t.take_writes();
    }

    #[test]
    fn willie_wants_the_bait_quest_and_level_48() {
        let (mut t, npcs) = setup();
        t.enter(1, 48, vec![]);
        assert!(t.quest_page(1, npcs.willie).contains("31574-02.htm"));
        t.enter(2, 47, vec![]);
        complete_bait(&mut t, 2);
        assert!(t.quest_page(2, npcs.willie).contains("31574-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_purple_box_for_the_glass_box_and_the_gloves() {
        let (mut t, npcs) = setup();
        t.enter(1, 48, vec![item(100, PURPLE_TREASURE_BOX, 1)]);
        complete_bait(&mut t, 1);
        assert!(t.quest_page(1, npcs.willie).contains("31574-01.htm"));
        assert!(t.event(1, NAME, "31574-04.htm").contains("31574-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Willie takes the box for the small glass box (cond 2).
        assert!(t.quest_page(1, npcs.willie).contains("31574-05.htm"));
        assert!(t.event(1, NAME, "31574-08.htm").contains("31574-07.htm"));
        assert_eq!(
            (t.count(1, PURPLE_TREASURE_BOX), t.count(1, SMALL_GLASS_BOX)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Anabel gives the gloves: `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.anabel).contains("30909-01.htm"));
        assert!(t.event(1, NAME, "30909-03.htm").contains("30909-02.htm"));
        assert_eq!(t.count(1, PLATED_LEATHER_GLOVES), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.willie)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn willie_asks_for_the_box_when_it_is_not_in_hand() {
        let (mut t, npcs) = setup();
        t.enter(1, 48, vec![]);
        complete_bait(&mut t, 1);
        t.quest_page(1, npcs.willie);
        t.event(1, NAME, "31574-04.htm");
        t.take_writes();
        assert!(t.quest_page(1, npcs.willie).contains("31574-06.htm"));
        assert!(t.take_writes().is_empty());
    }
}
