//! Quest 648: An Ice Merchant's Dream (Rafforty in the Town of Aden, the ice shelf of the Hot
//! Springs' Ice Shelf; the frost monsters drop silver ice crystals, the shelf enchants them into
//! black ice crystals, and the crystals sell for adena).
// Java: dist/game/data/scripts/quests/Q00648_AnIceMerchantsDream/Q00648_AnIceMerchantsDream.java
//
// Pattern: a kill drop per monster (two drops for some: the crystal, and the hemocyte when the
// other quest Q00115 is done), a state-free sell of the crystals, and a small enchant game on the
// ice shelf whose numbers are quest variables (`ex`). The quest reads Q00115 by name through
// `player_quest_state` (read only).
// NOTE(java-quirk): the Java `ex` variable is an int in the quest's own row (`getInt`/`set`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const RAFFORTY: i32 = 32020;
const ICE_SHELF: i32 = 32023;

// Items
const SILVER_HEMOCYTE: i32 = 8057;
const SILVER_ICE_CRYSTAL: i32 = 8077;
const BLACK_ICE_CRYSTAL: i32 = 8078;

// Misc
const MIN_LEVEL: i32 = 53;

/// Java `Q00115_TheOtherSideOfTruth.class.getSimpleName()`.
const THE_OTHER_SIDE_OF_TRUTH: &str = "Q00115_TheOtherSideOfTruth";

/// Java `DropInfo`: the silver crystal chance and the hemocyte chance (0 for none).
#[derive(Clone, Copy)]
struct DropInfo {
    first_chance: f64,
    second_chance: f64,
}

/// Java `MONSTERS`.
const MONSTERS: [(i32, DropInfo); 19] = [
    (
        22080,
        DropInfo {
            first_chance: 0.285,
            second_chance: 0.048,
        },
    ), // Massive Maze Bandersnatch
    (
        22081,
        DropInfo {
            first_chance: 0.443,
            second_chance: 0.0,
        },
    ), // Lost Watcher
    (
        22082,
        DropInfo {
            first_chance: 0.510,
            second_chance: 0.0,
        },
    ), // Elder Lost Watcher
    (
        22083,
        DropInfo {
            first_chance: 0.477,
            second_chance: 0.049,
        },
    ), // Baby Panthera
    (
        22084,
        DropInfo {
            first_chance: 0.477,
            second_chance: 0.049,
        },
    ), // Panthera
    (
        22085,
        DropInfo {
            first_chance: 0.420,
            second_chance: 0.043,
        },
    ), // Lost Gargoyle
    (
        22086,
        DropInfo {
            first_chance: 0.490,
            second_chance: 0.050,
        },
    ), // Lost Gargoyle Youngling
    (
        22087,
        DropInfo {
            first_chance: 0.787,
            second_chance: 0.081,
        },
    ), // Pronghorn Spirit
    (
        22088,
        DropInfo {
            first_chance: 0.480,
            second_chance: 0.049,
        },
    ), // Pronghorn
    (
        22089,
        DropInfo {
            first_chance: 0.550,
            second_chance: 0.056,
        },
    ), // Ice Tarantula
    (
        22090,
        DropInfo {
            first_chance: 0.570,
            second_chance: 0.058,
        },
    ), // Frost Tarantula
    (
        22091,
        DropInfo {
            first_chance: 0.623,
            second_chance: 0.0,
        },
    ), // Lost Iron Golem
    (
        22092,
        DropInfo {
            first_chance: 0.623,
            second_chance: 0.0,
        },
    ), // Frost Iron Golem
    (
        22093,
        DropInfo {
            first_chance: 0.910,
            second_chance: 0.093,
        },
    ), // Lost Buffalo
    (
        22094,
        DropInfo {
            first_chance: 0.553,
            second_chance: 0.057,
        },
    ), // Frost Buffalo
    (
        22095,
        DropInfo {
            first_chance: 0.593,
            second_chance: 0.061,
        },
    ), // Ursus Cub
    (
        22096,
        DropInfo {
            first_chance: 0.593,
            second_chance: 0.061,
        },
    ), // Ursus
    (
        22097,
        DropInfo {
            first_chance: 0.693,
            second_chance: 0.071,
        },
    ), // Lost Yeti
    (
        22098,
        DropInfo {
            first_chance: 0.717,
            second_chance: 0.074,
        },
    ), // Frost Yeti
];

/// Java `Q00648_AnIceMerchantsDream`.
pub struct Q00648AnIceMerchantsDream;

