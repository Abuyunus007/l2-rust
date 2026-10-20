//! The hate helpers of the aggro effects: `Attackable.reduceHate(target, amount)` for one target,
//! and `NpcTemplate.isClan(clans)`. Shared by `AddHate`, `GetAgro` and the hate effects.
// Java: entity/actor/Attackable.java (reduceHate), entity/actor/templates/NpcTemplate.java (isClan)

use std::collections::BTreeSet;

use l2_util::ObjectId;

use crate::world::World;

/// Java `Attackable.reduceHate(target, amount)` for a target that is not `null`: the hate of the
/// target grows by `amount` (negative takes hate away). When the target's hate is not negative
/// and nobody is hated any more, the NPC drops its aggro list, goes back to the active intention
/// and walks.
// NOTE: the siege guards' branch (`SiegeGuardAI`, `FortSiegeGuardAI`) and the whole-list branch
// (`target == null`) are not used by the effects; they are not ported.
pub(crate) fn reduce_hate(world: &mut World, npc: ObjectId, target: ObjectId, amount: i64) {
    let hate = {
        let state = world.combat_state_mut(npc);
        let Some(info) = state.aggro.iter_mut().find(|a| a.attacker == target) else {
            return;
        };
        info.add_hate(amount);
        info.hate
    };
    if hate >= 0 && world.aggro_most_hated(npc).is_none() {
        // Java `AttackableAI.setGlobalAggro(-25)`.
        if let Some(ai) = world.ai.creatures.get_mut(&npc) {
            ai.global_aggro = -25;
        }
        world.aggro_clear(npc);
        world.set_intention_active(npc);
        // Java `setWalking()` (fake players are not ported).
        world.set_running(npc, false);
    }
}

/// Java `NpcTemplate.isClan(clans)`: the NPC belongs to one of `clans`, or to the generic clan
/// (the clan every NPC of the generic class shares). An NPC without clans is in none.
pub(crate) fn is_clan(npc_clans: &BTreeSet<i32>, clans: &BTreeSet<i32>, generic_clan: i32) -> bool {
    if npc_clans.contains(&generic_clan) {
        return true;
    }
    clans.iter().any(|clan| npc_clans.contains(clan))
}
