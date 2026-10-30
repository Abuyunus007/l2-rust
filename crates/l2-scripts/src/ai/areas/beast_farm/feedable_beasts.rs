//! Feedable Beasts: growth-capable mobs polymorph when a character feeds them golden or crystal
//! spice (the older, pre-Freya beast farm).
// Java: dist/game/data/scripts/ai/areas/BeastFarm/FeedableBeasts.java
//
// FOLLOW-UP(engine): the `TamedBeast` of the last step (and the spice it eats later, the old
// trained beasts the character had, the rare chat of the new beast) is not modelled by the
// world, see `beast_farm.rs`. This port draws the same random numbers, deletes the fed mob and
// runs the quest hook (`Q00020`), but no tamed beast appears. `checkCrystalofPurity` of
// `Q00655_AGrandPlanForTamingWildBeasts` is not ported yet.

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::quests::q00020_bring_up_with_love::check_jewel_of_innocence;

const GOLDEN_SPICE: i32 = 6643;
const CRYSTAL_SPICE: i32 = 6644;
const SKILL_GOLDEN_SPICE: i32 = 2188;
const SKILL_CRYSTAL_SPICE: i32 = 2189;

// Tamed Wild Beasts
const TRAINED_BUFFALO1: i32 = 16013;
const TRAINED_BUFFALO2: i32 = 16014;
const TRAINED_COUGAR1: i32 = 16015;
const TRAINED_COUGAR2: i32 = 16016;
const TRAINED_KOOKABURRA1: i32 = 16017;
const TRAINED_KOOKABURRA2: i32 = 16018;
const TAMED_BEASTS: [i32; 6] = [
    TRAINED_BUFFALO1,
    TRAINED_BUFFALO2,
    TRAINED_COUGAR1,
    TRAINED_COUGAR2,
    TRAINED_KOOKABURRA1,
    TRAINED_KOOKABURRA2,
];

// all mobs that can eat...
const FEEDABLE_BEASTS: [i32; 69] = [
    21451,
    21452,
    21453,
    21454,
    21455,
    21456,
    21457,
    21458,
    21459,
    21460,
    21461,
    21462,
    21463,
    21464,
    21465,
    21466,
    21467,
    21468,
    21469,
    21470,
    21471,
    21472,
    21473,
    21474,
    21475,
    21476,
    21477,
    21478,
    21479,
    21480,
    21481,
    21482,
    21483,
    21484,
    21485,
    21486,
    21487,
    21488,
    21489,
    21490,
    21491,
    21492,
    21493,
    21494,
    21495,
    21496,
    21497,
    21498,
    21499,
    21500,
    21501,
    21502,
    21503,
    21504,
    21505,
    21506,
    21507,
    21824,
    21825,
    21826,
    21827,
    21828,
    21829,
    TRAINED_BUFFALO1,
    TRAINED_BUFFALO2,
    TRAINED_COUGAR1,
    TRAINED_COUGAR2,
    TRAINED_KOOKABURRA1,
    TRAINED_KOOKABURRA2,
];

/// Java `MAD_COW_POLYMORPH`.
fn mad_cow_polymorph(npc_id: i32) -> Option<i32> {
    match npc_id {
        21824 => Some(21468),
        21825 => Some(21469),
        21826 => Some(21487),
        21827 => Some(21488),
        21828 => Some(21506),
        21829 => Some(21507),
        _ => None,
    }
}

const TEXT_0: [NpcStringId; 10] = [
    NpcStringId::WhatDidYouJustDoToMe,
    NpcStringId::AreYouTryingToTameMeDonTDoThat,
    NpcStringId::DonTGiveSuchAThingYouCanEndangerYourself,
    NpcStringId::YuckWhatIsThisItTastesTerrible,
    NpcStringId::IMHungryGiveMeALittleMorePlease,
    NpcStringId::WhatIsThisIsThisEdible,
    NpcStringId::DonTWorryAboutMe,
    NpcStringId::ThankYouThatWasDelicious,
    NpcStringId::IThinkIAmStartingToLikeYou,
    NpcStringId::EeeeekEeeeek,
];
const TEXT_1: [NpcStringId; 5] = [
    NpcStringId::DonTKeepTryingToTameMeIDonTWantToBeTamed,
    NpcStringId::ItIsJustFoodToMeAlthoughItMayAlsoBeYourHand,
    NpcStringId::IfIKeepEatingLikeThisWonTIBecomeFatChompChomp,
    NpcStringId::WhyDoYouKeepFeedingMe,
    NpcStringId::DonTTrustMeIMAfraidIMayBetrayYouLater,
];
const TEXT_2: [NpcStringId; 5] = [
    NpcStringId::Grrrrr,
    NpcStringId::YouBroughtThisUponYourself,
    NpcStringId::IFeelStrangeIKeepHavingTheseEvilThoughts,
    NpcStringId::AlasSoThisIsHowItAllEnds,
    NpcStringId::IDonTFeelSoGoodOhMyMindIsVeryTroubled,
];

