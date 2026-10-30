//! Etis van Etina's Phantom AI (Seven Signs SSQ2 boss decoy).
// Java: dist/game/data/scripts/ai/areas/MonasteryOfSilence/EtisVanEtinaPhantom.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const PHANTOM_REAL: i32 = 18950;
const PHANTOM_FAKE: i32 = 18951;
// Skills
const PETRIFY: SkillKey = SkillKey { id: 6735, level: 1 };
const DF_SELF_DESTRUCTION: SkillKey = SkillKey { id: 6708, level: 1 };
const SELF_DESTRUCTION_DR_ID: i32 = 6707;
const SELF_DESTRUCTION_DF_ID: i32 = 6708;
// Events
const EVT_PHANTOM_REAL_DEAD: &str = "ETIS_PHANTOM_REAL_DEAD";
const EVT_BOSS_DEAD: &str = "ETIS_BOSS_DEAD";
// Timers
const CAST_PETRIFY: &str = "CAST_PETRIFY";
const DELAYED_ATTACK: &str = "DELAYED_ATTACK";
const DF_THINK: &str = "DF_THINK";
// Misc
const BROADCAST_RADIUS: i32 = 5000;
const PETRIFY_DELAY: i64 = 1000;
const PETRIFY_DURATION: i64 = 5000;
const POST_PETRIFY_BUFFER: i64 = 500;
const DF_THINK_INTERVAL: i64 = 500;
const DF_BLAST_RANGE: i32 = 100;
const TARGET_SCAN_RANGE: i32 = 1500;

/// The first player in `range` of the NPC that is alive and visible (Java's
/// `forEachVisibleObjectInRange(npc, Player.class, range, ..)` keeping the first match).
fn first_player_in_range(ctx: &ScriptCtx<'_>, npc: ScriptNpc, range: i32) -> Option<ObjectId> {
    ctx.world()
        .visible_objects_in_range(npc.object_id, range)
        .into_iter()
        .find(|p| {
            ctx.world().player(*p).is_some() && !ctx.is_dead(*p) && !ctx.world().is_object_invisible(*p)
        })
}

/// Java `ai.areas.MonasteryOfSilence.EtisVanEtinaPhantom`.
pub struct EtisVanEtinaPhantom;

