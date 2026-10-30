//! Quest 139: Shadow Fox - 1 (Mia; fragments and chests from the Tasaba lizardmen).
// Java: dist/game/data/scripts/quests/Q00139_ShadowFoxPart1/Q00139_ShadowFoxPart1.java
//
// Pattern: a one-NPC party-kill quest. The drop test reads the `talk` variable of the party
// member drawn, the items go to the killer (Java quirk).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MIA: i32 = 30896;

// Monsters
const MOBS: [i32; 4] = [
    20784, // Tasaba Lizardman
    20785, // Tasaba Lizardman Shaman
    21639, // Tasaba Lizardman
    21640, // Tasaba Lizardman Shaman
];

// Items
const FRAGMENT: i32 = 10345;
const CHEST: i32 = 10346;

// Misc
const MIN_LEVEL: i32 = 37;
const MAX_REWARD_LEVEL: i32 = 42;
const DROP_CHANCE: i32 = 68;

const TEMPLE_CHAMPION_PART2: &str = "Q00138_TempleChampionPart2";

/// Java `Q00139_ShadowFoxPart1`.
pub struct Q00139ShadowFoxPart1;

impl Script for Q00139ShadowFoxPart1 {
    fn name(&self) -> &'static str {
        "Q00139_ShadowFoxPart1"
    }

    fn path(&self) -> &'static str {
        "quests/Q00139_ShadowFoxPart1"
    }

    fn id(&self) -> i32 {
        139
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MIA]);
        s.add_talk_id(&[MIA]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[FRAGMENT, CHEST]);
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
            "30896-02.htm" => {
                if ctx.player_level(player) < MIN_LEVEL {
                    htmltext = Some("30896-03.htm".to_owned());
                }
            }
            "30896-04.htm" => qs.start_quest(ctx),
            "30896-11.html" => qs.set(ctx, "talk", "1"),
            "30896-13.html" => {
                qs.set_cond(ctx, 2, true);
                qs.unset(ctx, "talk");
            }
            "30896-17.html" => {
                if ctx.get_random(20) < 3 {
                    ctx.take_items(player, FRAGMENT, 10);
                    ctx.take_items(player, CHEST, 1);
                    return Some("30896-16.html".to_owned());
                }

                ctx.take_items(player, FRAGMENT, -1);
                ctx.take_items(player, CHEST, -1);
                qs.set(ctx, "talk", "1");
            }
            "30896-19.html" => {
                ctx.give_adena(player, 14_050, true);
                if ctx.player_level(player) <= MAX_REWARD_LEVEL {
                    ctx.add_exp_and_sp(player, 30_000, 2_000);
                }

                qs.exit_quest(ctx, false, true);
            }
            "30896-06.html" | "30896-07.html" | "30896-08.html" | "30896-09.html" | "30896-10.html"
            | "30896-12.html" | "30896-18.html" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(player, 2) else {
            return;
        };

        let Some(qs) = ctx.get_quest_state(member, false) else {
            return;
        };
        if !qs.is_set(ctx, "talk") && ctx.get_random(100) < DROP_CHANCE {
            let item_id = if ctx.get_random(11) == 0 { CHEST } else { FRAGMENT };
            // NOTE(java-quirk): the item goes to the killer, not to the member drawn.
            ctx.give_items(player, item_id, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let done = ctx
                    .player_quest_state(player, TEMPLE_CHAMPION_PART2)
                    .is_some_and(|q| q.is_completed());
                htmltext = if done { "30896-01.htm" } else { "30896-00.html" }.to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    htmltext = if qs.is_set(ctx, "talk") {
                        "30896-11.html"
                    } else {
                        "30896-05.html"
                    }
                    .to_owned();
                }
                2 => {
                    htmltext = if qs.is_set(ctx, "talk") {
                        "30896-18.html"
                    } else if ctx.get_quest_items_count(player, FRAGMENT) >= 10
                        && ctx.get_quest_items_count(player, CHEST) >= 1
                    {
                        "30896-15.html"
                    } else {
                        "30896-14.html"
                    }
                    .to_owned();
                }
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="30896" type="Folk" level="40" name="Mia"/>
        <npc id="20784" type="Monster" level="37" name="Tasaba Lizardman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10345" type="EtcItem" name="Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10346" type="EtcItem" name="Chest"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    const NAME: &str = "Q00139_ShadowFoxPart1";

    struct StandIn(&'static str);

    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn(TEMPLE_CHAMPION_PART2));
        registry.add(Q00139ShadowFoxPart1);
        let pages = Pages::of(&Q00139ShadowFoxPart1)
            .page("30896-01.htm", &["30896-02.htm"])
            .page("30896-02.htm", &["30896-04.htm"])
            .plain(&["30896-00.html", "30896-03.htm"])
            .page("30896-04.htm", &["30896-06.html"])
            .page("30896-05.html", &["30896-06.html"])
            .page("30896-06.html", &["30896-07.html"])
            .page("30896-07.html", &["30896-08.html"])
            .page("30896-08.html", &["30896-09.html"])
            .page("30896-09.html", &["30896-10.html"])
            .page("30896-10.html", &["30896-11.html"])
            .page("30896-11.html", &["30896-12.html"])
            .page("30896-12.html", &["30896-13.html"])
            .plain(&["30896-13.html", "30896-14.html"])
            .page("30896-15.html", &["30896-17.html"])
            .plain(&["30896-16.html"])
            .page("30896-17.html", &["30896-18.html"])
            .page("30896-18.html", &["30896-19.html"])
            .plain(&["30896-19.html"])
            .npcs(&[MIA])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mia = t.spawn(MIA, 20);
        (t, mia)
    }

    fn finish_part2(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(TEMPLE_CHAMPION_PART2, |ctx| {
            let q = ctx.get_quest_state(Oid(player), true).unwrap();
            q.start_quest(ctx);
            q.exit_quest(ctx, false, false);
        });
        t.take_writes();
    }

    #[test]
    fn quest_138_gates_the_start_and_level_37_sends_away_at_the_accept_page() {
        let (mut t, mia) = setup();
        t.enter(1, 36, vec![]);
        assert!(t.quest_page(1, mia).contains("30896-00.html"));
        finish_part2(&mut t, 1);
        assert!(t.quest_page(1, mia).contains("30896-01.htm"));
        // `30896-02.htm` below level 37 answers with page 03.
        assert!(t.event(1, NAME, "30896-02.htm").contains("30896-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_drops_then_the_chest_roll_and_the_reward() {
        let (mut t, mia) = setup();
        t.enter(1, 40, vec![]);
        finish_part2(&mut t, 1);
        assert!(t.quest_page(1, mia).contains("30896-01.htm"));
        assert!(t.event(1, NAME, "30896-02.htm").contains("30896-02.htm"));
        assert!(t.event(1, NAME, "30896-04.htm").contains("30896-04.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        for page in [
            "30896-06.html",
            "30896-07.html",
            "30896-08.html",
            "30896-09.html",
            "30896-10.html",
            "30896-11.html",
            "30896-12.html",
            "30896-13.html",
        ] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert!(t.take_writes().contains(&update(NAME, "cond", "2")));

        // Kills drop fragments (and now and then a chest) until there are 10 and 1.
        for _ in 0..2000 {
            if t.count(1, FRAGMENT) >= 10 && t.count(1, CHEST) >= 1 {
                break;
            }
            let mob = t.spawn(20784, 60);
            t.kill(1, mob);
        }
        assert!(t.count(1, FRAGMENT) >= 10 && t.count(1, CHEST) >= 1);
        assert!(t.quest_page(1, mia).contains("30896-15.html"));

        // The roll: 3 in 20 give page 16 (and take only 10 + 1); otherwise page 18 and all is taken.
        assert!(t.event(1, NAME, "30896-17.html").contains("30896-1"));
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set(ctx, "talk", "1");
        });
        t.take_writes();
        assert!(t.quest_page(1, mia).contains("30896-18.html"));
        assert!(t.event(1, NAME, "30896-19.html").contains("30896-19.html"));
        assert_eq!(t.count(1, 57), 14_050);
        assert!(
            t.take_writes()
                .ends_with(&[delete(NAME, false), update(NAME, "<state>", "Completed")])
        );
    }
}
