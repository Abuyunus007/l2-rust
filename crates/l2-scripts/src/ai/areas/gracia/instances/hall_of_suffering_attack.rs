//! The Hall of Suffering (attack): the instance of `Q00694_BreakThroughTheHallOfSuffering`.
// Java: dist/game/data/scripts/ai/areas/Gracia/instances/HallOfSufferingAttack/HallOfSufferingAttack.java
//
// The code is the Defence's too: see `hall_of_suffering.rs`.

use super::hall_of_suffering::{HallData, hall_of_suffering};

const INSTANCEID: i32 = 115;

// @formatter:off
const ENTER_TELEPORT: (i32, i32, i32) = (-187567, 205570, -9538);

// Doors/Walls/Zones
const ROOM_1_MOBS: [(i32, i32, i32, i32); 5] = [
    (22509, -186296, 208200, -9544), (22509, -186161, 208345, -9544), (22509, -186296, 208403, -9544),
    (22510, -186107, 208113, -9528), (22510, -186350, 208200, -9544),
];
const ROOM_2_MOBS: [(i32, i32, i32, i32); 6] = [
    (22511, -184433, 210953, -9536), (22511, -184406, 211301, -9536), (22509, -184541, 211272, -9544),
    (22510, -184244, 211098, -9536), (22510, -184352, 211243, -9536), (22510, -184298, 211330, -9528),
];
const ROOM_3_MOBS: [(i32, i32, i32, i32); 8] = [
    (22512, -182611, 213984, -9520), (22512, -182908, 214071, -9520), (22512, -182962, 213868, -9512),
    (22509, -182881, 213955, -9512), (22511, -182827, 213781, -9504), (22511, -182530, 213984, -9528),
    (22510, -182935, 213723, -9512), (22510, -182557, 213868, -9520),
];
const ROOM_4_MOBS: [(i32, i32, i32, i32); 9] = [
    (22514, -180958, 216860, -9544), (22514, -181012, 216628, -9536), (22514, -181120, 216715, -9536),
    (22513, -180661, 216599, -9536), (22513, -181039, 216599, -9536), (22511, -180715, 216599, -9536),
    (22511, -181012, 216889, -9536), (22512, -180931, 216918, -9536), (22512, -180742, 216628, -9536),
];
const ROOM_5_MOBS: [(i32, i32, i32, i32); 10] = [
    (22512, -177372, 217854, -9536), (22512, -177237, 218140, -9536), (22512, -177021, 217647, -9528),
    (22513, -177372, 217792, -9544), (22513, -177372, 218053, -9536), (22514, -177291, 217734, -9544),
    (22514, -177264, 217792, -9544), (22514, -177264, 218053, -9536), (22515, -177156, 217792, -9536),
    (22515, -177075, 217647, -9528),
];
const TUMOR_SPAWNS: [(i32, i32, i32); 5] = [
    (-186327, 208286, -9544), (-184429, 211155, -9544), (-182811, 213871, -9496),
    (-181039, 216633, -9528), (-177264, 217760, -9544),
];
// @formatter:on

const DATA: HallData = HallData {
    name: "HallOfSufferingAttack",
    path: "ai/areas/Gracia/instances/HallOfSufferingAttack",
    instance_id: INSTANCEID,
    quest: "Q00694_BreakThroughTheHallOfSuffering",
    enter_teleport: ENTER_TELEPORT,
    rooms: [&ROOM_1_MOBS, &ROOM_2_MOBS, &ROOM_3_MOBS, &ROOM_4_MOBS, &ROOM_5_MOBS],
    tumor_spawns: TUMOR_SPAWNS,
};

/// Java `ai.areas.Gracia.instances.HallOfSufferingAttack.HallOfSufferingAttack`.
pub struct HallOfSufferingAttack;