/// Java `TEXT[growthLevel]`.
fn text_of(growth_level: i32) -> &'static [NpcStringId] {
    match growth_level {
        0 => &TEXT_0,
        1 => &TEXT_1,
        _ => &TEXT_2,
    }
}

/// Java `FEED_INFO` (`Map<Integer, Integer>`): the NPC and the player who feeds it.
#[derive(Default)]
struct FeedInfo(HashMap<i32, i32>);

/// Java `GrowthCapableMob`: the growth level, the chance and, per spice, the mobs by type.
struct GrowthCapableMob {
    growth_level: i32,
    chance: i32,
    spice_to_mob: Vec<(i32, &'static [&'static [i32]])>,
}

impl GrowthCapableMob {
    fn mobs_of(&self, spice: i32) -> Option<&'static [&'static [i32]]> {
        self.spice_to_mob
            .iter()
            .find(|(s, _)| *s == spice)
            .map(|(_, mobs)| *mobs)
    }

    /// Java `getMob(spice, mobType, classType)`.
    fn get_mob(&self, spice: i32, mob_type: usize, class_type: usize) -> Option<i32> {
        self.mobs_of(spice)
            .and_then(|data| data.get(mob_type))
            .and_then(|row| row.get(class_type))
            .copied()
    }

    /// Java `getRandomMob(spice)`.
    fn get_random_mob(&self, ctx: &mut ScriptCtx<'_>, spice: i32) -> Option<i32> {
        let temp = self.mobs_of(spice)?;
        let rand = ctx.get_random(i32::try_from(temp[0].len()).unwrap_or(1));
        temp[0].get(usize::try_from(rand).unwrap_or(0)).copied()
    }
}

// @formatter:off
const KOOKABURA_0_GOLD: &[&[i32]] = &[&[21452, 21453, 21454, 21455]];
const KOOKABURA_0_CRYSTAL: &[&[i32]] = &[&[21456, 21457, 21458, 21459]];
const KOOKABURA_1_GOLD_1: &[&[i32]] = &[&[21460, 21462]];
const KOOKABURA_1_GOLD_2: &[&[i32]] = &[&[21461, 21463]];
const KOOKABURA_1_CRYSTAL_1: &[&[i32]] = &[&[21464, 21466]];
const KOOKABURA_1_CRYSTAL_2: &[&[i32]] = &[&[21465, 21467]];
const KOOKABURA_2_1: &[&[i32]] = &[&[21468, 21824], &[TRAINED_KOOKABURRA1, TRAINED_KOOKABURRA2]];
const KOOKABURA_2_2: &[&[i32]] = &[&[21469, 21825], &[TRAINED_KOOKABURRA1, TRAINED_KOOKABURRA2]];

const BUFFALO_0_GOLD: &[&[i32]] = &[&[21471, 21472, 21473, 21474]];
const BUFFALO_0_CRYSTAL: &[&[i32]] = &[&[21475, 21476, 21477, 21478]];
const BUFFALO_1_GOLD_1: &[&[i32]] = &[&[21479, 21481]];
// NOTE(java-quirk): 21481 is in both lists of Java (21480 is in none).
const BUFFALO_1_GOLD_2: &[&[i32]] = &[&[21481, 21482]];
const BUFFALO_1_CRYSTAL_1: &[&[i32]] = &[&[21483, 21485]];
const BUFFALO_1_CRYSTAL_2: &[&[i32]] = &[&[21484, 21486]];
const BUFFALO_2_1: &[&[i32]] = &[&[21487, 21826], &[TRAINED_BUFFALO1, TRAINED_BUFFALO2]];
const BUFFALO_2_2: &[&[i32]] = &[&[21488, 21827], &[TRAINED_BUFFALO1, TRAINED_BUFFALO2]];

