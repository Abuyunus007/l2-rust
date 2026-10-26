//! PC cafe points: what a character has (`characters.pccafe_points`), what hunting and the retail
//! timer give, and the packet the client shows them with.
// Java: managers/PcCafePointsManager.java
// Java: entity/actor/Player.java (getPcCafePoints, setPcCafePoints)
// Java: config/custom/PremiumSystemConfig.java (the `PcCafe*` keys of `Custom/PremiumSystem.ini`)
//
// The points are the `pccafe_points` column of the shared `characters` row (D-39). The world keeps
// them in `PlayerProfile::pccafe_points` and the persistence task writes them with every store,
// like the fame.
//
// # Left out
// - `hasPremiumStatus()`: the premium system is not ported (nobody is premium), so with
//   `PcCafeOnlyPremium = True` (the shipped file) no points are given, as on a Java server whose
//   accounts have no premium.
// - `isInOfflineMode()`: the offline shops are not modelled (nobody is offline).
// - The other givers (Krateis Cube, Underground Coliseum, Rim Kamaloka, quests' `addExpAndSp`)
//   and `//pccafe` (`AdminPcCafePoints`) belong to the scripts and the admin cards; they call
//   [`World::give_pc_cafe_point`], [`World::set_pc_cafe_points`] and
//   [`World::send_pc_cafe_point_info`].

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExPcCafePointInfo, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::formulas::Rnd;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// The `PcCafe*` keys of `Custom/PremiumSystem.ini`, with Java's corrections already applied
/// (`MaxPcCafePoints` is at least 0, the double chance is 1 outside 0 to 100, the point rate is 1
/// below 0, the low exp chance is clamped to 0 to 100). `game-server/src/world_data.rs` builds it.
#[derive(Debug, Clone, PartialEq)]
pub struct PcCafeSettings {
    /// `PcCafeEnabled`.
    pub enabled: bool,
    /// `PcCafeOnlyPremium`.
    pub only_premium: bool,
    /// `PcCafeRetailLike`: points on a timer, not from hunting.
    pub retail_like: bool,
    /// `PcCafeRewardTime` in milliseconds.
    pub reward_time_ms: i32,
    /// `MaxPcCafePoints`.
    pub max_points: i32,
    /// `DoublingAcquisitionPoints`.
    pub enable_double_points: bool,
    /// `DoublingAcquisitionPointsChance`.
    pub double_points_chance: i32,
    /// `AcquisitionPointsRetailLikePoints`.
    pub retail_like_points: i32,
    /// `AcquisitionPointsRate`.
    pub point_rate: f64,
    /// `AcquisitionPointsRandom`.
    pub random_point: bool,
    /// `RewardLowExpKills`.
    pub reward_low_exp_kills: bool,
    /// `RewardLowExpKillsChance`.
    pub low_exp_kills_chance: i32,
}

impl Default for PcCafeSettings {
    /// The Java defaults of the keys.
    fn default() -> Self {
        Self {
            enabled: false,
            only_premium: false,
            retail_like: true,
            reward_time_ms: 300_000,
            max_points: 200_000,
            enable_double_points: false,
            double_points_chance: 1,
            retail_like_points: 10,
            point_rate: 1.0,
            random_point: false,
            reward_low_exp_kills: true,
            low_exp_kills_chance: 50,
        }
    }
}

/// The settings and the retail timer of each online player.
#[derive(Debug, Default)]
pub(crate) struct PcCafeWorld {
    settings: PcCafeSettings,
    /// The pending `giveRetailPcCafePont` timer of a player.
    timers: HashMap<ObjectId, TimerId>,
}

impl World {
    /// Installs the `PcCafe*` values. Call before players enter.
    pub fn set_pc_cafe_settings(&mut self, settings: PcCafeSettings) {
        self.pccafe.settings = settings;
    }

