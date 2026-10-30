//! Quest 608: Slay the Enemy Commander! (Ketra) (Kadun in the Ketra Orc Village, Mos).
// Java: dist/game/data/scripts/quests/Q00608_SlayTheEnemyCommanderKetra/Q00608_SlayTheEnemyCommanderKetra.java
//
// Pattern: a one-kill quest, the same shape as Q00607 (`actionForEachPlayer` gives the head to the
// party members in range; the NPC rewards the totem and experience once).

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KADUN: i32 = 31370;

// Monster
const MOS: i32 = 25312;

// Items
const MOS_HEAD: i32 = 7236;
const WISDOM_TOTEM: i32 = 7220;
const KETRA_ALLIANCE_FOUR: i32 = 7214;

// Misc
const MIN_LEVEL: i32 = 75;

/// Java `Q00608_SlayTheEnemyCommanderKetra`.
pub struct Q00608SlayTheEnemyCommanderKetra;

impl Script for Q00608SlayTheEnemyCommanderKetra {
    fn name(&self) -> &'static str {
        "Q00608_SlayTheEnemyCommanderKetra"
    }

    fn path(&self) -> &'static str {
        "quests/Q00608_SlayTheEnemyCommanderKetra"
    }

    fn id(&self) -> i32 {
        608
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KADUN]);
        s.add_talk_id(&[KADUN]);
        s.add_kill_id(&[MOS]);
        s.register_quest_items(&[MOS_HEAD]);
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
            ctx.give_items(player, MOS_HEAD, 1);
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
                if ctx.has_quest_items(player, MOS_HEAD) && qs.is_cond(ctx, 2) {
                    ctx.give_items(player, WISDOM_TOTEM, 1);
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
                    if ctx.has_quest_items(player, KETRA_ALLIANCE_FOUR) {
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
                htmltext = if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, MOS_HEAD) {
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

    const NAME: &str = "Q00608_SlayTheEnemyCommanderKetra";

    const NPCS: &str = r#"
        <npc id="31370" type="Folk" level="70" name="Kadun"/>
        <npc id="25312" type="Monster" level="75" name="Mos"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7236" type="EtcItem" name="Mos's Head"><set name="is_questitem" val="true"/></item>
        <item id="7220" type="EtcItem" name="Wisdom Totem"/>
        <item id="7214" type="EtcItem" name="Ketra Alliance 4"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00608SlayTheEnemyCommanderKetra);
        let pages = Pages::of(&Q00608SlayTheEnemyCommanderKetra)
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
        t.enter(1, 74, vec![item(100, KETRA_ALLIANCE_FOUR, 1)]);
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
        t.enter(1, 75, vec![item(100, KETRA_ALLIANCE_FOUR, 1)]);
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
        let mos = t.spawn(MOS, 60);
        t.kill(1, mos);
        assert_eq!(t.count(1, MOS_HEAD), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, kadun).contains("31370-05.html"));
        assert!(t.event(1, NAME, "31370-07.html").contains("31370-07.html"));
        // `exitQuest` removes the registered quest items: the head is gone.
        assert_eq!((t.count(1, WISDOM_TOTEM), t.count(1, MOS_HEAD)), (1, 0));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn a_kill_outside_the_range_gives_nothing() {
        let (mut t, kadun) = setup();
        t.enter(1, 75, vec![item(100, KETRA_ALLIANCE_FOUR, 1)]);
        t.quest_page(1, kadun);
        t.event(1, NAME, "31370-04.htm");
        t.take_writes();
        let far = t
            .world
            .spawn_npc_at(
                MOS,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, MOS_HEAD), 0);
        assert!(t.take_writes().is_empty());
    }
}
