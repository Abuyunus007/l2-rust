//! The Epidos (Ekimus' Seed of Infinity): minions follow the damage it takes, and an idle one is
//! deleted.
// Java: dist/game/data/scripts/ai/others/Epidos.java

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const EPIDOSES: [i32; 4] = [25609, 25610, 25611, 25612];

const MINIONS: [i32; 4] = [25605, 25606, 25607, 25608];

const MINIONS_COUNT: [i32; 3] = [3, 6, 11];

/// Java `Epidos._lastHp`.
#[derive(Default)]
struct LastHp(HashMap<ObjectId, f64>);

/// Java `ai.others.Epidos`.
pub struct Epidos;

impl Script for Epidos {
    fn name(&self) -> &'static str {
        "Epidos"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&EPIDOSES);
        s.add_spawn_id(&EPIDOSES);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        if event.eq_ignore_ascii_case("check_minions") {
            if ctx.get_random(1000) > 250 && ctx.state::<LastHp>().0.contains_key(&npc.object_id) {
                let last_hp = ctx.state::<LastHp>().0[&npc.object_id];
                let max_hp = f64::from(ctx.npc_max_hp(npc));
                let current_hp = ctx.npc_current_hp(npc);
                let hp_decrease_percent = (((last_hp - current_hp) * 100.0) / max_hp) as i32;
                let mut minions_count = 0;
                let spawned_minions = ctx.spawned_minion_count(npc);

                if hp_decrease_percent > 5 && hp_decrease_percent <= 15 && spawned_minions <= 9 {
                    minions_count = MINIONS_COUNT[0];
                } else if ((hp_decrease_percent > 1 && hp_decrease_percent <= 5)
                    || (hp_decrease_percent > 15 && hp_decrease_percent <= 30))
                    && spawned_minions <= 6
                {
                    minions_count = MINIONS_COUNT[1];
                } else if spawned_minions == 0 {
                    minions_count = MINIONS_COUNT[2];
                }

                // `Arrays.binarySearch(EPIDOSES, npc.getId())`
                if let Ok(index) = EPIDOSES.binary_search(&npc.id) {
                    for _ in 0..minions_count {
                        ctx.spawn_minion(npc, MINIONS[index]);
                    }
                }

                ctx.state::<LastHp>().0.insert(npc.object_id, current_hp);
            }

            ctx.start_quest_timer("check_minions", 10000, Some(npc), None, false);
        } else if event.eq_ignore_ascii_case("check_idle") {
            if ctx.is_intention_active(npc) {
                ctx.delete_npc(npc);
            } else {
                ctx.start_quest_timer("check_idle", 600000, Some(npc), None, false);
            }
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        if ctx.is_inside_radius_3d(npc, -45474, 247450, -13994, 2000) {
            ctx.add_spawn(32376, -45482, 246277, -14184, 0, false, 0, 0);
        }

        ctx.state::<LastHp>().0.remove(&npc.object_id);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("check_minions", 10000, Some(npc), None, false);
        ctx.start_quest_timer("check_idle", 600000, Some(npc), None, false);
        let max_hp = f64::from(ctx.npc_max_hp(npc));
        ctx.state::<LastHp>().0.insert(npc.object_id, max_hp);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25609" type="RaidBoss" level="80" name="Epidos"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25605" type="Monster" level="80" name="Epidos Minion"><stats><vitals hp="100" mp="10"/></stats></npc>
        <npc id="32376" type="Folk" level="80" name="Exit"/>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Epidos);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let epidos = t.spawn(25609, 100);
        t.enter(1, 80, vec![]);
        t.take();
        t.world.script_drain();
        (t, epidos)
    }

    fn minions(t: &ScriptTest) -> usize {
        t.world
            .npc_ids()
            .into_iter()
            .filter(|id| t.world.npc(*id).unwrap().npc_id == 25605)
            .count()
    }

    #[test]
    fn the_spawn_starts_both_checks() {
        let (t, _) = setup();
        assert_eq!(t.world.quest_timer_count(), 2);
    }

    #[test]
    fn an_untouched_epidos_calls_eleven_minions_once_a_roll_passes() {
        let (mut t, epidos) = setup();
        // `getRandom(1000) > 250` passes in three of four rolls; give it a few 10 s rounds.
        t.ticks(200 * 6);
        assert_eq!(minions(&t), 11);
        assert_eq!(t.world.spawned_minion_count(epidos), 11);
        // With eleven minions alive and no damage taken, nothing more comes.
        t.ticks(200 * 6);
        assert_eq!(minions(&t), 11);
    }

    #[test]
    fn damage_calls_three_minions_for_five_to_fifteen_percent() {
        let (mut t, epidos) = setup();
        t.world.npc_mut(epidos).unwrap().core.cur_hp = 900.0; // 10 % less than the 1000 it spawned with
        t.ticks(200 * 6);
        assert_eq!(minions(&t), 3, "10% damage and no minions: three");
    }

    #[test]
    fn the_kill_forgets_the_hp_and_far_epidos_leave_no_exit() {
        let (mut t, epidos) = setup();
        t.world.script_on_attackable_kill(epidos, Some(ObjectId(1)));
        t.ticks(60);
        // (Not inside the Seed of Infinity here: no exit NPC.)
        assert!(
            t.world
                .npc_ids()
                .into_iter()
                .all(|id| t.world.npc(id).unwrap().npc_id != 32376)
        );
        // No last HP any more: the checks still run but spawn nothing.
        t.ticks(200 * 6);
        assert_eq!(minions(&t), 0);
    }
}
