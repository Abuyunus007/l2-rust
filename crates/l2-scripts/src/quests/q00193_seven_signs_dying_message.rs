//! Quest 193: Seven Signs, Dying Message (Hollint, Cain, Eric and Sir Gustav Athebaldt).
// Java: dist/game/data/scripts/quests/Q00193_SevenSignsDyingMessage/Q00193_SevenSignsDyingMessage.java
//
// Pattern: like Q00198 (a fight against Shilen's Evil Thoughts that Cain heals by timer, the
// `isBusy` field in `ctx.state`), a movie at cond 3 and the reward from another NPC.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const SHILENS_EVIL_THOUGHTS: i32 = 27343;
const HOLLINT: i32 = 30191;
const SIR_GUSTAV_ATHEBALDT: i32 = 30760;
const CAIN: i32 = 32569;
const ERIC: i32 = 32570;

// Items
const JACOBS_NECKLACE: i32 = 13814;
const DEADMANS_HERB: i32 = 13816;
const SCULPTURE_OF_DOUBT: i32 = 14353;

// Misc
const MIN_LEVEL: i32 = 79;
/// Java `Movie.SSQ_DYING_MASSAGE`.
const SSQ_DYING_MASSAGE: i32 = 9;
/// Java `NPC_HEAL = new SkillHolder(4065, 8)`.
const NPC_HEAL: SkillKey = SkillKey { id: 4065, level: 8 };
/// Java `Q00192_SevenSignsSeriesOfDoubt.class.getSimpleName()`: the quest this one follows.
const SERIES_OF_DOUBT: &str = "Q00192_SevenSignsSeriesOfDoubt";
/// Where Shilen's Evil Thoughts appear (Java `addSpawn(SHILENS_EVIL_THOUGHTS, 82425, 47232, -3216, ...)`).
const SHILEN_X: i32 = 82_425;
const SHILEN_Y: i32 = 47_232;
const SHILEN_Z: i32 = -3_216;

/// Java `private boolean isBusy` of the script: the fight of Shilen's Evil Thoughts runs.
#[derive(Default)]
struct DyingMessageState {
    is_busy: bool,
}

/// Java `Q00193_SevenSignsDyingMessage`.
pub struct Q00193SevenSignsDyingMessage;

