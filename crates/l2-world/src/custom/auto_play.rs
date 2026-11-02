//! Auto play (`config/Custom/AutoPlay.ini`, `EnableAutoPlay`, off by default): a player who typed
//! `.play start` hunts on its own. Every 700 ms it picks a target, attacks it, sweeps and picks up
//! the loot; every 300 ms it drinks a potion, casts its buffs and its attack skills.
// Java: taskmanagers/AutoPlayTaskManager.java
// Java: taskmanagers/AutoUseTaskManager.java
// Java: entity/actor/holders/player/{AutoPlaySettingsHolder,AutoUseSettingsHolder}.java
// Java: config/custom/AutoPlayConfig.java
// Java: dist/game/data/scripts/handlers/chat/commands/voiced/AutoPlay.java (the login and logout
//       listeners; the pages are in `auto_play_ui.rs`)
// Java: mechanics/skill/Skill.java (`checkForAreaOffensiveSkills`: the auto play lines)
//
// # Model
// Java keeps the players in pools of 200 that each run one repeating task. The world has one
// timer per task kind that fires while a player is in its set (the order of the players is the
// order of the object ids, D-13). Both tasks stop themselves when their set is empty.
//
// The settings of a player (`AutoPlaySettingsHolder`, `AutoUseSettingsHolder`) live in
// [`AutoPlayer`], kept by object id and dropped when the player leaves.
//
// # Left out
// - `PetSkillData.getKnownSkill(summon, id)`: a pet skill the pet does not know itself (the
//   summon's own skills are used).
// - The option skills of enchant options and augmentations in the skill list of `.playskills`
//   (they are shown as ordinary skills).
// - `AntiFeedManager` and the dual-box counters of `startOfflinePlay`.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use l2_data::skill::TargetType;
use l2_data::stats::AbnormalType;
use l2_util::ObjectId;

use crate::effects::SkillKey;
use crate::inventory::handlers::ItemHandlerKind;
use crate::movement::Intention;
use crate::objects::ObjectKind;
use crate::target::{distance_2d, distance_3d};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

use super::{CustomPlayTimer, java_hash_set_order};

/// `AutoPlayTaskManager.AUTO_ATTACK_ACTION`.
pub(crate) const AUTO_ATTACK_ACTION: i32 = 2;
/// `AutoUseTaskManager.REUSE_MARGIN_TIME`.
const REUSE_MARGIN_TIME: i32 = 3;
/// `AutoPlayTaskManager.TASK_DELAY` and `AutoUseTaskManager.TASK_DELAY`.
const PLAY_DELAY_MS: u64 = 700;
const USE_DELAY_MS: u64 = 300;

/// Java `AutoPlayConfig`. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoPlayRules {
    /// `EnableAutoPlay`.
    pub enable_auto_play: bool,
    /// `EnableAutoPotion`, `EnableAutoSkill`, `EnableAutoItem`.
    pub enable_auto_potion: bool,
    pub enable_auto_skill: bool,
    pub enable_auto_item: bool,
    /// `ResumeAutoPlay`.
    pub resume_auto_play: bool,
    /// `AssistLeader`.
    pub enable_auto_assist: bool,
    /// `ShortRange`, `LongRange`.
    pub short_range: i32,
    pub long_range: i32,
    /// `AutoPlayPremium`.
    pub auto_play_premium: bool,
    /// `DisabledSkillIds`, `DisabledItemIds`, `IgnoredAutoPickItems`.
    pub disabled_auto_skills: Vec<i32>,
    pub disabled_auto_items: Vec<i32>,
    pub ignored_auto_pick_items: Vec<i32>,
    /// `AutoPlayLoginMessage`.
    pub login_message: String,
}

impl Default for AutoPlayRules {
    fn default() -> Self {
        Self {
            enable_auto_play: false,
            enable_auto_potion: true,
            enable_auto_skill: true,
            enable_auto_item: true,
            resume_auto_play: false,
            enable_auto_assist: false,
            short_range: 600,
            long_range: 1400,
            auto_play_premium: false,
            disabled_auto_skills: Vec::new(),
            disabled_auto_items: Vec::new(),
            ignored_auto_pick_items: Vec::new(),
            login_message: String::new(),
        }
    }
}

/// `AutoPlaySettingsHolder` and `AutoUseSettingsHolder` of one player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutoPlayer {
    pub(crate) options: i32,
    pub(crate) pickup: bool,
    pub(crate) next_target_mode: i32,
    pub(crate) short_range: bool,
    pub(crate) respectful_hunting: bool,
    pub(crate) potion_percent: i32,
    /// `_autoSupplyItems`, `_autoActions` and `_autoBuffs`: `ConcurrentHashMap` key sets. The vector
    /// keeps the insertion order; the order Java walks is [`java_hash_set_order`].
    pub(crate) supply_items: Vec<i32>,
    pub(crate) actions: Vec<i32>,
    pub(crate) buffs: Vec<i32>,
    /// `_autoSkills`: a list.
    pub(crate) skills: Vec<i32>,
    pub(crate) potion_item: i32,
    skill_index: usize,
    /// `Player._autoPlaying`.
    pub(crate) auto_playing: bool,
    /// `AutoPlayTaskManager.IDLE_COUNT`.
    idle_count: i32,
}

impl Default for AutoPlayer {
    fn default() -> Self {
        Self {
            options: 0,
            pickup: true,
            next_target_mode: 1,
            short_range: false,
            respectful_hunting: true,
            potion_percent: 50,
            supply_items: Vec::new(),
            actions: Vec::new(),
            buffs: Vec::new(),
            skills: Vec::new(),
            potion_item: 0,
            skill_index: 0,
            auto_playing: false,
            idle_count: 0,
        }
    }
}

