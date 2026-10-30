//! Preacher of Doom and Orator of Revelations: the Seven Signs NPCs that bless the winning cabal
//! and curse the losing one around them, and preach now and then.
// Java: dist/game/data/scripts/ai/others/NpcBuffers/impl/CabaleBuffer.java
//
// Java schedules two `Runnable`s on the thread pool when the NPC spawns (`CabaleAI` every 3 s,
// `Talk` every 60 s). Here they are two scheduled script events of that NPC (`schedule_event`, the
// `ThreadPool.schedule` of a script), each one scheduling itself again as in Java.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

/// Java `SevenSigns.ORATOR_NPC_ID` and `PREACHER_NPC_ID`.
const ORATOR_NPC_ID: i32 = 31094;
const PREACHER_NPC_ID: i32 = 31093;

/// Java `SevenSigns.CABAL_NULL`, `CABAL_DUSK` and `CABAL_DAWN`.
const CABAL_NULL: i32 = 0;
const CABAL_DUSK: i32 = 1;
const CABAL_DAWN: i32 = 2;

const DISTANCE_TO_WATCH_OBJECT: i64 = 900;

const CABALE_AI_DELAY_MS: i64 = 3_000;
const TALK_DELAY_MS: i64 = 60_000;
const CABALE_AI: &str = "CABALE_AI";
const TALK: &str = "TALK";

// Messages
const ORATOR_MSG: [NpcStringId; 4] = [
    NpcStringId::TheDayOfJudgmentIsNear,
    NpcStringId::TheProphecyOfDarknessHasBeenFulfilled,
    NpcStringId::AsForetoldInTheProphecyOfDarknessTheEraOfChaosHasBegun,
    NpcStringId::TheProphecyOfDarknessHasComeToPass,
];
const PREACHER_MSG: [NpcStringId; 4] = [
    NpcStringId::ThisWorldWillSoonBeAnnihilated,
    NpcStringId::AllIsLostPrepareToMeetTheGoddessOfDeath,
    NpcStringId::AllIsLostTheProphecyOfDestructionHasBeenFulfilled,
    NpcStringId::TheEndOfTimeHasComeTheProphecyOfDestructionHasBeenFulfilled,
];

// Skills
const ORATOR_FIGTER: i32 = 4364;
const ORATOR_MAGE: i32 = 4365;
const PREACHER_FIGTER: i32 = 4361;
const PREACHER_MAGE: i32 = 4362;

/// Java `getAbnormalLevel(player, skillId)`: the abnormal level of the buff of that skill, 0 without it.
fn abnormal_level(ctx: &ScriptCtx<'_>, player: ObjectId, skill_id: i32) -> i32 {
    ctx.world()
        .script_buff_abnormal_level_of_skill(player, skill_id)
        .unwrap_or(0)
}

/// Java `broadcastSay(npc, message, param, chance)`: `chance` -1 says it always, without a
/// parameter; otherwise it is said with `param` in `chance` of 10000 runs.
fn broadcast_say(
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    message: NpcStringId,
    param: Option<&str>,
    chance: i32,
) {
    if chance == -1 {
        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, message, &[]);
    } else if ctx.get_random(10_000) < chance {
        // Java skips a null parameter.
        match param {
            Some(param) => ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, message, &[param]),
            None => ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, message, &[]),
        }
    }
}

/// Java `CabaleAI.handleCast(player, skillId)`: casts the buff or curse when the player does not
/// have it (or, one time in twenty, has its first level), and says whether the NPC cast.
///
/// Curse of Destruction (loser): fighters -25% accuracy and effect resistance, mystics -25%
/// casting speed and effect resistance. Blessing of Prophecy (winner): fighters +25% max load and
/// effect resistance, mystics +25% magic cancel resist and effect resistance.
fn handle_cast(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, skill_id: i32) -> bool {
    let near = match (ctx.world().position_of(npc.object_id), ctx.world().position_of(player)) {
        (Some((nx, ny, nz)), Some((px, py, pz))) => {
            let (dx, dy, dz) = (i64::from(nx - px), i64::from(ny - py), i64::from(nz - pz));
            // `calculateDistance3D(..) < radius`.
            dx * dx + dy * dy + dz * dz < DISTANCE_TO_WATCH_OBJECT * DISTANCE_TO_WATCH_OBJECT
        }
        _ => false,
    };
    if ctx.is_dead(player) || ctx.world().player(player).is_none() || !near {
        return false;
    }

    let mut do_cast = false;
    let mut skill_level = 1;
    let level = abnormal_level(ctx, player, skill_id);
    if level == 0 {
        do_cast = true;
    } else if level == 1 && ctx.get_random(100) < 5 {
        do_cast = true;
        skill_level = 2;
    }

    if do_cast {
        ctx.cast_skill(npc, player, SkillKey::new(skill_id, skill_level));
        return true;
    }
    false
}

