//! What scripts attach to an NPC: `getVariables()`, the script value, `setLethalable` and the
//! over-hit values of an attackable (S-API-04).
// Java: entity/actor/Npc.java (getVariables, hasVariables, _scriptValue, getScriptValue,
//       setScriptValue, isScriptValue), mechanics/variables/NpcVariables.java,
//       AbstractVariables.java
// Java: entity/actor/Creature.java (_lethalable, setLethalable, isLethalable)
// Java: entity/actor/Attackable.java (_overhit, _overhitDamage, _overhitAttacker, overhitEnabled,
//       setOverhitValues, isOverhit, getOverhitAttacker, getOverhitDamage, clearAggroList,
//       calculateOverhitExp)
//
// The values live in the script engine's world state, by NPC object id, and go away with the NPC
// (`script_on_npc_decay`, Java's `Npc.deleteMe`). A script that only needs a value of its own
// uses `ctx.state`; this is for the values Java keeps on the NPC, which other code reads too
// (the lethal effect, the exp of a kill, other scripts).

use std::collections::HashMap;

use l2_util::ObjectId;

use crate::world::World;

/// Java `NpcVariables` (an `AbstractVariables`: a map of named values with typed getters). The
/// values are kept as text like `GlobalVariables`; a value that is no number reads as the default
/// (Java throws).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NpcVariables {
    vars: HashMap<String, String>,
}

impl NpcVariables {
    /// Java `hasVariable(name)`.
    pub fn has_variable(&self, name: &str) -> bool {
        self.vars.contains_key(name)
    }

    /// Java `getString(name, default)`.
    pub fn get_string(&self, name: &str, default: &str) -> String {
        self.vars.get(name).cloned().unwrap_or_else(|| default.to_owned())
    }

    /// Java `getInt(name, default)`.
    pub fn get_int(&self, name: &str, default: i32) -> i32 {
        self.vars
            .get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Java `getLong(name, default)`.
    pub fn get_long(&self, name: &str, default: i64) -> i64 {
        self.vars
            .get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Java `getDouble(name, default)`.
    pub fn get_double(&self, name: &str, default: f64) -> f64 {
        self.vars
            .get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Java `getBoolean(name, default)`: `Boolean.parseBoolean`.
    pub fn get_bool(&self, name: &str, default: bool) -> bool {
        self.vars
            .get(name)
            .map_or(default, |v| v.eq_ignore_ascii_case("true"))
    }

    /// Java `set(name, value)` for text.
    pub fn set(&mut self, name: &str, value: &str) {
        self.vars.insert(name.to_owned(), value.to_owned());
    }

    /// Java `set(name, int)`.
    pub fn set_int(&mut self, name: &str, value: i32) {
        self.set(name, &value.to_string());
    }

    /// Java `set(name, long)`.
    pub fn set_long(&mut self, name: &str, value: i64) {
        self.set(name, &value.to_string());
    }

    /// Java `set(name, double)`.
    pub fn set_double(&mut self, name: &str, value: f64) {
        self.set(name, &value.to_string());
    }

    /// Java `set(name, boolean)`.
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.set(name, if value { "true" } else { "false" });
    }

    /// Java `remove(name)`.
    pub fn remove(&mut self, name: &str) {
        self.vars.remove(name);
    }
}

/// The over-hit values of an attackable.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Overhit {
    enabled: bool,
    damage: f64,
    attacker: Option<ObjectId>,
}

/// Everything scripts attach to one NPC.
#[derive(Debug, Clone, Default)]
pub(crate) struct NpcExtra {
    vars: Option<NpcVariables>,
    script_value: i32,
    /// `!_lethalable` (Java's default is lethalable, so the default here is false).
    not_lethalable: bool,
    /// `!_isMortal` (Java's default is mortal).
    immortal: bool,
    overhit: Overhit,
}

/// The extras of every NPC that has some.
#[derive(Debug, Default)]
pub(crate) struct NpcExtras {
    by_npc: HashMap<ObjectId, NpcExtra>,
}

impl World {
    fn npc_extra_mut(&mut self, npc: ObjectId) -> &mut NpcExtra {
        self.scripts.npc_extras.by_npc.entry(npc).or_default()
    }

