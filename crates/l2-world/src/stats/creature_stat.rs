//! The stat getters every creature has, over a read-only view of the world.
// Java: entity/actor/stat/CreatureStat.java (calcStat and the getters)
// Java: entity/actor/templates/CreatureTemplate.java (the base values and their defaults)
// Java: entity/actor/Creature.java (getLevelMod, getAttackType, getMaxLoad, getBonusWeightPenalty)
//
// Java's `CreatureStat` / `PlayableStat` / `PlayerStat` / `NpcStat` / `DoorStat` subclasses are
// one view here: a getter that a subclass overrides asks `self.who` which kind of creature it is.
// The player-only parts are in `player_stat.rs`, the NPC-only parts in `npc_stat.rs`; a door
// (`Who::Door`, M7-SIEGE-02) reads its `DoorTemplate` and `DoorStat`'s upgrade ratio.

use l2_data::item::WeaponType;
use l2_data::npc::NpcRace;
use l2_data::{NpcTemplate, PlayerTemplate, Stat};
use l2_util::ObjectId;
use tracing::warn;

use super::calculators::Calculators;
use super::{BaseStat, CalcSkill, ClassBalance, StatData, StatSettings, java_round, std_funcs};
use crate::door::Door;
use crate::objects::Npc;
use crate::player::{CreatureCore, Player};
use crate::world::World;
use crate::zones::ZoneId;

/// How deep stat calculations may nest (a formula asking for another stat, a condition asking
/// for max HP, ...). Java has no limit and would overflow its stack on a cycle; a cycle in the
/// data is cut here with a warning instead.
const MAX_DEPTH: u8 = 32;

/// Java `MoveType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveType {
    Run,
    Walk,
    FastSwim,
    SlowSwim,
    FastFly,
    SlowFly,
}

/// The creature a view reads.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Who<'w> {
    Player(&'w Player),
    Npc(&'w Npc),
    /// Java `Door extends Creature` (M7-SIEGE-02).
    Door(&'w Door),
}

/// A read-only view of one creature's stats (Java `Creature.getStat()`), built from `&World`
/// with [`World::stats`]. Cheap to make; holds no state of its own.
#[derive(Clone, Copy)]
pub struct StatView<'w> {
    world: &'w World,
    pub(crate) who: Who<'w>,
    depth: u8,
}

/// The `CreatureTemplate` values `CreatureStat` reads, with Java's defaults for anything the
/// template does not have.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Base {
    pub str: i32,
    pub con: i32,
    pub dex: i32,
    pub int: i32,
    pub wit: i32,
    pub men: i32,
    pub hp_max: f32,
    pub mp_max: f32,
    pub cp_max: f32,
    pub p_atk: i32,
    pub m_atk: i32,
    pub p_def: i32,
    pub m_def: i32,
    pub p_atk_spd: i32,
    pub m_atk_spd: i32,
    pub crit_rate: i32,
    pub atk_range: i32,
    pub attack_type: WeaponType,
    /// `_moveType[]` by [`MoveType`] order.
    pub speeds: [f64; 6],
    /// `getBaseFire()` ... `getBaseDark()` in element order (fire, water, wind, earth, holy, dark).
    pub elements: [i32; 6],
    /// `getBaseFireRes()` ... in element order.
    pub element_res: [f64; 6],
    pub base_element_res: f64,
}

impl Base {
    /// `new CreatureTemplate(new StatSet())`: what Java's `set` gives for an empty set.
    pub const JAVA_DEFAULT: Base = Base {
        str: 0,
        con: 0,
        dex: 0,
        int: 0,
        wit: 0,
        men: 0,
        hp_max: 0.0,
        mp_max: 0.0,
        cp_max: 0.0,
        p_atk: 0,
        m_atk: 0,
        p_def: 0,
        m_def: 0,
        p_atk_spd: 300,
        m_atk_spd: 333,
        crit_rate: 4,
        atk_range: 40,
        attack_type: WeaponType::Fist,
        speeds: [120.0, 50.0, 120.0, 50.0, 120.0, 50.0],
        elements: [0; 6],
        element_res: [0.0; 6],
        base_element_res: 0.0,
    };

    /// A player class template. The fields the XML does not have keep Java's defaults.
    pub fn of_player(t: &PlayerTemplate) -> Base {
        Base {
            str: t.base_str,
            con: t.base_con,
            dex: t.base_dex,
            int: t.base_int,
            wit: t.base_wit,
            men: t.base_men,
            p_atk: t.p_atk,
            m_atk: t.m_atk,
            p_def: t.p_def,
            m_def: t.m_def,
            p_atk_spd: t.p_atk_spd,
            m_atk_spd: t.m_atk_spd,
            crit_rate: t.crit_rate,
            atk_range: t.atk_range,
            // Java: baseFlyRunSpd / baseFlyWalkSpd fall back to the ground speeds.
            speeds: [
                t.run_speed,
                t.walk_speed,
                t.fast_swim_speed,
                t.slow_swim_speed,
                t.run_speed,
                t.walk_speed,
            ],
            ..Base::JAVA_DEFAULT
        }
    }

