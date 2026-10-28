//! What the four Mounted Troop instances of the Seed of Destruction share.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/SeedOfDestruction/MountedTroops/
//       {Chamblains,Soldiers,Warriors,GreatWarriors}MountedTroop.java
//
// The four Java classes are copies of one another that differ in the template id, the doorman,
// the monsters of the two halls and the `difficulty` they write to `Q00693_DefeatingDragonkinRemnants`
// (checked with `diff`). Each Rust script holds that data as a [`Spec`] and calls these functions,
// which are the Java methods line by line.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::instance::EnterOutcome;
use l2_world::{ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::util::{PolygonTerritory, java_hash_map_order};

const MAX_PLAYERS: usize = 9;
const MIN_LEVEL: i32 = 75;
const EDRIC: i32 = 32527;

const REMNANT_MACHINE: i32 = 18703;
const SEED_CONTROLLER: i32 = 32602;

const DOORS: [i32; 2] = [12240001, 12240002];

const ENTER_TELEPORT_LOC: (i32, i32, i32) = (-242754, 219982, -10011);
/// `(x, y, z, heading)`
const DOORMAN_SPAWN_LOC: (i32, i32, i32, i32) = (-239504, 219984, -10112, 32767);
const REMNANT_MACHINE_LOC: (i32, i32, i32, i32) = (-238320, 219983, -10112, 0);

const HALL_ZONE_1_COORDINATES: [[i32; 4]; 8] = [
    [-240766, 219168, -10178, -9978],
    [-240755, 220795, -10178, -9978],
    [-240667, 220889, -10178, -9978],
    [-239595, 220886, -10178, -9978],
    [-239487, 220765, -10178, -9978],
    [-239490, 219170, -10178, -9978],
    [-239583, 219059, -10178, -9978],
    [-240665, 219065, -10178, -9978],
];

const HALL_ZONE_2_COORDINATES: [[i32; 4]; 8] = [
    [-238353, 219165, -10175, -9975],
    [-238340, 220788, -10175, -9975],
    [-238431, 220888, -10175, -9975],
    [-238933, 220890, -10175, -9975],
    [-239027, 220789, -10175, -9975],
    [-239034, 219195, -10175, -9975],
    [-238924, 219067, -10175, -9975],
    [-238445, 219065, -10175, -9975],
];

/// Java `Q00693_DefeatingDragonkinRemnants.class.getSimpleName()`.
const DEFEATING_DRAGONKIN_REMNANTS: &str = "Q00693_DefeatingDragonkinRemnants";

/// The 24 hours of the instance lock (`86400000`).
const LOCK_MS: i64 = 86_400_000;

/// What differs between the four instances.
pub struct Spec {
    pub name: &'static str,
    pub template_id: i32,
    pub doorman: i32,
    /// `HALL_1_SPAWNS` as `(npc id, count)` in the order Java `put`s them.
    pub hall_1_spawns: &'static [(i32, i32)],
    /// `HALL_2_SPAWNS`.
    pub hall_2_spawns: &'static [(i32, i32)],
    /// The `difficulty` written to the quest.
    pub difficulty: i32,
}

/// The folder of the pages and of all four scripts.
pub const PATH: &str = "ai/areas/Gracia/instances/SeedOfDestruction/MountedTroops";

/// The constructor: `addStartNpc(EDRIC); addTalkId(EDRIC); addKillId(DOORMAN, REMNANT_MACHINE);`.
pub fn setup(spec: &Spec, s: &mut ScriptSetup) {
    s.add_start_npc(&[EDRIC]);
    s.add_talk_id(&[EDRIC]);
    s.add_kill_id(&[spec.doorman, REMNANT_MACHINE]);
}

/// `onEvent`.
pub fn on_event(
    spec: &Spec,
    ctx: &mut ScriptCtx<'_>,
    event: &str,
    npc: Option<ScriptNpc>,
    player: Option<ObjectId>,
) -> Option<String> {
    match event {
        "enter" => {
            let (npc, player) = (npc?, player?);
            if npc.id == EDRIC && check_conditions(spec, ctx, player) {
                for party_member in ctx.world().party_members_of(player) {
                    let now = ctx.now_ms();
                    ctx.world_mut()
                        .set_instance_time(party_member, spec.template_id, now);
                    // `enterInstance(partyMember, TEMPLATE_ID)`: `onEnterInstance` adds him to the
                    // world, the first entrance sets the instance up.
                    let outcome = ctx.world_mut().enter_instance(
                        party_member,
                        spec.template_id,
                        |_, _| true,
                        |world, member, instance_id, _first| {
                            if let Some(instance_world) = world.instance_world_mut(instance_id) {
                                instance_world.add_allowed(member);
                            }
                        },
                    );
                    if let EnterOutcome::New(instance_id) = outcome {
                        first_entrance(spec, ctx, party_member, instance_id);
                    }
                }

                return Some("32527-entrance.html".to_owned());
            }
        }
        "DOORMAN_KILLED" => {
            let npc = npc?;
            let instance_id = ctx.world().instance_id_of(npc.object_id);
            ctx.open_door(DOORS[0], instance_id);
            ctx.open_door(DOORS[1], instance_id);
        }
        _ => {}
    }

    None
}

/// `checkConditions(player)`.
fn check_conditions(spec: &Spec, ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    if ctx.world().party_of(player).is_none() {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter),
        );
        return false;
    }

    if !ctx.world().is_party_leader(player) {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter),
        );
        return false;
    }

    let members = ctx.world().party_members_of(player);
    if members.len() > MAX_PLAYERS {
        ctx.world().send(
            player,
            &SystemMessage::new(SystemMessageId::YouCannotEnterDueToThePartyHavingExceededTheLimit),
        );
        return false;
    }

    for &party_member in &members {
        if ctx.player_level(party_member) < MIN_LEVEL {
            broadcast_to_party(
                ctx,
                &members,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                party_member,
            );
            return false;
        }

        if !ctx.world().check_if_in_range(1000, player, party_member, true) {
            broadcast_to_party(
                ctx,
                &members,
                SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                party_member,
            );
            return false;
        }

        if ctx.now_ms() < ctx.world().instance_time(party_member, spec.template_id) + LOCK_MS {
            let html = ctx.get_htm(player, "32527-instance-lock.html");
            ctx.show_result(player, html.as_deref(), None);

            broadcast_to_party(ctx, &members, SystemMessageId::C1MayNotReEnterYet, party_member);
            return false;
        }

        if ctx.world().script_is_flying_mounted(party_member) {
            ctx.world().send(
                party_member,
                &SystemMessage::new(SystemMessageId::YouCannotEnterASeedWhileInAFlyingTransformationState),
            );
            return false;
        }
    }

    true
}

