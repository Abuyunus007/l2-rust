//! Quest 137: Temple Champion - 1 (Sylvain; 30 fragments, then the executor and missionary badges).
// Java: dist/game/data/scripts/quests/Q00137_TempleChampionPart1/Q00137_TempleChampionPart1.java
//
// Pattern: a one-NPC quest that echoes every event (Java `return event`), counts kills of the
// killer's own quest and tracks the dialogue in the `talk` variable.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SYLVAIN: i32 = 30070;
const MOBS: [i32; 6] = [
    20083, // Granite Golem
    20144, // Hangman Tree
    20199, // Amber Basilisk
    20200, // Strain
    20201, // Ghoul
    20202, // Dead Seeker
];

// Items
const FRAGMENT: i32 = 10340;
const EXECUTOR: i32 = 10334;
const MISSIONARY: i32 = 10339;

/// Java `Q00137_TempleChampionPart1`.
pub struct Q00137TempleChampionPart1;

impl Script for Q00137TempleChampionPart1 {
    fn name(&self) -> &'static str {
        "Q00137_TempleChampionPart1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00137_TempleChampionPart1"
    }

    fn id(&self) -> i32 {
        137
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SYLVAIN]);
        s.add_talk_id(&[SYLVAIN]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[FRAGMENT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        match event {
            "30070-02.htm" => qs.start_quest(ctx),
            "30070-05.html" => qs.set(ctx, "talk", "1"),
            "30070-06.html" => qs.set(ctx, "talk", "2"),
            "30070-08.html" => {
                qs.unset(ctx, "talk");
                qs.set_cond(ctx, 2, true);
            }
            "30070-16.html" => {
                if qs.is_cond(ctx, 3)
                    && ctx.has_quest_items(player, EXECUTOR)
                    && ctx.has_quest_items(player, MISSIONARY)
                {
                    ctx.take_items(player, EXECUTOR, -1);
                    ctx.take_items(player, MISSIONARY, -1);
                    ctx.give_adena(player, 69_146, true);
                    if ctx.player_level(player) < 41 {
                        ctx.add_exp_and_sp(player, 219_975, 13_047);
                    }

                    qs.exit_quest(ctx, false, true);
                }
            }
            _ => {}
        }

        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        if qs.is_started(ctx) && qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, FRAGMENT) < 30 {
            ctx.give_items(player, FRAGMENT, 1);
            if ctx.get_quest_items_count(player, FRAGMENT) >= 30 {
                qs.set_cond(ctx, 3, true);
            } else {
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        if qs.is_completed(ctx) {
            return Some(ctx.get_already_completed_msg(player));
        }

        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.get_cond(ctx) {
            1 => match qs.get_int(ctx, "talk") {
                1 => htmltext = "30070-05.html".to_owned(),
                2 => htmltext = "30070-06.html".to_owned(),
                _ => htmltext = "30070-03.html".to_owned(),
            },
            2 => htmltext = "30070-08.html".to_owned(),
            3 => {
                if qs.get_int(ctx, "talk") == 1 {
                    htmltext = "30070-10.html".to_owned();
                } else if ctx.get_quest_items_count(player, FRAGMENT) >= 30 {
                    qs.set(ctx, "talk", "1");
                    htmltext = "30070-09.html".to_owned();
                    ctx.take_items(player, FRAGMENT, -1);
                }
            }
            _ => {
                htmltext = if ctx.player_level(player) >= 35
                    && ctx.has_all_quest_items(player, &[EXECUTOR, MISSIONARY])
                {
                    "30070-01.htm"
                } else {
                    "30070-00.html"
                }
                .to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::item;
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="30070" type="Folk" level="40" name="Sylvain"/>
        <npc id="20083" type="Monster" level="30" name="Granite Golem"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10340" type="EtcItem" name="Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10334" type="EtcItem" name="Badge Temple Executor"/>
        <item id="10339" type="EtcItem" name="Badge Temple Missionary"/>"#;

    const NAME: &str = "Q00137_TempleChampionPart1";

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00137TempleChampionPart1);
        let pages = Pages::of(&Q00137TempleChampionPart1)
            .page("30070-01.htm", &["30070-02.htm"])
            .page("30070-02.htm", &["30070-05.html"])
            .plain(&["30070-00.html", "30070-03.html"])
            .page("30070-05.html", &["30070-06.html"])
            .page("30070-06.html", &["30070-08.html"])
            .plain(&["30070-08.html", "30070-10.html"])
            .page("30070-09.html", &["30070-16.html"])
            .plain(&["30070-16.html"])
            .npcs(&[SYLVAIN])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let sylvain = t.spawn(SYLVAIN, 20);
        (t, sylvain)
    }

    #[test]
    fn the_badges_and_level_35_gate_the_start() {
        let (mut t, sylvain) = setup();
        t.enter(1, 40, vec![]);
        assert!(t.quest_page(1, sylvain).contains("30070-00.html"));
        t.enter(2, 34, vec![item(300, EXECUTOR, 1), item(301, MISSIONARY, 1)]);
        assert!(t.quest_page(2, sylvain).contains("30070-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn thirty_fragments_then_the_badges_are_swapped_for_adena() {
        let (mut t, sylvain) = setup();
        t.enter(1, 40, vec![item(300, EXECUTOR, 1), item(301, MISSIONARY, 1)]);
        assert!(t.quest_page(1, sylvain).contains("30070-01.htm"));
        assert!(t.event(1, NAME, "30070-02.htm").contains("30070-02.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.event(1, NAME, "30070-05.html").contains("30070-05.html"));
        assert!(t.event(1, NAME, "30070-06.html").contains("30070-06.html"));
        assert!(t.event(1, NAME, "30070-08.html").contains("30070-08.html"));
        assert_eq!(
            t.take_writes(),
            [
                insert(NAME, "talk", "1"),
                update(NAME, "talk", "2"),
                // `unset("talk")` then `setCond(2)`.
                crate::testing::rows::delete_var(NAME, "talk"),
                update(NAME, "cond", "2")
            ]
        );

        for _ in 0..30 {
            let mob = t.spawn(20083, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, FRAGMENT), 30);
        assert!(t.take_writes().contains(&update(NAME, "cond", "3")));

        // Talk takes the fragments and sets "talk" 1; the last page pays.
        assert!(t.quest_page(1, sylvain).contains("30070-09.html"));
        assert_eq!(t.count(1, FRAGMENT), 0);
        t.take_writes();
        assert!(t.event(1, NAME, "30070-16.html").contains("30070-16.html"));
        assert_eq!(t.count(1, 57), 69_146);
        assert_eq!((t.count(1, EXECUTOR), t.count(1, MISSIONARY)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }
}
