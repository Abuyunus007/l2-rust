//! The effect handlers: the [`EffectHandler`] trait (Java `AbstractEffect`'s hooks) and the
//! [`EffectRegistry`] (Java `EffectHandler` + `EffectMasterHandler`), name to constructor.
// Java: mechanics/effects/{AbstractEffect,EffectFlag,EffectType}.java
// Java: handler/EffectHandler.java, dist/game/data/scripts/handlers/EffectMasterHandler.java
//
// Java builds one `AbstractEffect` object per `<effect>` of every skill level when the skills load
// (`AbstractEffect.createEffect`) and shares it between every `BuffInfo` of that skill. Here the
// templates stay plain data in `l2-data`; the world builds the handler of a template the first
// time it is used (`EffectsWorld::handler`) and keeps it, so the object is shared the same way.
//
// # Adding a handler (EFX-01..03)
// 1. A file `effects/handlers/<snake_name>.rs` with a struct that reads its parameters in a
//    `fn new(template: &EffectTemplate) -> Result<Self, String>` (Java's constructor) and
//    implements [`EffectHandler`] (only the hooks the Java class overrides).
// 2. One `registry.register("JavaName", ctor)` line in `effects/handlers/mod.rs`
//    (`register_all`). That replaces the [`Unported`] stand-in of the name.
// 3. The `// Java:` comment names the handler class.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use l2_data::SkillTemplate;
use l2_data::skill::EffectTemplate;
use l2_util::ObjectId;

use super::BuffInfoId;
use crate::world::World;

/// Java `EffectFlag` masks (`1 << ordinal()`).
pub mod effect_flag {
    pub const NONE: u32 = 1 << 0;
    pub const RESURRECTION_SPECIAL: u32 = 1 << 1;
    pub const NOBLESS_BLESSING: u32 = 1 << 2;
    pub const SILENT_MOVE: u32 = 1 << 3;
    pub const PROTECTION_BLESSING: u32 = 1 << 4;
    pub const RELAXING: u32 = 1 << 5;
    pub const FEAR: u32 = 1 << 6;
    pub const CONFUSED: u32 = 1 << 7;
    pub const MUTED: u32 = 1 << 8;
    pub const PSYCHICAL_MUTED: u32 = 1 << 9;
    pub const PSYCHICAL_ATTACK_MUTED: u32 = 1 << 10;
    pub const PASSIVE: u32 = 1 << 11;
    pub const DISARMED: u32 = 1 << 12;
    pub const ROOTED: u32 = 1 << 13;
    pub const SLEEP: u32 = 1 << 14;
    pub const STUNNED: u32 = 1 << 15;
    pub const BETRAYED: u32 = 1 << 16;
    pub const INVUL: u32 = 1 << 17;
    pub const PARALYZED: u32 = 1 << 18;
    pub const BLOCK_RESURRECTION: u32 = 1 << 19;
    pub const SERVITOR_SHARE: u32 = 1 << 20;
    pub const POLEARM_SINGLE_TARGET: u32 = 1 << 21;
}

/// Java `EffectType`, in Java's declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EffectType {
    Aggression,
    Buff,
    CharmOfLuck,
    ChatBlock,
    Cpheal,
    Debuff,
    Dispel,
    DispelBySlot,
    DmgOverTime,
    DmgOverTimePercent,
    DeathLink,
    FakeDeath,
    Fear,
    Fishing,
    FishingStart,
    Hate,
    Heal,
    HpDrain,
    MagicalAttack,
    ManahealByLevel,
    ManahealPercent,
    Mute,
    NevitsHourglass,
    NoblesseBlessing,
    None,
    Paralyze,
    PhysicalAttack,
    PhysicalAttackHpLink,
    RebalanceHp,
    RefuelAirship,
    Relaxing,
    Resurrection,
    ResurrectionSpecial,
    Root,
    Sleep,
    StealAbnormal,
    Stun,
    Summon,
    SummonPet,
    SummonNpc,
    Teleport,
    TeleportToTarget,
}

