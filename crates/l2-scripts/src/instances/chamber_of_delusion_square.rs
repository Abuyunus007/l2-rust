//! Chamber of Delusion Square, instance 131.
// Java: dist/game/data/scripts/instances/ChambersOfDelusion/ChamberOfDelusionSquare.java

use super::chamber::{ChamberData, chamber_script};

// NPCs
const ENTRANCE_GATEKEEPER: i32 = 32662;
const ROOM_GATEKEEPER_FIRST: i32 = 32684;
const ROOM_GATEKEEPER_LAST: i32 = 32692;
const AENKINEL: i32 = 25694;
const BOX: i32 = 18820;

// Misc
const ENTER_POINTS: [(i32, i32, i32); 9] = [
    (-122368, -153388, -6688),
    (-122368, -152524, -6688),
    (-120480, -155116, -6688),
    (-120480, -154236, -6688),
    (-121440, -151212, -6688),
    (-120464, -152908, -6688),
    (-122368, -154700, -6688),
    (-121440, -152908, -6688),
    (-121440, -154572, -6688), // Raid room
];
const INSTANCEID: i32 = 131;

const DATA: ChamberData = ChamberData {
    instance_id: INSTANCEID,
    entrance_gatekeeper: ENTRANCE_GATEKEEPER,
    room_gatekeeper_first: ROOM_GATEKEEPER_FIRST,
    room_gatekeeper_last: ROOM_GATEKEEPER_LAST,
    aenkinel: AENKINEL,
    box_id: BOX,
    enter_points: &ENTER_POINTS,
};

/// Java `instances.ChambersOfDelusion.ChamberOfDelusionSquare`.
pub struct ChamberOfDelusionSquare;

chamber_script!(ChamberOfDelusionSquare, "ChamberOfDelusionSquare", DATA);

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::instance::InstanceSettings;
    use l2_world::{Script, ScriptNpc, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::ScriptTest;

    const CLOCK: i64 = 1_793_182_520_000;
    const NPCS: &str = r#"
        <npc id="32662" type="Folk" level="70" name="Delusion Gatekeeper"/>
        <npc id="32692" type="Folk" level="70" name="Delusion Manager 9"/>
        <npc id="25694" type="Monster" level="80" name="Aenkinel"/>
        <npc id="18820" type="Monster" level="80" name="Treasure Chest"/>"#;
    const TEMPLATE: &str = r#"
        <activityTime val="60" />
        <exitPoint x="-114592" y="-152509" z="-6723" />
        <spawnlist>
          <group name="general">
            <npc id="32692" x="-121440" y="-154572" z="-6688" heading="0" />
            <npc id="25694" x="-121359" y="-154000" z="-6688" heading="0" />
          </group>
          <group name="boxes">
            <npc id="18820" x="-121359" y="-154000" z="-6688" heading="0" />
          </group>
        </spawnlist>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ChamberOfDelusionSquare);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.world.set_instance_settings(InstanceSettings {
            clock_override_unix_ms: Some(CLOCK),
            ..InstanceSettings::default()
        });
        t.world.set_instance_data(instance_data(
            &[(131, "Chamber of Delusion (Square)")],
            &[(131, TEMPLATE)],
        ));
        let gatekeeper = t.spawn(ENTRANCE_GATEKEEPER, 20);
        t.enter(1, 80, vec![]);
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        (t, gatekeeper)
    }

    fn talk(t: &mut ScriptTest, npc: Oid, player: i32) {
        t.world.run_as_script("ChamberOfDelusionSquare", |ctx| {
            ChamberOfDelusionSquare.on_talk(
                ctx,
                ScriptNpc {
                    object_id: npc,
                    id: ENTRANCE_GATEKEEPER,
                },
                Oid(player),
            );
        });
        t.ticks(60);
    }

    #[test]
    fn a_member_who_may_not_re_enter_yet_stops_the_party() {
        let (mut t, gatekeeper) = setup();
        t.world.set_instance_time(Oid(2), INSTANCEID, CLOCK + 3_600_000);
        talk(&mut t, gatekeeper, 1);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 0);
        // The time has passed.
        t.world.set_instance_time(Oid(2), INSTANCEID, CLOCK - 1);
        talk(&mut t, gatekeeper, 1);
        assert_eq!(t.world.instance_id_of(Oid(1)), 300_000);
    }

    #[test]
    fn the_last_room_is_the_raid_and_the_kill_restricts_the_entry() {
        let (mut t, gatekeeper) = setup();
        talk(&mut t, gatekeeper, 1);
        let id = t.world.instance_id_of(Oid(1));
        // 60 minutes of activity: the first room is not the raid room.
        let end = t.world.instance(id).unwrap().end_time_ms;
        assert!(end - CLOCK > 59 * 60_000);

        // Ten minutes before the end the change goes to the raid room and adds 20 minutes.
        t.wait_ms(58 * 60_000);
        let end_after = t.world.instance(id).unwrap().end_time_ms;
        assert!(end_after > end, "twenty minutes were added: {end_after} vs {end}");

        // The kill: the entry is restricted until the next 06:30 and five minutes are left.
        let aenkinel = t.world.instance_npc_by_template(id, AENKINEL).unwrap();
        t.world.run_as_script("ChamberOfDelusionSquare", |ctx| {
            ChamberOfDelusionSquare.on_kill(
                ctx,
                ScriptNpc {
                    object_id: aenkinel,
                    id: AENKINEL,
                },
                Oid(1),
                false,
            );
        });
        let now = t.world.instance_clock_ms();
        for player in [1, 2] {
            let time = t.world.instance_time(Oid(player), INSTANCEID);
            assert!(time > now && time <= now + 24 * 3_600_000, "{time} {now}");
        }
        assert!(t.world.instance(id).unwrap().end_time_ms - now <= 300_000 + 1_000);
    }
}
