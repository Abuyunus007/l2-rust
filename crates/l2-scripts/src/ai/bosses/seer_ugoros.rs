//! Seer Ugoros, the Pa'agrio Temple's Tanta prophet: the gatekeeper Batracos takes one pass and
//! sends a single player in, Ugoros eats Abyss Weed to heal, and the killer's "Handle With Care"
//! quest gets the scale that matches how the fight went.
// Java: dist/game/data/scripts/ai/bosses/SeerUgoros/SeerUgoros.java
//
// The `static` fields of the Java class are `UgorosState` (`ctx.state`); the NPCs and the attacker
// in them are ids.
//
// Java's `ThinkTask` is a `scheduleAtFixedRate(task, initial, 3000)`. It runs here as the quest
// timer `think_start` (the initial delay, one shot) that runs the task once and starts the
// repeating `think` timer, so the task runs at `initial`, `initial + 3000`, ... like Java's.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs.
const SEER_UGOROS: i32 = 18863;
const BATRACOS: i32 = 32740;
const WEED_ID: i32 = 18867;

// Items.
const SEER_UGOROS_PASS: i32 = 15496;
const HIGH_GRADE_LIZARD_SCALE: i32 = 15497;
const MIDDLE_GRADE_LIZARD_SCALE: i32 = 15498;

// Skill (id, level).
const UGOROS_SKILL: (i32, i32) = (6426, 1);

// Locations (x, y, z, heading).
const UGOROS_SPAWN_LOCATION: (i32, i32, i32, i32) = (96804, 85604, -3720, 34360);
const BATRACOS_SPAWN_LOCATION: (i32, i32, i32, i32) = (96782, 85918, -3720, 34360);
const ENTER_LOCATION: (i32, i32, i32) = (95984, 85692, -3720);
const EXIT_LOCATION: (i32, i32, i32) = (94763, 83562, -3425);

// State.
const ALIVE: u8 = 0;
const FIGHTING: u8 = 1;
const DEAD: u8 = 2;

// The quests the script reads.
const Q00288_HANDLE_WITH_CARE: &str = "Q00288_HandleWithCare";
const Q00423_TAKE_YOUR_BEST_SHOT: &str = "Q00423_TakeYourBestShot";

// Timers. `think_start` and `think` stand for Java's `ThinkTask` (see the module notes).
const EVENT_THINK_START: &str = "think_start";
const EVENT_THINK: &str = "think";

/// The static fields of the Java class.
#[derive(Debug)]
pub(crate) struct UgorosState {
    /// `_state`.
    pub(crate) state: u8,
    /// `_weedAttack`.
    pub(crate) weed_attack: bool,
    /// `_weedKilledByPlayer`.
    pub(crate) weed_killed_by_player: bool,
    /// `_killedOneWeed`.
    pub(crate) killed_one_weed: bool,
    /// `_weed`.
    pub(crate) weed: Option<ObjectId>,
    /// `_ugoros`.
    pub(crate) ugoros: Option<ObjectId>,
    /// `_attacker`.
    pub(crate) attacker: Option<ObjectId>,
}

impl Default for UgorosState {
    fn default() -> Self {
        Self {
            state: DEAD,
            weed_attack: false,
            weed_killed_by_player: false,
            killed_one_weed: false,
            weed: None,
            ugoros: None,
            attacker: None,
        }
    }
}

/// Java `ai.bosses.SeerUgoros.SeerUgoros`.
pub struct SeerUgoros;

