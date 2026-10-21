#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Duels (M6-DUEL-01): the challenge and the answer, the countdown, the fight, every way a duel
//! ends, party duels, and what a duelist's state changes elsewhere (`isAutoAttackable`, the PvP
//! flag, `checkPvpSkill`, the `FlagPvPZone`).
//!
//! The expected packets, messages and timings are read off the Java sources named in each
//! test, not off this code: `managers/DuelManager.java`,
//! `entity/actor/holders/player/Duel.java`, `network/clientpackets/RequestDuel*.java`,
//! `entity/actor/status/PlayerStatus.java`.
//! Java: managers/DuelManager.java, entity/actor/holders/player/Duel.java

mod party_scene;

use std::path::PathBuf;
use std::sync::Arc;

use l2_data::skill::EnchantSkillGroups;
use l2_data::{MapRegions, SkillLoadOptions, SkillTemplate, SkillTemplates, Zones};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExDuelAskStart, ExDuelUpdateUserInfo, SocialAction, SystemMessage, SystemMessageParam, UserInfo,
};
use l2_world::duel::DuelState;
use l2_world::skills::TargetContext;
use l2_world::*;
use party_scene::*;

const ENGAGED_IN_PVP: &str = "The duel was canceled because a duelist engaged in PvP combat.";
const WEAKNESS: i32 = 9040;
const WIND_STRIKE: i32 = 1177;

fn text(value: &str) -> SystemMessageParam {
    SystemMessageParam::Text(value.to_owned())
}

fn pc(value: &str) -> SystemMessageParam {
    SystemMessageParam::PlayerName(value.to_owned())
}

/// An ex packet: `0xFE`, the u16 ex id, then the fields.
fn decode_ex<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[3..])).unwrap()
}

/// The messages `to` got, as `(id, params)`.
fn msgs(sent: &Sent, to: i32) -> Vec<(SystemMessageId, Vec<SystemMessageParam>)> {
    messages(sent, to).into_iter().map(|m| (m.id, m.params)).collect()
}

/// The system message ids `to` got, in order.
fn ids(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    message_ids(sent, to)
}

/// A world without regeneration (HP numbers stay put) and a skill table with one debuff.
fn scene() -> Scene {
    let config = Config {
        skills_xml: format!(
            r#"<list>
<skill id="{WEAKNESS}" levels="1" name="Weakness"><operateType>A2</operateType><targetType>SELF</targetType>
<isDebuff>true</isDebuff><abnormalTime>600</abnormalTime><abnormalType>POISON</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="0.5"/></effect></effects></skill>
<skill id="{WIND_STRIKE}" levels="1" name="Wind Strike">
	<operateType>A1</operateType><targetType>ONE</targetType><castRange>600</castRange>
	<effectPoint>-92</effectPoint><effectRange>1100</effectRange><element>2</element><elementPower>20</elementPower>
	<hitTime>4000</hitTime><isMagic>1</isMagic><magicLevel>1</magicLevel><mpConsume>7</mpConsume>
	<mpInitialConsume>2</mpInitialConsume><power>12</power><reuseDelay>1200</reuseDelay>
	<effects><effect name="MagicalDamage" /></effects>
</skill>
</list>"#
        ),
        ..Config::default()
    };
    let mut s = Scene::with(config);
    s.world.set_regen_settings(RegenSettings {
        hp_regen_multiplier: 0.0,
        mp_regen_multiplier: 0.0,
        cp_regen_multiplier: 0.0,
        ..RegenSettings::default()
    });
    s
}

/// Alpha (P1) and Bravo (P2), next to each other.
fn pair() -> Scene {
    let mut s = scene();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s
}

fn challenge(s: &mut Scene, from: i32, name: &str, party_duel: i32) {
    s.input(
        from,
        ClientInput::DuelStart {
            name: name.into(),
            party_duel,
        },
    );
}

fn answer(s: &mut Scene, who: i32, party_duel: i32, response: i32) {
    s.input(who, ClientInput::DuelAnswerStart { party_duel, response });
}

/// Runs the world until `done` holds, at most `max` ticks. The log keeps only the last tick.
fn run_until(s: &mut Scene, max: u32, done: impl Fn(&World) -> bool) -> u32 {
    for tick in 1..=max {
        s.log.take();
        s.ticks(1);
        if done(&s.world) {
            return tick;
        }
    }
    panic!("not reached within {max} ticks");
}

/// Like [`run_until`], but also returns everything every client got on the way.
fn run_collect(s: &mut Scene, max: u32, done: impl Fn(&World) -> bool) -> (u32, Sent, Sent) {
    let mut all: Sent = Vec::new();
    for tick in 1..=max {
        s.ticks(1);
        let now = s.log.take();
        all.extend(now.iter().cloned());
        if done(&s.world) {
            return (tick, all, now);
        }
    }
    panic!("not reached within {max} ticks");
}

fn in_duel(s: &Scene, who: i32) -> bool {
    s.world.is_in_duel(id(who))
}

/// A 1 vs 1 duel between Alpha and Bravo that has started; the log is empty.
fn running_duel() -> Scene {
    let mut s = pair();
    challenge(&mut s, P1, "Bravo", 0);
    answer(&mut s, P2, 0, 1);
    run_until(&mut s, 400, |w| w.is_in_duel(id(P1)));
    s.log.take();
    s
}

fn hp(s: &Scene, who: i32) -> f64 {
    s.player(who).core.cur_hp
}

// ------------------------------------------------------------------ RequestDuelStart

/// `RequestDuelStart.runImpl` (1 vs 1): the target gets `ExDuelAskStart` and "C1 has challenged
/// you to a duel", the challenger "C1 has been challenged to a duel". The name is matched
/// ignoring case (`World.getPlayer`).
#[test]
fn a_challenge_asks_the_target_and_tells_both() {
    let mut s = pair();
    challenge(&mut s, P1, "bRAVO", 0);
    let sent = s.log.take();

    assert_eq!(names(&sent, P2), ["EX_DUEL_ASK_START", "SYSTEM_MESSAGE"]);
    let ask: ExDuelAskStart = decode_ex(of(&sent, P2, "EX_DUEL_ASK_START")[0]);
    assert_eq!(
        ask,
        ExDuelAskStart {
            requestor_name: "Alpha".into(),
            party_duel: 0
        }
    );
    assert_eq!(
        msgs(&sent, P2),
        [(SystemMessageId::C1HasChallengedYouToADuel, vec![text("Alpha")])]
    );
    assert_eq!(
        msgs(&sent, P1),
        [(SystemMessageId::C1HasBeenChallengedToADuel, vec![text("Bravo")])]
    );
    // The request is the shared transaction state: Bravo is asked, so he is busy.
    s.enter(P3, "Charlie");
    challenge(&mut s, P3, "Bravo", 0);
    assert_eq!(
        ids(&s.log.take(), P3),
        [SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater]
    );
}

