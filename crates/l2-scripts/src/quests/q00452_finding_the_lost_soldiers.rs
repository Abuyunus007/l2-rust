//! Quest 452: Finding the Lost Soldiers (Jakan; the soldier corpses give the dog tags).
// Java: dist/game/data/scripts/quests/Q00452_FindingtheLostSoldiers/Q00452_FindingtheLostSoldiers.java
//
// Pattern: talk NPCs that are not the start NPC. Each corpse is searched once (`on_event` gets the
// corpse, `deleteMe`) and moves the quest to cond 2; the start NPC pays out (daily quest).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const JAKAN: i32 = 32773;

// Item
const TAG_ID: i32 = 15513;

// NPCs (the soldier corpses)
const SOLDIER_CORPSES: [i32; 4] = [32769, 32770, 32771, 32772];

// Misc: Java compares the level with the literal 84.
const MIN_LEVEL: i32 = 84;

/// Java `Q00452_FindingtheLostSoldiers`.
pub struct Q00452FindingTheLostSoldiers;

impl Script for Q00452FindingTheLostSoldiers {
    fn name(&self) -> &'static str {
        "Q00452_FindingtheLostSoldiers"
    }

    fn path(&self) -> &'static str {
        "quests/Q00452_FindingtheLostSoldiers"
    }

    fn id(&self) -> i32 {
        452
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JAKAN]);
        s.add_talk_id(&[JAKAN]);
        s.add_talk_id(&SOLDIER_CORPSES);
        s.register_quest_items(&[TAG_ID]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java `return getNoQuestMsg(player)` (not null) when there is no state.
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        // Java calls `npc.getId()` here without a check.
        let npc = npc?;

        let mut htmltext = event.to_owned();
        if npc.id == JAKAN {
            if event == "32773-3.htm" {
                qs.start_quest(ctx);
            }
        } else if qs.is_cond(ctx, 1) {
            if ctx.get_random(10) < 5 {
                ctx.give_items(player, TAG_ID, 1);
            } else {
                htmltext = "corpse-3.html".to_owned();
            }
            qs.set_cond(ctx, 2, true);
            ctx.delete_npc(npc);
        } else {
            htmltext = "corpse-3.html".to_owned();
        }
        Some(htmltext)
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == JAKAN {
            match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "32773-0.html"
                    } else {
                        "32773-1.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "32773-4.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "32773-5.html".to_owned();
                        ctx.take_items(player, TAG_ID, -1);
                        ctx.give_adena(player, 95_200, true);
                        ctx.add_exp_and_sp(player, 435_024, 50_366);
                        qs.exit_quest_type(ctx, QuestType::Daily, true);
                    }
                }
                COMPLETED => {
                    if qs.is_now_available(ctx) {
                        qs.set_state(ctx, CREATED);
                        htmltext = if ctx.player_level(player) < MIN_LEVEL {
                            "32773-0.html"
                        } else {
                            "32773-1.htm"
                        }
                        .to_owned();
                    } else {
                        htmltext = "32773-6.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_cond(ctx, 1) {
            htmltext = "corpse-1.html".to_owned();
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{Script, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00452_FindingtheLostSoldiers";

    const NPCS: &str = r#"
        <npc id="32773" type="Folk" level="70" name="Jakan"/>
        <npc id="32769" type="Folk" level="1" name="Soldier Corpse"/>
        <npc id="32770" type="Folk" level="1" name="Soldier Corpse"/>
        <npc id="32771" type="Folk" level="1" name="Soldier Corpse"/>
        <npc id="32772" type="Folk" level="1" name="Soldier Corpse"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15513" type="EtcItem" name="Dog Tag"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00452FindingTheLostSoldiers);
        let pages = Pages::of(&Q00452FindingTheLostSoldiers)
            .page("32773-1.htm", &["32773-2.htm"])
            .page("32773-2.htm", &["32773-3.htm"])
            .plain(&[
                "32773-0.html",
                "32773-3.htm",
                "32773-4.html",
                "32773-5.html",
                "32773-6.html",
                "corpse-2.html",
                "corpse-3.html",
            ])
            .page("corpse-1.html", &["corpse-2.html"])
            .npcs(&[JAKAN, 32769, 32770, 32771, 32772]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let jakan = t.spawn(JAKAN, 20);
        (t, jakan)
    }

    /// Starts the quest (level 84, Jakan's accept page).
    fn start(t: &mut ScriptTest, jakan: Oid) {
        assert!(t.quest_page(1, jakan).contains("32773-1.htm"));
        assert!(t.event(1, NAME, "32773-2.htm").contains("32773-2.htm"));
        assert!(t.event(1, NAME, "32773-3.htm").contains("32773-3.htm"));
    }

    #[test]
    fn a_character_below_level_84_is_sent_away() {
        let (mut t, jakan) = setup();
        t.enter(1, 83, vec![]);
        assert!(t.quest_page(1, jakan).contains("32773-0.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_corpse_gives_a_tag_or_nothing_and_moves_to_cond_2() {
        let (mut t, jakan) = setup();
        t.enter(1, 84, vec![]);
        start(&mut t, jakan);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, jakan).contains("32773-4.html"));

        // The corpse: a tag half of the time (`getRandom(10) < 5`), and the corpse goes away.
        let corpse = t.spawn(32769, 40);
        assert!(t.quest_page(1, corpse).contains("corpse-1.html"));
        let page = t.event(1, NAME, "corpse-2.html");
        if t.count(1, TAG_ID) == 1 {
            assert!(page.contains("corpse-2.html"), "{page}");
        } else {
            assert!(page.contains("corpse-3.html"), "{page}");
        }
        assert!(!t.in_world(corpse));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Jakan pays out whatever the tags were: the adena, the exp and sp, `exitQuest(DAILY)`.
        assert!(t.quest_page(1, jakan).contains("32773-5.html"));
        assert_eq!(t.count(1, TAG_ID), 0);
        assert_eq!(t.count(1, 57), 95_200);
        // `exitQuest(DAILY)`: the quest is deleted and completed, then `restartTime` is written.
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(matches!(&writes[2], l2_world::QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done for today.
        assert!(t.quest_page(1, jakan).contains("32773-6.html"));
    }

    #[test]
    fn a_corpse_searched_at_cond_2_gives_the_empty_page_and_stays() {
        // The corpse link is only on the cond 1 page, so the cond 2 branch is called directly.
        // Java deletes the corpse only inside the cond 1 branch.
        let (mut t, jakan) = setup();
        t.enter(1, 84, vec![]);
        start(&mut t, jakan);
        let first = t.spawn(32770, 40);
        assert!(t.quest_page(1, first).contains("corpse-1.html"));
        assert!(t.event(1, NAME, "corpse-2.html").contains("corpse-"));
        assert!(!t.in_world(first));
        t.take_writes();

        let second = t.spawn(32771, 60);
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(second).unwrap();
            out = Q00452FindingTheLostSoldiers.on_event(ctx, "corpse-2.html", Some(npc), Some(Oid(1)));
        });
        assert_eq!(out.as_deref(), Some("corpse-3.html"));
        assert!(t.in_world(second));
        assert!(t.take_writes().is_empty());
    }
}
