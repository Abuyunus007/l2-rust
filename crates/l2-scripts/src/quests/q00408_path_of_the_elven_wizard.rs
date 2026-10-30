//! Quest 408: Path of the Elven Wizard (Rossela, Greenis, Thalia and Northwind).
// Java: dist/game/data/scripts/quests/Q00408_PathOfTheElvenWizard/Q00408_PathOfTheElvenWizard.java
//
// Pattern: four guild NPCs, each trading one item for the next one (a letter, a charm, an apple,
// a sap, a potpourri), with a kill step for the material each one needs (red down, gold leaves,
// amethysts). The quest never calls `setCond`: the cond stays 1 until the reward
// (`docs/design/scripts.md`, "Quest patterns").

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ROSSELA: i32 = 30414;
const GREENIS: i32 = 30157;
const THALIA: i32 = 30371;
const NORTHWIND: i32 = 30423;

// Items
const ROSELLAS_LETTER: i32 = 1218;
const RED_DOWN: i32 = 1219;
const MAGICAL_POWERS_RUBY: i32 = 1220;
const PURE_AQUAMARINE: i32 = 1221;
const APPETIZING_APPLE: i32 = 1222;
const GOLD_LEAVES: i32 = 1223;
const IMMORTAL_LOVE: i32 = 1224;
const AMETHYST: i32 = 1225;
const NOBILITY_AMETHYST: i32 = 1226;
const FERTILITY_PERIDOT: i32 = 1229;
const GREENISS_CHARM: i32 = 1272;
const SAP_OF_THE_MOTHER_TREE: i32 = 1273;
const LUCKY_POTPOURRI: i32 = 1274;

// Reward
const ETERNITY_DIAMOND: i32 = 1230;

