//! Quest 413: Path of the Shillien Oracle (Magister Sidra, Priest Adonius and Magister Talbot).
// Java: dist/game/data/scripts/quests/Q00413_PathOfTheShillienOracle/Q00413_PathOfTheShillienOracle.java
//
// Pattern: a kill chain where each kill swaps one item for another (`docs/design/scripts.md`,
// "Quest patterns"): dark succubi turn blank sheets into bloody runes, the undead drop ashen
// bones for the penitent's mark, then the reward from Sidra.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MAGISTER_SIDRA: i32 = 30330;
const PRIEST_ADONIUS: i32 = 30375;
const MAGISTER_TALBOT: i32 = 30377;

// Items
const SIDRAS_LETTER: i32 = 1262;
const BLANK_SHEET: i32 = 1263;
const BLOODY_RUNE: i32 = 1264;
const GARMIELS_BOOK: i32 = 1265;
const PRAYER_OF_ADONIUS: i32 = 1266;
const PENITENTS_MARK: i32 = 1267;
const ASHEN_BONES: i32 = 1268;
const ANDARIEL_BOOK: i32 = 1269;

// Reward
const ORB_OF_ABYSS: i32 = 1270;

// Monsters
const ZOMBIE_SOLDIER: i32 = 20457;
const ZOMBIE_WARRIOR: i32 = 20458;
const SHIELD_SKELETON: i32 = 20514;
const SKELETON_INFANTRYMAN: i32 = 20515;
const DARK_SUCCUBUS: i32 = 20776;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00413_PathOfTheShillienOracle`.
pub struct Q00413PathOfTheShillienOracle;

impl Script for Q00413PathOfTheShillienOracle {
    fn name(&self) -> &'static str {
        "Q00413_PathOfTheShillienOracle"
    }

    fn path(&self) -> &'static str {
        "quests/Q00413_PathOfTheShillienOracle"
    }

    fn id(&self) -> i32 {
        413
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MAGISTER_SIDRA]);
        s.add_talk_id(&[MAGISTER_SIDRA, PRIEST_ADONIUS, MAGISTER_TALBOT]);
        s.add_kill_id(&[
            ZOMBIE_SOLDIER,
            ZOMBIE_WARRIOR,
            SHIELD_SKELETON,
            SKELETON_INFANTRYMAN,
            DARK_SUCCUBUS,
        ]);
        s.register_quest_items(&[
            SIDRAS_LETTER,
            BLANK_SHEET,
            BLOODY_RUNE,
            GARMIELS_BOOK,
            PRAYER_OF_ADONIUS,
            PENITENTS_MARK,
            ASHEN_BONES,
            ANDARIEL_BOOK,
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
                    if ctx.player_class_id(player) == player_class::DARK_MAGE {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, ORB_OF_ABYSS) {
                                "30330-04.htm"
                            } else {
                                "30330-05.htm"
                            }
                        } else {
                            "30330-02.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::SHILLIEN_ORACLE {
                        "30330-02a.htm"
                    } else {
                        "30330-03.htm"
                    }
                    .to_owned(),
                );
            }
            "30330-06.htm" => {
                if !ctx.has_quest_items(player, SIDRAS_LETTER) {
                    ctx.give_items(player, SIDRAS_LETTER, 1);
                }
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30330-06a.html" | "30375-02.html" | "30375-03.html" => {
                htmltext = Some(event.to_owned());
            }
            "30375-04.html" => {
                if ctx.has_quest_items(player, PRAYER_OF_ADONIUS) {
                    ctx.take_items(player, PRAYER_OF_ADONIUS, 1);
                    ctx.give_items(player, PENITENTS_MARK, 1);
                    qs.set_cond(ctx, 5, true);
                }
                htmltext = Some(event.to_owned());
            }
            "30377-02.html" => {
                if ctx.has_quest_items(player, SIDRAS_LETTER) {
                    ctx.take_items(player, SIDRAS_LETTER, 1);
                    ctx.give_items(player, BLANK_SHEET, 5);
                    qs.set_cond(ctx, 2, true);
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
        match npc.id {
            ZOMBIE_SOLDIER | ZOMBIE_WARRIOR | SHIELD_SKELETON | SKELETON_INFANTRYMAN => {
                if ctx.has_quest_items(killer, PENITENTS_MARK)
                    && ctx.get_quest_items_count(killer, ASHEN_BONES) < 10
                {
                    ctx.give_items(killer, ASHEN_BONES, 1);
                    if ctx.get_quest_items_count(killer, ASHEN_BONES) == 10 {
                        qs.set_cond(ctx, 6, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            DARK_SUCCUBUS => {
                if ctx.has_quest_items(killer, BLANK_SHEET) {
                    ctx.give_items(killer, BLOODY_RUNE, 1);
                    ctx.take_items(killer, BLANK_SHEET, 1);
                    if !ctx.has_quest_items(killer, BLANK_SHEET)
                        && ctx.get_quest_items_count(killer, BLOODY_RUNE) == 5
                    {
                        qs.set_cond(ctx, 3, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
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
            if npc.id == MAGISTER_SIDRA {
                htmltext = "30330-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                MAGISTER_SIDRA => {
                    if ctx.has_quest_items(player, SIDRAS_LETTER) {
                        htmltext = "30330-07.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[BLANK_SHEET, BLOODY_RUNE]) {
                        htmltext = "30330-08.html".to_owned();
                    } else if !ctx.has_quest_items(player, ANDARIEL_BOOK)
                        && ctx.has_at_least_one_quest_item(
                            player,
                            &[PRAYER_OF_ADONIUS, GARMIELS_BOOK, PENITENTS_MARK, ASHEN_BONES],
                        )
                    {
                        htmltext = "30330-09.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[ANDARIEL_BOOK, GARMIELS_BOOK]) {
                        ctx.give_adena(player, 163_800, true);
                        ctx.give_items(player, ORB_OF_ABYSS, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 26_532);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 33_230);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 39_928);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30330-10.html".to_owned();
                    }
                }
                PRIEST_ADONIUS => {
                    if ctx.has_quest_items(player, PRAYER_OF_ADONIUS) {
                        htmltext = "30375-01.html".to_owned();
                    } else if ctx.has_quest_items(player, PENITENTS_MARK)
                        && !ctx.has_at_least_one_quest_item(player, &[ASHEN_BONES, ANDARIEL_BOOK])
                    {
                        htmltext = "30375-05.html".to_owned();
                    } else if ctx.has_quest_items(player, PENITENTS_MARK) {
                        if ctx.has_quest_items(player, ASHEN_BONES)
                            && ctx.get_quest_items_count(player, ASHEN_BONES) < 10
                        {
                            htmltext = "30375-06.html".to_owned();
                        } else {
                            ctx.take_items(player, PENITENTS_MARK, 1);
                            ctx.take_items(player, ASHEN_BONES, -1);
                            ctx.give_items(player, ANDARIEL_BOOK, 1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30375-07.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, ANDARIEL_BOOK) {
                        htmltext = "30375-08.html".to_owned();
                    }
                }
                MAGISTER_TALBOT => {
                    if ctx.has_quest_items(player, SIDRAS_LETTER) {
                        htmltext = "30377-01.html".to_owned();
                    } else if !ctx.has_quest_items(player, BLOODY_RUNE)
                        && ctx.get_quest_items_count(player, BLANK_SHEET) == 5
                    {
                        htmltext = "30377-03.html".to_owned();
                    } else if ctx.has_quest_items(player, BLOODY_RUNE)
                        && ctx.get_quest_items_count(player, BLOODY_RUNE) < 5
                    {
                        htmltext = "30377-04.html".to_owned();
                    } else if ctx.get_quest_items_count(player, BLOODY_RUNE) >= 5 {
                        ctx.take_items(player, BLOODY_RUNE, -1);
                        ctx.give_items(player, GARMIELS_BOOK, 1);
                        ctx.give_items(player, PRAYER_OF_ADONIUS, 1);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30377-05.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(
                        player,
                        &[PRAYER_OF_ADONIUS, PENITENTS_MARK, ASHEN_BONES],
                    ) {
                        htmltext = "30377-06.html".to_owned();
                    } else if ctx.has_all_quest_items(player, &[ANDARIEL_BOOK, GARMIELS_BOOK]) {
                        htmltext = "30377-07.html".to_owned();
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

    const NAME: &str = "Q00413_PathOfTheShillienOracle";

    const NPCS: &str = r#"
        <npc id="30330" type="Folk" level="70" name="Magister Sidra"/>
        <npc id="30375" type="Folk" level="70" name="Priest Adonius"/>
        <npc id="30377" type="Folk" level="70" name="Magister Talbot"/>
        <npc id="20457" type="Monster" level="20" name="Zombie Soldier"/>
        <npc id="20776" type="Monster" level="20" name="Dark Succubus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1262" type="EtcItem" name="Sidra's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1263" type="EtcItem" name="Blank Sheet"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1264" type="EtcItem" name="Bloody Rune"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1265" type="EtcItem" name="Garmiel's Book"><set name="is_questitem" val="true"/></item>
        <item id="1266" type="EtcItem" name="Prayer of Adonius"><set name="is_questitem" val="true"/></item>
        <item id="1267" type="EtcItem" name="Penitent's Mark"><set name="is_questitem" val="true"/></item>
        <item id="1268" type="EtcItem" name="Ashen Bones"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1269" type="EtcItem" name="Andariel's Book"><set name="is_questitem" val="true"/></item>
        <item id="1270" type="EtcItem" name="Orb of Abyss"/>"#;

    struct Npcs {
        sidra: Oid,
        adonius: Oid,
        talbot: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00413PathOfTheShillienOracle)
            .page("30330-01.htm", &["ACCEPT"])
            .page("30330-05.htm", &["30330-06.htm"])
            .page("30330-06.htm", &["30330-06a.html"])
            .page("30330-06a.html", &["30330-06.htm"])
            .page("30375-01.html", &["30375-02.html"])
            .page("30375-02.html", &["30375-03.html"])
            .page("30375-03.html", &["30375-04.html"])
            .page("30377-01.html", &["30377-02.html"])
            .plain(&[
                "30330-02.htm",
                "30330-02a.htm",
                "30330-03.htm",
                "30330-04.htm",
                "30330-07.html",
                "30330-08.html",
                "30330-09.html",
                "30330-10.html",
                "30375-04.html",
                "30375-05.html",
                "30375-06.html",
                "30375-07.html",
                "30375-08.html",
                "30377-02.html",
                "30377-03.html",
                "30377-04.html",
                "30377-05.html",
                "30377-06.html",
                "30377-07.html",
            ])
            .npcs(&[MAGISTER_SIDRA, PRIEST_ADONIUS, MAGISTER_TALBOT])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00413PathOfTheShillienOracle);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            sidra: t.spawn(MAGISTER_SIDRA, 20),
            adonius: t.spawn(PRIEST_ADONIUS, 40),
            talbot: t.spawn(MAGISTER_TALBOT, 60),
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
    fn the_accept_page_depends_on_class_level_and_orb() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::DARK_ELF, player_class::DARK_MAGE, vec![]);
        assert!(t.quest_page(1, npcs.sidra).contains("30330-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30330-02.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::DARK_ELF, player_class::SHILLIEN_ORACLE, vec![]);
        t.quest_page(1, npcs.sidra);
        assert!(t.event(1, NAME, "ACCEPT").contains("30330-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        t.quest_page(1, npcs.sidra);
        assert!(t.event(1, NAME, "ACCEPT").contains("30330-03.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::DARK_ELF, player_class::DARK_MAGE, vec![]);
        t.quest_page(1, npcs.sidra);
        assert!(t.event(1, NAME, "ACCEPT").contains("30330-05.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::DARK_ELF,
            player_class::DARK_MAGE,
            vec![item(5000, 1270, 1)],
        );
        t.quest_page(1, npcs.sidra);
        assert!(t.event(1, NAME, "ACCEPT").contains("30330-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_letter_to_the_orb() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::DARK_ELF, player_class::DARK_MAGE, vec![]);
        assert!(t.quest_page(1, npcs.sidra).contains("30330-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30330-05.htm"));
        assert!(t.event(1, NAME, "30330-06.htm").contains("30330-06.htm"));
        assert_eq!(t.count(1, SIDRAS_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.sidra).contains("30330-07.html"));

        // Talbot takes the letter and gives five blank sheets: cond 2.
        assert!(t.quest_page(1, npcs.talbot).contains("30377-01.html"));
        assert!(t.event(1, NAME, "30377-02.html").contains("30377-02.html"));
        assert_eq!((t.count(1, SIDRAS_LETTER), t.count(1, BLANK_SHEET)), (0, 5));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.talbot).contains("30377-03.html"));
        assert!(t.quest_page(1, npcs.sidra).contains("30330-08.html"));

        // Five dark succubi: a blank sheet becomes a bloody rune each; the fifth sets cond 3.
        hunt(&mut t, DARK_SUCCUBUS, BLOODY_RUNE, 5);
        assert_eq!((t.count(1, BLANK_SHEET), t.count(1, BLOODY_RUNE)), (0, 5));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Talbot takes the runes and gives Garmiel's book and the prayer: cond 4.
        assert!(t.quest_page(1, npcs.talbot).contains("30377-05.html"));
        assert_eq!(
            (
                t.count(1, BLOODY_RUNE),
                t.count(1, GARMIELS_BOOK),
                t.count(1, PRAYER_OF_ADONIUS)
            ),
            (0, 1, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.sidra).contains("30330-09.html"));
        assert!(t.quest_page(1, npcs.talbot).contains("30377-06.html"));

        // Adonius: the prayer becomes the penitent's mark: cond 5.
        assert!(t.quest_page(1, npcs.adonius).contains("30375-01.html"));
        assert!(t.event(1, NAME, "30375-02.html").contains("30375-02.html"));
        assert!(t.event(1, NAME, "30375-03.html").contains("30375-03.html"));
        assert!(t.event(1, NAME, "30375-04.html").contains("30375-04.html"));
        assert_eq!(
            (t.count(1, PRAYER_OF_ADONIUS), t.count(1, PENITENTS_MARK)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.adonius).contains("30375-05.html"));

        // Ten ashen bones from the undead; the tenth sets cond 6.
        hunt(&mut t, ZOMBIE_SOLDIER, ASHEN_BONES, 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Adonius takes the mark and the bones and gives Andariel's book: cond 7.
        assert!(t.quest_page(1, npcs.adonius).contains("30375-07.html"));
        assert_eq!(
            (
                t.count(1, PENITENTS_MARK),
                t.count(1, ASHEN_BONES),
                t.count(1, ANDARIEL_BOOK)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);
        assert!(t.quest_page(1, npcs.adonius).contains("30375-08.html"));

        // Sidra's reward: adena, the orb, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.sidra).contains("30330-10.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, ORB_OF_ABYSS), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: Sidra shows his first page again.
        assert!(t.quest_page(1, npcs.sidra).contains("30330-01.htm"));
    }
}
