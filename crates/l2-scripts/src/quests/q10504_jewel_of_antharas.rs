//! Quest 10504: Jewel of Antharas (Theodric in the Antharas lair, Antharas).
// Java: dist/game/data/scripts/quests/Q10504_JewelOfAntharas/Q10504_JewelOfAntharas.java
//
// Pattern: a party kill (`docs/design/scripts.md`, "Kills, drops and parties"): `actionForEachPlayer`
// swaps the empty crystal for the filled one and sets cond 2 for each member in range. The NPC
// gives the jewel at cond 2 and the quest ends (one time).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const THEODRIC: i32 = 30755;

// Monster
const ANTHARAS: i32 = 29068;

// Items
const CLEAR_CRYSTAL: i32 = 21905;
const FILLED_CRYSTAL_ANTHARAS_ENERGY: i32 = 21907;
const JEWEL_OF_ANTHARAS: i32 = 21898;
const PORTAL_STONE: i32 = 3865;

// Misc
const MIN_LEVEL: i32 = 84;

/// Java `Q10504_JewelOfAntharas`.
pub struct Q10504JewelOfAntharas;

impl Script for Q10504JewelOfAntharas {
    fn name(&self) -> &'static str {
        "Q10504_JewelOfAntharas"
    }

    fn path(&self) -> &'static str {
        "quests/Q10504_JewelOfAntharas"
    }

    fn id(&self) -> i32 {
        10504
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[THEODRIC]);
        s.add_talk_id(&[THEODRIC]);
        s.add_kill_id(&[ANTHARAS]);
        s.register_quest_items(&[CLEAR_CRYSTAL, FILLED_CRYSTAL_ANTHARAS_ENERGY]);
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
        {
            ctx.take_items(player, CLEAR_CRYSTAL, -1);
            ctx.give_items(player, FILLED_CRYSTAL_ANTHARAS_ENERGY, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            qs.set_cond(ctx, 2, true);
        }
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
        if ctx.player_level(player) >= MIN_LEVEL && ctx.has_quest_items(player, PORTAL_STONE) {
            match event {
                "30755-05.htm" | "30755-06.htm" => htmltext = Some(event.to_owned()),
                "30755-07.html" => {
                    qs.start_quest(ctx);
                    ctx.give_items(player, CLEAR_CRYSTAL, 1);
                    htmltext = Some(event.to_owned());
                }
                _ => {}
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, true);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30755-02.html"
                } else if !ctx.has_quest_items(player, PORTAL_STONE) {
                    "30755-04.html"
                } else {
                    "30755-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    if ctx.has_quest_items(player, CLEAR_CRYSTAL) {
                        htmltext = "30755-08.html".to_owned();
                    } else {
                        ctx.give_items(player, CLEAR_CRYSTAL, 1);
                        htmltext = "30755-09.html".to_owned();
                    }
                }
                2 => {
                    ctx.give_items(player, JEWEL_OF_ANTHARAS, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30755-10.html".to_owned();
                }
                _ => {}
            },
            COMPLETED => htmltext = "30755-03.html".to_owned(),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, player_data, rows};

    const NAME: &str = "Q10504_JewelOfAntharas";

    const NPCS: &str = r#"
        <npc id="30755" type="Folk" level="80" name="Theodric"/>
        <npc id="29068" type="RaidBoss" level="80" name="Antharas"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3865" type="EtcItem" name="Portal Stone"><set name="is_stackable" val="true"/></item>
        <item id="21905" type="EtcItem" name="Clear Crystal"><set name="is_questitem" val="true"/></item>
        <item id="21907" type="EtcItem" name="Filled Crystal: Antharas' Energy"><set name="is_questitem" val="true"/></item>
        <item id="21898" type="EtcItem" name="Jewel of Antharas"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10504JewelOfAntharas);
        let pages = Pages::of(&Q10504JewelOfAntharas)
            .page("30755-01.htm", &["30755-05.htm"])
            .page("30755-05.htm", &["30755-06.htm"])
            .page("30755-06.htm", &["30755-07.html"])
            .plain(&[
                "30755-02.html",
                "30755-03.html",
                "30755-04.html",
                "30755-07.html",
                "30755-08.html",
                "30755-09.html",
                "30755-10.html",
            ])
            .npcs(&[THEODRIC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let theodric = t.spawn(THEODRIC, 20);
        (t, theodric)
    }

    #[test]
    fn the_level_and_the_portal_stone_gate_the_start() {
        let (mut t, theodric) = setup();
        t.enter(1, 83, vec![item(100, PORTAL_STONE, 1)]);
        assert!(t.quest_page(1, theodric).contains("30755-02.html"));
        t.enter(2, 84, vec![]);
        assert!(t.quest_page(2, theodric).contains("30755-04.html"));
        t.enter(3, 84, vec![item(102, PORTAL_STONE, 1)]);
        assert!(t.quest_page(3, theodric).contains("30755-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_the_kill_then_the_jewel() {
        let (mut t, theodric) = setup();
        t.enter(1, 84, vec![item(100, PORTAL_STONE, 1)]);
        assert!(t.quest_page(1, theodric).contains("30755-01.htm"));
        assert!(t.event(1, NAME, "30755-05.htm").contains("30755-05.htm"));
        assert!(t.event(1, NAME, "30755-06.htm").contains("30755-06.htm"));
        assert!(t.event(1, NAME, "30755-07.html").contains("30755-07.html"));
        assert_eq!(t.count(1, CLEAR_CRYSTAL), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, theodric).contains("30755-08.html"));

        // A kill far away does nothing.
        let far = t
            .world
            .spawn_npc_at(
                ANTHARAS,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, CLEAR_CRYSTAL), 1);
        assert_eq!(t.count(1, FILLED_CRYSTAL_ANTHARAS_ENERGY), 0);

        // Antharas near: the clear crystal becomes the filled one, the quest moves to cond 2.
        let antharas = t.spawn(ANTHARAS, 60);
        t.kill(1, antharas);
        assert_eq!(t.count(1, CLEAR_CRYSTAL), 0);
        assert_eq!(t.count(1, FILLED_CRYSTAL_ANTHARAS_ENERGY), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The jewel and the end of the quest.
        t.talk(1, theodric);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert!(
            pages_to(&out, 1)
                .last()
                .is_some_and(|p| p.contains("30755-10.html"))
        );
        assert_eq!(t.count(1, JEWEL_OF_ANTHARAS), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, theodric).contains("30755-03.html"));
    }

    #[test]
    fn a_started_character_without_the_clear_crystal_gets_one() {
        let (mut t, theodric) = setup();
        let mut data = player_data(1, 84, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some("1".to_owned()))],
        };
        t.enter_with(data);
        assert!(t.quest_page(1, theodric).contains("30755-09.html"));
        assert_eq!(t.count(1, CLEAR_CRYSTAL), 1);
        assert!(t.take_writes().is_empty());
    }
}
