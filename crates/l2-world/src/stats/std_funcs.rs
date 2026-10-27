//! What each function does to a value: the template functions (`FuncSet` ... `FuncEnchantHp`)
//! and the standard formula functions of `Formulas`.
// Java: mechanics/stats/functions/{FuncSet,FuncAdd,FuncSub,FuncMul,FuncDiv,FuncShare,FuncEnchant,
//       FuncEnchantHp}.java
// Java: mechanics/stats/functions/formulas/*.java
//
// Every function gets the creature the stat belongs to (Java's `effector`) as a [`StatView`];
// a formula that needs another stat asks the view (`view.dex()`), which runs that stat's
// calculation again, as Java does.

use l2_data::item::{BodyPart, CrystalType, WeaponType};
use l2_data::{FuncKind, Stat};
use l2_protocol_game::server::charscreen::paperdoll;

use super::calculators::{Func, FuncCalc, FuncOwner};
use super::conditions::{self, CondEnv};
use super::creature_stat::StatView;
use super::{BaseStat, CalcSkill};

/// The formula functions (Java `functions/formulas/*`, each a singleton with order 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdFunc {
    /// `FuncMaxHpMul`: MAX_HP * CON bonus.
    MaxHpMul,
    /// `FuncMaxCpMul`: MAX_CP * CON bonus.
    MaxCpMul,
    /// `FuncMaxMpMul`: MAX_MP * MEN bonus.
    MaxMpMul,
    /// `FuncPAtkMod`: P. Atk. * STR bonus * level modifier.
    PAtkMod,
    /// `FuncMAtkMod`: M. Atk. * INT bonus² * level modifier².
    MAtkMod,
    /// `FuncPDefMod`: P. Def. without the slots that are worn, * level modifier.
    PDefMod,
    /// `FuncMDefMod`: M. Def. without the jewels that are worn, * MEN bonus * level modifier.
    MDefMod,
    /// `FuncAtkCritical`: critical rate * DEX bonus * 10.
    AtkCritical,
    /// `FuncMAtkCritical`: magic critical rate * WIT bonus * 10.
    MAtkCritical,
    /// `FuncAtkAccuracy`: sqrt(DEX) * 6 + level (+ high level bonuses).
    AtkAccuracy,
    /// `FuncAtkEvasion`: sqrt(DEX) * 6 + level (+ high level bonuses).
    AtkEvasion,
    /// `FuncPAtkSpeed`: * DEX bonus.
    PAtkSpeed,
    /// `FuncMAtkSpeed`: * WIT bonus.
    MAtkSpeed,
    /// `FuncMoveSpeed`: * DEX bonus.
    MoveSpeed,
    /// `FuncHenna.getInstance(stat)`: the dye bonus of one base stat.
    Henna(Stat),
    /// `FuncArmorSet.getInstance(stat)`: the armor set bonus of one base stat.
    ArmorSet(Stat),
    /// `FuncGatesPDefMod`: the Seven Signs gate modifier of doors.
    GatesPDefMod,
    /// `FuncGatesMDefMod`.
    GatesMDefMod,
}

impl StdFunc {
    /// The stat the function belongs to (Java's constructor argument).
    pub fn stat(self) -> Stat {
        match self {
            StdFunc::MaxHpMul => Stat::MaxHp,
            StdFunc::MaxCpMul => Stat::MaxCp,
            StdFunc::MaxMpMul => Stat::MaxMp,
            StdFunc::PAtkMod => Stat::PowerAttack,
            StdFunc::MAtkMod => Stat::MagicAttack,
            StdFunc::PDefMod | StdFunc::GatesPDefMod => Stat::PowerDefence,
            StdFunc::MDefMod | StdFunc::GatesMDefMod => Stat::MagicDefence,
            StdFunc::AtkCritical => Stat::CriticalRate,
            StdFunc::MAtkCritical => Stat::McriticalRate,
            StdFunc::AtkAccuracy => Stat::AccuracyCombat,
            StdFunc::AtkEvasion => Stat::EvasionRate,
            StdFunc::PAtkSpeed => Stat::PowerAttackSpeed,
            StdFunc::MAtkSpeed => Stat::MagicAttackSpeed,
            StdFunc::MoveSpeed => Stat::MoveSpeed,
            StdFunc::Henna(stat) | StdFunc::ArmorSet(stat) => stat,
        }
    }