/// The refusals of `RequestDuelStart`, in Java's order.
#[test]
fn the_refusals_of_a_challenge() {
    let mut s = pair();
    s.enter_at(P3, "Charlie", 10, (X + 300, Y, Z));

    // Nobody by that name, and yourself.
    challenge(&mut s, P1, "Nobody", 0);
    challenge(&mut s, P1, "Alpha", 0);
    let sent = s.log.take();
    assert_eq!(
        ids(&sent, P1),
        [
            SystemMessageId::ThereIsNoOpponentToReceiveYourChallengeForADuel,
            SystemMessageId::ThereIsNoOpponentToReceiveYourChallengeForADuel
        ]
    );

    // The challenged one is too far away (more than 250 on the plane).
    challenge(&mut s, P1, "Charlie", 0);
    let sent = s.log.take();
    assert_eq!(
        msgs(&sent, P1),
        [(
            SystemMessageId::C1CannotReceiveADuelChallengeBecauseC1IsTooFarAway,
            vec![text("Charlie")]
        )]
    );
    assert!(sent.iter().all(|(to, _)| *to == P1));

    // The target has less than half of its HP: his reason, with his name.
    s.world.set_current_hp(id(P2), 400.0, false);
    challenge(&mut s, P1, "Bravo", 0);
    let sent = s.log.take();
    assert_eq!(
        msgs(&sent, P1),
        [(
            SystemMessageId::C1CannotDuelBecauseC1SHpOrMpIsBelow50,
            vec![pc("Bravo")]
        )]
    );

    // The challenger has: "unable to request a duel at this time".
    s.world.set_current_hp(id(P2), 1000.0, false);
    s.world.set_current_mp(id(P1), 100.0, false);
    challenge(&mut s, P1, "Bravo", 0);
    assert_eq!(
        ids(&s.log.take(), P1),
        [SystemMessageId::YouAreUnableToRequestADuelAtThisTime]
    );
    s.world.set_current_mp(id(P1), 500.0, false);

    // Bravo is already asked by somebody else.
    s.enter_at(P4, "Delta", 10, (X + 50, Y, Z));
    challenge(&mut s, P4, "Bravo", 0);
    s.log.take();
    challenge(&mut s, P1, "Bravo", 0);
    let sent = s.log.take();
    assert_eq!(
        msgs(&sent, P1),
        [(
            SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater,
            vec![text("Bravo")]
        )]
    );
}

/// A player in a peace zone, a PvP zone, ... cannot duel: `canDuel` says so.
#[test]
fn nobody_duels_in_a_no_duel_zone() {
    let root = ZoneRoot::new("peace", &cuboid(1, "PeaceZone"));
    let mut s = scene();
    s.world.set_zone_data(
        Arc::new(Zones::load(&root.0)),
        Arc::new(MapRegions::load(&root.0)),
    );
    s.enter_at(P1, "Alpha", 10, (X, Y, Z));
    s.enter_at(P2, "Bravo", 10, (X + 2_000, Y, Z));
    s.ticks(5);
    assert_eq!(
        s.world.can_duel(id(P1)),
        Err(SystemMessageId::C1CannotMakeAChallengeToADuelBecauseC1IsCurrentlyInADuelProhibitedAreaPeacefulZoneSevenSignsZoneNearWaterRestartProhibitedArea)
    );
    assert_eq!(s.world.can_duel(id(P2)), Ok(()));
}

// ------------------------------------------------------------------ RequestDuelAnswerStart

/// `RequestDuelAnswerStart.runImpl`: accept (both are told, the duel is made), refuse (-1),
/// decline.
#[test]
fn the_answers() {
    // Accept.
    let mut s = pair();
    challenge(&mut s, P1, "Bravo", 0);
    s.log.take();
    answer(&mut s, P2, 0, 1);
    let sent = s.log.take();
    assert_eq!(
        msgs(&sent, P2),
        [(
            SystemMessageId::YouHaveAcceptedC1SChallengeADuelTheDuelWillBeginInAFewMoments,
            vec![text("Alpha")]
        )]
    );
    assert_eq!(
        msgs(&sent, P1),
        [(
            SystemMessageId::C1HasAcceptedYourChallengeToADuelTheDuelWillBeginInAFewMoments,
            vec![text("Bravo")]
        )]
    );
    assert_eq!(s.world.duel_count(), 1);
    // `Duel.Duel`: `setStartingDuel()` for both; "already engaged" until the duel is over.
    assert_eq!(
        s.world.can_duel(id(P1)),
        Err(SystemMessageId::C1CannotDuelBecauseC1IsAlreadyEngagedInADuel)
    );
    // The request is over: `setActiveRequester(null)`. A second "yes" has nobody to answer.
    s.log.take();
    answer(&mut s, P2, 0, 1);
    assert!(s.log.take().is_empty());

    // The client refuses duels (-1).
    let mut s = pair();
    challenge(&mut s, P1, "Bravo", 0);
    s.log.take();
    answer(&mut s, P2, 0, -1);
    let sent = s.log.take();
    assert_eq!(
        msgs(&sent, P1),
        [(
            SystemMessageId::C1IsSetToRefuseDuelRequestsAndCannotReceiveADuelRequest,
            vec![pc("Bravo")]
        )]
    );
    assert_eq!(s.world.duel_count(), 0);

    // Decline (any other value), 1 vs 1 and party.
    let mut s = pair();
    challenge(&mut s, P1, "Bravo", 0);
    s.log.take();
    answer(&mut s, P2, 0, 0);
    assert_eq!(
        msgs(&s.log.take(), P1),
        [(
            SystemMessageId::C1HasDeclinedYourChallengeToADuel,
            vec![pc("Bravo")]
        )]
    );
    challenge(&mut s, P1, "Bravo", 0);
    s.log.take();
    answer(&mut s, P2, 1, 0);
    assert_eq!(
        msgs(&s.log.take(), P1),
        [(
            SystemMessageId::TheOpposingPartyHasDeclinedYourChallengeToADuel,
            vec![]
        )]
    );

    // Nobody asked: nothing happens.
    let mut s = pair();
    answer(&mut s, P2, 0, 1);
    assert!(s.log.take().is_empty());
}

