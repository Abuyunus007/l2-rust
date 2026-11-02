//! Rabbits event: invisible chests at Fantasy Isle that players find and open with the skills of
//! the Rabbit transformation.
// Java: dist/game/data/scripts/custom/events/Rabbits/Rabbits.java
//
// Off by default: the shipped `config.xml` schedules nothing, so the event only runs when a GM
// starts it (`//event_start Rabbits`).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TimerParams};

use super::schedule;

// NPCs
const NPC_MANAGER: i32 = 900_101;
const CHEST: i32 = 900_102;

// Skills
const RABBIT_MAGIC_EYE: SkillKey = SkillKey { id: 629, level: 1 };
const RABBIT_TORNADO: SkillKey = SkillKey { id: 630, level: 1 };
const RABBIT_TRANSFORMATION: SkillKey = SkillKey { id: 2428, level: 1 };
const RAID_CURSE: SkillKey = SkillKey { id: 4515, level: 1 };

// Misc
const EVENT_TIME: i64 = 10;
const TOTAL_CHEST_COUNT: i32 = 75;
const TRANSFORMATION_ID: i32 = 105;

/// The timer `startQuestTimer("END_RABBITS_EVENT", ..)` is.
const END_RABBITS_EVENT: &str = "END_RABBITS_EVENT";

/// Drop data: higher the chance harder the item. ItemId, chance in percent, min amount, max amount.
const DROPLIST: [[i32; 4]; 9] = [
    [1540, 80, 10, 15], // Quick Healing Potion
    [1538, 60, 5, 10],  // Blessed Scroll of Escape
    [3936, 40, 5, 10],  // Blessed Scroll of Ressurection
    [6387, 25, 5, 10],  // Blessed Scroll of Ressurection Pets
    [22025, 15, 5, 10], // Powerful Healing Potion
    [6622, 10, 1, 1],   // Giant's Codex
    [20034, 5, 1, 1],   // Revita Pop
    [20004, 1, 1, 1],   // Energy Ginseng
    [20004, 0, 1, 1],   // Energy Ginseng
];

/// The Java fields `_npcs`, `_players`, `_isActive`.
#[derive(Default)]
struct RabbitsState {
    npcs: Vec<ObjectId>,
    players: Vec<ObjectId>,
    is_active: bool,
}

/// Java `custom.events.Rabbits.Rabbits`.
pub struct Rabbits;

