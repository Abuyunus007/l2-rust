//! Beast Farm (Freya): growth-capable mobs polymorph when a character feeds them spice.
// Java: dist/game/data/scripts/ai/areas/BeastFarm/BeastFarm.java
//
// FOLLOW-UP(engine): the last step of the growth, the `TamedBeast` (an NPC of the character that
// follows it and casts its beast skills), is not modelled by the world (`summon/mod.rs`, "Not
// modelled: TamedBeast"). That step is ported as far as the world allows: the fed mob goes, the
// random beast (name and skills) is drawn, and the quest hooks run, but no tamed beast appears.
// `Q00655_AGrandPlanForTamingWildBeasts.checkCrystalofPurity` is not ported yet either.

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::quests::q00020_bring_up_with_love::check_jewel_of_innocence;

const GOLDEN_SPICE: i32 = 15474;
const CRYSTAL_SPICE: i32 = 15475;
const SKILL_GOLDEN_SPICE: i32 = 9049;
const SKILL_CRYSTAL_SPICE: i32 = 9050;
const SKILL_BLESSED_GOLDEN_SPICE: i32 = 9051;
const SKILL_BLESSED_CRYSTAL_SPICE: i32 = 9052;
const SKILL_SGRADE_GOLDEN_SPICE: i32 = 9053;
const SKILL_SGRADE_CRYSTAL_SPICE: i32 = 9054;
const TAMED_BEASTS: [i32; 4] = [18869, 18870, 18871, 18872];
const TAME_CHANCE: i32 = 20;
const SPECIAL_SPICE_CHANCES: [i32; 2] = [33, 75];

/// The number of entries of Java's `TAMED_BEAST_DATA` (the random beast type is drawn from it).
const TAMED_BEAST_DATA_COUNT: usize = 6;

// all mobs that can eat...
const FEEDABLE_BEASTS: [i32; 28] = [
    // Kookaburras
    18873, 18874, 18875, 18876, 18877, 18878, 18879, // Cougars
    18880, 18881, 18882, 18883, 18884, 18885, 18886, // Buffalos
    18887, 18888, 18889, 18890, 18891, 18892, 18893, // Grendels
    18894, 18895, 18896, 18897, 18898, 18899, 18900,
];

/// Java `FEED_INFO` (`Map<Integer, Integer>`): the NPC and the player who feeds it.
#[derive(Default)]
struct FeedInfo(HashMap<i32, i32>);

/// Java `GrowthCapableMob`: `chance`, `growthLevel`, `tameNpcId` and the spice skill to NPC map.
struct GrowthCapableMob {
    chance: i32,
    growth_level: i32,
    tame_npc_id: i32,
    skill_success_npc_id_list: Vec<(i32, i32)>,
}

impl GrowthCapableMob {
    fn npc_id_for_skill(&self, skill_id: i32) -> Option<i32> {
        self.skill_success_npc_id_list
            .iter()
            .find(|(skill, _)| *skill == skill_id)
            .map(|(_, npc)| *npc)
    }

    /// Java `getLeveledNpcId(skillId)`: `-1` (none) when the beast does not eat it.
    fn leveled_npc_id(&self, ctx: &mut ScriptCtx<'_>, skill_id: i32) -> Option<i32> {
        let success = self.npc_id_for_skill(skill_id)?;
        if (skill_id == SKILL_BLESSED_GOLDEN_SPICE)
            || (skill_id == SKILL_BLESSED_CRYSTAL_SPICE)
            || (skill_id == SKILL_SGRADE_GOLDEN_SPICE)
            || (skill_id == SKILL_SGRADE_CRYSTAL_SPICE)
        {
            if ctx.get_random(100) < SPECIAL_SPICE_CHANCES[0] {
                if ctx.get_random(100) < SPECIAL_SPICE_CHANCES[1] {
                    return Some(success);
                } else if (skill_id == SKILL_BLESSED_GOLDEN_SPICE) || (skill_id == SKILL_SGRADE_GOLDEN_SPICE)
                {
                    return self.npc_id_for_skill(SKILL_GOLDEN_SPICE);
                } else {
                    return self.npc_id_for_skill(SKILL_CRYSTAL_SPICE);
                }
            }

            None
        } else if (self.growth_level == 2) && (ctx.get_random(100) < TAME_CHANCE) {
            Some(self.tame_npc_id)
        } else if ctx.get_random(100) < self.chance {
            Some(success)
        } else {
            None
        }
    }
}

