//! Quest 111: Elrokian Hunter's Proof (Marquez, Mushika, Asamah, Kirikachin on the Primeval Isle).
// Java: dist/game/data/scripts/quests/Q00111_ElrokianHuntersProof/Q00111_ElrokianHuntersProof.java
//
// Pattern: a memo-state chain (`setMemoState`) with a table of monsters -> (item, chance, memo
// state), kills for a random started party member, and a reward with items, adena and exp.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MARQUEZ: i32 = 32113;
const MUSHIKA: i32 = 32114;
const ASAMAH: i32 = 32115;
const KIRIKACHIN: i32 = 32116;

// Items
const ELROKIAN_TRAP: i32 = 8763;
const TRAP_STONE: i32 = 8764;
const DIARY_FRAGMENT: i32 = 8768;
const EXPEDITION_MEMBERS_LETTER: i32 = 8769;
const ORNITHOMINUS_CLAW: i32 = 8770;
const DEINONYCHUS_BONE: i32 = 8771;
const PACHYCEPHALOSAURUS_SKIN: i32 = 8772;
const PRACTICE_ELROKIAN_TRAP: i32 = 8773;

// Misc
const MIN_LEVEL: i32 = 75;

/// Java `MOBS_DROP_CHANCES`: `(npc id, item, chance, count)`. NOTE(java-quirk): the holder's count
/// is not an amount here, it is the memo state in which the monster drops.
#[rustfmt::skip]
const MOBS_DROP_CHANCES: [(i32, i32, f64, i32); 20] = [
    (22196, DIARY_FRAGMENT, 0.51, 4),
    (22197, DIARY_FRAGMENT, 0.51, 4),
    (22198, DIARY_FRAGMENT, 0.51, 4),
    (22218, DIARY_FRAGMENT, 0.25, 4),
    (22223, DIARY_FRAGMENT, 0.26, 4),
    (22200, ORNITHOMINUS_CLAW, 0.66, 11),
    (22201, ORNITHOMINUS_CLAW, 0.33, 11),
    (22202, ORNITHOMINUS_CLAW, 0.66, 11),
    (22219, ORNITHOMINUS_CLAW, 0.33, 11),
    (22224, ORNITHOMINUS_CLAW, 0.33, 11),
    (22203, DEINONYCHUS_BONE, 0.65, 11),
    (22204, DEINONYCHUS_BONE, 0.32, 11),
    (22205, DEINONYCHUS_BONE, 0.66, 11),
    (22220, DEINONYCHUS_BONE, 0.32, 11),
    (22225, DEINONYCHUS_BONE, 0.32, 11),
    (22208, PACHYCEPHALOSAURUS_SKIN, 0.50, 11),
    (22209, PACHYCEPHALOSAURUS_SKIN, 0.50, 11),
    (22210, PACHYCEPHALOSAURUS_SKIN, 0.50, 11),
    (22221, PACHYCEPHALOSAURUS_SKIN, 0.49, 11),
    (22226, PACHYCEPHALOSAURUS_SKIN, 0.50, 11),
];

/// Java `Q00111_ElrokianHuntersProof`.
pub struct Q00111ElrokianHuntersProof;

