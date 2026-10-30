//! Anais, the Divine Burner raid: every quarter of her HP a burner lights, a Grail Ward comes out
//! of it and hunts the target, and explodes after 20 seconds.
// Java: dist/game/data/scripts/ai/bosses/Anais/Anais.java
//
// The fields of the Java class are `AnaisState` (`ctx.state`); the NPCs in them are ids.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::ai::areas::gracia::util::set_display_effect;

// NPCs.
const ANAIS: i32 = 25701;
const DIVINE_BURNER: i32 = 18915;
const GRAIL_WARD: i32 = 18929;

// Skill.
const DIVINE_NOVA: SkillKey = SkillKey { id: 6326, level: 1 };

// Events.
const EVENT_CHECK: &str = "CHECK";
const EVENT_GUARD_ATTACK: &str = "GUARD_ATTACK";
const EVENT_SUICIDE: &str = "SUICIDE";

/// The instance fields of the Java class.
#[derive(Debug, Default)]
pub(crate) struct AnaisState {
    /// `_divineBurners`: every burner that spawned, in order (never emptied).
    pub(crate) divine_burners: Vec<ObjectId>,
    /// `_nextTarget`.
    pub(crate) next_target: Option<ObjectId>,
    /// `_current`.
    pub(crate) current: Option<ObjectId>,
    /// `_pot`.
    pub(crate) pot: usize,
}

/// Java `ai.bosses.Anais.Anais`.
pub struct Anais;

impl Script for Anais {
    fn name(&self) -> &'static str {
        "Anais"
    }

    fn path(&self) -> &'static str {
        "ai/bosses/Anais"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[ANAIS]);
        s.add_spawn_id(&[DIVINE_BURNER]);
        s.add_kill_id(&[GRAIL_WARD]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            EVENT_CHECK => {
                let npc = npc?;
                if !ctx.world().is_attacking_now(npc.object_id) {
                    ctx.cancel_quest_timer(EVENT_CHECK, Some(npc), None);
                }

                let (current, pot) = {
                    let state = ctx.state::<AnaisState>();
                    (state.current, state.pot)
                };
                if current.is_some() || pot < 4 {
                    let target = ctx.world().target_of(npc.object_id);
                    let next_target = target.filter(|t| ctx.world().player(*t).is_some());
                    ctx.state::<AnaisState>().next_target = next_target;
                    // NOTE(java-quirk): `_divineBurners.get(_pot)` throws once `_pot` has run past
                    // the list (the timer task dies); nothing happens here either.
                    let b = ctx.state::<AnaisState>().divine_burners.get(pot).copied()?;
                    ctx.state::<AnaisState>().pot += 1;
                    set_display_effect(ctx, b, 1);
                    ctx.world_mut().set_running(b, false);
                    let (x, y, z) = ctx.world().position_of(b)?;
                    // Heading 0, and the random offset `addSpawn(.., true, 0)` asks for.
                    let ward = ctx.add_spawn(GRAIL_WARD, x, y, z, 0, true, 0, 0);
                    if let Some(ward) = ward {
                        if let Some(target) = next_target {
                            // addDamageHate(_nextTarget, 0, 999); setRunning(); setIntentionAttack().
                            ctx.add_attack_desire(ward, target, 999);
                        }
                        ctx.start_quest_timer(EVENT_GUARD_ATTACK, 1000, Some(ward), next_target, true);
                        ctx.start_quest_timer(EVENT_SUICIDE, 20_000, Some(ward), None, false);
                    }
                }
            }
            EVENT_GUARD_ATTACK => {
                let npc = npc?;
                if let Some(next_target) = ctx.state::<AnaisState>().next_target {
                    let distance = distance_2d(ctx, npc.object_id, next_target);
                    if distance < 100.0 {
                        ctx.world_mut().do_cast(npc.object_id, DIVINE_NOVA);
                    } else if distance > 2000.0 {
                        ctx.world_mut().do_die(npc.object_id, None);
                        ctx.cancel_quest_timer(EVENT_GUARD_ATTACK, Some(npc), player);
                    }
                }
            }
            EVENT_SUICIDE => {
                let npc = npc?;
                ctx.world_mut().do_cast(npc.object_id, DIVINE_NOVA);
                let next_target = ctx.state::<AnaisState>().next_target;
                ctx.cancel_quest_timer(EVENT_GUARD_ATTACK, Some(npc), next_target);
                release_current(ctx);
                ctx.world_mut().do_die(npc.object_id, None);
            }
            _ => {}
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
        let pot = ctx.state::<AnaisState>().pot;
        // `npc.getMaxRecoverableHp()` is the maximum HP of a monster.
        let (hp, max_hp) = ctx.world().hp_and_max(npc.object_id).unwrap_or((0.0, 0.0));
        if pot == 0 {
            burner_on_attack(ctx, 0, npc);
        } else if hp <= (max_hp * 0.75) && pot == 1 {
            burner_on_attack(ctx, 1, npc);
        } else if hp <= (max_hp * 0.5) && pot == 2 {
            burner_on_attack(ctx, 2, npc);
        } else if hp <= (max_hp * 0.25) && pot == 3 {
            burner_on_attack(ctx, 3, npc);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.state::<AnaisState>().divine_burners.push(npc.object_id);
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.world_mut().do_cast(npc.object_id, DIVINE_NOVA);
        let next_target = ctx.state::<AnaisState>().next_target;
        ctx.cancel_quest_timer(EVENT_GUARD_ATTACK, Some(npc), next_target);
        ctx.cancel_quest_timer(EVENT_CHECK, Some(npc), None);
        release_current(ctx);
    }
}

