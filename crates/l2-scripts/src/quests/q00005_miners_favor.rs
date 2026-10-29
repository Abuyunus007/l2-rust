//! Quest 5: Miner's Favor (Bolter in Talking Island sends a miner to four NPCs for their goods).
// Java: dist/game/data/scripts/quests/Q00005_MinersFavor/Q00005_MinersFavor.java
//
// Pattern: a collection from several NPCs (`docs/design/scripts.md`, "Quest patterns"): each NPC
// gives its item once, `checkProgress` moves the quest to cond 2 when the five items are in, and
// the start NPC pays the reward (one-time, with the Newbie Guide hook). Brunon's pick comes in
// an event, and the socks it takes are the only ones this quest checks.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, Qs, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BOLTER: i32 = 30554;
const SHARI: i32 = 30517;
const GARITA: i32 = 30518;
const REED: i32 = 30520;
const BRUNON: i32 = 30526;

// Items
const BOLTERS_LIST: i32 = 1547;
const MINING_BOOTS: i32 = 1548;
const MINERS_PICK: i32 = 1549;
const BOOMBOOM_POWDER: i32 = 1550;
const REDSTONE_BEER: i32 = 1551;
const BOLTERS_SMELLY_SOCKS: i32 = 1552;
const NECKLACE: i32 = 906;

