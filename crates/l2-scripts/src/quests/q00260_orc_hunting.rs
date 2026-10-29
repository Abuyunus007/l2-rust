//! Quest 260: Orc Hunting (Rayen in the Elven Village hunts the Kaboo orcs for the amulets and
//! necklaces; rookies get shots, and the newbie guide hears about it).
// Java: dist/game/data/scripts/quests/Q00260_OrcHunting/Q00260_OrcHunting.java
//
// Pattern: a repeatable kill count with a random drop and a race restriction (`docs/design/
// scripts.md`, "Quest patterns"), a one-time level-25 flag for the shots, and the newbie guide's
// memo (Java `NewbieGuide`, a script of its own: read through `ctx.quest_state_of`).

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RAYEN: i32 = 30221;

// Items
const ORC_AMULET: i32 = 1114;
const ORC_NECKLACE: i32 = 1115;
const SPIRITSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5790, 3000);
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 6000);

// Monsters and the item each drops: Kaboo Orcs (amulet), Kaboo Orc Fighters (necklace)
const MONSTERS: [(i32, i32); 6] = [
    (20468, ORC_AMULET),
    (20469, ORC_AMULET),
    (20470, ORC_AMULET),
    (20471, ORC_NECKLACE),
    (20472, ORC_NECKLACE),
    (20473, ORC_NECKLACE),
];

