//! The soul crystal table for scripts and the crystal exchange of quest 350 (S-API-05).
// Java: data/xml/LevelUpCrystalData.java (the table, `getInstance()`),
//       quests/Q00350_EnhanceYourWeapon (`exchangeCrystal`)
//
// The table is loaded by the game server (`l2_data::LevelUpCrystalData::load` with
// `SoulCrystalChanceMultiplier`) and installed before the scripts, so Q00350's
// `Script::setup_with` can register its kill and skill-see ids from it. The absorb list of a
// monster (`Attackable.addAbsorber`, ...) is in `npc_extra.rs`.

use std::sync::Arc;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

pub use l2_data::level_up_crystal::{
    AbsorbCrystalType, LevelUpCrystalData, LevelingSoulCrystalInfo, SoulCrystal,
};

use crate::inventory::item_name_param;
use crate::world::World;

impl World {
    /// Installs `LevelUpCrystalData` (`data/LevelUpCrystalData.xml`). Call before
    /// [`World::install_scripts`]: Q00350 registers its monsters from it.
    pub fn set_level_up_crystal_data(&mut self, data: Arc<LevelUpCrystalData>) {
        self.scripts.level_up_crystals = data;
    }

    /// Java `LevelUpCrystalData.getInstance()`. Empty until the game server installs it.
    pub fn level_up_crystal_data(&self) -> &LevelUpCrystalData {
        &self.scripts.level_up_crystals
    }

    /// Q00350 `exchangeCrystal(player, mob, takeId, giveId, broke)`: one `take_id` out
    /// (`PlayerInventory.destroyItemByItemId`), one `give_id` in (`PlayerInventory.addItem`), the
    /// absorb (or broke) message, "You have earned $s1" and one inventory update with both.
    /// Nothing happens when the player has no `take_id`.
    pub fn exchange_soul_crystal(&mut self, player: ObjectId, take_id: i32, give_id: i32, broke: bool) {
        let Some(object_id) = self
            .inventory_item_by_item_id(player, take_id)
            .map(|i| i.object_id)
        else {
            return;
        };
        let Some(removed) = self.destroy_inventory_item(player, object_id, 1) else {
            return;
        };
        let change = if removed.count == 0 {
            l2_protocol_game::server::InventoryUpdate::REMOVED
        } else {
            l2_protocol_game::server::InventoryUpdate::MODIFIED
        };
        let entry = self.update_entry(&removed, change);
        // `sendInventoryUpdate` joins the pending update, so the removal and the new crystal go
        // out in one `InventoryUpdate`, as Java's `playerIU`.
        self.send_inventory_update(player, entry);
        // `addItem` adds its entry to the same pending update.
        let _ = self.inventory_add_item_by_id(player, give_id, 1);

        // Send a sound event and text message to the player.
        self.send(
            player,
            &SystemMessage::new(if broke {
                SystemMessageId::TheSoulCrystalBrokeBecauseItWasNotAbleToEndureTheSoulEnergy
            } else {
                SystemMessageId::TheSoulCrystalSucceededInAbsorbingASoul
            }),
        );
        // Send system message.
        let templates = self.item_templates();
        let name = item_name_param(templates.as_deref(), give_id);
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveEarnedS1).with(name),
        );
    }
}
