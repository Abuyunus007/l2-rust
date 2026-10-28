//! The Lucky Lottery: the weekly draw, the `lottery` table and the ticket checks (S-HAND-03).
// Java: managers/games/LotteryManager.java
// Java: dist/game/data/scripts/handlers/bypass/npc/Loto.java (see `loto.rs`)
//
// # Where it lives
// One [`LotteryWorld`] in the world (D-10). Java's manager is a singleton with three
// `ThreadPool` tasks; here they are timers on the wheel ([`LotteryTimer`]). The newest row of
// `lottery` is read before the world starts ([`World::load_lottery`], D-15), like the manager's
// constructor does. (The table is `lottery`; the card calls it `games`.)
//
// # Rows (D-39, D-53)
// `lottery` is a global cycle shared with the Java GS. The statements are Java's
// ([`LotteryWrite`] through `PersistCommand::Lottery`). While the Java GS shares the DB the Rust
// GS runs no draw of its own: a lottery runs only with `AllowLottery` **and**
// `CycleDatabaseWrites = True` ([`LotterySettings::runs`]). Without it no ticket is sold here
// ("not currently being sold"), the pages that only show data work, and the winning numbers and
// prizes are read again whenever a ticket list is shown or a ticket is claimed
// (`PersistCommand::LotteryRequest`, D-50).
//
// # The draw
// `finishLottery` counts the tickets in the `items` table, which only the database knows
// (offline players' items): the world draws the numbers, asks for the tickets
// (`LotteryThen::Draw`) and finishes when the answer comes back ([`World::on_lottery_reread`]).
//
// # Left out on purpose
// - NOTE(java-quirk): the next draw's day is found with `Calendar.set(DAY_OF_WEEK, SUNDAY)`,
//   whose week starts on Sunday or Monday by locale. The Monday-first week (the coming Sunday)
//   is used; a Sunday-first week would give the Sunday before, i.e. a draw that finishes at once.

mod loto;

pub(crate) use loto::loto_handler;

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, CreatureSay, SayBody, SenderField, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::outbox::PersistCommand;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

const MINUTE: i64 = 60_000;
const DAY: i64 = 86_400_000;
const WEEK: i64 = 7 * DAY;
/// The lottery ticket item.
pub const LOTTERY_TICKET: i32 = 4442;

/// The values the lottery reads (`General.ini`, and `SideBySide.ini` for the switch).
#[derive(Debug, Clone, PartialEq)]
pub struct LotterySettings {
    /// `ALLOW_LOTTERY` (`AllowLottery`).
    pub allow_lottery: bool,
    /// `ALT_LOTTERY_PRIZE` (`AltLotteryPrize`).
    pub prize: i64,
    /// `ALT_LOTTERY_TICKET_PRICE` (`AltLotteryTicketPrice`).
    pub ticket_price: i64,
    /// `ALT_LOTTERY_5_NUMBER_RATE`, `_4_`, `_3_` (a Java `float`).
    pub rate_5: f32,
    pub rate_4: f32,
    pub rate_3: f32,
    /// `ALT_LOTTERY_2_AND_1_NUMBER_PRIZE` (`AltLottery2and1NumberPrize`).
    pub two_and_one_prize: i64,
    /// `CycleDatabaseWrites` of `config/SideBySide.ini` (D-53), see the module notes.
    pub cycle_database_writes: bool,
}

impl Default for LotterySettings {
    fn default() -> Self {
        Self {
            allow_lottery: true,
            prize: 50_000,
            ticket_price: 2000,
            rate_5: 0.6,
            rate_4: 0.2,
            rate_3: 0.2,
            two_and_one_prize: 200,
            cycle_database_writes: false,
        }
    }
}

impl LotterySettings {
    /// Whether this server runs the draws (starts, sells and finishes lotteries).
    pub fn runs(&self) -> bool {
        self.allow_lottery && self.cycle_database_writes
    }
}

/// The newest `lottery` row (`SELECT_LAST_LOTTERY`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotteryRow {
    pub idnr: i32,
    pub prize: i64,
    pub newprize: i64,
    /// Unix milliseconds.
    pub enddate: i64,
    pub finished: bool,
}

