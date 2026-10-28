//! Scarlet van Halisha, the two forms Frintezza's party fights: the skill rotation and the target
//! choice. (The movie and the morph are `LastImperialTomb`.)
// Java: dist/game/data/scripts/ai/bosses/Frintezza/ScarletVanHalisha.java
//
// The Java fields are `ScarletState` (`ctx.state`). Java registers the three hooks twice
// (`addAttackId` ... repeated); a second registration changes nothing.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const HALISHA2: i32 = 29046;
const HALISHA3: i32 = 29047;

// Skills
const FRINTEZZA_DAEMON_ATTACK: i32 = 5014;
const FRINTEZZA_DAEMON_CHARGE: i32 = 5015;
const YOKE_OF_SCARLET: i32 = 5016;
const FRINTEZZA_DAEMON_MORPH: i32 = 5018;
const FRINTEZZA_DAEMON_FIELD: i32 = 5019;

// Misc
const RANGED_SKILL_MIN_COOLTIME: i64 = 60000; // 1 minute

/// The fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct ScarletState {
    /// `_target`.
    pub(crate) target: Option<ObjectId>,
    /// `_skill`.
    pub(crate) skill: Option<SkillKey>,
    /// `_lastRangedSkillTime`.
    pub(crate) last_ranged_skill_time: i64,
}

/// Java `ai.bosses.Frintezza.ScarletVanHalisha`.
pub struct ScarletVanHalisha;

impl Script for ScarletVanHalisha {
    fn name(&self) -> &'static str {
        "ScarletVanHalisha"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Frintezza"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[HALISHA2, HALISHA3]);
        s.add_kill_id(&[HALISHA2, HALISHA3]);
        s.add_spell_finished_id(&[HALISHA2, HALISHA3]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "ATTACK" => {
                if let Some(npc) = npc {
                    get_skill_ai(ctx, npc);
                }
            }
            "RANDOM_TARGET" => {
                let target = npc.and_then(|npc| get_random_target(ctx, npc, None));
                ctx.state::<ScarletState>().target = target;
            }
            _ => {}
        }

        None
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        _skill: SkillKey,
    ) {
        get_skill_ai(ctx, npc);
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
        ctx.start_quest_timer("RANDOM_TARGET", 5000, Some(npc), None, true);
        ctx.start_quest_timer("ATTACK", 500, Some(npc), None, true);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.cancel_quest_timers("ATTACK");
        ctx.cancel_quest_timers("RANDOM_TARGET");
    }
}

fn get_rnd_skills(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) -> SkillKey {
    match npc.id {
        HALISHA2 => {
            if ctx.get_random(100) < 10 {
                SkillKey::new(FRINTEZZA_DAEMON_CHARGE, 2)
            } else if ctx.get_random(100) < 10 {
                SkillKey::new(FRINTEZZA_DAEMON_CHARGE, 5)
            } else if ctx.get_random(100) < 2 {
                SkillKey::new(YOKE_OF_SCARLET, 1)
            } else {
                SkillKey::new(FRINTEZZA_DAEMON_ATTACK, 2)
            }
        }
        HALISHA3 => {
            let last = ctx.state::<ScarletState>().last_ranged_skill_time;
            let ranged_ready = (last + RANGED_SKILL_MIN_COOLTIME) < ctx.now_ms();
            if ctx.get_random(100) < 10 {
                SkillKey::new(FRINTEZZA_DAEMON_CHARGE, 3)
            } else if ctx.get_random(100) < 10 {
                SkillKey::new(FRINTEZZA_DAEMON_CHARGE, 6)
            } else if ctx.get_random(100) < 10 {
                SkillKey::new(FRINTEZZA_DAEMON_CHARGE, 2)
            } else if ranged_ready && (ctx.get_random(100) < 10) {
                SkillKey::new(FRINTEZZA_DAEMON_FIELD, 1)
            } else if ranged_ready && (ctx.get_random(100) < 10) {
                SkillKey::new(FRINTEZZA_DAEMON_MORPH, 1)
            } else if ctx.get_random(100) < 2 {
                SkillKey::new(YOKE_OF_SCARLET, 1)
            } else {
                SkillKey::new(FRINTEZZA_DAEMON_ATTACK, 3)
            }
        }
        _ => SkillKey::new(FRINTEZZA_DAEMON_ATTACK, 1),
    }
}

