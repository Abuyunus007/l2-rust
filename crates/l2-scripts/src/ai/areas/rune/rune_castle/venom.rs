//! Venom, the raid boss in the dungeon of Rune Castle, who moves to the throne during the siege.
// Java: dist/game/data/scripts/ai/areas/Rune/RuneCastle/Venom/Venom.java
//
// D-54: the `VenomStatus` row decides whether Venom lives, so `onSpawn` re-reads it first
// (`reread_global_variables`) and the rest of Java's `onSpawn` runs as the `SPAWN_CHECKED` event.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportTarget, TeleportWhereType};

const CASTLE: i32 = 8; // Rune
const VENOM: i32 = 29054;
const TELEPORT_CUBE: i32 = 29055;
const DUNGEON_KEEPER: i32 = 35506;

const ALIVE: i32 = 0;
const DEAD: i32 = 1;

const HOURS_BEFORE: i64 = 24;

type Location = (i32, i32, i32);

const TARGET_TELEPORTS: [Location; 12] = [
    (12860, -49158, 976),
    (14878, -51339, 1024),
    (15674, -49970, 864),
    (15696, -48326, 864),
    (14873, -46956, 1024),
    (12157, -49135, -1088),
    (12875, -46392, -288),
    (14087, -46706, -288),
    (14086, -51593, -288),
    (12864, -51898, -288),
    (15538, -49153, -1056),
    (17001, -49149, -1064),
];
const TRHONE: Location = (11025, -49152, -537);
const DUNGEON: Location = (11882, -49216, -3008);
const TELEPORT: Location = (12589, -49044, -3008);
const CUBE: Location = (12047, -49211, -3009);

const VENOM_STRIKE: SkillKey = SkillKey { id: 4993, level: 1 };
const SONIC_STORM: SkillKey = SkillKey { id: 4994, level: 1 };
const VENOM_TELEPORT: SkillKey = SkillKey { id: 4995, level: 1 };
const RANGE_TELEPORT: SkillKey = SkillKey { id: 4996, level: 1 };

#[rustfmt::skip]
const TARGET_TELEPORTS_OFFSET: [i32; 12] = [650, 100, 100, 100, 100, 650, 200, 200, 200, 200, 200, 650];

/// The continuation of `onSpawn` after the `VenomStatus` row was re-read.
const SPAWN_CHECKED: &str = "SPAWN_CHECKED";

/// The fields of the Java class (`_venom`, `_massymore`, `_loc`, `_aggroMode`, `_prisonIsOpen`,
/// and the `static` `_targets`).
#[derive(Default)]
struct VenomState {
    venom: Option<ScriptNpc>,
    massymore: Option<ScriptNpc>,
    loc: Location,
    aggro_mode: bool,
    prison_is_open: bool,
    targets: Vec<ObjectId>,
}

/// Java `ai.areas.Rune.RuneCastle.Venom.Venom`.
pub struct Venom;

#[derive(Clone, Copy)]
enum MoveTo {
    Throne,
    Prison,
}

fn in_world(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    ctx.world().npc(npc.object_id).is_some_and(|n| !n.decayed)
}

