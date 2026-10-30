//! Dragon Valley AI: the Necromancer's ghosts, the Drakos' assassins, the herbs of spoiled
//! monsters and the party's Morale Boost.
// Java: dist/game/data/scripts/ai/areas/DragonValley/DragonValley.java

use l2_util::ObjectId;
use l2_world::script::player_class;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const NECROMANCER_OF_THE_VALLEY: i32 = 22858;
const EXPLODING_ORC_GHOST: i32 = 22818;
/// Java `SPAWN_ANIMATION`: Scorpion Bones, Drakos Assassin, Parasitic Leech.
const SPAWN_ANIMATION: [i32; 3] = [22826, 22823, 22828];
const WRATHFUL_ORC_GHOST: i32 = 22819;
const DRAKOS_ASSASSIN: i32 = 22823;
const SUMMON_NPC: [i32; 3] = [
    22822, // Drakos Warrior
    22824, // Drakos Guardian
    22862, // Drakos Hunter
];
const SPOIL_REACT_MONSTER: [i32; 16] = [
    22822, // Drakos Warrior
    22823, // Drakos Assassin
    22824, // Drakos Guardian
    22825, // Giant Scorpion Bones
    22826, // Scorpion Bones
    22827, // Batwing Drake
    22828, // Parasitic Leech
    22829, // Emerald Drake
    22830, // Gem Dragon
    22831, // Dragon Tracker of the Valley
    22832, // Dragon Scout of the Valley
    22833, // Sand Drake Tracker
    22834, // Dust Dragon Tracker
    22860, // Hungry Parasitic Leech
    22861, // Hard Scorpion Bones
    22862, // Drakos Hunter
];

// Items
const GREATER_HERB_OF_MANA: i32 = 8604;
const SUPERIOR_HERB_OF_MANA: i32 = 8605;

// Skills
const SELF_DESTRUCTION: SkillKey = SkillKey { id: 6850, level: 1 };
const MORALE_BOOST1: SkillKey = SkillKey { id: 6885, level: 1 };
const MORALE_BOOST2: SkillKey = SkillKey { id: 6885, level: 2 };
const MORALE_BOOST3: SkillKey = SkillKey { id: 6885, level: 3 };

// Misc
const MIN_DISTANCE: f64 = 1500.0;
const MIN_MEMBERS: usize = 3;
const MIN_LEVEL: i32 = 80;
const CLASS_LEVEL: i32 = 3;

/// Java `CLASS_POINTS` (`EnumMap<PlayerClass, Double>`), by class id.
fn class_points(class_id: i32) -> Option<f64> {
    Some(match class_id {
        player_class::ADVENTURER => 0.2,
        player_class::ARCANA_LORD => 1.5,
        player_class::ARCHMAGE => 0.3,
        player_class::CARDINAL => -0.6,
        player_class::DOMINATOR => 0.2,
        player_class::DOOMBRINGER => 0.2,
        player_class::DOOMCRYER => 0.1,
        player_class::DREADNOUGHT => 0.7,
        player_class::DUELIST => 0.2,
        player_class::ELEMENTAL_MASTER => 1.4,
        player_class::EVA_SAINT => -0.6,
        player_class::EVA_TEMPLAR => 0.8,
        player_class::FEMALE_SOUL_HOUND => 0.4,
        player_class::FORTUNE_SEEKER => 0.9,
        player_class::GHOST_HUNTER => 0.2,
        player_class::GHOST_SENTINEL => 0.2,
        player_class::GRAND_KHAVATARI => 0.2,
        player_class::HELL_KNIGHT => 0.6,
        player_class::HIEROPHANT => 0.0,
        player_class::JUDICATOR => 0.1,
        player_class::MOONLIGHT_SENTINEL => 0.2,
        player_class::MAESTRO => 0.7,
        player_class::MALE_SOUL_HOUND => 0.4,
        player_class::MYSTIC_MUSE => 0.3,
        player_class::PHOENIX_KNIGHT => 0.6,
        player_class::SAGITTARIUS => 0.2,
        player_class::SHILLIEN_SAINT => -0.6,
        player_class::SHILLIEN_TEMPLAR => 0.8,
        player_class::SOULTAKER => 0.3,
        player_class::SPECTRAL_DANCER => 0.4,
        player_class::SPECTRAL_MASTER => 1.4,
        player_class::STORM_SCREAMER => 0.3,
        player_class::SWORD_MUSE => 0.4,
        player_class::TITAN => 0.3,
        player_class::TRICKSTER => 0.5,
        player_class::WIND_RIDER => 0.2,
        _ => return None,
    })
}