const COUGAR_0_GOLD: &[&[i32]] = &[&[21490, 21491, 21492, 21493]];
const COUGAR_0_CRYSTAL: &[&[i32]] = &[&[21494, 21495, 21496, 21497]];
const COUGAR_1_GOLD_1: &[&[i32]] = &[&[21498, 21500]];
const COUGAR_1_GOLD_2: &[&[i32]] = &[&[21499, 21501]];
const COUGAR_1_CRYSTAL_1: &[&[i32]] = &[&[21502, 21504]];
const COUGAR_1_CRYSTAL_2: &[&[i32]] = &[&[21503, 21505]];
const COUGAR_2_1: &[&[i32]] = &[&[21506, 21828], &[TRAINED_COUGAR1, TRAINED_COUGAR2]];
const COUGAR_2_2: &[&[i32]] = &[&[21507, 21829], &[TRAINED_COUGAR1, TRAINED_COUGAR2]];
// @formatter:on

/// Java `GROWTH_CAPABLE_MONSTERS.get(npcId)`.
fn growth_capable_mob(npc_id: i32) -> Option<GrowthCapableMob> {
    let mob = |growth_level, chance, spice_to_mob| GrowthCapableMob {
        growth_level,
        chance,
        spice_to_mob,
    };
    let gold = GOLDEN_SPICE;
    let crystal = CRYSTAL_SPICE;
    Some(match npc_id {
        // Alpen Kookabura
        21451 => mob(
            0,
            100,
            vec![(gold, KOOKABURA_0_GOLD), (crystal, KOOKABURA_0_CRYSTAL)],
        ),
        21452 | 21454 => mob(1, 40, vec![(gold, KOOKABURA_1_GOLD_1)]),
        21453 | 21455 => mob(1, 40, vec![(gold, KOOKABURA_1_GOLD_2)]),
        21456 | 21458 => mob(1, 40, vec![(crystal, KOOKABURA_1_CRYSTAL_1)]),
        21457 | 21459 => mob(1, 40, vec![(crystal, KOOKABURA_1_CRYSTAL_2)]),
        21460 | 21462 => mob(2, 25, vec![(gold, KOOKABURA_2_1)]),
        21461 | 21463 => mob(2, 25, vec![(gold, KOOKABURA_2_2)]),
        21464 | 21466 => mob(2, 25, vec![(crystal, KOOKABURA_2_1)]),
        21465 | 21467 => mob(2, 25, vec![(crystal, KOOKABURA_2_2)]),
        // Alpen Buffalo
        21470 => mob(0, 100, vec![(gold, BUFFALO_0_GOLD), (crystal, BUFFALO_0_CRYSTAL)]),
        21471 | 21473 => mob(1, 40, vec![(gold, BUFFALO_1_GOLD_1)]),
        21472 | 21474 => mob(1, 40, vec![(gold, BUFFALO_1_GOLD_2)]),
        21475 | 21477 => mob(1, 40, vec![(crystal, BUFFALO_1_CRYSTAL_1)]),
        21476 | 21478 => mob(1, 40, vec![(crystal, BUFFALO_1_CRYSTAL_2)]),
        21479 | 21481 => mob(2, 25, vec![(gold, BUFFALO_2_1)]),
        21480 | 21482 => mob(2, 25, vec![(gold, BUFFALO_2_2)]),
        21483 | 21485 => mob(2, 25, vec![(crystal, BUFFALO_2_1)]),
        21484 | 21486 => mob(2, 25, vec![(crystal, BUFFALO_2_2)]),
        // Alpen Cougar
        21489 => mob(0, 100, vec![(gold, COUGAR_0_GOLD), (crystal, COUGAR_0_CRYSTAL)]),
        21490 | 21492 => mob(1, 40, vec![(gold, COUGAR_1_GOLD_1)]),
        21491 | 21493 => mob(1, 40, vec![(gold, COUGAR_1_GOLD_2)]),
        21494 | 21496 => mob(1, 40, vec![(crystal, COUGAR_1_CRYSTAL_1)]),
        21495 | 21497 => mob(1, 40, vec![(crystal, COUGAR_1_CRYSTAL_2)]),
        21498 | 21500 => mob(2, 25, vec![(gold, COUGAR_2_1)]),
        21499 | 21501 => mob(2, 25, vec![(gold, COUGAR_2_2)]),
        21502 | 21504 => mob(2, 25, vec![(crystal, COUGAR_2_1)]),
        21503 | 21505 => mob(2, 25, vec![(crystal, COUGAR_2_2)]),
        _ => return None,
    })
}

