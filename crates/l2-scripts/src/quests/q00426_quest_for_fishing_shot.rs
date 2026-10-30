//! Quest 426: Quest for Fishing Shot (the fishing shot masters; Sweet Fluid from the monsters).
// Java: dist/game/data/scripts/quests/Q00426_QuestForFishingShot/Q00426_QuestForFishingShot.java
//
// Pattern: a drop quest with a chance per monster. `random_party_member_state` picks the started
// member who gets the drop, the roll is out of 1000. The quest is repeatable: the NPC's page asks
// to go on or to quit (`exitQuest(true)`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs: the fishing shot masters
const NPC: [i32; 23] = [
    31562, // Klufe
    31563, // Perelin
    31564, // Mishini
    31565, // Ogord
    31566, // Ropfi
    31567, // Bleaker
    31568, // Pamfus
    31569, // Cyano
    31570, // Lanosco
    31571, // Hufs
    31572, // O'Fulle
    31573, // Monakan
    31574, // Willie
    31575, // Litulon
    31576, // Berix
    31577, // Linnaeus
    31578, // Hilgendorf
    31579, // Klaus
    31696, // Platis
    31697, // Eindarkner
    31989, // Batidae
    32007, // Galba
    32348, // Burang
];

// Item
const SWEET_FLUID: i32 = 7586;

