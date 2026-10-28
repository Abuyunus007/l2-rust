//! Quest 28: Chest Caught with a Bait of Icy Air (Ofulle in the Fishing Village, the bait quest of the icy air).
// Java: dist/game/data/scripts/quests/Q00028_ChestCaughtWithABaitOfIcyAir/Q00028_ChestCaughtWithABaitOfIcyAir.java
//
// Pattern: a fishing chest hand-over (`docs/design/scripts.md`, "Quest patterns"): Ofulle starts
// the quest once O'Fulle's Special Bait (Q00051) is done, takes the yellow treasure box for
// Kiki's letter, and Kiki pays the elven ring. One-time: `exit_quest(ctx, false, true)`.
//
// NOTE(java-quirk): an event that is not handled is still shown back as its page (the Java
// `htmltext = event` default), so the quest answers every event it gets.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const OFULLE: i32 = 31572;
const KIKI: i32 = 31442;

// Items
const YELLOW_TREASURE_BOX: i32 = 6503;
const KIKIS_LETTER: i32 = 7626;
const ELVEN_RING: i32 = 881;

// Misc
const MIN_LEVEL: i32 = 36;
/// Java `Q00051_OFullesSpecialBait.class.getSimpleName()`: the quest that comes before this one.
const OFULLES_SPECIAL_BAIT: &str = "Q00051_OFullesSpecialBait";

/// Java `Q00028_ChestCaughtWithABaitOfIcyAir`.
pub struct Q00028ChestCaughtWithABaitOfIcyAir;

impl Script for Q00028ChestCaughtWithABaitOfIcyAir {
    fn name(&self) -> &'static str {
        "Q00028_ChestCaughtWithABaitOfIcyAir"
    }

    fn path(&self) -> &'static str {
        "quests/Q00028_ChestCaughtWithABaitOfIcyAir"
    }

    fn id(&self) -> i32 {
        28
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[OFULLE]);
        s.add_talk_id(&[OFULLE, KIKI]);
        s.register_quest_items(&[KIKIS_LETTER]);
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
            "31572-04.htm" => qs.start_quest(ctx),
            "31572-08.htm" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, YELLOW_TREASURE_BOX) {
                    ctx.give_items(player, KIKIS_LETTER, 1);
                    ctx.take_items(player, YELLOW_TREASURE_BOX, -1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = "31572-07.htm".to_owned();
                }
            }
            "31442-03.htm" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, KIKIS_LETTER) {
                    ctx.give_items(player, ELVEN_RING, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "31442-02.htm".to_owned();
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
                // Java: `player.getQuestState(Q00051...)` completed and level 36 (on Ofulle only).
                if npc.id == OFULLE {
                    htmltext = "31572-02.htm".to_owned();
                    let bait_done = ctx
                        .player_quest_state(player, OFULLES_SPECIAL_BAIT)
                        .is_some_and(|q| q.is_completed());
                    if ctx.player_level(player) >= MIN_LEVEL && bait_done {
                        htmltext = "31572-01.htm".to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                OFULLE => match qs.get_cond(ctx) {
                    1 => {
                        htmltext = if ctx.has_quest_items(player, YELLOW_TREASURE_BOX) {
                            "31572-05.htm"
                        } else {
                            "31572-06.htm"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "31572-09.htm".to_owned(),
                    _ => {}
                },
                KIKI => {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "31442-01.htm".to_owned();
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

    const NAME: &str = "Q00028_ChestCaughtWithABaitOfIcyAir";

    const NPCS: &str = r#"
        <npc id="31572" type="Folk" level="70" name="Ofulle"/>
        <npc id="31442" type="Folk" level="70" name="Kiki"/>"#;

    const ITEMS: &str = r#"
        <item id="6503" type="EtcItem" name="Yellow Treasure Chest"><set name="is_questitem" val="true"/></item>
        <item id="7626" type="EtcItem" name="Kiki's Letter"><set name="is_questitem" val="true"/></item>
        <item id="881" type="Armor" name="Elven Ring"><set name="bodypart" val="rfinger;lfinger"/></item>"#;

    /// Java `Q00051_OFullesSpecialBait` as far as this quest needs it (a stand-in state).
    struct OFullesSpecialBait;

    impl Script for OFullesSpecialBait {
        fn name(&self) -> &'static str {
            "Q00051_OFullesSpecialBait"
        }
        fn path(&self) -> &'static str {
            "quests/Q00051_OFullesSpecialBait"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        ofulle: Oid,
        kiki: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(OFullesSpecialBait);
        registry.add(Q00028ChestCaughtWithABaitOfIcyAir);
        let pages = Pages::of(&Q00028ChestCaughtWithABaitOfIcyAir)
            .page("31572-01.htm", &["31572-04.htm"])
            .page("31572-05.htm", &["31572-08.htm"])
            .page("31442-01.htm", &["31442-03.htm"])
            .plain(&[
                "31572-02.htm",
                "31572-04.htm",
                "31572-06.htm",
                "31572-07.htm",
                "31572-09.htm",
                "31442-02.htm",
                "31442-03.htm",
            ])
            .npcs(&[OFULLE, KIKI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            ofulle: t.spawn(OFULLE, 20),
            kiki: t.spawn(KIKI, 40),
        };
        (t, npcs)
    }

    /// O'Fulle's Special Bait done (its state completed), through its own exit path.
    fn complete_bait(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(OFULLES_SPECIAL_BAIT, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
        });
        t.take_writes();
    }

    #[test]
    fn ofulle_wants_the_bait_quest_and_level_36() {
        let (mut t, npcs) = setup();
        t.enter(1, 36, vec![]);
        assert!(t.quest_page(1, npcs.ofulle).contains("31572-02.htm"));
        t.enter(2, 35, vec![]);
        complete_bait(&mut t, 2);
        assert!(t.quest_page(2, npcs.ofulle).contains("31572-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_yellow_box_for_kikis_letter_and_the_ring() {
        let (mut t, npcs) = setup();
        t.enter(1, 36, vec![item(100, YELLOW_TREASURE_BOX, 1)]);
        complete_bait(&mut t, 1);
        assert!(t.quest_page(1, npcs.ofulle).contains("31572-01.htm"));
        assert!(t.event(1, NAME, "31572-04.htm").contains("31572-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Ofulle takes the box for Kiki's letter (cond 2).
        assert!(t.quest_page(1, npcs.ofulle).contains("31572-05.htm"));
        assert!(t.event(1, NAME, "31572-08.htm").contains("31572-07.htm"));
        assert_eq!(
            (t.count(1, YELLOW_TREASURE_BOX), t.count(1, KIKIS_LETTER)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Kiki gives the ring: `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.kiki).contains("31442-01.htm"));
        assert!(t.event(1, NAME, "31442-03.htm").contains("31442-02.htm"));
        assert_eq!(t.count(1, ELVEN_RING), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.ofulle)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn ofulle_asks_for_the_box_when_it_is_not_in_hand() {
        let (mut t, npcs) = setup();
        t.enter(1, 36, vec![]);
        complete_bait(&mut t, 1);
        t.quest_page(1, npcs.ofulle);
        t.event(1, NAME, "31572-04.htm");
        t.take_writes();
        assert!(t.quest_page(1, npcs.ofulle).contains("31572-06.htm"));
        assert!(t.take_writes().is_empty());
    }
}
