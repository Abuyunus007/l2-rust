//! Quest 30: Chest Caught with a Bait of Fire (Linnaeus in the Fishing Village, the bait quest of the fire).
// Java: dist/game/data/scripts/quests/Q00030_ChestCaughtWithABaitOfFire/Q00030_ChestCaughtWithABaitOfFire.java
//
// Pattern: a fishing chest hand-over (`docs/design/scripts.md`, "Quest patterns"): Linnaeus
// starts the quest at level 61 once Linnaeus' Special Bait (Q00053) is done, takes the red
// treasure box for Rukal's musical score, and Rukal pays the protection necklace. One-time:
// `exit_quest(ctx, false, true)`.
//
// NOTE(java-quirk): an event that is not handled is still shown back as its page (the Java
// `htmltext = event` default), so the quest answers every event it gets.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LINNAEUS: i32 = 31577;
const RUKAL: i32 = 30629;

// Items
const RED_TREASURE_BOX: i32 = 6511;
const RUKAL_MUSICAL: i32 = 7628;
const PROTECTION_NECKLACE: i32 = 916;

// Misc
const MIN_LEVEL: i32 = 61;
/// Java `Q00053_LinnaeusSpecialBait.class.getSimpleName()`: the quest that comes before this one.
const LINNAEUS_SPECIAL_BAIT: &str = "Q00053_LinnaeusSpecialBait";

/// Java `Q00030_ChestCaughtWithABaitOfFire`.
pub struct Q00030ChestCaughtWithABaitOfFire;

impl Script for Q00030ChestCaughtWithABaitOfFire {
    fn name(&self) -> &'static str {
        "Q00030_ChestCaughtWithABaitOfFire"
    }

    fn path(&self) -> &'static str {
        "quests/Q00030_ChestCaughtWithABaitOfFire"
    }

    fn id(&self) -> i32 {
        30
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LINNAEUS]);
        s.add_talk_id(&[LINNAEUS, RUKAL]);
        s.register_quest_items(&[RUKAL_MUSICAL]);
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
            "31577-02.htm" => qs.start_quest(ctx),
            "31577-04a.htm" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, RED_TREASURE_BOX) {
                    ctx.give_items(player, RUKAL_MUSICAL, 1);
                    ctx.take_items(player, RED_TREASURE_BOX, -1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = "31577-04.htm".to_owned();
                }
            }
            "30629-02.htm" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, RUKAL_MUSICAL) {
                    ctx.give_items(player, PROTECTION_NECKLACE, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30629-03.htm".to_owned();
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
                // Java: `player.getQuestState(Q00053...)` completed and level 61 (on Linnaeus only).
                if npc.id == LINNAEUS {
                    htmltext = "31577-00.htm".to_owned();
                    let bait_done = ctx
                        .player_quest_state(player, LINNAEUS_SPECIAL_BAIT)
                        .is_some_and(|q| q.is_completed());
                    if ctx.player_level(player) >= MIN_LEVEL && bait_done {
                        htmltext = "31577-01.htm".to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                LINNAEUS => match qs.get_cond(ctx) {
                    1 => {
                        htmltext = if ctx.has_quest_items(player, RED_TREASURE_BOX) {
                            "31577-03.htm"
                        } else {
                            "31577-03a.htm"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "31577-05.htm".to_owned(),
                    _ => {}
                },
                RUKAL => {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "30629-01.htm".to_owned();
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

    const NAME: &str = "Q00030_ChestCaughtWithABaitOfFire";

    const NPCS: &str = r#"
        <npc id="31577" type="Folk" level="70" name="Linnaeus"/>
        <npc id="30629" type="Folk" level="70" name="Rukal"/>"#;

    const ITEMS: &str = r#"
        <item id="6511" type="EtcItem" name="Red Treasure Chest"><set name="is_questitem" val="true"/></item>
        <item id="7628" type="EtcItem" name="Rukal's Musical Score"><set name="is_questitem" val="true"/></item>
        <item id="916" type="Armor" name="Protection Necklace"><set name="bodypart" val="neck"/></item>"#;

    /// Java `Q00053_LinnaeusSpecialBait` as far as this quest needs it (a stand-in state).
    struct LinnaeusSpecialBait;

    impl Script for LinnaeusSpecialBait {
        fn name(&self) -> &'static str {
            "Q00053_LinnaeusSpecialBait"
        }
        fn path(&self) -> &'static str {
            "quests/Q00053_LinnaeusSpecialBait"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        linnaeus: Oid,
        rukal: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(LinnaeusSpecialBait);
        registry.add(Q00030ChestCaughtWithABaitOfFire);
        let pages = Pages::of(&Q00030ChestCaughtWithABaitOfFire)
            .page("31577-01.htm", &["31577-02.htm"])
            .page("31577-03.htm", &["31577-04a.htm"])
            .page("30629-01.htm", &["30629-02.htm"])
            .plain(&[
                "31577-00.htm",
                "31577-02.htm",
                "31577-03a.htm",
                "31577-04.htm",
                "31577-04a.htm",
                "31577-05.htm",
                "30629-02.htm",
                "30629-03.htm",
            ])
            .npcs(&[LINNAEUS, RUKAL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            linnaeus: t.spawn(LINNAEUS, 20),
            rukal: t.spawn(RUKAL, 40),
        };
        (t, npcs)
    }

    /// Linnaeus' Special Bait done (its state completed), through its own exit path.
    fn complete_bait(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(LINNAEUS_SPECIAL_BAIT, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
        });
        t.take_writes();
    }

    #[test]
    fn linnaeus_wants_the_bait_quest_and_level_61() {
        let (mut t, npcs) = setup();
        t.enter(1, 61, vec![]);
        assert!(t.quest_page(1, npcs.linnaeus).contains("31577-00.htm"));
        t.enter(2, 60, vec![]);
        complete_bait(&mut t, 2);
        assert!(t.quest_page(2, npcs.linnaeus).contains("31577-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_red_box_for_the_musical_score_and_the_necklace() {
        let (mut t, npcs) = setup();
        t.enter(1, 61, vec![item(100, RED_TREASURE_BOX, 1)]);
        complete_bait(&mut t, 1);
        assert!(t.quest_page(1, npcs.linnaeus).contains("31577-01.htm"));
        assert!(t.event(1, NAME, "31577-02.htm").contains("31577-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Linnaeus takes the box for the score (cond 2); the page is the one without "a".
        assert!(t.quest_page(1, npcs.linnaeus).contains("31577-03.htm"));
        assert!(t.event(1, NAME, "31577-04a.htm").contains("31577-04.htm"));
        assert_eq!((t.count(1, RED_TREASURE_BOX), t.count(1, RUKAL_MUSICAL)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.linnaeus).contains("31577-05.htm"));

        // Rukal gives the necklace: `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.rukal).contains("30629-01.htm"));
        assert!(t.event(1, NAME, "30629-02.htm").contains("30629-03.htm"));
        assert_eq!(t.count(1, PROTECTION_NECKLACE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.linnaeus)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn linnaeus_says_he_is_still_waiting_without_the_box() {
        let (mut t, npcs) = setup();
        t.enter(1, 61, vec![]);
        complete_bait(&mut t, 1);
        t.quest_page(1, npcs.linnaeus);
        t.event(1, NAME, "31577-02.htm");
        t.take_writes();
        assert!(t.quest_page(1, npcs.linnaeus).contains("31577-03a.htm"));
        assert!(t.take_writes().is_empty());
    }
}