/// A flagged duelist stops the duel before it exists: both hear why, nothing starts, and the
/// request is closed.
#[test]
fn a_pvp_flag_stops_the_duel_before_it_starts() {
    let mut s = pair();
    challenge(&mut s, P1, "Bravo", 0);
    s.world.update_pvp_status(id(P1));
    s.log.take();
    answer(&mut s, P2, 0, 1);
    let sent = s.log.take();
    let flagged = SystemMessage::text(ENGAGED_IN_PVP).to_bytes();
    for who in [P1, P2] {
        assert!(
            of(&sent, who, "SYSTEM_MESSAGE").contains(&flagged.as_slice()),
            "{who:#x} is told"
        );
    }
    assert_eq!(s.world.duel_count(), 0);
}

// ------------------------------------------------------------------ the countdown and the start

/// `ScheduleStartDuelTask` / `Duel.countdown`: 3 seconds, then the count 4 (silent), a tick
/// later "begins in 3", every second "2", "1", "Let the duel begin" and the start.
#[test]
fn the_countdown_and_the_start_packets() {
    let mut s = pair();
    challenge(&mut s, P1, "Bravo", 0);
    answer(&mut s, P2, 0, 1);
    s.log.take();

    // Nothing for the first 3 seconds (60 ticks) but the silent count.
    s.ticks(59);
    assert!(s.log.take().is_empty());
    s.ticks(1);
    assert!(s.log.take().is_empty(), "count 4 says nothing");
    s.ticks(1);
    let begin = |count: i32| {
        (
            SystemMessageId::TheDuelWillBeginInS1SecondS,
            vec![SystemMessageParam::IntNumber(count)],
        )
    };
    let sent = s.log.take();
    for who in [P1, P2] {
        assert_eq!(msgs(&sent, who), [begin(3)]);
    }
    for count in [2, 1] {
        s.ticks(20);
        let sent = s.log.take();
        for who in [P1, P2] {
            assert_eq!(msgs(&sent, who), [begin(count)]);
        }
    }
    assert!(!in_duel(&s, P1));

    // The last second: "Let the duel begin" and the start, in Java's order.
    let ticks = run_until(&mut s, 40, |w| w.is_in_duel(id(P1)));
    assert_eq!(ticks, 20);
    let sent = s.log.take();
    assert_eq!(
        names(&sent, P1),
        [
            "SYSTEM_MESSAGE",           // Let the duel begin
            "USER_INFO",                // setTeam(BLUE) -> broadcastUserInfo
            "EX_BR_EXTRA_USER_INFO",    //
            "EX_DUEL_READY",            //
            "EX_DUEL_START",            //
            "EX_DUEL_UPDATE_USER_INFO", // the opponent's vitals
            "USER_INFO",                // _playerA.broadcastUserInfo()
            "EX_BR_EXTRA_USER_INFO",
            "PLAY_SOUND",
        ]
    );
    assert_eq!(
        names(&sent, P2),
        [
            "SYSTEM_MESSAGE",
            "USER_INFO", // setTeam(RED)
            "EX_BR_EXTRA_USER_INFO",
            "EX_DUEL_READY",
            "EX_DUEL_START",
            "EX_DUEL_UPDATE_USER_INFO",
            "USER_INFO", // _playerB.broadcastUserInfo()
            "EX_BR_EXTRA_USER_INFO",
            "PLAY_SOUND",
        ]
    );
    assert_eq!(ids(&sent, P1), [SystemMessageId::LetTheDuelBegin]);

    // The teams: BLUE (1) for the challenger, RED (2) for the challenged, in `UserInfo`.
    let alpha: UserInfo = decode(of(&sent, P1, "USER_INFO")[0]);
    let bravo: UserInfo = decode(of(&sent, P2, "USER_INFO")[0]);
    assert_eq!((alpha.team, bravo.team), (1, 2));
    assert_eq!((s.world.team_of(id(P1)), s.world.team_of(id(P2))), (1, 2));
    // The duel window of each side shows the other one.
    let shown: ExDuelUpdateUserInfo = decode_ex(of(&sent, P1, "EX_DUEL_UPDATE_USER_INFO")[0]);
    assert_eq!(
        shown,
        ExDuelUpdateUserInfo {
            name: "Bravo".into(),
            object_id: P2,
            class_id: 0,
            level: 10,
            current_hp: 1000,
            max_hp: 1000,
            current_mp: 500,
            max_mp: 500,
            current_cp: 200,
            max_cp: 200,
        }
    );
    let sound = of(&sent, P1, "PLAY_SOUND")[0];
    assert_eq!(
        sound,
        l2_protocol_game::server::PlaySound::with_unknown1(1, "B04_S01").to_bytes()
    );
    // The ready/start packets say "not a party duel".
    assert_eq!(of(&sent, P1, "EX_DUEL_READY")[0][3..], [0, 0, 0, 0]);
    assert_eq!(s.world.duel_state_of(id(P1)), DuelState::Duelling);
    assert_ne!(s.world.duel_id_of(id(P1)), 0);
    assert_eq!(s.world.duel_id_of(id(P1)), s.world.duel_id_of(id(P2)));
}

// ------------------------------------------------------------------ the fight and the end

