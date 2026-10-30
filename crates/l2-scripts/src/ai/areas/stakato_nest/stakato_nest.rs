//! The Stakato Nest monsters: nurses and females that change, a leader that devours its minion,
//! and the cocoons that become a Cannibalistic Stakato Chief.
// Java: dist/game/data/scripts/ai/areas/StakatoNest/StakatoNest.java

use l2_util::ObjectId;
use l2_world::movement::heading_from_points;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// Cocoons
const COCOONS: [i32; 6] = [18793, 18794, 18795, 18796, 18797, 18798];
// Cannibalistic Stakato Leader
const STAKATO_LEADER: i32 = 22625;
// Spike Stakato Nurse
const STAKATO_NURSE: i32 = 22630;
// Spike Stakato Nurse (Changed)
const STAKATO_NURSE_2: i32 = 22631;
// Spiked Stakato Baby
const STAKATO_BABY: i32 = 22632;
// Spiked Stakato Captain
const STAKATO_CAPTAIN: i32 = 22629;
// Female Spiked Stakato
const STAKATO_FEMALE: i32 = 22620;
// Male Spiked Stakato
const STAKATO_MALE: i32 = 22621;
// Male Spiked Stakato (Changed)
const STAKATO_MALE_2: i32 = 22622;
// Spiked Stakato Guard
const STAKATO_GUARD: i32 = 22619;
// Cannibalistic Stakato Chief
const STAKATO_CHIEF: i32 = 25667;
// Growth Accelerator
const GROWTH_ACCELERATOR: i32 = 2905;
// Small Stakato Cocoon
const SMALL_COCOON: i32 = 14833;
// Large Stakato Cocoon
const LARGE_COCOON: i32 = 14834;
// Subordinate skill
const DEVOUR_SUBORDINATE: SkillKey = SkillKey { id: 4484, level: 1 };

/// Java `ai.areas.StakatoNest.StakatoNest`.
pub struct StakatoNest;

