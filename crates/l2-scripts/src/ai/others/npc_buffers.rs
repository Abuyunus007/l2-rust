//! NPC Buffers: summoned NPCs (totems, cakes, decoys) that apply a skill to the summoner's party
//! or to the friends or enemies around them, again and again.
// Java: dist/game/data/scripts/ai/others/NpcBuffers/{NpcBuffers,NpcBufferAI,NpcBufferSkillData}.java
//
// Java's `NpcBufferAI` is a `Runnable` that schedules itself; here each skill of an NPC is a quest
// timer `NPC_BUFFER_<index>` of that NPC that starts itself again. A run that finds the NPC gone
// or without a player as summoner stops, as in Java.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use super::npc_buffers_data::{AffectObject, AffectScope, BufferSkill, NPC_BUFFERS};

const TIMER_PREFIX: &str = "NPC_BUFFER_";

fn buffer_skills(npc_id: i32) -> &'static [BufferSkill] {
    NPC_BUFFERS
        .iter()
        .find(|(id, _)| *id == npc_id)
        .map_or(&[], |(_, skills)| skills)
}

/// The player a playable stands for: the player itself, or the owner of a summon.
fn playable_player(ctx: &ScriptCtx<'_>, creature: ObjectId) -> Option<ObjectId> {
    if ctx.world().player(creature).is_some() {
        Some(creature)
    } else {
        ctx.world().summon_owner(creature)
    }
}

/// Java `NpcBufferAI.isFriendly(player, target)`.
fn is_friendly(ctx: &ScriptCtx<'_>, player: ObjectId, target: ObjectId) -> bool {
    let Some(target_player) = playable_player(ctx, target) else {
        return false;
    };
    let w = ctx.world();
    player == target_player
        || w.is_in_party_with(player, target_player)
        || w.is_in_command_channel_with(player, target_player)
        || w.is_in_clan_with(player, target_player)
        || w.is_in_ally_with(player, target_player)
        || w.is_on_same_siege_side_with(player, target_player)
}

/// Java `NpcBufferAI.isEnemy(player, target)`.
fn is_enemy(ctx: &ScriptCtx<'_>, player: ObjectId, target: ObjectId) -> bool {
    if is_friendly(ctx, player, target) {
        return false;
    }

    // `target instanceof TamedBeast`: the beast stands for its owner.
    if ctx.world().is_tamed_beast(target) {
        // NOTE(java-quirk): a beast without an owner makes Java's `isFriendly(player, null)`
        // throw; here it is no enemy.
        return ctx
            .world()
            .tamed_beast_owner(target)
            .is_some_and(|owner| is_enemy(ctx, player, owner));
    }

    if ctx.world().is_monster_npc(target) {
        return true;
    }

    if let Some(target_player) = playable_player(ctx, target) {
        let w = ctx.world();
        if !is_friendly(ctx, player, target_player) {
            if w.pvp_flag_of(target_player) != 0 {
                return true;
            }

            if w.script_player_karma(target_player) != 0 {
                return true;
            }

            if w.is_at_war_with(player, target_player) {
                return true;
            }

            if ctx.is_inside_pvp_zone(target_player) {
                return true;
            }
        }
    }

    false
}

/// Java `NpcBufferAI.run()` for the skill `index` of the NPC.
fn run(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, index: usize) {
    let Some(&(skill_id, skill_level, delay, scope, object)) = buffer_skills(npc.id).get(index) else {
        return;
    };
    let alive = ctx.world().npc(npc.object_id).is_some_and(|n| !n.decayed) && !ctx.is_dead(npc.object_id);
    if !alive {
        return;
    }

    let Some(player) = ctx.summoner(npc).filter(|s| ctx.world().player(*s).is_some()) else {
        return;
    };

    let skill = SkillKey {
        id: skill_id,
        level: skill_level,
    };
    let range = ctx.world().script_skill_affect_range(skill);
    match scope {
        AffectScope::Party => {
            if ctx.is_in_party(player) {
                for member in ctx.world().party_members_of(player) {
                    if ctx.check_if_in_range(range, npc.object_id, member, true) && !ctx.is_dead(member) {
                        ctx.world_mut().apply_effects(player, member, skill);
                    }
                }
            } else if ctx.check_if_in_range(range, npc.object_id, player, true) && !ctx.is_dead(player) {
                ctx.world_mut().apply_effects(player, player, skill);
            }
        }
        AffectScope::Range => {
            for target in ctx
                .world()
                .script_visible_creatures_in_range(npc.object_id, range)
            {
                match object {
                    AffectObject::Friend => {
                        if is_friendly(ctx, player, target) && !ctx.is_dead(target) {
                            ctx.world_mut().apply_effects(target, target, skill);
                        }
                    }
                    AffectObject::NotFriend => {
                        if is_enemy(ctx, player, target) && !ctx.is_dead(target) {
                            // Update PvP status
                            if playable_player(ctx, target).is_some() {
                                ctx.world_mut().script_update_pvp_status(player, target);
                            }
                            ctx.world_mut().apply_effects(target, target, skill);
                        }
                    }
                }
            }
        }
    }

    let name = format!("{TIMER_PREFIX}{index}");
    ctx.start_quest_timer(&name, i64::from(delay) * 1000, Some(npc), None, false);
}

