//! The Zealots of Shilen and the guards that fight the monsters next to them.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/NPC/ZealotOfShilen/ZealotOfShilen.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ZEALOT: i32 = 18782;
const GUARDS: [i32; 2] = [32628, 32629];

/// Java `ai.areas.Gracia.AI.NPC.ZealotOfShilen.ZealotOfShilen`.
pub struct ZealotOfShilen;

impl ZealotOfShilen {
    /// Java `World.getNearestVisibleObject(npc, Monster.class, m -> !m.isDead() && !m.isDecayed())`.
    fn nearest_monster(ctx: &ScriptCtx<'_>, npc: ObjectId) -> Option<ObjectId> {
        let world = ctx.world();
        let (x, y, z) = world.position_of(npc)?;
        let mut nearest = None;
        let mut best_distance = f64::MAX;
        for wo in world.visible_objects(npc) {
            let Some(monster) = world.npc(wo).filter(|n| n.class.is_monster()) else {
                continue;
            };
            if world.is_dead(wo) || monster.decayed {
                continue;
            }
            let Some((ox, oy, oz)) = world.position_of(wo) else {
                continue;
            };
            let (dx, dy, dz) = (f64::from(ox - x), f64::from(oy - y), f64::from(oz - z));
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            if distance < best_distance {
                best_distance = distance;
                nearest = Some(wo);
            }
        }
        nearest
    }
}

impl Script for ZealotOfShilen {
    fn name(&self) -> &'static str {
        "ZealotOfShilen"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/NPC/ZealotOfShilen"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[ZEALOT]);
        s.add_spawn_id(&GUARDS);
        s.add_first_talk_id(&GUARDS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;

        if !ctx.world().is_attacking_now(npc.object_id) && !ctx.world().is_alike_dead(npc.object_id) {
            if let Some(nearby) = Self::nearest_monster(ctx, npc.object_id) {
                // `setRunning()`, `addDamageHate(nearby, 0, 999)`, `setIntentionAttack(nearby)`.
                ctx.add_attack_desire(npc, nearby, 999);
            }
        }

        // `super.onEvent(event, npc, player)`
        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        // NOTE(java-quirk): the page of the fighting guard is always "32628-01.html", for 32629 too.
        Some(if ctx.world().is_attacking_now(npc.object_id) {
            "32628-01.html".to_owned()
        } else {
            format!("{}.html", npc.id)
        })
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == ZEALOT {
            ctx.set_random_walking(npc, false);
        } else {
            ctx.world_mut().set_invul(npc.object_id, true);
            ctx.set_can_return_to_spawn_point(npc, false);
            ctx.cancel_quest_timer("WATCHING", Some(npc), None);
            ctx.start_quest_timer("WATCHING", 10000, Some(npc), None, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="18782" type="Monster" level="80" name="Zealot"/>
        <npc id="32628" type="Folk" level="80" name="Guard"/>
        <npc id="32629" type="Folk" level="80" name="Guard"/>
        <npc id="22000" type="Monster" level="80" name="Prey"/>"#;
    const DIR: &str = "data/scripts/ai/areas/Gracia/AI/NPC/ZealotOfShilen";

    #[test]
    fn a_guard_is_invulnerable_and_attacks_a_monster_in_sight_every_ten_seconds() {
        let mut registry = ScriptRegistry::new();
        registry.add(ZealotOfShilen);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let guard = t.spawn(32628, 20);
        t.ticks(1);
        assert!(t.world.is_invul(guard));
        assert!(!t.world.is_attacking_now(guard));
        // Nothing to fight: the timer finds no monster.
        t.ticks(210);
        assert!(!format!("{:?}", t.world.intention_of(guard)).contains("Attack"));

        let prey = t.spawn(22000, 60);
        t.ticks(210);
        assert!(format!("{:?}", t.world.intention_of(guard)).contains("Attack"));
        assert_eq!(t.world.attack_target_of(guard), Some(prey));
    }

    #[test]
    fn the_first_talk_page_is_the_id_of_the_guard() {
        let mut registry = ScriptRegistry::new();
        registry.add(ZealotOfShilen);
        let pages = [
            (
                format!("{DIR}/32628.html"),
                "<html><body>32628</body></html>".to_owned(),
            ),
            (
                format!("{DIR}/32629.html"),
                "<html><body>32629</body></html>".to_owned(),
            ),
        ];
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, "", &pages);
        let a = t.spawn(32628, 20);
        let b = t.spawn(32629, 25);
        t.enter(1, 80, vec![]);
        t.talk(1, a);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32628"));
        t.talk(1, b);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32629"));
    }
}
