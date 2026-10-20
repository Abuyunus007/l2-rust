//! The effect engine: what skills leave on a creature. Design: `docs/design/m5-skills.md`.
// Java: entity/actor/holders/creature/EffectList.java (see `list.rs`)
// Java: mechanics/skill/{BuffInfo,BuffFinishTask}.java (see `buff_info.rs`)
// Java: mechanics/skill/Skill.java (applyEffects, applyEffectScope, activateSkill; `apply.rs`)
// Java: mechanics/effects/{AbstractEffect,EffectFlag,EffectType,EffectTickTask}.java and the
//       handler registry (`registry.rs`, `handlers/`)
// Java: entity/actor/Creature.java (getEffectList, stopAllEffects*, the abnormal visual effects,
//       isAffected, addInvulAgainst / isInvulAgainst)
//
// # How it fits together
// - Every creature that has effects has one [`EffectList`] in `World::effects` (a side table by
//   object id, like `statuses`). It holds Java's six queues of [`BuffInfo`]s (buffs, triggered,
//   dances, toggles, debuffs, passives), the abnormal type stack, the effect flags and the
//   per-creature parts Java keeps on `Creature` (abnormal visual effects, `_invulAgainst`, the
//   `BuffFinishTask`, `CreatureStat._maxBuffCount`).
// - A [`BuffInfo`] is one applied skill: effector and effected (ids), the skill (id and level),
//   its continuous effects (each with the shared [`EffectHandler`] of its template), the abnormal
//   time and the tick at which it started. It is identified by a [`BuffInfoId`]; every timer and
//   stat function of the info names that id or its effects, so removing it cancels them.
// - Timers (`TimerKind::BuffFinish`, `EffectTick`, `EffectIcons`, `UpdateAndBroadcastStatus`)
//   live on the world's timer wheel. Java's `BuffFinishTask` (one 1 s task per creature that
//   counts every timed buff) and `EffectTickTask` (one fixed-rate task per ticking effect) keep
//   their shapes.
// - Stat functions of an effect are owned by `FuncOwner::Effect` (the effect template, the way
//   Java owns them by the shared `AbstractEffect` object) and go through the stat engine.
// - The entry points for casting (M5-CAST-01), items (M5-ITEM-01) and everything else that
//   gives effects are [`World::apply_effects`], [`World::apply_effects_with`] and
//   [`World::activate_skill`] (`apply.rs`).
// - Handlers ([`registry`]) are trait objects built once per `<effect>` template; a name Java
//   registers but Rust has not ported yet gets an `Unported` stand-in that keeps Java's
//   `isInstant`/`getEffectType`/`canStart` and does nothing (logged once per name).
//
// Not here: parties (`PartySpelled`), summons, olympiad (`ExOlympiadSpelledInfo`), the formulas
// (M5-FORM-02 fills `apply.rs`' hooks), the handlers (M5-EFX-01..03), saving buffs (M5-SAVE-01).

pub mod apply;
pub mod buff_info;
mod charges;
pub mod control;
mod handlers;
pub mod list;
mod probability;
pub mod registry;
pub(crate) mod traits;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use l2_data::SkillTemplate;
use l2_data::skill::EffectScope;
use l2_util::ObjectId;
use tracing::warn;

pub use buff_info::{ActiveEffect, BuffInfo};
pub use list::{EffectList, IconEntry, ListKind};
pub use registry::{EffectCtor, EffectCx, EffectHandler, EffectRegistry, EffectType, effect_flag};

use crate::timer::TimerId;
use crate::world::World;

/// One applied skill on one creature (Java compares `BuffInfo` objects by identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BuffInfoId(pub u64);

/// Java `SkillFinishType`: how an effect ended, which picks the system message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillFinishType {
    /// Ran out or was replaced: "$s1 has worn off." (nothing for a passive).
    Normal,
    /// Taken away: "The effect of $s1 has been removed."
    Removed,
    /// No message.
    Silent,
}

/// A skill as the effect engine names it: id and level (Java holds the `Skill` object).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SkillKey {
    pub id: i32,
    pub level: i32,
}

impl SkillKey {
    pub fn new(id: i32, level: i32) -> Self {
        Self { id, level }
    }

    pub(crate) fn of(skill: &SkillTemplate) -> Self {
        Self::new(skill.id, skill.level)
    }
}

/// The configuration values the effect engine reads. `Default` is Java's default of each key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectSettings {
    /// `PlayerConfig.BUFFS_MAX_AMOUNT` (`MaxBuffAmount`, a byte).
    pub buffs_max_amount: i32,
    /// `TRIGGERED_BUFFS_MAX_AMOUNT` (`MaxTriggeredBuffAmount`).
    pub triggered_buffs_max_amount: i32,
    /// `DANCES_MAX_AMOUNT` (`MaxDanceAmount`).
    pub dances_max_amount: i32,
    /// `EFFECT_TICK_RATIO` (`EffectTickRatio`): milliseconds per effect tick.
    pub effect_tick_ratio: i64,
    /// `SHOW_EFFECT_MESSAGES_ON_LOGIN` (`ShowEffectMessagesOnLogin`).
    pub show_effect_messages_on_login: bool,
    /// `GeneralConfig.DEBUFF_DURATION_USES_RESISTS` (`DebuffDurationUsesResists`), for M5-FORM-02's
    /// abnormal time.
    pub debuff_duration_uses_resists: bool,
    /// `PlayerConfig.PLAYER_FAKEDEATH_UP_PROTECTION` (`PlayerFakeDeathUpProtection`): seconds the
    /// monsters leave a player alone after it got up from fake death.
    pub fake_death_up_protection: i32,
    /// `PlayerConfig.FAKE_DEATH_UNTARGET` (`FakeDeathUntarget`).
    pub fake_death_untarget: bool,
    /// `PlayerConfig.FAKE_DEATH_DAMAGE_STAND` (`FakeDeathDamageStand`).
    pub fake_death_damage_stand: bool,
}