fn drop_info(npc_id: i32) -> Option<DropInfo> {
    MONSTERS
        .iter()
        .find(|(id, _)| *id == npc_id)
        .map(|(_, info)| *info)
}

impl Script for Q00648AnIceMerchantsDream {
    fn name(&self) -> &'static str {
        "Q00648_AnIceMerchantsDream"
    }

    fn path(&self) -> &'static str {
        "quests/Q00648_AnIceMerchantsDream"
    }

    fn id(&self) -> i32 {
        648
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAFFORTY]);
        s.add_talk_id(&[RAFFORTY, ICE_SHELF]);
        let mobs: Vec<i32> = MONSTERS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SILVER_HEMOCYTE, SILVER_ICE_CRYSTAL, BLACK_ICE_CRYSTAL]);
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
        // Java: `player.getQuestState(Q00115)`, read only; `None` when there is no such state.
        let q115_open = ctx
            .player_quest_state(player, THE_OTHER_SIDE_OF_TRUTH)
            .is_some_and(|q| !q.is_completed());
        let q115_done = ctx
            .player_quest_state(player, THE_OTHER_SIDE_OF_TRUTH)
            .is_some_and(|q| q.is_completed());
        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                qs.start_quest(ctx);
                if q115_done {
                    htmltext = Some("32020-04.htm".to_owned());
                } else {
                    qs.set_cond(ctx, 2, false);
                    htmltext = Some("32020-05.htm".to_owned());
                }
            }
            "ASK" => {
                if qs.get_cond(ctx) >= 1 {
                    htmltext = Some(
                        if q115_open {
                            "32020-14.html"
                        } else {
                            "32020-15.html"
                        }
                        .to_owned(),
                    );
                }
            }
            "LATER" => {
                if qs.get_cond(ctx) >= 1 {
                    htmltext = Some(
                        if q115_open {
                            "32020-19.html"
                        } else {
                            "32020-20.html"
                        }
                        .to_owned(),
                    );
                }
            }
            "REWARD" => {
                if qs.get_cond(ctx) >= 1 {
                    let silver = ctx.get_quest_items_count(player, SILVER_ICE_CRYSTAL);
                    let black = ctx.get_quest_items_count(player, BLACK_ICE_CRYSTAL);
                    if silver + black > 0 {
                        ctx.give_adena(player, silver * 300 + black * 1200, true);
                        ctx.take_items_many(player, -1, &[SILVER_ICE_CRYSTAL, BLACK_ICE_CRYSTAL]);
                        htmltext = Some(
                            if q115_open {
                                "32020-16.html"
                            } else {
                                "32020-17.html"
                            }
                            .to_owned(),
                        );
                    } else {
                        htmltext = Some("32020-18.html".to_owned());
                    }
                }
            }
            "QUIT" => {
                if qs.get_cond(ctx) >= 1 {
                    if q115_open {
                        htmltext = Some("32020-21.html".to_owned());
                        qs.exit_quest(ctx, true, true);
                    } else {
                        htmltext = Some("32020-22.html".to_owned());
                    }
                }
            }
            "32020-06.html" | "32020-07.html" | "32020-08.html" | "32020-09.html" => {
                if qs.get_cond(ctx) >= 1 {
                    htmltext = Some(event.to_owned());
                }
            }
            "32020-23.html" => {
                if qs.get_cond(ctx) >= 1 {
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32023-04.html" => {
                if qs.get_cond(ctx) >= 1
                    && ctx.has_quest_items(player, SILVER_ICE_CRYSTAL)
                    && qs.get_int(ctx, "ex") == 0
                {
                    let value = (ctx.get_random(4) + 1) * 10;
                    qs.set_int(ctx, "ex", value);
                    htmltext = Some(event.to_owned());
                }
            }
            "32023-05.html" => {
                if qs.get_cond(ctx) >= 1
                    && ctx.has_quest_items(player, SILVER_ICE_CRYSTAL)
                    && qs.get_int(ctx, "ex") > 0
                {
                    ctx.take_items(player, SILVER_ICE_CRYSTAL, 1);
                    let value = qs.get_int(ctx, "ex") + 1;
                    qs.set_int(ctx, "ex", value);
                    ctx.play_sound(player, QuestSound::ItemsoundBrokenKey);
                    htmltext = Some(event.to_owned());
                }
            }
            "32023-06.html" => {
                if qs.get_cond(ctx) >= 1
                    && ctx.has_quest_items(player, SILVER_ICE_CRYSTAL)
                    && qs.get_int(ctx, "ex") > 0
                {
                    ctx.take_items(player, SILVER_ICE_CRYSTAL, 1);
                    let value = qs.get_int(ctx, "ex") + 2;
                    qs.set_int(ctx, "ex", value);
                    ctx.play_sound(player, QuestSound::ItemsoundBrokenKey);
                    htmltext = Some(event.to_owned());
                }
            }
            "REPLY4" | "REPLY5" => {
                if qs.get_cond(ctx) >= 1 && qs.get_int(ctx, "ex") > 0 {
                    let ex = qs.get_int(ctx, "ex");
                    let val1 = ex / 10;
                    let extra = if event == "REPLY5" { 2 } else { 0 };
                    let val2 = (ex - val1 * 10) + extra;
                    if val1 == val2 {
                        htmltext = Some("32023-07.html".to_owned());
                        ctx.give_items(player, BLACK_ICE_CRYSTAL, 1);
                        ctx.play_sound(player, QuestSound::ItemsoundEnchantSuccess);
                    } else {
                        htmltext = Some("32023-08.html".to_owned());
                        ctx.play_sound(player, QuestSound::ItemsoundEnchantFailed);
                    }
                    qs.set_int(ctx, "ex", 0);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getRandomPartyMemberState(killer, -1, 3, npc)`.
        let Some(qs) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) else {
            return;
        };
        let Some(info) = drop_info(npc.id) else {
            return;
        };
        let member = qs.player();
        if qs.get_cond(ctx) >= 1 {
            ctx.give_item_randomly(
                member,
                Some(npc),
                SILVER_ICE_CRYSTAL,
                1,
                1,
                0,
                info.first_chance,
                true,
            );
        }
        if info.second_chance > 0.0 {
            let q115_done = ctx
                .player_quest_state(member, THE_OTHER_SIDE_OF_TRUTH)
                .is_some_and(|q| q.is_completed());
            if qs.get_cond(ctx) >= 2 && q115_done {
                ctx.give_item_randomly(
                    member,
                    Some(npc),
                    SILVER_HEMOCYTE,
                    1,
                    1,
                    0,
                    info.second_chance,
                    true,
                );
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let q115_done = ctx
            .player_quest_state(player, THE_OTHER_SIDE_OF_TRUTH)
            .is_some_and(|q| q.is_completed());
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == RAFFORTY {
            let state = qs.state(ctx);
            if state == CREATED {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "32020-01.htm".to_owned()
                } else if q115_done {
                    "32020-02.htm".to_owned()
                } else {
                    "32020-03.htm".to_owned()
                };
            } else if state == STARTED {
                let crystals = ctx.get_quest_items_count(player, SILVER_ICE_CRYSTAL)
                    + ctx.get_quest_items_count(player, BLACK_ICE_CRYSTAL);
                if q115_done {
                    htmltext = if crystals > 0 {
                        "32020-13.html".to_owned()
                    } else {
                        "32020-11.html".to_owned()
                    };
                    if qs.is_cond(ctx, 1) {
                        qs.set_cond(ctx, 2, true);
                    }
                } else {
                    htmltext = if crystals > 0 {
                        "32020-12.html".to_owned()
                    } else {
                        "32020-10.html".to_owned()
                    };
                }
            }
        } else if npc.id == ICE_SHELF {
            if qs.is_started(ctx) {
                if ctx.has_quest_items(player, SILVER_ICE_CRYSTAL) {
                    let val = qs.get_int(ctx, "ex") % 10;
                    if !qs.is_set(ctx, "ex") || val == 0 {
                        htmltext = "32023-03.html".to_owned();
                        qs.set_int(ctx, "ex", 0);
                    } else {
                        htmltext = "32023-09.html".to_owned();
                    }
                } else {
                    htmltext = "32023-02.html".to_owned();
                }
            } else {
                htmltext = "32023-01.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use l2_world::QuestWrite;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00648_AnIceMerchantsDream";

    const NPCS: &str = r#"
        <npc id="32020" type="Folk" level="70" name="Rafforty"/>
        <npc id="32023" type="Folk" level="70" name="Ice Shelf"/>
        <npc id="22080" type="Monster" level="60" name="Massive Maze Bandersnatch"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8057" type="EtcItem" name="Silver Hemocyte"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8077" type="EtcItem" name="Silver Ice Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8078" type="EtcItem" name="Black Ice Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00648AnIceMerchantsDream);
        let pages = Pages::of(&Q00648AnIceMerchantsDream)
            .page("32020-01.htm", &[])
            .page("32020-02.htm", &["ACCEPT"])
            .page("32020-03.htm", &["ACCEPT"])
            .page("32020-05.htm", &["32020-06.html", "32020-08.html"])
            .plain(&["32020-04.htm", "32020-06.html", "32020-07.html"])
            .page("32020-10.html", &["32020-07.html"])
            .page("32020-12.html", &["ASK", "LATER", "QUIT"])
            .page("32020-15.html", &["LATER", "REWARD"])
            .plain(&[
                "32020-16.html",
                "32020-17.html",
                "32020-18.html",
                "32020-11.html",
                "32020-13.html",
                "32020-22.html",
                "32020-14.html",
                "32020-19.html",
                "32020-20.html",
                "32020-21.html",
            ])
            .page("32023-01.html", &[])
            .page("32023-03.html", &["32023-04.html"])
            .page("32023-04.html", &["32023-05.html", "32023-06.html"])
            .page("32023-05.html", &["REPLY4", "REPLY5"])
            .page("32023-06.html", &["REPLY4", "REPLY5"])
            .plain(&["32023-02.html", "32023-07.html", "32023-08.html", "32023-09.html"])
            .npcs(&[RAFFORTY, ICE_SHELF]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rafforty = t.spawn(RAFFORTY, 20);
        let shelf = t.spawn(ICE_SHELF, 22);
        (t, rafforty, shelf)
    }

    #[test]
    fn the_gate_is_level_53_and_accepting_sets_cond_2_without_the_other_quest() {
        let (mut t, rafforty, _) = setup();
        t.enter(1, 52, vec![]);
        assert!(t.quest_page(1, rafforty).contains("32020-01.htm"));
        t.enter(2, 53, vec![]);
        assert!(t.quest_page(2, rafforty).contains("32020-03.htm"));
        assert!(t.take_writes().is_empty());

        assert!(t.event(2, NAME, "ACCEPT").contains("32020-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::of(2, rows::insert(NAME, "cond", "1")),
                rows::of(2, rows::insert(NAME, "<state>", "Started")),
                rows::of(2, rows::update(NAME, "cond", "2")),
            ]
        );
    }

    #[test]
    fn the_silver_crystal_is_enchanted_on_the_shelf_and_the_crystals_sell() {
        let (mut t, rafforty, shelf) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, rafforty);
        t.event(1, NAME, "ACCEPT");
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SILVER_ICE_CRYSTAL, 3);
        });
        t.take();
        t.take_writes();

        // The shelf: the first talk sets `ex` to 0; the enchant starts (ex 10..40), 1 crystal used.
        assert!(t.quest_page(1, shelf).contains("32023-03.html"));
        assert_eq!(t.take_writes(), [rows::insert(NAME, "ex", "0")]);
        assert!(t.event(1, NAME, "32023-04.html").contains("32023-04.html"));
        assert!(matches!(t.take_writes().as_slice(), [QuestWrite::UpdateVar { var, .. }] if var == "ex"));
        assert!(t.event(1, NAME, "32023-05.html").contains("32023-05.html"));
        assert_eq!(t.count(1, SILVER_ICE_CRYSTAL), 2);
        assert!(matches!(t.take_writes().as_slice(), [QuestWrite::UpdateVar { var, .. }] if var == "ex"));

        // REPLY4 settles the enchant: a black crystal or not, `ex` back to 0.
        assert!(t.event(1, NAME, "REPLY4").contains("32023-0"));
        let black = t.count(1, BLACK_ICE_CRYSTAL);
        assert!(black <= 1);
        assert!(
            matches!(t.take_writes().as_slice(), [QuestWrite::UpdateVar { var, value, .. }] if *var == "ex" && *value == "0")
        );
    }

    #[test]
    fn rafforty_pays_the_crystals_at_the_reward() {
        let (mut t, rafforty, _) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, rafforty);
        t.event(1, NAME, "ACCEPT");
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), SILVER_ICE_CRYSTAL, 2);
            ctx.give_items(Oid(1), BLACK_ICE_CRYSTAL, 1);
        });
        t.take();
        t.take_writes();

        assert!(t.quest_page(1, rafforty).contains("32020-12.html"));
        assert!(t.event(1, NAME, "ASK").contains("32020-15.html"));
        assert!(t.event(1, NAME, "REWARD").contains("32020-17.html"));
        // Two silver at 300 and one black at 1200 adena.
        assert_eq!(t.count(1, 57), 2 * 300 + 1200);
        assert_eq!(
            (t.count(1, SILVER_ICE_CRYSTAL), t.count(1, BLACK_ICE_CRYSTAL)),
            (0, 0)
        );
        assert!(t.take_writes().is_empty());
    }
}