/// Java `GROWTH_CAPABLE_MONSTERS.get(npcId)`. Each family (Kookaburra 18873, Cougar 18880,
/// Buffalo 18887, Grendel 18894; `base`) is built the same way in the Java constructor:
/// the base eats everything, the next two eat one spice each and so on up to the tame beast.
fn growth_capable_mob(npc_id: i32) -> Option<GrowthCapableMob> {
    let (base, tame) = match npc_id {
        18873..=18879 => (18873, 18869),
        18880..=18886 => (18880, 18870),
        18887..=18893 => (18887, 18871),
        18894..=18900 => (18894, 18872),
        _ => return None,
    };
    let mob = |chance, growth_level, skills: Vec<(i32, i32)>| GrowthCapableMob {
        chance,
        growth_level,
        tame_npc_id: tame,
        skill_success_npc_id_list: skills,
    };
    Some(match npc_id - base {
        0 => mob(
            100,
            0,
            vec![
                (SKILL_GOLDEN_SPICE, base + 1),
                (SKILL_CRYSTAL_SPICE, base + 2),
                (SKILL_BLESSED_GOLDEN_SPICE, tame),
                (SKILL_BLESSED_CRYSTAL_SPICE, tame),
                (SKILL_SGRADE_GOLDEN_SPICE, base + 5),
                (SKILL_SGRADE_CRYSTAL_SPICE, base + 6),
            ],
        ),
        1 => mob(40, 1, vec![(SKILL_GOLDEN_SPICE, base + 3)]),
        2 => mob(40, 1, vec![(SKILL_CRYSTAL_SPICE, base + 4)]),
        3 => mob(25, 2, vec![(SKILL_GOLDEN_SPICE, base + 5)]),
        4 => mob(25, 2, vec![(SKILL_CRYSTAL_SPICE, base + 6)]),
        // The last two of a family (5 and 6) are not in `GROWTH_CAPABLE_MONSTERS`: max growth.
        _ => return None,
    })
}

/// Java `spawnNext(npc, player, nextNpcId, food)`.
fn spawn_next(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, next_npc_id: i32, food: i32) {
    // remove the feedinfo of the mob that got despawned, if any
    let feed = ctx.state::<FeedInfo>();
    if feed.0.get(&npc.object_id.0) == Some(&player.0) {
        feed.0.remove(&npc.object_id.0);
    }

    // The old mob's place is read before it is deleted (a deleted NPC has none here).
    let at = ctx.npc_position(npc);
    let instance = ctx.world().instance_id_of(npc.object_id);

    // despawn the old mob
    ctx.delete_npc(npc);

    // if this is finally a trained mob, then despawn any other trained mobs that the
    // player might have and initialize the Tamed Beast.
    if TAMED_BEASTS.contains(&next_npc_id) {
        // Java draws `getRandomEntry(TAMED_BEAST_DATA)` here for the beast's name and skills.
        let indexes: [usize; TAMED_BEAST_DATA_COUNT] = [0, 1, 2, 3, 4, 5];
        let _beast = ctx.get_random_entry(&indexes);
        // FOLLOW-UP(engine): `new TamedBeast(nextNpcId, player, food, x, y, z, true)` with the
        // name and `addBeastSkill`s, and `broadcastPacket(NpcInfo)`.
        let _ = food;

        check_jewel_of_innocence(ctx, player);
        // FOLLOW-UP: `Q00655_AGrandPlanForTamingWildBeasts.checkCrystalofPurity(player)`.
    } else {
        // if not trained, the newly spawned mob will automatically be agro against its feeder
        // (what happened to "never bite the hand that feeds you" anyway?!)
        let Some((x, y, z, heading)) = at else {
            return;
        };
        let Some(next) = ctx.add_spawn(next_npc_id, x, y, z, heading, false, 0, instance) else {
            return;
        };

        // register the player in the feedinfo for the mob that just spawned
        ctx.state::<FeedInfo>().0.insert(next.object_id.0, player.0);
        ctx.add_attack_desire(next, player, 99_999);
        ctx.world_mut().set_target(player, Some(next.object_id));
    }
}

/// Java `ai.areas.BeastFarm.BeastFarm`.
pub struct BeastFarm;

