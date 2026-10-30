//! Quest 457: Lost and Found (Gumiel follows the player out of the Solina area; a daily escort).
// Java: dist/game/data/scripts/quests/Q00457_LostAndFound/Q00457_LostAndFound.java
//
// Pattern: an escort. The NPC follows the player (`script_start_follow`) and a repeating quest
// timer ("CHECK") watches the distance and the escort checkers; the timers of the NPC's lines and
// the time limit are quest timers too. Gumiel can also appear next to a killed Solina (1%).
//
// FOLLOW-UP(engine): `npc.setWalking()` (the escort walks instead of running) has no engine call.
// FOLLOW-UP(engine): `SpawnTable.getSpawns(ESCORT_CHECKER)` and `Spawn.getLastSpawn()` have no
// engine counterpart. The checkers are the NPCs `on_spawn` reported that are still in the world:
// the same set as Java's last spawns while the checkers are alive (see `EscortCheckers`).

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, CreatureSay, NpcSay, SayBody, SenderField};
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestType};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GUMIEL: i32 = 32759;
const ESCORT_CHECKER: i32 = 32764;
const SOLINA_CLAN: [i32; 4] = [
    22789, // Guide Solina
    22790, // Seeker Solina
    22791, // Savior Solina
    22793, // Ascetic Solina
];

// Misc
const PACKAGED_BOOK: i32 = 15716;
/// Java `CHANCE_SPAWN`: 1 %, out of 100.
const CHANCE_SPAWN: i32 = 1;
const MIN_LV: i32 = 82;

/// Java `_escortCheckers`: the escort checkers `on_spawn` reported. See the FOLLOW-UP above.
#[derive(Default)]
struct EscortCheckers(Vec<ObjectId>);

/// Java `Q00457_LostAndFound`.
pub struct Q00457LostAndFound;

