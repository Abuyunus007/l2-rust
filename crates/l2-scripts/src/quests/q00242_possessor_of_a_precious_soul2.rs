//! Quest 242: Possessor of a Precious Soul 2 (Virgil sends a sub-class character through
//! Kassandra, Ogmar and the Mysterious Knight to the Angel Corpse, Kalis and Matild, then to the
//! unicorns, whose cornerstones need four Orbs of Binding from the Restrainers of Glory).
// Java: dist/game/data/scripts/quests/Q00242_PossessorOfAPreciousSoul2/Q00242_PossessorOfAPreciousSoul2.java
//
// Pattern: a sub-class quest (`no_sub.html`), a per-quest variable that picks a random party
// member (`awaitsDrops`), NPCs that die and cast, spawns made by timers.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const VIRGIL: i32 = 31742;
const KASSANDRA: i32 = 31743;
const OGMAR: i32 = 31744;
const FALLEN_UNICORN: i32 = 31746;
const PURE_UNICORN: i32 = 31747;
const CORNERSTONE: i32 = 31748;
const MYSTERIOUS_KNIGHT: i32 = 31751;
const ANGEL_CORPSE: i32 = 31752;
const KALIS: i32 = 30759;
const MATILD: i32 = 30738;
const RESTRAINER_OF_GLORY: i32 = 27317;

// Items
const VIRGILS_LETTER: i32 = 7677;
const GOLDEN_HAIR: i32 = 7590;
const ORB_OF_BINDING: i32 = 7595;
const SORCERY_INGREDIENT: i32 = 7596;
const CARADINE_LETTER: i32 = 7678;

// Rewards
const CHANCE_FOR_HAIR: i32 = 20;

/// Java `Q00242_PossessorOfAPreciousSoul2`.
pub struct Q00242PossessorOfAPreciousSoul2;