impl AutoPlayer {
    /// `AutoUseSettingsHolder.getNextSkillId()`: `None` is `Integer.MIN_VALUE` (an empty list).
    fn next_skill_id(&mut self) -> Option<i32> {
        if self.skill_index >= self.skills.len() {
            self.skill_index = 0;
        }
        let id = self.skills.get(self.skill_index).copied();
        if id.is_none() {
            self.skill_index = 0;
        }
        id
    }
}

/// What the world keeps for the auto play.
#[derive(Debug, Default)]
pub(crate) struct AutoPlayWorld {
    pub(crate) rules: Arc<AutoPlayRules>,
    pub(crate) players: HashMap<ObjectId, AutoPlayer>,
    /// The players of `AutoPlayTaskManager.POOLS` and `AutoUseTaskManager.POOLS`.
    play: BTreeSet<ObjectId>,
    use_set: BTreeSet<ObjectId>,
    play_timer: Option<TimerId>,
    use_timer: Option<TimerId>,
}

impl World {
    /// Installs `AutoPlay.ini`. Call before players enter.
    pub fn set_auto_play_rules(&mut self, rules: AutoPlayRules) {
        self.auto_play.rules = Arc::new(rules);
    }

    /// Java `AutoPlayConfig.ENABLE_AUTO_PLAY`.
    pub fn auto_play_enabled(&self) -> bool {
        self.auto_play.rules.enable_auto_play
    }

    /// The auto play state of a player (made on first use, like Java's holders).
    pub(crate) fn auto_player_mut(&mut self, id: ObjectId) -> &mut AutoPlayer {
        self.auto_play.players.entry(id).or_default()
    }

    /// Java `Player.isAutoPlaying()`.
    pub fn is_auto_playing(&self, id: ObjectId) -> bool {
        self.auto_play.players.get(&id).is_some_and(|p| p.auto_playing)
    }

    /// `player.getAutoPlaySettings().getNextTargetMode()`.
    pub(crate) fn auto_play_target_mode(&self, id: ObjectId) -> i32 {
        self.auto_play.players.get(&id).map_or(1, |p| p.next_target_mode)
    }

    /// Whether the player is in a Java "offline mode": its client is gone but the player stays.
    fn in_offline_mode(&self, id: ObjectId) -> bool {
        self.players.contains_key(&id) && !self.sinks.contains_key(&id)
    }

    // ------------------------------------------------------------------ start and stop

    /// Java `AutoPlayTaskManager.startAutoPlay(player)`.
    pub fn start_auto_play(&mut self, id: ObjectId) {
        if self.auto_play.play.contains(&id) {
            return;
        }
        self.auto_player_mut(id).auto_playing = true;
        self.on_action_request(id);
        self.auto_play.play.insert(id);
        if self.auto_play.play_timer.is_none() {
            self.schedule_auto_play();
        }
    }

    /// Java `AutoPlayTaskManager.stopAutoPlay(player)`.
    pub fn stop_auto_play(&mut self, id: ObjectId) {
        if !self.auto_play.play.remove(&id) {
            return;
        }
        self.set_auto_playing(id, false);
        // Pets must follow their owner.
        if let Some(summon) = self.summon_of(id) {
            self.set_follow_status(summon, true);
        }
        if let Some(player) = self.auto_play.players.get_mut(&id) {
            player.idle_count = 0;
        }
    }

    /// Java `Player.setAutoPlaying(value)`: a player who stops while in offline play loses its row
    /// (`RESTORE_AUTO_PLAY_OFFLINERS`, see `offline_play.rs`).
    fn set_auto_playing(&mut self, id: ObjectId, value: bool) {
        self.auto_player_mut(id).auto_playing = value;
        if !value {
            self.offline_play_auto_play_stopped(id);
        }
    }

    /// Java `AutoUseTaskManager.startAutoUseTask(player)`.
    pub fn start_auto_use_task(&mut self, id: ObjectId) {
        if self.auto_play.use_set.contains(&id) {
            return;
        }
        self.auto_play.use_set.insert(id);
        if self.auto_play.use_timer.is_none() {
            self.schedule_auto_use();
        }
    }

    /// Java `AutoUseTaskManager.stopAutoUseTask(player)`.
    pub fn stop_auto_use_task(&mut self, id: ObjectId) {
        if let Some(player) = self.auto_play.players.get_mut(&id) {
            player.skill_index = 0;
        }
        self.auto_play.use_set.remove(&id);
    }

    fn schedule_auto_play(&mut self) {
        let due = self.tick + TimerWheel::ms_to_ticks(PLAY_DELAY_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::CustomPlay(CustomPlayTimer::AutoPlay));
        self.auto_play.play_timer = Some(timer);
    }

    fn schedule_auto_use(&mut self) {
        let due = self.tick + TimerWheel::ms_to_ticks(USE_DELAY_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::CustomPlay(CustomPlayTimer::AutoUse));
        self.auto_play.use_timer = Some(timer);
    }

    // ------------------------------------------------------------------ login and logout

