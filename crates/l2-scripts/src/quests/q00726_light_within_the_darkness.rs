//! Quest 726: Light Within the Darkness (the fortress wardens; the party of the fortress clan
//! enters the Rim Pailaka, and kills Kanadis Guide twice for 152 Knight's Epaulettes).
// Java: dist/game/data/scripts/quests/Q00726_LightWithinTheDarkness/Q00726_LightWithinTheDarkness.java
//
// Pattern: a quest with its own instance (`PAWORLD`): the wardens' `enter` event makes a dynamic
// instance of the fortress' template, and three timed waves spawn in it. The Java `Runnable`s
// on the thread pool are quest timers with the instance id in their parameters. A world of this
// script carries the parameter `script` (Java's `instanceof PAWORLD`).

use std::collections::HashMap;

use l2_protocol_game::server::ExShowScreenMessage;
use l2_protocol_game::{NpcStringId, SystemMessageId};
use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED, TimerParams};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::instances::common::{
    add_allowed, allowed, create_world, is_online, named_message, set_parameter,
};

// Items
const KNIGHTS_EPAULETTE: i32 = 9912;

// NPCs
const SEDUCED_KNIGHT: i32 = 36562;
const SEDUCED_RANGER: i32 = 36563;
const SEDUCED_MAGE: i32 = 36564;
const SEDUCED_WARRIOR: i32 = 36565;
const KANADIS_GUIDE1: i32 = 25659;
const KANADIS_GUIDE2: i32 = 25660;
const KANADIS_GUIDE3: i32 = 25661;
const KANADIS_FOLLOWER1: i32 = 25662;
const KANADIS_FOLLOWER2: i32 = 25663;
const KANADIS_FOLLOWER3: i32 = 25664;

/// Java `_fortDungeons`: the warden NPC and the instance template of its fortress.
const FORT_DUNGEONS: [(i32, i32); 21] = [
    (35666, 80),
    (35698, 81),
    (35735, 82),
    (35767, 83),
    (35804, 84),
    (35835, 85),
    (35867, 86),
    (35904, 87),
    (35936, 88),
    (35974, 89),
    (36011, 90),
    (36043, 91),
    (36081, 92),
    (36118, 93),
    (36149, 94),
    (36181, 95),
    (36219, 96),
    (36257, 97),
    (36294, 98),
    (36326, 99),
    (36364, 100),
];

/// The "world type" of Java's `PAWORLD`.
const WORLD_TYPE: &str = "Q00726";
/// `tele` of the `enter` event.
const ENTER_LOC: (i32, i32, i32) = (48200, -12232, -9128);
const REENTER_INTERVAL_MS: i64 = 14_400_000;
const WAVE_DELAY_MS: i64 = 8 * 60 * 1000;
const SPAWN_AT: (i32, i32, i32, i32) = (50536, -12232, -9384, 32768);

const Q727: &str = "Q00727_HopeWithinTheDarkness";

/// Java `FortDungeon._reEnterTime`, by warden.
#[derive(Default)]
struct ReEnterTimes(HashMap<i32, i64>);

/// Java `Q00726_LightWithinTheDarkness`.
pub struct Q00726LightWithinTheDarkness;

fn dungeon_of(npc_id: i32) -> Option<i32> {
    FORT_DUNGEONS
        .iter()
        .find(|(warden, _)| *warden == npc_id)
        .map(|(_, template)| *template)
}

/// `clan.getFortId()` of a player (0 without a clan).
fn clan_fort_id(ctx: &ScriptCtx<'_>, player: ObjectId) -> Option<i32> {
    ctx.world().clan_of(player).map(|c| c.fort_id)
}

/// Java `checkFortCondition(player, npc, isEnter)`: a page to show, `None` when all is well.
fn check_fort_condition(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    npc: ScriptNpc,
    is_enter: bool,
) -> Option<String> {
    let fort = ctx.world().npc_fort(npc.object_id);
    let dungeon = dungeon_of(npc.id);
    let (Some(fort), Some(_)) = (fort, dungeon) else {
        return Some("FortWarden-01a.htm".to_owned());
    };
    let (fort_id, fort_state) = {
        let f = ctx.world().fort(fort)?;
        (f.id.0, f.state)
    };

    if clan_fort_id(ctx, player) != Some(fort_id) {
        return Some("FortWarden-01.htm".to_owned());
    } else if fort_state == 0 {
        return Some("FortWarden-07.htm".to_owned());
    } else if fort_state == 2 {
        return Some("FortWarden-08.htm".to_owned());
    } else if is_enter {
        let reenter = ctx.state::<ReEnterTimes>().0.get(&npc.id).copied().unwrap_or(0);
        if reenter > ctx.now_ms() {
            return Some("FortWarden-09.htm".to_owned());
        }
    }

    let Some(party) = ctx.world().party_of(player) else {
        return Some("FortWarden-10.htm".to_owned());
    };
    for member in ctx.world().party_members(party) {
        let member_fort = clan_fort_id(ctx, member);
        if member_fort.is_none() || member_fort == Some(0) || member_fort != Some(fort_id) {
            // Java shows the page too (`showHtmlFile`) and returns its text.
            let name = ctx.player_name(member);
            return ctx
                .show_html_file(player, "FortWarden-11.htm", None)
                .map(|page| page.replace("%player%", &name));
        }
    }
    None
}

