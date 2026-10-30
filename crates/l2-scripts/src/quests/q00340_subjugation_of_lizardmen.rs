//! Quest 340: Subjugation Of Lizardmen (Guard Weisz, Priestess Levian, Priest Adonius, the
//! Serpent Demon Bifrons and its chest in Innadril).
// Java: dist/game/data/scripts/quests/Q00340_SubjugationOfLizardmen/Q00340_SubjugationOfLizardmen.java
//
// Pattern: the memo state is the step (1..7), kept as Java keeps it: `setMemoState` next to every
// `setCond`. The trade cargo comes from kills (limit 30) and is sold; the holy symbol and the
// rosary are dropped by the Langk lizardmen; the Bifrons kill spawns the chest that holds the totem.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HIGH_PRIESTESS_LEVIAN: i32 = 30037;
const PRIEST_ADONIUS: i32 = 30375;
const GUARD_WEISZ: i32 = 30385;
const CHEST_OF_BIFRONS: i32 = 30989;

// Items
const TRADE_CARGO: i32 = 4255;
const AGNESS_HOLY_SYMBOL: i32 = 4256;
const AGNESS_ROSARY: i32 = 4257;
const SINISTER_TOTEM: i32 = 4258;

// Monsters
const FELIM_LIZARDMAN: i32 = 20008;
const FELIM_LIZARDMAN_SCOUT: i32 = 20010;
const FELIM_LIZARDMAN_WARRIOR: i32 = 20014;
const LANGK_LIZARDMAN_WARRIOR: i32 = 20024;
const LANGK_LIZARDMAN_SCOUT: i32 = 20027;
const LANGK_LIZARDMAN: i32 = 20030;

// Raid Boss
const SERPENT_DEMON_BIFRONS: i32 = 25146;

// Misc
const MIN_LEVEL: i32 = 17;

/// Java `Q00340_SubjugationOfLizardmen`.
pub struct Q00340SubjugationOfLizardmen;