    /// The function's `calc(effector, effected, skill, initVal)`.
    pub(crate) fn calc(self, view: &StatView<'_>, init: f64) -> f64 {
        match self {
            StdFunc::MaxHpMul | StdFunc::MaxCpMul => init * view.bonus(BaseStat::Con),
            StdFunc::MaxMpMul => init * view.bonus(BaseStat::Men),
            StdFunc::PAtkMod => init * view.bonus(BaseStat::Str) * view.level_mod(),
            StdFunc::MAtkMod => {
                // NOTE(java-quirk): Java names the INT bonus `lvlMod` and the level modifier
                // `intMod`; the product is the same.
                let int_mod = view.bonus(BaseStat::Int);
                let lvl_mod = view.level_mod();
                init * (int_mod * int_mod) * (lvl_mod * lvl_mod)
            }
            StdFunc::PDefMod => p_def_mod(view, init),
            StdFunc::MDefMod => m_def_mod(view, init),
            StdFunc::AtkCritical => init * view.bonus(BaseStat::Dex) * 10.0,
            StdFunc::MAtkCritical => {
                // "CT2: The magic critical rate has been increased to 10 times."
                if !view.is_player() || view.active_weapon_instance().is_some() {
                    init * view.bonus(BaseStat::Wit) * 10.0
                } else {
                    init
                }
            }
            StdFunc::AtkAccuracy => {
                let level = view.level();
                let mut value = init + f64::from(view.dex()).sqrt() * 6.0 + f64::from(level);
                if level > 77 {
                    value += f64::from(level - 76);
                }
                if level > 69 {
                    value += f64::from(level - 69);
                }
                value
            }
            StdFunc::AtkEvasion => {
                let level = view.level();
                let mut value = init + f64::from(view.dex()).sqrt() * 6.0 + f64::from(level);
                if view.is_player() {
                    let mut diff = f64::from(level - 69);
                    if level >= 78 {
                        diff *= 1.2;
                    }
                    if level >= 70 {
                        value += diff;
                    }
                } else if level > 69 {
                    value += f64::from(level - 69 + 2);
                }
                value
            }
            StdFunc::PAtkSpeed | StdFunc::MoveSpeed => init * view.bonus(BaseStat::Dex),
            StdFunc::MAtkSpeed => init * view.bonus(BaseStat::Wit),
            // "Should not apply henna bonus to summons."
            StdFunc::Henna(stat) => match view.henna_stats() {
                Some(h) => init + f64::from(h.of(stat)),
                None => init,
            },
            // "Should not apply armor set bonus to summons."
            StdFunc::ArmorSet(stat) => init + f64::from(view.armor_set_bonus(stat)),
            // Seven Signs is not modelled, so no cabal owns the Seal of Strife and Java returns
            // the value unchanged.
            StdFunc::GatesPDefMod | StdFunc::GatesMDefMod => init,
        }
    }
}

/// `FuncPDefMod`: the base defence of a worn slot is replaced by the armor's own functions.
fn p_def_mod(view: &StatView<'_>, init: f64) -> f64 {
    let mut value = init;
    if let Some(template) = view.player_template() {
        let worn = |slot: usize| view.paperdoll(slot).is_some();
        if worn(paperdoll::CHEST) {
            // A form's chest defence is subtracted as it is; the other slots use it as a slot
            // number (`def_mod_slot_key`).
            value -= if view.is_transformed() {
                f64::from(view.transform_def_by_slot(paperdoll::CHEST))
            } else {
                f64::from(template.base_def_by_slot(paperdoll::CHEST))
            };
        }
        let full_armor = view
            .paperdoll_template(paperdoll::CHEST)
            .is_some_and(|t| t.body_part == BodyPart::FullArmor);
        if worn(paperdoll::LEGS) || (worn(paperdoll::CHEST) && full_armor) {
            value -= f64::from(template.base_def_by_slot(view.def_mod_slot_key(paperdoll::LEGS)));
        }
        for slot in [
            paperdoll::HEAD,
            paperdoll::FEET,
            paperdoll::GLOVES,
            paperdoll::UNDER,
            paperdoll::CLOAK,
        ] {
            if worn(slot) {
                value -= f64::from(template.base_def_by_slot(view.def_mod_slot_key(slot)));
            }
        }
    }
    value * view.level_mod()
}

