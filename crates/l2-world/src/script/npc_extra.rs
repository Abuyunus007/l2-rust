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
    overhit: Overhit,
    /// `Attackable.setSeeThroughSilentMove(true)`.
    see_through_silent_move: bool,
    /// `!_isMortal` (the default is mortal).
    immortal: bool,
    /// `!QuestGuard._isAutoAttackable` (Java's default is true).
    quest_guard_not_auto_attackable: bool,
    /// `QuestGuard._isPassive`.
    quest_guard_passive: bool,
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

    /// Java `Monster.getLeader()`: the master of a minion.
    pub fn npc_leader(&self, npc: ObjectId) -> Option<ObjectId> {
        self.leader_of(npc)
    }

    /// Java `npc.getTemplate().getParameters().getMinionList(name)`: the ids of the minions.
    pub fn npc_template_minion_ids(&self, npc: ObjectId, name: &str) -> Vec<i32> {
        self.npc_template(npc)
            .map_or_else(Vec::new, |t| t.minion_list(name).iter().map(|m| m.id).collect())
    }

    /// Java `Npc.getAIValue(name)` for `hasAIValue(name)`: the `AIData` value of the NPC's
    /// spawn; `None` when the spawn has none.
    pub fn npc_ai_value(&self, npc: ObjectId, name: &str) -> Option<i32> {
        self.spawn_ai_value(npc, name)
    }

    /// Java `Npc.staysInSpawnLoc()`: the spawn point's x and y are the NPC's.
    pub fn npc_stays_in_spawn_loc(&self, npc: ObjectId) -> bool {
        let Some(n) = self.npcs.get(&npc) else {
            return false;
        };
        n.spawn.is_some()
            && self
                .position_of(npc)
                .is_some_and(|(x, y, _)| n.spawn_point.0 == x && n.spawn_point.1 == y)
    }

    /// Java `Npc.broadcastEvent(eventName, radius, reference)`: `ON_NPC_EVENT_RECEIVED` to every
    /// NPC with the listener within `radius` of the sender.
    pub fn broadcast_event(
        &mut self,
        sender: ObjectId,
        event: &str,
        radius: i32,
        reference: Option<ObjectId>,
    ) {
        for obj in self.visible_objects_in_range(sender, radius) {
            if self.npcs.contains_key(&obj) {
                self.script_send_event(event, sender, obj, reference);
            }
        }
    }

    /// Java `Npc.addAttackerToAttackByList(attacker)`.
    pub fn add_attacker_to_attack_by(&mut self, npc: ObjectId, attacker: ObjectId) {
        self.add_attacker_to_attack_by_list(npc, attacker);
    }

    /// Java `Npc.scheduleDespawn(delay)`.
    pub fn schedule_npc_despawn(&mut self, npc: ObjectId, delay_ms: i64) {
        self.schedule_script_despawn(npc, delay_ms);
    }

    /// Java `npc.getWorldRegion().isActive()`.
    pub fn npc_region_is_active(&self, npc: ObjectId) -> bool {
        self.objects
            .get(&npc)
            .and_then(|e| e.region)
            .is_some_and(|r| self.is_region_active(r))
    }

    /// Java `GeoEngine.getValidLocation(npc.getX(), npc.getY(), npc.getZ(), x, y, npc.getZ(), 0)`:
    /// how far toward (x, y) the NPC can go.
    pub fn npc_valid_location_toward(&self, npc: ObjectId, x: i32, y: i32) -> Option<(i32, i32, i32)> {
        let (nx, ny, nz) = self.position_of(npc)?;
        let instance = self.instance_id_of(npc);
        let to = self.geo_valid_location(
            l2_geo::Location::new(nx, ny, nz),
            l2_geo::Location::new(x, y, nz),
            instance,
        );
        Some((to.x, to.y, to.z))
    }

    /// Java `npc.getFort().getOwnerClan().getId()`; 0 without a fortress or an owner.
    pub fn npc_fort_owner_clan_id(&self, npc: ObjectId) -> i32 {
        self.npc_fort(npc)
            .and_then(|f| self.fort(f))
            .map_or(0, |f| f.owner_id)
    }

    /// Java `player.getAllActiveQuests().size()`.
    pub fn active_quest_count(&self, player: ObjectId) -> usize {
        self.all_active_quests(player).len()
    }

    /// Java `npc.getRace()` as the `Race` ordinal of the six player races (`script::race`);
    /// -1 for any other race.
    pub fn npc_race_ordinal(&self, npc: ObjectId) -> i32 {
        use l2_data::npc::NpcRace;
        match self.npc_template(npc).map(|t| t.race) {
            Some(NpcRace::Human) => 0,
            Some(NpcRace::Elf) => 1,
            Some(NpcRace::DarkElf) => 2,
            Some(NpcRace::Orc) => 3,
            Some(NpcRace::Dwarf) => 4,
            Some(NpcRace::Kamael) => 5,
            _ => -1,
        }
    }

    /// Java `Creature.setMortal(value)`.
    pub fn set_mortal(&mut self, id: ObjectId, value: bool) {
        self.npc_extra_mut(id).immortal = !value;
    }

    /// Java `Creature.isMortal()`.
    pub fn is_mortal(&self, id: ObjectId) -> bool {
        !self
            .scripts
            .npc_extras
            .by_npc
            .get(&id)
            .is_some_and(|e| e.immortal)
    }

    /// Java `Attackable.setSeeThroughSilentMove(value)`.
    pub fn set_see_through_silent_move(&mut self, id: ObjectId, value: bool) {
        self.npc_extra_mut(id).see_through_silent_move = value;
    }

    /// Java `Attackable.canSeeThroughSilentMove()`.
    pub fn can_see_through_silent_move(&self, id: ObjectId) -> bool {
        self.scripts
            .npc_extras
            .by_npc
            .get(&id)
            .is_some_and(|e| e.see_through_silent_move)
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

    /// Java `QuestGuard.setAutoAttackable(value)`.
    pub fn set_quest_guard_auto_attackable(&mut self, id: ObjectId, value: bool) {
        self.npc_extra_mut(id).quest_guard_not_auto_attackable = !value;
    }

    /// Java `QuestGuard._isAutoAttackable`.
    pub fn is_quest_guard_auto_attackable(&self, id: ObjectId) -> bool {
        !self
            .scripts
            .npc_extras
            .by_npc
            .get(&id)
            .is_some_and(|e| e.quest_guard_not_auto_attackable)
    }

    /// Java `QuestGuard.setPassive(value)`.
    pub fn set_quest_guard_passive(&mut self, id: ObjectId, value: bool) {
        self.npc_extra_mut(id).quest_guard_passive = value;
    }

    /// Java `QuestGuard.isPassive()`.
    pub fn is_quest_guard_passive(&self, id: ObjectId) -> bool {
        self.scripts
            .npc_extras
            .by_npc
            .get(&id)
            .is_some_and(|e| e.quest_guard_passive)
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