impl Script for Q00340SubjugationOfLizardmen {
    fn name(&self) -> &'static str {
        "Q00340_SubjugationOfLizardmen"
    }

    fn path(&self) -> &'static str {
        "quests/Q00340_SubjugationOfLizardmen"
    }

    fn id(&self) -> i32 {
        340
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUARD_WEISZ]);
        s.add_talk_id(&[
            GUARD_WEISZ,
            HIGH_PRIESTESS_LEVIAN,
            PRIEST_ADONIUS,
            CHEST_OF_BIFRONS,
        ]);
        s.add_kill_id(&[
            FELIM_LIZARDMAN,
            FELIM_LIZARDMAN_SCOUT,
            FELIM_LIZARDMAN_WARRIOR,
            LANGK_LIZARDMAN_WARRIOR,
            LANGK_LIZARDMAN_SCOUT,
            LANGK_LIZARDMAN,
            SERPENT_DEMON_BIFRONS,
        ]);
        s.register_quest_items(&[TRADE_CARGO, AGNESS_HOLY_SYMBOL, AGNESS_ROSARY, SINISTER_TOTEM]);
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
            "30385-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30385-04.html" | "30385-08.html" => {
                htmltext = Some(event.to_owned());
            }
            "30385-07.html" => {
                ctx.take_items(player, TRADE_CARGO, -1);
                qs.set_memo_state(ctx, 2);
                qs.set_cond(ctx, 2, true);
                htmltext = Some(event.to_owned());
            }
            "30385-09.html" => {
                if ctx.get_quest_items_count(player, TRADE_CARGO) >= 30 {
                    ctx.give_adena(player, 4090, true);
                    ctx.take_items(player, TRADE_CARGO, -1);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30385-10.html" => {
                if ctx.get_quest_items_count(player, TRADE_CARGO) >= 30 {
                    ctx.give_adena(player, 4090, true);
                    ctx.take_items(player, TRADE_CARGO, -1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30037-02.html" => {
                qs.set_memo_state(ctx, 5);
                qs.set_cond(ctx, 5, true);
                htmltext = Some(event.to_owned());
            }
            "30375-02.html" => {
                qs.set_memo_state(ctx, 3);
                qs.set_cond(ctx, 3, true);
                htmltext = Some(event.to_owned());
            }
            "30989-02.html" => {
                if qs.is_memo_state(ctx, 5) {
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 6, true);
                    ctx.give_items(player, SINISTER_TOTEM, 1);
                    htmltext = Some(event.to_owned());
                } else {
                    htmltext = Some("30989-03.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_started(ctx)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
        {
            match npc.id {
                FELIM_LIZARDMAN | FELIM_LIZARDMAN_SCOUT => {
                    if qs.is_memo_state(ctx, 1) {
                        ctx.give_item_randomly(killer, Some(npc), TRADE_CARGO, 1, 1, 30, 0.63, true);
                    }
                }
                FELIM_LIZARDMAN_WARRIOR => {
                    if qs.is_memo_state(ctx, 1) {
                        ctx.give_item_randomly(killer, Some(npc), TRADE_CARGO, 1, 1, 30, 0.68, true);
                    }
                }
                LANGK_LIZARDMAN_WARRIOR => {
                    if qs.is_memo_state(ctx, 3) {
                        if !ctx.has_quest_items(killer, AGNESS_HOLY_SYMBOL) && ctx.get_random(100) <= 19 {
                            ctx.give_items(killer, AGNESS_HOLY_SYMBOL, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        } else if ctx.has_quest_items(killer, AGNESS_HOLY_SYMBOL)
                            && !ctx.has_quest_items(killer, AGNESS_ROSARY)
                            && ctx.get_random(100) <= 18
                        {
                            ctx.give_items(killer, AGNESS_ROSARY, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                LANGK_LIZARDMAN_SCOUT | LANGK_LIZARDMAN => {
                    if qs.is_memo_state(ctx, 3) {
                        if !ctx.has_quest_items(killer, AGNESS_HOLY_SYMBOL) && ctx.get_random(100) <= 18 {
                            ctx.give_items(killer, AGNESS_HOLY_SYMBOL, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        } else if ctx.has_quest_items(killer, AGNESS_HOLY_SYMBOL)
                            && !ctx.has_quest_items(killer, AGNESS_ROSARY)
                            && ctx.get_random(100) <= 18
                        {
                            ctx.give_items(killer, AGNESS_ROSARY, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                SERPENT_DEMON_BIFRONS => {
                    // Java `addSpawn(CHEST_OF_BIFRONS, npc, true, 30000)`: the chest where the boss died.
                    ctx.add_spawn_at(CHEST_OF_BIFRONS, npc.object_id, true, 30_000);
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == GUARD_WEISZ {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30385-02.htm"
                } else {
                    "30385-01.htm"
                }
                .to_owned();
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                GUARD_WEISZ => {
                    if memo_state == 1 {
                        if ctx.get_quest_items_count(player, TRADE_CARGO) < 30 {
                            htmltext = "30385-05.html".to_owned();
                        } else {
                            htmltext = "30385-06.html".to_owned();
                        }
                    } else if memo_state == 2 {
                        htmltext = "30385-11.html".to_owned();
                    } else if (3..7).contains(&memo_state) {
                        htmltext = "30385-12.html".to_owned();
                    } else if memo_state == 7 {
                        ctx.give_adena(player, 14700, true);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30385-13.html".to_owned();
                    }
                }
                HIGH_PRIESTESS_LEVIAN => {
                    if memo_state == 4 {
                        htmltext = "30037-01.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "30037-03.html".to_owned();
                    } else if memo_state == 6 {
                        ctx.take_items(player, SINISTER_TOTEM, 1);
                        qs.set_memo_state(ctx, 7);
                        qs.set_cond(ctx, 7, true);
                        htmltext = "30037-04.html".to_owned();
                    } else if memo_state == 7 {
                        htmltext = "30037-05.html".to_owned();
                    }
                }
                PRIEST_ADONIUS => {
                    if memo_state == 2 {
                        htmltext = "30375-01.html".to_owned();
                    } else if memo_state == 3 {
                        if ctx.has_all_quest_items(player, &[AGNESS_HOLY_SYMBOL, AGNESS_ROSARY]) {
                            ctx.take_items(player, AGNESS_HOLY_SYMBOL, 1);
                            ctx.take_items(player, AGNESS_ROSARY, 1);
                            qs.set_memo_state(ctx, 4);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30375-04.html".to_owned();
                        } else {
                            htmltext = "30375-03.html".to_owned();
                        }
                    } else if memo_state == 4 {
                        htmltext = "30375-05.html".to_owned();
                    } else if memo_state >= 5 {
                        htmltext = "30375-06.html".to_owned();
                    }
                }
                CHEST_OF_BIFRONS => {
                    if memo_state == 5 {
                        htmltext = "30989-01.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == GUARD_WEISZ {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00340_SubjugationOfLizardmen";

    const NPCS: &str = r#"
        <npc id="30385" type="Folk" level="40" name="Guard Weisz"/>
        <npc id="30037" type="Folk" level="40" name="High Priestess Levian"/>
        <npc id="30375" type="Folk" level="40" name="Priest Adonius"/>
        <npc id="30989" type="Folk" level="1" name="Chest of Bifrons"/>
        <npc id="20008" type="Monster" level="20" name="Felim Lizardman"/>
        <npc id="20010" type="Monster" level="20" name="Felim Lizardman Scout"/>
        <npc id="20014" type="Monster" level="20" name="Felim Lizardman Warrior"/>
        <npc id="20024" type="Monster" level="20" name="Langk Lizardman Warrior"/>
        <npc id="20027" type="Monster" level="20" name="Langk Lizardman Scout"/>
        <npc id="20030" type="Monster" level="20" name="Langk Lizardman"/>
        <npc id="25146" type="Monster" level="30" name="Serpent Demon Bifrons"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4255" type="EtcItem" name="Trade Cargo"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="4256" type="EtcItem" name="Agness Holy Symbol"><set name="is_questitem" val="true"/></item>
        <item id="4257" type="EtcItem" name="Agness Rosary"><set name="is_questitem" val="true"/></item>
        <item id="4258" type="EtcItem" name="Sinister Totem"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00340SubjugationOfLizardmen);
        let pages = Pages::of(&Q00340SubjugationOfLizardmen)
            .page("30385-02.htm", &["30385-03.htm"])
            .page("30385-03.htm", &["30385-04.html"])
            .plain(&["30385-01.htm", "30385-04.html", "30385-05.html", "30385-07.html"])
            .page("30385-06.html", &["30385-07.html", "30385-08.html"])
            .page("30385-08.html", &["30385-09.html", "30385-10.html"])
            .plain(&[
                "30385-09.html",
                "30385-10.html",
                "30385-11.html",
                "30385-12.html",
                "30385-13.html",
            ])
            .page("30037-01.html", &["30037-02.html"])
            .plain(&["30037-02.html", "30037-03.html", "30037-04.html", "30037-05.html"])
            .page("30375-01.html", &["30375-02.html"])
            .plain(&[
                "30375-02.html",
                "30375-03.html",
                "30375-04.html",
                "30375-05.html",
                "30375-06.html",
            ])
            .page("30989-01.html", &["30989-02.html"])
            .plain(&["30989-02.html", "30989-03.html"])
            .npcs(&[
                GUARD_WEISZ,
                HIGH_PRIESTESS_LEVIAN,
                PRIEST_ADONIUS,
                CHEST_OF_BIFRONS,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let weisz = t.spawn(GUARD_WEISZ, 20);
        let levian = t.spawn(HIGH_PRIESTESS_LEVIAN, 40);
        let adonius = t.spawn(PRIEST_ADONIUS, 60);
        (t, weisz, levian, adonius)
    }

    /// Kills the Felim lizardmen until the player has 30 trade cargo.
    fn collect_cargo(t: &mut ScriptTest) {
        let ids = [FELIM_LIZARDMAN, FELIM_LIZARDMAN_SCOUT, FELIM_LIZARDMAN_WARRIOR];
        for kill in 0..400 {
            if t.count(1, TRADE_CARGO) >= 30 {
                return;
            }
            let lizardman = t.spawn(ids[kill % ids.len()], 60);
            t.kill(1, lizardman);
        }
        panic!("no 30 cargo after 400 kills");
    }

    /// Kills the Langk lizardmen until the player has the holy symbol and the rosary.
    fn collect_agness_items(t: &mut ScriptTest) {
        let ids = [LANGK_LIZARDMAN_WARRIOR, LANGK_LIZARDMAN_SCOUT, LANGK_LIZARDMAN];
        for kill in 0..800 {
            if t.count(1, AGNESS_HOLY_SYMBOL) >= 1 && t.count(1, AGNESS_ROSARY) >= 1 {
                return;
            }
            let lizardman = t.spawn(ids[kill % ids.len()], 60);
            t.kill(1, lizardman);
        }
        panic!("no holy symbol and rosary after 800 kills");
    }

    fn accept(t: &mut ScriptTest, weisz: Oid) {
        assert!(t.quest_page(1, weisz).contains("30385-02.htm"));
        assert!(t.event(1, NAME, "30385-03.htm").contains("30385-03.htm"));
        assert!(t.event(1, NAME, "30385-04.html").contains("30385-04.html"));
    }

    #[test]
    fn a_character_below_level_seventeen_is_sent_away() {
        let (mut t, weisz, _levian, _adonius) = setup();
        t.enter(1, 16, vec![]);
        assert!(t.quest_page(1, weisz).contains("30385-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn weisz_starts_the_quest_at_level_seventeen_with_memo_state_one() {
        let (mut t, weisz, _levian, _adonius) = setup();
        t.enter(1, 17, vec![]);
        assert!(t.quest_page(1, weisz).contains("30385-02.htm"));
        assert!(t.event(1, NAME, "30385-03.htm").contains("30385-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.quest_page(1, weisz).contains("30385-05.html"));
    }

    #[test]
    fn the_cargo_sells_for_4090_and_the_quest_goes_on() {
        let (mut t, weisz, _levian, _adonius) = setup();
        t.enter(1, 17, vec![item(100, TRADE_CARGO, 30)]);
        accept(&mut t, weisz);
        t.take_writes();

        assert!(t.quest_page(1, weisz).contains("30385-06.html"));
        assert!(t.event(1, NAME, "30385-08.html").contains("30385-08.html"));
        assert!(t.event(1, NAME, "30385-09.html").contains("30385-09.html"));
        assert_eq!(t.count(1, 57), 4090);
        assert_eq!(t.count(1, TRADE_CARGO), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "1")]);
        assert!(t.quest_page(1, weisz).contains("30385-05.html"));
    }

    #[test]
    fn the_last_sale_ends_the_quest_one_time() {
        let (mut t, weisz, _levian, _adonius) = setup();
        t.enter(1, 17, vec![item(100, TRADE_CARGO, 30)]);
        accept(&mut t, weisz);
        t.take_writes();

        assert!(t.quest_page(1, weisz).contains("30385-06.html"));
        assert!(t.event(1, NAME, "30385-08.html").contains("30385-08.html"));
        assert!(t.event(1, NAME, "30385-10.html").contains("30385-10.html"));
        assert_eq!(t.count(1, 57), 4090);
        // `exitQuest(false, true)`: the state is kept as Completed.
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, weisz)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_whole_quest_from_the_cargo_to_the_adena() {
        let (mut t, weisz, levian, adonius) = setup();
        t.enter(1, 17, vec![]);
        accept(&mut t, weisz);
        t.take_writes();

        // 30 cargo from the Felim lizardmen; Weisz pays the cargo to the Priestess later.
        collect_cargo(&mut t);
        assert_eq!(t.count(1, TRADE_CARGO), 30);
        assert!(t.quest_page(1, weisz).contains("30385-06.html"));
        assert!(t.event(1, NAME, "30385-07.html").contains("30385-07.html"));
        assert_eq!(t.count(1, TRADE_CARGO), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2"),
            ]
        );
        assert!(t.quest_page(1, weisz).contains("30385-11.html"));

        // Adonius: the step to memo 3 and cond 3.
        assert!(t.quest_page(1, adonius).contains("30375-01.html"));
        assert!(t.event(1, NAME, "30375-02.html").contains("30375-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3"),
            ]
        );

        // The holy symbol and the rosary from the Langk lizardmen; Adonius takes them: memo 4.
        collect_agness_items(&mut t);
        assert!(t.quest_page(1, adonius).contains("30375-04.html"));
        assert_eq!(
            (t.count(1, AGNESS_HOLY_SYMBOL), t.count(1, AGNESS_ROSARY)),
            (0, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4"),
            ]
        );
        assert!(t.quest_page(1, adonius).contains("30375-05.html"));

        // Levian: memo 5 and cond 5.
        assert!(t.quest_page(1, levian).contains("30037-01.html"));
        assert!(t.event(1, NAME, "30037-02.html").contains("30037-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5"),
            ]
        );

        // The Serpent Demon Bifrons dies: its chest is made where it died (kept 30 s, so the
        // chest is talked to right away). The chest gives the totem: memo 6 and cond 6.
        let bifrons = t.spawn(SERPENT_DEMON_BIFRONS, 60);
        t.kill(1, bifrons);
        let chest = *t.npcs_of(CHEST_OF_BIFRONS).first().unwrap();
        assert!(t.quest_page(1, chest).contains("30989-01.html"));
        assert!(t.event(1, NAME, "30989-02.html").contains("30989-02.html"));
        assert_eq!(t.count(1, SINISTER_TOTEM), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "6"),
                rows::update(NAME, "cond", "6"),
            ]
        );

        // Levian takes the totem: memo 7 and cond 7.
        assert!(t.quest_page(1, levian).contains("30037-04.html"));
        assert_eq!(t.count(1, SINISTER_TOTEM), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "7"),
                rows::update(NAME, "cond", "7"),
            ]
        );
        assert!(t.quest_page(1, levian).contains("30037-05.html"));

        // Weisz: 14700 adena and the end of the quest.
        t.take();
        assert!(t.quest_page(1, weisz).contains("30385-13.html"));
        assert_eq!(t.count(1, 57), 14700);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, weisz)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_kill_out_of_range_gives_no_cargo() {
        let (mut t, weisz, _levian, _adonius) = setup();
        t.enter(1, 17, vec![]);
        accept(&mut t, weisz);
        let far = t
            .world
            .spawn_npc_at(
                FELIM_LIZARDMAN,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, TRADE_CARGO), 0);
    }
}