impl Script for Q00193SevenSignsDyingMessage {
    fn name(&self) -> &'static str {
        "Q00193_SevenSignsDyingMessage"
    }

    fn path(&self) -> &'static str {
        "quests/Q00193_SevenSignsDyingMessage"
    }

    fn id(&self) -> i32 {
        193
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HOLLINT]);
        s.add_talk_id(&[HOLLINT, CAIN, ERIC, SIR_GUSTAV_ATHEBALDT]);
        s.add_kill_id(&[SHILENS_EVIL_THOUGHTS]);
        s.register_quest_items(&[JACOBS_NECKLACE, DEADMANS_HERB, SCULPTURE_OF_DOUBT]);
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
                ctx.state::<DyingMessageState>().is_busy = false;
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
            "30191-02.html" => {
                ctx.give_items(player, JACOBS_NECKLACE, 1);
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32569-02.html" | "32569-03.html" | "32569-04.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, JACOBS_NECKLACE) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32569-05.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, JACOBS_NECKLACE) {
                    ctx.take_items(player, JACOBS_NECKLACE, -1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "showmovie" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, DEADMANS_HERB) {
                    ctx.take_items(player, DEADMANS_HERB, -1);
                    qs.set_cond(ctx, 4, true);
                    ctx.play_movie(player, SSQ_DYING_MASSAGE);
                    return Some(String::new());
                }
            }
            "32569-10.html" | "32569-11.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32569-12.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                    ctx.take_items(player, SCULPTURE_OF_DOUBT, -1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32570-02.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.give_items(player, DEADMANS_HERB, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "fight" => {
                htmltext = Some("32569-14.html".to_owned());
                if qs.is_cond(ctx, 4) {
                    // Java `npc` is Cain here (the bypass of his page).
                    let Some(cain) = npc else {
                        return htmltext;
                    };
                    ctx.state::<DyingMessageState>().is_busy = true;
                    let name = ctx.player_name(player);
                    ctx.broadcast_npc_say(
                        cain,
                        ChatType::NpcGeneral,
                        NpcStringId::S1ThatStrangerMustBeDefeatedHereIsTheUltimateHelp,
                        &[&name],
                    );
                    let heal_in = 30_000 - i64::from(ctx.get_random(20_000));
                    ctx.start_quest_timer("heal", heal_in, Some(cain), Some(player), false);
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
                }
            }
            "heal" => {
                if let Some(cain) = npc {
                    let inside = match ctx.world().position_of(player) {
                        Some((x, y, z)) => ctx.is_inside_radius_3d(cain, x, y, z, 600),
                        None => false,
                    };
                    if !inside {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            cain,
                            ChatType::NpcGeneral,
                            NpcStringId::LookHereS1DonTFallTooFarBehind,
                            &[&name],
                        );
                    } else if !ctx.is_dead(player) {
                        ctx.cast_skill(cain, player, NPC_HEAL);
                    }
                    let heal_in = 30_000 - i64::from(ctx.get_random(20_000));
                    ctx.start_quest_timer("heal", heal_in, Some(cain), Some(player), false);
                }
            }
            "reward" => {
                if qs.is_cond(ctx, 6) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        ctx.add_exp_and_sp(player, 52_518_015, 5_817_677);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some("30760-02.html".to_owned());
                    } else {
                        htmltext = Some("level_check.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(party_member) = ctx.get_random_party_member_cond(player, 4) else {
            return;
        };

        let qs = ctx.get_quest_state(party_member, false);
        let in_range = match ctx.world().position_of(party_member) {
            Some((x, y, z)) => ctx.is_inside_radius_3d(npc, x, y, z, ctx.alt_party_range()),
            None => false,
        };
        if in_range {
            // NOTE(java-quirk): the sculpture and the sound go to the killer, the cond to the
            // member drawn.
            ctx.give_items(player, SCULPTURE_OF_DOUBT, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestFinish);
            if let Some(qs) = qs {
                qs.set_cond(ctx, 5, false);
            }
        }

        ctx.state::<DyingMessageState>().is_busy = false;
        ctx.cancel_quest_timers("despawn");
        ctx.cancel_quest_timers("heal");
        let name = ctx.player_name(party_member);
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
                if npc.id == HOLLINT {
                    let doubt_done = ctx
                        .player_quest_state(player, SERIES_OF_DOUBT)
                        .is_some_and(|qs192| qs192.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && doubt_done {
                        "30191-01.htm"
                    } else {
                        "30191-03.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                HOLLINT => {
                    if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, JACOBS_NECKLACE) {
                        htmltext = "30191-04.html".to_owned();
                    }
                }
                CAIN => match qs.get_cond(ctx) {
                    1 => {
                        if ctx.has_quest_items(player, JACOBS_NECKLACE) {
                            htmltext = "32569-01.html".to_owned();
                        }
                    }
                    2 => htmltext = "32569-06.html".to_owned(),
                    3 => {
                        if ctx.has_quest_items(player, DEADMANS_HERB) {
                            htmltext = "32569-07.html".to_owned();
                        }
                    }
                    4 => {
                        htmltext = if ctx.state::<DyingMessageState>().is_busy {
                            "32569-13.html"
                        } else {
                            "32569-08.html"
                        }
                        .to_owned();
                    }
                    5 => {
                        if ctx.has_quest_items(player, SCULPTURE_OF_DOUBT) {
                            htmltext = "32569-09.html".to_owned();
                        }
                    }
                    _ => {}
                },
                ERIC => match qs.get_cond(ctx) {
                    2 => htmltext = "32570-01.html".to_owned(),
                    3 => htmltext = "32570-03.html".to_owned(),
                    _ => {}
                },
                SIR_GUSTAV_ATHEBALDT => {
                    if qs.is_cond(ctx, 6) {
                        htmltext = "30760-01.html".to_owned();
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00193_SevenSignsDyingMessage";

    const NPCS: &str = r#"
        <npc id="27343" type="Monster" level="80" name="Shilen's Evil Thoughts"/>
        <npc id="30191" type="Folk" level="80" name="Hollint"/>
        <npc id="30760" type="Folk" level="80" name="Sir Gustav Athebaldt"/>
        <npc id="32569" type="Folk" level="80" name="Cain"/>
        <npc id="32570" type="Folk" level="80" name="Eric"/>"#;

    const ITEMS: &str = r#"
        <item id="13814" type="EtcItem" name="Jacob's Necklace"><set name="is_questitem" val="true"/></item>
        <item id="13816" type="EtcItem" name="Deadman's Herb"><set name="is_questitem" val="true"/></item>
        <item id="14353" type="EtcItem" name="Sculpture of Doubt"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00192_SevenSignsSeriesOfDoubt` as far as this quest reads it: a script of that name.
    struct SeriesOfDoubt;

    impl Script for SeriesOfDoubt {
        fn name(&self) -> &'static str {
            SERIES_OF_DOUBT
        }
        fn path(&self) -> &'static str {
            "quests/Q00192_SevenSignsSeriesOfDoubt"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        hollint: Oid,
        gustav: Oid,
        cain: Oid,
        eric: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(SeriesOfDoubt);
        registry.add(Q00193SevenSignsDyingMessage);
        let pages = Pages::of(&Q00193SevenSignsDyingMessage)
            .page("30191-01.htm", &["30191-02.html"])
            .page("30760-01.html", &["reward"])
            .page("32569-01.html", &["32569-02.html"])
            .page("32569-02.html", &["32569-03.html"])
            .page("32569-03.html", &["32569-04.html"])
            .page("32569-04.html", &["32569-05.html"])
            .page("32569-07.html", &["showmovie"])
            .page("32569-08.html", &["fight"])
            .page("32569-09.html", &["32569-10.html"])
            .page("32569-10.html", &["32569-11.html"])
            .page("32569-11.html", &["32569-12.html"])
            .page("32570-01.html", &["32570-02.html"])
            .plain(&[
                "30191-02.html",
                "30191-03.html",
                "30191-04.html",
                "30760-02.html",
                "32569-05.html",
                "32569-06.html",
                "32569-12.html",
                "32569-13.html",
                "32569-14.html",
                "32570-02.html",
                "32570-03.html",
                "level_check.html",
            ])
            .npcs(&[HOLLINT, CAIN, ERIC, SIR_GUSTAV_ATHEBALDT]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            hollint: t.spawn(HOLLINT, 20),
            gustav: t.spawn(SIR_GUSTAV_ATHEBALDT, 25),
            cain: t.spawn(CAIN, 40),
            eric: t.spawn(ERIC, 45),
        };
        (t, npcs)
    }

    fn finish_series_of_doubt(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(SERIES_OF_DOUBT, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_offer_needs_level_79_and_the_previous_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, npcs.hollint).contains("30191-03.html"));
        finish_series_of_doubt(&mut t, 1);
        assert!(t.quest_page(1, npcs.hollint).contains("30191-01.htm"));
        t.enter(2, 78, vec![]);
        finish_series_of_doubt(&mut t, 2);
        assert!(t.quest_page(2, npcs.hollint).contains("30191-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_necklace_the_herb_the_fight_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 79, vec![]);
        finish_series_of_doubt(&mut t, 1);
        assert!(t.quest_page(1, npcs.hollint).contains("30191-01.htm"));
        assert!(t.event(1, NAME, "30191-02.html").contains("30191-02.html"));
        assert_eq!(t.count(1, JACOBS_NECKLACE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.hollint).contains("30191-04.html"));

        // Cain takes the necklace (cond 2); Eric gives the herb (cond 3).
        assert!(t.quest_page(1, npcs.cain).contains("32569-01.html"));
        for e in ["32569-02.html", "32569-03.html", "32569-04.html", "32569-05.html"] {
            assert!(t.event(1, NAME, e).contains(e), "{e}");
        }
        assert_eq!(t.count(1, JACOBS_NECKLACE), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.cain).contains("32569-06.html"));
        assert!(t.quest_page(1, npcs.eric).contains("32570-01.html"));
        assert!(t.event(1, NAME, "32570-02.html").contains("32570-02.html"));
        assert_eq!(t.count(1, DEADMANS_HERB), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.eric).contains("32570-03.html"));

        // Cain shows the movie (cond 4) and starts the fight.
        assert!(t.quest_page(1, npcs.cain).contains("32569-07.html"));
        t.event(1, NAME, "showmovie");
        assert_eq!(t.count(1, DEADMANS_HERB), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.cain).contains("32569-08.html"));
        assert!(t.event(1, NAME, "fight").contains("32569-14.html"));
        assert!(t.quest_page(1, npcs.cain).contains("32569-13.html"));
        let shilen = *t.npcs_of(SHILENS_EVIL_THOUGHTS).first().unwrap();
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(shilen).unwrap();
            ctx.tele_npc_to(npc, crate::testing::X, crate::testing::Y, crate::testing::Z);
        });
        t.kill(1, shilen);
        assert_eq!(t.count(1, SCULPTURE_OF_DOUBT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        // The fight is over: Cain is not busy any more.
        assert!(t.quest_page(1, npcs.cain).contains("32569-09.html"));
        for e in ["32569-10.html", "32569-11.html", "32569-12.html"] {
            assert!(t.event(1, NAME, e).contains(e), "{e}");
        }
        assert_eq!(t.count(1, SCULPTURE_OF_DOUBT), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Sir Gustav pays exp and sp: `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.gustav).contains("30760-01.html"));
        assert!(t.event(1, NAME, "reward").contains("30760-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.gustav).contains("already been completed"));
    }

    #[test]
    fn a_level_below_79_gets_the_level_check_page() {
        let (mut t, npcs) = setup();
        t.enter(1, 78, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, 6, false);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.gustav).contains("30760-01.html"));
        assert!(t.event(1, NAME, "reward").contains("level_check.html"));
        assert!(t.take_writes().is_empty());
    }
}
