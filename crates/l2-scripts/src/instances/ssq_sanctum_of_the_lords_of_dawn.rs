//! Sanctum of the Lords of Dawn (instance 111), the Seven Signs quest 195 (Secret Ritual of the Priests).
// Java: dist/game/data/scripts/instances/SSQSanctumOfTheLordsOfDawn/SSQSanctumOfTheLordsOfDawn.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

// NPCs
const GUARDS_OF_THE_DAWN: i32 = 18834;
const GUARDS_OF_THE_DAWN_2: i32 = 18835;
const GUARDS_OF_THE_DAWN_3: i32 = 27351;
const LIGHT_OF_DAWN: i32 = 32575;
const PASSWORD_ENTRY_DEVICE: i32 = 32577;
const IDENTITY_CONFIRM_DEVICE: i32 = 32578;
const DARKNESS_OF_DAWN: i32 = 32579;
const SHELF: i32 = 32580;

// Item
const IDENTITY_CARD: i32 = 13822;

// Skill
const GUARD_SKILL_ID: i32 = 5978;

// Locations
const ENTER: (i32, i32, i32) = (-76161, 213401, -7120);
const EXIT: (i32, i32, i32) = (-12585, 122305, -2989);

// Misc
const TEMPLATE_ID: i32 = 111;
const DOOR_ONE: i32 = 17240001;
const DOOR_TWO: i32 = 17240003;
const DOOR_THREE: i32 = 17240005;
const SAVE_POINT: [(i32, i32, i32); 4] = [
    (-75775, 213415, -7120),
    (-74959, 209240, -7472),
    (-77699, 208905, -7640),
    (-79939, 205857, -7888),
];
/// The spawn groups of the instance template whose guards send the player to `SAVE_POINT[i]`.
const SAVE_GROUPS: [&str; 4] = ["save_point1", "save_point2", "save_point3", "save_point4"];

// Movie.SSQ_RITUAL_OF_PRIEST
const SSQ_RITUAL_OF_PRIEST: i32 = 11;
// Transformation id of the Dawn's disguise.
const TRANSFORMATION: i32 = 113;

const Q00195: &str = "Q00195_SevenSignsSecretRitualOfThePriests";

/// Java `instances.SSQSanctumOfTheLordsOfDawn.SSQSanctumOfTheLordsOfDawn`.
pub struct SSQSanctumOfTheLordsOfDawn;