/// Java `burnerOnAttack(pot, anais)`.
fn burner_on_attack(ctx: &mut ScriptCtx<'_>, pot: usize, anais: ScriptNpc) {
    // NOTE(java-quirk): a burner that has not spawned makes `get(pot)` throw; nothing happens.
    let Some(npc) = ctx.state::<AnaisState>().divine_burners.get(pot).copied() else {
        return;
    };
    set_display_effect(ctx, npc, 1);
    ctx.world_mut().set_running(npc, false);
    if pot < 4 {
        ctx.state::<AnaisState>().current = Some(npc);
        if !ctx.has_quest_timer(EVENT_CHECK, Some(anais), None) {
            ctx.start_quest_timer(EVENT_CHECK, 3000, Some(anais), None, false);
        }
    } else {
        ctx.cancel_quest_timer(EVENT_CHECK, Some(anais), None);
    }
}

/// `_current.setDisplayEffect(2); _current.setWalking(); _current = null;`
fn release_current(ctx: &mut ScriptCtx<'_>) {
    if let Some(current) = ctx.state::<AnaisState>().current.take() {
        set_display_effect(ctx, current, 2);
        ctx.world_mut().set_running(current, false);
    }
}

/// Java `calculateDistance2D`: `f64::MAX` when either is not in the world.
fn distance_2d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    let (Some(a), Some(b)) = (ctx.world().position_of(a), ctx.world().position_of(b)) else {
        return f64::MAX;
    };
    let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20"><vitals hp="1000" hpRegen="2" mp="500" mpRegen="0.9" /><attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" /><defence physical="44.44444" magical="29.5" /><speed><walk ground="20" /><run ground="50" /></speed></stats><collision><radius normal="10"/><height normal="20"/></collision>"#;

    fn npcs() -> String {
        format!(
            r#"<npc id="25701" type="RaidBoss" level="80" name="Anais">{STATS}</npc>
<npc id="18915" type="Folk" level="80" name="Divine Burner"/>
<npc id="18929" type="Monster" level="80" name="Grail Ward">{STATS}</npc>"#
        )
    }

    fn world() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Anais);
        ScriptTest::new(registry, &npcs(), "", &[])
    }

    /// Four burners, as the instance has them.
    fn burners(t: &mut ScriptTest) -> Vec<ObjectId> {
        let burners: Vec<ObjectId> = (0..4).map(|i| t.spawn(DIVINE_BURNER, 100 + i * 10)).collect();
        t.ticks(2);
        burners
    }

    fn attack(t: &mut ScriptTest, anais: ObjectId, hp: f64) {
        t.world.set_current_hp_mp(anais, hp, 500.0);
        t.world.run_as_script("Anais", |ctx| {
            let npc = ctx.script_npc(anais).unwrap();
            Anais.on_attack(ctx, npc, ObjectId(1), 1, false, None);
        });
    }

    fn effect(t: &ScriptTest, burner: ObjectId) -> i32 {
        t.world.npc(burner).unwrap().display_effect
    }

    fn state_pot(t: &mut ScriptTest) -> usize {
        let mut pot = 0;
        t.world
            .run_as_script("Anais", |ctx| pot = ctx.state::<AnaisState>().pot);
        pot
    }

    #[test]
    fn every_burner_that_spawns_is_remembered_in_order() {
        let mut t = world();
        let burners = burners(&mut t);
        let mut seen = Vec::new();
        t.world.run_as_script("Anais", |ctx| {
            seen = ctx.state::<AnaisState>().divine_burners.clone()
        });
        assert_eq!(seen, burners);
    }

    #[test]
    fn the_first_hit_lights_the_first_burner_and_starts_the_check() {
        let mut t = world();
        let burners = burners(&mut t);
        let anais = t.spawn(ANAIS, 50);
        attack(&mut t, anais, 1000.0);
        assert_eq!(effect(&t, burners[0]), 1);
        let mut has = false;
        t.world.run_as_script("Anais", |ctx| {
            let npc = ctx.script_npc(anais).unwrap();
            has = ctx.has_quest_timer(EVENT_CHECK, Some(npc), None);
            assert_eq!(ctx.state::<AnaisState>().current, Some(burners[0]));
        });
        assert!(has);
        // Still at pot 0 until the check ran.
        assert_eq!(state_pot(&mut t), 0);
    }

    #[test]
    fn the_check_spawns_a_ward_for_the_burner_and_the_pot_advances() {
        let mut t = world();
        let burners = burners(&mut t);
        let anais = t.spawn(ANAIS, 50);
        attack(&mut t, anais, 1000.0);
        t.world.run_as_script("Anais", |ctx| {
            let npc = ctx.script_npc(anais).unwrap();
            ctx.notify_event(EVENT_CHECK, Some(npc), None);
        });
        assert_eq!(state_pot(&mut t), 1);
        let wards = t.npcs_of(GRAIL_WARD);
        assert_eq!(wards.len(), 1);
        let ward_at = t.world.position_of(wards[0]).unwrap();
        let burner_at = t.world.position_of(burners[0]).unwrap();
        // `addSpawn(.., true, ..)` keeps it near the burner.
        assert!((ward_at.0 - burner_at.0).abs() < 300 && (ward_at.1 - burner_at.1).abs() < 300);
        t.world.run_as_script("Anais", |ctx| {
            let ward = ctx.script_npc(wards[0]).unwrap();
            assert!(ctx.has_quest_timer(EVENT_GUARD_ATTACK, Some(ward), None));
            assert!(ctx.has_quest_timer(EVENT_SUICIDE, Some(ward), None));
        });
    }

    #[test]
    fn the_next_burners_wait_for_the_hp_steps() {
        let mut t = world();
        let burners = burners(&mut t);
        let anais = t.spawn(ANAIS, 50);
        attack(&mut t, anais, 1000.0);
        t.world.run_as_script("Anais", |ctx| {
            let npc = ctx.script_npc(anais).unwrap();
            ctx.notify_event(EVENT_CHECK, Some(npc), None);
        });
        // pot 1: needs 75% or less.
        attack(&mut t, anais, 800.0);
        assert_eq!(effect(&t, burners[1]), 0);
        attack(&mut t, anais, 750.0);
        assert_eq!(effect(&t, burners[1]), 1);
        t.world.run_as_script("Anais", |ctx| {
            assert_eq!(ctx.state::<AnaisState>().current, Some(burners[1]));
        });
    }

    #[test]
    fn the_suicide_explodes_the_ward_and_turns_the_burner_off() {
        let mut t = world();
        let burners = burners(&mut t);
        let anais = t.spawn(ANAIS, 50);
        attack(&mut t, anais, 1000.0);
        t.world.run_as_script("Anais", |ctx| {
            let npc = ctx.script_npc(anais).unwrap();
            ctx.notify_event(EVENT_CHECK, Some(npc), None);
        });
        let ward = t.npcs_of(GRAIL_WARD)[0];
        t.world.run_as_script("Anais", |ctx| {
            let ward = ctx.script_npc(ward).unwrap();
            ctx.notify_event(EVENT_SUICIDE, Some(ward), None);
        });
        assert!(t.world.is_dead(ward));
        assert_eq!(effect(&t, burners[0]), 2);
        t.world.run_as_script("Anais", |ctx| {
            assert_eq!(ctx.state::<AnaisState>().current, None);
            let ward = ctx.script_npc(ward).unwrap();
            assert!(!ctx.has_quest_timer(EVENT_GUARD_ATTACK, Some(ward), None));
        });
    }

    #[test]
    fn a_ward_far_from_its_target_dies() {
        let mut t = world();
        let _burners = burners(&mut t);
        t.enter(1, 80, vec![]);
        let ward = t.spawn(GRAIL_WARD, 3000);
        t.world.run_as_script("Anais", |ctx| {
            ctx.state::<AnaisState>().next_target = Some(ObjectId(1));
            let ward = ctx.script_npc(ward).unwrap();
            ctx.notify_event(EVENT_GUARD_ATTACK, Some(ward), Some(ObjectId(1)));
        });
        assert!(t.world.is_dead(ward));
    }
}