/// The winning numbers and prizes of a finished draw (`SELECT_LOTTERY_TICKET`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LotteryResult {
    pub idnr: i32,
    pub number1: i32,
    pub number2: i32,
    pub prize1: i64,
    pub prize2: i64,
    pub prize3: i64,
}

/// A `lottery` write with Java's statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LotteryWrite {
    /// `INSERT_LOTTERY` (`id` is 1).
    Insert {
        idnr: i32,
        enddate: i64,
        prize: i64,
        newprize: i64,
    },
    /// `UPDATE_PRICE`.
    UpdatePrice { idnr: i32, prize: i64 },
    /// `UPDATE_LOTTERY`.
    Finish {
        idnr: i32,
        prize: i64,
        newprize: i64,
        number1: i32,
        number2: i32,
        prize1: i64,
        prize2: i64,
        prize3: i64,
    },
}

/// What to do when a read the world asked for is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LotteryThen {
    /// `finishLottery`: the tickets (`enchant_level`, `custom_type2`) of draw `number`.
    Draw { number: i32 },
    /// Page 24 of `Loto`: the list of the tickets of earlier draws.
    Page { player: ObjectId, npc: ObjectId },
    /// A ticket of earlier draws is claimed (`Loto` with an item object id).
    Claim {
        player: ObjectId,
        npc: ObjectId,
        object_id: i32,
    },
}

/// The answer to a [`LotteryThen`] read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LotteryReply {
    /// `(enchant_level, custom_type2)` of every ticket of a draw.
    Tickets(Vec<(i32, i32)>),
    /// Every draw's numbers and prizes.
    Results(Vec<LotteryResult>),
}

/// The lottery's timers (one `TimerKind::Lottery`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LotteryTimer {
    /// Java `startLottery`.
    Start,
    /// Java `stopSellingTickets`.
    StopSelling,
    /// Java `finishLottery`.
    Finish,
}

/// A draw waiting for its tickets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingDraw {
    enchant: i32,
    type2: i32,
}

/// The world's lottery state (Java `LotteryManager`'s fields and `Player._loto`).
#[derive(Debug, Default)]
pub struct LotteryWorld {
    pub(crate) settings: LotterySettings,
    /// `_number`.
    number: i32,
    /// `_prize`.
    prize: i64,
    /// `_isSellingTickets`.
    selling: bool,
    /// `_isStarted`.
    started: bool,
    /// `_enddate`.
    enddate: i64,
    /// The row `startLottery` reads: the newest one, kept up to date by the draws of this server.
    last: Option<LotteryRow>,
    /// The numbers each player picked (`Player.getLoto(i)`).
    pub(crate) picks: HashMap<ObjectId, [i32; 5]>,
    pending: Option<PendingDraw>,
    /// Tests: the clock the manager reads (`System.currentTimeMillis()`).
    clock: Option<i64>,
}

/// The matches between a ticket and the drawn numbers, counted like `finishLottery`'s loop.
///
/// `curenchant` and `curtype2` are the ticket's bit masks already `&`ed with the drawn ones.
fn count_matches(mut curenchant: i32, mut curtype2: i32) -> i32 {
    let mut count = 0;
    for _ in 1..=16 {
        // `val != Math.round((double) curenchant / 2)`: true for an odd number.
        if curenchant % 2 != 0 {
            count += 1;
        }
        if curtype2 % 2 != 0 {
            count += 1;
        }
        curenchant /= 2;
        curtype2 /= 2;
    }
    count
}

/// `MathUtil.pow(2, n)` for the numbers 1 to 20 split in `enchant` (1 to 16) and `type2`
/// (17 to 20).
fn encode_numbers(numbers: &[i32]) -> (i32, i32) {
    let mut enchant = 0;
    let mut type2 = 0;
    for n in numbers {
        if *n < 17 {
            enchant += 1 << (n - 1);
        } else {
            type2 += 1 << (n - 17);
        }
    }
    (enchant, type2)
}

