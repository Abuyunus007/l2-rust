//! Queen Shyeed, the Stakato Nest raid boss: the respawn row in the global variables, the
//! three-hour despawn, and the Stakato Nest buff zones she switches.
// Java: dist/game/data/scripts/ai/bosses/QueenShyeed/QueenShyeed.java
//
// The Java constructor is `on_load` (it decides on the `QueenShyeedRespawn` row loaded at start).
//
// Side by side (D-54): the row is a global variable. `respawn` reads it again before it decides
// (`ctx.reread_global_variables`); with `CycleDatabaseWrites` off the row written here stays in
// memory.
//
// The zones 200103 (`MOB_BUFF_ZONE`) is commented out in Java and is not touched here either.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC.
const SHYEED: i32 = 25671;
const SHYEED_LOC: (i32, i32, i32, i32) = (79634, -55428, -6104, 0);

// Respawn.
const RESPAWN: i64 = 86_400_000; // 24 h
const RANDOM_RESPAWN: i32 = 43_200_000; // 12 h

// Zones (`EffectZone`).
const MOB_BUFF_DISPLAY_ZONE: i32 = 200_104;
const PC_BUFF_ZONE: i32 = 200_105;

/// The `global_variables` row of the next respawn.
const RESPAWN_VARIABLE: &str = "QueenShyeedRespawn";

// Events.
const EVENT_RESPAWN: &str = "respawn";
const EVENT_DESPAWN: &str = "despawn";
/// Not a Java event: the continuation of the D-54 re-read `respawn` asks for.
const EVENT_RESPAWN_ROW_READ: &str = "D54_RESPAWN_ROW_READ";

/// Java `ai.bosses.QueenShyeed.QueenShyeed`.
pub struct QueenShyeed;

impl Script for QueenShyeed {
    fn name(&self) -> &'static str {
        "QueenShyeed"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/QueenShyeed"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&[SHYEED]);
    }

    /// The constructor's `spawnShyeed()`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        spawn_shyeed(ctx);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            EVENT_RESPAWN => {
                // D-54: the row decides, so it is read again first.
                ctx.reread_global_variables(&[RESPAWN_VARIABLE], EVENT_RESPAWN_ROW_READ, None, None);
            }
            EVENT_RESPAWN_ROW_READ => spawn_shyeed(ctx),
            EVENT_DESPAWN => {
                // Java: `npc.isDead()` on the NPC the timer was started with.
                let npc = npc?;
                if !ctx.is_dead(npc.object_id) {
                    start_respawn(ctx);
                    ctx.delete_npc(npc);
                }
            }
            _ => {}
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.broadcast_npc_say(
            npc,
            ChatType::NpcGeneral,
            NpcStringId::ShyeedSCryIsSteadilyDyingDown,
            &[],
        );
        start_respawn(ctx);
        ctx.world_mut().set_effect_zone_enabled(PC_BUFF_ZONE, true);
    }
}

fn spawn_shyeed(ctx: &mut ScriptCtx<'_>) {
    let respawn = ctx.global_variables().get_long(RESPAWN_VARIABLE, 0);
    let remain = if respawn != 0 { respawn - ctx.now_ms() } else { 0 };
    if remain > 0 {
        ctx.start_quest_timer(EVENT_RESPAWN, remain, None, None, false);
        return;
    }

    let (x, y, z, heading) = SHYEED_LOC;
    let npc = ctx.add_spawn(SHYEED, x, y, z, heading, false, 0, 0);
    // Java: `startQuestTimer(.., npc, ..)` with a null NPC when the spawn failed.
    ctx.start_quest_timer(EVENT_DESPAWN, 10_800_000, npc, None, false);
    ctx.world_mut().set_effect_zone_enabled(PC_BUFF_ZONE, false);

    // MOB_BUFF_ZONE.setEnabled(true);
    ctx.world_mut()
        .set_effect_zone_enabled(MOB_BUFF_DISPLAY_ZONE, true);
}

fn start_respawn(ctx: &mut ScriptCtx<'_>) {
    let respawn_time = RESPAWN - i64::from(ctx.get_random(RANDOM_RESPAWN));
    let at = ctx.now_ms() + respawn_time;
    ctx.set_global_variable(RESPAWN_VARIABLE, &at.to_string());
    ctx.start_quest_timer(EVENT_RESPAWN, respawn_time, None, None, false);

    // MOB_BUFF_ZONE.setEnabled(false);
    ctx.world_mut()
        .set_effect_zone_enabled(MOB_BUFF_DISPLAY_ZONE, false);
}

#[cfg(test)]
mod tests {
    use l2_world::{ScriptRegistry, ScriptSettings, WorldCommand};

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25671" type="RaidBoss" level="80" name="Queen Shyeed"><stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision></npc>"#;

    /// The three Stakato Nest buff zones (`data/zones/effect.xml`), all off by default.
    const ZONES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="22_16_stakato_mob_buff" id="200103" type="EffectZone" shape="NPoly" minZ="-6368" maxZ="-3296">
		<stat name="skillIdLvl" val="6170-1;" />
		<stat name="default_enabled" val="false" />
		<node X="73945" Y="-57263" />
		<node X="90046" Y="-57259" />
		<node X="89999" Y="-41698" />
		<node X="73982" Y="-41655" />
	</zone>
	<zone name="22_16_stakato_mob_buff_display" id="200104" type="EffectZone" shape="NPoly" minZ="-6368" maxZ="-3296">
		<stat name="skillIdLvl" val="6169-1;" />
		<stat name="default_enabled" val="false" />
		<node X="73945" Y="-57263" />
		<node X="90046" Y="-57259" />
		<node X="89999" Y="-41698" />
		<node X="73982" Y="-41655" />
	</zone>
	<zone name="22_16_stakato_pc_buff" id="200105" type="EffectZone" shape="NPoly" minZ="-6368" maxZ="-3296">
		<stat name="skillIdLvl" val="6171-1;" />
		<stat name="default_enabled" val="false" />
		<node X="73945" Y="-57263" />
		<node X="90046" Y="-57259" />
		<node X="89999" Y="-41698" />
		<node X="73982" Y="-41655" />
	</zone>
</list>
"#;