hall_of_suffering!(HallOfSufferingAttack, DATA);

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestLoad, Script, ScriptCtx, ScriptNpc, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::ai::areas::gracia::instances::hall_of_suffering::{
        KLANIKUS, KLODEKUS, MOUTHOFEKIMUS, TEPIOS, TUMOR_ALIVE, TUMOR_DEAD,
    };
    use crate::testing::{ScriptTest, player_data};

    const Q694: &str = "Q00694_BreakThroughTheHallOfSuffering";

    pub(crate) const NPCS: &str = r#"
        <npc id="32537" type="Folk" level="70" name="Mouth of Ekimus"/>
        <npc id="32530" type="Folk" level="70" name="Tepios"/>
        <npc id="32531" type="Folk" level="70" name="Destroyed Tumor"/>
        <npc id="18704" type="Monster" level="79" name="Tumor of Death">
            <stats><vitals hp="1000" hpRegen="0" mp="100" mpRegen="0" /></stats></npc>
        <npc id="22509" type="Monster" level="79" name="Mob"/>
        <npc id="22510" type="Monster" level="79" name="Mob"/>
        <npc id="22511" type="Monster" level="79" name="Mob"/>
        <npc id="22512" type="Monster" level="79" name="Mob"/>
        <npc id="22513" type="Monster" level="79" name="Mob"/>
        <npc id="22514" type="Monster" level="79" name="Mob"/>
        <npc id="22515" type="Monster" level="79" name="Mob"/>
        <npc id="25665" type="RaidBoss" level="81" name="Yehan Klodekus">
            <stats><vitals hp="5000" hpRegen="0" mp="100" mpRegen="0" /></stats></npc>
        <npc id="25666" type="RaidBoss" level="81" name="Yehan Klanikus">
            <stats><vitals hp="5000" hpRegen="0" mp="100" mpRegen="0" /></stats></npc>"#;

    /// A quest of Q00694's name for the entry check.
    struct Quest694;

    impl Script for Quest694 {
        fn name(&self) -> &'static str {
            Q694
        }
        fn path(&self) -> &'static str {
            "quests/Q00694_BreakThroughTheHallOfSuffering"
        }
        fn id(&self) -> i32 {
            694
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn with_quest(t: &mut ScriptTest, id: i32, level: i32, cond: i32) {
        let mut data = player_data(id, level, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(Q694.to_owned(), Some("Started".to_owned()))],
            variables: vec![(Q694.to_owned(), "cond".to_owned(), Some(cond.to_string()))],
        };
        t.enter_with(data);
    }

    fn setup() -> (ScriptTest, ScriptNpc) {
        let mut registry = ScriptRegistry::new();
        registry.add(HallOfSufferingAttack);
        registry.add(Quest694);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let mouth = t.spawn(MOUTHOFEKIMUS, 20);
        (
            t,
            ScriptNpc {
                object_id: mouth,
                id: MOUTHOFEKIMUS,
            },
        )
    }

    fn talk(t: &mut ScriptTest, player: i32, mouth: ScriptNpc) {
        t.world.run_as_script("HallOfSufferingAttack", |ctx: &mut ScriptCtx<'_>| {
            HallOfSufferingAttack.on_talk(ctx, mouth, Oid(player));
        });
    }

    fn alive(t: &ScriptTest, npc_id: i32) -> Vec<Oid> {
        t.npcs_of(npc_id)
            .into_iter()
            .filter(|n| !t.world.is_dead(*n))
            .collect()
    }

    #[test]
    fn a_party_without_the_quest_or_its_leader_stays_out() {
        let (mut t, mouth) = setup();
        with_quest(&mut t, 1, 80, 1);
        with_quest(&mut t, 2, 80, 2);
        // Alone: not in a party.
        talk(&mut t, 1, mouth);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        t.party(1, 2);
        // Not the leader.
        talk(&mut t, 2, mouth);
        assert_eq!(t.world.instance_id_of(Oid(2)), 0);
        // The member's quest is at cond 2.
        talk(&mut t, 1, mouth);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert!(t.world.instances().all(|i| i.id <= 0));
    }

    #[test]
    fn the_party_goes_through_the_rooms_to_the_twins_and_finishes() {
        let (mut t, mouth) = setup();
        with_quest(&mut t, 1, 80, 1);
        with_quest(&mut t, 2, 80, 1);
        t.party(1, 2);
        talk(&mut t, 1, mouth);

        // Both are in the new instance, room 1 is up (status 1) with its tumor at half health.
        let instance = t.world.instance_id_of(Oid(1));
        assert!(instance > 0);
        assert_eq!(t.world.instance_id_of(Oid(2)), instance);
        let world = t.world.instance_world(instance).unwrap();
        assert_eq!((world.template_id, world.status()), (INSTANCEID, 1));
        assert_eq!(world.allowed, vec![Oid(1), Oid(2)]);
        let tumor = alive(&t, TUMOR_ALIVE)[0];
        assert_eq!(t.world.creature_hp(tumor).map(|h| h.0), Some(500.0));

        // Each room cleared opens the next; the tumor leaves a destroyed one.
        for room in 0..5 {
            let mut mobs: Vec<Oid> = [22509, 22510, 22511, 22512, 22513, 22514, 22515]
                .iter()
                .flat_map(|id| alive(&t, *id))
                .collect();
            mobs.extend(alive(&t, TUMOR_ALIVE));
            assert_eq!(mobs.len(), DATA.rooms[room].len() + 1, "room {room}");
            for mob in mobs {
                t.kill(1, mob);
            }
            assert_eq!(t.npcs_of(TUMOR_DEAD).len(), room + 1);
        }

        // The twins came (status 6) and cannot die from damage.
        assert_eq!(t.world.instance_world(instance).unwrap().status(), 6);
        let klodekus = alive(&t, KLODEKUS)[0];
        let klanikus = alive(&t, KLANIKUS)[0];
        assert!(!t.world.is_mortal(klodekus));
        let npc = ScriptNpc {
            object_id: klodekus,
            id: KLODEKUS,
        };
        // The first hit restricts the entry for a day.
        t.world.run_as_script("HallOfSufferingAttack", |ctx: &mut ScriptCtx<'_>| {
            HallOfSufferingAttack.on_attack(ctx, npc, Oid(1), 10, false, None);
        });
        assert!(t.world.instance_time(Oid(1), INSTANCEID) > 0);
        assert!(t.world.instance_time(Oid(2), INSTANCEID) > 0);
        // A killing blow on one twin only makes him look dead, to rise again.
        t.world.set_current_hp(klodekus, 5.0, false);
        t.world.run_as_script("HallOfSufferingAttack", |ctx: &mut ScriptCtx<'_>| {
            HallOfSufferingAttack.on_attack(ctx, npc, Oid(1), 10, false, None);
        });
        assert!(t.world.is_dead(klodekus));
        assert!(t.world.run_as_script("HallOfSufferingAttack", |ctx| {
            assert!(ctx.has_quest_timer("ressurectTwin", Some(npc), None));
        }));
        // The killing blow on the other then kills both.
        let other = ScriptNpc {
            object_id: klanikus,
            id: KLANIKUS,
        };
        t.world.set_current_hp(klanikus, 5.0, false);
        t.world.run_as_script("HallOfSufferingAttack", |ctx: &mut ScriptCtx<'_>| {
            HallOfSufferingAttack.on_attack(ctx, other, Oid(1), 10, false, None);
        });
        t.wait_ms(2_600);

        // Finished: status 7, Tepios, a reward tag of the fast run, five minutes left.
        let world = t.world.instance_world(instance).unwrap();
        assert_eq!(world.status(), 7);
        assert_eq!(world.parameters.get("tag").map(String::as_str), Some("13777"));
        assert_eq!(t.npcs_of(TEPIOS).len(), 1);
        t.world.run_as_script("HallOfSufferingAttack", |ctx| {
            assert!(!ctx.has_quest_timer("spawnBossGuards", Some(npc), None));
        });
    }

    #[test]
    fn talking_again_takes_a_member_back_into_the_same_instance() {
        let (mut t, mouth) = setup();
        with_quest(&mut t, 1, 80, 1);
        with_quest(&mut t, 2, 80, 1);
        t.party(1, 2);
        talk(&mut t, 1, mouth);
        let instance = t.world.instance_id_of(Oid(2));
        t.world
            .tele_to_location(Oid(2), l2_world::teleport::TeleportTarget::at(0, 1, 0).with_instance(0));
        assert_eq!(t.world.instance_id_of(Oid(2)), 0);
        talk(&mut t, 2, mouth);
        assert_eq!(t.world.instance_id_of(Oid(2)), instance);
        assert_eq!(t.world.instances().filter(|i| i.id > 0).count(), 1);
    }
}
