//! Server packets of the Seven Signs status (`SSQ_STATUS`, four pages) and of the Seed map.
//!
//! The status packet is decided by the Seven Signs state. Java's `writeImpl` works the
//! percentages, the seal results and the messages out from that state, so [`SsqStatus::new`]
//! does the same from a [`SevenSignsView`] of plain values the world fills in. The struct itself
//! holds what goes on the wire, so it decodes back to the same value (for the proxy and tests).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;
use crate::generated::system_message_id::SystemMessageId;

/// Java `SevenSigns.CABAL_NULL`: nobody.
pub const CABAL_NULL: i32 = 0;
/// Java `SevenSigns.CABAL_DUSK`.
pub const CABAL_DUSK: i32 = 1;
/// Java `SevenSigns.CABAL_DAWN`.
pub const CABAL_DAWN: i32 = 2;

/// Java `SevenSigns.PERIOD_COMP_RECRUITING`.
pub const PERIOD_COMP_RECRUITING: i32 = 0;
/// Java `SevenSigns.PERIOD_COMPETITION`.
pub const PERIOD_COMPETITION: i32 = 1;
/// Java `SevenSigns.PERIOD_COMP_RESULTS`.
pub const PERIOD_COMP_RESULTS: i32 = 2;
/// Java `SevenSigns.PERIOD_SEAL_VALIDATION`.
pub const PERIOD_SEAL_VALIDATION: i32 = 3;

/// Java `SevenSignsFestival.FESTIVAL_LEVEL_SCORES`: one per festival (five).
pub const FESTIVAL_LEVEL_SCORES: [i32; 5] = [60, 70, 100, 120, 150];

/// The Seven Signs state a status page is built from. Seals are indexed `1..=3` (index `0` is
/// unused), cabals as [`CABAL_NULL`], [`CABAL_DUSK`] and [`CABAL_DAWN`].
#[derive(Debug, Clone, PartialEq)]
pub struct SevenSignsView {
    /// Java `getCabalHighestScore()`.
    pub winning_cabal: i32,
    /// Java `getCurrentPeriod()`, one of the `PERIOD_*` values.
    pub current_period: i32,
    /// Java `getCurrentCycle()`.
    pub current_cycle: i32,
    /// The player's cabal and seal (`getPlayerCabal`, `getPlayerSeal`).
    pub player_cabal: i32,
    pub player_seal: i32,
    /// `getPlayerStoneContrib` and `getPlayerAdenaCollect`.
    pub player_stone_contrib: i64,
    pub player_adena_collect: i64,
    /// `getCurrentStoneScore(CABAL_DAWN / CABAL_DUSK)`.
    pub dawn_stone_score: f64,
    pub dusk_stone_score: f64,
    /// `getCurrentFestivalScore(CABAL_DAWN / CABAL_DUSK)`.
    pub dawn_festival_score: i32,
    pub dusk_festival_score: i32,
    /// `getCurrentScore(CABAL_DAWN / CABAL_DUSK)`.
    pub dawn_total_score: i32,
    pub dusk_total_score: i32,
    /// `getTotalMembers(CABAL_DAWN / CABAL_DUSK)`.
    pub total_dawn_members: i32,
    pub total_dusk_members: i32,
    /// `getSealOwner(seal)`, indexed by seal.
    pub seal_owner: [i32; 4],
    /// `getSealProportion(seal, CABAL_DAWN / CABAL_DUSK)`, indexed by seal.
    pub dawn_proportion: [i32; 4],
    pub dusk_proportion: [i32; 4],
    /// The highest festival scores, one per festival (see [`FESTIVAL_LEVEL_SCORES`]).
    pub festivals: Vec<FestivalHighScore>,
}

/// The best score of one festival for both cabals, with the members of that score (Java
/// `getHighestScoreData(...).getString("members").split(",")`, one name per entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FestivalHighScore {
    pub dusk_score: i64,
    pub dusk_members: Vec<String>,
    pub dawn_score: i64,
    pub dawn_members: Vec<String>,
}