/// Java `LotteryManager.decodeNumbers`.
pub fn decode_numbers(enchant_value: i32, type2_value: i32) -> [i32; 5] {
    let mut res = [0; 5];
    let mut id = 0;
    let mut nr = 1;
    let mut enchant = enchant_value;
    while enchant > 0 {
        if enchant % 2 != 0 {
            // NOTE(java-quirk): Java writes `res[id]` without a bound; a ticket with more than
            // five numbers throws there. The extra numbers are dropped here.
            if let Some(slot) = res.get_mut(id) {
                *slot = nr;
            }
            id += 1;
        }
        enchant /= 2;
        nr += 1;
    }
    nr = 17;
    let mut type2 = type2_value;
    while type2 > 0 {
        if type2 % 2 != 0 {
            if let Some(slot) = res.get_mut(id) {
                *slot = nr;
            }
            id += 1;
        }
        type2 /= 2;
        nr += 1;
    }
    res
}

/// Java `LotteryManager.checkTicket(id, enchant, type2)`: `[rank, adena]`, `[0, 0]` for no prize
/// or a draw the rows do not have.
pub fn check_ticket(
    rows: &[LotteryResult],
    id: i32,
    enchant: i32,
    type2: i32,
    two_and_one_prize: i64,
) -> [i64; 2] {
    let Some(row) = rows.iter().find(|r| r.idnr == id) else {
        return [0, 0];
    };
    let curenchant = row.number1 & enchant;
    let curtype2 = row.number2 & type2;
    if curenchant == 0 && curtype2 == 0 {
        return [0, 0];
    }
    match count_matches(curenchant, curtype2) {
        0 => [0, 0],
        5 => [1, row.prize1],
        4 => [2, row.prize2],
        3 => [3, row.prize3],
        _ => [4, two_and_one_prize],
    }
}

/// The end of the next draw: the first Sunday 19:00 after `end` as `startLottery` computes it
/// (minutes and seconds zeroed, the milliseconds kept; a Sunday gives next week's Sunday).
/// `offset_at(utc_ms)` is the zone's offset (D-48).
pub fn next_draw_end(end: i64, offset_at: impl Fn(i64) -> i64) -> i64 {
    let local = end.wrapping_add(offset_at(end));
    let ms = local.rem_euclid(1000);
    let day = local.div_euclid(DAY);
    // 1970-01-01 was a Thursday; 0 is Sunday.
    let weekday = (day + 4).rem_euclid(7);
    let sunday = weekday == 0;
    let ahead = if sunday { 0 } else { 7 - weekday };
    let target_local = (day + ahead) * DAY + 19 * 3_600_000 + ms;
    let guess = target_local.wrapping_sub(offset_at(target_local));
    let utc = target_local.wrapping_sub(offset_at(guess));
    if sunday { utc + WEEK } else { utc }
}

impl World {
    /// The lottery settings (`General.ini`, `SideBySide.ini`).
    pub fn set_lottery_settings(&mut self, settings: LotterySettings) {
        self.lottery.settings = settings;
    }

    /// Tests: the clock the lottery reads instead of the machine's.
    pub fn set_lottery_clock(&mut self, unix_ms: Option<i64>) {
        self.lottery.clock = unix_ms;
    }

    fn lottery_now(&self) -> i64 {
        self.lottery.clock.unwrap_or_else(unix_ms_now)
    }

    /// Java `LotteryManager.getId()`.
    pub fn lottery_id(&self) -> i32 {
        self.lottery.number
    }

    /// Java `LotteryManager.getPrize()`.
    pub fn lottery_prize(&self) -> i64 {
        self.lottery.prize
    }

    /// Java `LotteryManager.getEndDate()`.
    pub fn lottery_end_date(&self) -> i64 {
        self.lottery.enddate
    }

    /// Java `LotteryManager.isSellableTickets()`.
    pub fn lottery_is_sellable(&self) -> bool {
        self.lottery.selling
    }

    /// Java `LotteryManager.isStarted()`.
    pub fn lottery_is_started(&self) -> bool {
        self.lottery.started
    }

    fn lottery_schedule(&mut self, delay_ms: i64, timer: LotteryTimer) {
        let ticks = TimerWheel::ms_to_ticks(u64::try_from(delay_ms.max(0)).unwrap_or(0));
        self.timers
            .schedule_at(self.tick + ticks, TimerKind::Lottery(timer));
    }

