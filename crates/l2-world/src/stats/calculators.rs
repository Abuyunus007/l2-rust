//! The function lists of one creature, one per stat.
// Java: mechanics/stats/Calculator.java (addFunc, removeFunc, removeOwner, equalsCals)
// Java: mechanics/stats/functions/AbstractFunction.java, FuncTemplate.java (getFunc)
// Java: entity/actor/Creature.java (_calculators, NPC_STD_CALCULATOR, addStatFunc,
//       removeStatFunc, removeStatsOwner)
// Java: mechanics/stats/Formulas.java (getStdNPCCalculators, getStdDoorCalculators,
//       addFuncsToNewCharacter)
//
// Java keeps one `Calculator` object per stat (null when the stat has no function) and replaces
// its function array on every change. Here a [`CalcSet`] holds one `Vec<Func>` per stat (empty =
// Java's null calculator) and a [`Calculators`] holds the set behind an `Arc`: NPCs share the
// standard NPC set until something is added, then `Arc::make_mut` copies it (Java: "create a copy
// of NPC_STD_CALCULATOR in _calculators before adding new Func object").

use std::sync::Arc;

use l2_data::stats::NUM_STATS;
use l2_data::{Condition, FuncKind, FuncTemplate, Stat};
use l2_util::ObjectId;

use super::std_funcs::StdFunc;

/// Java `AbstractFunction._funcOwner`: what added a function, so it can be removed again. Java
/// compares owners by identity; ids say the same thing (D-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FuncOwner {
    /// The standard formula functions (Java: owner `null`).
    Std,
    /// An equipped item (Java: the `Item` instance), by object id.
    Item(ObjectId),
    /// A skill (Java: the `Skill`, or the passive effect it started).
    Skill { id: i32, level: i32 },
    /// An armor set's own functions, by chest item id.
    ArmorSet(i32),
}

/// Which `Func*` class a function is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuncCalc {
    /// `FuncSet`, `FuncAdd`, ... made from a [`FuncTemplate`].
    Template(FuncKind),
    /// One of the formula functions of `Formulas` (`FuncPAtkMod`, `FuncHenna`, ...).
    Std(StdFunc),
}

/// Java `AbstractFunction`: one step of a stat calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct Func {
    pub stat: Stat,
    /// Lower orders run first; equal orders keep the order they were added in.
    pub order: i32,
    pub calc: FuncCalc,
    pub value: f64,
    pub owner: FuncOwner,
    /// Java `_applyCond`: tested every time the function runs. `None` means always.
    pub cond: Option<Arc<Condition>>,
}

impl Func {
    /// A formula function (Java: the `getInstance()` singletons, order 1, owner `null`).
    pub fn std(func: StdFunc) -> Self {
        Self {
            stat: func.stat(),
            order: 1,
            calc: FuncCalc::Std(func),
            value: 0.0,
            owner: FuncOwner::Std,
            cond: None,
        }
    }

    /// Java `FuncTemplate.getFunc` after the attach condition held: the function made from
    /// `template` for `owner`.
    pub fn from_template(template: &FuncTemplate, owner: FuncOwner) -> Self {
        Self {
            stat: template.stat,
            order: template.order,
            calc: FuncCalc::Template(template.kind),
            value: template.value,
            owner,
            cond: template.apply_cond.clone(),
        }
    }
}

/// Java `Calculator[]`: the functions of every stat.
#[derive(Debug, Clone, PartialEq)]
pub struct CalcSet {
    slots: Vec<Vec<Func>>,
}

impl Default for CalcSet {
    fn default() -> Self {
        Self {
            slots: vec![Vec::new(); NUM_STATS],
        }
    }
}

impl CalcSet {
    /// The functions of `stat`, in calculation order.
    pub fn funcs(&self, stat: Stat) -> &[Func] {
        self.slots
            .get(stat.ordinal())
            .map_or(&[][..], |list| list.as_slice())
    }

    /// Java `Calculator.addFunc`: inserted after every function whose order is lower or equal.
    pub fn add(&mut self, func: Func) {
        let Some(list) = self.slots.get_mut(func.stat.ordinal()) else {
            return;
        };
        let at = list
            .iter()
            .position(|f| func.order < f.order)
            .unwrap_or(list.len());
        list.insert(at, func);
    }

