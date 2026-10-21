//! The skill formulas with their inputs taken from the stat engine: what `Formulas.calcPhysDam
//! (.., skill, ..)`, `calcBlowDamage`, `calcBackstabDamage`, `calcCrit(.., skill)`, `calcShldUse
//! (.., skill)`, `calcBlowSuccess`, `calcPhysicalSkillEvasion`, `calcMagicDam`, `calcManaDam`,
//! `calcMagicAffected` and `calcDamageReflected` read from the creatures, and the messages Java
//! sends from inside them.
// Java: mechanics/stats/Formulas.java (the methods above)
// Java: entity/actor/stat/CreatureStat.java (getMCriticalHit)
//
// The numbers are M5-FORM-02's pure functions (`crate::formulas`, checked against Java vectors);
// this file only fills their inputs, the way `combat/hit.rs` does for a normal attack. Every draw
// goes through the world RNG in Java's order.
//
// Left out, each with its marker:
// - `M5-SHOT-01`: the soulshot / spiritshot charge (the callers pass `false`).
// The `ClassBalanceConfig` tables come from `StatSettings::class_balance` (M5-FIX-03).

use l2_data::SkillTemplate;
use l2_data::Stat;
use l2_data::item::WeaponType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::combat::attacker_position;
use crate::combat::hit::calc_shld_use_for;
use crate::effects::apply::has_negative_effect;
use crate::effects::registry::EffectType;
use crate::formulas::{
    BlowClassBalance, BlowInput, BlowSuccessInput, CounterDamageInput, MagicAffectedInput,
    MagicDamClassBalance, MagicDamInput, MagicDamResult, MagicFailure, MagicSuccessInput, ManaDamInput,
    ManaDamResult, Position, ReflectChanceInput, SkillCritInput, SkillPhysDamClassBalance, SkillPhysDamInput,
    SkillPower, calc_attack_trait_bonus, calc_backstab_damage, calc_blow_damage, calc_blow_success,
    calc_counter_damage, calc_damage_reflected, calc_magic_affected, calc_magic_dam, calc_mana_dam,
    calc_mcrit, calc_physical_skill_evasion, calc_skill_crit, calc_skill_phys_dam, calc_weapon_trait_bonus,
};
use crate::stats::skills::calc_skill;
use crate::stats::{BaseStat, ClassBalance, StatView};
use crate::world::World;

/// Which creature's HP `Skill.getPower(creature, target, isPvP, isPvE)` scales the power with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HpLink {
    /// No `DEATH_LINK` and no `PHYSICAL_ATTACK_HP_LINK` effect in the skill.
    None,
    /// `hasEffectType(DEATH_LINK)`: the attacker's HP.
    Attacker,
    /// `hasEffectType(PHYSICAL_ATTACK_HP_LINK)`: the target's HP.
    Target,
}

/// The shot and critical flags `calcMagicDam` and `calcManaDam` take after the shield.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MagicFlags {
    /// `sps`: a spiritshot is charged.
    pub sps: bool,
    /// `bss`: a blessed spiritshot is charged.
    pub bss: bool,
    /// `mcrit`: the magic critical hit rolled.
    pub mcrit: bool,
}

/// The class id the `ClassBalanceConfig` arrays are indexed with (`asPlayer().getPlayerClass()`).
fn class_id(view: &StatView<'_>) -> i32 {
    view.player().map_or(-1, |p| p.data.class_id)
}

/// Java `isRaidMinion()`: a minion whose leader is a raid boss.
fn is_raid_minion(world: &World, view: &StatView<'_>) -> bool {
    view.npc()
        .and_then(|n| n.leader)
        .is_some_and(|leader| world.is_raid(leader))
}

/// `getActiveWeaponItem()` is a bow or a crossbow.
fn uses_bow(view: &StatView<'_>) -> bool {
    matches!(
        view.active_weapon_item()
            .and_then(|t| t.weapon())
            .map(|w| w.weapon_type),
        Some(WeaponType::Bow | WeaponType::CrossBow)
    )
}

