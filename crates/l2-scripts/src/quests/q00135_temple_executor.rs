//! Quest 135: Temple Executor (Shegfield, Pano, Alex and Sonin; three kinds of loot, three credentials).
// Java: dist/game/data/scripts/quests/Q00135_TempleExecutor/Q00135_TempleExecutor.java
//
// Pattern: party kills (`get_random_party_member_cond`) fill three item stacks one after the other
// (cargo, crystals, maps); the NPCs swap them for credentials. Items are counted on the killer.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SHEGFIELD: i32 = 30068;
const PANO: i32 = 30078;
const ALEX: i32 = 30291;
const SONIN: i32 = 31773;

// Items
const STOLEN_CARGO: i32 = 10328;
const HATE_CRYSTAL: i32 = 10329;
const OLD_TREASURE_MAP: i32 = 10330;
const SONINS_CREDENTIALS: i32 = 10331;
const PANOS_CREDENTIALS: i32 = 10332;
const ALEXS_CREDENTIALS: i32 = 10333;
const BADGE_TEMPLE_EXECUTOR: i32 = 10334;

/// Java `MOBS`: the chance (out of 1000) of a loot item per monster.
fn mob_chance(npc_id: i32) -> Option<i32> {
    match npc_id {
        20781 => Some(439), // Delu Lizardman Shaman
        21104 => Some(439), // Delu Lizardman Supplier
        21105 => Some(504), // Delu Lizardman Special Agent
        21106 => Some(423), // Cursed Seer
        21107 => Some(902), // Delu Lizardman Commander
        _ => None,
    }
}

// Misc
const MIN_LEVEL: i32 = 35;
const ITEM_COUNT: i64 = 10;
const MAX_REWARD_LEVEL: i32 = 41;

/// Java `Q00135_TempleExecutor`.
pub struct Q00135TempleExecutor;