fn location_of(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> Option<Location> {
    ctx.world().position_of(npc.object_id)
}

fn tele_npc(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, (x, y, z): Location) {
    // `teleToLocation(loc, false)`: no offset.
    ctx.world_mut()
        .tele_to_location(npc.object_id, TeleportTarget::at(x, y, z).with_instance(0));
}

/// Java `checkStatus()`.
fn check_status(ctx: &mut ScriptCtx<'_>) -> i32 {
    let mut check_status = ALIVE;
    if ctx.global_variables().has_variable("VenomStatus") {
        check_status = ctx.global_variables().get_int("VenomStatus", 0);
    } else {
        ctx.set_global_variable("VenomStatus", "0");
    }
    check_status
}

/// Java `updateStatus(status)`.
fn update_status(ctx: &mut ScriptCtx<'_>, status: i32) {
    ctx.set_global_variable("VenomStatus", &status.to_string());
}

/// Java `changeLocation(loc)`.
fn change_location(ctx: &mut ScriptCtx<'_>, loc: MoveTo) {
    let Some(mut venom) = ctx.state::<VenomState>().venom else {
        return;
    };
    match loc {
        MoveTo::Throne => tele_npc(ctx, venom, TRHONE),
        MoveTo::Prison => {
            if ctx.is_dead(venom.object_id) || !in_world(ctx, venom) {
                if let Some(spawned) = ctx.add_spawn(VENOM, DUNGEON.0, DUNGEON.1, DUNGEON.2, 0, false, 0, 0) {
                    venom = spawned;
                    ctx.state::<VenomState>().venom = Some(venom);
                }
            } else {
                tele_npc(ctx, venom, DUNGEON);
            }
            ctx.cancel_quest_timer("raid_check", Some(venom), None);
            ctx.cancel_quest_timer("tower_check", Some(venom), None);
        }
    }
    if let Some(at) = location_of(ctx, venom) {
        ctx.state::<VenomState>().loc = at;
    }
}

/// Java `teleportTarget(player)`.
fn teleport_target(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    if !ctx.is_dead(player) {
        // NOTE(java-quirk): `getRandom(11)` is 0..10; the twelfth location is never used.
        let rnd = usize::try_from(ctx.get_random(11)).unwrap_or(0);
        let (x, y, z) = TARGET_TELEPORTS[rnd];
        ctx.world_mut().tele_to_location(
            player,
            TeleportTarget::at(x, y, z)
                .with_instance(0)
                .with_random_offset(TARGET_TELEPORTS_OFFSET[rnd]),
        );
        ctx.world_mut().script_set_intention_idle(player);
    }
}

impl Script for Venom {
    fn name(&self) -> &'static str {
        "Venom"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Rune/RuneCastle/Venom"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DUNGEON_KEEPER, TELEPORT_CUBE]);
        s.add_first_talk_id(&[DUNGEON_KEEPER, TELEPORT_CUBE]);
        s.add_talk_id(&[DUNGEON_KEEPER, TELEPORT_CUBE]);
        s.add_spawn_id(&[VENOM, DUNGEON_KEEPER]);
        s.add_spell_finished_id(&[VENOM]);
        s.add_attack_id(&[VENOM]);
        s.add_kill_id(&[VENOM]);
        s.add_aggro_range_enter_id(&[VENOM]);
        s.set_on_castle_siege_start();
        s.set_on_castle_siege_finish();
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        // The constructor: the prison is open in the day before the siege.
        // NOTE(java-quirk): `HOURS_BEFORE * 360000` is 6 minutes per "hour", not 3600000.
        if let Some((start_siege_date, current_time)) = ctx.world().script_castle_siege_clock(CASTLE)
            && current_time > start_siege_date - HOURS_BEFORE * 360000
            && current_time < start_siege_date
        {
            ctx.state::<VenomState>().prison_is_open = true;
        }
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        match npc.id {
            TELEPORT_CUBE => {
                ctx.world_mut()
                    .tele_to_location_where(talker, TeleportWhereType::Town);
            }
            DUNGEON_KEEPER => {
                if ctx.state::<VenomState>().prison_is_open {
                    ctx.teleport_player(talker, TELEPORT.0, TELEPORT.1, TELEPORT.2, 0, false);
                } else {
                    return Some("35506-02.html".to_owned());
                }
            }
            _ => {}
        }
        None
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "tower_check" => {
                if ctx.world().script_castle_control_tower_count(CASTLE) <= 1 {
                    change_location(ctx, MoveTo::Throne);
                    if let Some(massymore) = ctx.state::<VenomState>().massymore {
                        ctx.broadcast_npc_say(
                            massymore,
                            ChatType::NpcShout,
                            NpcStringId::OhNoTheDefensesHaveFailedItIsTooDangerousToRemainInsideTheCastleFleeEveryManForHimself,
                            &[],
                        );
                    }
                    ctx.cancel_quest_timer("tower_check", npc, None);
                    ctx.start_quest_timer("raid_check", 10000, npc, None, true);
                }
            }
            "raid_check" => {
                let npc = npc?;
                if !ctx.world().script_is_inside_siege_zone(npc.object_id)
                    && !ctx.world().is_teleporting(npc.object_id)
                {
                    let loc = ctx.state::<VenomState>().loc;
                    tele_npc(ctx, npc, loc);
                }
            }
            "cube_despawn" => {
                if let Some(npc) = npc {
                    ctx.delete_npc(npc);
                }
            }
            SPAWN_CHECKED => {
                self.spawned(ctx, npc?);
            }
            _ => {}
        }
        // NOTE(java-quirk): Java returns the event name from every event; with no player there
        // is nobody to show it to.
        Some(event.to_owned())
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _npc: ScriptNpc,
        player: ObjectId,
        is_summon: bool,
    ) {
        if is_summon {
            return;
        }
        if ctx.state::<VenomState>().aggro_mode
            && ctx.state::<VenomState>().targets.len() < 10
            && ctx.get_random(3) < 1
            && !ctx.is_dead(player)
        {
            ctx.state::<VenomState>().targets.push(player);
        }
    }

    fn on_castle_siege_start(&self, ctx: &mut ScriptCtx<'_>, castle_id: i32) {
        if castle_id != CASTLE {
            return;
        }
        ctx.state::<VenomState>().aggro_mode = true;
        ctx.state::<VenomState>().prison_is_open = false;
        if let Some(venom) = ctx.state::<VenomState>().venom
            && !ctx.is_dead(venom.object_id)
        {
            let max_hp = f64::from(ctx.npc_max_hp(venom));
            let max_mp = ctx
                .world()
                .stats(venom.object_id)
                .map_or(0.0, |s| f64::from(s.max_mp()));
            ctx.world_mut().set_current_hp_mp(venom.object_id, max_hp, max_mp);
            ctx.world_mut()
                .script_enable_skill(venom.object_id, VENOM_TELEPORT);
            ctx.world_mut()
                .script_enable_skill(venom.object_id, RANGE_TELEPORT);
            ctx.start_quest_timer("tower_check", 30000, Some(venom), None, true);
        }
    }

    fn on_castle_siege_finish(&self, ctx: &mut ScriptCtx<'_>, castle_id: i32) {
        if castle_id != CASTLE {
            return;
        }
        ctx.state::<VenomState>().aggro_mode = false;
        let venom = ctx.state::<VenomState>().venom;
        if let Some(venom) = venom
            && !ctx.is_dead(venom.object_id)
        {
            change_location(ctx, MoveTo::Prison);
            // `changeLocation` may have made a new Venom.
            let venom = ctx.state::<VenomState>().venom.unwrap_or(venom);
            ctx.world_mut()
                .script_disable_skill(venom.object_id, VENOM_TELEPORT, -1);
            ctx.world_mut()
                .script_disable_skill(venom.object_id, RANGE_TELEPORT, -1);
        }
        update_status(ctx, ALIVE);
        let venom = ctx.state::<VenomState>().venom;
        ctx.cancel_quest_timer("tower_check", venom, None);
        ctx.cancel_quest_timer("raid_check", venom, None);
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        match skill.id {
            4222 => {
                let loc = ctx.state::<VenomState>().loc;
                tele_npc(ctx, npc, loc);
            }
            4995 => {
                if let Some(player) = player {
                    teleport_target(ctx, player);
                    ctx.stop_hating(npc, player);
                }
            }
            4996 => {
                // NOTE(java-quirk): Java dereferences `player` below; without one it throws.
                let Some(player) = player else {
                    return;
                };
                teleport_target(ctx, player);
                ctx.stop_hating(npc, player);
                let targets = ctx.state::<VenomState>().targets.clone();
                if !targets.is_empty() {
                    for target in targets {
                        let (Some(p), Some(t)) =
                            (ctx.world().position_of(player), ctx.world().position_of(target))
                        else {
                            continue;
                        };
                        let x = i64::from(p.0) - i64::from(t.0);
                        let y = i64::from(p.1) - i64::from(t.1);
                        let z = i64::from(p.2) - i64::from(t.2);
                        let range: i64 = 250;
                        if (x * x) + (y * y) + (z * z) <= (range * range) {
                            teleport_target(ctx, target);
                            ctx.stop_hating(npc, target);
                        }
                    }
                    ctx.state::<VenomState>().targets.clear();
                }
            }
            _ => {}
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.reread_global_variables(&["VenomStatus"], SPAWN_CHECKED, Some(npc), None);
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
        let distance = match (
            ctx.world().position_of(npc.object_id),
            ctx.world().position_of(attacker),
        ) {
            (Some(a), Some(b)) => {
                let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
                (dx * dx + dy * dy).sqrt()
            }
            _ => 0.0,
        };
        let aggro_mode = ctx.state::<VenomState>().aggro_mode;
        if aggro_mode && ctx.get_random(100) < 25 {
            ctx.cast_skill(npc, attacker, VENOM_TELEPORT);
        } else if aggro_mode
            && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc) / 3)
            && ctx.get_random(100) < 25
            && !ctx.world().is_casting_now(npc.object_id)
        {
            ctx.cast_skill(npc, attacker, RANGE_TELEPORT);
        } else if distance > 300.0 && ctx.get_random(100) < 10 && !ctx.world().is_casting_now(npc.object_id) {
            ctx.cast_skill(npc, attacker, VENOM_STRIKE);
        } else if ctx.get_random(100) < 10 && !ctx.world().is_casting_now(npc.object_id) {
            ctx.cast_skill(npc, attacker, SONIC_STORM);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        update_status(ctx, DEAD);
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcShout,
            NpcStringId::ItSNotOverYetItWonTBeOverLikeThisNever,
            &[],
        );
        if !ctx.world().script_castle_siege_in_progress(CASTLE)
            && let Some(cube) = ctx.add_spawn(TELEPORT_CUBE, CUBE.0, CUBE.1, CUBE.2, 0, false, 0, 0)
        {
            ctx.start_quest_timer("cube_despawn", 120000, Some(cube), None, false);
        }
        ctx.cancel_quest_timer("raid_check", Some(npc), None);
    }
}

