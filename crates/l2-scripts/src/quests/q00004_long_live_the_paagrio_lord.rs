//! Quest 4: Long Live the Pa'agrio Lord (Nakusin in the Orc Village sends an orc to six NPCs for six items).
// Java: dist/game/data/scripts/quests/Q00004_LongLiveThePaagrioLord/Q00004_LongLiveThePaagrioLord.java
//
// Pattern: a collection from several NPCs (`docs/design/scripts.md`, "Quest patterns"): each NPC
// gives its item once, and the quest moves to cond 2 when all six are in. The start NPC pays the
// reward (one-time, with the Newbie Guide hook).

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KUNAI: i32 = 30559;
const USKA: i32 = 30560;
const GROOKIN: i32 = 30562;
const VARKEES: i32 = 30566;
const NAKUSIN: i32 = 30578;
const HESTUI: i32 = 30585;
const URUTU: i32 = 30587;

// Items
const CLUB: i32 = 4;
const HONEY_KHANDAR: i32 = 1541;
const BEAR_FUR_CLOAK: i32 = 1542;
const BLOODY_AXE: i32 = 1543;
const ANCESTOR_SKULL: i32 = 1544;
const SPIDER_DUST: i32 = 1545;
const DEEP_SEA_ORB: i32 = 1546;

// Misc
const MIN_LEVEL: i32 = 2;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Every quest item of this quest (Java `getRegisteredItemIds()`).
const REGISTERED: [i32; 6] = [
    HONEY_KHANDAR,
    BEAR_FUR_CLOAK,
    BLOODY_AXE,
    ANCESTOR_SKULL,
    SPIDER_DUST,
    DEEP_SEA_ORB,
];

/// Java `Q00004_LongLiveThePaagrioLord`.
pub struct Q00004LongLiveThePaagrioLord;

impl Q00004LongLiveThePaagrioLord {
    /// Java `giveItem(player, qs, npcId, itemId, items...)`: the NPC's item once, then cond 2
    /// when every quest item is in. Returns the NPC's page.
    fn give_item(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        qs: l2_world::script::Qs,
        npc_id: i32,
        item_id: i32,
    ) -> String {
        if !qs.is_started(ctx) {
            return ctx.get_no_quest_msg(player);
        } else if ctx.has_quest_items(player, item_id) {
            return format!("{npc_id}-02.html");
        }

        ctx.give_items(player, item_id, 1);
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        if ctx.has_all_quest_items(player, &REGISTERED) {
            qs.set_cond(ctx, 2, true);
        }

        format!("{npc_id}-01.html")
    }
}