impl Script for Q00135TempleExecutor {
    fn name(&self) -> &'static str {
        "Q00135_TempleExecutor"
    }

    fn path(&self) -> &'static str {
        "quests/Q00135_TempleExecutor"
    }

    fn id(&self) -> i32 {
        135
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SHEGFIELD]);
        s.add_talk_id(&[SHEGFIELD, ALEX, SONIN, PANO]);
        s.add_kill_id(&[20781, 21104, 21105, 21106, 21107]);
        s.register_quest_items(&[
            STOLEN_CARGO,
            HATE_CRYSTAL,
            OLD_TREASURE_MAP,
            SONINS_CREDENTIALS,
            PANOS_CREDENTIALS,
            ALEXS_CREDENTIALS,
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

        let mut htmltext = Some(event.to_owned());
        match event {
            "30291-02a.html" | "30291-04.html" | "30291-05.html" | "30291-06.html" | "30068-08.html"
            | "30068-09.html" | "30068-10.html" => {}
            "30068-03.htm" => qs.start_quest(ctx),
            "30068-04.html" => qs.set_cond(ctx, 2, true),
            "30291-07.html" => {
                qs.unset(ctx, "talk");
                qs.set_cond(ctx, 3, true);
            }
            "30068-11.html" => {
                ctx.give_items(player, BADGE_TEMPLE_EXECUTOR, 1);
                ctx.give_adena(player, 16_924, true);
                if ctx.player_level(player) < MAX_REWARD_LEVEL {
                    ctx.add_exp_and_sp(player, 30_000, 2_000);
                }

                qs.exit_quest(ctx, false, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(player, 3) else {
            return;
        };

        let qs = ctx.get_quest_state(member, false);
        let Some(chance) = mob_chance(npc.id) else {
            return;
        };
        if ctx.get_random(1000) < chance {
            // NOTE(java-quirk): the loot is counted and given on the killer, not the member.
            if ctx.get_quest_items_count(player, STOLEN_CARGO) < ITEM_COUNT {
                ctx.give_items(player, STOLEN_CARGO, 1);
            } else if ctx.get_quest_items_count(player, HATE_CRYSTAL) < ITEM_COUNT {
                ctx.give_items(player, HATE_CRYSTAL, 1);
            } else {
                ctx.give_items(player, OLD_TREASURE_MAP, 1);
            }

            if ctx.get_quest_items_count(player, STOLEN_CARGO) >= ITEM_COUNT
                && ctx.get_quest_items_count(player, HATE_CRYSTAL) >= ITEM_COUNT
                && ctx.get_quest_items_count(player, OLD_TREASURE_MAP) >= ITEM_COUNT
                && let Some(qs) = qs
            {
                qs.set_cond(ctx, 4, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            SHEGFIELD => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30068-01.htm"
                    } else {
                        "30068-02.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30068-04.html".to_owned();
                    }
                    2 | 3 => htmltext = "30068-05.html".to_owned(),
                    4 => htmltext = "30068-06.html".to_owned(),
                    5 => {
                        if qs.is_set(ctx, "talk") {
                            htmltext = "30068-08.html".to_owned();
                        } else if ctx.has_all_quest_items(
                            player,
                            &[PANOS_CREDENTIALS, SONINS_CREDENTIALS, ALEXS_CREDENTIALS],
                        ) {
                            ctx.take_items(player, SONINS_CREDENTIALS, -1);
                            ctx.take_items(player, PANOS_CREDENTIALS, -1);
                            ctx.take_items(player, ALEXS_CREDENTIALS, -1);
                            qs.set(ctx, "talk", "1");
                            htmltext = "30068-07.html".to_owned();
                        } else {
                            htmltext = "30068-06.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ALEX => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30291-01.html".to_owned(),
                        2 => {
                            if qs.is_set(ctx, "talk") {
                                htmltext = "30291-03.html".to_owned();
                            } else {
                                qs.set(ctx, "talk", "1");
                                htmltext = "30291-02.html".to_owned();
                            }
                        }
                        3 => htmltext = "30291-08.html".to_owned(),
                        4 => {
                            if ctx.has_all_quest_items(player, &[PANOS_CREDENTIALS, SONINS_CREDENTIALS]) {
                                if ctx.get_quest_items_count(player, OLD_TREASURE_MAP) < ITEM_COUNT {
                                    return Some(htmltext);
                                }

                                qs.set_cond(ctx, 5, true);
                                ctx.take_items(player, OLD_TREASURE_MAP, -1);
                                ctx.give_items(player, ALEXS_CREDENTIALS, 1);
                                htmltext = "30291-10.html".to_owned();
                            } else {
                                htmltext = "30291-09.html".to_owned();
                            }
                        }
                        5 => htmltext = "30291-11.html".to_owned(),
                        _ => {}
                    }
                }
            }
            PANO => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30078-01.html".to_owned(),
                        2 => htmltext = "30078-02.html".to_owned(),
                        3 => htmltext = "30078-03.html".to_owned(),
                        4 => {
                            if !qs.is_set(ctx, "Pano") {
                                if ctx.get_quest_items_count(player, HATE_CRYSTAL) < ITEM_COUNT {
                                    return Some(htmltext);
                                }

                                ctx.take_items(player, HATE_CRYSTAL, -1);
                                ctx.give_items(player, PANOS_CREDENTIALS, 1);
                                qs.set(ctx, "Pano", "1");
                                htmltext = "30078-04.html".to_owned();
                            }
                        }
                        5 => htmltext = "30078-05.html".to_owned(),
                        _ => {}
                    }
                }
            }
            SONIN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "31773-01.html".to_owned(),
                        2 => htmltext = "31773-02.html".to_owned(),
                        3 => htmltext = "31773-03.html".to_owned(),
                        4 => {
                            if !qs.is_set(ctx, "Sonin") {
                                if ctx.get_quest_items_count(player, STOLEN_CARGO) < ITEM_COUNT {
                                    return Some(htmltext);
                                }

                                ctx.take_items(player, STOLEN_CARGO, -1);
                                ctx.give_items(player, SONINS_CREDENTIALS, 1);
                                qs.set(ctx, "Sonin", "1");
                                htmltext = "31773-04.html".to_owned();
                            }
                        }
                        5 => htmltext = "31773-05.html".to_owned(),
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="30068" type="Folk" level="40" name="Shegfield"/>
        <npc id="30078" type="Folk" level="40" name="Pano"/>
        <npc id="30291" type="Folk" level="40" name="Alex"/>
        <npc id="31773" type="Folk" level="40" name="Sonin"/>
        <npc id="21107" type="Monster" level="38" name="Delu Lizardman Commander"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10328" type="EtcItem" name="Cargo"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10329" type="EtcItem" name="Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10330" type="EtcItem" name="Map"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="10331" type="EtcItem" name="Sonin's Credentials"><set name="is_questitem" val="true"/></item>
        <item id="10332" type="EtcItem" name="Pano's Credentials"><set name="is_questitem" val="true"/></item>
        <item id="10333" type="EtcItem" name="Alex's Credentials"><set name="is_questitem" val="true"/></item>
        <item id="10334" type="EtcItem" name="Badge"/>"#;

    const NAME: &str = "Q00135_TempleExecutor";

    struct Npcs {
        shegfield: Oid,
        pano: Oid,
        alex: Oid,
        sonin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00135TempleExecutor);
        let pages = Pages::of(&Q00135TempleExecutor)
            .page("30068-01.htm", &["30068-03.htm"])
            .plain(&["30068-02.htm", "30068-03.htm", "30068-04.html", "30068-05.html"])
            .plain(&["30068-06.html", "30068-07.html"])
            .page("30068-08.html", &["30068-09.html"])
            .page("30068-09.html", &["30068-10.html"])
            .page("30068-10.html", &["30068-11.html"])
            .plain(&["30068-11.html"])
            .plain(&["30291-01.html", "30291-03.html", "30291-08.html", "30291-09.html"])
            .page("30291-02.html", &["30291-02a.html"])
            .page("30291-02a.html", &["30291-04.html"])
            .page("30291-04.html", &["30291-05.html"])
            .page("30291-05.html", &["30291-06.html"])
            .page("30291-06.html", &["30291-07.html"])
            .plain(&["30291-07.html", "30291-10.html", "30291-11.html"])
            .plain(&["30078-01.html", "30078-02.html", "30078-03.html", "30078-04.html"])
            .plain(&["31773-01.html", "31773-02.html", "31773-03.html", "31773-04.html"])
            .npcs(&[SHEGFIELD, PANO, ALEX, SONIN])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            shegfield: t.spawn(SHEGFIELD, 20),
            pano: t.spawn(PANO, 30),
            alex: t.spawn(ALEX, 40),
            sonin: t.spawn(SONIN, 50),
        };
        (t, npcs)
    }

    #[test]
    fn shegfield_gates_level_35() {
        let (mut t, npcs) = setup();
        t.enter(1, 34, vec![]);
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_three_loot_stacks_the_credentials_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 38, vec![]);
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-01.htm"));
        assert!(t.event(1, NAME, "30068-03.htm").contains("30068-03.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        // Talking to Shegfield at cond 1 moves to cond 2.
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-04.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);

        // Alex: first talk sets "talk", the pages lead to cond 3.
        assert!(t.quest_page(1, npcs.alex).contains("30291-02.html"));
        assert_eq!(t.take_writes(), [insert(NAME, "talk", "1")]);
        for page in [
            "30291-02a.html",
            "30291-04.html",
            "30291-05.html",
            "30291-06.html",
            "30291-07.html",
        ] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert_eq!(t.take_writes().last(), Some(&update(NAME, "cond", "3")));

        // Kills fill 10 cargo, 10 crystals and 10 maps; the commander drops 90% of the time.
        for _ in 0..2000 {
            if t.count(1, OLD_TREASURE_MAP) >= 10 {
                break;
            }
            let mob = t.spawn(21107, 80);
            t.kill(1, mob);
        }
        assert_eq!(
            (
                t.count(1, STOLEN_CARGO),
                t.count(1, HATE_CRYSTAL),
                t.count(1, OLD_TREASURE_MAP)
            ),
            (10, 10, 10)
        );
        assert!(t.take_writes().contains(&update(NAME, "cond", "4")));

        // Pano and Sonin swap their stack for credentials, Alex then takes the maps (cond 5).
        assert!(t.quest_page(1, npcs.pano).contains("30078-04.html"));
        assert_eq!(t.count(1, PANOS_CREDENTIALS), 1);
        assert!(t.quest_page(1, npcs.sonin).contains("31773-04.html"));
        assert_eq!(t.count(1, SONINS_CREDENTIALS), 1);
        assert!(t.quest_page(1, npcs.alex).contains("30291-10.html"));
        assert_eq!(t.count(1, ALEXS_CREDENTIALS), 1);
        assert_eq!(t.count(1, OLD_TREASURE_MAP), 0);

        // Shegfield takes the credentials, then pays.
        t.take_writes();
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-07.html"));
        assert_eq!(t.count(1, PANOS_CREDENTIALS), 0);
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-08.html"));
        assert!(t.event(1, NAME, "30068-09.html").contains("30068-09.html"));
        assert!(t.event(1, NAME, "30068-10.html").contains("30068-10.html"));
        assert!(t.event(1, NAME, "30068-11.html").contains("30068-11.html"));
        assert_eq!(t.count(1, BADGE_TEMPLE_EXECUTOR), 1);
        assert_eq!(t.count(1, 57), 16_924);
        assert!(
            t.take_writes()
                .ends_with(&[delete(NAME, false), update(NAME, "<state>", "Completed")])
        );
    }
}
