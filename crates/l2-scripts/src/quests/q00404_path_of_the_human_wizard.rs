//! Quest 404: Path of the Human Wizard (Parina and the five elemental guides).
// Java: dist/game/data/scripts/quests/Q00404_PathOfTheHumanWizard/Q00404_PathOfTheHumanWizard.java
//
// Pattern: a delivery chain through five elemental NPCs, each handing out one item and taking
// the previous one, with the kill steps in between: a ratman warrior's key, a water seer's two
// pebbles, a red bear's red soil (`docs/design/scripts.md`, "Quest patterns"). Parina gives the
// bead of the season at the end.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PARINA: i32 = 30391;
const EARTH_SNAKE: i32 = 30409;
const WASTELAND_LIZARDMAN: i32 = 30410;
const FLAME_SALAMANDER: i32 = 30411;
const WIND_SYLPH: i32 = 30412;
const WATER_UNDINE: i32 = 30413;

// Items
const MAP_OF_LUSTER: i32 = 1280;
const KEY_OF_FLAME: i32 = 1281;
const FLAME_EARING: i32 = 1282;
const BROKEN_BRONZE_MIRROR: i32 = 1283;
const WIND_FEATHER: i32 = 1284;
const WIND_BANGLE: i32 = 1285;
const RAMAS_DIARY: i32 = 1286;
const SPARKLE_PEBBLE: i32 = 1287;
const WATER_NECKLACE: i32 = 1288;
const RUSTY_COIN: i32 = 1289;
const RED_SOIL: i32 = 1290;
const EARTH_RING: i32 = 1291;

// Reward
const BEAD_OF_SEASON: i32 = 1292;

// Monsters
const RED_BEAR: i32 = 20021;
const RATMAN_WARRIOR: i32 = 20359;