/// `sm.addPcName(partyMember); party.broadcastPacket(sm);`
fn broadcast_to_party(ctx: &ScriptCtx<'_>, members: &[ObjectId], message: SystemMessageId, named: ObjectId) {
    let name = ctx.player_name(named);
    let sm = SystemMessage::new(message).with(SystemMessageParam::PlayerName(name));
    for &member in members {
        ctx.world().send(member, &sm);
    }
}

/// The `firstEntrance` part of `onEnterInstance`.
fn first_entrance(spec: &Spec, ctx: &mut ScriptCtx<'_>, player: ObjectId, instance_id: i32) {
    let (x, y, z, heading) = DOORMAN_SPAWN_LOC;
    if let Some(door_man) = ctx.add_spawn(spec.doorman, x, y, z, heading, false, 0, instance_id) {
        ctx.world_mut()
            .script_set_random_walking(door_man.object_id, true);
        set_parameter(ctx, instance_id, "doorMan", door_man.object_id);
    }

    let (x, y, z, heading) = REMNANT_MACHINE_LOC;
    if let Some(remnant_machine) = ctx.add_spawn(REMNANT_MACHINE, x, y, z, heading, false, 0, instance_id) {
        ctx.world_mut()
            .script_disable_core_ai(remnant_machine.object_id, true);
        ctx.world_mut().set_immobilized(remnant_machine.object_id, true);
        set_parameter(ctx, instance_id, "remnantMachine", remnant_machine.object_id);
    }

    // Spawn mobs in the 1st hall, then in the 2nd hall.
    spawn_hall(ctx, instance_id, 1, HALL_ZONE_1_COORDINATES, spec.hall_1_spawns);
    spawn_hall(ctx, instance_id, 2, HALL_ZONE_2_COORDINATES, spec.hall_2_spawns);

    // Spawn temporary teleporter.
    let (x, y, z) = ENTER_TELEPORT_LOC;
    ctx.add_spawn(SEED_CONTROLLER, x, y, z, 0, false, 0, instance_id);
    ctx.close_door(DOORS[0], instance_id);
    ctx.close_door(DOORS[1], instance_id);

    let members = ctx.world().party_members_of(player);
    if ctx.world().party_of(player).is_some() {
        for players in &members {
            let Some(qs) = ctx.quest_state_of(DEFEATING_DRAGONKIN_REMNANTS, *players, false) else {
                continue;
            };
            qs.set_int(ctx, "difficulty", spec.difficulty);
            qs.set_memo_state(ctx, 2);
            qs.set_int(ctx, "members", i32::try_from(members.len()).unwrap_or(0));
            if let Some(instance_world) = ctx.world_mut().instance_world_mut(instance_id) {
                instance_world.add_allowed(*players);
            }

            ctx.teleport_player(*players, x, y, z, instance_id, true);
        }
    }
}