impl Script for EtisVanEtinaPhantom {
    fn name(&self) -> &'static str {
        "EtisVanEtinaPhantom"
    }

    fn path(&self) -> &'static str {
        "ai/areas/MonasteryOfSilence"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[PHANTOM_REAL, PHANTOM_FAKE]);
        s.add_kill_id(&[PHANTOM_REAL]);
        s.add_event_received_id(&[PHANTOM_REAL, PHANTOM_FAKE]);
        s.add_spell_finished_id(&[PHANTOM_REAL, PHANTOM_FAKE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        if ctx.is_dead(npc.object_id) {
            return None;
        }

        if event == CAST_PETRIFY {
            if !ctx.world().is_casting_now(npc.object_id) {
                ctx.world_mut().do_cast(npc.object_id, PETRIFY);
            }
        } else if event == DELAYED_ATTACK {
            let most_hated = ctx.most_hated(npc);
            if let Some(most_hated) = most_hated
                && !ctx.is_dead(most_hated)
            {
                ctx.set_running(npc);
                ctx.set_intention_attack(npc, most_hated);
            }
        } else if event == DF_THINK && npc.id == PHANTOM_FAKE {
            let current_target = ctx.target_of(npc.object_id);
            let nearest_player = if let Some(target) = current_target
                && ctx.world().player(target).is_some()
                && !ctx.is_dead(target)
            {
                Some(target)
            } else {
                let found = first_player_in_range(ctx, npc, DF_BLAST_RANGE);
                if let Some(found) = found {
                    ctx.world_mut().set_npc_target(npc.object_id, Some(found));
                }
                found
            };

            if let Some(nearest) = nearest_player
                && ctx
                    .distance_3d(npc.object_id, nearest)
                    .is_some_and(|d| d <= f64::from(DF_BLAST_RANGE))
                && !ctx.world().is_casting_now(npc.object_id)
            {
                ctx.world_mut().do_cast(npc.object_id, DF_SELF_DESTRUCTION);
            }

            ctx.start_quest_timer(DF_THINK, DF_THINK_INTERVAL, Some(npc), None, false);
        }

        None
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if ctx.is_dead(npc.object_id) {
            return;
        }

        let skill_id = skill.id;
        if (skill_id == SELF_DESTRUCTION_DR_ID) || (skill_id == SELF_DESTRUCTION_DF_ID) {
            ctx.world_mut().do_die(npc.object_id, None);
        }
    }

    fn on_event_received(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_name: &str,
        _sender: ScriptNpc,
        receiver: ScriptNpc,
        _reference: Option<ObjectId>,
    ) -> Option<String> {
        if ctx.is_dead(receiver.object_id) {
            return None;
        }

        // Java has two `if` branches with the same body: the boss died (every phantom goes),
        // or the real phantom died (the fake one goes).
        if event_name == EVT_BOSS_DEAD || (event_name == EVT_PHANTOM_REAL_DEAD && receiver.id == PHANTOM_FAKE)
        {
            ctx.delete_npc(receiver);
        }

        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let mut target = None;
        let target_id = ctx
            .world_mut()
            .npc_variables_mut(npc.object_id)
            .get_int("etisTargetId", 0);
        if target_id > 0 {
            let p = ObjectId(target_id);
            if ctx.world().player(p).is_some() && !ctx.is_dead(p) {
                target = Some(p);
            }
        }

        if target.is_none() {
            target = first_player_in_range(ctx, npc, TARGET_SCAN_RANGE);
        }

        if let Some(target) = target {
            ctx.world_mut().add_damage_hate(npc.object_id, target, 0, 999);
            ctx.world_mut().set_npc_target(npc.object_id, Some(target));
        }

        ctx.start_quest_timer(CAST_PETRIFY, PETRIFY_DELAY, Some(npc), None, false);
        ctx.start_quest_timer(
            DELAYED_ATTACK,
            PETRIFY_DELAY + PETRIFY_DURATION + POST_PETRIFY_BUFFER,
            Some(npc),
            None,
            false,
        );
        if npc.id == PHANTOM_FAKE {
            ctx.start_quest_timer(
                DF_THINK,
                PETRIFY_DELAY + PETRIFY_DURATION + POST_PETRIFY_BUFFER + DF_THINK_INTERVAL,
                Some(npc),
                None,
                false,
            );
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        if npc.id == PHANTOM_REAL {
            ctx.world_mut()
                .broadcast_event(npc.object_id, EVT_PHANTOM_REAL_DEAD, BROADCAST_RADIUS, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="18950" type="Monster" level="80" name="Etis van Etina Phantom"/>
        <npc id="18951" type="Monster" level="80" name="Etis van Etina Phantom"/>"#;

    #[test]
    fn a_phantom_targets_its_marked_player_and_starts_its_timers() {
        let mut registry = ScriptRegistry::new();
        registry.add(EtisVanEtinaPhantom);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let fake = t.spawn(PHANTOM_FAKE, 20);
        t.ticks(2);
        t.world.run_as_script("EtisVanEtinaPhantom", |ctx| {
            let npc = ctx.script_npc(fake).unwrap();
            assert!(ctx.has_quest_timer(CAST_PETRIFY, Some(npc), None));
            assert!(ctx.has_quest_timer(DELAYED_ATTACK, Some(npc), None));
            assert!(ctx.has_quest_timer(DF_THINK, Some(npc), None));
        });
    }

    #[test]
    fn the_boss_death_event_removes_the_phantoms_and_the_real_one_s_death_the_fake() {
        let mut registry = ScriptRegistry::new();
        registry.add(EtisVanEtinaPhantom);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let real = t.spawn(PHANTOM_REAL, 20);
        let fake = t.spawn(PHANTOM_FAKE, 30);
        t.world.run_as_script("EtisVanEtinaPhantom", |ctx| {
            let (r, f) = (ctx.script_npc(real).unwrap(), ctx.script_npc(fake).unwrap());
            EtisVanEtinaPhantom.on_event_received(ctx, EVT_PHANTOM_REAL_DEAD, r, f, None);
        });
        assert!(!t.in_world(fake));
        assert!(t.in_world(real));
        t.world.run_as_script("EtisVanEtinaPhantom", |ctx| {
            let r = ctx.script_npc(real).unwrap();
            EtisVanEtinaPhantom.on_event_received(ctx, EVT_BOSS_DEAD, r, r, None);
        });
        assert!(!t.in_world(real));
    }
}
