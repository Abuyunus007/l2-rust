//! Fishing: the cast, the bite, the fight and the catch (M6-FU-08).
// Java: mechanics/fishing/{Fishing,Fish,FishingRod,FishingMonster}.java
// Java: data/xml/{FishData,FishingRodsData,FishingMonstersData}.java
// Java: entity/actor/Player.java (startFishing, stopLookingForFishTask, startLookingForFishTask,
//       getRandomFishGrade/Group/Lvl, startFishCombat, endFishing, isFishing, setLure)
// Java: entity/actor/tasks/player/LookingForFishTask.java
// Java: dist/game/data/scripts/handlers/skill/effects/{Fishing,Pumping,Reeling}.java
// Java: dist/game/data/scripts/handlers/items/FishShots.java
// Java: network/clientpackets/{UseItem (the lure branch),RequestExFishRanking}.java
//
// # Where things are
// - `cast.rs`: the `Fishing` skill effect (the checks, the place of the bait, the geodata checks),
//   `Player.startFishing` and the bite task (`LookingForFishTask`), `endFishing`.
// - `combat.rs`: `Fishing.java`, the fight with its once-a-second task, pumping and reeling.
// - `shots.rs`: the `FishShots` item handler and the lure going to the left hand.
// - `championship.rs`: `FishingChampionshipManager` (the weekly ranking) and the `Fisherman`
//   bypasses of it.
// - `skills.rs`: the `Fisherman` skill list (`FishSkillList`, M6-FU-10); the lesson itself is in
//   `skills/learn.rs`.
//
// # Model
// A player's fishing state is [`PlayerFishing`] (Java's `_fishing`, `_fish`, `_lure`, `_fishX/Y/Z`,
// `_taskForFish` and `_fishCombat`). Both Java tasks are `scheduleAtFixedRate` jobs; here they are
// `TimerKind::Fishing` entries on the wheel that put themselves back (`FixedRate`), so a cancelled
// state leaves no live timer. The data (`FishData`, `FishingRodsData`, `FishingMonstersData`) is
// immutable and handed in at startup like the recipes.
//
// # Left out on purpose
// - `Player.setTarget(player)` around the `FishShots` animation: the target change sends nothing
//   and is put back at once.
// - The `_fishing` check of the mount action (`Player.mount`), which belongs to the mounts card.

mod cast;
mod championship;
mod combat;
mod shots;
mod skills;

pub(crate) use combat::FishAction;
pub(crate) use shots::is_usable_while_fishing;

use std::sync::Arc;

use l2_data::{Fish, FishData, FishingMonstersData, FishingRodsData};
use l2_util::ObjectId;

pub(crate) use championship::Championship;
pub use championship::{ChampionshipData, ChampionshipRow};

use crate::timer::{TimerId, TimerWheel};
use crate::world::World;

/// Java `getSkillLevel(1315 /* Fishing Expertise */)`.
pub(crate) const FISHING_EXPERTISE_SKILL: i32 = 1315;
/// Java `getBuffInfoBySkillId(2274 /* Fisherman's Potion */)`.
pub(crate) const FISHERMANS_POTION_SKILL: i32 = 2274;

/// The values of `General.ini` that fishing reads. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FishingSettings {
    /// `GeneralConfig.ALLOW_FISHING` (`AllowFishing`).
    pub allow_fishing: bool,
    /// `GeneralConfig.ALT_FISH_CHAMPIONSHIP_ENABLED` (`AltFishChampionshipEnabled`).
    pub championship_enabled: bool,
    /// `GeneralConfig.ALT_FISH_CHAMPIONSHIP_REWARD_ITEM` (`AltFishChampionshipRewardItemId`).
    pub championship_reward_item: i32,
    /// `ALT_FISH_CHAMPIONSHIP_REWARD_1` to `_5` (`AltFishChampionshipReward1` ...).
    pub championship_rewards: [i32; 5],
    /// Rust-only (`SideBySide.ini`, `FishingChampionshipDatabaseWrites`, D-36, D-49): whether the
    /// weekly end of the championship and the shutdown write `fishing_championship`. False while
    /// the Java game server shares the database (its `DELETE FROM fishing_championship` and the
    /// re-inserts would clobber the other server's ranking).
    pub championship_database_writes: bool,
    /// Tests only (like `TalkSettings::clock_override_unix_ms`): pins `GameTimeTaskManager`'s
    /// `isNight()`, which the game clock would switch with the wall clock.
    pub clock_override_night: Option<bool>,
    /// Tests only (like `TalkSettings::clock_override_unix_ms`): the clock of the championship
    /// (`System.currentTimeMillis()`).
    pub clock_override_unix_ms: Option<i64>,
}

impl Default for FishingSettings {
    fn default() -> Self {
        Self {
            allow_fishing: true,
            championship_enabled: true,
            championship_reward_item: 57,
            championship_rewards: [800_000, 500_000, 300_000, 200_000, 100_000],
            championship_database_writes: false,
            clock_override_night: None,
            clock_override_unix_ms: None,
        }
    }
}

/// Java's `ScheduledThreadPoolExecutor.scheduleAtFixedRate(task, initial, period)`: run `n` is due
/// `initial + n * period` after the start, not `period` after the last run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FixedRate {
    start_tick: u64,
    initial_ms: u64,
    period_ms: u64,
    runs: u64,
}

