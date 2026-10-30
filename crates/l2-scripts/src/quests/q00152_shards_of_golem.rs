//! Quest 152: Shards of Golem (Harrys and Altran, five golem shards for a tool box).
// Java: dist/game/data/scripts/quests/Q00152_ShardsOfGolem/Q00152_ShardsOfGolem.java
//
// Pattern: a delivery chain with a kill count (30 % per kill up to 5 shards, no party share).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HARRYS: i32 = 30035;
const ALTRAN: i32 = 30283;

// Monster
const STONE_GOLEM: i32 = 20016;

// Items
const WOODEN_BREASTPLATE: i32 = 23;
const HARRYS_1ST_RECIEPT: i32 = 1008;
const HARRYS_2ND_RECIEPT: i32 = 1009;
const GOLEM_SHARD: i32 = 1010;
const TOOL_BOX: i32 = 1011;

// Misc
const MIN_LEVEL: i32 = 10;

/// Java `Q00152_ShardsOfGolem`.
pub struct Q00152ShardsOfGolem;

impl Script for Q00152ShardsOfGolem {
    fn name(&self) -> &'static str {
        "Q00152_ShardsOfGolem"
    }

    fn path(&self) -> &'static str {
        "quests/Q00152_ShardsOfGolem"
    }

    fn id(&self) -> i32 {
        152
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HARRYS]);
        s.add_talk_id(&[HARRYS, ALTRAN]);
        s.add_kill_id(&[STONE_GOLEM]);
        s.register_quest_items(&[HARRYS_1ST_RECIEPT, HARRYS_2ND_RECIEPT, GOLEM_SHARD, TOOL_BOX]);
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
        let mut htmltext = None;
        match event {
            "30035-03.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, HARRYS_1ST_RECIEPT, 1);
                htmltext = Some(event.to_owned());
            }
            "30283-02.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, HARRYS_1ST_RECIEPT) {
                    ctx.take_items(player, HARRYS_1ST_RECIEPT, -1);
                    ctx.give_items(player, HARRYS_2ND_RECIEPT, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 2)
            && ctx.get_random(100) < 30
            && ctx.get_quest_items_count(killer, GOLEM_SHARD) < 5
        {
            ctx.give_items(killer, GOLEM_SHARD, 1);
            if ctx.get_quest_items_count(killer, GOLEM_SHARD) >= 5 {
                qs.set_cond(ctx, 3, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            HARRYS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30035-02.htm".to_owned()
                    } else {
                        "30035-01.htm".to_owned()
                    };
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        if ctx.has_quest_items(player, HARRYS_1ST_RECIEPT) {
                            htmltext = "30035-04a.html".to_owned();
                        }
                    }
                    2 | 3 => {
                        if ctx.has_quest_items(player, HARRYS_2ND_RECIEPT) {
                            htmltext = "30035-04.html".to_owned();
                        }
                    }
                    4 => {
                        if ctx.has_all_quest_items(player, &[HARRYS_2ND_RECIEPT, TOOL_BOX]) {
                            ctx.give_items(player, WOODEN_BREASTPLATE, 1);
                            ctx.add_exp_and_sp(player, 5_000, 0);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "30035-05.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ALTRAN => match qs.get_cond(ctx) {
                1 => {
                    if ctx.has_quest_items(player, HARRYS_1ST_RECIEPT) {
                        htmltext = "30283-01.html".to_owned();
                    }
                }
                2 => {
                    if ctx.has_quest_items(player, HARRYS_2ND_RECIEPT)
                        && ctx.get_quest_items_count(player, GOLEM_SHARD) < 5
                    {
                        htmltext = "30283-03.html".to_owned();
                    }
                }
                3 => {
                    if ctx.has_quest_items(player, HARRYS_2ND_RECIEPT)
                        && ctx.get_quest_items_count(player, GOLEM_SHARD) >= 5
                    {
                        ctx.take_items(player, GOLEM_SHARD, -1);
                        ctx.give_items(player, TOOL_BOX, 1);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30283-04.html".to_owned();
                    }
                }
                4 => {
                    if ctx.has_all_quest_items(player, &[HARRYS_2ND_RECIEPT, TOOL_BOX]) {
                        htmltext = "30283-05.html".to_owned();
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00152_ShardsOfGolem";

    const NPCS: &str = r#"
        <npc id="30035" type="Folk" level="1" name="Harrys"/>
        <npc id="30283" type="Folk" level="1" name="Altran"/>
        <npc id="20016" type="Monster" level="10" name="Stone Golem"/>"#;

    const ITEMS: &str = r#"
        <item id="23" type="Armor" name="Wooden Breastplate"/>
        <item id="1008" type="EtcItem" name="Harry's 1st Receipt"><set name="is_questitem" val="true"/></item>
        <item id="1009" type="EtcItem" name="Harry's 2nd Receipt"><set name="is_questitem" val="true"/></item>
        <item id="1010" type="EtcItem" name="Golem Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1011" type="EtcItem" name="Tool Box"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00152ShardsOfGolem);
        let pages = Pages::of(&Q00152ShardsOfGolem)
            .page("30035-02.htm", &["30035-03.htm"])
            .page("30283-01.html", &["30283-02.html"])
            .plain(&[
                "30035-01.htm",
                "30035-03.htm",
                "30035-04.html",
                "30035-04a.html",
                "30035-05.html",
                "30283-02.html",
                "30283-03.html",
                "30283-04.html",
                "30283-05.html",
            ])
            .npcs(&[HARRYS, ALTRAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let harrys = t.spawn(HARRYS, 20);
        let altran = t.spawn(ALTRAN, 40);
        (t, harrys, altran)
    }

    #[test]
    fn a_character_below_level_10_does_not_start() {
        let (mut t, harrys, _) = setup();
        t.enter(1, 9, vec![]);
        assert!(t.quest_page(1, harrys).contains("30035-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn receipts_shards_tool_box_and_the_reward() {
        let (mut t, harrys, altran) = setup();
        t.enter(1, 10, vec![]);
        assert!(t.quest_page(1, harrys).contains("30035-02.htm"));
        assert!(t.event(1, NAME, "30035-03.htm").contains("30035-03.htm"));
        assert_eq!(t.count(1, HARRYS_1ST_RECIEPT), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, harrys).contains("30035-04a.html"));

        // Altran swaps the receipts.
        assert!(t.quest_page(1, altran).contains("30283-01.html"));
        assert!(t.event(1, NAME, "30283-02.html").contains("30283-02.html"));
        assert_eq!(t.count(1, HARRYS_1ST_RECIEPT), 0);
        assert_eq!(t.count(1, HARRYS_2ND_RECIEPT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, altran).contains("30283-03.html"));
        assert!(t.quest_page(1, harrys).contains("30035-04.html"));

        // Five shards (30 % each) make cond 3.
        let mut kills = 0;
        while t.count(1, GOLEM_SHARD) < 5 {
            let golem = t.spawn(STONE_GOLEM, 60);
            t.kill(1, golem);
            kills += 1;
            assert!(kills < 500, "the golems never dropped 5 shards");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        assert!(t.quest_page(1, altran).contains("30283-04.html"));
        assert_eq!(t.count(1, GOLEM_SHARD), 0);
        assert_eq!(t.count(1, TOOL_BOX), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, altran).contains("30283-05.html"));

        assert!(t.quest_page(1, harrys).contains("30035-05.html"));
        assert_eq!(t.count(1, WOODEN_BREASTPLATE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