    /// `ON_PLAYER_LOGIN` of `AutoPlay`: the login message and the settings of the last session.
    pub(crate) fn auto_play_on_enter(&mut self, id: ObjectId) {
        let rules = Arc::clone(&self.auto_play.rules);
        if !rules.enable_auto_play {
            return;
        }
        if !rules.login_message.is_empty() {
            self.send_announcement_say(id, "AutoPlay", &rules.login_message);
        }
        let list = |world: &World, name: &str| world.player_variable_int_list(id, name);
        let actions = list(self, "AUTO_USE_ACTIONS");
        let buffs = list(self, "AUTO_USE_BUFFS");
        let skills = list(self, "AUTO_USE_SKILLS");
        let items = list(self, "AUTO_USE_ITEMS");
        let potion = self
            .players
            .get(&id)
            .and_then(|p| p.data.profile.variables.get("AUTO_USE_POTION"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        let state = self.auto_player_mut(id);
        state.actions.extend(actions);
        state.buffs.extend(buffs);
        state.skills.extend(skills);
        state.supply_items.extend(items);
        state.potion_item = potion;

        let settings = self.player_variable_int_list(id, "AUTO_USE_SETTINGS");
        // Java reads seven entries and throws on a shorter list; the world takes it as unset.
        if settings.len() < 7 {
            return;
        }
        let active = rules.resume_auto_play && settings[1] == 1;
        let state = self.auto_player_mut(id);
        state.potion_percent = settings[5];
        state.options = settings[0];
        state.pickup = settings[2] == 1;
        state.next_target_mode = settings[3];
        state.short_range = settings[4] == 1;
        state.respectful_hunting = settings[6] == 1;
        if active {
            self.start_auto_play(id);
        }
    }

    /// `ON_PLAYER_LOGOUT` of `AutoPlay`: the settings go to the player variables, which the final
    /// store writes. Called before the snapshot is taken.
    pub(crate) fn auto_play_before_store(&mut self, id: ObjectId) {
        if !self.auto_play.rules.enable_auto_play {
            return;
        }
        let Some(state) = self.auto_play.players.get(&id).cloned() else {
            return;
        };
        self.set_player_variable_int_list(id, "AUTO_USE_ACTIONS", &java_hash_set_order(&state.actions));
        self.set_player_variable_int_list(id, "AUTO_USE_BUFFS", &java_hash_set_order(&state.buffs));
        self.set_player_variable_int_list(id, "AUTO_USE_SKILLS", &state.skills);
        self.set_player_variable_int_list(id, "AUTO_USE_ITEMS", &java_hash_set_order(&state.supply_items));
        // NOTE(java-quirk): Java reads the potion back from the variable it is about to write, not
        // from the settings, so the chosen potion is never stored.
        let potion = self
            .players
            .get(&id)
            .and_then(|p| p.data.profile.variables.get("AUTO_USE_POTION"))
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0);
        if potion < 1 {
            self.remove_player_variable(id, "AUTO_USE_POTION");
        } else {
            self.set_player_variable(id, "AUTO_USE_POTION", potion);
        }
        let settings = [
            state.options,
            i32::from(state.auto_playing),
            i32::from(state.pickup),
            state.next_target_mode,
            i32::from(state.short_range),
            state.potion_percent,
            i32::from(state.respectful_hunting),
        ];
        self.set_player_variable_int_list(id, "AUTO_USE_SETTINGS", &settings);
    }

    /// `Player.onTeleported`: "Stop auto play."
    pub(crate) fn auto_play_on_teleported(&mut self, id: ObjectId) {
        if self.auto_play.rules.enable_auto_play {
            self.stop_auto_play(id);
            self.stop_auto_use_task(id);
        }
    }

    /// The player leaves: both tasks drop it (Java's pass stops it at the next run, and
    /// `Player.deleteMe` is not a stop that touches the offline play row).
    pub(crate) fn auto_play_on_leave(&mut self, id: ObjectId) {
        self.auto_play.play.remove(&id);
        self.auto_play.use_set.remove(&id);
        self.auto_play.players.remove(&id);
    }

    /// `StatSet.getIntegerList(key)` of the player variables.
    fn player_variable_int_list(&self, id: ObjectId, name: &str) -> Vec<i32> {
        self.players
            .get(&id)
            .and_then(|p| p.data.profile.variables.get(name))
            .map(|v| v.split(',').filter_map(|s| s.parse::<i32>().ok()).collect())
            .unwrap_or_default()
    }

    /// `StatSet.setIntegerList(key, list)`: an empty list removes the variable.
    fn set_player_variable_int_list(&mut self, id: ObjectId, name: &str, list: &[i32]) {
        if list.is_empty() {
            self.remove_player_variable(id, name);
            return;
        }
        let text = list.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
        self.set_player_variable_text(id, name, text);
    }

    // ------------------------------------------------------------------ the target rules

    /// `AutoPlayTaskManager.AutoPlay.isTargetModeValid(mode, player, creature)`.
    pub(crate) fn auto_play_target_mode_valid(
        &self,
        mode: i32,
        player: ObjectId,
        creature: ObjectId,
    ) -> bool {
        let kind = self.objects.get(&creature).map(|e| e.kind);
        let is_npc = kind == Some(ObjectKind::Npc);
        let is_playable = matches!(kind, Some(ObjectKind::Player | ObjectKind::Summon));
        if !self.is_targetable(creature)
            || (is_npc
                && (self.is_invul(creature) || !self.npc_template(creature).is_some_and(|t| t.show_name)))
        {
            return false;
        }
        match mode {
            1 => {
                self.is_monster(creature)
                    && !self.is_raid(creature)
                    && self.is_auto_attackable_by(creature, player)
            }
            2 => is_playable && self.is_auto_attackable_by(creature, player),
            3 => is_npc && !self.is_monster(creature) && !self.is_inside_zone(creature, ZoneId::Peace),
            _ => {
                (is_npc && !self.is_inside_zone(creature, ZoneId::Peace))
                    || (is_playable && self.is_auto_attackable_by(creature, player))
            }
        }
    }

    /// `isMageCaster(player)`: "We take granted that mage classes do not auto hit."
    fn auto_play_is_mage_caster(&self, id: ObjectId) -> bool {
        !self
            .auto_play
            .players
            .get(&id)
            .is_some_and(|p| p.actions.contains(&AUTO_ATTACK_ACTION))
    }

    /// Java `Skill.checkForAreaOffensiveSkills`, the first auto play block: while auto playing for
    /// monsters or NPCs a player is no target.
    pub(crate) fn auto_play_refuses_player_target(&self, caster: ObjectId) -> bool {
        self.is_auto_playing(caster) && matches!(self.auto_play_target_mode(caster), 1 | 3)
    }

    /// Java `Skill.checkForAreaOffensiveSkills`, the "target mode check" block.
    pub(crate) fn auto_play_refuses_unflagged_target(&self, caster: ObjectId, target: ObjectId) -> bool {
        if !self.is_auto_playing(caster) {
            return false;
        }
        let flag = self.pvp_flag_of(target);
        let karma = self.players.get(&target).map_or(0, |p| p.data.karma);
        (flag == 0 || karma < 1) && !matches!(self.auto_play_target_mode(caster), 0 | 2)
    }

    // ------------------------------------------------------------------ AutoPlay

    /// `AutoPlayTaskManager.AutoPlay.run()`.
    pub(crate) fn on_auto_play_timer(&mut self) {
        self.auto_play.play_timer = None;
        let ids: Vec<ObjectId> = self.auto_play.play.iter().copied().collect();
        for id in ids {
            self.auto_play_think(id);
        }
        if !self.auto_play.play.is_empty() {
            self.schedule_auto_play();
        }
    }

    fn auto_play_think(&mut self, id: ObjectId) {
        let rules = Arc::clone(&self.auto_play.rules);
        if !self.players.contains_key(&id)
            || (self.in_offline_mode(id) && !self.is_offline_play(id))
            || !rules.enable_auto_play
        {
            self.stop_auto_play(id);
            return;
        }
        if self.is_sitting(id) || self.is_casting_now(id) || self.queued_skill(id).is_some() {
            return;
        }

        // Next target mode.
        let target_mode = self.auto_play_target_mode(id);

        // Skip thinking.
        let target = self.target_of(id).filter(|t| self.is_creature_object(*t));
        if let Some(creature) = target {
            if self.is_alike_dead(creature) || !self.auto_play_target_mode_valid(target_mode, id, creature) {
                // Logic for Spoil (254) skill.
                let has_spoil = self
                    .auto_play
                    .players
                    .get(&id)
                    .is_some_and(|p| p.skills.contains(&254));
                if self.is_monster(creature) && self.is_dead(creature) && has_spoil {
                    if let Some(sweeper) = self.known_skill(id, 42)
                        && self.check_spoil_owner(creature, id)
                    {
                        // Move to target.
                        if self.play_distance_2d(id, creature) > 40.0 {
                            if !self.is_moving(id)
                                && let Some((x, y, z)) = self.position_of(creature)
                            {
                                self.set_intention_move_to(id, x, y, z);
                            }
                            return;
                        }
                        // Sweep target.
                        self.do_cast(id, sweeper);
                        return;
                    }
                }
                // Clear target.
                self.set_target(id, None);
            } else {
                let their_target = self.creature_target(creature);
                if their_target == Some(id) || their_target.is_none() {
                    self.auto_play_fight(id, creature);
                    return;
                }
            }
        }

        // Reset idle count.
        if let Some(player) = self.auto_play.players.get_mut(&id) {
            player.idle_count = 0;
        }

        // Pickup.
        let pickup = self.auto_play.players.get(&id).is_none_or(|p| p.pickup);
        if pickup && self.is_inventory_under_90(id, false) && self.auto_play_pickup(id, &rules) {
            return;
        }

        // Find target.
        let creature = self.auto_play_find_target(id, &rules, target_mode);

        // New target was assigned.
        if let Some(creature) = creature {
            self.set_target(id, Some(creature));
            // We take granted that mage classes do not auto hit.
            if self.auto_play_is_mage_caster(id) {
                self.auto_play_walk_to_caster_range(id, creature);
                return;
            }
            self.set_intention_attack(id, creature);
        }
    }

    /// `creature.isCreature()` for the objects the auto play can target.
    fn is_creature_object(&self, id: ObjectId) -> bool {
        matches!(
            self.objects.get(&id).map(|e| e.kind),
            Some(
                ObjectKind::Player
                    | ObjectKind::Npc
                    | ObjectKind::Summon
                    | ObjectKind::Door
                    | ObjectKind::Vehicle
            )
        )
    }

    fn play_distance_2d(&self, a: ObjectId, b: ObjectId) -> f64 {
        match (self.position_of(a), self.position_of(b)) {
            (Some(a), Some(b)) => distance_2d(a, b),
            _ => f64::MAX,
        }
    }

    fn play_distance_3d(&self, a: ObjectId, b: ObjectId) -> f64 {
        match (self.position_of(a), self.position_of(b)) {
            (Some(a), Some(b)) => distance_3d(a, b),
            _ => f64::MAX,
        }
    }

    /// "A caster out of range would otherwise stand still casting at a target it never approaches."
    fn auto_play_walk_to_caster_range(&mut self, id: ObjectId, creature: ObjectId) {
        if self.play_distance_2d(id, creature) > 900.0
            && !self.is_moving(id)
            && self.has_ai(id)
            && !self.is_disabled(id)
            && let Some((x, y, z)) = self.position_of(creature)
        {
            self.set_intention_move_to(id, x, y, z);
        }
    }

    /// The `(creature.getTarget() == player) || (creature.getTarget() == null)` block: the target
    /// is valid and nobody else fights it. Always ends the pass of the player.
    fn auto_play_fight(&mut self, id: ObjectId, creature: ObjectId) {
        // GeoEngine can see target check.
        if !self.can_see_target(id, creature) {
            self.set_target(id, None);
            return;
        }

        // Logic adjustment for summons not attacking.
        if let Some(summon) = self.summon_of(id)
            && self.has_ai(summon)
            && !self.is_moving(summon)
            && !self.is_disabled(summon)
            && !matches!(self.intention_of(summon), Intention::Attack | Intention::Cast)
            && self.is_auto_attackable_by(creature, id)
            && self.can_see_target(id, creature)
        {
            self.set_intention_attack(summon, creature);
        }

        // We take granted that mage classes do not auto hit.
        if self.auto_play_is_mage_caster(id) {
            self.auto_play_walk_to_caster_range(id, creature);
            return;
        }

        // Check if actually attacking.
        if self.has_ai(id)
            && !self.is_attacking_now(id)
            && !self.is_casting_now(id)
            && !self.is_moving(id)
            && !self.is_disabled(id)
        {
            if self.intention_of(id) != Intention::Attack {
                if self.is_auto_attackable_by(creature, id) {
                    // GeoEngine can see target check.
                    if !self.can_see_target(id, creature) {
                        self.set_target(id, None);
                        return;
                    }
                    self.set_intention_attack(id, creature);
                }
            } else {
                // Re-submitting the same attack revalidates a move to target that stalled.
                self.set_intention_attack(id, creature);
                if self.is_attacking_now(id) || self.is_casting_now(id) || self.is_moving(id) {
                    if let Some(player) = self.auto_play.players.get_mut(&id) {
                        player.idle_count = 0;
                    }
                    return;
                }
                self.auto_play_unstick(id, creature);
            }
        }
    }

    /// The `idleCount > 10` block: a character that stands next to its target without hitting it
    /// steps back.
    fn auto_play_unstick(&mut self, id: ObjectId, creature: ObjectId) {
        let (ranged, has_weapon) = {
            let Some(view) = self.stats(id) else {
                return;
            };
            let weapon = view.active_weapon_item();
            (
                weapon
                    .and_then(crate::inventory::weapon_type)
                    .is_some_and(|w| w.is_ranged()),
                weapon.is_some(),
            )
        };
        if !has_weapon {
            return;
        }
        let idle = self.auto_play.players.get(&id).map_or(0, |p| p.idle_count);
        if idle > 10 {
            let (Some(from), Some(to)) = (self.position_of(id), self.position_of(creature)) else {
                return;
            };
            // NOTE(java-quirk): Java turns the heading value (0 to 65535) into radians as if it
            // were degrees, and the course of 180 degrees makes the step `cos(radian)`.
            let heading = f64::from(heading_from(from, to));
            let radian = heading.to_radians();
            let course = 180.0_f64.to_radians();
            let own_radius = self.stats(id).map_or(0.0, |s| f64::from(s.collision_radius()));
            let their_radius = f64::from(self.collision_radius_of(creature));
            let distance = (if ranged {
                own_radius
            } else {
                own_radius + their_radius
            }) * 2.0;
            let x1 = ((std::f64::consts::PI + radian + course).cos() * distance) as i32;
            let y1 = ((std::f64::consts::PI + radian + course).sin() * distance) as i32;
            let (x, y) = if ranged {
                (from.0 + x1, from.1 + y1)
            } else {
                (to.0 + x1, to.1 + y1)
            };
            self.set_intention_move_to(id, x, y, from.2);
            if let Some(player) = self.auto_play.players.get_mut(&id) {
                player.idle_count = 0;
            }
        } else if let Some(player) = self.auto_play.players.get_mut(&id) {
            player.idle_count = idle + 1;
        }
    }

    /// The pickup block: `true` when the pass is over (the player walks to an item or picks it).
    fn auto_play_pickup(&mut self, id: ObjectId, rules: &AutoPlayRules) -> bool {
        let instance = self.instance_id_of(id);
        let Some(me) = self.position_of(id) else {
            return false;
        };
        // First item we can act on: either reachable far (move to it) or reachable close and
        // pickup-eligible.
        let mut found: Option<ObjectId> = None;
        for object in self.visible_objects_in_range(id, 200) {
            if self.objects.get(&object).map(|e| e.kind) != Some(ObjectKind::Item) {
                continue;
            }
            let Some(item) = self.ground.items.get(&object) else {
                continue;
            };
            if !self.objects.get(&object).is_some_and(|e| e.spawned)
                || rules.ignored_auto_pick_items.contains(&item.data.item_id)
                || !self.can_move_to_target(me, (item.x, item.y, item.z), instance)
            {
                continue;
            }
            // Far item: we can move to it.
            let far = distance_2d(me, (item.x, item.y, item.z)) > 70.0;
            // Close item: must be unprotected or owned by player.
            if far || !item.protected || item.owner == Some(id) {
                found = Some(object);
                break;
            }
        }
        let Some(item) = found else {
            return false;
        };
        let Some(spot) = self.position_of(item) else {
            return false;
        };
        if distance_2d(me, spot) > 70.0 {
            if !self.is_moving(id) {
                self.set_intention_move_to(id, spot.0, spot.1, spot.2);
            }
            return true;
        }
        // Item is close and pickup-eligible.
        self.do_pickup_item(id, item);
        // Avoid pickup being skipped.
        true
    }

    /// The "Find target" block: the leader's target, or the nearest creature that fits.
    fn auto_play_find_target(
        &mut self,
        id: ObjectId,
        rules: &AutoPlayRules,
        target_mode: i32,
    ) -> Option<ObjectId> {
        let party = self.party_of(id);
        let leader = party.and_then(|p| self.party_leader(p));
        if rules.enable_auto_assist
            && let (Some(party), Some(leader)) = (party, leader)
            && leader != id
            && !self.is_dead(leader)
        {
            let mut creature = None;
            let range = f64::from(self.settings.exp.alt_party_range) * 2.0;
            if self.play_distance_3d(leader, id) < range {
                let leader_target = self.target_of(leader);
                let attackable = leader_target.is_some_and(|t| {
                    self.is_attackable_npc(t)
                        || (matches!(
                            self.objects.get(&t).map(|e| e.kind),
                            Some(ObjectKind::Player | ObjectKind::Summon)
                        ) && !self.party_members(party).contains(&t))
                });
                if attackable {
                    creature = leader_target;
                } else if self.intention_of(id) != Intention::Follow && !self.is_disabled(id) {
                    self.auto_play_follow(id, leader);
                }
            }
            return creature;
        }
        let (respectful, short) = self
            .auto_play
            .players
            .get(&id)
            .map_or((true, false), |p| (p.respectful_hunting, p.short_range));
        let range = if short && target_mode != 2 {
            rules.short_range
        } else {
            rules.long_range
        };
        let me = self.position_of(id)?;
        let instance = self.instance_id_of(id);
        let own_summon = self.summon_of(id);
        let mut nearest: Option<ObjectId> = None;
        let mut best = f64::MAX;
        for nearby in self.visible_objects_in_range(id, range) {
            if !matches!(
                self.objects.get(&nearby).map(|e| e.kind),
                Some(ObjectKind::Player | ObjectKind::Npc | ObjectKind::Summon)
            ) {
                continue;
            }
            let Some(spot) = self.position_of(nearby) else {
                continue;
            };
            let distance = distance_3d(me, spot);
            if distance >= best {
                continue;
            }
            // Skip unavailable creatures.
            if self.is_alike_dead(nearby) {
                continue;
            }
            // Check creature target.
            let playable = matches!(
                self.objects.get(&nearby).map(|e| e.kind),
                Some(ObjectKind::Player | ObjectKind::Summon)
            );
            if respectful
                && !playable
                && let Some(their) = self.creature_target(nearby)
                && their != id
                && own_summon != Some(their)
            {
                continue;
            }
            // Check next target mode.
            if !self.auto_play_target_mode_valid(target_mode, id, nearby) {
                continue;
            }
            // Check if creature is reachable.
            if (me.2 - spot.2).abs() < 800
                && self.can_see_target(id, nearby)
                && self.can_move_to_target(me, spot, instance)
            {
                best = distance;
                nearest = Some(nearby);
            }
        }
        nearest
    }

    /// Java `PlayerAI.setIntentionFollow(target)`.
    fn auto_play_follow(&mut self, id: ObjectId, target: ObjectId) {
        if self.intention_of(id) == Intention::Rest
            || self.is_all_skills_disabled(id)
            || self.is_casting_now(id)
            || self.is_movement_disabled(id)
            || self.is_dead(id)
            || id == target
        {
            self.client_action_failed(id);
            return;
        }
        self.client_stop_auto_attack(id);
        if let Some(player) = self.players.get_mut(&id) {
            player.movement.intention = Intention::Follow;
        }
        self.start_follow(id, target, crate::interact::NORMAL_FOLLOW_RANGE);
    }

    // ------------------------------------------------------------------ AutoUse

    /// `AutoUseTaskManager.AutoUse.run()`.
    pub(crate) fn on_auto_use_timer(&mut self) {
        self.auto_play.use_timer = None;
        let ids: Vec<ObjectId> = self.auto_play.use_set.iter().copied().collect();
        for id in ids {
            self.auto_use_think(id);
        }
        if !self.auto_play.use_set.is_empty() {
            self.schedule_auto_use();
        }
    }

    fn auto_use_think(&mut self, id: ObjectId) {
        let rules = Arc::clone(&self.auto_play.rules);
        if !self.players.contains_key(&id) || (self.in_offline_mode(id) && !self.is_offline_play(id)) {
            self.stop_auto_use_task(id);
            return;
        }
        if self.is_sitting(id)
            || self.is_stunned(id)
            || self.is_sleeping(id)
            || self.is_paralyzed(id)
            || self.is_control_blocked(id)
            || self.is_alike_dead(id)
            || self.is_mounted(id)
        {
            return;
        }
        let in_peace = self.is_inside_zone(id, ZoneId::Peace);

        if rules.enable_auto_item && !in_peace {
            self.auto_use_items(id);
        }
        if rules.enable_auto_potion && !in_peace {
            let percent = self.auto_play.players.get(&id).map_or(50, |p| p.potion_percent);
            if self.current_hp_percent_of(id) < percent {
                self.auto_use_potion(id);
            }
        }
        if rules.enable_auto_skill {
            self.auto_use_buffs(id, in_peace);
            // Continue when auto play is not enabled.
            if !self.is_auto_playing(id) {
                return;
            }
            self.auto_use_skills(id);
        }
    }

    /// `getCurrentHpPercent()`: `(int) ((currentHp * 100) / maxHp)`.
    fn current_hp_percent_of(&self, id: ObjectId) -> i32 {
        let hp = self.core_of(id).map_or(0.0, |c| c.cur_hp);
        let max = self.stats(id).map_or(1, |s| s.max_hp());
        ((hp * 100.0) / f64::from(max)) as i32
    }

    fn auto_use_items(&mut self, id: ObjectId) {
        let supply = self
            .auto_play
            .players
            .get(&id)
            .map(|p| java_hash_set_order(&p.supply_items))
            .unwrap_or_default();
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        'items: for item_id in supply {
            if self.is_teleporting(id) {
                break;
            }
            let Some(item) = self.inventory_item_by_item_id(id, item_id).cloned() else {
                self.auto_player_mut(id).supply_items.retain(|i| *i != item_id);
                continue;
            };
            let Some(template) = templates.get(item_id) else {
                continue;
            };
            if template.oly_restricted && self.is_in_olympiad_mode(id) {
                self.auto_player_mut(id).supply_items.retain(|i| *i != item_id);
                continue;
            }
            if !self.check_item_condition(id, template, false) {
                continue;
            }
            if let Some(skills) = &template.skills.item_skills {
                for holder in skills {
                    let Some(skill) = table.get(holder.id, holder.level) else {
                        continue;
                    };
                    if !skill.is_active() {
                        continue;
                    }
                    if self.is_affected_by_skill(id, skill.id)
                        || self.has_skill_reuse(id, skill.reuse_hash_code)
                        || !self.check_skill_condition(id, id, SkillKey::new(skill.id, skill.level))
                    {
                        continue 'items;
                    }
                }
            }
            let reuse_delay = template.reuse_delay;
            if (reuse_delay <= 0 || self.item_remaining_reuse_time(id, item.object_id) <= 0)
                && let Some(kind) = ItemHandlerKind::of_template(template)
                && self.run_item_handler(kind, id, &item, template, false)
                && reuse_delay > 0
            {
                self.add_time_stamp_item(id, &item, template, i64::from(reuse_delay));
            }
        }
    }

