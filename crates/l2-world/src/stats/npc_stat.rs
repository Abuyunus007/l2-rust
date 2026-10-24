//! The NPC-only parts of the stat getters and the standard NPC calculators.
// Java: entity/actor/stat/NpcStat.java (getLevel)
// Java: entity/actor/Creature.java (the constructor: `_calculators = NPC_STD_CALCULATOR`)
// Java: entity/actor/Npc.java (getTemplate)
//
// NPCs start with the one shared standard set (`Formulas.getStdNPCCalculators`). The functions
// of their template skills join at spawn in phase 2 of M4-STAT-01 (the skill tables), which
// copies the set for that NPC only.

use std::sync::Arc;

use l2_data::NpcTemplate;
use l2_util::ObjectId;

use super::calculators::{CalcSet, Calculators};
use super::creature_stat::{StatView, Who};
use crate::world::World;

/// The shared standard calculators of NPCs (Java `NPC_STD_CALCULATOR`). One per world.
#[derive(Debug, Clone)]
pub(crate) struct StdNpcCalculators(pub Arc<CalcSet>);

impl Default for StdNpcCalculators {
    fn default() -> Self {
        Self(Arc::new(CalcSet::std_npc()))
    }
}

impl<'w> StatView<'w> {
    /// The NPC's template.
    pub fn npc_template(&self) -> Option<&'w NpcTemplate> {
        match self.who {
            Who::Npc(n) => self.world().npc_world.template(n.npc_id),
            Who::Player(_) | Who::Door(_) => None,
        }
    }

    /// Java `NpcStat.getLevel()`: the template's level.
    pub(crate) fn npc_level(&self) -> i32 {
        match self.npc_template() {
            Some(t) => i32::from(t.level),
            None => self.core().level,
        }
    }

    /// Java `NpcStat.getPhysicalAttackAngle()`.
    pub fn physical_attack_angle(&self) -> i32 {
        self.npc_template().map_or(0, |t| t.base_attack_angle)
    }
}

impl World {
    /// A new NPC's calculators: the shared standard set.
    pub(crate) fn npc_calculators(&self) -> Calculators {
        Calculators::shared(&self.std_npc_calculators.0)
    }

    /// Java `Npc.onSpawn` -> `setCurrentHpMp(getMaxHp(), getMaxMp())`: full hit points.
    pub(crate) fn init_npc_stats(&mut self, id: ObjectId) {
        let Some((hp, mp)) = self
            .stats(id)
            .map(|v| (f64::from(v.max_hp()), f64::from(v.max_mp())))
        else {
            return;
        };
        if let Some(npc) = self.npcs.get_mut(&id) {
            npc.core.max_hp = hp;
            npc.core.max_mp = mp;
        }
        self.set_current_hp_mp(id, hp, mp);
    }
}