    /// The lottery runs only with `CycleDatabaseWrites` (D-53), so every write goes out.
    fn lottery_write(&mut self, write: LotteryWrite) {
        self.outbox.persist(PersistCommand::Lottery(write));
    }

    /// `World.broadcastToAllOnlinePlayers(String)`.
    fn lottery_announce(&self, text: String) {
        self.broadcast_to_all_online_players(&CreatureSay {
            sender_object_id: 0,
            chat_type: ChatType::Announcement,
            sender: SenderField::Name(String::new()),
            message_id: -1,
            body: SayBody::Text(text),
        });
    }

    /// Java `new LotteryManager()` (GameServer): the initial state, and `startLottery.run()` with
    /// `AllowLottery`. `last` is the newest `lottery` row, read before the world started.
    pub fn load_lottery(&mut self, last: Option<LotteryRow>) {
        self.lottery.number = 1;
        self.lottery.prize = self.lottery.settings.prize;
        self.lottery.selling = false;
        self.lottery.started = false;
        self.lottery.enddate = self.lottery_now();
        self.lottery.last = last;
        if self.lottery.settings.runs() {
            self.lottery_start();
        } else if let Some(row) = self.lottery.last.clone() {
            // A mirror of the Java GS's lottery (D-53): the number and the prize it shows.
            self.lottery.number = row.idnr;
            if row.finished {
                self.lottery.number += 1;
                self.lottery.prize = row.newprize;
            } else {
                self.lottery.prize = row.prize;
                self.lottery.enddate = row.enddate;
            }
        }
    }

    /// Java `startLottery.run()`.
    fn lottery_start(&mut self) {
        let now = self.lottery_now();
        if let Some(row) = self.lottery.last.clone() {
            self.lottery.number = row.idnr;
            if row.finished {
                self.lottery.number += 1;
                self.lottery.prize = row.newprize;
            } else {
                self.lottery.prize = row.prize;
                self.lottery.enddate = row.enddate;
                if self.lottery.enddate <= now + 2 * MINUTE {
                    self.lottery_finish();
                    return;
                }
                if self.lottery.enddate > now {
                    self.lottery.started = true;
                    self.lottery_schedule(self.lottery.enddate - now, LotteryTimer::Finish);
                    if self.lottery.enddate > now + 12 * MINUTE {
                        self.lottery.selling = true;
                        self.lottery_schedule(
                            self.lottery.enddate - now - 10 * MINUTE,
                            LotteryTimer::StopSelling,
                        );
                    }
                    return;
                }
            }
        }

        self.lottery.selling = true;
        self.lottery.started = true;
        self.lottery_announce(format!(
            "Lottery tickets are now available for Lucky Lottery #{}.",
            self.lottery.number
        ));
        self.lottery.enddate = next_draw_end(self.lottery.enddate, l2_util::time::local_offset_millis);
        let end_in = self.lottery.enddate - now;
        self.lottery_schedule((end_in - 10 * MINUTE).max(0), LotteryTimer::StopSelling);
        self.lottery_schedule(end_in.max(0), LotteryTimer::Finish);
        let write = LotteryWrite::Insert {
            idnr: self.lottery.number,
            enddate: self.lottery.enddate,
            prize: self.lottery.prize,
            newprize: self.lottery.prize,
        };
        self.lottery_write(write);
    }

    /// The lottery's timers.
    pub(crate) fn on_lottery_timer(&mut self, timer: LotteryTimer) {
        match timer {
            LotteryTimer::Start => self.lottery_start(),
            LotteryTimer::StopSelling => {
                self.lottery.selling = false;
                self.broadcast_to_all_online_players(&SystemMessage::new(
                    SystemMessageId::LotteryTicketSalesHaveBeenTemporarilySuspended,
                ));
            }
            LotteryTimer::Finish => self.lottery_finish(),
        }
    }

