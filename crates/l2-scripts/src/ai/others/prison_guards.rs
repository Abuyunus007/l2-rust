//! The Prison Guards of the Race Event: a stamp for those with the event timer, stone for the rest.
// Java: dist/game/data/scripts/ai/others/PrisonGuards.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const GUARD_HEAD: i32 = 18367; // Prison Guard
const GUARD: i32 = 18368; // Prison Guard

// Item
const STAMP: i32 = 10013; // Race Stamp

// Skills
const TIMER: i32 = 5239; // Event Timer
const STONE: SkillKey = SkillKey { id: 4578, level: 1 }; // Petrification
const SILENCE: SkillKey = SkillKey { id: 4098, level: 9 }; // Silence

/// Java `ai.others.PrisonGuards`.
pub struct PrisonGuards;

impl Script for PrisonGuards {
    fn name(&self) -> &'static str {
        "PrisonGuards"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[GUARD_HEAD, GUARD]);
        s.add_spawn_id(&[GUARD_HEAD, GUARD]);
        s.add_npc_hate_id(&[GUARD]);
        s.add_skill_see_id(&[GUARD]);
        s.add_spell_finished_id(&[GUARD_HEAD, GUARD]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "CLEAR_STATUS" {
            let npc = npc?;
            ctx.set_script_value(npc, 0);
        } else if event == "CHECK_HOME" {
            let npc = npc?;
            if ctx.npc_distance_from_spawn(npc) > 10.0
                && !ctx.is_in_combat(npc.object_id)
                && !ctx.is_dead(npc.object_id)
            {
                if let Some((x, y, z)) = ctx.npc_spawn_point(npc) {
                    ctx.tele_npc_to(npc, x, y, z);
                }
            }

            ctx.start_quest_timer("CHECK_HOME", 30000, Some(npc), None, false);
        }

        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if npc.id == GUARD_HEAD {
            if ctx.is_affected_by_skill(player, TIMER) {
                if ctx.get_random(100) < 10
                    && ctx.distance_3d(npc.object_id, player).is_some_and(|d| d < 100.0)
                    && ctx.get_quest_items_count(player, STAMP) <= 3
                    && ctx.is_script_value(npc, 0)
                {
                    ctx.give_items(player, STAMP, 1);
                    ctx.set_script_value(npc, 1);
                    ctx.start_quest_timer("CLEAR_STATUS", 600000, Some(npc), None, false);
                }
            } else {
                ctx.cast_skill(npc, player, STONE);
                ctx.broadcast_npc_say(npc, ChatType::General, NpcStringId::ItSNotEasyToObtain, &[]);
            }
        } else if !ctx.is_affected_by_skill(player, TIMER) && ctx.npc_distance_from_spawn(npc) < 2000.0 {
            ctx.cast_skill(npc, player, STONE);
            ctx.broadcast_npc_say(
                npc,
                ChatType::General,
                NpcStringId::YouReOutOfYourMindComingHere,
                &[],
            );
        }
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        _skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if !ctx.is_affected_by_skill(caster, TIMER) {
            ctx.cast_skill(npc, caster, SILENCE);
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if skill == SILENCE || skill == STONE {
            ctx.world_mut().clear_aggro_list(npc.object_id);
            ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
        }
    }

    fn on_npc_hate(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _mob: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) -> bool {
        ctx.is_affected_by_skill(player, TIMER)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == GUARD_HEAD {
            ctx.set_immobilized(npc, true);
            ctx.world_mut().set_invul(npc.object_id, true);
        } else {
            ctx.set_random_walking(npc, false);
            ctx.cancel_quest_timer("CHECK_HOME", Some(npc), None);
            ctx.start_quest_timer("CHECK_HOME", 30000, Some(npc), None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="18367" type="Monster" level="50" name="Prison Guard"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="18368" type="Monster" level="50" name="Prison Guard"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(PrisonGuards);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let head = t.spawn(GUARD_HEAD, 20);
        let guard = t.spawn(GUARD, 40);
        t.enter(1, 60, vec![]);
        t.world.script_drain();
        t.take();
        (t, head, guard)
    }

    #[test]
    fn the_head_is_invulnerable_and_the_guard_checks_home() {
        let (t, head, guard) = setup();
        assert!(t.world.is_invul(head));
        assert!(!t.world.is_invul(guard));
        assert!(t.world.npc_random_walking_forbidden(guard));
        assert_eq!(t.world.quest_timer_count(), 1);
    }

    #[test]
    fn a_player_without_the_timer_is_cursed_with_stone_and_a_line() {
        let (mut t, head, _) = setup();
        t.world.script_on_attackable_attack(head, ObjectId(1), 10, None);
        t.world.script_drain();
        // The engine's skill table is empty here: the cast does nothing, the guard still says it.
        assert!(
            crate::testing::names_to(&t.take(), 1)
                .iter()
                .any(|n| n == "NPC_SAY")
        );
        assert!(t.world.creature_target(head) == Some(ObjectId(1)));
    }

    #[test]
    fn nobody_without_the_timer_is_hated_by_a_guard() {
        let (mut t, _, guard) = setup();
        let mut hated = true;
        t.world.run_as_script("PrisonGuards", |ctx| {
            let npc = ctx.script_npc(guard).unwrap();
            hated = PrisonGuards.on_npc_hate(ctx, npc, ObjectId(1), false);
        });
        assert!(!hated);
    }

    #[test]
    fn the_status_is_cleared_by_its_event() {
        let (mut t, head, _) = setup();
        t.world.run_as_script("PrisonGuards", |ctx| {
            let npc = ctx.script_npc(head).unwrap();
            ctx.set_script_value(npc, 1);
            PrisonGuards.on_event(ctx, "CLEAR_STATUS", Some(npc), None);
            assert!(ctx.is_script_value(npc, 0));
        });
    }
}