impl World {
    /// Whether the skill has a `DEATH_LINK` or `PHYSICAL_ATTACK_HP_LINK` effect (the two cases of
    /// `Skill.getPower(creature, target, ..)`).
    pub(super) fn skill_hp_link(&mut self, skill: &SkillTemplate) -> HpLink {
        if self.skill_has_effect_type(skill, &[EffectType::DeathLink]) {
            HpLink::Attacker
        } else if self.skill_has_effect_type(skill, &[EffectType::PhysicalAttackHpLink]) {
            HpLink::Target
        } else {
            HpLink::None
        }
    }

    /// Java `Skill.getPower(attacker, target, isPvP, isPvE)` as the formulas take it.
    fn skill_power_for(
        &self,
        skill: &SkillTemplate,
        attacker: ObjectId,
        target: ObjectId,
        link: HpLink,
    ) -> SkillPower {
        let vitals = |id: ObjectId| {
            self.core_of(id)
                .zip(self.stats(id))
                .map(|(core, view)| (core.cur_hp, view.max_hp()))
        };
        SkillPower {
            power: f64::from(skill.power),
            pvp_power: f64::from(skill.pvp_power),
            pve_power: f64::from(skill.pve_power),
            hp_link: match link {
                HpLink::None => None,
                HpLink::Attacker => vitals(attacker),
                HpLink::Target => vitals(target),
            },
        }
    }

    /// Java `Skill.getPower(isPvP, isPvE)` (the blow and backstab overload: no HP link).
    fn flat_skill_power(skill: &SkillTemplate) -> SkillPower {
        SkillPower {
            power: f64::from(skill.power),
            pvp_power: f64::from(skill.pvp_power),
            pve_power: f64::from(skill.pve_power),
            hp_link: None,
        }
    }

    // ------------------------------------------------------------------ evasion, shield, crit

