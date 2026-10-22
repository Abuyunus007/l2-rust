//! The pet-only parts of the stat getters: the numbers of the pet's level row, hunger and
//! starvation.
// Java: entity/actor/stat/PetStat.java
// Java: entity/actor/instance/Pet.java (getRunSpeed, getWalkSpeed, getMovementSpeedMultiplier,
//       getSkillLevel)

use l2_data::PetLevelData;

use super::creature_stat::StatView;

impl<'w> StatView<'w> {
    /// Java `isPet()`.
    pub fn is_pet(&self) -> bool {
        self.npc().is_some_and(|n| self.world().is_pet(n.core.object_id))
    }

    /// Java `Pet.getPetLevelData()`: `None` for anything but a pet.
    pub(crate) fn pet_level_data(&self) -> Option<&'w PetLevelData> {
        let npc = self.npc()?;
        self.world().pet_level_data_of(npc.core.object_id)
    }

    /// Java `Pet.isHungry()`.
    pub(crate) fn pet_is_hungry(&self) -> bool {
        self.is_pet() && self.world().pet_is_hungry(self.object_id())
    }

    /// Java `Pet.isUncontrollable()`: the speed factor of a starving pet is a half.
    pub(crate) fn pet_speed_factor(&self) -> f64 {
        if self.is_pet() && self.world().pet_is_uncontrollable(self.object_id()) {
            0.5
        } else {
            1.0
        }
    }

    /// The class id of the pet's owner (`getActiveChar().asPlayer().getPlayerClass().getId()`).
    pub(crate) fn pet_owner_class_id(&self) -> Option<i32> {
        let owner = self.world().summon_owner(self.object_id())?;
        self.world().players.get(&owner).map(|p| p.data.class_id)
    }
}
