//! Talent Show AI: the MC of Fantasy Isle opens a show every four hours, with the singers, the
//! circus, three individuals and the show staff, all driven by chained quest timers.
// Java: dist/game/data/scripts/ai/areas/FantasyIsle/TalentShow.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, PlaySound};
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, game_time_minutes};
use tracing::info;

const MC: i32 = 32433;
// @formatter:off
const SINGERS: [i32; 2] = [32431, 32432];
const CIRCUS: [i32; 5] = [32442, 32443, 32444, 32445, 32446];
const INDIVIDUALS: [i32; 3] = [32439, 32440, 32441];
const SHOWSTUFF: [i32; 5] = [32424, 32425, 32426, 32427, 32428];
// @formatter:on

const MESSAGES: [NpcStringId; 24] = [
    NpcStringId::HowComePeopleAreNotHereWeAreAboutToStartTheShowHmm,
    NpcStringId::UghIHaveButterfliesInMyStomachTheShowStartsSoon,
    NpcStringId::ThankYouAllForComingHereTonight,
    NpcStringId::ItIsAnHonorToHaveTheSpecialShowToday,
    NpcStringId::FantasyIsleIsFullyCommittedToYourHappiness,
    NpcStringId::NowIDLikeToIntroduceTheMostBeautifulSingerInAdenPleaseWelcomeLeylaMira,
    NpcStringId::HereSheComes,
    NpcStringId::ThankYouVeryMuchLeyla,
    NpcStringId::NowWeReInForARealTreat,
    NpcStringId::JustBackFromTheirWorldTourPutYourHandsTogetherForTheFantasyIsleCircus,
    NpcStringId::ComeOnEveryone,
    NpcStringId::DidYouLikeItThatWasSoAmazing,
    NpcStringId::NowWeAlsoInvitedIndividualsWithSpecialTalents,
    NpcStringId::LetSWelcomeTheFirstPersonHere,
    NpcStringId::Oh,
    NpcStringId::OkayNowHereComesTheNextPersonComeOnUpPlease,
    NpcStringId::OhItLooksLikeSomethingGreatIsGoingToHappenRight,
    NpcStringId::OhMy,
    NpcStringId::ThatSGGreatNowHereComesTheLastPerson,
    NpcStringId::NowThisIsTheEndOfTodaySShow,
    NpcStringId::HowWasItIHopeYouAllEnjoyedIt,
    NpcStringId::PleaseRememberThatFantasyIsleIsAlwaysPlanningALotOfGreatShowsForYou,
    NpcStringId::WellIWishICouldContinueAllNightLongButThisIsItForTodayThankYou,
    NpcStringId::WeLoveYou,
];

/// Java `TALKS`: `(event, message index, next event, time)`.
const TALKS: [(&str, usize, &str, i64); 17] = [
    ("1", 1, "2", 1000),
    ("2", 2, "3", 6000),
    ("3", 3, "4", 4000),
    ("4", 4, "5", 5000),
    ("5", 5, "6", 3000),
    ("8", 9, "9", 5000),
    ("9", 10, "10", 5000),
    ("12", 12, "13", 5000),
    ("13", 13, "14", 5000),
    ("15", 14, "16", 5000),
    ("16", 15, "17", 5000),
    ("18", 17, "19", 5000),
    ("19", 18, "20", 5000),
    ("21", 19, "22", 5000),
    ("22", 20, "23", 400),
    ("25", 21, "26", 5000),
    ("26", 22, "27", 5400),
];

