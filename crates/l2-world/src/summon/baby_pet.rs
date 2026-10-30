//! `BabyPet`: the baby buffalo, kookaburra and cougar (and their improved forms) heal and buff
//! their owner on their own every two seconds, unless they are switched to the attack mode.
// Java: entity/actor/instance/BabyPet.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::SummonTimer;
use crate::effects::{EffectType, SkillKey};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `BabyPet.BUFF_CONTROL`.
const BUFF_CONTROL: i32 = 5771;
/// `BabyPet.AWAKENING`.
const AWAKENING: i32 = 5753;
/// `scheduleAtFixedRate(new CastTask(this), 3000, 2000)`.
const FIRST_CAST_MS: u64 = 3000;
const CAST_INTERVAL_MS: u64 = 2000;
/// `CategoryType.BABY_PET_GROUP` in `categoryData.xml` (used when no table is loaded).
const BABY_PET_GROUP: [i32; 3] = [12780, 12781, 12782];

/// The fields only a baby pet has.
#[derive(Debug, Clone, PartialEq)]
pub struct BabyState {
    /// `_buffs`: the buffs it casts on the owner.
    pub buffs: Vec<SkillKey>,
    /// `_majorHeal` and `_minorHeal`.
    pub major_heal: Option<SkillKey>,
    pub minor_heal: Option<SkillKey>,
    /// `_recharge`.
    pub recharge: Option<SkillKey>,
    /// `_castTask`.
    pub cast_timer: Option<TimerId>,
    /// `_bufferMode`: the support mode (true) or the attack mode.
    pub buffer_mode: bool,
}

impl Default for BabyState {
    fn default() -> Self {
        Self {
            buffs: Vec::new(),
            major_heal: None,
            minor_heal: None,
            recharge: None,
            cast_timer: None,
            buffer_mode: true,
        }
    }
}

impl World {
    /// The baby state of a pet, if it is a baby pet.
    fn baby_state(&self, pet: ObjectId) -> Option<&BabyState> {
        self.pet_state(pet)?.baby.as_ref()
    }

    fn baby_state_mut(&mut self, pet: ObjectId) -> Option<&mut BabyState> {
        self.pet_state_mut(pet)?.baby.as_mut()
    }

    /// Java `pet instanceof BabyPet`.
    pub fn is_baby_pet(&self, pet: ObjectId) -> bool {
        self.baby_state(pet).is_some()
    }

    /// Java `BabyPet.isInSupportMode()`.
    pub fn baby_is_in_support_mode(&self, pet: ObjectId) -> bool {
        self.baby_state(pet).is_some_and(|b| b.buffer_mode)
    }

    /// Java `Summon.switchMode()` (`BabyPet`'s): support mode and attack mode swap.
    pub fn summon_switch_mode(&mut self, summon: ObjectId) {
        if let Some(baby) = self.baby_state_mut(summon) {
            baby.buffer_mode = !baby.buffer_mode;
        }
    }

    /// Java `BabyPet.onSpawn()`: the skills the baby pet has at its level are sorted into heals,
    /// buffs and the mana recharge, and the cast task starts.
    pub(crate) fn baby_on_spawn(&mut self, pet: ObjectId) {
        if !self.is_baby_pet(pet) {
            return;
        }
        let Some(npc_id) = self.summon_npc_id(pet) else {
            return;
        };
        let level = self.npcs.get(&pet).map_or(1, |n| n.core.level);
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(data) = self.pet_table().and_then(|t| t.pet_data(npc_id)).cloned() else {
            return;
        };
        let max_level = |id: i32| table.max_level(id);
        let mut heal_power = 0.0_f64;
        let mut buffs = Vec::new();
        let (mut major, mut minor, mut recharge) = (None, None, None);
        for learn in data.skills() {
            let id = learn.skill_id;
            let skill_level = data.available_level(id, level, &max_level);
            if skill_level == 0 {
                continue;
            }
            let Some(skill) = table.get(id, skill_level) else {
                continue;
            };
            if skill.id == BUFF_CONTROL || skill.id == AWAKENING {
                continue;
            }
            let key = SkillKey::of(skill);
            if self.skill_has_effect_type(skill, &[EffectType::ManahealByLevel]) {
                recharge = Some(key);
                continue;
            }
            if self.skill_has_effect_type(skill, &[EffectType::Heal]) {
                let power = f64::from(skill.power);
                if heal_power == 0.0 {
                    // Set both heal types to the same skill.
                    major = Some(key);
                    minor = major;
                    heal_power = power;
                } else if power > heal_power {
                    // Another heal skill found - search for most powerful.
                    major = Some(key);
                } else {
                    minor = Some(key);
                }
                continue;
            }
            if crate::skills::checks::is_continuous(skill) && !skill.is_debuff {
                buffs.push(key);
            }
        }
        if let Some(baby) = self.baby_state_mut(pet) {
            baby.buffs = buffs;
            baby.major_heal = major;
            baby.minor_heal = minor;
            baby.recharge = recharge;
        }
        self.baby_start_cast_task(pet);
    }