impl Script for Q00242PossessorOfAPreciousSoul2 {
    fn name(&self) -> &'static str {
        "Q00242_PossessorOfAPreciousSoul2"
    }

    fn path(&self) -> &'static str {
        "quests/Q00242_PossessorOfAPreciousSoul2"
    }

    fn id(&self) -> i32 {
        242
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VIRGIL]);
        s.add_talk_id(&[
            VIRGIL,
            KASSANDRA,
            OGMAR,
            MYSTERIOUS_KNIGHT,
            ANGEL_CORPSE,
            KALIS,
            MATILD,
            FALLEN_UNICORN,
            CORNERSTONE,
            PURE_UNICORN,
        ]);
        s.add_kill_id(&[RESTRAINER_OF_GLORY]);
        s.register_quest_items(&[GOLDEN_HAIR, ORB_OF_BINDING, SORCERY_INGREDIENT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(st) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        // NOTE(java-quirk): the check also stops the unicorn timers of a player who left the sub class.
        if !ctx.is_sub_class_active(player) {
            return Some("no_sub.html".to_owned());
        }
        match event {
            "31742-02.html" => {
                st.start_quest(ctx);
                ctx.take_items(player, VIRGILS_LETTER, -1);
            }
            "31743-05.html" => {
                if st.is_cond(ctx, 1) {
                    st.set_cond(ctx, 2, true);
                }
            }
            "31744-02.html" => {
                if st.is_cond(ctx, 2) {
                    st.set_cond(ctx, 3, true);
                }
            }
            "31751-02.html" => {
                if st.is_cond(ctx, 3) {
                    st.set_cond(ctx, 4, true);
                }
            }
            "30759-02.html" => {
                if st.is_cond(ctx, 6) {
                    st.set_cond(ctx, 7, true);
                }
            }
            "30738-02.html" => {
                if st.is_cond(ctx, 7) {
                    st.set_cond(ctx, 8, true);
                    ctx.give_items(player, SORCERY_INGREDIENT, 1);
                }
            }
            "30759-05.html" => {
                if st.is_cond(ctx, 8) {
                    ctx.take_items(player, GOLDEN_HAIR, -1);
                    ctx.take_items(player, SORCERY_INGREDIENT, -1);
                    st.set(ctx, "awaitsDrops", "1");
                    st.set_cond(ctx, 9, true);
                    st.set(ctx, "cornerstones", "0");
                }
            }
            "PURE_UNICORN" => {
                if let Some(npc) = npc {
                    if let Some(index) = ctx.world().npc_spawn_index(npc.object_id) {
                        ctx.world_mut().stop_spawn_respawn(index);
                    }
                    ctx.delete_npc(npc);
                }
                let pure = ctx.add_spawn(PURE_UNICORN, 85884, -76588, -3470, 30000, false, 0, 0);
                ctx.start_quest_timer("FALLEN_UNICORN", 30000, pure, Some(player), false);
                return None;
            }
            "FALLEN_UNICORN" => {
                let fallen = ctx.add_spawn(FALLEN_UNICORN, 85884, -76588, -3470, 0, false, 0, 0);
                if let Some(index) = fallen.and_then(|f| ctx.world().npc_spawn_index(f.object_id)) {
                    ctx.world_mut().start_spawn_respawn(index);
                }
                return None;
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(party_member) = ctx.get_random_party_member_var(killer, Some("awaitsDrops"), "1") else {
            return;
        };
        let Some(st) = ctx.get_quest_state(party_member, false) else {
            return;
        };
        if st.is_cond(ctx, 9) && ctx.get_quest_items_count(party_member, ORB_OF_BINDING) < 4 {
            ctx.give_items(party_member, ORB_OF_BINDING, 1);
            ctx.play_sound(party_member, QuestSound::ItemsoundQuestItemget);
        }
        if ctx.get_quest_items_count(party_member, ORB_OF_BINDING) >= 4 {
            st.unset(ctx, "awaitsDrops");
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let st = ctx.get_quest_state(player, true)?;
        if st.is_started(ctx) && !ctx.is_sub_class_active(player) {
            return Some("no_sub.html".to_owned());
        }
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            VIRGIL => match st.state(ctx) {
                CREATED => {
                    let fore_done = ctx
                        .player_quest_state(player, "Q00241_PossessorOfAPreciousSoul1")
                        .is_some_and(|q| q.is_completed());
                    if fore_done {
                        htmltext = if ctx.is_sub_class_active(player) && ctx.player_level(player) >= 60 {
                            "31742-01.htm"
                        } else {
                            "31742-00.htm"
                        }
                        .to_owned();
                    }
                }
                STARTED => match st.get_cond(ctx) {
                    1 => htmltext = "31742-03.html".to_owned(),
                    11 => {
                        htmltext = "31742-04.html".to_owned();
                        ctx.give_items(player, CARADINE_LETTER, 1);
                        ctx.add_exp_and_sp(player, 455764, 0);
                        st.exit_quest(ctx, false, true);
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            KASSANDRA => match st.get_cond(ctx) {
                1 => htmltext = "31743-01.html".to_owned(),
                2 => htmltext = "31743-06.html".to_owned(),
                11 => htmltext = "31743-07.html".to_owned(),
                _ => {}
            },
            OGMAR => match st.get_cond(ctx) {
                2 => htmltext = "31744-01.html".to_owned(),
                3 => htmltext = "31744-03.html".to_owned(),
                _ => {}
            },
            MYSTERIOUS_KNIGHT => match st.get_cond(ctx) {
                3 => htmltext = "31751-01.html".to_owned(),
                4 => htmltext = "31751-03.html".to_owned(),
                5 => {
                    if ctx.has_quest_items(player, GOLDEN_HAIR) {
                        st.set_cond(ctx, 6, true);
                        htmltext = "31751-04.html".to_owned();
                    }
                }
                6 => htmltext = "31751-05.html".to_owned(),
                _ => {}
            },
            ANGEL_CORPSE => match st.get_cond(ctx) {
                4 => {
                    ctx.do_die(npc, npc.object_id);
                    if CHANCE_FOR_HAIR >= ctx.get_random(100) {
                        ctx.give_items(player, GOLDEN_HAIR, 1);
                        st.set_cond(ctx, 5, true);
                        htmltext = "31752-01.html".to_owned();
                    } else {
                        htmltext = "31752-02.html".to_owned();
                    }
                }
                5 => htmltext = "31752-02.html".to_owned(),
                _ => {}
            },
            KALIS => match st.get_cond(ctx) {
                6 => htmltext = "30759-01.html".to_owned(),
                7 => htmltext = "30759-03.html".to_owned(),
                8 => {
                    if ctx.has_quest_items(player, SORCERY_INGREDIENT) {
                        htmltext = "30759-04.html".to_owned();
                    }
                }
                9 => htmltext = "30759-06.html".to_owned(),
                _ => {}
            },
            MATILD => match st.get_cond(ctx) {
                7 => htmltext = "30738-01.html".to_owned(),
                8 => htmltext = "30738-03.html".to_owned(),
                _ => {}
            },
            CORNERSTONE => {
                if st.is_cond(ctx, 9) {
                    if ctx.has_quest_items(player, ORB_OF_BINDING) {
                        htmltext = "31748-02.html".to_owned();
                        ctx.take_items(player, ORB_OF_BINDING, 1);
                        ctx.do_die(npc, npc.object_id);

                        st.set(ctx, "cornerstones", &(st.get_int(ctx, "cornerstones") + 1).to_string());
                        if st.get_int(ctx, "cornerstones") == 4 {
                            st.set_cond(ctx, 10, false);
                        }

                        ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                        ctx.cast_skill_at(npc, Some(player), SkillKey::new(4546, 1));
                    } else {
                        htmltext = "31748-01.html".to_owned();
                    }
                }
            }
            FALLEN_UNICORN => match st.get_cond(ctx) {
                9 => htmltext = "31746-01.html".to_owned(),
                10 => {
                    htmltext = "31746-02.html".to_owned();
                    ctx.start_quest_timer("PURE_UNICORN", 3000, Some(npc), Some(player), false);
                }
                _ => {}
            },
            PURE_UNICORN => match st.get_cond(ctx) {
                10 => {
                    st.set_cond(ctx, 11, true);
                    htmltext = "31747-01.html".to_owned();
                }
                11 => htmltext = "31747-02.html".to_owned(),
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
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00242_PossessorOfAPreciousSoul2";
    const FORE: &str = "Q00241_PossessorOfAPreciousSoul1";

    const NPCS: &str = r#"
        <npc id="31742" type="Folk" level="70" name="Virgil"/>
        <npc id="31743" type="Folk" level="70" name="Kassandra"/>
        <npc id="31744" type="Folk" level="70" name="Ogmar"/>
        <npc id="31751" type="Folk" level="70" name="Mysterious Knight"/>
        <npc id="30759" type="Folk" level="70" name="Kalis"/>
        <npc id="30738" type="Folk" level="70" name="Matild"/>
        <npc id="31747" type="Folk" level="70" name="Pure Unicorn"/>
        <npc id="27317" type="Monster" level="60" name="Restrainer of Glory"/>"#;

    const ITEMS: &str = r#"
        <item id="7677" type="EtcItem" name="Virgil's Letter"/>
        <item id="7678" type="EtcItem" name="Caradine's Letter"/>
        <item id="7590" type="EtcItem" name="Golden Hair"><set name="is_questitem" val="true"/></item>
        <item id="7595" type="EtcItem" name="Orb of Binding"><set name="is_questitem" val="true"/></item>
        <item id="7596" type="EtcItem" name="Sorcery Ingredient"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00241_PossessorOfAPreciousSoul1`: a stand-in whose state a test completes.
    struct Fore;

    impl Script for Fore {
        fn name(&self) -> &'static str {
            FORE
        }
        fn path(&self) -> &'static str {
            "quests/Q00241_PossessorOfAPreciousSoul1"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Fore);
        registry.add(Q00242PossessorOfAPreciousSoul2);
        let pages = Pages::of(&Q00242PossessorOfAPreciousSoul2)
            .page("31742-01.htm", &["31742-02.html"])
            .plain(&[
                "31742-00.htm",
                "31742-02.html",
                "31742-03.html",
                "31742-04.html",
                "31743-05.html",
                "no_sub.html",
                "30759-02.html",
                "30759-05.html",
                "30738-02.html",
            ])
            .page("31743-01.html", &["31743-05.html"])
            .page("30759-04.html", &["30759-05.html"])
            .npcs(&[VIRGIL, KASSANDRA, KALIS, MATILD, PURE_UNICORN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let virgil = t.spawn(VIRGIL, 20);
        let kassandra = t.spawn(KASSANDRA, 40);
        (t, virgil, kassandra)
    }

    fn ready(t: &mut ScriptTest, id: i32, sub: bool) {
        let mut data = player_data(id, 70, vec![item(10 * id, VIRGILS_LETTER, 1)]);
        if sub {
            data.profile.class_index = 1;
        }
        t.enter_with(data);
        t.world.run_as_script(FORE, |ctx| {
            if let Some(qs) = ctx.quest_state_of(FORE, Oid(id), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
        t.take_writes();
    }

    #[test]
    fn the_quest_is_offered_to_a_sub_class_of_level_60() {
        let (mut t, virgil, _) = setup();
        ready(&mut t, 1, true);
        assert!(t.quest_page(1, virgil).contains("31742-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_main_class_is_sent_away() {
        let (mut t, virgil, _) = setup();
        ready(&mut t, 1, false);
        assert!(t.quest_page(1, virgil).contains("31742-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_quest_goes_from_virgil_to_the_unicorn_quest_and_pays() {
        let (mut t, virgil, kassandra) = setup();
        ready(&mut t, 1, true);
        assert!(t.quest_page(1, virgil).contains("31742-01.htm"));
        assert!(t.event(1, NAME, "31742-02.html").contains("31742-02.html"));
        assert_eq!(t.count(1, VIRGILS_LETTER), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        assert!(t.quest_page(1, kassandra).contains("31743-01.html"));
        assert!(t.event(1, NAME, "31743-05.html").contains("31743-05.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Cond 8 to 9: Kalis takes the hair and the ingredient and starts the Orb hunt.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 8, false);
            ctx.give_items(Oid(1), GOLDEN_HAIR, 1);
            ctx.give_items(Oid(1), SORCERY_INGREDIENT, 1);
        });
        t.take_writes();
        let kalis = t.spawn(KALIS, 60);
        assert!(t.quest_page(1, kalis).contains("30759-04.html"));
        assert!(t.event(1, NAME, "30759-05.html").contains("30759-05.html"));
        assert_eq!((t.count(1, GOLDEN_HAIR), t.count(1, SORCERY_INGREDIENT)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "awaitsDrops", "1"),
                rows::update(NAME, "cond", "9"),
                // The cond jump past 8 sets the progress flag bit (Java `QuestState.setCond`).
                rows::update(NAME, "__compltdStateFlags", "-2147483261"),
                rows::insert(NAME, "cornerstones", "0")
            ]
        );

        // Four kills give four orbs; the variable goes with the fourth.
        for _ in 0..4 {
            let m = t.spawn(RESTRAINER_OF_GLORY, 20);
            t.kill(1, m);
        }
        assert_eq!(t.count(1, ORB_OF_BINDING), 4);
        assert_eq!(t.take_writes(), [rows::delete_var(NAME, "awaitsDrops")]);

        // At cond 11 Virgil pays.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 11, false);
        });
        t.take_writes();
        assert!(t.quest_page(1, virgil).contains("31742-04.html"));
        assert_eq!(t.count(1, CARADINE_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
