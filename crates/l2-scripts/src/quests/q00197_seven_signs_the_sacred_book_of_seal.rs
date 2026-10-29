//! Quest 197: Seven Signs, The Sacred Book of Seal (Wood, Orven, Leopard, Lawrence and Sophia; Shilen's Evil Thoughts).
// Java: dist/game/data/scripts/quests/Q00197_SevenSignsTheSacredBookOfSeal/Q00197_SevenSignsTheSacredBookOfSeal.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SHILENS_EVIL_THOUGHTS: i32 = 27396;
const ORVEN: i32 = 30857;
const WOOD: i32 = 32593;
const LEOPARD: i32 = 32594;
const LAWRENCE: i32 = 32595;
const SOPHIA: i32 = 32596;

// Items
const MYSTERIOUS_HAND_WRITTEN_TEXT: i32 = 13829;
const SCULPTURE_OF_DOUBT: i32 = 14354;

// Misc
const MIN_LEVEL: i32 = 79;
/// Java `Q00196_SevenSignsSealOfTheEmperor.class.getSimpleName()`: the quest this one follows.
const SEAL_OF_THE_EMPEROR: &str = "Q00196_SevenSignsSealOfTheEmperor";
/// Where Shilen's Evil Thoughts appear at the Lawrence step (Java `addSpawn(.., 152520, -57502, -3408, ..)`).
const SHILEN_X: i32 = 152_520;
const SHILEN_Y: i32 = -57_502;
const SHILEN_Z: i32 = -3_408;

/// Java `private boolean isBusy` of the script.
#[derive(Default)]
struct SacredBookState {
    is_busy: bool,
}

/// Java `Q00197_SevenSignsTheSacredBookOfSeal`.
pub struct Q00197SevenSignsTheSacredBookOfSeal;

