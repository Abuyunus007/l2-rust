//! `data/scripts/village_master`: the village masters' scripts (clan, alliance, class change).
//!
//! One module per script, sorted by name; one `add` line per script in [`register`], in the
//! same order (Java's load order is the folder listing). `class_change` holds what the
//! `*Change1/2` scripts share.

pub mod alliance_master;
pub mod clan_master;
pub(crate) mod class_change;
pub mod dark_elf_change1;
pub mod dark_elf_change2;
pub mod dwarf_blacksmith_change1;
pub mod dwarf_blacksmith_change2;
pub mod dwarf_warehouse_change1;
pub mod dwarf_warehouse_change2;
pub mod elf_human_cleric_change2;
pub mod elf_human_fighter_change1;
pub mod elf_human_fighter_change2;
pub mod elf_human_wizard_change1;
pub mod elf_human_wizard_change2;
pub mod first_class_transfer_talk;
pub mod kamael_change1;
pub mod kamael_change2;
pub mod orc_change1;
pub mod orc_change2;

#[cfg(test)]
pub(crate) mod testing;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(alliance_master::AllianceMaster);
    r.add(clan_master::ClanMaster);
    r.add(dark_elf_change1::DarkElfChange1);
    r.add(dark_elf_change2::DarkElfChange2);
    r.add(dwarf_blacksmith_change1::DwarfBlacksmithChange1);
    r.add(dwarf_blacksmith_change2::DwarfBlacksmithChange2);
    r.add(dwarf_warehouse_change1::DwarfWarehouseChange1);
    r.add(dwarf_warehouse_change2::DwarfWarehouseChange2);
    r.add(elf_human_cleric_change2::ElfHumanClericChange2);
    r.add(elf_human_fighter_change1::ElfHumanFighterChange1);
    r.add(elf_human_fighter_change2::ElfHumanFighterChange2);
    r.add(elf_human_wizard_change1::ElfHumanWizardChange1);
    r.add(elf_human_wizard_change2::ElfHumanWizardChange2);
    r.add(first_class_transfer_talk::FirstClassTransferTalk);
    r.add(kamael_change1::KamaelChange1);
    r.add(kamael_change2::KamaelChange2);
    r.add(orc_change1::OrcChange1);
    r.add(orc_change2::OrcChange2);
}