    /// Java `Npc.hasVariables()`.
    pub fn npc_has_variables(&self, npc: ObjectId) -> bool {
        self.scripts
            .npc_extras
            .by_npc
            .get(&npc)
            .is_some_and(|e| e.vars.is_some())
    }

    /// Java `Npc.getVariables()` for reading: `None` while the NPC has no variables yet.
    pub fn npc_variables(&self, npc: ObjectId) -> Option<&NpcVariables> {
        self.scripts.npc_extras.by_npc.get(&npc)?.vars.as_ref()
    }

    /// Java `Npc.getVariables()`: the variables, created empty on the first call.
    pub fn npc_variables_mut(&mut self, npc: ObjectId) -> &mut NpcVariables {
        self.npc_extra_mut(npc)
            .vars
            .get_or_insert_with(NpcVariables::default)
    }

    /// Java `Npc.getScriptValue()`.
    pub fn npc_script_value(&self, npc: ObjectId) -> i32 {
        self.scripts
            .npc_extras
            .by_npc
            .get(&npc)
            .map_or(0, |e| e.script_value)
    }

    /// Java `Npc.setScriptValue(value)`.
    pub fn set_npc_script_value(&mut self, npc: ObjectId, value: i32) {
        self.npc_extra_mut(npc).script_value = value;
    }

    /// Java `Npc.isScriptValue(value)`.
    pub fn is_npc_script_value(&self, npc: ObjectId, value: i32) -> bool {
        self.npc_script_value(npc) == value
    }

    /// Java `Creature.setLethalable(value)`.
    pub fn set_lethalable(&mut self, id: ObjectId, value: bool) {
        self.npc_extra_mut(id).not_lethalable = !value;
    }

    /// Java `Creature.isLethalable()` as scripts set it; the class defaults (raid bosses, doors)
    /// are the lethal handler's.
    pub fn is_lethalable_by_script(&self, id: ObjectId) -> bool {
        !self
            .scripts
            .npc_extras
            .by_npc
            .get(&id)
            .is_some_and(|e| e.not_lethalable)
    }

    /// Java `Creature.setMortal(value)` (S-BOSS-02): an immortal creature does not die when its
    /// HP reaches 0.
    pub fn set_mortal(&mut self, id: ObjectId, value: bool) {
        self.npc_extra_mut(id).immortal = !value;
    }

    /// Java `Creature.isMortal()`.
    pub fn is_mortal(&self, id: ObjectId) -> bool {
        !self.scripts.npc_extras.by_npc.get(&id).is_some_and(|e| e.immortal)
    }

    /// Java `NpcConfig.RAID_DISABLE_CURSE` (`DisableRaidCurse`).
    pub fn raid_curse_disabled(&self) -> bool {
        self.settings.cast.raid_disable_curse
    }

    /// Java `Monster.getLeader()`: the master of a minion.
    pub fn npc_leader(&self, npc: ObjectId) -> Option<ObjectId> {
        self.leader_of(npc)
    }

    /// Java `master.getMinionList().onMinionDie(minion, respawnDelay)`: the minion leaves the
    /// list and comes back after `respawn_ms` (`-1` the default, `0` never).
    pub fn minion_died(&mut self, master: ObjectId, minion: ObjectId, respawn_ms: i32) {
        self.minion_list_on_minion_die(master, minion, respawn_ms);
    }

    /// Java `Attackable.useMagic(skill)` (S-BOSS-02): the checks, then `setIntentionCast(skill,
    /// target)` at what the NPC has targeted (`getFirstOfTargetList` of a single-target skill).
    pub fn attackable_use_magic(&mut self, npc: ObjectId, key: crate::effects::SkillKey) {
        let Some(table) = self.skill_table() else {
            return;
        };
        let Some(skill) = table.get(key.id, key.level) else {
            return;
        };
        if self.is_alike_dead(npc)
            || skill.is_passive()
            || self.is_casting_now(npc)
            || self.is_skill_disabled(npc, skill)
        {
            return;
        }
        let (hp, mp) = self.core_of(npc).map_or((0.0, 0.0), |c| (c.cur_hp, c.cur_mp));
        if mp < f64::from(self.mp_consume(npc, skill) + self.mp_initial_consume(npc, skill))
            || hp <= f64::from(skill.hp_consume)
        {
            return;
        }
        if !skill.is_static() {
            if skill.is_magic() {
                if self.is_muted(npc) {
                    return;
                }
            } else if self.is_physical_muted(npc) {
                return;
            }
        }
        if let Some(target) = self.creature_target(npc) {
            self.set_intention_cast(npc, key, Some(target));
        }
    }