    /// Java `Calculator.removeFunc`: the first function equal to `func`. `false` when there is
    /// none.
    // NOTE(java-quirk): Java's removeFunc drops the *last* function of the calculator when the
    // one asked for is not in it (its loop stops one short and the "not found" check can never
    // be true). Java only calls it with functions it added, so this never happens there; here a
    // function that is not present changes nothing.
    pub fn remove(&mut self, func: &Func) -> bool {
        let Some(list) = self.slots.get_mut(func.stat.ordinal()) else {
            return false;
        };
        match list.iter().position(|f| f == func) {
            Some(at) => {
                list.remove(at);
                true
            }
            None => false,
        }
    }

    /// Whether any function belongs to `owner`.
    fn has_owner(&self, owner: FuncOwner) -> bool {
        self.slots.iter().flatten().any(|f| f.owner == owner)
    }

    /// Java `Calculator.removeOwner` over every stat in ordinal order: the stat of every removed
    /// function, once per function (so a stat can appear twice).
    fn remove_owner(&mut self, owner: FuncOwner) -> Vec<Stat> {
        let mut stats = Vec::new();
        for list in &mut self.slots {
            list.retain(|f| {
                let keep = f.owner != owner;
                if !keep {
                    stats.push(f.stat);
                }
                keep
            });
        }
        stats
    }

    /// Number of functions over all stats.
    pub fn len(&self) -> usize {
        self.slots.iter().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Java `Formulas.getStdNPCCalculators()`.
    pub fn std_npc() -> Self {
        let mut set = Self::default();
        for func in [
            StdFunc::MaxHpMul,
            StdFunc::MaxMpMul,
            StdFunc::PAtkMod,
            StdFunc::MAtkMod,
            StdFunc::PDefMod,
            StdFunc::MDefMod,
            StdFunc::AtkCritical,
            StdFunc::MAtkCritical,
            StdFunc::AtkAccuracy,
            StdFunc::AtkEvasion,
            StdFunc::PAtkSpeed,
            StdFunc::MAtkSpeed,
            StdFunc::MoveSpeed,
        ] {
            set.add(Func::std(func));
        }
        set
    }

    /// Java `Formulas.getStdDoorCalculators()`. Doors keep their own fixed state in M4 (no stat
    /// reads them yet); the set is here so the door card can use it.
    pub fn std_door() -> Self {
        let mut set = Self::default();
        for func in [
            StdFunc::AtkAccuracy,
            StdFunc::AtkEvasion,
            StdFunc::GatesPDefMod,
            StdFunc::GatesMDefMod,
        ] {
            set.add(Func::std(func));
        }
        set
    }

    /// Java `Formulas.addFuncsToNewCharacter` for a player, in Java's order.
    pub fn std_player() -> Self {
        let mut set = Self::default();
        for func in [
            StdFunc::MaxHpMul,
            StdFunc::MaxCpMul,
            StdFunc::MaxMpMul,
            StdFunc::PAtkMod,
            StdFunc::MAtkMod,
            StdFunc::PDefMod,
            StdFunc::MDefMod,
            StdFunc::AtkCritical,
            StdFunc::MAtkCritical,
            StdFunc::AtkAccuracy,
            StdFunc::AtkEvasion,
            StdFunc::PAtkSpeed,
            StdFunc::MAtkSpeed,
            StdFunc::MoveSpeed,
        ] {
            set.add(Func::std(func));
        }
        for wrap in [StdFunc::Henna as fn(Stat) -> StdFunc, StdFunc::ArmorSet] {
            for stat in [
                Stat::StatStr,
                Stat::StatDex,
                Stat::StatInt,
                Stat::StatMen,
                Stat::StatCon,
                Stat::StatWit,
            ] {
                set.add(Func::std(wrap(stat)));
            }
        }
        set
    }
}

/// Java `Creature._calculators`: a creature's [`CalcSet`], possibly shared with every other NPC
/// that has nothing of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct Calculators {
    set: Arc<CalcSet>,
    /// The shared standard set an NPC goes back to when it has nothing of its own any more
    /// (Java compares with `NPC_STD_CALCULATOR`). `None` for players.
    std: Option<Arc<CalcSet>>,
}