/// One run of Java `CabaleAI.run()`.
fn cabale_ai(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    // `_npc == null || !_npc.isSpawned()`: the loop ends with the NPC.
    if !ctx.world().npc(npc.object_id).is_some_and(|n| !n.decayed) {
        return;
    }

    let mut is_buff_a_winner = false;
    let mut is_buff_a_loser = false;

    let winning_cabal = ctx.ss_cabal_highest_score();
    let losing_cabal = match winning_cabal {
        CABAL_DAWN => CABAL_DUSK,
        CABAL_DUSK => CABAL_DAWN,
        _ => CABAL_NULL,
    };

    for player in ctx.world().visible_players(npc.object_id) {
        if ctx.world().is_invul(player) {
            continue;
        }

        let player_cabal = ctx.ss_player_cabal(player);
        let name = ctx.world().player(player).map(|p| p.data.name.clone()).unwrap_or_default();
        if player_cabal == winning_cabal && player_cabal != CABAL_NULL && npc.id == ORATOR_NPC_ID {
            if !ctx.is_mage_class(player) {
                if handle_cast(ctx, npc, player, ORATOR_FIGTER) {
                    if abnormal_level(ctx, player, ORATOR_FIGTER) == 2 {
                        broadcast_say(
                            ctx,
                            npc,
                            NpcStringId::S1IGiveYouTheBlessingOfProphecy,
                            Some(&name),
                            500,
                        );
                    } else {
                        broadcast_say(ctx, npc, NpcStringId::IBestowUponYouABlessing, None, 1);
                    }

                    is_buff_a_winner = true;
                    continue;
                }
            } else if handle_cast(ctx, npc, player, ORATOR_MAGE) {
                if abnormal_level(ctx, player, ORATOR_MAGE) == 2 {
                    broadcast_say(
                        ctx,
                        npc,
                        NpcStringId::S1IBestowUponYouTheAuthorityOfTheAbyss,
                        Some(&name),
                        500,
                    );
                } else {
                    broadcast_say(ctx, npc, NpcStringId::HeraldOfTheNewEraOpenYourEyes, None, 1);
                }

                is_buff_a_winner = true;
                continue;
            }
        } else if player_cabal == losing_cabal && player_cabal != CABAL_NULL && npc.id == PREACHER_NPC_ID {
            if !ctx.is_mage_class(player) {
                if handle_cast(ctx, npc, player, PREACHER_FIGTER) {
                    if abnormal_level(ctx, player, PREACHER_FIGTER) == 2 {
                        broadcast_say(ctx, npc, NpcStringId::ACurseUponYou, Some(&name), 500);
                    } else {
                        broadcast_say(ctx, npc, NpcStringId::YouDonTHaveAnyHopeYourEndHasCome, None, 1);
                    }

                    is_buff_a_loser = true;
                    continue;
                }
            } else if handle_cast(ctx, npc, player, PREACHER_MAGE) {
                if abnormal_level(ctx, player, PREACHER_MAGE) == 2 {
                    broadcast_say(ctx, npc, NpcStringId::S1YouMightAsWellGiveUp, Some(&name), 500);
                } else {
                    broadcast_say(ctx, npc, NpcStringId::S1YouBringAnIllWind, Some(&name), 1);
                }

                is_buff_a_loser = true;
                continue;
            }
        }

        if is_buff_a_winner && is_buff_a_loser {
            break;
        }
    }

    ctx.schedule_event(CABALE_AI, CABALE_AI_DELAY_MS, Some(npc), None);
}

