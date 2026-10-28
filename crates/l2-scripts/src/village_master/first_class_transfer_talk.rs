//! The headmasters of the newbie villages: none of them changes a class, they only talk about it.
// Java: dist/game/data/scripts/village_master/FirstClassTransferTalk/FirstClassTransferTalk.java

use l2_util::ObjectId;
use l2_world::subclass::{ClassId, Race, VillageMasterKind};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{class_level, class_of};

/// Java `MASTERS`: the race each headmaster talks to.
const MASTERS: [(i32, Race); 15] = [
    (30026, Race::Human),   // Blitz, TI Fighter Guild Head Master
    (30031, Race::Human),   // Biotin, TI Einhasad Temple High Priest
    (30154, Race::Elf),     // Asterios, Elven Village Tetrarch
    (30358, Race::DarkElf), // Thifiell, Dark Elf Village Tetrarch
    (30565, Race::Orc),     // Kakai, Orc Village Flame Lord
    (30520, Race::Dwarf),   // Reed, Dwarven Village Warehouse Chief
    (30525, Race::Dwarf),   // Bronk, Dwarven Village Head Blacksmith
    // Kamael Village NPCs
    (32171, Race::Dwarf),   // Hoffa, Warehouse Chief
    (32158, Race::Dwarf),   // Fisler, Dwarf Guild Warehouse Chief
    (32157, Race::Dwarf),   // Moka, Dwarf Guild Head Blacksmith
    (32160, Race::DarkElf), // Devon, Dark Elf Guild Grand Magister
    (32147, Race::Elf),     // Rivian, Elf Guild Grand Master
    (32150, Race::Orc),     // Took, Orc Guild High Prefect
    (32153, Race::Human),   // Prana, Human Guild High Priest
    (32154, Race::Human),   // Aldenia, Human Guild Grand Master
];

/// Java `village_master.FirstClassTransferTalk.FirstClassTransferTalk`.
pub struct FirstClassTransferTalk;

impl Script for FirstClassTransferTalk {
    fn name(&self) -> &'static str {
        "FirstClassTransferTalk"
    }

    fn path(&self) -> &'static str {
        "village_master/FirstClassTransferTalk"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        // NOTE: Java registers `MASTERS.keySet()` (hash order); the order only matters for the
        // listeners of one NPC, and each NPC is listed once.
        let ids: Vec<i32> = MASTERS.iter().map(|(id, _)| *id).collect();
        s.add_start_npc(&ids);
        s.add_talk_id(&ids);
    }

    fn on_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = format!("{}_", npc.id);
        let race = MASTERS
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map(|(_, race)| *race);
        if race.is_none() || race != ctx.world().player_race(player) {
            htmltext.push_str("no.html");
            return Some(htmltext);
        }
        let level = class_level(ctx, player);
        let mage = u16::try_from(class_of(ctx, player)).is_ok_and(|c| ClassId(c).is_mage());
        let kind = ctx.world().npc_village_master_kind(npc.object_id);
        let page = match race {
            Some(Race::Human) => {
                if level == 0 {
                    if mage {
                        if kind == Some(VillageMasterKind::Priest) {
                            "mystic.html"
                        } else {
                            "no.html"
                        }
                    } else if kind == Some(VillageMasterKind::Fighter) {
                        "fighter.html"
                    } else {
                        "no.html"
                    }
                } else if level == 1 {
                    "transfer_1.html"
                } else {
                    "transfer_2.html"
                }
            }
            Some(Race::Elf | Race::DarkElf | Race::Orc) => {
                if level == 0 {
                    if mage { "mystic.html" } else { "fighter.html" }
                } else if level == 1 {
                    "transfer_1.html"
                } else {
                    "transfer_2.html"
                }
            }
            Some(Race::Dwarf) => {
                if level == 0 {
                    "fighter.html"
                } else if level == 1 {
                    "transfer_1.html"
                } else {
                    "transfer_2.html"
                }
            }
            _ => "no.html",
        };
        htmltext.push_str(page);
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::Vm;

    const DIR: &str = "data/scripts/village_master/FirstClassTransferTalk";

    fn setup() -> Vm {
        let mut vm = Vm::new(
            FirstClassTransferTalk,
            DIR,
            "FirstClassTransferTalk",
            &[
                (30026, "VillageMasterFighter"),
                (30031, "VillageMasterPriest"),
                (30520, "VillageMasterDwarf"),
            ],
        );
        vm.page("30026_fighter.html", &["30026_warrior_01.html"]);
        for page in [
            "30026_no.html",
            "30026_warrior_01.html",
            "30026_transfer_1.html",
            "30031_mystic.html",
            "30031_no.html",
            "30520_fighter.html",
            "30520_transfer_2.html",
            "30520_no.html",
        ] {
            vm.page(page, &[]);
        }
        vm.build()
    }

    #[test]
    fn the_headmasters_talk_to_their_race_and_type() {
        let mut vm = setup();
        // A human fighter at the fighter guild, then the page it links to.
        vm.enter(1, 1, 0, vec![]);
        assert!(vm.talk(1, 30026).contains("30026_fighter.html"));
        assert!(
            vm.click(1, "30026_warrior_01.html")
                .contains("30026_warrior_01.html")
        );
        // ... and at the temple: not a priest's student.
        assert!(vm.talk(1, 30031).contains("30031_no.html"));
        // A human mystic: the temple, not the fighter guild.
        vm.enter(2, 1, 10, vec![]);
        assert!(vm.talk(2, 30031).contains("30031_mystic.html"));
        assert!(vm.talk(2, 30026).contains("30026_no.html"));
        // A warrior has had the 1st transfer.
        vm.enter(3, 20, 1, vec![]);
        assert!(vm.talk(3, 30026).contains("30026_transfer_1.html"));
        // Dwarves at Reed; an elf there is sent away.
        vm.enter(4, 1, 53, vec![]);
        assert!(vm.talk(4, 30520).contains("30520_fighter.html"));
        vm.enter(5, 40, 57, vec![]);
        assert!(vm.talk(5, 30520).contains("30520_transfer_2.html"));
        vm.enter(6, 1, 18, vec![]);
        assert!(vm.talk(6, 30520).contains("30520_no.html"));
    }
}
