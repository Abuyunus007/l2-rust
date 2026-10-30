//! Quest 140: Shadow Fox - 2 (Kluck and Xenovia, dark crystals for the goddess sword cryptogram).
// Java: dist/game/data/scripts/quests/Q00140_ShadowFoxPart2/Q00140_ShadowFoxPart2.java
//
// Pattern: a party kill count at cond 3 (`getRandomPartyMember(player, 3)`), an item roll at the
// NPC (`30912-14.html`) and a `talk` flag that picks the page.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KLUCK: i32 = 30895;
const XENOVIA: i32 = 30912;

// Items
const DARK_CRYSTAL: i32 = 10347;
const DARK_OXYDE: i32 = 10348;
const CRYPTOGRAM_OF_THE_GODDESS_SWORD: i32 = 10349;

// Monsters and their drop chance
const MOBS: [(i32, i32); 4] = [
    (20789, 45),  // Crokian
    (20790, 58),  // Dailaon
    (20791, 100), // Crokian Warrior
    (20792, 92),  // Farhite
];

// Misc
const MIN_LEVEL: i32 = 37;
const MAX_REWARD_LEVEL: i32 = 42;
const CHANCE: i32 = 8;
const CRYSTAL_COUNT: i64 = 5;
const OXYDE_COUNT: i64 = 2;

/// Java `Q00139_ShadowFoxPart1.class.getSimpleName()`.
const Q00139: &str = "Q00139_ShadowFoxPart1";

/// Java `Q00140_ShadowFoxPart2`.
pub struct Q00140ShadowFoxPart2;

