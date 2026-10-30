//! Quest 405: Path of the Cleric (Zigaunt, Gallint, Vivyan, Trader Simplon, Guard Praga, Lionel).
// Java: dist/game/data/scripts/quests/Q00405_PathOfTheCleric/Q00405_PathOfTheCleric.java
//
// Pattern: a delivery chain through six NPCs, each handing out or taking one item and setting
// the cond when the set is complete (`docs/design/scripts.md`, "Quest patterns"). The pendant of
// the mother comes from a ruin zombie kill.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GALLINT: i32 = 30017;
const ZIGAUNT: i32 = 30022;
const VIVYAN: i32 = 30030;
const TRADER_SIMPLON: i32 = 30253;
const GUARD_PRAGA: i32 = 30333;
const LIONEL: i32 = 30408;

// Items
const LETTER_OF_ORDER_1ST: i32 = 1191;
const LETTER_OF_ORDER_2ND: i32 = 1192;
const LIONELS_BOOK: i32 = 1193;
const BOOK_OF_VIVYAN: i32 = 1194;
const BOOK_OF_SIMPLON: i32 = 1195;
const BOOK_OF_PRAGA: i32 = 1196;
const CERTIFICATE_OF_GALLINT: i32 = 1197;
const PENDANT_OF_MOTHER: i32 = 1198;
const NECKLACE_OF_MOTHER: i32 = 1199;
const LEMONIELLS_COVENANT: i32 = 1200;

// Reward
const MARK_OF_FAITH: i32 = 1201;