/// Java `checkMinion(npc)`: the first spawned minion that is not dead.
fn check_minion(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> Option<ScriptNpc> {
    ctx.world()
        .minions_of(npc.object_id)
        .iter()
        .copied()
        .find(|m| !ctx.is_dead(*m))
        .and_then(|m| ctx.script_npc(m))
}

/// Java `player.addItem(REWARD, small or large cocoon, 1, npc, true)`.
fn give_cocoon(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let item = if ctx.get_random(100) > 80 {
        LARGE_COCOON
    } else {
        SMALL_COCOON
    };
    ctx.world_mut().add_item(player, item, 1, true);
}

impl Script for StakatoNest {
    fn name(&self) -> &'static str {
        "StakatoNest"
    }

    fn path(&self) -> &'static str {
        "ai/areas/StakatoNest"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[STAKATO_LEADER]);
        s.add_skill_see_id(&COCOONS);
        s.add_kill_id(&[
            STAKATO_NURSE,
            STAKATO_BABY,
            STAKATO_MALE,
            STAKATO_FEMALE,
            STAKATO_CHIEF,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let (Some(npc), Some(player)) = (npc, player) else {
            return None;
        };
        if ctx.is_dead(npc.object_id) {
            return None;
        }
        let npc_id = match event {
            "nurse_change" => STAKATO_NURSE_2,
            "male_change" => STAKATO_MALE_2,
            _ => 0,
        };
        if npc_id > 0 {
            ctx.world_mut().script_spawn_decrease_count(npc.object_id);
            if let Some((x, y, z, heading)) = ctx.npc_position(npc) {
                if let Some(spawned) = ctx.add_spawn(npc_id, x, y, z, heading, false, 0, 0) {
                    ctx.add_attack_desire(spawned, player, 999);
                }
            }
            ctx.delete_npc(npc);
        }
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if ctx.get_random(100) < 10 && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.3 {
            if let Some(follower) = check_minion(ctx, npc) {
                let hp = ctx.npc_current_hp(follower);
                if hp > f64::from(ctx.npc_max_hp(follower)) * 0.3 {
                    ctx.world_mut().abort_attack(npc.object_id);
                    ctx.world_mut().abort_cast(npc.object_id);
                    if let (Some(a), Some(b)) = (
                        ctx.world().position_of(npc.object_id),
                        ctx.world().position_of(follower.object_id),
                    ) {
                        let heading = heading_from_points(a.0, a.1, b.0, b.1);
                        ctx.world_mut().script_set_heading(npc.object_id, heading);
                    }
                    ctx.world_mut().do_cast(npc.object_id, DEVOUR_SUBORDINATE);
                    let new_hp = ctx.npc_current_hp(npc) + hp;
                    ctx.world_mut().set_current_hp(npc.object_id, new_hp, true);
                    ctx.world_mut()
                        .do_die(follower.object_id, Some(follower.object_id));
                    ctx.delete_npc(follower);
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            STAKATO_NURSE | STAKATO_MALE => {
                let spawn_id = if npc.id == STAKATO_NURSE {
                    STAKATO_CAPTAIN
                } else {
                    STAKATO_GUARD
                };
                if let Some(monster) = check_minion(ctx, npc) {
                    ctx.broadcast_magic_skill_use(
                        npc.object_id,
                        npc.object_id,
                        npc.object_id,
                        2046,
                        1,
                        1000,
                        0,
                    );
                    for _ in 0..3 {
                        if let Some(spawned) = ctx.add_spawn_at(spawn_id, monster.object_id, true, 0) {
                            ctx.add_attack_desire(spawned, killer, 999);
                        }
                    }
                }
            }
            STAKATO_BABY | STAKATO_FEMALE => {
                let event = if npc.id == STAKATO_BABY {
                    "nurse_change"
                } else {
                    "male_change"
                };
                if let Some(monster) = ctx
                    .world()
                    .npc_leader(npc.object_id)
                    .filter(|m| !ctx.is_dead(*m))
                    .and_then(|m| ctx.script_npc(m))
                {
                    ctx.start_quest_timer(event, 5000, Some(monster), Some(killer), false);
                }
            }
            STAKATO_CHIEF => {
                if ctx.is_in_party(killer) {
                    for member in ctx.world().party_members_of(killer) {
                        give_cocoon(ctx, member);
                    }
                } else {
                    give_cocoon(ctx, killer);
                }
            }
            _ => {}
        }
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
        if targets.contains(&npc.object_id) && skill.id == GROWTH_ACCELERATOR {
            ctx.world_mut().do_die(npc.object_id, Some(caster));
            let (Some(at), Some(from)) = (
                ctx.world().position_of(npc.object_id),
                ctx.world().position_of(caster),
            ) else {
                return;
            };
            let heading = heading_from_points(at.0, at.1, from.0, from.1);
            if let Some(spawned) = ctx.add_spawn(STAKATO_CHIEF, at.0, at.1, at.2, heading, false, 0, 0) {
                ctx.add_attack_desire(spawned, caster, 999);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="18793" type="Monster" level="80" name="Cocoon"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="25667" type="Monster" level="80" name="Chief"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22630" type="Monster" level="80" name="Nurse">
          <parameters><minions name="Privates"><npc id="22632" count="1" respawnTime="60"/></minions></parameters>
          <stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22632" type="Monster" level="80" name="Baby"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22631" type="Monster" level="80" name="Nurse 2"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22629" type="Monster" level="80" name="Captain"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;
    const ITEMS: &str = r#"<item id="14833" type="EtcItem" name="Small Cocoon"/>
        <item id="14834" type="EtcItem" name="Large Cocoon"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(StakatoNest);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.enter(1, 80, vec![]);
        t
    }

    /// The nurse's private (the datapack spawn brings it; a script-made spawn does not).
    fn add_baby(t: &mut ScriptTest, nurse: ObjectId) {
        t.world.run_as_script("StakatoNest", |ctx| {
            let master = ctx.script_npc(nurse).unwrap();
            ctx.spawn_minion(master, STAKATO_BABY);
        });
    }

    #[test]
    fn growth_accelerator_on_a_cocoon_makes_a_chief_whose_death_pays_a_cocoon() {
        let mut t = setup();
        let cocoon = t.spawn(18793, 300);
        let npc = ScriptNpc {
            object_id: cocoon,
            id: 18793,
        };
        // Another skill is ignored.
        t.world.run_as_script("StakatoNest", |ctx| {
            StakatoNest.on_skill_see(ctx, npc, ObjectId(1), SkillKey::new(1, 1), &[cocoon], false);
        });
        assert!(t.npcs_of(STAKATO_CHIEF).is_empty());
        t.world.run_as_script("StakatoNest", |ctx| {
            StakatoNest.on_skill_see(
                ctx,
                npc,
                ObjectId(1),
                SkillKey::new(GROWTH_ACCELERATOR, 1),
                &[cocoon],
                false,
            );
        });
        let chief = t.npcs_of(STAKATO_CHIEF);
        assert_eq!(chief.len(), 1);
        t.kill(1, chief[0]);
        assert_eq!(t.count(1, SMALL_COCOON) + t.count(1, LARGE_COCOON), 1);
    }

    #[test]
    fn the_baby_s_death_turns_the_nurse_into_the_changed_one() {
        let mut t = setup();
        let nurse = t.spawn(STAKATO_NURSE, 300);
        add_baby(&mut t, nurse);
        let babies = t.npcs_of(STAKATO_BABY);
        assert_eq!(babies.len(), 1);
        t.kill(1, babies[0]);
        t.wait_ms(5_500);
        assert!(!t.in_world(nurse));
        let changed = t.npcs_of(STAKATO_NURSE_2);
        assert_eq!(changed.len(), 1);
        t.world.delete_npc(changed[0]);
    }

    #[test]
    fn the_nurse_s_death_calls_three_captains_when_a_minion_lives() {
        let mut t = setup();
        let nurse = t.spawn(STAKATO_NURSE, 300);
        add_baby(&mut t, nurse);
        t.kill(1, nurse);
        assert_eq!(t.npcs_of(STAKATO_CAPTAIN).len(), 3);
        for id in t.npcs_of(STAKATO_CAPTAIN) {
            t.world.delete_npc(id);
        }
    }
}