    fn auto_use_potion(&mut self, id: ObjectId) {
        let item_id = self.auto_play.players.get(&id).map_or(0, |p| p.potion_item);
        if item_id <= 0 {
            return;
        }
        let Some(item) = self.inventory_item_by_item_id(id, item_id).cloned() else {
            self.auto_player_mut(id).potion_item = 0;
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            return;
        };
        let reuse_delay = template.reuse_delay;
        if (reuse_delay <= 0 || self.item_remaining_reuse_time(id, item.object_id) <= 0)
            && let Some(kind) = ItemHandlerKind::of_template(template)
            && self.run_item_handler(kind, id, &item, template, false)
            && reuse_delay > 0
        {
            self.add_time_stamp_item(id, &item, template, i64::from(reuse_delay));
        }
    }

    /// The skill `skill_id` as the player (or its summon) knows it: the caster that casts it.
    fn auto_use_skill_of(&self, id: ObjectId, skill_id: i32) -> Option<(ObjectId, SkillKey)> {
        if let Some(key) = self.known_skill(id, skill_id) {
            return Some((id, key));
        }
        let summon = self.summon_of(id)?;
        // NOTE: `PetSkillData.getKnownSkill` (a pet skill by the pet's level) is left out.
        self.known_skill(summon, skill_id).map(|key| (summon, key))
    }

