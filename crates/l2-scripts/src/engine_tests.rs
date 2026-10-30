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