    /// A door template: `DoorData` hands the XML attributes to `CreatureTemplate` as they are,
    /// so a door has `baseHpMax` (1 when missing), `basePDef` and `baseMDef`, everything else is
    /// Java's default.
    pub fn of_door(t: &l2_data::DoorTemplate) -> Base {
        Base {
            hp_max: t.base_hp_max,
            p_def: t.base_p_def,
            m_def: t.base_m_def,
            ..Base::JAVA_DEFAULT
        }
    }

    /// An NPC template.
    pub fn of_npc(t: &NpcTemplate) -> Base {
        Base {
            str: t.base_str,
            con: t.base_con,
            dex: t.base_dex,
            int: t.base_int,
            wit: t.base_wit,
            men: t.base_men,
            hp_max: t.base_hp_max,
            mp_max: t.base_mp_max,
            // NPC files have no `baseCpMax`: Java's default 0.
            cp_max: 0.0,
            p_atk: t.base_p_atk,
            m_atk: t.base_m_atk,
            p_def: t.base_p_def,
            m_def: t.base_m_def,
            p_atk_spd: t.base_p_atk_spd,
            m_atk_spd: t.base_m_atk_spd,
            crit_rate: t.base_crit_rate,
            atk_range: t.base_attack_range,
            attack_type: t.base_attack_type,
            speeds: [
                t.run_speed,
                t.walk_speed,
                t.swim_run_speed,
                t.swim_walk_speed,
                t.fly_run_speed,
                t.fly_walk_speed,
            ],
            elements: [
                t.base_fire,
                t.base_water,
                t.base_wind,
                t.base_earth,
                t.base_holy,
                t.base_dark,
            ],
            element_res: [
                t.base_fire_res,
                t.base_water_res,
                t.base_wind_res,
                t.base_earth_res,
                t.base_holy_res,
                t.base_dark_res,
            ],
            base_element_res: t.base_element_res,
        }
    }

    fn speed(&self, kind: MoveType) -> f64 {
        self.speeds[kind as usize]
    }
}

/// The stats `calcStat` never lets drop to 0 or below ("avoid some troubles with negative
/// stats").
fn never_below_one(stat: Stat) -> bool {
    matches!(
        stat,
        Stat::MaxHp
            | Stat::MaxMp
            | Stat::MaxCp
            | Stat::MagicDefence
            | Stat::PowerDefence
            | Stat::PowerAttack
            | Stat::MagicAttack
            | Stat::PowerAttackSpeed
            | Stat::MagicAttackSpeed
            | Stat::ShieldDefence
            | Stat::StatCon
            | Stat::StatDex
            | Stat::StatInt
            | Stat::StatMen
            | Stat::StatStr
            | Stat::StatWit
    )
}

/// Java's `Elementals` ids, in the order of the stat arrays.
const ATTACK_ELEMENT_STATS: [Stat; 6] = [
    Stat::FirePower,
    Stat::WaterPower,
    Stat::WindPower,
    Stat::EarthPower,
    Stat::HolyPower,
    Stat::DarkPower,
];
const DEFENCE_ELEMENT_STATS: [Stat; 6] = [
    Stat::FireRes,
    Stat::WaterRes,
    Stat::WindRes,
    Stat::EarthRes,
    Stat::HolyRes,
    Stat::DarkRes,
];

impl World {
    /// The stats of a player, NPC or door (Java `getStat()`); `None` for any other id.
    pub fn stats(&self, id: ObjectId) -> Option<StatView<'_>> {
        if let Some(player) = self.players.get(&id) {
            return Some(StatView::new(self, Who::Player(player)));
        }
        if let Some(npc) = self.npcs.get(&id) {
            return Some(StatView::new(self, Who::Npc(npc)));
        }
        self.doors
            .get(&id)
            .map(|door| StatView::new(self, Who::Door(door)))
    }
}

impl<'w> StatView<'w> {
    pub(crate) fn new(world: &'w World, who: Who<'w>) -> Self {
        Self { world, who, depth: 0 }
    }

    /// The view of a player the caller already holds.
    pub(crate) fn of_player(world: &'w World, player: &'w Player) -> Self {
        Self::new(world, Who::Player(player))
    }

    /// The same creature one calculation deeper.
    fn child(&self) -> Self {
        Self {
            depth: self.depth.saturating_add(1),
            ..*self
        }
    }

