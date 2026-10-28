//! Varka Silenos and Ketra Orcs: killing one of the other side lowers the alliance.
// Java: dist/game/data/scripts/ai/others/VarkaKetra.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Monsters
const KETRA: [i32; 25] = [
    21324, // Ketra Orc Footman
    21325, // Ketra's War Hound
    21327, // Ketra Orc Raider
    21328, // Ketra Orc Scout
    21329, // Ketra Orc Shaman
    21331, // Ketra Orc Warrior
    21332, // Ketra Orc Lieutenant
    21334, // Ketra Orc Medium
    21335, // Ketra Orc Elite Soldier
    21336, // Ketra Orc White Captain
    21338, // Ketra Orc Seer
    21339, // Ketra Orc General
    21340, // Ketra Orc Battalion Commander
    21342, // Ketra Orc Grand Seer
    21343, // Ketra Commander
    21344, // Ketra Elite Guard
    21345, // Ketra's Head Shaman
    21346, // Ketra's Head Guard
    21347, // Ketra Prophet
    21348, // Prophet's Guard
    21349, // Prophet's Aide
    25299, // Ketra's Hero Hekaton (Raid Boss)
    25302, // Ketra's Commander Tayr (Raid Boss)
    25305, // Ketra's Chief Brakki (Raid Boss)
    25306, // Soul of Fire Nastron (Raid Boss)
];
const VARKA: [i32; 25] = [
    21350, // Varka Silenos Recruit
    21351, // Varka Silenos Footman
    21353, // Varka Silenos Scout
    21354, // Varka Silenos Hunter
    21355, // Varka Silenos Shaman
    21357, // Varka Silenos Priest
    21358, // Varka Silenos Warrior
    21360, // Varka Silenos Medium
    21361, // Varka Silenos Magus
    21362, // Varka Silenos Officer
    21364, // Varka Silenos Seer
    21365, // Varka Silenos Great Magus
    21366, // Varka Silenos General
    21368, // Varka Silenos Great Seer
    21369, // Varka's Commander
    21370, // Varka's Elite Guard
    21371, // Varka's Head Magus
    21372, // Varka's Head Guard
    21373, // Varka's Prophet
    21374, // Prophet's Guard
    21375, // Disciple of Prophet
    25309, // Varka's Hero Shadith (Raid Boss)
    25312, // Varka's Commander Mos (Raid Boss)
    25315, // Varka's Chief Horus (Raid Boss)
    25316, // Soul of Water Ashutar (Raid Boss)
];

// Items
const KETRA_MARKS: [i32; 5] = [
    7211, // Mark of Ketra's Alliance - Level 1
    7212, // Mark of Ketra's Alliance - Level 2
    7213, // Mark of Ketra's Alliance - Level 3
    7214, // Mark of Ketra's Alliance - Level 4
    7215, // Mark of Ketra's Alliance - Level 5
];
const VARKA_MARKS: [i32; 5] = [
    7221, // Mark of Varka's Alliance - Level 1
    7222, // Mark of Varka's Alliance - Level 2
    7223, // Mark of Varka's Alliance - Level 3
    7224, // Mark of Varka's Alliance - Level 4
    7225, // Mark of Varka's Alliance - Level 5
];

// Quests
const KETRA_QUESTS: [&str; 6] = [
    "Q00605_AllianceWithKetraOrcs",
    "Q00606_BattleAgainstVarkaSilenos",
    "Q00607_ProveYourCourageKetra",
    "Q00608_SlayTheEnemyCommanderKetra",
    "Q00609_MagicalPowerOfWaterPart1",
    "Q00610_MagicalPowerOfWaterPart2",
];
const VARKA_QUESTS: [&str; 6] = [
    "Q00611_AllianceWithVarkaSilenos",
    "Q00612_BattleAgainstKetraOrcs",
    "Q00613_ProveYourCourageVarka",
    "Q00614_SlayTheEnemyCommanderVarka",
    "Q00615_MagicalPowerOfFirePart1",
    "Q00616_MagicalPowerOfFirePart2",
];

