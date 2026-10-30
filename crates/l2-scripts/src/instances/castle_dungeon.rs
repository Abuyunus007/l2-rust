//! Castle Dungeon instance zone: the prison manager of a castle lets its clan's party fight three
//! raid bosses in a row.
// Java: dist/game/data/scripts/instances/CastleDungeon/CastleDungeon.java

use l2_util::ObjectId;
use l2_world::residence::FortId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Locations
const ENTER_LOC: [(i32, i32, i32); 3] = [
    (12188, -48770, -3008),
    (12218, -48770, -3008),
    (12248, -48770, -3008),
];

const RAIDS_LOC: (i32, i32, i32) = (11793, -49190, -3008);

// Misc
/// `CASTLE_DUNGEON`: the prison manager and the template of its castle.
const CASTLE_DUNGEON: [(i32, i32); 9] = [
    (36403, 13), // Gludio
    (36404, 14), // Dion
    (36405, 15), // Giran
    (36406, 16), // Oren
    (36407, 17), // Aden
    (36408, 18), // Innadril
    (36409, 19), // Goddard
    (36410, 20), // Rune
    (36411, 21), // Schuttgart
];

/// `FORTRESS`: the fortresses by castle id.
fn fortress(castle_id: i32) -> &'static [i32] {
    match castle_id {
        1 => &[101, 102, 112, 113],      // Gludio Castle
        2 => &[103, 112, 114, 115],      // Dion Castle
        3 => &[104, 114, 116, 118, 119], // Giran Castle
        4 => &[105, 113, 115, 116, 117], // Oren Castle
        5 => &[106, 107, 117, 118],      // Aden Castle
        6 => &[108, 119],                // Innadril Castle
        7 => &[109, 117, 120],           // Goddard Castle
        8 => &[110, 120, 121],           // Rune Castle
        9 => &[111, 121],                // Schuttgart Castle
        _ => &[],
    }
}

// Raid Bosses
const RAIDS1: [i32; 3] = [
    25546, // Rhianna the Traitor
    25549, // Tesla the Deceiver
    25552, // Soul Hunter Chakundel
];
const RAIDS2: [i32; 4] = [
    25553, // Durango the Crusher
    25554, // Brutus the Obstinate
    25557, // Ranger Karankawa
    25560, // Sargon the Mad
];
const RAIDS3: [i32; 3] = [
    25563, // Beautiful Atrielle
    25566, // Nagen the Tomboy
    25569, // Jax the Destroyer
];

/// `CASTLE_DUNGEON.get(npcId)`.
fn template_of(npc_id: i32) -> Option<i32> {
    CASTLE_DUNGEON.iter().find(|(n, _)| *n == npc_id).map(|(_, t)| *t)
}

/// Java `instances.CastleDungeon.CastleDungeon`.
pub struct CastleDungeon;

