//! Auto potions (`config/Custom/AutoPotions.ini`, `AutoPotionsEnabled`, off by default): a player
//! who typed `.apon` drinks a CP, HP and MP potion from the lists of the file once a second when a
//! vital falls under its percentage.
// Java: taskmanagers/AutoPotionTaskManager.java
// Java: config/custom/AutoPotionsConfig.java
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/AutoPotion.java
//
// Java's lists are `HashSet<Integer>`s, so the potions are tried in the order of the hash table,
// not in the order of the file (`1540,1539,1061,1060` is walked as 1539, 1540, 1060, 1061). The
// settings keep the file order and [`super::java_hash_set_order`] gives Java's.

use std::collections::BTreeSet;

use l2_util::ObjectId;

use crate::inventory::handlers::ItemHandlerKind;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

use super::{CustomPlayTimer, java_hash_set_order};

/// Java `AutoPotionsConfig`. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoPotionSettings {
    /// `AutoPotionsEnabled`.
    pub enabled: bool,
    /// `AutoPotionsInOlympiad`.
    pub in_olympiad: bool,
    /// `AutoPotionMinimumLevel`.
    pub min_level: i32,
    /// `AutoCpEnabled`, `AutoHpEnabled`, `AutoMpEnabled`.
    pub cp_enabled: bool,
    pub hp_enabled: bool,
    pub mp_enabled: bool,
    /// `AutoCpPercentage`, `AutoHpPercentage`, `AutoMpPercentage`.
    pub cp_percentage: i32,
    pub hp_percentage: i32,
    pub mp_percentage: i32,
    /// `AutoCpItemIds`, `AutoHpItemIds`, `AutoMpItemIds` in the order of the file.
    pub cp_item_ids: Vec<i32>,
    pub hp_item_ids: Vec<i32>,
    pub mp_item_ids: Vec<i32>,
}

impl Default for AutoPotionSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            in_olympiad: false,
            min_level: 1,
            cp_enabled: true,
            hp_enabled: true,
            mp_enabled: true,
            cp_percentage: 70,
            hp_percentage: 70,
            mp_percentage: 70,
            cp_item_ids: vec![0],
            hp_item_ids: vec![0],
            mp_item_ids: vec![0],
        }
    }
}

/// What the world keeps for the auto potions.
#[derive(Debug, Default)]
pub(crate) struct AutoPotionWorld {
    pub(crate) settings: AutoPotionSettings,
    /// `AutoPotionTaskManager.PLAYERS`.
    players: BTreeSet<ObjectId>,
    /// The next pass; there is one while a player is in the set.
    timer: Option<TimerId>,
}

/// The three vitals of the pass.
#[derive(Clone, Copy)]
enum Vital {
    Hp,
    Cp,
    Mp,
}

impl World {
    /// Installs `AutoPotions.ini`. Call before players enter.
    pub fn set_auto_potion_settings(&mut self, settings: AutoPotionSettings) {
        self.auto_potion.settings = settings;
    }

    /// Java `AutoPotionsConfig.AUTO_POTIONS_ENABLED`.
    pub(crate) fn auto_potion_enabled(&self) -> bool {
        self.auto_potion.settings.enabled
    }

    /// Java `AutoPotionTaskManager.hasPlayer(player)`.
    pub fn has_auto_potion(&self, id: ObjectId) -> bool {
        self.auto_potion.players.contains(&id)
    }

    /// Java `AutoPotionTaskManager.add(player)`.
    pub fn add_auto_potion(&mut self, id: ObjectId) {
        self.auto_potion.players.insert(id);
        if self.auto_potion.timer.is_none() {
            self.schedule_auto_potion();
        }
    }

    /// Java `AutoPotionTaskManager.remove(player)`.
    pub fn remove_auto_potion(&mut self, id: ObjectId) {
        self.auto_potion.players.remove(&id);
    }