/// Java `checkConditions(player)`.
fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<String> {
    let Some(party) = ctx.world().party_of(player) else {
        ctx.send_system_message(
            player,
            SystemMessageId::YouAreNotCurrentlyInAPartySoYouCannotEnter,
        );
        return Some("FortWarden-10.htm".to_owned());
    };
    if ctx.world().party_leader(party) != Some(player) {
        ctx.send_system_message(player, SystemMessageId::OnlyAPartyLeaderCanMakeTheRequestToEnter);
        return None;
    }
    for member in ctx.world().party_members(party) {
        if !ctx.distance_3d(member, player).is_some_and(|d| d <= 1000.0) {
            let sm = named_message(
                ctx,
                SystemMessageId::C1IsInALocationWhichCannotBeEnteredThereforeItCannotBeProcessed,
                member,
            );
            ctx.world().send(player, &sm);
            return None;
        }
        if ctx.player_level(member) < 70 {
            let sm = named_message(
                ctx,
                SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry,
                member,
            );
            ctx.world().send(player, &sm);
            return None;
        }
    }
    None
}

/// Java `enterInstance(player, template, coords, dungeon, retValue)`.
fn enter_instance(
    ctx: &mut ScriptCtx<'_>,
    player: ObjectId,
    npc: ScriptNpc,
    ret_value: Option<String>,
) -> String {
    let (x, y, z) = ENTER_LOC;
    if let Some(world) = ctx
        .world()
        .player_instance_world(player)
        .map(|w| (w.instance_id, w.parameters.get("script").cloned()))
    {
        if world.1.as_deref() != Some(WORLD_TYPE) {
            ctx.send_system_message(
                player,
                SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon,
            );
            return String::new();
        }
        ctx.teleport_player(player, x, y, z, world.0, false);
        return String::new();
    }

    if let Some(ret) = ret_value {
        return ret;
    }
    if let Some(ret) = check_conditions(ctx, player) {
        // NOTE(java-quirk): the other refusals of `checkConditions` (not the leader, too far, too
        // low) return null in Java: the instance is made all the same.
        return ret;
    }

    let Some(template) = dungeon_of(npc.id) else {
        return String::new();
    };
    let instance = create_world(ctx, template);
    set_parameter(ctx, instance, "script", WORLD_TYPE);
    if let Some(at) = ctx.world().position_of(player)
        && let Some(i) = ctx.world_mut().instance_mut(instance)
    {
        i.exit_location = Some(at);
    }
    let reenter = ctx.now_ms() + REENTER_INTERVAL_MS;
    ctx.state::<ReEnterTimes>().0.insert(npc.id, reenter);
    ctx.add_timer(
        "SpawnNpcs",
        Some(TimerParams::new().set("instance", instance)),
        10_000,
        Some(npc),
        Some(player),
    );
    let members = match ctx.world().party_of(player) {
        Some(party) => ctx.world().party_members(party),
        None => vec![player],
    };
    for member in members {
        ctx.teleport_player(member, x, y, z, instance, false);
        add_allowed(ctx, instance, member);
        if ctx.get_quest_state(member, false).is_none() {
            ctx.get_quest_state(member, true);
        }
    }
    String::new()
}

/// Java `broadCastPacket(world, new ExShowScreenMessage(id, 2, 3000))`.
fn broadcast_stage(ctx: &mut ScriptCtx<'_>, instance: i32, message: NpcStringId) {
    let packet = ExShowScreenMessage::npc_string(message, 2, 3000, &[]);
    for player in allowed(ctx, instance) {
        if is_online(ctx, player) && ctx.world().instance_id_of(player) == instance {
            ctx.world().send(player, &packet);
        }
    }
}