/// Java `Math.round(float)` for the non-negative values used here.
fn java_round(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// The seal result of page 4: the owner the packet shows and the message id. Java's decision
/// tree, kept as written.
fn seal_outcome(owner: i32, winning: i32, dawn_percent: i32, dusk_percent: i32) -> (u8, i32) {
    let tie = MSG_TIE;
    let owned_35 = MSG_OWNED_35;
    let not_owned_voted = MSG_NOT_OWNED_VOTED;
    let owned_10 = MSG_OWNED_10;
    let owned_less_10 = MSG_OWNED_LESS_10;
    let (cabal, message) = match owner {
        CABAL_NULL => match winning {
            CABAL_DAWN => {
                if dawn_percent >= 35 {
                    (CABAL_DAWN, owned_35)
                } else {
                    (CABAL_NULL, not_owned_voted)
                }
            }
            CABAL_DUSK => {
                if dusk_percent >= 35 {
                    (CABAL_DUSK, owned_35)
                } else {
                    (CABAL_NULL, not_owned_voted)
                }
            }
            _ => (CABAL_NULL, tie),
        },
        CABAL_DAWN => match winning {
            CABAL_DAWN => {
                if dawn_percent >= 10 {
                    (CABAL_DAWN, owned_10)
                } else {
                    (CABAL_NULL, owned_less_10)
                }
            }
            CABAL_DUSK => {
                if dusk_percent >= 35 {
                    (CABAL_DUSK, owned_35)
                } else if dawn_percent >= 10 {
                    (CABAL_DAWN, owned_10)
                } else {
                    (CABAL_NULL, owned_less_10)
                }
            }
            _ => {
                if dawn_percent >= 10 {
                    (CABAL_DAWN, owned_10)
                } else {
                    (CABAL_NULL, tie)
                }
            }
        },
        _ => match winning {
            CABAL_DAWN => {
                if dawn_percent >= 35 {
                    (CABAL_DAWN, owned_35)
                } else if dusk_percent >= 10 {
                    (CABAL_DUSK, owned_10)
                } else {
                    (CABAL_NULL, owned_less_10)
                }
            }
            CABAL_DUSK => {
                if dusk_percent >= 10 {
                    (CABAL_DUSK, owned_10)
                } else {
                    (CABAL_NULL, owned_less_10)
                }
            }
            _ => {
                if dusk_percent >= 10 {
                    (CABAL_DUSK, owned_10)
                } else {
                    (CABAL_NULL, tie)
                }
            }
        },
    };
    (cabal as u8, message)
}

/// Client string ids of the Seven Signs messages (`SystemMessageId`, generated).
const MSG_TIE: i32 = SystemMessageId::TheCompetitionHasEndedInATieThereforeNobodyHasBeenAwardedTheSeal.id();
const MSG_OWNED_35: i32 = SystemMessageId::AlthoughTheSealWasNotOwned35OrMorePeopleHaveParticipated.id();
const MSG_OWNED_10: i32 =
    SystemMessageId::SinceTheSealWasOwnedDuringThePreviousPeriodAnd10OrMorePeopleHaveParticipated.id();
const MSG_OWNED_LESS_10: i32 =
    SystemMessageId::AlthoughTheSealWasOwnedDuringThePreviousPeriodLessThan10OfPeopleHaveVoted.id();
const MSG_NOT_OWNED_VOTED: i32 =
    SystemMessageId::SinceTheSealWasNotOwnedDuringThePreviousPeriodAndSinceLessThan35PercentOfPeopleHaveVoted
        .id();
const MSG_INITIAL_PERIOD: i32 = SystemMessageId::ThisIsTheInitialPeriod.id();
const MSG_COMPETITION_UNDERWAY: i32 = SystemMessageId::TheSsqCompetitionPeriodIsUnderway.id();
const MSG_STATISTICS_PERIOD: i32 = SystemMessageId::ThisIsAPeriodWhenServerStatisticsAreCalculated.id();
const MSG_SEAL_VALIDATION_PERIOD: i32 = SystemMessageId::ThisIsTheSealValidationPeriod.id();
const MSG_UNTIL_TODAY: i32 = SystemMessageId::UntilTodayAt600PM.id();
const MSG_UNTIL_NEXT_MONDAY: i32 = SystemMessageId::UntilNextMondayAt600PM.id();

/// The score block of one cabal on page 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SsqScore {
    /// Seal stone score, the share of 500.
    pub stone_score: i64,
    pub festival_score: i64,
    pub total_score: i64,
    /// Share of the total score in percent.
    pub percent: u8,
}

