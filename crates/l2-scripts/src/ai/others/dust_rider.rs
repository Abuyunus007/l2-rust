//! Dust Rider (raid boss 25719): Haste below 30% HP, and back to its spawn when pulled too far.
// Java: dist/game/data/scripts/ai/others/DustRider.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

const DUST_RIDER: i32 = 25719;

// Skills
const NPC_HASTE_LEVEL_3: SkillKey = SkillKey { id: 6914, level: 3 };

// Variables
const CAST_FLAG: &str = "CAST_FLAG";

// Misc
const MAX_CHASE_DIST: f64 = 2500.0;
const MIN_HP_PERCENTAGE: f64 = 0.30;

/// Java `ai.others.DustRider`.
pub struct DustRider;

impl Script for DustRider {
    fn name(&self) -> &'static str {
        "DustRider"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[DUST_RIDER]);
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
        if ctx.npc_distance_from_spawn(npc) > MAX_CHASE_DIST {
            if let Some((x, y, z)) = ctx.npc_spawn_point(npc) {
                ctx.tele_npc_to(npc, x, y, z);
            }
        }

        if !ctx.npc_flag(npc, CAST_FLAG)
            && ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * MIN_HP_PERCENTAGE
        {
            ctx.set_npc_flag(npc, CAST_FLAG, true);
            ctx.add_skill_cast_desire(npc, npc.object_id, NPC_HASTE_LEVEL_3, 99999);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, X, Y, Z};

    const NPCS: &str = r#"<npc id="25719" type="RaidBoss" level="75" name="Dust Rider"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(DustRider);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let boss = t.spawn(DUST_RIDER, 100);
        t.enter(1, 75, vec![]);
        t.take();
        (t, boss)
    }

    fn hit(t: &mut ScriptTest, boss: ObjectId) {
        t.world.script_on_attackable_attack(boss, ObjectId(1), 10, None);
        t.world.script_drain();
    }

    #[test]
    fn the_boss_hastes_itself_once_below_thirty_percent() {
        let (mut t, boss) = setup();
        assert_eq!(t.world.script_npc_var("DustRider", boss, CAST_FLAG), 0);
        t.world.npc_mut(boss).unwrap().core.cur_hp = 310.0;
        hit(&mut t, boss);
        assert_eq!(t.world.script_npc_var("DustRider", boss, CAST_FLAG), 0, "30% of 1000 is 300");

        t.world.npc_mut(boss).unwrap().core.cur_hp = 290.0;
        hit(&mut t, boss);
        // `addSkillCastDesire` on itself: the flag keeps the cast to once.
        assert_eq!(t.world.script_npc_var("DustRider", boss, CAST_FLAG), 1);
    }

    #[test]
    fn a_boss_pulled_too_far_goes_home() {
        let (mut t, boss) = setup();
        let (hx, hy, hz) = t.world.position_of(boss).unwrap();
        // Pulled 3000 away: the spawn point is where it was spawned.
        let npc = t.world.npc_mut(boss).unwrap();
        npc.spawn = Some(0);
        npc.spawn_point = (hx - 3000, hy, hz);
        hit(&mut t, boss);
        let at = t.world.position_of(boss).unwrap();
        assert!((at.0 - (hx - 3000)).abs() < 100, "{at:?} from {:?}", (X, Y, Z));
    }
}
