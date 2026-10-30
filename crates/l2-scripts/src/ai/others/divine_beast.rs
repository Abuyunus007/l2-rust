//! The Divine Beast servitor: it stays only while its owner keeps the Divine Beast
//! transformation.
// Java: dist/game/data/scripts/ai/others/DivineBeast.java
//
// `addSummonSpawnId` starts a repeating check for the owner (S-API-05's `on_summon_spawn`); the
// check stops when the owner has no servitor and unsummons the beast once the owner's
// transformation is another one.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const DIVINE_BEAST: i32 = 14870;
const TRANSFORMATION_ID: i32 = 258;
const CHECK_TIME: i64 = 2 * 1000;

/// Java `ai.others.DivineBeast`.
pub struct DivineBeast;

impl Script for DivineBeast {
    fn name(&self) -> &'static str {
        "DivineBeast"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_summon_spawn_id(&[DIVINE_BEAST]);
    }

    fn on_summon_spawn(&self, ctx: &mut ScriptCtx<'_>, summon: ObjectId) {
        // `summon.asPlayer()`: the owner.
        let owner = ctx.world().summon_owner(summon);
        ctx.start_quest_timer("VALIDATE_TRANSFORMATION", CHECK_TIME, None, owner, true);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match player {
            Some(player) if ctx.world().has_servitor(player) => {
                if ctx.world().transformation_id(player) != TRANSFORMATION_ID {
                    ctx.cancel_quest_timer(event, npc, Some(player));
                    if let Some(summon) = ctx.world().summon_of(player) {
                        ctx.world_mut().unsummon(summon, Some(player));
                    }
                }
            }
            _ => ctx.cancel_quest_timer(event, npc, player),
        }
        // `super.onEvent(event, npc, player)`: `Script.onEvent` answers null.
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="14870" type="Servitor" level="80" name="Divine Beast"/>"#;

    #[test]
    fn the_beast_goes_once_the_owner_is_no_longer_transformed() {
        let mut registry = ScriptRegistry::new();
        registry.add(DivineBeast);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let beast = t.summon(1, DIVINE_BEAST);
        // The spawn event is queued; a script call delivers it.
        t.world.run_as_script("DivineBeast", |_| {});
        assert_eq!(t.world.quest_timer_count(), 1);
        assert_eq!(t.world.summon_of(Oid(1)), Some(beast));
        // The owner is not transformed (id 0): the next check unsummons the beast and stops.
        t.wait_ms(2_100);
        assert_eq!(t.world.summon_of(Oid(1)), None);
        assert_eq!(t.world.quest_timer_count(), 0);
    }

    #[test]
    fn the_check_stops_without_a_servitor() {
        let mut registry = ScriptRegistry::new();
        registry.add(DivineBeast);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t.world.run_as_script("DivineBeast", |ctx| {
            ctx.start_quest_timer("VALIDATE_TRANSFORMATION", CHECK_TIME, None, Some(Oid(1)), true);
        });
        t.wait_ms(2_100);
        assert_eq!(t.world.quest_timer_count(), 0);
    }
}