/// `PlayerStatus.reduceHp`: a duelist whose HP runs out stays at 1 HP, is beaten (`DEAD`), the
/// opponent wins; both ignore further damage, and the beaten one cannot move. A second later the
/// watch task sees the winner; the loser kneels; 5 seconds later the duel ends with the winner's
/// name, the HP are back and the teams are gone.
#[test]
fn a_beaten_duelist_stays_at_one_hp_and_the_duel_ends_with_the_winner() {
    let mut s = running_duel();
    // Some damage first, to see it restored: CP takes it first (200), then the HP.
    s.world.reduce_current_hp(id(P2), 300.0, Some(id(P1)));
    s.ticks(2);
    assert_eq!((hp(&s, P2), s.player(P2).core.cur_cp), (900.0, 0.0));
    // The duel window of the opponent follows the vitals (`broadcastStatusUpdate`).
    let sent = s.log.take();
    let shown: ExDuelUpdateUserInfo = decode_ex(of(&sent, P1, "EX_DUEL_UPDATE_USER_INFO").last().unwrap());
    assert_eq!(
        (shown.object_id, shown.current_hp, shown.current_cp),
        (P2, 900, 0)
    );
    assert!(
        of(&sent, P2, "EX_DUEL_UPDATE_USER_INFO").is_empty(),
        "only the other team"
    );

    // The killing blow.
    s.world.reduce_current_hp(id(P2), 1_000_000.0, Some(id(P1)));
    assert_eq!(hp(&s, P2), 1.0);
    assert!(!s.world.is_dead(id(P2)), "beaten, not killed");
    assert_eq!(s.world.duel_state_of(id(P2)), DuelState::Dead);
    assert_eq!(s.world.duel_state_of(id(P1)), DuelState::Winner);
    // The attacker is stopped: ActionFailed.
    assert!(names(&s.log.take(), P1).contains(&"ACTION_FAIL".to_string()));
    // Neither takes damage any more.
    s.world.reduce_current_hp(id(P2), 500.0, Some(id(P1)));
    s.world.reduce_current_hp(id(P1), 500.0, Some(id(P2)));
    assert_eq!((hp(&s, P2), hp(&s, P1)), (1.0, 1000.0));
    // The beaten one is frozen: "You cannot move while frozen".
    s.log.take();
    s.input(
        P2,
        ClientInput::MoveTo {
            target_x: X + 100,
            target_y: Y,
            target_z: Z,
            origin_x: X,
            origin_y: Y,
            origin_z: Z,
            movement_mode: 1,
        },
    );
    let sent = s.log.take();
    assert_eq!(names(&sent, P2), ["ACTION_FAIL", "SYSTEM_MESSAGE"]);
    assert_eq!(
        ids(&sent, P2),
        [SystemMessageId::YouCannotMoveWhileFrozenPleaseWait]
    );

    // The watch task finds the winner within a second: the loser bows (SocialAction 7). The
    // end comes 5 seconds (100 ticks) later.
    let (ticks, all, last) = run_collect(&mut s, 130, |w| !w.is_in_duel(id(P1)));
    assert!((101..=120).contains(&ticks), "{ticks}");
    let bow: SocialAction = decode(of(&all, P1, "SOCIAL_ACTION")[0]);
    assert_eq!((bow.object_id, bow.action_id), (P2, 7));
    assert_eq!(of(&all, P2, "SOCIAL_ACTION").len(), 1, "the loser sees it too");
    let sent = last;
    for who in [P1, P2] {
        assert_eq!(
            msgs(&sent, who),
            [(SystemMessageId::C1HasWonTheDuel, vec![text("Alpha")])],
            "{who:#x}"
        );
        assert_eq!(names(&sent, who).last().unwrap(), "EX_DUEL_END");
        assert_eq!(of(&sent, who, "EX_DUEL_END")[0][3..], [0, 0, 0, 0]);
    }
    // The HP, MP and CP of the start are back; the states and the teams are gone.
    assert_eq!((hp(&s, P2), s.player(P2).core.cur_cp), (1000.0, 200.0));
    for who in [P1, P2] {
        assert!(!in_duel(&s, who));
        assert_eq!(s.world.duel_state_of(id(who)), DuelState::NoDuel);
        assert_eq!(s.world.team_of(id(who)), 0);
    }
    assert_eq!(s.world.duel_count(), 0);
    // Free to duel again.
    assert_eq!(s.world.can_duel(id(P1)), Ok(()));
}

/// The challenged player wins: the message names him (`TEAM_2_WIN` -> `_playerB`).
#[test]
fn the_challenged_player_can_win() {
    let mut s = running_duel();
    s.world.reduce_current_hp(id(P1), 1_000_000.0, Some(id(P2)));
    assert_eq!(s.world.duel_state_of(id(P2)), DuelState::Winner);
    s.log.take();
    s.ticks(21 + 101);
    let sent = s.log.take();
    for who in [P1, P2] {
        assert_eq!(
            msgs(&sent, who),
            [(SystemMessageId::C1HasWonTheDuel, vec![text("Bravo")])]
        );
    }
}

/// `RequestDuelSurrender`: the surrendering side is `DEAD`, the other wins; the end names the
/// winner. Surrendering twice does nothing.
#[test]
fn surrender() {
    let mut s = running_duel();
    s.input(P2, ClientInput::DuelSurrender);
    assert_eq!(s.world.duel_state_of(id(P2)), DuelState::Dead);
    assert_eq!(s.world.duel_state_of(id(P1)), DuelState::Winner);
    // Both are stopped (`stopFighting`).
    let sent = s.log.take();
    assert!(names(&sent, P1).contains(&"ACTION_FAIL".to_string()));
    assert!(names(&sent, P2).contains(&"ACTION_FAIL".to_string()));
    // A second surrender (even by the winner) changes nothing.
    s.input(P1, ClientInput::DuelSurrender);
    assert_eq!(s.world.duel_state_of(id(P1)), DuelState::Winner);

    s.ticks(21 + 101);
    let sent = s.log.take();
    for who in [P1, P2] {
        assert_eq!(
            msgs(&sent, who),
            [(SystemMessageId::C1HasWonTheDuel, vec![text("Alpha")])]
        );
    }
    // A player in no duel surrendering does nothing.
    s.input(P1, ClientInput::DuelSurrender);
    assert!(s.log.take().is_empty());
}

/// `TIMEOUT` (120 s from the moment the duel was made): a tie, the HP restored.
#[test]
fn a_duel_that_runs_out_of_time_is_a_tie_and_restores_the_vitals() {
    let mut s = pair();
    challenge(&mut s, P1, "Bravo", 0);
    answer(&mut s, P2, 0, 1);
    run_until(&mut s, 400, |w| w.is_in_duel(id(P1)));
    s.world.reduce_current_hp(id(P1), 300.0, Some(id(P2)));
    assert_eq!(hp(&s, P1), 900.0);
    s.log.take();

    // 120 s = 2400 ticks after the answer; the watch notices within a second.
    run_until(&mut s, 2600, |w| !w.is_in_duel(id(P1)));
    let sent = s.log.take();
    for who in [P1, P2] {
        assert_eq!(ids(&sent, who), [SystemMessageId::TheDuelHasEndedInATie]);
        assert!(!of(&sent, who, "EX_DUEL_END").is_empty());
    }
    assert_eq!((hp(&s, P1), s.player(P1).core.cur_cp), (1000.0, 200.0));
    assert_eq!(s.world.duel_count(), 0);
}