/// What a hook is called for: Java's `(effector, effected, skill)` arguments, plus the effect's
/// own template (its parameters and `ticks`) and the buff info it belongs to.
#[derive(Clone, Copy)]
pub struct EffectCx<'a> {
    /// Java `effector` (the caster, or the owner of a passive skill).
    pub effector: ObjectId,
    /// Java `effected`.
    pub effected: ObjectId,
    /// Java `skill`.
    pub skill: &'a SkillTemplate,
    /// The `<effect>` this handler was built from.
    pub effect: &'a EffectTemplate,
    /// The buff info of a continuous effect. `None` for an instant effect: Java makes a
    /// `BuffInfo` for it that never joins an effect list.
    pub info: Option<BuffInfoId>,
}

impl EffectCx<'_> {
    /// Java `AbstractEffect.getTicks()`.
    pub fn ticks(&self) -> i32 {
        self.effect.ticks
    }

    /// Java `AbstractEffect.getTicksMultiplier()`: `(ticks * EFFECT_TICK_RATIO) / 1000f`.
    // NOTE(java-quirk): `ticks * EFFECT_TICK_RATIO` is a `long`, divided by a `float`: the
    // quotient is a float, widened to double by the return type.
    pub fn ticks_multiplier(&self, world: &World) -> f64 {
        let product = i64::from(self.effect.ticks) * world.settings.effects.effect_tick_ratio;
        f64::from(product as f32 / 1000.0_f32)
    }
}

/// Java `AbstractEffect`: the hooks of one effect handler. Every default is Java's base class
/// behaviour, so a handler implements only what its Java class overrides.
///
/// A handler is built once per `<effect>` template and shared by every buff info of that skill
/// level, like the Java object: it must not keep per-target state (Java's handlers keep theirs on
/// the creature).
pub trait EffectHandler: Send + Sync + fmt::Debug {
    /// Java `getEffectType()`.
    fn effect_type(&self) -> EffectType {
        EffectType::None
    }

    /// Java `getEffectFlags()`: [`effect_flag`] masks.
    fn effect_flags(&self) -> u32 {
        effect_flag::NONE
    }

    /// Java `isInstant()`: applied once when the skill lands, never kept in a buff info.
    fn is_instant(&self) -> bool {
        false
    }

    /// Java `getTicks()`: the ticks of the effect. `template_ticks` is the base class' value (the
    /// `ticks` attribute); `Fear` overrides it with a constant.
    fn ticks(&self, template_ticks: i32) -> i32 {
        template_ticks
    }

    /// Java `calcSuccess(effector, effected, skill)`: whether an instant effect lands.
    fn calc_success(&self, _world: &mut World, _cx: &EffectCx<'_>) -> bool {
        true
    }

    /// Java `canStart(effector, effected, skill)`: whether a continuous effect joins the buff
    /// info.
    fn can_start(&self, _world: &World, _cx: &EffectCx<'_>) -> bool {
        true
    }

    /// Java `onStart(effector, effected, skill)`.
    fn on_start(&self, _world: &mut World, _cx: &EffectCx<'_>) {}

    /// Java `onActionTime(effector, effected, skill)`: one tick of an effect with `ticks`. `false`
    /// ends a toggle.
    fn on_action_time(&self, _world: &mut World, _cx: &EffectCx<'_>) -> bool {
        false
    }

    /// Java `onExit(effector, effected, skill)`.
    fn on_exit(&self, _world: &mut World, _cx: &EffectCx<'_>) {}

    /// Java `checkCondition(Object)` (only `BlockAction` overrides it, with an action id).
    fn check_condition(&self, _value: i32) -> bool {
        true
    }
}

/// Java's handler constructor: builds the handler of one `<effect>` template. An `Err` is what
/// the Java constructor throws (`createEffect` logs it and the effect is dropped).
pub type EffectCtor = fn(&EffectTemplate) -> Result<Box<dyn EffectHandler>, String>;

/// Java `canStart` of the handlers that are not ported yet but that M4's passive skills already
/// needed (the stat engine applied their passive effects before M5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartRule {
    Always,
    /// `effected.isAttackable()` (`Passive`).
    Attackable,
    /// `effected.isPlayer()`.
    Player,
    /// `effected.isPlayable()` (`NoblesseBless`).
    Playable,
    /// `effector.isPlayer() && effected.isSummon()` (`Betray`).
    Summon,
    /// `Fear`: players, summons, and attackables that are no siege NPCs.
    Fear,
}

/// The stand-in for a Java handler that is not ported yet: it keeps Java's `isInstant()`,
/// `getEffectType()` and the `canStart` the stat engine needs, and does nothing else (its stat
/// functions still apply: they belong to the template, not to the handler class).
#[derive(Debug, Clone, Copy)]
pub struct Unported {
    pub name: &'static str,
    instant: bool,
    effect_type: EffectType,
    start: StartRule,
}

impl EffectHandler for Unported {
    fn effect_type(&self) -> EffectType {
        self.effect_type
    }