impl Script for SeerUgoros {
    fn name(&self) -> &'static str {
        "SeerUgoros"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/SeerUgoros"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BATRACOS]);
        s.add_talk_id(&[BATRACOS]);
        s.add_kill_id(&[SEER_UGOROS]);
        s.add_attack_id(&[WEED_ID]);
    }

    /// The constructor's `startQuestTimer("ugoros_respawn", 60000, null, null)`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        ctx.start_quest_timer("ugoros_respawn", 60_000, None, None, false);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "ugoros_respawn" => {
                if ctx.state::<UgorosState>().ugoros.is_none() {
                    let (x, y, z, heading) = UGOROS_SPAWN_LOCATION;
                    let ugoros = ctx.add_spawn(SEER_UGOROS, x, y, z, heading, false, 0, 0);
                    ctx.state::<UgorosState>().ugoros = ugoros.map(|u| u.object_id);
                    ctx.state::<UgorosState>().state = ALIVE;
                    broadcast_in_region(
                        ctx,
                        NpcStringId::ListenOhTantasIHaveReturnedTheProphetYugorosOfTheBlackAbyssIsWithMeSoDoNotBeAfraid,
                    );
                    ctx.start_quest_timer("ugoros_shout", 120_000, None, None, false);
                }
            }
            "ugoros_shout" => {
                let (state, attacker, ugoros) = {
                    let st = ctx.state::<UgorosState>();
                    (st.state, st.attacker, st.ugoros)
                };
                if state == FIGHTING {
                    if attacker.is_none() {
                        ctx.state::<UgorosState>().state = ALIVE;
                    } else if let Some(attacker) = attacker
                        && ugoros.is_none_or(|u| distance_2d(ctx, attacker, u) > 2000.0)
                    {
                        let st = ctx.state::<UgorosState>();
                        st.state = ALIVE;
                        st.attacker = None;
                    }
                } else if state == ALIVE {
                    broadcast_in_region(
                        ctx,
                        NpcStringId::ListenOhTantasTheBlackAbyssIsFamishedFindSomeFreshOfferings,
                    );
                }

                ctx.start_quest_timer("ugoros_shout", 120_000, None, None, false);
            }
            "ugoros_attack" => {
                if let Some(attacker) = ctx.state::<UgorosState>().attacker {
                    // NOTE(java-quirk): Java throws on a null `_ugoros` here (the target is
                    // changed on it first); the event does nothing then.
                    let ugoros = ctx
                        .state::<UgorosState>()
                        .ugoros
                        .and_then(|u| ctx.script_npc(u))?;
                    change_attack_target(ctx, attacker);
                    let name = ctx.player_name(attacker);
                    ctx.broadcast_npc_say(
                        ugoros,
                        ChatType::NpcGeneral,
                        NpcStringId::WelcomeS1LetUsSeeIfYouHaveBroughtAWorthyOfferingForTheBlackAbyss,
                        &[&name],
                    );
                    start_think_task(ctx, 1000);
                }
            }
            "weed_check" => {
                let (weed_attack, ugoros, weed) = {
                    let st = ctx.state::<UgorosState>();
                    (st.weed_attack, st.ugoros, st.weed)
                };
                if weed_attack && let (Some(ugoros), Some(weed)) = (ugoros, weed) {
                    let killed_by_player = ctx.state::<UgorosState>().weed_killed_by_player;
                    if ctx.is_dead(weed) && !killed_by_player {
                        {
                            let st = ctx.state::<UgorosState>();
                            st.killed_one_weed = true;
                            st.weed = None;
                            st.weed_attack = false;
                        }
                        if let Some((hp, max_hp)) = ctx.world().hp_and_max(ugoros) {
                            ctx.world_mut().set_current_hp(ugoros, hp + (max_hp * 0.2), true);
                        }
                        if let Some(ugoros) = ctx.script_npc(ugoros) {
                            ctx.broadcast_npc_say(
                                ugoros,
                                ChatType::NpcGeneral,
                                NpcStringId::WhatAFormidableFoeButIHaveTheAbyssWeedGivenToMeByTheBlackAbyssLetMeSee,
                                &[],
                            );
                        }
                    } else {
                        ctx.start_quest_timer("weed_check", 2000, None, None, false);
                    }
                } else {
                    let st = ctx.state::<UgorosState>();
                    st.weed = None;
                    st.weed_attack = false;
                }
            }
            "ugoros_expel" => {
                if let Some(attacker) = ctx.state::<UgorosState>().attacker {
                    tele_to(ctx, attacker, EXIT_LOCATION);
                    ctx.state::<UgorosState>().attacker = None;
                }
            }
            "teleportInside" => {
                let state = ctx.state::<UgorosState>().state;
                if let Some(player) = player
                    && state == ALIVE
                {
                    if ctx.has_at_least_one_quest_item(player, &[SEER_UGOROS_PASS]) {
                        {
                            let st = ctx.state::<UgorosState>();
                            st.state = FIGHTING;
                            st.attacker = Some(player);
                            st.killed_one_weed = false;
                        }
                        ctx.take_items(player, SEER_UGOROS_PASS, 1);
                        tele_to(ctx, player, ENTER_LOCATION);
                        ctx.start_quest_timer("ugoros_attack", 2000, None, None, false);

                        if let Some(qs) = ctx.quest_state_of(Q00288_HANDLE_WITH_CARE, player, false) {
                            qs.set(ctx, "drop", "1");
                        }
                    } else {
                        if ctx
                            .player_quest_state(player, Q00423_TAKE_YOUR_BEST_SHOT)
                            .is_none()
                        {
                            return Some("<html><body>Gatekeeper Batracos:<br>You look too inexperienced to make a journey to see Tanta Seer Ugoros. If you can convince Chief Investigator Johnny that you should go, then I will let you pass. Johnny has been everywhere and done everything. He may not be of my people but he has my respect, and anyone who has his will in turn have mine as well.<br></body></html>".to_owned());
                        }

                        return Some("<html><body>Gatekeeper Batracos:<br>Tanta Seer Ugoros is hard to find. You'll just have to keep looking.<br></body></html>".to_owned());
                    }
                } else {
                    return Some("<html><body>Gatekeeper Batracos:<br>Tanta Seer Ugoros is hard to find. You'll just have to keep looking.<br></body></html>".to_owned());
                }
            }
            "teleport_back" => {
                if let Some(player) = player {
                    tele_to(ctx, player, EXIT_LOCATION);
                    ctx.state::<UgorosState>().attacker = None;
                }
            }
            EVENT_THINK_START => {
                ctx.start_quest_timer(EVENT_THINK, 3000, None, None, true);
                think(ctx);
            }
            EVENT_THINK => think(ctx),
            _ => {}
        }

        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<l2_world::SkillKey>,
    ) {
        if ctx.is_dead(npc.object_id) {
            return;
        }

        let (ugoros, weed) = {
            let st = ctx.state::<UgorosState>();
            (st.ugoros, st.weed)
        };
        if let Some(ugoros) = ugoros
            && weed == Some(npc.object_id)
        {
            // Reset weed, reset attack state, set it.
            {
                let st = ctx.state::<UgorosState>();
                st.weed = None;
                st.weed_attack = false;
                st.weed_killed_by_player = true;
            }

            // Complain.
            if let Some(ugoros) = ctx.script_npc(ugoros) {
                ctx.broadcast_npc_say(
                    ugoros,
                    ChatType::NpcGeneral,
                    NpcStringId::NoHowDareYouStopMeFromUsingTheAbyssWeedDoYouKnowWhatYouHaveDone,
                    &[],
                );
            }

            // Cancel the current think-task and set it up again to think the attack over.
            start_think_task(ctx, 500);
        }

        ctx.world_mut().do_die(npc.object_id, Some(attacker));
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        cancel_think_task(ctx);

        ctx.state::<UgorosState>().state = DEAD;
        broadcast_in_region(ctx, NpcStringId::AhHowCouldILoseOhBlackAbyssReceiveMe);
        ctx.state::<UgorosState>().ugoros = None;

        let (x, y, z, heading) = BATRACOS_SPAWN_LOCATION;
        ctx.add_spawn(BATRACOS, x, y, z, heading, false, 60_000, 0);

        ctx.start_quest_timer("ugoros_expel", 50_000, None, None, false);
        ctx.start_quest_timer("ugoros_respawn", 60_000, None, None, false);

        if let Some(qs) = ctx.quest_state_of(Q00288_HANDLE_WITH_CARE, killer, false)
            && qs.is_cond(ctx, 1)
            && qs.get_int(ctx, "drop") == 1
        {
            if ctx.state::<UgorosState>().killed_one_weed {
                ctx.give_items(killer, MIDDLE_GRADE_LIZARD_SCALE, 1);
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.give_items(killer, HIGH_GRADE_LIZARD_SCALE, 1);
                qs.set_cond(ctx, 3, true);
            }

            qs.unset(ctx, "drop");
        }
    }
}

