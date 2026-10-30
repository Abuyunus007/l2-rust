//! Quest 607: Prove Your Courage! (Ketra) (Kadun in the Ketra Orc Village, Shadith).
// Java: dist/game/data/scripts/quests/Q00607_ProveYourCourageKetra/Q00607_ProveYourCourageKetra.java
//
// Pattern: a one-kill quest. The monster's party members in range get the head through
// `actionForEachPlayer` (`executeForEachPlayer(killer, npc, isSummon, true, false)`), and the
// NPC rewards the totem and experience once.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KADUN: i32 = 31370;

// Monster
const SHADITH: i32 = 25309;

// Items
const SHADITH_HEAD: i32 = 7235;
const VALOR_TOTEM: i32 = 7219;
const KETRA_ALLIANCE_THREE: i32 = 7213;

// Misc
const MIN_LEVEL: i32 = 75;

/// Java `Q00607_ProveYourCourageKetra`.
pub struct Q00607ProveYourCourageKetra;

impl Script for Q00607ProveYourCourageKetra {
    fn name(&self) -> &'static str {
        "Q00607_ProveYourCourageKetra"
    }

    fn path(&self) -> &'static str {
        "quests/Q00607_ProveYourCourageKetra"
    }

    fn id(&self) -> i32 {
        607
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KADUN]);
        s.add_talk_id(&[KADUN]);
        s.add_kill_id(&[SHADITH]);
        s.register_quest_items(&[SHADITH_HEAD]);
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
            ctx.give_items(player, SHADITH_HEAD, 1);
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
            "31370-04.htm" => {
                qs.start_quest(ctx);
            }
            "31370-07.html" => {
                if ctx.has_quest_items(player, SHADITH_HEAD) && qs.is_cond(ctx, 2) {
                    ctx.give_items(player, VALOR_TOTEM, 1);
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
                    if ctx.has_quest_items(player, KETRA_ALLIANCE_THREE) {
                        "31370-01.htm"
                    } else {
                        "31370-02.htm"
                    }
                } else {
                    "31370-03.htm"
                }
                .to_owned();
            }
            STARTED => {
                htmltext = if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, SHADITH_HEAD) {
                    "31370-05.html"
                } else {
                    "31370-06.html"
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

    const NAME: &str = "Q00607_ProveYourCourageKetra";

    const NPCS: &str = r#"
        <npc id="31370" type="Folk" level="70" name="Kadun"/>
        <npc id="25309" type="Monster" level="75" name="Shadith"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7235" type="EtcItem" name="Shadith's Head"><set name="is_questitem" val="true"/></item>
        <item id="7219" type="EtcItem" name="Valor Totem"/>
        <item id="7213" type="EtcItem" name="Ketra Alliance 3"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00607ProveYourCourageKetra);
        let pages = Pages::of(&Q00607ProveYourCourageKetra)
            .page("31370-01.htm", &["31370-04.htm"])
            .plain(&[
                "31370-02.htm",
                "31370-03.htm",
                "31370-04.htm",
                "31370-06.html",
                "31370-07.html",
            ])
            .page("31370-05.html", &["31370-07.html"])
            .npcs(&[KADUN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kadun = t.spawn(KADUN, 20);
        (t, kadun)
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, kadun) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, kadun).contains("31370-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn without_the_alliance_item_the_quest_is_refused() {
        let (mut t, kadun) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, kadun).contains("31370-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_head_then_the_totem_and_experience() {
        let (mut t, kadun) = setup();
        t.enter(1, 75, vec![item(100, KETRA_ALLIANCE_THREE, 1)]);
        assert!(t.quest_page(1, kadun).contains("31370-01.htm"));
        assert!(t.event(1, NAME, "31370-04.htm").contains("31370-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kadun).contains("31370-06.html"));

        // The monster's kill gives the head and sets cond 2 (`setCond(2, true)`).
        let shadith = t.spawn(SHADITH, 60);
        t.kill(1, shadith);
        assert_eq!(t.count(1, SHADITH_HEAD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, kadun).contains("31370-05.html"));
        assert!(t.event(1, NAME, "31370-07.html").contains("31370-07.html"));
        // `exitQuest` removes the registered quest items: the head is gone.
        assert_eq!((t.count(1, VALOR_TOTEM), t.count(1, SHADITH_HEAD)), (1, 0));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_outside_the_range_gives_nothing() {
        let (mut t, kadun) = setup();
        t.enter(1, 75, vec![item(100, KETRA_ALLIANCE_THREE, 1)]);
        t.quest_page(1, kadun);
        t.event(1, NAME, "31370-04.htm");
        t.take_writes();
        let far = t
            .world
            .spawn_npc_at(
                SHADITH,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, SHADITH_HEAD), 0);
        assert!(t.take_writes().is_empty());
    }
}