/// Java `addSpawn(id, npc)` of a place read before the NPC was deleted.
fn spawn_at(ctx: &mut ScriptCtx<'_>, at: (i32, i32, i32, i32), instance: i32, id: i32) -> Option<ScriptNpc> {
    let (x, y, z, heading) = at;
    ctx.add_spawn(id, x, y, z, heading, false, 0, instance)
}

/// Java `spawnNext(npc, growthLevel, player, food)`.
fn spawn_next(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, growth_level: i32, player: ObjectId, food: i32) {
    if ctx.is_script_value(npc, 1) {
        return;
    }

    ctx.set_script_value(npc, 1);

    if !ctx.world().is_object_spawned(npc.object_id) {
        return;
    }

    let npc_id = npc.id;
    let Some(growth) = growth_capable_mob(npc_id) else {
        return;
    };

    // find the next mob to spawn, based on the current npcId, growthlevel, and food.
    let next_npc_id = if growth_level == 2 {
        // if tamed, the mob that will spawn depends on the class type (fighter/mage) of the player!
        if ctx.get_random(2) == 0 {
            if ctx.is_mage_class(player) {
                growth.get_mob(food, 1, 1)
            } else {
                growth.get_mob(food, 1, 0)
            }
        } else if ctx.get_random(5) == 0 {
            // if not tamed, there is a small chance that have "mad cow" disease.
            // that is a stronger-than-normal animal that attacks its feeder
            growth.get_mob(food, 0, 1)
        } else {
            growth.get_mob(food, 0, 0)
        }
    } else {
        // all other levels of growth are straight-forward
        growth.get_random_mob(ctx, food)
    };
    let Some(next_npc_id) = next_npc_id else {
        return;
    };

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
        // FOLLOW-UP(engine): the character's old trained beasts are deleted and
        // `new TamedBeast(nextNpcId, player, food - FOODSKILLDIFF, x, y, z)` appears here.
        check_jewel_of_innocence(ctx, player);
        // FOLLOW-UP: `Q00655_AGrandPlanForTamingWildBeasts.checkCrystalofPurity(player)`.

        // also, perform a rare random chat
        if ctx.get_random(20) == 0 {
            // Java draws the message `getRandom(2024, 2029)` for the new beast to say.
            let _message = ctx.get_random_range(2024, 2029);
        }
    } else {
        // if not trained, the newly spawned mob will automatically be aggro against its feeder
        // (what happened to "never bite the hand that feeds you" anyway?!)
        let Some(at) = at else {
            return;
        };
        let Some(next) = spawn_at(ctx, at, instance, next_npc_id) else {
            return;
        };

        if mad_cow_polymorph(next_npc_id).is_some() {
            ctx.start_quest_timer("polymorph Mad Cow", 10_000, Some(next), Some(player), false);
        }

        // register the player in the feedinfo for the mob that just spawned
        ctx.state::<FeedInfo>().0.insert(next.object_id.0, player.0);
        ctx.add_attack_desire(next, player, 99_999);
    }
}

/// Java `ai.areas.BeastFarm.FeedableBeasts`.
pub struct FeedableBeasts;