/// `FuncMDefMod`: the same for the jewels.
fn m_def_mod(view: &StatView<'_>, init: f64) -> f64 {
    let mut value = init;
    if let Some(template) = view.player_template() {
        for slot in [
            paperdoll::LFINGER,
            paperdoll::RFINGER,
            paperdoll::LEAR,
            paperdoll::REAR,
            paperdoll::NECK,
        ] {
            if view.paperdoll(slot).is_some() {
                value -= f64::from(template.base_def_by_slot(view.def_mod_slot_key(slot)));
            }
        }
    } else if view.is_pet() && view.world().pet_jewel_id(view.object_id()) != 0 {
        // A pet wearing a neck item loses 13.
        value -= 13.0;
    }
    value * view.bonus(BaseStat::Men) * view.level_mod()
}

/// One function applied to `init` (Java `AbstractFunction.calc`). `view` is the creature the
/// stat belongs to.
pub(crate) fn apply(
    func: &Func,
    view: &StatView<'_>,
    target: Option<l2_util::ObjectId>,
    skill: Option<&CalcSkill>,
    init: f64,
) -> f64 {
    let kind = match func.calc {
        FuncCalc::Std(std) => return std.calc(view, init),
        FuncCalc::Template(kind) => kind,
    };
    let holds = || {
        func.cond.as_deref().is_none_or(|cond| {
            conditions::test(
                cond,
                &CondEnv {
                    actor: view,
                    target,
                    skill,
                },
            )
        })
    };
    match kind {
        FuncKind::Set => {
            if holds() {
                func.value
            } else {
                init
            }
        }
        FuncKind::Add => {
            if holds() {
                init + func.value
            } else {
                init
            }
        }
        FuncKind::Sub => {
            if holds() {
                init - func.value
            } else {
                init
            }
        }
        FuncKind::Mul => {
            if holds() {
                init * func.value
            } else {
                init
            }
        }
        // NOTE(java-quirk): Java catches an exception to log "Division by zero", but a double
        // division never throws: a 0 divisor gives an infinity, as here.
        FuncKind::Div => {
            if holds() {
                init / func.value
            } else {
                init
            }
        }
        FuncKind::Share => {
            // `FuncShare`: a servitor gets the owner's base value of the stat times the value.
            if !holds() {
                return init;
            }
            match view.servitor_owner_view() {
                Some(owner) => init + share_base_value(func.stat, &owner) * func.value,
                None => init,
            }
        }
        FuncKind::Enchant => {
            if !holds() {
                return init;
            }
            enchant(func, view, init)
        }
        FuncKind::EnchantHp => {
            if !holds() {
                return init;
            }
            enchant_hp(func, view, init)
        }
    }
}

