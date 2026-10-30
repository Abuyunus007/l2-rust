//! Quest 141: Shadow Fox - 3 (Natools wants thirty reports of the predecessors).
// Java: dist/game/data/scripts/quests/Q00141_ShadowFoxPart3/Q00141_ShadowFoxPart3.java
//
// Pattern: a party kill count at cond 2 (`getRandomPartyMember(player, 2)`) and a `talk` memo that
// picks the page at cond 3. The reward hands the player over to Q00998_FallenAngelSelect.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const NATOOLS: i32 = 30894;

// Monsters and their drop chance
const MOBS: [(i32, i32); 3] = [
    (20135, 53),  // Alligator
    (20791, 100), // Crokian Warrior
    (20792, 92),  // Farhite
];

// Item
const PREDECESSORS_REPORT: i32 = 10350;

// Misc
const MIN_LEVEL: i32 = 37;
const MAX_REWARD_LEVEL: i32 = 42;
const REPORT_COUNT: i64 = 30;

/// Java `Q00140_ShadowFoxPart2.class.getSimpleName()`.
const Q00140: &str = "Q00140_ShadowFoxPart2";
/// Java `Q00998_FallenAngelSelect.class.getSimpleName()`.
const Q00998: &str = "Q00998_FallenAngelSelect";

/// Java `Q00141_ShadowFoxPart3`.
pub struct Q00141ShadowFoxPart3;

