//! Quest 613: Prove Your Courage! (Varka) (Ashas in the Varka Silenos Village, Hekaton Prime).
// Java: dist/game/data/scripts/quests/Q00613_ProveYourCourageVarka/Q00613_ProveYourCourageVarka.java
//
// Pattern: a one-kill quest, the same shape as Q00607 (`actionForEachPlayer` gives the head to the
// party members in range; the NPC rewards the feather and experience once).

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ASHAS: i32 = 31377;

// Monster
const HEKATON: i32 = 25299;

// Items
const HEKATON_HEAD: i32 = 7240;
const VALOR_FEATHER: i32 = 7229;
const VARKA_ALLIANCE_THREE: i32 = 7223;

// Misc
const MIN_LEVEL: i32 = 75;

/// Java `Q00613_ProveYourCourageVarka`.
pub struct Q00613ProveYourCourageVarka;

impl Script for Q00613ProveYourCourageVarka {
    fn name(&self) -> &'static str {
        "Q00613_ProveYourCourageVarka"
    }

    fn path(&self) -> &'static str {
        "quests/Q00613_ProveYourCourageVarka"
    }

    fn id(&self) -> i32 {
        613
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ASHAS]);
        s.add_talk_id(&[ASHAS]);
        s.add_kill_id(&[HEKATON]);
        s.register_quest_items(&[HEKATON_HEAD]);
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        if let Some(qs) = ctx.get_quest_state(player, false)
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
        {
            ctx.give_items(player, HEKATON_HEAD, 1);
            qs.set_cond(ctx, 2, true);
        }
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
        let mut htmltext = Some(event.to_owned());
        match event {
            "31377-04.htm" => {
                qs.start_quest(ctx);
            }
            "31377-07.html" => {
                if ctx.has_quest_items(player, HEKATON_HEAD) && qs.is_cond(ctx, 2) {
                    ctx.give_items(player, VALOR_FEATHER, 1);
                    ctx.add_exp_and_sp(player, 10000, 0);
                    qs.exit_quest(ctx, true, true);
                } else {
                    htmltext = Some(ctx.get_no_quest_msg(player));
                }
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    if ctx.has_quest_items(player, VARKA_ALLIANCE_THREE) {
                        "31377-01.htm"
                    } else {
                        "31377-02.htm"
                    }
                } else {
                    "31377-03.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, HEKATON_HEAD) {
                    "31377-05.html"
                } else {
                    "31377-06.html"
                }
                .to_owned();
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00613_ProveYourCourageVarka";

    const NPCS: &str = r#"
        <npc id="31377" type="Folk" level="70" name="Ashas"/>
        <npc id="25299" type="Monster" level="75" name="Hekaton Prime"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7240" type="EtcItem" name="Hekaton Prime's Head"><set name="is_questitem" val="true"/></item>
        <item id="7229" type="EtcItem" name="Valor Feather"/>
        <item id="7223" type="EtcItem" name="Varka Alliance 3"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00613ProveYourCourageVarka);
        let pages = Pages::of(&Q00613ProveYourCourageVarka)
            .page("31377-01.htm", &["31377-04.htm"])
            .plain(&[
                "31377-02.htm",
                "31377-03.htm",
                "31377-04.htm",
                "31377-06.html",
                "31377-07.html",
            ])
            .page("31377-05.html", &["31377-07.html"])
            .npcs(&[ASHAS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ashas = t.spawn(ASHAS, 20);
        (t, ashas)
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, ashas) = setup();
        t.enter(1, 74, vec![item(100, VARKA_ALLIANCE_THREE, 1)]);
        assert!(t.quest_page(1, ashas).contains("31377-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn without_the_alliance_item_the_quest_is_refused() {
        let (mut t, ashas) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, ashas).contains("31377-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_head_then_the_feather_and_experience() {
        let (mut t, ashas) = setup();
        t.enter(1, 75, vec![item(100, VARKA_ALLIANCE_THREE, 1)]);
        assert!(t.quest_page(1, ashas).contains("31377-01.htm"));
        assert!(t.event(1, NAME, "31377-04.htm").contains("31377-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ashas).contains("31377-06.html"));

        // The monster's kill gives the head and sets cond 2 (`setCond(2, true)`).
        let hekaton = t.spawn(HEKATON, 60);
        t.kill(1, hekaton);
        assert_eq!(t.count(1, HEKATON_HEAD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, ashas).contains("31377-05.html"));
        assert!(t.event(1, NAME, "31377-07.html").contains("31377-07.html"));
        // `exitQuest` removes the registered quest items: the head is gone.
        assert_eq!((t.count(1, VALOR_FEATHER), t.count(1, HEKATON_HEAD)), (1, 0));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_outside_the_range_gives_nothing() {
        let (mut t, ashas) = setup();
        t.enter(1, 75, vec![item(100, VARKA_ALLIANCE_THREE, 1)]);
        t.quest_page(1, ashas);
        t.event(1, NAME, "31377-04.htm");
        t.take_writes();
        let far = t
            .world
            .spawn_npc_at(
                HEKATON,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, HEKATON_HEAD), 0);
        assert!(t.take_writes().is_empty());
    }
}
