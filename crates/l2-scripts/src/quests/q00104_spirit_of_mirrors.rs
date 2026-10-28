//! Quest 104: Spirit of Mirrors (Gallint gives three Gallint's Oak Wands; a wand equipped in the
//! right hand kills a Spirit of Mirrors for a Spirit-bound Wand).
// Java: dist/game/data/scripts/quests/Q00104_SpiritOfMirrors/Q00104_SpiritOfMirrors.java
//
// Pattern: a kill chain that needs an item equipped (`getItemEquipped`), the NewbieGuide memo
// (`ScriptManager.getScript("NewbieGuide")`), and the three guards' names as quest variables.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GALLINT: i32 = 30017;
const ARNOLD: i32 = 30041;
const JOHNSTONE: i32 = 30043;
const KENYOS: i32 = 30045;

// Items
const GALLINTS_OAK_WAND: i32 = 748;
const SPIRITBOUND_WAND1: i32 = 1135;
const SPIRITBOUND_WAND2: i32 = 1136;
const SPIRITBOUND_WAND3: i32 = 1137;

/// Java `MONSTERS`: Spirit Of Mirrors -> the wand it gives.
const MONSTERS: [(i32, i32); 3] = [
    (27003, SPIRITBOUND_WAND1),
    (27004, SPIRITBOUND_WAND2),
    (27005, SPIRITBOUND_WAND3),
];

/// Java `REWARDS` (`ItemHolder`s): item id and count.
const REWARDS: [(i32, i64); 7] = [
    (1060, 100), // Lesser Healing Potion
    (4412, 10),  // Echo Crystal - Theme of Battle
    (4413, 10),  // Echo Crystal - Theme of Love
    (4414, 10),  // Echo Crystal - Theme of Solitude
    (4415, 10),  // Echo Crystal - Theme of Feast
    (4416, 10),  // Echo Crystal - Theme of Celebration
    (747, 1),    // Wand of Adept
];
const SPIRITSHOTS_NO_GRADE_FOR_ROOKIES: (i32, i64) = (5790, 3000);
const SOULSHOTS_NO_GRADE: (i32, i64) = (1835, 1000);
const SPIRITSHOTS_NO_GRADE: (i32, i64) = (2509, 500);

// Misc
const MIN_LEVEL: i32 = 10;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `MONSTERS.get(npcId)`: the wand a Spirit of Mirrors gives (registered kills only).
fn wand_of(npc_id: i32) -> Option<i32> {
    MONSTERS
        .iter()
        .find(|(monster, _)| *monster == npc_id)
        .map(|(_, wand)| *wand)
}

/// Java `getItemEquipped(player, slot)`: the item id in a paperdoll slot, 0 when empty.
fn item_equipped(ctx: &ScriptCtx<'_>, player: ObjectId, slot: usize) -> i32 {
    let Some(object_id) = ctx.world().paperdoll_object(player, slot) else {
        return 0;
    };
    ctx.world()
        .player(player)
        .and_then(|p| p.data.items.iter().find(|i| i.object_id == object_id))
        .map_or(0, |i| i.item_id)
}

/// Java `Q00104_SpiritOfMirrors`.
pub struct Q00104SpiritOfMirrors;

