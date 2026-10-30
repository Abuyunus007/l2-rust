//! Library of Sages (instance 156), the Seven Signs quests 10292 and 10293.
// Java: dist/game/data/scripts/instances/SSQLibraryOfSages/SSQLibraryOfSages.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

// NPCs
const SOPHIA1: i32 = 32596;
const PILE_OF_BOOKS1: i32 = 32809;
const PILE_OF_BOOKS2: i32 = 32810;
const PILE_OF_BOOKS3: i32 = 32811;
const PILE_OF_BOOKS4: i32 = 32812;
const PILE_OF_BOOKS5: i32 = 32813;
const SOPHIA2: i32 = 32861;
const SOPHIA3: i32 = 32863;
const ELCADIA_INSTANCE: i32 = 32785;

// Locations
const START_LOC: (i32, i32, i32) = (37063, -49813, -1128);
const EXIT_LOC: (i32, i32, i32) = (37063, -49813, -1128);
const LIBRARY_LOC: (i32, i32, i32) = (37355, -50065, -1127);

// NpcString
const ELCADIA_DIALOGS: [NpcStringId; 4] = [
    NpcStringId::IMustAskLibrarianSophiaAboutTheBook,
    NpcStringId::ThisLibraryItSHugeButThereArenTManyUsefulBooksRight,
    NpcStringId::AnUndergroundLibraryIHateDampAndSmellyPlaces,
    NpcStringId::TheBookThatWeSeekIsCertainlyHereSearchInchByInch,
];

// Misc
const TEMPLATE_ID: i32 = 156;

/// Java `instances.SSQLibraryOfSages.SSQLibraryOfSages`.
pub struct SSQLibraryOfSages;

impl Script for SSQLibraryOfSages {
    fn name(&self) -> &'static str {
        "SSQLibraryOfSages"
    }

    fn path(&self) -> &'static str {
        "instances/SSQLibraryOfSages"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[
            SOPHIA2,
            ELCADIA_INSTANCE,
            PILE_OF_BOOKS1,
            PILE_OF_BOOKS2,
            PILE_OF_BOOKS3,
            PILE_OF_BOOKS4,
            PILE_OF_BOOKS5,
        ]);
        s.add_start_npc(&[SOPHIA1, SOPHIA2, SOPHIA3]);
        s.add_talk_id(&[SOPHIA1, SOPHIA2, SOPHIA3]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // `getPlayerWorld(player)` of a null player is a NullPointerException.
        let player = player?;
        let instance_id = ctx.world().player_instance_world(player).map(|w| w.instance_id);
        if let Some(instance_id) = instance_id {
            match event {
                "TELEPORT2" => {
                    ctx.teleport_player(
                        player,
                        LIBRARY_LOC.0,
                        LIBRARY_LOC.1,
                        LIBRARY_LOC.2,
                        instance_id,
                        true,
                    );
                    // A world without Elcadia is a NullPointerException.
                    let elcadia = elcadia(ctx, instance_id)?;
                    tele_npc(ctx, elcadia, LIBRARY_LOC, instance_id);
                }
                "exit" => {
                    ctx.cancel_quest_timer("FOLLOW", npc, Some(player));
                    // `player.teleToLocation(EXIT_LOC)`: the Location carries instance 0.
                    ctx.world_mut().tele_to_location(
                        player,
                        TeleportTarget::at(EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2).with_instance(0),
                    );
                    let elcadia = elcadia(ctx, instance_id)?;
                    ctx.delete_npc(elcadia);
                }
                "FOLLOW" => {
                    // A null NPC (the first entrance, see `spawn_elcadia`) is a NullPointerException.
                    let npc = npc?;
                    // NOTE(java-quirk): `exit`/`ENTER` cancel this timer with the NPC that was
                    // clicked, not Elcadia, so it never stops in Java and goes on with a deleted
                    // Elcadia. Here it ends when Elcadia is gone: a deleted NPC does nothing visible.
                    if !ctx.world().npc(npc.object_id).is_some_and(|n| !n.decayed) {
                        return None;
                    }
                    ctx.set_running(npc);
                    ctx.world_mut().script_start_follow(npc.object_id, player);
                    if let Some(text) = ctx.get_random_entry(&ELCADIA_DIALOGS) {
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
                    }
                    ctx.start_quest_timer("FOLLOW", 10000, Some(npc), Some(player), false);
                }
                "ENTER" => {
                    ctx.cancel_quest_timer("FOLLOW", npc, Some(player));
                    ctx.teleport_player(player, START_LOC.0, START_LOC.1, START_LOC.2, instance_id, true);
                    let elcadia = elcadia(ctx, instance_id)?;
                    tele_npc(ctx, elcadia, START_LOC, instance_id);
                }
                _ => {}
            }
        }
        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        enter_instance(ctx, talker);
        // `super.onTalk(npc, talker)`.
        None
    }
}

