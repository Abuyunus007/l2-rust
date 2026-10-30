//! Tests of the S-API-05 engine fixes and additions, run through a probe script the way a
//! quest calls them.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, NpcSay, RadarControl};
use l2_util::ObjectId as Oid;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptRegistry, ScriptSetup};

use crate::testing::{ScriptTest, X, Y, decode, name_of, names_to, pages_to};

const NAME: &str = "Probe";
const NPC_ID: i32 = 31_000;
const NPCS: &str = r#"<npc id="31000" type="Folk" level="70" name="Probe"/>"#;

struct Probe;

impl Script for Probe {
    fn name(&self) -> &'static str {
        NAME
    }

    fn path(&self) -> &'static str {
        "test/Probe"
    }

    fn id(&self) -> i32 {
        30_000
    }

    fn setup(&self, _s: &mut ScriptSetup) {}

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<Oid>,
    ) -> Option<String> {
        ctx.state::<Events>().0.push(format!("{event} {player:?}"));
        None
    }
}

/// The events the probe heard.
#[derive(Default)]
struct Events(Vec<String>);

fn events(t: &mut ScriptTest) -> Vec<String> {
    let mut out = Vec::new();
    t.world.run_as_script(NAME, |ctx| {
        out = std::mem::take(&mut ctx.state::<Events>().0);
    });
    out
}

fn setup() -> ScriptTest {
    let mut registry = ScriptRegistry::new();
    registry.add(Probe);
    let mut t = ScriptTest::new(registry, NPCS, "", &[]);
    t.enter(1, 40, vec![]);
    t
}

#[test]
fn is_cond_0_is_true_for_a_fresh_and_for_a_removed_state() {
    let mut t = setup();
    t.world.run_as_script(NAME, |ctx| {
        let qs = ctx.get_quest_state(Oid(1), true).unwrap();
        // Java `isCond(0)` of a new `QuestState` (`_cond = 0`).
        assert!(qs.is_cond(ctx, 0));
        assert!(!qs.is_cond(ctx, 1));
        assert!(qs.is_memo_state(ctx, 0));
        qs.start_quest(ctx);
        assert!(qs.is_cond(ctx, 1));
        assert!(!qs.is_cond(ctx, 0));
        // `exitQuest(true)` removes the state: the handle reads like an empty one.
        qs.exit_quest(ctx, true, false);
        assert!(qs.is_cond(ctx, 0));
        assert!(!qs.is_cond(ctx, 1));
    });
}

#[test]
fn is_inside_radius_2d_is_strict_like_java() {
    let mut t = setup();
    // Exactly 100 east of the player: Java `calculateDistance2D(..) < 100` is false.
    let at_100 = t.spawn(NPC_ID, 100);
    let at_99 = t.spawn(NPC_ID, 99);
    t.world.run_as_script(NAME, |ctx| {
        assert!(!ctx.is_inside_radius_2d(Oid(1), at_100, 100));
        assert!(ctx.is_inside_radius_2d(Oid(1), at_100, 101));
        assert!(ctx.is_inside_radius_2d(Oid(1), at_99, 100));
        assert!(!ctx.is_inside_radius_2d_xy(Oid(1), X + 100, Y, 100));
        assert!(ctx.is_inside_radius_2d_xy(Oid(1), X + 60, Y + 79, 100));
        // Not in the world.
        assert!(!ctx.is_inside_radius_2d(Oid(1), Oid(424_242), 100_000));
    });
}