    /// Java `finishLottery.run()`, first half: the five numbers, then the tickets are asked for.
    fn lottery_finish(&mut self) {
        let mut luckynums = [0_i32; 5];
        for i in 0..5 {
            loop {
                let luckynum = self.rng().below(20) + 1;
                if !luckynums[..i].contains(&luckynum) {
                    luckynums[i] = luckynum;
                    break;
                }
            }
        }
        let (enchant, type2) = encode_numbers(&luckynums);
        self.lottery.pending = Some(PendingDraw { enchant, type2 });
        self.outbox.persist(PersistCommand::LotteryRequest {
            then: LotteryThen::Draw {
                number: self.lottery.number,
            },
        });
    }

    /// Tests: the numbers the draw in flight has (`enchant`, `type2`).
    pub fn lottery_pending_numbers(&self) -> Option<(i32, i32)> {
        self.lottery.pending.map(|p| (p.enchant, p.type2))
    }

    /// The answer to a `PersistCommand::LotteryRequest`.
    pub(crate) fn on_lottery_reread(&mut self, result: Result<LotteryReply, ()>, then: LotteryThen) {
        match then {
            LotteryThen::Draw { .. } => {
                let tickets = match result {
                    Ok(LotteryReply::Tickets(tickets)) => tickets,
                    _ => {
                        // Java logs "Could restore lottery data" and counts nothing.
                        warn!("Lottery: Could restore lottery data");
                        Vec::new()
                    }
                };
                self.lottery_finish_with(&tickets);
            }
            LotteryThen::Page { player, npc } => {
                let Ok(LotteryReply::Results(rows)) = result else {
                    warn!("Lottery: the results could not be read; the request of {player} is refused.");
                    return;
                };
                if self.players.contains_key(&player) {
                    self.loto_window_with(player, npc, 24, Some(&rows));
                }
            }
            LotteryThen::Claim {
                player,
                npc,
                object_id,
            } => {
                let Ok(LotteryReply::Results(rows)) = result else {
                    warn!("Lottery: the results could not be read; the request of {player} is refused.");
                    return;
                };
                if self.players.contains_key(&player) {
                    self.loto_window_with(player, npc, object_id, Some(&rows));
                }
            }
        }
    }

    /// Java `finishLottery.run()`, second half: the counts, the prizes, the broadcast, the row.
    fn lottery_finish_with(&mut self, tickets: &[(i32, i32)]) {
        let Some(PendingDraw { enchant, type2 }) = self.lottery.pending.take() else {
            return;
        };
        let (mut count1, mut count2, mut count3, mut count4) = (0_i64, 0_i64, 0_i64, 0_i64);
        for (ticket_enchant, ticket_type2) in tickets {
            let curenchant = ticket_enchant & enchant;
            let curtype2 = ticket_type2 & type2;
            if curenchant == 0 && curtype2 == 0 {
                continue;
            }
            match count_matches(curenchant, curtype2) {
                5 => count1 += 1,
                4 => count2 += 1,
                3 => count3 += 1,
                c if c > 0 => count4 += 1,
                _ => {}
            }
        }

        let settings = self.lottery.settings.clone();
        let prize = self.lottery.prize;
        let prize4 = count4 * settings.two_and_one_prize;
        // `(long) (((_prize - prize4) * RATE) / count)`: a `long` times a `float` is a float.
        let share = |rate: f32, count: i64| -> i64 {
            if count > 0 {
                (((prize - prize4) as f32 * rate) / count as f32) as i64
            } else {
                0
            }
        };
        let prize1 = share(settings.rate_5, count1);
        let prize2 = share(settings.rate_4, count2);
        let prize3 = share(settings.rate_3, count3);
        let newprize = prize - (prize1 + prize2 + prize3 + prize4);

        let number = self.lottery.number;
        let message = if count1 > 0 {
            // There are winners.
            SystemMessage::new(
                SystemMessageId::ThePrizeAmountForTheWinnerOfLotteryS1IsS2AdenaWeHaveS3FirstPrizeWinners,
            )
            .with(SystemMessageParam::IntNumber(number))
            .with(SystemMessageParam::LongNumber(prize))
            .with(SystemMessageParam::LongNumber(count1))
        } else {
            // There are no winners.
            SystemMessage::new(
                SystemMessageId::ThePrizeAmountForLuckyLotteryS1IsS2AdenaThereWasNoFirstPrizeWinnerInThisDrawingThereforeTheJackpotWillBeAddedToTheNextDrawing,
            )
            .with(SystemMessageParam::IntNumber(number))
            .with(SystemMessageParam::LongNumber(prize))
        };
        self.broadcast_to_all_online_players(&message);

        self.lottery_write(LotteryWrite::Finish {
            idnr: number,
            prize,
            newprize,
            number1: enchant,
            number2: type2,
            prize1,
            prize2,
            prize3,
        });
        info!("Lottery #{number} finished: {count1}/{count2}/{count3}/{count4} tickets.");

        // `startLottery` reads this row back after a minute.
        self.lottery.last = Some(LotteryRow {
            idnr: number,
            prize,
            newprize,
            enddate: self.lottery.enddate,
            finished: true,
        });
        self.lottery_schedule(MINUTE, LotteryTimer::Start);
        self.lottery.number += 1;
        self.lottery.started = false;
    }