/// `world.getParameters().getObject("elcadia", Npc.class)`.
fn elcadia(ctx: &ScriptCtx<'_>, instance_id: i32) -> Option<ScriptNpc> {
    let id = ctx
        .world()
        .instance_world(instance_id)?
        .parameters
        .get("elcadia")?
        .parse()
        .ok()?;
    Some(ScriptNpc {
        object_id: ObjectId(id),
        id: ELCADIA_INSTANCE,
    })
}

/// `elcadia.teleToLocation(x, y, z, 0, instanceId)`.
fn tele_npc(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, (x, y, z): (i32, i32, i32), instance_id: i32) {
    ctx.world_mut().tele_to_location(
        npc.object_id,
        TeleportTarget::at(x, y, z).with_instance(instance_id),
    );
}

/// Java `enterInstance(talker, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let mut entered = None;
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance && let Some(instance_world) = world.instance_world_mut(instance_id) {
                instance_world.add_allowed(player);
            }
            // `teleportPlayer(player, START_LOC, id, false)`.
            world.tele_to_location(
                player,
                TeleportTarget::at(START_LOC.0, START_LOC.1, START_LOC.2).with_instance(instance_id),
            );
            entered = Some(instance_id);
        },
    );
    if let Some(instance_id) = entered {
        spawn_elcadia(ctx, player, instance_id);
    }
}

