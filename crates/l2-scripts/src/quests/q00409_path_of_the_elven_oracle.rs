//! Quest 409: Path of the Elven Oracle (Priest Manuel, Allana and Perrin).
// Java: dist/game/data/scripts/quests/Q00409_PathOfTheElvenOracle/Q00409_PathOfTheElvenOracle.java
//
// Pattern: the memo state as the step counter, and monsters that are summoned by an event
// ("replay_1", "replay_2"): three lizardmen at Allana, then Tamil at Perrin, spawned at the NPC's
// place with an attack desire to the player. The lizardmen count only for the player who hit
// them first (the NPC's script value, `onAttack`), see `docs/design/scripts.md`, "Quest patterns".

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const PRIEST_MANUEL: i32 = 30293;
const ALLANA: i32 = 30424;
const PERRIN: i32 = 30428;

// Items
const CRYSTAL_MEDALLION: i32 = 1231;
const SWINDLERS_MONEY: i32 = 1232;
const ALLANA_OF_DAIRY: i32 = 1233;
const LIZARD_CAPTAIN_ORDER: i32 = 1234;
const HALF_OF_DAIRY: i32 = 1236;
const TAMIL_NECKLACE: i32 = 1275;

// Reward
const LEAF_OF_ORACLE: i32 = 1235;

// Misc
const MIN_LEVEL: i32 = 18;

// Quest monsters
const LIZARDMAN_WARRIOR: i32 = 27032;
const LIZARDMAN_SCOUT: i32 = 27033;
const LIZARDMAN_SOLDIER: i32 = 27034;
const TAMIL: i32 = 27035;

/// Java `Q00409_PathOfTheElvenOracle`.
pub struct Q00409PathOfTheElvenOracle;