impl Script for BeastFarm {
    fn name(&self) -> &'static str {
        "BeastFarm"
    }

    fn path(&self) -> &'static str {
        "ai/areas/BeastFarm"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_skill_see_id(&FEEDABLE_BEASTS);
        s.add_kill_id(&FEEDABLE_BEASTS);
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
        // this behavior is only run when the target of skill is the passed npc (chest)
        // i.e. when the player is attempting to open the chest using a skill
        if !targets.contains(&npc.object_id) {
            return;
        }

        // gather some values on local variables
        let npc_id = npc.id;
        let skill_id = skill.id;

        // check if the npc and skills used are valid for this script. Exit if invalid.
        if !FEEDABLE_BEASTS.contains(&npc_id)
            || ((skill_id != SKILL_GOLDEN_SPICE)
                && (skill_id != SKILL_CRYSTAL_SPICE)
                && (skill_id != SKILL_BLESSED_GOLDEN_SPICE)
                && (skill_id != SKILL_BLESSED_CRYSTAL_SPICE)
                && (skill_id != SKILL_SGRADE_GOLDEN_SPICE)
                && (skill_id != SKILL_SGRADE_CRYSTAL_SPICE))
        {
            return;
        }

        // first gather some values on local variables
        let object_id = npc.object_id.0;
        let growth_data = growth_capable_mob(npc_id);
        // if a mob is in FEEDABLE_BEASTS but not in _GrowthCapableMobs, then it's at max growth (3)
        let growth_level = growth_data.as_ref().map_or(3, |g| g.growth_level);

        // prevent exploit which allows 2 players to simultaneously raise the same 0-growth beast
        // If the mob is at 0th level (when it still listens to all feeders) lock it to the first feeder!
        if (growth_level == 0) && ctx.state::<FeedInfo>().0.contains_key(&object_id) {
            return;
        }

        ctx.state::<FeedInfo>().0.insert(object_id, caster.0);

        // display the social action of the beast eating the food.
        ctx.broadcast_npc_social_action(npc, 2);

        let mut food = 0;
        if (skill_id == SKILL_GOLDEN_SPICE) || (skill_id == SKILL_BLESSED_GOLDEN_SPICE) {
            food = GOLDEN_SPICE;
        } else if (skill_id == SKILL_CRYSTAL_SPICE) || (skill_id == SKILL_BLESSED_CRYSTAL_SPICE) {
            food = CRYSTAL_SPICE;
        }

        // if this pet can't grow, it's all done.
        if let Some(growth_data) = growth_data {
            // Do nothing if this monster doesn't eat the specified food (food gets consumed but has no effect).
            let Some(new_npc_id) = growth_data.leveled_npc_id(ctx, skill_id) else {
                if growth_level == 0 {
                    ctx.state::<FeedInfo>().0.remove(&object_id);
                    ctx.world_mut().add_damage_hate(npc.object_id, caster, 0, 1);
                    ctx.set_running(npc);
                    ctx.set_intention_attack(npc, caster);
                }
                return;
            };
            // NOTE(java-quirk): the feeder was stored a few lines up, so this "same player as the
            // one who raised it" test can never fail.
            if (growth_level > 0) && (ctx.state::<FeedInfo>().0.get(&object_id) != Some(&caster.0)) {
                // check if this is the same player as the one who raised it from growth 0.
                // if no, then do not allow a chance to raise the pet (food gets consumed but has no effect).
                return;
            }

            spawn_next(ctx, npc, caster, new_npc_id, food);
        } else {
            ctx.send_message(caster, "The beast spit out the feed instead of eating it.");
            ctx.drop_item(npc, Some(caster), food, 1);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        // remove the feedinfo of the mob that got killed, if any
        ctx.state::<FeedInfo>().0.remove(&npc.object_id.0);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="18873" type="Monster" level="80" name="Alpine Kookaburra"/>
        <npc id="18874" type="Monster" level="80" name="Alpine Kookaburra"/>
        <npc id="18875" type="Monster" level="80" name="Alpine Kookaburra"/>"#;

    #[test]
    fn the_growth_table_is_javas() {
        let kookaburra = growth_capable_mob(18873).unwrap();
        assert_eq!(
            (kookaburra.chance, kookaburra.growth_level, kookaburra.tame_npc_id),
            (100, 0, 18869)
        );
        assert_eq!(kookaburra.npc_id_for_skill(SKILL_GOLDEN_SPICE), Some(18874));
        assert_eq!(
            kookaburra.npc_id_for_skill(SKILL_SGRADE_CRYSTAL_SPICE),
            Some(18879)
        );
        assert_eq!(
            kookaburra.npc_id_for_skill(SKILL_BLESSED_GOLDEN_SPICE),
            Some(18869)
        );
        assert_eq!(
            growth_capable_mob(18877)
                .unwrap()
                .npc_id_for_skill(SKILL_CRYSTAL_SPICE),
            Some(18879)
        );
        assert_eq!(
            growth_capable_mob(18883)
                .unwrap()
                .npc_id_for_skill(SKILL_GOLDEN_SPICE),
            Some(18885)
        );
        assert_eq!(growth_capable_mob(18894).unwrap().tame_npc_id, 18872);
        assert!(growth_capable_mob(18879).is_none());
    }

    #[test]
    fn feeding_golden_spice_grows_the_kookaburra_and_the_feeder_is_registered() {
        let mut registry = ScriptRegistry::new();
        registry.add(BeastFarm);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 82, vec![]);
        let calf = t.spawn(18873, 20);
        t.world.run_as_script("BeastFarm", |ctx| {
            let npc = ctx.script_npc(calf).unwrap();
            // The chance of the first growth step is 100.
            BeastFarm.on_skill_see(
                ctx,
                npc,
                ObjectId(1),
                SkillKey {
                    id: SKILL_GOLDEN_SPICE,
                    level: 1,
                },
                &[calf],
                false,
            );
        });
        assert!(!t.in_world(calf));
        let grown = t.npcs_of(18874);
        assert_eq!(grown.len(), 1);
        t.world.run_as_script("BeastFarm", |ctx| {
            assert_eq!(ctx.state::<FeedInfo>().0.get(&grown[0].0), Some(&1));
        });
    }
}