/// S-Q-18 saw a click on Chamberlain Logan (35274) answer with `MoveToPawn` + `ActionFailed`
/// and no page. The castle chamberlains are built-in castle scripts (`residence/npc/`), whose
/// `onFirstTalk` page (`chamberlain-04.html` for a stranger) lives in
/// `ai/others/CastleChamberlain/`; the quest test world had no such file, and Java's
/// `showHtmlFile` sends nothing for a missing page. With the page (the real datapack has it) the
/// click shows it, so the real server does not have the problem.
#[test]
fn a_click_on_a_castle_chamberlain_shows_its_first_talk_page() {
    let npcs = r#"<npc id="35274" type="Merchant" level="75" name="Logan"/>"#;
    let page = (
        "data/scripts/ai/others/CastleChamberlain/chamberlain-04.html",
        "<html><body>chamberlain-04 %objectId%</body></html>",
    );
    for with_page in [true, false] {
        let mut registry = ScriptRegistry::new();
        registry.add(Probe);
        let pages: &[(&str, &str)] = if with_page { &[page] } else { &[] };
        let mut t = ScriptTest::new(registry, npcs, "", pages);
        t.enter(1, 40, vec![]);
        let logan = t.spawn(35_274, 20);
        t.talk(1, logan);
        let shown = pages_to(&t.take(), 1);
        if with_page {
            assert_eq!(
                shown,
                [format!("<html><body>chamberlain-04 {}</body></html>", logan.0)]
            );
        } else {
            // The S-Q-18 observation: nothing, as Java's `showHtmlFile` of a missing page.
            assert!(shown.is_empty(), "{shown:?}");
        }
    }
}

#[test]
fn schedule_event_does_not_merge_like_java_thread_pool_tasks() {
    let mut t = setup();
    t.world.run_as_script(NAME, |ctx| {
        ctx.schedule_event("zoneCheck", 1_000, None, Some(Oid(1)));
        ctx.schedule_event("zoneCheck", 1_000, None, Some(Oid(1)));
        // Not a quest timer: the name lookups do not see it.
        assert!(!ctx.has_quest_timer("zoneCheck", None, Some(Oid(1))));
        ctx.cancel_quest_timers("zoneCheck");
        // A quest timer of the same name merges.
        ctx.start_quest_timer("timer", 1_000, None, Some(Oid(1)), false);
        ctx.start_quest_timer("timer", 1_000, None, Some(Oid(1)), false);
    });
    t.wait_ms(1_100);
    assert_eq!(
        events(&mut t),
        [
            "zoneCheck Some(ObjectId(1))",
            "zoneCheck Some(ObjectId(1))",
            "timer Some(ObjectId(1))"
        ]
    );
}

#[test]
fn radar_control_and_npc_say_go_to_one_player() {
    let mut t = setup();
    t.enter(2, 40, vec![]);
    let npc = t.spawn(NPC_ID, 20);
    t.take();
    t.world.run_as_script(NAME, |ctx| {
        ctx.send_radar_control(Oid(1), 0, 2, 77_325, 205_773, -3_432);
        let npc = ctx.script_npc(npc).unwrap();
        ctx.npc_say_to(
            Oid(1),
            npc,
            ChatType::NpcGeneral,
            NpcStringId::YouChildishFoolDoYouThinkYouCanCatchMe,
            &[],
        );
    });
    let out = t.take();
    let to_1: Vec<_> = out.iter().filter(|(p, _)| *p == 1).map(|(_, b)| b).collect();
    assert_eq!(to_1.len(), 2);
    let radar: RadarControl = decode(to_1[0]);
    assert_eq!(
        (radar.show_radar, radar.radar_type, radar.x, radar.y, radar.z),
        (0, 2, 77_325, 205_773, -3_432)
    );
    assert_eq!(name_of(to_1[1]), "NPC_SAY");
    let say: NpcSay = decode(to_1[1]);
    assert_eq!(say.object_id, npc.0);
    assert_eq!(
        say.npc_string,
        NpcStringId::YouChildishFoolDoYouThinkYouCanCatchMe.id()
    );
    assert!(names_to(&out, 2).is_empty());
}

#[test]
fn the_clan_info_update_goes_to_the_clan_and_is_my_lord_needs_a_residence() {
    let mut t = setup();
    t.enter(2, 40, vec![]);
    let npc = t.spawn(NPC_ID, 20);
    t.world.create_clan(Oid(1), "Probes").unwrap();
    t.take();
    t.world.run_as_script(NAME, |ctx| {
        ctx.broadcast_clan_info_update(Oid(1));
        // Player 2 has no clan: nothing.
        ctx.broadcast_clan_info_update(Oid(2));
        let npc = ctx.script_npc(npc).unwrap();
        // The leader of a clan without castle or fortress, at an NPC without one.
        assert!(!ctx.is_my_lord(npc, Oid(1)));
        assert!(!ctx.is_my_lord(npc, Oid(2)));
    });
    let out = t.take();
    assert_eq!(names_to(&out, 1), ["PLEDGE_SHOW_INFO_UPDATE"]);
    assert!(names_to(&out, 2).is_empty());
}

