//! Quest 126: The Name of Evil - 2 (Asamah, the three Kaimu, the Warrior's Grave and Shilen's Stone Statue).
// Java: dist/game/data/scripts/quests/Q00126_TheNameOfEvil2/Q00126_TheNameOfEvil2.java
//
// Pattern: a long chain of cond steps with three note puzzles at the Warrior's Grave (the notes
// are quest variables, cleared when the grave is talked to and after a wrong or complete try).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SHILENS_STONE_STATUE: i32 = 32109;
const MUSHIKA: i32 = 32114;
const ASAMAH: i32 = 32115;
const ULU_KAIMU: i32 = 32119;
const BALU_KAIMU: i32 = 32120;
const CHUTA_KAIMU: i32 = 32121;
const WARRIORS_GRAVE: i32 = 32122;

// Items
const GAZKH_FRAGMENT: i32 = 8782;
const BONE_POWDER: i32 = 8783;

// Reward
const ENCHANT_WEAPON_A: i32 = 729;

const THE_NAME_OF_EVIL1: &str = "Q00125_TheNameOfEvil1";

/// Java `qs.unset(..)` of every note in the list.
fn unset_all(ctx: &mut ScriptCtx<'_>, qs: Qs, names: &[&str]) {
    for name in names {
        qs.unset(ctx, name);
    }
}

/// Java `npc.broadcastPacket(new MagicSkillUse(npc, player, 5089, 1, 1000, 0))`.
fn sign(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) {
    ctx.broadcast_magic_skill_use(npc.object_id, npc.object_id, player, 5089, 1, 1000, 0);
}

/// Java `Q00126_TheNameOfEvil2`.
pub struct Q00126TheNameOfEvil2;