// Misc
const MIN_LEVEL: i32 = 6;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00260_OrcHunting`.
pub struct Q00260OrcHunting;

impl Script for Q00260OrcHunting {
    fn name(&self) -> &'static str {
        "Q00260_OrcHunting"
    }

    fn path(&self) -> &'static str {
        "quests/Q00260_OrcHunting"
    }

    fn id(&self) -> i32 {
        260
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAYEN]);
        s.add_talk_id(&[RAYEN]);
        for (mob, _) in MONSTERS {
            s.add_kill_id(&[mob]);
        }
        s.register_quest_items(&[ORC_AMULET, ORC_NECKLACE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java returns null (no page) when there is no quest state.
        let qs = ctx.get_quest_state(player, false)?;
        match event {
            "30221-04.htm" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "30221-07.html" => {
                qs.exit_quest(ctx, true, true);
                Some(event.to_owned())
            }
            "30221-08.html" => Some(event.to_owned()),
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_some() && ctx.get_random(10) > 4 {
            if let Some((_, item)) = MONSTERS.iter().find(|(mob, _)| *mob == npc.id) {
                ctx.give_items(killer, *item, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30221-03.htm"
                    } else {
                        "30221-02.html"
                    }
                } else {
                    "30221-01.html"
                }
                .to_owned();
            }
            STARTED => {
                let registered = [ORC_AMULET, ORC_NECKLACE];
                if ctx.has_at_least_one_quest_item(player, &registered) {
                    let amulets = ctx.get_quest_items_count(player, ORC_AMULET);
                    let necklaces = ctx.get_quest_items_count(player, ORC_NECKLACE);
                    let bonus = if amulets + necklaces >= 10 { 1000 } else { 0 };
                    ctx.give_adena(player, amulets * 12 + necklaces * 30 + bonus, true);
                    ctx.take_items_many(player, -1, &registered);

                    if ctx.player_level(player) < 25 && ctx.get_one_time_quest_flag(player, 57) == 0 {
                        let (item, count) = if ctx.is_mage_class(player) {
                            ctx.play_sound_named(player, "tutorial_voice_027");
                            SPIRITSHOTS_NO_GRADE_FOR_ROOKIES
                        } else {
                            ctx.play_sound_named(player, "tutorial_voice_026");
                            SOULSHOTS_NO_GRADE_FOR_ROOKIES
                        };
                        ctx.give_items(player, item, count);
                        ctx.set_one_time_quest_flag(player, 57, 1);
                    }

                    // Newbie Guide (only when `ai/others/NewbieGuide` is installed, as in Java).
                    if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                        if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, 1000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::AcquisitionOfSoulshotForBeginnersCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 10000) / 1000 != 1 {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 1000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::AcquisitionOfSoulshotForBeginnersCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        }
                    }

                    htmltext = "30221-06.html".to_owned();
                } else {
                    htmltext = "30221-05.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::player_class;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00260_OrcHunting";

    const NPCS: &str = r#"
        <npc id="30221" type="Folk" level="70" name="Rayen"/>
        <npc id="20468" type="Monster" level="6" name="Kaboo Orc"/>
        <npc id="20471" type="Monster" level="8" name="Kaboo Orc Fighter"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1114" type="EtcItem" name="Orc Amulet"><set name="is_questitem" val="true"/></item>
        <item id="1115" type="EtcItem" name="Orc Necklace"><set name="is_questitem" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true"/></item>
        <item id="5790" type="EtcItem" name="Spiritshot: No Grade"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00057_NewbieShots`, the quest the one-time flag 57 reads: a stand-in with its id.
    struct NewbieShots;

    impl Script for NewbieShots {
        fn name(&self) -> &'static str {
            "Q00057_NewbieShots"
        }
        fn path(&self) -> &'static str {
            "quests/Dummy/Q00057_NewbieShots"
        }
        fn id(&self) -> i32 {
            57
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieShots);
        registry.add(Q00260OrcHunting);
        let pages = Pages::of(&Q00260OrcHunting)
            .page("30221-03.htm", &["30221-04.htm"])
            .plain(&["30221-01.html", "30221-02.html", "30221-04.htm"])
            .page("30221-06.html", &["30221-08.html", "30221-07.html"])
            .page("30221-05.html", &["30221-08.html", "30221-07.html"])
            .plain(&["30221-07.html", "30221-08.html"])
            .npcs(&[RAYEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rayen = t.spawn(RAYEN, 20);
        (t, rayen)
    }

    /// Kills orcs of `mob` until the player has `count` of the two items together.
    fn kill_until(t: &mut ScriptTest, killer: i32, mob: i32, count: i64) {
        for _ in 0..500 {
            let total = t.count(killer, ORC_AMULET) + t.count(killer, ORC_NECKLACE);
            if total >= count {
                return;
            }
            let orc = t.spawn(mob, 20);
            t.kill(killer, orc);
        }
        panic!("no drops in 500 kills");
    }

    #[test]
    fn only_elves_of_level_6_are_offered_the_quest() {
        let (mut t, rayen) = setup();
        t.enter_as(1, 6, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, rayen).contains("30221-03.htm"));
        t.enter_as(2, 5, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(2, rayen).contains("30221-02.html"));
        t.enter_as(3, 10, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(3, rayen).contains("30221-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hunt_pays_the_drops_and_the_soulshots() {
        let (mut t, rayen) = setup();
        t.enter_as(1, 6, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, rayen).contains("30221-03.htm"));
        assert!(t.event(1, NAME, "30221-04.htm").contains("30221-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nothing yet: the page says so.
        assert!(t.quest_page(1, rayen).contains("30221-05.html"));

        kill_until(&mut t, 1, 20468, 10);
        let amulets = t.count(1, ORC_AMULET);
        let necklaces = t.count(1, ORC_NECKLACE);
        assert!(amulets + necklaces >= 10);
        let adena = amulets * 12 + necklaces * 30 + 1000;

        // Rayen pays the drops, the rookie soulshots once (not a mage: soulshots).
        assert!(t.quest_page(1, rayen).contains("30221-06.html"));
        assert_eq!(t.count(1, 57), adena);
        assert_eq!((t.count(1, ORC_AMULET), t.count(1, ORC_NECKLACE)), (0, 0));
        assert_eq!(t.count(1, 5789), 6000);

        // The one-time flag (57): a second payment brings no more shots.
        kill_until(&mut t, 1, 20468, 10);
        assert!(t.quest_page(1, rayen).contains("30221-06.html"));
        assert_eq!(t.count(1, 5789), 6000);

        // Quit: `exitQuest(true, true)` deletes the quest.
        assert!(t.event(1, NAME, "30221-07.html").contains("30221-07.html"));
        // The flag of the first payment was written as the state of Q00057 (`setState` saves it).
        assert_eq!(
            t.take_writes(),
            [
                rows::insert("Q00057_NewbieShots", "<state>", "Completed"),
                rows::delete(NAME, true)
            ]
        );
    }
}