impl Script for Q00140ShadowFoxPart2 {
    fn name(&self) -> &'static str {
        "Q00140_ShadowFoxPart2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00140_ShadowFoxPart2"
    }

    fn id(&self) -> i32 {
        140
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLUCK]);
        s.add_talk_id(&[KLUCK, XENOVIA]);
        let mobs: Vec<i32> = MOBS.iter().map(|&(id, _)| id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[DARK_CRYSTAL, DARK_OXYDE, CRYPTOGRAM_OF_THE_GODDESS_SWORD]);
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
            "30895-05.html" | "30895-06.html" | "30912-03.html" | "30912-04.html" | "30912-05.html"
            | "30912-08.html" | "30895-10.html" => {}
            "30895-03.htm" => qs.start_quest(ctx),
            "30895-07.html" => qs.set_cond(ctx, 2, true),
            "30912-06.html" => qs.set(ctx, "talk", "1"),
            "30912-09.html" => {
                qs.unset(ctx, "talk");
                qs.set_cond(ctx, 3, true);
            }
            "30912-14.html" => {
                if ctx.get_random(10) < CHANCE {
                    if ctx.get_quest_items_count(player, DARK_OXYDE) < OXYDE_COUNT {
                        ctx.give_items(player, DARK_OXYDE, 1);
                        ctx.take_items(player, DARK_CRYSTAL, 5);
                        return Some("30912-12.html".to_owned());
                    }

                    ctx.give_items(player, CRYPTOGRAM_OF_THE_GODDESS_SWORD, 1);
                    ctx.take_items(player, DARK_CRYSTAL, -1);
                    ctx.take_items(player, DARK_OXYDE, -1);
                    qs.set_cond(ctx, 4, true);
                    return Some("30912-13.html".to_owned());
                }

                ctx.take_items(player, DARK_CRYSTAL, 5);
            }
            "30895-11.html" => {
                ctx.give_adena(player, 18_775, true);
                if ctx.player_level(player) <= MAX_REWARD_LEVEL {
                    ctx.add_exp_and_sp(player, 30_000, 2_000);
                }
                qs.exit_quest(ctx, false, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(player, 3) else {
            return;
        };

        let chance = MOBS.iter().find(|&&(id, _)| id == npc.id).map_or(0, |&(_, c)| c);
        if ctx.get_random(100) < chance {
            ctx.give_items(member, DARK_CRYSTAL, 1);
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            KLUCK => match qs.state(ctx) {
                CREATED => {
                    let done = ctx
                        .player_quest_state(player, Q00139)
                        .is_some_and(|s| s.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        if done { "30895-01.htm" } else { "30895-00.htm" }
                    } else {
                        "30895-02.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30895-04.html".to_owned(),
                    2 | 3 => htmltext = "30895-08.html".to_owned(),
                    4 => {
                        if qs.is_set(ctx, "talk") {
                            htmltext = "30895-10.html".to_owned();
                        } else {
                            ctx.take_items(player, CRYPTOGRAM_OF_THE_GODDESS_SWORD, -1);
                            qs.set(ctx, "talk", "1");
                            htmltext = "30895-09.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            XENOVIA => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30912-01.html".to_owned(),
                        2 => {
                            htmltext = if qs.is_set(ctx, "talk") {
                                "30912-07.html"
                            } else {
                                "30912-02.html"
                            }
                            .to_owned();
                        }
                        3 => {
                            htmltext = if ctx.get_quest_items_count(player, DARK_CRYSTAL) >= CRYSTAL_COUNT {
                                "30912-11.html"
                            } else {
                                "30912-10.html"
                            }
                            .to_owned();
                        }
                        4 => htmltext = "30912-15.html".to_owned(),
                        _ => {}
                    }
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
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00140_ShadowFoxPart2";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30895" type="Folk" level="1" name="Kluck"/>
        <npc id="30912" type="Folk" level="1" name="Xenovia"/>
        <npc id="20791" type="Monster" level="40" name="Crokian Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10347" type="EtcItem" name="Dark Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10348" type="EtcItem" name="Dark Oxyde"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10349" type="EtcItem" name="Cryptogram of the Goddess Sword"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00139_ShadowFoxPart1`, as far as this quest reads it: the state.
    struct ShadowFoxPart1;

    impl Script for ShadowFoxPart1 {
        fn name(&self) -> &'static str {
            Q00139
        }
        fn path(&self) -> &'static str {
            "quests/Q00139_ShadowFoxPart1"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00140ShadowFoxPart2);
        registry.add(ShadowFoxPart1);
        let pages = Pages::of(&Q00140ShadowFoxPart2)
            .page("30895-01.htm", &["30895-03.htm"])
            .page("30895-03.htm", &["30895-05.html"])
            .page("30895-05.html", &["30895-06.html"])
            .page("30895-06.html", &["30895-07.html"])
            .page("30895-09.html", &["30895-10.html"])
            .page("30895-10.html", &["30895-11.html"])
            .page("30912-02.html", &["30912-03.html"])
            .page("30912-03.html", &["30912-04.html"])
            .page("30912-04.html", &["30912-05.html"])
            .page("30912-05.html", &["30912-06.html"])
            .page("30912-06.html", &["30912-08.html"])
            .page("30912-07.html", &["30912-08.html"])
            .page("30912-08.html", &["30912-09.html"])
            .page("30912-11.html", &["30912-14.html"])
            .plain(&[
                "30895-00.htm",
                "30895-02.htm",
                "30895-04.html",
                "30895-07.html",
                "30895-08.html",
                "30895-11.html",
                "30912-01.html",
                "30912-09.html",
                "30912-10.html",
                "30912-12.html",
                "30912-13.html",
                "30912-14.html",
                "30912-15.html",
            ])
            .npcs(&[KLUCK, XENOVIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kluck = t.spawn(KLUCK, 20);
        let xenovia = t.spawn(XENOVIA, 40);
        (t, kluck, xenovia)
    }

    fn finish_part1(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(Q00139, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    #[test]
    fn level_and_the_previous_quest_pick_the_first_page() {
        let (mut t, kluck, _) = setup();
        t.enter(1, 36, vec![]);
        assert!(t.quest_page(1, kluck).contains("30895-02.htm"));
        t.enter(2, 37, vec![]);
        assert!(t.quest_page(2, kluck).contains("30895-00.htm"));
        t.enter(3, 37, vec![]);
        finish_part1(&mut t, 3);
        assert!(t.quest_page(3, kluck).contains("30895-01.htm"));
    }

    #[test]
    fn crystals_oxyde_cryptogram_and_the_reward() {
        let (mut t, kluck, xenovia) = setup();
        t.enter(1, 40, vec![]);
        finish_part1(&mut t, 1);
        assert!(t.quest_page(1, kluck).contains("30895-01.htm"));
        assert!(t.event(1, NAME, "30895-03.htm").contains("30895-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kluck).contains("30895-04.html"));
        assert!(t.quest_page(1, xenovia).contains("30912-01.html"));
        assert!(t.event(1, NAME, "30895-05.html").contains("30895-05.html"));
        assert!(t.event(1, NAME, "30895-06.html").contains("30895-06.html"));
        assert!(t.event(1, NAME, "30895-07.html").contains("30895-07.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, kluck).contains("30895-08.html"));

        // Xenovia: the `talk` flag moves her page.
        assert!(t.quest_page(1, xenovia).contains("30912-02.html"));
        for e in ["30912-03.html", "30912-04.html", "30912-05.html"] {
            assert!(t.event(1, NAME, e).contains(e));
        }
        assert!(t.event(1, NAME, "30912-06.html").contains("30912-06.html"));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "talk", "1")]);
        assert!(t.quest_page(1, xenovia).contains("30912-07.html"));
        assert!(t.event(1, NAME, "30912-08.html").contains("30912-08.html"));
        assert!(t.event(1, NAME, "30912-09.html").contains("30912-09.html"));
        assert_eq!(
            t.take_writes(),
            [rows::delete_var(NAME, "talk"), rows::update(NAME, "cond", "3")]
        );
        assert!(t.quest_page(1, xenovia).contains("30912-10.html"));

        // Hand in five crystals again and again until the oxyde count and the roll give the
        // cryptogram (cond 4).
        let mut rounds = 0;
        while t.count(1, CRYPTOGRAM_OF_THE_GODDESS_SWORD) == 0 {
            let mut kills = 0;
            while t.count(1, DARK_CRYSTAL) < CRYSTAL_COUNT {
                let mob = t.spawn(20791, 60);
                t.kill(1, mob);
                kills += 1;
                assert!(kills < 50);
            }
            t.take_writes();
            assert!(t.quest_page(1, xenovia).contains("30912-11.html"));
            t.event(1, NAME, "30912-14.html");
            rounds += 1;
            assert!(rounds < 200, "no cryptogram");
        }
        assert_eq!(t.count(1, DARK_CRYSTAL), 0);
        assert_eq!(t.count(1, DARK_OXYDE), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "4")));
        assert!(t.quest_page(1, xenovia).contains("30912-15.html"));

        // Kluck takes the cryptogram once (`talk` = 1), then the reward at level <= 42.
        assert!(t.quest_page(1, kluck).contains("30895-09.html"));
        assert_eq!(t.count(1, CRYPTOGRAM_OF_THE_GODDESS_SWORD), 0);
        assert_eq!(t.take_writes(), [rows::insert(NAME, "talk", "1")]);
        assert!(t.quest_page(1, kluck).contains("30895-10.html"));
        assert!(t.event(1, NAME, "30895-11.html").contains("30895-11.html"));
        assert_eq!(t.count(1, ADENA), 18_775);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