/// The ways a 1 vs 1 duel is canceled: a tie message, the vitals stay as they are.
#[test]
fn a_third_player_hitting_a_duelist_cancels_the_duel() {
    let mut s = running_duel();
    s.enter(P3, "Charlie");
    s.world.reduce_current_hp(id(P2), 100.0, Some(id(P1)));
    s.world.reduce_current_hp(id(P2), 50.0, Some(id(P3)));
    assert_eq!(s.world.duel_state_of(id(P2)), DuelState::Interrupted);
    run_until(&mut s, 30, |w| !w.is_in_duel(id(P1)));
    let sent = s.log.take();
    for who in [P1, P2] {
        assert_eq!(ids(&sent, who), [SystemMessageId::TheDuelHasEndedInATie]);
    }
    // No restore: the damage stays (the CP took both hits).
    assert_eq!((s.player(P2).core.cur_cp, hp(&s, P2)), (50.0, 1000.0));
}

#[test]
fn a_duelist_hitting_a_monster_cancels_the_duel() {
    let mut s = running_duel();
    let monster = s.spawn_monster();
    s.world.reduce_current_hp(monster, 5.0, Some(id(P1)));
    assert_eq!(s.world.duel_state_of(id(P1)), DuelState::Interrupted);
    run_until(&mut s, 30, |w| !w.is_in_duel(id(P1)));
    assert!(!in_duel(&s, P2));
    assert_eq!(s.world.team_of(id(P1)), 0);
}

#[test]
fn a_pvp_flag_during_the_duel_cancels_it_with_a_message() {
    let mut s = running_duel();
    s.world.update_pvp_status(id(P2));
    run_until(&mut s, 30, |w| !w.is_in_duel(id(P1)));
    let sent = s.log.take();
    let flagged = SystemMessage::text(ENGAGED_IN_PVP).to_bytes();
    for who in [P1, P2] {
        let all = of(&sent, who, "SYSTEM_MESSAGE");
        assert_eq!(all[0], flagged.as_slice());
        assert_eq!(all.len(), 2, "the PvP text, then the tie");
    }
}

#[test]
fn duelists_too_far_apart_cancel_the_duel() {
    let mut s = running_duel();
    s.world
        .tele_to_location(id(P2), TeleportTarget::at(X + 1_700, Y, Z));
    run_until(&mut s, 30, |w| !w.is_in_duel(id(P1)));
    assert_eq!(ids(&s.log.take(), P1), [SystemMessageId::TheDuelHasEndedInATie]);
}

#[test]
fn a_duelist_who_leaves_cancels_the_duel() {
    let mut s = running_duel();
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P2).unwrap()),
    });
    s.log.take();
    run_until(&mut s, 30, |w| !w.is_in_duel(id(P1)));
    let sent = s.log.take();
    assert_eq!(ids(&sent, P1), [SystemMessageId::TheDuelHasEndedInATie]);
    assert_eq!(s.world.duel_count(), 0);
    assert_eq!(s.world.team_of(id(P1)), 0);
    // The one who left keeps no duel state: a new login is a new Player.
    assert!(!in_duel(&s, P2));
    assert_eq!(s.world.team_of(id(P2)), 0);
}

/// `DuelManager.onBuff`: a debuff registered for a duelist is removed when the duel ends.
#[test]
fn registered_debuffs_are_removed_when_the_duel_ends() {
    let mut s = running_duel();
    s.world.apply_effects(id(P2), id(P2), SkillKey::new(WEAKNESS, 1));
    s.world.duel_on_buff(id(P2), WEAKNESS);
    assert!(s.world.effect_list(id(P2)).is_some_and(|l| !l.is_empty()));
    s.input(P1, ClientInput::DuelSurrender);
    s.ticks(21 + 101);
    assert!(!in_duel(&s, P2));
    assert!(
        s.world.effect_list(id(P2)).is_none_or(EffectList::is_empty),
        "the debuff is gone"
    );
}

/// An unregistered effect stays.
#[test]
fn other_effects_stay_when_the_duel_ends() {
    let mut s = running_duel();
    s.world.apply_effects(id(P2), id(P2), SkillKey::new(WEAKNESS, 1));
    s.input(P1, ClientInput::DuelSurrender);
    s.ticks(21 + 101);
    assert!(s.world.effect_list(id(P2)).is_some_and(|l| !l.is_empty()));
}

// ------------------------------------------------------------------ what a duel changes elsewhere

/// `Player.isAutoAttackable`: the duel partner is attackable without Ctrl while the duel is on.
#[test]
fn the_opponent_is_auto_attackable_only_during_the_duel() {
    let mut s = pair();
    s.enter(P3, "Charlie");
    assert!(!s.world.is_auto_attackable(id(P2), id(P1)));
    challenge(&mut s, P1, "Bravo", 0);
    answer(&mut s, P2, 0, 1);
    run_until(&mut s, 400, |w| w.is_in_duel(id(P1)));
    assert!(s.world.is_auto_attackable(id(P2), id(P1)));
    assert!(s.world.is_auto_attackable(id(P1), id(P2)));
    assert!(!s.world.is_auto_attackable(id(P2), id(P3)), "a bystander is not");
    assert!(!s.world.is_auto_attackable(id(P3), id(P1)));
    // A beaten duelist is no longer `DUELLING`.
    s.world.reduce_current_hp(id(P2), 1_000_000.0, Some(id(P1)));
    assert!(!s.world.is_auto_attackable(id(P2), id(P1)));
}

/// `Player.updatePvPStatus(target)` does not flag in a duel, and a duelist's death-free fight
/// gives no PvP kill (`onKillUpdatePvPKarma`).
#[test]
fn duelling_does_not_flag() {
    let mut s = running_duel();
    s.world.update_pvp_status_target(id(P1), id(P2));
    assert_eq!(s.world.pvp_flag_of(id(P1)), 0);
    // Outside the duel the same call flags.
    s.input(P1, ClientInput::DuelSurrender);
    s.ticks(21 + 101);
    s.world.update_pvp_status_target(id(P1), id(P2));
    assert_eq!(s.world.pvp_flag_of(id(P1)), 1);
}

#[test]
fn a_duel_kill_gives_no_pvp_point() {
    let mut s = running_duel();
    s.world.on_kill_update_pvp_karma(id(P1), id(P2));
    assert_eq!(s.player(P1).data.pvp_kills, 0);
    assert_eq!(s.player(P1).data.pk_kills, 0);
    assert_eq!(s.player(P1).data.karma, 0);
    // The same kill in no duel is a PK of a peaceful player.
    s.input(P1, ClientInput::DuelSurrender);
    s.ticks(21 + 101);
    s.world.on_kill_update_pvp_karma(id(P1), id(P2));
    assert_eq!(s.player(P1).data.pk_kills, 1);
    assert!(s.player(P1).data.karma > 0);
}