/// Java `MOBS`: (monster, chance out of 1000, sweet fluid given). The kill ids are these monsters.
// NOTE(java-quirk): 21263 is put twice in the Java table; the second put (chance 795, 2 fluid) is
// the one `HashMap` keeps, so the first line is left out here.
const MOBS: &[(i32, i32, i32)] = &[
    (20005, 45, 1),
    (20013, 100, 1),
    (20016, 100, 1),
    (20017, 115, 1),
    (20030, 105, 1),
    (20132, 70, 1),
    (20038, 135, 1),
    (20044, 125, 1),
    (20046, 100, 1),
    (20047, 100, 1),
    (20050, 140, 1),
    (20058, 140, 1),
    (20063, 160, 1),
    (20066, 170, 1),
    (20070, 180, 1),
    (20074, 195, 1),
    (20077, 205, 1),
    (20078, 205, 1),
    (20079, 205, 1),
    (20080, 220, 1),
    (20081, 370, 1),
    (20083, 245, 1),
    (20084, 255, 1),
    (20085, 265, 1),
    (20087, 565, 1),
    (20088, 605, 1),
    (20089, 250, 1),
    (20100, 85, 1),
    (20103, 110, 1),
    (20105, 110, 1),
    (20115, 190, 1),
    (20120, 20, 1),
    (20131, 45, 1),
    (20135, 360, 1),
    (20157, 235, 1),
    (20162, 195, 1),
    (20176, 280, 1),
    (20211, 170, 1),
    (20225, 160, 1),
    (20227, 180, 1),
    (20230, 260, 1),
    (20232, 245, 1),
    (20234, 290, 1),
    (20241, 700, 1),
    (20267, 215, 1),
    (20268, 295, 1),
    (20269, 255, 1),
    (20270, 365, 1),
    (20271, 295, 1),
    (20286, 700, 1),
    (20308, 110, 1),
    (20312, 45, 1),
    (20317, 20, 1),
    (20324, 85, 1),
    (20333, 100, 1),
    (20341, 100, 1),
    (20346, 85, 1),
    (20349, 850, 1),
    (20356, 165, 1),
    (20357, 140, 1),
    (20363, 70, 1),
    (20368, 85, 1),
    (20371, 100, 1),
    (20386, 85, 1),
    (20389, 90, 1),
    (20403, 110, 1),
    (20404, 95, 1),
    (20433, 100, 1),
    (20436, 140, 1),
    (20448, 45, 1),
    (20456, 20, 1),
    (20463, 85, 1),
    (20470, 45, 1),
    (20471, 85, 1),
    (20475, 20, 1),
    (20478, 110, 1),
    (20487, 90, 1),
    (20511, 100, 1),
    (20525, 20, 1),
    (20528, 100, 1),
    (20536, 15, 1),
    (20537, 15, 1),
    (20538, 15, 1),
    (20539, 15, 1),
    (20544, 15, 1),
    (20550, 300, 1),
    (20551, 300, 1),
    (20552, 650, 1),
    (20553, 335, 1),
    (20554, 390, 1),
    (20555, 350, 1),
    (20557, 390, 1),
    (20559, 420, 1),
    (20560, 440, 1),
    (20562, 485, 1),
    (20573, 545, 1),
    (20575, 645, 1),
    (20630, 350, 1),
    (20632, 475, 1),
    (20634, 960, 1),
    (20636, 495, 1),
    (20638, 540, 1),
    (20641, 680, 1),
    (20643, 660, 1),
    (20644, 645, 1),
    (20659, 440, 1),
    (20661, 575, 1),
    (20663, 525, 1),
    (20665, 680, 1),
    (20667, 730, 1),
    (20766, 210, 1),
    (20781, 270, 1),
    (20783, 140, 1),
    (20784, 155, 1),
    (20786, 170, 1),
    (20788, 325, 1),
    (20790, 390, 1),
    (20792, 620, 1),
    (20794, 635, 1),
    (20796, 640, 1),
    (20798, 850, 1),
    (20800, 740, 1),
    (20802, 900, 1),
    (20804, 775, 1),
    (20806, 805, 1),
    (20833, 455, 1),
    (20834, 680, 1),
    (20836, 785, 1),
    (20837, 835, 1),
    (20839, 430, 1),
    (20841, 460, 1),
    (20845, 605, 1),
    (20847, 570, 1),
    (20849, 585, 1),
    (20936, 290, 1),
    (20937, 315, 1),
    (20939, 385, 1),
    (20940, 500, 1),
    (20941, 460, 1),
    (20943, 345, 1),
    (20944, 335, 1),
    (21100, 125, 1),
    (21101, 155, 1),
    (21103, 215, 1),
    (21105, 310, 1),
    (21107, 600, 1),
    (21117, 120, 1),
    (21023, 170, 1),
    (21024, 175, 1),
    (21025, 185, 1),
    (21026, 200, 1),
    (21034, 195, 1),
    (21125, 12, 1),
    (21520, 880, 1),
    (21526, 970, 1),
    (21536, 985, 1),
    (21602, 555, 1),
    (21603, 750, 1),
    (21605, 620, 1),
    (21606, 875, 1),
    (21611, 590, 1),
    (21612, 835, 1),
    (21617, 615, 1),
    (21618, 875, 1),
    (21635, 775, 1),
    (21638, 165, 1),
    (21639, 185, 1),
    (21641, 195, 1),
    (21644, 170, 1),
    (22231, 10, 1),
    (22233, 20, 1),
    (22234, 30, 1),
    (22235, 35, 1),
    (22237, 55, 1),
    (22238, 70, 1),
    (22241, 80, 1),
    (22244, 90, 1),
    (22247, 90, 1),
    (22250, 90, 1),
    (22252, 95, 1),
    (20579, 420, 2),
    (20639, 280, 2),
    (20646, 145, 2),
    (20648, 120, 2),
    (20650, 460, 2),
    (20651, 260, 2),
    (20652, 335, 2),
    (20657, 630, 2),
    (20658, 570, 2),
    (20808, 50, 2),
    (20809, 865, 2),
    (20832, 700, 2),
    (20979, 980, 2),
    (20991, 665, 2),
    (20994, 590, 2),
    (21261, 170, 2),
    (21263, 795, 2),
    (21508, 100, 2),
    (21510, 280, 2),
    (21511, 995, 2),
    (21512, 995, 2),
    (21514, 185, 2),
    (21516, 495, 2),
    (21517, 495, 2),
    (21518, 255, 2),
    (21636, 950, 2),
    (20655, 110, 3),
    (20656, 150, 3),
    (20772, 105, 3),
    (20810, 50, 3),
    (20812, 490, 3),
    (20814, 775, 3),
    (20816, 875, 3),
    (20819, 280, 3),
    (20955, 670, 3),
    (20978, 555, 3),
    (21058, 355, 3),
    (21060, 45, 3),
    (21075, 110, 3),
    (21078, 610, 3),
    (21081, 955, 3),
    (21264, 920, 3),
    (20815, 205, 4),
    (20822, 100, 4),
    (20824, 665, 4),
    (20825, 620, 4),
    (20983, 205, 4),
    (21314, 145, 4),
    (21316, 235, 4),
    (21318, 280, 4),
    (21320, 355, 4),
    (21322, 430, 4),
    (21376, 280, 4),
    (21378, 375, 4),
    (21380, 375, 4),
    (21387, 640, 4),
    (21393, 935, 4),
    (21395, 855, 4),
    (21652, 375, 4),
    (21655, 640, 4),
    (21657, 935, 4),
    (20828, 935, 5),
    (21061, 530, 5),
    (21069, 825, 5),
    (21382, 125, 5),
    (21384, 400, 5),
    (21390, 750, 5),
    (21654, 400, 5),
    (21656, 750, 5),
    (22634, 280, 4),
    (22636, 375, 4),
    (22638, 375, 4),
    (22640, 125, 5),
    (22644, 640, 4),
    (22646, 750, 5),
    (22649, 935, 4),
];

/// Java `MOBS_SPECIAL`: (monster, chance out of 1000 for one more fluid, fluid given).
// NOTE(java-quirk): none of these monsters is in `addKillId` (they are not in `MOBS`), so the
// engine never calls `on_kill` for them. The branch is kept as Java has it.
const MOBS_SPECIAL: [(i32, i32, i32); 5] = [
    (20829, 115, 6),  // Platinum Tribe Overlord
    (20859, 890, 8),  // Guardian Angel
    (21066, 5, 5),    // Platinum Guardian Shaman
    (21068, 565, 11), // Guardian Archangel
    (21071, 400, 12), // Seal Archangel
];