impl Script for Q00111ElrokianHuntersProof {
    fn name(&self) -> &'static str {
        "Q00111_ElrokianHuntersProof"
    }

    fn path(&self) -> &'static str {
        "quests/Q00111_ElrokianHuntersProof"
    }

    fn id(&self) -> i32 {
        111
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MARQUEZ]);
        s.add_talk_id(&[MARQUEZ, MUSHIKA, ASAMAH, KIRIKACHIN]);
        let mobs: Vec<i32> = MOBS_DROP_CHANCES.iter().map(|m| m.0).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[
            DIARY_FRAGMENT,
            EXPEDITION_MEMBERS_LETTER,
            ORNITHOMINUS_CLAW,
            DEINONYCHUS_BONE,
            PACHYCEPHALOSAURUS_SKIN,
            PRACTICE_ELROKIAN_TRAP,
        ]);
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
            "32113-02.htm" | "32113-05.htm" | "32113-04.html" | "32113-10.html" | "32113-11.html"
            | "32113-12.html" | "32113-13.html" | "32113-14.html" | "32113-18.html" | "32113-19.html"
            | "32113-20.html" | "32113-21.html" | "32113-22.html" | "32113-23.html" | "32113-24.html"
            | "32115-08.html" | "32116-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "32113-03.html" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "32113-15.html" => {
                if qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32113-25.html" => {
                if qs.is_memo_state(ctx, 5) {
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 6, true);
                    ctx.give_items(player, EXPEDITION_MEMBERS_LETTER, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32115-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32115-06.html" => {
                if qs.is_memo_state(ctx, 9) {
                    qs.set_memo_state(ctx, 10);
                    qs.set_cond(ctx, 9, false);
                    ctx.play_sound(player, QuestSound::EtcsoundElrokiSongFull);
                    htmltext = Some(event.to_owned());
                }
            }
            "32115-09.html" => {
                if qs.is_memo_state(ctx, 10) {
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 10, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32116-04.html" => {
                if qs.is_memo_state(ctx, 7) {
                    qs.set_memo_state(ctx, 8);
                    ctx.play_sound(player, QuestSound::EtcsoundElrokiSongFull);
                    htmltext = Some(event.to_owned());
                }
            }
            "32116-07.html" => {
                if qs.is_memo_state(ctx, 8) {
                    qs.set_memo_state(ctx, 9);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32116-10.html" => {
                if qs.is_memo_state(ctx, 12) && ctx.has_quest_items(player, PRACTICE_ELROKIAN_TRAP) {
                    ctx.take_items(player, PRACTICE_ELROKIAN_TRAP, -1);
                    ctx.give_items(player, ELROKIAN_TRAP, 1);
                    ctx.give_items(player, TRAP_STONE, 100);
                    ctx.give_adena(player, 1_071_691, true);
                    ctx.add_exp_and_sp(player, 553_524, 55_538);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.random_party_member_state(player, -1, 3, Some(npc)) else {
            return;
        };
        let Some(&(_, item_id, chance, count)) = MOBS_DROP_CHANCES.iter().find(|m| m.0 == npc.id) else {
            return;
        };
        let member = qs.player();
        if count == qs.get_memo_state(ctx) {
            if qs.is_cond(ctx, 4)
                && ctx.give_item_randomly(member, Some(npc), item_id, 1, 1, 50, chance, true)
            {
                qs.set_cond(ctx, 5, false);
            } else if qs.is_cond(ctx, 10)
                && ctx.give_item_randomly(member, Some(npc), item_id, 1, 1, 10, chance, true)
                && ctx.get_quest_items_count(member, ORNITHOMINUS_CLAW) >= 10
                && ctx.get_quest_items_count(member, DEINONYCHUS_BONE) >= 10
                && ctx.get_quest_items_count(member, PACHYCEPHALOSAURUS_SKIN) >= 10
            {
                qs.set_cond(ctx, 11, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => {
                if npc.id == MARQUEZ {
                    htmltext = ctx.get_already_completed_msg(player);
                }
            }
            CREATED => {
                if npc.id == MARQUEZ {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "32113-01.htm"
                    } else {
                        "32113-06.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                MARQUEZ => match qs.get_memo_state(ctx) {
                    1 => htmltext = "32113-07.html".to_owned(),
                    2 => htmltext = "32113-08.html".to_owned(),
                    3 => htmltext = "32113-09.html".to_owned(),
                    4 => {
                        if ctx.get_quest_items_count(player, DIARY_FRAGMENT) < 50 {
                            htmltext = "32113-16.html".to_owned();
                        } else {
                            ctx.take_items(player, DIARY_FRAGMENT, -1);
                            qs.set_memo_state(ctx, 5);
                            htmltext = "32113-17.html".to_owned();
                        }
                    }
                    5 => htmltext = "32113-26.html".to_owned(),
                    6 => htmltext = "32113-27.html".to_owned(),
                    7 | 8 => htmltext = "32113-28.html".to_owned(),
                    9 => htmltext = "32113-29.html".to_owned(),
                    10..=12 => htmltext = "32113-30.html".to_owned(),
                    _ => {}
                },
                MUSHIKA => {
                    if qs.is_memo_state(ctx, 1) {
                        qs.set_cond(ctx, 2, true);
                        qs.set_memo_state(ctx, 2);
                        htmltext = "32114-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) > 1 && qs.get_memo_state(ctx) < 10 {
                        htmltext = "32114-02.html".to_owned();
                    } else {
                        htmltext = "32114-03.html".to_owned();
                    }
                }
                ASAMAH => match qs.get_memo_state(ctx) {
                    1 => htmltext = "32115-01.html".to_owned(),
                    2 => htmltext = "32115-02.html".to_owned(),
                    3..=8 => htmltext = "32115-04.html".to_owned(),
                    9 => htmltext = "32115-05.html".to_owned(),
                    10 => htmltext = "32115-07.html".to_owned(),
                    11 => {
                        if ctx.get_quest_items_count(player, ORNITHOMINUS_CLAW) < 10
                            || ctx.get_quest_items_count(player, DEINONYCHUS_BONE) < 10
                            || ctx.get_quest_items_count(player, PACHYCEPHALOSAURUS_SKIN) < 10
                        {
                            htmltext = "32115-10.html".to_owned();
                        } else {
                            qs.set_memo_state(ctx, 12);
                            qs.set_cond(ctx, 12, true);
                            ctx.give_items(player, PRACTICE_ELROKIAN_TRAP, 1);
                            ctx.take_items(player, ORNITHOMINUS_CLAW, -1);
                            ctx.take_items(player, DEINONYCHUS_BONE, -1);
                            ctx.take_items(player, PACHYCEPHALOSAURUS_SKIN, -1);
                            htmltext = "32115-11.html".to_owned();
                        }
                    }
                    12 => htmltext = "32115-12.html".to_owned(),
                    _ => {}
                },
                KIRIKACHIN => match qs.get_memo_state(ctx) {
                    1..=5 => htmltext = "32116-01.html".to_owned(),
                    6 => {
                        if ctx.has_quest_items(player, EXPEDITION_MEMBERS_LETTER) {
                            qs.set_memo_state(ctx, 7);
                            qs.set_cond(ctx, 7, true);
                            ctx.take_items(player, EXPEDITION_MEMBERS_LETTER, -1);
                            htmltext = "32116-02.html".to_owned();
                        }
                    }
                    7 => htmltext = "32116-05.html".to_owned(),
                    8 => htmltext = "32116-06.html".to_owned(),
                    9..=11 => htmltext = "32116-08.html".to_owned(),
                    12 => htmltext = "32116-09.html".to_owned(),
                    _ => {}
                },
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32113" type="Folk" level="70" name="Marquez"/>
        <npc id="32114" type="Folk" level="70" name="Mushika"/>
        <npc id="32115" type="Folk" level="70" name="Asamah"/>
        <npc id="32116" type="Folk" level="70" name="Kirikachin"/>
        <npc id="22197" type="Monster" level="75" name="Velociraptor"/>
        <npc id="22201" type="Monster" level="75" name="Ornithomimus"/>
        <npc id="22203" type="Monster" level="75" name="Deinonychus"/>
        <npc id="22208" type="Monster" level="75" name="Pachycephalosaurus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8763" type="EtcItem" name="Elrokian Trap"/>
        <item id="8764" type="EtcItem" name="Trap Stone"><set name="is_stackable" val="true"/></item>
        <item id="8768" type="EtcItem" name="Diary"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8769" type="EtcItem" name="Letter"><set name="is_questitem" val="true"/></item>
        <item id="8770" type="EtcItem" name="Claw"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8771" type="EtcItem" name="Bone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8772" type="EtcItem" name="Skin"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8773" type="EtcItem" name="Practice Trap"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00111_ElrokianHuntersProof";

    struct Npcs {
        marquez: Oid,
        mushika: Oid,
        asamah: Oid,
        kirikachin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00111ElrokianHuntersProof);
        // The links of the real pages: a chain of "next" links.
        let chain: [(&str, &str); 22] = [
            ("32113-09.html", "32113-10.html"),
            ("32113-10.html", "32113-11.html"),
            ("32113-11.html", "32113-12.html"),
            ("32113-12.html", "32113-13.html"),
            ("32113-13.html", "32113-14.html"),
            ("32113-14.html", "32113-15.html"),
            ("32113-17.html", "32113-18.html"),
            ("32113-18.html", "32113-19.html"),
            ("32113-19.html", "32113-20.html"),
            ("32113-20.html", "32113-21.html"),
            ("32113-21.html", "32113-22.html"),
            ("32113-22.html", "32113-23.html"),
            ("32113-23.html", "32113-24.html"),
            ("32113-24.html", "32113-25.html"),
            ("32115-02.html", "32115-03.html"),
            ("32115-05.html", "32115-06.html"),
            ("32115-07.html", "32115-08.html"),
            ("32115-08.html", "32115-09.html"),
            ("32116-02.html", "32116-03.html"),
            ("32116-03.html", "32116-04.html"),
            ("32116-06.html", "32116-07.html"),
            ("32116-09.html", "32116-10.html"),
        ];
        let mut pages = Pages::of(&Q00111ElrokianHuntersProof)
            .page("32113-01.htm", &["32113-02.htm"])
            .page("32113-02.htm", &["32113-03.html", "32113-05.htm"])
            .page("32113-03.html", &["32113-04.html"]);
        for (page, link) in chain {
            pages = pages.page(page, &[link]);
        }
        let pages = pages
            .plain(&[
                "32113-04.html",
                "32113-05.htm",
                "32113-06.html",
                "32113-07.html",
                "32113-08.html",
                "32113-15.html",
                "32113-16.html",
                "32113-25.html",
                "32113-26.html",
                "32113-27.html",
                "32113-30.html",
                "32114-01.html",
                "32114-02.html",
                "32114-03.html",
                "32115-01.html",
                "32115-03.html",
                "32115-04.html",
                "32115-06.html",
                "32115-09.html",
                "32115-10.html",
                "32115-11.html",
                "32115-12.html",
                "32116-01.html",
                "32116-04.html",
                "32116-07.html",
                "32116-10.html",
            ])
            .npcs(&[MARQUEZ, MUSHIKA, ASAMAH, KIRIKACHIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            marquez: t.spawn(MARQUEZ, 20),
            mushika: t.spawn(MUSHIKA, 25),
            asamah: t.spawn(ASAMAH, 30),
            kirikachin: t.spawn(KIRIKACHIN, 35),
        };
        (t, npcs)
    }

    fn hunt(t: &mut ScriptTest, mob: i32, item: i32, count: i64) {
        while t.count(1, item) < count {
            let m = t.spawn(mob, 60);
            t.kill(1, m);
        }
    }

    #[test]
    fn below_level_75_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, npcs.marquez).contains("32113-06.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, npcs.marquez).contains("32113-01.htm"));
        assert!(t.event(1, NAME, "32113-02.htm").contains("32113-02.htm"));
        assert!(t.event(1, NAME, "32113-03.html").contains("32113-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                insert(NAME, "cond", "1"),
                insert(NAME, "<state>", "Started"),
                insert(NAME, "memoState", "1")
            ]
        );

        // Mushika sends us to Asamah; Asamah asks for the hunt.
        assert!(t.quest_page(1, npcs.mushika).contains("32114-01.html"));
        assert_eq!(
            t.take_writes(),
            [update(NAME, "cond", "2"), update(NAME, "memoState", "2")]
        );
        assert!(t.quest_page(1, npcs.asamah).contains("32115-02.html"));
        assert!(t.event(1, NAME, "32115-03.html").contains("32115-03.html"));
        assert_eq!(
            t.take_writes(),
            [update(NAME, "memoState", "3"), update(NAME, "cond", "3")]
        );

        // Marquez gives the diary task (memo 4), 50 fragments from the velociraptors.
        assert!(t.quest_page(1, npcs.marquez).contains("32113-09.html"));
        for page in [
            "32113-10.html",
            "32113-11.html",
            "32113-12.html",
            "32113-13.html",
            "32113-14.html",
            "32113-15.html",
        ] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        t.take_writes();
        // The ornithomimus drops only at memo state 11: nothing now.
        let other = t.spawn(22201, 60);
        t.kill(1, other);
        assert_eq!(t.count(1, ORNITHOMINUS_CLAW), 0);
        hunt(&mut t, 22197, DIARY_FRAGMENT, 50);
        assert!(t.quest_page(1, npcs.marquez).contains("32113-17.html"));
        assert_eq!(t.count(1, DIARY_FRAGMENT), 0);
        for page in [
            "32113-18.html",
            "32113-19.html",
            "32113-20.html",
            "32113-21.html",
            "32113-22.html",
            "32113-23.html",
            "32113-24.html",
            "32113-25.html",
        ] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        assert_eq!(t.count(1, EXPEDITION_MEMBERS_LETTER), 1);

        // Kirikachin takes the letter, sings (memo 8), sends us back to Asamah (memo 9).
        assert!(t.quest_page(1, npcs.kirikachin).contains("32116-02.html"));
        assert_eq!(t.count(1, EXPEDITION_MEMBERS_LETTER), 0);
        assert!(t.event(1, NAME, "32116-03.html").contains("32116-03.html"));
        assert!(t.event(1, NAME, "32116-04.html").contains("32116-04.html"));
        assert!(t.quest_page(1, npcs.kirikachin).contains("32116-06.html"));
        assert!(t.event(1, NAME, "32116-07.html").contains("32116-07.html"));
        assert!(t.quest_page(1, npcs.asamah).contains("32115-05.html"));
        assert!(t.event(1, NAME, "32115-06.html").contains("32115-06.html"));
        assert!(t.quest_page(1, npcs.asamah).contains("32115-07.html"));
        assert!(t.event(1, NAME, "32115-08.html").contains("32115-08.html"));
        assert!(t.event(1, NAME, "32115-09.html").contains("32115-09.html"));
        t.take_writes();

        // Ten of each of the three trophies, the practice trap, then the reward.
        hunt(&mut t, 22201, ORNITHOMINUS_CLAW, 10);
        hunt(&mut t, 22203, DEINONYCHUS_BONE, 10);
        hunt(&mut t, 22208, PACHYCEPHALOSAURUS_SKIN, 10);
        assert!(t.quest_page(1, npcs.asamah).contains("32115-11.html"));
        assert_eq!(t.count(1, PRACTICE_ELROKIAN_TRAP), 1);
        assert_eq!(t.count(1, ORNITHOMINUS_CLAW), 0);
        assert!(t.quest_page(1, npcs.kirikachin).contains("32116-09.html"));
        t.take_writes();
        assert!(t.event(1, NAME, "32116-10.html").contains("32116-10.html"));
        assert_eq!(t.count(1, ELROKIAN_TRAP), 1);
        assert_eq!(t.count(1, TRAP_STONE), 100);
        assert_eq!(t.count(1, 57), 1_071_691);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, npcs.marquez).contains("already been completed"));
        let _ = npcs.mushika;
    }
}
