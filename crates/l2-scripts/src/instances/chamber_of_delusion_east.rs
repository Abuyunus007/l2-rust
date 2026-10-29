//! Chamber of Delusion East, instance 127.
// Java: dist/game/data/scripts/instances/ChambersOfDelusion/ChamberOfDelusionEast.java

use super::chamber::{ChamberData, chamber_script};

// NPCs
const ENTRANCE_GATEKEEPER: i32 = 32658;
const ROOM_GATEKEEPER_FIRST: i32 = 32664;
const ROOM_GATEKEEPER_LAST: i32 = 32668;
const AENKINEL: i32 = 25690;
const BOX: i32 = 18838;

// Misc
const ENTER_POINTS: [(i32, i32, i32); 5] = [
    (-122368, -218972, -6720),
    (-122352, -218044, -6720),
    (-122368, -220220, -6720),
    (-121440, -218444, -6720),
    (-121424, -220124, -6720), // Raid room
];
const INSTANCEID: i32 = 127;

const DATA: ChamberData = ChamberData {
    instance_id: INSTANCEID,
    entrance_gatekeeper: ENTRANCE_GATEKEEPER,
    room_gatekeeper_first: ROOM_GATEKEEPER_FIRST,
    room_gatekeeper_last: ROOM_GATEKEEPER_LAST,
    aenkinel: AENKINEL,
    box_id: BOX,
    enter_points: &ENTER_POINTS,
};

/// Java `instances.ChambersOfDelusion.ChamberOfDelusionEast`.
pub struct ChamberOfDelusionEast;