// Monsters
const RUIN_ZOMBIE: i32 = 20026;
const RUIN_ZOMBIE_LEADER: i32 = 20029;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00405_PathOfTheCleric`.
pub struct Q00405PathOfTheCleric;

impl Script for Q00405PathOfTheCleric {
    fn name(&self) -> &'static str {
        "Q00405_PathOfTheCleric"
    }

    fn path(&self) -> &'static str {
        "quests/Q00405_PathOfTheCleric"
    }

    fn id(&self) -> i32 {
        405
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ZIGAUNT]);
        s.add_talk_id(&[ZIGAUNT, GALLINT, VIVYAN, TRADER_SIMPLON, GUARD_PRAGA, LIONEL]);
        s.add_kill_id(&[RUIN_ZOMBIE, RUIN_ZOMBIE_LEADER]);
        s.register_quest_items(&[
            LETTER_OF_ORDER_1ST,
            LETTER_OF_ORDER_2ND,
            LIONELS_BOOK,
            BOOK_OF_VIVYAN,
            BOOK_OF_SIMPLON,
            BOOK_OF_PRAGA,
            CERTIFICATE_OF_GALLINT,
            PENDANT_OF_MOTHER,
            NECKLACE_OF_MOTHER,
            LEMONIELLS_COVENANT,
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
        if event == "ACCEPT" {
            htmltext = Some(
                if ctx.player_class_id(player) == player_class::MAGE {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_quest_items(player, MARK_OF_FAITH) {
                            "30022-04.htm"
                        } else {
                            qs.start_quest(ctx);
                            ctx.give_items(player, LETTER_OF_ORDER_1ST, 1);
                            "30022-05.htm"
                        }
                    } else {
                        "30022-03.htm"
                    }
                } else if ctx.player_class_id(player) == player_class::CLERIC {
                    "30022-02a.htm"
                } else {
                    "30022-02.htm"
                }
                .to_owned(),
            );
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            && ctx.has_quest_items(killer, NECKLACE_OF_MOTHER)
            && !ctx.has_quest_items(killer, PENDANT_OF_MOTHER)
        {
            ctx.give_items(killer, PENDANT_OF_MOTHER, 1);
            ctx.play_sound(qs.player(), QuestSound::ItemsoundQuestMiddle);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == ZIGAUNT {
                htmltext = if !ctx.has_quest_items(player, MARK_OF_FAITH) {
                    "30022-01.htm"
                } else {
                    "30022-04.htm"
                }
                .to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                ZIGAUNT => {
                    if !ctx.has_quest_items(player, LEMONIELLS_COVENANT)
                        && ctx.has_quest_items(player, LETTER_OF_ORDER_2ND)
                    {
                        htmltext = "30022-07.html".to_owned();
                    } else if ctx.has_all_quest_items(player, &[LETTER_OF_ORDER_2ND, LEMONIELLS_COVENANT]) {
                        ctx.give_adena(player, 163_800, true);
                        ctx.take_items(player, LETTER_OF_ORDER_2ND, 1);
                        ctx.take_items(player, LEMONIELLS_COVENANT, 1);
                        ctx.give_items(player, MARK_OF_FAITH, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 23_152);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 28_630);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 35_328);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30022-09.html".to_owned();
                    } else if ctx.has_quest_items(player, LETTER_OF_ORDER_1ST) {
                        if ctx.has_all_quest_items(player, &[BOOK_OF_VIVYAN, BOOK_OF_SIMPLON, BOOK_OF_PRAGA])
                        {
                            ctx.take_items(player, LETTER_OF_ORDER_1ST, 1);
                            ctx.give_items(player, LETTER_OF_ORDER_2ND, 1);
                            ctx.take_items(player, BOOK_OF_VIVYAN, 1);
                            ctx.take_items(player, BOOK_OF_SIMPLON, -1);
                            ctx.take_items(player, BOOK_OF_PRAGA, 1);
                            qs.set_cond(ctx, 3, true);
                            htmltext = "30022-08.html".to_owned();
                        } else {
                            htmltext = "30022-06.html".to_owned();
                        }
                    }
                }
                GALLINT => {
                    if !ctx.has_quest_items(player, LEMONIELLS_COVENANT)
                        && ctx.has_quest_items(player, LETTER_OF_ORDER_2ND)
                    {
                        if !ctx.has_quest_items(player, CERTIFICATE_OF_GALLINT)
                            && ctx.has_quest_items(player, LIONELS_BOOK)
                        {
                            ctx.take_items(player, LIONELS_BOOK, 1);
                            ctx.give_items(player, CERTIFICATE_OF_GALLINT, 1);
                            qs.set_cond(ctx, 5, true);
                            htmltext = "30017-01.html".to_owned();
                        } else {
                            htmltext = "30017-02.html".to_owned();
                        }
                    }
                }
                VIVYAN => {
                    if ctx.has_quest_items(player, LETTER_OF_ORDER_1ST) {
                        if !ctx.has_quest_items(player, BOOK_OF_VIVYAN) {
                            ctx.give_items(player, BOOK_OF_VIVYAN, 1);
                            if ctx.get_quest_items_count(player, BOOK_OF_SIMPLON) >= 3
                                && ctx.get_quest_items_count(player, BOOK_OF_VIVYAN) >= 0
                                && ctx.get_quest_items_count(player, BOOK_OF_PRAGA) >= 1
                            {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30030-01.html".to_owned();
                        } else {
                            htmltext = "30030-02.html".to_owned();
                        }
                    }
                }
                TRADER_SIMPLON => {
                    if ctx.has_quest_items(player, LETTER_OF_ORDER_1ST) {
                        if !ctx.has_quest_items(player, BOOK_OF_SIMPLON) {
                            ctx.give_items(player, BOOK_OF_SIMPLON, 3);
                            if ctx.get_quest_items_count(player, BOOK_OF_SIMPLON) >= 0
                                && ctx.get_quest_items_count(player, BOOK_OF_VIVYAN) >= 1
                                && ctx.get_quest_items_count(player, BOOK_OF_PRAGA) >= 1
                            {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30253-01.html".to_owned();
                        } else {
                            htmltext = "30253-02.html".to_owned();
                        }
                    }
                }
                GUARD_PRAGA => {
                    if ctx.has_quest_items(player, LETTER_OF_ORDER_1ST) {
                        if !ctx.has_at_least_one_quest_item(player, &[BOOK_OF_PRAGA, NECKLACE_OF_MOTHER]) {
                            ctx.give_items(player, NECKLACE_OF_MOTHER, 1);
                            htmltext = "30333-01.html".to_owned();
                        } else if !ctx
                            .has_at_least_one_quest_item(player, &[BOOK_OF_PRAGA, PENDANT_OF_MOTHER])
                            && ctx.has_quest_items(player, NECKLACE_OF_MOTHER)
                        {
                            htmltext = "30333-02.html".to_owned();
                        } else if !ctx.has_quest_items(player, BOOK_OF_PRAGA)
                            && ctx.has_all_quest_items(player, &[NECKLACE_OF_MOTHER, PENDANT_OF_MOTHER])
                        {
                            ctx.give_items(player, BOOK_OF_PRAGA, 1);
                            ctx.take_items(player, PENDANT_OF_MOTHER, 1);
                            ctx.take_items(player, NECKLACE_OF_MOTHER, 1);
                            if ctx.get_quest_items_count(player, BOOK_OF_SIMPLON) >= 3
                                && ctx.get_quest_items_count(player, BOOK_OF_VIVYAN) >= 1
                                && ctx.get_quest_items_count(player, BOOK_OF_PRAGA) >= 0
                            {
                                qs.set_cond(ctx, 2, true);
                            }
                            htmltext = "30333-03.html".to_owned();
                        } else if ctx.has_quest_items(player, BOOK_OF_PRAGA) {
                            htmltext = "30333-04.html".to_owned();
                        }
                    }
                }
                LIONEL => {
                    if !ctx.has_quest_items(player, LETTER_OF_ORDER_2ND) {
                        htmltext = "30408-02.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[LIONELS_BOOK, LEMONIELLS_COVENANT, CERTIFICATE_OF_GALLINT],
                    ) && ctx.has_quest_items(player, LETTER_OF_ORDER_2ND)
                    {
                        ctx.give_items(player, LIONELS_BOOK, 1);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30408-01.html".to_owned();
                    } else if !ctx
                        .has_at_least_one_quest_item(player, &[LEMONIELLS_COVENANT, CERTIFICATE_OF_GALLINT])
                        && ctx.has_all_quest_items(player, &[LETTER_OF_ORDER_2ND, LIONELS_BOOK])
                    {
                        htmltext = "30408-03.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(player, &[LIONELS_BOOK, LEMONIELLS_COVENANT])
                        && ctx.has_all_quest_items(player, &[LETTER_OF_ORDER_2ND, CERTIFICATE_OF_GALLINT])
                    {
                        ctx.take_items(player, CERTIFICATE_OF_GALLINT, 1);
                        ctx.give_items(player, LEMONIELLS_COVENANT, 1);
                        qs.set_cond(ctx, 6, true);
                        htmltext = "30408-04.html".to_owned();
                    } else if !ctx
                        .has_at_least_one_quest_item(player, &[LIONELS_BOOK, CERTIFICATE_OF_GALLINT])
                        && ctx.has_all_quest_items(player, &[LETTER_OF_ORDER_2ND, LEMONIELLS_COVENANT])
                    {
                        htmltext = "30408-05.html".to_owned();
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

    const NAME: &str = "Q00405_PathOfTheCleric";

    const NPCS: &str = r#"
        <npc id="30017" type="Folk" level="70" name="Gallint"/>
        <npc id="30022" type="Folk" level="70" name="Zigaunt"/>
        <npc id="30030" type="Folk" level="70" name="Vivyan"/>
        <npc id="30253" type="Folk" level="70" name="Trader Simplon"/>
        <npc id="30333" type="Folk" level="70" name="Guard Praga"/>
        <npc id="30408" type="Folk" level="70" name="Lionel"/>
        <npc id="20026" type="Monster" level="20" name="Ruin Zombie"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1191" type="EtcItem" name="Letter of Order 1st"><set name="is_questitem" val="true"/></item>
        <item id="1192" type="EtcItem" name="Letter of Order 2nd"><set name="is_questitem" val="true"/></item>
        <item id="1193" type="EtcItem" name="Lionel's Book"><set name="is_questitem" val="true"/></item>
        <item id="1194" type="EtcItem" name="Book of Vivyan"><set name="is_questitem" val="true"/></item>
        <item id="1195" type="EtcItem" name="Book of Simplon"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1196" type="EtcItem" name="Book of Praga"><set name="is_questitem" val="true"/></item>
        <item id="1197" type="EtcItem" name="Certificate of Gallint"><set name="is_questitem" val="true"/></item>
        <item id="1198" type="EtcItem" name="Pendant of Mother"><set name="is_questitem" val="true"/></item>
        <item id="1199" type="EtcItem" name="Necklace of Mother"><set name="is_questitem" val="true"/></item>
        <item id="1200" type="EtcItem" name="Lemoniell's Covenant"><set name="is_questitem" val="true"/></item>
        <item id="1201" type="EtcItem" name="Mark of Faith"/>"#;

    struct Npcs {
        zigaunt: Oid,
        gallint: Oid,
        vivyan: Oid,
        simplon: Oid,
        praga: Oid,
        lionel: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00405PathOfTheCleric)
            .page("30022-01.htm", &["ACCEPT"])
            .plain(&[
                "30017-01.html",
                "30017-02.html",
                "30022-02.htm",
                "30022-02a.htm",
                "30022-03.htm",
                "30022-04.htm",
                "30022-05.htm",
                "30022-06.html",
                "30022-07.html",
                "30022-08.html",
                "30022-09.html",
                "30030-01.html",
                "30030-02.html",
                "30253-01.html",
                "30253-02.html",
                "30333-01.html",
                "30333-02.html",
                "30333-03.html",
                "30333-04.html",
                "30408-01.html",
                "30408-02.html",
                "30408-03.html",
                "30408-04.html",
                "30408-05.html",
            ])
            .npcs(&[ZIGAUNT, GALLINT, VIVYAN, TRADER_SIMPLON, GUARD_PRAGA, LIONEL])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00405PathOfTheCleric);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            zigaunt: t.spawn(ZIGAUNT, 20),
            gallint: t.spawn(GALLINT, 40),
            vivyan: t.spawn(VIVYAN, 60),
            simplon: t.spawn(TRADER_SIMPLON, 80),
            praga: t.spawn(GUARD_PRAGA, 100),
            lionel: t.spawn(LIONEL, 120),
        };
        (t, npcs)
    }

    #[test]
    fn the_accept_page_depends_on_class_level_and_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::MAGE, vec![]);
        assert!(t.quest_page(1, npcs.zigaunt).contains("30022-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30022-03.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::CLERIC, vec![]);
        t.quest_page(1, npcs.zigaunt);
        assert!(t.event(1, NAME, "ACCEPT").contains("30022-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::HUMAN, player_class::FIGHTER, vec![]);
        t.quest_page(1, npcs.zigaunt);
        assert!(t.event(1, NAME, "ACCEPT").contains("30022-02.htm"));

        let (mut t, npcs) = setup();
        // With the mark, Zigaunt's talk already shows the page (no link to ACCEPT on it).
        t.enter_as(1, 18, race::HUMAN, player_class::MAGE, vec![item(5000, 1201, 1)]);
        assert!(t.quest_page(1, npcs.zigaunt).contains("30022-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_path_from_the_letter_to_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::MAGE, vec![]);
        assert!(t.quest_page(1, npcs.zigaunt).contains("30022-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30022-05.htm"));
        assert_eq!(t.count(1, LETTER_OF_ORDER_1ST), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.zigaunt).contains("30022-06.html"));

        // The three books: Vivyan's, Simplon's (three of them) and Praga's are needed.
        assert!(t.quest_page(1, npcs.vivyan).contains("30030-01.html"));
        assert!(t.quest_page(1, npcs.vivyan).contains("30030-02.html"));
        assert!(t.quest_page(1, npcs.simplon).contains("30253-01.html"));
        assert_eq!(t.count(1, BOOK_OF_SIMPLON), 3);
        assert!(t.quest_page(1, npcs.simplon).contains("30253-02.html"));
        assert!(t.take_writes().is_empty());

        // Praga gives the necklace; a ruin zombie kill gives the pendant (with its sound).
        assert!(t.quest_page(1, npcs.praga).contains("30333-01.html"));
        assert!(t.quest_page(1, npcs.praga).contains("30333-02.html"));
        let mob = t.spawn(RUIN_ZOMBIE, 80);
        t.kill(1, mob);
        assert_eq!(t.count(1, PENDANT_OF_MOTHER), 1);
        assert!(t.take_writes().is_empty());

        // Praga takes the necklace and the pendant for his book: cond 2.
        assert!(t.quest_page(1, npcs.praga).contains("30333-03.html"));
        assert_eq!(
            (
                t.count(1, PENDANT_OF_MOTHER),
                t.count(1, NECKLACE_OF_MOTHER),
                t.count(1, BOOK_OF_PRAGA)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.praga).contains("30333-04.html"));

        // Zigaunt takes the three books and gives the second letter: cond 3.
        assert!(t.quest_page(1, npcs.zigaunt).contains("30022-08.html"));
        assert_eq!(
            (
                t.count(1, LETTER_OF_ORDER_1ST),
                t.count(1, LETTER_OF_ORDER_2ND),
                t.count(1, BOOK_OF_SIMPLON),
                t.count(1, BOOK_OF_VIVYAN),
                t.count(1, BOOK_OF_PRAGA)
            ),
            (0, 1, 0, 0, 0)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.zigaunt).contains("30022-07.html"));

        // Gallint has no lionel's book yet, Lionel gives it: cond 4, then Gallint's certificate: cond 5.
        assert!(t.quest_page(1, npcs.gallint).contains("30017-02.html"));
        assert!(t.quest_page(1, npcs.lionel).contains("30408-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.gallint).contains("30017-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Lionel makes the covenant from the certificate: cond 6; then he has nothing more to say.
        assert!(t.quest_page(1, npcs.lionel).contains("30408-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, npcs.lionel).contains("30408-05.html"));

        // Zigaunt's reward: adena, the mark, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.zigaunt).contains("30022-09.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, MARK_OF_FAITH), 1);
        assert_eq!(t.count(1, LEMONIELLS_COVENANT), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: Zigaunt says the mark is already here.
        assert!(t.quest_page(1, npcs.zigaunt).contains("30022-04.htm"));
    }

    #[test]
    fn a_ruin_zombie_gives_no_pendant_without_the_necklace() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::MAGE, vec![]);
        t.quest_page(1, npcs.zigaunt);
        t.event(1, NAME, "ACCEPT");
        t.take_writes();
        let mob = t.spawn(RUIN_ZOMBIE, 80);
        t.kill(1, mob);
        assert_eq!(t.count(1, PENDANT_OF_MOTHER), 0);
        assert!(t.take_writes().is_empty());
    }
}