impl Script for Q00104SpiritOfMirrors {
    fn name(&self) -> &'static str {
        "Q00104_SpiritOfMirrors"
    }

    fn path(&self) -> &'static str {
        "quests/Q00104_SpiritOfMirrors"
    }

    fn id(&self) -> i32 {
        104
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GALLINT]);
        s.add_talk_id(&[ARNOLD, GALLINT, JOHNSTONE, KENYOS]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[
            GALLINTS_OAK_WAND,
            SPIRITBOUND_WAND1,
            SPIRITBOUND_WAND2,
            SPIRITBOUND_WAND3,
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
        if event == "30017-04.htm" {
            qs.start_quest(ctx);
            ctx.give_items(player, GALLINTS_OAK_WAND, 3);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        let Some(wand) = wand_of(npc.id) else {
            return;
        };
        if (qs.is_cond(ctx, 1) || qs.is_cond(ctx, 2))
            && item_equipped(ctx, killer, paperdoll::RHAND) == GALLINTS_OAK_WAND
            && !ctx.has_quest_items(killer, wand)
        {
            ctx.take_items(killer, GALLINTS_OAK_WAND, 1);
            ctx.give_items(killer, wand, 1);
            if ctx.has_all_quest_items(killer, &[SPIRITBOUND_WAND1, SPIRITBOUND_WAND2, SPIRITBOUND_WAND3]) {
                qs.set_cond(ctx, 3, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            GALLINT => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::HUMAN {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "30017-03.htm"
                        } else {
                            "30017-02.htm"
                        }
                    } else {
                        "30017-01.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 3)
                        && ctx.has_all_quest_items(
                            player,
                            &[SPIRITBOUND_WAND1, SPIRITBOUND_WAND2, SPIRITBOUND_WAND3],
                        )
                    {
                        if ctx.player_level(player) < 25 && ctx.is_mage_class(player) {
                            let (id, count) = SPIRITSHOTS_NO_GRADE_FOR_ROOKIES;
                            ctx.give_items(player, id, count);
                            ctx.play_sound_named(player, "tutorial_voice_027");
                        }
                        if !ctx.is_mage_class(player) {
                            let (id, count) = SOULSHOTS_NO_GRADE;
                            ctx.give_items(player, id, count);
                        } else {
                            let (id, count) = SPIRITSHOTS_NO_GRADE;
                            ctx.give_items(player, id, count);
                        }
                        for (id, count) in REWARDS {
                            ctx.give_items(player, id, count);
                        }
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
                            } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 1_000_000) / 100_000 != 1
                            {
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
                        ctx.add_exp_and_sp(player, 39750, 3407);
                        ctx.give_adena(player, 16866, true);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30017-06.html".to_owned();
                    } else {
                        htmltext = "30017-05.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ARNOLD | JOHNSTONE | KENYOS => {
                if qs.is_cond(ctx, 1) {
                    // NOTE(java-quirk): Java keys the variable by `npc.getName()`; these are the
                    // template names of the three guards (stats/npcs/30000-30099.xml).
                    let name = match npc.id {
                        ARNOLD => "Arnold",
                        JOHNSTONE => "Johnstone",
                        _ => "Kenyos",
                    };
                    if !qs.is_set(ctx, name) {
                        qs.set(ctx, name, "1");
                    }
                    if qs.is_set(ctx, "Arnold") && qs.is_set(ctx, "Johnstone") && qs.is_set(ctx, "Kenyos") {
                        qs.set_cond(ctx, 2, true);
                    }
                }
                htmltext = format!("{}-01.html", npc.id);
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_protocol_game::server::charscreen::paperdoll;
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::{
        ARNOLD, GALLINT, GALLINTS_OAK_WAND, JOHNSTONE, KENYOS, Q00104SpiritOfMirrors as Quest,
        SPIRITBOUND_WAND1, SPIRITBOUND_WAND2, SPIRITBOUND_WAND3,
    };
    use crate::testing::{Pages, ScriptTest, item, rows};
    use l2_world::ItemData;

    const NAME: &str = "Q00104_SpiritOfMirrors";
    const MONSTER1: i32 = 27003;
    const MONSTER3: i32 = 27005;

    const NPCS: &str = r#"
        <npc id="30017" type="Folk" level="70" name="Gallint"/>
        <npc id="30041" type="Guard" level="80" name="Arnold"/>
        <npc id="30043" type="Guard" level="80" name="Johnstone"/>
        <npc id="30045" type="Guard" level="80" name="Kenyos"/>
        <npc id="27003" type="Monster" level="76" name="Spirit Of Mirrors"/>
        <npc id="27005" type="Monster" level="76" name="Spirit Of Mirrors"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="748" type="Weapon" name="Gallint's Oak Wand"><set name="is_questitem" val="true"/></item>
        <item id="1135" type="Weapon" name="Spirit-bound Wand 1"><set name="is_questitem" val="true"/></item>
        <item id="1136" type="Weapon" name="Spirit-bound Wand 2"><set name="is_questitem" val="true"/></item>
        <item id="1137" type="Weapon" name="Spirit-bound Wand 3"><set name="is_questitem" val="true"/></item>
        <item id="1060" type="EtcItem" name="Lesser Healing Potion"><set name="is_stackable" val="true"/></item>
        <item id="747" type="Weapon" name="Wand of Adept"/>"#;

    fn pages() -> Pages {
        Pages::of(&Quest)
            .page("30017-03.htm", &["30017-04.htm"])
            .plain(&[
                "30017-01.htm",
                "30017-02.htm",
                "30017-04.htm",
                "30017-05.html",
                "30017-06.html",
                "30041-01.html",
                "30043-01.html",
                "30045-01.html",
            ])
            // The guards' default pages are in the `guard` folder (`Guard` type).
            .file(
                "data/html/guard/30041.htm",
                r#"<html><body>guard <a action="bypass Script">Quest</a></body></html>"#,
            )
            .file(
                "data/html/guard/30043.htm",
                r#"<html><body>guard <a action="bypass Script">Quest</a></body></html>"#,
            )
            .file(
                "data/html/guard/30045.htm",
                r#"<html><body>guard <a action="bypass Script">Quest</a></body></html>"#,
            )
            .npcs(&[GALLINT])
    }

    /// An item in the right hand: the wand the kills need.
    fn equipped_wand(object_id: i32) -> ItemData {
        ItemData {
            loc: "PAPERDOLL".to_owned(),
            loc_data: paperdoll::RHAND as i32,
            ..item(object_id, GALLINTS_OAK_WAND, 1)
        }
    }

    struct Setup {
        t: ScriptTest,
        gallint: Oid,
    }

    fn setup() -> Setup {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let gallint = t.spawn(GALLINT, 20);
        t.spawn(ARNOLD, 30);
        t.spawn(JOHNSTONE, 40);
        t.spawn(KENYOS, 50);
        Setup { t, gallint }
    }

    /// Starts the quest and sets `cond` through the world (the steps are tested apart).
    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, cond, false);
        });
    }

    #[test]
    fn a_human_of_level_10_starts_and_gets_three_wands() {
        let Setup { mut t, gallint } = setup();
        t.enter_as(1, 10, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, gallint).contains("30017-03.htm"));
        assert!(t.event(1, NAME, "30017-04.htm").contains("30017-04.htm"));
        assert_eq!(t.count(1, GALLINTS_OAK_WAND), 3);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_start_gates_by_race_and_level() {
        let Setup { mut t, gallint } = setup();
        t.enter_as(1, 9, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, gallint).contains("30017-02.htm"));
        t.enter_as(2, 20, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(2, gallint).contains("30017-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_guards_set_their_names_and_the_third_one_moves_on() {
        let Setup { mut t, .. } = setup();
        t.enter_as(1, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        set_cond(&mut t, 1);
        t.take_writes();
        let arnold = t.npcs_of(ARNOLD)[0];
        let johnstone = t.npcs_of(JOHNSTONE)[0];
        let kenyos = t.npcs_of(KENYOS)[0];
        assert!(t.quest_page(1, arnold).contains("30041-01.html"));
        assert!(t.quest_page(1, johnstone).contains("30043-01.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "Arnold", "1"),
                rows::insert(NAME, "Johnstone", "1"),
            ]
        );
        assert!(t.quest_page(1, kenyos).contains("30045-01.html"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "Kenyos", "1"), rows::update(NAME, "cond", "2"),]
        );
    }

    #[test]
    fn a_wand_in_hand_kills_a_spirit_for_its_wand() {
        let Setup { mut t, .. } = setup();
        t.enter_as(
            1,
            20,
            race::HUMAN,
            player_class::FIGHTER,
            vec![
                equipped_wand(0x6000_0100),
                item(0x6000_0101, SPIRITBOUND_WAND2, 1),
            ],
        );
        set_cond(&mut t, 2);
        t.take_writes();
        let mob = t.spawn(MONSTER1, 60);
        t.kill(1, mob);
        assert_eq!(t.count(1, SPIRITBOUND_WAND1), 1);
        assert_eq!(t.count(1, GALLINTS_OAK_WAND), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_last_wand_moves_on_to_gallint() {
        let Setup { mut t, gallint } = setup();
        t.enter_as(
            1,
            20,
            race::HUMAN,
            player_class::FIGHTER,
            vec![
                equipped_wand(0x6000_0100),
                item(0x6000_0101, SPIRITBOUND_WAND1, 1),
                item(0x6000_0102, SPIRITBOUND_WAND2, 1),
            ],
        );
        set_cond(&mut t, 2);
        t.take_writes();
        let mob = t.spawn(MONSTER3, 60);
        t.kill(1, mob);
        assert_eq!(t.count(1, SPIRITBOUND_WAND3), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Cond 3 with the three wands: the reward, the exp and the one-time exit.
        assert!(t.quest_page(1, gallint).contains("30017-06.html"));
        assert_eq!(t.count(1, 1060), 100);
        assert_eq!(t.count(1, 747), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, gallint).contains("already been completed"));
    }
}
