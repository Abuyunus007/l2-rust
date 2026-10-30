//! Nornil's Garden instance zone, template 11 (`Q00179_IntoTheLargeCavern`).
// Java: dist/game/data/scripts/instances/NornilsGarden/NornilsGarden.java

use std::collections::HashMap;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::script::{STARTED, race};
use l2_world::{InstanceWorld, Script, ScriptCtx, ScriptNpc, ScriptSetup};

const Q00179_INTO_THE_LARGE_CAVERN: &str = "Q00179_IntoTheLargeCavern";

// NPCs
const GARDEN_GUARD: i32 = 32330;
const FINAL_GATES: [i32; 3] = [32260, 32261, 32262];

// Skills
const SKILL_1: SkillKey = SkillKey { id: 4322, level: 1 };
const SKILL_2: SkillKey = SkillKey { id: 4327, level: 1 };
const SKILL_3: SkillKey = SkillKey { id: 4329, level: 1 };
const SKILL_4: SkillKey = SkillKey { id: 4324, level: 1 };

// Locations
const SPAWN_PPL: (i32, i32, i32) = (-111184, 74540, -12430);
const EXIT_PPL: (i32, i32, i32) = (-74058, 52040, -3680);

// Misc
const TEMPLATE_ID: i32 = 11;
const DURATION_TIME: i32 = 70;
const EMPTY_DESTROY_TIME: i64 = 5;
const INSTANCE_LEVEL_MIN: i32 = 17;
const INSTANCE_LEVEL_MAX: i32 = 21;
/// `(zone id, door id)`.
const AUTO_GATES: [(i32, i32); 3] = [
    (20110, 16200001), // Warriors gate
    (20111, 16200004), // Midway gate
    (20112, 16200013), // Gate
];
const HERB_JAR: i32 = 18478;
/// `(npc id, key item id, door id to open or 0)`.
const GATEKEEPERS: [(i32, i32, i32); 11] = [
    (18352, 9703, 0),        // Kamael Guard
    (18353, 9704, 0),        // Guardian of Records
    (18354, 9705, 0),        // Guardian of Observation
    (18355, 9706, 0),        // Spicula's Guard
    (18356, 9707, 16200024), // Harkilgamed's Gatekeeper
    (18357, 9708, 16200025), // Rodenpicula's Gatekeeper
    (18358, 9713, 0),        // Guardian of Secrets
    (18359, 9709, 16200023), // Arviterre's Guardian
    (18360, 9710, 0),        // Katenar's Gatekeeper
    (18361, 9711, 0),        // Guardian of Prediction
    (25528, 9712, 0),        // Tiberias
];
/// `(item id, count, chance)`.
const HP_HERBS_DROPLIST: [(i32, i64, i32); 3] = [(8602, 1, 10), (8601, 2, 40), (8600, 3, 70)];
/// `(npc id, x, y, z, heading)`.
const GROUP_1: [(i32, i32, i32, i32, i32); 4] = [
    (18363, -109899, 74431, -12528, 16488),
    (18483, -109701, 74501, -12528, 24576),
    (18483, -109892, 74886, -12528, 0),
    (18363, -109703, 74879, -12528, 49336),
];
const GROUP_2: [(i32, i32, i32, i32, i32); 8] = [
    (18363, -110393, 78276, -12848, 49152),
    (18363, -110561, 78276, -12848, 49152),
    (18362, -110414, 78495, -12905, 48112),
    (18362, -110545, 78489, -12903, 48939),
    (18483, -110474, 78601, -12915, 49488),
    (18362, -110474, 78884, -12915, 49338),
    (18483, -110389, 79131, -12915, 48539),
    (18483, -110551, 79134, -12915, 49151),
];
const GROUP_3: [(i32, i32, i32, i32, i32); 7] = [
    (18483, -107798, 80721, -12912, 0),
    (18483, -107798, 80546, -12912, 0),
    (18347, -108033, 80644, -12912, 0),
    (18363, -108520, 80647, -12912, 0),
    (18483, -108740, 80752, -12912, 0),
    (18363, -109016, 80642, -12912, 0),
    (18483, -108740, 80546, -12912, 0),
];
const GROUP_4: [(i32, i32, i32, i32, i32); 9] = [
    (18362, -110082, 83998, -12928, 0),
    (18362, -110082, 84210, -12928, 0),
    (18363, -109963, 84102, -12896, 0),
    (18347, -109322, 84102, -12880, 0),
    (18362, -109131, 84097, -12880, 0),
    (18483, -108932, 84101, -12880, 0),
    (18483, -109313, 84488, -12880, 0),
    (18362, -109122, 84490, -12880, 0),
    (18347, -108939, 84489, -12880, 0),
];
const MP_HERBS_DROPLIST: [(i32, i64, i32); 3] = [(8605, 1, 10), (8604, 2, 40), (8603, 3, 70)];