/// Page 1: the cycle, the period messages, the player's own seal and the two cabals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SsqCycle {
    pub cycle: i32,
    /// Client string id for the period (Java `SystemMessageId` of the first switch).
    pub period_message: i32,
    /// Client string id for the deadline (`UntilTodayAt600PM` or `UntilNextMondayAt600PM`).
    pub deadline_message: i32,
    pub player_cabal: u8,
    pub player_seal: u8,
    /// Seal stones turned in by the player.
    pub stone_contrib: i64,
    /// Ancient adena to collect.
    pub adena_collect: i64,
    pub dusk: SsqScore,
    pub dawn: SsqScore,
}

/// Page 2: one festival with the best score of each cabal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsqFestival {
    pub level_score: i32,
    pub dusk_score: i64,
    pub dusk_members: Vec<String>,
    pub dawn_score: i64,
    pub dawn_members: Vec<String>,
}

/// One seal row of page 3: the owner and the share of each cabal in percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SsqSealRow {
    pub seal: u8,
    pub owner: u8,
    pub dusk_percent: u8,
    pub dawn_percent: u8,
}

/// Page 3: the seal rules and the rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsqSeals {
    /// Java `10`: the minimum for the winning cabal to keep its seal.
    pub retain_limit: u8,
    /// Java `35`: the minimum for the winning cabal to claim a seal.
    pub claim_limit: u8,
    pub rows: Vec<SsqSealRow>,
}

/// One seal of page 4: the owner the packet predicts and the message that says why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SsqSealOutcome {
    pub seal: u8,
    pub owner: u8,
    pub message: i32,
}

/// Page 4: the predicted winner and the outcome of each seal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsqPredicted {
    pub winner: u8,
    pub seals: Vec<SsqSealOutcome>,
}

/// The part after the page and the period byte. Pages other than 1 to 4 have none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsqStatusBody {
    None,
    Cycle(SsqCycle),
    Festivals(Vec<SsqFestival>),
    Seals(SsqSeals),
    Predicted(SsqPredicted),
}

// Java: gameserver/network/serverpackets/SSQStatus.java
/// One page of the Seven Signs status (`SSQ_STATUS`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsqStatus {
    /// Java `_page` (`writeByte`).
    pub page: i8,
    /// Java `getCurrentPeriod()` (`writeByte`).
    pub period: u8,
    pub body: SsqStatusBody,
}

impl SsqStatus {
    pub const OPCODE: ServerOpcode = ServerOpcode::SsqStatus;

    /// Java `writeImpl`: the page's body from the Seven Signs state.
    pub fn new(page: i8, view: &SevenSignsView) -> Self {
        let body = match page {
            1 => SsqStatusBody::Cycle(cycle_page(view)),
            2 => SsqStatusBody::Festivals(festival_page(view)),
            3 => SsqStatusBody::Seals(seal_page(view)),
            4 => SsqStatusBody::Predicted(predicted_page(view)),
            _ => SsqStatusBody::None,
        };
        Self {
            page,
            period: view.current_period as u8,
            body,
        }
    }
}