/// Java `WALKS`: `(event, x, y, z, heading, next event, time)`.
const WALKS: [(&str, i32, i32, i32, i32, &str, i64); 88] = [
    ("npc1_1", -56546, -56384, -2008, 0, "npc1_2", 1200),
    ("npc1_2", -56597, -56384, -2008, 0, "npc1_3", 1200),
    ("npc1_3", -56596, -56428, -2008, 0, "npc1_4", 1200),
    ("npc1_4", -56593, -56474, -2008, 0, "npc1_5", 1000),
    ("npc1_5", -56542, -56474, -2008, 0, "npc1_6", 1000),
    ("npc1_6", -56493, -56473, -2008, 0, "npc1_7", 2000),
    ("npc1_7", -56495, -56425, -2008, 0, "npc1_1", 4000),
    ("npc2_1", -56550, -56291, -2008, 0, "npc2_2", 1200),
    ("npc2_2", -56601, -56293, -2008, 0, "npc2_3", 1200),
    ("npc2_3", -56603, -56247, -2008, 0, "npc2_4", 1200),
    ("npc2_4", -56605, -56203, -2008, 0, "npc2_5", 1000),
    ("npc2_5", -56553, -56202, -2008, 0, "npc2_6", 1100),
    ("npc2_6", -56504, -56200, -2008, 0, "npc2_7", 2000),
    ("npc2_7", -56503, -56243, -2008, 0, "npc2_1", 4000),
    ("npc3_1", -56500, -56290, -2008, 0, "npc3_2", 1200),
    ("npc3_2", -56551, -56313, -2008, 0, "npc3_3", 1200),
    ("npc3_3", -56601, -56293, -2008, 0, "npc3_4", 1200),
    ("npc3_4", -56651, -56294, -2008, 0, "npc3_5", 1200),
    ("npc3_5", -56653, -56250, -2008, 0, "npc3_6", 1200),
    ("npc3_6", -56654, -56204, -2008, 0, "npc3_7", 1200),
    ("npc3_7", -56605, -56203, -2008, 0, "npc3_8", 1200),
    ("npc3_8", -56554, -56202, -2008, 0, "npc3_9", 1200),
    ("npc3_9", -56503, -56200, -2008, 0, "npc3_10", 1200),
    ("npc3_10", -56502, -56244, -2008, 0, "npc3_1", 900),
    ("npc4_1", -56495, -56381, -2008, 0, "npc4_2", 1200),
    ("npc4_2", -56548, -56383, -2008, 0, "npc4_3", 1200),
    ("npc4_3", -56597, -56383, -2008, 0, "npc4_4", 1200),
    ("npc4_4", -56643, -56385, -2008, 0, "npc4_5", 1200),
    ("npc4_5", -56639, -56436, -2008, 0, "npc4_6", 1200),
    ("npc4_6", -56639, -56473, -2008, 0, "npc4_7", 1200),
    ("npc4_7", -56589, -56473, -2008, 0, "npc4_8", 1200),
    ("npc4_8", -56541, -56473, -2008, 0, "npc4_9", 1200),
    ("npc4_9", -56496, -56473, -2008, 0, "npc4_10", 1200),
    ("npc4_10", -56496, -56429, -2008, 0, "npc4_1", 900),
    ("npc5_1", -56549, -56335, -2008, 0, "npc5_2", 1000),
    ("npc5_2", -56599, -56337, -2008, 0, "npc5_3", 2000),
    ("npc5_3", -56649, -56341, -2008, 0, "npc5_4", 26000),
    ("npc5_4", -56600, -56341, -2008, 0, "npc5_5", 1000),
    ("npc5_5", -56553, -56341, -2008, 0, "npc5_6", 1000),
    ("npc5_6", -56508, -56331, -2008, 0, "npc5_2", 8000),
    ("npc6_1", -56595, -56428, -2008, 0, "npc6_2", 1000),
    ("npc6_2", -56596, -56383, -2008, 0, "npc6_3", 1000),
    ("npc6_3", -56648, -56384, -2008, 0, "npc6_4", 1000),
    ("npc6_4", -56645, -56429, -2008, 0, "npc6_5", 1000),
    ("npc6_5", -56644, -56475, -2008, 0, "npc6_6", 1000),
    ("npc6_6", -56595, -56473, -2008, 0, "npc6_7", 1000),
    ("npc6_7", -56542, -56473, -2008, 0, "npc6_8", 1000),
    ("npc6_8", -56492, -56472, -2008, 0, "npc6_9", 1200),
    ("npc6_9", -56495, -56426, -2008, 0, "npc6_10", 2000),
    ("npc6_10", -56540, -56426, -2008, 0, "npc6_1", 3000),
    ("npc7_1", -56603, -56249, -2008, 0, "npc7_2", 1000),
    ("npc7_2", -56601, -56294, -2008, 0, "npc7_3", 1000),
    ("npc7_3", -56651, -56295, -2008, 0, "npc7_4", 1000),
    ("npc7_4", -56653, -56248, -2008, 0, "npc7_5", 1000),
    ("npc7_5", -56605, -56203, -2008, 0, "npc7_6", 1000),
    ("npc7_6", -56554, -56202, -2008, 0, "npc7_7", 1000),
    ("npc7_7", -56504, -56201, -2008, 0, "npc7_8", 1000),
    ("npc7_8", -56502, -56247, -2008, 0, "npc7_9", 1200),
    ("npc7_9", -56549, -56248, -2008, 0, "npc7_10", 2000),
    ("npc7_10", -56549, -56248, -2008, 0, "npc7_1", 3000),
    ("npc8_1", -56493, -56426, -2008, 0, "npc8_2", 1000),
    ("npc8_2", -56497, -56381, -2008, 0, "npc8_3", 1200),
    ("npc8_3", -56544, -56381, -2008, 0, "npc8_4", 1200),
    ("npc8_4", -56596, -56383, -2008, 0, "npc8_5", 1200),
    ("npc8_5", -56594, -56428, -2008, 0, "npc8_6", 900),
    ("npc8_6", -56645, -56429, -2008, 0, "npc8_7", 1200),
    ("npc8_7", -56647, -56384, -2008, 0, "npc8_8", 1200),
    ("npc8_8", -56649, -56362, -2008, 0, "npc8_9", 9200),
    ("npc8_9", -56654, -56429, -2008, 0, "npc8_10", 1200),
    ("npc8_10", -56644, -56474, -2008, 0, "npc8_11", 900),
    ("npc8_11", -56593, -56473, -2008, 0, "npc8_12", 1100),
    ("npc8_12", -56543, -56472, -2008, 0, "npc8_13", 1200),
    ("npc8_13", -56491, -56471, -2008, 0, "npc8_1", 1200),
    ("npc9_1", -56505, -56246, -2008, 0, "npc9_2", 1000),
    ("npc9_2", -56504, -56291, -2008, 0, "npc9_3", 1200),
    ("npc9_3", -56550, -56291, -2008, 0, "npc9_4", 1200),
    ("npc9_4", -56600, -56292, -2008, 0, "npc9_5", 1200),
    ("npc9_5", -56603, -56248, -2008, 0, "npc9_6", 900),
    ("npc9_6", -56653, -56249, -2008, 0, "npc9_7", 1200),
    ("npc9_7", -56651, -56294, -2008, 0, "npc9_8", 1200),
    ("npc9_8", -56650, -56316, -2008, 0, "npc9_9", 9200),
    ("npc9_9", -56660, -56250, -2008, 0, "npc9_10", 1200),
    ("npc9_10", -56656, -56205, -2008, 0, "npc9_11", 900),
    ("npc9_11", -56606, -56204, -2008, 0, "npc9_12", 1100),
    ("npc9_12", -56554, -56203, -2008, 0, "npc9_13", 1200),
    ("npc9_13", -56506, -56203, -2008, 0, "npc9_1", 1200),
    ("24", -56730, -56340, -2008, 0, "25", 1800),
    ("27", -56702, -56340, -2008, 0, "29", 1800),
];