impl Script for CastleDungeon {
    fn name(&self) -> &'static str {
        "CastleDungeon"
    }

    fn path(&self) -> &'static str {
        "instances/CastleDungeon"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let managers: Vec<i32> = CASTLE_DUNGEON.iter().map(|(n, _)| *n).collect();
        s.add_first_talk_id(&managers);
        s.add_start_npc(&managers);
        s.add_talk_id(&managers);
        s.add_kill_id(&RAIDS1);
        s.add_kill_id(&RAIDS2);
        s.add_kill_id(&RAIDS3);
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("36403.html".to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let Some(instance_id) = ctx
            .world()
            .instance_world_of(npc.object_id)
            .map(|w| w.instance_id)
        else {
            return;
        };
        if RAIDS3.contains(&npc.id) {
            ctx.world_mut().finish_instance(instance_id);
        } else {
            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.inc_status();
            }
            spawn_raid(ctx, instance_id);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.world().party_of(player).is_none() {
            return Some("36403-01.html".to_owned());
        }

        // NOTE(java-quirk): Java would throw on an NPC without a castle (and `showError`);
        // here nothing happens.
        let castle = ctx.world().npc_castle(npc.object_id)?;
        if ctx.world().is_castle_siege_in_progress(castle) {
            return Some("36403-04.html".to_owned());
        }

        let clan_castle = ctx.world().clan_of(player).map(|c| c.castle_id);
        if ctx.world().npc_is_my_lord(npc.object_id, player)
            || clan_castle.is_some_and(|id| id == castle.0 && id > 0)
        {
            let num_fort = if castle.0 == 1 || castle.0 == 5 { 2 } else { 1 };
            let fort = fortress(castle.0);
            for id in fort.iter().take(num_fort) {
                // `fortress.getFortState() == 0`
                if ctx.world().fort(FortId(*id)).is_some_and(|f| f.state == 0) {
                    return Some("36403-05.html".to_owned());
                }
            }
        }

        let template_id = template_of(npc.id)?;
        let now = ctx.world().instance_clock_ms();
        for party_member in ctx.world().party_members_of(player) {
            let member_castle = ctx.world().clan_of(party_member).map(|c| c.castle_id);
            if member_castle != Some(castle.0) {
                return Some("36403-02.html".to_owned());
            }

            if now < ctx.world().instance_time(party_member, template_id) {
                return Some("36403-03.html".to_owned());
            }
        }

        enter_instance(ctx, player, template_id);
        // `super.onTalk(npc, player)`.
        None
    }
}

/// Java `enterInstance(player, templateId)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId, template_id: i32) {
    let mut entered: Option<(i32, bool)> = None;
    ctx.world_mut().enter_instance(
        player,
        template_id,
        |_, _| true,
        |_, _, instance_id, first| entered = Some((instance_id, first)),
    );
    // The templates have no enter location and no buff removal, so `enterInstance` has nothing
    // to do between the world's making and `onEnterInstance`; the teleport goes on its own here.
    let Some((instance_id, first_entrance)) = entered else {
        return;
    };

    if first_entrance {
        // `party == null`: just the player.
        for member in ctx.world().party_members_of(player) {
            if let Some((x, y, z)) = ctx.get_random_entry(&ENTER_LOC) {
                ctx.teleport_player(member, x, y, z, instance_id, true);
            }
            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.add_allowed(member);
            }
        }

        spawn_raid(ctx, instance_id);
    } else if let Some((x, y, z)) = ctx.get_random_entry(&ENTER_LOC) {
        ctx.teleport_player(player, x, y, z, instance_id, true);
    }
}

