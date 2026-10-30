//! Quest 10292: Seven Signs, Girl of Doubt (Wood, Franz, Elcadia and Hardin in the Hideout of the Dawn).
// Java: dist/game/data/scripts/quests/Q10292_SevenSignsGirlOfDoubt/Q10292_SevenSignsGirlOfDoubt.java
//
// Pattern: a memo-state quest with a party kill (`docs/design/scripts.md`, "Kills, drops and
// parties"): the Cruel and Horrifying Golems drop Elcadia's marks to a random started member
// (`random_party_member_state`), the two Creatures of the Dusk (spawned by Elcadia's `SPAWN`
// event, once per NPC) count the kills. The memo state is the step, the marks are the check.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HARDIN: i32 = 30832;
const WOOD: i32 = 32593;
const FRANZ: i32 = 32597;
const ELCADIA: i32 = 32784;

// Item (Java `ELCADIAS_MARK` = ItemHolder(17226, 10))
const ELCADIAS_MARK: i32 = 17226;
const ELCADIAS_MARK_COUNT: i64 = 10;

// Misc
const MIN_LEVEL: i32 = 81;

// Variables
const I_QUEST1: &str = "I_QUEST1";

/// Java `Q00198_SevenSignsEmbryo.class.getSimpleName()`.
const SEVEN_SIGNS_EMBRYO: &str = "Q00198_SevenSignsEmbryo";

// Monsters
const CREATURE_OF_THE_DUSK1: i32 = 27422;
const CREATURE_OF_THE_DUSK2: i32 = 27424;
const MOBS: [i32; 6] = [
    22801, // Cruel Pincer Golem
    22802, // Cruel Pincer Golem
    22803, // Cruel Pincer Golem
    22804, // Horrifying Jackhammer Golem
    22805, // Horrifying Jackhammer Golem
    22806, // Horrifying Jackhammer Golem
];

/// Java `npc.getVariables().getBoolean(I_QUEST1, false)`.
fn quest1_flag(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    ctx.world()
        .npc_variables(npc.object_id)
        .is_some_and(|v| v.get_bool(I_QUEST1, false))
}

/// Java `Q10292_SevenSignsGirlOfDoubt`.
pub struct Q10292SevenSignsGirlOfDoubt;

