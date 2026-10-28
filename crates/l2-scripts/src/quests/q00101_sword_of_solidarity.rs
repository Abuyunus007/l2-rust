//! Quest 101: Sword of Solidarity (Roien sends a human to Altran for the directions to the
//! ruins, the Tunath Orcs give the two broken blade parts, and Roien's helper gets the sword).
// Java: dist/game/data/scripts/quests/Q00101_SwordOfSolidarity/Q00101_SwordOfSolidarity.java
//
// Pattern: a delivery chain with a kill chance (`getRandom(5) == 0`) for two parts that set the
// next cond once both are in, and the NewbieGuide memo as in quest 104.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ROIEN: i32 = 30008;
const ALTRAN: i32 = 30283;

// Items
const BROKEN_SWORD_HANDLE: i32 = 739;
const BROKEN_BLADE_BOTTOM: i32 = 740;
const BROKEN_BLADE_TOP: i32 = 741;
const ALTRANS_NOTE: i32 = 742;
const ROIENS_LETTER: i32 = 796;
const DIRECTIONS_TO_RUINS: i32 = 937;

// Monsters
const MONSTERS: [i32; 2] = [
    20361, // Tunath Orc Marksman
    20362, // Tunath Orc Warrior
];

/// Java `REWARDS` (`ItemHolder`s): item id and count.
const REWARDS: [(i32, i64); 7] = [
    (738, 1),    // Sword of Solidarity
    (1060, 100), // Lesser Healing Potion
    (4412, 10),  // Echo Crystal - Theme of Battle
    (4413, 10),  // Echo Crystal - Theme of Love
    (4414, 10),  // Echo Crystal - Theme of Solitude
    (4415, 10),  // Echo Crystal - Theme of Feast
    (4416, 10),  // Echo Crystal - Theme of Celebration
];
const SOULSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5789, 7000);