// Monsters
const DRYAD_ELDER: i32 = 20019;
const SUKAR_WERERAT_LEADER: i32 = 20047;
const PINCER_SPIDER: i32 = 20466;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00408_PathOfTheElvenWizard`.
pub struct Q00408PathOfTheElvenWizard;

impl Script for Q00408PathOfTheElvenWizard {
    fn name(&self) -> &'static str {
        "Q00408_PathOfTheElvenWizard"
    }

    fn path(&self) -> &'static str {
        "quests/Q00408_PathOfTheElvenWizard"
    }

    fn id(&self) -> i32 {
        408
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ROSSELA]);
        s.add_talk_id(&[ROSSELA, GREENIS, THALIA, NORTHWIND]);
        s.add_kill_id(&[DRYAD_ELDER, SUKAR_WERERAT_LEADER, PINCER_SPIDER]);
        s.register_quest_items(&[
            ROSELLAS_LETTER,
            RED_DOWN,
            MAGICAL_POWERS_RUBY,
            PURE_AQUAMARINE,
            APPETIZING_APPLE,
            GOLD_LEAVES,
            IMMORTAL_LOVE,
            AMETHYST,
            NOBILITY_AMETHYST,
            FERTILITY_PERIDOT,
            GREENISS_CHARM,
            SAP_OF_THE_MOTHER_TREE,
            LUCKY_POTPOURRI,
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
                    if ctx.player_class_id(player) != player_class::ELVEN_MAGE {
                        if ctx.player_class_id(player) == player_class::ELVEN_WIZARD {
                            "30414-02a.htm"
                        } else {
                            "30414-03.htm"
                        }
                    } else if ctx.player_level(player) < MIN_LEVEL {
                        "30414-04.htm"
                    } else if ctx.has_quest_items(player, ETERNITY_DIAMOND) {
                        "30414-05.htm"
                    } else {
                        if !ctx.has_quest_items(player, FERTILITY_PERIDOT) {
                            ctx.give_items(player, FERTILITY_PERIDOT, 1);
                        }
                        qs.start_quest(ctx);
                        "30414-06.htm"
                    }
                    .to_owned(),
                );
            }
            "30414-02.htm" => {
                htmltext = Some(event.to_owned());
            }
            "30414-10.html" => {
                if ctx.has_quest_items(player, MAGICAL_POWERS_RUBY) {
                    htmltext = Some(event.to_owned());
                } else if !ctx.has_quest_items(player, MAGICAL_POWERS_RUBY)
                    && ctx.has_quest_items(player, FERTILITY_PERIDOT)
                {
                    if !ctx.has_quest_items(player, ROSELLAS_LETTER) {
                        ctx.give_items(player, ROSELLAS_LETTER, 1);
                    }
                    htmltext = Some("30414-07.html".to_owned());
                }
            }
            "30414-12.html" => {
                if ctx.has_quest_items(player, PURE_AQUAMARINE) {
                    htmltext = Some(event.to_owned());
                } else if !ctx.has_quest_items(player, PURE_AQUAMARINE)
                    && ctx.has_quest_items(player, FERTILITY_PERIDOT)
                {
                    if !ctx.has_quest_items(player, APPETIZING_APPLE) {
                        ctx.give_items(player, APPETIZING_APPLE, 1);
                    }
                    htmltext = Some("30414-13.html".to_owned());
                }
            }
            "30414-16.html" => {
                if ctx.has_quest_items(player, NOBILITY_AMETHYST) {
                    htmltext = Some(event.to_owned());
                } else if !ctx.has_quest_items(player, NOBILITY_AMETHYST)
                    && ctx.has_quest_items(player, FERTILITY_PERIDOT)
                {
                    if !ctx.has_quest_items(player, IMMORTAL_LOVE) {
                        ctx.give_items(player, IMMORTAL_LOVE, 1);
                    }
                    htmltext = Some("30414-17.html".to_owned());
                }
            }
            "30157-02.html" => {
                if ctx.has_quest_items(player, ROSELLAS_LETTER) {
                    ctx.take_items(player, ROSELLAS_LETTER, 1);
                    if !ctx.has_quest_items(player, GREENISS_CHARM) {
                        ctx.give_items(player, GREENISS_CHARM, 1);
                    }
                }
                htmltext = Some(event.to_owned());
            }
            "30371-02.html" => {
                if ctx.has_quest_items(player, APPETIZING_APPLE) {
                    ctx.take_items(player, APPETIZING_APPLE, 1);
                    if !ctx.has_quest_items(player, SAP_OF_THE_MOTHER_TREE) {
                        ctx.give_items(player, SAP_OF_THE_MOTHER_TREE, 1);
                    }
                }
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
        // (the item, its limit and the chance out of 100) of the monster.
        let (item, limit, chance, needed) = match npc.id {
            DRYAD_ELDER => {
                if !ctx.has_quest_items(killer, SAP_OF_THE_MOTHER_TREE) {
                    return;
                }
                (GOLD_LEAVES, 5, 40, 5)
            }
            SUKAR_WERERAT_LEADER => {
                if !ctx.has_quest_items(killer, LUCKY_POTPOURRI) {
                    return;
                }
                (AMETHYST, 2, 40, 2)
            }
            PINCER_SPIDER => {
                if !ctx.has_quest_items(killer, GREENISS_CHARM) {
                    return;
                }
                (RED_DOWN, 5, 70, 5)
            }
            _ => return,
        };
        if ctx.get_quest_items_count(killer, item) < limit && ctx.get_random(100) < chance {
            ctx.give_items(killer, item, 1);
            if ctx.get_quest_items_count(killer, item) == needed {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == ROSSELA {
                htmltext = "30414-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                ROSSELA => {
                    if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ROSELLAS_LETTER,
                            APPETIZING_APPLE,
                            IMMORTAL_LOVE,
                            GREENISS_CHARM,
                            SAP_OF_THE_MOTHER_TREE,
                            LUCKY_POTPOURRI,
                        ],
                    ) && ctx.has_quest_items(player, FERTILITY_PERIDOT)
                        && !ctx.has_all_quest_items(
                            player,
                            &[MAGICAL_POWERS_RUBY, NOBILITY_AMETHYST, PURE_AQUAMARINE],
                        )
                    {
                        htmltext = "30414-11.html".to_owned();
                    } else if ctx.has_quest_items(player, ROSELLAS_LETTER) {
                        htmltext = "30414-08.html".to_owned();
                    } else if ctx.has_quest_items(player, GREENISS_CHARM) {
                        if ctx.get_quest_items_count(player, RED_DOWN) < 5 {
                            htmltext = "30414-09.html".to_owned();
                        } else {
                            htmltext = "30414-21.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, APPETIZING_APPLE) {
                        htmltext = "30414-14.html".to_owned();
                    } else if ctx.has_quest_items(player, SAP_OF_THE_MOTHER_TREE) {
                        if ctx.get_quest_items_count(player, GOLD_LEAVES) < 5 {
                            htmltext = "30414-15.html".to_owned();
                        } else {
                            htmltext = "30414-22.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, IMMORTAL_LOVE) {
                        htmltext = "30414-18.html".to_owned();
                    } else if ctx.has_quest_items(player, LUCKY_POTPOURRI) {
                        if ctx.get_quest_items_count(player, AMETHYST) < 2 {
                            htmltext = "30414-19.html".to_owned();
                        } else {
                            htmltext = "30414-23.html".to_owned();
                        }
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ROSELLAS_LETTER,
                            APPETIZING_APPLE,
                            IMMORTAL_LOVE,
                            GREENISS_CHARM,
                            SAP_OF_THE_MOTHER_TREE,
                            LUCKY_POTPOURRI,
                        ],
                    ) && ctx.has_all_quest_items(
                        player,
                        &[
                            FERTILITY_PERIDOT,
                            MAGICAL_POWERS_RUBY,
                            NOBILITY_AMETHYST,
                            PURE_AQUAMARINE,
                        ],
                    ) {
                        ctx.give_adena(player, 163_800, true);
                        if !ctx.has_quest_items(player, ETERNITY_DIAMOND) {
                            ctx.give_items(player, ETERNITY_DIAMOND, 1);
                        }
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 22_532);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 29_230);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 35_928);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30414-20.html".to_owned();
                    }
                }
                GREENIS => {
                    if ctx.has_quest_items(player, ROSELLAS_LETTER) {
                        htmltext = "30157-01.html".to_owned();
                    } else if ctx.has_quest_items(player, GREENISS_CHARM) {
                        if ctx.get_quest_items_count(player, RED_DOWN) < 5 {
                            htmltext = "30157-03.html".to_owned();
                        } else {
                            ctx.take_items(player, RED_DOWN, -1);
                            if !ctx.has_quest_items(player, MAGICAL_POWERS_RUBY) {
                                ctx.give_items(player, MAGICAL_POWERS_RUBY, 1);
                            }
                            ctx.take_items(player, GREENISS_CHARM, 1);
                            htmltext = "30157-04.html".to_owned();
                        }
                    }
                }
                THALIA => {
                    if ctx.has_quest_items(player, APPETIZING_APPLE) {
                        htmltext = "30371-01.html".to_owned();
                    } else if ctx.has_quest_items(player, SAP_OF_THE_MOTHER_TREE) {
                        if ctx.get_quest_items_count(player, GOLD_LEAVES) < 5 {
                            htmltext = "30371-03.html".to_owned();
                        } else {
                            if !ctx.has_quest_items(player, PURE_AQUAMARINE) {
                                ctx.give_items(player, PURE_AQUAMARINE, 1);
                            }
                            ctx.take_items(player, GOLD_LEAVES, -1);
                            ctx.take_items(player, SAP_OF_THE_MOTHER_TREE, 1);
                            htmltext = "30371-04.html".to_owned();
                        }
                    }
                }
                NORTHWIND => {
                    if ctx.has_quest_items(player, IMMORTAL_LOVE) {
                        ctx.take_items(player, IMMORTAL_LOVE, 1);
                        if !ctx.has_quest_items(player, LUCKY_POTPOURRI) {
                            ctx.give_items(player, LUCKY_POTPOURRI, 1);
                        }
                        htmltext = "30423-01.html".to_owned();
                    } else if ctx.has_quest_items(player, LUCKY_POTPOURRI) {
                        if ctx.get_quest_items_count(player, AMETHYST) < 2 {
                            htmltext = "30423-02.html".to_owned();
                        } else {
                            ctx.take_items(player, AMETHYST, -1);
                            if !ctx.has_quest_items(player, NOBILITY_AMETHYST) {
                                ctx.give_items(player, NOBILITY_AMETHYST, 1);
                            }
                            ctx.take_items(player, LUCKY_POTPOURRI, 1);
                            htmltext = "30423-03.html".to_owned();
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

    const NAME: &str = "Q00408_PathOfTheElvenWizard";

    const NPCS: &str = r#"
        <npc id="30414" type="Folk" level="70" name="Rossela"/>
        <npc id="30157" type="Folk" level="70" name="Greenis"/>
        <npc id="30371" type="Folk" level="70" name="Thalia"/>
        <npc id="30423" type="Folk" level="70" name="Northwind"/>
        <npc id="20019" type="Monster" level="20" name="Dryad Elder"/>
        <npc id="20047" type="Monster" level="20" name="Sukar Wererat Leader"/>
        <npc id="20466" type="Monster" level="20" name="Pincer Spider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1218" type="EtcItem" name="Rossella's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1219" type="EtcItem" name="Red Down"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1220" type="EtcItem" name="Magical Powers Ruby"><set name="is_questitem" val="true"/></item>
        <item id="1221" type="EtcItem" name="Pure Aquamarine"><set name="is_questitem" val="true"/></item>
        <item id="1222" type="EtcItem" name="Appetizing Apple"><set name="is_questitem" val="true"/></item>
        <item id="1223" type="EtcItem" name="Gold Leaves"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1224" type="EtcItem" name="Immortal Love"><set name="is_questitem" val="true"/></item>
        <item id="1225" type="EtcItem" name="Amethyst"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1226" type="EtcItem" name="Nobility Amethyst"><set name="is_questitem" val="true"/></item>
        <item id="1229" type="EtcItem" name="Fertility Peridot"><set name="is_questitem" val="true"/></item>
        <item id="1272" type="EtcItem" name="Greeniss' Charm"><set name="is_questitem" val="true"/></item>
        <item id="1273" type="EtcItem" name="Sap of the Mother Tree"><set name="is_questitem" val="true"/></item>
        <item id="1274" type="EtcItem" name="Lucky Potpourri"><set name="is_questitem" val="true"/></item>
        <item id="1230" type="EtcItem" name="Eternity Diamond"/>"#;

    struct Npcs {
        rossela: Oid,
        greenis: Oid,
        thalia: Oid,
        northwind: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00408PathOfTheElvenWizard)
            .page("30414-01.htm", &["ACCEPT", "30414-02.htm"])
            .page(
                "30414-06.htm",
                &["30414-10.html", "30414-12.html", "30414-16.html"],
            )
            .page(
                "30414-11.html",
                &["30414-10.html", "30414-12.html", "30414-16.html"],
            )
            .page("30157-01.html", &["30157-02.html"])
            .page("30371-01.html", &["30371-02.html"])
            .plain(&[
                "30414-02.htm",
                "30414-02a.htm",
                "30414-03.htm",
                "30414-04.htm",
                "30414-05.htm",
                "30414-07.html",
                "30414-08.html",
                "30414-09.html",
                "30414-13.html",
                "30414-14.html",
                "30414-15.html",
                "30414-17.html",
                "30414-18.html",
                "30414-19.html",
                "30414-20.html",
                "30414-21.html",
                "30414-22.html",
                "30414-23.html",
                "30157-02.html",
                "30157-03.html",
                "30157-04.html",
                "30371-02.html",
                "30371-03.html",
                "30371-04.html",
                "30423-01.html",
                "30423-02.html",
                "30423-03.html",
            ])
            .npcs(&[ROSSELA, GREENIS, THALIA, NORTHWIND])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00408PathOfTheElvenWizard);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            rossela: t.spawn(ROSSELA, 20),
            greenis: t.spawn(GREENIS, 40),
            thalia: t.spawn(THALIA, 60),
            northwind: t.spawn(NORTHWIND, 80),
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
    fn the_accept_page_depends_on_class_level_and_diamond() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::ELF, player_class::ELVEN_MAGE, vec![]);
        t.quest_page(1, npcs.rossela);
        assert!(t.event(1, NAME, "ACCEPT").contains("30414-04.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ELF, player_class::ELVEN_WIZARD, vec![]);
        t.quest_page(1, npcs.rossela);
        assert!(t.event(1, NAME, "ACCEPT").contains("30414-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        t.quest_page(1, npcs.rossela);
        assert!(t.event(1, NAME, "ACCEPT").contains("30414-03.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::ELF,
            player_class::ELVEN_MAGE,
            vec![item(5000, 1230, 1)],
        );
        t.quest_page(1, npcs.rossela);
        assert!(t.event(1, NAME, "ACCEPT").contains("30414-05.htm"));
    }

    #[test]
    fn the_whole_path_from_the_peridot_to_the_diamond() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_MAGE, vec![]);
        assert!(t.quest_page(1, npcs.rossela).contains("30414-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30414-06.htm"));
        assert_eq!(t.count(1, FERTILITY_PERIDOT), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Rossela sends the player to Greenis: the letter.
        assert!(t.quest_page(1, npcs.rossela).contains("30414-11.html"));
        assert!(t.event(1, NAME, "30414-10.html").contains("30414-07.html"));
        assert_eq!(t.count(1, ROSELLAS_LETTER), 1);

        // Greenis takes the letter and gives the charm; the pincer spiders give 5 red down.
        assert!(t.quest_page(1, npcs.greenis).contains("30157-01.html"));
        assert!(t.event(1, NAME, "30157-02.html").contains("30157-02.html"));
        assert_eq!((t.count(1, ROSELLAS_LETTER), t.count(1, GREENISS_CHARM)), (0, 1));
        hunt(&mut t, PINCER_SPIDER, RED_DOWN, 5);
        assert!(t.quest_page(1, npcs.greenis).contains("30157-04.html"));
        assert_eq!(
            (
                t.count(1, RED_DOWN),
                t.count(1, GREENISS_CHARM),
                t.count(1, MAGICAL_POWERS_RUBY)
            ),
            (0, 0, 1)
        );

        // Rossela sends the player to Thalia: the apple.
        assert!(t.quest_page(1, npcs.rossela).contains("30414-11.html"));
        assert!(t.event(1, NAME, "30414-12.html").contains("30414-13.html"));
        assert_eq!(t.count(1, APPETIZING_APPLE), 1);

        // Thalia takes the apple and gives the sap; the dryad elders give 5 gold leaves.
        assert!(t.quest_page(1, npcs.thalia).contains("30371-01.html"));
        assert!(t.event(1, NAME, "30371-02.html").contains("30371-02.html"));
        assert_eq!(
            (t.count(1, APPETIZING_APPLE), t.count(1, SAP_OF_THE_MOTHER_TREE)),
            (0, 1)
        );
        hunt(&mut t, DRYAD_ELDER, GOLD_LEAVES, 5);
        assert!(t.quest_page(1, npcs.thalia).contains("30371-04.html"));
        assert_eq!((t.count(1, GOLD_LEAVES), t.count(1, PURE_AQUAMARINE)), (0, 1));

        // Rossela sends the player to Northwind: the immortal love.
        assert!(t.quest_page(1, npcs.rossela).contains("30414-11.html"));
        assert!(t.event(1, NAME, "30414-16.html").contains("30414-17.html"));
        assert_eq!(t.count(1, IMMORTAL_LOVE), 1);

        // Northwind takes it and gives the potpourri; the wererat leaders give 2 amethysts.
        assert!(t.quest_page(1, npcs.northwind).contains("30423-01.html"));
        assert_eq!((t.count(1, IMMORTAL_LOVE), t.count(1, LUCKY_POTPOURRI)), (0, 1));
        hunt(&mut t, SUKAR_WERERAT_LEADER, AMETHYST, 2);
        assert!(t.quest_page(1, npcs.northwind).contains("30423-03.html"));
        assert_eq!((t.count(1, AMETHYST), t.count(1, NOBILITY_AMETHYST)), (0, 1));
        assert!(t.take_writes().is_empty());

        // Rossela's reward: adena, the diamond, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.rossela).contains("30414-20.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, ETERNITY_DIAMOND), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, npcs.rossela).contains("30414-01.htm"));
    }
}
