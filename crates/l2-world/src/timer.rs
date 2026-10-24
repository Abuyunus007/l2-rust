//! The timer wheel: what Java does with `ThreadPool.schedule` and the task managers (D-12).
//!
//! Timers are due at a world tick. Firing order is by tick, then by scheduling order. A
//! cancelled timer never fires. Later cards add [`TimerKind`] variants (autosave, regen…).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::time::Duration;

use l2_util::ObjectId;

use crate::world::TICK;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimerId(u64);

/// What to do when a timer fires. The world matches on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerKind {
    /// Store an online player (Java `PlayerAutoSaveTaskManager`), see M2-PERSIST-01.
    Autosave(ObjectId),
    /// For tests and tools: carries a number, does nothing.
    Marker(u32),
    /// Java `WorldRegion.startActivation`'s timer: switch the neighbours of a region on.
    RegionNeighborsOn(crate::region::RegionId),
    /// Java `WorldRegion.startDeactivation`'s timer: switch off the neighbours that hold no
    /// player.
    RegionNeighborsOff(crate::region::RegionId),
    /// Java `TeleportWatchdogTask`: completes a teleport whose client never sent `Appearing`.
    TeleportWatchdog(ObjectId),
    /// Java `Player.sendInventoryUpdate`'s 100 ms send task.
    InventoryUpdate(ObjectId),
    /// Java `CreatureFollowTaskManager`'s 500 ms follow task, for one player.
    Follow(ObjectId),
    /// Java `HitTask`: one hit of an attack lands (`Creature.onHitTimer`), see `combat/`.
    Hit(crate::combat::HitTask),
    /// Java `ThreadPool.schedule(() -> getAI().notifyActionReadyToAct(), delay)`: the attack (or
    /// a refused bow shot) is over and the AI may act again.
    ReadyToAct(ObjectId),
    /// Java `AttackStanceTaskManager`'s 1 s run: ends the combat stances older than 15 s.
    AttackStanceCheck,
    /// Java `UseItem`'s `ThreadPool.schedule(() -> player.useEquippableItem(item, false), ...)`:
    /// an item used during an attack is put on (or off) when the attack animation ends.
    EquipAfterAttack { player: ObjectId, item: ObjectId },
    /// Java `Player.broadcastCharInfo`'s 100 ms task.
    CharInfo(ObjectId),
    /// Java `Player.sendSkillList`'s 300 ms task.
    SkillList(ObjectId),
    /// Java `Player.sendItemList`'s 250 ms task.
    ItemList(ObjectId),
    /// Java `CreatureStatus`'s regeneration task (every 3 s) of one creature, see `status.rs`.
    Regeneration(ObjectId),
    /// Java `Player.broadcastStatusUpdate`'s 50 ms task: a player's own vitals, see `status.rs`.
    SelfStatusUpdate(ObjectId),
    /// Java `DecayTaskManager`: the corpse (or, with `DisconnectAfterDeath`, the dead player)
    /// is due, see `respawn.rs`.
    Decay(ObjectId),
    /// Java `RespawnTaskManager`: the NPC (by its old object id) comes back at its spawn.
    Respawn(ObjectId),
    /// Java `MinionList.MinionRespawnTask`: a dead minion (by its old object id) comes back near
    /// its master, see `ai/minions.rs`.
    MinionRespawn(ObjectId),
    /// Java `SpawnGroupTaskManager`: a spawn group materialises the unit it reserved when one of
    /// its units died (`group` and `entry` are indices into the spawn data).
    GroupSpawn { group: usize, entry: usize },
    /// Java `ResetOwner` (`ItemManager.createItem`): the loot privilege of a dropped item ends.
    ItemOwnerReset(ObjectId),
    /// Java `DropProtection.run()`: the 15 second pickup protection of a dropped item ends.
    ItemDropProtection(ObjectId),
    /// Java `ItemsAutoDestroyTaskManager.run()`, every 5 seconds while items wait.
    ItemsAutoDestroy,
    /// Java `ItemsOnGroundManager.run()`: every `SaveDroppedItemInterval` the items on the
    /// ground are written to `itemsonground`.
    SaveGroundItems,
    /// Java `RandomAnimationTaskManager`: the NPC's next random animation check, see `ai/idle.rs`.
    RandomAnimation(ObjectId),
    /// Java `VitalityTask` (every 60 s, `Player.startVitalityTask`): see `rewards/vitality.rs`.
    Vitality(ObjectId),
    /// Java `Nevit.AdventTask` (30 s): see `rewards/nevit.rs`.
    NevitAdvent(ObjectId),
    /// Java `Nevit.NevitEffectEnd`: the end of Nevit's blessing.
    NevitEffectEnd(ObjectId),
    /// Java `RecoBonusTaskEnd`: the end of the Nevit hourglass.
    NevitHourglassEnd(ObjectId),
    /// Java `RecoGiveTask`: recommendations to give out, 2 h after the login and then every hour,
    /// see `rewards/recommendations.rs`.
    RecoGive(ObjectId),
    /// Java `DailyResetManager`'s daily run at 6:30 (server local time), see
    /// `rewards/recommendations.rs`.
    DailyReset,
    /// Java `BotReportTable.ResetPointTask`: the daily reset of the report points, see
    /// `bot_report.rs`.
    BotReportReset,
    /// Java `BuffFinishTask`'s 1 s run for one creature: its timed buffs count down, see
    /// `effects/list.rs`.
    BuffFinish(ObjectId),
    /// Java `EffectTickTask`: one tick of a ticking effect (`onActionTime`), see
    /// `effects/buff_info.rs`.
    EffectTick(crate::effects::buff_info::EffectTickTask),
    /// Java `EffectList`'s 300 ms icon task: the player's `AbnormalStatusUpdate`.
    EffectIcons(ObjectId),
    /// Java `Player.updateAndBroadcastStatus`'s 50 ms task.
    UpdateAndBroadcastStatus(ObjectId),
    /// The casting tasks (`MagicUseTask`, `QueuedMagicUseTask` and the delayed steps of a cast),
    /// see `skills/cast.rs`.
    Cast(crate::skills::cast::CastTimer),
    /// Sitting, standing, charges, souls and the fear of an NPC, see `effects/control.rs`.
    Control(crate::effects::control::ControlTimer),
    /// Java `Party`'s position broadcast (`scheduleAtFixedRate`, every 12 s), see `party/`.
    PartyPosition(crate::party::PartyId),
    /// The end of a punishment, the delay of an illegal action's penalty and the jail teleports,
    /// see `punishments.rs`.
    Punishment(crate::punishments::PunishmentTimer),
    /// A servitor's life task, a summon's avoid task and its abnormal effect update, see
    /// `summon/`.
    Summon(crate::summon::SummonTimer),
    /// The dissolution of a clan, the clan ranks and the end of a clan invitation, see `clan/`.
    Clan(crate::clan::ClanTimer),
    /// Java `PvpFlagTaskManager`'s 1 s pass over one flagged player, see `pvp.rs`.
    PvpFlag(ObjectId),
    /// The restock of limited products and the inventory block of a shop window, see `shop/`.
    Shop(crate::shop::ShopTimer),
    /// The three tasks of a duel (start countdown, the 1 s watch, the end), see `duel.rs`.
    Duel(crate::duel::DuelTimer),
    /// The 10 s deletion task of the expired mails, see `mail/`.
    Mail(crate::mail::MailTimer),
    /// The passes of an alt crafting (`RecipeItemMaker`), see `craft/`.
    Craft(crate::craft::CraftTimer),
    /// The bite task and the fight of a fisher, and the two tasks of the championship, see
    /// `fishing/`.
    Fishing(crate::fishing::FishingTimer),
    /// The fee tasks of the residence functions, see `residence/`.
    Residence(crate::residence::ResidenceTimer),
    /// The siege schedule, the end countdown, the headquarters' warnings, the fame of the siege
    /// zone, see `siege/`.
    Siege(crate::siege::SiegeTimer),
    /// The mode change of the castle manor and its automatic save, see `manor/`.
    Manor(crate::manor::ManorTimer),
    /// The rent of a clan hall and the end of an auction, see `clanhall/`.
    ClanHall(crate::clanhall::ClanHallTimer),
    /// The feed task of a mount, the dismount task of a no-landing zone and the rent task, see
    /// `mount.rs`.
    Mount(crate::mount::MountTimer),
    /// The fortress siege tasks, `FortUpdater` and the fame of the siege zone, see `fort/`.
    Fort(crate::fort::FortTimer),
    /// The quest timers of the castle NPCs (the mass gatekeeper, the envoys), see
    /// `residence/npc/`.
    ResidenceNpc(crate::residence::npc::ResidenceNpcTimer),
}