/// Java `broadcastInRegion(_ugoros, string)`: a shout to the players within 6000.
fn broadcast_in_region(ctx: &mut ScriptCtx<'_>, npc_string: NpcStringId) {
    let Some(ugoros) = ctx.state::<UgorosState>().ugoros else {
        return;
    };
    ctx.world()
        .script_broadcast_npc_say_in_range(ugoros, ChatType::NpcShout, npc_string, 6000);
}

/// `player.teleToLocation(loc)`.
fn tele_to(ctx: &mut ScriptCtx<'_>, player: ObjectId, (x, y, z): (i32, i32, i32)) {
    ctx.teleport_player(player, x, y, z, 0, false);
}

/// Java `calculateDistance2D`: `f64::MAX` when either is not in the world.
fn distance_2d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    let (Some(a), Some(b)) = (ctx.world().position_of(a), ctx.world().position_of(b)) else {
        return f64::MAX;
    };
    let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
    (dx * dx + dy * dy).sqrt()
}

/// `_thinkTask.cancel(true); _thinkTask = null;`
fn cancel_think_task(ctx: &mut ScriptCtx<'_>) {
    ctx.cancel_quest_timers(EVENT_THINK_START);
    ctx.cancel_quest_timers(EVENT_THINK);
}

/// `if (_thinkTask != null) _thinkTask.cancel(true); _thinkTask = scheduleAtFixedRate(new
/// ThinkTask(), initial, 3000);`
fn start_think_task(ctx: &mut ScriptCtx<'_>, initial_delay_ms: i64) {
    cancel_think_task(ctx);
    ctx.start_quest_timer(EVENT_THINK_START, initial_delay_ms, None, None, false);
}