// Misc
const MIN_LEVEL: i32 = 2;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00005_MinersFavor`.
pub struct Q00005MinersFavor;

/// Java `checkProgress(player, qs)`: cond 2 when the list, the boots, the pick, the powder and
/// the beer are all in.
fn check_progress(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs) {
    let items = [
        BOLTERS_LIST,
        MINING_BOOTS,
        MINERS_PICK,
        BOOMBOOM_POWDER,
        REDSTONE_BEER,
    ];
    if ctx.has_all_quest_items(player, &items) {
        qs.set_cond(ctx, 2, true);
    }
}

/// Java `giveItem(player, qs, npcId, itemId)`: the NPC's item once, then the progress check.
fn give_item(ctx: &mut ScriptCtx<'_>, player: ObjectId, qs: Qs, npc_id: i32, item_id: i32) -> String {
    if !qs.is_started(ctx) {
        return ctx.get_no_quest_msg(player);
    } else if ctx.has_quest_items(player, item_id) {
        return format!("{npc_id}-02.html");
    }

    ctx.give_items(player, item_id, 1);
    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    check_progress(ctx, player, qs);
    format!("{npc_id}-01.html")
}

impl Script for Q00005MinersFavor {
    fn name(&self) -> &'static str {
        "Q00005_MinersFavor"
    }

    fn path(&self) -> &'static str {
        "quests/Q00005_MinersFavor"
    }

    fn id(&self) -> i32 {
        5
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BOLTER]);
        s.add_talk_id(&[BOLTER, SHARI, GARITA, REED, BRUNON]);
        s.register_quest_items(&[
            BOLTERS_LIST,
            MINING_BOOTS,
            MINERS_PICK,
            BOOMBOOM_POWDER,
            REDSTONE_BEER,
            BOLTERS_SMELLY_SOCKS,
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

        match event {
            "30554-03.htm" => {
                qs.start_quest(ctx);
                ctx.give_items(player, BOLTERS_LIST, 1);
                ctx.give_items(player, BOLTERS_SMELLY_SOCKS, 1);
            }
            "30526-02.html" => {
                if !ctx.has_quest_items(player, BOLTERS_SMELLY_SOCKS) {
                    return Some("30526-04.html".to_owned());
                }

                ctx.take_items(player, BOLTERS_SMELLY_SOCKS, -1);
                ctx.give_items(player, MINERS_PICK, 1);
                check_progress(ctx, player, qs);
            }
            "30554-05.html" => {}
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            BOLTER => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30554-02.htm"
                    } else {
                        "30554-01.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30554-04.html".to_owned();
                    } else {
                        ctx.give_items(player, NECKLACE, 1);
                        ctx.add_exp_and_sp(player, 5672, 446);
                        ctx.give_adena(player, 2466, true);
                        qs.exit_quest(ctx, false, true);

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

                        htmltext = "30554-06.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            BRUNON => {
                if qs.is_started(ctx) {
                    htmltext = if ctx.has_quest_items(player, MINERS_PICK) {
                        "30526-03.html"
                    } else {
                        "30526-01.html"
                    }
                    .to_owned();
                }
            }
            REED => htmltext = give_item(ctx, player, qs, npc.id, REDSTONE_BEER),
            SHARI => htmltext = give_item(ctx, player, qs, npc.id, BOOMBOOM_POWDER),
            GARITA => htmltext = give_item(ctx, player, qs, npc.id, MINING_BOOTS),
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00005_MinersFavor";

    const NPCS: &str = r#"
        <npc id="30554" type="Folk" level="70" name="Bolter"/>
        <npc id="30517" type="Folk" level="70" name="Shari"/>
        <npc id="30518" type="Folk" level="70" name="Garita"/>
        <npc id="30520" type="Folk" level="70" name="Reed"/>
        <npc id="30526" type="Folk" level="70" name="Brunon"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="906" type="Armor" name="Necklace of Knowledge"><set name="bodypart" val="neck"/></item>
        <item id="1547" type="EtcItem" name="Bolter's List"><set name="is_questitem" val="true"/></item>
        <item id="1548" type="EtcItem" name="Mining Boots"><set name="is_questitem" val="true"/></item>
        <item id="1549" type="EtcItem" name="Miner's Pick"><set name="is_questitem" val="true"/></item>
        <item id="1550" type="EtcItem" name="Boomboom Powder"><set name="is_questitem" val="true"/></item>
        <item id="1551" type="EtcItem" name="Redstone Beer"><set name="is_questitem" val="true"/></item>
        <item id="1552" type="EtcItem" name="Bolter's Smelly Socks"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        bolter: Oid,
        shari: Oid,
        garita: Oid,
        reed: Oid,
        brunon: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00005MinersFavor);
        let pages = Pages::of(&Q00005MinersFavor)
            .page("30554-02.htm", &["30554-03.htm"])
            .page("30554-04.html", &["30554-05.html"])
            .page("30526-01.html", &["30526-02.html"])
            .plain(&[
                "30554-01.html",
                "30554-03.htm",
                "30554-05.html",
                "30554-06.html",
                "30526-02.html",
                "30526-03.html",
                "30526-04.html",
                "30517-01.html",
                "30517-02.html",
                "30518-01.html",
                "30518-02.html",
                "30520-01.html",
                "30520-02.html",
            ])
            .npcs(&[BOLTER, SHARI, GARITA, REED, BRUNON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            bolter: t.spawn(BOLTER, 20),
            shari: t.spawn(SHARI, 40),
            garita: t.spawn(GARITA, 60),
            reed: t.spawn(REED, 80),
            brunon: t.spawn(BRUNON, 100),
        };
        (t, npcs)
    }

    #[test]
    fn a_level_one_character_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 1, vec![]);
        assert!(t.quest_page(1, npcs.bolter).contains("30554-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_five_items_then_the_necklace() {
        let (mut t, npcs) = setup();
        t.enter(1, 2, vec![]);
        assert!(t.quest_page(1, npcs.bolter).contains("30554-02.htm"));
        assert!(t.event(1, NAME, "30554-03.htm").contains("30554-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(
            (t.count(1, BOLTERS_LIST), t.count(1, BOLTERS_SMELLY_SOCKS)),
            (1, 1)
        );

        // Bolter waits until the items are in.
        assert!(t.quest_page(1, npcs.bolter).contains("30554-04.html"));

        // Brunon: the socks give the pick; the list is not complete yet, so no cond.
        assert!(t.quest_page(1, npcs.brunon).contains("30526-01.html"));
        assert!(t.event(1, NAME, "30526-02.html").contains("30526-02.html"));
        assert_eq!(
            (t.count(1, BOLTERS_SMELLY_SOCKS), t.count(1, MINERS_PICK)),
            (0, 1)
        );
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, npcs.brunon).contains("30526-03.html"));

        // Shari, Reed and Garita; Garita's boots are the last item (cond 2).
        assert!(t.quest_page(1, npcs.shari).contains("30517-01.html"));
        assert!(t.quest_page(1, npcs.reed).contains("30520-01.html"));
        assert!(t.quest_page(1, npcs.shari).contains("30517-02.html"));
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, npcs.garita).contains("30518-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Bolter: the necklace, the exp and the adena, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.bolter).contains("30554-06.html"));
        assert_eq!(t.count(1, NECKLACE), 1);
        assert_eq!(t.count(1, 57), 2466);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.bolter)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_second_visit_to_an_npc_shows_its_second_page() {
        let (mut t, npcs) = setup();
        t.enter(1, 2, vec![]);
        t.quest_page(1, npcs.bolter);
        t.event(1, NAME, "30554-03.htm");
        assert!(t.quest_page(1, npcs.reed).contains("30520-01.html"));
        assert!(t.quest_page(1, npcs.reed).contains("30520-02.html"));
        assert_eq!(t.count(1, REDSTONE_BEER), 1);
    }
}
