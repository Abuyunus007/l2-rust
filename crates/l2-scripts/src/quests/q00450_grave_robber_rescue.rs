//! Quest 450: Grave Robber Rescue (Kanemika at the Seed of Infinity, the wounded grave robbers).
// Java: dist/game/data/scripts/quests/Q00450_GraveRobberRescue/Q00450_GraveRobberRescue.java
//
// Pattern: a quest with a timer (`docs/design/scripts.md`, "Quest patterns"): a rescued robber
// walks away, `start_quest_timer("despawn", 3000, npc, player)` and the `"despawn"` branch of
// `on_event` deletes it (a timer is an event with no page). A robber that turns hostile is
// replaced with a monster (`add_spawn` + `add_attack_desire`). Daily: `exit_quest_type(Daily)`
// stores the next 6:30 and `is_now_available` reopens the quest.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KANEMIKA: i32 = 32650;
const WARRIOR: i32 = 32651;

// Monster
const WARRIOR_MON: i32 = 22741;

// Item
const EVIDENCE_OF_MIGRATION: i32 = 14876;

// Misc
const MIN_LEVEL: i32 = 80;

/// Java `Q00450_GraveRobberRescue`.
pub struct Q00450GraveRobberRescue;

impl Script for Q00450GraveRobberRescue {
    fn name(&self) -> &'static str {
        "Q00450_GraveRobberRescue"
    }

    fn path(&self) -> &'static str {
        "quests/Q00450_GraveRobberRescue"
    }

    fn id(&self) -> i32 {
        450
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KANEMIKA]);
        s.add_talk_id(&[KANEMIKA, WARRIOR]);
        s.register_quest_items(&[EVIDENCE_OF_MIGRATION]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = Some(event.to_owned());
        match event {
            "32650-04.htm" | "32650-05.htm" | "32650-06.html" => {}
            "32650-07.htm" => qs.start_quest(ctx),
            "despawn" => {
                // The timer carries the robber.
                if let Some(npc) = npc {
                    ctx.set_npc_busy(npc, false);
                    ctx.delete_npc(npc);
                }
                htmltext = None;
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        if npc.id == KANEMIKA {
            let mut state = qs.state(ctx);
            if state == COMPLETED {
                if !qs.is_now_available(ctx) {
                    return Some("32650-03.html".to_owned());
                }
                qs.set_state(ctx, CREATED);
                // fall through to CREATED
                state = CREATED;
            }
            match state {
                CREATED => {
                    htmltext = Some(
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "32650-01.htm"
                        } else {
                            "32650-02.htm"
                        }
                        .to_owned(),
                    );
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = Some(
                            if !ctx.has_quest_items(player, EVIDENCE_OF_MIGRATION) {
                                "32650-08.html"
                            } else {
                                "32650-09.html"
                            }
                            .to_owned(),
                        );
                    } else {
                        // Glory days reward: 6 886 980 exp, 8 116 410 sp, 371 400 Adena
                        ctx.give_adena(player, 65000, true);
                        qs.exit_quest_type(ctx, QuestType::Daily, true);
                        htmltext = Some("32650-10.html".to_owned());
                    }
                }
                _ => {}
            }
        } else if qs.is_cond(ctx, 1) {
            if ctx.is_npc_busy(npc) {
                return None;
            }

            if ctx.get_random(100) < 66 {
                ctx.give_items(player, EVIDENCE_OF_MIGRATION, 1);
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                    ctx.npc_move_to(npc, x + 100, y + 100, z);
                }
                ctx.set_npc_busy(npc, true);

                ctx.start_quest_timer("despawn", 3000, Some(npc), Some(player), false);
                if ctx.get_quest_items_count(player, EVIDENCE_OF_MIGRATION) == 10 {
                    qs.set_cond(ctx, 2, true);
                }

                htmltext = Some("32651-01.html".to_owned());
            } else {
                if ctx.get_random(100) < 50 {
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::GruntOh, &[]);
                } else {
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::GruntWhatSWrongWithMe, &[]);
                }

                // The robber's place and heading before it leaves the world (Java reads them
                // from the deleted object).
                let at = ctx.world().position_of(npc.object_id);
                let heading = ctx.heading_of(npc.object_id);
                ctx.delete_npc(npc);
                htmltext = None;

                if let Some((x, y, z)) = at
                    && let Some(monster) = ctx.add_spawn(WARRIOR_MON, x, y, z, heading, true, 600_000, 0)
                {
                    // `setRunning()`, `addDamageHate(player, 0, 999)`, `setIntentionAttack(player)`.
                    ctx.add_attack_desire(monster, player, 999);
                }
                ctx.show_on_screen_npc_string(
                    player,
                    NpcStringId::TheGraveRobberWarriorHasBeenFilledWithDarkEnergyAndIsAttackingYou,
                    5,
                    5000,
                    &[],
                );
            }
        }
        htmltext
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00450_GraveRobberRescue";

    const NPCS: &str = r#"
        <npc id="32650" type="Folk" level="70" name="Kanemika"/>
        <npc id="32651" type="Folk" level="70" name="Grave Robber Warrior"/>
        <npc id="22741" type="Monster" level="80" name="Grave Robber Warrior"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14876" type="EtcItem" name="Evidence of Migration"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00450GraveRobberRescue);
        let pages = Pages::of(&Q00450GraveRobberRescue)
            .page("32650-01.htm", &["32650-04.htm"])
            .page("32650-04.htm", &["32650-05.htm", "32650-07.htm"])
            .page("32650-05.htm", &["32650-06.html", "32650-07.htm"])
            .plain(&[
                "32650-02.htm",
                "32650-03.html",
                "32650-06.html",
                "32650-07.htm",
                "32650-08.html",
                "32650-09.html",
                "32650-10.html",
                "32651-01.html",
            ])
            .npcs(&[KANEMIKA, WARRIOR]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kanemika = t.spawn(KANEMIKA, 20);
        (t, kanemika)
    }

    #[test]
    fn rescue_ten_robbers_then_the_daily_lock() {
        let (mut t, kanemika) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, kanemika).contains("32650-01.htm"));
        t.event(1, NAME, "32650-04.htm");
        assert!(t.event(1, NAME, "32650-07.htm").contains("32650-07.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kanemika).contains("32650-08.html"));

        let (mut rescued, mut hostile) = (0, 0);
        while t.count(1, EVIDENCE_OF_MIGRATION) < 10 {
            let robber = t.spawn(WARRIOR, 40);
            let page = t.quest_page(1, robber);
            if page.contains("32651-01.html") {
                rescued += 1;
                // Busy while it walks away: the quest link gets nothing (`return null`).
                t.bypass(1, "Script");
                assert!(crate::testing::pages_to(&t.take(), 1).is_empty());
                // The "despawn" timer deletes it after 3 s.
                assert!(t.in_world(robber));
                assert_eq!(t.world.quest_timer_count(), 1);
                t.wait_ms(3_000);
                assert_eq!(t.world.quest_timer_count(), 0);
                assert!(!t.in_world(robber));
            } else {
                hostile += 1;
                // It turned: gone, and a monster attacks in its place (taken away again, before
                // it kills the test character).
                assert!(!t.in_world(robber), "{page}");
                let monsters = t.npcs_of(WARRIOR_MON);
                assert_eq!(monsters.len(), 1);
                assert!(
                    t.world
                        .aggro_list(monsters[0])
                        .iter()
                        .any(|a| a.attacker == Oid(1))
                );
                t.world.delete_npc(monsters[0]);
            }
        }
        assert_eq!(rescued, 10);
        assert!(hostile > 0, "seed 7 should turn at least one robber");
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The reward and the daily lock.
        t.take();
        assert!(t.quest_page(1, kanemika).contains("32650-10.html"));
        assert_eq!((t.count(1, 57), t.count(1, EVIDENCE_OF_MIGRATION)), (65000, 0));
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(matches!(&writes[2], l2_world::QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, kanemika).contains("32650-03.html"));

        // After 6:30 the quest is offered again: `setState(CREATED)` writes the state row (Java's
        // name of CREATED is "Start").
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set(ctx, "restartTime", "0");
        });
        t.take_writes();
        assert!(t.quest_page(1, kanemika).contains("32650-01.htm"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "<state>", "Start")]);
    }

    #[test]
    fn the_hostile_robber_says_why() {
        let (mut t, kanemika) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, kanemika);
        t.event(1, NAME, "32650-04.htm");
        t.event(1, NAME, "32650-07.htm");
        loop {
            let robber = t.spawn(WARRIOR, 40);
            t.talk(1, robber);
            t.bypass(1, "Script");
            let names = names_to(&t.take(), 1);
            if !t.in_world(robber) {
                assert!(names.contains(&"NPC_SAY".to_owned()));
                assert!(names.contains(&"EX_SHOW_SCREEN_MESSAGE".to_owned()));
                break;
            }
            t.wait_ms(3_000);
        }
    }
}