impl Default for EffectSettings {
    fn default() -> Self {
        Self {
            buffs_max_amount: 20,
            triggered_buffs_max_amount: 12,
            dances_max_amount: 12,
            effect_tick_ratio: 666,
            show_effect_messages_on_login: false,
            debuff_duration_uses_resists: true,
            fake_death_up_protection: 0,
            fake_death_untarget: false,
            fake_death_damage_stand: true,
        }
    }
}

/// Which `<effect>` of which skill level (Java: the identity of the `AbstractEffect` object).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct EffectKey {
    pub skill: SkillKey,
    pub scope: EffectScope,
    pub index: usize,
}

/// The effect state of the world.
#[derive(Debug, Default)]
pub struct EffectsWorld {
    /// Every creature's effect list (an entry exists once something was put on the creature).
    pub(crate) lists: HashMap<ObjectId, EffectList>,
    /// The handler constructors.
    pub(crate) registry: EffectRegistry,
    /// The handler of each template used so far (`None`: Java's `createEffect` gave `null`).
    handlers: HashMap<EffectKey, Option<Arc<dyn EffectHandler>>>,
    /// The handler names already logged (unported or unknown), so each is logged once.
    logged: HashSet<String>,
    next_info: u64,
    /// What effects change on a creature besides its effect list (see `control.rs`).
    pub(crate) control: control::ControlTable,
    /// Java `Player._updateAndBroadcastStatusTask`.
    pub(crate) status_tasks: HashMap<ObjectId, TimerId>,
}

impl EffectsWorld {
    pub(crate) fn next_info_id(&mut self) -> BuffInfoId {
        self.next_info += 1;
        BuffInfoId(self.next_info)
    }

    /// Forgets the built handlers (the skill table was replaced).
    pub(crate) fn clear_handlers(&mut self) {
        self.handlers.clear();
    }

    /// The registry, to inspect it.
    pub fn registry(&self) -> &EffectRegistry {
        &self.registry
    }
}

impl World {
    /// The shared skill table, cloned out of the world so a template can be read while the
    /// world changes (`None` in tests without data).
    pub(crate) fn skill_table(&self) -> Option<Arc<l2_data::SkillTemplates>> {
        self.data.stat_data().map(|d| Arc::clone(&d.skills))
    }

    /// The handler of `skill`'s effect `index` in `scope` (Java's `AbstractEffect` object of
    /// that template), built on first use.
    pub(crate) fn effect_handler(
        &mut self,
        skill: &SkillTemplate,
        scope: EffectScope,
        index: usize,
    ) -> Option<Arc<dyn EffectHandler>> {
        let key = EffectKey {
            skill: SkillKey::of(skill),
            scope,
            index,
        };
        if let Some(cached) = self.effects.handlers.get(&key) {
            return cached.clone();
        }
        let template = skill.effects.get(scope).get(index)?;
        let handler = match self.effects.registry.create(template) {
            registry::Created::Ported(handler) => Some(handler),
            registry::Created::Unported(handler) => {
                if self.effects.logged.insert(template.name.clone()) {
                    warn!(
                        "Effect handler {} is not ported yet; its effects do nothing but their stat functions.",
                        template.name
                    );
                }
                Some(handler)
            }
            registry::Created::Failed(message) => {
                if self.effects.logged.insert(template.name.clone()) {
                    warn!("{message}");
                }
                None
            }
        };
        self.effects.handlers.insert(key, handler.clone());
        handler
    }

    /// The effect list of a creature, if anything was ever put on it.
    pub fn effect_list(&self, id: ObjectId) -> Option<&EffectList> {
        self.effects.lists.get(&id)
    }

    /// The effect list of a creature in the world, made on first use (Java makes it in the
    /// `Creature` constructor). `None` for an id that is no player or NPC.
    pub(crate) fn effect_list_mut(&mut self, id: ObjectId) -> Option<&mut EffectList> {
        if !self.players.contains_key(&id) && !self.npcs.contains_key(&id) {
            return None;
        }
        let max_buffs = self.settings.effects.buffs_max_amount;
        Some(
            self.effects
                .lists
                .entry(id)
                .or_insert_with(|| EffectList::new(max_buffs)),
        )
    }

    /// Whether `id` still is a player or NPC; when not, its list goes (a timer of a creature that
    /// left the world finds nothing to do).
    pub(crate) fn effect_owner_present(&mut self, id: ObjectId) -> bool {
        if self.players.contains_key(&id) || self.npcs.contains_key(&id) {
            return true;
        }
        self.forget_effects(id);
        false
    }

    /// Drops the lists of creatures that left the world (with `sweep_statuses`, once a minute).
    pub(crate) fn sweep_effects(&mut self) {
        let gone: Vec<ObjectId> = self
            .effects
            .lists
            .keys()
            .copied()
            .filter(|id| !self.players.contains_key(id) && !self.npcs.contains_key(id))
            .collect();
        for id in gone {
            self.forget_effects(id);
        }
    }

    /// Drops a creature's effect list and cancels its timers without running any hook: the
    /// creature's object is gone (an NPC whose corpse decayed is made anew at its respawn, a new
    /// login is a new `Player`).
    pub(crate) fn forget_effects(&mut self, id: ObjectId) {
        self.forget_control(id);
        if let Some(task) = self.effects.status_tasks.remove(&id) {
            self.timers.cancel(task);
        }
        let Some(list) = self.effects.lists.remove(&id) else {
            return;
        };
        for timer in list.timers() {
            self.timers.cancel(timer);
        }
    }
}