/// Java `decreaseAlliance(player, marks)`: the mark of the player's level goes, the one below
/// takes its place.
fn decrease_alliance(ctx: &mut ScriptCtx<'_>, player: ObjectId, marks: &[i32]) {
    for (i, &mark) in marks.iter().enumerate() {
        if ctx.has_quest_items(player, mark) {
            ctx.take_items(player, mark, -1);
            if i > 0 {
                ctx.give_items(player, marks[i - 1], 1);
            }
            return;
        }
    }
}

/// Java `exitQuests(player, quests)`.
fn exit_quests(ctx: &mut ScriptCtx<'_>, player: ObjectId, quests: &[&str]) {
    for quest in quests {
        if ctx
            .player_quest_state(player, quest)
            .is_some_and(|qs| qs.is_started())
        {
            if let Some(qs) = ctx.quest_state_of(quest, player, false) {
                qs.exit_quest(ctx, true, false);
            }
        }
    }
}

/// Java `ai.others.VarkaKetra`.
pub struct VarkaKetra;

impl Script for VarkaKetra {
    fn name(&self) -> &'static str {
        "VarkaKetra"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&KETRA);
        s.add_kill_id(&VARKA);
        s.add_npc_hate_id(&KETRA);
        s.add_npc_hate_id(&VARKA);
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        if ctx.check_if_in_range(1500, player, npc.object_id, false) {
            if KETRA.contains(&npc.id) && ctx.has_at_least_one_quest_item(player, &KETRA_MARKS) {
                decrease_alliance(ctx, player, &KETRA_MARKS);
                exit_quests(ctx, player, &KETRA_QUESTS);
            } else if VARKA.contains(&npc.id) && ctx.has_at_least_one_quest_item(player, &VARKA_MARKS) {
                decrease_alliance(ctx, player, &VARKA_MARKS);
                exit_quests(ctx, player, &VARKA_QUESTS);
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);
    }

    fn on_npc_hate(
        &self,
        ctx: &mut ScriptCtx<'_>,
        mob: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) -> bool {
        if KETRA.contains(&mob.id) {
            return !ctx.has_at_least_one_quest_item(player, &KETRA_MARKS);
        }

        !ctx.has_at_least_one_quest_item(player, &VARKA_MARKS)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item};

    const NPCS: &str = r#"<npc id="21324" type="Monster" level="60" name="Ketra Orc Footman"><stats><vitals hp="10" mp="10"/></stats></npc>
        <npc id="21350" type="Monster" level="60" name="Varka Silenos Recruit"><stats><vitals hp="10" mp="10"/></stats></npc>"#;
    const ITEMS: &str = r#"<item id="7211" type="EtcItem" name="Ketra 1"/>
        <item id="7212" type="EtcItem" name="Ketra 2"/>
        <item id="7221" type="EtcItem" name="Varka 1"/>"#;

    fn setup(items: Vec<l2_world::ItemData>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(VarkaKetra);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.enter(1, 60, items);
        t
    }

    #[test]
    fn killing_a_ketra_lowers_the_ketra_alliance_by_one_level() {
        let mut t = setup(vec![item(5000, 7212, 1)]);
        let mob = t.spawn(21324, 20);
        t.kill(1, mob);
        assert_eq!(t.count(1, 7212), 0);
        assert_eq!(t.count(1, 7211), 1);
    }

    #[test]
    fn the_lowest_mark_is_just_taken_and_the_other_side_is_left_alone() {
        let mut t = setup(vec![item(5000, 7211, 1), item(5001, 7221, 1)]);
        let mob = t.spawn(21324, 20);
        t.kill(1, mob);
        assert_eq!(t.count(1, 7211), 0);
        assert_eq!(t.count(1, 7221), 1);
    }

    #[test]
    fn the_monsters_hate_only_those_without_a_mark_of_their_side() {
        let mut t = setup(vec![item(5000, 7221, 1)]);
        let (ketra, varka) = (
            ScriptNpc {
                object_id: ObjectId(1),
                id: 21324,
            },
            ScriptNpc {
                object_id: ObjectId(2),
                id: 21350,
            },
        );
        let mut answers = Vec::new();
        t.world.run_as_script("VarkaKetra", |ctx| {
            answers.push(VarkaKetra.on_npc_hate(ctx, ketra, ObjectId(1), false));
            answers.push(VarkaKetra.on_npc_hate(ctx, varka, ObjectId(1), false));
        });
        assert_eq!(answers, [true, false]);
    }
}