impl Script for Rabbits {
    fn name(&self) -> &'static str {
        "Rabbits"
    }

    fn path(&self) -> &'static str {
        "custom/events/Rabbits"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[NPC_MANAGER, CHEST]);
        s.add_talk_id(&[NPC_MANAGER]);
        s.add_start_npc(&[NPC_MANAGER]);
        s.add_skill_see_id(&[CHEST]);
        s.add_attack_id(&[CHEST]);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        schedule::load_config(ctx, "Rabbits", "Rabbits");
    }

    fn is_event(&self) -> bool {
        true
    }

    fn on_timer_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) {
        if schedule::is_schedule(event) {
            self.event_start(ctx, None);
            schedule::reschedule(ctx, event, params);
        }
    }

    fn event_start(&self, ctx: &mut ScriptCtx<'_>, event_maker: Option<ObjectId>) -> bool {
        // Don't start event if it is active.
        if ctx.state::<RabbitsState>().is_active {
            // NOTE(java-quirk): Java calls `eventMaker.sendMessage` unchecked: a scheduled start
            // (no maker) while the event runs is a NullPointerException.
            if let Some(maker) = event_maker {
                ctx.world()
                    .send_message(maker, "Event Rabbits is already started!");
            }
            return false;
        }

        // Check starting conditions
        if !ctx.world().custom_script_settings().custom_npc_data {
            tracing::info!("Rabbits: Event can't be started, because custom NPCs are disabled!");
            if let Some(maker) = event_maker {
                ctx.world().send_message(
                    maker,
                    "Event Rabbits can't be started because custom NPCs are disabled!",
                );
            }
            return false;
        }

        // Set Event active
        ctx.state::<RabbitsState>().is_active = true;

        // Spawn Manager
        record_spawn(ctx, NPC_MANAGER, -59_227, -56_939, -2039, 64_106, false, 0);

        // Spawn Chests
        for _ in 0..=TOTAL_CHEST_COUNT {
            let x = ctx.get_random_range(-60_653, -58_772);
            // NOTE(java-quirk): the range is written the wrong way round, and `Rnd.get(origin,
            // bound)` answers `origin` for it: every chest stands at y -55830.
            let y = ctx.get_random_range(-55_830, -58_146);
            record_spawn(ctx, CHEST, x, y, -2030, 0, false, EVENT_TIME * 60_000);
        }

        // Announce event start
        ctx.broadcast_to_all_online_players("Rabbits Event: Chests spawned!");
        ctx.broadcast_to_all_online_players("Rabbits Event: Go to Fantasy Isle and grab some rewards!");
        ctx.broadcast_to_all_online_players(&format!("Rabbits Event: You have {EVENT_TIME} minutes!"));
        ctx.broadcast_to_all_online_players("Rabbits Event: After that time all chests will disappear...");

        // Schedule event end
        ctx.start_quest_timer(END_RABBITS_EVENT, EVENT_TIME * 60_000, None, event_maker, false);
        true
    }

    fn event_stop(&self, ctx: &mut ScriptCtx<'_>) -> bool {
        // Don't stop inactive event.
        if !ctx.state::<RabbitsState>().is_active {
            return false;
        }

        // Set inactive
        ctx.state::<RabbitsState>().is_active = false;

        // Cancel timer
        ctx.cancel_quest_timers(END_RABBITS_EVENT);

        // Despawn NPCs
        let npcs = std::mem::take(&mut ctx.state::<RabbitsState>().npcs);
        for npc in npcs {
            if let Some(npc) = ctx.script_npc(npc) {
                ctx.delete_npc(npc);
            }
        }

        let players = std::mem::take(&mut ctx.state::<RabbitsState>().players);
        for player in players {
            if ctx.world().player(player).is_some()
                && ctx.world().transformation_id(player) == TRANSFORMATION_ID
            {
                ctx.world_mut().untransform(player);
            }
        }

        // Announce event end
        ctx.broadcast_to_all_online_players("Rabbits Event: Event has finished.");
        true
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "900101-1.htm" => {
                htmltext = Some("900101-1.htm".to_owned());
            }
            "transform" => {
                let player = player?;
                let npc = npc?;
                if ctx.world().is_transformed(player) || ctx.world().is_in_stance(player) {
                    ctx.world_mut().untransform(player);
                }

                ctx.world_mut()
                    .apply_effects(npc.object_id, player, RABBIT_TRANSFORMATION);
                // `_players.add(player)`: a list, so a second click adds the player again.
                ctx.state::<RabbitsState>().players.push(player);
            }
            END_RABBITS_EVENT => {
                ctx.broadcast_to_all_online_players("Rabbits Event: Time up!");
                self.event_stop(ctx);
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.htm", npc.id))
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if skill.id == RABBIT_TORNADO.id {
            if !ctx.world().is_object_invisible(npc.object_id) && targets.contains(&npc.object_id) {
                drop_item(ctx, npc, caster, &DROPLIST);
                ctx.delete_npc(npc);
                ctx.state::<RabbitsState>().npcs.retain(|n| *n != npc.object_id);

                if ctx.state::<RabbitsState>().npcs.is_empty() {
                    ctx.broadcast_to_all_online_players("Rabbits Event: No more chests...");
                    self.event_stop(ctx);
                }
            }
        } else if skill.id == RABBIT_MAGIC_EYE.id
            && ctx.world().is_object_invisible(npc.object_id)
            && ctx.is_inside_radius_2d(
                npc.object_id,
                caster,
                ctx.world().script_skill_affect_range(skill),
            )
        {
            ctx.world_mut().set_invisible(npc.object_id, false);
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        skill: Option<SkillKey>,
    ) {
        if ctx.state::<RabbitsState>().is_active && skill.is_none_or(|s| s.id != RABBIT_TORNADO.id) {
            ctx.world_mut().apply_effects(npc.object_id, attacker, RAID_CURSE);
        }
    }
}

/// Java `dropItem(npc, player, droplist)`.
fn drop_item(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, droplist: &[[i32; 4]]) {
    let chance = ctx.get_random(100);
    for drop in droplist {
        // NOTE(java-quirk): `>` here, `>=` in Elpies: the chance 0 entry is reached by no roll.
        if chance > drop[1] {
            let count = ctx.get_random_range(drop[2], drop[3]);
            ctx.drop_item(npc, Some(player), drop[0], i64::from(count));
            return;
        }
    }
}

/// Java `recordSpawn(npcs, npcId, x, y, z, heading, randomOffSet, despawnDelay)`.
#[allow(clippy::too_many_arguments)]
fn record_spawn(
    ctx: &mut ScriptCtx<'_>,
    npc_id: i32,
    x: i32,
    y: i32,
    z: i32,
    heading: i32,
    random_offset: bool,
    despawn_delay: i64,
) {
    let Some(npc) = ctx.add_spawn(npc_id, x, y, z, heading, random_offset, despawn_delay, 0) else {
        return;
    };
    if npc.id == CHEST {
        ctx.set_immobilized(npc, true);
        ctx.disable_core_ai(npc, true);
        ctx.world_mut().set_invisible(npc.object_id, true);
    }

    ctx.state::<RabbitsState>().npcs.push(npc.object_id);
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::custom::CustomScriptSettings;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="900101" type="Folk" level="1" name="Snowden"/>
        <npc id="900102" type="Monster" level="80" name="Chest"/>"#;

    fn scene(custom_npc_data: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Rabbits);
        let mut t = ScriptTest::new_with(registry, NPCS, "", &[], |world, _| {
            world.set_custom_script_settings(CustomScriptSettings {
                custom_npc_data,
                ..CustomScriptSettings::default()
            });
        });
        t.enter(1, 40, vec![]);
        t.take();
        t
    }

    fn start(t: &mut ScriptTest) -> bool {
        let mut started = false;
        t.world.run_as_script("Rabbits", |ctx| {
            started = Rabbits.event_start(ctx, Some(ObjectId(1)));
        });
        started
    }

    #[test]
    fn switched_off_nothing_spawns() {
        let mut t = scene(false);
        t.ticks(5);
        assert!(t.npcs_of(CHEST).is_empty() && t.npcs_of(NPC_MANAGER).is_empty());
        // Without the custom NPC table the GM start is refused as well.
        assert!(!start(&mut t));
        assert!(t.npcs_of(CHEST).is_empty());
    }

    #[test]
    fn the_event_spawns_the_manager_and_76_invisible_chests() {
        let mut t = scene(true);
        assert!(start(&mut t));
        assert_eq!(t.npcs_of(NPC_MANAGER).len(), 1);
        let chests = t.npcs_of(CHEST);
        assert_eq!(chests.len(), 76);
        assert!(chests.iter().all(|c| t.world.is_object_invisible(*c)));
        assert!(!start(&mut t));
    }

    #[test]
    fn a_chest_stops_the_event_when_it_was_the_last_one_and_the_timer_ends_it() {
        let mut t = scene(true);
        assert!(start(&mut t));
        t.wait_ms(10 * 60_000 + 2_000);
        assert!(t.npcs_of(CHEST).iter().all(|n| !t.in_world(*n)));
        assert!(start(&mut t), "stopped by the timer, so it can start again");
    }

    #[test]
    fn the_first_talk_page_is_the_npc_id() {
        let mut t = scene(true);
        assert!(start(&mut t));
        let manager = t.npcs_of(NPC_MANAGER)[0];
        t.world.run_as_script("Rabbits", |ctx| {
            let n = ctx.script_npc(manager).unwrap();
            assert_eq!(
                Rabbits.on_first_talk(ctx, n, ObjectId(1)).as_deref(),
                Some("900101.htm")
            );
        });
    }

    const SKILLS: &str = r#"
        <skill id="629" levels="1" name="Rabbit Magic Eye"><operateType>A1</operateType><targetType>SELF</targetType><affectRange>600</affectRange></skill>
        <skill id="630" levels="1" name="Rabbit Tornado"><operateType>A1</operateType><targetType>ONE</targetType></skill>"#;

    #[test]
    fn the_magic_eye_shows_a_chest_and_the_tornado_opens_it() {
        let mut t = scene(true);
        t.set_skills("", SKILLS);
        assert!(start(&mut t));
        let chest = t.npcs_of(CHEST)[0];
        let mut other = Vec::new();
        t.world.run_as_script("Rabbits", |ctx| {
            let npc = ctx.script_npc(chest).unwrap();
            // The tornado does nothing to a chest nobody has seen yet.
            Rabbits.on_skill_see(ctx, npc, ObjectId(1), RABBIT_TORNADO, &[chest], false);
            other.push(ctx.world().is_object_invisible(chest));
            // The magic eye shows the chests near the caster.
            Rabbits.on_skill_see(ctx, npc, ObjectId(1), RABBIT_MAGIC_EYE, &[], false);
            other.push(ctx.world().is_object_invisible(chest));
        });
        // The chests are far from the test player, so the eye did not show this one.
        assert_eq!(other, [true, true]);

        // A visible chest is opened by the tornado: it is gone, the event goes on.
        t.world.set_invisible(chest, false);
        t.world.run_as_script("Rabbits", |ctx| {
            let npc = ctx.script_npc(chest).unwrap();
            Rabbits.on_skill_see(ctx, npc, ObjectId(1), RABBIT_TORNADO, &[chest], false);
        });
        assert!(!t.in_world(chest));
        assert!(!start(&mut t), "the event is still running");
    }

    #[test]
    fn the_last_chest_ends_the_event() {
        let mut t = scene(true);
        assert!(start(&mut t));
        let chests = t.npcs_of(CHEST);
        for chest in &chests {
            t.world.set_invisible(*chest, false);
            let chest = *chest;
            t.world.run_as_script("Rabbits", |ctx| {
                let npc = ctx.script_npc(chest).unwrap();
                Rabbits.on_skill_see(ctx, npc, ObjectId(1), RABBIT_TORNADO, &[chest], false);
            });
        }
        // The manager is not a chest and stays in the set: `_npcs` is not empty, so the event
        // runs until the timer (Java quirk, the manager is recorded too).
        assert!(!start(&mut t));
        t.wait_ms(10 * 60_000 + 2_000);
        assert!(start(&mut t));
    }
}
