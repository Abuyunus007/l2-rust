//! Quest 307: Control Device of the Giants (Droph in the Town of Giran sends level 79 players after the three giants' sheets and Hekaton Prime).
// Java: dist/game/data/scripts/quests/Q00307_ControlDeviceOfTheGiants/Q00307_ControlDeviceOfTheGiants.java
//
// Pattern: a boss summon (`docs/design/scripts.md`, "Quest patterns"): the three giants drop one
// control sheet each for a random party member in cond 1; with all three the player asks Droph for
// Hekaton Prime, which spawns once (the script keeps the spawned boss in its own state) and is
// shared through the global variable `GiantsControlDeviceRespawn` for one hour after the kill. The
// boss's death in a party sets cond 2 for the members in cond 1; Droph gives the support items and
// ends the quest.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const DROPH: i32 = 32711;

// RB
const GORGOLOS: i32 = 25681;
const LAST_TITAN_UTENUS: i32 = 25684;
const GIANT_MARPANAK: i32 = 25680;
const HEKATON_PRIME: i32 = 25687;

// Items
const SUPPORT_ITEMS: i32 = 14850;
const CET_1_SHEET: i32 = 14851;
const CET_2_SHEET: i32 = 14852;
const CET_3_SHEET: i32 = 14853;
const SHEETS: [i32; 3] = [CET_1_SHEET, CET_2_SHEET, CET_3_SHEET];

// Misc
const MIN_LEVEL: i32 = 79;
/// Java `RESPAWN_DELAY`: one hour.
const RESPAWN_DELAY_MS: i64 = 3_600_000;
/// Java `GlobalVariablesManager` key of the respawn time.
const RESPAWN_VARIABLE: &str = "GiantsControlDeviceRespawn";

/// Java `private static Npc hekaton`: the Hekaton Prime this script spawned last.
#[derive(Default)]
struct ScriptState {
    hekaton: Option<ScriptNpc>,
}

/// Java `Q00307_ControlDeviceOfTheGiants`.
pub struct Q00307ControlDeviceOfTheGiants;