/// Java `ai.others.NpcBuffers.NpcBuffers`.
pub struct NpcBuffers;

impl Script for NpcBuffers {
    fn name(&self) -> &'static str {
        "NpcBuffers"
    }

    fn path(&self) -> &'static str {
        "ai/others/NpcBuffers"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let ids: Vec<i32> = NPC_BUFFERS.iter().map(|(id, _)| *id).collect();
        s.add_first_talk_id(&ids);
        s.add_spawn_id(&ids);
    }

    // TODO (Java): cleanup once the NPC rework is finished and the default html is configurable.
    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        None
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        for index in 0..buffer_skills(npc.id).len() {
            // The initial delay is always 0 (see `npc_buffers_data`); the timer runs the AI at once.
            let name = format!("{TIMER_PREFIX}{index}");
            ctx.start_quest_timer(&name, 1, Some(npc), None, false);
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let (Some(index), Some(npc)) = (event.strip_prefix(TIMER_PREFIX), npc)
            && let Ok(index) = index.parse::<usize>()
        {
            run(ctx, npc, index);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    // A totem that buffs friends around (skill 23308 "Totem's Energy - Body", delay 5 s) and a
    // monster (not a friend of anybody).
    const NPCS: &str = r#"
        <npc id="143" type="Folk" level="70" name="Totem of Body"/>
        <npc id="13019" type="Folk" level="70" name="Anti-Music"/>
        <npc id="20001" type="Monster" level="70" name="Monster"/>"#;

    #[test]
    fn every_listed_buffer_has_its_skills_in_the_table() {
        assert_eq!(NPC_BUFFERS.len(), 33);
        assert_eq!(
            buffer_skills(143),
            &[(23308, 1, 5, AffectScope::Range, AffectObject::Friend)]
        );
        assert_eq!(
            buffer_skills(106),
            &[(22250, 1, 60, AffectScope::Party, AffectObject::Friend)]
        );
        assert_eq!(buffer_skills(13257).len(), 1);
        assert!(buffer_skills(1).is_empty());
    }

    #[test]
    fn a_buffer_without_a_player_summoner_does_not_run_again() {
        let mut registry = ScriptRegistry::new();
        registry.add(NpcBuffers);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let totem = t.spawn(143, 20);
        t.ticks(5);
        // Spawned with no summoner: the first run stops and no timer is left.
        t.world.run_as_script("NpcBuffers", |ctx| {
            let npc = ctx.script_npc(totem).unwrap();
            assert!(!ctx.has_quest_timer("NPC_BUFFER_0", Some(npc), None));
        });
    }

    #[test]
    fn a_buffer_with_a_player_summoner_runs_every_delay() {
        let mut registry = ScriptRegistry::new();
        registry.add(NpcBuffers);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 70, vec![]);
        let mut totem = None;
        t.world.run_as_script("NpcBuffers", |ctx| {
            let owner = ctx.script_npc(ObjectId(1));
            assert!(owner.is_none());
            totem = ctx.add_spawn(
                143,
                crate::testing::X + 20,
                crate::testing::Y,
                crate::testing::Z,
                0,
                false,
                0,
                0,
            );
        });
        let totem = totem.unwrap();
        // The summoner is a player: the run keeps its timer (5 s) on the wheel.
        t.world.run_as_script("NpcBuffers", |ctx| {
            ctx.world_mut().add_summoned_npc(ObjectId(1), totem.object_id);
            let _ = NpcBuffers.on_event(ctx, "NPC_BUFFER_0", Some(totem), None);
            assert!(ctx.has_quest_timer("NPC_BUFFER_0", Some(totem), None));
        });
        t.wait_ms(5_100);
        t.world.run_as_script("NpcBuffers", |ctx| {
            assert!(ctx.has_quest_timer("NPC_BUFFER_0", Some(totem), None));
        });
    }
}
