//! Quest 411: Path of the Assassin (Triskel, Guard Leikan and Arkenia).
// Java: dist/game/data/scripts/quests/Q00411_PathOfTheAssassin/Q00411_PathOfTheAssassin.java
//
// Pattern: a delivery chain through three NPCs with a kill step in the middle (moonstone beast
// molars, then the quest monster Calpico's tears), as in `docs/design/scripts.md`, "Quest patterns".

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const TRISKEL: i32 = 30416;
const GUARD_LEIKAN: i32 = 30382;
const ARKENIA: i32 = 30419;

// Items
const SHILENS_CALL: i32 = 1245;
const ARKENIAS_LETTER: i32 = 1246;
const LEIKANS_NOTE: i32 = 1247;
const MOONSTONE_BEASTS_MOLAR: i32 = 1248;
const SHILENS_TEARS: i32 = 1250;
const ARKENIAS_RECOMMENDATION: i32 = 1251;

// Reward
const IRON_HEART: i32 = 1252;

// Monsters
const MOONSTONE_BEAST: i32 = 20369;
/// Quest monster.
const CALPICO: i32 = 27036;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00411_PathOfTheAssassin`.
pub struct Q00411PathOfTheAssassin;

impl Script for Q00411PathOfTheAssassin {
    fn name(&self) -> &'static str {
        "Q00411_PathOfTheAssassin"
    }

    fn path(&self) -> &'static str {
        "quests/Q00411_PathOfTheAssassin"
    }

    fn id(&self) -> i32 {
        411
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TRISKEL]);
        s.add_talk_id(&[TRISKEL, GUARD_LEIKAN, ARKENIA]);
        s.add_kill_id(&[MOONSTONE_BEAST, CALPICO]);
        s.register_quest_items(&[
            SHILENS_CALL,
            ARKENIAS_LETTER,
            LEIKANS_NOTE,
            MOONSTONE_BEASTS_MOLAR,
            SHILENS_TEARS,
            ARKENIAS_RECOMMENDATION,
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
                if ctx.player_class_id(player) == player_class::DARK_FIGHTER {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_quest_items(player, IRON_HEART) {
                            htmltext = Some("30416-04.htm".to_owned());
                        } else {
                            qs.start_quest(ctx);
                            ctx.give_items(player, SHILENS_CALL, 1);
                            htmltext = Some("30416-05.htm".to_owned());
                        }
                    } else {
                        htmltext = Some("30416-03.htm".to_owned());
                    }
                } else if ctx.player_class_id(player) == player_class::ASSASSIN {
                    htmltext = Some("30416-02a.htm".to_owned());
                } else {
                    htmltext = Some("30416-02.htm".to_owned());
                }
            }
            "30382-02.html" | "30382-04.html" => {
                htmltext = Some(event.to_owned());
            }
            "30382-03.html" => {
                if ctx.has_quest_items(player, ARKENIAS_LETTER) {
                    ctx.take_items(player, ARKENIAS_LETTER, 1);
                    ctx.give_items(player, LEIKANS_NOTE, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30419-02.html" | "30419-03.html" | "30419-04.html" | "30419-06.html" => {
                htmltext = Some(event.to_owned());
            }
            "30419-05.html" => {
                if ctx.has_quest_items(player, SHILENS_CALL) {
                    ctx.take_items(player, SHILENS_CALL, 1);
                    ctx.give_items(player, ARKENIAS_LETTER, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
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
            MOONSTONE_BEAST => {
                if ctx.has_quest_items(killer, LEIKANS_NOTE)
                    && ctx.get_quest_items_count(killer, MOONSTONE_BEASTS_MOLAR) < 10
                {
                    ctx.give_items(killer, MOONSTONE_BEASTS_MOLAR, 1);
                    if ctx.get_quest_items_count(killer, MOONSTONE_BEASTS_MOLAR) == 10 {
                        qs.set_cond(ctx, 4, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            CALPICO => {
                if !ctx.has_quest_items(killer, SHILENS_TEARS) {
                    ctx.give_items(killer, SHILENS_TEARS, 1);
                    qs.set_cond(ctx, 6, true);
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
            if npc.id == TRISKEL {
                htmltext = if !ctx.has_quest_items(player, IRON_HEART) {
                    "30416-01.htm"
                } else {
                    "30416-04.htm"
                }
                .to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                TRISKEL => {
                    if !ctx.has_at_least_one_quest_item(
                        player,
                        &[ARKENIAS_LETTER, LEIKANS_NOTE, SHILENS_TEARS, IRON_HEART],
                    ) && ctx.has_quest_items(player, ARKENIAS_RECOMMENDATION)
                    {
                        ctx.give_adena(player, 163_800, true);
                        ctx.give_items(player, IRON_HEART, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 35_830);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 35_830);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 42_528);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30416-06.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) && ctx.has_quest_items(player, ARKENIAS_LETTER)
                    {
                        htmltext = "30416-07.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) && ctx.has_quest_items(player, LEIKANS_NOTE)
                    {
                        htmltext = "30416-08.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) {
                        htmltext = "30416-09.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            LEIKANS_NOTE,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) && ctx.has_quest_items(player, SHILENS_TEARS)
                    {
                        htmltext = "30416-10.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                        ],
                    ) && ctx.has_quest_items(player, SHILENS_CALL)
                    {
                        htmltext = "30416-11.html".to_owned();
                    }
                }
                GUARD_LEIKAN => {
                    if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                            MOONSTONE_BEASTS_MOLAR,
                        ],
                    ) && ctx.has_quest_items(player, ARKENIAS_LETTER)
                    {
                        htmltext = "30382-01.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                            MOONSTONE_BEASTS_MOLAR,
                        ],
                    ) && ctx.has_quest_items(player, LEIKANS_NOTE)
                    {
                        htmltext = "30382-05.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) && ctx.has_quest_items(player, LEIKANS_NOTE)
                    {
                        if ctx.has_quest_items(player, MOONSTONE_BEASTS_MOLAR)
                            && ctx.get_quest_items_count(player, MOONSTONE_BEASTS_MOLAR) < 10
                        {
                            htmltext = "30382-06.html".to_owned();
                        } else {
                            ctx.take_items(player, LEIKANS_NOTE, 1);
                            ctx.take_items(player, MOONSTONE_BEASTS_MOLAR, -1);
                            qs.set_cond(ctx, 5, true);
                            htmltext = "30382-07.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, SHILENS_TEARS) {
                        htmltext = "30382-08.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                            MOONSTONE_BEASTS_MOLAR,
                        ],
                    ) {
                        htmltext = "30382-09.html".to_owned();
                    }
                }
                ARKENIA => {
                    if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                        ],
                    ) && ctx.has_quest_items(player, SHILENS_CALL)
                    {
                        htmltext = "30419-01.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) && ctx.has_quest_items(player, ARKENIAS_LETTER)
                    {
                        htmltext = "30419-07.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            LEIKANS_NOTE,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) && ctx.has_quest_items(player, SHILENS_TEARS)
                    {
                        ctx.take_items(player, SHILENS_TEARS, 1);
                        ctx.give_items(player, ARKENIAS_RECOMMENDATION, 1);
                        qs.set_cond(ctx, 7, true);
                        htmltext = "30419-08.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) && ctx.has_quest_items(player, ARKENIAS_RECOMMENDATION)
                    {
                        htmltext = "30419-09.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) && ctx.has_quest_items(player, LEIKANS_NOTE)
                    {
                        htmltext = "30419-10.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            ARKENIAS_LETTER,
                            LEIKANS_NOTE,
                            SHILENS_TEARS,
                            ARKENIAS_RECOMMENDATION,
                            IRON_HEART,
                            SHILENS_CALL,
                        ],
                    ) {
                        htmltext = "30419-11.html".to_owned();
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

    const NAME: &str = "Q00411_PathOfTheAssassin";

    const NPCS: &str = r#"
        <npc id="30416" type="Folk" level="70" name="Triskel"/>
        <npc id="30382" type="Folk" level="70" name="Guard Leikan"/>
        <npc id="30419" type="Folk" level="70" name="Arkenia"/>
        <npc id="20369" type="Monster" level="20" name="Moonstone Beast"/>
        <npc id="27036" type="Monster" level="30" name="Calpico"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1245" type="EtcItem" name="Shilen's Call"><set name="is_questitem" val="true"/></item>
        <item id="1246" type="EtcItem" name="Arkenia's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1247" type="EtcItem" name="Leikan's Note"><set name="is_questitem" val="true"/></item>
        <item id="1248" type="EtcItem" name="Moonstone Beast's Molar"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1250" type="EtcItem" name="Shilen's Tears"><set name="is_questitem" val="true"/></item>
        <item id="1251" type="EtcItem" name="Arkenia's Recommendation"><set name="is_questitem" val="true"/></item>
        <item id="1252" type="EtcItem" name="Iron Heart"/>"#;

    struct Npcs {
        triskel: Oid,
        leikan: Oid,
        arkenia: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00411PathOfTheAssassin)
            .page("30416-01.htm", &["ACCEPT"])
            .page("30382-01.html", &["30382-02.html"])
            .page("30382-02.html", &["30382-03.html", "30382-04.html"])
            .page("30419-01.html", &["30419-02.html", "30419-03.html"])
            .page(
                "30419-02.html",
                &["30419-04.html", "30419-05.html", "30419-06.html"],
            )
            .page("30419-04.html", &["30419-05.html", "30419-06.html"])
            .plain(&[
                "30382-03.html",
                "30382-04.html",
                "30382-05.html",
                "30382-06.html",
                "30382-07.html",
                "30382-08.html",
                "30382-09.html",
                "30416-02.htm",
                "30416-02a.htm",
                "30416-03.htm",
                "30416-04.htm",
                "30416-05.htm",
                "30416-06.html",
                "30416-07.html",
                "30416-08.html",
                "30416-09.html",
                "30416-10.html",
                "30416-11.html",
                "30419-03.html",
                "30419-05.html",
                "30419-06.html",
                "30419-07.html",
                "30419-08.html",
                "30419-09.html",
                "30419-10.html",
                "30419-11.html",
            ])
            .npcs(&[TRISKEL, GUARD_LEIKAN, ARKENIA])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00411PathOfTheAssassin);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            triskel: t.spawn(TRISKEL, 20),
            leikan: t.spawn(GUARD_LEIKAN, 40),
            arkenia: t.spawn(ARKENIA, 60),
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
    fn the_accept_page_depends_on_class_level_and_heart() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.triskel).contains("30416-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30416-03.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::DARK_ELF, player_class::ASSASSIN, vec![]);
        t.quest_page(1, npcs.triskel);
        assert!(t.event(1, NAME, "ACCEPT").contains("30416-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::DARK_ELF, player_class::DARK_MAGE, vec![]);
        t.quest_page(1, npcs.triskel);
        assert!(t.event(1, NAME, "ACCEPT").contains("30416-02.htm"));

        let (mut t, npcs) = setup();
        // With the heart, Triskel's talk already shows the page (no link to ACCEPT on it).
        t.enter_as(
            1,
            30,
            race::DARK_ELF,
            player_class::DARK_FIGHTER,
            vec![item(5000, 1252, 1)],
        );
        assert!(t.quest_page(1, npcs.triskel).contains("30416-04.htm"));
    }

    #[test]
    fn the_whole_path_from_the_call_to_the_iron_heart() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::DARK_ELF, player_class::DARK_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.triskel).contains("30416-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30416-05.htm"));
        assert_eq!(t.count(1, SHILENS_CALL), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.triskel).contains("30416-11.html"));

        // Arkenia takes the call and gives her letter: cond 2.
        assert!(t.quest_page(1, npcs.arkenia).contains("30419-01.html"));
        assert!(t.event(1, NAME, "30419-02.html").contains("30419-02.html"));
        assert!(t.event(1, NAME, "30419-05.html").contains("30419-05.html"));
        assert_eq!((t.count(1, SHILENS_CALL), t.count(1, ARKENIAS_LETTER)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Leikan takes the letter and gives his note: cond 3.
        assert!(t.quest_page(1, npcs.leikan).contains("30382-01.html"));
        assert!(t.event(1, NAME, "30382-02.html").contains("30382-02.html"));
        assert!(t.event(1, NAME, "30382-03.html").contains("30382-03.html"));
        assert_eq!((t.count(1, ARKENIAS_LETTER), t.count(1, LEIKANS_NOTE)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.triskel).contains("30416-08.html"));
        assert!(t.quest_page(1, npcs.leikan).contains("30382-05.html"));

        // Ten moonstone beast molars; the tenth sets cond 4.
        hunt(&mut t, MOONSTONE_BEAST, MOONSTONE_BEASTS_MOLAR, 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Leikan takes the note and the molars: cond 5.
        assert!(t.quest_page(1, npcs.leikan).contains("30382-07.html"));
        assert_eq!(
            (t.count(1, LEIKANS_NOTE), t.count(1, MOONSTONE_BEASTS_MOLAR)),
            (0, 0)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // Calpico's tears: cond 6.
        let calpico = t.spawn(CALPICO, 80);
        t.kill(1, calpico);
        assert_eq!(t.count(1, SHILENS_TEARS), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);
        assert!(t.quest_page(1, npcs.leikan).contains("30382-08.html"));

        // Arkenia takes the tears and gives her recommendation: cond 7.
        assert!(t.quest_page(1, npcs.arkenia).contains("30419-08.html"));
        assert_eq!(
            (t.count(1, SHILENS_TEARS), t.count(1, ARKENIAS_RECOMMENDATION)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);
        assert!(t.quest_page(1, npcs.arkenia).contains("30419-09.html"));

        // Triskel's reward: adena, the iron heart, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.triskel).contains("30416-06.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, IRON_HEART), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: Triskel says the heart is already here.
        assert!(t.quest_page(1, npcs.triskel).contains("30416-04.htm"));
    }
}
