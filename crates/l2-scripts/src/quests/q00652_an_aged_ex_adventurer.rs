//! Quest 652: An Aged Ex-Adventurer (Tantan, the old adventurer out of Soulshots, and his daughter-in-law Sara in Oren).
// Java: dist/game/data/scripts/quests/Q00652_AnAgedExAdventurer/Q00652_AnAgedExAdventurer.java
//
// Pattern: a level-gated quest that takes 100 Soulshots on start (`take_items`) and deletes the
// start NPC. The reward is a random choice (`get_random(10) <= 4`) between an enchant scroll with
// adena and adena alone.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const TANTAN: i32 = 32012;
const SARA: i32 = 30180;

// Items
const SOULSHOT_C: i32 = 1464;
const ENCHANT_ARMOR_D: i32 = 956;

/// Java `Q00652_AnAgedExAdventurer`.
pub struct Q00652AnAgedExAdventurer;

impl Script for Q00652AnAgedExAdventurer {
    fn name(&self) -> &'static str {
        "Q00652_AnAgedExAdventurer"
    }

    fn path(&self) -> &'static str {
        "quests/Q00652_AnAgedExAdventurer"
    }

    fn id(&self) -> i32 {
        652
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TANTAN]);
        s.add_talk_id(&[TANTAN, SARA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        if event == "32012-04.htm" {
            if ctx.get_quest_items_count(player, SOULSHOT_C) < 100 {
                return Some("32012-05.htm".to_owned());
            }

            qs.start_quest(ctx);
            ctx.take_items(player, SOULSHOT_C, 100);
            // Java `npc.deleteMe()`: Tantan leaves with the Soulshots.
            if let Some(npc) = npc {
                ctx.delete_npc(npc);
            }
            htmltext = Some(event.to_owned());
        } else if event == "32012-03.html" {
            htmltext = Some(event.to_owned());
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            TANTAN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= 46 {
                        "32012-01.htm"
                    } else {
                        "32012-01a.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "32012-02.html".to_owned(),
                _ => {}
            },
            SARA => {
                if qs.is_started(ctx) {
                    if ctx.get_random(10) <= 4 {
                        ctx.give_items(player, ENCHANT_ARMOR_D, 1);
                        ctx.give_adena(player, 5026, true);
                        htmltext = "30180-01.html".to_owned();
                    } else {
                        ctx.give_adena(player, 10000, true);
                        htmltext = "30180-02.html".to_owned();
                    }
                    qs.exit_quest(ctx, true, true);
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

    const NAME: &str = "Q00652_AnAgedExAdventurer";

    const NPCS: &str = r#"
        <npc id="30180" type="Folk" level="1" name="Sara"/>
        <npc id="32012" type="Folk" level="1" name="Tantan"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1464" type="EtcItem" name="Soulshot C"><set name="is_stackable" val="true"/></item>
        <item id="956" type="EtcItem" name="Enchant Armor D"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00652AnAgedExAdventurer);
        let pages = Pages::of(&Q00652AnAgedExAdventurer)
            .page("32012-01.htm", &["32012-04.htm", "32012-03.html"])
            .plain(&[
                "32012-01a.htm",
                "32012-02.html",
                "32012-03.html",
                "32012-04.htm",
                "32012-05.htm",
                "30180-01.html",
                "30180-02.html",
            ])
            .npcs(&[TANTAN, SARA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tantan = t.spawn(TANTAN, 20);
        let sara = t.spawn(SARA, 30);
        (t, tantan, sara)
    }

    #[test]
    fn a_level_45_character_is_told_to_come_back() {
        let (mut t, tantan, _) = setup();
        t.enter(1, 45, vec![]);
        assert!(t.quest_page(1, tantan).contains("32012-01a.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_46_character_starts_with_100_soulshots() {
        let (mut t, tantan, _) = setup();
        t.enter(1, 46, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SOULSHOT_C, 99);
        });
        assert!(t.quest_page(1, tantan).contains("32012-01.htm"));
        // Not enough Soulshots: the quest does not start and Tantan stays.
        assert!(t.event(1, NAME, "32012-04.htm").contains("32012-05.htm"));
        assert!(t.in_world(tantan));
        assert!(t.take_writes().is_empty());

        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SOULSHOT_C, 1);
        });
        // The last page was 32012-05.htm (no link back): talk again.
        assert!(t.quest_page(1, tantan).contains("32012-01.htm"));
        assert!(t.event(1, NAME, "32012-04.htm").contains("32012-04.htm"));
        assert_eq!(t.count(1, SOULSHOT_C), 0);
        assert!(!t.in_world(tantan));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn sara_rewards_adena_and_maybe_an_enchant_scroll() {
        let (mut t, tantan, sara) = setup();
        t.enter(1, 46, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SOULSHOT_C, 100);
        });
        t.quest_page(1, tantan);
        t.event(1, NAME, "32012-04.htm");
        t.take();
        t.take_writes();

        // The roll decides the page: 30180-01 (scroll and 5 026 adena) or 30180-02 (10 000 adena).
        let page = t.quest_page(1, sara);
        if page.contains("30180-01.html") {
            assert_eq!((t.count(1, ENCHANT_ARMOR_D), t.count(1, 57)), (1, 5026));
        } else {
            assert!(page.contains("30180-02.html"), "{page}");
            assert_eq!((t.count(1, ENCHANT_ARMOR_D), t.count(1, 57)), (0, 10000));
        }
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
