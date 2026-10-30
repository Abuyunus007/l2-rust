//! Primeval Isle AI: the dinosaurs flee or bite with their skills, the Tyrannosaurus calls the
//! herbivores, the Ancient Egg calls its neighbours and the Mesozoic Stone is a rare drop.
// Java: dist/game/data/scripts/ai/areas/PrimevalIsle/PrimevalIsle.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::movement::{heading_from_points, heading_to_degree};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const EGG: i32 = 18344; // Ancient Egg
const SAILREN: i32 = 29065; // Sailren
const ORNIT: i32 = 22742; // Ornithomimus
const DEINO: i32 = 22743; // Deinonychus
const SPRIGNANT: [i32; 2] = [
    18345, // Sprigant (Anesthesia)
    18346, // Sprigant (Deadly Poison)
];
const MONSTERS: [i32; 17] = [
    22196, // Velociraptor
    22198, // Velociraptor
    22200, // Ornithomimus
    22202, // Ornithomimus
    22203, // Deinonychus
    22205, // Deinonychus
    22208, // Pachycephalosaurus
    22210, // Pachycephalosaurus
    22211, // Wild Strider
    22213, // Wild Strider
    22223, // Velociraptor
    22224, // Ornithomimus
    22225, // Deinonychus
    22226, // Pachycephalosaurus
    22227, // Wild Strider
    22742, // Ornithomimus
    22743, // Deinonychus
];
const TREX: [i32; 3] = [
    22215, // Tyrannosaurus
    22216, // Tyrannosaurus
    22217, // Tyrannosaurus
];
const VEGETABLE: [i32; 8] = [
    22200, // Ornithomimus
    22201, // Ornithomimus
    22202, // Ornithomimus
    22203, // Deinonychus
    22204, // Deinonychus
    22205, // Deinonychus
    22224, // Ornithomimus
    22225, // Deinonychus
];

// Item
const DEINONYCHUS: i32 = 14828; // Deinonychus Mesozoic Stone

// Skill
const ANESTHESIA: SkillKey = SkillKey { id: 5085, level: 1 }; // Anesthesia
const DEADLY_POISON: SkillKey = SkillKey { id: 5086, level: 1 }; // Deadly Poison
const SELFBUFF1: SkillKey = SkillKey { id: 5087, level: 1 }; // Berserk
const SELFBUFF2: SkillKey = SkillKey { id: 5087, level: 2 }; // Berserk
const LONGRANGEDMAGIC1: SkillKey = SkillKey { id: 5120, level: 1 }; // Stun
const PHYSICALSPECIAL1: SkillKey = SkillKey { id: 5083, level: 4 }; // Stun
const PHYSICALSPECIAL2: SkillKey = SkillKey { id: 5081, level: 4 }; // Silence
const PHYSICALSPECIAL3: SkillKey = SkillKey { id: 5082, level: 4 }; // NPC Spinning, Slashing Trick
const CREW_SKILL: SkillKey = SkillKey { id: 6172, level: 1 }; // Presentation - Tyranno
const INVIN_BUFF_ON: SkillKey = SkillKey { id: 5225, level: 1 }; // Invincible

/// Java `npc.setTarget(target)` (a missing target clears it) and `npc.doCast(skill)`.
fn cast_at(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, target: Option<ObjectId>, skill: SkillKey) {
    ctx.world_mut().set_npc_target(npc.object_id, target);
    ctx.world_mut().do_cast(npc.object_id, skill);
}