fn get_skill_ai(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
    let id = npc.object_id;
    if ctx.world().is_invul(id) || ctx.world().is_casting_now(id) {
        return;
    }

    let target = ctx.state::<ScarletState>().target;
    if (ctx.get_random(100) < 30) || target.is_none_or(|t| ctx.world().is_dead(t)) {
        let skill = get_rnd_skills(ctx, npc);
        ctx.state::<ScarletState>().skill = Some(skill);
        let target = get_random_target(ctx, npc, Some(skill));
        ctx.state::<ScarletState>().target = target;
    }

    let skill = match ctx.state::<ScarletState>().skill {
        Some(skill) => skill,
        None => get_rnd_skills(ctx, npc),
    };

    if ctx.world().is_physical_muted(id) {
        return;
    }

    let Some(target) = ctx.state::<ScarletState>().target else {
        return;
    };
    if ctx.world().is_dead(target) {
        return;
    }

    // Java: a skill the table does not have is null and `getCastRange()` throws.
    let Some(range) = ctx.world().skill_cast_range(skill) else {
        return;
    };
    if ctx.check_if_in_range(range, id, target, true) {
        ctx.world_mut().npc_set_intention_idle(id);
        ctx.world_mut().set_npc_target(id, Some(target));
        ctx.state::<ScarletState>().target = None;
        ctx.world_mut().do_cast(id, skill);
    } else {
        ctx.world_mut().npc_set_intention_follow(id, target);
        ctx.world_mut().set_intention_attack(id, target);
    }
}