/// Hears the zone 70000 (S-API-05: NPCs enter and leave zones for the scripts too).
struct ZoneProbe;

impl Script for ZoneProbe {
    fn name(&self) -> &'static str {
        "ZoneProbe"
    }

    fn path(&self) -> &'static str {
        "test/ZoneProbe"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_enter_zone_id(&[70_000]);
        s.add_exit_zone_id(&[70_000]);
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: Oid, zone_id: i32) {
        ctx.state::<Events>()
            .0
            .push(format!("enter {} {zone_id}", creature.0));
    }

    fn on_exit_zone(&self, ctx: &mut ScriptCtx<'_>, creature: Oid, zone_id: i32) {
        ctx.state::<Events>()
            .0
            .push(format!("exit {} {zone_id}", creature.0));
    }
}

fn zone_events(t: &mut ScriptTest) -> Vec<String> {
    // The zone events are queued (`notifyEventAsync`): a first call delivers them.
    t.world.run_as_script("ZoneProbe", |_| {});
    let mut out = Vec::new();
    t.world.run_as_script("ZoneProbe", |ctx| {
        out = std::mem::take(&mut ctx.state::<Events>().0);
    });
    out
}

#[test]
fn npcs_enter_and_leave_the_zones_scripts_listen_to() {
    use crate::ai::areas::gracia::testing::{load_zones, square_zone, zone_file};
    let zones = zone_file(&[square_zone(
        "probe",
        70_000,
        "ScriptZone",
        (X + 2_000, Y),
        200,
        (-5_000, 0),
        "",
    )]);
    let mut registry = ScriptRegistry::new();
    registry.add(ZoneProbe);
    let mut t = ScriptTest::new_with(
        registry,
        NPCS,
        "",
        &[("data/zones/probe.xml", &zones)],
        |world, root| load_zones(world, root),
    );
    // Spawned outside: nothing. Spawned inside: the enter.
    let outside = t.spawn(NPC_ID, 0);
    let inside = t.spawn(NPC_ID, 2_000);
    assert_eq!(zone_events(&mut t), [format!("enter {} 70000", inside.0)]);
    assert_eq!(
        t.world.npc_script_zones(inside),
        [(70_000, "ScriptZone".to_owned())]
    );
    // The outside NPC is moved in, then out again.
    let z = crate::testing::Z;
    t.world.set_position(outside, X + 2_050, Y, z);
    assert_eq!(zone_events(&mut t), [format!("enter {} 70000", outside.0)]);
    t.world.set_position(outside, X, Y, z);
    assert_eq!(zone_events(&mut t), [format!("exit {} 70000", outside.0)]);
    // A deleted NPC leaves its zones (`removeFromZones`).
    t.world.delete_npc(inside);
    assert_eq!(zone_events(&mut t), [format!("exit {} 70000", inside.0)]);
    assert!(t.world.npc_script_zones(inside).is_empty());
}

#[test]
fn point_in_range_walking_region_and_right_hand() {
    let mut t = setup();
    let npc = t.spawn(NPC_ID, 20);
    let far = t.spawn(NPC_ID, 100_000);
    t.world.set_npc_right_hand(npc, 6_579);
    t.world.run_as_script(NAME, |ctx| {
        let n = ctx.script_npc(npc).unwrap();
        // `radiusMax` 0 or below `radiusMin`: the NPC's place.
        assert_eq!(
            ctx.get_point_in_range(n, 0, 0),
            Some((X + 20, Y, crate::testing::Z))
        );
        assert_eq!(
            ctx.get_point_in_range(n, 50, 10),
            Some((X + 20, Y, crate::testing::Z))
        );
        for _ in 0..20 {
            let (x, y, z) = ctx.get_point_in_range(n, 100, 200).unwrap();
            let d = f64::from(x - (X + 20)).hypot(f64::from(y - Y));
            // The cast truncates, so the distance can be up to 1.5 below the radius.
            assert!((98.0..=201.0).contains(&d), "{d}");
            assert_eq!(z, crate::testing::Z);
        }
        assert!(ctx.is_in_surrounding_region(npc, Oid(1)));
        assert!(!ctx.is_in_surrounding_region(npc, far));
        assert_eq!(ctx.right_hand_item(n), 6_579);
        // A script's `setRandomWalking(false)` turns it off whatever the template says.
        ctx.set_random_walking(n, false);
        assert!(!ctx.is_random_walking_enabled(n));
    });
}