fn period_messages(period: i32) -> (i32, i32) {
    let first = match period {
        PERIOD_COMP_RECRUITING => MSG_INITIAL_PERIOD,
        PERIOD_COMPETITION => MSG_COMPETITION_UNDERWAY,
        PERIOD_COMP_RESULTS => MSG_STATISTICS_PERIOD,
        _ => MSG_SEAL_VALIDATION_PERIOD,
    };
    let deadline = match period {
        PERIOD_COMP_RECRUITING | PERIOD_COMP_RESULTS => MSG_UNTIL_TODAY,
        _ => MSG_UNTIL_NEXT_MONDAY,
    };
    (first, deadline)
}

fn score_block(stone_prop: i32, festival: i32, total: i32, percent: i32) -> SsqScore {
    SsqScore {
        stone_score: i64::from(stone_prop),
        festival_score: i64::from(festival),
        total_score: i64::from(total),
        percent: percent as u8,
    }
}

fn cycle_page(view: &SevenSignsView) -> SsqCycle {
    let (period_message, deadline_message) = period_messages(view.current_period);
    let total_stone = view.dusk_stone_score + view.dawn_stone_score;
    let mut dusk_stone_prop = 0;
    let mut dawn_stone_prop = 0;
    if total_stone != 0.0 {
        let total = total_stone as f32;
        dusk_stone_prop = java_round((view.dusk_stone_score as f32 / total) * 500.0);
        dawn_stone_prop = java_round((view.dawn_stone_score as f32 / total) * 500.0);
    }
    let total_overall = view.dusk_total_score + view.dawn_total_score;
    let mut dusk_percent = 0;
    let mut dawn_percent = 0;
    if total_overall != 0 {
        dawn_percent = java_round((view.dawn_total_score as f32 / total_overall as f32) * 100.0);
        dusk_percent = java_round((view.dusk_total_score as f32 / total_overall as f32) * 100.0);
    }
    SsqCycle {
        cycle: view.current_cycle,
        period_message,
        deadline_message,
        player_cabal: view.player_cabal as u8,
        player_seal: view.player_seal as u8,
        stone_contrib: view.player_stone_contrib,
        adena_collect: view.player_adena_collect,
        dusk: score_block(
            dusk_stone_prop,
            view.dusk_festival_score,
            view.dusk_total_score,
            dusk_percent,
        ),
        dawn: score_block(
            dawn_stone_prop,
            view.dawn_festival_score,
            view.dawn_total_score,
            dawn_percent,
        ),
    }
}

fn festival_page(view: &SevenSignsView) -> Vec<SsqFestival> {
    FESTIVAL_LEVEL_SCORES
        .iter()
        .enumerate()
        .map(|(index, level_score)| {
            let high = view.festivals.get(index).cloned().unwrap_or(FestivalHighScore {
                dusk_score: 0,
                dusk_members: Vec::new(),
                dawn_score: 0,
                dawn_members: Vec::new(),
            });
            SsqFestival {
                level_score: *level_score,
                dusk_score: high.dusk_score,
                dusk_members: high.dusk_members,
                dawn_score: high.dawn_score,
                dawn_members: high.dawn_members,
            }
        })
        .collect()
}

fn seal_page(view: &SevenSignsView) -> SsqSeals {
    let mut rows = Vec::new();
    for seal in 1..=3usize {
        let dawn_proportion = view.dawn_proportion[seal];
        let dusk_proportion = view.dusk_proportion[seal];
        // Java: a cabal without members shows 0%, the other one is a share of its members.
        let dusk_percent = if view.total_dusk_members == 0 {
            0
        } else {
            java_round((dusk_proportion as f32 / view.total_dusk_members as f32) * 100.0)
        };
        let dawn_percent = if view.total_dawn_members == 0 {
            0
        } else {
            java_round((dawn_proportion as f32 / view.total_dawn_members as f32) * 100.0)
        };
        rows.push(SsqSealRow {
            seal: seal as u8,
            owner: view.seal_owner[seal] as u8,
            dusk_percent: dusk_percent as u8,
            dawn_percent: dawn_percent as u8,
        });
    }
    SsqSeals {
        retain_limit: 10,
        claim_limit: 35,
        rows,
    }
}

