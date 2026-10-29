//! Quest 27: Chest Caught with a Bait of Wind (Lanosco in the Fishing Village, the bait quest of the wind).
// Java: dist/game/data/scripts/quests/Q00027_ChestCaughtWithABaitOfWind/Q00027_ChestCaughtWithABaitOfWind.java
//
// Pattern: a fishing chest hand-over (`docs/design/scripts.md`, "Quest patterns"): Lanosco
// starts the quest once Lanosco's Special Bait (Q00050) is done, takes the blue treasure box
// for the blueprint, and Shaling pays the black pearl ring. One-time: `exit_quest(ctx, false, true)`.
//
// NOTE(java-quirk): an event that is not handled is still shown back as its page (the Java
// `htmltext = event` default), so the quest answers every event it gets.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LANOSCO: i32 = 31570;
const SHALING: i32 = 31434;

// Items
const BLUE_TREASURE_BOX: i32 = 6500;
const STRANGE_BLUESPRINT: i32 = 7625;
const BLACK_PEARL_RING: i32 = 880;

// Misc
const MIN_LEVEL: i32 = 27;
/// Java `Q00050_LanoscosSpecialBait.class.getSimpleName()`: the quest that comes before this one.
const LANOSCOS_SPECIAL_BAIT: &str = "Q00050_LanoscosSpecialBait";

/// Java `Q00027_ChestCaughtWithABaitOfWind`.
pub struct Q00027ChestCaughtWithABaitOfWind;

impl Script for Q00027ChestCaughtWithABaitOfWind {
    fn name(&self) -> &'static str {
        "Q00027_ChestCaughtWithABaitOfWind"
    }

    fn path(&self) -> &'static str {
        "quests/Q00027_ChestCaughtWithABaitOfWind"
    }

    fn id(&self) -> i32 {
        27
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LANOSCO]);
        s.add_talk_id(&[LANOSCO, SHALING]);
        s.register_quest_items(&[STRANGE_BLUESPRINT]);
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
            "31570-03.htm" => qs.start_quest(ctx),
            "31570-05.htm" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, BLUE_TREASURE_BOX) {
                    htmltext = "31570-06.htm".to_owned();
                    qs.set_cond(ctx, 2, true);
                    ctx.give_items(player, STRANGE_BLUESPRINT, 1);
                    ctx.take_items(player, BLUE_TREASURE_BOX, -1);
                }
            }
            "31434-02.htm" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, STRANGE_BLUESPRINT) {
                    ctx.give_items(player, BLACK_PEARL_RING, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "31434-01.htm".to_owned();
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
                // Java: `player.getQuestState(Q00050...)` completed and level 27 (on Lanosco only).
                if npc.id == LANOSCO {
                    htmltext = "31570-02.htm".to_owned();
                    let bait_done = ctx
                        .player_quest_state(player, LANOSCOS_SPECIAL_BAIT)
                        .is_some_and(|q| q.is_completed());
                    if ctx.player_level(player) >= MIN_LEVEL && bait_done {
                        htmltext = "31570-01.htm".to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                LANOSCO => {
                    htmltext = if qs.is_cond(ctx, 1) {
                        if ctx.has_quest_items(player, BLUE_TREASURE_BOX) {
                            "31570-04.htm"
                        } else {
                            "31570-05.htm"
                        }
                    } else {
                        "31570-07.htm"
                    }
                    .to_owned();
                }
                SHALING => {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "31434-00.htm".to_owned();
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

    const NAME: &str = "Q00027_ChestCaughtWithABaitOfWind";

    const NPCS: &str = r#"
        <npc id="31570" type="Folk" level="70" name="Lanosco"/>
        <npc id="31434" type="Folk" level="70" name="Shaling"/>"#;

    const ITEMS: &str = r#"
        <item id="6500" type="EtcItem" name="Blue Treasure Chest"><set name="is_questitem" val="true"/></item>
        <item id="7625" type="EtcItem" name="Strange Blueprint"><set name="is_questitem" val="true"/></item>
        <item id="880" type="Armor" name="Black Pearl Ring"><set name="bodypart" val="rfinger;lfinger"/></item>"#;

    /// Java `Q00050_LanoscosSpecialBait` as far as this quest needs it: a script of that name
    /// with no NPCs, whose state the Lanosco page reads.
    struct LanoscosSpecialBait;

    impl Script for LanoscosSpecialBait {
        fn name(&self) -> &'static str {
            "Q00050_LanoscosSpecialBait"
        }
        fn path(&self) -> &'static str {
            "quests/Q00050_LanoscosSpecialBait"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        lanosco: Oid,
        shaling: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(LanoscosSpecialBait);
        registry.add(Q00027ChestCaughtWithABaitOfWind);
        let pages = Pages::of(&Q00027ChestCaughtWithABaitOfWind)
            .page("31570-01.htm", &["31570-03.htm"])
            .page("31570-04.htm", &["31570-05.htm"])
            .page("31434-00.htm", &["31434-02.htm"])
            .plain(&[
                "31570-02.htm",
                "31570-03.htm",
                "31570-05.htm",
                "31570-06.htm",
                "31570-07.htm",
                "31434-01.htm",
                "31434-02.htm",
            ])
            .npcs(&[LANOSCO, SHALING]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            lanosco: t.spawn(LANOSCO, 20),
            shaling: t.spawn(SHALING, 40),
        };
        (t, npcs)
    }

    /// Lanosco's Special Bait done (its state completed), through its own exit path.
    fn complete_bait(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(LANOSCOS_SPECIAL_BAIT, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.start_quest(ctx);
                qs.exit_quest(ctx, false, false);
            }
        });
        t.take_writes();
    }

    #[test]
    fn the_bait_quest_comes_first() {
        let (mut t, npcs) = setup();
        t.enter(1, 27, vec![]);
        assert!(t.quest_page(1, npcs.lanosco).contains("31570-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_box_for_the_blueprint_and_the_ring() {
        let (mut t, npcs) = setup();
        t.enter(1, 27, vec![item(100, BLUE_TREASURE_BOX, 1)]);
        complete_bait(&mut t, 1);
        assert!(t.quest_page(1, npcs.lanosco).contains("31570-01.htm"));
        assert!(t.event(1, NAME, "31570-03.htm").contains("31570-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Lanosco takes the box for the blueprint (cond 2).
        assert!(t.quest_page(1, npcs.lanosco).contains("31570-04.htm"));
        assert!(t.event(1, NAME, "31570-05.htm").contains("31570-06.htm"));
        assert_eq!(
            (t.count(1, BLUE_TREASURE_BOX), t.count(1, STRANGE_BLUESPRINT)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Shaling gives the ring: `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.shaling).contains("31434-00.htm"));
        assert!(t.event(1, NAME, "31434-02.htm").contains("31434-01.htm"));
        assert_eq!(t.count(1, BLACK_PEARL_RING), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.lanosco)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn lanosco_says_nothing_new_without_the_box() {
        let (mut t, npcs) = setup();
        t.enter(1, 27, vec![]);
        complete_bait(&mut t, 1);
        t.quest_page(1, npcs.lanosco);
        assert!(t.event(1, NAME, "31570-03.htm").contains("31570-03.htm"));
        t.take_writes();
        // Without the box Lanosco asks for it, and nothing is written.
        assert!(t.quest_page(1, npcs.lanosco).contains("31570-05.htm"));
        assert!(t.take_writes().is_empty());
        assert_eq!(t.count(1, STRANGE_BLUESPRINT), 0);
    }
}