#[test]
fn the_test_world_has_clans_with_residences_summons_and_skills() {
    let npcs = r#"<npc id="31000" type="Folk" level="70" name="Probe"/>
        <npc id="14001" type="Servitor" level="20" name="Kat the Cat"/>"#;
    let mut registry = ScriptRegistry::new();
    registry.add(Probe);
    let mut t = ScriptTest::new(registry, npcs, "", &[]);
    t.enter(1, 40, vec![]);
    t.enter(2, 40, vec![]);

    let clan = t.clan_with_residence(1, "Lords", Some(5), None);
    let owner = t.world.clan_of(Oid(1)).unwrap();
    assert_eq!(owner.id, clan);
    assert_eq!((owner.castle_id, owner.fort_id), (5, 0));
    assert!(t.world.clan_of(Oid(2)).is_none());

    let mut t2 = ScriptTest::new(ScriptRegistry::new(), npcs, "", &[]);
    t2.enter(1, 40, vec![]);
    t2.clan_with_residence(1, "Wardens", None, Some(101));
    let warden = t2.world.clan_of(Oid(1)).unwrap();
    assert_eq!((warden.castle_id, warden.fort_id), (0, 101));

    let kat = t.summon(2, 14_001);
    assert_eq!(t.world.summon_of(Oid(2)), Some(kat));

    t.set_skills(
        "",
        r#"<skill id="1177" levels="1" name="Wind Strike"><operateType>A1</operateType><targetType>ONE</targetType></skill>"#,
    );
    let wind_strike = l2_world::SkillKey::new(1177, 1);
    assert!(t.world.script_skill_mp_hp_consume(wind_strike).is_some());
    assert!(
        t.world
            .script_skill_mp_hp_consume(l2_world::SkillKey::new(1178, 1))
            .is_none()
    );
}

#[test]
fn the_seed_map_shows_the_states_the_seed_managers_keep() {
    use l2_protocol_game::server::ExShowSeedMapInfo;
    let mut t = setup();
    t.take();
    // Nothing stored yet: both managers start at 1.
    t.input(1, l2_world::ClientInput::SeedPhase);
    let map: ExShowSeedMapInfo = decode(&t.take()[0].1);
    assert_eq!(map, ExShowSeedMapInfo::new(1, 1));
    assert_eq!(
        map.entries.iter().map(|e| e.state).collect::<Vec<_>>(),
        [2771, 2766]
    );
    t.world.run_as_script(NAME, |ctx| {
        ctx.set_global_variable("SoDState", "2");
        ctx.set_global_variable("SoI_stage", "5");
    });
    t.input(1, l2_world::ClientInput::SeedPhase);
    let map: ExShowSeedMapInfo = decode(&t.take()[0].1);
    assert_eq!(
        map.entries.iter().map(|e| e.state).collect::<Vec<_>>(),
        [2772, 2770]
    );
}

// ---------------------------------------------------------------------- tamed beasts (S-API-05)

const BEAST_NPCS: &str = r#"<npc id="16013" type="TamedBeast" level="67" name="Trained Buffalo"/>
    <npc id="18869" type="TamedBeast" level="83" name="Kookaburra"/>"#;
const BEAST_ITEMS: &str = r#"
    <item id="6643" type="EtcItem" name="Golden Spice"><set name="is_stackable" val="true"/></item>
    <item id="15474" type="EtcItem" name="Golden Spice"><set name="is_stackable" val="true"/></item>"#;

fn beast_world() -> ScriptTest {
    let mut registry = ScriptRegistry::new();
    registry.add(Probe);
    ScriptTest::new(registry, BEAST_NPCS, BEAST_ITEMS, &[])
}