impl Script for SSQSanctumOfTheLordsOfDawn {
    fn name(&self) -> &'static str {
        "SSQSanctumOfTheLordsOfDawn"
    }

    fn path(&self) -> &'static str {
        "instances/SSQSanctumOfTheLordsOfDawn"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LIGHT_OF_DAWN]);
        s.add_talk_id(&[
            LIGHT_OF_DAWN,
            IDENTITY_CONFIRM_DEVICE,
            PASSWORD_ENTRY_DEVICE,
            DARKNESS_OF_DAWN,
            SHELF,
        ]);
        s.add_aggro_range_enter_id(&[GUARDS_OF_THE_DAWN, GUARDS_OF_THE_DAWN_2, GUARDS_OF_THE_DAWN_3]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "spawn" => {
                let player = player?;
                let instance_id = ctx.world().player_instance_world(player).map(|w| w.instance_id);
                if let Some(instance_id) = instance_id {
                    ctx.world_mut()
                        .spawn_instance_group(instance_id, "high_priest_of_dawn");
                    ctx.send_system_message(
                        player,
                        SystemMessageId::ByUsingTheSkillOfEinhasadSHolySwordDefeatTheEvilLilims,
                    );
                }
            }
            "teleportPlayer" => {
                // `npc.getId()` of a null NPC is a NullPointerException.
                let npc = npc?;
                let player = player?;
                match npc.id {
                    GUARDS_OF_THE_DAWN => {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::IntruderProtectThePriestsOfDawn,
                            &[],
                        );
                    }
                    GUARDS_OF_THE_DAWN_2 => {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::HowDareYouIntrudeWithThatTransformationGetLost,
                            &[],
                        );
                    }
                    GUARDS_OF_THE_DAWN_3 => {
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcGeneral,
                            NpcStringId::WhoAreYouANewFaceLikeYouCanTApproachThisPlace,
                            &[],
                        );
                    }
                    _ => {}
                }

                let instance_id = ctx.world().instance_id_of(npc.object_id);
                if let Some(world) = ctx.world().instance_world(instance_id) {
                    // The first group that holds the guard sends the player to its save point.
                    let point = SAVE_GROUPS
                        .iter()
                        .position(|group| id_list(world.parameters.get(*group)).contains(&npc.object_id));
                    if let Some(index) = point {
                        let (x, y, z) = SAVE_POINT[index];
                        let instance = ctx.world().instance_id_of(player);
                        ctx.world_mut()
                            .tele_to_location(player, TeleportTarget::at(x, y, z).with_instance(instance));
                    }
                }
            }
            _ => {}
        }
        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        match npc.id {
            LIGHT_OF_DAWN => {
                let cond3 = ctx
                    .player_quest_state(talker, Q00195)
                    .is_some_and(|q| q.is_cond(3));
                if cond3
                    && ctx.has_quest_items(talker, IDENTITY_CARD)
                    && ctx.world().transformation_id(talker) == TRANSFORMATION
                {
                    enter_instance(ctx, talker);
                    return Some("32575-01.html".to_owned());
                }
                return Some("32575-02.html".to_owned());
            }
            IDENTITY_CONFIRM_DEVICE => {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                if ctx.world().instance_world(instance_id).is_some() {
                    if ctx.has_quest_items(talker, IDENTITY_CARD)
                        && ctx.world().transformation_id(talker) == TRANSFORMATION
                    {
                        let doorst = param_int(ctx, instance_id, "doorst");
                        if doorst == 0 {
                            ctx.world_mut().instance_open_door(instance_id, DOOR_ONE);
                            ctx.send_system_message(
                                talker,
                                SystemMessageId::ByUsingTheInvisibleSkillSneakIntoTheDawnSDocumentStorage,
                            );
                            ctx.send_system_message(
                                talker,
                                SystemMessageId::MaleGuardsCanDetectTheConcealmentButTheFemaleGuardsCannot,
                            );
                            ctx.send_system_message(
                                talker,
                                SystemMessageId::FemaleGuardsNoticeTheDisguisesFromFarAwayBetterThanTheMaleGuardsDoSoBeware,
                            );
                            set_param_int(ctx, instance_id, "doorst", doorst + 1);
                            ctx.world_mut().decay(npc.object_id);
                        } else if doorst == 1 {
                            ctx.world_mut().instance_open_door(instance_id, DOOR_TWO);
                            set_param_int(ctx, instance_id, "doorst", doorst + 1);
                            ctx.world_mut().decay(npc.object_id);
                            let allowed = ctx
                                .world()
                                .instance_world(instance_id)
                                .map(|w| w.allowed.clone())
                                .unwrap_or_default();
                            for plr in allowed {
                                ctx.play_movie(plr, SSQ_RITUAL_OF_PRIEST);
                                ctx.start_quest_timer("spawn", 35000, None, Some(talker), false);
                            }
                        }
                        return Some("32578-01.html".to_owned());
                    }
                    return Some("32578-02.html".to_owned());
                }
            }
            PASSWORD_ENTRY_DEVICE => {
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                if ctx.world().instance_world(instance_id).is_some() {
                    ctx.world_mut().instance_open_door(instance_id, DOOR_THREE);
                    return Some("32577-01.html".to_owned());
                }
            }
            DARKNESS_OF_DAWN => {
                // `world.removeAllowed(talker)` of a null world is a NullPointerException.
                let instance_id = ctx.world().player_instance_world(talker).map(|w| w.instance_id)?;
                if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                    world.remove_allowed(talker);
                }
                // `teleToLocation(EXIT, 0)`: the Location carries instance 0.
                ctx.world_mut().tele_to_location(
                    talker,
                    TeleportTarget::at(EXIT.0, EXIT.1, EXIT.2).with_instance(0),
                );
                return Some("32579-01.html".to_owned());
            }
            SHELF => {
                // `world.getInstanceId()` of a null world is a NullPointerException.
                let instance_id = ctx.world().instance_id_of(npc.object_id);
                ctx.world().instance_world(instance_id)?;
                ctx.world_mut().set_instance_duration(instance_id, 300000);
                ctx.world_mut()
                    .tele_to_location(talker, TeleportTarget::at(-75925, 213399, -7128));
                return Some("32580-01.html".to_owned());
            }
            _ => {}
        }
        Some(String::new())
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) {
        ctx.disable_core_ai(npc, true);
        ctx.broadcast_magic_skill_use(npc.object_id, npc.object_id, player, GUARD_SKILL_ID, 1, 2000, 1);
        ctx.start_quest_timer("teleportPlayer", 2000, Some(npc), Some(player), false);
    }
}

