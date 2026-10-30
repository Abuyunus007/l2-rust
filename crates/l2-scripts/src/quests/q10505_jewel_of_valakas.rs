//! Quest 10505: Jewel of Valakas (Klein in the Valakas lair, Valakas).
// Java: dist/game/data/scripts/quests/Q10505_JewelOfValakas/Q10505_JewelOfValakas.java
//
// Pattern: a party kill (`docs/design/scripts.md`, "Kills, drops and parties"): `actionForEachPlayer`
// swaps the empty crystal for the filled one and sets cond 2 for each member in range. The NPC
// gives the jewel at cond 2 and the quest ends (one time).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KLEIN: i32 = 31540;

// Monster
const VALAKAS: i32 = 29028;

// Items
const EMPTY_CRYSTAL: i32 = 21906;
const FILLED_CRYSTAL_VALAKAS_ENERGY: i32 = 21908;
const JEWEL_OF_VALAKAS: i32 = 21896;
const VACUALITE_FLOATING_STONE: i32 = 7267;

// Misc
const MIN_LEVEL: i32 = 83;

/// Java `Q10505_JewelOfValakas`.
pub struct Q10505JewelOfValakas;

impl Script for Q10505JewelOfValakas {
    fn name(&self) -> &'static str {
        "Q10505_JewelOfValakas"
    }

    fn path(&self) -> &'static str {
        "quests/Q10505_JewelOfValakas"
    }

    fn id(&self) -> i32 {
        10505
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLEIN]);
        s.add_talk_id(&[KLEIN]);
        s.add_kill_id(&[VALAKAS]);
        s.register_quest_items(&[EMPTY_CRYSTAL, FILLED_CRYSTAL_VALAKAS_ENERGY]);
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
            ctx.take_items(player, EMPTY_CRYSTAL, -1);
            ctx.give_items(player, FILLED_CRYSTAL_VALAKAS_ENERGY, 1);
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
        if ctx.player_level(player) >= MIN_LEVEL && ctx.has_quest_items(player, VACUALITE_FLOATING_STONE) {
            match event {
                "31540-05.htm" | "31540-06.htm" => htmltext = Some(event.to_owned()),
                "31540-07.html" => {
                    qs.start_quest(ctx);
                    ctx.give_items(player, EMPTY_CRYSTAL, 1);
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
                    "31540-02.html"
                } else if !ctx.has_quest_items(player, VACUALITE_FLOATING_STONE) {
                    "31540-04.html"
                } else {
                    "31540-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => {
                    if ctx.has_quest_items(player, EMPTY_CRYSTAL) {
                        htmltext = "31540-08.html".to_owned();
                    } else {
                        ctx.give_items(player, EMPTY_CRYSTAL, 1);
                        htmltext = "31540-09.html".to_owned();
                    }
                }
                2 => {
                    ctx.give_items(player, JEWEL_OF_VALAKAS, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "31540-10.html".to_owned();
                }
                _ => {}
            },
            COMPLETED => htmltext = "31540-03.html".to_owned(),
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

    const NAME: &str = "Q10505_JewelOfValakas";

    const NPCS: &str = r#"
        <npc id="31540" type="Folk" level="80" name="Klein"/>
        <npc id="29028" type="RaidBoss" level="80" name="Valakas"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7267" type="EtcItem" name="Vacualite Floating Stone"><set name="is_stackable" val="true"/></item>
        <item id="21906" type="EtcItem" name="Empty Crystal"><set name="is_questitem" val="true"/></item>
        <item id="21908" type="EtcItem" name="Filled Crystal: Valakas' Energy"><set name="is_questitem" val="true"/></item>
        <item id="21896" type="EtcItem" name="Jewel of Valakas"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10505JewelOfValakas);
        let pages = Pages::of(&Q10505JewelOfValakas)
            .page("31540-01.htm", &["31540-05.htm"])
            .page("31540-05.htm", &["31540-06.htm"])
            .page("31540-06.htm", &["31540-07.html"])
            .plain(&[
                "31540-02.html",
                "31540-03.html",
                "31540-04.html",
                "31540-07.html",
                "31540-08.html",
                "31540-09.html",
                "31540-10.html",
            ])
            .npcs(&[KLEIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let klein = t.spawn(KLEIN, 20);
        (t, klein)
    }

    #[test]
    fn the_level_and_the_vacualite_stone_gate_the_start() {
        let (mut t, klein) = setup();
        t.enter(1, 82, vec![item(100, VACUALITE_FLOATING_STONE, 1)]);
        assert!(t.quest_page(1, klein).contains("31540-02.html"));
        t.enter(2, 83, vec![]);
        assert!(t.quest_page(2, klein).contains("31540-04.html"));
        t.enter(3, 83, vec![item(102, VACUALITE_FLOATING_STONE, 1)]);
        assert!(t.quest_page(3, klein).contains("31540-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_the_kill_then_the_jewel() {
        let (mut t, klein) = setup();
        t.enter(1, 83, vec![item(100, VACUALITE_FLOATING_STONE, 1)]);
        assert!(t.quest_page(1, klein).contains("31540-01.htm"));
        assert!(t.event(1, NAME, "31540-05.htm").contains("31540-05.htm"));
        assert!(t.event(1, NAME, "31540-06.htm").contains("31540-06.htm"));
        assert!(t.event(1, NAME, "31540-07.html").contains("31540-07.html"));
        assert_eq!(t.count(1, EMPTY_CRYSTAL), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, klein).contains("31540-08.html"));

        // A kill far away does nothing.
        let far = t
            .world
            .spawn_npc_at(
                VALAKAS,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, EMPTY_CRYSTAL), 1);
        assert_eq!(t.count(1, FILLED_CRYSTAL_VALAKAS_ENERGY), 0);

        // Valakas near: the empty crystal becomes the filled one, the quest moves to cond 2.
        let valakas = t.spawn(VALAKAS, 60);
        t.kill(1, valakas);
        assert_eq!(t.count(1, EMPTY_CRYSTAL), 0);
        assert_eq!(t.count(1, FILLED_CRYSTAL_VALAKAS_ENERGY), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The jewel and the end of the quest.
        t.talk(1, klein);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert!(
            pages_to(&out, 1)
                .last()
                .is_some_and(|p| p.contains("31540-10.html"))
        );
        assert_eq!(t.count(1, JEWEL_OF_VALAKAS), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, klein).contains("31540-03.html"));
    }

    #[test]
    fn a_started_character_without_the_empty_crystal_gets_one() {
        let (mut t, klein) = setup();
        let mut data = player_data(1, 83, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some("1".to_owned()))],
        };
        t.enter_with(data);
        assert!(t.quest_page(1, klein).contains("31540-09.html"));
        assert_eq!(t.count(1, EMPTY_CRYSTAL), 1);
        assert!(t.take_writes().is_empty());
    }
}