/// Java `ai.areas.DragonValley.DragonValley`.
pub struct DragonValley;

/// Java `attacker.getSummon()` when `isSummon`, else the attacker.
fn playable_of(ctx: &ScriptCtx<'_>, player: ObjectId, is_summon: bool) -> Option<ObjectId> {
    if is_summon {
        ctx.summon_of(player)
    } else {
        Some(player)
    }
}

fn manage_morale_boost(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc) {
    let mut points = 0.0_f64;
    let mut morale_boost_lv = 0;
    let members = ctx.world().party_members_of(player);
    if members.len() >= MIN_MEMBERS {
        for member in &members {
            if (ctx.player_level(*member) >= MIN_LEVEL)
                && (ctx.player_class_level(*member) >= CLASS_LEVEL)
                && ctx
                    .distance_3d(npc.object_id, *member)
                    .is_some_and(|d| d < MIN_DISTANCE)
            {
                // NOTE(java-quirk): `CLASS_POINTS.get(..)` of a class that is not in the map
                // would throw (unboxing null); every third-profession class is in it.
                points += class_points(ctx.player_class_id(*member)).unwrap_or(0.0);
            }
        }

        if points >= 3.0 {
            morale_boost_lv = 3;
        } else if points >= 2.0 {
            morale_boost_lv = 2;
        } else if points >= 1.0 {
            morale_boost_lv = 1;
        }

        for member in members {
            if ctx
                .distance_3d(npc.object_id, member)
                .is_some_and(|d| d < MIN_DISTANCE)
            {
                let skill = match morale_boost_lv {
                    1 => MORALE_BOOST1,
                    2 => MORALE_BOOST2,
                    3 => MORALE_BOOST3,
                    _ => continue,
                };
                ctx.world_mut().apply_effects(member, member, skill);
            }
        }
    }
}

fn spawn_ghost(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId, is_summon: bool, chance: i32) {
    if (ctx.get_script_value(npc) < 2) && (ctx.get_random(100) < chance) {
        let mut val = ctx.get_script_value(npc);
        let attacker = playable_of(ctx, player, is_summon);
        let Some((x, y, z, heading)) = ctx.npc_position(npc) else {
            return;
        };
        if let Some(ghost1) = ctx.add_spawn(EXPLODING_ORC_GHOST, x, y, z + 20, heading, false, 0, 0) {
            if let Some(attacker) = attacker {
                ctx.world_mut()
                    .npc_variables_mut(ghost1.object_id)
                    .set_int("playable", attacker.0);
                ctx.add_attack_desire(ghost1, attacker, 999);
            }
        }
        val += 1;
        if (val < 2) && ctx.get_random_boolean() {
            if let Some(ghost2) = ctx.add_spawn(WRATHFUL_ORC_GHOST, x, y, z + 20, heading, false, 0, 0) {
                if let Some(attacker) = attacker {
                    ctx.add_attack_desire(ghost2, attacker, 999);
                }
            }
            val += 1;
        }

        ctx.set_script_value(npc, val);
    }
}