impl Script for Q00004LongLiveThePaagrioLord {
    fn name(&self) -> &'static str {
        "Q00004_LongLiveThePaagrioLord"
    }

    fn path(&self) -> &'static str {
        "quests/Q00004_LongLiveThePaagrioLord"
    }

    fn id(&self) -> i32 {
        4
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NAKUSIN]);
        s.add_talk_id(&[NAKUSIN, VARKEES, URUTU, HESTUI, KUNAI, USKA, GROOKIN]);
        s.register_quest_items(&REGISTERED);
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
            "30578-03.htm" => qs.start_quest(ctx),
            "30578-05.html" => {}
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            NAKUSIN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) != race::ORC {
                        "30578-00.htm"
                    } else if ctx.player_level(player) >= MIN_LEVEL {
                        "30578-02.htm"
                    } else {
                        "30578-01.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30578-04.html".to_owned();
                    } else {
                        ctx.give_items(player, CLUB, 1);
                        ctx.add_exp_and_sp(player, 4254, 335);
                        ctx.give_adena(player, 1850, true);
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

                        htmltext = "30578-06.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            VARKEES => htmltext = Self::give_item(ctx, player, qs, npc.id, HONEY_KHANDAR),
            URUTU => htmltext = Self::give_item(ctx, player, qs, npc.id, DEEP_SEA_ORB),
            HESTUI => htmltext = Self::give_item(ctx, player, qs, npc.id, BEAR_FUR_CLOAK),
            KUNAI => htmltext = Self::give_item(ctx, player, qs, npc.id, SPIDER_DUST),
            USKA => htmltext = Self::give_item(ctx, player, qs, npc.id, ANCESTOR_SKULL),
            GROOKIN => htmltext = Self::give_item(ctx, player, qs, npc.id, BLOODY_AXE),
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

    const NAME: &str = "Q00004_LongLiveThePaagrioLord";

    const NPCS: &str = r#"
        <npc id="30559" type="Folk" level="70" name="Kunai"/>
        <npc id="30560" type="Folk" level="70" name="Uska"/>
        <npc id="30562" type="Folk" level="70" name="Grookin"/>
        <npc id="30566" type="Folk" level="70" name="Varkees"/>
        <npc id="30578" type="Folk" level="70" name="Nakusin"/>
        <npc id="30585" type="Folk" level="70" name="Hestui"/>
        <npc id="30587" type="Folk" level="70" name="Urutu"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4" type="EtcItem" name="Club"/>
        <item id="1541" type="EtcItem" name="Honey Khandar"><set name="is_questitem" val="true"/></item>
        <item id="1542" type="EtcItem" name="Bear Fur Cloak"><set name="is_questitem" val="true"/></item>
        <item id="1543" type="EtcItem" name="Bloody Axe"><set name="is_questitem" val="true"/></item>
        <item id="1544" type="EtcItem" name="Ancestor Skull"><set name="is_questitem" val="true"/></item>
        <item id="1545" type="EtcItem" name="Spider Dust"><set name="is_questitem" val="true"/></item>
        <item id="1546" type="EtcItem" name="Deep Sea Orb"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        kunai: Oid,
        uska: Oid,
        grookin: Oid,
        varkees: Oid,
        nakusin: Oid,
        hestui: Oid,
        urutu: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00004LongLiveThePaagrioLord);
        let pages = Pages::of(&Q00004LongLiveThePaagrioLord)
            .page("30578-02.htm", &["30578-03.htm"])
            .page("30578-04.html", &["30578-05.html"])
            .plain(&[
                "30578-00.htm",
                "30578-01.htm",
                "30578-03.htm",
                "30578-05.html",
                "30578-06.html",
                "30559-01.html",
                "30559-02.html",
                "30560-01.html",
                "30560-02.html",
                "30562-01.html",
                "30562-02.html",
                "30566-01.html",
                "30566-02.html",
                "30585-01.html",
                "30585-02.html",
                "30587-01.html",
                "30587-02.html",
            ])
            .npcs(&[KUNAI, USKA, GROOKIN, VARKEES, NAKUSIN, HESTUI, URUTU]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            kunai: t.spawn(KUNAI, 20),
            uska: t.spawn(USKA, 30),
            grookin: t.spawn(GROOKIN, 40),
            varkees: t.spawn(VARKEES, 50),
            nakusin: t.spawn(NAKUSIN, 60),
            hestui: t.spawn(HESTUI, 70),
            urutu: t.spawn(URUTU, 80),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_orc_of_level_two_may_start() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 10, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.nakusin).contains("30578-00.htm"));
        t.enter_as(2, 1, race::ORC, 0, vec![]);
        assert!(t.quest_page(2, npcs.nakusin).contains("30578-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn six_items_from_six_npcs_then_the_club() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 2, race::ORC, 0, vec![]);
        assert!(t.quest_page(1, npcs.nakusin).contains("30578-02.htm"));
        assert!(t.event(1, NAME, "30578-03.htm").contains("30578-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Nakusin waits until the items are in.
        assert!(t.quest_page(1, npcs.nakusin).contains("30578-04.html"));

        // The first five NPCs give their item, the last one sets cond 2.
        assert!(t.quest_page(1, npcs.kunai).contains("30559-01.html"));
        assert!(t.quest_page(1, npcs.kunai).contains("30559-02.html"));
        assert!(t.quest_page(1, npcs.uska).contains("30560-01.html"));
        assert!(t.quest_page(1, npcs.grookin).contains("30562-01.html"));
        assert!(t.quest_page(1, npcs.varkees).contains("30566-01.html"));
        assert!(t.quest_page(1, npcs.hestui).contains("30585-01.html"));
        assert!(t.take_writes().is_empty());
        assert!(t.quest_page(1, npcs.urutu).contains("30587-01.html"));
        assert_eq!(t.count(1, DEEP_SEA_ORB), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Nakusin: the club, the exp and the adena, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.nakusin).contains("30578-06.html"));
        assert_eq!(t.count(1, CLUB), 1);
        assert_eq!(t.count(1, 57), 1850);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.nakusin)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn a_second_item_from_the_same_npc_is_refused_with_its_page() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 2, race::ORC, 0, vec![]);
        t.quest_page(1, npcs.nakusin);
        t.event(1, NAME, "30578-03.htm");
        assert!(t.quest_page(1, npcs.uska).contains("30560-01.html"));
        assert!(t.quest_page(1, npcs.uska).contains("30560-02.html"));
        assert_eq!(t.count(1, ANCESTOR_SKULL), 1);
    }
}
