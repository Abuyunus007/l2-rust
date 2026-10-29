//! Quest 174: Supply Check (Marcela, Benis and Nika in the Talking Island warehouses).
// Java: dist/game/data/scripts/quests/Q00174_SupplyCheck/Q00174_SupplyCheck.java

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NIKA: i32 = 32167;
const BENIS: i32 = 32170;
const MARCELA: i32 = 32173;

// Items
const WAREHOUSE_MANIFEST: i32 = 9792;
const GROCERY_STORE_MANIFEST: i32 = 9793;
const REWARD: [i32; 5] = [
    23,   // Wooden Breastplate
    43,   // Wooden Helmet
    49,   // Gloves
    2386, // Wooden Gaiters
    37,   // Leather Shoes
];

// Misc
const MIN_LEVEL: i32 = 2;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00174_SupplyCheck`.
pub struct Q00174SupplyCheck;

impl Script for Q00174SupplyCheck {
    fn name(&self) -> &'static str {
        "Q00174_SupplyCheck"
    }

    fn path(&self) -> &'static str {
        "quests/Q00174_SupplyCheck"
    }

    fn id(&self) -> i32 {
        174
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MARCELA]);
        s.add_talk_id(&[MARCELA, BENIS, NIKA]);
        s.register_quest_items(&[WAREHOUSE_MANIFEST, GROCERY_STORE_MANIFEST]);
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

        if event.eq_ignore_ascii_case("32173-03.htm") {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            MARCELA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "32173-01.htm"
                    } else {
                        "32173-02.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "32173-04.html".to_owned(),
                    2 => {
                        qs.set_cond(ctx, 3, true);
                        ctx.take_items(player, WAREHOUSE_MANIFEST, -1);
                        htmltext = "32173-05.html".to_owned();
                    }
                    3 => htmltext = "32173-06.html".to_owned(),
                    4 => {
                        for item_id in REWARD {
                            ctx.give_items(player, item_id, 1);
                        }
                        // Newbie Guide: the mission counter of the guide, when it is in the game.
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
                        ctx.add_exp_and_sp(player, 5672, 446);
                        ctx.give_adena(player, 2466, true);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "32173-07.html".to_owned();
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            BENIS => {
                if qs.is_started(ctx) {
                    htmltext = match qs.get_cond(ctx) {
                        1 => {
                            qs.set_cond(ctx, 2, true);
                            ctx.give_items(player, WAREHOUSE_MANIFEST, 1);
                            "32170-01.html".to_owned()
                        }
                        2 => "32170-02.html".to_owned(),
                        _ => "32170-03.html".to_owned(),
                    };
                }
            }
            NIKA => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 | 2 => htmltext = "32167-01.html".to_owned(),
                        3 => {
                            qs.set_cond(ctx, 4, true);
                            ctx.give_items(player, GROCERY_STORE_MANIFEST, 1);
                            htmltext = "32167-02.html".to_owned();
                        }
                        4 => htmltext = "32167-03.html".to_owned(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::race;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00174_SupplyCheck";

    const NPCS: &str = r#"
        <npc id="32167" type="Folk" level="70" name="Nika"/>
        <npc id="32170" type="Folk" level="70" name="Benis"/>
        <npc id="32173" type="Folk" level="70" name="Marcela"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="23" type="Armor" name="Wooden Breastplate"><set name="bodypart" val="chest"/></item>
        <item id="43" type="Armor" name="Wooden Helmet"><set name="bodypart" val="head"/></item>
        <item id="49" type="Armor" name="Gloves"><set name="bodypart" val="gloves"/></item>
        <item id="2386" type="Armor" name="Wooden Gaiters"><set name="bodypart" val="legs"/></item>
        <item id="37" type="Armor" name="Leather Shoes"><set name="bodypart" val="feet"/></item>
        <item id="9792" type="EtcItem" name="Warehouse Manifest"><set name="is_questitem" val="true"/></item>
        <item id="9793" type="EtcItem" name="Grocery Store Manifest"><set name="is_questitem" val="true"/></item>"#;

    /// Java `NewbieGuide` as far as this quest writes it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            NEWBIE_GUIDE
        }
        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        marcela: Oid,
        benis: Oid,
        nika: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Q00174SupplyCheck);
        let pages = Pages::of(&Q00174SupplyCheck)
            .page("32173-01.htm", &["32173-03.htm"])
            .plain(&[
                "32173-02.htm",
                "32173-03.htm",
                "32173-04.html",
                "32173-05.html",
                "32173-06.html",
                "32173-07.html",
                "32170-01.html",
                "32170-02.html",
                "32170-03.html",
                "32167-01.html",
                "32167-02.html",
                "32167-03.html",
            ])
            .npcs(&[NIKA, BENIS, MARCELA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            marcela: t.spawn(MARCELA, 20),
            benis: t.spawn(BENIS, 40),
            nika: t.spawn(NIKA, 60),
        };
        (t, npcs)
    }

    #[test]
    fn the_level_2_characters_are_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 1, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.marcela).contains("32173-02.htm"));
        t.enter_as(2, 2, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(2, npcs.marcela).contains("32173-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_manifests_in_turn_then_the_reward_and_the_guide() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 2, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.marcela).contains("32173-01.htm"));
        assert!(t.event(1, NAME, "32173-03.htm").contains("32173-03.htm"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]
        );
        assert!(t.quest_page(1, npcs.marcela).contains("32173-04.html"));

        // Benis gives the warehouse manifest (cond 2).
        assert!(t.quest_page(1, npcs.benis).contains("32170-01.html"));
        assert_eq!(t.count(1, WAREHOUSE_MANIFEST), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.benis).contains("32170-02.html"));

        // Marcela takes it back (cond 3); Nika gives the grocery manifest (cond 4).
        assert!(t.quest_page(1, npcs.marcela).contains("32173-05.html"));
        assert_eq!((t.count(1, WAREHOUSE_MANIFEST), t.take_writes().len()), (0, 1));
        assert!(t.quest_page(1, npcs.marcela).contains("32173-06.html"));
        assert!(t.quest_page(1, npcs.nika).contains("32167-02.html"));
        assert_eq!(t.count(1, GROCERY_STORE_MANIFEST), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        assert!(t.quest_page(1, npcs.nika).contains("32167-03.html"));

        // Marcela: the five pieces, the guide's mission, exp, sp and adena, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.marcela).contains("32173-07.html"));
        for item_id in REWARD {
            assert_eq!(t.count(1, item_id), 1, "{item_id}");
        }
        assert_eq!(t.count(1, 57), 2466);
        let guide = |var: &str, value: &str| rows::insert(NEWBIE_GUIDE, var, value);
        assert_eq!(
            t.take_writes(),
            [
                guide("NRmemo", "41"),
                guide("NRmemoState41", "1"),
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, npcs.marcela).contains("already been completed"));
    }
}