impl Script for DragonValley {
    fn name(&self) -> &'static str {
        "DragonValley"
    }

    fn path(&self) -> &'static str {
        "ai/areas/DragonValley"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[NECROMANCER_OF_THE_VALLEY]);
        s.add_attack_id(&SUMMON_NPC);
        s.add_kill_id(&[NECROMANCER_OF_THE_VALLEY]);
        s.add_kill_id(&SPOIL_REACT_MONSTER);
        s.add_spawn_id(&[EXPLODING_ORC_GHOST, NECROMANCER_OF_THE_VALLEY]);
        s.add_spawn_id(&SPOIL_REACT_MONSTER);
        s.add_spell_finished_id(&[EXPLODING_ORC_GHOST]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "SELF_DESTRUCTION"
            && let Some(npc) = npc
            && !ctx.is_dead(npc.object_id)
        {
            let playable = ctx
                .world()
                .npc_variables(npc.object_id)
                .map(|v| v.get_int("playable", 0));
            let playable = playable.filter(|id| *id != 0).map(ObjectId);
            if let Some(playable) = playable {
                if ctx
                    .distance_3d(npc.object_id, playable)
                    .is_some_and(|d| d < 250.0)
                {
                    ctx.disable_core_ai(npc, true);
                    ctx.world_mut().do_cast(npc.object_id, SELF_DESTRUCTION);
                } else {
                    ctx.start_quest_timer("SELF_DESTRUCTION", 3000, Some(npc), None, false);
                }
            }
        }

        None
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
        if npc.id == NECROMANCER_OF_THE_VALLEY {
            spawn_ghost(ctx, npc, attacker, is_summon, 1);
        } else if (ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc) / 2))
            && (ctx.get_random(100) < 5)
            && ctx.is_script_value(npc, 0)
        {
            ctx.set_script_value(npc, 1);
            let rnd = ctx.get_random_range(3, 5);
            for _ in 0..rnd {
                let playable = playable_of(ctx, attacker, is_summon);
                let Some((x, y, z, heading)) = ctx.npc_position(npc) else {
                    return;
                };
                let minion = ctx.add_spawn(DRAKOS_ASSASSIN, x, y, z + 20, heading, true, 0, 0);
                if let (Some(minion), Some(playable)) = (minion, playable) {
                    ctx.add_attack_desire(minion, playable, 999);
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        if npc.id == NECROMANCER_OF_THE_VALLEY {
            spawn_ghost(ctx, npc, killer, is_summon, 20);
        } else if ctx.world().is_spoiled(npc.object_id) {
            let herb = ctx.get_random_range(GREATER_HERB_OF_MANA, SUPERIOR_HERB_OF_MANA);
            ctx.drop_item(npc, Some(killer), herb, 1);
            manage_morale_boost(ctx, killer, npc);
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().set_on_kill_delay(npc.object_id, 0);
        if npc.id == EXPLODING_ORC_GHOST {
            ctx.start_quest_timer("SELF_DESTRUCTION", 3000, Some(npc), None, false);
        } else if SPAWN_ANIMATION.contains(&npc.id) {
            // `npc.setShowSummonAnimation(true)` (S-API-05).
            ctx.world_mut().set_npc_show_summon_animation(npc.object_id, true);
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if skill == SELF_DESTRUCTION {
            ctx.world_mut().do_die(npc.object_id, player);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="22858" type="Monster" level="80" name="Necromancer of the Valley"/>
        <npc id="22818" type="Monster" level="80" name="Exploding Orc Ghost"/>
        <npc id="22819" type="Monster" level="80" name="Wrathful Orc Ghost"/>"#;

    #[test]
    fn killing_the_necromancer_can_call_up_ghosts() {
        let mut registry = ScriptRegistry::new();
        registry.add(DragonValley);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        // Seeded RNG: a 20% chance per Necromancer, at most two ghosts (script value 2).
        let mut kills = 0;
        while t.npcs_of(EXPLODING_ORC_GHOST).is_empty() && kills < 100 {
            let necro = t.spawn(NECROMANCER_OF_THE_VALLEY, 20);
            t.kill(1, necro);
            kills += 1;
        }
        let ghosts = t.npcs_of(EXPLODING_ORC_GHOST);
        assert!(!ghosts.is_empty());
        // The ghost has the killer marked (`getVariables().set("playable", ..)`).
        assert_eq!(
            t.world.npc_variables(ghosts[0]).unwrap().get_int("playable", 0),
            1
        );
    }

    #[test]
    fn the_class_points_are_java_s() {
        assert_eq!(class_points(player_class::ARCANA_LORD), Some(1.5));
        assert_eq!(class_points(player_class::CARDINAL), Some(-0.6));
        assert_eq!(class_points(player_class::FIGHTER), None);
    }
}