    /// Java `Player.getPcCafePoints()`.
    pub fn pc_cafe_points(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.data.profile.pccafe_points)
    }

    /// Java `Player.setPcCafePoints(count)`: never above `MaxPcCafePoints`.
    pub fn set_pc_cafe_points(&mut self, id: ObjectId, count: i32) {
        let max = self.pccafe.settings.max_points;
        if let Some(player) = self.players.get_mut(&id) {
            player.data.profile.pccafe_points = if count < max { count } else { max };
        }
    }

    /// `player.sendPacket(new ExPCCafePointInfo(points, pointsToAdd, time))`.
    pub fn send_pc_cafe_point_info(&self, id: ObjectId, points: i32, points_to_add: i32, time: i32) {
        self.send(id, &ExPcCafePointInfo::new(points, points_to_add, time));
    }

    /// Java `player.hasPremiumStatus()`: the premium system is not ported.
    fn pc_cafe_premium(&self, _id: ObjectId) -> bool {
        false
    }

    /// Java `EnterWorld`: "if ((hasPremiumStatus() || !PC_CAFE_ONLY_PREMIUM) && PC_CAFE_RETAIL_LIKE)
    /// PcCafePointsManager.run(player)".
    pub(crate) fn pc_cafe_on_enter(&mut self, id: ObjectId) {
        // `Player.restore`: `setPcCafePoints(rset.getInt("pccafe_points"))` applies the cap.
        let loaded = self.pc_cafe_points(id);
        self.set_pc_cafe_points(id, loaded);
        let settings = &self.pccafe.settings;
        if (self.pc_cafe_premium(id) || !settings.only_premium) && settings.retail_like {
            self.pc_cafe_run(id);
        }
    }

    /// Java `PcCafePointsManager.run(player)`: every `PcCafeRewardTime` the retail points come.
    // NOTE(java-quirk): Java never stops the task (a second `run` from the community board or
    // `//premium` adds another one, and a task outlives its player and only returns early). Here
    // there is one timer per online player and it ends with the player.
    pub(crate) fn pc_cafe_run(&mut self, id: ObjectId) {
        let settings = &self.pccafe.settings;
        let entered = self.players.get(&id).is_some_and(|p| p.entered_world);
        if !settings.enabled || !settings.retail_like || !entered {
            return;
        }
        // `scheduleAtFixedRate` refuses a period of 0 or less.
        let Ok(period) = u64::try_from(settings.reward_time_ms) else {
            return;
        };
        if period == 0 {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(period);
        let timer = self.timers.schedule_at(due, TimerKind::PcCafe(id));
        if let Some(old) = self.pccafe.timers.insert(id, timer) {
            self.timers.cancel(old);
        }
    }

    /// The timer of [`World::pc_cafe_run`].
    pub(crate) fn on_pc_cafe_timer(&mut self, id: ObjectId) {
        self.pccafe.timers.remove(&id);
        if !self.players.contains_key(&id) {
            return;
        }
        self.give_retail_pc_cafe_point(id);
        self.pc_cafe_run(id);
    }

    /// The player leaves: the timer goes.
    pub(crate) fn pc_cafe_on_player_leave(&mut self, id: ObjectId) {
        if let Some(timer) = self.pccafe.timers.remove(&id) {
            self.timers.cancel(timer);
        }
    }

    /// Java `PcCafePointsManager.giveRetailPcCafePont(player)`.
    pub(crate) fn give_retail_pc_cafe_point(&mut self, id: ObjectId) {
        let settings = self.pccafe.settings.clone();
        // `isOnlineInt() == 0`: a player in the world is online.
        if !settings.enabled
            || !settings.retail_like
            || !self.players.contains_key(&id)
            || (!self.pc_cafe_premium(id) && settings.only_premium)
        {
            return;
        }
        let mut points = settings.retail_like_points;
        if points >= settings.max_points {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::TheMaximumAccumulationAllowedOfPcCafePointsHasBeenExceededYouCanNoLongerAcquirePcCafePoints,
                ),
            );
            return;
        }
        if settings.random_point {
            points = self.rng().get_int_range(points / 2, points);
        }
        self.finish_pc_cafe_gift(id, points, &settings, 1);
    }

    /// Java `PcCafePointsManager.givePcCafePoint(player, exp)`: points for the exp of a kill
    /// (called where `Attackable.calculateRewards` and `Party.distributeXpAndSp` give it).
    pub fn give_pc_cafe_point(&mut self, id: ObjectId, exp: f64) {
        let settings = self.pccafe.settings.clone();
        if settings.retail_like
            || !settings.enabled
            || self.is_inside_zone(id, ZoneId::Peace)
            || self.is_inside_zone(id, ZoneId::Pvp)
            || self.is_inside_zone(id, ZoneId::Siege)
            || !self.players.contains_key(&id)
            || self.is_jailed(id)
        {
            return;
        }
        // PC-points only premium accounts.
        if settings.only_premium && !self.pc_cafe_premium(id) {
            return;
        }
        if self.pc_cafe_points(id) >= settings.max_points {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::TheMaximumAccumulationAllowedOfPcCafePointsHasBeenExceededYouCanNoLongerAcquirePcCafePoints,
                ),
            );
            return;
        }
        // `(int) (exp * 0.0001 * PC_CAFE_POINT_RATE)`.
        let mut points = (exp * 0.0001 * settings.point_rate) as i32;
        if settings.random_point {
            points = self.rng().get_int_range(points / 2, points);
        }
        if points == 0
            && exp > 0.0
            && settings.reward_low_exp_kills
            && self.rng().get_int(100) < settings.low_exp_kills_chance
        {
            // Minimum points.
            points = 1;
        }
        if points <= 0 {
            return;
        }
        self.finish_pc_cafe_gift(id, points, &settings, 0);
    }

    /// The end both givers share: the double chance, the cap, the message, the points and the
    /// packet (`time` is the last argument of `ExPCCafePointInfo`).
    fn finish_pc_cafe_gift(&mut self, id: ObjectId, points: i32, settings: &PcCafeSettings, time: i32) {
        let mut points = points;
        let message =
            if settings.enable_double_points && self.rng().get_int(100) < settings.double_points_chance {
                points = points.wrapping_mul(2);
                SystemMessageId::DoublePointsYouAcquiredS1PcBangPoint
            } else {
                SystemMessageId::YouHaveAcquiredS1PcCafePoints
            };
        let current = self.pc_cafe_points(id);
        if current.wrapping_add(points) > settings.max_points {
            points = settings.max_points - current;
        }
        self.send(
            id,
            &SystemMessage::new(message).with(SystemMessageParam::LongNumber(i64::from(points))),
        );
        self.set_pc_cafe_points(id, current.wrapping_add(points));
        let now = self.pc_cafe_points(id);
        self.send_pc_cafe_point_info(id, now, points, time);
    }
}