impl Venom {
    /// The body of Java's `onSpawn(npc)`, run once the `VenomStatus` row is current.
    fn spawned(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        match npc.id {
            DUNGEON_KEEPER => {
                ctx.state::<VenomState>().massymore = Some(npc);
            }
            VENOM => {
                ctx.state::<VenomState>().venom = Some(npc);
                if let Some(at) = location_of(ctx, npc) {
                    ctx.state::<VenomState>().loc = at;
                }
                ctx.world_mut()
                    .script_disable_skill(npc.object_id, VENOM_TELEPORT, -1);
                ctx.world_mut()
                    .script_disable_skill(npc.object_id, RANGE_TELEPORT, -1);
                ctx.world_mut().do_revive(npc.object_id);
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcShout,
                    NpcStringId::WhoDaresToCovetTheThroneOfOurCastleLeaveImmediatelyOrYouWillPayThePriceOfYourAudacityWithYourVeryOwnBlood,
                    &[],
                );
                ctx.set_can_return_to_spawn_point(npc, false);
                if check_status(ctx) == DEAD {
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }
        if check_status(ctx) == DEAD {
            ctx.delete_npc(npc);
        } else {
            ctx.world_mut().do_revive(npc.object_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{ScriptRegistry, ScriptSettings};

    use super::*;
    use crate::testing::{Pages, ScriptTest, X, pages_to};

    const NPCS: &str = r#"
        <npc id="29054" type="Monster" level="80" name="Venom"><stats><vitals hp="100000" mp="1000"/></stats></npc>
        <npc id="29055" type="Folk" level="70" name="Teleport Cube"/>
        <npc id="35506" type="Folk" level="70" name="Massymore"/>"#;

    fn setup(status: Option<i32>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Venom);
        let pages = Pages::of(&Venom).plain(&["35506-02.html", "35506.html", "29055.html"]);
        let mut t = ScriptTest::new_with(registry, NPCS, "", &[], |world, _| {
            world.set_script_settings(ScriptSettings {
                cycle_database_writes: true,
                ..Default::default()
            });
        });
        let _ = pages;
        if let Some(status) = status {
            t.world.run_as_script("Venom", |ctx| {
                ctx.set_global_variable("VenomStatus", &status.to_string());
            });
        }
        t
    }

    #[test]
    fn a_dead_venom_is_removed_at_spawn_and_a_live_one_is_kept() {
        let mut t = setup(Some(DEAD));
        let venom = t.spawn(VENOM, 500);
        t.ticks(4);
        assert!(!t.in_world(venom));

        let mut t = setup(None);
        let venom = t.spawn(VENOM, 500);
        t.ticks(4);
        assert!(t.in_world(venom));
        // The first look at the row writes the default.
        t.world.run_as_script("Venom", |ctx| {
            assert_eq!(ctx.global_variables().get_int("VenomStatus", 9), ALIVE);
        });
    }

    #[test]
    fn the_kill_marks_venom_dead_and_leaves_a_cube_for_two_minutes() {
        let mut t = setup(None);
        t.enter(1, 80, vec![]);
        let venom = t.spawn(VENOM, 500);
        t.ticks(4);
        t.kill(1, venom);
        t.world.run_as_script("Venom", |ctx| {
            assert_eq!(ctx.global_variables().get_int("VenomStatus", 9), DEAD);
        });
        let cube = t.npcs_of(TELEPORT_CUBE);
        assert_eq!(cube.len(), 1);
        t.wait_ms(121_000);
        assert!(!t.in_world(cube[0]));
    }

    #[test]
    fn the_siege_wakes_venom_and_its_end_sends_him_back_to_the_dungeon() {
        let mut t = setup(None);
        let venom = t.spawn(VENOM, 500);
        t.ticks(4);
        t.world.run_as_script("Venom", |ctx| {
            Venom.on_castle_siege_start(ctx, 7);
            assert!(!ctx.state::<VenomState>().aggro_mode);
            Venom.on_castle_siege_start(ctx, CASTLE);
            assert!(ctx.state::<VenomState>().aggro_mode);
            let npc = ctx.script_npc(venom).unwrap();
            assert!(ctx.has_quest_timer("tower_check", Some(npc), None));
        });
        t.world.run_as_script("Venom", |ctx| {
            Venom.on_castle_siege_finish(ctx, CASTLE);
            assert!(!ctx.state::<VenomState>().aggro_mode);
            let npc = ctx.script_npc(venom).unwrap();
            assert!(!ctx.has_quest_timer("tower_check", Some(npc), None));
        });
        // (The ground height under the place decides the z.)
        let (x, y, _) = t.world.position_of(venom).unwrap();
        assert_eq!((x, y), (DUNGEON.0, DUNGEON.1));
    }

    #[test]
    fn the_dungeon_keeper_opens_the_prison_only_before_the_siege() {
        let mut registry = ScriptRegistry::new();
        registry.add(Venom);
        let pages = Pages::of(&Venom).plain(&["35506-02.html", "35506.html", "29055.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let keeper = t.spawn(DUNGEON_KEEPER, 20);
        t.enter(1, 80, vec![]);
        t.talk(1, keeper);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("35506.html"));
        t.world.run_as_script("Venom", |ctx| {
            let npc = ctx.script_npc(keeper).unwrap();
            assert_eq!(
                Venom.on_talk(ctx, npc, ObjectId(1)).as_deref(),
                Some("35506-02.html")
            );
        });
        assert_eq!(t.world.position_of(ObjectId(1)).unwrap().0, X);

        t.world
            .run_as_script("Venom", |ctx| ctx.state::<VenomState>().prison_is_open = true);
        t.world.run_as_script("Venom", |ctx| {
            let npc = ctx.script_npc(keeper).unwrap();
            assert_eq!(Venom.on_talk(ctx, npc, ObjectId(1)), None);
        });
        let (x, y, _) = t.world.position_of(ObjectId(1)).unwrap();
        assert!((x - TELEPORT.0).abs() < 5 && (y - TELEPORT.1).abs() < 5);
    }
}