fn predicted_page(view: &SevenSignsView) -> SsqPredicted {
    let mut seals = Vec::new();
    for seal in 1..=3usize {
        let dawn_proportion = view.dawn_proportion[seal];
        let dusk_proportion = view.dusk_proportion[seal];
        let dawn_denominator = if view.total_dawn_members == 0 {
            1.0
        } else {
            view.total_dawn_members as f32
        };
        let dusk_denominator = if view.total_dusk_members == 0 {
            1.0
        } else {
            view.total_dusk_members as f32
        };
        let dawn_percent = java_round((dawn_proportion as f32 / dawn_denominator) * 100.0);
        let dusk_percent = java_round((dusk_proportion as f32 / dusk_denominator) * 100.0);
        let owner = view.seal_owner[seal];
        let (shown_owner, message) = seal_outcome(owner, view.winning_cabal, dawn_percent, dusk_percent);
        seals.push(SsqSealOutcome {
            seal: seal as u8,
            owner: shown_owner,
            message,
        });
    }
    SsqPredicted {
        winner: view.winning_cabal as u8,
        seals,
    }
}

fn write_score(w: &mut PacketWriter, score: &SsqScore) {
    w.i64(score.stone_score)
        .i64(score.festival_score)
        .i64(score.total_score)
        .u8(score.percent);
}

fn read_score(r: &mut PacketReader) -> Result<SsqScore, PacketError> {
    Ok(SsqScore {
        stone_score: r.i64()?,
        festival_score: r.i64()?,
        total_score: r.i64()?,
        percent: r.u8()?,
    })
}

fn write_members(w: &mut PacketWriter, members: &[String]) {
    w.u8(members.len() as u8);
    for member in members {
        w.string(member);
    }
}

fn read_members(r: &mut PacketReader) -> Result<Vec<String>, PacketError> {
    let count = r.u8()?;
    Ok((0..count).map(|_| r.string()).collect())
}

impl Encode for SsqStatus {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.u8(self.page as u8).u8(self.period);
        match &self.body {
            SsqStatusBody::None => {}
            SsqStatusBody::Cycle(cycle) => {
                w.i32(cycle.cycle)
                    .i32(cycle.period_message)
                    .i32(cycle.deadline_message)
                    .u8(cycle.player_cabal)
                    .u8(cycle.player_seal)
                    .i64(cycle.stone_contrib)
                    .i64(cycle.adena_collect);
                write_score(w, &cycle.dusk);
                write_score(w, &cycle.dawn);
            }
            SsqStatusBody::Festivals(festivals) => {
                // Java: `writeShort(1)`, then the festival count as a byte.
                w.i16(1).u8(festivals.len() as u8);
                for (index, festival) in festivals.iter().enumerate() {
                    w.u8((index + 1) as u8)
                        .i32(festival.level_score)
                        .i64(festival.dusk_score);
                    write_members(w, &festival.dusk_members);
                    w.i64(festival.dawn_score);
                    write_members(w, &festival.dawn_members);
                }
            }
            SsqStatusBody::Seals(seals) => {
                w.u8(seals.retain_limit)
                    .u8(seals.claim_limit)
                    .u8(seals.rows.len() as u8);
                for row in &seals.rows {
                    w.u8(row.seal)
                        .u8(row.owner)
                        .u8(row.dusk_percent)
                        .u8(row.dawn_percent);
                }
            }
            SsqStatusBody::Predicted(predicted) => {
                w.u8(predicted.winner).u8(predicted.seals.len() as u8);
                for seal in &predicted.seals {
                    w.u8(seal.seal).u8(seal.owner).i32(seal.message);
                }
            }
        }
    }
}