/// `(npc.getCurrentHp() / npc.getMaxHp()) * 100`.
fn hp_percent(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> f64 {
    (ctx.npc_current_hp(npc) / f64::from(ctx.npc_max_hp(npc))) * 100.0
}

/// The `addDamageHate(target, 0, 555)` + `setRunning()` + `setIntentionAttack(target)` of
/// `onSpellFinished`.
fn attack_target(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, target: Option<ObjectId>) {
    if let Some(target) = target {
        ctx.world_mut().set_npc_target(npc.object_id, Some(target));
        ctx.set_running(npc);
        ctx.world_mut().add_damage_hate(npc.object_id, target, 0, 555);
        ctx.set_intention_attack(npc, target);
    }
}

/// Java `ai.areas.PrimevalIsle.PrimevalIsle`.
pub struct PrimevalIsle;

impl Script for PrimevalIsle {
    fn name(&self) -> &'static str {
        "PrimevalIsle"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PrimevalIsle"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&TREX);
        s.add_spawn_id(&SPRIGNANT);
        s.add_spawn_id(&MONSTERS);
        s.add_aggro_range_enter_id(&TREX);
        s.add_spell_finished_id(&TREX);
        s.add_attack_id(&[EGG]);
        s.add_attack_id(&TREX);
        s.add_attack_id(&MONSTERS);
        s.add_kill_id(&[EGG, SAILREN, DEINO, ORNIT]);
        s.add_creature_see_id(&TREX);
        s.add_creature_see_id(&MONSTERS);
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if skill.id == CREW_SKILL.id {
            ctx.start_quest_timer("START_INVUL", 4000, Some(npc), None, false);
            if let Some(target) = ctx.target_of(npc.object_id)
                && ctx.world().is_npc(target)
            {
                ctx.world_mut().do_die(target, Some(npc.object_id));
            }
        }

        if ctx.is_in_combat(npc.object_id) {
            let target = ctx.most_hated(npc);
            if hp_percent(ctx, npc) < 60.0 {
                if skill.id == SELFBUFF1.id {
                    ctx.set_script_value(npc, 3);
                    attack_target(ctx, npc, target);
                }
            } else if hp_percent(ctx, npc) < 30.0 {
                // NOTE(java-quirk): never reached, a hit point share below 30 is below 60 and
                // the branch above took it.
                if skill.id == SELFBUFF1.id {
                    ctx.set_script_value(npc, 1);
                    attack_target(ctx, npc, target);
                } else if skill.id == SELFBUFF2.id {
                    ctx.set_script_value(npc, 5);
                    attack_target(ctx, npc, target);
                }
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "USE_SKILL" => {
                if let Some(npc) = npc
                    && !ctx.is_dead(npc.object_id)
                {
                    let skill = if npc.id == SPRIGNANT[0] {
                        ANESTHESIA
                    } else {
                        DEADLY_POISON
                    };
                    ctx.world_mut().do_cast(npc.object_id, skill);
                    ctx.start_quest_timer("USE_SKILL", 15_000, Some(npc), None, false);
                }
            }
            "GHOST_DESPAWN" => {
                if let Some(npc) = npc
                    && !ctx.is_dead(npc.object_id)
                {
                    if !ctx.is_in_combat(npc.object_id) {
                        ctx.delete_npc(npc);
                    } else {
                        ctx.start_quest_timer("GHOST_DESPAWN", 1_800_000, Some(npc), None, false);
                    }
                }
            }
            "TREX_ATTACK" => {
                if let (Some(npc), Some(player)) = (npc, player) {
                    ctx.set_script_value(npc, 0);
                    if ctx.distance_3d(npc.object_id, player).is_some_and(|d| d <= 800.0) {
                        cast_at(ctx, npc, Some(player), LONGRANGEDMAGIC1);
                        ctx.add_attack_desire(npc, player, 999);
                    }
                }
            }
            "START_INVUL" => {
                if let Some(npc) = npc
                    && !ctx.is_dead(npc.object_id)
                {
                    ctx.world_mut().do_cast(npc.object_id, INVIN_BUFF_ON);
                    ctx.start_quest_timer("START_INVUL_2", 30_000, Some(npc), None, false);
                }
            }
            "START_INVUL_2" => {
                if let Some(npc) = npc
                    && !ctx.is_dead(npc.object_id)
                {
                    ctx.world_mut()
                        .apply_effects(npc.object_id, npc.object_id, INVIN_BUFF_ON);
                }
            }
            _ => {}
        }

        None
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if MONSTERS.contains(&npc.id) {
            if ctx.world().player(creature).is_some() {
                let ag_type = ctx.npc_template_int(npc, "ag_type", 0);
                let prob_physical_special1 = ctx.npc_template_int(npc, "ProbPhysicalSpecial1", 0);
                let prob_physical_special2 = ctx.npc_template_int(npc, "ProbPhysicalSpecial2", 0);
                let physical_special1 = ctx.npc_template_skill(npc, "PhysicalSpecial1");
                let physical_special2 = ctx.npc_template_skill(npc, "PhysicalSpecial2");

                // The chance is rolled before the NPC id is compared, as in Java's `&&` order.
                let panic_roll = ctx.get_random(100) < 30;
                if (panic_roll && (npc.id == DEINO)) || ((npc.id == ORNIT) && ctx.is_script_value(npc, 0)) {
                    ctx.world_mut().clear_aggro_list(npc.object_id);
                    ctx.set_script_value(npc, 1);
                    ctx.set_running(npc);

                    let distance = 3000.0_f64;
                    if let (Some((cx, cy, _)), Some((nx, ny, _))) = (
                        ctx.world().position_of(creature),
                        ctx.world().position_of(npc.object_id),
                    ) {
                        let heading = heading_from_points(cx, cy, nx, ny);
                        let angle = heading_to_degree(heading);
                        let radian = angle.to_radians();
                        let (sin, cos) = (radian.sin(), radian.cos());
                        let new_x = (f64::from(nx) + (cos * distance)) as i32;
                        let new_y = (f64::from(ny) + (sin * distance)) as i32;
                        if let Some((x, y, z)) =
                            ctx.world().npc_valid_location_toward(npc.object_id, new_x, new_y)
                        {
                            ctx.npc_move_to(npc, x, y, z);
                        }
                    }
                } else if ag_type == 1 {
                    let multiplier = ctx
                        .world_mut()
                        .npc_variables_mut(npc.object_id)
                        .get_int("SKILL_MULTIPLER", 0);
                    if ctx.get_random(100) <= (prob_physical_special1 * multiplier) {
                        if let Some(skill) = physical_special1
                            && !ctx.world_mut().is_skill_disabled_of(npc.object_id, skill)
                        {
                            cast_at(ctx, npc, Some(creature), skill);
                        }
                    } else if (ctx.get_random(100) <= (prob_physical_special2 * multiplier))
                        && let Some(skill) = physical_special2
                        && !ctx.world_mut().is_skill_disabled_of(npc.object_id, skill)
                    {
                        cast_at(ctx, npc, Some(creature), skill);
                    }
                }
            }
        } else if ctx
            .script_npc(creature)
            .is_some_and(|c| VEGETABLE.contains(&c.id))
        {
            cast_at(ctx, npc, Some(creature), CREW_SKILL);
            ctx.set_running(npc);
            ctx.set_intention_attack(npc, creature);
        }
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) {
        if ctx.is_script_value(npc, 0) {
            ctx.set_script_value(npc, 1);
            ctx.broadcast_npc_say_text(npc, ChatType::NpcGeneral, "?");
            ctx.world_mut().clear_aggro_list(npc.object_id);
            ctx.start_quest_timer("TREX_ATTACK", 6000, Some(npc), Some(player), false);
        }
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if npc.id == EGG {
            if (ctx.get_random(100) <= 80) && ctx.is_script_value(npc, 0) {
                ctx.set_script_value(npc, 1);
                let playable = if is_summon {
                    ctx.summon_of(attacker)
                } else {
                    Some(attacker)
                };
                for object in ctx.world().visible_objects_in_range(npc.object_id, 500) {
                    if !ctx.world().is_attackable_npc(object) {
                        continue;
                    }
                    if ctx.get_random_boolean()
                        && let (Some(monster), Some(playable)) = (ctx.script_npc(object), playable)
                    {
                        ctx.add_attack_desire(monster, playable, 999);
                    }
                }
            }
        } else if TREX.contains(&npc.id) {
            let target = ctx.most_hated(npc);

            if hp_percent(ctx, npc) <= 30.0 {
                if ctx.is_script_value(npc, 3) {
                    if !ctx.world_mut().is_skill_disabled_of(npc.object_id, SELFBUFF1) {
                        ctx.world_mut().do_cast(npc.object_id, SELFBUFF1);
                    }
                } else if ctx.is_script_value(npc, 1)
                    && !ctx.world_mut().is_skill_disabled_of(npc.object_id, SELFBUFF2)
                {
                    ctx.world_mut().do_cast(npc.object_id, SELFBUFF2);
                }
            } else if (hp_percent(ctx, npc) <= 60.0)
                && ctx.is_script_value(npc, 3)
                && !ctx.world_mut().is_skill_disabled_of(npc.object_id, SELFBUFF1)
            {
                ctx.world_mut().do_cast(npc.object_id, SELFBUFF1);
            }

            if ctx
                .distance_3d(npc.object_id, attacker)
                .is_some_and(|d| d > 100.0)
            {
                if !ctx
                    .world_mut()
                    .is_skill_disabled_of(npc.object_id, LONGRANGEDMAGIC1)
                    && (ctx.get_random(100) <= (10 * ctx.get_script_value(npc)))
                {
                    cast_at(ctx, npc, Some(attacker), LONGRANGEDMAGIC1);
                }
            } else {
                if !ctx
                    .world_mut()
                    .is_skill_disabled_of(npc.object_id, LONGRANGEDMAGIC1)
                    && (ctx.get_random(100) <= (10 * ctx.get_script_value(npc)))
                {
                    cast_at(ctx, npc, target, LONGRANGEDMAGIC1);
                }

                if !ctx
                    .world_mut()
                    .is_skill_disabled_of(npc.object_id, PHYSICALSPECIAL1)
                    && (ctx.get_random(100) <= (5 * ctx.get_script_value(npc)))
                {
                    cast_at(ctx, npc, target, PHYSICALSPECIAL1);
                }

                if !ctx
                    .world_mut()
                    .is_skill_disabled_of(npc.object_id, PHYSICALSPECIAL2)
                    && (ctx.get_random(100) <= (3 * ctx.get_script_value(npc)))
                {
                    cast_at(ctx, npc, target, PHYSICALSPECIAL2);
                }

                if !ctx
                    .world_mut()
                    .is_skill_disabled_of(npc.object_id, PHYSICALSPECIAL3)
                    && (ctx.get_random(100) <= (5 * ctx.get_script_value(npc)))
                {
                    cast_at(ctx, npc, target, PHYSICALSPECIAL3);
                }
            }
        } else {
            let mut target = None;
            let prob_physical_special1 = ctx.npc_template_int(npc, "ProbPhysicalSpecial1", 0);
            let prob_physical_special2 = ctx.npc_template_int(npc, "ProbPhysicalSpecial2", 0);
            let self_range_buff1 = ctx.npc_template_skill(npc, "SelfRangeBuff1");
            let physical_special1 = ctx.npc_template_skill(npc, "PhysicalSpecial1");
            let physical_special2 = ctx.npc_template_skill(npc, "PhysicalSpecial2");

            let multiplier = if hp_percent(ctx, npc) <= 50.0 { 2 } else { 1 };
            ctx.world_mut()
                .npc_variables_mut(npc.object_id)
                .set_int("SKILL_MULTIPLER", multiplier);

            if (hp_percent(ctx, npc) <= 30.0)
                && (ctx
                    .world_mut()
                    .npc_variables_mut(npc.object_id)
                    .get_int("SELFBUFF_USED", 0)
                    == 0)
            {
                target = ctx.most_hated(npc);
                ctx.world_mut().clear_aggro_list(npc.object_id);
                // NOTE(java-quirk): a template without `SelfRangeBuff1` throws in Java here.
                if let Some(buff) = self_range_buff1
                    && !ctx.world_mut().is_skill_disabled_of(npc.object_id, buff)
                {
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_int("SELFBUFF_USED", 1);
                    ctx.world_mut().do_cast(npc.object_id, buff);
                    ctx.set_running(npc);
                    if let Some(target) = target {
                        ctx.set_intention_attack(npc, target);
                    }
                }
            }

            if target.is_some() {
                let multiplier = ctx
                    .world_mut()
                    .npc_variables_mut(npc.object_id)
                    .get_int("SKILL_MULTIPLER", 0);
                if (ctx.get_random(100) <= (prob_physical_special1 * multiplier))
                    && let Some(skill) = physical_special1
                    && !ctx.world_mut().is_skill_disabled_of(npc.object_id, skill)
                {
                    cast_at(ctx, npc, target, skill);
                }

                if (ctx.get_random(100) <= (prob_physical_special2 * multiplier))
                    && let Some(skill) = physical_special2
                    && !ctx.world_mut().is_skill_disabled_of(npc.object_id, skill)
                {
                    cast_at(ctx, npc, target, skill);
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if (npc.id == DEINO) || ((npc.id == ORNIT) && !ctx.is_script_value(npc, 1)) {
            return;
        }

        if (npc.id == SAILREN) || (ctx.get_random(100) < 3) {
            let player = if npc.id == SAILREN {
                ctx.get_random_party_member(killer)
            } else {
                Some(killer)
            };
            let Some(player) = player else {
                return;
            };
            if ctx.is_inventory_under_80(player, false) {
                ctx.give_items(player, DEINONYCHUS, 1);
                if !ctx.has_pet(player) {
                    ctx.use_summon_item_forced(player, DEINONYCHUS);
                }

                ctx.show_on_screen_npc_string(
                    player,
                    NpcStringId::LifeStoneFromTheBeginningAcquired,
                    2,
                    6000,
                    &[],
                );
            } else {
                ctx.show_on_screen_npc_string(
                    player,
                    NpcStringId::WhenInventoryWeightNumberAreMoreThan80TheLifeStoneFromTheBeginningCannotBeAcquired,
                    2,
                    6000,
                    &[],
                );
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if SPRIGNANT.contains(&npc.id) {
            ctx.cancel_quest_timer("USE_SKILL", Some(npc), None);
            ctx.start_quest_timer("USE_SKILL", 15_000, Some(npc), None, false);
        } else if TREX.contains(&npc.id) {
            let collect_ghost = ctx.npc_template_int(npc, "CollectGhost", 0);
            let collect_despawn = ctx.npc_template_int(npc, "CollectGhostDespawnTime", 30);

            if collect_ghost == 1 {
                ctx.cancel_quest_timer("GHOST_DESPAWN", Some(npc), None);
                ctx.start_quest_timer(
                    "GHOST_DESPAWN",
                    i64::from(collect_despawn) * 60_000,
                    Some(npc),
                    None,
                    false,
                );
            }
        } else {
            let vars = ctx.world_mut().npc_variables_mut(npc.object_id);
            vars.set_int("SELFBUFF_USED", 0);
            vars.set_int("SKILL_MULTIPLER", 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22196" type="Monster" level="70" name="Velociraptor"/>
        <npc id="22215" type="Monster" level="70" name="Tyrannosaurus"/>
        <npc id="18344" type="Monster" level="70" name="Ancient Egg"/>
        <npc id="18345" type="Monster" level="70" name="Sprigant"/>"#;
    const ITEMS: &str = r#"<item id="14828" type="EtcItem" name="Deinonychus Mesozoic Stone"/>"#;

    #[test]
    fn a_monster_gets_the_skill_multiplier_at_spawn_and_the_sprigant_its_skill_timer() {
        let mut registry = ScriptRegistry::new();
        registry.add(PrimevalIsle);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        let raptor = t.spawn(22196, 20);
        let sprigant = t.spawn(SPRIGNANT[0], 30);
        t.ticks(2);
        assert_eq!(
            t.world
                .npc_variables(raptor)
                .unwrap()
                .get_int("SKILL_MULTIPLER", 0),
            1
        );
        assert_eq!(
            t.world
                .npc_variables(raptor)
                .unwrap()
                .get_int("SELFBUFF_USED", -1),
            0
        );
        t.world.run_as_script("PrimevalIsle", |ctx| {
            let npc = ctx.script_npc(sprigant).unwrap();
            assert!(ctx.has_quest_timer("USE_SKILL", Some(npc), None));
        });
    }

    #[test]
    fn the_egg_wakes_the_neighbours_when_hit() {
        let mut registry = ScriptRegistry::new();
        registry.add(PrimevalIsle);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.enter(1, 75, vec![]);
        let egg = t.spawn(EGG, 20);
        t.world.run_as_script("PrimevalIsle", |ctx| {
            let npc = ctx.script_npc(egg).unwrap();
            // Rolls until the 80% chance lets the egg call (the world RNG is seeded).
            for _ in 0..20 {
                if ctx.get_script_value(npc) == 1 {
                    break;
                }
                PrimevalIsle.on_attack(ctx, npc, ObjectId(1), 1, false, None);
            }
            assert_eq!(ctx.get_script_value(npc), 1);
        });
    }
}