#[test]
fn a_tamed_beast_follows_its_owner_and_leaves_without_food() {
    let mut t = beast_world();
    t.enter(1, 60, vec![]);
    let beast = t
        .world
        .spawn_tamed_beast(16_013, Oid(1), 2188, (X + 50, Y, crate::testing::Z), false)
        .unwrap();
    assert!(t.world.is_tamed_beast(beast));
    assert_eq!(t.world.tamed_beast_owner(beast), Some(Oid(1)));
    assert_eq!(t.world.trained_beasts(Oid(1)), [beast]);
    assert_eq!(t.world.tamed_beast_food_type(beast), 2188);
    assert_eq!(t.world.tamed_beast_remaining_time(beast), 1_200_000);
    // Not a Freya beast: others may attack it.
    assert!(t.world.is_auto_attackable_by(beast, Oid(1)));

    // Food extends the time, up to 20 minutes.
    t.world.tamed_beast_on_receive_food(beast);
    assert_eq!(t.world.tamed_beast_remaining_time(beast), 1_200_000);

    // Without spice the time runs one minute per check; below 15 minutes it is over.
    t.wait_ms(60_050);
    assert_eq!(t.world.tamed_beast_remaining_time(beast), 1_140_000);
    t.world.tamed_beast_on_receive_food(beast);
    assert_eq!(t.world.tamed_beast_remaining_time(beast), 1_160_000);
    // 1 100 000, 1 040 000, 980 000, 920 000: still here; 860 000 is below 15 minutes.
    t.wait_ms(4 * 60_000);
    assert_eq!(t.world.tamed_beast_remaining_time(beast), 920_000);
    assert!(t.in_world(beast));
    t.wait_ms(60_000);
    assert!(!t.in_world(beast));
    assert!(!t.world.is_tamed_beast(beast));
    assert!(t.world.trained_beasts(Oid(1)).is_empty());
}

#[test]
fn a_freya_beast_eats_its_spice_every_minute_and_is_not_attackable() {
    let mut t = beast_world();
    t.enter(1, 85, vec![crate::testing::item(500, 15_474, 2)]);
    let beast = t
        .world
        .spawn_tamed_beast(18_869, Oid(1), 15_474, (X + 50, Y, crate::testing::Z), true)
        .unwrap();
    assert!(t.world.is_freya_beast(beast));
    assert!(!t.world.is_auto_attackable_by(beast, Oid(1)));
    t.take();
    t.wait_ms(60_050);
    assert_eq!(t.count(1, 15_474), 1);
    assert!(names_to(&t.take(), 1).contains(&"SOCIAL_ACTION".to_owned()));
    t.wait_ms(60_000);
    assert_eq!(t.count(1, 15_474), 0);
    // No spice left: the beast goes at the next check.
    t.wait_ms(60_000);
    assert!(!t.in_world(beast));

    // A click on a Freya beast targets it, the second one does nothing.
    let beast = t
        .world
        .spawn_tamed_beast(18_869, Oid(1), 15_474, (X + 50, Y, crate::testing::Z), true)
        .unwrap();
    t.take();
    t.talk(1, beast);
    assert_eq!(t.world.target_of(Oid(1)), Some(beast));
    let names = names_to(&t.take(), 1);
    // One `ActionFailed` after each click, and the beast's own refusal of the second one.
    assert_eq!(
        names.iter().filter(|n| *n == "ACTION_FAIL").count(),
        3,
        "{names:?}"
    );
}

#[test]
fn the_beasts_are_lost_when_the_owner_teleports() {
    let mut t = beast_world();
    t.enter(1, 60, vec![]);
    let beast = t
        .world
        .spawn_tamed_beast(16_013, Oid(1), 2188, (X + 50, Y, crate::testing::Z), false)
        .unwrap();
    t.world.run_as_script(NAME, |ctx| {
        ctx.teleport_player(Oid(1), X + 3_000, Y, crate::testing::Z, 0, false);
    });
    t.input(1, l2_world::ClientInput::Appearing);
    assert!(!t.in_world(beast));
    assert!(t.world.trained_beasts(Oid(1)).is_empty());
}