fn skill(effect_point: &str) -> SkillTemplate {
    let xml = format!(
        "<list><skill id=\"1\" levels=\"1\" name=\"T\"><operateType>A1</operateType>\
         <targetType>ONE</targetType><effectPoint>{effect_point}</effectPoint><effects></effects></skill></list>"
    );
    SkillTemplates::from_xml(&xml, &EnchantSkillGroups::default(), &SkillLoadOptions::default())
        .get(1, 1)
        .unwrap()
        .clone()
}

/// `Player.checkPvpSkill`: a debuff on the duel partner is fine; on a peaceful bystander it is
/// not. And `useMagic`: a duelist cannot cast at somebody outside the duel.
#[test]
fn skills_in_a_duel() {
    let mut s = pair();
    s.enter(P3, "Charlie");
    let negative = skill("-100");
    let ctx = TargetContext::default();
    assert!(
        !s.world.check_pvp_skill(id(P1), id(P2), &negative, &ctx),
        "peaceful, no duel"
    );
    challenge(&mut s, P1, "Bravo", 0);
    answer(&mut s, P2, 0, 1);
    run_until(&mut s, 400, |w| w.is_in_duel(id(P1)));
    assert!(s.world.check_pvp_skill(id(P1), id(P2), &negative, &ctx), "duel");
    assert!(
        !s.world.check_pvp_skill(id(P1), id(P3), &negative, &ctx),
        "bystander"
    );
}

/// `AuraFriendly` / `AreaFriendly.checkTarget`: `player.isInDuelWith(target)` is not a friend, a
/// bystander is.
#[test]
fn friendly_area_skills_leave_out_the_duel_opponent() {
    let mut s = pair();
    s.enter(P3, "Charlie");
    let friendly = {
        let xml = "<list><skill id=\"1\" levels=\"1\" name=\"T\"><operateType>A1</operateType>                   <targetType>AURA_FRIENDLY</targetType><affectRange>1000</affectRange>                   <effectPoint>100</effectPoint><effects></effects></skill></list>";
        SkillTemplates::from_xml(xml, &EnchantSkillGroups::default(), &SkillLoadOptions::default())
            .get(1, 1)
            .unwrap()
            .clone()
    };
    let targets = |s: &mut Scene| {
        let mut list: Vec<i32> = s
            .world
            .skill_target_list(&friendly, id(P1), false, None, &TargetContext::default())
            .targets
            .iter()
            .map(|t| t.0)
            .collect();
        list.sort_unstable();
        list
    };
    assert_eq!(targets(&mut s), [P2, P3], "the caster is not in the list");

    challenge(&mut s, P1, "Bravo", 0);
    answer(&mut s, P2, 0, 1);
    run_until(&mut s, 400, |w| w.is_in_duel(id(P1)));
    assert_eq!(targets(&mut s), [P3], "the opponent is out");
}