    /// Java `Formulas.calcPhysicalSkillEvasion(creature, target, skill)` with its two messages.
    pub(super) fn calc_physical_skill_evasion_for(
        &mut self,
        creature: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> bool {
        let ctx = calc_skill(skill);
        let evasion = self
            .stats(target)
            .map_or(0.0, |t| t.calc_stat(Stat::PSkillEvasion, 0.0, None, Some(&ctx)));
        let evaded = calc_physical_skill_evasion(skill.is_magic(), skill.is_debuff, evasion, self.rng());
        if evaded {
            if self.is_player(creature) {
                self.send(
                    creature,
                    &SystemMessage::new(SystemMessageId::C1DodgesTheAttack)
                        .with(SystemMessageParam::Text(self.creature_name(target))),
                );
            }
            if self.is_player(target) {
                self.send(
                    target,
                    &SystemMessage::new(SystemMessageId::YouHaveAvoidedC1SAttack2)
                        .with(SystemMessageParam::Text(self.creature_name(creature))),
                );
            }
        }
        evaded
    }

    /// Java `Formulas.calcShldUse(attacker, target, skill, sendSysMsg)`.
    pub(crate) fn calc_shld_use_skill(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        send_sys_msg: bool,
    ) -> u8 {
        if skill.ignore_shield {
            return 0;
        }
        let shld = self.with_rng(|world, rng| calc_shld_use_for(world, attacker, target, rng));
        if send_sys_msg && self.is_player(target) {
            let message = match shld {
                crate::formulas::SHIELD_DEFENSE_SUCCEED => {
                    Some(SystemMessageId::YourShieldDefenseHasSucceeded)
                }
                crate::formulas::SHIELD_DEFENSE_PERFECT_BLOCK => {
                    Some(SystemMessageId::YourExcellentShieldDefenseWasASuccess)
                }
                _ => None,
            };
            if let Some(id) = message {
                self.send(target, &SystemMessage::new(id));
            }
        }
        shld
    }

    /// Java `Formulas.calcCrit(attacker, target, skill)` for a skill.
    pub(super) fn calc_skill_crit_for(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> bool {
        self.with_rng(|world, rng| {
            let (Some(a), Some(t)) = (world.stats(attacker), world.stats(target)) else {
                return false;
            };
            let defence = |rate: f64| t.calc_stat(Stat::DefenceCriticalRate, rate, None, None);
            let cb = &world.settings.stats.class_balance;
            let input = SkillCritInput {
                base_crit_rate: skill.base_crit_rate,
                str_bonus: a.bonus(BaseStat::Str),
                attacker_is_playable: a.is_playable(),
                target_is_playable: t.is_playable(),
                target_is_attackable: t.is_attackable(),
                pvp_physical_skill_critical_chance_multiplier: ClassBalance::of(
                    &cb.pvp_physical_skill_critical_chance,
                    class_id(&a),
                ),
                pve_physical_skill_critical_chance_multiplier: ClassBalance::of(
                    &cb.pve_physical_skill_critical_chance,
                    class_id(&a),
                ),
                defence_critical_rate: &defence,
                defence_critical_rate_add: t.calc(Stat::DefenceCriticalRateAdd, 0.0),
            };
            calc_skill_crit(&input, rng)
        })
    }

    /// Java `Formulas.calcBlowSuccess(creature, target, skill)`.
    pub(super) fn calc_blow_success_for(
        &mut self,
        creature: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> bool {
        self.with_rng(|world, rng| {
            let Some(c) = world.stats(creature) else {
                return false;
            };
            let blow_rate = |rate: f64| c.calc_stat(Stat::BlowRate, rate, Some(target), None);
            let input = BlowSuccessInput {
                blow_chance: skill.blow_chance,
                dex_bonus: c.bonus(BaseStat::Dex),
                position: attacker_position(world, creature, target),
                blow_rate: &blow_rate,
            };
            calc_blow_success(&input, rng)
        })
    }

    // ------------------------------------------------------------------ physical damage

    /// Java `Formulas.calcPhysDam(attacker, target, skill, shld, crit, ss)` for a skill.
    pub(super) fn calc_skill_phys_dam_for(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        shld: u8,
        crit: bool,
        ss: bool,
    ) -> f64 {
        let link = self.skill_hp_link(skill);
        let skill_power = self.skill_power_for(skill, attacker, target, link);
        let ctx = calc_skill(skill);
        self.with_rng(|world, rng| {
            let (Some(a), Some(t)) = (world.stats(attacker), world.stats(target)) else {
                return 0.0;
            };
            let combat = &world.settings.combat;
            let cb = &world.settings.stats.class_balance;
            let (weapon_defence_trait, weaknesses) = world.attack_trait_inputs(attacker, target);
            let power = |damage: f64| a.calc_stat(Stat::PhysicalSkillPower, damage, None, None);
            let input = SkillPhysDamInput {
                attacker_is_playable: a.is_playable(),
                target_is_playable: t.is_playable(),
                target_is_attackable: t.is_attackable(),
                position: attacker_position(world, attacker, target),
                p_atk: a.p_atk(Some(target)),
                p_def: t.p_def(Some(attacker)),
                shld_def: t.shld_def(),
                pvp_phys_skill_def: t.calc(Stat::PvpPhysSkillDef, 1.0),
                skill_power,
                critical_damage: a.calc_stat(Stat::CriticalDamage, 1.0, Some(target), Some(&ctx)),
                critical_damage_pos: a.calc_stat(Stat::CriticalDamagePos, 1.0, Some(target), Some(&ctx)),
                // NOTE(java-quirk): the defence critical damage is asked without the skill here
                // (the blow formulas pass it).
                defence_critical_damage: t.calc_stat(Stat::DefenceCriticalDamage, 1.0, Some(target), None),
                critical_damage_add: a.calc_stat(Stat::CriticalDamageAdd, 0.0, Some(target), Some(&ctx)),
                defence_critical_damage_add: t.calc_stat(
                    Stat::DefenceCriticalDamageAdd,
                    0.0,
                    Some(target),
                    Some(&ctx),
                ),
                attack_trait_bonus: calc_attack_trait_bonus(weapon_defence_trait, &weaknesses),
                randomize_physical_skill_damage: combat.randomize_physical_skill_damage,
                attacker_has_active_weapon: a.active_weapon_item().is_some(),
                random_damage: a.calc(Stat::RandomDamage, 0.0) as i32,
                attacker_level: a.level(),
                pvp_phys_skill_dmg: a.calc(Stat::PvpPhysSkillDmg, 1.0),
                physical_skill_power: &power,
                attribute_bonus: world.calc_attribute_bonus_for_effect(attacker, target, skill),
                attacker_weapon_is_bow_or_crossbow: uses_bow(&a),
                pve_bow_skill_dmg: a.calc(Stat::PveBowSkillDmg, 1.0),
                pve_physical_dmg: a.calc(Stat::PvePhysicalDmg, 1.0),
                target_is_raid: t.is_raid(),
                target_is_raid_minion: is_raid_minion(world, &t),
                target_level: t.level(),
                attacker_player_level: a.player().map(|p| p.core.level),
                class_balance: SkillPhysDamClassBalance {
                    pvp_physical_skill_defence: ClassBalance::of(
                        &cb.pvp_physical_skill_defence,
                        class_id(&t),
                    ),
                    pvp_physical_skill_critical_damage: ClassBalance::of(
                        &cb.pvp_physical_skill_critical_damage,
                        class_id(&a),
                    ),
                    pve_physical_skill_critical_damage: ClassBalance::of(
                        &cb.pve_physical_skill_critical_damage,
                        class_id(&a),
                    ),
                    pvp_physical_skill_damage: ClassBalance::of(&cb.pvp_physical_skill_damage, class_id(&a)),
                    pve_physical_skill_damage: ClassBalance::of(&cb.pve_physical_skill_damage, class_id(&a)),
                },
                alt_game_shield_blocks: combat.alt_game_shield_blocks,
                min_npc_level_dmg_penalty: combat.min_npc_level_dmg_penalty,
                npc_skill_dmg_penalty: &combat.npc_skill_dmg_penalty,
            };
            calc_skill_phys_dam(&input, shld, crit, ss, rng)
        })
    }

    /// The inputs `calcBlowDamage` and `calcBackstabDamage` share.
    fn blow_input<'w>(
        world: &'w World,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        backstab: bool,
    ) -> Option<BlowInput<'w>> {
        let (a, t) = (world.stats(attacker)?, world.stats(target)?);
        let ctx = calc_skill(skill);
        let combat = &world.settings.combat;
        let cb = &world.settings.stats.class_balance;
        let (weapon_defence_trait, _) = world.attack_trait_inputs(attacker, target);
        Some(BlowInput {
            attacker_is_playable: a.is_playable(),
            attacker_is_attackable: a.is_attackable(),
            target_is_playable: t.is_playable(),
            target_is_player: t.is_player(),
            target_is_attackable: t.is_attackable(),
            position: attacker_position(world, attacker, target),
            p_atk: a.p_atk(Some(target)),
            p_def: t.p_def(Some(attacker)),
            shld_def: t.shld_def(),
            skill_power: Self::flat_skill_power(skill),
            pvp_phys_skill_dmg: a.calc(Stat::PvpPhysSkillDmg, 1.0),
            pvp_phys_skill_def: t.calc(Stat::PvpPhysSkillDef, 1.0),
            // A blow reads the `*_BLOW_SKILL_*` tables, a backstab the `PVP_PHYSICAL_SKILL_*` ones
            // (it has no PvE entries).
            class_balance: if backstab {
                BlowClassBalance {
                    pvp_skill_damage: ClassBalance::of(&cb.pvp_physical_skill_damage, class_id(&a)),
                    pvp_skill_defence: ClassBalance::of(&cb.pvp_physical_skill_defence, class_id(&t)),
                    ..BlowClassBalance::default()
                }
            } else {
                BlowClassBalance {
                    pvp_skill_damage: ClassBalance::of(&cb.pvp_blow_skill_damage, class_id(&a)),
                    pvp_skill_defence: ClassBalance::of(&cb.pvp_blow_skill_defence, class_id(&t)),
                    pve_skill_damage: ClassBalance::of(&cb.pve_blow_skill_damage, class_id(&a)),
                    pve_skill_defence: ClassBalance::of(&cb.pve_blow_skill_defence, class_id(&t)),
                }
            },
            critical_damage: a.calc_stat(Stat::CriticalDamage, 1.0, Some(target), Some(&ctx)),
            critical_damage_pos: a.calc_stat(Stat::CriticalDamagePos, 1.0, Some(target), Some(&ctx)),
            defence_critical_damage: t.calc_stat(Stat::DefenceCriticalDamage, 1.0, Some(target), Some(&ctx)),
            // A blow asks without target and skill, a backstab with both.
            critical_damage_add: if backstab {
                a.calc_stat(Stat::CriticalDamageAdd, 0.0, Some(target), Some(&ctx))
            } else {
                a.calc(Stat::CriticalDamageAdd, 0.0)
            },
            defence_critical_damage_add: t.calc_stat(
                Stat::DefenceCriticalDamageAdd,
                0.0,
                Some(target),
                Some(&ctx),
            ),
            weapon_trait_bonus: if backstab {
                1.0
            } else {
                calc_weapon_trait_bonus(weapon_defence_trait)
            },
            general_trait_bonus: world.calc_general_trait_bonus(attacker, target, skill.trait_type, false),
            attribute_bonus: world.calc_attribute_bonus_for_effect(attacker, target, skill),
            randomize_physical_skill_damage: combat.randomize_physical_skill_damage,
            attacker_has_active_weapon: a.active_weapon_item().is_some(),
            random_damage: a.calc(Stat::RandomDamage, 0.0) as i32,
            attacker_level: a.level(),
            target_is_raid: t.is_raid(),
            target_is_raid_minion: is_raid_minion(world, &t),
            target_level: t.level(),
            attacker_player_level: a.player().map(|p| p.core.level),
            min_npc_level_dmg_penalty: combat.min_npc_level_dmg_penalty,
            npc_skill_dmg_penalty: &combat.npc_skill_dmg_penalty,
        })
    }

    /// Java `Formulas.calcBlowDamage(attacker, target, skill, shld, ss)`.
    pub(super) fn calc_blow_damage_for(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        shld: u8,
        ss: bool,
    ) -> f64 {
        self.with_rng(|world, rng| {
            Self::blow_input(world, attacker, target, skill, false)
                .map_or(0.0, |input| calc_blow_damage(&input, shld, ss, rng))
        })
    }

    /// Java `Formulas.calcBackstabDamage(attacker, target, skill, shld, ss)`.
    pub(super) fn calc_backstab_damage_for(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        shld: u8,
        ss: bool,
    ) -> f64 {
        self.with_rng(|world, rng| {
            Self::blow_input(world, attacker, target, skill, true)
                .map_or(0.0, |input| calc_backstab_damage(&input, shld, ss, rng))
        })
    }

    // ------------------------------------------------------------------ magic

    /// Java `Formulas.calcMCrit(effector.getMCriticalHit(effected, skill))`.
    pub(crate) fn calc_mcrit_for(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> bool {
        let ctx = calc_skill(skill);
        self.with_rng(|world, rng| {
            let Some(a) = world.stats(attacker) else {
                return false;
            };
            calc_mcrit(f64::from(a.m_critical_hit(Some(target), Some(&ctx))), rng)
        })
    }

    /// The inputs of the `calcMagicSuccess` calls inside `calcMagicDam` and `calcManaDam`.
    pub(crate) fn magic_success_input<'w>(
        world: &'w World,
        a: &StatView<'w>,
        t: &StatView<'w>,
        skill: &SkillTemplate,
    ) -> MagicSuccessInput<'w> {
        let combat = &world.settings.combat;
        let ctx = calc_skill(skill);
        MagicSuccessInput {
            skill_power: f64::from(skill.power),
            skill_activate_rate: skill.activate_rate,
            calculate_magic_success_by_skill_magic_level: combat.calculate_magic_success_by_skill_magic_level,
            skill_magic_level: skill.magic_level,
            attacker_level: a.level(),
            target_level: t.level(),
            target_is_attackable: t.is_attackable(),
            target_is_raid: t.is_raid(),
            target_is_raid_minion: is_raid_minion(world, t),
            min_npc_level_magic_penalty: combat.min_npc_level_magic_penalty,
            attacker_player_level: a.player().map(|p| p.core.level),
            npc_skill_chance_penalty: &combat.npc_skill_chance_penalty,
            magic_success_res: t.calc_stat(Stat::MagicSuccessRes, 1.0, None, Some(&ctx)),
        }
    }

