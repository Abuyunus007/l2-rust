//! The effects that raise the maximum HP or CP: `MaxHp` and `MaxCp`.
// Java: dist/game/data/scripts/handlers/skill/effects/MaxHp.java
// Java: dist/game/data/scripts/handlers/skill/effects/MaxCp.java

use l2_data::skill::EffectTemplate;
use l2_data::{FuncKind, Stat};
use tracing::warn;

use super::statset::{get_boolean, get_enum, get_int};
use super::vitals::Vital;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::stats::calculators::{Func, FuncCalc, FuncOwner};
use crate::world::World;

/// Java `EffectCalculationType`: the power is an amount (`DIFF`) or a percentage (`PER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalculationType {
    Diff,
    Per,
}

/// What `MaxHp` and `MaxCp` share; they differ in the stat and the vital only.
#[derive(Debug, Clone, Copy)]
struct MaxVital {
    vital: Vital,
    stat: Stat,
    power: f64,
    kind: CalculationType,
    heal: bool,
}

impl MaxVital {
    /// The constructors of `MaxHp` and `MaxCp`.
    fn new(template: &EffectTemplate, name: &str, vital: Vital, stat: Stat) -> Result<Self, String> {
        let kind = get_enum(
            template,
            "type",
            &[("DIFF", CalculationType::Diff), ("PER", CalculationType::Per)],
            CalculationType::Diff,
        )?;
        let power = match kind {
            CalculationType::Diff => f64::from(get_int(template, "power", 0)?),
            CalculationType::Per => 1.0 + (f64::from(get_int(template, "power", 0)?) / 100.0),
        };
        let heal = get_boolean(template, "heal", false);
        if template.params.is_empty() {
            warn!("{name}: must have parameters.");
        }
        Ok(Self {
            vital,
            stat,
            power,
            kind,
            heal,
        })
    }

    /// The owner of the function this effect adds: Java passes `this`, the same object that owns
    /// the effect's template functions.
    fn owner(cx: &EffectCx<'_>) -> FuncOwner {
        let index = cx
            .skill
            .effects
            .get(cx.effect.scope)
            .iter()
            .position(|e| std::ptr::eq(e, cx.effect))
            .unwrap_or(0);
        FuncOwner::Effect {
            skill_id: cx.skill.id,
            skill_level: cx.skill.level,
            scope: cx.effect.scope,
            index: u16::try_from(index).unwrap_or(u16::MAX),
        }
    }

    /// Java `onStart`.
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effected = cx.effected;
        let current = self.vital.current(world, effected);
        let mut amount = self.power;
        let owner = Self::owner(cx);
        match self.kind {
            CalculationType::Diff => {
                let func = Func {
                    stat: self.stat,
                    order: 1,
                    calc: FuncCalc::Template(FuncKind::Add),
                    value: self.power,
                    owner,
                    cond: None,
                };
                world.add_stat_funcs(effected, vec![func], false);
                if self.heal {
                    self.vital.set(world, effected, current + self.power);
                }
            }
            CalculationType::Per => {
                let max = self.vital.max(world, effected);
                let func = Func {
                    stat: self.stat,
                    order: 1,
                    calc: FuncCalc::Template(FuncKind::Mul),
                    value: self.power,
                    owner,
                    cond: None,
                };
                world.add_stat_funcs(effected, vec![func], false);
                if self.heal {
                    amount = (self.power - 1.0) * max;
                    self.vital.set(world, effected, current + amount);
                }
            }
        }
        if self.heal {
            self.vital
                .send_restored(world, cx.effector, effected, cx.effector != effected, amount);
        }
    }
}

/// Java `MaxHp`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MaxHp(MaxVital);

impl MaxHp {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(MaxHp(MaxVital::new(
            template,
            "MaxHp",
            Vital::Hp,
            Stat::MaxHp,
        )?)))
    }
}

impl EffectHandler for MaxHp {
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        self.0.on_start(world, cx);
    }

    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.remove_stats_owner(cx.effected, MaxVital::owner(cx), false);
        // Ensure current HP does not exceed the new maximum after removing the effect.
        let max_hp = Vital::Hp.max(world, cx.effected);
        if Vital::Hp.current(world, cx.effected) > max_hp {
            world.set_current_hp(cx.effected, max_hp, true);
        }
    }
}

/// Java `MaxCp`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MaxCp(MaxVital);

impl MaxCp {
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(MaxCp(MaxVital::new(
            template,
            "MaxCp",
            Vital::Cp,
            Stat::MaxCp,
        )?)))
    }
}

impl EffectHandler for MaxCp {
    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        self.0.on_start(world, cx);
    }

    // NOTE(java-quirk): unlike `MaxHp`, the current CP is not pulled down to the new maximum.
    fn on_exit(&self, world: &mut World, cx: &EffectCx<'_>) {
        world.remove_stats_owner(cx.effected, MaxVital::owner(cx), false);
    }
}
