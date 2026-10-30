//! Quest 363: Sorrowful Sound of Flute (Nanarin in the Town of Gludin).
// Java: dist/game/data/scripts/quests/Q00363_SorrowfulSoundOfFlute/Q00363_SorrowfulSoundOfFlute.java
//
// Pattern: a repeatable delivery quest. The `ex` variable packs one "visited" digit per NPC
// (Poitan 10s, Holvas 100s, Ranspo 1000s, Aldo 10000s, Opix 100000s); the memo state is the stage.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ALDO: i32 = 30057;
const HOLVAS: i32 = 30058;
const POITAN: i32 = 30458;
const RANSPO: i32 = 30594;
const OPIX: i32 = 30595;
const NANARIN: i32 = 30956;
const BARBADO: i32 = 30959;

// Items
const EVENT_CLOTHES: i32 = 4318;
const NANARINS_FLUTE: i32 = 4319;
const SABRINS_BLACK_BEER: i32 = 4320;
const THEME_OF_SOLITUDE: i32 = 4420;

// Misc
const MIN_LEVEL: i32 = 15;

/// Java `Q00363_SorrowfulSoundOfFlute`.
pub struct Q00363SorrowfulSoundOfFlute;

/// One of the three random pages of a villager: `<npc>-01.html` .. `<npc>-03.html`.
fn random_page(ctx: &mut ScriptCtx<'_>, npc: i32) -> String {
    match ctx.get_random(3) {
        0 => format!("{npc}-01.html"),
        1 => format!("{npc}-02.html"),
        _ => format!("{npc}-03.html"),
    }
}