    fn auto_use_buffs(&mut self, id: ObjectId, in_peace: bool) {
        let buffs = self
            .auto_play
            .players
            .get(&id)
            .map(|p| java_hash_set_order(&p.buffs))
            .unwrap_or_default();
        let Some(table) = self.skill_table() else {
            return;
        };
        for skill_id in buffs {
            // Fixes start area issue.
            if in_peace {
                break;
            }
            // Already casting, attacking, or teleporting.
            if self.is_casting_now(id) || self.is_attacking_now(id) || self.is_teleporting(id) {
                break;
            }
            let Some((caster, key)) = self.auto_use_skill_of(id, skill_id) else {
                self.auto_player_mut(id).buffs.retain(|b| *b != skill_id);
                continue;
            };
            let Some(skill) = table.get(key.id, key.level) else {
                continue;
            };
            // Buff use check.
            let target = self.target_of(id);
            if !self.auto_play_can_cast_buff(id, target, skill) {
                continue;
            }
            // Playable target cast.
            let target_kind = target.and_then(|t| self.objects.get(&t).map(|e| e.kind));
            if let Some(target) = target
                && matches!(target_kind, Some(ObjectKind::Player | ObjectKind::Summon))
            {
                // NOTE(java-quirk): `target.asPlayer()` of a summon target is null and the
                // statement throws in Java; the buff is skipped here.
                if target_kind != Some(ObjectKind::Player) {
                    continue;
                }
                let flag = self.pvp_flag_of(target);
                let karma = self.players.get(&target).map_or(0, |p| p.data.karma);
                // `targetPlayer.getParty() == caster.getParty()`: two players without a party
                // are equal.
                let same_party = self.party_of(target) == self.party_of(id);
                if (flag == 0 && karma <= 0) || same_party {
                    self.do_cast(caster, key);
                } else if !self.is_affected_by_skill(caster, skill.id) {
                    self.auto_use_cast_on_self(caster, key, Some(target));
                }
            } else {
                // Target self, cast and re-target.
                self.auto_use_cast_on_self(caster, key, target);
            }
        }
    }