    fn is_instant(&self) -> bool {
        self.instant
    }

    fn can_start(&self, world: &World, cx: &EffectCx<'_>) -> bool {
        if self.start == StartRule::Always {
            return true;
        }
        let Some(view) = world.stats(cx.effected) else {
            return false;
        };
        match self.start {
            StartRule::Always => true,
            StartRule::Attackable => view.is_attackable(),
            StartRule::Player => view.is_player(),
            StartRule::Playable => view.is_playable(),
            StartRule::Summon => world.is_player(cx.effector) && world.is_summon(cx.effected),
            StartRule::Fear => {
                view.is_player()
                    || view.is_summon()
                    || (view.is_attackable()
                        && !view.npc_template().is_some_and(|t| {
                            matches!(t.npc_type.as_str(), "Defender" | "FortCommander" | "SiegeFlag")
                                || t.race == l2_data::npc::NpcRace::SiegeWeapon
                        }))
            }
        }
    }
}

/// One row of Java's `EffectMasterHandler.EFFECTS`: the class name, `isInstant()`,
/// `getEffectType()` and the `canStart` rule.
type JavaEffect = (&'static str, bool, EffectType, StartRule);

/// Every handler class `EffectMasterHandler` registers (165), in its order. The ported ones are
/// replaced in [`EffectRegistry::new`] by `handlers::register_all`.
#[rustfmt::skip]
const JAVA_EFFECTS: &[JavaEffect] = {
    use EffectType as T;
    use StartRule as S;
    &[
        ("AddHate", true, T::None, S::Always),
        ("ApplySkillEffects", true, T::None, S::Always),
        ("AttackTrait", false, T::None, S::Always),
        ("Backstab", true, T::PhysicalAttack, S::Always),
        ("Betray", false, T::Debuff, S::Summon),
        ("Blink", true, T::None, S::Always),
        ("BlockAction", false, T::None, S::Player),
        ("BlockAbnormalSlot", false, T::None, S::Always),
        ("BlockChat", false, T::ChatBlock, S::Player),
        ("BlockParty", false, T::None, S::Player),
        ("BlockResurrection", false, T::None, S::Always),
        ("Bluff", true, T::None, S::Always),
        ("Buff", false, T::Buff, S::Always),
        ("CallParty", true, T::None, S::Always),
        ("CallPc", true, T::None, S::Always),
        ("CallSkill", true, T::None, S::Always),
        ("ChameleonRest", false, T::Relaxing, S::Always),
        ("ChangeFace", true, T::None, S::Always),
        ("ChangeFishingMastery", false, T::None, S::Always),
        ("ChangeHairColor", true, T::None, S::Always),
        ("ChangeHairStyle", true, T::None, S::Always),
        ("ClanGate", false, T::None, S::Always),
        ("Confuse", true, T::None, S::Always),
        ("ConsumeBody", true, T::None, S::Always),
        ("ConvertItem", true, T::None, S::Always),
        ("CpDamPercent", true, T::None, S::Always),
        ("CpHeal", true, T::Cpheal, S::Always),
        ("CpHealOverTime", false, T::None, S::Always),
        ("CpHealPercent", true, T::None, S::Always),
        ("CrystalGradeModify", false, T::None, S::Player),
        ("CubicMastery", false, T::None, S::Player),
        ("DamOverTime", false, T::DmgOverTime, S::Always),
        ("DamOverTimePercent", false, T::DmgOverTimePercent, S::Always),
        ("DeathLink", true, T::DeathLink, S::Always),
        ("Debuff", false, T::Debuff, S::Always),
        ("DefenceTrait", false, T::None, S::Always),
        ("DeleteHate", true, T::Hate, S::Always),
        ("DeleteHateOfMe", true, T::Hate, S::Always),
        ("DetectHiddenObjects", true, T::None, S::Always),
        ("Detection", true, T::None, S::Always),
        ("Disarm", false, T::None, S::Player),
        ("DispelAll", true, T::Dispel, S::Always),
        ("DispelByCategory", true, T::Dispel, S::Always),
        ("DispelBySlot", true, T::DispelBySlot, S::Always),
        ("DispelBySlotProbability", true, T::Dispel, S::Always),
        ("Distrust", false, T::None, S::Always),
        ("EnableCloak", false, T::None, S::Player),
        ("EnemyCharge", true, T::None, S::Always),
        ("EnergyDamage", true, T::PhysicalAttack, S::Always),
        ("EnlargeAbnormalSlot", false, T::None, S::Player),
        ("Escape", true, T::Teleport, S::Always),
        ("FakeDeath", false, T::FakeDeath, S::Always),
        ("FatalBlow", true, T::PhysicalAttack, S::Always),
        ("Fear", false, T::Fear, S::Fear),
        ("Fishing", true, T::FishingStart, S::Always),
        ("Flag", false, T::None, S::Player),
        ("FlipBlock", true, T::None, S::Always),
        ("FocusEnergy", true, T::None, S::Always),
        ("FocusMaxEnergy", true, T::None, S::Always),
        ("FocusSouls", true, T::None, S::Always),
        ("GetAgro", true, T::Aggression, S::Always),
        ("GiveRecommendation", true, T::None, S::Always),
        ("GiveSp", true, T::None, S::Always),
        ("Grow", false, T::Buff, S::Always),
        ("Harvesting", true, T::None, S::Always),
        ("HeadquarterCreate", true, T::None, S::Always),
        ("Heal", true, T::Heal, S::Always),
        ("HealOverTime", false, T::None, S::Always),
        ("HealPercent", true, T::Heal, S::Always),
        ("Hide", false, T::None, S::Always),
        ("HpByLevel", true, T::Buff, S::Always),
        ("HpDrain", true, T::HpDrain, S::Always),
        ("ImmobileBuff", false, T::Buff, S::Always),
        ("ImmobilePetBuff", false, T::Buff, S::Always),
        ("Invincible", false, T::None, S::Always),
        ("Lethal", true, T::None, S::Always),
        ("Lucky", false, T::None, S::Player),
        ("MagicalDamage", true, T::MagicalAttack, S::Always),
        ("MagicalDamageByAbnormal", true, T::MagicalAttack, S::Always),
        ("MagicalDamageMp", true, T::MagicalAttack, S::Always),
        ("MagicalSoulDamage", true, T::MagicalAttack, S::Always),
        ("ManaDamOverTime", false, T::None, S::Always),
        ("ManaHeal", true, T::None, S::Always),
        ("ManaHealByLevel", true, T::ManahealByLevel, S::Always),
        ("ManaHealOverTime", false, T::None, S::Always),
        ("ManaHealPercent", true, T::ManahealPercent, S::Always),
        ("MaxCp", false, T::None, S::Always),
        ("MaxHp", false, T::None, S::Always),
        ("MpConsumePerLevel", false, T::None, S::Always),
        ("Mute", false, T::Mute, S::Always),
        ("NevitsHourglass", false, T::NevitsHourglass, S::Player),
        ("NoblesseBless", false, T::NoblesseBlessing, S::Playable),
        ("OpenChest", true, T::None, S::Always),
        ("OpenCommonRecipeBook", true, T::None, S::Always),
        ("OpenDoor", true, T::None, S::Always),
        ("OpenDwarfRecipeBook", true, T::None, S::Always),
        ("OutpostCreate", true, T::None, S::Always),
        ("OutpostDestroy", true, T::None, S::Always),
        ("Paralyze", false, T::Paralyze, S::Always),
        ("Passive", false, T::None, S::Attackable),
        ("Petrification", false, T::None, S::Always),
        ("PhysicalDamage", true, T::PhysicalAttack, S::Always),
        ("PhysicalDamageHpLink", true, T::PhysicalAttackHpLink, S::Always),
        ("PhysicalDamageMute", false, T::None, S::Always),
        ("PhysicalMute", false, T::None, S::Always),
        ("PhysicalSoulDamage", true, T::PhysicalAttack, S::Always),
        ("PolearmSingleTarget", false, T::None, S::Always),
        ("ProtectionBlessing", false, T::None, S::Player),
        ("Pumping", true, T::Fishing, S::Always),
        ("RandomizeHate", true, T::None, S::Always),
        ("RebalanceHP", true, T::RebalanceHp, S::Always),
        ("Recovery", true, T::None, S::Always),
        ("Reeling", true, T::Fishing, S::Always),
        ("RefuelAirship", true, T::RefuelAirship, S::Always),
        ("Relax", false, T::Relaxing, S::Always),
        ("ResistSkill", false, T::Buff, S::Always),
        ("Restoration", true, T::None, S::Always),
        ("RestorationRandom", true, T::None, S::Always),
        ("Resurrection", true, T::Resurrection, S::Always),
        ("ResurrectionSpecial", false, T::ResurrectionSpecial, S::Always),
        ("Ride", true, T::None, S::Always),
        ("Root", false, T::Root, S::Always),
        ("RunAway", true, T::None, S::Always),
        ("ServitorShare", false, T::Buff, S::Always),
        ("SetSkill", true, T::None, S::Always),
        ("SilentMove", false, T::None, S::Always),
        ("SkillTurning", true, T::None, S::Always),
        ("Sleep", false, T::Sleep, S::Always),
        ("SoulBlow", true, T::PhysicalAttack, S::Always),
        ("SoulEating", false, T::None, S::Always),
        ("Sow", true, T::None, S::Always),
        ("Spoil", true, T::None, S::Always),
        ("StaticDamage", true, T::None, S::Always),
        ("StealAbnormal", true, T::StealAbnormal, S::Always),
        ("Stun", false, T::Stun, S::Always),
        ("Summon", true, T::None, S::Always),
        ("SummonAgathion", true, T::None, S::Always),
        ("SummonCubic", true, T::None, S::Always),
        ("SummonNpc", true, T::SummonNpc, S::Always),
        ("SummonPet", true, T::SummonPet, S::Always),
        ("SummonTrap", true, T::None, S::Always),
        ("Sweeper", true, T::None, S::Always),
        ("TakeCastle", true, T::None, S::Always),
        ("TakeFort", true, T::None, S::Always),
        ("TakeFortStart", true, T::None, S::Always),
        ("TakeTerritoryFlag", true, T::None, S::Always),
        ("TalismanSlot", false, T::None, S::Player),
        ("TargetCancel", true, T::None, S::Always),
        ("TargetMe", false, T::None, S::Always),
        ("TargetMeProbability", true, T::None, S::Always),
        ("Teleport", true, T::Teleport, S::Always),
        // Java: `canStart` = `GeoEngine.canSeeTarget(effected, effector)`; the effect is instant,
        // so `applyEffectScope` never asks it.
        ("TeleportToTarget", true, T::TeleportToTarget, S::Always),
        ("ThrowUp", true, T::None, S::Always),
        ("TransferDamage", false, T::None, S::Always),
        ("TransferHate", true, T::None, S::Always),
        ("Transformation", false, T::None, S::Player),
        ("TrapDetect", true, T::None, S::Always),
        ("TrapRemove", true, T::None, S::Always),
        ("TriggerSkillByAvoid", false, T::None, S::Always),
        ("TriggerSkillByDamageDealt", false, T::None, S::Always),
        ("TriggerSkillByDamageReceived", false, T::None, S::Always),
        ("TriggerSkillBySkill", false, T::None, S::Always),
        ("Unsummon", true, T::None, S::Always),
        ("UnsummonAgathion", true, T::None, S::Always),
        ("VitalityPointUp", true, T::None, S::Always),
    ]
};

/// How a name is known to the registry.
#[derive(Clone, Copy)]
enum Entry {
    /// A ported handler.
    Ported(EffectCtor),
    /// A Java handler without a Rust port yet.
    Unported(Unported),
}

/// Java `EffectHandler`: every effect handler by its Java class name.
pub struct EffectRegistry {
    entries: HashMap<&'static str, Entry>,
}

impl fmt::Debug for EffectRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectRegistry")
            .field("handlers", &self.entries.len())
            .field("ported", &self.ported_count())
            .finish()
    }
}