    /// Java `LotteryManager.increasePrize(count)`.
    pub(crate) fn lottery_increase_prize(&mut self, count: i64) {
        self.lottery.prize += count;
        let write = LotteryWrite::UpdatePrice {
            idnr: self.lottery.number,
            prize: self.lottery.prize,
        };
        self.lottery_write(write);
    }

    /// `Player.getLoto(i)` / `setLoto(i, value)`: the five picks of a player.
    pub(crate) fn lottery_picks(&mut self, player: ObjectId) -> &mut [i32; 5] {
        self.lottery.picks.entry(player).or_insert([0; 5])
    }

    /// A player left: its picks go with the `Player` object.
    pub(crate) fn lottery_on_player_leave(&mut self, player: ObjectId) {
        self.lottery.picks.remove(&player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(_: i64) -> i64 {
        0
    }

    /// 2026-10-27 (a Tuesday) 12:34:56.789 UTC.
    const TUESDAY: i64 = 1_793_104_496_789;

    #[test]
    fn the_numbers_are_encoded_and_decoded() {
        let (enchant, type2) = encode_numbers(&[1, 2, 16, 17, 20]);
        assert_eq!(enchant, 1 + 2 + (1 << 15));
        assert_eq!(type2, 1 + (1 << 3));
        assert_eq!(decode_numbers(enchant, type2), [1, 2, 16, 17, 20]);
    }

    #[test]
    fn matches_are_counted_over_both_masks() {
        assert_eq!(count_matches(0b1011, 0b1), 4);
        assert_eq!(count_matches(0, 0), 0);
        assert_eq!(count_matches(0xFFFF, 0), 16);
    }

    #[test]
    fn the_next_draw_is_a_sunday_at_19() {
        // Tuesday -> the coming Sunday (2026-11-01) 19:00:00.789.
        let sunday = next_draw_end(TUESDAY, utc);
        let midnight = TUESDAY - (12 * 3600 + 34 * 60 + 56) * 1000 - 789;
        assert_eq!(sunday, midnight + 5 * DAY + 19 * 3_600_000 + 789);
        // A Sunday gives the Sunday a week later (Java adds a week).
        assert_eq!(next_draw_end(sunday, utc), sunday + WEEK);
    }

    #[test]
    fn a_ticket_is_checked_against_the_row() {
        let (n1, n2) = encode_numbers(&[1, 2, 3, 4, 5]);
        let row = LotteryResult {
            idnr: 7,
            number1: n1,
            number2: n2,
            prize1: 1000,
            prize2: 500,
            prize3: 100,
        };
        let check = |numbers: &[i32], id: i32| {
            let (t1, t2) = encode_numbers(numbers);
            check_ticket(&[row], id, t1, t2, 200)
        };
        assert_eq!(check(&[1, 2, 3, 4, 5], 7), [1, 1000]);
        assert_eq!(check(&[1, 2, 3, 4, 20], 7), [2, 500]);
        assert_eq!(check(&[1, 2, 3, 19, 20], 7), [3, 100]);
        assert_eq!(check(&[1, 18, 19, 20, 17], 7), [4, 200]);
        assert_eq!(check(&[10, 11, 12, 13, 14], 7), [0, 0]);
        assert_eq!(check(&[1, 2, 3, 4, 5], 8), [0, 0]);
    }
}