impl Script for Q00409PathOfTheElvenOracle {
    fn name(&self) -> &'static str {
        "Q00409_PathOfTheElvenOracle"
    }

    fn path(&self) -> &'static str {
        "quests/Q00409_PathOfTheElvenOracle"
    }

    fn id(&self) -> i32 {
        409
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PRIEST_MANUEL]);
        s.add_talk_id(&[PRIEST_MANUEL, ALLANA, PERRIN]);
        s.add_kill_id(&[TAMIL, LIZARDMAN_WARRIOR, LIZARDMAN_SCOUT, LIZARDMAN_SOLDIER]);
        s.add_attack_id(&[TAMIL, LIZARDMAN_WARRIOR, LIZARDMAN_SCOUT, LIZARDMAN_SOLDIER]);
        s.register_quest_items(&[
            CRYSTAL_MEDALLION,
            SWINDLERS_MONEY,
            ALLANA_OF_DAIRY,
            LIZARD_CAPTAIN_ORDER,
            HALF_OF_DAIRY,
            TAMIL_NECKLACE,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                htmltext = Some(
                    if ctx.player_class_id(player) == player_class::ELVEN_MAGE {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, LEAF_OF_ORACLE) {
                                "30293-04.htm"
                            } else {
                                qs.start_quest(ctx);
                                qs.set_memo_state(ctx, 1);
                                ctx.give_items(player, CRYSTAL_MEDALLION, 1);
                                "30293-05.htm"
                            }
                        } else {
                            "30293-03.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::ORACLE {
                        "30293-02a.htm"
                    } else {
                        "30293-02.htm"
                    }
                    .to_owned(),
                );
            }
            "30424-08.html" | "30424-09.html" => {
                htmltext = Some(event.to_owned());
            }
            "30424-07.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "replay_1" => {
                qs.set_memo_state(ctx, 2);
                if let Some(npc) = npc {
                    for id in [LIZARDMAN_WARRIOR, LIZARDMAN_SCOUT, LIZARDMAN_SOLDIER] {
                        if let Some(mob) = ctx.add_spawn_at(id, npc.object_id, true, 0) {
                            ctx.add_attack_desire(mob, player, 999);
                        }
                    }
                }
            }
            "30428-02.html" | "30428-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "replay_2" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    if let Some(npc) = npc
                        && let Some(mob) = ctx.add_spawn_at(TAMIL, npc.object_id, true, 0)
                    {
                        ctx.add_attack_desire(mob, player, 999);
                    }
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.get_quest_state(attacker, false).is_none() {
            return;
        }
        match ctx.get_script_value(npc) {
            0 => {
                match npc.id {
                    // The three lizardmen shout the same line when first hit.
                    LIZARDMAN_WARRIOR | LIZARDMAN_SCOUT | LIZARDMAN_SOLDIER => {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::TheSacredFlameIsOurs,
                            &[],
                        );
                    }
                    TAMIL => {
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::AsYouWishMaster, &[]);
                    }
                    _ => {}
                }
                ctx.set_script_value(npc, 1);
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_int("firstAttacker", attacker.0);
            }
            1 => {
                let first = ctx
                    .world()
                    .npc_variables(npc.object_id)
                    .map_or(0, |v| v.get_int("firstAttacker", 0));
                if first != attacker.0 {
                    ctx.set_script_value(npc, 2);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx)
            && ctx.is_script_value(npc, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        match npc.id {
            LIZARDMAN_WARRIOR => {
                if !ctx.has_quest_items(killer, LIZARD_CAPTAIN_ORDER) {
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::ArrghhWeShallNeverSurrender,
                        &[],
                    );
                    ctx.give_items(killer, LIZARD_CAPTAIN_ORDER, 1);
                    qs.set_cond(ctx, 3, true);
                }
            }
            LIZARDMAN_SCOUT | LIZARDMAN_SOLDIER => {
                if !ctx.has_quest_items(killer, LIZARD_CAPTAIN_ORDER) {
                    ctx.give_items(killer, LIZARD_CAPTAIN_ORDER, 1);
                    qs.set_cond(ctx, 3, true);
                }
            }
            TAMIL => {
                if !ctx.has_quest_items(killer, TAMIL_NECKLACE) {
                    ctx.give_items(killer, TAMIL_NECKLACE, 1);
                    qs.set_cond(ctx, 5, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == PRIEST_MANUEL {
                htmltext = if !ctx.has_quest_items(player, LEAF_OF_ORACLE) {
                    "30293-01.htm"
                } else {
                    "30293-04.htm"
                }
                .to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                PRIEST_MANUEL => {
                    if ctx.has_quest_items(player, CRYSTAL_MEDALLION) {
                        if !ctx.has_at_least_one_quest_item(
                            player,
                            &[
                                SWINDLERS_MONEY,
                                ALLANA_OF_DAIRY,
                                LIZARD_CAPTAIN_ORDER,
                                HALF_OF_DAIRY,
                            ],
                        ) {
                            if qs.is_memo_state(ctx, 2) {
                                qs.set_memo_state(ctx, 1);
                                qs.set_cond(ctx, 8, false);
                                htmltext = "30293-09.html".to_owned();
                            } else {
                                qs.set_memo_state(ctx, 1);
                                htmltext = "30293-06.html".to_owned();
                            }
                        } else if ctx.has_all_quest_items(
                            player,
                            &[SWINDLERS_MONEY, ALLANA_OF_DAIRY, LIZARD_CAPTAIN_ORDER],
                        ) {
                            if !ctx.has_quest_items(player, HALF_OF_DAIRY) {
                                ctx.give_adena(player, 163_800, true);
                                ctx.give_items(player, LEAF_OF_ORACLE, 1);
                                let level = ctx.player_level(player);
                                if level >= 20 {
                                    ctx.add_exp_and_sp(player, 320_534, 20_392);
                                } else if level == 19 {
                                    ctx.add_exp_and_sp(player, 456_128, 27_090);
                                } else {
                                    ctx.add_exp_and_sp(player, 591_724, 33_788);
                                }
                                qs.exit_quest(ctx, false, true);
                                ctx.world_mut().send_social_action(player, 3);
                                htmltext = "30293-08.html".to_owned();
                            }
                        } else {
                            htmltext = "30293-07.html".to_owned();
                        }
                    }
                }
                ALLANA => {
                    if ctx.has_quest_items(player, CRYSTAL_MEDALLION) {
                        if !ctx.has_at_least_one_quest_item(
                            player,
                            &[
                                SWINDLERS_MONEY,
                                ALLANA_OF_DAIRY,
                                LIZARD_CAPTAIN_ORDER,
                                HALF_OF_DAIRY,
                            ],
                        ) {
                            if qs.is_memo_state(ctx, 2) {
                                htmltext = "30424-05.html".to_owned();
                            } else if qs.is_memo_state(ctx, 1) {
                                qs.set_cond(ctx, 2, true);
                                htmltext = "30424-01.html".to_owned();
                            }
                        } else if !ctx.has_at_least_one_quest_item(
                            player,
                            &[SWINDLERS_MONEY, ALLANA_OF_DAIRY, HALF_OF_DAIRY],
                        ) && ctx.has_quest_items(player, LIZARD_CAPTAIN_ORDER)
                        {
                            qs.set_memo_state(ctx, 2);
                            ctx.give_items(player, HALF_OF_DAIRY, 1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30424-02.html".to_owned();
                        } else if !ctx
                            .has_at_least_one_quest_item(player, &[SWINDLERS_MONEY, ALLANA_OF_DAIRY])
                            && ctx.has_all_quest_items(player, &[LIZARD_CAPTAIN_ORDER, HALF_OF_DAIRY])
                        {
                            if qs.is_memo_state(ctx, 3) && !ctx.has_quest_items(player, TAMIL_NECKLACE) {
                                qs.set_memo_state(ctx, 2);
                                qs.set_cond(ctx, 4, true);
                                htmltext = "30424-06.html".to_owned();
                            } else {
                                htmltext = "30424-03.html".to_owned();
                            }
                        } else if ctx.has_all_quest_items(
                            player,
                            &[SWINDLERS_MONEY, LIZARD_CAPTAIN_ORDER, HALF_OF_DAIRY],
                        ) && !ctx.has_quest_items(player, ALLANA_OF_DAIRY)
                        {
                            ctx.give_items(player, ALLANA_OF_DAIRY, 1);
                            ctx.take_items(player, HALF_OF_DAIRY, 1);
                            qs.set_cond(ctx, 9, true);
                            htmltext = "30424-04.html".to_owned();
                        } else if ctx.has_all_quest_items(
                            player,
                            &[SWINDLERS_MONEY, LIZARD_CAPTAIN_ORDER, ALLANA_OF_DAIRY],
                        ) {
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30424-05.html".to_owned();
                        }
                    }
                }
                PERRIN => {
                    if ctx.has_all_quest_items(
                        player,
                        &[CRYSTAL_MEDALLION, LIZARD_CAPTAIN_ORDER, HALF_OF_DAIRY],
                    ) {
                        if ctx.has_quest_items(player, TAMIL_NECKLACE) {
                            ctx.give_items(player, SWINDLERS_MONEY, 1);
                            ctx.take_items(player, TAMIL_NECKLACE, 1);
                            qs.set_cond(ctx, 6, true);
                            htmltext = "30428-04.html".to_owned();
                        } else if ctx.has_quest_items(player, SWINDLERS_MONEY) {
                            htmltext = "30428-05.html".to_owned();
                        } else if qs.is_memo_state(ctx, 3) {
                            htmltext = "30428-06.html".to_owned();
                        } else {
                            htmltext = "30428-01.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00409_PathOfTheElvenOracle";

    const NPCS: &str = r#"
        <npc id="30293" type="Folk" level="70" name="Priest Manuel"/>
        <npc id="30424" type="Folk" level="70" name="Allana"/>
        <npc id="30428" type="Folk" level="70" name="Perrin"/>
        <npc id="27032" type="Monster" level="20" name="Lizardman Warrior"/>
        <npc id="27033" type="Monster" level="20" name="Lizardman Scout"/>
        <npc id="27034" type="Monster" level="20" name="Lizardman Soldier"/>
        <npc id="27035" type="Monster" level="30" name="Tamil"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1231" type="EtcItem" name="Crystal Medallion"><set name="is_questitem" val="true"/></item>
        <item id="1232" type="EtcItem" name="Swindler's Money"><set name="is_questitem" val="true"/></item>
        <item id="1233" type="EtcItem" name="Allana's Diary"><set name="is_questitem" val="true"/></item>
        <item id="1234" type="EtcItem" name="Lizard Captain's Order"><set name="is_questitem" val="true"/></item>
        <item id="1235" type="EtcItem" name="Leaf of Oracle"/>
        <item id="1236" type="EtcItem" name="Half of Diary"><set name="is_questitem" val="true"/></item>
        <item id="1275" type="EtcItem" name="Tamil's Necklace"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        manuel: Oid,
        allana: Oid,
        perrin: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00409PathOfTheElvenOracle)
            .page("30293-01.htm", &["ACCEPT"])
            .page("30424-01.html", &["30424-07.html", "30424-08.html"])
            .page("30424-07.html", &["30424-09.html", "30424-08.html"])
            .page("30424-08.html", &["replay_1"])
            .page("30428-01.html", &["30428-02.html", "30428-03.html"])
            .page("30428-02.html", &["replay_2"])
            .plain(&[
                "30293-02.htm",
                "30293-02a.htm",
                "30293-03.htm",
                "30293-04.htm",
                "30293-05.htm",
                "30293-06.html",
                "30293-07.html",
                "30293-08.html",
                "30293-09.html",
                "30424-02.html",
                "30424-03.html",
                "30424-04.html",
                "30424-05.html",
                "30424-06.html",
                "30424-09.html",
                "30428-03.html",
                "30428-04.html",
                "30428-05.html",
                "30428-06.html",
            ])
            .npcs(&[PRIEST_MANUEL, ALLANA, PERRIN])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00409PathOfTheElvenOracle);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            manuel: t.spawn(PRIEST_MANUEL, 20),
            allana: t.spawn(ALLANA, 40),
            perrin: t.spawn(PERRIN, 60),
        };
        (t, npcs)
    }

    /// Hits a quest monster that is already in the world as the player, then kills it. The
    /// hooks are called directly: `t.kill` waits 2.5 s, and the other spawns of the event would
    /// attack the test character (no HP in the test world) in that time.
    fn hit_and_kill(t: &mut ScriptTest, mob: Oid) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(mob) {
                Q00409PathOfTheElvenOracle.on_attack(ctx, npc, Oid(1), 1, false, None);
                Q00409PathOfTheElvenOracle.on_kill(ctx, npc, Oid(1), false);
            }
        });
        t.world.delete_npc(mob);
    }

    #[test]
    fn the_accept_page_depends_on_class_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::ELF, player_class::ELVEN_MAGE, vec![]);
        assert!(t.quest_page(1, npcs.manuel).contains("30293-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30293-03.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ELF, player_class::ORACLE, vec![]);
        t.quest_page(1, npcs.manuel);
        assert!(t.event(1, NAME, "ACCEPT").contains("30293-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        t.quest_page(1, npcs.manuel);
        assert!(t.event(1, NAME, "ACCEPT").contains("30293-02.htm"));

        // With the leaf the talk already shows the page (no link to ACCEPT on it).
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::ELF,
            player_class::ELVEN_MAGE,
            vec![item(5000, 1235, 1)],
        );
        assert!(t.quest_page(1, npcs.manuel).contains("30293-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_medallion_to_the_leaf() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_MAGE, vec![]);
        assert!(t.quest_page(1, npcs.manuel).contains("30293-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30293-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Allana: cond 2 while the memo state is 1; Manuel sends the player back (memo 1 again).
        assert!(t.quest_page(1, npcs.allana).contains("30424-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.manuel).contains("30293-06.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "1")]);

        // Allana's page 08 starts the lizardmen (replay_1): memo state 2, three spawns.
        assert!(t.quest_page(1, npcs.allana).contains("30424-01.html"));
        assert!(t.event(1, NAME, "30424-08.html").contains("30424-08.html"));
        t.event(1, NAME, "replay_1");
        // The NPC's page asks again: cond 2 is written again (Java's setCond always writes).
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "2"),
                rows::update(NAME, "memoState", "2")
            ]
        );
        assert_eq!(t.npcs_of(LIZARDMAN_WARRIOR).len(), 1);
        assert_eq!(t.npcs_of(LIZARDMAN_SCOUT).len(), 1);
        assert_eq!(t.npcs_of(LIZARDMAN_SOLDIER).len(), 1);

        // The three lizardmen, hit first by the player: the warrior gives the order (cond 3).
        for id in [LIZARDMAN_WARRIOR, LIZARDMAN_SCOUT, LIZARDMAN_SOLDIER] {
            let mob = t.npcs_of(id)[0];
            hit_and_kill(&mut t, mob);
        }
        assert_eq!(t.count(1, LIZARD_CAPTAIN_ORDER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Allana gives the half of the diary: memo state 2 and cond 4.
        assert!(t.quest_page(1, npcs.allana).contains("30424-02.html"));
        assert_eq!(
            (t.count(1, LIZARD_CAPTAIN_ORDER), t.count(1, HALF_OF_DAIRY)),
            (1, 1)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "4")
            ]
        );

        // Perrin sends the player to the Tamil (replay_2): memo state 3, one spawn.
        assert!(t.quest_page(1, npcs.perrin).contains("30428-01.html"));
        assert!(t.event(1, NAME, "30428-02.html").contains("30428-02.html"));
        t.event(1, NAME, "replay_2");
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "3")]);
        let tamil = t.npcs_of(TAMIL)[0];
        hit_and_kill(&mut t, tamil);
        assert_eq!(t.count(1, TAMIL_NECKLACE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Allana needs the necklace gone first; Perrin takes it and gives the money: cond 6.
        assert!(t.quest_page(1, npcs.allana).contains("30424-03.html"));
        assert!(t.quest_page(1, npcs.perrin).contains("30428-04.html"));
        assert_eq!((t.count(1, TAMIL_NECKLACE), t.count(1, SWINDLERS_MONEY)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Allana takes the half and gives her diary: cond 9.
        assert!(t.quest_page(1, npcs.allana).contains("30424-04.html"));
        assert_eq!((t.count(1, HALF_OF_DAIRY), t.count(1, ALLANA_OF_DAIRY)), (0, 1));
        // A jump from cond 6 to 9 skips 7 and 8: the completed-state flags (0x8000013F).
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "9"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483329"),
            ]
        );

        // Allana with all three: cond 7.
        assert!(t.quest_page(1, npcs.allana).contains("30424-05.html"));
        // Back from 9 to 7 keeps the skipped steps: the flags become 0x8000003F.
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "7"),
                rows::update(NAME, "__compltdStateFlags", "-2147483585"),
            ]
        );

        // Manuel's reward: adena, the leaf, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.manuel).contains("30293-08.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, LEAF_OF_ORACLE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