/// Java `NornilsGarden.NornilsWorld`: what the world of an instance of this script carries. The
/// script keeps it by instance id; a world of another script has no entry (Java's
/// `instanceof NornilsWorld`).
#[derive(Default)]
struct NornilsWorld {
    first_npc: Option<ObjectId>,
    spawned_1: bool,
    spawned_2: bool,
    spawned_3: bool,
    spawned_4: bool,
}

#[derive(Default)]
struct State {
    worlds: HashMap<i32, NornilsWorld>,
}

/// Java `instances.NornilsGarden.NornilsGarden`.
pub struct NornilsGarden;

impl Script for NornilsGarden {
    fn name(&self) -> &'static str {
        "NornilsGarden"
    }

    fn path(&self) -> &'static str {
        "instances/NornilsGarden"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GARDEN_GUARD]);
        s.add_first_talk_id(&[GARDEN_GUARD]);
        s.add_talk_id(&[GARDEN_GUARD]);
        for (npc, _, _) in GATEKEEPERS {
            s.add_kill_id(&[npc]);
        }

        for (zone, _) in AUTO_GATES {
            s.add_enter_zone_id(&[zone]);
        }

        s.add_talk_id(&FINAL_GATES);
        s.add_attack_id(&[HERB_JAR]);
        s.add_attack_id(&[18362]); // first garden guard
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {
        let live_player = ctx.world().player(creature).is_some()
            && !ctx.world().is_dead(creature)
            && !ctx.world().is_teleporting(creature);
        if live_player {
            let instance_id = nornils_world_of(ctx, creature);
            if let Some(instance_id) = instance_id {
                for (zone, door) in AUTO_GATES {
                    if zone_id == zone {
                        ctx.world_mut().instance_open_door(instance_id, door);
                    }

                    if zone_id == 20111 {
                        spawn3(ctx, instance_id);
                    } else if zone_id == 20112 {
                        spawn4(ctx, instance_id);
                    }
                }
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
        let npc = npc?;
        let mut htmltext = Some(event.to_owned());
        let Some(st) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        if npc.id == GARDEN_GUARD && event.eq_ignore_ascii_case("enter_instance") {
            htmltext = enter_instance(ctx, npc, player);
        } else if npc.id == 32258 && event.eq_ignore_ascii_case("exit") {
            if ctx.world().is_in_instance(player) {
                // `super.teleportPlayer(player, EXIT_PPL, 0)`: no buffs.
                ctx.teleport_player(player, EXIT_PPL.0, EXIT_PPL.1, EXIT_PPL.2, 0, true);
            }
        } else if FINAL_GATES.contains(&npc.id) {
            if event.eq_ignore_ascii_case("32260-02.html")
                || event.eq_ignore_ascii_case("32261-02.html")
                || event.eq_ignore_ascii_case("32262-02.html")
            {
                st.unset(ctx, "correct");
            } else if !event.is_empty() && event.chars().all(|c| c.is_ascii_digit()) {
                let correct = st.get_int(ctx, "correct") + 1;
                st.set(ctx, "correct", &correct.to_string());
                htmltext = Some(format!("{}-0{}.html", npc.id, correct + 2));
            } else if event.eq_ignore_ascii_case("check") {
                let correct = st.get_int(ctx, "correct");
                if npc.id == 32260 && correct == 3 {
                    open_door(ctx, st, player, 16200014);
                } else if npc.id == 32261 && correct == 3 {
                    open_door(ctx, st, player, 16200015);
                } else if npc.id == 32262 && correct == 4 {
                    open_door(ctx, st, player, 16200016);
                } else {
                    return Some(format!("{}-00.html", npc.id));
                }
                // NOTE(java-quirk): a right answer leaves `htmltext` at the event, so the player
                // gets the text "check" as a chat message.
            }
        }

        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if FINAL_GATES.contains(&npc.id) {
            let started = ctx
                .player_quest_state(player, Q00179_INTO_THE_LARGE_CAVERN)
                .is_some_and(|q| q.state() == STARTED);
            if started {
                return Some(format!("{}-01.html", npc.id));
            }

            return Some(ctx.get_no_quest_msg(player));
        }

        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        ctx.get_quest_state(player, true);
        Some(format!("{}.html", npc.id))
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if npc.id == HERB_JAR && !ctx.world().is_dead(npc.object_id) {
            drop_herb(ctx, npc, attacker, &HP_HERBS_DROPLIST);
            drop_herb(ctx, npc, attacker, &MP_HERBS_DROPLIST);
            ctx.do_die(npc, attacker);
        } else if npc.id == 18362 && ctx.world().is_in_instance(npc.object_id) {
            spawn1(ctx, npc);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(player, false).is_none() {
            return;
        }

        for (gatekeeper, key, door) in GATEKEEPERS {
            if npc.id == gatekeeper {
                // Drop key
                ctx.world_mut().npc_drop_item(npc.object_id, Some(player), key, 1);

                // Check if gatekeeper should open bridge, and open it
                if door > 0
                    && let Some(instance_id) = nornils_world_of(ctx, player)
                {
                    ctx.world_mut().instance_open_door(instance_id, door);
                }
            }

            if npc.id == 18355 {
                spawn2(ctx, npc);
            }
        }
    }
}

/// `InstanceManager.getInstance().getWorld(creature) instanceof NornilsWorld`: its instance id.
fn nornils_world_of(ctx: &mut ScriptCtx<'_>, creature: ObjectId) -> Option<i32> {
    let instance_id = ctx.world().instance_world_of(creature).map(|w| w.instance_id)?;
    ctx.state::<State>()
        .worlds
        .contains_key(&instance_id)
        .then_some(instance_id)
}

/// Java `dropHerb(mob, player, drop)`.
fn drop_herb(ctx: &mut ScriptCtx<'_>, mob: ScriptNpc, player: ObjectId, drop: &[(i32, i64, i32)]) {
    let chance = ctx.get_random(100);
    for (item_id, count, element_chance) in drop {
        if chance < *element_chance {
            ctx.world_mut()
                .npc_drop_item(mob.object_id, Some(player), *item_id, *count);
        }
    }
}

/// Java `giveBuffs(ch)`.
fn give_buffs(ctx: &mut ScriptCtx<'_>, ch: ObjectId) {
    for skill in [SKILL_1, SKILL_2, SKILL_3, SKILL_4] {
        ctx.world_mut().apply_effects(ch, ch, skill);
    }
}

/// This script's `teleportPlayer(player, loc, instanceId)`: the buffs first.
fn teleport_player(ctx: &mut ScriptCtx<'_>, player: ObjectId, loc: (i32, i32, i32), instance_id: i32) {
    give_buffs(ctx, player);
    if let Some(summon) = ctx.world().summon_of(player) {
        give_buffs(ctx, summon);
    }

    ctx.teleport_player(player, loc.0, loc.1, loc.2, instance_id, true);
}

/// `addSpawn(mob[0], mob[1], mob[2], mob[3], mob[4], false, 0, false, instanceId)` for a group.
fn spawn_group(ctx: &mut ScriptCtx<'_>, instance_id: i32, group: &[(i32, i32, i32, i32, i32)]) {
    for (id, x, y, z, heading) in group {
        ctx.add_spawn(*id, *x, *y, *z, *heading, false, 0, instance_id);
    }
}

fn spawn1(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let Some(instance_id) = nornils_world_of(ctx, npc.object_id) else {
        return;
    };
    let world = ctx.state::<State>().worlds.entry(instance_id).or_default();
    if world.first_npc == Some(npc.object_id) && !world.spawned_1 {
        world.spawned_1 = true;
        spawn_group(ctx, instance_id, &GROUP_1);
    }
}

fn spawn2(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let Some(instance_id) = nornils_world_of(ctx, npc.object_id) else {
        return;
    };
    let world = ctx.state::<State>().worlds.entry(instance_id).or_default();
    if !world.spawned_2 {
        world.spawned_2 = true;
        spawn_group(ctx, instance_id, &GROUP_2);
    }
}

fn spawn3(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let world = ctx.state::<State>().worlds.entry(instance_id).or_default();
    if !world.spawned_3 {
        world.spawned_3 = true;
        spawn_group(ctx, instance_id, &GROUP_3);
    }
}

fn spawn4(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let world = ctx.state::<State>().worlds.entry(instance_id).or_default();
    if !world.spawned_4 {
        world.spawned_4 = true;
        spawn_group(ctx, instance_id, &GROUP_4);
    }
}

/// Java `openDoor(st, player, doorId)`.
fn open_door(ctx: &mut ScriptCtx<'_>, st: l2_world::script::Qs, player: ObjectId, door_id: i32) {
    st.unset(ctx, "correct");
    if let Some(instance_id) = nornils_world_of(ctx, player) {
        ctx.world_mut().instance_open_door(instance_id, door_id);
    }
}

/// `SystemMessage(C1_S_LEVEL_DOES_NOT_CORRESPOND_TO_THE_REQUIREMENTS_FOR_ENTRY)` with a name,
/// to `player`.
fn send_with_name(ctx: &ScriptCtx<'_>, to: ObjectId, id: SystemMessageId, about: ObjectId) {
    let sm = SystemMessage::new(id).with(SystemMessageParam::PlayerName(ctx.player_name(about)));
    ctx.world().send(to, &sm);
}

/// Java `checkConditions(npc, player)`: `"ok"` or the page to show.
fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> &'static str {
    let Some(party) = ctx.world().party_of(player) else {
        // player must be in party
        ctx.send_system_message(
            player,
            SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter,
        );
        return "32330-05.html";
    };

    // ...and be party leader
    if ctx.world().party_leader(party) != Some(player) {
        ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
        return "32330-08.html";
    }

    let mut kamael = false;

    // for each party member
    for party_member in ctx.world().party_members(party) {
        // player level must be in range
        if ctx.player_level(party_member) > INSTANCE_LEVEL_MAX {
            send_with_name(
                ctx,
                player,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                party_member,
            );
            return "32330-06.html";
        }

        if ctx.player_level(party_member) < INSTANCE_LEVEL_MIN {
            send_with_name(
                ctx,
                player,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                party_member,
            );
            return "32330-07.html";
        }

        if ctx.player_class_level(party_member) != 0 {
            send_with_name(
                ctx,
                player,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                party_member,
            );
            return "32330-06.html";
        }

        // player must be near party leader
        if !ctx.distance_3d(party_member, player).is_some_and(|d| d <= 500.0) {
            send_with_name(
                ctx,
                player,
                SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                party_member,
            );
            return "32330-08.html";
        }

        if ctx.player_race(party_member) == race::KAMAEL {
            let started = ctx
                .player_quest_state(party_member, Q00179_INTO_THE_LARGE_CAVERN)
                .is_some_and(|q| q.state() == STARTED);
            if started {
                kamael = true;
            } else {
                send_with_name(
                    ctx,
                    player,
                    SystemMessageId::C1SQuestRequirementIsNotSufficientAndCannotBeEntered,
                    party_member,
                );
                return "32330-08.html";
            }
        }
    }

    if !kamael {
        return "32330-08.html";
    }

    "ok"
}

/// Java `enterInstance(npc, player)` (this script's own, not `InstanceScript`'s).
fn enter_instance(ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
    let world = ctx
        .world()
        .player_instance_world(player)
        .map(|w| (w.instance_id, w.template_id));
    if let Some((instance_id, template_id)) = world {
        if !ctx.state::<State>().worlds.contains_key(&instance_id) || template_id != TEMPLATE_ID {
            ctx.send_system_message(
                player,
                SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
            );
            return None;
        }

        // check for level difference again on reenter
        let level = ctx.player_level(player);
        if !(INSTANCE_LEVEL_MIN..=INSTANCE_LEVEL_MAX).contains(&level) {
            send_with_name(
                ctx,
                player,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                player,
            );
            return None;
        }

        // check what instance still exist
        if ctx.world().instance(instance_id).is_some() {
            teleport_player(ctx, player, SPAWN_PPL, instance_id);
        }

        return None;
    }

    // Creating new instance
    let result = check_conditions(ctx, player);
    if !result.eq_ignore_ascii_case("ok") {
        return Some(result.to_owned());
    }

    let instance_id = ctx.world_mut().create_dynamic_instance(TEMPLATE_ID);
    let template = ctx.world().instance(instance_id).map_or(-1, |i| i.template_id);
    ctx.world_mut()
        .add_instance_world(InstanceWorld::new(instance_id, template));
    ctx.state::<State>()
        .worlds
        .insert(instance_id, NornilsWorld::default());
    // `instance.setExitLoc(new Location(player))`.
    let exit = ctx.world().position_of(player);
    if let Some(instance) = ctx.world_mut().instance_mut(instance_id) {
        instance.exit_location = exit;
        instance.allow_summon = false;
    }
    ctx.world_mut()
        .set_instance_duration(instance_id, DURATION_TIME * 60000);
    ctx.world_mut()
        .set_instance_empty_destroy_time(instance_id, EMPTY_DESTROY_TIME * 60000);
    tracing::info!(
        "Nornils Garden: started, Instance: {instance_id} created by player: {}",
        ctx.player_name(player)
    );
    let first_npc = ctx.add_spawn(18362, -109702, 74696, -12528, 49568, false, 0, instance_id);
    if let Some(world) = ctx.state::<State>().worlds.get_mut(&instance_id) {
        world.first_npc = first_npc.map(|n| n.object_id);
    }
    if let Some(door) = ctx.world().instance_door(instance_id, 16200010) {
        ctx.world_mut().set_door_targetable(door, false);
        ctx.world_mut().set_door_mesh_index(door, 2);
    }

    // and finally teleport party into instance
    if ctx.world().party_of(player).is_some() {
        for party_member in ctx.world().party_members_of(player) {
            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.add_allowed(party_member);
            }
            teleport_player(ctx, party_member, SPAWN_PPL, instance_id);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32330" type="Folk" level="70" name="Garden Guard"/>
        <npc id="32260" type="Folk" level="70" name="Final Gate 1"/>
        <npc id="18478" type="Monster" level="20" name="Herb Jar"/>
        <npc id="18362" type="Monster" level="20" name="First Garden Guard"/>
        <npc id="18363" type="Monster" level="20" name="Garden Guard 2"/>
        <npc id="18483" type="Monster" level="20" name="Garden Guard 3"/>
        <npc id="18347" type="Monster" level="20" name="Garden Guard 4"/>
        <npc id="18355" type="Monster" level="20" name="Spicula's Guard"/>"#;
    const ITEMS: &str = r#"
        <item id="8602" type="EtcItem" name="HP Herb 1"/>
        <item id="8601" type="EtcItem" name="HP Herb 2"/>
        <item id="8600" type="EtcItem" name="HP Herb 3"/>
        <item id="8605" type="EtcItem" name="MP Herb 1"/>
        <item id="8604" type="EtcItem" name="MP Herb 2"/>
        <item id="8603" type="EtcItem" name="MP Herb 3"/>
        <item id="9706" type="EtcItem" name="Key"/>"#;

    /// Into the Large Cavern: a script of that name.
    struct StandIn;
    impl Script for StandIn {
        fn name(&self) -> &'static str {
            Q00179_INTO_THE_LARGE_CAVERN
        }
        fn path(&self) -> &'static str {
            "quests/Q00179_IntoTheLargeCavern"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn);
        registry.add(NornilsGarden);
        let pages = Pages::of(&NornilsGarden)
            .page("32330.html", &["enter_instance"])
            .plain(&["32330-05.html", "32330-06.html", "32330-07.html", "32330-08.html"])
            .file("data/html/noquest.htm", "<html><body>no quest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        t.world.set_instance_data(instance_data(
            &[(11, "Nornil's Garden")],
            &[(11, r#"<exitPoint x="-74058" y="52040" z="-3680" />"#)],
        ));
        let guard = t.spawn(GARDEN_GUARD, 20);
        let gate = t.spawn(32260, 40);
        (t, guard, gate)
    }

    fn page(t: &mut ScriptTest) -> String {
        crate::testing::pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    fn started(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(Q00179_INTO_THE_LARGE_CAVERN, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.start_quest(ctx);
        });
        t.take_writes();
    }

    /// A Kamael on the quest leads a human into the garden.
    fn enter_the_garden(t: &mut ScriptTest, guard: Oid) -> i32 {
        t.enter_as(1, 18, race::KAMAEL, 123, vec![]);
        t.enter(2, 18, vec![]);
        started(t, 1);
        t.party(1, 2);
        t.talk(1, guard);
        t.ticks(5);
        assert!(page(t).contains("32330.html"));
        t.event(1, "NornilsGarden", "enter_instance");
        t.ticks(60);
        t.world.instance_id_of(Oid(1))
    }

    fn count_of(t: &ScriptTest, id: i32, instance_id: i32) -> usize {
        t.npcs_of(id)
            .into_iter()
            .filter(|n| t.world.instance_id_of(*n) == instance_id)
            .count()
    }

    #[test]
    fn the_guard_shows_its_page_and_refuses_a_player_without_a_party() {
        let (mut t, guard, _) = setup();
        t.enter(1, 18, vec![]);
        t.talk(1, guard);
        t.ticks(5);
        assert!(page(&mut t).contains("32330.html"));
        assert!(
            t.event(1, "NornilsGarden", "enter_instance")
                .contains("32330-05.html")
        );
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
    }

    #[test]
    fn a_party_with_a_kamael_on_the_quest_enters_the_garden() {
        let (mut t, guard, _) = setup();
        let instance_id = enter_the_garden(&mut t, guard);
        assert!(instance_id > 0);
        assert_eq!(t.world.instance_id_of(Oid(2)), instance_id);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!(
            (world.template_id, world.allowed.clone()),
            (11, vec![Oid(1), Oid(2)])
        );
        let instance = t.world.instance(instance_id).unwrap();
        assert!(!instance.allow_summon);
        assert_eq!(instance.empty_destroy_time_ms, 5 * 60_000);
        // The first garden guard stands there.
        assert_eq!(count_of(&t, 18362, instance_id), 1);
        // Entering again from inside goes to the same instance (level in range, world exists).
        t.world.run_as_script("NornilsGarden", |ctx| {
            let page = enter_instance(
                ctx,
                ScriptNpc {
                    object_id: guard,
                    id: GARDEN_GUARD,
                },
                Oid(1),
            );
            assert_eq!(page, None);
        });
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 1);
    }

    #[test]
    fn a_party_without_a_kamael_gets_the_refusal_page() {
        let (mut t, guard, _) = setup();
        t.enter(1, 18, vec![]);
        t.enter(2, 18, vec![]);
        t.party(1, 2);
        t.talk(1, guard);
        t.ticks(5);
        t.take();
        assert!(
            t.event(1, "NornilsGarden", "enter_instance")
                .contains("32330-08.html")
        );
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
    }

    #[test]
    fn the_stages_spawn_once_each() {
        let (mut t, guard, _) = setup();
        let instance_id = enter_the_garden(&mut t, guard);
        let first = t.npcs_of(18362)[0];
        let npc = ScriptNpc {
            object_id: first,
            id: 18362,
        };
        // Stage 1: hit the first guard, four more appear; a second hit adds none.
        t.world.run_as_script("NornilsGarden", |ctx| {
            NornilsGarden.on_attack(ctx, npc, Oid(1), 5, false, None);
            NornilsGarden.on_attack(ctx, npc, Oid(1), 5, false, None);
        });
        assert_eq!(count_of(&t, 18363, instance_id), 2);
        assert_eq!(count_of(&t, 18483, instance_id), 2);

        // Stage 2: Spicula's guard dies, group 2 (eight, three of them 18362).
        let spicula = t.spawn(18355, 10);
        t.world.set_instance_id(spicula, instance_id);
        let spicula_npc = ScriptNpc {
            object_id: spicula,
            id: 18355,
        };
        t.world.run_as_script("NornilsGarden", |ctx| {
            NornilsGarden.on_kill(ctx, spicula_npc, Oid(1), false);
            NornilsGarden.on_kill(ctx, spicula_npc, Oid(1), false);
        });
        assert_eq!(count_of(&t, 18362, instance_id), 1 + 3);

        // Stage 3 and 4: the zones, once the client has loaded the target (`Appearing`).
        t.input(2, l2_world::ClientInput::Appearing);
        t.world.run_as_script("NornilsGarden", |ctx| {
            NornilsGarden.on_enter_zone(ctx, Oid(2), 20111);
            NornilsGarden.on_enter_zone(ctx, Oid(2), 20111);
            NornilsGarden.on_enter_zone(ctx, Oid(2), 20112);
        });
        assert_eq!(count_of(&t, 18347, instance_id), 1 + 2);
    }

    #[test]
    fn the_herb_jar_drops_herbs_and_dies() {
        let (mut t, _, _) = setup();
        t.enter(1, 18, vec![]);
        let jar = t.spawn(HERB_JAR, 40);
        let npc = ScriptNpc {
            object_id: jar,
            id: HERB_JAR,
        };
        t.world.run_as_script("NornilsGarden", |ctx| {
            NornilsGarden.on_attack(ctx, npc, Oid(1), 5, false, None);
        });
        assert!(t.world.is_dead(jar));
    }

    #[test]
    fn the_final_gate_counts_the_right_answers() {
        let (mut t, _, gate) = setup();
        t.enter(1, 18, vec![]);
        started(&mut t, 1);
        t.world.run_as_script("NornilsGarden", |ctx| {
            ctx.get_quest_state(Oid(1), true);
        });
        let npc = ScriptNpc {
            object_id: gate,
            id: 32260,
        };
        let mut pages = Vec::new();
        t.world.run_as_script("NornilsGarden", |ctx| {
            pages.push(NornilsGarden.on_talk(ctx, npc, Oid(1)));
            pages.push(NornilsGarden.on_event(ctx, "1", Some(npc), Some(Oid(1))));
            pages.push(NornilsGarden.on_event(ctx, "1", Some(npc), Some(Oid(1))));
            // Two answers are not enough for gate 32260 (it wants three).
            pages.push(NornilsGarden.on_event(ctx, "check", Some(npc), Some(Oid(1))));
            pages.push(NornilsGarden.on_event(ctx, "32260-02.html", Some(npc), Some(Oid(1))));
        });
        assert_eq!(
            pages,
            [
                Some("32260-01.html".to_owned()),
                Some("32260-03.html".to_owned()),
                Some("32260-04.html".to_owned()),
                Some("32260-00.html".to_owned()),
                Some("32260-02.html".to_owned()),
            ]
        );
    }
}