/// Java `private static boolean HAS_STARTED`.
#[derive(Default)]
struct ShowState {
    has_started: bool,
}

/// Java `npc.broadcastSay(ChatType.NPC_SHOUT, MESSAGES[index])`.
fn shout(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, index: usize) {
    ctx.broadcast_npc_say(npc, ChatType::NpcShout, MESSAGES[index], &[]);
}

/// Java `npc.getAI().setIntentionMoveTo(new Location(x, y, z, heading))`.
fn move_to(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, x: i32, y: i32, z: i32) {
    ctx.npc_move_to(npc, x, y, z);
}

/// Java `npc.broadcastPacket(new PlaySound(1, file, 0, 0, 0, 0, 0))`.
fn play_sound(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, file: &str) {
    ctx.world().broadcast_packet(npc.object_id, &PlaySound::with_unknown1(1, file));
}

/// Java `addSpawn(npcId, x, y, z, heading, false, despawnDelay)`.
fn add(ctx: &mut ScriptCtx<'_>, npc_id: i32, at: (i32, i32, i32, i32), despawn_ms: i64) -> Option<ScriptNpc> {
    ctx.add_spawn(npc_id, at.0, at.1, at.2, at.3, false, despawn_ms, 0)
}

/// Java `ai.areas.FantasyIsle.TalentShow`.
pub struct TalentShow;