impl Script for FeedableBeasts {
    fn name(&self) -> &'static str {
        "FeedableBeasts"
    }

    fn path(&self) -> &'static str {
        "ai/areas/BeastFarm"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&FEEDABLE_BEASTS);
        s.add_skill_see_id(&FEEDABLE_BEASTS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("polymorph Mad Cow")
            && let (Some(npc), Some(player)) = (npc, player)
            && let Some(next_id) = mad_cow_polymorph(npc.id)
        {
            // remove the feed info from the previous mob
            let feed = ctx.state::<FeedInfo>();
            if feed.0.get(&npc.object_id.0) == Some(&player.0) {
                feed.0.remove(&npc.object_id.0);
            }

            let at = ctx.npc_position(npc);
            let instance = ctx.world().instance_id_of(npc.object_id);

            // despawn the mad cow
            ctx.delete_npc(npc);

            // spawn the new mob
            if let Some(at) = at
                && let Some(next) = spawn_at(ctx, at, instance, next_id)
            {
                // register the player in the feedinfo for the mob that just spawned
                ctx.state::<FeedInfo>().0.insert(next.object_id.0, player.0);
                ctx.add_attack_desire(next, player, 99_999);
            }
        }

        None
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

        // check if the npc and skills used are valid for this script. Exit if invalid.
        let skill_id = skill.id;
        if (skill_id != SKILL_GOLDEN_SPICE) && (skill_id != SKILL_CRYSTAL_SPICE) {
            return;
        }

        let npc_id = npc.id;

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

        let food = if skill_id == SKILL_GOLDEN_SPICE {
            GOLDEN_SPICE
        } else {
            // if (skillId == SKILL_CRYSTAL_SPICE)
            CRYSTAL_SPICE
        };

        // display the social action of the beast eating the food.
        ctx.broadcast_npc_social_action(npc, 2);

        // if this pet can't grow, it's all done.
        if let Some(growth_data) = growth_data {
            // Do nothing if this monster doesn't eat the specified food (food gets consumed but has no effect).
            if growth_data.get_mob(food, 0, 0).is_none() {
                return;
            }

            // rare random talk...
            if ctx.get_random(20) == 0 {
                if let Some(message) = ctx.get_random_entry(text_of(growth_level)) {
                    let name = ctx.player_name(caster);
                    // `if (message.getParamCount() > 0) packet.addStringParameter(name)`
                    if message.param_count() > 0 {
                        ctx.broadcast_npc_say(npc, ChatType::General, message, &[&name]);
                    } else {
                        ctx.broadcast_npc_say(npc, ChatType::General, message, &[]);
                    }
                }
            }

            // NOTE(java-quirk): the feeder was stored a few lines up, so this "same player as
            // the one who raised it" test can never fail.
            if (growth_level > 0) && (ctx.state::<FeedInfo>().0.get(&object_id) != Some(&caster.0)) {
                // check if this is the same player as the one who raised it from growth 0.
                // if no, then do not allow a chance to raise the pet (food gets consumed but has no effect).
                return;
            }

            // Polymorph the mob, with a certain chance, given its current growth level
            if ctx.get_random(100) < growth_data.chance {
                spawn_next(ctx, npc, growth_level, caster, food);
            }
        }
        // FOLLOW-UP(engine): `else if (TAMED_BEASTS.contains(npcId) && npc instanceof TamedBeast)`
        // feeds a tamed beast (`beast.onReceiveFood()` and its chat).
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
        <npc id="21451" type="Monster" level="80" name="Alpine Kookaburra"/>
        <npc id="21452" type="Monster" level="80" name="Alpine Kookaburra"/>
        <npc id="21453" type="Monster" level="80" name="Alpine Kookaburra"/>
        <npc id="21454" type="Monster" level="80" name="Alpine Kookaburra"/>
        <npc id="21455" type="Monster" level="80" name="Alpine Kookaburra"/>"#;

    #[test]
    fn the_growth_table_is_javas() {
        let calf = growth_capable_mob(21451).unwrap();
        assert_eq!((calf.growth_level, calf.chance), (0, 100));
        assert_eq!(calf.get_mob(GOLDEN_SPICE, 0, 0), Some(21452));
        assert_eq!(calf.get_mob(CRYSTAL_SPICE, 0, 0), Some(21456));
        assert_eq!(
            growth_capable_mob(21479).unwrap().get_mob(GOLDEN_SPICE, 1, 1),
            Some(TRAINED_BUFFALO2)
        );
        // Java's quirk: Buffalo_1_Gold_2 holds 21481 twice over (21480 is missing).
        assert_eq!(
            growth_capable_mob(21472).unwrap().get_mob(GOLDEN_SPICE, 0, 0),
            Some(21481)
        );
        assert!(
            growth_capable_mob(21451)
                .unwrap()
                .get_mob(SKILL_GOLDEN_SPICE, 0, 0)
                .is_none()
        );
        assert!(growth_capable_mob(21824).is_none());
    }

    #[test]
    fn feeding_golden_spice_grows_the_kookaburra_and_the_new_one_attacks_the_feeder() {
        let mut registry = ScriptRegistry::new();
        registry.add(FeedableBeasts);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 82, vec![]);
        let calf = t.spawn(21451, 20);
        t.world.run_as_script("FeedableBeasts", |ctx| {
            let npc = ctx.script_npc(calf).unwrap();
            FeedableBeasts.on_skill_see(
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
        let grown: Vec<ObjectId> = [21452, 21453, 21454, 21455]
            .iter()
            .flat_map(|id| t.npcs_of(*id))
            .collect();
        assert_eq!(grown.len(), 1);
        t.world.run_as_script("FeedableBeasts", |ctx| {
            assert_eq!(ctx.state::<FeedInfo>().0.get(&grown[0].0), Some(&1));
        });
    }
}