/// Java `ThinkTask.run()`.
fn think(ctx: &mut ScriptCtx<'_>) {
    let (state, attacker, weed_attack, weed, ugoros) = {
        let st = ctx.state::<UgorosState>();
        (st.state, st.attacker, st.weed_attack, st.weed, st.ugoros)
    };
    if state == FIGHTING
        && let Some(attacker) = attacker
        && !ctx.is_dead(attacker)
    {
        if weed_attack && weed.is_some() {
            // Dummy, just wait.
        } else if ctx.get_random(10) < 6 {
            ctx.state::<UgorosState>().weed = None;
            // NOTE(java-quirk): a null `_ugoros` makes Java throw here, ending the task run.
            let Some(ugoros) = ugoros else {
                return;
            };
            let found = ctx
                .world()
                .visible_objects_in_range(ugoros, 2000)
                .into_iter()
                .find(|o| {
                    ctx.world().is_attackable_npc(*o)
                        && !ctx.is_dead(*o)
                        && ctx.world().npc(*o).is_some_and(|n| n.npc_id == WEED_ID)
                });
            if let Some(found) = found {
                {
                    let st = ctx.state::<UgorosState>();
                    st.weed_attack = true;
                    st.weed = Some(found);
                }
                change_attack_target(ctx, found);
                ctx.start_quest_timer("weed_check", 1000, None, None, false);
            }

            if ctx.state::<UgorosState>().weed.is_none() {
                change_attack_target(ctx, attacker);
            }
        } else {
            change_attack_target(ctx, attacker);
        }
    } else {
        {
            let st = ctx.state::<UgorosState>();
            st.state = ALIVE;
            st.attacker = None;
        }
        cancel_think_task(ctx);
    }
}