// Misc
const MIN_LEVEL: i32 = 9;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00101_SwordOfSolidarity`.
pub struct Q00101SwordOfSolidarity;

impl Script for Q00101SwordOfSolidarity {
    fn name(&self) -> &'static str {
        "Q00101_SwordOfSolidarity"
    }

    fn path(&self) -> &'static str {
        "quests/Q00101_SwordOfSolidarity"
    }

    fn id(&self) -> i32 {
        101
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ROIEN]);
        s.add_kill_id(&MONSTERS);
        s.add_talk_id(&[ROIEN, ALTRAN]);
        s.register_quest_items(&[
            BROKEN_SWORD_HANDLE,
            BROKEN_BLADE_BOTTOM,
            BROKEN_BLADE_TOP,
            ALTRANS_NOTE,
            ROIENS_LETTER,
            DIRECTIONS_TO_RUINS,
        ]);
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
            "30008-03.html" | "30008-09.html" => Some(event.to_owned()),
            "30008-04.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, ROIENS_LETTER, 1);
                Some(event.to_owned())
            }
            "30283-02.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, ROIENS_LETTER) {
                    ctx.take_items(player, ROIENS_LETTER, -1);
                    ctx.give_items(player, DIRECTIONS_TO_RUINS, 1);
                    qs.set_cond(ctx, 2, true);
                    return Some(event.to_owned());
                }
                None
            }
            "30283-07.html" => {
                if qs.is_cond(ctx, 5) && ctx.has_quest_items(player, BROKEN_SWORD_HANDLE) {
                    if ctx.player_level(player) < 25 && !ctx.is_mage_class(player) {
                        let (id, count) = SOULSHOTS_NO_GRADE_FOR_ROOKIES;
                        ctx.give_items(player, id, count);
                        ctx.play_sound_named(player, "tutorial_voice_026");
                    }
                    for (id, count) in REWARDS {
                        ctx.give_items(player, id, count);
                    }
                    ctx.add_exp_and_sp(player, 25747, 2171);
                    ctx.give_adena(player, 10981, true);
                    qs.exit_quest(ctx, false, true);
                    ctx.world().send(
                        player,
                        &SocialAction {
                            object_id: player.0,
                            action_id: 3,
                        },
                    );
                    // Newbie Guide.
                    if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                        if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::AcquisitionOfRaceSpecificWeaponCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 1_000_000) / 100_000 != 1 {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 100_000);
                            ctx.show_on_screen_npc_string(
                                player,
                                NpcStringId::AcquisitionOfRaceSpecificWeaponCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        }
                    }
                    return Some(event.to_owned());
                }
                None
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 2) && ctx.get_random(5) == 0 {
            if !ctx.has_quest_items(killer, BROKEN_BLADE_TOP) {
                ctx.give_items(killer, BROKEN_BLADE_TOP, 1);
                if ctx.has_quest_items(killer, BROKEN_BLADE_BOTTOM) {
                    qs.set_cond(ctx, 3, true);
                } else {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            } else if !ctx.has_quest_items(killer, BROKEN_BLADE_BOTTOM) {
                ctx.give_items(killer, BROKEN_BLADE_BOTTOM, 1);
                if ctx.has_quest_items(killer, BROKEN_BLADE_TOP) {
                    qs.set_cond(ctx, 3, true);
                } else {
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ROIEN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::HUMAN {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "30008-02.htm"
                        } else {
                            "30008-08.htm"
                        }
                    } else {
                        "30008-01.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        if ctx.has_quest_items(player, ROIENS_LETTER) {
                            htmltext = "30008-05.html".to_owned();
                        }
                    }
                    2 => {
                        if ctx.has_at_least_one_quest_item(player, &[BROKEN_BLADE_BOTTOM, BROKEN_BLADE_TOP]) {
                            htmltext = "30008-11.html".to_owned();
                        } else if ctx.has_quest_items(player, DIRECTIONS_TO_RUINS) {
                            htmltext = "30008-10.html".to_owned();
                        }
                    }
                    3 => {
                        if ctx.has_all_quest_items(player, &[BROKEN_BLADE_BOTTOM, BROKEN_BLADE_TOP]) {
                            htmltext = "30008-12.html".to_owned();
                        }
                    }
                    4 => {
                        if ctx.has_quest_items(player, ALTRANS_NOTE) {
                            ctx.take_items(player, ALTRANS_NOTE, -1);
                            ctx.give_items(player, BROKEN_SWORD_HANDLE, 1);
                            qs.set_cond(ctx, 5, true);
                            htmltext = "30008-06.html".to_owned();
                        }
                    }
                    5 => {
                        if ctx.has_quest_items(player, BROKEN_SWORD_HANDLE) {
                            htmltext = "30008-07.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ALTRAN => match qs.get_cond(ctx) {
                1 => {
                    if ctx.has_quest_items(player, ROIENS_LETTER) {
                        htmltext = "30283-01.html".to_owned();
                    }
                }
                2 => {
                    if ctx.has_at_least_one_quest_item(player, &[BROKEN_BLADE_BOTTOM, BROKEN_BLADE_TOP]) {
                        htmltext = "30283-08.html".to_owned();
                    } else if ctx.has_quest_items(player, DIRECTIONS_TO_RUINS) {
                        htmltext = "30283-03.html".to_owned();
                    }
                }
                3 => {
                    if ctx.has_all_quest_items(player, &[BROKEN_BLADE_BOTTOM, BROKEN_BLADE_TOP]) {
                        ctx.take_items_many(
                            player,
                            -1,
                            &[DIRECTIONS_TO_RUINS, BROKEN_BLADE_TOP, BROKEN_BLADE_BOTTOM],
                        );
                        ctx.give_items(player, ALTRANS_NOTE, 1);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30283-04.html".to_owned();
                    }
                }
                4 => {
                    if ctx.has_quest_items(player, ALTRANS_NOTE) {
                        htmltext = "30283-05.html".to_owned();
                    }
                }
                5 => {
                    if ctx.has_quest_items(player, BROKEN_SWORD_HANDLE) {
                        htmltext = "30283-06.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::{ALTRAN, Q00101SwordOfSolidarity as Quest, ROIEN};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00101_SwordOfSolidarity";
    const HANDLE: i32 = 739;
    const BOTTOM: i32 = 740;
    const TOP: i32 = 741;
    const NOTE: i32 = 742;
    const LETTER: i32 = 796;
    const DIRECTIONS: i32 = 937;

    const NPCS: &str = r#"
        <npc id="30008" type="Folk" level="70" name="Roien"/>
        <npc id="30283" type="Folk" level="70" name="Altran"/>
        <npc id="20361" type="Monster" level="20" name="Tunath Orc Marksman"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="739" type="EtcItem" name="Broken Sword Handle"><set name="is_questitem" val="true"/></item>
        <item id="740" type="EtcItem" name="Broken Blade Bottom"><set name="is_questitem" val="true"/></item>
        <item id="741" type="EtcItem" name="Broken Blade Top"><set name="is_questitem" val="true"/></item>
        <item id="742" type="EtcItem" name="Altran's Note"><set name="is_questitem" val="true"/></item>
        <item id="796" type="EtcItem" name="Roien's Letter"><set name="is_questitem" val="true"/></item>
        <item id="937" type="EtcItem" name="Directions to the Ruins"><set name="is_questitem" val="true"/></item>
        <item id="738" type="Weapon" name="Sword of Solidarity"/>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>"#;

    fn pages() -> Pages {
        Pages::of(&Quest)
            .page("30008-02.htm", &["30008-03.html"])
            .page("30008-03.html", &["30008-09.html"])
            .page("30008-09.html", &["30008-04.htm"])
            .page("30283-01.html", &["30283-02.html"])
            .page("30283-06.html", &["30283-07.html"])
            .plain(&[
                "30008-01.htm",
                "30008-04.htm",
                "30008-05.html",
                "30008-06.html",
                "30008-07.html",
                "30008-08.htm",
                "30008-10.html",
                "30008-11.html",
                "30008-12.html",
                "30283-02.html",
                "30283-03.html",
                "30283-04.html",
                "30283-05.html",
                "30283-07.html",
                "30283-08.html",
            ])
            .npcs(&[ROIEN, ALTRAN])
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let roien = t.spawn(ROIEN, 20);
        let altran = t.spawn(ALTRAN, 30);
        (t, roien, altran)
    }

    /// Starts the quest through the Roien pages, as a human of `level`.
    fn start(t: &mut ScriptTest, roien: Oid, level: i32) {
        t.enter_as(1, level, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, roien).contains("30008-02.htm"));
        assert!(t.event(1, NAME, "30008-03.html").contains("30008-03.html"));
        assert!(t.event(1, NAME, "30008-09.html").contains("30008-09.html"));
        assert!(t.event(1, NAME, "30008-04.htm").contains("30008-04.htm"));
    }

    #[test]
    fn a_human_of_level_9_starts_and_gets_the_letter() {
        let (mut t, roien, _) = setup();
        start(&mut t, roien, 9);
        assert_eq!(t.count(1, LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_start_is_refused_below_level_9_and_for_other_races() {
        let (mut t, roien, _) = setup();
        t.enter_as(1, 8, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, roien).contains("30008-08.htm"));
        t.enter_as(2, 20, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(2, roien).contains("30008-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_directions_the_blades_the_note_the_handle_and_the_reward() {
        let (mut t, roien, altran) = setup();
        start(&mut t, roien, 20);
        t.take_writes();

        // Altran: the letter goes, the directions come, cond 2.
        assert!(t.quest_page(1, altran).contains("30283-01.html"));
        assert!(t.event(1, NAME, "30283-02.html").contains("30283-02.html"));
        assert_eq!((t.count(1, LETTER), t.count(1, DIRECTIONS)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The Tunath Orcs: the blade parts drop by chance; both in, cond 3.
        for _ in 0..500 {
            if t.count(1, TOP) == 1 && t.count(1, BOTTOM) == 1 {
                break;
            }
            let orc = t.spawn(20361, 60);
            t.kill(1, orc);
        }
        assert_eq!((t.count(1, TOP), t.count(1, BOTTOM)), (1, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Altran: the parts and the directions go, the note comes, cond 4.
        assert!(t.quest_page(1, altran).contains("30283-04.html"));
        assert_eq!(
            (
                t.count(1, TOP),
                t.count(1, BOTTOM),
                t.count(1, DIRECTIONS),
                t.count(1, NOTE)
            ),
            (0, 0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Roien: the note goes, the handle comes, cond 5.
        assert!(t.quest_page(1, roien).contains("30008-06.html"));
        assert_eq!((t.count(1, NOTE), t.count(1, HANDLE)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Altran: the sword and the exit.
        assert!(t.quest_page(1, altran).contains("30283-06.html"));
        assert!(t.event(1, NAME, "30283-07.html").contains("30283-07.html"));
        assert_eq!(t.count(1, 738), 1);
        assert_eq!(t.count(1, 1060), 100);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