impl Script for TalentShow {
    fn name(&self) -> &'static str {
        "TalentShow"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FantasyIsle"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[32433, 32431, 32432, 32442, 32443, 32444, 32445, 32446, 32424, 32425, 32426, 32427, 32428]);
    }

    // The Java constructor `scheduleTimer()`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let game_time = game_time_minutes(ctx.now_ms());
        let hours = (game_time / 60) % 24;
        let minutes = game_time % 60;
        // NOTE(java-quirk): the differences are taken in game time but used as real
        // milliseconds, and a negative difference becomes `24 - |difference|`.
        let mut hour_diff = 20 - hours;
        if hour_diff < 0 {
            hour_diff = 24 - hour_diff.abs();
        }

        let mut min_diff = 30 - minutes;
        if min_diff < 0 {
            min_diff = 60 - min_diff.abs();
        }

        let diff = i64::from(hour_diff) * 3_600_000 + i64::from(min_diff) * 60_000;
        info!(
            "Fantasy Isle: MC show script starting in {} minutes and is scheduled each next 4 hours.",
            diff / 60_000
        );
        ctx.start_quest_timer("Start", diff, None, None, false); // first start
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if ctx.state::<ShowState>().has_started {
            match npc.id {
                32433 => {
                    shout(ctx, npc, 0);
                    ctx.start_quest_timer("1", 30_000, Some(npc), None, false);
                }
                32431 => {
                    move_to(ctx, npc, -56657, -56338, -2008);
                    ctx.start_quest_timer("social1", 6000, Some(npc), None, true);
                    ctx.start_quest_timer("7", 215_000, Some(npc), None, false);
                }
                32432 => {
                    ctx.start_quest_timer("social1", 6000, Some(npc), None, true);
                    ctx.start_quest_timer("7", 215_000, Some(npc), None, false);
                }
                32442..=32446 => {
                    ctx.start_quest_timer("11", 100_000, Some(npc), None, false);
                }
                32424..=32428 => {
                    ctx.start_quest_timer("social1", 5500, Some(npc), None, false);
                    ctx.start_quest_timer("social1", 12_500, Some(npc), None, false);
                    ctx.start_quest_timer("28", 19_700, Some(npc), None, false);
                }
                _ => {}
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.is_empty() {
            tracing::warn!("MC_Show: Null/Empty event for {npc:?}!");
            return None;
        }

        if event.eq_ignore_ascii_case("Start") {
            ctx.state::<ShowState>().has_started = true;
            add(ctx, MC, (-56698, -56430, -2008, 32768), 0);
            ctx.start_quest_timer("Start", 14_400_000, None, None, false); // repeat
        } else if let Some(npc) = npc
            && ctx.state::<ShowState>().has_started
        {
            if event.eq_ignore_ascii_case("6") {
                shout(ctx, npc, 6);
                move_to(ctx, npc, -56511, -56647, -2008);
                play_sound(ctx, npc, "NS22_F");
                add(ctx, SINGERS[0], (-56344, -56328, -2008, 32768), 224_000);
                add(ctx, SINGERS[1], (-56552, -56245, -2008, 36863), 224_000);
                add(ctx, SINGERS[1], (-56546, -56426, -2008, 28672), 224_000);
                add(ctx, SINGERS[1], (-56570, -56473, -2008, 28672), 224_000);
                add(ctx, SINGERS[1], (-56594, -56516, -2008, 28672), 224_000);
                add(ctx, SINGERS[1], (-56580, -56203, -2008, 36863), 224_000);
                add(ctx, SINGERS[1], (-56606, -56157, -2008, 36863), 224_000);
                ctx.start_quest_timer("7", 215_000, Some(npc), None, false);
            } else if event.eq_ignore_ascii_case("7") {
                match npc.id {
                    32433 => {
                        shout(ctx, npc, 7);
                        move_to(ctx, npc, -56698, -56430, -2008);
                        ctx.start_quest_timer("8", 12_000, Some(npc), None, false);
                    }
                    _ => {
                        ctx.cancel_quest_timer("social1", Some(npc), None);
                        move_to(ctx, npc, -56594, -56064, -2008);
                    }
                }
            } else if event.eq_ignore_ascii_case("10") {
                move_to(ctx, npc, -56483, -56665, -2034);
                play_sound(ctx, npc, "TP05_F");
                let circus = [
                    ("npc1_1", 3000, CIRCUS[0], (-56495, -56375, -2008, 32768)),
                    ("npc2_1", 3000, CIRCUS[0], (-56491, -56289, -2008, 32768)),
                    ("npc3_1", 3000, CIRCUS[1], (-56502, -56246, -2008, 32768)),
                    ("npc4_1", 3000, CIRCUS[1], (-56496, -56429, -2008, 32768)),
                    ("npc5_1", 3500, CIRCUS[2], (-56505, -56334, -2008, 32768)),
                    ("npc6_1", 4000, CIRCUS[3], (-56545, -56427, -2008, 32768)),
                    ("npc7_1", 4000, CIRCUS[3], (-56552, -56248, -2008, 32768)),
                    ("npc8_1", 3000, CIRCUS[4], (-56493, -56473, -2008, 32768)),
                    ("npc9_1", 3000, CIRCUS[4], (-56504, -56201, -2008, 32768)),
                ];
                for (timer, time, npc_id, at) in circus {
                    let spawned = add(ctx, npc_id, at, 101_000);
                    ctx.start_quest_timer(timer, time, spawned, None, false);
                }
                ctx.start_quest_timer("11", 100_000, Some(npc), None, false);
            } else if event.eq_ignore_ascii_case("11") {
                match npc.id {
                    32433 => {
                        shout(ctx, npc, 11);
                        move_to(ctx, npc, -56698, -56430, -2008);
                        ctx.start_quest_timer("12", 5000, Some(npc), None, false);
                    }
                    _ => {
                        move_to(ctx, npc, -56343, -56330, -2008);
                    }
                }
            } else if event.eq_ignore_ascii_case("14") {
                let spawned = add(ctx, INDIVIDUALS[0], (-56700, -56385, -2008, 32768), 49_000);
                ctx.start_quest_timer("social1", 2000, spawned, None, false);
                ctx.start_quest_timer("15", 7000, Some(npc), None, false);
            } else if event.eq_ignore_ascii_case("17") {
                shout(ctx, npc, 16);
                let spawned = add(ctx, INDIVIDUALS[1], (-56700, -56340, -2008, 32768), 32_000);
                ctx.start_quest_timer("social1", 2000, spawned, None, false);
                ctx.start_quest_timer("18", 9000, Some(npc), None, false);
            } else if event.eq_ignore_ascii_case("20") {
                let spawned = add(ctx, INDIVIDUALS[2], (-56703, -56296, -2008, 32768), 13_000);
                ctx.start_quest_timer("social1", 2000, spawned, None, false);
                ctx.start_quest_timer("21", 8000, Some(npc), None, false);
            } else if event.eq_ignore_ascii_case("23") {
                move_to(ctx, npc, -56702, -56340, -2008);
                ctx.start_quest_timer("24", 2800, Some(npc), None, false);
                add(ctx, SHOWSTUFF[0], (-56672, -56406, -2000, 32768), 20_900);
                add(ctx, SHOWSTUFF[1], (-56648, -56368, -2000, 32768), 20_900);
                add(ctx, SHOWSTUFF[2], (-56608, -56338, -2000, 32768), 20_900);
                add(ctx, SHOWSTUFF[3], (-56652, -56307, -2000, 32768), 20_900);
                add(ctx, SHOWSTUFF[4], (-56672, -56272, -2000, 32768), 20_900);
            } else if event.eq_ignore_ascii_case("28") {
                shout(ctx, npc, 23);
                ctx.start_quest_timer("social1", 1, Some(npc), None, false);
            } else if event.eq_ignore_ascii_case("29") {
                move_to(ctx, npc, -56730, -56340, -2008);
                ctx.start_quest_timer("clean_npc", 4100, Some(npc), None, false);
                ctx.start_quest_timer("timer_check", 60_000, None, None, true);
            } else if event.eq_ignore_ascii_case("social1") {
                ctx.broadcast_npc_social_action(npc, 1);
            } else if event.eq_ignore_ascii_case("clean_npc") {
                ctx.state::<ShowState>().has_started = false;
                ctx.delete_npc(npc);
            } else if let Some((_, message, next_event, time)) = TALKS.iter().find(|t| t.0 == event) {
                shout(ctx, npc, *message);
                ctx.start_quest_timer(next_event, *time, Some(npc), None, false);
            } else if let Some((_, x, y, z, _, next_event, time)) = WALKS.iter().find(|w| w.0 == event) {
                move_to(ctx, npc, *x, *y, *z);
                ctx.start_quest_timer(next_event, *time, Some(npc), None, false);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="32433" type="Folk" level="70" name="MC"/>
        <npc id="32431" type="Folk" level="70" name="Leyla"/>
        <npc id="32432" type="Folk" level="70" name="Backup singer"/>"#;

    #[test]
    fn the_tables_are_javas() {
        assert_eq!(MESSAGES.len(), 24);
        assert_eq!(TALKS.len(), 17);
        assert_eq!(WALKS.len(), 88);
        // Every chained event has a next event.
        assert_eq!(TALKS[0], ("1", 1, "2", 1000));
        assert!(WALKS.iter().any(|w| w.0 == "27" && w.5 == "29"));
    }

    #[test]
    fn the_start_event_spawns_the_mc_who_opens_the_show() {
        let mut registry = ScriptRegistry::new();
        registry.add(TalentShow);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.world.run_as_script("TalentShow", |ctx| {
            assert!(ctx.has_quest_timer("Start", None, None));
            let _ = TalentShow.on_event(ctx, "Start", None, None);
        });
        let mc = t.npcs_of(MC);
        assert_eq!(mc.len(), 1);
        t.ticks(2);
        // The spawn of the MC after the show started: the first line timer.
        t.world.run_as_script("TalentShow", |ctx| {
            let npc = ctx.script_npc(mc[0]).unwrap();
            assert!(ctx.has_quest_timer("1", Some(npc), None));
            // "1" shouts message 1 and chains to "2" a second later.
            let _ = TalentShow.on_event(ctx, "1", Some(npc), None);
            assert!(ctx.has_quest_timer("2", Some(npc), None));
        });
    }
}