impl Default for EffectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// What [`EffectRegistry::create`] made of a template.
pub(crate) enum Created {
    /// A ported handler.
    Ported(Arc<dyn EffectHandler>),
    /// The stand-in of a Java handler that is not ported yet.
    Unported(Arc<dyn EffectHandler>),
    /// Java `createEffect` returned `null`: no such handler, or its constructor failed.
    Failed(String),
}

impl EffectRegistry {
    /// Java `EffectMasterHandler.main`: every Java class, with the ported ones in place.
    pub fn new() -> Self {
        let mut registry = Self {
            entries: JAVA_EFFECTS
                .iter()
                .map(|&(name, instant, effect_type, start)| {
                    (
                        name,
                        Entry::Unported(Unported {
                            name,
                            instant,
                            effect_type,
                            start,
                        }),
                    )
                })
                .collect(),
        };
        super::handlers::register_all(&mut registry);
        registry
    }

    /// Java `EffectHandler.registerHandler`: `name` is built by `ctor` from now on.
    pub fn register(&mut self, name: &'static str, ctor: EffectCtor) {
        self.entries.insert(name, Entry::Ported(ctor));
    }

    /// Java `EffectHandler.size()`.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many names have a Rust handler.
    pub fn ported_count(&self) -> usize {
        self.entries
            .values()
            .filter(|e| matches!(e, Entry::Ported(_)))
            .count()
    }