/// `world.getParameters().getList("save_pointN", Npc.class)` as object ids (comma separated).
fn id_list(value: Option<&String>) -> Vec<ObjectId> {
    value
        .map(|v| {
            v.split(',')
                .filter_map(|id| id.parse().ok())
                .map(ObjectId)
                .collect()
        })
        .unwrap_or_default()
}

/// `world.getParameters().getInt(name, 0)`.
fn param_int(ctx: &ScriptCtx<'_>, instance_id: i32, name: &str) -> i32 {
    ctx.world()
        .instance_world(instance_id)
        .and_then(|w| w.parameters.get(name))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// `world.setParameter(name, value)`.
fn set_param_int(ctx: &mut ScriptCtx<'_>, instance_id: i32, name: &str, value: i32) {
    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
        world.parameters.insert(name.to_owned(), value.to_string());
    }
}

/// Java `enterInstance(talker, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let offset = ctx.world().max_offset_on_teleport();
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        |_, _| true,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                if let Some(instance_world) = world.instance_world_mut(instance_id) {
                    instance_world.add_allowed(player);
                }
                for group in SAVE_GROUPS {
                    let spawned = world.spawn_instance_group(instance_id, group);
                    let list: Vec<String> = spawned.iter().map(|id| id.0.to_string()).collect();
                    if let Some(instance_world) = world.instance_world_mut(instance_id) {
                        instance_world.parameters.insert(group.to_owned(), list.join(","));
                    }
                }
            }
            // `teleportPlayer(player, ENTER, id)`: with the random offset.
            world.tele_to_location(
                player,
                TeleportTarget::at(ENTER.0, ENTER.1, ENTER.2)
                    .with_instance(instance_id)
                    .with_random_offset(offset),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use l2_data::DoorTemplates;
    use l2_util::ObjectId as Oid;
    use l2_world::script::STARTED;
    use l2_world::{IdFactory, ScriptRegistry, TeleportTarget};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest, X, Y, Z};

    const NPCS: &str = r#"
        <npc id="32575" type="Folk" level="80" name="Light of Dawn"/>
        <npc id="32577" type="Folk" level="80" name="Password Entry Device"/>
        <npc id="32578" type="Folk" level="80" name="Identity Confirm Device"/>
        <npc id="32579" type="Folk" level="80" name="Darkness of Dawn"/>
        <npc id="32580" type="Folk" level="80" name="Shelf"/>
        <npc id="18834" type="Monster" level="80" name="Guard of the Dawn"/>
        <npc id="18828" type="Folk" level="80" name="High Priest of Dawn"/>"#;
    const ITEMS: &str = r#"<item id="13822" type="EtcItem" name="Identity Card"><set name="is_questitem" val="true"/></item>"#;
    const TEMPLATE: &str = r#"
        <exitPoint x="-12585" y="122305" z="-2989" />
        <doorlist><door id="17240001" /><door id="17240003" /><door id="17240005" /></doorlist>
        <spawnlist>
          <group name="general">
            <npc id="32578" x="-75695" y="213537" z="-7128" heading="0" respawn="60" />
            <npc id="32578" x="-78289" y="205749" z="-7884" heading="0" respawn="60" />
          </group>
          <group name="high_priest_of_dawn">
            <npc id="18828" x="-79229" y="205782" z="-7896" heading="28672" respawn="60" />
            <npc id="18828" x="-79362" y="205706" z="-7896" heading="16383" respawn="60" />
          </group>
          <group name="save_point1"><npc id="18834" x="-75190" y="210176" z="-7418" heading="0" respawn="60" /></group>
          <group name="save_point2"><npc id="18834" x="-75559" y="207860" z="-7515" heading="0" respawn="60" /></group>
          <group name="save_point3"><npc id="18834" x="-77000" y="208000" z="-7600" heading="0" respawn="60" /></group>
          <group name="save_point4"><npc id="18834" x="-79000" y="205900" z="-7880" heading="0" respawn="60" /></group>
        </spawnlist>"#;

    struct Quest;

    impl Script for Quest {
        fn name(&self) -> &'static str {
            Q00195
        }
        fn path(&self) -> &'static str {
            "quests/Standin"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Quest);
        registry.add(SSQSanctumOfTheLordsOfDawn);
        let pages = Pages::of(&SSQSanctumOfTheLordsOfDawn)
            .plain(&[
                "32575-01.html",
                "32575-02.html",
                "32577-01.html",
                "32578-01.html",
                "32578-02.html",
                "32579-01.html",
                "32580-01.html",
            ])
            .npcs(&[]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        // The doors of the normal world that the instance template copies.
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        t.world.set_id_factory(Arc::clone(&ids));
        let doors: String = [DOOR_ONE, DOOR_TWO, DOOR_THREE]
            .iter()
            .map(|id| {
                format!(
                    r#"<door id="{id}" name="door{id}" height="150" baseHpMax="1000" pos="0,0,0"
                    node1="0,0" node2="0,10" node3="10,10" node4="10,0" nodeZ="0"/>"#
                )
            })
            .collect();
        let templates = DoorTemplates::from_xml(&format!("<list>{doors}</list>")).unwrap();
        t.world.spawn_doors(&templates, &ids).unwrap();
        t.world.set_instance_data(instance_data(
            &[(111, "Sanctum of the Lords of Dawn")],
            &[(111, TEMPLATE)],
        ));
        t
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let id = t.world.npc(npc).unwrap().npc_id;
        let mut result = None;
        t.world.run_as_script("SSQSanctumOfTheLordsOfDawn", |ctx| {
            result = SSQSanctumOfTheLordsOfDawn.on_talk(ctx, ScriptNpc { object_id: npc, id }, Oid(1));
        });
        t.ticks(60);
        result
    }

    fn cond3(t: &mut ScriptTest) {
        t.world.run_as_script(Q00195, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, STARTED);
            qs.set_cond(ctx, 3, false);
        });
        t.take_writes();
    }

    /// Player 1 (with the card, cond 3) enters. The transformation 113 cannot be made in the
    /// test world (no transformation table), so the entrance is called the way `onTalk` does.
    fn inside(t: &mut ScriptTest) {
        t.enter(1, 80, vec![]);
        cond3(t);
        t.world
            .run_as_script("SSQSanctumOfTheLordsOfDawn", |ctx| enter_instance(ctx, Oid(1)));
        t.ticks(60);
    }

    fn guard_and_param(t: &ScriptTest, group: &str) -> (Oid, String) {
        let world = t.world.instance_world(t.world.instance_id_of(Oid(1))).unwrap();
        let ids = id_list(world.parameters.get(group));
        (ids[0], group.to_owned())
    }

    #[test]
    fn without_the_disguise_the_light_of_dawn_sends_the_player_away() {
        let mut t = setup();
        let light = t.spawn(LIGHT_OF_DAWN, 20);
        t.enter(1, 80, vec![]);
        // No quest.
        assert_eq!(talk(&mut t, light).as_deref(), Some("32575-02.html"));
        // Cond 3 and the card, but not transformed (113).
        cond3(&mut t);
        t.world
            .run_as_script(Q00195, |ctx| ctx.give_items(Oid(1), IDENTITY_CARD, 1));
        assert_eq!(talk(&mut t, light).as_deref(), Some("32575-02.html"));
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
    }

    #[test]
    fn entering_makes_the_instance_with_its_four_save_point_groups() {
        let mut t = setup();
        inside(&mut t);
        let instance_id = t.world.instance_id_of(Oid(1));
        assert_eq!(instance_id, 300_000);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (111, vec![Oid(1)]));
        for group in SAVE_GROUPS {
            assert_eq!(id_list(world.parameters.get(group)).len(), 1, "{group}");
        }
        assert_eq!(t.npcs_of(18834).len(), 4);
        let offset = t.world.max_offset_on_teleport();
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert!((x - ENTER.0).abs() <= offset && (y - ENTER.1).abs() <= offset);
    }

    #[test]
    fn the_devices_open_the_doors_and_the_second_one_starts_the_scene() {
        let mut t = setup();
        inside(&mut t);
        let instance_id = t.world.instance_id_of(Oid(1));
        let devices = t.npcs_of(IDENTITY_CONFIRM_DEVICE);
        assert_eq!(devices.len(), 2);
        // Without the card and the disguise: the refusal page.
        assert_eq!(talk(&mut t, devices[0]).as_deref(), Some("32578-02.html"));
        // With the card but not transformed: still refused (the disguise is part of the check).
        t.world
            .run_as_script(Q00195, |ctx| ctx.give_items(Oid(1), IDENTITY_CARD, 1));
        assert_eq!(talk(&mut t, devices[0]).as_deref(), Some("32578-02.html"));
        assert!(t.in_world(devices[0]));
        assert_eq!(param_int_of(&t, instance_id, "doorst"), 0);
    }

    fn param_int_of(t: &ScriptTest, instance_id: i32, name: &str) -> i32 {
        t.world
            .instance_world(instance_id)
            .and_then(|w| w.parameters.get(name))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    #[test]
    fn the_password_device_opens_the_third_door() {
        let mut t = setup();
        inside(&mut t);
        let instance_id = t.world.instance_id_of(Oid(1));
        // The device spawned in the instance (the template of the test has none).
        let object_id = t
            .world
            .script_add_spawn(
                PASSWORD_ENTRY_DEVICE,
                (ENTER.0, ENTER.1, ENTER.2),
                0,
                false,
                0,
                instance_id,
            )
            .unwrap();
        assert!(!t.world.is_instance_door_open(instance_id, DOOR_THREE));
        assert_eq!(talk(&mut t, object_id).as_deref(), Some("32577-01.html"));
        assert!(t.world.is_instance_door_open(instance_id, DOOR_THREE));
        assert!(!t.world.is_instance_door_open(instance_id, DOOR_ONE));
    }

    #[test]
    fn the_spawn_event_makes_the_priests_and_tells_the_player() {
        let mut t = setup();
        inside(&mut t);
        assert_eq!(t.npcs_of(18828).len(), 0);
        t.take();
        t.world.run_as_script("SSQSanctumOfTheLordsOfDawn", |ctx| {
            assert_eq!(
                SSQSanctumOfTheLordsOfDawn.on_event(ctx, "spawn", None, Some(Oid(1))),
                None
            );
        });
        assert_eq!(t.npcs_of(18828).len(), 2);
        assert!(!t.take().is_empty());
    }

    #[test]
    fn a_guard_that_sees_the_player_sends_him_to_its_save_point_two_seconds_later() {
        let mut t = setup();
        inside(&mut t);
        let (guard, _) = guard_and_param(&t, "save_point3");
        let npc = ScriptNpc {
            object_id: guard,
            id: GUARDS_OF_THE_DAWN,
        };
        t.world.run_as_script("SSQSanctumOfTheLordsOfDawn", |ctx| {
            SSQSanctumOfTheLordsOfDawn.on_aggro_range_enter(ctx, npc, Oid(1), false);
        });
        assert!(t.world.npc_core_ai_disabled(guard));
        assert_eq!(t.world.quest_timer_count(), 1);
        t.wait_ms(2_100);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (SAVE_POINT[2].0, SAVE_POINT[2].1));
        assert_eq!(t.world.instance_id_of(Oid(1)), 300_000);
    }

    #[test]
    fn the_end_devices_open_a_door_and_take_the_player_out() {
        let mut t = setup();
        inside(&mut t);
        let shelf = t.spawn(SHELF, 20);
        let darkness = t.spawn(DARKNESS_OF_DAWN, 30);
        // Both NPCs stand outside the instance in this test world: the shelf has no world
        // (NullPointerException, nothing shown), the Darkness of Dawn works on the player's world.
        assert_eq!(talk(&mut t, shelf), None);
        let instance_id = t.world.instance_id_of(Oid(1));
        t.world
            .tele_to_location(shelf, TeleportTarget::at(X, Y, Z).with_instance(instance_id));
        t.ticks(5);
        assert_eq!(talk(&mut t, shelf).as_deref(), Some("32580-01.html"));
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (-75925, 213399));

        assert_eq!(talk(&mut t, darkness).as_deref(), Some("32579-01.html"));
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (EXIT.0, EXIT.1));
        assert!(t.world.instance_world(instance_id).unwrap().allowed.is_empty());
    }
}