/// `world.setParameter(name, npc)`: the parameters of the engine's world hold text, so the NPC is
/// kept as its object id.
fn set_parameter(ctx: &mut ScriptCtx<'_>, instance_id: i32, name: &str, npc: ObjectId) {
    if let Some(instance_world) = ctx.world_mut().instance_world_mut(instance_id) {
        instance_world
            .parameters
            .insert(name.to_owned(), npc.0.to_string());
    }
}

/// One of the two "Spawn mobs in the Nth hall" loops.
fn spawn_hall(
    ctx: &mut ScriptCtx<'_>,
    instance_id: i32,
    hall: i32,
    coordinates: [[i32; 4]; 8],
    spawns: &[(i32, i32)],
) {
    let mut zone = PolygonTerritory::new(hall);
    for coord in coordinates {
        zone.add(coord[0], coord[1], coord[2], coord[3], 0);
    }

    // `HashMap.entrySet()` order.
    for (npc_id, count) in java_hash_map_order(spawns) {
        for _ in 0..count {
            let Some((x, y, z)) = zone.random_point(ctx) else {
                // Java would throw a NullPointerException on the null location.
                continue;
            };
            let heading = ctx.get_random(65536);
            if let Some(spawn) = ctx.add_spawn(npc_id, x, y, z, heading, false, 0, instance_id) {
                ctx.world_mut().script_set_random_walking(spawn.object_id, true);
            }
        }
    }
}

/// `onKill`.
pub fn on_kill(spec: &Spec, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId) {
    let instance_id = ctx.world().instance_id_of(npc.object_id);
    let Some(world) = ctx.world().instance_world(instance_id) else {
        return;
    };

    let door_man = world
        .parameters
        .get("doorMan")
        .and_then(|id| id.parse::<i32>().ok());
    if door_man == Some(npc.object_id.0) {
        ctx.start_quest_timer("DOORMAN_KILLED", 2000, Some(npc), None, false);
    }

    if npc.id == REMNANT_MACHINE {
        for monster in ctx.world().script_alive_monsters_of_instance(instance_id) {
            ctx.world_mut().delete_npc(monster);
        }

        if ctx.world().party_of(killer).is_some() {
            for member in ctx.world().party_members_of(killer) {
                if ctx.world().instance_id_of(member) == instance_id {
                    if let Some(qs) = ctx.quest_state_of(DEFEATING_DRAGONKIN_REMNANTS, member, false) {
                        qs.set_memo_state(ctx, 3);
                    }
                }
            }
        }

        // Finish Instance.
        ctx.cancel_quest_timers("FAILED_IN_TIME");

        if ctx.world().instance(instance_id).is_some() {
            let finish_time = ctx.world().script_instance_finish_time_ms();
            ctx.world_mut().set_instance_duration(instance_id, finish_time);
            ctx.world_mut().set_instance_empty_destroy_time(instance_id, 0);
        }
    }

    // The doorman id of a spec is registered for kills; nothing else is read from it.
    let _ = spec;
}