    /// `caster.setTarget(caster); caster.doCast(skill); caster.setTarget(savedTarget);`
    fn auto_use_cast_on_self(&mut self, caster: ObjectId, key: SkillKey, saved: Option<ObjectId>) {
        self.set_creature_target(caster, Some(caster));
        self.do_cast(caster, key);
        self.set_creature_target(caster, saved);
    }

    /// `AutoUse.canCastBuff(player, target, skill)`.
    fn auto_play_can_cast_buff(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
        skill: &l2_data::SkillTemplate,
    ) -> bool {
        if let Some(t) = target
            && self.is_creature_object(t)
            && self.is_alike_dead(t)
            && !matches!(
                skill.target_type,
                TargetType::SelfTarget | TargetType::Corpse | TargetType::PcBody
            )
        {
            return false;
        }
        let playable_target = match target {
            Some(t)
                if matches!(
                    self.objects.get(&t).map(|e| e.kind),
                    Some(ObjectKind::Player | ObjectKind::Summon)
                ) && skill.target_type != TargetType::SelfTarget =>
            {
                t
            }
            _ => id,
        };
        if id != playable_target && self.play_distance_3d(id, playable_target) > f64::from(skill.cast_range) {
            return false;
        }
        if !self.auto_play_can_use_magic(id, Some(playable_target), skill) {
            return false;
        }
        let (buff, abnormal) = {
            let Some(list) = self.effect_list(playable_target) else {
                return true;
            };
            (
                list.info_by_skill_id(skill.id)
                    .map(|i| (i.skill, i.time(self.tick))),
                list.info_by_abnormal_type(skill.abnormal_type).map(|i| i.skill),
            )
        };
        let Some(table) = self.skill_table() else {
            return false;
        };
        if let Some(abnormal) = abnormal {
            if let Some((buff_skill, time)) = buff {
                return abnormal.id == buff_skill.id
                    && (time <= REUSE_MARGIN_TIME || buff_skill.level < skill.level);
            }
            let abnormal_level = table
                .get(abnormal.id, abnormal.level)
                .map_or(0, |s| s.abnormal_level);
            return abnormal_level < skill.abnormal_level || skill.abnormal_type == AbnormalType::None;
        }
        buff.is_none()
    }