    /// Java `Formulas.calcMagicDam(attacker, target, skill, shld, sps, bss, mcrit)`, with the
    /// resist messages it sends.
    pub(super) fn calc_magic_dam_for(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        shld: u8,
        flags: MagicFlags,
    ) -> f64 {
        let link = self.skill_hp_link(skill);
        let skill_power = self.skill_power_for(skill, attacker, target, link);
        let is_drain = self.skill_has_effect_type(skill, &[EffectType::HpDrain]);
        let ctx = calc_skill(skill);
        let result = self.with_rng(|world, rng| {
            let (Some(a), Some(t)) = (world.stats(attacker), world.stats(target)) else {
                return None;
            };
            let combat = &world.settings.combat;
            let cb = &world.settings.stats.class_balance;
            let input = MagicDamInput {
                attacker_is_playable: a.is_playable(),
                attacker_is_player: a.is_player(),
                target_is_playable: t.is_playable(),
                target_is_player: t.is_player(),
                target_is_attackable: t.is_attackable(),
                m_def: t.m_def(Some(attacker), Some(&ctx)),
                shld_def: t.shld_def(),
                m_atk: a.m_atk(Some(target), Some(&ctx)),
                skill_is_magic: skill.is_magic(),
                skill_power,
                pvp_magical_def: t.calc(Stat::PvpMagicalDef, 1.0),
                pvp_phys_skill_def: t.calc(Stat::PvpPhysSkillDef, 1.0),
                alt_game_magicfailures: combat.alt_game_magicfailures,
                magic_success: Self::magic_success_input(world, &a, &t, skill),
                attacker_level: a.level(),
                magic_crit_dmg: a.calc(Stat::MagicCritDmg, 1.0),
                randomize_magical_skill_damage: combat.randomize_magical_skill_damage,
                attacker_has_active_weapon: a.active_weapon_item().is_some(),
                random_damage: a.calc(Stat::RandomDamage, 0.0) as i32,
                pvp_magical_dmg: a.calc(Stat::PvpMagicalDmg, 1.0),
                pvp_phys_skill_dmg: a.calc(Stat::PvpPhysSkillDmg, 1.0),
                attribute_bonus: world.calc_attribute_bonus_for_effect(attacker, target, skill),
                pve_magical_dmg: a.calc(Stat::PveMagicalDmg, 1.0),
                attacker_player_level: a.player().map(|p| p.core.level),
                target_is_raid: t.is_raid(),
                target_is_raid_minion: is_raid_minion(world, &t),
                target_level: t.level(),
                min_npc_level_dmg_penalty: combat.min_npc_level_dmg_penalty,
                npc_skill_dmg_penalty: &combat.npc_skill_dmg_penalty,
                class_balance: MagicDamClassBalance {
                    pvp_magical_skill_defence: ClassBalance::of(&cb.pvp_magical_skill_defence, class_id(&t)),
                    pvp_physical_skill_defence: ClassBalance::of(
                        &cb.pvp_physical_skill_defence,
                        class_id(&t),
                    ),
                    pvp_magical_skill_critical_damage: ClassBalance::of(
                        &cb.pvp_magical_skill_critical_damage,
                        class_id(&a),
                    ),
                    pve_magical_skill_critical_damage: ClassBalance::of(
                        &cb.pve_magical_skill_critical_damage,
                        class_id(&a),
                    ),
                    pvp_magical_skill_damage: ClassBalance::of(&cb.pvp_magical_skill_damage, class_id(&a)),
                    pvp_physical_skill_damage: ClassBalance::of(&cb.pvp_physical_skill_damage, class_id(&a)),
                    pve_magical_skill_damage: ClassBalance::of(&cb.pve_magical_skill_damage, class_id(&a)),
                },
            };
            Some(calc_magic_dam(
                &input,
                shld,
                flags.sps,
                flags.bss,
                flags.mcrit,
                rng,
            ))
        });
        let Some(MagicDamResult {
            damage,
            failed,
            failure,
        }) = result
        else {
            return 0.0;
        };
        // The messages Java sends from inside the formula, in its order.
        if failed {
            match failure {
                MagicFailure::Half => {
                    let id = if is_drain {
                        SystemMessageId::DrainWasOnly50PercentSuccessful
                    } else {
                        SystemMessageId::YourAttackHasFailed
                    };
                    self.send(attacker, &SystemMessage::new(id));
                }
                MagicFailure::Resisted => {
                    self.send(
                        attacker,
                        &SystemMessage::new(SystemMessageId::C1HasResistedYourS2)
                            .with(SystemMessageParam::Text(self.creature_name(target)))
                            .with(super::super::buff_info::skill_name_param(skill)),
                    );
                }
                MagicFailure::None => {}
            }
            if self.is_player(target) {
                let id = if is_drain {
                    SystemMessageId::YouResistedC1SDrain
                } else {
                    SystemMessageId::YouResistedC1SMagic
                };
                self.send(
                    target,
                    &SystemMessage::new(id).with(SystemMessageParam::Text(self.creature_name(attacker))),
                );
            }
        }
        damage
    }