    /// Whether `name` has a Rust handler (not just the stand-in).
    pub fn is_ported(&self, name: &str) -> bool {
        matches!(self.entries.get(name), Some(Entry::Ported(_)))
    }

    /// Whether Java knows a handler of this name.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// Java `AbstractEffect.createEffect`: the handler of one template.
    pub(crate) fn create(&self, template: &EffectTemplate) -> Created {
        match self.entries.get(template.name.as_str()) {
            Some(Entry::Ported(ctor)) => match ctor(template) {
                Ok(handler) => Created::Ported(Arc::from(handler)),
                Err(error) => Created::Failed(format!(
                    "AbstractEffect: Unable to initialize effect handler: {}: {error}",
                    template.name
                )),
            },
            Some(Entry::Unported(stub)) => Created::Unported(Arc::new(*stub)),
            None => Created::Failed(format!(
                "AbstractEffect: Requested unexistent effect handler: {}",
                template.name
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_knows_every_java_handler() {
        let registry = EffectRegistry::new();
        assert_eq!(registry.len(), 165);
        assert!(registry.is_ported("Buff"));
        // Cubics are M6-CUBIC-01: this one stays a stand-in until then.
        assert!(registry.contains("SummonCubic"));
        assert!(!registry.is_ported("SummonCubic"));
        assert!(!registry.contains("NoSuchEffect"));
        let mut names: Vec<&str> = JAVA_EFFECTS.iter().map(|e| e.0).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), JAVA_EFFECTS.len(), "no name twice");
    }

    #[test]
    fn flags_are_one_bit_per_ordinal() {
        assert_eq!(effect_flag::NONE, 1);
        assert_eq!(effect_flag::INVUL, 0x20000);
        assert_eq!(effect_flag::POLEARM_SINGLE_TARGET, 1 << 21);
    }
}