    fn auto_use_skills(&mut self, id: ObjectId) {
        let count = self.auto_play.players.get(&id).map_or(0, |p| p.skills.len());
        let Some(table) = self.skill_table() else {
            return;
        };
        for _ in 0..count {
            // Already casting, or teleporting.
            if self.is_casting_now(id) || self.is_teleporting(id) {
                break;
            }
            // Acquire next skill.
            let target = self.target_of(id);
            let Some(skill_id) = self.auto_player_mut(id).next_skill_id() else {
                break;
            };
            let Some((caster, key)) = self.auto_use_skill_of(id, skill_id) else {
                let state = self.auto_player_mut(id);
                state.skills.retain(|s| *s != skill_id);
                state.skill_index = 0;
                break;
            };
            if caster != id {
                self.set_creature_target(caster, target);
            }
            // Casting on self stops movement.
            if target == Some(id) {
                break;
            }
            // Check negative effect skill target.
            let Some(target) = target.filter(|t| !self.is_dead(*t)) else {
                // Remove queued skill.
                if self.queued_skill(id).is_some() {
                    self.set_queued_skill(id, None);
                }
                break;
            };
            // Peace zone and auto attackable checks.
            if self.is_inside_zone(target, ZoneId::Peace) || !self.is_auto_attackable_by(target, id) {
                break;
            }
            // Do not attack guards.
            if self.npcs.get(&target).is_some_and(|n| n.class.is_guard()) {
                let mode = self.auto_play_target_mode(id);
                if mode != 3 && mode != 0 {
                    break;
                }
            }
            // Increment skill order.
            self.auto_player_mut(id).skill_index += 1;
            // Skill use check.
            let Some(skill) = table.get(key.id, key.level) else {
                continue;
            };
            if !self.auto_play_can_use_magic(caster, Some(target), skill) {
                continue;
            }
            // Use the skill.
            self.use_magic(caster, key, true, false);
            break;
        }
    }