    fn schedule_auto_potion(&mut self) {
        let due = self.tick + TimerWheel::ms_to_ticks(1000);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::CustomPlay(CustomPlayTimer::AutoPotion));
        self.auto_potion.timer = Some(timer);
    }

    /// The player leaves: Java's task removes it at its next pass.
    pub(crate) fn auto_potion_on_leave(&mut self, id: ObjectId) {
        self.auto_potion.players.remove(&id);
    }

    /// `AutoPotionTaskManager.run()`, every second.
    pub(crate) fn on_auto_potion_timer(&mut self) {
        self.auto_potion.timer = None;
        let settings = self.auto_potion.settings.clone();
        // NOTE(java-quirk): Java walks a `HashSet<Player>` (identity hash order); the world walks
        // the object ids in order (D-13).
        let ids: Vec<ObjectId> = self.auto_potion.players.iter().copied().collect();
        for id in ids {
            if !self.players.contains_key(&id)
                || self.is_alike_dead(id)
                || (!settings.in_olympiad && self.is_in_olympiad_mode(id))
            {
                self.auto_potion.players.remove(&id);
                continue;
            }
            let mut success = false;
            if settings.hp_enabled {
                success |=
                    self.auto_potion_pass(id, Vital::Hp, settings.hp_percentage, &settings.hp_item_ids);
            }
            if settings.cp_enabled {
                success |=
                    self.auto_potion_pass(id, Vital::Cp, settings.cp_percentage, &settings.cp_item_ids);
            }
            if settings.mp_enabled {
                success |=
                    self.auto_potion_pass(id, Vital::Mp, settings.mp_percentage, &settings.mp_item_ids);
            }
            if !success {
                self.send_message(id, "Auto potion: You are out of potions!");
            }
        }
        if !self.auto_potion.players.is_empty() {
            self.schedule_auto_potion();
        }
    }

    /// One of the three blocks of `run()`: `true` when the player has a potion of the list.
    fn auto_potion_pass(&mut self, id: ObjectId, vital: Vital, percentage: i32, item_ids: &[i32]) -> bool {
        let Some(core) = self.core_of(id) else {
            return false;
        };
        let (current, max, label) = match vital {
            Vital::Hp => (core.cur_hp, self.stats(id).map_or(0, |v| v.max_hp()), "HP"),
            Vital::Cp => (core.cur_cp, self.stats(id).map_or(0, |v| v.max_cp()), "CP"),
            Vital::Mp => (core.cur_mp, self.stats(id).map_or(0, |v| v.max_mp()), "MP"),
        };
        // `((current / max) * 100) < percentage`: a double over an int.
        let restore = ((current / f64::from(max)) * 100.0) < f64::from(percentage);
        let mut success = false;
        for item_id in java_hash_set_order(item_ids) {
            let Some(item) = self.inventory_item_by_item_id(id, item_id).cloned() else {
                continue;
            };
            if item.count <= 0 {
                continue;
            }
            success = true;
            if restore {
                // `ItemHandler.getHandler(item.getEtcItem()).onItemUse(player, item, false)`.
                if let Some(templates) = self.item_templates()
                    && let Some(template) = templates.get(item.item_id)
                    && let Some(kind) = ItemHandlerKind::of_template(template)
                {
                    self.run_item_handler(kind, id, &item, template, false);
                }
                self.send_message(id, &format!("Auto potion: Restored {label}."));
                break;
            }
        }
        success
    }

    /// `AutoPotion.onCommand` through `VoicedCommandHandler.getHandler(word)`: `None` when the
    /// feature is off (`MasterHandler` registers the handler only then) or the word is another.
    pub(crate) fn auto_potion_voiced_handler(&mut self, player: ObjectId, word: &str) -> Option<bool> {
        if !self.auto_potion.settings.enabled || !matches!(word, "apon" | "apoff" | "potionon" | "potionoff")
        {
            return None;
        }
        let level = self.core_of(player).map_or(0, |c| c.level);
        let min = self.auto_potion.settings.min_level;
        if level < min {
            self.send_message(
                player,
                &format!("You need to be at least {min} to use auto potions."),
            );
            return Some(false);
        }
        match word {
            "apon" | "potionon" => {
                self.add_auto_potion(player);
                self.send_message(player, "Auto potions is enabled.");
            }
            _ => {
                self.remove_auto_potion(player);
                self.send_message(player, "Auto potions is disabled.");
            }
        }
        Some(true)
    }
}