impl Script for Q00363SorrowfulSoundOfFlute {
    fn name(&self) -> &'static str {
        "Q00363_SorrowfulSoundOfFlute"
    }

    fn path(&self) -> &'static str {
        "quests/Q00363_SorrowfulSoundOfFlute"
    }

    fn id(&self) -> i32 {
        363
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NANARIN]);
        s.add_talk_id(&[NANARIN, POITAN, RANSPO, ALDO, HOLVAS, OPIX, BARBADO]);
        s.register_quest_items(&[EVENT_CLOTHES, NANARINS_FLUTE, SABRINS_BLACK_BEER]);
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
            "START" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 2);
                    htmltext = Some("30956-02.htm".to_owned());
                } else {
                    htmltext = Some("30956-03.htm".to_owned());
                }
            }
            "30956-05.html" => {
                ctx.give_items(player, EVENT_CLOTHES, 1);
                qs.set_memo_state(ctx, 4);
                qs.set_cond(ctx, 3, true);
                htmltext = Some(event.to_owned());
            }
            "30956-06.html" => {
                ctx.give_items(player, NANARINS_FLUTE, 1);
                qs.set_memo_state(ctx, 4);
                qs.set_cond(ctx, 3, true);
                htmltext = Some(event.to_owned());
            }
            "30956-07.html" => {
                ctx.give_items(player, SABRINS_BLACK_BEER, 1);
                qs.set_memo_state(ctx, 4);
                qs.set_cond(ctx, 3, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == NANARIN {
                htmltext = "30956-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                NANARIN => match qs.get_memo_state(ctx) {
                    2 => htmltext = "30956-04.html".to_owned(),
                    4 => htmltext = "30956-08.html".to_owned(),
                    5 => {
                        ctx.reward_items(player, THEME_OF_SOLITUDE, 1);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30956-09.html".to_owned();
                    }
                    6 => {
                        qs.exit_quest(ctx, true, false);
                        ctx.play_sound(player, QuestSound::ItemsoundQuestGiveup);
                        htmltext = "30956-10.html".to_owned();
                    }
                    _ => {}
                },
                POITAN => {
                    let ex = qs.get_int(ctx, "ex");
                    if qs.is_memo_state(ctx, 2) && (ex % 100) < 10 {
                        qs.set_int(ctx, "ex", ex + 11);
                        htmltext = random_page(ctx, POITAN);
                        qs.set_cond(ctx, 2, true);
                    } else if qs.get_memo_state(ctx) >= 2 && (ex % 100) >= 10 {
                        htmltext = "30458-04.html".to_owned();
                    }
                }
                RANSPO => {
                    let ex = qs.get_int(ctx, "ex");
                    if qs.is_memo_state(ctx, 2) && (ex % 10000) < 1000 {
                        qs.set_int(ctx, "ex", ex + 1001);
                        htmltext = random_page(ctx, RANSPO);
                        qs.set_cond(ctx, 2, true);
                    } else if qs.get_memo_state(ctx) >= 2 && (ex % 10000) >= 1000 {
                        htmltext = "30594-04.html".to_owned();
                    }
                }
                ALDO => {
                    let ex = qs.get_int(ctx, "ex");
                    if qs.is_memo_state(ctx, 2) && (ex % 100000) < 10000 {
                        qs.set_int(ctx, "ex", ex + 10001);
                        htmltext = random_page(ctx, ALDO);
                        qs.set_cond(ctx, 2, true);
                    } else if qs.get_memo_state(ctx) >= 2 && (ex % 100000) >= 10000 {
                        htmltext = "30057-04.html".to_owned();
                    }
                }
                HOLVAS => {
                    let ex = qs.get_int(ctx, "ex");
                    if qs.is_memo_state(ctx, 2) && (ex % 1000) < 100 {
                        qs.set_int(ctx, "ex", ex + 101);
                        htmltext = random_page(ctx, HOLVAS);
                        qs.set_cond(ctx, 2, true);
                    } else if qs.get_memo_state(ctx) >= 2 && (ex % 1000) >= 100 {
                        htmltext = "30058-04.html".to_owned();
                    }
                }
                OPIX => {
                    let ex = qs.get_int(ctx, "ex");
                    if qs.is_memo_state(ctx, 2) && ex < 100000 {
                        qs.set_int(ctx, "ex", ex + 100001);
                        htmltext = random_page(ctx, OPIX);
                        qs.set_cond(ctx, 2, true);
                    } else if qs.get_memo_state(ctx) >= 2 && ex >= 100000 {
                        htmltext = "30595-04.html".to_owned();
                    }
                }
                BARBADO => {
                    if qs.is_memo_state(ctx, 4) {
                        let ex = (qs.get_int(ctx, "ex") % 10) * 20;
                        if ctx.get_random(100) < ex {
                            if ctx.has_quest_items(player, EVENT_CLOTHES) {
                                ctx.take_items(player, EVENT_CLOTHES, -1);
                            } else if ctx.has_quest_items(player, NANARINS_FLUTE) {
                                ctx.take_items(player, NANARINS_FLUTE, -1);
                            } else if ctx.has_quest_items(player, SABRINS_BLACK_BEER) {
                                ctx.take_items(player, SABRINS_BLACK_BEER, -1);
                            }
                            qs.set_memo_state(ctx, 5);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30959-01.html".to_owned();
                        } else {
                            qs.set_memo_state(ctx, 6);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "30959-02.html".to_owned();
                        }
                    } else if qs.get_memo_state(ctx) >= 5 {
                        htmltext = "30959-03.html".to_owned();
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
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q00363_SorrowfulSoundOfFlute";

    const NPCS: &str = r#"
        <npc id="30057" type="Folk" level="50" name="Aldo"/>
        <npc id="30058" type="Folk" level="50" name="Holvas"/>
        <npc id="30458" type="Folk" level="50" name="Poitan"/>
        <npc id="30594" type="Folk" level="50" name="Ranspo"/>
        <npc id="30595" type="Folk" level="50" name="Opix"/>
        <npc id="30956" type="Folk" level="50" name="Nanarin"/>
        <npc id="30959" type="Folk" level="50" name="Barbado"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4318" type="EtcItem" name="Event Clothes"><set name="is_questitem" val="true"/></item>
        <item id="4319" type="EtcItem" name="Nanarin's Flute"><set name="is_questitem" val="true"/></item>
        <item id="4320" type="EtcItem" name="Sabrin's Black Beer"><set name="is_questitem" val="true"/></item>
        <item id="4420" type="EtcItem" name="Theme of Solitude"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00363SorrowfulSoundOfFlute);
        let pages = Pages::of(&Q00363SorrowfulSoundOfFlute)
            .page("30956-01.htm", &["START"])
            .plain(&[
                "30956-02.htm",
                "30956-03.htm",
                "30956-08.html",
                "30956-09.html",
                "30956-10.html",
                "30458-01.html",
                "30458-02.html",
                "30458-03.html",
                "30458-04.html",
                "30594-01.html",
                "30594-02.html",
                "30594-03.html",
                "30594-04.html",
                "30057-01.html",
                "30057-02.html",
                "30057-03.html",
                "30057-04.html",
                "30058-01.html",
                "30058-02.html",
                "30058-03.html",
                "30058-04.html",
                "30595-01.html",
                "30595-02.html",
                "30595-03.html",
                "30595-04.html",
                "30959-01.html",
                "30959-02.html",
                "30959-03.html",
            ])
            .page(
                "30956-04.html",
                &["30956-05.html", "30956-06.html", "30956-07.html"],
            )
            .plain(&["30956-05.html", "30956-06.html", "30956-07.html"])
            .npcs(&[ALDO, HOLVAS, POITAN, RANSPO, OPIX, NANARIN, BARBADO]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let nanarin = t.spawn(NANARIN, 20);
        let poitan = t.spawn(POITAN, 40);
        let barbado = t.spawn(BARBADO, 60);
        (t, nanarin, poitan, barbado)
    }

    #[test]
    fn a_level_14_character_is_sent_away() {
        let (mut t, nanarin, ..) = setup();
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, nanarin).contains("30956-01.htm"));
        assert!(t.event(1, NAME, "START").contains("30956-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn start_visit_item_and_barbado_answers() {
        let (mut t, nanarin, poitan, barbado) = setup();
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, nanarin).contains("30956-01.htm"));
        assert!(t.event(1, NAME, "START").contains("30956-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "2"),
            ]
        );

        // Poitan: ex 0 -> 11, cond 2; a second talk gives the "already visited" page.
        let page = t.quest_page(1, poitan);
        assert!(page.contains("30458-0"), "{page}");
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "ex", "11"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, poitan).contains("30458-04.html"));
        assert!(t.take_writes().is_empty());

        // The clothes: memo 4, cond 3.
        assert!(t.quest_page(1, nanarin).contains("30956-04.html"));
        assert!(t.event(1, NAME, "30956-05.html").contains("30956-05.html"));
        assert_eq!(t.count(1, EVENT_CLOTHES), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "3")
            ]
        );

        // Barbado: ex % 10 = 1, so a 20 % chance. Both answers write memo state and cond 4.
        let page = t.quest_page(1, barbado);
        let won = page.contains("30959-01.html");
        assert!(won || page.contains("30959-02.html"), "{page}");
        let writes = t.take_writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0], rows::update(NAME, "memoState", if won { "5" } else { "6" }));
        assert_eq!(writes[1], rows::update(NAME, "cond", "4"));
        assert_eq!(t.count(1, EVENT_CLOTHES), if won { 0 } else { 1 });
    }

    fn started_with(memo: &str) -> l2_world::PlayerData {
        let mut data = player_data(1, 20, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![
                (NAME.to_owned(), "cond".to_owned(), Some("4".to_owned())),
                (NAME.to_owned(), "memoState".to_owned(), Some(memo.to_owned())),
            ],
        };
        data
    }

    #[test]
    fn nanarin_pays_the_theme_at_memo_5() {
        let (mut t, nanarin, ..) = setup();
        t.enter_with(started_with("5"));
        assert!(t.quest_page(1, nanarin).contains("30956-09.html"));
        assert_eq!(t.count(1, THEME_OF_SOLITUDE), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }

    #[test]
    fn nanarin_gives_up_at_memo_6() {
        let (mut t, nanarin, ..) = setup();
        t.enter_with(started_with("6"));
        assert!(t.quest_page(1, nanarin).contains("30956-10.html"));
        assert_eq!(t.count(1, THEME_OF_SOLITUDE), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