#[derive(Debug, Default)]
pub struct TimerWheel {
    /// (due tick, timer id) — ids grow, so equal ticks keep scheduling order.
    queue: BinaryHeap<Reverse<(u64, TimerId)>>,
    live: HashMap<TimerId, TimerKind>,
    next_id: u64,
}

impl TimerWheel {
    /// Java delays are milliseconds; the world counts ticks. Rounds up, at least one tick.
    pub fn ms_to_ticks(ms: u64) -> u64 {
        let tick_ms = TICK.as_millis() as u64; // 50
        ms.div_ceil(tick_ms).max(1)
    }

    pub fn duration_to_ticks(delay: Duration) -> u64 {
        Self::ms_to_ticks(u64::try_from(delay.as_millis()).unwrap_or(u64::MAX))
    }

    /// Schedules `kind` for tick `due`.
    pub fn schedule_at(&mut self, due: u64, kind: TimerKind) -> TimerId {
        self.next_id += 1;
        let id = TimerId(self.next_id);
        self.queue.push(Reverse((due, id)));
        self.live.insert(id, kind);
        id
    }

    /// Cancels a timer. `false` if it already fired or was cancelled.
    pub fn cancel(&mut self, id: TimerId) -> bool {
        self.live.remove(&id).is_some()
    }

