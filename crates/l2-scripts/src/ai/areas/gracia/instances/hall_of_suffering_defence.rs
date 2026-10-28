//! The Hall of Suffering (defence): the instance of `Q00695_DefendTheHallOfSuffering`.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/HallOfSufferingDefence/HallOfSufferingDefence.java
//
// The code is the Attack's too: see `hall_of_suffering.rs`.

use super::hall_of_suffering::{HallData, hall_of_suffering};

const INSTANCEID: i32 = 116; // this is the client number

// @formatter:off
const ENTER_TELEPORT: (i32, i32, i32) = (-174701, 218109, -9592);

// Doors/Walls/Zones
const ROOM_1_MOBS: [(i32, i32, i32, i32); 5] = [
    (22509, -173712, 217838, -9559), (22509, -173489, 218281, -9557), (22509, -173824, 218389, -9558),
    (22510, -174018, 217970, -9559), (22510, -173382, 218198, -9547),
];
const ROOM_2_MOBS: [(i32, i32, i32, i32); 6] = [
    (22511, -173456, 217976, -9556), (22511, -173673, 217951, -9547), (22509, -173622, 218233, -9547),
    (22510, -173775, 218218, -9545), (22510, -173660, 217980, -9542), (22510, -173712, 217838, -9559),
];
const ROOM_3_MOBS: [(i32, i32, i32, i32); 8] = [
    (22512, -173489, 218281, -9557), (22512, -173824, 218389, -9558), (22512, -174018, 217970, -9559),
    (22509, -173382, 218198, -9547), (22511, -173456, 217976, -9556), (22511, -173673, 217951, -9547),
    (22510, -173622, 218233, -9547), (22510, -173775, 218218, -9545),
];
const ROOM_4_MOBS: [(i32, i32, i32, i32); 9] = [
    (22514, -173660, 217980, -9542), (22514, -173712, 217838, -9559), (22514, -173489, 218281, -9557),
    (22513, -173824, 218389, -9558), (22513, -174018, 217970, -9559), (22511, -173382, 218198, -9547),
    (22511, -173456, 217976, -9556), (22512, -173673, 217951, -9547), (22512, -173622, 218233, -9547),
];
const ROOM_5_MOBS: [(i32, i32, i32, i32); 10] = [
    (22512, -173775, 218218, -9545), (22512, -173660, 217980, -9542), (22512, -173712, 217838, -9559),
    (22513, -173489, 218281, -9557), (22513, -173824, 218389, -9558), (22514, -174018, 217970, -9559),
    (22514, -173382, 218198, -9547), (22514, -173456, 217976, -9556), (22515, -173673, 217951, -9547),
    (22515, -173622, 218233, -9547),
];
const TUMOR_SPAWNS: [(i32, i32, i32); 5] = [
    (-173727, 218109, -9536), (-173727, 218109, -9536), (-173727, 218109, -9536),
    (-173727, 218109, -9536), (-173727, 218109, -9536),
];
// @formatter:on

const DATA: HallData = HallData {
    name: "HallOfSufferingDefence",
    path: "ai/areas/Gracia/instances/HallOfSufferingDefence",
    instance_id: INSTANCEID,
    quest: "Q00695_DefendTheHallOfSuffering",
    enter_teleport: ENTER_TELEPORT,
    rooms: [&ROOM_1_MOBS, &ROOM_2_MOBS, &ROOM_3_MOBS, &ROOM_4_MOBS, &ROOM_5_MOBS],
    tumor_spawns: TUMOR_SPAWNS,
};

/// Java `ai.areas.Gracia.instances.HallOfSufferingDefence.HallOfSufferingDefence`.
pub struct HallOfSufferingDefence;

hall_of_suffering!(HallOfSufferingDefence, DATA);

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, Script, ScriptCtx, ScriptNpc, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::ai::areas::gracia::instances::hall_of_suffering::{MOUTHOFEKIMUS, TUMOR_ALIVE};
    use crate::testing::{ScriptTest, player_data};

    const Q695: &str = "Q00695_DefendTheHallOfSuffering";

    struct Quest695;

    impl Script for Quest695 {
        fn name(&self) -> &'static str {
            Q695
        }
        fn path(&self) -> &'static str {
            "quests/Q00695_DefendTheHallOfSuffering"
        }
        fn id(&self) -> i32 {
            695
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    #[test]
    fn the_defence_starts_in_its_own_template_at_the_heart_of_the_hall() {
        let mut registry = ScriptRegistry::new();
        registry.add(HallOfSufferingDefence);
        registry.add(Quest695);
        let npcs = r#"<npc id="32537" type="Folk" level="70" name="Mouth of Ekimus"/>
            <npc id="18704" type="Monster" level="79" name="Tumor of Death"/>
            <npc id="22509" type="Monster" level="79" name="Mob"/>
            <npc id="22510" type="Monster" level="79" name="Mob"/>"#;
        let mut t = ScriptTest::new(registry, npcs, "", &[]);
        let mouth = ScriptNpc {
            object_id: t.spawn(MOUTHOFEKIMUS, 20),
            id: MOUTHOFEKIMUS,
        };
        for id in [1, 2] {
            let mut data = player_data(id, 78, vec![]);
            data.profile.quests = QuestLoad {
                states: vec![(Q695.to_owned(), Some("Started".to_owned()))],
                variables: vec![(Q695.to_owned(), "cond".to_owned(), Some("1".to_owned()))],
            };
            t.enter_with(data);
        }
        t.party(1, 2);
        t.world.run_as_script("HallOfSufferingDefence", |ctx: &mut ScriptCtx<'_>| {
            HallOfSufferingDefence.on_talk(ctx, mouth, Oid(1));
        });
        let instance = t.world.instance_id_of(Oid(1));
        assert!(instance > 0);
        let world = t.world.instance_world(instance).unwrap();
        assert_eq!((world.template_id, world.status()), (INSTANCEID, 1));
        assert_eq!(t.world.position_of(Oid(1)).map(|p| (p.0, p.1)), Some((ENTER_TELEPORT.0, ENTER_TELEPORT.1)));
        let tumor = t.npcs_of(TUMOR_ALIVE)[0];
        assert_eq!(t.world.position_of(tumor).map(|p| (p.0, p.1)), Some((-173727, 218109)));
        assert_eq!(t.npcs_of(22509).len() + t.npcs_of(22510).len(), 5);
    }
}
