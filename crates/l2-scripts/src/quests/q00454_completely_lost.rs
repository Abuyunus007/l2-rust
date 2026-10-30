//! Quest 454: Completely Lost (the injured soldier follows a party to Ermian in the Seed of Infinity).
// Java: dist/game/data/scripts/quests/Q00454_CompletelyLost/Q00454_CompletelyLost.java
//
// Pattern: a daily escort. The NPC keeps the leader, the escort state and the whisper step in its
// own variables (`npc_variables_mut`); the quest timers watch the distance and the time limit, the
// NPC-to-NPC script events (`SCE_A_SEED_ESCORT_QUEST_*`) move the members' memo states, and
// `ON_CREATURE_ATTACKED` makes the soldier unhurtable.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, NpcSay};
use l2_util::ObjectId;
use l2_world::movement::heading_from_points;
use l2_world::script::{COMPLETED, CREATED, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TerminateReturn};

// NPCs
const INJURED_SOLDIER: i32 = 32738;
const ERMIAN: i32 = 32736;

// Misc
const MIN_LEVEL: i32 = 84;
const MOVE_TO: (i32, i32, i32) = (-180219, 186341, -10600);

/// Java `getRandomBoolean()` rewards, group 0: the recipes (one item) ...
const RECIPES_0: [i32; 9] = [15792, 15798, 15795, 15801, 15808, 15804, 15809, 15810, 15811];
/// ... and the pieces and gems (three items).
const PIECES_0: [i32; 9] = [15660, 15666, 15663, 15667, 15669, 15668, 15769, 15770, 15771];
const RECIPES_1: [i32; 8] = [15805, 15796, 15793, 15799, 15802, 15809, 15810, 15811];
const PIECES_1: [i32; 8] = [15672, 15664, 15661, 15670, 15671, 15769, 15770, 15771];
const RECIPES_2: [i32; 9] = [15800, 15803, 15806, 15807, 15797, 15794, 15809, 15810, 15811];
const PIECES_2: [i32; 9] = [15673, 15674, 15675, 15691, 15665, 15662, 15769, 15770, 15771];

/// Java's `if (chance < step) ... else if ((chance <= step) && (chance < 2 * step)) ... else last`.
// NOTE(java-quirk): after the first branch every test is `(chance <= n * step) && (chance < (n + 1)
// * step)`, so the second item only comes for `chance == step` and every later item holds the
// rolls up to its own multiple of `step`: the items are shifted one slot (the item of
// `chance < step` is the first, the next one only for exactly `step`). Ported as written.
fn pick(chance: i32, step: i32, items: &[i32]) -> i32 {
    if chance < step {
        return items[0];
    }
    for (n, item) in items.iter().enumerate().take(items.len() - 1).skip(1) {
        let low = step * n as i32;
        let high = step * (n as i32 + 1);
        if chance <= low && chance < high {
            return *item;
        }
    }
    items[items.len() - 1]
}

/// Java `Q00454_CompletelyLost`.
pub struct Q00454CompletelyLost;