    /// Another creature (a condition's target), one calculation deeper.
    pub(crate) fn other(&self, id: ObjectId) -> Option<StatView<'w>> {
        let mut view = self.world.stats(id)?;
        view.depth = self.depth.saturating_add(1);
        Some(view)
    }

    pub(crate) fn world(&self) -> &'w World {
        self.world
    }

    pub(crate) fn settings(&self) -> &'w StatSettings {
        &self.world.settings.stats
    }

    /// The datapack tables; `None` when the world has none (tests).
    pub(crate) fn stat_data(&self) -> Option<&'w StatData> {
        self.world.data.stat_data()
    }

    pub fn object_id(&self) -> ObjectId {
        self.core().object_id
    }

    pub fn core(&self) -> &'w CreatureCore {
        match self.who {
            Who::Player(p) => &p.core,
            Who::Npc(n) => &n.core,
            Who::Door(d) => &d.core,
        }
    }

    fn calculators(&self) -> &'w Calculators {
        &self.core().calculators
    }

    pub fn player(&self) -> Option<&'w Player> {
        match self.who {
            Who::Player(p) => Some(p),
            Who::Npc(_) | Who::Door(_) => None,
        }
    }

    pub fn npc(&self) -> Option<&'w Npc> {
        match self.who {
            Who::Npc(n) => Some(n),
            Who::Player(_) | Who::Door(_) => None,
        }
    }

    /// The door (Java `asDoor()`).
    pub fn door(&self) -> Option<&'w Door> {
        match self.who {
            Who::Door(d) => Some(d),
            Who::Player(_) | Who::Npc(_) => None,
        }
    }

    /// Java `isPlayer()`.
    pub fn is_player(&self) -> bool {
        matches!(self.who, Who::Player(_))
    }

    /// Java `isPlayable()`: players and summons.
    pub fn is_playable(&self) -> bool {
        self.is_player() || self.is_summon()
    }

    /// Java `isSummon()`.
    pub fn is_summon(&self) -> bool {
        self.npc()
            .is_some_and(|n| n.class == crate::npc::NpcClass::Summon)
    }

    /// Java `isRaid()`.
    pub fn is_raid(&self) -> bool {
        self.npc().is_some_and(|n| n.class.is_raid())
    }

    /// Java `isChampion()`: champions are rolled at spawn (`ChampionMonstersConfig`), which is
    /// not ported; nobody is one.
    pub fn is_champion(&self) -> bool {
        false
    }

    /// The template values the getters start from.
    pub(crate) fn base(&self) -> Base {
        match self.who {
            Who::Player(_) => self.player_template().map_or(Base::JAVA_DEFAULT, Base::of_player),
            Who::Npc(_) => self.npc_template().map_or(Base::JAVA_DEFAULT, Base::of_npc),
            Who::Door(d) => Base::of_door(&d.template),
        }
    }

    /// Java `getLevel()`.
    pub fn level(&self) -> i32 {
        match self.who {
            Who::Player(p) => p.core.level,
            // `PetStat` keeps its own level.
            Who::Npc(n) if self.is_pet() => n.core.level,
            Who::Npc(_) => self.npc_level(),
            // `Door.getLevel()`: the template's.
            Who::Door(d) => d.template.level,
        }
    }

    /// Java `Creature.getLevelMod()`; `Player.getLevelMod` takes the transformation's when it has
    /// one for the level.
    pub fn level_mod(&self) -> f64 {
        if self.is_transformed() {
            let level_mod = self.transform_level_mod();
            if level_mod > -1.0 {
                return level_mod;
            }
        }
        f64::from(self.level() + 89) / 100.0
    }

    /// Java `getRace()`.
    pub fn race(&self) -> Option<NpcRace> {
        match self.who {
            Who::Player(p) => usize::try_from(p.data.race)
                .ok()
                .and_then(|r| NpcRace::ALL.get(r))
                .copied(),
            Who::Npc(_) => self.npc_template().map(|t| t.race),
            // A door template has no race.
            Who::Door(_) => None,
        }
    }

    /// Java `isInsideZone(ZoneId.WATER)`.
    pub fn in_water(&self) -> bool {
        match self.who {
            Who::Player(p) => p.zones.is_inside(ZoneId::Water),
            Who::Npc(n) => n.in_water,
            Who::Door(_) => false,
        }
    }

    /// Java `isRunning()`. NPCs walk until their AI makes them run.
    pub fn is_running(&self) -> bool {
        match self.who {
            Who::Player(p) => p.movement.running,
            Who::Npc(n) => n.movement.running,
            Who::Door(_) => false,
        }
    }

    /// Java `isMoving()`.
    pub fn is_moving(&self) -> bool {
        self.world.is_moving(self.object_id())
    }

    /// Java `getInstanceId()`.
    pub fn instance_id(&self) -> i32 {
        self.world
            .objects
            .get(&self.object_id())
            .map_or(0, |e| e.instance_id)
    }

    /// Java `getAttackType()`: a form's, else the weapon's type, else the template's.
    pub fn attack_type(&self) -> WeaponType {
        if self.is_transformed()
            && let Some(template) = self.transform_template()
        {
            return template.base_attack_type;
        }
        match self.active_weapon_item().and_then(|t| t.weapon()) {
            Some(w) => w.weapon_type,
            None => self.base().attack_type,
        }
    }

    /// The skills a creature knows as `(id, level)` (Java `getAllSkills()`).
    pub(crate) fn skills(&self) -> impl Iterator<Item = (i32, i32)> + 'w {
        self.core().skills.iter().copied()
    }

    /// Java `getSkillLevel(id)`: 0 for a skill the creature does not know.
    pub fn skill_level(&self, id: i32) -> i32 {
        // `Pet.getSkillLevel(skillId)`: the level the pet's own level gives, for a skill it knows.
        if self.is_pet() {
            if self.skills().all(|(skill, _)| skill != id) {
                return 0;
            }
            let level = self.level();
            return if level > 70 {
                7 + ((level - 70) / 5)
            } else {
                level / 10
            };
        }
        self.skills()
            .find(|(skill, _)| *skill == id)
            .map_or(0, |(_, level)| level)
    }

    // ------------------------------------------------------------------ calcStat

    /// Java `CreatureStat.calcStat(stat, init, target, skill)`.
    pub fn calc_stat(
        &self,
        stat: Stat,
        init: f64,
        target: Option<ObjectId>,
        skill: Option<&CalcSkill>,
    ) -> f64 {
        let funcs = self.calculators().funcs(stat);
        // "If no Func object found, no modifier is applied."
        if funcs.is_empty() {
            return init;
        }
        if self.depth >= MAX_DEPTH {
            warn!(
                "Stat {} of {} nests deeper than {MAX_DEPTH} calculations; the cycle is cut.",
                stat.java_name(),
                self.object_id()
            );
            return init;
        }
        let view = self.child();
        // Apply transformation stats.
        let mut value = init;
        if self.transformation().is_some() {
            let transformed = self.transform_stat(stat);
            if transformed > 0.0 {
                value = transformed;
            }
        }
        for func in funcs {
            value = std_funcs::apply(func, &view, target, skill, value);
        }
        if value <= 0.0 && never_below_one(stat) {
            value = 1.0;
        }
        value
    }

    /// `calcStat(stat, init)`.
    pub fn calc(&self, stat: Stat, init: f64) -> f64 {
        self.calc_stat(stat, init, None, None)
    }

    /// Java `BaseStat.X.calcBonus(this)`: the `statBonus.xml` multiplier at the current value.
    pub fn bonus(&self, stat: BaseStat) -> f64 {
        let value = match stat {
            BaseStat::Str => self.str(),
            BaseStat::Int => self.int(),
            BaseStat::Dex => self.dex(),
            BaseStat::Wit => self.wit(),
            BaseStat::Con => self.con(),
            BaseStat::Men => self.men(),
        };
        self.stat_data()
            .map_or(1.0, |d| d.base_stat_bonus.bonus(stat, value))
    }

    // ------------------------------------------------------------------ base stats

    /// Java `getSTR()`.
    pub fn str(&self) -> i32 {
        self.calc(Stat::StatStr, f64::from(self.base().str)) as i32
    }

    pub fn con(&self) -> i32 {
        self.calc(Stat::StatCon, f64::from(self.base().con)) as i32
    }

    pub fn dex(&self) -> i32 {
        self.calc(Stat::StatDex, f64::from(self.base().dex)) as i32
    }

    pub fn int(&self) -> i32 {
        self.calc(Stat::StatInt, f64::from(self.base().int)) as i32
    }

    pub fn wit(&self) -> i32 {
        self.calc(Stat::StatWit, f64::from(self.base().wit)) as i32
    }

    pub fn men(&self) -> i32 {
        self.calc(Stat::StatMen, f64::from(self.base().men)) as i32
    }

    // ------------------------------------------------------------------ hit points

    /// Java `getMaxHp()` (`PlayerStat` takes the class template's value at the current level).
    pub fn max_hp(&self) -> i32 {
        let base = match self.who {
            Who::Player(_) => self.player_level_value(|t, l| t.base_hp_max(l)),
            Who::Npc(_) => self.pet_level_data().map_or(self.base().hp_max, |l| l.max_hp),
            // `DoorStat.getMaxHp()`: `super.getMaxHp() * _upgradeHpRatio` (int arithmetic).
            Who::Door(d) => {
                let value = self.calc(Stat::MaxHp, f64::from(self.base().hp_max)) as i32;
                return value.wrapping_mul(d.upgrade_hp_ratio);
            }
        };
        let value = self.calc(Stat::MaxHp, f64::from(base)) as i32;
        // `Servitor.getMaxHp()`: plus the owner's share (`summon/share.rs`).
        let share = self.servitor_share(Stat::MaxHp, |o| f64::from(o.max_hp()));
        if share == 0.0 {
            value
        } else {
            (f64::from(value) + share) as i32
        }
    }

    /// Java `getMaxMp()`.
    pub fn max_mp(&self) -> i32 {
        let base = match self.who {
            Who::Player(_) => self.player_level_value(|t, l| t.base_mp_max(l)),
            Who::Npc(_) => self.pet_level_data().map_or(self.base().mp_max, |l| l.max_mp),
            Who::Door(_) => self.base().mp_max,
        };
        let value = self.calc(Stat::MaxMp, f64::from(base)) as i32;
        // `Servitor.getMaxMp()`.
        let share = self.servitor_share(Stat::MaxMp, |o| f64::from(o.max_mp()));
        if share == 0.0 {
            value
        } else {
            (f64::from(value) + share) as i32
        }
    }

    /// Java `getMaxCp()`.
    pub fn max_cp(&self) -> i32 {
        let base = match self.who {
            Who::Player(_) => self.player_level_value(|t, l| t.base_cp_max(l)),
            Who::Npc(_) | Who::Door(_) => self.base().cp_max,
        };
        self.calc(Stat::MaxCp, f64::from(base)) as i32
    }

    /// Java `getMaxRecoverableHp()`.
    pub fn max_recoverable_hp(&self) -> i32 {
        self.calc(Stat::MaxRecoverableHp, f64::from(self.max_hp())) as i32
    }

    pub fn max_recoverable_mp(&self) -> i32 {
        self.calc(Stat::MaxRecoverableMp, f64::from(self.max_mp())) as i32
    }

    pub fn max_recoverable_cp(&self) -> i32 {
        self.calc(Stat::MaxRecoverableCp, f64::from(self.max_cp())) as i32
    }

    // ------------------------------------------------------------------ attack and defence

    /// Java `getPAtk(target)`.
    pub fn p_atk(&self, target: Option<ObjectId>) -> f64 {
        // `PetStat.getPAtk(target)`: the level row's value, nothing else.
        if let Some(level) = self.pet_level_data() {
            return self.calc_stat(Stat::PowerAttack, f64::from(level.p_atk), target, None);
        }
        let s = self.settings();
        let mut bonus: f32 = 1.0;
        if s.champion_enable && self.is_champion() {
            bonus = s.champion_atk;
        }
        if self.is_raid() {
            // `bonusAtk *= RAID_PATTACK_MULTIPLIER`: a double product narrowed to the float.
            bonus = (f64::from(bonus) * s.raid_pattack_multiplier) as f32;
        }
        // `getBasePAtk() * bonusAtk` is int * float.
        let init = f64::from(self.base().p_atk as f32 * bonus);
        let value = self
            .calc_stat(Stat::PowerAttack, init, target, None)
            .min(f64::from(s.max_patk));
        // `Servitor.getPAtk(target)`.
        value + self.servitor_share(Stat::PowerAttack, |o| o.p_atk(target))
    }

    /// Java `getMAtk(target, skill)`.
    pub fn m_atk(&self, target: Option<ObjectId>, skill: Option<&CalcSkill>) -> f64 {
        // `PetStat.getMAtk(target, skill)`.
        if let Some(level) = self.pet_level_data() {
            return self.calc_stat(Stat::MagicAttack, f64::from(level.m_atk), target, skill);
        }
        let s = self.settings();
        let mut bonus: f32 = 1.0;
        if s.champion_enable && self.is_champion() {
            bonus = s.champion_atk;
        }
        if self.is_raid() {
            bonus = (f64::from(bonus) * s.raid_mattack_multiplier) as f32;
        }
        let init = f64::from(self.base().m_atk as f32 * bonus);
        let value = self
            .calc_stat(Stat::MagicAttack, init, target, skill)
            .min(f64::from(s.max_matk));
        // `Servitor.getMAtk(target, skill)`.
        value + self.servitor_share(Stat::MagicAttack, |o| o.m_atk(target, skill))
    }

    /// Java `getPDef(target)`.
    pub fn p_def(&self, target: Option<ObjectId>) -> f64 {
        // `PetStat.getPDef(target)`: the owner's class balance when the pet is hit or hits.
        if let Some(level) = self.pet_level_data() {
            let value = self.calc_stat(Stat::PowerDefence, f64::from(level.p_def), target, None);
            let Some(target) = target else {
                return value;
            };
            let cb = &self.settings().class_balance;
            let table = if self.other(target).is_some_and(|t| t.is_playable()) {
                &cb.pvp_physical_attack_defence
            } else {
                &cb.pve_physical_attack_defence
            };
            return f64::from(ClassBalance::of(table, self.pet_owner_class_id().unwrap_or(0))) * value;
        }
        let s = self.settings();
        let base = f64::from(self.base().p_def);
        let init = if self.is_raid() {
            base * s.raid_pdefence_multiplier
        } else {
            base
        };
        let value = self.calc_stat(Stat::PowerDefence, init, target, None);
        match (self.player(), target) {
            (Some(p), Some(t)) => {
                let cb = &s.class_balance;
                let table = if self.other(t).is_some_and(|t| t.is_playable()) {
                    &cb.pvp_physical_attack_defence
                } else {
                    &cb.pve_physical_attack_defence
                };
                // `float * double`.
                f64::from(ClassBalance::of(table, p.data.class_id)) * value
            }
            // `Servitor.getPDef(target)`.
            _ => value + self.servitor_share(Stat::PowerDefence, |o| o.p_def(target)),
        }
    }

    /// Java `getMDef(target, skill)`.
    pub fn m_def(&self, target: Option<ObjectId>, skill: Option<&CalcSkill>) -> f64 {
        // `PetStat.getMDef(target, skill)`.
        if let Some(level) = self.pet_level_data() {
            return self.calc_stat(Stat::MagicDefence, f64::from(level.m_def), target, skill);
        }
        let s = self.settings();
        let mut defence = f64::from(self.base().m_def);
        if self.is_raid() {
            defence *= s.raid_mdefence_multiplier;
        }
        if let (Some(skill), Some(t)) = (skill, target)
            && let Some(t) = self.other(t)
            && let Some(tp) = t.player()
        {
            // NOTE(java-quirk): Java uses the PvE tables here even though the target is a
            // playable, and the target's class (not the defender's).
            let cb = &s.class_balance;
            let table = if skill.magic {
                &cb.pve_magical_skill_defence
            } else {
                &cb.pve_physical_skill_defence
            };
            defence *= f64::from(ClassBalance::of(table, tp.data.class_id));
        }
        // `Servitor.getMDef(target, skill)`.
        self.calc_stat(Stat::MagicDefence, defence, target, skill)
            + self.servitor_share(Stat::MagicDefence, |o| o.m_def(target, skill))
    }

    /// Java `getPAtkSpd()`: `Math.round` of the calculation (`PlayerStat` caps it).
    pub fn p_atk_spd(&self) -> f64 {
        let s = self.settings();
        let mut bonus: f32 = 1.0;
        if s.champion_enable && self.is_champion() {
            bonus = s.champion_spd_atk;
        }
        let init = f64::from(self.base().p_atk_spd as f32 * bonus);
        let value = java_round(self.calc(Stat::PowerAttackSpeed, init)) as f64;
        if self.is_player() && value > f64::from(s.max_patk_speed) && !self.is_gm() {
            return f64::from(s.max_patk_speed);
        }
        // `PetStat.getPAtkSpd()`: a hungry pet attacks at half the speed.
        if self.pet_is_hungry() {
            return value / 2.0;
        }
        // `Servitor.getPAtkSpd()`.
        value + self.servitor_share(Stat::PowerAttackSpeed, |o| o.p_atk_spd())
    }

    /// Java `getMAtkSpd()`.
    pub fn m_atk_spd(&self) -> i32 {
        let s = self.settings();
        let mut bonus: f32 = 1.0;
        if s.champion_enable && self.is_champion() {
            bonus = s.champion_spd_atk;
        }
        let init = f64::from(self.base().m_atk_spd as f32 * bonus);
        let value = self
            .calc(Stat::MagicAttackSpeed, init)
            .min(f64::from(s.max_matk_speed));
        // `PetStat.getMAtkSpd()`: `super.getMAtkSpd() / 2` when hungry (an int division).
        if self.pet_is_hungry() {
            return (value as i32) / 2;
        }
        // `Servitor.getMAtkSpd()`: `(int) (super + owner * (bonus - 1))`.
        let share = self.servitor_share(Stat::MagicAttackSpeed, |o| f64::from(o.m_atk_spd()));
        if share == 0.0 {
            value as i32
        } else {
            (f64::from(value as i32) + share) as i32
        }
    }

    /// Java `getCriticalHit(target, skill)`.
    pub fn critical_hit(&self, target: Option<ObjectId>, skill: Option<&CalcSkill>) -> i32 {
        let value = self.calc_stat(
            Stat::CriticalRate,
            f64::from(self.base().crit_rate),
            target,
            skill,
        );
        let value = value.min(f64::from(self.settings().max_pcrit_rate));
        let value = (value + 0.5) as i32;
        // `Servitor.getCriticalHit(target, skill)`.
        let share = self.servitor_share(Stat::CriticalRate, |o| f64::from(o.critical_hit(target, skill)));
        if share == 0.0 {
            value
        } else {
            (f64::from(value) + share) as i32
        }
    }

    /// Java `getMCriticalHit(target, skill)`.
    pub fn m_critical_hit(&self, target: Option<ObjectId>, skill: Option<&CalcSkill>) -> i32 {
        // NOTE(java-quirk): `(int) calcStat(MCRITICAL_RATE, 1, ...) * 10`: the calculation
        // starts at 1, not at the template's `baseMCritRate`, and the cast comes before the * 10.
        let mut value = (self.calc_stat(Stat::McriticalRate, 1.0, target, skill) as i32).wrapping_mul(10);
        if let (Some(_), Some(t), Some(p)) = (skill, target, self.player()) {
            let cb = &self.settings().class_balance;
            let table = if self.other(t).is_some_and(|t| t.is_playable()) {
                &cb.pvp_magical_skill_critical_chance
            } else {
                &cb.pve_magical_skill_critical_chance
            };
            // `val *= float`: int * float, narrowed back to the int.
            value = (value as f32 * ClassBalance::of(table, p.data.class_id)) as i32;
        }
        value.min(self.settings().max_mcrit_rate)
    }

    /// Java `getEvasionRate(target)`.
    pub fn evasion_rate(&self, target: Option<ObjectId>) -> i32 {
        let value = java_round(self.calc_stat(Stat::EvasionRate, 0.0, target, None)) as i32;
        value.min(self.settings().max_evasion)
    }

    /// Java `getAccuracy()`.
    pub fn accuracy(&self) -> i32 {
        java_round(self.calc(Stat::AccuracyCombat, 0.0)) as i32
    }

    /// Java `getShldDef()`.
    pub fn shld_def(&self) -> i32 {
        self.calc(Stat::ShieldDefence, 0.0) as i32
    }

    /// Java `getPhysicalAttackRange()`.
    pub fn physical_attack_range(&self) -> i32 {
        // NOTE(java-quirk): with any weapon, the fists of an unarmed player included, the base is
        // 0 and the range comes from the weapon's `pAtkRange` function alone; the fists are never
        // equipped, so an unarmed player has range 0.
        let base = if self.transformation().is_some() {
            // `Transform.getBaseAttackRange`: the template's, else the class template's.
            self.transform_template()
                .map_or(self.base().atk_range, |t| t.base_attack_range)
        } else if self.active_weapon_item().is_none() {
            self.base().atk_range
        } else {
            0
        };
        self.calc(Stat::PowerAttackRange, f64::from(base)) as i32
    }

    /// Java `getMagicalAttackRange(skill)`: the skill's cast range is M5; without a skill the
    /// template's attack range.
    pub fn magical_attack_range(&self) -> i32 {
        self.base().atk_range
    }

    /// Java `getAttackSpeedMultiplier()`.
    pub fn attack_speed_multiplier(&self) -> f32 {
        ((1.1 * self.p_atk_spd()) / f64::from(self.base().p_atk_spd)) as f32
    }

    // ------------------------------------------------------------------ speeds

    /// Java `getBaseMoveSpeed(type)`: `PlayerStat` takes a form's speeds, or the speeds of the
    /// mount's `speed_on_ride` while the player rides one.
    pub fn base_move_speed(&self, kind: MoveType) -> f64 {
        if self.is_transformed() {
            if let Some(template) = self.transform_template() {
                return Self::transform_move_speed(template, kind);
            }
        } else if self.is_mounted()
            && let Some(speed) = self.mount_move_speed(kind)
        {
            return speed;
        }
        self.base().speed(kind)
    }

    /// Java `getRunSpeed()`.
    // NOTE(java-quirk): in water the base is `getSwimRunSpeed()`, which already went through
    // `MOVE_SPEED`, so the speed functions (the DEX bonus) apply twice.
    pub fn run_speed(&self) -> f64 {
        let base = if self.in_water() {
            self.swim_run_speed()
        } else {
            self.base_move_speed(MoveType::Run)
        };
        // `_gmSpeedMultiplier` is 1 until the admin speed command (M5).
        let value = if base <= 0.0 {
            0.0
        } else {
            self.calc(Stat::MoveSpeed, base)
        };
        self.player_speed_cap(value) * self.pet_speed_factor() * self.swamp_factor()
    }

    /// `PlayableStat.getRunSpeed` / `getWalkSpeed`: the move bonus of the swamp zone the player
    /// stands in (M7-FU-03), 1 elsewhere.
    fn swamp_factor(&self) -> f64 {
        match self.who {
            Who::Player(p) => self.world.swamp_move_bonus(p.core.object_id).unwrap_or(1.0),
            _ => 1.0,
        }
    }

    /// Java `getWalkSpeed()`.
    pub fn walk_speed(&self) -> f64 {
        let base = if self.in_water() {
            self.swim_walk_speed()
        } else {
            self.base_move_speed(MoveType::Walk)
        };
        let value = if base <= 0.0 {
            0.0
        } else {
            self.calc(Stat::MoveSpeed, base)
        };
        self.player_speed_cap(value) * self.pet_speed_factor() * self.swamp_factor()
    }

    /// Java `getSwimRunSpeed()`.
    pub fn swim_run_speed(&self) -> f64 {
        let base = self.base_move_speed(MoveType::FastSwim);
        if base <= 0.0 {
            return 0.0;
        }
        self.calc(Stat::MoveSpeed, base)
    }

    /// Java `getSwimWalkSpeed()`.
    pub fn swim_walk_speed(&self) -> f64 {
        let base = self.base_move_speed(MoveType::SlowSwim);
        if base <= 0.0 {
            return 0.0;
        }
        self.calc(Stat::MoveSpeed, base)
    }

    /// Java `getMoveSpeed()`.
    pub fn move_speed(&self) -> f64 {
        if self.in_water() {
            if self.is_running() {
                self.swim_run_speed()
            } else {
                self.swim_walk_speed()
            }
        } else if self.is_running() {
            self.run_speed()
        } else {
            self.walk_speed()
        }
    }

    /// Java `getMovementSpeedMultiplier()`.
    pub fn movement_speed_multiplier(&self) -> f64 {
        let kind = match (self.in_water(), self.is_running()) {
            (true, true) => MoveType::FastSwim,
            (true, false) => MoveType::SlowSwim,
            (false, true) => MoveType::Run,
            (false, false) => MoveType::Walk,
        };
        // `Pet.getMovementSpeedMultiplier()` halves it again for a starving pet (`getMoveSpeed()`
        // already used the halved run and walk speeds).
        self.move_speed() * (1.0 / self.base_move_speed(kind)) * self.pet_speed_factor()
    }

    // ------------------------------------------------------------------ elements

    /// Java `getAttackElement()`: the weapon's element, else the strongest element power.
    pub fn attack_element(&self) -> i8 {
        // `Servitor.getAttackElement()`: the owner's.
        if let Some(owner) = self.servitor_owner_view() {
            return owner.attack_element();
        }
        if let Some(element) = self.weapon_attack_element()
            && element >= 0
        {
            return element;
        }
        let base = self.base();
        let mut best = 0;
        let mut element = -2;
        for (x, stat) in ATTACK_ELEMENT_STATS.into_iter().enumerate() {
            let value = self.calc(stat, f64::from(base.elements[x])) as i32;
            if value > best {
                element = x as i8;
                best = value;
            }
        }
        element
    }

    /// Java `getAttackElementValue(attribute)`.
    pub fn attack_element_value(&self, element: i8) -> i32 {
        // `Servitor.getAttackElementValue(attribute)`: the owner's.
        if let Some(owner) = self.servitor_owner_view() {
            return owner.attack_element_value(element);
        }
        match usize::try_from(element).ok().filter(|e| *e < 6) {
            Some(e) => self.calc(ATTACK_ELEMENT_STATS[e], f64::from(self.base().elements[e])) as i32,
            None => 0,
        }
    }

    /// Java `getDefenseElementValue(attribute)`.
    pub fn defense_element_value(&self, element: i8) -> i32 {
        // `Servitor.getDefenseElementValue(attribute)`: the owner's.
        if let Some(owner) = self.servitor_owner_view() {
            return owner.defense_element_value(element);
        }
        let base = self.base();
        match usize::try_from(element).ok().filter(|e| *e < 6) {
            Some(e) => self.calc(DEFENCE_ELEMENT_STATS[e], base.element_res[e]) as i32,
            None => base.base_element_res as i32,
        }
    }

    // ------------------------------------------------------------------ collision

    /// Java `getCollisionRadius()` (players: the base class template by sex; NPCs: the
    /// template).
    pub fn collision_radius(&self) -> f32 {
        match self.who {
            Who::Player(_) if self.mount_template().is_some() => {
                self.mount_template().map_or(0.0, |t| t.collision_radius)
            }
            Who::Player(_) if self.is_transformed() => self
                .transform_template()
                .map_or_else(|| self.player_collision().0, |t| t.collision_radius),
            Who::Player(_) => self.player_collision().0,
            Who::Npc(_) => self.npc_template().map_or(0.0, |t| t.collision_radius),
            // `DoorData.insertCollisionData`.
            Who::Door(d) => d.template.collision_radius as f32,
        }
    }

    /// Java `getCollisionHeight()`.
    pub fn collision_height(&self) -> f32 {
        match self.who {
            Who::Player(_) if self.mount_template().is_some() => {
                self.mount_template().map_or(0.0, |t| t.collision_height)
            }
            Who::Player(_) if self.is_transformed() => self
                .transform_template()
                .map_or_else(|| self.player_collision().1, |t| t.collision_height),
            Who::Player(_) => self.player_collision().1,
            Who::Npc(_) => self.npc_template().map_or(0.0, |t| t.collision_height),
            Who::Door(d) => d.template.collision_height as f32,
        }
    }
}
