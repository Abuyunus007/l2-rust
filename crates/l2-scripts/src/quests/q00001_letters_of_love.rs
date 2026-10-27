//! Quest 1: Letters of Love (Darin, Roxxy and Baulro in Talking Island).
// Java: dist/game/data/scripts/quests/Q00001_LettersOfLove/Q00001_LettersOfLove.java
//
// The reference port of a quest for the batch workers (`docs/design/scripts.md`): constants as
// in Java, `setup` = the constructor, `on_event` / `on_talk` line by line.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const DARIN: i32 = 30048;
const ROXXY: i32 = 30006;
const BAULRO: i32 = 30033;

// Items
const DARINS_LETTER: i32 = 687;
const ROXXYS_KERCHIEF: i32 = 688;
const DARINS_RECEIPT: i32 = 1079;
const BAULROS_POTION: i32 = 1080;
const NECKLACE_OF_KNOWLEDGE: i32 = 906;

// Misc
const MIN_LEVEL: i32 = 2;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00001_LettersOfLove`.
pub struct Q00001LettersOfLove;

impl Script for Q00001LettersOfLove {
    fn name(&self) -> &'static str {
        "Q00001_LettersOfLove"
    }

    fn path(&self) -> &'static str {
        "quests/Q00001_LettersOfLove"
    }

    fn id(&self) -> i32 {
        1
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DARIN]);
        s.add_talk_id(&[DARIN, ROXXY, BAULRO]);
        s.register_quest_items(&[DARINS_LETTER, ROXXYS_KERCHIEF, DARINS_RECEIPT, BAULROS_POTION]);
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
            "30048-03.html" | "30048-04.html" | "30048-05.html" => {
                htmltext = Some(event.to_owned());
            }
            "30048-06.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    ctx.give_items(player, DARINS_LETTER, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let page = |name: &str| name.to_owned();
        match qs.state(ctx) {
            CREATED => {
                htmltext = page(if ctx.player_level(player) < MIN_LEVEL {
                    "30048-01.html"
                } else {
                    "30048-02.html"
                });
            }
            STARTED => match qs.get_cond(ctx) {
                1 => match npc.id {
                    DARIN => htmltext = page("30048-07.html"),
                    ROXXY => {
                        if ctx.has_quest_items(player, DARINS_LETTER) && !ctx.has_quest_items(player, ROXXYS_KERCHIEF) {
                            ctx.take_items(player, DARINS_LETTER, -1);
                            ctx.give_items(player, ROXXYS_KERCHIEF, 1);
                            qs.set_cond(ctx, 2, true);
                            htmltext = page("30006-01.html");
                        }
                    }
                    _ => {}
                },
                2 => match npc.id {
                    DARIN => {
                        if ctx.has_quest_items(player, ROXXYS_KERCHIEF) {
                            ctx.take_items(player, ROXXYS_KERCHIEF, -1);
                            ctx.give_items(player, DARINS_RECEIPT, 1);
                            qs.set_cond(ctx, 3, true);
                            htmltext = page("30048-08.html");
                        }
                    }
                    ROXXY => {
                        if ctx.has_quest_items(player, ROXXYS_KERCHIEF) {
                            htmltext = page("30006-02.html");
                        }
                    }
                    _ => {}
                },
                3 => match npc.id {
                    DARIN => {
                        if ctx.has_quest_items(player, DARINS_RECEIPT) || !ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30048-09.html");
                        }
                    }
                    ROXXY => {
                        if ctx.has_quest_items(player, DARINS_RECEIPT) || ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30006-03.html");
                        }
                    }
                    BAULRO => {
                        if ctx.has_quest_items(player, DARINS_RECEIPT) {
                            ctx.take_items(player, DARINS_RECEIPT, -1);
                            ctx.give_items(player, BAULROS_POTION, 1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = page("30033-01.html");
                        } else if ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30033-02.html");
                        }
                    }
                    _ => {}
                },
                4 => match npc.id {
                    DARIN => {
                        // Newbie Guide (only once `ai/others/NewbieGuide` is ported; Java skips
                        // the block the same way when the script is missing).
                        if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                            if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, 1);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::DeliveryDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            } else if ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 10 != 1 {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 1);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::DeliveryDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            }
                        }

                        ctx.give_items(player, NECKLACE_OF_KNOWLEDGE, 1);
                        ctx.add_exp_and_sp(player, 5672, 446);
                        ctx.give_adena(player, 2466, false);
                        qs.exit_quest(ctx, false, true);
                        htmltext = page("30048-10.html");
                    }
                    BAULRO => {
                        if ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30033-02.html");
                        }
                    }
                    ROXXY => {
                        if ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30006-03.html");
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to};

    const NPCS: &str = r#"
        <npc id="30048" type="Folk" level="70" name="Darin"/>
        <npc id="30006" type="Folk" level="70" name="Roxxy"/>
        <npc id="30033" type="Folk" level="70" name="Baulro"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="687" type="EtcItem" name="Darin's Letter"><set name="is_questitem" val="true"/></item>
        <item id="688" type="EtcItem" name="Roxxy's Kerchief"><set name="is_questitem" val="true"/></item>
        <item id="1079" type="EtcItem" name="Darin's Receipt"><set name="is_questitem" val="true"/></item>
        <item id="1080" type="EtcItem" name="Baulro's Potion"><set name="is_questitem" val="true"/></item>
        <item id="906" type="Armor" name="Necklace of Knowledge"><set name="bodypart" val="neck"/></item>"#;

    const DIR: &str = "data/scripts/quests/Q00001_LettersOfLove";

    /// Stand-ins for the quest's pages: the file name, and the links of the real page.
    fn pages() -> Vec<(String, String)> {
        let link = |page: &str| format!(r#"<a action="bypass Script Q00001_LettersOfLove {page}">x</a>"#);
        let mut pages: Vec<(String, String)> = [
            "30048-01.html",
            "30048-07.html",
            "30048-08.html",
            "30048-09.html",
            "30048-10.html",
            "30006-01.html",
            "30006-02.html",
            "30006-03.html",
            "30033-01.html",
            "30033-02.html",
        ]
        .iter()
        .map(|p| (format!("{DIR}/{p}"), format!("<html><body>{p}</body></html>")))
        .collect();
        for (page, links) in [
            ("30048-02.html", link("30048-03.html")),
            ("30048-03.html", link("30048-04.html") + &link("30048-05.html")),
            ("30048-04.html", link("30048-06.htm")),
            ("30048-05.html", link("30048-06.htm")),
            ("30048-06.htm", String::new()),
        ] {
            pages.push((format!("{DIR}/{page}"), format!("<html><body>{page}{links}</body></html>")));
        }
        for npc in [DARIN, ROXXY, BAULRO] {
            pages.push((
                format!("data/html/default/{npc}.htm"),
                format!(r#"<html><body>npc {npc} <a action="bypass Script">Quest</a></body></html>"#),
            ));
        }
        pages.push((
            "data/html/alreadycompleted.htm".to_owned(),
            "<html><body>This quest has already been completed.</body></html>".to_owned(),
        ));
        pages
    }

    /// Java `ai.others.NewbieGuide` as far as this quest needs it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            "NewbieGuide"
        }
        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        darin: Oid,
        roxxy: Oid,
        baulro: Oid,
    }

    fn setup(with_guide: bool) -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        if with_guide {
            registry.add(NewbieGuide);
        }
        registry.add(Q00001LettersOfLove);
        let pages = pages();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        let npcs = Npcs {
            darin: t.spawn(DARIN, 20),
            roxxy: t.spawn(ROXXY, 40),
            baulro: t.spawn(BAULRO, 60),
        };
        (t, npcs)
    }

    /// Talks to the NPC, opens its quest link and returns the last page it shows.
    fn quest_page(t: &mut ScriptTest, npc: Oid) -> String {
        t.talk(1, npc);
        t.bypass(1, "Script");
        let out = t.take();
        pages_to(&out, 1).pop().unwrap_or_default()
    }

    fn insert(var: &str, value: &str) -> QuestWrite {
        QuestWrite::InsertVar {
            char_id: 1,
            name: "Q00001_LettersOfLove".to_owned(),
            var: var.to_owned(),
            value: value.to_owned(),
        }
    }

    fn update(var: &str, value: &str) -> QuestWrite {
        QuestWrite::UpdateVar {
            char_id: 1,
            name: "Q00001_LettersOfLove".to_owned(),
            var: var.to_owned(),
            value: value.to_owned(),
        }
    }

    #[test]
    fn a_level_one_character_is_sent_away() {
        let (mut t, npcs) = setup(false);
        t.enter(1, 1, vec![]);
        assert!(quest_page(&mut t, npcs.darin).contains("30048-01.html"));
        // `getQuestState(player, true)` made a state, but nothing is written.
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_darin_to_the_necklace() {
        let (mut t, npcs) = setup(false);
        t.enter(1, 2, vec![]);
        t.take();
        assert!(quest_page(&mut t, npcs.darin).contains("30048-02.html"));
        for event in ["30048-03.html", "30048-04.html"] {
            t.bypass(1, &format!("Script Q00001_LettersOfLove {event}"));
            assert!(pages_to(&t.take(), 1).pop().unwrap().contains(event), "{event}");
        }

        // Accept: `startQuest()` and the letter.
        t.bypass(1, "Script Q00001_LettersOfLove 30048-06.htm");
        let out = t.take();
        let names = names_to(&out, 1);
        // `set("cond", "1")` (QuestList, ExShowQuestMark), `setState(STARTED)` (QuestList), the
        // accept sound, then the letter and the `.htm` page as the quest window.
        let first = |name: &str| names.iter().position(|n| n == name).unwrap();
        assert!(first("QUEST_LIST") < first("EX_SHOW_QUEST_MARK"));
        assert!(first("EX_SHOW_QUEST_MARK") < first("PLAY_SOUND"));
        assert!(names.contains(&"EX_NPC_QUEST_HTML_MESSAGE".to_owned()));
        assert_eq!(t.take_writes(), [insert("cond", "1"), insert("<state>", "Started")]);
        assert_eq!(t.count(1, DARINS_LETTER), 1);
        let journal = t.world.quest_list(Oid(1));
        assert_eq!(journal.quests.len(), 1);
        assert_eq!((journal.quests[0].quest_id, journal.quests[0].status), (1, 1));

        // Darin waits for the answer.
        assert!(quest_page(&mut t, npcs.darin).contains("30048-07.html"));

        // Roxxy takes the letter and gives the kerchief.
        assert!(quest_page(&mut t, npcs.roxxy).contains("30006-01.html"));
        assert_eq!((t.count(1, DARINS_LETTER), t.count(1, ROXXYS_KERCHIEF)), (0, 1));
        assert_eq!(t.take_writes(), [update("cond", "2")]);
        assert!(quest_page(&mut t, npcs.roxxy).contains("30006-02.html"));

        // Darin gives the receipt, Baulro the potion.
        assert!(quest_page(&mut t, npcs.darin).contains("30048-08.html"));
        assert_eq!((t.count(1, ROXXYS_KERCHIEF), t.count(1, DARINS_RECEIPT)), (0, 1));
        assert!(quest_page(&mut t, npcs.baulro).contains("30033-01.html"));
        assert_eq!((t.count(1, DARINS_RECEIPT), t.count(1, BAULROS_POTION)), (0, 1));
        assert_eq!(t.take_writes(), [update("cond", "3"), update("cond", "4")]);
        assert!(quest_page(&mut t, npcs.baulro).contains("30033-02.html"));

        // Darin's reward: the necklace, the adena, `exitQuest(false, true)`.
        assert!(quest_page(&mut t, npcs.darin).contains("30048-10.html"));
        assert_eq!(t.count(1, NECKLACE_OF_KNOWLEDGE), 1);
        assert_eq!(t.count(1, 57), 2466);
        assert_eq!(t.count(1, BAULROS_POTION), 0);
        assert_eq!(
            t.take_writes(),
            [
                QuestWrite::DeleteQuest {
                    char_id: 1,
                    name: "Q00001_LettersOfLove".to_owned(),
                    repeatable: false,
                },
                update("<state>", "Completed"),
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Done: the "already completed" page.
        assert!(quest_page(&mut t, npcs.darin).contains("This quest has already been completed."));
    }

    #[test]
    fn the_newbie_guide_hears_about_the_delivery() {
        let (mut t, npcs) = setup(true);
        t.enter(1, 2, vec![]);
        quest_page(&mut t, npcs.darin);
        t.bypass(1, "Script Q00001_LettersOfLove 30048-03.html");
        t.bypass(1, "Script Q00001_LettersOfLove 30048-04.html");
        t.bypass(1, "Script Q00001_LettersOfLove 30048-06.htm");
        quest_page(&mut t, npcs.roxxy);
        quest_page(&mut t, npcs.darin);
        quest_page(&mut t, npcs.baulro);
        t.take_writes();
        t.talk(1, npcs.darin);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"EX_SHOW_SCREEN_MESSAGE".to_owned()));
        let guide = |var: &str, value: &str| QuestWrite::InsertVar {
            char_id: 1,
            name: "NewbieGuide".to_owned(),
            var: var.to_owned(),
            value: value.to_owned(),
        };
        let writes = t.take_writes();
        assert_eq!(writes[..2], [guide("NRmemo", "41"), guide("NRmemoState41", "1")]);
    }
}