/// Java `Q00426_QuestForFishingShot`.
pub struct Q00426QuestForFishingShot;

impl Script for Q00426QuestForFishingShot {
    fn name(&self) -> &'static str {
        "Q00426_QuestForFishingShot"
    }

    fn path(&self) -> &'static str {
        "quests/Q00426_QuestForFishingShot"
    }

    fn id(&self) -> i32 {
        426
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let kill_ids: Vec<i32> = MOBS.iter().map(|(mob, _, _)| *mob).collect();
        s.add_start_npc(&NPC);
        s.add_talk_id(&NPC);
        s.add_kill_id(&kill_ids);
        s.register_quest_items(&[SWEET_FLUID]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        match event {
            "QUEST_ACEPT" => {
                qs.start_quest(ctx);
                Some("03.htm".to_owned())
            }
            "1" => Some("06.html".to_owned()),
            "2" => Some("07.html".to_owned()),
            "3" => {
                qs.exit_quest(ctx, true, false);
                Some("08.html".to_owned())
            }
            _ => Some(event.to_owned()),
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, -1, 2, npc)`: a started member gets the drop.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 2, Some(npc)) else {
            return;
        };
        let member = qs.player();
        if let Some(&(_, chance, reward)) = MOBS_SPECIAL.iter().find(|(mob, _, _)| *mob == npc.id) {
            let amount = if ctx.get_random(1000) <= chance {
                reward + 1
            } else {
                reward
            };
            ctx.reward_items(member, SWEET_FLUID, i64::from(amount));
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        } else if let Some(&(_, chance, reward)) = MOBS.iter().find(|(mob, _, _)| *mob == npc.id)
            && ctx.get_random(1000) <= chance
        {
            ctx.reward_items(member, SWEET_FLUID, i64::from(reward));
            ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let no_quest = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;
        match qs.state(ctx) {
            CREATED => Some("01.htm".to_owned()),
            STARTED => {
                if ctx.has_quest_items(player, SWEET_FLUID) {
                    Some("05.html".to_owned())
                } else {
                    Some("04.html".to_owned())
                }
            }
            _ => Some(no_quest),
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00426_QuestForFishingShot";

    const NPCS: &str = r#"
        <npc id="31562" type="Folk" level="70" name="Klufe"/>
        <npc id="20120" type="Monster" level="5" name="Wolf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7586" type="EtcItem" name="Sweet Fluid"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00426QuestForFishingShot);
        let pages = Pages::of(&Q00426QuestForFishingShot)
            .page("01.htm", &["QUEST_ACEPT"])
            .page("03.htm", &["1"])
            .page("04.html", &["1", "2", "3"])
            .page("05.html", &["1", "2", "3"])
            .page("06.html", &["09.html"])
            .plain(&["07.html", "08.html", "09.html"])
            .npcs(&[31562]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let klufe = t.spawn(31562, 20);
        (t, klufe)
    }

    #[test]
    fn the_start_writes_the_cond_and_the_state() {
        let (mut t, klufe) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, klufe).contains("01.htm"));
        assert!(t.event(1, NAME, "QUEST_ACEPT").contains("03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, klufe).contains("04.html"));
    }

    #[test]
    fn the_fluid_drops_until_the_talk_says_go_on() {
        let (mut t, klufe) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, klufe).contains("01.htm"));
        t.event(1, NAME, "QUEST_ACEPT");
        t.take_writes();

        // A wolf drops the fluid 2 times out of 100: hunt until the first one.
        for _ in 0..1_000 {
            if t.count(1, SWEET_FLUID) > 0 {
                break;
            }
            let wolf = t.spawn(20120, 60);
            t.kill(1, wolf);
        }
        assert!(t.count(1, SWEET_FLUID) >= 1);
        // The drop is no quest row.
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, klufe).contains("05.html"));
    }

    #[test]
    fn quitting_ends_the_quest_and_takes_the_fluid_back() {
        let (mut t, klufe) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, klufe).contains("01.htm"));
        t.event(1, NAME, "QUEST_ACEPT");
        assert!(t.quest_page(1, klufe).contains("04.html"));
        t.take_writes();

        // "Quit": `exitQuest(true)` (repeatable, no sound) and the quest page.
        assert!(t.event(1, NAME, "3").contains("08.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn the_go_on_pages_show_their_own_pages() {
        let (mut t, klufe) = setup();
        t.enter(1, 20, vec![]);
        assert!(t.quest_page(1, klufe).contains("01.htm"));
        t.event(1, NAME, "QUEST_ACEPT");
        assert!(t.quest_page(1, klufe).contains("04.html"));
        // "1" is the 06 page; a link of that page is the default: the event is the page.
        assert!(t.event(1, NAME, "1").contains("06.html"));
        assert!(t.event(1, NAME, "09.html").contains("09.html"));
        assert!(
            t.take_writes()
                .iter()
                .all(|w| !matches!(w, QuestWrite::DeleteQuest { .. }))
        );
    }
}