    fn world(rows: Vec<(&str, &str)>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(QueenShyeed);
        let rows: Vec<(String, Option<String>)> = rows
            .into_iter()
            .map(|(k, v)| (k.to_owned(), Some(v.to_owned())))
            .collect();
        ScriptTest::new_with(
            registry,
            NPCS,
            "",
            &[("data/zones/effect.xml", ZONES)],
            move |world, root| {
                world.set_zone_data(
                    std::sync::Arc::new(l2_data::Zones::load(root)),
                    std::sync::Arc::new(l2_data::MapRegions::load(root)),
                );
                world.set_script_settings(ScriptSettings {
                    cycle_database_writes: true,
                    ..ScriptSettings::default()
                });
                world.set_global_variables(rows);
            },
        )
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap())
            .unwrap()
    }

    fn row(t: &ScriptTest) -> i64 {
        t.world.global_variables().get_long(RESPAWN_VARIABLE, 0)
    }

    #[test]
    fn without_a_row_shyeed_spawns_at_start_and_the_buff_zones_are_set() {
        let t = world(vec![]);
        let shyeed = t.npcs_of(SHYEED);
        assert_eq!(shyeed.len(), 1);
        let at = t.world.position_of(shyeed[0]).unwrap();
        assert_eq!(at, (79634, -55428, -6104));
        // The PC buff goes off, the display zone on.
        assert!(!t.world.is_effect_zone_enabled(PC_BUFF_ZONE));
        assert!(t.world.is_effect_zone_enabled(MOB_BUFF_DISPLAY_ZONE));
        // The mob buff zone is never touched.
        assert!(!t.world.is_effect_zone_enabled(200_103));
        assert_eq!(row(&t), 0);
    }

    #[test]
    fn a_future_row_waits_for_the_respawn_timer() {
        let mut t = world(vec![(RESPAWN_VARIABLE, &(now_ms() + 3_600_000).to_string())]);
        assert!(t.npcs_of(SHYEED).is_empty());
        let mut waiting = false;
        t.world.run_as_script("QueenShyeed", |ctx| {
            waiting = ctx.has_quest_timer(EVENT_RESPAWN, None, None);
        });
        assert!(waiting);
    }

    #[test]
    fn a_past_row_spawns_shyeed() {
        let t = world(vec![(RESPAWN_VARIABLE, &(now_ms() - 1000).to_string())]);
        assert_eq!(t.npcs_of(SHYEED).len(), 1);
    }

    #[test]
    fn her_death_stores_the_next_respawn_and_opens_the_pc_buff() {
        let mut t = world(vec![]);
        t.enter(1, 80, vec![]);
        let shyeed = t.npcs_of(SHYEED)[0];
        let before = now_ms();
        t.kill(1, shyeed);
        let stored = row(&t);
        // 24 h minus up to 12 h.
        assert!(stored >= before + RESPAWN - i64::from(RANDOM_RESPAWN), "{stored}");
        assert!(stored <= now_ms() + RESPAWN, "{stored}");
        assert!(t.world.is_effect_zone_enabled(PC_BUFF_ZONE));
        assert!(!t.world.is_effect_zone_enabled(MOB_BUFF_DISPLAY_ZONE));
        let mut waiting = false;
        t.world.run_as_script("QueenShyeed", |ctx| {
            waiting = ctx.has_quest_timer(EVENT_RESPAWN, None, None);
        });
        assert!(waiting);
    }

    #[test]
    fn the_despawn_timer_removes_a_live_shyeed_and_starts_the_respawn() {
        let mut t = world(vec![]);
        let shyeed = t.npcs_of(SHYEED)[0];
        t.wait_ms(10_800_000 / 100);
        assert!(t.in_world(shyeed));
        t.world.run_as_script("QueenShyeed", |ctx| {
            let npc = ctx.script_npc(shyeed).unwrap();
            let _ = QueenShyeed.on_event(ctx, EVENT_DESPAWN, Some(npc), None);
        });
        assert!(!t.in_world(shyeed));
        assert!(row(&t) > now_ms());
        assert!(!t.world.is_effect_zone_enabled(MOB_BUFF_DISPLAY_ZONE));
    }

    #[test]
    fn the_respawn_event_reads_the_row_again_with_the_switch_off() {
        let mut registry = ScriptRegistry::new();
        registry.add(QueenShyeed);
        let mut t = ScriptTest::new_with(registry, NPCS, "", &[], |world, _| {
            world.set_script_settings(ScriptSettings {
                cycle_database_writes: false,
                ..ScriptSettings::default()
            });
        });
        // Started without a row: she is there. A respawn asks for the row first.
        let first = t.npcs_of(SHYEED)[0];
        t.world.run_as_script("QueenShyeed", |ctx| {
            ctx.notify_event(EVENT_RESPAWN, None, None);
        });
        assert_eq!(t.world.script_rereads_waiting(), 1);
        // The Java GS stored a respawn in the future: she does not come again.
        t.world.handle(WorldCommand::GlobalVariablesReread {
            token: 1,
            result: Ok(vec![(
                RESPAWN_VARIABLE.to_owned(),
                Some((now_ms() + 7_200_000).to_string()),
            )]),
        });
        assert_eq!(t.npcs_of(SHYEED), vec![first]);
    }
}