impl FixedRate {
    pub(crate) fn new(start_tick: u64, initial_ms: u64, period_ms: u64) -> Self {
        Self {
            start_tick,
            initial_ms,
            period_ms,
            runs: 0,
        }
    }

    /// The tick of the next run, and counts it. Never earlier than the tick after `now`.
    pub(crate) fn next_due(&mut self, now: u64) -> u64 {
        let ms = self.initial_ms + self.runs * self.period_ms;
        self.runs += 1;
        (self.start_tick + TimerWheel::ms_to_ticks(ms)).max(now + 1)
    }
}

/// Java `LookingForFishTask` and the `_taskForFish` future that runs it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LookingForFish {
    pub(crate) timer: Option<TimerId>,
    pub(crate) rate: FixedRate,
    /// Java `_endTaskTime`, in world milliseconds.
    pub(crate) end_time_ms: i64,
    pub(crate) is_noob: bool,
    pub(crate) is_upper_grade: bool,
}

/// Java's `Fishing` object (`Player._fishCombat`): the fight with one fish.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FishCombat {
    /// Java `_time`: the seconds left.
    pub(crate) time: i32,
    pub(crate) stop: i32,
    pub(crate) good_use: u8,
    pub(crate) anim: u8,
    pub(crate) mode: u8,
    pub(crate) deceptive_mode: u8,
    pub(crate) timer: Option<TimerId>,
    pub(crate) rate: FixedRate,
    /// Java `_fishId`: the item the fish turns into.
    pub(crate) fish_id: i32,
    pub(crate) fish_max_hp: i32,
    pub(crate) fish_cur_hp: i32,
    pub(crate) regen_hp: f64,
    pub(crate) is_upper_grade: bool,
    pub(crate) lure_id: i32,
}

/// What a player has of fishing (Java `_fishing`, `_fishX/Y/Z`, `_fish`, `_lure`, `_taskForFish`,
/// `_fishCombat`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerFishing {
    /// Java `_fishing`.
    pub(crate) fishing: bool,
    pub(crate) fish_x: i32,
    pub(crate) fish_y: i32,
    pub(crate) fish_z: i32,
    /// Java `_lure`: the item id (the item itself is used up when the line is cast).
    pub(crate) lure_item_id: Option<i32>,
    /// Java `_fish`: a copy of the fish the player will fight.
    pub(crate) fish: Option<Fish>,
    pub(crate) look: Option<LookingForFish>,
    pub(crate) combat: Option<FishCombat>,
}

/// The timers of `TimerKind::Fishing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FishingTimer {
    /// Java `LookingForFishTask.run`.
    LookForFish(ObjectId),
    /// Java `Fishing.run` (every second of a fight).
    Combat(ObjectId),
    /// Java `FishingChampionshipManager.finishChamp`: the end of the weekly period.
    ChampionshipFinish,
    /// Java `FishingChampionshipManager.needRefresh`: the ranking page may be rebuilt again.
    ChampionshipRefresh,
}

/// The fishing state of the world: the tables and the championship.
#[derive(Debug, Default)]
pub struct FishingWorld {
    pub(crate) fish: Option<Arc<FishData>>,
    pub(crate) rods: Option<Arc<FishingRodsData>>,
    pub(crate) monsters: Option<Arc<FishingMonstersData>>,
    pub(crate) championship: Championship,
}

impl World {
    /// Installs `fishes.xml`, `fishingRods.xml` and `fishingMonsters.xml`. Call before players
    /// enter.
    pub fn set_fishing_data(
        &mut self,
        fish: Arc<FishData>,
        rods: Arc<FishingRodsData>,
        monsters: Arc<FishingMonstersData>,
    ) {
        self.fishing.fish = Some(fish);
        self.fishing.rods = Some(rods);
        self.fishing.monsters = Some(monsters);
    }

    /// Java `Player.isFishing()`.
    pub fn is_fishing(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.fishing.fishing)
    }

    /// Java `Player.getFishCombat() != null`.
    pub fn has_fish_combat(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.fishing.combat.is_some())
    }

    /// Routes the timers of `TimerKind::Fishing`.
    pub(crate) fn on_fishing_timer(&mut self, timer: FishingTimer) {
        match timer {
            FishingTimer::LookForFish(id) => self.on_look_for_fish_timer(id),
            FishingTimer::Combat(id) => self.on_fish_combat_timer(id),
            FishingTimer::ChampionshipFinish => self.on_championship_finish_timer(),
            FishingTimer::ChampionshipRefresh => self.on_championship_refresh_timer(),
        }
    }

    /// Java `Player.deleteMe`: `_taskForFish.cancel(false)`. The fight's own task (`_fishAiTask`)
    /// keeps running in Java and ends the fishing of a player who is gone; here it goes with the
    /// player.
    pub(crate) fn fishing_on_player_leave(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let state = std::mem::take(&mut player.fishing);
        for timer in [
            state.look.and_then(|l| l.timer),
            state.combat.and_then(|c| c.timer),
        ]
        .into_iter()
        .flatten()
        {
            self.timers.cancel(timer);
        }
    }
}