fn get_random_target(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, skill: Option<SkillKey>) -> Option<ObjectId> {
    let id = npc.object_id;
    let allowed = ctx
        .world()
        .instance_world_of(id)
        .map(|w| w.allowed.clone())
        .unwrap_or_default();
    let mut result = Vec::new();
    for obj in allowed {
        if ctx.world().player(obj).is_none() {
            continue;
        }
        if ctx.world().is_object_invisible(obj) {
            continue;
        }

        // NOTE(java-quirk): `(z < npcZ - 100) && (z > npcZ + 100)` is never true, so only the
        // line of sight can skip a player here.
        if !ctx.world().can_see_target(obj, id) {
            continue;
        }

        let mut skill_range = 150;
        // NOTE(java-quirk): the player is only added when a skill was given; `RANDOM_TARGET`
        // passes none, so it always ends with no target.
        if let Some(skill) = skill {
            match skill.id {
                FRINTEZZA_DAEMON_ATTACK => skill_range = 150,
                FRINTEZZA_DAEMON_CHARGE => skill_range = 400,
                YOKE_OF_SCARLET => skill_range = 200,
                FRINTEZZA_DAEMON_MORPH | FRINTEZZA_DAEMON_FIELD => {
                    let now = ctx.now_ms();
                    ctx.state::<ScarletState>().last_ranged_skill_time = now;
                    skill_range = 550;
                }
                _ => {}
            }

            if ctx.check_if_in_range(skill_range, id, obj, true) && !ctx.world().is_dead(obj) {
                result.push(obj);
            }
        }
    }

    ctx.get_random_entry(&result)
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{InstanceWorld, ScriptRegistry};

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{ScriptTest, player_data};

    const BODY: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision>"#;

    fn npcs() -> String {
        format!(
            r#"<npc id="29046" type="Monster" level="85" name="Scarlet van Halisha">{BODY}</npc>
<npc id="29047" type="Monster" level="85" name="Scarlet van Halisha">{BODY}</npc>"#
        )
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(ScarletVanHalisha);
        let mut t = ScriptTest::new(registry, &npcs(), "", &[]);
        t.world
            .set_instance_data(instance_data(&[(136, "Last Imperial Tomb")], &[(136, "")]));
        let instance_id = t.world.create_dynamic_instance(136);
        t.world.add_instance_world(InstanceWorld::new(instance_id, 136));
        t.world
            .instance_world_mut(instance_id)
            .unwrap()
            .add_allowed(Oid(1));
        t.enter_with(player_data(1, 80, vec![]));
        let (x, y, z) = t.world.position_of(Oid(1)).unwrap();
        let scarlet = t
            .world
            .script_add_spawn(HALISHA3, (x + 50, y, z), 0, false, 0, instance_id)
            .unwrap();
        (t, scarlet)
    }

    fn state<R>(t: &mut ScriptTest, f: impl FnOnce(&mut ScarletState) -> R) -> R {
        let mut result = None;
        t.world.run_as_script("ScarletVanHalisha", |ctx| {
            result = Some(f(ctx.state::<ScarletState>()))
        });
        result.unwrap()
    }

    #[test]
    fn the_first_hit_starts_the_two_timers_and_the_kill_stops_them() {
        let (mut t, scarlet) = setup();
        let mut found = (false, false);
        t.world.run_as_script("ScarletVanHalisha", |ctx| {
            let npc = ctx.script_npc(scarlet).unwrap();
            ScarletVanHalisha.on_attack(ctx, npc, Oid(1), 10, false, None);
            found = (
                ctx.has_quest_timer("RANDOM_TARGET", Some(npc), None),
                ctx.has_quest_timer("ATTACK", Some(npc), None),
            );
            ScarletVanHalisha.on_kill(ctx, npc, Oid(1), false);
            assert!(!ctx.has_quest_timer("RANDOM_TARGET", Some(npc), None));
            assert!(!ctx.has_quest_timer("ATTACK", Some(npc), None));
        });
        assert_eq!(found, (true, true));
    }

    #[test]
    fn the_random_target_event_never_finds_one_without_a_skill() {
        let (mut t, scarlet) = setup();
        state(&mut t, |s| s.target = Some(Oid(1)));
        t.world.run_as_script("ScarletVanHalisha", |ctx| {
            let npc = ctx.script_npc(scarlet);
            ScarletVanHalisha.on_event(ctx, "RANDOM_TARGET", npc, None);
        });
        assert_eq!(state(&mut t, |s| s.target), None);
    }

    #[test]
    fn the_skill_choice_of_the_third_form_follows_java_s_chain() {
        let (mut t, scarlet) = setup();
        let mut ids = std::collections::BTreeSet::new();
        t.world.run_as_script("ScarletVanHalisha", |ctx| {
            let npc = ctx.script_npc(scarlet).unwrap();
            for _ in 0..600 {
                let key = get_rnd_skills(ctx, npc);
                ids.insert((key.id, key.level));
            }
        });
        // Charge 3/6/2, the field and the morph (the last ranged time is 0), yoke, attack 3.
        for key in [
            (FRINTEZZA_DAEMON_CHARGE, 3),
            (FRINTEZZA_DAEMON_CHARGE, 6),
            (FRINTEZZA_DAEMON_CHARGE, 2),
            (FRINTEZZA_DAEMON_FIELD, 1),
            (FRINTEZZA_DAEMON_MORPH, 1),
            (YOKE_OF_SCARLET, 1),
            (FRINTEZZA_DAEMON_ATTACK, 3),
        ] {
            assert!(ids.contains(&key), "{key:?}");
        }
        assert_eq!(ids.len(), 7);
    }

    #[test]
    fn the_second_form_uses_its_own_chain() {
        let (mut t, scarlet) = setup();
        let mut ids = std::collections::BTreeSet::new();
        t.world.run_as_script("ScarletVanHalisha", |ctx| {
            let npc = ctx.script_npc(scarlet).unwrap();
            let second = ScriptNpc {
                object_id: npc.object_id,
                id: HALISHA2,
            };
            for _ in 0..600 {
                let key = get_rnd_skills(ctx, second);
                ids.insert((key.id, key.level));
            }
        });
        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            [
                (FRINTEZZA_DAEMON_ATTACK, 2),
                (FRINTEZZA_DAEMON_CHARGE, 2),
                (FRINTEZZA_DAEMON_CHARGE, 5),
                (YOKE_OF_SCARLET, 1)
            ]
        );
    }
}