#[cfg(test)]
pub(crate) mod tests {
    use l2_world::{ClientInput, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{ScriptTest, names_to, pages_to, player_data};

    pub const NPCS: &str = r#"<npc id="32527" type="Folk" level="70" name="Edric"/>
        <npc id="32602" type="Folk" level="70" name="Seed Controller"/>
        <npc id="18703" type="Monster" level="80" name="Remnant Machine"/>
        <npc id="18783" type="Monster" level="80" name="Commander"/>
        <npc id="18784" type="Monster" level="80" name="Batalion Commander"/>
        <npc id="18785" type="Monster" level="80" name="White Dragon Leader"/>
        <npc id="18786" type="Monster" level="80" name="Infantry"/>
        <npc id="18787" type="Monster" level="80" name="Magic Leader"/>
        <npc id="18788" type="Monster" level="80" name="Magician"/>
        <npc id="18789" type="Monster" level="80" name="Magic Soldier"/>
        <npc id="18790" type="Monster" level="80" name="Healer"/>"#;

    /// Java `Q00693_DefeatingDragonkinRemnants` as far as the troops need it: a quest of that name.
    pub struct DragonkinRemnants;

    impl Script for DragonkinRemnants {
        fn name(&self) -> &'static str {
            DEFEATING_DRAGONKIN_REMNANTS
        }
        fn path(&self) -> &'static str {
            "quests/Q00693_DefeatingDragonkinRemnants"
        }
        fn id(&self) -> i32 {
            693
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    /// A world with the script `registry` holds, Edric, and a party of `levels` (the first is the
    /// leader, object ids 1, 2, ...). Returns the world and Edric.
    pub fn world(registry: ScriptRegistry, script: &str, levels: &[i32]) -> (ScriptTest, ObjectId) {
        let dir = format!("data/scripts/{PATH}");
        let pages = [
            (
                format!("{dir}/32527-entrance.html"),
                "<html><body>entrance</body></html>".to_owned(),
            ),
            (
                format!("{dir}/32527-instance-lock.html"),
                "<html><body>lock</body></html>".to_owned(),
            ),
            (
                "data/html/default/32527.htm".to_owned(),
                format!(
                    r#"<html><body>edric <a action="bypass -h Script {script} enter">x</a></body></html>"#
                ),
            ),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let edric = t.spawn(EDRIC, 20);
        for (at, level) in levels.iter().enumerate() {
            let id = i32::try_from(at).unwrap() + 1;
            t.enter_with(player_data(id, *level, vec![]));
            if id > 1 {
                t.input(
                    1,
                    ClientInput::JoinParty {
                        name: format!("P{id}"),
                        distribution_type_id: 0,
                    },
                );
                t.input(id, ClientInput::AnswerJoinParty { response: 1 });
            }
        }
        if levels.len() > 1 {
            assert_eq!(t.world.party_members_of(ObjectId(1)).len(), levels.len());
        }
        // The members that took the quest (a state of it) are the ones the entrance puts in.
        for at in 0..levels.len() {
            let id = ObjectId(i32::try_from(at).unwrap() + 1);
            t.world.run_as_script(DEFEATING_DRAGONKIN_REMNANTS, |ctx| {
                ctx.get_quest_state(id, true);
            });
        }
        t.talk(1, edric);
        t.take();
        (t, edric)
    }

    /// The whole run of one instance: the refusals, the entrance, the doorman and the machine.
    pub fn run(registry: impl Fn() -> ScriptRegistry, script: &str, spec: &Spec) {
        // Not in a party: refused.
        let (mut t, _) = world(registry(), script, &[80]);
        t.bypass(1, &format!("Script {script} enter"));
        let out = t.take();
        assert!(pages_to(&out, 1).is_empty());
        assert!(names_to(&out, 1).contains(&"SYSTEM_MESSAGE".to_owned()));

        // A member below level 75: refused, both are told.
        let (mut t, _) = world(registry(), script, &[80, 74]);
        t.bypass(1, &format!("Script {script} enter"));
        assert!(names_to(&t.take(), 2).contains(&"SYSTEM_MESSAGE".to_owned()));
        assert!(t.world.player_instance_world(ObjectId(1)).is_none());

        // A party of two: in.
        let (mut t, edric) = world(registry(), script, &[80, 80]);
        let before = t.world.npc_count();
        t.bypass(1, &format!("Script {script} enter"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("entrance"));
        let world = t.world.player_instance_world(ObjectId(1)).unwrap();
        let instance_id = world.instance_id;
        assert!(world.is_allowed(ObjectId(2)));
        // The doorman, the machine, the teleporter and the monsters of both halls.
        let halls: i32 = spec
            .hall_1_spawns
            .iter()
            .chain(spec.hall_2_spawns)
            .map(|e| e.1)
            .sum();
        assert_eq!(i32::try_from(t.world.npc_count() - before).unwrap(), 3 + halls);
        // The entrance time is set for both, so a new try is locked.
        assert!(t.world.instance_time(ObjectId(1), spec.template_id) > 0);
        assert!(t.world.instance_time(ObjectId(2), spec.template_id) > 0);
        // (The party stands in the instance now, away from Edric: the event is fired directly.)
        t.take();
        t.world.run_as_script(script, |ctx| {
            let edric = ctx.script_npc(edric).unwrap();
            assert_eq!(on_event(spec, ctx, "enter", Some(edric), Some(ObjectId(1))), None);
        });
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("lock"));

        // The doorman dies: its doors open after 2 seconds.
        let door_man: i32 = t.world.instance_world(instance_id).unwrap().parameters["doorMan"]
            .parse()
            .unwrap();
        t.world.run_as_script(script, |ctx| {
            let npc = ctx.script_npc(ObjectId(door_man)).unwrap();
            on_kill(spec, ctx, npc, ObjectId(1));
            assert!(ctx.has_quest_timer("DOORMAN_KILLED", Some(npc), None));
        });
        t.ticks(45);
        let machine: i32 = t.world.instance_world(instance_id).unwrap().parameters["remnantMachine"]
            .parse()
            .unwrap();
        // The machine dies: every monster of the instance goes and the instance is given its end.
        t.world.run_as_script(script, |ctx| {
            let npc = ctx.script_npc(ObjectId(machine)).unwrap();
            on_kill(spec, ctx, npc, ObjectId(1));
        });
        assert!(t.world.script_alive_monsters_of_instance(instance_id).is_empty());
    }
}