impl Default for Calculators {
    /// An empty set: every stat is its base value.
    fn default() -> Self {
        Self::own(CalcSet::default())
    }
}

impl Calculators {
    /// A set owned by one creature (players: [`CalcSet::std_player`]).
    pub fn own(set: CalcSet) -> Self {
        Self {
            set: Arc::new(set),
            std: None,
        }
    }

    /// An NPC's calculators: the shared standard set, copied on the first change.
    pub fn shared(std: &Arc<CalcSet>) -> Self {
        Self {
            set: Arc::clone(std),
            std: Some(Arc::clone(std)),
        }
    }

    /// Whether this still is the shared standard set (Java `_calculators == NPC_STD_CALCULATOR`).
    pub fn is_shared(&self) -> bool {
        self.std.as_ref().is_some_and(|s| Arc::ptr_eq(s, &self.set))
    }

    /// The whole set.
    pub fn set(&self) -> &CalcSet {
        &self.set
    }

    /// The functions of `stat`, in calculation order.
    pub fn funcs(&self, stat: Stat) -> &[Func] {
        self.set.funcs(stat)
    }

    /// Java `Creature.addStatFunc`.
    pub fn add(&mut self, func: Func) {
        Arc::make_mut(&mut self.set).add(func);
    }

    /// Java `Creature.addStatFuncs` without the broadcast: the stats of the added functions, in
    /// order (the caller broadcasts them, see `World::broadcast_modified_stats`).
    pub fn add_all(&mut self, funcs: Vec<Func>) -> Vec<Stat> {
        let stats = funcs.iter().map(|f| f.stat).collect();
        for func in funcs {
            self.add(func);
        }
        stats
    }

    /// Java `Creature.removeStatFunc`.
    pub fn remove(&mut self, func: &Func) -> bool {
        if !self.set.funcs(func.stat).contains(func) {
            return false;
        }
        let removed = Arc::make_mut(&mut self.set).remove(func);
        self.relink();
        removed
    }

    /// Java `Creature.removeStatsOwner` without the broadcast: the stats that changed.
    // NOTE(java-quirk): Java runs `removeOwner` on the shared NPC calculators themselves, so a
    // call with owner `null` would strip the standard functions of every NPC. Here the shared
    // set is never changed: an owner with no function changes nothing, any other change copies
    // first.
    pub fn remove_owner(&mut self, owner: FuncOwner) -> Vec<Stat> {
        if !self.set.has_owner(owner) {
            return Vec::new();
        }
        let stats = Arc::make_mut(&mut self.set).remove_owner(owner);
        self.relink();
        stats
    }

