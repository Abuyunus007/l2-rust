//! Quest 406: Path of the Elven Knight (Master Sorius and Blacksmith Kluto).
// Java: dist/game/data/scripts/quests/Q00406_PathOfTheElvenKnight/Q00406_PathOfTheElvenKnight.java
//
// Pattern: a kill chain with drop chances up to a limit (`docs/design/scripts.md`, "Quest
// patterns"): topaz pieces from six monsters, emerald pieces from Ol Mahum Novices (only with
// Kluto's memo), then Kluto's box and Sorius' reward.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BLACKSMITH_KLUTO: i32 = 30317;
const MASTER_SORIUS: i32 = 30327;

// Items
const SORIUS_LETTER: i32 = 1202;
const KLUTO_BOX: i32 = 1203;
const TOPAZ_PIECE: i32 = 1205;
const EMERALD_PIECE: i32 = 1206;
const KLUTO_MEMO: i32 = 1276;

// Reward
const ELVEN_KNIGHT_BROOCH: i32 = 1204;

// Misc
const MIN_LEVEL: i32 = 18;

// Mobs
const OL_MAHUM_NOVICE: i32 = 20782;
/// Java `MONSTER_DROPS`: (monster, item, chance in percent).
const MONSTER_DROPS: &[(i32, i32, i32)] = &[
    (20035, TOPAZ_PIECE, 70), // Tracker Skeleton
    (20042, TOPAZ_PIECE, 70), // Tracker Skeleton Leader
    (20045, TOPAZ_PIECE, 70), // Skeleton Scout
    (20051, TOPAZ_PIECE, 70), // Skeleton Bowman
    (20054, TOPAZ_PIECE, 70), // Ruin Spartoi
    (20060, TOPAZ_PIECE, 70), // Salamander Noble
    (OL_MAHUM_NOVICE, EMERALD_PIECE, 50),
];

/// Java `Q00406_PathOfTheElvenKnight`.
pub struct Q00406PathOfTheElvenKnight;