    /// Java `Formulas.calcManaDam(attacker, target, skill, shld, sps, bss, mcrit)`, with its
    /// messages.
    pub(super) fn calc_mana_dam_for(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        shld: u8,
        flags: MagicFlags,
    ) -> f64 {
        let link = self.skill_hp_link(skill);
        let skill_power = self.skill_power_for(skill, attacker, target, link);
        let ctx = calc_skill(skill);
        let result = self.with_rng(|world, rng| {
            let (Some(a), Some(t)) = (world.stats(attacker), world.stats(target)) else {
                return None;
            };
            let combat = &world.settings.combat;
            let cb = &world.settings.stats.class_balance;
            let input = ManaDamInput {
                m_atk: a.m_atk(Some(target), Some(&ctx)),
                m_def: t.m_def(Some(attacker), Some(&ctx)),
                shld_def: t.shld_def(),
                target_max_mp: t.max_mp(),
                skill_power,
                attacker_is_playable: a.is_playable(),
                attacker_is_player: a.is_player(),
                target_is_playable: t.is_playable(),
                target_is_attackable: t.is_attackable(),
                general_trait_bonus: world.calc_general_trait_bonus(
                    attacker,
                    target,
                    skill.trait_type,
                    false,
                ),
                pve_magical_dmg: a.calc(Stat::PveMagicalDmg, 1.0),
                attacker_player_level: a.player().map(|p| p.core.level),
                pve_magical_skill_damage: ClassBalance::of(&cb.pve_magical_skill_damage, class_id(&a)),
                target_is_raid: t.is_raid(),
                target_is_raid_minion: is_raid_minion(world, &t),
                target_level: t.level(),
                min_npc_level_dmg_penalty: combat.min_npc_level_dmg_penalty,
                npc_skill_dmg_penalty: &combat.npc_skill_dmg_penalty,
                alt_game_magicfailures: combat.alt_game_magicfailures,
                magic_success: Self::magic_success_input(world, &a, &t, skill),
            };
            Some(calc_mana_dam(
                &input,
                shld,
                flags.sps,
                flags.bss,
                flags.mcrit,
                rng,
            ))
        });
        let Some(ManaDamResult { damage, resisted }) = result else {
            return 0.0;
        };
        if resisted {
            if self.is_player(attacker) {
                self.send(
                    attacker,
                    &SystemMessage::new(SystemMessageId::DamageIsDecreasedBecauseC1ResistedC2SMagic)
                        .with(SystemMessageParam::Text(self.creature_name(target)))
                        .with(SystemMessageParam::Text(self.creature_name(attacker))),
                );
            }
            if self.is_player(target) {
                self.send(
                    target,
                    &SystemMessage::new(SystemMessageId::C1WeaklyResistedC2SMagic)
                        .with(SystemMessageParam::Text(self.creature_name(target)))
                        .with(SystemMessageParam::Text(self.creature_name(attacker))),
                );
            }
        }
        if flags.mcrit {
            // `attacker.sendPacket(MAGIC_CRITICAL_HIT)`; a creature that is no player ignores it.
            self.send(attacker, &SystemMessage::new(SystemMessageId::MagicCriticalHit));
        }
        damage
    }