impl Script for Q00126TheNameOfEvil2 {
    fn name(&self) -> &'static str {
        "Q00126_TheNameOfEvil2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00126_TheNameOfEvil2"
    }

    fn id(&self) -> i32 {
        126
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ASAMAH]);
        s.add_talk_id(&[
            ASAMAH,
            ULU_KAIMU,
            BALU_KAIMU,
            CHUTA_KAIMU,
            WARRIORS_GRAVE,
            SHILENS_STONE_STATUE,
            MUSHIKA,
        ]);
        s.register_quest_items(&[GAZKH_FRAGMENT, BONE_POWDER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        match event {
            "32115-1.html" => qs.start_quest(ctx),
            "32115-1b.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                }
            }
            "32119-3.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                }
            }
            "32119-4.html" => {
                if qs.is_cond(ctx, 3) {
                    qs.set_cond(ctx, 4, true);
                }
            }
            "32119-4a.html" | "32119-5b.html" => {
                ctx.play_sound(player, QuestSound::EtcsoundElrokiSong1st);
            }
            "32119-5.html" => {
                if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, true);
                }
            }
            "32120-3.html" => {
                if qs.is_cond(ctx, 5) {
                    qs.set_cond(ctx, 6, true);
                }
            }
            "32120-4.html" => {
                if qs.is_cond(ctx, 6) {
                    qs.set_cond(ctx, 7, true);
                }
            }
            "32120-4a.html" | "32120-5b.html" => {
                ctx.play_sound(player, QuestSound::EtcsoundElrokiSong2nd);
            }
            "32120-5.html" => {
                if qs.is_cond(ctx, 7) {
                    qs.set_cond(ctx, 8, true);
                }
            }
            "32121-3.html" => {
                if qs.is_cond(ctx, 8) {
                    qs.set_cond(ctx, 9, true);
                }
            }
            "32121-4.html" => {
                if qs.is_cond(ctx, 9) {
                    qs.set_cond(ctx, 10, true);
                }
            }
            "32121-4a.html" | "32121-5b.html" => {
                ctx.play_sound(player, QuestSound::EtcsoundElrokiSong3rd);
            }
            "32121-5.html" => {
                if qs.is_cond(ctx, 10) {
                    ctx.give_items(player, GAZKH_FRAGMENT, 1);
                    qs.set_cond(ctx, 11, true);
                }
            }
            "32122-2a.html" => {
                if let Some(npc) = npc {
                    sign(ctx, npc, player);
                }
            }
            "32122-2d.html" => {
                ctx.take_items(player, GAZKH_FRAGMENT, -1);
            }
            "32122-3.html" => {
                if qs.is_cond(ctx, 12) {
                    qs.set_cond(ctx, 13, true);
                }
            }
            "32122-4.html" => {
                if qs.is_cond(ctx, 13) {
                    qs.set_cond(ctx, 14, true);
                }
            }
            "DO_One" => {
                qs.set(ctx, "DO", "1");
                return Some("32122-4d.html".to_owned());
            }
            "MI_One" => {
                qs.set(ctx, "MI", "1");
                return Some("32122-4f.html".to_owned());
            }
            "FA_One" => {
                qs.set(ctx, "FA", "1");
                return Some("32122-4h.html".to_owned());
            }
            "SOL_One" => {
                qs.set(ctx, "SOL", "1");
                return Some("32122-4j.html".to_owned());
            }
            "FA2_One" => {
                qs.set(ctx, "FA2", "1");
                let notes = ["DO", "MI", "FA", "SOL", "FA2"];
                if qs.is_cond(ctx, 14) && notes.iter().all(|n| qs.get_int(ctx, n) > 0) {
                    qs.set_cond(ctx, 15, true);
                    unset_all(ctx, qs, &notes);
                    return Some("32122-4n.html".to_owned());
                }

                unset_all(ctx, qs, &notes);
                return Some("32122-4m.html".to_owned());
            }
            "32122-4m.html" => unset_all(ctx, qs, &["DO", "MI", "FA", "SOL", "FA2"]),
            "FA_Two" => {
                qs.set(ctx, "FA", "1");
                return Some("32122-5a.html".to_owned());
            }
            "SOL_Two" => {
                qs.set(ctx, "SOL", "1");
                return Some("32122-5c.html".to_owned());
            }
            "TI_Two" => {
                qs.set(ctx, "TI", "1");
                return Some("32122-5e.html".to_owned());
            }
            "SOL2_Two" => {
                qs.set(ctx, "SOL2", "1");
                return Some("32122-5g.html".to_owned());
            }
            "FA2_Two" => {
                qs.set(ctx, "FA2", "1");
                let notes = ["FA", "SOL", "TI", "SOL2", "FA2"];
                if qs.is_cond(ctx, 15) && notes.iter().all(|n| qs.get_int(ctx, n) > 0) {
                    qs.set_cond(ctx, 16, true);
                    unset_all(ctx, qs, &notes);
                    return Some("32122-5j.html".to_owned());
                }

                unset_all(ctx, qs, &notes);
                return Some("32122-5i.html".to_owned());
            }
            "32122-5i.html" => unset_all(ctx, qs, &["FA", "SOL", "TI", "SOL2", "FA2"]),
            "SOL_Three" => {
                qs.set(ctx, "SOL", "1");
                return Some("32122-6a.html".to_owned());
            }
            "FA_Three" => {
                qs.set(ctx, "FA", "1");
                return Some("32122-6c.html".to_owned());
            }
            "MI_Three" => {
                qs.set(ctx, "MI", "1");
                return Some("32122-6e.html".to_owned());
            }
            "FA2_Three" => {
                qs.set(ctx, "FA2", "1");
                return Some("32122-6g.html".to_owned());
            }
            "MI2_Three" => {
                qs.set(ctx, "MI2", "1");
                let notes = ["SOL", "FA", "MI", "FA2", "MI2"];
                if qs.is_cond(ctx, 16) && notes.iter().all(|n| qs.get_int(ctx, n) > 0) {
                    qs.set_cond(ctx, 17, true);
                    unset_all(ctx, qs, &notes);
                    return Some("32122-6j.html".to_owned());
                }

                unset_all(ctx, qs, &notes);
                return Some("32122-6i.html".to_owned());
            }
            "32122-6i.html" => unset_all(ctx, qs, &["SOL", "FA", "MI", "FA2", "MI2"]),
            "32122-7.html" => {
                ctx.give_items(player, BONE_POWDER, 1);
                ctx.play_sound(player, QuestSound::EtcsoundElrokiSongFull);
                if let Some(npc) = npc {
                    sign(ctx, npc, player);
                }
            }
            "32122-8.html" => {
                if qs.is_cond(ctx, 17) {
                    qs.set_cond(ctx, 18, true);
                }
            }
            "32109-2.html" => {
                if qs.is_cond(ctx, 18) {
                    qs.set_cond(ctx, 19, true);
                }
            }
            "32109-3.html" => {
                if qs.is_cond(ctx, 19) {
                    ctx.take_items(player, BONE_POWDER, -1);
                    qs.set_cond(ctx, 20, true);
                }
            }
            "32115-4.html" => {
                if qs.is_cond(ctx, 20) {
                    qs.set_cond(ctx, 21, true);
                }
            }
            "32115-5.html" => {
                if qs.is_cond(ctx, 21) {
                    qs.set_cond(ctx, 22, true);
                }
            }
            "32114-2.html" => {
                if qs.is_cond(ctx, 22) {
                    qs.set_cond(ctx, 23, true);
                }
            }
            "32114-3.html" => {
                ctx.give_items(player, ENCHANT_WEAPON_A, 1);
                ctx.give_adena(player, 460_483, true);
                ctx.add_exp_and_sp(player, 1_015_973, 102_802);
                qs.exit_quest(ctx, false, true);
            }
            _ => {}
        }

        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;

        match npc.id {
            ASAMAH => match qs.state(ctx) {
                CREATED => {
                    if ctx.player_level(player) < 77 {
                        htmltext = "32115-0.htm".to_owned();
                    } else {
                        let done = ctx
                            .player_quest_state(player, THE_NAME_OF_EVIL1)
                            .is_some_and(|q| q.is_completed());
                        htmltext = if done { "32115-0a.htm" } else { "32115-0b.htm" }.to_owned();
                    }
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "32115-1d.html".to_owned(),
                    2 => htmltext = "32115-1c.html".to_owned(),
                    3..=19 => htmltext = "32115-2.html".to_owned(),
                    20 => htmltext = "32115-3.html".to_owned(),
                    21 => htmltext = "32115-4j.html".to_owned(),
                    22 => htmltext = "32115-5a.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ULU_KAIMU => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "32119-1.html".to_owned(),
                        2 => {
                            htmltext = "32119-2.html".to_owned();
                            sign(ctx, npc, player);
                        }
                        3 => htmltext = "32119-3c.html".to_owned(),
                        4 => htmltext = "32119-4c.html".to_owned(),
                        5 => htmltext = "32119-5a.html".to_owned(),
                        _ => {}
                    }
                }
            }
            BALU_KAIMU => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=4 => htmltext = "32120-1.html".to_owned(),
                        5 => {
                            htmltext = "32120-2.html".to_owned();
                            sign(ctx, npc, player);
                        }
                        6 => htmltext = "32120-3c.html".to_owned(),
                        7 => htmltext = "32120-4c.html".to_owned(),
                        _ => htmltext = "32120-5a.html".to_owned(),
                    }
                }
            }
            CHUTA_KAIMU => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=7 => htmltext = "32121-1.html".to_owned(),
                        8 => {
                            htmltext = "32121-2.html".to_owned();
                            sign(ctx, npc, player);
                        }
                        9 => htmltext = "32121-3e.html".to_owned(),
                        10 => htmltext = "32121-4e.html".to_owned(),
                        _ => htmltext = "32121-5a.html".to_owned(),
                    }
                }
            }
            WARRIORS_GRAVE => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=10 => htmltext = "32122-1.html".to_owned(),
                        11 => {
                            htmltext = "32122-2.html".to_owned();
                            qs.set_cond(ctx, 12, true);
                        }
                        12 => htmltext = "32122-2l.html".to_owned(),
                        13 => htmltext = "32122-3b.html".to_owned(),
                        14 => {
                            htmltext = "32122-4.html".to_owned();
                            unset_all(ctx, qs, &["DO", "MI", "FA", "SOL", "FA2"]);
                        }
                        15 => {
                            htmltext = "32122-5.html".to_owned();
                            unset_all(ctx, qs, &["FA", "SOL", "TI", "SOL2", "FA2"]);
                        }
                        16 => {
                            htmltext = "32122-6.html".to_owned();
                            unset_all(ctx, qs, &["SOL", "FA", "MI", "FA2", "MI2"]);
                        }
                        17 => {
                            htmltext = if ctx.has_quest_items(player, BONE_POWDER) {
                                "32122-7.html"
                            } else {
                                "32122-7b.html"
                            }
                            .to_owned();
                        }
                        18 => htmltext = "32122-8.html".to_owned(),
                        _ => htmltext = "32122-9.html".to_owned(),
                    }
                }
            }
            SHILENS_STONE_STATUE => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=17 => htmltext = "32109-1a.html".to_owned(),
                        18 => {
                            if ctx.has_quest_items(player, BONE_POWDER) {
                                htmltext = "32109-1.html".to_owned();
                            }
                        }
                        19 => htmltext = "32109-2l.html".to_owned(),
                        20 => htmltext = "32109-5.html".to_owned(),
                        _ => htmltext = "32109-4.html".to_owned(),
                    }
                }
            }
            MUSHIKA => {
                if qs.is_started(ctx) {
                    if qs.get_cond(ctx) < 22 {
                        htmltext = "32114-4.html".to_owned();
                    } else if qs.is_cond(ctx, 22) {
                        htmltext = "32114-1.html".to_owned();
                    } else {
                        htmltext = "32114-2.html".to_owned();
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::rows::{self, delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32109" type="Folk" level="75" name="Shilen's Stone Statue"/>
        <npc id="32114" type="Folk" level="75" name="Mushika"/>
        <npc id="32115" type="Folk" level="75" name="Asamah"/>
        <npc id="32119" type="Folk" level="75" name="Ulu Kaimu"/>
        <npc id="32122" type="Folk" level="75" name="Warrior's Grave"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="729" type="EtcItem" name="Scroll: Enchant Weapon (A)"/>
        <item id="8782" type="EtcItem" name="Fragment"><set name="is_questitem" val="true"/></item>
        <item id="8783" type="EtcItem" name="Bone Powder"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00126_TheNameOfEvil2";

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

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn(THE_NAME_OF_EVIL1));
        registry.add(Q00126TheNameOfEvil2);
        let pages = Pages::of(&Q00126TheNameOfEvil2)
            .page("32115-0a.htm", &["32115-1.html"])
            .plain(&["32115-0.htm", "32115-0b.htm", "32115-1.html"])
            .page("32115-1b.html", &[])
            .page("32115-1c.html", &["32115-1b.html"])
            .plain(&["32115-1d.html", "32115-2.html", "32115-3.html"])
            .page("32122-2.html", &["32122-2a.html"])
            .page("32114-1.html", &["32114-2.html"])
            .page("32114-2.html", &["32114-3.html"])
            .plain(&["32114-3.html"])
            .npcs(&[ASAMAH, MUSHIKA, WARRIORS_GRAVE, ULU_KAIMU, SHILENS_STONE_STATUE])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let asamah = t.spawn(ASAMAH, 20);
        let mushika = t.spawn(MUSHIKA, 30);
        let grave = t.spawn(WARRIORS_GRAVE, 40);
        (t, asamah, mushika, grave)
    }

    fn call(t: &mut ScriptTest, npc: Option<Oid>, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = npc.and_then(|n| ctx.script_npc(n));
            out = Q00126TheNameOfEvil2.on_event(ctx, event, npc, Some(Oid(1)));
        });
        out
    }

    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            if !qs.is_started(ctx) {
                qs.start_quest(ctx);
            }
            qs.set_cond(ctx, cond, false);
        });
        t.take_writes();
    }

    #[test]
    fn level_77_and_quest_125_gate_the_start() {
        let (mut t, asamah, _, _) = setup();
        t.enter(1, 76, vec![]);
        assert!(t.quest_page(1, asamah).contains("32115-0.htm"));
        t.enter(2, 77, vec![]);
        assert!(t.quest_page(2, asamah).contains("32115-0b.htm"));
        t.world.run_as_script(THE_NAME_OF_EVIL1, |ctx| {
            let q = ctx.get_quest_state(Oid(2), true).unwrap();
            q.start_quest(ctx);
            q.exit_quest(ctx, false, false);
        });
        t.take_writes();
        assert!(t.quest_page(2, asamah).contains("32115-0a.htm"));
        assert!(t.event(2, NAME, "32115-1.html").contains("32115-1.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::of(2, insert(NAME, "cond", "1")),
                rows::of(2, insert(NAME, "<state>", "Started"))
            ]
        );
        assert!(t.quest_page(2, asamah).contains("32115-1d.html"));
    }

    #[test]
    fn the_first_cond_steps_follow_the_page_links() {
        let (mut t, asamah, _, _) = setup();
        t.enter(1, 77, vec![]);
        set_cond(&mut t, 2);
        assert!(t.quest_page(1, asamah).contains("32115-1c.html"));
        // cond 2: the event only works at cond 1, so nothing is written here.
        assert!(t.event(1, NAME, "32115-1b.html").contains("32115-1b.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_grave_notes_open_cond_15_and_a_wrong_try_clears_them() {
        let (mut t, _, _, grave) = setup();
        t.enter(1, 77, vec![]);
        set_cond(&mut t, 14);
        assert_eq!(
            call(&mut t, Some(grave), "DO_One").as_deref(),
            Some("32122-4d.html")
        );
        assert_eq!(
            call(&mut t, Some(grave), "MI_One").as_deref(),
            Some("32122-4f.html")
        );
        // The fifth note comes before the others are all set: "wrong".
        assert_eq!(
            call(&mut t, Some(grave), "FA2_One").as_deref(),
            Some("32122-4m.html")
        );
        t.take_writes();
        for e in ["DO_One", "MI_One", "FA_One", "SOL_One"] {
            call(&mut t, Some(grave), e);
        }
        t.take_writes();
        assert_eq!(
            call(&mut t, Some(grave), "FA2_One").as_deref(),
            Some("32122-4n.html")
        );
        assert!(t.take_writes().contains(&update(NAME, "cond", "15")));
    }

    #[test]
    fn the_bone_powder_and_the_reward() {
        let (mut t, _, mushika, grave) = setup();
        t.enter(1, 77, vec![]);
        set_cond(&mut t, 17);
        assert_eq!(
            call(&mut t, Some(grave), "32122-7.html").as_deref(),
            Some("32122-7.html")
        );
        assert_eq!(t.count(1, BONE_POWDER), 1);
        assert_eq!(
            call(&mut t, None, "32122-8.html").as_deref(),
            Some("32122-8.html")
        );
        assert_eq!(
            call(&mut t, None, "32109-2.html").as_deref(),
            Some("32109-2.html")
        );
        assert_eq!(
            call(&mut t, None, "32109-3.html").as_deref(),
            Some("32109-3.html")
        );
        assert_eq!(t.count(1, BONE_POWDER), 0);
        set_cond(&mut t, 22);
        assert!(t.quest_page(1, mushika).contains("32114-1.html"));
        assert!(t.event(1, NAME, "32114-2.html").contains("32114-2.html"));
        // The test jumped over conds with `set_cond`, so a `__compltdStateFlags` row comes with it.
        assert!(t.take_writes().contains(&update(NAME, "cond", "23")));
        assert!(t.event(1, NAME, "32114-3.html").contains("32114-3.html"));
        assert_eq!(t.count(1, ENCHANT_WEAPON_A), 1);
        assert_eq!(t.count(1, 57), 460_483);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
    }
}
