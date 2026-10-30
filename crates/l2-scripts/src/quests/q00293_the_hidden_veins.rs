//! Quest 293: The Hidden Veins (Filaur in the Dwarven Village buys the chrysolite ore and the hidden ore maps).
// Java: dist/game/data/scripts/quests/Q00293_TheHiddenVeins/Q00293_TheHiddenVeins.java
//
// Pattern: a collect quest that ends with a trade (`docs/design/scripts.md`, "Quest patterns"): the
// lizardman-like kasha monsters drop chrysolite ore (half of the rolls) or a torn map fragment (rare);
// Chichirin trades four fragments for a hidden ore map; Filaur pays for the ore and the maps and
// the quest goes on until the player leaves it. Only dwarves may take it.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const FILAUR: i32 = 30535;
const CHICHIRIN: i32 = 30539;

// Monsters
const MONSTERS: [i32; 3] = [20446, 20447, 20448];

// Items
const CHRYSOLITE_ORE: i32 = 1488;
const TORN_MAP_FRAGMENT: i32 = 1489;
const HIDDEN_ORE_MAP: i32 = 1490;
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 6000);

// Misc
const MIN_LEVEL: i32 = 6;
const REQUIRED_TORN_MAP_FRAGMENT: i64 = 4;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00293_TheHiddenVeins`.
pub struct Q00293TheHiddenVeins;

impl Script for Q00293TheHiddenVeins {
    fn name(&self) -> &'static str {
        "Q00293_TheHiddenVeins"
    }

    fn path(&self) -> &'static str {
        "quests/Q00293_TheHiddenVeins"
    }

    fn id(&self) -> i32 {
        293
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FILAUR]);
        s.add_talk_id(&[FILAUR, CHICHIRIN]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[CHRYSOLITE_ORE, TORN_MAP_FRAGMENT, HIDDEN_ORE_MAP]);
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
        let mut htmltext = None;
        match event {
            "30535-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30535-07.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30535-08.html" => htmltext = Some(event.to_owned()),
            "30539-03.html" => {
                if ctx.get_quest_items_count(player, TORN_MAP_FRAGMENT) >= REQUIRED_TORN_MAP_FRAGMENT {
                    ctx.give_items(player, HIDDEN_ORE_MAP, 1);
                    ctx.take_items(player, TORN_MAP_FRAGMENT, REQUIRED_TORN_MAP_FRAGMENT);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30539-02.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_some() {
            let chance = ctx.get_random(100);
            if chance > 50 {
                ctx.give_items(killer, CHRYSOLITE_ORE, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            } else if chance < 5 {
                ctx.give_items(killer, TORN_MAP_FRAGMENT, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            FILAUR => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::DWARF {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "30535-03.htm"
                        } else {
                            "30535-02.htm"
                        }
                    } else {
                        "30535-01.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    let items = [CHRYSOLITE_ORE, HIDDEN_ORE_MAP];
                    if ctx.has_at_least_one_quest_item(player, &items) {
                        let ores = ctx.get_quest_items_count(player, CHRYSOLITE_ORE);
                        let maps = ctx.get_quest_items_count(player, HIDDEN_ORE_MAP);
                        let bonus = if ores + maps >= 10 { 2000 } else { 0 };
                        ctx.give_adena(player, ores * 5 + maps * 500 + bonus, true);
                        ctx.take_items_many(player, -1, &items);

                        if ctx.player_level(player) < 25 && ctx.get_one_time_quest_flag(player, 57) == 0 {
                            if !ctx.is_mage_class(player) {
                                ctx.give_items(
                                    player,
                                    SOULSHOTS_NO_GRADE_FOR_ROOKIES.0,
                                    SOULSHOTS_NO_GRADE_FOR_ROOKIES.1,
                                );
                                ctx.play_sound_named(player, "tutorial_voice_026");
                            }
                            ctx.set_one_time_quest_flag(player, 57, 1);
                        }

                        // Newbie Guide: only when `ai/others/NewbieGuide` is installed.
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
                            } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 10_000) / 1000 != 1 {
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

                        htmltext = if ores > 0 {
                            if maps > 0 {
                                "30535-10.html"
                            } else {
                                "30535-06.html"
                            }
                        } else {
                            "30535-09.html"
                        }
                        .to_owned();
                    } else {
                        htmltext = "30535-05.html".to_owned();
                    }
                }
                _ => {}
            },
            CHICHIRIN => htmltext = "30539-01.html".to_owned(),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00293_TheHiddenVeins";

    const NPCS: &str = r#"
        <npc id="30535" type="Folk" level="70" name="Filaur"/>
        <npc id="30539" type="Folk" level="70" name="Chichirin"/>
        <npc id="20446" type="Monster" level="10" name="Kasha Dog"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1488" type="EtcItem" name="Chrysolite Ore"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1489" type="EtcItem" name="Torn Map Fragment"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1490" type="EtcItem" name="Hidden Ore Map"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5789" type="EtcItem" name="Soulshot: No Grade"><set name="is_stackable" val="true"/></item>"#;

    /// Java `ai.others.NewbieGuide` as far as this quest needs it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            "NewbieGuide"
        }
        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Q00293TheHiddenVeins);
        let pages = Pages::of(&Q00293TheHiddenVeins)
            .page("30535-03.htm", &["30535-04.htm"])
            .plain(&[
                "30535-01.htm",
                "30535-02.htm",
                "30535-04.htm",
                "30535-07.html",
                "30535-08.html",
            ])
            .page("30535-05.html", &["30535-07.html", "30535-08.html"])
            .page("30535-10.html", &["30535-07.html", "30535-08.html"])
            .plain(&["30535-06.html", "30535-09.html"])
            .page("30539-01.html", &["30539-03.html"])
            .plain(&["30539-02.html", "30539-03.html"])
            .npcs(&[FILAUR, CHICHIRIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let filaur = t.spawn(FILAUR, 20);
        let chichirin = t.spawn(CHICHIRIN, 40);
        (t, filaur, chichirin)
    }

    #[test]
    fn only_dwarves_of_level_six_can_take_it() {
        let (mut t, filaur, _chichirin) = setup();
        t.enter_as(1, 6, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, filaur).contains("30535-01.htm"));
        t.enter_as(2, 5, race::DWARF, player_class::DWARVEN_FIGHTER, vec![]);
        assert!(t.quest_page(2, filaur).contains("30535-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn four_torn_fragments_make_a_map_and_the_ore_is_paid_with_the_shots() {
        let (mut t, filaur, chichirin) = setup();
        t.enter_as(1, 6, race::DWARF, player_class::DWARVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, filaur).contains("30535-03.htm"));
        assert!(t.event(1, NAME, "30535-04.htm").contains("30535-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, filaur).contains("30535-05.html"));

        // Ore on most rolls, a torn fragment on the rare low ones; 4 fragments are needed.
        for _ in 0..2000 {
            if t.count(1, TORN_MAP_FRAGMENT) >= REQUIRED_TORN_MAP_FRAGMENT && t.count(1, CHRYSOLITE_ORE) >= 1
            {
                break;
            }
            let dog = t.spawn(20446, 60);
            t.kill(1, dog);
        }
        assert_eq!(t.count(1, TORN_MAP_FRAGMENT), REQUIRED_TORN_MAP_FRAGMENT);
        assert!(t.take_writes().is_empty());

        // Chichirin trades the fragments for a hidden ore map.
        assert!(t.quest_page(1, chichirin).contains("30539-01.html"));
        assert!(t.event(1, NAME, "30539-03.html").contains("30539-03.html"));
        assert_eq!(t.count(1, HIDDEN_ORE_MAP), 1);
        assert_eq!(t.count(1, TORN_MAP_FRAGMENT), 0);

        // Filaur pays for the ore and the map (both, page 10), the shots once, the guide's mission.
        let ores = t.count(1, CHRYSOLITE_ORE);
        let before = t.count(1, 57);
        t.talk(1, filaur);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(
            crate::testing::pages_to(&out, 1)
                .pop()
                .unwrap_or_default()
                .contains("30535-10.html")
        );
        let bonus = if ores + 1 >= 10 { 2000 } else { 0 };
        assert_eq!(t.count(1, 57) - before, ores * 5 + 500 + bonus);
        assert_eq!(t.count(1, 5789), 6000);
        assert_eq!((t.count(1, CHRYSOLITE_ORE), t.count(1, HIDDEN_ORE_MAP)), (0, 0));
        let writes = t.take_writes();
        assert_eq!(
            writes,
            [
                rows::insert(NEWBIE_GUIDE, "NRmemo", "41"),
                rows::insert(NEWBIE_GUIDE, "NRmemoState41", "1000"),
            ]
        );

        // `exitQuest(true, true)` from the page.
        assert!(t.event(1, NAME, "30535-07.html").contains("30535-07.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