/// One run of Java `Talk.run()`.
fn talk(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    // `_npc != null && !_npc.isDecayed()`
    if !ctx.world().npc(npc.object_id).is_some_and(|n| !n.decayed) {
        return;
    }
    let messages: &[NpcStringId] = if npc.id == PREACHER_NPC_ID {
        &PREACHER_MSG
    } else {
        &ORATOR_MSG
    };
    if let Some(message) = ctx.get_random_entry(messages) {
        broadcast_say(ctx, npc, message, None, -1);
    }
    ctx.schedule_event(TALK, TALK_DELAY_MS, Some(npc), None);
}

/// Java `ai.others.NpcBuffers.impl.CabaleBuffer`.
pub struct CabaleBuffer;

impl Script for CabaleBuffer {
    fn name(&self) -> &'static str {
        "CabaleBuffer"
    }

    fn path(&self) -> &'static str {
        "ai/others/NpcBuffers/impl"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[ORATOR_NPC_ID, PREACHER_NPC_ID]);
        s.add_spawn_id(&[ORATOR_NPC_ID, PREACHER_NPC_ID]);
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.schedule_event(CABALE_AI, 3_000, Some(npc), None);
        ctx.schedule_event(TALK, TALK_DELAY_MS, Some(npc), None);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        match event {
            CABALE_AI => cabale_ai(ctx, npc),
            TALK => talk(ctx, npc),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::script::player_class;
    use l2_world::sevensigns::{PERIOD_SEAL_VALIDATION, SevenSignsLoad, SevenSignsStatus, SignsPlayer};

    use super::*;
    use crate::testing::{ScriptTest, decode, name_of};

    const NPCS: &str = r#"<npc id="31094" type="Folk" level="70" name="Orator of Revelations"/>
        <npc id="31093" type="Folk" level="70" name="Preacher of Doom"/>"#;
    const SKILLS: &str = r#"
        <skill id="4361" levels="2" name="Curse of Destruction"><operateType>A1</operateType><targetType>ONE</targetType></skill>
        <skill id="4362" levels="2" name="Curse of Destruction"><operateType>A1</operateType><targetType>ONE</targetType></skill>
        <skill id="4364" levels="2" name="Blessing of Prophecy"><operateType>A1</operateType><targetType>ONE</targetType></skill>
        <skill id="4365" levels="2" name="Blessing of Prophecy"><operateType>A1</operateType><targetType>ONE</targetType></skill>"#;
    const WEDNESDAY: i64 = 1_700_000_000_000;

    fn load_cycle(t: &mut ScriptTest, dawn_leads: bool, members: &[(i32, &str)]) {
        let status = SevenSignsStatus {
            current_cycle: 4,
            active_period: PERIOD_SEAL_VALIDATION,
            last_save: WEDNESDAY - 3_600_000,
            dawn_stone_score: if dawn_leads { 100.0 } else { 0.0 },
            ..SevenSignsStatus::default()
        };
        let players = members
            .iter()
            .map(|&(char_id, cabal)| SignsPlayer {
                char_id,
                cabal: cabal.into(),
                ..SignsPlayer::default()
            })
            .collect();
        t.world.load_seven_signs_at(
            SevenSignsLoad {
                players,
                status: Some(status),
                ..SevenSignsLoad::default()
            },
            WEDNESDAY,
            |_| 0,
        );
    }

    /// Dawn leads the validation period. Player 1 is a dawn fighter, 2 a dawn mystic, 3 a dusk
    /// fighter and 4 a dusk mystic; the NPC of `npc_id` stands next to them.
    fn setup(npc_id: i32) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(CabaleBuffer);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.set_skills("", SKILLS);
        load_cycle(&mut t, true, &[(1, "dawn"), (2, "dawn"), (3, "dusk"), (4, "dusk")]);
        let human = l2_world::script::race::HUMAN;
        t.enter_as(1, 80, human, player_class::DREADNOUGHT, vec![]);
        t.enter_as(2, 80, human, player_class::ARCHMAGE, vec![]);
        t.enter_as(3, 80, human, player_class::DREADNOUGHT, vec![]);
        t.enter_as(4, 80, human, player_class::ARCHMAGE, vec![]);
        let npc = t.spawn(npc_id, 20);
        t.take();
        (t, npc)
    }

    /// What the NPC cast during the next run of its AI (3 s).
    fn cast_after_run(t: &mut ScriptTest, npc: ObjectId) -> Option<SkillKey> {
        t.world.script_clear_last_skill_cast(npc);
        t.wait_ms(3_100);
        t.world.script_last_skill_cast(npc)
    }

    #[test]
    fn the_orator_blesses_the_winning_cabal() {
        let (mut t, orator) = setup(ORATOR_NPC_ID);
        let mut seen = Vec::new();
        for _ in 0..4 {
            if let Some(skill) = cast_after_run(&mut t, orator) {
                seen.push(skill.id);
            }
        }
        // Only the dawn members (a fighter and a mystic) are blessed, never with a curse.
        assert!(!seen.is_empty());
        assert!(
            seen.iter().all(|id| *id == ORATOR_FIGTER || *id == ORATOR_MAGE),
            "{seen:?}"
        );
    }

    #[test]
    fn a_fighter_and_a_mystic_get_their_own_skill() {
        for (class, skill_id) in [
            (player_class::DREADNOUGHT, ORATOR_FIGTER),
            (player_class::ARCHMAGE, ORATOR_MAGE),
        ] {
            let mut registry = ScriptRegistry::new();
            registry.add(CabaleBuffer);
            let mut t = ScriptTest::new(registry, NPCS, "", &[]);
            t.set_skills("", SKILLS);
            load_cycle(&mut t, true, &[(1, "dawn")]);
            t.enter_as(1, 80, l2_world::script::race::HUMAN, class, vec![]);
            let orator = t.spawn(ORATOR_NPC_ID, 20);
            assert_eq!(
                cast_after_run(&mut t, orator),
                Some(SkillKey::new(skill_id, 1)),
                "class {class}"
            );
        }
    }

    #[test]
    fn the_preacher_curses_the_losing_cabal() {
        let (mut t, preacher) = setup(PREACHER_NPC_ID);
        let mut seen = Vec::new();
        for _ in 0..4 {
            if let Some(skill) = cast_after_run(&mut t, preacher) {
                seen.push(skill.id);
            }
        }
        assert!(!seen.is_empty());
        assert!(
            seen.iter().all(|id| *id == PREACHER_FIGTER || *id == PREACHER_MAGE),
            "{seen:?}"
        );
    }

    #[test]
    fn nobody_is_blessed_when_the_winner_is_the_losing_side_of_the_npc() {
        // Only dusk members are around and dawn leads: the orator has nobody to bless.
        let mut registry = ScriptRegistry::new();
        registry.add(CabaleBuffer);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.set_skills("", SKILLS);
        load_cycle(&mut t, true, &[(1, "dusk")]);
        t.enter_as(1, 80, l2_world::script::race::HUMAN, player_class::DREADNOUGHT, vec![]);
        let orator = t.spawn(ORATOR_NPC_ID, 20);
        assert_eq!(cast_after_run(&mut t, orator), None);
    }

    #[test]
    fn the_npc_preaches_every_minute() {
        let (mut t, orator) = setup(ORATOR_NPC_ID);
        t.take();
        t.wait_ms(60_100);
        let out = t.take();
        let said: Vec<_> = out
            .iter()
            .filter(|(to, b)| *to == 1 && name_of(b) == "NPC_SAY")
            .map(|(_, b)| decode::<l2_protocol_game::server::NpcSay>(b))
            .collect();
        assert!(
            said.iter()
                .any(|s| s.object_id == orator.0 && ORATOR_MSG.iter().any(|m| m.id() == s.npc_string))
        );
        // And again a minute later.
        t.take();
        t.wait_ms(60_000);
        assert!(t.take().iter().any(|(to, b)| *to == 1 && name_of(b) == "NPC_SAY"));
    }
}