impl Script for Q10292SevenSignsGirlOfDoubt {
    fn name(&self) -> &'static str {
        "Q10292_SevenSignsGirlOfDoubt"
    }

    fn path(&self) -> &'static str {
        "quests/Q10292_SevenSignsGirlOfDoubt"
    }

    fn id(&self) -> i32 {
        10292
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WOOD]);
        s.add_spawn_id(&[ELCADIA]);
        s.add_talk_id(&[WOOD, FRANZ, ELCADIA, HARDIN]);
        s.add_kill_id(&MOBS);
        s.add_kill_id(&[CREATURE_OF_THE_DUSK1, CREATURE_OF_THE_DUSK2]);
        s.register_quest_items(&[ELCADIAS_MARK]);
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
        match event {
            "32593-02.htm" | "32593-04.htm" | "32597-02.html" | "32597-04.html" => {
                htmltext = Some(event.to_owned());
            }
            "32593-03.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "32597-03.html" => {
                qs.set_memo_state(ctx, 2);
                qs.set_cond(ctx, 2, true);
                htmltext = Some(event.to_owned());
            }
            "32784-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32784-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32784-05.html" => {
                if qs.is_memo_state(ctx, 4) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32784-06.html" => {
                if qs.is_memo_state(ctx, 4) {
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "SPAWN" => {
                // Java `npc.getVariables()`: the event comes from Elcadia's page.
                if let Some(npc) = npc {
                    if !quest1_flag(ctx, npc) {
                        qs.set_int(ctx, "killCount", 0);
                        ctx.world_mut()
                            .npc_variables_mut(npc.object_id)
                            .set_bool(I_QUEST1, true);
                        // Java `addSpawn(.., getRandom(360), false, 0, false, player.getInstanceId())`:
                        // the heading is drawn before each spawn.
                        let instance = ctx.world().instance_id_of(player);
                        let heading = ctx.get_random(360);
                        ctx.add_spawn(
                            CREATURE_OF_THE_DUSK1,
                            89_440,
                            -238_016,
                            -9_632,
                            heading,
                            false,
                            0,
                            instance,
                        );
                        let heading = ctx.get_random(360);
                        ctx.add_spawn(
                            CREATURE_OF_THE_DUSK2,
                            89_524,
                            -238_131,
                            -9_632,
                            heading,
                            false,
                            0,
                            instance,
                        );
                    } else {
                        htmltext = Some("32784-07.html".to_owned());
                    }
                }
            }
            "32784-11.html" | "32784-12.html" => {
                if qs.is_memo_state(ctx, 6) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32784-13.html" => {
                if qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 7);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30832-02.html" => {
                if qs.is_memo_state(ctx, 7) {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30832-03.html" => {
                if qs.is_memo_state(ctx, 8) {
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        if MOBS.contains(&npc.id) {
            let member = qs.player();
            if ctx.give_item_randomly(
                member,
                Some(npc),
                ELCADIAS_MARK,
                1,
                ELCADIAS_MARK_COUNT,
                0,
                0.7,
                true,
            ) && qs.is_memo_state(ctx, 3)
            {
                qs.set_cond(ctx, 4, true);
            }
        } else {
            let kill_count = qs.get_int(ctx, "killCount") + 1;
            qs.set_int(ctx, "killCount", kill_count);
            if kill_count == 2 {
                qs.set_memo_state(ctx, 6);
                qs.set_cond(ctx, 6, false);
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut()
            .npc_variables_mut(npc.object_id)
            .set_bool(I_QUEST1, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_completed(ctx) {
            if npc.id == WOOD {
                htmltext = "32593-05.html".to_owned();
            }
        } else if qs.is_created(ctx) {
            let embryo_done = ctx
                .player_quest_state(player, SEVEN_SIGNS_EMBRYO)
                .is_some_and(|q| q.is_completed());
            htmltext = if ctx.player_level(player) >= MIN_LEVEL && embryo_done {
                "32593-01.htm"
            } else {
                "32593-06.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            match npc.id {
                WOOD => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state > 0 && memo_state < 9 {
                        htmltext = "32593-07.html".to_owned();
                    }
                }
                FRANZ => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state == 1 {
                        htmltext = "32597-01.html".to_owned();
                    } else if (2..7).contains(&memo_state) {
                        htmltext = "32597-05.html".to_owned();
                    } else if memo_state == 7 {
                        htmltext = "32597-06.html".to_owned();
                    }
                }
                ELCADIA => match qs.get_memo_state(ctx) {
                    2 => htmltext = "32784-01.html".to_owned(),
                    3 => {
                        if ctx.get_quest_items_count(player, ELCADIAS_MARK) < ELCADIAS_MARK_COUNT {
                            htmltext = "32784-03.html".to_owned();
                        } else {
                            ctx.take_items(player, ELCADIAS_MARK, ELCADIAS_MARK_COUNT);
                            qs.set_memo_state(ctx, 4);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "32784-04.html".to_owned();
                        }
                    }
                    4 => htmltext = "32784-08.html".to_owned(),
                    5 => htmltext = "32784-09.html".to_owned(),
                    6 => htmltext = "32784-10.html".to_owned(),
                    7 => htmltext = "32784-14.html".to_owned(),
                    8 => {
                        if ctx.is_sub_class_active(player) {
                            htmltext = "32784-15.html".to_owned();
                        } else {
                            ctx.add_exp_and_sp(player, 10_000_000, 1_000_000);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "32784-16.html".to_owned();
                        }
                    }
                    _ => {}
                },
                HARDIN => {
                    if qs.is_memo_state(ctx, 7) {
                        htmltext = "30832-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 8) {
                        htmltext = "30832-03.html".to_owned();
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q10292_SevenSignsGirlOfDoubt";

    const NPCS: &str = r#"
        <npc id="30832" type="Folk" level="80" name="Hardin"/>
        <npc id="32593" type="Folk" level="80" name="Wood"/>
        <npc id="32597" type="Folk" level="80" name="Franz"/>
        <npc id="32784" type="Folk" level="80" name="Elcadia"/>
        <npc id="22801" type="Monster" level="60" name="Cruel Pincer Golem"/>
        <npc id="27422" type="Monster" level="60" name="Creature of the Dusk"/>
        <npc id="27424" type="Monster" level="60" name="Creature of the Dusk"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="17226" type="EtcItem" name="Elcadia's Mark"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// Stands in for Q00198 (its completion is read through `player_quest_state` only).
    struct SevenSignsEmbryoStandIn;

    impl Script for SevenSignsEmbryoStandIn {
        fn name(&self) -> &'static str {
            SEVEN_SIGNS_EMBRYO
        }

        fn path(&self) -> &'static str {
            "quests/Q00198_SevenSignsEmbryo"
        }

        fn id(&self) -> i32 {
            198
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    struct Npcs {
        wood: Oid,
        franz: Oid,
        elcadia: Oid,
        hardin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10292SevenSignsGirlOfDoubt);
        registry.add(SevenSignsEmbryoStandIn);
        let pages = Pages::of(&Q10292SevenSignsGirlOfDoubt)
            .page("32593-01.htm", &["32593-02.htm"])
            .page("32593-02.htm", &["32593-03.htm", "32593-04.htm"])
            .plain(&[
                "32593-03.htm",
                "32593-04.htm",
                "32593-05.html",
                "32593-06.htm",
                "32593-07.html",
            ])
            .page("32597-01.html", &["32597-02.html"])
            .page("32597-02.html", &["32597-03.html", "32597-04.html"])
            .plain(&["32597-03.html", "32597-04.html", "32597-05.html", "32597-06.html"])
            .page("32784-01.html", &["32784-02.html"])
            .page("32784-02.html", &["32784-03.html"])
            .plain(&["32784-03.html"])
            .page("32784-04.html", &["32784-05.html"])
            .page("32784-05.html", &["32784-06.html"])
            .page("32784-06.html", &["SPAWN"])
            .plain(&["32784-07.html", "32784-14.html", "32784-15.html", "32784-16.html"])
            .page("32784-08.html", &["SPAWN"])
            .page("32784-10.html", &["32784-11.html"])
            .page("32784-11.html", &["32784-12.html"])
            .page("32784-12.html", &["32784-13.html"])
            .plain(&["32784-13.html", "32784-09.html"])
            .page("30832-01.html", &["30832-02.html"])
            .page("30832-02.html", &["30832-03.html"])
            .plain(&["30832-03.html"])
            .npcs(&[HARDIN, WOOD, FRANZ, ELCADIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            wood: t.spawn(WOOD, 20),
            franz: t.spawn(FRANZ, 30),
            elcadia: t.spawn(ELCADIA, 40),
            hardin: t.spawn(HARDIN, 50),
        };
        (t, npcs)
    }

    /// A character started at `cond`/`memo` with `marks` of Elcadia, and Q00198 done.
    fn started_at(
        t: &mut ScriptTest,
        id: i32,
        level: i32,
        cond: &str,
        memo: &str,
        marks: Vec<l2_world::ItemData>,
    ) {
        let mut data = player_data(id, level, marks);
        data.profile.quests = QuestLoad {
            states: vec![
                (SEVEN_SIGNS_EMBRYO.to_owned(), Some("Completed".to_owned())),
                (NAME.to_owned(), Some("Started".to_owned())),
            ],
            variables: vec![
                (NAME.to_owned(), "cond".to_owned(), Some(cond.to_owned())),
                (NAME.to_owned(), "memoState".to_owned(), Some(memo.to_owned())),
            ],
        };
        t.enter_with(data);
    }

    #[test]
    fn wood_starts_the_quest_at_81_with_the_embryo() {
        let (mut t, npcs) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, npcs.wood).contains("32593-06.htm"));
        assert!(t.take_writes().is_empty());
        let mut data = player_data(3, 81, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(SEVEN_SIGNS_EMBRYO.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
        assert!(t.quest_page(3, npcs.wood).contains("32593-01.htm"));
        assert!(t.event(3, NAME, "32593-02.htm").contains("32593-02.htm"));
        assert!(t.event(3, NAME, "32593-03.htm").contains("32593-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::of(3, rows::insert(NAME, "cond", "1")),
                rows::of(3, rows::insert(NAME, "<state>", "Started")),
                rows::of(3, rows::insert(NAME, "memoState", "1")),
            ]
        );
    }

    #[test]
    fn the_marks_the_spawn_and_the_two_kills_move_the_quest_on() {
        let (mut t, npcs) = setup();
        // Memo state 3 (Elcadia wants the marks), no mark yet.
        started_at(&mut t, 1, 81, "3", "3", vec![]);
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-03.html"));

        // The golems drop the marks to the killer (one or more at a time).
        for _ in 0..200 {
            if t.count(1, ELCADIAS_MARK) >= ELCADIAS_MARK_COUNT {
                break;
            }
            let golem = t.spawn(22801, 60);
            t.kill(1, golem);
        }
        let marks = t.count(1, ELCADIAS_MARK);
        assert!(marks >= ELCADIAS_MARK_COUNT, "{marks}");
        t.take_writes();

        // Elcadia takes ten marks and the quest goes to cond 4 / memo 4.
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-04.html"));
        assert_eq!(t.count(1, ELCADIAS_MARK), marks - ELCADIAS_MARK_COUNT);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4")
            ]
        );
        assert!(t.event(1, NAME, "32784-05.html").contains("32784-05.html"));
        assert!(t.event(1, NAME, "32784-06.html").contains("32784-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5")
            ]
        );

        // The spawn: two creatures of the dusk, one time only.
        assert!(t.event(1, NAME, "SPAWN").is_empty());
        assert_eq!(t.npcs_of(CREATURE_OF_THE_DUSK1).len(), 1);
        assert_eq!(t.npcs_of(CREATURE_OF_THE_DUSK2).len(), 1);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "killCount", "0")]);

        // The two kills (the spawned ones are far away: the test kills its own near the player).
        let dusk1 = t.spawn(CREATURE_OF_THE_DUSK1, 60);
        t.kill(1, dusk1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "killCount", "1")]);
        let dusk2 = t.spawn(CREATURE_OF_THE_DUSK2, 60);
        t.kill(1, dusk2);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "killCount", "2"),
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "6"),
            ]
        );

        // Elcadia, Franz and Hardin, to the end.
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-10.html"));
        assert!(t.event(1, NAME, "32784-11.html").contains("32784-11.html"));
        assert!(t.event(1, NAME, "32784-12.html").contains("32784-12.html"));
        assert!(t.event(1, NAME, "32784-13.html").contains("32784-13.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "7"),
                rows::update(NAME, "cond", "7")
            ]
        );
        assert!(t.quest_page(1, npcs.franz).contains("32597-06.html"));
        assert!(t.quest_page(1, npcs.hardin).contains("30832-01.html"));
        assert!(t.event(1, NAME, "30832-02.html").contains("30832-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "8"),
                rows::update(NAME, "cond", "8")
            ]
        );
        assert!(t.quest_page(1, npcs.hardin).contains("30832-03.html"));

        // Elcadia pays the experience (no sub-class) and the quest ends.
        assert!(t.quest_page(1, npcs.elcadia).contains("32784-16.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