    /// "If possible, free the memory and just create a link on NPC_STD_CALCULATOR."
    // Java compares function identity (`equalsCals`); the standard functions are singletons
    // there, so equal contents mean the same thing here.
    fn relink(&mut self) {
        if let Some(std) = &self.std
            && !Arc::ptr_eq(std, &self.set)
            && *self.set == **std
        {
            self.set = Arc::clone(std);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template(kind: FuncKind, order: i32, value: f64) -> Func {
        Func::from_template(
            &FuncTemplate::with_kind(None, None, kind, order, Stat::PowerAttack, value),
            FuncOwner::Item(ObjectId(7)),
        )
    }

    #[test]
    fn add_inserts_after_equal_orders() {
        let mut set = CalcSet::default();
        set.add(template(FuncKind::Add, 30, 1.0));
        set.add(template(FuncKind::Mul, 20, 2.0));
        set.add(template(FuncKind::Add, 30, 3.0));
        set.add(template(FuncKind::Set, 0, 4.0));
        set.add(template(FuncKind::Mul, 20, 5.0));
        let values: Vec<f64> = set.funcs(Stat::PowerAttack).iter().map(|f| f.value).collect();
        // Java: order 0, then the two 20s and the two 30s, each pair in insertion order.
        assert_eq!(values, [4.0, 2.0, 5.0, 1.0, 3.0]);
    }

    #[test]
    fn item_functions_go_before_the_formula_functions() {
        let mut set = CalcSet::std_player();
        set.add(template(FuncKind::Set, 0, 8.0));
        let funcs = set.funcs(Stat::PowerAttack);
        assert_eq!(funcs.len(), 2);
        assert_eq!(funcs[0].calc, FuncCalc::Template(FuncKind::Set));
        assert_eq!(funcs[1].calc, FuncCalc::Std(StdFunc::PAtkMod));
    }

    #[test]
    fn standard_sets_have_javas_layout() {
        let npc = CalcSet::std_npc();
        assert_eq!(npc.len(), 13);
        assert!(npc.funcs(Stat::MaxCp).is_empty());
        assert_eq!(
            npc.funcs(Stat::MoveSpeed)[0].calc,
            FuncCalc::Std(StdFunc::MoveSpeed)
        );
        let player = CalcSet::std_player();
        assert_eq!(player.len(), 14 + 12);
        assert_eq!(
            player
                .funcs(Stat::StatStr)
                .iter()
                .map(|f| f.calc)
                .collect::<Vec<_>>(),
            [
                FuncCalc::Std(StdFunc::Henna(Stat::StatStr)),
                FuncCalc::Std(StdFunc::ArmorSet(Stat::StatStr))
            ]
        );
        assert!(player.funcs(Stat::StatStr).iter().all(|f| f.order == 1));
        let door = CalcSet::std_door();
        assert_eq!(door.len(), 4);
        assert_eq!(
            door.funcs(Stat::PowerDefence)[0].calc,
            FuncCalc::Std(StdFunc::GatesPDefMod)
        );
    }

    #[test]
    fn remove_owner_returns_one_stat_per_function() {
        let mut calcs = Calculators::own(CalcSet::std_player());
        calcs.add(template(FuncKind::Set, 0, 8.0));
        calcs.add(template(FuncKind::Enchant, 0, 0.0));
        let mut other = template(FuncKind::Add, 0, 1.0);
        other.owner = FuncOwner::Item(ObjectId(8));
        calcs.add(other);
        assert_eq!(
            calcs.remove_owner(FuncOwner::Item(ObjectId(7))),
            [Stat::PowerAttack, Stat::PowerAttack]
        );
        assert_eq!(calcs.funcs(Stat::PowerAttack).len(), 2);
        assert!(calcs.remove_owner(FuncOwner::Item(ObjectId(7))).is_empty());
    }

    #[test]
    fn npcs_share_the_standard_set_until_they_change_it() {
        let std = Arc::new(CalcSet::std_npc());
        let mut a = Calculators::shared(&std);
        let b = Calculators::shared(&std);
        assert!(a.is_shared() && b.is_shared());
        // Removing an owner that has nothing does not copy.
        assert!(a.remove_owner(FuncOwner::Skill { id: 1, level: 1 }).is_empty());
        assert!(a.is_shared());

        let mut func = template(FuncKind::Mul, 20, 1.5);
        func.owner = FuncOwner::Skill { id: 4408, level: 1 };
        a.add(func.clone());
        assert!(!a.is_shared(), "copy on write");
        assert!(b.is_shared(), "the others keep the shared set");
        assert_eq!(std.len(), 13, "the shared set is never changed");
        assert_eq!(a.set().len(), 14);

        // Taking it away again links back to the shared set (Java: equalsCals over every stat).
        assert_eq!(
            a.remove_owner(FuncOwner::Skill { id: 4408, level: 1 }),
            [Stat::PowerAttack]
        );
        assert!(a.is_shared());

        a.add(func.clone());
        assert!(a.remove(&func));
        assert!(a.is_shared());
        assert!(!a.remove(&func));
    }

    #[test]
    fn players_own_their_set() {
        let mut p = Calculators::own(CalcSet::std_player());
        assert!(!p.is_shared());
        p.add(template(FuncKind::Add, 0, 1.0));
        assert!(!p.remove_owner(FuncOwner::Item(ObjectId(7))).is_empty());
        assert!(!p.is_shared());
    }
}