/// `Player.useMagic` (`checkUseMagicConditions`): "Are the target and the player in the same
/// duel?" A duelist who aims at somebody else is refused with a message.
#[test]
fn a_duelist_cannot_cast_at_somebody_outside_the_duel() {
    let mut s = scene();
    let mut alpha = player_data(P1, "Alpha", 10, (X, Y, Z));
    alpha.skills = vec![SkillData {
        skill_id: WIND_STRIKE,
        skill_level: 1,
    }];
    s.enter_data(alpha);
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    challenge(&mut s, P1, "Bravo", 0);
    answer(&mut s, P2, 0, 1);
    run_until(&mut s, 400, |w| w.is_in_duel(id(P1)));

    // At the opponent: no refusal for the duel (the cast starts).
    s.world.set_target(id(P1), Some(id(P2)));
    s.log.take();
    s.input(
        P1,
        ClientInput::MagicSkillUse {
            magic_id: WIND_STRIKE,
            ctrl_pressed: false,
            shift_pressed: false,
        },
    );
    let refusal = SystemMessage::text("You cannot do this while duelling.").to_bytes();
    assert!(!of(&s.log.take(), P1, "SYSTEM_MESSAGE").contains(&refusal.as_slice()));
    s.ticks(100);

    // At a bystander: refused, nothing is cast.
    s.world.set_target(id(P1), Some(id(P3)));
    s.log.take();
    s.input(
        P1,
        ClientInput::MagicSkillUse {
            magic_id: WIND_STRIKE,
            ctrl_pressed: false,
            shift_pressed: false,
        },
    );
    let sent = s.log.take();
    assert_eq!(names(&sent, P1), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(of(&sent, P1, "SYSTEM_MESSAGE")[0], refusal.as_slice());
}

// ------------------------------------------------------------------ party duels

/// Alpha leads Bravo (party A), Charlie leads Delta (party B).
fn two_parties() -> Scene {
    let mut s = scene();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.enter(P4, "Delta");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    s.form_party(P3, &[(P4, "Delta")], 0);
    s
}

#[test]
fn the_refusals_of_a_party_challenge() {
    let mut s = two_parties();
    let text_msg = |t: &str| SystemMessage::text(t).to_bytes();

    // Only the leader asks.
    challenge(&mut s, P2, "Charlie", 1);
    let sent = s.log.take();
    assert_eq!(
        of(&sent, P2, "SYSTEM_MESSAGE"),
        [text_msg("You have to be the leader of a party in order to request a party duel.").as_slice()]
    );
    // The target must have a party; and not be in the same one.
    s.enter(P5, "Echo");
    challenge(&mut s, P1, "Echo", 1);
    assert_eq!(
        ids(&s.log.take(), P1),
        [SystemMessageId::SinceThePersonYouChallengedIsNotCurrentlyInAPartyTheyCannotDuelAgainstYourParty]
    );
    challenge(&mut s, P1, "Bravo", 1);
    assert_eq!(
        of(&s.log.take(), P1, "SYSTEM_MESSAGE"),
        [text_msg("This player is a member of your own party.").as_slice()]
    );
    // Every member of the own party must be ready.
    s.world.set_current_hp(id(P2), 100.0, false);
    challenge(&mut s, P1, "Charlie", 1);
    assert_eq!(
        of(&s.log.take(), P1, "SYSTEM_MESSAGE"),
        [text_msg("Not all the members of your party are ready for a duel.").as_slice()]
    );
    s.world.set_current_hp(id(P2), 1000.0, false);
    // ... and of the other one.
    s.world.set_current_hp(id(P4), 100.0, false);
    challenge(&mut s, P1, "Charlie", 1);
    assert_eq!(
        ids(&s.log.take(), P1),
        [SystemMessageId::TheOpposingPartyIsCurrentlyUnableToAcceptAChallengeToADuel]
    );
}

/// The question goes to the leader of the challenged party, whoever was named; "C1's party has
/// challenged your party" goes to the one named (a Java quirk).
#[test]
fn a_party_challenge_asks_the_leader_of_the_other_party() {
    let mut s = two_parties();
    challenge(&mut s, P1, "Delta", 1);
    let sent = s.log.take();
    let ask: ExDuelAskStart = decode_ex(of(&sent, P3, "EX_DUEL_ASK_START")[0]);
    assert_eq!(
        ask,
        ExDuelAskStart {
            requestor_name: "Alpha".into(),
            party_duel: 1
        }
    );
    assert_eq!(
        msgs(&sent, P1),
        [(
            SystemMessageId::C1SPartyHasBeenChallengedToADuel,
            vec![text("Charlie")]
        )]
    );
    // Delta (named) hears it, not Charlie (asked).
    assert_eq!(
        msgs(&sent, P4),
        [(
            SystemMessageId::C1SPartyHasChallengedYourPartyToADuel,
            vec![text("Alpha")]
        )]
    );
    assert!(msgs(&sent, P3).is_empty());
    // The leader is busy now.
    challenge(&mut s, P1, "Delta", 1);
    assert_eq!(
        msgs(&s.log.take(), P1),
        [(
            SystemMessageId::C1IsOnAnotherTaskPleaseTryAgainLater,
            vec![text("Charlie")]
        )]
    );
}

/// A whole party duel, in place (no arena instance yet): the countdown with the 20 seconds wait,
/// the teams, the windows, a team that is beaten, the end.
#[test]
fn a_party_duel_from_the_answer_to_the_end() {
    let mut s = two_parties();
    challenge(&mut s, P1, "Charlie", 1);
    s.log.take();
    answer(&mut s, P3, 1, 1);
    let sent = s.log.take();
    assert_eq!(
        msgs(&sent, P3),
        [
            (
                SystemMessageId::YouHaveAcceptedC1SChallengeToAPartyDuelTheDuelWillBeginInAFewMoments,
                vec![text("Alpha")]
            ),
            (
                SystemMessageId::InAMomentYouWillBeTransportedToTheSiteWhereTheDuelWillTakePlace,
                vec![]
            )
        ]
    );
    assert_eq!(
        msgs(&sent, P1)[0],
        (
            SystemMessageId::S1HasAcceptedYourChallengeToDuelAgainstTheirPartyTheDuelWillBeginInAFewMoments,
            vec![text("Charlie")]
        )
    );
    // The transport message goes to every member of both parties.
    for who in [P1, P2, P3, P4] {
        assert!(
            ids(&sent, who)
                .contains(&SystemMessageId::InAMomentYouWillBeTransportedToTheSiteWhereTheDuelWillTakePlace)
        );
    }

    // 3 s, then 20 s of waiting before the "3": nothing is said, nobody is in the duel.
    s.ticks(60 + 399);
    // (The party's own periodic packets and the character infos of the joins are not the duel.)
    let quiet = s.log.take();
    assert!(
        quiet
            .iter()
            .all(|(_, b)| !name_of(b).contains("DUEL") && name_of(b) != "SYSTEM_MESSAGE"),
        "{:?}",
        quiet.iter().map(|(to, b)| (*to, name_of(b))).collect::<Vec<_>>()
    );
    assert!(!in_duel(&s, P1));
    s.ticks(1);
    let sent = s.log.take();
    for who in [P1, P2, P3, P4] {
        assert_eq!(
            msgs(&sent, who),
            [(
                SystemMessageId::TheDuelWillBeginInS1SecondS,
                vec![SystemMessageParam::IntNumber(3)]
            )]
        );
    }

    // 2, 1, then the start: three more seconds.
    let ticks = run_until(&mut s, 100, |w| w.is_in_duel(id(P1)));
    assert_eq!(ticks, 60);
    let sent = s.log.take();
    for who in [P1, P2, P3, P4] {
        assert!(in_duel(&s, who));
        assert_eq!(ids(&sent, who), [SystemMessageId::LetTheDuelBegin], "{who:#x}");
        let ready = of(&sent, who, "EX_DUEL_READY")[0];
        assert_eq!(ready[3..], [1, 0, 0, 0], "party duel flag");
        assert_eq!(
            of(&sent, who, "EX_DUEL_UPDATE_USER_INFO").len(),
            2,
            "the two opponents"
        );
    }
    assert_eq!([P1, P2, P3, P4].map(|p| s.world.team_of(id(p))), [1, 1, 2, 2]);
    // A member who is not a duelist: one more beaten member does not end the duel.
    s.world.reduce_current_hp(id(P3), 1_000_000.0, Some(id(P1)));
    assert_eq!(s.world.duel_state_of(id(P3)), DuelState::Dead);
    assert_eq!(
        s.world.duel_state_of(id(P1)),
        DuelState::Duelling,
        "Delta still stands"
    );
    s.ticks(40);
    assert!(in_duel(&s, P1));
    // The other one falls: party A wins.
    s.world.reduce_current_hp(id(P4), 1_000_000.0, Some(id(P2)));
    assert_eq!(s.world.duel_state_of(id(P1)), DuelState::Winner);
    assert_eq!(s.world.duel_state_of(id(P2)), DuelState::Winner);
    s.log.take();
    s.ticks(21);
    let sent = s.log.take();
    // The whole losing party bows.
    let bowed: Vec<i32> = [P3, P4]
        .into_iter()
        .filter(|p| {
            of(&sent, P1, "SOCIAL_ACTION")
                .iter()
                .any(|raw| decode::<SocialAction>(raw).object_id == *p)
        })
        .collect();
    assert_eq!(bowed, [P3, P4]);
    s.ticks(101);
    let sent = s.log.take();
    for who in [P1, P2, P3, P4] {
        assert_eq!(
            msgs(&sent, who),
            [(SystemMessageId::C1SPartyHasWonTheDuel, vec![text("Alpha")])],
            "{who:#x}"
        );
        assert_eq!(of(&sent, who, "EX_DUEL_END")[0][3..], [1, 0, 0, 0]);
        assert!(!in_duel(&s, who));
        assert_eq!(s.world.team_of(id(who)), 0);
    }
    assert_eq!(hp(&s, P3), 1000.0, "restored");
    assert_eq!(hp(&s, P4), 1000.0);
}

/// `Duel.doSurrender` in a party duel: every member of the surrendering party is `DEAD`, the
/// other party wins, whoever of the party pressed the button.
#[test]
fn a_party_duel_surrender_is_the_whole_party() {
    let mut s = two_parties();
    challenge(&mut s, P1, "Charlie", 1);
    answer(&mut s, P3, 1, 1);
    run_until(&mut s, 600, |w| w.is_in_duel(id(P1)));
    s.input(P4, ClientInput::DuelSurrender);
    assert_eq!(
        [P1, P2, P3, P4].map(|p| s.world.duel_state_of(id(p))),
        [
            DuelState::Winner,
            DuelState::Winner,
            DuelState::Dead,
            DuelState::Dead
        ]
    );
    s.log.take();
    s.ticks(21 + 101);
    let sent = s.log.take();
    for who in [P1, P2, P3, P4] {
        assert_eq!(
            msgs(&sent, who),
            [(SystemMessageId::C1SPartyHasWonTheDuel, vec![text("Alpha")])]
        );
    }
}

/// `Party.removePartyMember` -> `DuelManager.onRemoveFromParty`: a member who leaves is taken out
/// of the duel; if it is one of the two named duelists the duel is cancelled for everybody, with
/// no end message (Java sets both duelists to null).
#[test]
fn leaving_the_party_in_a_party_duel() {
    // A plain member.
    let mut s = two_parties();
    s.enter(P5, "Echo");
    s.invite(P3, "Echo", 0);
    s.answer(P5, 1);
    challenge(&mut s, P1, "Charlie", 1);
    answer(&mut s, P3, 1, 1);
    run_until(&mut s, 600, |w| w.is_in_duel(id(P1)));
    assert!(in_duel(&s, P5));
    s.input(P5, ClientInput::WithdrawParty);
    assert!(!in_duel(&s, P5));
    assert!(in_duel(&s, P1) && in_duel(&s, P3), "the duel goes on");
    assert_eq!(s.world.duel_count(), 1);

    // One of the named duelists: everybody is out of the duel.
    s.log.take();
    s.input(P1, ClientInput::WithdrawParty);
    for who in [P1, P2, P3, P4] {
        assert!(!in_duel(&s, who), "{who:#x}");
    }
    // NOTE(java-quirk): the team colours stay.
    assert_eq!(s.world.team_of(id(P3)), 2);
    // The watch task finds no duelists: cancelled, cleaned up, nothing said.
    s.log.take();
    s.ticks(40);
    let sent = s.log.take();
    assert_eq!(s.world.duel_count(), 0);
    assert!(sent.iter().all(|(_, b)| name_of(b) != "EX_DUEL_END"));
}

/// `Party.setLeader`: a duelist cannot be made leader (nothing happens, nothing is said).
#[test]
fn a_duelist_cannot_become_party_leader() {
    let mut s = two_parties();
    challenge(&mut s, P1, "Charlie", 1);
    answer(&mut s, P3, 1, 1);
    run_until(&mut s, 600, |w| w.is_in_duel(id(P1)));
    s.log.take();
    s.input(P1, ClientInput::ChangePartyLeader { name: "Bravo".into() });
    assert!(s.log.take().is_empty());
    assert_eq!(s.members(P1)[0], P1);
}

// ------------------------------------------------------------------ FlagPvPZone

struct ZoneRoot(PathBuf);

impl ZoneRoot {
    fn new(tag: &str, zone_xml: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-duel-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["zones", "mapregion"] {
            std::fs::create_dir_all(dir.join("data").join(sub)).unwrap();
        }
        std::fs::write(
            dir.join("data/zones/test.xml"),
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">{zone_xml}</list>\n"),
        )
        .unwrap();
        std::fs::write(
            dir.join("data/mapregion/talking_island_town.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
             </region>\n</list>\n",
        )
        .unwrap();
        Self(dir)
    }
}

impl Drop for ZoneRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A cuboid zone `size` around the scene's center.
fn cuboid(zone_id: i32, zone_type: &str) -> String {
    format!(
        "<zone id=\"{zone_id}\" name=\"z{zone_id}\" type=\"{zone_type}\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
         <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>\n",
        X - 500,
        Y - 500,
        X + 500,
        Y + 500
    )
}

/// `FlagPvPZone.onEnter` / `onExit`: the flag follows the player in and out, with a message and
/// a `UserInfo` each way.
#[test]
fn the_flag_pvp_zone_flags_and_unflags() {
    let root = ZoneRoot::new("flag", &cuboid(1, "FlagPvPZone"));
    let mut s = scene();
    s.world.set_zone_data(
        Arc::new(Zones::load(&root.0)),
        Arc::new(MapRegions::load(&root.0)),
    );
    // Starts outside, 2000 away.
    s.enter_at(P1, "Alpha", 10, (X + 2_000, Y, Z));
    s.enter_at(P2, "Bravo", 10, (X + 2_050, Y, Z));
    s.ticks(5);
    s.log.take();
    assert_eq!(s.world.pvp_flag_of(id(P1)), 0);

    s.world.tele_to_location(id(P1), TeleportTarget::at(X, Y, Z));
    s.input(P1, ClientInput::Appearing);
    s.ticks(5);
    assert_eq!(s.world.pvp_flag_of(id(P1)), 1, "flagged inside");
    let sent = s.log.take();
    let entered = SystemMessage::text("You entered a PvP Flag zone.").to_bytes();
    assert!(of(&sent, P1, "SYSTEM_MESSAGE").contains(&entered.as_slice()));
    let flagged: Vec<i32> = of(&sent, P1, "USER_INFO")
        .iter()
        .map(|raw| decode::<UserInfo>(raw).pvp_flag)
        .collect();
    assert!(flagged.contains(&1), "{flagged:?}");

    s.world
        .tele_to_location(id(P1), TeleportTarget::at(X + 2_000, Y, Z));
    s.input(P1, ClientInput::Appearing);
    s.ticks(5);
    assert_eq!(s.world.pvp_flag_of(id(P1)), 0, "unflagged outside");
    let sent = s.log.take();
    let left = SystemMessage::text("You left the PvP Flag zone.").to_bytes();
    assert!(of(&sent, P1, "SYSTEM_MESSAGE").contains(&left.as_slice()));
}