/// `FuncEnchant.calc` after the condition: the enchant bonus of the owner item.
fn enchant(func: &Func, view: &StatView<'_>, init: f64) -> f64 {
    let FuncOwner::Item(owner) = func.owner else {
        return init;
    };
    let Some((item, Some(template))) = view.item(owner) else {
        return init;
    };
    let mut value = init;
    let mut enchant = item.enchant_level;
    if enchant <= 0 {
        return value;
    }
    let mut overenchant = 0;
    if enchant > 3 {
        overenchant = enchant - 3;
        enchant = 3;
    }
    // The cap of a player in olympiad mode (`OlympiadEnchantLimit`, M8-OLY-01).
    let limit = view.settings().olympiad_enchant_limit;
    if view.is_player()
        && view.world().is_in_olympiad_mode(view.object_id())
        && limit >= 0
        && enchant + overenchant > limit
    {
        if limit > 3 {
            overenchant = limit - 3;
        } else {
            overenchant = 0;
            enchant = limit;
        }
    }
    let (e, o) = (f64::from(enchant), f64::from(overenchant));
    if func.stat == Stat::MagicDefence || func.stat == Stat::PowerDefence {
        return value + e + 3.0 * o;
    }
    let grade = template.crystal_type_plus();
    if func.stat == Stat::MagicAttack {
        // "M. Atk. increases by 4/3/2 for all weapons. Starting at +4, M. Atk. bonus double."
        value += match grade {
            CrystalType::S | CrystalType::S80 | CrystalType::S84 => 4.0 * e + 8.0 * o,
            CrystalType::A | CrystalType::B | CrystalType::C => 3.0 * e + 6.0 * o,
            CrystalType::D | CrystalType::None => 2.0 * e + 4.0 * o,
        };
        return value;
    }
    // NOTE(java-quirk): any other stat (the shield's `sDef` enchant function) gets nothing.
    if let Some(weapon) = template.weapon() {
        let kind = weapon.weapon_type;
        let two_handed = template.body_part == BodyPart::BothHands;
        // (bow, crossbow, other two-handed, one-handed) per point of the first three levels;
        // every level above +3 counts double.
        let per_level = match grade {
            CrystalType::S | CrystalType::S80 | CrystalType::S84 => {
                if two_handed {
                    match kind {
                        WeaponType::Bow => 10.0,
                        WeaponType::CrossBow => 7.0,
                        _ => 6.0,
                    }
                } else {
                    5.0
                }
            }
            CrystalType::A => {
                if two_handed {
                    match kind {
                        WeaponType::Bow => 8.0,
                        WeaponType::CrossBow => 6.0,
                        _ => 5.0,
                    }
                } else {
                    4.0
                }
            }
            CrystalType::B | CrystalType::C => {
                if two_handed {
                    match kind {
                        WeaponType::Bow => 6.0,
                        WeaponType::CrossBow => 5.0,
                        _ => 4.0,
                    }
                } else {
                    3.0
                }
            }
            // D and no grade: by weapon type only, whatever the hands.
            CrystalType::D | CrystalType::None => match kind {
                WeaponType::Bow => 4.0,
                WeaponType::CrossBow => 3.0,
                _ => 2.0,
            },
        };
        value += per_level * e + 2.0 * per_level * o;
    }
    value
}

/// `FuncEnchantHp.calc` after the condition: `EnchantItemHPBonusData.getHPBonus(item)` for an
/// enchanted item.
fn enchant_hp(func: &Func, view: &StatView<'_>, init: f64) -> f64 {
    let FuncOwner::Item(owner) = func.owner else {
        return init;
    };
    let Some((item, Some(template))) = view.item(owner) else {
        return init;
    };
    if item.enchant_level <= 0 {
        return init;
    }
    let Some(data) = view.stat_data() else {
        return init;
    };
    // `getOlyEnchantLevel()`: capped in olympiad mode (`OlympiadEnchantLimit`, M8-OLY-01).
    let limit = view.settings().olympiad_enchant_limit;
    let mut enchant_level = item.enchant_level;
    if view.is_player() && view.world().is_in_olympiad_mode(view.object_id()) && limit >= 0 && enchant_level > limit {
        enchant_level = limit;
    }
    let bonus = data.enchant_hp_bonus.hp_bonus(
        template.crystal_type,
        enchant_level,
        template.body_part == BodyPart::FullArmor,
    );
    init + f64::from(bonus)
}

/// Java `FuncShare.getBaseValue(stat, player)`: the owner's value a servitor shares.
fn share_base_value(stat: Stat, owner: &StatView<'_>) -> f64 {
    match stat {
        Stat::MaxHp => f64::from(owner.max_hp()),
        Stat::MaxMp => f64::from(owner.max_mp()),
        Stat::PowerAttack => owner.p_atk(None),
        Stat::MagicAttack => owner.m_atk(None, None),
        Stat::PowerDefence => owner.p_def(None),
        Stat::MagicDefence => owner.m_def(None, None),
        Stat::CriticalRate => f64::from(owner.critical_hit(None, None)),
        Stat::PowerAttackSpeed => owner.p_atk_spd(),
        Stat::MagicAttackSpeed => f64::from(owner.m_atk_spd()),
        _ => owner.calc_stat(stat, 0.0, None, None),
    }
}