/// The leader the soldier follows (Java `getVariables().getObject("leader", Player.class)`).
fn leader_of(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> Option<ObjectId> {
    let id = ctx.world().npc_variables(npc.object_id)?.get_int("leader", 0);
    (id != 0).then_some(ObjectId(id))
}

/// Java `leader.isInParty()` (an unknown player is in no party).
fn is_in_party(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world().party_of(player).is_some()
}

/// Java `leader.getParty().containsPlayer(player)`.
fn party_contains(ctx: &ScriptCtx<'_>, leader: ObjectId, player: ObjectId) -> bool {
    ctx.world().party_of(leader).is_some() && ctx.world().party_members_of(leader).contains(&player)
}

/// Java `leader.getParty().getMembers()` (just the leader without a party).
fn members_of(ctx: &ScriptCtx<'_>, leader: ObjectId) -> Vec<ObjectId> {
    ctx.world().party_members_of(leader)
}

/// Java `party.getLeaderObjectId()`.
fn party_leader_id(ctx: &ScriptCtx<'_>, player: ObjectId) -> i32 {
    ctx.world()
        .party_of(player)
        .and_then(|p| ctx.world().party_leader(p))
        .map_or(0, |l| l.0)
}

/// Java `whisper(npc, player, stringId)`.
fn whisper(ctx: &ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, string: NpcStringId) {
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

/// The cleanup the success and failure events share.
fn cancel_timers(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    for timer in [
        "CHECK_TIMER",
        "TIME_LIMIT1",
        "TIME_LIMIT2",
        "TIME_LIMIT3",
        "TIME_LIMIT4",
        "TIME_LIMIT5",
    ] {
        ctx.cancel_quest_timer(timer, Some(npc), None);
    }
}

/// Java `TIME_LIMITn`: the next timer and the line of the soldier.
fn time_limit(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, next: &str, string: NpcStringId) {
    if let Some(leader) = leader_of(ctx, npc) {
        ctx.start_quest_timer(next, 150_000, Some(npc), None, false);
        whisper(ctx, npc, leader, string);
    }
}

impl Script for Q00454CompletelyLost {
    fn name(&self) -> &'static str {
        "Q00454_CompletelyLost"
    }

    fn path(&self) -> &'static str {
        "quests/Q00454_CompletelyLost"
    }

    fn id(&self) -> i32 {
        454
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[INJURED_SOLDIER]);
        s.add_talk_id(&[INJURED_SOLDIER, ERMIAN]);
        s.add_move_finished_id(&[INJURED_SOLDIER]);
        s.add_event_received_id(&[INJURED_SOLDIER]);
        s.add_creature_see_id(&[INJURED_SOLDIER]);
        // `@RegisterEvent(ON_CREATURE_ATTACKED) @RegisterType(NPC) @Id(INJURED_SOLDIER)`.
        s.set_on_creature_attacked();
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // Java uses `npc` without a check in every case: all of these events come with an NPC.
        let npc = npc?;
        match event {
            "SAY_TIMER1" => {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::Gasp, &[]);
            }
            "SAY_TIMER2" => {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::SobToSeeErmianAgainCanIGoToMyFamilyNow,
                    &[],
                );
                ctx.start_quest_timer("EXPIRED_TIMER", 2000, Some(npc), None, false);
            }
            "CHECK_TIMER" => {
                if let Some(leader) = leader_of(ctx, npc)
                    && let (Some((nx, ny, _)), Some((lx, ly, _))) = (
                        ctx.world().position_of(npc.object_id),
                        ctx.world().position_of(leader),
                    )
                {
                    // Java `LocationUtil.calculateDistance(npc, leader, false, false)`: 2D, not squared.
                    let (dx, dy) = (f64::from(nx - lx), f64::from(ny - ly));
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 1000.0 {
                        let whisper_step = ctx
                            .world()
                            .npc_variables(npc.object_id)
                            .map_or(0, |v| v.get_int("whisper", 0));
                        if (dist > 5000.0 && dist < 6900.0) || (dist > 31000.0 && dist < 32000.0) {
                            ctx.tele_npc_to(npc, lx, ly, ctx.world().position_of(leader).map_or(0, |p| p.2));
                        } else if whisper_step == 0 {
                            whisper(ctx, npc, leader, NpcStringId::WhereAreYouICanTSeeAnything);
                            ctx.world_mut().npc_variables_mut(npc.object_id).set_int("whisper", 1);
                        } else if whisper_step == 1 {
                            whisper(ctx, npc, leader, NpcStringId::WhereAreYouReallyICanTFollowYouLikeThis);
                            ctx.world_mut().npc_variables_mut(npc.object_id).set_int("whisper", 2);
                        } else if whisper_step == 2 {
                            whisper(ctx, npc, leader, NpcStringId::IMSorryThisIsItForMe);
                            ctx.world_mut()
                                .script_send_event("SCE_A_SEED_ESCORT_QUEST_FAILURE", npc.object_id, npc.object_id, None);
                        }
                    }
                }

                ctx.start_quest_timer("CHECK_TIMER", 2000, Some(npc), None, false);
            }
            "TIME_LIMIT1" => {
                if let Some(leader) = leader_of(ctx, npc) {
                    ctx.start_quest_timer("TIME_LIMIT2", 150_000, Some(npc), None, false);
                    whisper(ctx, npc, leader, NpcStringId::IsItStillLongOff);
                }
            }
            "TIME_LIMIT2" => time_limit(
                ctx,
                npc,
                "TIME_LIMIT3",
                NpcStringId::IsErmianWellEvenICanTBelieveThatISurvivedInAPlaceLikeThis,
            ),
            "TIME_LIMIT3" => time_limit(
                ctx,
                npc,
                "TIME_LIMIT4",
                NpcStringId::IDonTKnowHowLongItSBeenSinceIPartedCompanyWithYouTimeDoesnTSeemToMoveItJustFeelsTooLong,
            ),
            "TIME_LIMIT4" => time_limit(
                ctx,
                npc,
                "TIME_LIMIT5",
                NpcStringId::SorryToSayThisButThePlaceYouStruckMeBeforeNowHurtsGreatly,
            ),
            "TIME_LIMIT5" => {
                if let Some(leader) = leader_of(ctx, npc) {
                    whisper(
                        ctx,
                        npc,
                        leader,
                        NpcStringId::UghIMSorryItLooksLikeThisIsItForMeIWantedToLiveAndSeeMyFamily,
                    );
                }

                ctx.world_mut()
                    .script_send_event("SCE_A_SEED_ESCORT_QUEST_FAILURE", npc.object_id, npc.object_id, None);
                ctx.start_quest_timer("EXPIRED_TIMER", 2000, Some(npc), None, false);
            }
            "EXPIRED_TIMER" => ctx.delete_npc(npc),
            _ => {}
        }

        // For NPC-only timers, player is null and no further checks or actions are required.
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32738-04.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    let quest_escort = ctx
                        .world()
                        .npc_variables(npc.object_id)
                        .map_or(0, |v| v.get_int("quest_escort", 0));
                    if quest_escort == 0 {
                        let v = ctx.world_mut().npc_variables_mut(npc.object_id);
                        v.set_int("leader", player.0);
                        v.set_int("quest_escort", 1);
                        if is_in_party(ctx, player) {
                            let leader_id = party_leader_id(ctx, player);
                            ctx.world_mut()
                                .npc_variables_mut(npc.object_id)
                                .set_int("partyId", leader_id);
                        }

                        qs.set_memo_state(ctx, 1);
                        qs.start_quest(ctx);
                        htmltext = Some(event.to_owned());
                    } else {
                        // Java dereferences `leader` here without a check.
                        let leader = leader_of(ctx, npc)?;
                        let leader_name = ctx.player_name(leader);
                        if is_in_party(ctx, leader) && party_contains(ctx, leader, player) {
                            qs.start_quest(ctx);
                            qs.set_memo_state(ctx, 1);
                            htmltext = ctx
                                .get_htm(player, "32738-04a.htm")
                                .map(|page| page.replace("%leader%", &leader_name));
                        } else {
                            htmltext = ctx
                                .get_htm(player, "32738-01b.htm")
                                .map(|page| page.replace("%leader%", &leader_name));
                        }
                    }
                }
            }
            "agree1" => {
                if qs.is_memo_state(ctx, 1)
                    && let Some(leader) = leader_of(ctx, npc)
                {
                    if is_in_party(ctx, leader) {
                        qs.set_memo_state(ctx, 2);
                        ctx.world_mut().script_send_event(
                            "SCE_A_SEED_ESCORT_QUEST_START",
                            npc.object_id,
                            npc.object_id,
                            None,
                        );
                        htmltext = Some("32738-06.html".to_owned());
                    } else {
                        htmltext = Some("32738-05a.html".to_owned());
                    }
                }
            }
            "agree2" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    htmltext = Some("32738-06.html".to_owned());
                    ctx.world_mut().script_send_event(
                        "SCE_A_SEED_ESCORT_QUEST_START",
                        npc.object_id,
                        npc.object_id,
                        None,
                    );
                    if let Some(leader) = leader_of(ctx, npc)
                        && is_in_party(ctx, leader)
                    {
                        let party_id = ctx
                            .world()
                            .npc_variables(npc.object_id)
                            .map_or(0, |v| v.get_int("partyId", 0));
                        let leader_id = party_leader_id(ctx, leader);
                        for member in members_of(ctx, leader) {
                            let qs_member = ctx.get_quest_state(member, false);
                            if let Some(qs_member) = qs_member
                                && qs_member.is_memo_state(ctx, 1)
                                && party_id == leader_id
                            {
                                qs_member.set_memo_state(ctx, 2);
                            }
                        }
                    }
                }
            }
            "32738-07.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_event_received(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_name: &str,
        _sender: ScriptNpc,
        receiver: ScriptNpc,
        _reference: Option<ObjectId>,
    ) -> Option<String> {
        match event_name {
            "SCE_A_SEED_ESCORT_QUEST_START" => {
                if let Some(leader) = leader_of(ctx, receiver) {
                    ctx.world_mut().set_npc_target(receiver.object_id, Some(leader));
                    ctx.world_mut().script_start_follow(receiver.object_id, leader);
                }

                ctx.start_quest_timer("CHECK_TIMER", 1000, Some(receiver), None, false);
                ctx.start_quest_timer("TIME_LIMIT1", 60_000, Some(receiver), None, false);
                ctx.set_script_value(receiver, 2);
                ctx.world_mut()
                    .npc_variables_mut(receiver.object_id)
                    .set_int("quest_escort", 99);
            }
            "SCE_A_SEED_ESCORT_QUEST_SUCCESS" | "SCE_A_SEED_ESCORT_QUEST_FAILURE" => {
                let success = event_name == "SCE_A_SEED_ESCORT_QUEST_SUCCESS";
                // The members whose memo state is 2 move on to 4 (success) or 3 (failure).
                let to = if success { 4 } else { 3 };
                if let Some(leader) = leader_of(ctx, receiver) {
                    let members = if is_in_party(ctx, leader) {
                        members_of(ctx, leader)
                    } else {
                        vec![leader]
                    };
                    for member in members {
                        let qs = ctx.get_quest_state(member, false);
                        if let Some(qs) = qs
                            && qs.is_memo_state(ctx, 2)
                        {
                            qs.set_memo_state(ctx, to);
                        }
                    }
                }

                if !success {
                    ctx.delete_npc(receiver);
                }

                // Timers cleanup
                cancel_timers(ctx, receiver);
            }
            _ => {}
        }
        None
    }

    fn on_move_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if ctx.is_script_value(npc, 2) {
            // Java `World.forFirstVisibleObjectInRange(npc, Npc.class, 300, id == ERMIAN, ...)`.
            let nearby = ctx
                .world()
                .visible_objects_in_range(npc.object_id, 300)
                .into_iter()
                .find(|o| ctx.script_npc(*o).is_some_and(|n| n.id == ERMIAN));
            if let Some(nearby) = nearby {
                ctx.set_script_value(npc, 3);
                ctx.world_mut().npc_set_intention_idle(npc.object_id);
                // Quest.addMoveToDesire(npc, MOVE_TO, 10000000): `setIntentionMoveTo(loc)`.
                ctx.npc_move_to(npc, MOVE_TO.0, MOVE_TO.1, MOVE_TO.2);
                ctx.world_mut().script_send_event(
                    "SCE_A_SEED_ESCORT_QUEST_SUCCESS",
                    npc.object_id,
                    npc.object_id,
                    None,
                );
                if let (Some((nx, ny, _)), Some((ex, ey, _))) = (
                    ctx.world().position_of(npc.object_id),
                    ctx.world().position_of(nearby),
                ) {
                    ctx.set_npc_heading(npc, heading_from_points(nx, ny, ex, ey));
                }
                ctx.start_quest_timer("SAY_TIMER2", 2000, Some(npc), None, false);
            }
        }
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_some() && ctx.is_script_value(npc, 0) {
            ctx.add_attack_desire(npc, creature, 10);
        }
    }

    fn on_creature_attacked(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _attacker: ObjectId,
        target: ObjectId,
    ) -> Option<TerminateReturn> {
        let npc = ctx.script_npc(target).filter(|n| n.id == INJURED_SOLDIER)?;
        ctx.set_script_value(npc, 1);
        ctx.world_mut().npc_set_intention_idle(npc.object_id);
        ctx.world_mut().set_intention_active(npc.object_id);
        ctx.start_quest_timer("SAY_TIMER1", 2000, Some(npc), None, false);
        Some(TerminateReturn::new(true))
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let mut state = qs.state(ctx);
        if state == COMPLETED {
            if !qs.is_now_available(ctx) {
                return Some("32738-02.htm".to_owned());
            }

            qs.set_state(ctx, CREATED);
            // break; fallthrough
            state = CREATED;
        }
        if state == CREATED {
            if ctx.player_level(player) >= MIN_LEVEL {
                let quest_escort = ctx
                    .world()
                    .npc_variables(npc.object_id)
                    .map_or(0, |v| v.get_int("quest_escort", 0));
                if quest_escort == 0 {
                    htmltext = "32738-01.htm".to_owned();
                } else if quest_escort == 99 {
                    htmltext = "32738-01c.htm".to_owned();
                } else {
                    // Java dereferences `leader` here without a check.
                    let leader = leader_of(ctx, npc)?;
                    let leader_name = ctx.player_name(leader);
                    if is_in_party(ctx, leader) && party_contains(ctx, leader, player) {
                        let name = ctx.player_name(player);
                        htmltext = ctx
                            .get_htm(player, "32738-01a.htm")
                            .map(|page| page.replace("%leader%", &leader_name).replace("%name%", &name))
                            .unwrap_or_default();
                    } else {
                        htmltext = ctx
                            .get_htm(player, "32738-01b.htm")
                            .map(|page| page.replace("%leader%", &leader_name))
                            .unwrap_or_default();
                    }
                }
            } else {
                htmltext = "32738-03.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                INJURED_SOLDIER => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32738-05.html".to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "32738-08.html".to_owned();
                    }
                }
                ERMIAN => match qs.get_memo_state(ctx) {
                    1 | 2 => htmltext = "32736-01.html".to_owned(),
                    3 => {
                        qs.exit_quest_type(ctx, QuestType::Daily, true);
                        htmltext = "32736-02.html".to_owned();
                    }
                    4 => {
                        let group = ctx.get_random(3);
                        let chance = ctx.get_random(100);
                        // The three groups: the step of the chance and the recipe / piece lists.
                        let (step, recipes, pieces): (i32, &[i32], &[i32]) = match group {
                            0 => (11, &RECIPES_0, &PIECES_0),
                            1 => (12, &RECIPES_1, &PIECES_1),
                            _ => (11, &RECIPES_2, &PIECES_2),
                        };
                        if ctx.get_random_boolean() {
                            ctx.give_items(player, pick(chance, step, recipes), 1);
                        } else {
                            ctx.give_items(player, pick(chance, step, pieces), 3);
                        }

                        qs.exit_quest_type(ctx, QuestType::Daily, true);
                        htmltext = "32736-03.html".to_owned();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00454_CompletelyLost";

    const NPCS: &str = r#"
        <npc id="32738" type="Folk" level="80" name="Injured Soldier"/>
        <npc id="32736" type="Folk" level="80" name="Ermian"/>"#;

    /// An item template for every reward id.
    fn items() -> String {
        let mut xml = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#,
        );
        for id in RECIPES_0
            .iter()
            .chain(&PIECES_0)
            .chain(&RECIPES_1)
            .chain(&PIECES_1)
            .chain(&RECIPES_2)
            .chain(&PIECES_2)
        {
            xml.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Reward {id}"><set name="is_stackable" val="true"/></item>"#
            ));
        }
        xml
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00454CompletelyLost);
        let pages = Pages::of(&Q00454CompletelyLost)
            .page("32738-01.htm", &["32738-04.htm"])
            .page("32738-04.htm", &["agree1", "32738-07.html"])
            .page("32738-05a.html", &["agree2", "32738-07.html"])
            .plain(&[
                "32738-02.htm",
                "32738-03.htm",
                "32738-05.html",
                "32738-06.html",
                "32738-07.html",
                "32738-08.html",
                "32736-01.html",
                "32736-02.html",
                "32736-03.html",
            ])
            .file(
                "data/scripts/quests/Q00454_CompletelyLost/32738-01a.htm",
                r#"<html><body>32738-01a.htm %leader% %name%<a action="bypass Script Q00454_CompletelyLost 32738-04.htm">go</a></body></html>"#,
            )
            .file(
                "data/scripts/quests/Q00454_CompletelyLost/32738-01b.htm",
                "<html><body>32738-01b.htm %leader%</body></html>",
            )
            .file(
                "data/scripts/quests/Q00454_CompletelyLost/32738-01c.htm",
                "<html><body>32738-01c.htm</body></html>",
            )
            .file(
                "data/scripts/quests/Q00454_CompletelyLost/32738-04a.htm",
                "<html><body>32738-04a.htm %leader%</body></html>",
            )
            .npcs(&[INJURED_SOLDIER, ERMIAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let soldier = t.spawn(INJURED_SOLDIER, 20);
        let ermian = t.spawn(ERMIAN, 200);
        (t, soldier, ermian)
    }

    #[test]
    fn a_level_83_character_is_sent_away() {
        let (mut t, soldier, _) = setup();
        t.enter(1, 83, vec![]);
        assert!(t.quest_page(1, soldier).contains("32738-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_keeps_the_leader_and_a_solo_player_cannot_agree() {
        let (mut t, soldier, _) = setup();
        t.enter(1, 84, vec![]);
        assert!(t.quest_page(1, soldier).contains("32738-01.htm"));
        assert!(t.event(1, NAME, "32738-04.htm").contains("32738-04.htm"));
        assert_eq!(
            t.world
                .npc_variables(soldier)
                .map(|v| v.get_int("quest_escort", 0)),
            Some(1)
        );
        assert_eq!(
            t.world.npc_variables(soldier).map(|v| v.get_int("leader", 0)),
            Some(1)
        );
        let writes = t.take_writes();
        assert!(
            writes.contains(&rows::insert(NAME, "<state>", "Started")),
            "{writes:?}"
        );
        // Alone: "the party leader must be in a party".
        assert!(t.event(1, NAME, "agree1").contains("32738-05a.html"));
    }

    #[test]
    fn the_escort_to_ermian_ends_with_a_reward_and_a_daily_exit() {
        let (mut t, soldier, ermian) = setup();
        t.enter(1, 84, vec![]);
        t.enter(2, 84, vec![]);
        t.party(1, 2);
        t.quest_page(1, soldier);
        t.event(1, NAME, "32738-04.htm");
        // The other member opens the page: the escort is taken by leader 1's party.
        assert!(t.quest_page(2, soldier).contains("32738-01a.htm"));
        t.event(2, NAME, "32738-04.htm");
        t.take_writes();
        assert!(t.event(1, NAME, "agree1").contains("32738-06.html"));
        // The queued event reaches the soldier on the next ticks: it follows the leader.
        t.ticks(2);
        assert!(t.world.is_npc_script_value(soldier, 2));
        assert_eq!(t.world.creature_target(soldier), Some(Oid(1)));
        assert_eq!(
            t.world
                .npc_variables(soldier)
                .map(|v| v.get_int("quest_escort", 0)),
            Some(99)
        );
        // A newcomer is told the escort left.
        t.enter(3, 84, vec![]);
        assert!(t.quest_page(3, soldier).contains("32738-01c.htm"));

        // The soldier stops next to Ermian: the success event moves the memo states 2 -> 4.
        t.world.run_as_script(NAME, |ctx| {
            Q00454CompletelyLost.on_move_finished(
                ctx,
                ScriptNpc {
                    object_id: soldier,
                    id: INJURED_SOLDIER,
                },
            );
        });
        t.ticks(2);
        assert!(t.world.is_npc_script_value(soldier, 3));
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            assert_eq!(qs.get_memo_state(ctx), 4);
        });

        // Ermian pays one item (the one of `pick`) and the quest ends daily.
        t.take_writes();
        t.take();
        assert!(t.quest_page(1, ermian).contains("32736-03.html"));
        let writes = t.take_writes();
        assert!(
            writes
                .iter()
                .any(|w| matches!(w, l2_world::QuestWrite::InsertVar { var, .. } if var == "restartTime")),
            "{writes:?}"
        );
        let mut all: Vec<i32> = RECIPES_0
            .iter()
            .chain(&PIECES_0)
            .chain(&RECIPES_1)
            .chain(&PIECES_1)
            .chain(&RECIPES_2)
            .chain(&PIECES_2)
            .copied()
            .collect();
        // Some gems and recipes are in more than one group.
        all.sort_unstable();
        all.dedup();
        let got: i64 = all.iter().map(|id| t.count(1, *id)).sum();
        assert!(got == 1 || got == 3, "{got}");
    }

    #[test]
    fn the_reward_roll_is_shifted_by_one_slot() {
        // `(chance <= 11) && (chance < 22)` holds only for 11 (the roll is >= 11 there).
        assert_eq!(pick(5, 11, &RECIPES_0), 15792);
        assert_eq!(pick(11, 11, &RECIPES_0), 15798);
        assert_eq!(pick(15, 11, &RECIPES_0), 15795);
        assert_eq!(pick(22, 11, &RECIPES_0), 15795);
        assert_eq!(pick(23, 11, &RECIPES_0), 15801);
        assert_eq!(pick(99, 11, &RECIPES_0), 15811);
        assert_eq!(pick(11, 12, &PIECES_1), 15672);
        assert_eq!(pick(12, 12, &PIECES_1), 15664);
        assert_eq!(pick(30, 12, &PIECES_1), 15670);
    }

    #[test]
    fn the_soldier_cannot_be_hurt_and_gasps() {
        let (mut t, soldier, _) = setup();
        t.enter(1, 84, vec![]);
        t.take();
        t.world.run_as_script(NAME, |ctx| {
            let r = Q00454CompletelyLost.on_creature_attacked(ctx, Oid(1), soldier);
            assert_eq!(r, Some(TerminateReturn::new(true)));
            // Anything else is none of its business.
            assert_eq!(
                Q00454CompletelyLost.on_creature_attacked(ctx, soldier, Oid(1)),
                None
            );
        });
        assert!(t.world.is_npc_script_value(soldier, 1));
        t.wait_ms(2_100);
        let says = names_to(&t.take(), 1).iter().filter(|n| *n == "NPC_SAY").count();
        assert_eq!(says, 1);
    }
}