impl Script for Q00307ControlDeviceOfTheGiants {
    fn name(&self) -> &'static str {
        "Q00307_ControlDeviceOfTheGiants"
    }

    fn path(&self) -> &'static str {
        "quests/Q00307_ControlDeviceOfTheGiants"
    }

    fn id(&self) -> i32 {
        307
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DROPH]);
        s.add_talk_id(&[DROPH]);
        s.add_kill_id(&[GORGOLOS, LAST_TITAN_UTENUS, GIANT_MARPANAK, HEKATON_PRIME]);
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
        // NOTE(java-quirk): an event the script does not know comes back as the text too, only the
        // `default` branch of Java's switch (below) answers null.
        let mut htmltext = Some(event.to_owned());
        match event {
            "32711-04.html" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    htmltext = Some(
                        if ctx.has_all_quest_items(player, &SHEETS) {
                            "32711-04a.html"
                        } else {
                            "32711-04.html"
                        }
                        .to_owned(),
                    );
                }
            }
            "32711-05a.html" => {
                ctx.send_radar_control(player, 0, 2, 186_214, 61_591, -4_152);
            }
            "32711-05b.html" => {
                ctx.send_radar_control(player, 0, 2, 187_554, 60_800, -4_984);
            }
            "32711-05c.html" => {
                ctx.send_radar_control(player, 0, 2, 193_432, 53_922, -4_368);
            }
            "spawn" => {
                if !ctx.has_all_quest_items(player, &SHEETS) {
                    return Some(ctx.get_no_quest_msg(player));
                }
                let hekaton = ctx.state::<ScriptState>().hekaton;
                if hekaton.is_some_and(|hekaton| !ctx.is_dead(hekaton.object_id)) {
                    return Some("32711-09.html".to_owned());
                }
                if ctx.global_variables().get_long(RESPAWN_VARIABLE, 0) - ctx.now_ms() > 0 {
                    return Some("32711-09a.html".to_owned());
                }
                for sheet in SHEETS {
                    ctx.take_items(player, sheet, 1);
                }
                let hekaton = ctx.add_spawn(HEKATON_PRIME, 191_777, 56_197, -7_624, 0, false, 0, 0);
                ctx.state::<ScriptState>().hekaton = hekaton;
                htmltext = Some("32711-09.html".to_owned());
            }
            "32711-03.htm" | "32711-05.html" | "32711-06.html" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(member) = ctx.get_random_party_member_cond(killer, 1) else {
            return;
        };
        let sheet = match npc.id {
            GORGOLOS => CET_1_SHEET,
            LAST_TITAN_UTENUS => CET_2_SHEET,
            GIANT_MARPANAK => CET_3_SHEET,
            HEKATON_PRIME => {
                if ctx.is_in_party(killer) {
                    for party_member in ctx.world().party_members_of(killer) {
                        if let Some(qs) = ctx.get_quest_state(party_member, false)
                            && qs.is_cond(ctx, 1)
                        {
                            qs.set_cond(ctx, 2, true);
                        }
                    }
                    let respawn = ctx.now_ms() + RESPAWN_DELAY_MS;
                    ctx.set_global_variable(RESPAWN_VARIABLE, &respawn.to_string());
                }
                return;
            }
            _ => return,
        };
        ctx.give_items(member, sheet, 1);
        ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32711-01.htm"
                } else {
                    "32711-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                let hekaton = ctx.state::<ScriptState>().hekaton;
                if hekaton.is_some_and(|hekaton| !ctx.is_dead(hekaton.object_id)) {
                    htmltext = "32711-09.html".to_owned();
                } else if qs.is_cond(ctx, 1) {
                    htmltext = if ctx.has_all_quest_items(player, &SHEETS) {
                        "32711-08.html"
                    } else {
                        "32711-07.html"
                    }
                    .to_owned();
                } else if qs.is_cond(ctx, 2) {
                    ctx.give_items(player, SUPPORT_ITEMS, 1);
                    qs.exit_quest(ctx, true, true);
                    htmltext = "32711-10.html".to_owned();
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
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00307_ControlDeviceOfTheGiants";

    const NPCS: &str = r#"
        <npc id="32711" type="Folk" level="85" name="Droph"/>
        <npc id="25681" type="RaidBoss" level="80" name="Gorgolos"/>
        <npc id="25684" type="RaidBoss" level="80" name="Last Titan Utenus"/>
        <npc id="25680" type="RaidBoss" level="80" name="Giant Marpanak"/>
        <npc id="25687" type="RaidBoss" level="85" name="Hekaton Prime"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14850" type="EtcItem" name="Support Items"/>
        <item id="14851" type="EtcItem" name="CET 1 Sheet"><set name="is_questitem" val="true"/></item>
        <item id="14852" type="EtcItem" name="CET 2 Sheet"><set name="is_questitem" val="true"/></item>
        <item id="14853" type="EtcItem" name="CET 3 Sheet"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00307ControlDeviceOfTheGiants);
        let pages = Pages::of(&Q00307ControlDeviceOfTheGiants)
            .page("32711-01.htm", &["32711-03.htm"])
            .page("32711-03.htm", &["32711-04.html"])
            .page("32711-04.html", &["32711-05.html", "32711-06.html"])
            .page(
                "32711-05.html",
                &["32711-05a.html", "32711-05b.html", "32711-05c.html"],
            )
            .plain(&[
                "32711-02.htm",
                "32711-04a.html",
                "32711-05a.html",
                "32711-05b.html",
                "32711-05c.html",
            ])
            .page(
                "32711-07.html",
                &["32711-05a.html", "32711-05b.html", "32711-05c.html"],
            )
            .page("32711-08.html", &["spawn"])
            .plain(&["32711-06.html", "32711-09.html", "32711-10.html"])
            .npcs(&[DROPH]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let droph = t.spawn(DROPH, 20);
        (t, droph)
    }

    #[test]
    fn a_level_seventy_eight_character_is_sent_away() {
        let (mut t, droph) = setup();
        t.enter_as(1, 78, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, droph).contains("32711-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn three_sheets_then_hekaton_then_the_support_items() {
        let (mut t, droph) = setup();
        t.enter_as(1, 79, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, droph).contains("32711-01.htm"));
        assert!(t.event(1, NAME, "32711-03.htm").contains("32711-03.htm"));
        assert!(t.event(1, NAME, "32711-04.html").contains("32711-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The giants drop one sheet each for the member drawn (the killer here).
        assert!(t.quest_page(1, droph).contains("32711-07.html"));
        for giant in [25681, 25684, 25680] {
            let boss = t.spawn(giant, 60);
            t.kill(1, boss);
        }
        for sheet in SHEETS {
            assert_eq!(t.count(1, sheet), 1);
        }

        // With the three sheets Droph offers Hekaton Prime: the sheets go, the boss spawns once.
        assert!(t.quest_page(1, droph).contains("32711-08.html"));
        assert!(t.event(1, NAME, "spawn").contains("32711-09.html"));
        assert_eq!(t.npcs_of(HEKATON_PRIME).len(), 1);
        for sheet in SHEETS {
            assert_eq!(t.count(1, sheet), 0);
        }
        assert!(t.take_writes().is_empty());

        // The boss dies in a party: the members in cond 1 go to cond 2 (`setCond(2, true)`).
        let boss = t.npcs_of(HEKATON_PRIME)[0];
        t.enter_as(2, 1, race::HUMAN, player_class::FIGHTER, vec![]);
        t.party(1, 2);
        t.kill(1, boss);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Droph gives the support items and ends the quest.
        assert!(t.quest_page(1, droph).contains("32711-10.html"));
        assert_eq!(t.count(1, SUPPORT_ITEMS), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