/// Java `changeAttackTarget(attacker)`: Ugoros turns on a weed (running, fearless, the Abyss
/// skill off) or on the player (walking, the skill on).
fn change_attack_target(ctx: &mut ScriptCtx<'_>, attacker: ObjectId) {
    let Some(ugoros) = ctx.state::<UgorosState>().ugoros else {
        return;
    };
    let (skill_id, skill_level) = UGOROS_SKILL;
    ctx.world_mut().npc_set_intention_idle(ugoros);
    ctx.world_mut().clear_aggro_list(ugoros);
    ctx.world_mut().set_npc_target(ugoros, Some(attacker));

    if ctx.world().is_attackable_npc(attacker) {
        ctx.state::<UgorosState>().weed_killed_by_player = false;
        ctx.world_mut()
            .disable_skill_by_id(ugoros, skill_id, skill_level, 100_000);
        ctx.world_mut().set_running(ugoros, true);
        ctx.world_mut()
            .add_damage_hate(ugoros, attacker, 0, i64::from(i32::MAX));
    } else {
        ctx.world_mut().enable_skill_by_id(ugoros, skill_id, skill_level);
        ctx.world_mut().add_damage_hate(ugoros, attacker, 0, 99);
        ctx.world_mut().set_running(ugoros, false);
    }

    ctx.world_mut().set_intention_attack(ugoros, attacker);
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, rows};

    const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision>"#;

    fn npcs() -> String {
        format!(
            r#"<npc id="18863" type="Monster" level="80" name="Seer Ugoros">{STATS}</npc>
<npc id="32740" type="Folk" level="70" name="Batracos"/>
<npc id="18867" type="Monster" level="80" name="Abyss Weed">{STATS}</npc>"#
        )
    }

    const ITEMS: &str = r#"<item id="15496" type="EtcItem" name="Pass"><set name="is_questitem" val="true"/></item>