/// Java `spawnRaid(world)`.
fn spawn_raid(ctx: &mut ScriptCtx<'_>, instance_id: i32) {
    let status = ctx
        .world()
        .instance_world(instance_id)
        .map_or(0, |w| w.status());
    let spawn_id = if status == 0 {
        ctx.get_random_entry(&RAIDS1)
    } else if status == 1 {
        ctx.get_random_entry(&RAIDS2)
    } else {
        ctx.get_random_entry(&RAIDS3)
    };
    if let Some(spawn_id) = spawn_id {
        ctx.add_spawn(spawn_id, RAIDS_LOC.0, RAIDS_LOC.1, RAIDS_LOC.2, 0, false, 0, instance_id);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="36403" type="Folk" level="70" name="Prison Manager"/>
        <npc id="25546" type="RaidBoss" level="70" name="Rhianna the Traitor"/>
        <npc id="25549" type="RaidBoss" level="70" name="Tesla the Deceiver"/>
        <npc id="25552" type="RaidBoss" level="70" name="Soul Hunter Chakundel"/>
        <npc id="25553" type="RaidBoss" level="70" name="Durango the Crusher"/>
        <npc id="25554" type="RaidBoss" level="70" name="Brutus the Obstinate"/>
        <npc id="25557" type="RaidBoss" level="70" name="Ranger Karankawa"/>
        <npc id="25560" type="RaidBoss" level="70" name="Sargon the Mad"/>
        <npc id="25563" type="RaidBoss" level="70" name="Beautiful Atrielle"/>
        <npc id="25566" type="RaidBoss" level="70" name="Nagen the Tomboy"/>
        <npc id="25569" type="RaidBoss" level="70" name="Jax the Destroyer"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(CastleDungeon);
        let pages = Pages::of(&CastleDungeon)
            .file(
                "data/scripts/instances/CastleDungeon/36403.html",
                r#"<html><body>36403.html<a action="bypass -h Script CastleDungeon">enter</a></body></html>"#,
            )
            .plain(&["36403-01.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.world.set_instance_data(instance_data(
            &[(13, "Castle Dungeon (Gludio)")],
            &[(13, r#"<exitPoint x="12000" y="-48000" z="-3000" />"#)],
        ));
        let manager = t.spawn(36403, 20);
        (t, manager)
    }

    fn raids(t: &ScriptTest, ids: &[i32]) -> Vec<Oid> {
        ids.iter().flat_map(|id| t.npcs_of(*id)).collect()
    }

    #[test]
    fn the_first_talk_shows_the_prison_page_and_a_player_without_party_is_refused() {
        let (mut t, manager) = setup();
        t.enter(1, 70, vec![]);
        t.talk(1, manager);
        assert!(
            crate::testing::pages_to(&t.take(), 1)
                .pop()
                .unwrap()
                .contains("36403.html")
        );
        t.bypass(1, "Script CastleDungeon");
        assert!(
            crate::testing::pages_to(&t.take(), 1)
                .pop()
                .unwrap()
                .contains("36403-01.html")
        );
    }

    #[test]
    fn enter_spawns_a_raid_and_each_kill_brings_the_next_until_the_instance_ends() {
        let (mut t, _) = setup();
        t.enter(1, 70, vec![]);
        t.world.run_as_script("CastleDungeon", |ctx| enter_instance(ctx, Oid(1), 13));
        t.ticks(60);

        // Enter: allowed, teleported to one of the three points, the first raid spawned.
        let instance_id = t.world.instance_id_of(Oid(1));
        assert!(instance_id > 0);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone(), world.status()), (13, vec![Oid(1)], 0));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        let offset = t.world.max_offset_on_teleport();
        assert!(ENTER_LOC.iter().any(|(ex, ey, _)| (x - ex).abs() <= offset && (y - ey).abs() <= offset));
        let first = raids(&t, &RAIDS1);
        assert_eq!(first.len(), 1);
        assert_eq!(t.world.instance_id_of(first[0]), instance_id);

        // Kill 1 -> status 1 and a raid of the second group; kill 2 -> the third group.
        t.kill(1, first[0]);
        assert_eq!(t.world.instance_world(instance_id).unwrap().status(), 1);
        let second = raids(&t, &RAIDS2);
        assert_eq!(second.len(), 1);
        t.kill(1, second[0]);
        assert_eq!(t.world.instance_world(instance_id).unwrap().status(), 2);
        let third = raids(&t, &RAIDS3);
        assert_eq!(third.len(), 1);

        // Kill 3 -> `finishInstance(world)`: no grace for an empty instance, the end time is the
        // default finish time away.
        let before = t.world.instance(instance_id).unwrap().empty_destroy_time_ms;
        assert_ne!(before, 0);
        t.kill(1, third[0]);
        let instance = t.world.instance(instance_id).unwrap();
        assert_eq!(instance.empty_destroy_time_ms, 0);
        assert!(instance.end_time_ms > t.world.instance_clock_ms());
    }

    #[test]
    fn a_second_enter_goes_back_into_the_same_instance() {
        let (mut t, _) = setup();
        t.enter(1, 70, vec![]);
        t.world.run_as_script("CastleDungeon", |ctx| enter_instance(ctx, Oid(1), 13));
        t.ticks(60);
        let instance_id = t.world.instance_id_of(Oid(1));
        t.world.run_as_script("CastleDungeon", |ctx| enter_instance(ctx, Oid(1), 13));
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(1)), instance_id);
        assert_eq!(raids(&t, &RAIDS1).len(), 1);
    }
}