    /// `AutoUse.canUseMagic(playable, target, skill)`.
    fn auto_play_can_use_magic(
        &mut self,
        caster: ObjectId,
        target: Option<ObjectId>,
        skill: &l2_data::SkillTemplate,
    ) -> bool {
        let owner = self.summon_owner(caster).unwrap_or(caster);
        if skill.item_consume_count > 0
            && self.inventory_item_count(owner, skill.item_consume_id) < i64::from(skill.item_consume_count)
        {
            return false;
        }
        if self.is_player(caster) && self.charges_of(caster) < skill.charge_consume {
            return false;
        }
        let mp_consume = self.mp_initial_consume(caster, skill) + self.mp_consume(caster, skill);
        if mp_consume > 0
            && self
                .core_of(caster)
                .is_none_or(|c| c.cur_mp < f64::from(mp_consume))
        {
            return false;
        }
        // Check if monster is spoiled to avoid Spoil (254) skill recast.
        if skill.id == 254
            && let Some(t) = target
            && self.is_monster(t)
            && self.is_spoiled(t)
        {
            return false;
        }
        !self.is_skill_disabled(caster, skill)
            && self.check_skill_condition(
                caster,
                target.unwrap_or(caster),
                SkillKey::new(skill.id, skill.level),
            )
    }
}

/// `LocationUtil.calculateHeadingFrom(origin, target)`.
fn heading_from(origin: (i32, i32, i32), target: (i32, i32, i32)) -> i32 {
    let mut angle = f64::from(target.1 - origin.1)
        .atan2(f64::from(target.0 - origin.0))
        .to_degrees();
    if angle < 0.0 {
        angle += 360.0;
    }
    (angle * 182.044_444_444) as i32
}