impl Decode for SsqStatus {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let page = r.i8()?;
        let period = r.u8()?;
        let body = match page {
            1 => SsqStatusBody::Cycle(SsqCycle {
                cycle: r.i32()?,
                period_message: r.i32()?,
                deadline_message: r.i32()?,
                player_cabal: r.u8()?,
                player_seal: r.u8()?,
                stone_contrib: r.i64()?,
                adena_collect: r.i64()?,
                dusk: read_score(r)?,
                dawn: read_score(r)?,
            }),
            2 => {
                let _short_one = r.i16()?;
                let count = r.u8()?;
                let mut festivals = Vec::new();
                for _ in 0..count {
                    let _festival_id = r.u8()?;
                    festivals.push(SsqFestival {
                        level_score: r.i32()?,
                        dusk_score: r.i64()?,
                        dusk_members: read_members(r)?,
                        dawn_score: r.i64()?,
                        dawn_members: read_members(r)?,
                    });
                }
                SsqStatusBody::Festivals(festivals)
            }
            3 => {
                let retain_limit = r.u8()?;
                let claim_limit = r.u8()?;
                let count = r.u8()?;
                let mut rows = Vec::new();
                for _ in 0..count {
                    rows.push(SsqSealRow {
                        seal: r.u8()?,
                        owner: r.u8()?,
                        dusk_percent: r.u8()?,
                        dawn_percent: r.u8()?,
                    });
                }
                SsqStatusBody::Seals(SsqSeals {
                    retain_limit,
                    claim_limit,
                    rows,
                })
            }
            4 => {
                let winner = r.u8()?;
                let count = r.u8()?;
                let mut seals = Vec::new();
                for _ in 0..count {
                    seals.push(SsqSealOutcome {
                        seal: r.u8()?,
                        owner: r.u8()?,
                        message: r.i32()?,
                    });
                }
                SsqStatusBody::Predicted(SsqPredicted { winner, seals })
            }
            _ => SsqStatusBody::None,
        };
        Ok(Self { page, period, body })
    }
}

/// One entrance of the Seed map: the position and the state of the seed (Java
/// `ENTRANCES[i]` with the state it writes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedMapEntry {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Java: `2770 + SoDState` for the Seed of Destruction, `stage + 2765` for the Seed of
    /// Infinity.
    pub state: i32,
}

// Java: gameserver/network/serverpackets/ExShowSeedMapInfo.java
/// The two Seed entrances on the map (`EX_SHOW_SEED_MAP_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowSeedMapInfo {
    pub entries: Vec<SeedMapEntry>,
}

impl ExShowSeedMapInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowSeedMapInfo;

    /// Java's static packet: the Seed of Destruction entrance first, then the Seed of
    /// Infinity one, with their current states.
    pub fn new(sod_state: i32, soi_stage: i32) -> Self {
        Self {
            entries: vec![
                SeedMapEntry {
                    x: -246_857,
                    y: 251_960,
                    z: 4331,
                    state: 2770 + sod_state,
                },
                SeedMapEntry {
                    x: -213_770,
                    y: 210_760,
                    z: 4400,
                    state: soi_stage + 2765,
                },
            ],
        }
    }
}

impl Encode for ExShowSeedMapInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.entries.len() as i32);
        for entry in &self.entries {
            w.i32(entry.x).i32(entry.y).i32(entry.z).i32(entry.state);
        }
    }
}

impl Decode for ExShowSeedMapInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = super::read_count(r, "seed entrance count")?;
        let mut entries = Vec::new();
        for _ in 0..count {
            entries.push(SeedMapEntry {
                x: r.i32()?,
                y: r.i32()?,
                z: r.i32()?,
                state: r.i32()?,
            });
        }
        Ok(Self { entries })
    }
}