    /// Removes and returns every timer due at or before `now`, in firing order.
    pub fn take_due(&mut self, now: u64) -> Vec<(TimerId, TimerKind)> {
        let mut due = Vec::new();
        while let Some(Reverse((tick, id))) = self.queue.peek().copied() {
            if tick > now {
                break;
            }
            self.queue.pop();
            if let Some(kind) = self.live.remove(&id) {
                due.push((id, kind));
            }
        }
        due
    }

    /// Timers still waiting (cancelled ones are not counted).
    pub fn len(&self) -> usize {
        self.live.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ms_round_up_to_ticks() {
        assert_eq!(TimerWheel::ms_to_ticks(0), 1);
        assert_eq!(TimerWheel::ms_to_ticks(1), 1);
        assert_eq!(TimerWheel::ms_to_ticks(50), 1);
        assert_eq!(TimerWheel::ms_to_ticks(51), 2);
        assert_eq!(TimerWheel::ms_to_ticks(60_000), 1200);
    }

    #[test]
    fn fires_in_order_and_respects_cancel() {
        let mut wheel = TimerWheel::default();
        let late = wheel.schedule_at(5, TimerKind::Marker(3));
        let a = wheel.schedule_at(2, TimerKind::Marker(1));
        let b = wheel.schedule_at(2, TimerKind::Marker(2));
        let gone = wheel.schedule_at(1, TimerKind::Marker(9));
        assert!(wheel.cancel(gone));
        assert!(!wheel.cancel(gone));
        assert!(wheel.take_due(1).is_empty());
        assert_eq!(
            wheel.take_due(4),
            vec![(a, TimerKind::Marker(1)), (b, TimerKind::Marker(2))]
        );
        assert_eq!(wheel.len(), 1);
        assert_eq!(wheel.take_due(10), vec![(late, TimerKind::Marker(3))]);
        assert!(wheel.is_empty());
    }
}