/// Java `spawnElcadia(player, world)`.
fn spawn_elcadia(ctx: &mut ScriptCtx<'_>, player: ObjectId, instance_id: i32) {
    let old = elcadia(ctx, instance_id);
    if let Some(old) = old {
        ctx.delete_npc(old);
    }
    // `addSpawn(ELCADIA_INSTANCE, player, false, 0, false, player.getInstanceId())`.
    let spawned = ctx.world().position_of(player).map(|(x, y, z)| {
        (
            x,
            y,
            z,
            ctx.world().heading_of(player),
            ctx.world().instance_id_of(player),
        )
    });
    if let Some((x, y, z, heading, instance)) = spawned
        && let Some(new) = ctx.add_spawn(ELCADIA_INSTANCE, x, y, z, heading, false, 0, instance)
        && let Some(world) = ctx.world_mut().instance_world_mut(instance_id)
    {
        world
            .parameters
            .insert("elcadia".to_owned(), new.object_id.0.to_string());
    }
    // NOTE(java-quirk): the timer gets the `elcadia` read before the new spawn: null on the first
    // entrance (the FOLLOW event then ends in a NullPointerException) and the deleted one after.
    ctx.start_quest_timer("FOLLOW", 3000, old, Some(player), false);
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="32596" type="Folk" level="80" name="Sophia"/>
        <npc id="32785" type="Folk" level="80" name="Elcadia"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(SSQLibraryOfSages);
        let pages = Pages::of(&SSQLibraryOfSages)
            .plain(&["32861.html", "32785.html"])
            .npcs(&[]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.world.set_instance_data(instance_data(
            &[(156, "Library of Sages")],
            &[(156, r#"<exitPoint x="37063" y="-49813" z="-1128" />"#)],
        ));
        let sophia = t.spawn(SOPHIA1, 20);
        (t, sophia)
    }

    fn event(t: &mut ScriptTest, name: &str, npc: Oid, player: i32) {
        let npc = ScriptNpc {
            object_id: npc,
            id: t.world.npc(npc).unwrap().npc_id,
        };
        t.world.run_as_script("SSQLibraryOfSages", |ctx| {
            SSQLibraryOfSages.on_event(ctx, name, Some(npc), Some(Oid(player)));
        });
        t.ticks(60);
    }

    fn talk(t: &mut ScriptTest, npc: Oid) {
        let npc = ScriptNpc {
            object_id: npc,
            id: SOPHIA1,
        };
        t.world.run_as_script("SSQLibraryOfSages", |ctx| {
            assert_eq!(SSQLibraryOfSages.on_talk(ctx, npc, Oid(1)), None);
        });
        t.ticks(60);
    }

    fn elcadia_of(t: &ScriptTest) -> Oid {
        let ids = t.npcs_of(ELCADIA_INSTANCE);
        assert_eq!(ids.len(), 1);
        ids[0]
    }

    #[test]
    fn enter_makes_the_instance_and_a_guide_who_goes_with_the_player() {
        let (mut t, sophia) = setup();
        t.enter(1, 80, vec![]);
        talk(&mut t, sophia);
        let instance_id = t.world.instance_id_of(Oid(1));
        assert_eq!(instance_id, 300_000);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (156, vec![Oid(1)]));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (START_LOC.0, START_LOC.1));
        // Elcadia stands in the instance, at the player's place.
        let elcadia = elcadia_of(&t);
        assert_eq!(t.world.instance_id_of(elcadia), instance_id);
        assert_eq!(world.parameters.get("elcadia"), Some(&elcadia.0.to_string()));
        // The first FOLLOW timer has no NPC (Java quirk): nothing follows.
        assert_eq!(t.world.quest_timer_count(), 0);
    }

    #[test]
    fn the_scene_teleports_player_and_guide_together_and_exit_removes_the_guide() {
        let (mut t, sophia) = setup();
        t.enter(1, 80, vec![]);
        talk(&mut t, sophia);
        let elcadia = elcadia_of(&t);
        let instance_id = t.world.instance_id_of(Oid(1));

        event(&mut t, "TELEPORT2", sophia, 1);
        let offset = t.world.max_offset_on_teleport();
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert!((x - LIBRARY_LOC.0).abs() <= offset && (y - LIBRARY_LOC.1).abs() <= offset);
        let (x, y, _) = t.world.position_of(elcadia).unwrap();
        assert_eq!((x, y), (LIBRARY_LOC.0, LIBRARY_LOC.1));
        assert_eq!(t.world.instance_id_of(elcadia), instance_id);

        event(&mut t, "ENTER", sophia, 1);
        let (x, y, _) = t.world.position_of(elcadia).unwrap();
        assert_eq!((x, y), (START_LOC.0, START_LOC.1));

        event(&mut t, "exit", sophia, 1);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert!(!t.in_world(elcadia));
    }

    #[test]
    fn a_second_entrance_replaces_the_guide_and_it_follows() {
        let (mut t, sophia) = setup();
        t.enter(1, 80, vec![]);
        talk(&mut t, sophia);
        let first = elcadia_of(&t);
        talk(&mut t, sophia);
        // The enter of an existing world does not spawn again (`onEnterInstance` still does).
        let second = elcadia_of(&t);
        assert_ne!(first, second);
        assert!(!t.in_world(first));
        // The timer got the deleted first Elcadia: its FOLLOW ends at once.
        t.ticks(80);
        assert_eq!(t.world.quest_timer_count(), 0);
        // FOLLOW on the live one follows the player and re-arms.
        event(&mut t, "FOLLOW", second, 1);
        assert_eq!(t.world.quest_timer_count(), 1);
        assert!(t.world.is_intention_follow(second));
    }
}