// Quest monster
const WATER_SEER: i32 = 27030;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00404_PathOfTheHumanWizard`.
pub struct Q00404PathOfTheHumanWizard;

impl Script for Q00404PathOfTheHumanWizard {
    fn name(&self) -> &'static str {
        "Q00404_PathOfTheHumanWizard"
    }

    fn path(&self) -> &'static str {
        "quests/Q00404_PathOfTheHumanWizard"
    }

    fn id(&self) -> i32 {
        404
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PARINA]);
        s.add_talk_id(&[
            PARINA,
            EARTH_SNAKE,
            WASTELAND_LIZARDMAN,
            FLAME_SALAMANDER,
            WIND_SYLPH,
            WATER_UNDINE,
        ]);
        s.add_kill_id(&[RED_BEAR, RATMAN_WARRIOR, WATER_SEER]);
        s.register_quest_items(&[
            MAP_OF_LUSTER,
            KEY_OF_FLAME,
            FLAME_EARING,
            BROKEN_BRONZE_MIRROR,
            WIND_FEATHER,
            WIND_BANGLE,
            RAMAS_DIARY,
            SPARKLE_PEBBLE,
            WATER_NECKLACE,
            RUSTY_COIN,
            RED_SOIL,
            EARTH_RING,
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

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                htmltext = Some(
                    if ctx.player_class_id(player) == player_class::MAGE {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, BEAD_OF_SEASON) {
                                "30391-03.htm"
                            } else {
                                qs.start_quest(ctx);
                                "30391-07.htm"
                            }
                        } else {
                            "30391-02.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::WIZARD {
                        "30391-02a.htm"
                    } else {
                        "30391-01.htm"
                    }
                    .to_owned(),
                );
            }
            "30410-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30410-03.html" => {
                ctx.give_items(player, WIND_FEATHER, 1);
                qs.set_cond(ctx, 6, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        match npc.id {
            RED_BEAR => {
                if ctx.has_quest_items(killer, RUSTY_COIN)
                    && !ctx.has_quest_items(killer, RED_SOIL)
                    && ctx.get_random(100) < 20
                {
                    ctx.give_items(killer, RED_SOIL, 1);
                    qs.set_cond(ctx, 12, true);
                }
            }
            RATMAN_WARRIOR => {
                if ctx.has_quest_items(killer, MAP_OF_LUSTER)
                    && !ctx.has_quest_items(killer, KEY_OF_FLAME)
                    && ctx.get_random(100) < 80
                {
                    ctx.give_items(killer, KEY_OF_FLAME, 1);
                    qs.set_cond(ctx, 3, true);
                }
            }
            WATER_SEER => {
                if ctx.has_quest_items(killer, RAMAS_DIARY)
                    && ctx.get_quest_items_count(killer, SPARKLE_PEBBLE) < 2
                    && ctx.get_random(100) < 80
                {
                    ctx.give_items(killer, SPARKLE_PEBBLE, 1);
                    if ctx.get_quest_items_count(killer, SPARKLE_PEBBLE) == 2 {
                        qs.set_cond(ctx, 9, true);
                    } else {
                        ctx.play_sound(qs.player(), QuestSound::ItemsoundQuestItemget);
                    }
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
            if npc.id == PARINA {
                htmltext = "30391-04.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                PARINA => {
                    if !ctx
                        .has_all_quest_items(player, &[FLAME_EARING, WIND_BANGLE, WATER_NECKLACE, EARTH_RING])
                    {
                        htmltext = "30391-05.html".to_owned();
                    } else {
                        ctx.give_adena(player, 163_800, true);
                        ctx.take_items(player, FLAME_EARING, 1);
                        ctx.take_items(player, WIND_BANGLE, 1);
                        ctx.take_items(player, WATER_NECKLACE, 1);
                        ctx.take_items(player, EARTH_RING, 1);
                        if !ctx.has_quest_items(player, BEAD_OF_SEASON) {
                            ctx.give_items(player, BEAD_OF_SEASON, 1);
                        }
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 23_152);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 29_850);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 36_548);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30391-06.html".to_owned();
                    }
                }
                EARTH_SNAKE => {
                    if ctx.has_quest_items(player, WATER_NECKLACE)
                        && !ctx.has_at_least_one_quest_item(player, &[RUSTY_COIN, EARTH_RING])
                    {
                        if !ctx.has_quest_items(player, RUSTY_COIN) {
                            ctx.give_items(player, RUSTY_COIN, 1);
                        }
                        qs.set_cond(ctx, 11, true);
                        htmltext = "30409-01.html".to_owned();
                    } else if ctx.has_quest_items(player, RUSTY_COIN) {
                        if !ctx.has_quest_items(player, RED_SOIL) {
                            htmltext = "30409-02.html".to_owned();
                        } else {
                            ctx.take_items(player, RUSTY_COIN, 1);
                            ctx.take_items(player, RED_SOIL, 1);
                            if !ctx.has_quest_items(player, EARTH_RING) {
                                ctx.give_items(player, EARTH_RING, 1);
                            }
                            qs.set_cond(ctx, 13, true);
                            htmltext = "30409-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, EARTH_RING) {
                        htmltext = "30409-04.html".to_owned();
                    }
                }
                WASTELAND_LIZARDMAN => {
                    if ctx.has_quest_items(player, BROKEN_BRONZE_MIRROR) {
                        if !ctx.has_quest_items(player, WIND_FEATHER) {
                            htmltext = "30410-01.html".to_owned();
                        } else {
                            htmltext = "30410-04.html".to_owned();
                        }
                    }
                }
                FLAME_SALAMANDER => {
                    if !ctx.has_at_least_one_quest_item(player, &[MAP_OF_LUSTER, FLAME_EARING]) {
                        if !ctx.has_quest_items(player, MAP_OF_LUSTER) {
                            ctx.give_items(player, MAP_OF_LUSTER, 1);
                        }
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30411-01.html".to_owned();
                    } else if ctx.has_quest_items(player, MAP_OF_LUSTER) {
                        if !ctx.has_quest_items(player, KEY_OF_FLAME) {
                            htmltext = "30411-02.html".to_owned();
                        } else {
                            ctx.take_items(player, MAP_OF_LUSTER, 1);
                            ctx.take_items(player, KEY_OF_FLAME, 1);
                            if !ctx.has_quest_items(player, FLAME_EARING) {
                                ctx.give_items(player, FLAME_EARING, 1);
                            }
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30411-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, FLAME_EARING) {
                        htmltext = "30411-04.html".to_owned();
                    }
                }
                WIND_SYLPH => {
                    if ctx.has_quest_items(player, FLAME_EARING)
                        && !ctx.has_at_least_one_quest_item(player, &[BROKEN_BRONZE_MIRROR, WIND_BANGLE])
                    {
                        if !ctx.has_quest_items(player, BROKEN_BRONZE_MIRROR) {
                            ctx.give_items(player, BROKEN_BRONZE_MIRROR, 1);
                        }
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30412-01.html".to_owned();
                    } else if ctx.has_quest_items(player, BROKEN_BRONZE_MIRROR) {
                        if !ctx.has_quest_items(player, WIND_FEATHER) {
                            htmltext = "30412-02.html".to_owned();
                        } else {
                            ctx.take_items(player, BROKEN_BRONZE_MIRROR, 1);
                            ctx.take_items(player, WIND_FEATHER, 1);
                            if !ctx.has_quest_items(player, WIND_BANGLE) {
                                ctx.give_items(player, WIND_BANGLE, 1);
                            }
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30412-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, WIND_BANGLE) {
                        htmltext = "30412-04.html".to_owned();
                    }
                }
                WATER_UNDINE => {
                    if ctx.has_quest_items(player, WIND_BANGLE)
                        && !ctx.has_at_least_one_quest_item(player, &[RAMAS_DIARY, WATER_NECKLACE])
                    {
                        if !ctx.has_quest_items(player, RAMAS_DIARY) {
                            ctx.give_items(player, RAMAS_DIARY, 1);
                        }
                        qs.set_cond(ctx, 8, true);
                        htmltext = "30413-01.html".to_owned();
                    } else if ctx.has_quest_items(player, RAMAS_DIARY) {
                        if ctx.get_quest_items_count(player, SPARKLE_PEBBLE) < 2 {
                            htmltext = "30413-02.html".to_owned();
                        } else {
                            ctx.take_items(player, RAMAS_DIARY, 1);
                            ctx.take_items(player, SPARKLE_PEBBLE, -1);
                            if !ctx.has_quest_items(player, WATER_NECKLACE) {
                                ctx.give_items(player, WATER_NECKLACE, 1);
                            }
                            qs.set_cond(ctx, 10, true);
                            htmltext = "30413-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, WATER_NECKLACE) {
                        htmltext = "30413-04.html".to_owned();
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

    const NAME: &str = "Q00404_PathOfTheHumanWizard";

    const NPCS: &str = r#"
        <npc id="30391" type="Folk" level="70" name="Parina"/>
        <npc id="30409" type="Folk" level="70" name="Earth Snake"/>
        <npc id="30410" type="Folk" level="70" name="Wasteland Lizardman"/>
        <npc id="30411" type="Folk" level="70" name="Flame Salamander"/>
        <npc id="30412" type="Folk" level="70" name="Wind Sylph"/>
        <npc id="30413" type="Folk" level="70" name="Water Undine"/>
        <npc id="20021" type="Monster" level="20" name="Red Bear"/>
        <npc id="20359" type="Monster" level="20" name="Ratman Warrior"/>
        <npc id="27030" type="Monster" level="30" name="Water Seer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1280" type="EtcItem" name="Map of Luster"><set name="is_questitem" val="true"/></item>
        <item id="1281" type="EtcItem" name="Key of Flame"><set name="is_questitem" val="true"/></item>
        <item id="1282" type="EtcItem" name="Flame Earing"><set name="is_questitem" val="true"/></item>
        <item id="1283" type="EtcItem" name="Broken Bronze Mirror"><set name="is_questitem" val="true"/></item>
        <item id="1284" type="EtcItem" name="Wind Feather"><set name="is_questitem" val="true"/></item>
        <item id="1285" type="EtcItem" name="Wind Bangle"><set name="is_questitem" val="true"/></item>
        <item id="1286" type="EtcItem" name="Rama's Diary"><set name="is_questitem" val="true"/></item>
        <item id="1287" type="EtcItem" name="Sparkle Pebble"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1288" type="EtcItem" name="Water Necklace"><set name="is_questitem" val="true"/></item>
        <item id="1289" type="EtcItem" name="Rusty Coin"><set name="is_questitem" val="true"/></item>
        <item id="1290" type="EtcItem" name="Red Soil"><set name="is_questitem" val="true"/></item>
        <item id="1291" type="EtcItem" name="Earth Ring"><set name="is_questitem" val="true"/></item>
        <item id="1292" type="EtcItem" name="Bead of Season"/>"#;

    struct Npcs {
        parina: Oid,
        snake: Oid,
        lizardman: Oid,
        salamander: Oid,
        sylph: Oid,
        undine: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00404PathOfTheHumanWizard)
            .page("30391-04.htm", &["ACCEPT"])
            .page("30410-01.html", &["30410-02.html"])
            .page("30410-02.html", &["30410-03.html"])
            .plain(&[
                "30391-01.htm",
                "30391-02.htm",
                "30391-02a.htm",
                "30391-03.htm",
                "30391-05.html",
                "30391-06.html",
                "30391-07.htm",
                "30409-01.html",
                "30409-02.html",
                "30409-03.html",
                "30409-04.html",
                "30410-03.html",
                "30410-04.html",
                "30411-01.html",
                "30411-02.html",
                "30411-03.html",
                "30411-04.html",
                "30412-01.html",
                "30412-02.html",
                "30412-03.html",
                "30412-04.html",
                "30413-01.html",
                "30413-02.html",
                "30413-03.html",
                "30413-04.html",
            ])
            .npcs(&[
                PARINA,
                EARTH_SNAKE,
                WASTELAND_LIZARDMAN,
                FLAME_SALAMANDER,
                WIND_SYLPH,
                WATER_UNDINE,
            ])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00404PathOfTheHumanWizard);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            parina: t.spawn(PARINA, 20),
            snake: t.spawn(EARTH_SNAKE, 40),
            lizardman: t.spawn(WASTELAND_LIZARDMAN, 60),
            salamander: t.spawn(FLAME_SALAMANDER, 80),
            sylph: t.spawn(WIND_SYLPH, 100),
            undine: t.spawn(WATER_UNDINE, 120),
        };
        (t, npcs)
    }

    /// Kills `npc_id` until the player has `count` of `item_id`; returns the number of kills.
    fn hunt(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) -> usize {
        for kills in 1..2000 {
            let mob = t.spawn(npc_id, 80);
            t.kill(1, mob);
            if t.count(1, item_id) >= count {
                return kills;
            }
        }
        panic!("no {count} of {item_id} after 2000 kills");
    }

    #[test]
    fn the_accept_page_depends_on_class_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::MAGE, vec![]);
        assert!(t.quest_page(1, npcs.parina).contains("30391-04.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30391-02.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::WIZARD, vec![]);
        t.quest_page(1, npcs.parina);
        assert!(t.event(1, NAME, "ACCEPT").contains("30391-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, npcs.parina);
        assert!(t.event(1, NAME, "ACCEPT").contains("30391-01.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::MAGE, vec![]);
        t.quest_page(1, npcs.parina);
        assert!(t.event(1, NAME, "ACCEPT").contains("30391-07.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_whole_path_from_the_map_to_the_bead() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::MAGE, vec![]);
        t.quest_page(1, npcs.parina);
        assert!(t.event(1, NAME, "ACCEPT").contains("30391-07.htm"));
        t.take_writes();

        // The flame salamander gives the map: cond 2.
        assert!(t.quest_page(1, npcs.salamander).contains("30411-01.html"));
        assert_eq!(t.count(1, MAP_OF_LUSTER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // A ratman warrior drops the key of flame: cond 3.
        hunt(&mut t, RATMAN_WARRIOR, KEY_OF_FLAME, 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // The salamander takes the map and the key and gives the flame earing: cond 4.
        assert!(t.quest_page(1, npcs.salamander).contains("30411-03.html"));
        assert_eq!(
            (
                t.count(1, MAP_OF_LUSTER),
                t.count(1, KEY_OF_FLAME),
                t.count(1, FLAME_EARING)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // The sylph gives the broken bronze mirror: cond 5.
        assert!(t.quest_page(1, npcs.sylph).contains("30412-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // The lizardman gives the wind feather with his page 02 and 03: cond 6.
        assert!(t.quest_page(1, npcs.lizardman).contains("30410-01.html"));
        assert!(t.event(1, NAME, "30410-02.html").contains("30410-02.html"));
        assert!(t.event(1, NAME, "30410-03.html").contains("30410-03.html"));
        assert_eq!(t.count(1, WIND_FEATHER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // The sylph takes the mirror and the feather and gives the wind bangle: cond 7.
        assert!(t.quest_page(1, npcs.sylph).contains("30412-03.html"));
        assert_eq!(
            (
                t.count(1, BROKEN_BRONZE_MIRROR),
                t.count(1, WIND_FEATHER),
                t.count(1, WIND_BANGLE)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // The undine gives Rama's diary: cond 8.
        assert!(t.quest_page(1, npcs.undine).contains("30413-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);

        // Two sparkle pebbles from the water seer; the second sets cond 9.
        hunt(&mut t, WATER_SEER, SPARKLE_PEBBLE, 2);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "9")]);

        // The undine takes the diary and the pebbles and gives the water necklace: cond 10.
        assert!(t.quest_page(1, npcs.undine).contains("30413-03.html"));
        assert_eq!(
            (
                t.count(1, RAMAS_DIARY),
                t.count(1, SPARKLE_PEBBLE),
                t.count(1, WATER_NECKLACE)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "10")]);

        // The earth snake gives the rusty coin: cond 11.
        assert!(t.quest_page(1, npcs.snake).contains("30409-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "11")]);
        assert!(t.quest_page(1, npcs.snake).contains("30409-02.html"));

        // A red bear drops the red soil (a 20% drop): cond 12.
        hunt(&mut t, RED_BEAR, RED_SOIL, 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "12")]);

        // The snake takes the coin and the soil and gives the earth ring: cond 13.
        assert!(t.quest_page(1, npcs.snake).contains("30409-03.html"));
        assert_eq!(
            (
                t.count(1, RUSTY_COIN),
                t.count(1, RED_SOIL),
                t.count(1, EARTH_RING)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "13")]);

        // Parina's reward: adena, the bead, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.parina).contains("30391-06.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, BEAD_OF_SEASON), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, npcs.parina).contains("30391-04.htm"));
    }

    #[test]
    fn parina_waits_for_all_four_items() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::MAGE, vec![item(5000, 1282, 1)]);
        t.quest_page(1, npcs.parina);
        assert!(t.event(1, NAME, "ACCEPT").contains("30391-07.htm"));
        t.take_writes();
        assert!(t.quest_page(1, npcs.parina).contains("30391-05.html"));
        assert!(t.take_writes().is_empty());
    }
}
