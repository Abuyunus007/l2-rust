//! A summon in its owner's party: the pet window of the other members, the summon block of
//! `PartySmallWindowAll` and the summon's `PartySpelled`.
// Java: network/serverpackets/{ExPartyPetWindowAdd,ExPartyPetWindowUpdate,
//       ExPartyPetWindowDelete,PartySmallWindowAll}.java
// Java: entity/actor/Summon.java (onSpawn, updateAndBroadcastStatus, unSummon, deleteMe: the
//       `broadcastToPartyMembers(owner, ...)` lines)
// Java: entity/actor/holders/creature/EffectList.java (updateEffectIcons, the summon branch)

use l2_protocol_game::server::{
    EffectIcon, ExPartyPetWindowAdd, ExPartyPetWindowDelete, ExPartyPetWindowUpdate, PartyPetWindow,
    PartySpelled, PartySummon,
};
use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// The name and numbers a party packet shows of a summon. `display_id` picks the template's
    /// display id (the pet window packets) or its id (`PartySmallWindowAll` writes `getId()`).
    pub(crate) fn party_summon(&self, summon: ObjectId, display_id: bool) -> Option<PartySummon> {
        let state = self.summon(summon)?;
        let npc = self.npcs.get(&summon)?;
        let template = self.npc_world.template(npc.npc_id)?;
        let stats = self.stats(summon)?;
        Some(PartySummon {
            object_id: summon.0,
            display_id: if display_id {
                template.display_id
            } else {
                template.id
            },
            summon_type: state.kind.summon_type(),
            name: state.name.clone(),
            hp: npc.core.cur_hp as i32,
            max_hp: stats.max_hp(),
            mp: npc.core.cur_mp as i32,
            max_mp: stats.max_mp(),
            level: stats.level(),
        })
    }

    fn party_pet_window(&self, summon: ObjectId) -> Option<PartyPetWindow> {
        Some(PartyPetWindow {
            summon: self.party_summon(summon, true)?,
            owner_object_id: self.summon_owner(summon)?.0,
        })
    }

    /// `new ExPartyPetWindowAdd(summon)`.
    pub(crate) fn party_pet_window_add(&self, summon: ObjectId) -> Option<ExPartyPetWindowAdd> {
        Some(ExPartyPetWindowAdd {
            window: self.party_pet_window(summon)?,
        })
    }

    /// `new ExPartyPetWindowDelete(summon)`.
    pub(crate) fn party_pet_window_delete(&self, summon: ObjectId) -> Option<ExPartyPetWindowDelete> {
        Some(ExPartyPetWindowDelete {
            summon_object_id: summon.0,
            owner_object_id: self.summon_owner(summon)?.0,
            summon_name: self.summon(summon)?.name.clone(),
        })
    }

    /// `party.broadcastToPartyMembers(owner, new ExPartyPetWindowAdd(this))` when the owner is in
    /// a party.
    pub(crate) fn summon_party_window_add(&self, summon: ObjectId) {
        let Some(owner) = self.summon_owner(summon) else {
            return;
        };
        if let (Some(party), Some(packet)) = (self.party_of(owner), self.party_pet_window_add(summon)) {
            self.broadcast_party_except(party, owner, &packet);
        }
    }

    /// `party.broadcastToPartyMembers(owner, new ExPartyPetWindowUpdate(this))`.
    pub(crate) fn summon_party_window_update(&self, summon: ObjectId) {
        let Some(owner) = self.summon_owner(summon) else {
            return;
        };
        if let (Some(party), Some(window)) = (self.party_of(owner), self.party_pet_window(summon)) {
            self.broadcast_party_except(party, owner, &ExPartyPetWindowUpdate { window });
        }
    }

    /// `party.broadcastToPartyMembers(owner, new ExPartyPetWindowDelete(this))`.
    pub(crate) fn summon_party_window_delete(&self, summon: ObjectId, owner: ObjectId) {
        if let (Some(party), Some(packet)) = (self.party_of(owner), self.party_pet_window_delete(summon)) {
            self.broadcast_party_except(party, owner, &packet);
        }
    }

    /// The summon branch of the icon task of `EffectList.updateEffectIcons()`: `ps` (every icon
    /// in use, toggles too) goes to the owner, `psSummon` (no toggles) to the other members of
    /// its party.
    pub(crate) fn send_summon_party_spelled(&mut self, summon: ObjectId) {
        let Some(owner) = self.summon_owner(summon) else {
            return;
        };
        let creature_type = if self.is_servitor(summon) {
            PartySpelled::SERVITOR
        } else {
            PartySpelled::PET
        };
        let mut ps = PartySpelled {
            creature_type,
            creature_id: summon.0,
            effects: Vec::new(),
        };
        let mut ps_summon = ps.clone();
        if let Some(list) = self.effect_list(summon) {
            for info in list.effects().filter(|i| i.in_use && !i.facts.healing_potion()) {
                let icon = EffectIcon {
                    display_id: info.facts.display_id,
                    display_level: info.facts.display_level,
                    time: info.time(self.tick),
                    in_use: true,
                };
                if !info.facts.toggle {
                    ps_summon.add_skill(icon);
                }
                ps.add_skill(icon);
            }
        }
        if let Some(party) = self.party_of(owner) {
            // Send to all members except the owner, then to the owner.
            self.broadcast_party_except(party, owner, &ps_summon);
        }
        self.send(owner, &ps);
    }
}