impl Script for Q00141ShadowFoxPart3 {
    fn name(&self) -> &'static str {
        "Q00141_ShadowFoxPart3"
    }

    fn path(&self) -> &'static str {
        "quests/Q00141_ShadowFoxPart3"
    }

    fn id(&self) -> i32 {
        141
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NATOOLS]);
        s.add_talk_id(&[NATOOLS]);
        let mobs: Vec<i32> = MOBS.iter().map(|&(id, _)| id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[PREDECESSORS_REPORT]);
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
            "30894-05.html" | "30894-10.html" | "30894-11.html" | "30894-12.html" | "30894-13.html"
            | "30894-14.html" | "30894-16.html" | "30894-17.html" | "30894-19.html" | "30894-20.html" => {}
            "30894-03.htm" => qs.start_quest(ctx),
            "30894-06.html" => qs.set_cond(ctx, 2, true),
            "30894-15.html" => qs.set(ctx, "talk", "2"),
            "30894-18.html" => {
                qs.set_cond(ctx, 4, true);
                qs.unset(ctx, "talk");
            }
            "30894-21.html" => {
                ctx.give_adena(player, 88_888, true);
                if ctx.player_level(player) <= MAX_REWARD_LEVEL {
                    ctx.add_exp_and_sp(player, 278_005, 17_058);
                }

                qs.exit_quest(ctx, false, true);

                // `q.newQuestState(player).setState(State.STARTED)`: the select quest is a custom
                // one, so this is the only way it gets started.
                if let Some(select) = ctx.quest_state_of(Q00998, player, true) {
                    select.set_state(ctx, STARTED);
                }
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(player, 2) else {
            return;
        };

        let qs = ctx.get_quest_state(member, false);
        let chance = MOBS.iter().find(|&&(id, _)| id == npc.id).map_or(0, |&(_, c)| c);
        if ctx.get_random(100) < chance {
            ctx.give_items(member, PREDECESSORS_REPORT, 1);
            if ctx.get_quest_items_count(member, PREDECESSORS_REPORT) >= REPORT_COUNT {
                if let Some(qs) = qs {
                    qs.set_cond(ctx, 3, true);
                }
            } else {
                ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let done = ctx
                    .player_quest_state(player, Q00140)
                    .is_some_and(|s| s.is_completed());
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    if done { "30894-01.htm" } else { "30894-00.html" }
                } else {
                    "30894-02.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30894-04.html".to_owned(),
                2 => htmltext = "30894-07.html".to_owned(),
                3 => {
                    if qs.get_int(ctx, "talk") == 1 {
                        htmltext = "30894-09.html".to_owned();
                    } else if qs.get_int(ctx, "talk") == 2 {
                        htmltext = "30894-16.html".to_owned();
                    } else {
                        htmltext = "30894-08.html".to_owned();
                        ctx.take_items(player, PREDECESSORS_REPORT, -1);
                        qs.set(ctx, "talk", "1");
                    }
                }
                4 => htmltext = "30894-19.html".to_owned(),
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
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00141_ShadowFoxPart3";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30894" type="Folk" level="1" name="Natools"/>
        <npc id="20791" type="Monster" level="40" name="Crokian Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10350" type="EtcItem" name="Predecessor's Report"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// A script the quest reads (Q00140) or starts (Q00998): no hooks of their own.
    struct Other(&'static str);

    impl Script for Other {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Other"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00141ShadowFoxPart3);
        registry.add(Other(Q00140));
        registry.add(Other(Q00998));
        let pages = Pages::of(&Q00141ShadowFoxPart3)
            .page("30894-01.htm", &["30894-03.htm"])
            .page("30894-03.htm", &["30894-05.html"])
            .page("30894-05.html", &["30894-06.html"])
            .page("30894-08.html", &["30894-10.html"])
            .page("30894-09.html", &["30894-10.html"])
            .page("30894-10.html", &["30894-11.html"])
            .page("30894-11.html", &["30894-12.html"])
            .page("30894-12.html", &["30894-13.html"])
            .page("30894-13.html", &["30894-14.html"])
            .page("30894-14.html", &["30894-15.html"])
            .page("30894-15.html", &["30894-16.html"])
            .page("30894-16.html", &["30894-17.html"])
            .page("30894-17.html", &["30894-18.html"])
            .page("30894-18.html", &["30894-19.html"])
            .page("30894-19.html", &["30894-20.html"])
            .page("30894-20.html", &["30894-21.html"])
            .plain(&[
                "30894-00.html",
                "30894-02.htm",
                "30894-04.html",
                "30894-06.html",
                "30894-07.html",
                "30894-21.html",
            ])
            .npcs(&[NATOOLS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let natools = t.spawn(NATOOLS, 20);
        (t, natools)
    }

    fn finish_part2(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(Q00140, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    #[test]
    fn level_and_the_previous_quest_pick_the_first_page() {
        let (mut t, natools) = setup();
        t.enter(1, 36, vec![]);
        assert!(t.quest_page(1, natools).contains("30894-02.htm"));
        t.enter(2, 37, vec![]);
        assert!(t.quest_page(2, natools).contains("30894-00.html"));
        t.enter(3, 37, vec![]);
        finish_part2(&mut t, 3);
        assert!(t.quest_page(3, natools).contains("30894-01.htm"));
    }

    #[test]
    fn reports_the_talk_memo_and_the_hand_over_to_the_select_quest() {
        let (mut t, natools) = setup();
        t.enter(1, 40, vec![]);
        finish_part2(&mut t, 1);
        assert!(t.quest_page(1, natools).contains("30894-01.htm"));
        assert!(t.event(1, NAME, "30894-03.htm").contains("30894-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, natools).contains("30894-04.html"));
        assert!(t.event(1, NAME, "30894-05.html").contains("30894-05.html"));
        assert!(t.event(1, NAME, "30894-06.html").contains("30894-06.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, natools).contains("30894-07.html"));

        // Thirty reports (the warrior drops 100 %): cond 3.
        for _ in 0..30 {
            let mob = t.spawn(20791, 60);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, PREDECESSORS_REPORT), 30);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // First visit takes the reports and sets `talk` = 1; the page chain sets it to 2.
        assert!(t.quest_page(1, natools).contains("30894-08.html"));
        assert_eq!(t.count(1, PREDECESSORS_REPORT), 0);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "talk", "1")]);
        for e in [
            "30894-10.html",
            "30894-11.html",
            "30894-12.html",
            "30894-13.html",
            "30894-14.html",
        ] {
            assert!(t.event(1, NAME, e).contains(e));
        }
        assert!(t.event(1, NAME, "30894-15.html").contains("30894-15.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "talk", "2")]);
        assert!(t.quest_page(1, natools).contains("30894-16.html"));
        assert!(t.event(1, NAME, "30894-17.html").contains("30894-17.html"));
        assert!(t.event(1, NAME, "30894-18.html").contains("30894-18.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "cond", "4"), rows::delete_var(NAME, "talk")]
        );
        assert!(t.quest_page(1, natools).contains("30894-19.html"));
        assert!(t.event(1, NAME, "30894-20.html").contains("30894-20.html"));
        assert!(t.event(1, NAME, "30894-21.html").contains("30894-21.html"));
        assert_eq!(t.count(1, ADENA), 88_888);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        // Q00998 now has a Started state.
        let mut state = None;
        t.world.run_as_script(NAME, |ctx| {
            state = ctx.player_quest_state(Oid(1), Q00998).map(|q| q.state());
        });
        assert_eq!(state, Some(STARTED));
    }
}