impl Script for Q00197SevenSignsTheSacredBookOfSeal {
    fn name(&self) -> &'static str {
        "Q00197_SevenSignsTheSacredBookOfSeal"
    }

    fn path(&self) -> &'static str {
        "quests/Q00197_SevenSignsTheSacredBookOfSeal"
    }

    fn id(&self) -> i32 {
        197
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WOOD]);
        s.add_talk_id(&[WOOD, ORVEN, LEOPARD, LAWRENCE, SOPHIA]);
        s.add_kill_id(&[SHILENS_EVIL_THOUGHTS]);
        s.register_quest_items(&[MYSTERIOUS_HAND_WRITTEN_TEXT, SCULPTURE_OF_DOUBT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(monster) = npc
            && monster.id == SHILENS_EVIL_THOUGHTS
            && event == "despawn"
        {
            if !ctx.is_dead(monster.object_id) {
                ctx.state::<SacredBookState>().is_busy = false;
                ctx.broadcast_npc_say(
                    monster,
                    ChatType::NpcGeneral,
                    NpcStringId::NextTimeYouWillNotEscape,
                    &[],
                );
                ctx.delete_npc(monster);
            }
            // Java `super.onEvent(...)`: the base quest has no answer.
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32593-02.htm" | "32593-03.htm" => htmltext = Some(event.to_owned()),
            "32593-04.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32593-08.html" => {
                if qs.is_cond(ctx, 6)
                    && ctx.has_quest_items(player, MYSTERIOUS_HAND_WRITTEN_TEXT)
                    && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT)
                {
                    htmltext = Some(event.to_owned());
                }
            }
            "32593-09.html" => {
                if qs.is_cond(ctx, 6) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        ctx.add_exp_and_sp(player, 52_518_015, 5_817_677);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some(event.to_owned());
                    } else {
                        htmltext = Some("level_check.html".to_owned());
                    }
                }
            }
            "30857-02.html" | "30857-03.html" => {
                if qs.is_cond(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30857-04.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32594-02.html" => {
                if qs.is_cond(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32594-03.html" => {
                if qs.is_cond(ctx, 2) {
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32595-02.html" | "32595-03.html" => {
                if qs.is_cond(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32595-04.html" => {
                if qs.is_cond(ctx, 3) {
                    // Java `npc` is Lawrence here (the bypass of his page).
                    let Some(lawrence) = npc else {
                        return htmltext;
                    };
                    ctx.state::<SacredBookState>().is_busy = true;
                    let name = ctx.player_name(player);
                    ctx.broadcast_npc_say(
                        lawrence,
                        ChatType::NpcGeneral,
                        NpcStringId::S1ThatStrangerMustBeDefeatedHereIsTheUltimateHelp,
                        &[&name],
                    );
                    if let Some(monster) = ctx.add_spawn(
                        SHILENS_EVIL_THOUGHTS,
                        SHILEN_X,
                        SHILEN_Y,
                        SHILEN_Z,
                        0,
                        false,
                        0,
                        0,
                    ) {
                        ctx.broadcast_npc_say(
                            monster,
                            ChatType::NpcGeneral,
                            NpcStringId::YouAreNotTheOwnerOfThatItem,
                            &[],
                        );
                        ctx.set_running(monster);
                        ctx.add_attack_desire(monster, player, 999);
                        ctx.set_intention_attack(monster, player);
                        ctx.start_quest_timer("despawn", 300_000, Some(monster), None, false);
                    }
                    htmltext = Some(event.to_owned());
                }
            }
            "32595-06.html" | "32595-07.html" | "32595-08.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32595-09.html" => {
                if qs.is_cond(ctx, 4) && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32596-02.html" | "32596-03.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32596-04.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                    ctx.give_items(player, MYSTERIOUS_HAND_WRITTEN_TEXT, 1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(killer, 3) else {
            return;
        };
        let in_range = match ctx.world().position_of(member) {
            Some((x, y, z)) => ctx.is_inside_radius_3d(npc, x, y, z, ctx.alt_party_range()),
            None => false,
        };
        if in_range {
            // NOTE(java-quirk): Java gives the sculpture and plays the sound to the killer, not to
            // the party member drawn; the member's cond is set (`setCond(4)`, no sound).
            ctx.give_items(killer, SCULPTURE_OF_DOUBT, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestFinish);
            if let Some(qs) = ctx.get_quest_state(member, false) {
                qs.set_cond(ctx, 4, false);
            }
        }
        ctx.state::<SacredBookState>().is_busy = false;
        ctx.cancel_quest_timers("despawn");
        let name = ctx.player_name(member);
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::S1YouMayHaveWonThisTimeButNextTimeIWillSurelyCaptureYou,
            &[&name],
        );
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc.id == WOOD {
                    let seal_done = ctx
                        .player_quest_state(player, SEAL_OF_THE_EMPEROR)
                        .is_some_and(|q196| q196.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && seal_done {
                        "32593-01.htm"
                    } else {
                        "32593-05.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                WOOD => {
                    let cond = qs.get_cond(ctx);
                    if cond > 0 && cond < 6 {
                        htmltext = "32593-06.html".to_owned();
                    } else if qs.is_cond(ctx, 6)
                        && ctx.has_quest_items(player, MYSTERIOUS_HAND_WRITTEN_TEXT)
                        && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT)
                    {
                        htmltext = "32593-07.html".to_owned();
                    }
                }
                ORVEN => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30857-01.html".to_owned();
                    } else if qs.get_cond(ctx) >= 2 {
                        htmltext = "30857-05.html".to_owned();
                    }
                }
                LEOPARD => {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "32594-01.html".to_owned();
                    } else if qs.get_cond(ctx) >= 3 {
                        htmltext = "32594-04.html".to_owned();
                    }
                }
                LAWRENCE => {
                    if qs.is_cond(ctx, 3) {
                        htmltext = if ctx.state::<SacredBookState>().is_busy {
                            "32595-05.html"
                        } else {
                            "32595-01.html"
                        }
                        .to_owned();
                    } else if qs.is_cond(ctx, 4) {
                        if ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                            htmltext = "32595-06.html".to_owned();
                        }
                    } else if qs.get_cond(ctx) >= 5 && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                        htmltext = "32595-10.html".to_owned();
                    }
                }
                SOPHIA => {
                    if qs.is_cond(ctx, 5) {
                        if ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                            htmltext = "32596-01.html".to_owned();
                        }
                    } else if qs.get_cond(ctx) >= 6
                        && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT)
                        && ctx.has_quest_items(player, MYSTERIOUS_HAND_WRITTEN_TEXT)
                    {
                        htmltext = "32596-05.html".to_owned();
                    }
                }
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
    use l2_world::script::{COMPLETED, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00197_SevenSignsTheSacredBookOfSeal";

    const NPCS: &str = r#"
        <npc id="27396" type="Monster" level="80" name="Shilen's Evil Thoughts"/>
        <npc id="30857" type="Folk" level="80" name="Orven"/>
        <npc id="32593" type="Folk" level="80" name="Wood"/>
        <npc id="32594" type="Folk" level="80" name="Leopard"/>
        <npc id="32595" type="Folk" level="80" name="Lawrence"/>
        <npc id="32596" type="Folk" level="80" name="Sophia"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13829" type="EtcItem" name="Mysterious Hand-Written Text"><set name="is_questitem" val="true"/></item>
        <item id="14354" type="EtcItem" name="Sculpture of Doubt"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00196_SevenSignsSealOfTheEmperor` as far as this quest reads it: a script of that name.
    struct SealOfTheEmperor;

    impl Script for SealOfTheEmperor {
        fn name(&self) -> &'static str {
            SEAL_OF_THE_EMPEROR
        }
        fn path(&self) -> &'static str {
            "quests/Q00196_SevenSignsSealOfTheEmperor"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        wood: Oid,
        orven: Oid,
        leopard: Oid,
        lawrence: Oid,
        sophia: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(SealOfTheEmperor);
        registry.add(Q00197SevenSignsTheSacredBookOfSeal);
        let pages = Pages::of(&Q00197SevenSignsTheSacredBookOfSeal)
            .page("32593-01.htm", &["32593-02.htm"])
            .page("32593-02.htm", &["32593-03.htm"])
            .page("32593-03.htm", &["32593-04.html"])
            .plain(&[
                "32593-04.html",
                "32593-05.html",
                "32593-06.html",
                "level_check.html",
            ])
            .page("32593-07.html", &["32593-08.html"])
            .page("32593-08.html", &["32593-09.html"])
            .plain(&["32593-09.html"])
            .page("30857-01.html", &["30857-02.html"])
            .page("30857-02.html", &["30857-03.html"])
            .page("30857-03.html", &["30857-04.html"])
            .plain(&["30857-04.html", "30857-05.html"])
            .page("32594-01.html", &["32594-02.html"])
            .page("32594-02.html", &["32594-03.html"])
            .plain(&["32594-03.html", "32594-04.html"])
            .page("32595-01.html", &["32595-02.html"])
            .page("32595-02.html", &["32595-03.html"])
            .page("32595-03.html", &["32595-04.html"])
            .page("32595-04.html", &[])
            .plain(&["32595-05.html"])
            .page("32595-06.html", &["32595-07.html"])
            .page("32595-07.html", &["32595-08.html"])
            .page("32595-08.html", &["32595-09.html"])
            .plain(&["32595-09.html", "32595-10.html"])
            .page("32596-01.html", &["32596-02.html"])
            .page("32596-02.html", &["32596-03.html"])
            .page("32596-03.html", &["32596-04.html"])
            .plain(&["32596-04.html", "32596-05.html"])
            .npcs(&[WOOD, ORVEN, LEOPARD, LAWRENCE, SOPHIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            wood: t.spawn(WOOD, 20),
            orven: t.spawn(ORVEN, 40),
            leopard: t.spawn(LEOPARD, 60),
            lawrence: t.spawn(LAWRENCE, 80),
            sophia: t.spawn(SOPHIA, 100),
        };
        (t, npcs)
    }

    fn finish_seal_of_the_emperor(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(SEAL_OF_THE_EMPEROR, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_offer_needs_level_79_and_the_seal_of_the_emperor() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.wood).contains("32593-05.html"));
        finish_seal_of_the_emperor(&mut t, 1);
        assert!(t.quest_page(1, npcs.wood).contains("32593-01.htm"));
        t.enter_as(2, 78, race::HUMAN, 0, vec![]);
        finish_seal_of_the_emperor(&mut t, 2);
        assert!(t.quest_page(2, npcs.wood).contains("32593-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_orven_leopard_lawrence_sophia_steps_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        finish_seal_of_the_emperor(&mut t, 1);
        assert!(t.quest_page(1, npcs.wood).contains("32593-01.htm"));
        assert!(t.event(1, NAME, "32593-02.htm").contains("32593-02.htm"));
        assert!(t.event(1, NAME, "32593-03.htm").contains("32593-03.htm"));
        assert!(t.event(1, NAME, "32593-04.html").contains("32593-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Orven: cond 2.
        assert!(t.quest_page(1, npcs.orven).contains("30857-01.html"));
        assert!(t.event(1, NAME, "30857-02.html").contains("30857-02.html"));
        assert!(t.event(1, NAME, "30857-03.html").contains("30857-03.html"));
        assert!(t.event(1, NAME, "30857-04.html").contains("30857-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Leopard: cond 3.
        assert!(t.quest_page(1, npcs.leopard).contains("32594-01.html"));
        assert!(t.event(1, NAME, "32594-02.html").contains("32594-02.html"));
        assert!(t.event(1, NAME, "32594-03.html").contains("32594-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Lawrence: the fight with Shilen's Evil Thoughts, the sculpture (cond 4), the movie-less kill.
        assert!(t.quest_page(1, npcs.lawrence).contains("32595-01.html"));
        assert!(t.event(1, NAME, "32595-02.html").contains("32595-02.html"));
        assert!(t.event(1, NAME, "32595-03.html").contains("32595-03.html"));
        assert!(t.event(1, NAME, "32595-04.html").contains("32595-04.html"));
        let shilen = *t.npcs_of(SHILENS_EVIL_THOUGHTS).first().unwrap();
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(shilen).unwrap();
            ctx.tele_npc_to(npc, crate::testing::X, crate::testing::Y, crate::testing::Z);
        });
        t.kill(1, shilen);
        assert_eq!(t.count(1, SCULPTURE_OF_DOUBT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Lawrence again: the sculpture goes on to cond 5.
        assert!(t.quest_page(1, npcs.lawrence).contains("32595-06.html"));
        assert!(t.event(1, NAME, "32595-07.html").contains("32595-07.html"));
        assert!(t.event(1, NAME, "32595-08.html").contains("32595-08.html"));
        assert!(t.event(1, NAME, "32595-09.html").contains("32595-09.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Sophia: the hand-written text (cond 6); Wood pays on the level 79 check.
        assert!(t.quest_page(1, npcs.sophia).contains("32596-01.html"));
        assert!(t.event(1, NAME, "32596-02.html").contains("32596-02.html"));
        assert!(t.event(1, NAME, "32596-03.html").contains("32596-03.html"));
        assert!(t.event(1, NAME, "32596-04.html").contains("32596-04.html"));
        assert_eq!(t.count(1, MYSTERIOUS_HAND_WRITTEN_TEXT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        assert!(t.quest_page(1, npcs.wood).contains("32593-07.html"));
        assert!(t.event(1, NAME, "32593-08.html").contains("32593-08.html"));
        assert!(t.event(1, NAME, "32593-09.html").contains("32593-09.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.wood).contains("already been completed"));
    }
}