    /// Java `BabyPet.startCastTask()`.
    // NOTE(java-quirk): the condition is `major != null || buffs != null || (recharge != null &&
    // castTask == null && !isDead())`, so a pet with a heal or a buff is never refused a task.
    // Java would start a second task over the first one then; here a running task stays.
    pub(crate) fn baby_start_cast_task(&mut self, pet: ObjectId) {
        let Some(baby) = self.baby_state(pet) else {
            return;
        };
        let has_buffs = !baby.buffs.is_empty();
        let cond = baby.major_heal.is_some()
            || has_buffs
            || (baby.recharge.is_some() && baby.cast_timer.is_none() && !self.is_dead(pet));
        if !cond || baby.cast_timer.is_some() {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(FIRST_CAST_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Summon(SummonTimer::BabyCast(pet)));
        if let Some(baby) = self.baby_state_mut(pet) {
            baby.cast_timer = Some(timer);
        }
    }

    /// Java `BabyPet.stopCastTask()`.
    pub(crate) fn baby_stop_cast_task(&mut self, pet: ObjectId) {
        let timer = self.baby_state_mut(pet).and_then(|b| b.cast_timer.take());
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// Java `BabyPet.doDie` after the pet died: `stopCastTask(); abortCast();`.
    pub(crate) fn baby_on_die(&mut self, pet: ObjectId) {
        if self.is_baby_pet(pet) {
            self.baby_stop_cast_task(pet);
            self.abort_cast(pet);
        }
    }

    /// Java `BabyPet.unSummon(owner)` before `Pet.unSummon`: `stopCastTask(); abortCast();`.
    pub(crate) fn baby_on_unsummon(&mut self, pet: ObjectId) {
        self.baby_on_die(pet);
    }

    /// Java `BabyPet.CastTask.run()`, the fixed rate included.
    pub(crate) fn on_baby_cast_timer(&mut self, pet: ObjectId) {
        let Some(baby) = self.baby_state_mut(pet) else {
            return;
        };
        baby.cast_timer = None;
        let due = self.tick + TimerWheel::ms_to_ticks(CAST_INTERVAL_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Summon(SummonTimer::BabyCast(pet)));
        if let Some(baby) = self.baby_state_mut(pet) {
            baby.cast_timer = Some(timer);
        }
        self.baby_cast_task_run(pet);
    }

    fn baby_cast_task_run(&mut self, pet: ObjectId) {
        let Some(owner) = self.summon_owner(pet) else {
            return;
        };
        // If the owner doesn't meet the conditions avoid casting.
        if !self.players.contains_key(&owner) || self.is_dead(owner) || self.is_invul(owner) {
            return;
        }
        let Some(baby) = self.baby_state(pet).cloned() else {
            return;
        };
        // If the pet doesn't meet the conditions avoid casting.
        if self.is_dead(pet)
            || self.is_casting_now(pet)
            || self.is_betrayed(pet)
            || self.is_muted(pet)
            || self.is_out_of_control(pet)
            || !baby.buffer_mode
            || self.intention_of(pet) == crate::movement::Intention::Cast
        {
            return;
        }
        let Some(table) = self.skill_table() else {
            return;
        };

        if let (Some(major), Some(minor)) = (baby.major_heal, baby.minor_heal) {
            // If the owner's HP is more than 80% for Baby Pets and 70% for Improved Baby pets, do
            // nothing. If the owner's HP is very low, under 15% for Baby pets and under 30% for
            // Improved Baby Pets, have 75% chances of using a strong heal. Otherwise, have 25%
            // chances for weak heal.
            let (cur_hp, max_hp) = (
                self.core_of(owner).map_or(0.0, |c| c.cur_hp),
                self.stats(owner).map_or(1, |s| s.max_hp()),
            );
            let hp_percent = cur_hp / f64::from(max_hp);
            // NOTE(java-quirk): the name says "improved", the category is the plain baby pets.
            let is_improved_baby = self.summon_npc_id(pet).is_some_and(|id| {
                self.id_in_category_or(l2_data::stats::CategoryType::BabyPetGroup, id, |id| {
                    BABY_PET_GROUP.contains(&id)
                })
            });
            if (is_improved_baby && hp_percent < 0.3) || (!is_improved_baby && hp_percent < 0.15) {
                if let Some(skill) = table.get(major.id, major.level)
                    && !self.is_skill_disabled(pet, skill)
                    && self.rng().below(100) <= 75
                    && self.core_of(pet).map_or(0.0, |c| c.cur_mp) >= f64::from(skill.mp_consume)
                {
                    self.baby_cast_skill(pet, owner, major);
                    return;
                }
            } else if major != minor
                && ((is_improved_baby && hp_percent < 0.7) || (!is_improved_baby && hp_percent < 0.8))
            {
                // Cast _minorHeal only if it's different than _majorHeal, then pet has two heals
                // available.
                if let Some(skill) = table.get(minor.id, minor.level)
                    && !self.is_skill_disabled(pet, skill)
                    && self.rng().below(100) <= 25
                    && self.core_of(pet).map_or(0.0, |c| c.cur_mp) >= f64::from(skill.mp_consume)
                {
                    self.baby_cast_skill(pet, owner, minor);
                    return;
                }
            }
        }

        // Buff Control is not active.
        if !self.is_affected_by_skill(pet, BUFF_CONTROL) {
            // searching for usable buffs
            let mut current: Vec<SkillKey> = Vec::new();
            for buff in &baby.buffs {
                let Some(skill) = table.get(buff.id, buff.level) else {
                    continue;
                };
                if self.is_skill_disabled(pet, skill) {
                    continue;
                }
                if self.core_of(pet).map_or(0.0, |c| c.cur_mp) < f64::from(skill.mp_consume) {
                    continue;
                }
                // If owner already have the buff, continue.
                let list = self.effect_list(owner);
                let owner_buff = list.and_then(|l| l.info_by_abnormal_type(skill.abnormal_type));
                if let Some(info) = owner_buff
                    && skill.abnormal_level
                        <= table
                            .get(info.skill.id, info.skill.level)
                            .map_or(0, |s| s.abnormal_level)
                {
                    continue;
                }
                // If owner have the buff blocked, continue.
                if list.is_some_and(|l| l.blocked_abnormal_types().contains(&skill.abnormal_type)) {
                    continue;
                }
                current.push(*buff);
            }
            if !current.is_empty() {
                let index = self.rng().below(current.len() as i32);
                let skill = current[usize::try_from(index).unwrap_or(0)];
                self.baby_cast_skill(pet, owner, skill);
                return;
            }
        }

        // Buffs/heal not casted, trying recharge, if exist recharge casted only if owner in
        // combat stance.
        if let Some(recharge) = baby.recharge
            && self.is_in_combat(owner)
        {
            let (mp, max_mp) = (
                self.core_of(owner).map_or(0.0, |c| c.cur_mp),
                self.stats(owner).map_or(1, |s| s.max_mp()),
            );
            if mp / f64::from(max_mp) < 0.6
                && self.rng().below(100) <= 60
                && let Some(skill) = table.get(recharge.id, recharge.level)
                && !self.is_skill_disabled(pet, skill)
                && self.core_of(pet).map_or(0.0, |c| c.cur_mp) >= f64::from(skill.mp_consume)
            {
                self.baby_cast_skill(pet, owner, recharge);
            }
        }
    }

    /// Java `BabyPet.castSkill(skill)`.
    fn baby_cast_skill(&mut self, pet: ObjectId, owner: ObjectId, key: SkillKey) {
        // casting automatically stops any other action (such as autofollow or a move-to). We need
        // to gather the necessary info to restore the previous state.
        let previous_follow = self.follow_status(pet);
        let cast_range = self
            .skill_table()
            .and_then(|t| t.get(key.id, key.level).map(|s| s.cast_range))
            .unwrap_or(0);
        // Pet not following and owner outside cast range.
        if !previous_follow {
            let inside = match (self.position_of(pet), self.position_of(owner)) {
                (Some(a), Some(b)) => crate::target::distance_3d(a, b) < f64::from(cast_range),
                _ => false,
            };
            if !inside {
                return;
            }
        }
        self.set_creature_target(pet, Some(owner));
        self.summon_use_magic(pet, key, false, false);
        self.send_to_summon_owner(
            pet,
            &SystemMessage::new(SystemMessageId::YourPetUsesS1).with(SystemMessageParam::SkillName {
                id: key.id,
                level: key.level,
            }),
        );
        // calling useMagic changes the follow status, if the babypet actually casts (as opposed to
        // failing due some factors, such as too low MP, etc). if the status has actually been
        // changed, revert it. Else, allow the pet to continue whatever it was trying to do.
        if previous_follow != self.follow_status(pet) {
            self.set_follow_status(pet, previous_follow);
        }
    }
}