chamber_script!(ChamberOfDelusionEast, "ChamberOfDelusionEast", DATA);

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::instance::InstanceSettings;
    use l2_world::{Script, ScriptNpc, ScriptRegistry};

    use super::*;
    use crate::instances::chamber::ChamberState;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, X, Y, Z};

    const MARK: i32 = 15311;
    const DIR: &str = "data/scripts/instances/ChambersOfDelusion";
    /// 2026-10-28 10:15:20 UTC: the instance clock at tick 0.
    const CLOCK: i64 = 1_793_182_520_000;
    const NPCS: &str = r#"
        <npc id="32658" type="Folk" level="70" name="Delusion Gatekeeper"/>
        <npc id="32664" type="Folk" level="70" name="Delusion Manager 1"/>
        <npc id="32668" type="Folk" level="70" name="Delusion Manager 5"/>
        <npc id="25690" type="Monster" level="80" name="Aenkinel"/>
        <npc id="18838" type="Monster" level="80" name="Treasure Chest"/>
        <npc id="22708" type="Monster" level="80" name="Invader"/>"#;
    const ITEMS: &str = r#"
        <item id="15311" type="EtcItem" name="Delusion Mark"><set name="is_stackable" val="true"/></item>
        <item id="4042" type="EtcItem" name="Enria"><set name="is_stackable" val="true"/></item>"#;
    const TEMPLATE: &str = r#"
        <activityTime val="40" />
        <exitPoint x="-114592" y="-152509" z="-6723" />
        <spawnlist>
          <group name="general">
            <npc id="22708" x="-122525" y="-218890" z="-6720" heading="0" respawn="25" />
            <npc id="32664" x="-122368" y="-219072" z="-6720" heading="0" />
            <npc id="32668" x="-121424" y="-220224" z="-6720" heading="0" />
            <npc id="25690" x="-121359" y="-219866" z="-6720" heading="0" respawn="65" />
          </group>
          <group name="boxes">
            <npc id="18838" x="-121359" y="-219866" z="-6720" heading="0" />
            <npc id="18838" x="-121663" y="-219885" z="-6720" heading="0" />
          </group>
        </spawnlist>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ChamberOfDelusionEast);
        let pages: Vec<(String, String)> = ["no_party.html", "no_leader.html", "no_item.html"]
            .iter()
            .map(|p| (format!("{DIR}/{p}"), format!("<html><body>{p}</body></html>")))
            .collect();
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &pages);
        t.world.set_instance_settings(InstanceSettings {
            clock_override_unix_ms: Some(CLOCK),
            ..InstanceSettings::default()
        });
        t.world.set_instance_data(instance_data(
            &[(127, "Chamber of Delusion (East)")],
            &[(127, TEMPLATE)],
        ));
        let gatekeeper = t.spawn(ENTRANCE_GATEKEEPER, 20);
        (t, gatekeeper)
    }

    /// Players 1 (leader) and 2 of level 80.
    fn party(t: &mut ScriptTest) {
        t.enter(1, 80, vec![]);
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        t.take();
    }

    fn script_npc(t: &ScriptTest, npc: Oid) -> ScriptNpc {
        ScriptNpc {
            object_id: npc,
            id: t.world.npc(npc).unwrap().npc_id,
        }
    }

    fn talk(t: &mut ScriptTest, npc: Oid, player: i32) {
        let npc = script_npc(t, npc);
        t.world.run_as_script("ChamberOfDelusionEast", |ctx| {
            assert_eq!(
                ChamberOfDelusionEast.on_talk(ctx, npc, Oid(player)),
                Some(String::new())
            );
        });
        t.ticks(60);
    }

    fn event(t: &mut ScriptTest, event: &str, npc: Oid, player: i32) -> Option<String> {
        let npc = script_npc(t, npc);
        let mut result = None;
        t.world.run_as_script("ChamberOfDelusionEast", |ctx| {
            result = ChamberOfDelusionEast.on_event(ctx, event, Some(npc), Some(Oid(player)));
        });
        t.ticks(60);
        result
    }

    fn room(t: &mut ScriptTest, instance_id: i32) -> i32 {
        let mut room = -1;
        t.world.run_as_script("ChamberOfDelusionEast", |ctx| {
            room = ctx.state::<ChamberState>().current_room(instance_id);
        });
        room
    }

    fn near(t: &ScriptTest, player: i32, point: (i32, i32, i32)) -> bool {
        let (x, y, _) = t.world.position_of(Oid(player)).unwrap();
        let offset = t.world.max_offset_on_teleport();
        (x - point.0).abs() <= offset && (y - point.1).abs() <= offset
    }

    fn npc_of(t: &ScriptTest, instance_id: i32, template: i32) -> Option<Oid> {
        t.world.instance_npc_by_template(instance_id, template)
    }

    fn count_of(t: &ScriptTest, instance_id: i32, template: i32) -> usize {
        t.world
            .instance(instance_id)
            .unwrap()
            .npcs
            .keys()
            .filter(|n| t.world.npc(**n).is_some_and(|n| n.npc_id == template))
            .count()
    }

    #[test]
    fn only_a_party_leader_of_level_80_enters() {
        let (mut t, gatekeeper) = setup();
        t.enter(1, 80, vec![]);
        // No party.
        talk(&mut t, gatekeeper, 1);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 0);

        // A member of level 79 stops the leader.
        t.enter(2, 79, vec![]);
        t.party(1, 2);
        talk(&mut t, gatekeeper, 1);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 0);

        // The member is no leader.
        talk(&mut t, gatekeeper, 2);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 0);
    }

    #[test]
    fn the_party_enters_and_the_leader_gets_the_mark() {
        let (mut t, gatekeeper) = setup();
        party(&mut t);
        talk(&mut t, gatekeeper, 1);

        let id = t.world.instance_id_of(Oid(1));
        assert_eq!(id, 300_000);
        assert_eq!(t.world.instance_id_of(Oid(2)), id);
        let world = t.world.instance_world(id).unwrap();
        assert_eq!(
            (world.template_id, world.allowed.clone()),
            (127, vec![Oid(1), Oid(2)])
        );
        // Only the leader holds the mark.
        assert_eq!(t.count(1, MARK), 1);
        assert_eq!(t.count(2, MARK), 0);
        // The return point is saved before the teleport.
        assert_eq!(
            t.world
                .player_variable_text(Oid(1), "Chamber_return")
                .map(str::to_owned),
            Some(format!("{X};{Y};{Z}"))
        );
        // Both stand in the room the first change picked (never room 0, the current one).
        let current = room(&mut t, id);
        let point = ENTER_POINTS[usize::try_from(current).unwrap()];
        assert!(near(&t, 1, point) && near(&t, 2, point));
        assert_eq!(
            t.world.quest_timer_count(),
            2,
            "the banish task and the room change"
        );
    }

    #[test]
    fn next_room_takes_the_mark_and_changes_the_room() {
        let (mut t, gatekeeper) = setup();
        party(&mut t);
        talk(&mut t, gatekeeper, 1);
        let id = t.world.instance_id_of(Oid(1));
        let keeper = npc_of(&t, id, ROOM_GATEKEEPER_FIRST).unwrap();
        let before = room(&mut t, id);

        // A member is no leader.
        let page = event(&mut t, "next_room", keeper, 2).unwrap();
        assert!(page.contains("no_leader.html"));
        // The leader pays the mark.
        assert_eq!(event(&mut t, "next_room", keeper, 1), Some(String::new()));
        assert_eq!(t.count(1, MARK), 0);
        let after = room(&mut t, id);
        assert_ne!(after, before);
        assert!(near(&t, 2, ENTER_POINTS[usize::try_from(after).unwrap()]));
        // Without the mark: the page of no item.
        let page = event(&mut t, "next_room", keeper, 1).unwrap();
        assert!(page.contains("no_item.html"));
        assert_eq!(room(&mut t, id), after);
    }

    #[test]
    fn the_room_changes_by_itself_after_the_earthquake() {
        let (mut t, gatekeeper) = setup();
        party(&mut t);
        talk(&mut t, gatekeeper, 1);
        let id = t.world.instance_id_of(Oid(1));
        let before = room(&mut t, id);
        // 8 to 10 minutes, less the 5 seconds of the earthquake.
        t.wait_ms(8 * 60_000 - 5_000 - 1_000);
        assert_eq!(room(&mut t, id), before);
        t.wait_ms(2 * 60_000 + 7_000);
        assert_ne!(room(&mut t, id), before);
        assert!(t.world.quest_timer_count() >= 1, "the next change is scheduled");
    }

    #[test]
    fn killing_aenkinel_opens_the_boxes_and_the_room_changes_after_a_minute() {
        let (mut t, gatekeeper) = setup();
        party(&mut t);
        talk(&mut t, gatekeeper, 1);
        let id = t.world.instance_id_of(Oid(1));
        let aenkinel = script_npc(&t, npc_of(&t, id, AENKINEL).unwrap());
        assert_eq!(count_of(&t, id, BOX), 0);
        t.world.run_as_script("ChamberOfDelusionEast", |ctx| {
            ChamberOfDelusionEast.on_kill(ctx, aenkinel, Oid(1), false);
        });
        assert_eq!(count_of(&t, id, BOX), 2);
        let before = room(&mut t, id);
        t.wait_ms(60_000 + 1_000);
        assert_ne!(room(&mut t, id), before);
    }

    #[test]
    fn the_boxes_cast_together_and_die_when_the_cast_ends() {
        let (mut t, _) = setup();
        t.enter(1, 80, vec![]);
        let a = t.spawn(BOX, 30);
        let b = t.spawn(BOX, 31);
        let npc_a = ScriptNpc {
            object_id: a,
            id: BOX,
        };
        let npc_b = ScriptNpc {
            object_id: b,
            id: BOX,
        };
        t.world.run_as_script("ChamberOfDelusionEast", |ctx| {
            // `onEventReceived`: the receiver is busy and casts.
            assert_eq!(
                ChamberOfDelusionEast.on_event_received(ctx, "SCE_LUCKY", npc_a, npc_b, None),
                None
            );
            assert!(ctx.is_npc_busy(npc_b));
            // A box at full health is left alone (`getCurrentHp() < getMaxHp() / 10`).
            ChamberOfDelusionEast.on_attack(ctx, npc_a, Oid(1), 10, false, None);
            assert!(!ctx.is_npc_busy(npc_a));
        });
        // `onSpellFinished`: the right skill ends the box, another does not.
        let other = l2_world::effects::SkillKey { id: 1, level: 1 };
        let success = l2_world::effects::SkillKey { id: 5758, level: 1 };
        t.world.run_as_script("ChamberOfDelusionEast", |ctx| {
            ChamberOfDelusionEast.on_spell_finished(ctx, npc_b, Some(Oid(1)), other);
        });
        assert!(!t.world.is_dead(b));
        t.world.run_as_script("ChamberOfDelusionEast", |ctx| {
            ChamberOfDelusionEast.on_spell_finished(ctx, npc_b, Some(Oid(1)), success);
        });
        assert!(t.world.is_dead(b));
    }

    #[test]
    fn go_out_takes_the_party_back_and_the_banish_task_sends_strangers_away() {
        let (mut t, gatekeeper) = setup();
        party(&mut t);
        t.enter(3, 80, vec![]);
        talk(&mut t, gatekeeper, 1);
        let id = t.world.instance_id_of(Oid(1));
        let keeper = npc_of(&t, id, ROOM_GATEKEEPER_FIRST).unwrap();

        // A stranger inside is banished by the task after a minute.
        t.world.set_instance_id(Oid(3), id);
        assert_eq!(t.world.instance_id_of(Oid(3)), id);
        t.wait_ms(61_000);
        t.ticks(60);
        assert_eq!(t.world.instance_id_of(Oid(3)), 0);
        assert_eq!(t.world.instance_id_of(Oid(1)), id, "the party stays");

        // A member cannot take the party out.
        let page = event(&mut t, "go_out", keeper, 2).unwrap();
        assert!(page.contains("no_leader.html"));
        // The leader can: everybody goes to the saved point, the instance empties at once.
        assert_eq!(event(&mut t, "go_out", keeper, 1), Some(String::new()));
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert_eq!(t.world.instance_id_of(Oid(2)), 0);
        assert!(near(&t, 1, (X, Y, Z)) && near(&t, 2, (X, Y, Z)));
        assert!(t.world.instance_world(id).unwrap().allowed.is_empty());
        assert_eq!(t.world.instance(id).unwrap().empty_destroy_time_ms, 0);
        // Both tasks are gone.
        assert_eq!(t.world.quest_timer_count(), 0);
    }

    #[test]
    fn look_party_brings_a_member_to_the_room() {
        let (mut t, gatekeeper) = setup();
        party(&mut t);
        talk(&mut t, gatekeeper, 1);
        let id = t.world.instance_id_of(Oid(1));
        let keeper = npc_of(&t, id, ROOM_GATEKEEPER_FIRST).unwrap();
        t.world
            .tele_to_location(Oid(2), l2_world::TeleportTarget::at(X, Y, Z).with_instance(id));
        t.ticks(60);
        assert!(near(&t, 2, (X, Y, Z)));
        assert_eq!(event(&mut t, "look_party", keeper, 2), Some(String::new()));
        let current = room(&mut t, id);
        assert!(near(&t, 2, ENTER_POINTS[usize::try_from(current).unwrap()]));
    }
}
