//! The weekly rent of an auctionable hall.
// Java: entity/residences/AuctionableHall.java (CH_RATE, initialyzeTask, FeeTask)
//
// A hall pays `lease` adena from the clan warehouse every `CH_RATE` (one week). The task runs when
// `paidUntil` is reached; a clan that cannot pay gets a message every day, and the hall is taken
// away (and put up for auction again) when the rent is a week late.
//
// # One task per hall
// Java starts a new `FeeTask` object at every owner change; the old ones find `paidUntil` in the
// future and only schedule themselves again, so they are harmless copies. Here a hall has one
// pending task, a new one replaces the old.
//
// # Side by side (D-51)
// The rent is a job both game servers run. Without `SiegeDatabaseWrites` this world only moves the
// paid time on, as when the rent was paid: nothing is taken from the clan warehouse, no row is
// written and the hall is never taken away (the Java game server does that; the new owner is read
// again at `CharacterSelect`).

use tracing::error;

use super::ClanHallTimer;
use crate::residence::{ClanHallId, ClanHallKind, Writes};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};

/// `AuctionableHall.CH_RATE`: one week in milliseconds.
const CH_RATE: i64 = 604_800_000;
/// `3600000 * 24`.
const DAY: i64 = 86_400_000;

impl World {
    /// Java `initialyzeTask(forced)`: when the first run of the rent task is due.
    pub(crate) fn clan_hall_initialize_rent_task(&mut self, id: ClanHallId, forced: bool) {
        let Some(ClanHallKind::Auctionable { paid_until, paid, .. }) = self.clan_hall(id).map(|h| &h.kind)
        else {
            return;
        };
        let (paid_until, paid) = (*paid_until, *paid);
        let now = unix_ms_now();
        let delay = if paid_until > now {
            paid_until - now
        } else if !paid && !forced {
            if now + DAY <= paid_until + CH_RATE {
                // NOTE(java-quirk): `schedule(task, System.currentTimeMillis() + 24 h)`: the delay
                // is the absolute time plus a day, so the task runs in about 54 years.
                now + DAY
            } else {
                (paid_until + CH_RATE) - now
            }
        } else {
            0
        };
        self.schedule_rent_task(id, delay);
    }

    /// `ThreadPool.schedule(new FeeTask(), delay)`.
    fn schedule_rent_task(&mut self, id: ClanHallId, delay_ms: i64) {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        let timer = self
            .timers
            .schedule_at(due, TimerKind::ClanHall(ClanHallTimer::Rent(id)));
        if let Some(old) = self.clanhall.rent_timers.insert(id, timer) {
            self.timers.cancel(old);
        }
    }

    /// A hall that was freed or lost its owner: its rent task is not needed any more.
    pub(crate) fn clan_hall_cancel_rent_task(&mut self, id: ClanHallId) {
        if let Some(timer) = self.clanhall.rent_timers.remove(&id) {
            self.timers.cancel(timer);
        }
    }

    /// A hall that came to an owner by a re-read gets the task `new AuctionableHall(set)` starts.
    pub(crate) fn clan_hall_rent_after_reread(&mut self, id: ClanHallId) {
        let owned = self.clan_hall(id).is_some_and(|h| !h.is_free && h.owner_id > 0);
        if !owned {
            self.clan_hall_cancel_rent_task(id);
        } else if !self.clanhall.rent_timers.contains_key(&id) {
            self.clan_hall_initialize_rent_task(id, false);
        }
    }

    /// Sets `paidUntil` and `paid` of an auctionable hall.
    fn set_rent_paid(&mut self, id: ClanHallId, new_paid_until: i64, new_paid: bool) {
        if let Some(ClanHallKind::Auctionable { paid_until, paid, .. }) =
            self.clan_hall_mut(id).map(|h| &mut h.kind)
        {
            *paid_until = new_paid_until;
            *paid = new_paid;
        }
    }

    /// Java `FeeTask.run()`.
    pub(crate) fn on_rent_due(&mut self, id: ClanHallId) {
        self.clanhall.rent_timers.remove(&id);
        let Some(hall) = self.clan_hall(id) else {
            return;
        };
        // `if (isFree()) return;`
        if hall.is_free {
            return;
        }
        let ClanHallKind::Auctionable {
            lease,
            mut paid_until,
            ..
        } = hall.kind
        else {
            return;
        };
        let owner = hall.owner_id;
        let now = unix_ms_now();
        if paid_until > now {
            self.schedule_rent_task(id, paid_until - now);
            return;
        }
        let writes = self.siege_database_writes();
        if !writes {
            // D-51: the Java game server charges the real clan warehouse. Move the paid time on
            // as if the rent had been paid.
            if paid_until != 0 {
                while paid_until <= now {
                    paid_until += CH_RATE;
                }
            } else {
                paid_until = now + CH_RATE;
            }
            self.set_rent_paid(id, paid_until, true);
            self.schedule_rent_task(id, paid_until - now);
            return;
        }
        // `ClanTable.getClan(getOwnerId()).getWarehouse()` throws for a clan that is gone: logged,
        // the task is over.
        if self.clan(owner).is_none() {
            error!("{id}: the rent has no clan {owner} to pay it.");
            return;
        }
        if self.clan_warehouse_adena(owner) >= i64::from(lease) {
            if paid_until != 0 {
                while paid_until <= now {
                    paid_until += CH_RATE;
                }
            } else {
                paid_until = now + CH_RATE;
            }
            self.set_rent_paid(id, paid_until, true);
            self.clan_warehouse_destroy_adena(owner, i64::from(lease));
            self.schedule_rent_task(id, paid_until - now);
            self.clan_hall_update_db(id, Writes::Job);
            return;
        }
        // The clan cannot pay.
        self.set_rent_paid(id, paid_until, false);
        if now > paid_until + CH_RATE {
            if self.clanhall.loaded {
                self.clan_hall_init_npc_auction(id.0);
                self.clan_hall_table_set_free(id);
                self.broadcast_to_clan(
                    owner,
                    &SystemMessage::new(
                        SystemMessageId::TheClanHallFeeIsOneWeekOverdueThereforeTheClanHallOwnershipHasBeenRevoked,
                    ),
                );
            } else {
                self.schedule_rent_task(id, 3000);
            }
        } else {
            self.clan_hall_update_db(id, Writes::Job);
            let message = SystemMessage::new(
                SystemMessageId::PaymentForYourClanHallHasNotBeenMadePleaseMakePaymentToYourClanWarehouseByS1Tomorrow,
            )
            .with(SystemMessageParam::IntNumber(lease));
            self.broadcast_to_clan(owner, &message);
            if now + DAY <= paid_until + CH_RATE {
                // NOTE(java-quirk): the absolute time again (see `clan_hall_initialize_rent_task`).
                self.schedule_rent_task(id, now + DAY);
            } else {
                self.schedule_rent_task(id, (paid_until + CH_RATE) - now);
            }
        }
    }
}