fn spawn_in(ctx: &mut ScriptCtx<'_>, npc_id: i32, instance: i32) {
    let (x, y, z, heading) = SPAWN_AT;
    ctx.add_spawn(npc_id, x, y, z, heading, false, 0, instance);
}

impl Script for Q00726LightWithinTheDarkness {
    fn name(&self) -> &'static str {
        "Q00726_LightWithinTheDarkness"
    }

    fn path(&self) -> &'static str {
        "quests/Q00726_LightWithinTheDarkness"
    }

    fn id(&self) -> i32 {
        726
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let wardens = FORT_DUNGEONS.map(|(warden, _)| warden);
        s.add_start_npc(&wardens);
        s.add_talk_id(&wardens);
        s.add_kill_id(&[KANADIS_GUIDE3]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        if event.eq_ignore_ascii_case("FortWarden-04.htm") {
            qs.set(ctx, "kanadis", "0");
            qs.start_quest(ctx);
        } else if event.eq_ignore_ascii_case("reward") && qs.is_cond(ctx, 2) && qs.get_int(ctx, "done") == 1 {
            qs.set(ctx, "done", "0");
            ctx.give_items(player, KNIGHTS_EPAULETTE, 152);
            qs.exit_quest(ctx, true, true);
            return None;
        } else if event.eq_ignore_ascii_case("enter") {
            let npc = npc?;
            // The argument of `enterInstance` is evaluated first: its page may be shown.
            let ret = check_fort_condition(ctx, player, npc, true);
            return Some(enter_instance(ctx, player, npc, ret));
        }
        Some(event.to_owned())
    }

    fn on_timer_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        let Some(params) = params else {
            return;
        };
        let instance = params.get_int("instance", 0);
        // The instance is gone (Java would spawn into an id without an instance).
        if ctx.world().instance_world(instance).is_none() {
            return;
        }
        let next = |ctx: &mut ScriptCtx<'_>, event: &str, ms: i64| {
            ctx.add_timer(
                event,
                Some(TimerParams::new().set("instance", instance)),
                ms,
                npc,
                player,
            );
        };
        match event {
            // `spawnNpcs.run()` calls `FirstWave(world)`, which waits another ten seconds.
            "SpawnNpcs" => next(ctx, "FirstWave", 10_000),
            "FirstWave" => {
                for (id, x, y, z) in [
                    (SEDUCED_KNIGHT, 49384, -12232, -9384),
                    (SEDUCED_RANGER, 49192, -12232, -9384),
                    (SEDUCED_MAGE, 49192, -12456, -9392),
                    (SEDUCED_WARRIOR, 49192, -11992, -9392),
                ] {
                    ctx.add_spawn(id, x, y, z, 0, false, 0, instance);
                }
                spawn_in(ctx, KANADIS_GUIDE1, instance);
                broadcast_stage(ctx, instance, NpcStringId::BeginStage1);
                for _ in 0..10 {
                    spawn_in(ctx, KANADIS_FOLLOWER1, instance);
                }
                next(ctx, "SecondWave", WAVE_DELAY_MS);
            }
            "SecondWave" => {
                broadcast_stage(ctx, instance, NpcStringId::BeginStage2);
                spawn_in(ctx, KANADIS_GUIDE2, instance);
                for _ in 0..10 {
                    spawn_in(ctx, KANADIS_FOLLOWER2, instance);
                }
                next(ctx, "ThirdWave", WAVE_DELAY_MS);
            }
            "ThirdWave" => {
                broadcast_stage(ctx, instance, NpcStringId::BeginStage3);
                spawn_in(ctx, KANADIS_GUIDE3, instance);
                spawn_in(ctx, KANADIS_GUIDE3, instance);
                for _ in 0..10 {
                    spawn_in(ctx, KANADIS_FOLLOWER3, instance);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if let Some(ret) = check_fort_condition(ctx, player, npc, false) {
            return Some(ret);
        }
        let qs = ctx.get_quest_state(player, true)?;
        if ctx.player_quest_state(player, Q727).is_some() {
            qs.exit_quest(ctx, true, false);
            return Some("FortWarden-01b.htm".to_owned());
        }

        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if ctx.player_level(player) >= 70 {
                    htmltext = "FortWarden-01.htm".to_owned();
                } else {
                    htmltext = "FortWarden-00.htm".to_owned();
                    qs.exit_quest(ctx, true, false);
                }
            }
            STARTED => {
                htmltext = if qs.get_int(ctx, "done") == 1 {
                    "FortWarden-06.htm"
                } else {
                    "FortWarden-05.htm"
                }
                .to_owned();
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, _is_summon: bool) {
        let Some(party) = ctx.world().party_of(player) else {
            return;
        };
        let Some(instance) = ctx
            .world()
            .instance_world_of(npc.object_id)
            .filter(|w| w.parameters.get("script").map(String::as_str) == Some(WORLD_TYPE))
            .map(|w| w.instance_id)
        else {
            return;
        };
        // The distances are read first: `removeNpcs()` below takes the Guide out of the world, but
        // the Java object still has its place for the next member.
        let in_range: Vec<(ObjectId, bool)> = ctx
            .world()
            .party_members(party)
            .into_iter()
            .map(|m| (m, ctx.distance_3d(m, npc.object_id).is_some_and(|d| d <= 1000.0)))
            .collect();
        for (member, in_range) in in_range {
            let Some(qs) = ctx.get_quest_state(member, false) else {
                continue;
            };
            if qs.is_cond(ctx, 1) && npc.id == KANADIS_GUIDE3 && !ctx.is_dead(member) && in_range {
                if qs.get_int(ctx, "kanadis") == 0 {
                    qs.set(ctx, "kanadis", "1");
                } else {
                    qs.set(ctx, "done", "1");
                    qs.set_cond(ctx, 2, true);
                    ctx.world_mut().set_instance_duration(instance, 360_000);
                    ctx.world_mut().remove_instance_npcs(instance);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00726_LightWithinTheDarkness";
    const WARDEN: i32 = 35666;

    const NPCS: &str = r#"
        <npc id="35666" type="Folk" level="1" name="Fort Warden"/>
        <npc id="36562" type="Folk" level="1" name="Seduced Knight"/>
        <npc id="36563" type="Folk" level="1" name="Seduced Ranger"/>
        <npc id="36564" type="Folk" level="1" name="Seduced Mage"/>
        <npc id="36565" type="Folk" level="1" name="Seduced Warrior"/>
        <npc id="25659" type="Folk" level="1" name="Guide 1"/>
        <npc id="25660" type="Folk" level="1" name="Guide 2"/>
        <npc id="25661" type="Monster" level="1" name="Guide 3"/>
        <npc id="25662" type="Folk" level="1" name="Follower 1"/>
        <npc id="25663" type="Folk" level="1" name="Follower 2"/>
        <npc id="25664" type="Folk" level="1" name="Follower 3"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9912" type="EtcItem" name="Knight's Epaulette"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00726LightWithinTheDarkness);
        let pages = Pages::of(&Q00726LightWithinTheDarkness)
            .plain(&["FortWarden-01a.htm", "FortWarden-04.htm"])
            .npcs(&[WARDEN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let warden = t.spawn(WARDEN, 20);
        (t, warden)
    }

    fn script_npc(t: &ScriptTest, npc: Oid) -> ScriptNpc {
        ScriptNpc {
            object_id: npc,
            id: t.world.npc(npc).map_or(0, |n| n.npc_id),
        }
    }

    #[test]
    fn a_warden_outside_a_fortress_shows_the_no_fortress_page() {
        let (mut t, warden) = setup();
        t.enter(1, 80, vec![]);
        let npc = script_npc(&t, warden);
        let mut page = None;
        t.world.run_as_script(NAME, |ctx| {
            page = Q00726LightWithinTheDarkness.on_talk(ctx, npc, Oid(1));
        });
        assert_eq!(page.as_deref(), Some("FortWarden-01a.htm"));
    }

    #[test]
    fn the_quest_starts_and_pays_152_epaulettes() {
        let (mut t, _) = setup();
        t.enter(1, 80, vec![]);
        // The state exists after the first talk (Java creates it there).
        t.world.run_as_script(NAME, |ctx| {
            ctx.get_quest_state(Oid(1), true);
        });
        t.take_writes();
        let mut page = None;
        t.world.run_as_script(NAME, |ctx| {
            page = Q00726LightWithinTheDarkness.on_event(ctx, "FortWarden-04.htm", None, Some(Oid(1)));
        });
        assert_eq!(page.as_deref(), Some("FortWarden-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "kanadis", "0"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // `reward` needs cond 2 and `done` 1 (set by the instance).
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set(ctx, "done", "1");
            qs.set_cond(ctx, 2, false);
        });
        t.take_writes();
        let mut page = Some(String::new());
        t.world.run_as_script(NAME, |ctx| {
            page = Q00726LightWithinTheDarkness.on_event(ctx, "reward", None, Some(Oid(1)));
        });
        assert_eq!(page, None);
        assert_eq!(t.count(1, KNIGHTS_EPAULETTE), 152);
        assert!(t.take_writes().contains(&rows::delete(NAME, true)));
    }

    #[test]
    fn a_party_enters_the_instance_and_the_three_waves_follow() {
        let (mut t, warden) = setup();
        t.enter(1, 80, vec![]);
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        let npc = script_npc(&t, warden);
        t.world.run_as_script(NAME, |ctx| {
            assert_eq!(enter_instance(ctx, Oid(1), npc, None), "");
        });
        let instance = t.world.instance_id_of(Oid(1));
        assert!(instance > 0);
        assert_eq!(t.world.instance_id_of(Oid(2)), instance);
        let world = t.world.instance_world(instance).unwrap();
        assert_eq!(
            world.parameters.get("script").map(String::as_str),
            Some(WORLD_TYPE)
        );
        assert_eq!(world.allowed, vec![Oid(1), Oid(2)]);
        // Both got a quest state; the re-enter time keeps the warden closed.
        t.world.run_as_script(NAME, |ctx| {
            assert!(ctx.has_quest_state(Oid(1)) && ctx.has_quest_state(Oid(2)));
            assert!(ctx.state::<ReEnterTimes>().0.get(&WARDEN).copied().unwrap_or(0) > ctx.now_ms());
        });

        // Ten seconds, then ten more: the first wave (the four seduced and Guide 1 with ten followers).
        t.wait_ms(10_500);
        assert!(t.npcs_of(SEDUCED_KNIGHT).is_empty());
        t.wait_ms(10_000);
        assert_eq!(t.npcs_of(SEDUCED_KNIGHT).len(), 1);
        assert_eq!(t.npcs_of(KANADIS_GUIDE1).len(), 1);
        assert_eq!(t.npcs_of(KANADIS_FOLLOWER1).len(), 10);
        assert!(t.npcs_of(KANADIS_GUIDE2).is_empty());
        // Eight minutes: the second wave.
        t.wait_ms(8 * 60 * 1000);
        assert_eq!(t.npcs_of(KANADIS_GUIDE2).len(), 1);
        assert_eq!(t.npcs_of(KANADIS_FOLLOWER2).len(), 10);
        // Eight more: two Guides 3.
        t.wait_ms(8 * 60 * 1000);
        assert_eq!(t.npcs_of(KANADIS_GUIDE3).len(), 2);
        assert_eq!(t.npcs_of(KANADIS_FOLLOWER3).len(), 10);
    }

    #[test]
    fn the_second_guide_3_kill_completes_the_quest_for_the_party() {
        let (mut t, warden) = setup();
        t.enter(1, 80, vec![]);
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        let npc = script_npc(&t, warden);
        t.world.run_as_script(NAME, |ctx| {
            enter_instance(ctx, Oid(1), npc, None);
            for p in [1, 2] {
                let qs = ctx.get_quest_state(Oid(p), true).unwrap();
                qs.set(ctx, "kanadis", "0");
                qs.start_quest(ctx);
            }
        });
        let instance = t.world.instance_id_of(Oid(1));
        t.take_writes();

        // The teleport takes a moment: stand where the party will be.
        t.wait_ms(3_000);
        let at = t.world.position_of(Oid(1)).unwrap();
        // One guide at a time: a live one would attack the 0 HP test characters.
        let spawn_guide = |t: &mut ScriptTest| {
            let mut guide = None;
            t.world.run_as_script(NAME, |ctx| {
                guide = ctx.add_spawn(KANADIS_GUIDE3, at.0 + 50, at.1, at.2, 0, false, 0, instance);
            });
            guide.unwrap().object_id
        };
        let first = spawn_guide(&mut t);
        t.kill(1, first);
        for p in [1, 2] {
            t.world.run_as_script(NAME, |ctx| {
                let qs = ctx.get_quest_state(Oid(p), false).unwrap();
                assert_eq!((qs.get_int(ctx, "kanadis"), qs.get_int(ctx, "done")), (1, 0));
            });
        }
        let second = spawn_guide(&mut t);
        t.kill(1, second);
        for p in [1, 2] {
            t.world.run_as_script(NAME, |ctx| {
                let qs = ctx.get_quest_state(Oid(p), false).unwrap();
                assert_eq!((qs.get_cond(ctx), qs.get_int(ctx, "done")), (2, 1));
            });
        }
    }
}