impl Script for Q00406PathOfTheElvenKnight {
    fn name(&self) -> &'static str {
        "Q00406_PathOfTheElvenKnight"
    }

    fn path(&self) -> &'static str {
        "quests/Q00406_PathOfTheElvenKnight"
    }

    fn id(&self) -> i32 {
        406
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_SORIUS]);
        s.add_talk_id(&[MASTER_SORIUS, BLACKSMITH_KLUTO]);
        let mobs: Vec<i32> = MONSTER_DROPS.iter().map(|d| d.0).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SORIUS_LETTER, KLUTO_BOX, TOPAZ_PIECE, EMERALD_PIECE, KLUTO_MEMO]);
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
                    if ctx.player_class_id(player) != player_class::ELVEN_FIGHTER {
                        if ctx.player_class_id(player) == player_class::ELVEN_KNIGHT {
                            "30327-02a.htm"
                        } else {
                            "30327-02.htm"
                        }
                    } else if ctx.player_level(player) < MIN_LEVEL {
                        "30327-03.htm"
                    } else if ctx.has_quest_items(player, ELVEN_KNIGHT_BROOCH) {
                        "30327-04.htm"
                    } else {
                        "30327-05.htm"
                    }
                    .to_owned(),
                );
            }
            "30327-06.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30317-02.html" => {
                ctx.take_items(player, SORIUS_LETTER, 1);
                if !ctx.has_quest_items(player, KLUTO_MEMO) {
                    ctx.give_items(player, KLUTO_MEMO, 1);
                }
                qs.set_cond(ctx, 4, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(&(_, item, chance)) = MONSTER_DROPS.iter().find(|d| d.0 == npc.id) else {
            return;
        };
        let mut cond = 2;
        let mut check = !ctx.has_quest_items(killer, KLUTO_BOX);
        if npc.id == OL_MAHUM_NOVICE {
            cond = 5;
            check = ctx.has_quest_items(killer, KLUTO_MEMO);
        }

        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
            && check
            && ctx.get_quest_items_count(killer, item) < 20
            && ctx.get_random(100) < chance
        {
            ctx.give_items(killer, item, 1);
            if ctx.get_quest_items_count(killer, item) == 20 {
                qs.set_cond(ctx, cond, true);
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
            if npc.id == MASTER_SORIUS {
                htmltext = "30327-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                MASTER_SORIUS => {
                    if !ctx.has_quest_items(player, KLUTO_BOX) {
                        if !ctx.has_quest_items(player, TOPAZ_PIECE) {
                            htmltext = "30327-07.html".to_owned();
                        } else if ctx.has_quest_items(player, TOPAZ_PIECE)
                            && ctx.get_quest_items_count(player, TOPAZ_PIECE) < 20
                        {
                            htmltext = "30327-08.html".to_owned();
                        } else if !ctx.has_at_least_one_quest_item(player, &[KLUTO_MEMO, SORIUS_LETTER])
                            && ctx.get_quest_items_count(player, TOPAZ_PIECE) >= 20
                        {
                            if !ctx.has_quest_items(player, SORIUS_LETTER) {
                                ctx.give_items(player, SORIUS_LETTER, 1);
                            }
                            qs.set_cond(ctx, 3, true);
                            htmltext = "30327-09.html".to_owned();
                        } else if ctx.get_quest_items_count(player, TOPAZ_PIECE) >= 20
                            && ctx.has_at_least_one_quest_item(player, &[SORIUS_LETTER, KLUTO_MEMO])
                        {
                            htmltext = "30327-11.html".to_owned();
                        }
                    } else {
                        ctx.give_adena(player, 163_800, true);
                        if !ctx.has_quest_items(player, ELVEN_KNIGHT_BROOCH) {
                            ctx.give_items(player, ELVEN_KNIGHT_BROOCH, 1);
                        }
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 23_152);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 29_850);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 33_328);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30327-10.html".to_owned();
                    }
                }
                BLACKSMITH_KLUTO => {
                    if !ctx.has_quest_items(player, KLUTO_BOX) {
                        let topaz = ctx.get_quest_items_count(player, TOPAZ_PIECE);
                        let emerald = ctx.get_quest_items_count(player, EMERALD_PIECE);
                        if ctx.has_quest_items(player, SORIUS_LETTER) && topaz >= 20 {
                            htmltext = "30317-01.html".to_owned();
                        } else if !ctx.has_quest_items(player, EMERALD_PIECE)
                            && ctx.has_quest_items(player, KLUTO_MEMO)
                            && topaz >= 20
                        {
                            htmltext = "30317-03.html".to_owned();
                        } else if ctx.has_quest_items(player, KLUTO_MEMO)
                            && ctx.has_quest_items(player, EMERALD_PIECE)
                            && topaz >= 20
                            && emerald < 20
                        {
                            htmltext = "30317-04.html".to_owned();
                        } else if ctx.has_quest_items(player, KLUTO_MEMO) && topaz >= 20 && emerald >= 20 {
                            if !ctx.has_quest_items(player, KLUTO_BOX) {
                                ctx.give_items(player, KLUTO_BOX, 1);
                            }
                            ctx.take_items(player, TOPAZ_PIECE, -1);
                            ctx.take_items(player, EMERALD_PIECE, -1);
                            ctx.take_items(player, KLUTO_MEMO, 1);
                            qs.set_cond(ctx, 6, true);
                            htmltext = "30317-05.html".to_owned();
                        }
                    } else {
                        htmltext = "30317-06.html".to_owned();
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

    const NAME: &str = "Q00406_PathOfTheElvenKnight";

    const NPCS: &str = r#"
        <npc id="30317" type="Folk" level="70" name="Blacksmith Kluto"/>
        <npc id="30327" type="Folk" level="70" name="Master Sorius"/>
        <npc id="20035" type="Monster" level="20" name="Tracker Skeleton"/>
        <npc id="20782" type="Monster" level="20" name="Ol Mahum Novice"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1202" type="EtcItem" name="Sorius' Letter"><set name="is_questitem" val="true"/></item>
        <item id="1203" type="EtcItem" name="Kluto's Box"><set name="is_questitem" val="true"/></item>
        <item id="1204" type="EtcItem" name="Elven Knight Brooch"/>
        <item id="1205" type="EtcItem" name="Topaz Piece"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1206" type="EtcItem" name="Emerald Piece"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1276" type="EtcItem" name="Kluto's Memo"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        sorius: Oid,
        kluto: Oid,
    }

    /// The stand-in pages: the file names and the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00406PathOfTheElvenKnight)
            .page("30327-01.htm", &["ACCEPT"])
            .page("30327-05.htm", &["30327-06.htm"])
            .page("30317-01.html", &["30317-02.html"])
            .plain(&[
                "30327-02.htm",
                "30327-02a.htm",
                "30327-03.htm",
                "30327-04.htm",
                "30327-06.htm",
                "30327-07.html",
                "30327-08.html",
                "30327-09.html",
                "30327-10.html",
                "30327-11.html",
                "30317-02.html",
                "30317-03.html",
                "30317-04.html",
                "30317-05.html",
                "30317-06.html",
            ])
            .npcs(&[MASTER_SORIUS, BLACKSMITH_KLUTO])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00406PathOfTheElvenKnight);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            sorius: t.spawn(MASTER_SORIUS, 20),
            kluto: t.spawn(BLACKSMITH_KLUTO, 40),
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
    fn the_accept_page_depends_on_class_level_and_brooch() {
        // An elven fighter of level 17: too low.
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.sorius).contains("30327-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30327-03.htm"));
        assert!(t.take_writes().is_empty());

        // An elven fighter of level 18 starts it.
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        t.quest_page(1, npcs.sorius);
        assert!(t.event(1, NAME, "ACCEPT").contains("30327-05.htm"));

        // An elven knight is sent to the page for that class.
        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ELF, player_class::ELVEN_KNIGHT, vec![]);
        t.quest_page(1, npcs.sorius);
        assert!(t.event(1, NAME, "ACCEPT").contains("30327-02a.htm"));

        // Any other class.
        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ELF, player_class::ELVEN_SCOUT, vec![]);
        t.quest_page(1, npcs.sorius);
        assert!(t.event(1, NAME, "ACCEPT").contains("30327-02.htm"));

        // A character that has the brooch already.
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::ELF,
            player_class::ELVEN_FIGHTER,
            vec![item(5000, 1204, 1)],
        );
        t.quest_page(1, npcs.sorius);
        assert!(t.event(1, NAME, "ACCEPT").contains("30327-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_topaz_to_the_brooch() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.sorius).contains("30327-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30327-05.htm"));
        assert!(t.event(1, NAME, "30327-06.htm").contains("30327-06.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // No topaz yet.
        assert!(t.quest_page(1, npcs.sorius).contains("30327-07.html"));

        // Twenty topaz pieces (a 70% drop); the twentieth sets cond 2. Sorius then gives the letter.
        hunt(&mut t, 20035, TOPAZ_PIECE, 20);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.sorius).contains("30327-09.html"));
        assert_eq!(t.count(1, SORIUS_LETTER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Kluto takes the letter and gives the memo: cond 4.
        assert!(t.quest_page(1, npcs.kluto).contains("30317-01.html"));
        assert!(t.event(1, NAME, "30317-02.html").contains("30317-02.html"));
        assert_eq!((t.count(1, SORIUS_LETTER), t.count(1, KLUTO_MEMO)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Sorius now sees the topaz and the memo.
        assert!(t.quest_page(1, npcs.sorius).contains("30327-11.html"));

        // Emerald pieces need the memo: a 50% drop of Ol Mahum Novices, cond 5 at twenty.
        hunt(&mut t, OL_MAHUM_NOVICE, EMERALD_PIECE, 20);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Kluto makes the box and takes the topaz, the emerald and the memo: cond 6.
        assert!(t.quest_page(1, npcs.kluto).contains("30317-05.html"));
        assert_eq!(
            (
                t.count(1, KLUTO_BOX),
                t.count(1, TOPAZ_PIECE),
                t.count(1, EMERALD_PIECE),
                t.count(1, KLUTO_MEMO)
            ),
            (1, 0, 0, 0)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, npcs.kluto).contains("30317-06.html"));

        // Sorius' reward: adena, the brooch, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.sorius).contains("30327-10.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, ELVEN_KNIGHT_BROOCH), 1);
        assert_eq!(t.count(1, KLUTO_BOX), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: Sorius shows his first page again.
        assert!(t.quest_page(1, npcs.sorius).contains("30327-01.htm"));
    }

    #[test]
    fn ol_mahum_novices_give_nothing_without_the_memo() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ELF, player_class::ELVEN_FIGHTER, vec![]);
        t.quest_page(1, npcs.sorius);
        t.event(1, NAME, "ACCEPT");
        t.event(1, NAME, "30327-06.htm");
        t.take_writes();
        for _ in 0..30 {
            let mob = t.spawn(OL_MAHUM_NOVICE, 80);
            t.kill(1, mob);
        }
        assert_eq!(t.count(1, EMERALD_PIECE), 0);
        assert!(t.take_writes().is_empty());
    }
}