    /// Java `Formulas.calcMagicAffected(actor, target, skill)`.
    pub(super) fn calc_magic_affected_for(
        &mut self,
        actor: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> bool {
        let ctx = calc_skill(skill);
        self.with_rng(|world, rng| {
            let (Some(a), Some(t)) = (world.stats(actor), world.stats(target)) else {
                return false;
            };
            let input = MagicAffectedInput {
                skill_is_active: skill.is_active(),
                skill_has_negative_effect: has_negative_effect(skill),
                skill_is_debuff: skill.is_debuff,
                m_def: t.m_def(Some(actor), Some(&ctx)),
                m_atk: a.m_atk(Some(target), Some(&ctx)),
                trait_bonus: world.calc_general_trait_bonus(actor, target, skill.trait_type, false),
                debuff_immunity: t.calc_stat(Stat::DebuffImmunity, 0.0, None, Some(&ctx)),
            };
            calc_magic_affected(&input, rng)
        })
    }

    // ------------------------------------------------------------------ reflect

    /// Java `Formulas.calcDamageReflected(attacker, target, skill, crit)`: a target with the
    /// physical vengeance stat counters a melee skill (twice on a critical hit).
    pub(super) fn calc_damage_reflected_for(
        &mut self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
        crit: bool,
    ) {
        let ctx = calc_skill(skill);
        let countered = self.with_rng(|world, rng| {
            let Some(t) = world.stats(target) else {
                return false;
            };
            let input = ReflectChanceInput {
                skill_is_magic: skill.is_magic(),
                skill_cast_range: skill.cast_range,
                vengeance_chance: t.calc_stat(
                    Stat::VengeanceSkillPhysicalDamage,
                    0.0,
                    Some(target),
                    Some(&ctx),
                ),
            };
            calc_damage_reflected(&input, rng)
        });
        if !countered {
            return;
        }
        if self.is_player(target) {
            self.send(
                target,
                &SystemMessage::new(SystemMessageId::YouCounteredC1SAttack)
                    .with(SystemMessageParam::Text(self.creature_name(attacker))),
            );
        }
        if self.is_player(attacker) {
            self.send(
                attacker,
                &SystemMessage::new(SystemMessageId::C1IsPerformingACounterattack)
                    .with(SystemMessageParam::Text(self.creature_name(target))),
            );
        }
        let counter = {
            let (Some(a), Some(t)) = (self.stats(attacker), self.stats(target)) else {
                return;
            };
            let (weapon_defence_trait, _) = self.attack_trait_inputs(attacker, target);
            calc_counter_damage(&CounterDamageInput {
                target_p_atk: t.p_atk(Some(attacker)),
                attacker_p_def: a.p_def(Some(target)),
                weapon_trait_bonus: calc_weapon_trait_bonus(weapon_defence_trait),
                general_trait_bonus: self.calc_general_trait_bonus(attacker, target, skill.trait_type, false),
                attribute_bonus: self.calc_attribute_bonus_for_effect(attacker, target, skill),
            })
        };
        self.reduce_current_hp_by_skill(attacker, counter, Some(target), skill);
        // NOTE(java-quirk): Java's own TODO says the counter should repeat once per effect of the
        // skill; it is simply doubled on a critical hit.
        if crit {
            self.reduce_current_hp_by_skill(attacker, counter, Some(target), skill);
        }
    }

    // ------------------------------------------------------------------ small shared steps

    /// `if (effected.isPlayer() && effected.asPlayer().isFakeDeath() &&
    /// PlayerConfig.FAKE_DEATH_DAMAGE_STAND) effected.stopFakeDeath(true);`
    pub(super) fn stand_up_from_fake_death_on_damage(&mut self, effected: ObjectId) {
        if self.is_player(effected)
            && self.is_fake_death(effected)
            && self.settings.effects.fake_death_damage_stand
        {
            self.stop_fake_death(effected, true);
        }
    }

    /// The `Position` check of `isInFrontOf`.
    pub(super) fn is_in_front_of(&self, creature: ObjectId, target: ObjectId) -> bool {
        attacker_position(self, creature, target) == Position::Front
    }

    /// Java `CreatureStat.getMCriticalHit(target, skill)` (the stat view applies the class balance and the cap).
    pub(super) fn m_critical_hit_of(
        &self,
        attacker: ObjectId,
        target: ObjectId,
        skill: &SkillTemplate,
    ) -> i32 {
        let ctx = calc_skill(skill);
        self.stats(attacker)
            .map_or(0, |a| a.m_critical_hit(Some(target), Some(&ctx)))
    }
}