    /// Java `Attackable.overhitEnabled(status)`.
    pub fn overhit_enabled(&mut self, npc: ObjectId, status: bool) {
        self.npc_extra_mut(npc).overhit.enabled = status;
    }

    /// Java `Attackable.setOverhitValues(attacker, damage)`: the damage beyond the HP left is
    /// the over-hit; a hit that does not kill clears the values.
    pub fn set_overhit_values(&mut self, npc: ObjectId, attacker: ObjectId, damage: f64) {
        let current_hp = self.core_of(npc).map_or(0.0, |c| c.cur_hp);
        let overhit_dmg = -(current_hp - damage);
        let extra = self.npc_extra_mut(npc);
        if overhit_dmg < 0.0 {
            // We didn't kill the mob with the over-hit strike.
            extra.overhit = Overhit::default();
            return;
        }
        extra.overhit = Overhit {
            enabled: true,
            damage: overhit_dmg,
            attacker: Some(attacker),
        };
    }

    /// Java `Attackable.isOverhit()`.
    pub fn is_overhit(&self, npc: ObjectId) -> bool {
        self.scripts
            .npc_extras
            .by_npc
            .get(&npc)
            .is_some_and(|e| e.overhit.enabled)
    }

    /// Java `Attackable.getOverhitAttacker()`.
    pub fn overhit_attacker(&self, npc: ObjectId) -> Option<ObjectId> {
        self.scripts.npc_extras.by_npc.get(&npc)?.overhit.attacker
    }

    /// Java `Attackable.getOverhitDamage()`.
    pub fn overhit_damage(&self, npc: ObjectId) -> f64 {
        self.scripts
            .npc_extras
            .by_npc
            .get(&npc)
            .map_or(0.0, |e| e.overhit.damage)
    }

    /// Java `Attackable.clearAggroList()`: "clear overhit values".
    pub(crate) fn clear_overhit(&mut self, npc: ObjectId) {
        if let Some(extra) = self.scripts.npc_extras.by_npc.get_mut(&npc) {
            extra.overhit = Overhit::default();
        }
    }

    /// Java `Attackable.calculateOverhitExp(exp)`: up to 25% of the exp, the share of the
    /// over-hit damage in the maximum HP.
    pub(crate) fn calculate_overhit_exp(&self, npc: ObjectId, exp: f64) -> f64 {
        let max_hp = self.stats(npc).map_or(0.0, |v| f64::from(v.max_hp()));
        // Java divides by the double maximum HP; a zero gives infinity, capped below.
        let mut percentage = (self.overhit_damage(npc) * 100.0) / max_hp;
        // Over-hit damage percentages are limited to 25% max.
        if percentage > 25.0 {
            percentage = 25.0;
        }
        (percentage / 100.0) * exp
    }

    /// Java `Npc.deleteMe`: everything attached to the NPC goes with it.
    pub(crate) fn script_forget_npc_extras(&mut self, npc: ObjectId) {
        self.scripts.npc_extras.by_npc.remove(&npc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_read_like_abstract_variables() {
        let mut v = NpcVariables::default();
        v.set_int("a", 5);
        v.set_bool("b", true);
        v.set("c", "x");
        v.set_double("d", 1.5);
        assert_eq!(v.get_int("a", 0), 5);
        assert!(v.get_bool("b", false));
        assert_eq!(v.get_int("c", 9), 9);
        assert_eq!(v.get_double("d", 0.0), 1.5);
        assert!(v.has_variable("c"));
        v.remove("c");
        assert!(!v.has_variable("c"));
        assert_eq!(v.get_string("c", "d"), "d");
    }
}