<item id="15497" type="EtcItem" name="High scale"><set name="is_questitem" val="true"/></item>
<item id="15498" type="EtcItem" name="Middle scale"><set name="is_questitem" val="true"/></item>"#;

    /// The quest the script writes to, in its started state.
    struct Q288;

    impl Script for Q288 {
        fn name(&self) -> &'static str {
            Q00288_HANDLE_WITH_CARE
        }

        fn path(&self) -> &'static str {
            "quests/Q00288_HandleWithCare"
        }

        fn id(&self) -> i32 {
            288
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn world() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(SeerUgoros);
        registry.add(Q288);
        ScriptTest::new(registry, &npcs(), ITEMS, &[])
    }

    fn pass() -> l2_world::ItemData {
        crate::testing::item(900_001, SEER_UGOROS_PASS, 1)
    }

    fn state<R>(t: &mut ScriptTest, f: impl FnOnce(&mut UgorosState) -> R) -> R {
        let mut out = None;
        t.world
            .run_as_script("SeerUgoros", |ctx| out = Some(f(ctx.state::<UgorosState>())));
        out.unwrap()
    }

    fn event(t: &mut ScriptTest, event: &str, player: Option<i32>) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("SeerUgoros", |ctx| {
            out = SeerUgoros.on_event(ctx, event, None, player.map(ObjectId));
        });
        out
    }

    fn has_timer(t: &mut ScriptTest, name: &str) -> bool {
        let mut found = false;
        t.world
            .run_as_script("SeerUgoros", |ctx| found = ctx.has_quest_timer(name, None, None));
        found
    }

    #[test]
    fn ugoros_comes_a_minute_after_the_start_and_shouts() {
        let mut t = world();
        assert_eq!(state(&mut t, |s| s.state), DEAD);
        assert!(t.npcs_of(SEER_UGOROS).is_empty());
        t.wait_ms(60_100);
        let ugoros = t.npcs_of(SEER_UGOROS);
        assert_eq!(ugoros.len(), 1);
        assert_eq!(t.world.position_of(ugoros[0]).unwrap(), (96804, 85604, -3720));
        assert_eq!(state(&mut t, |s| s.state), ALIVE);
        assert_eq!(state(&mut t, |s| s.ugoros), Some(ugoros[0]));
        assert!(has_timer(&mut t, "ugoros_shout"));
        // A second respawn event does nothing while he lives.
        event(&mut t, "ugoros_respawn", None);
        assert_eq!(t.npcs_of(SEER_UGOROS).len(), 1);
    }

    #[test]
    fn the_gate_refuses_without_a_pass_or_while_ugoros_is_away() {
        let mut t = world();
        t.enter(1, 80, vec![]);
        // Not there yet: the same answer for everyone.
        let page = event(&mut t, "teleportInside", Some(1)).unwrap();
        assert!(page.contains("hard to find"), "{page}");
        t.wait_ms(60_100);
        // No pass and no Take Your Best Shot: the "inexperienced" page.
        let page = event(&mut t, "teleportInside", Some(1)).unwrap();
        assert!(page.contains("too inexperienced"), "{page}");
        assert_eq!(state(&mut t, |s| s.state), ALIVE);
    }

    #[test]
    fn a_pass_takes_the_player_in_and_marks_the_quest() {
        let mut t = world();
        t.enter(1, 80, vec![pass()]);
        t.world.run_as_script(Q00288_HANDLE_WITH_CARE, |ctx| {
            let qs = ctx.get_quest_state(ObjectId(1), true).unwrap();
            qs.start_quest(ctx);
        });
        t.take_writes();
        t.wait_ms(60_100);
        assert_eq!(event(&mut t, "teleportInside", Some(1)), None);
        assert_eq!(state(&mut t, |s| s.state), FIGHTING);
        assert_eq!(state(&mut t, |s| s.attacker), Some(ObjectId(1)));
        assert_eq!(t.count(1, SEER_UGOROS_PASS), 0);
        assert!(has_timer(&mut t, "ugoros_attack"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(Q00288_HANDLE_WITH_CARE, "drop", "1")]
        );
        // The attack starts after two seconds; the think task runs a second later.
        t.wait_ms(2_100);
        assert!(has_timer(&mut t, EVENT_THINK_START));
        t.wait_ms(1_000);
        assert!(has_timer(&mut t, EVENT_THINK));
    }

    #[test]
    fn the_think_task_ends_when_the_attacker_is_gone() {
        let mut t = world();
        t.enter(1, 80, vec![pass()]);
        t.wait_ms(60_100);
        event(&mut t, "teleportInside", Some(1));
        t.wait_ms(3_200);
        assert!(has_timer(&mut t, EVENT_THINK));
        t.world.set_current_hp_mp(ObjectId(1), 0.0, 0.0);
        t.world.do_die(ObjectId(1), None);
        event(&mut t, EVENT_THINK, None);
        assert_eq!(state(&mut t, |s| s.state), ALIVE);
        assert_eq!(state(&mut t, |s| s.attacker), None);
        assert!(!has_timer(&mut t, EVENT_THINK));
    }

    #[test]
    fn a_weed_that_is_hit_dies_and_makes_ugoros_complain() {
        let mut t = world();
        t.enter(1, 80, vec![]);
        t.wait_ms(60_100);
        let ugoros = state(&mut t, |s| s.ugoros).unwrap();
        let weed = t.spawn(WEED_ID, 10);
        state(&mut t, |s| {
            s.weed = Some(weed);
            s.weed_attack = true;
        });
        t.world.run_as_script("SeerUgoros", |ctx| {
            let weed = ctx.script_npc(weed).unwrap();
            SeerUgoros.on_attack(ctx, weed, ObjectId(1), 5, false, None);
        });
        assert!(t.world.is_dead(weed));
        assert!(state(&mut t, |s| s.weed_killed_by_player));
        assert_eq!(state(&mut t, |s| s.weed), None);
        assert!(!state(&mut t, |s| s.weed_attack));
        assert!(has_timer(&mut t, EVENT_THINK_START));
        assert!(t.in_world(ugoros));
    }

    #[test]
    fn a_weed_that_dies_without_a_player_heals_ugoros() {
        let mut t = world();
        t.wait_ms(60_100);
        let ugoros = state(&mut t, |s| s.ugoros).unwrap();
        t.world.set_current_hp_mp(ugoros, 100.0, 500.0);
        let weed = t.spawn(WEED_ID, 10);
        t.world.do_die(weed, None);
        state(&mut t, |s| {
            s.weed = Some(weed);
            s.weed_attack = true;
            s.weed_killed_by_player = false;
        });
        event(&mut t, "weed_check", None);
        assert!(state(&mut t, |s| s.killed_one_weed));
        assert_eq!(t.world.hp_and_max(ugoros).unwrap().0, 300.0);
    }

    #[test]
    fn killing_ugoros_pays_the_scale_by_the_weeds_and_calls_batracos() {
        for (killed_one_weed, scale, cond) in [
            (true, MIDDLE_GRADE_LIZARD_SCALE, "2"),
            (false, HIGH_GRADE_LIZARD_SCALE, "3"),
        ] {
            let mut t = world();
            t.enter(1, 80, vec![]);
            t.world.run_as_script(Q00288_HANDLE_WITH_CARE, |ctx| {
                let qs = ctx.get_quest_state(ObjectId(1), true).unwrap();
                qs.start_quest(ctx);
                qs.set(ctx, "drop", "1");
            });
            t.take_writes();
            t.wait_ms(60_100);
            state(&mut t, |s| s.killed_one_weed = killed_one_weed);
            let ugoros = state(&mut t, |s| s.ugoros).unwrap();
            t.kill(1, ugoros);
            assert_eq!(t.count(1, scale), 1);
            assert_eq!(state(&mut t, |s| s.state), DEAD);
            assert_eq!(state(&mut t, |s| s.ugoros), None);
            assert_eq!(t.npcs_of(BATRACOS).len(), 1);
            // `setCond(.., true)` also stores the completed-state flags of the journal.
            let writes = t.take_writes();
            assert_eq!(
                writes.first(),
                Some(&rows::update(Q00288_HANDLE_WITH_CARE, "cond", cond))
            );
            assert_eq!(
                writes.last(),
                Some(&rows::delete_var(Q00288_HANDLE_WITH_CARE, "drop"))
            );
            // Batracos leaves after a minute, Ugoros is back as well.
            t.wait_ms(61_000);
            assert!(t.npcs_of(BATRACOS).is_empty());
            assert_eq!(t.npcs_of(SEER_UGOROS).len(), 1);
        }
    }

    #[test]
    fn the_expel_and_the_way_back_send_the_attacker_out() {
        let mut t = world();
        t.enter(1, 80, vec![pass()]);
        t.wait_ms(60_100);
        event(&mut t, "teleportInside", Some(1));
        t.wait_ms(500);
        assert_eq!(event(&mut t, "ugoros_expel", None), None);
        assert_eq!(state(&mut t, |s| s.attacker), None);
        state(&mut t, |s| s.attacker = Some(ObjectId(1)));
        event(&mut t, "teleport_back", Some(1));
        assert_eq!(state(&mut t, |s| s.attacker), None);
    }
}