impl Script for Q00457LostAndFound {
    fn name(&self) -> &'static str {
        "Q00457_LostAndFound"
    }

    fn path(&self) -> &'static str {
        "quests/Q00457_LostAndFound"
    }

    fn id(&self) -> i32 {
        457
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUMIEL]);
        s.add_spawn_id(&[ESCORT_CHECKER]);
        s.add_first_talk_id(&[GUMIEL]);
        s.add_talk_id(&[GUMIEL]);
        s.add_kill_id(&SOLINA_CLAN);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == ESCORT_CHECKER {
            let checkers = &mut ctx.state::<EscortCheckers>().0;
            if !checkers.contains(&npc.object_id) {
                checkers.push(npc.object_id);
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // Java `return getNoQuestMsg(player)` (not null) when there is no state.
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };
        // Java calls `npc.setScriptValue` and friends without a check: every event has an NPC.
        let npc = npc?;

        let mut htmltext = None;
        match event {
            "32759-06.html" => {
                ctx.set_script_value(npc, 0);
                qs.start_quest(ctx);
                ctx.world_mut().set_npc_target(npc.object_id, Some(player));
                // FOLLOW-UP(engine): `npc.setWalking()`.
                ctx.world_mut().script_start_follow(npc.object_id, player);
                ctx.start_quest_timer("CHECK", 1_000, Some(npc), Some(player), true);
                ctx.start_quest_timer("TIME_LIMIT", 600_000, Some(npc), Some(player), false);
                ctx.start_quest_timer("TALK_TIME", 120_000, Some(npc), Some(player), false);
                ctx.start_quest_timer("TALK_TIME2", 30_000, Some(npc), Some(player), false);
            }
            "TALK_TIME" => {
                npc_say(ctx, npc, player, NpcStringId::AhIThinkIRememberThisPlace, false);
            }
            "TALK_TIME2" => {
                npc_say(ctx, npc, player, NpcStringId::WhatWereYouDoingHere, false);
                ctx.start_quest_timer("TALK_TIME3", 10 * 1_000, Some(npc), Some(player), false);
            }
            "TALK_TIME3" => {
                npc_say(
                    ctx,
                    npc,
                    player,
                    NpcStringId::IGuessYouReTheSilentTypeThenAreYouLookingForTreasureLikeMe,
                    false,
                );
            }
            "TIME_LIMIT" => {
                ctx.start_quest_timer("STOP", 2_000, Some(npc), Some(player), false);
                qs.exit_quest_type(ctx, QuestType::Daily, false);
            }
            "CHECK" => {
                // Java `npc.calculateDistance3D(player)`: both are in the world while the escort runs.
                let distance = ctx.distance_3d(npc.object_id, player).unwrap_or(0.0);
                if distance > 1_000.0 {
                    if distance > 5_000.0 {
                        ctx.start_quest_timer("STOP", 2_000, Some(npc), Some(player), false);
                        qs.exit_quest_type(ctx, QuestType::Daily, false);
                    } else if ctx.is_script_value(npc, 0) {
                        npc_say(ctx, npc, player, NpcStringId::HeyDonTGoSoFast, true);
                        ctx.set_script_value(npc, 1);
                    } else if ctx.is_script_value(npc, 1) {
                        npc_say(ctx, npc, player, NpcStringId::ItSHardToFollow, true);
                        ctx.set_script_value(npc, 2);
                    } else if ctx.is_script_value(npc, 2) {
                        ctx.start_quest_timer("STOP", 2_000, Some(npc), Some(player), false);
                        qs.exit_quest_type(ctx, QuestType::Daily, false);
                    }
                }

                // Java `for (Spawn escortSpawn : _escortCheckers)`: the first checker within 1000
                // (2D, strictly) ends the escort with the book.
                let checkers = checkers_in_world(ctx);
                for escort in checkers {
                    if inside_radius_2d(ctx, npc.object_id, escort, 1_000) {
                        ctx.start_quest_timer("STOP", 1_000, Some(npc), Some(player), false);
                        ctx.start_quest_timer("BYE", 3_000, Some(npc), Some(player), false);
                        ctx.cancel_quest_timer("CHECK", Some(npc), Some(player));
                        ctx.world().broadcast_packet(
                            npc.object_id,
                            &CreatureSay {
                                sender_object_id: npc.object_id.0,
                                chat_type: ChatType::NpcGeneral,
                                sender: SenderField::Name(ctx.world().npc_template_name(npc.object_id)),
                                message_id: NpcStringId::AhFreshAir.id(),
                                body: SayBody::Parameters(Vec::new()),
                            },
                        );
                        npc_say(ctx, npc, player, NpcStringId::AhFreshAir, false);
                        ctx.give_items(player, PACKAGED_BOOK, 1);
                        qs.exit_quest_type(ctx, QuestType::Daily, true);
                        break;
                    }
                }
            }
            "STOP" => {
                ctx.world_mut().set_npc_target(npc.object_id, None);
                ctx.world_mut().script_stop_follow(npc.object_id);
                ctx.world_mut().npc_set_intention_idle(npc.object_id);
                ctx.cancel_quest_timer("CHECK", Some(npc), Some(player));
                ctx.cancel_quest_timer("TIME_LIMIT", Some(npc), Some(player));
                ctx.cancel_quest_timer("TALK_TIME", Some(npc), Some(player));
                ctx.cancel_quest_timer("TALK_TIME2", Some(npc), Some(player));
            }
            "BYE" => ctx.delete_npc(npc),
            _ => htmltext = Some(event.to_owned()),
        }
        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        // Java `npc.getTarget()`: the escort's target is the player it walks with. (The NPC's
        // target is `World::creature_target`; `ScriptCtx::target_of` reads players only.)
        match ctx.world().creature_target(npc.object_id) {
            Some(target) => Some(
                if target == player {
                    "32759-08.html"
                } else {
                    "32759-01a.html"
                }
                .to_owned(),
            ),
            None => Some("32759.html".to_owned()),
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Java `getQuestState(player, true)`: the state is made here if the player has none.
        let Some(qs) = ctx.get_quest_state(killer, true) else {
            return;
        };
        if ctx.get_random(100) < CHANCE_SPAWN
            && qs.is_now_available(ctx)
            && ctx.player_level(killer) >= MIN_LV
        {
            // Java `addSpawn(GUMIEL, npc)`: where the Solina died.
            ctx.add_spawn_at(GUMIEL, npc.object_id, false, 0);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = ctx.get_no_quest_msg(player);
        let qs = ctx.get_quest_state(player, true)?;
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LV {
                    "32759-01.htm"
                } else {
                    "32759-03.html"
                }
                .to_owned();
            }
            COMPLETED => {
                if qs.is_now_available(ctx) {
                    qs.set_state(ctx, CREATED);
                    htmltext = if ctx.player_level(player) >= MIN_LV {
                        "32759-01.htm"
                    } else {
                        "32759-03.html"
                    }
                    .to_owned();
                } else {
                    htmltext = "32759-02.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `npc.broadcastSay`-like lines of Gumiel: a whisper goes to the player; the other kind is
/// Java's `npc.sendPacket(...)`, which an NPC never gets (`WorldObject.sendPacket` is empty).
fn npc_say(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, string: NpcStringId, whisper: bool) {
    if whisper {
        let packet = NpcSay {
            object_id: npc.object_id.0,
            text_type: ChatType::Whisper.client_id(),
            npc_id: npc.id,
            npc_string: string.id(),
            text: String::new(),
            parameters: Vec::new(),
        };
        ctx.world().send(player, &packet);
    }
    // NOTE(java-quirk): `broadcastNpcSay(npc, player, id, false)` calls `npc.sendPacket(...)`, and
    // `WorldObject.sendPacket` does nothing, so the NPC's line is never sent to anybody.
}

/// The escort checkers that are still in the world, in the order `on_spawn` saw them. The list is
/// pruned to those (Java's last spawns of the spawns of `ESCORT_CHECKER`).
fn checkers_in_world(ctx: &mut ScriptCtx<'_>) -> Vec<ObjectId> {
    let known = ctx.state::<EscortCheckers>().0.clone();
    let alive: Vec<ObjectId> = known
        .into_iter()
        .filter(|id| ctx.world().position_of(*id).is_some())
        .collect();
    ctx.state::<EscortCheckers>().0.clone_from(&alive);
    alive
}

/// Java `npc.isInsideRadius2D(escort, radius)`: the 2D distance is strictly below the radius.
fn inside_radius_2d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId, radius: i32) -> bool {
    match (ctx.world().position_of(a), ctx.world().position_of(b)) {
        (Some((ax, ay, _)), Some((bx, by, _))) => {
            let dx = f64::from(ax - bx);
            let dy = f64::from(ay - by);
            (dx * dx + dy * dy).sqrt() < f64::from(radius)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, pages_to, player_data, rows};

    const NAME: &str = "Q00457_LostAndFound";

    const NPCS: &str = r#"
        <npc id="32759" type="Folk" level="70" name="Gumiel"/>
        <npc id="32764" type="Folk" level="70" name="Escort Checker"/>
        <npc id="22789" type="Monster" level="80" name="Guide Solina"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15716" type="EtcItem" name="Packaged Book"/>"#;

    /// Talks to Gumiel and opens its quest window. The first-talk page links to it with
    /// `bypass -h Script Q00457_LostAndFound`, which the world caches as the text after `-h`.
    fn gumiel_page(t: &mut ScriptTest, npc: Oid) -> String {
        t.talk(1, npc);
        t.bypass(1, "Script Q00457_LostAndFound");
        pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00457LostAndFound);
        let pages = Pages::of(&Q00457LostAndFound)
            .page("32759-01.htm", &["32759-04.html"])
            .page("32759-04.html", &["32759-06.html", "32759-05.html"])
            .page("32759-05.html", &["32759-06.html", "32759-07.html"])
            .plain(&[
                "32759-01a.html",
                "32759-02.html",
                "32759-03.html",
                "32759-06.html",
                "32759-07.html",
                "32759-08.html",
            ])
            // The first-talk page: its own link opens the quest window.
            .file(
                "data/scripts/quests/Q00457_LostAndFound/32759.html",
                r#"<html><body>32759.html<a action="bypass -h Script Q00457_LostAndFound">Quest</a></body></html>"#,
            )
            .npcs(&[GUMIEL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gumiel = t.spawn(GUMIEL, 20);
        (t, gumiel)
    }

    #[test]
    fn a_character_below_level_82_is_sent_away() {
        let (mut t, gumiel) = setup();
        t.enter(1, 81, vec![]);
        assert!(gumiel_page(&mut t, gumiel).contains("32759-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_escort_ends_at_the_checker_with_the_book() {
        let (mut t, gumiel) = setup();
        t.enter(1, 82, vec![]);
        assert!(gumiel_page(&mut t, gumiel).contains("32759-01.htm"));
        // The accept link returns no page (Java's `case` leaves `htmltext` null).
        assert!(t.event(1, NAME, "32759-04.html").contains("32759-04.html"));
        assert_eq!(t.event(1, NAME, "32759-06.html"), "");
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.world.creature_target(gumiel), Some(Oid(1)));

        // The first talk of a walking NPC: its target is the player, so the other page.
        t.talk(1, gumiel);
        assert!(pages_contain(&mut t, "32759-08.html"));

        // A checker next to Gumiel: the next CHECK (one second) ends the escort with the book.
        let checker = t.spawn(ESCORT_CHECKER, 30);
        assert!(t.in_world(checker));
        t.wait_ms(1_000);
        assert_eq!(t.count(1, PACKAGED_BOOK), 1);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                QuestWrite::DeleteQuest {
                    char_id: 1,
                    name: NAME.to_owned(),
                    repeatable: false,
                },
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));

        // The NPC leaves 3 s later.
        t.wait_ms(3_000);
        assert!(!t.in_world(gumiel));
    }

    /// Whether the last page the player got contains `text`.
    fn pages_contain(t: &mut ScriptTest, text: &str) -> bool {
        let out = t.take();
        pages_to(&out, 1).iter().any(|p| p.contains(text))
    }

    #[test]
    fn a_solina_kill_can_bring_gumiel_back() {
        // Gumiel appears where a Solina died with a 1% chance per kill, for a character that has
        // finished the quest and whose restart time has passed (`isNowAvailable()`).
        let (mut t, _gumiel) = setup();
        let mut data = player_data(1, 82, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Completed".to_owned()))],
            variables: vec![(NAME.to_owned(), "restartTime".to_owned(), Some("0".to_owned()))],
        };
        t.enter_with(data);
        let mut spawned = false;
        for _ in 0..1_000 {
            let solina = t.spawn(22789, 60);
            t.kill(1, solina);
            // The setup's Gumiel is the first one; a kill that brings another one is the chance.
            if t.npcs_of(GUMIEL).len() > 1 {
                spawned = true;
                break;
            }
        }
        assert!(spawned, "no Gumiel after 1000 kills");
    }

    #[test]
    fn a_character_who_never_did_the_quest_never_meets_gumiel_from_a_kill() {
        // Java `isNowAvailable()` is false without a `restartTime` row, so no kill can spawn it.
        let (mut t, _gumiel) = setup();
        t.enter(1, 82, vec![]);
        for _ in 0..200 {
            let solina = t.spawn(22789, 60);
            t.kill(1, solina);
        }
        assert_eq!(t.npcs_of(GUMIEL).len(), 1);
    }
}
