//! The newbie helper NPC bypasses `SupportMagic`, `SupportMagicServitor` and `GiveBlessing`.
// Java: dist/game/data/scripts/handlers/bypass/npc/SupportMagic.java
// Java: dist/game/data/scripts/handlers/bypass/npc/SupportBlessing.java
//
// The NPC casts the skills itself ([`World::set_npc_target`] + [`World::do_cast`]), as Java's
// `npc.setTarget(..)` + `npc.doCast(..)`.

use l2_data::ClassId;
use l2_util::ObjectId;

use crate::bypass::BypassHandlers;
use crate::effects::SkillKey;
use crate::world::World;

/// Registers the handlers of this module (called by `BypassHandlers::default`).
pub(crate) fn register(handlers: &mut BypassHandlers) {
    handlers.register("supportmagicservitor", support_magic_handler);
    handlers.register("supportmagic", support_magic_handler);
    handlers.register("GiveBlessing", support_blessing_handler);
}

const fn skill(id: i32) -> SkillKey {
    SkillKey { id, level: 1 }
}

const HASTE_1: SkillKey = skill(4327);
const HASTE_2: SkillKey = skill(5632);
const CUBIC: SkillKey = skill(4338);
/// Java `CommonSkill.BLESSING_OF_PROTECTION`.
const BLESSING_OF_PROTECTION: SkillKey = skill(5182);

const FIGHTER_BUFFS: [SkillKey; 6] = [
    skill(4322),
    skill(4323),
    skill(5637),
    skill(4324),
    skill(4325),
    skill(4326),
];
const MAGE_BUFFS: [SkillKey; 7] = [
    skill(4322),
    skill(4323),
    skill(5637),
    skill(4328),
    skill(4329),
    skill(4330),
    skill(4331),
];
const SUMMON_BUFFS: [SkillKey; 10] = [
    skill(4322),
    skill(4323),
    skill(5637),
    skill(4324),
    skill(4325),
    skill(4326),
    skill(4328),
    skill(4329),
    skill(4330),
    skill(4331),
];

const LOWEST_LEVEL: i32 = 6;
const HIGHEST_LEVEL: i32 = 75;
const CUBIC_LOWEST: i32 = 16;
const CUBIC_HIGHEST: i32 = 34;
const HASTE_LEVEL_2: i32 = 40;

/// `CategoryData` `BEGINNER_MAGE` (`CategoryData.xml`): the first and second mage classes of
/// every race.
pub(crate) fn is_beginner_mage(class_id: i32) -> bool {
    matches!(class_id, 10..=17 | 25..=30 | 38..=43)
}

/// Java `getPlayerClass().level()`.
fn class_level(class_id: i32) -> i32 {
    u16::try_from(class_id).map_or(0, |c| ClassId(c).class_level())
}

/// Java `SupportMagic.onCommand`.
fn support_magic_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) else {
        return false;
    };
    if world.is_cursed_weapon_equipped(player) {
        return false;
    }
    if command.eq_ignore_ascii_case("supportmagicservitor") {
        make_support_magic(world, player, npc, true);
    } else if command.eq_ignore_ascii_case("supportmagic") {
        make_support_magic(world, player, npc, false);
    }
    true
}

/// Java `SupportMagic.makeSupportMagic`.
fn make_support_magic(world: &mut World, player: ObjectId, npc: ObjectId, is_summon: bool) {
    if is_summon && !world.has_servitor(player) {
        world.npc_show_chat_window_file(npc, player, "data/html/default/SupportMagicNoSummon.htm");
        return;
    }
    let Some((level, class_id)) = world.players.get(&player).map(|p| (p.core.level, p.data.class_id)) else {
        return;
    };
    if level > HIGHEST_LEVEL {
        world.npc_show_chat_window_file(npc, player, "data/html/default/SupportMagicHighLevel.htm");
        return;
    }
    if level < LOWEST_LEVEL {
        world.npc_show_chat_window_file(npc, player, "data/html/default/SupportMagicLowLevel.htm");
        return;
    }
    if class_level(class_id) == 3 {
        // Custom message in Java.
        world.send_message(
            player,
            "Only adventurers who have not completed their 3rd class transfer may receive these buffs.",
        );
        return;
    }

    let haste = if level >= HASTE_LEVEL_2 { HASTE_2 } else { HASTE_1 };
    if is_summon {
        let summon = world.summon_of(player);
        world.set_npc_target(npc, summon);
        for buff in SUMMON_BUFFS {
            world.do_cast(npc, buff);
        }
        world.do_cast(npc, haste);
    } else {
        world.set_npc_target(npc, Some(player));
        if is_beginner_mage(class_id) {
            for buff in MAGE_BUFFS {
                world.do_cast(npc, buff);
            }
        } else {
            for buff in FIGHTER_BUFFS {
                world.do_cast(npc, buff);
            }
            world.do_cast(npc, haste);
        }
        if (CUBIC_LOWEST..=CUBIC_HIGHEST).contains(&level) {
            world.do_simultaneous_cast(player, CUBIC);
        }
    }
}

/// Java `SupportBlessing.onCommand`.
fn support_blessing_handler(world: &mut World, player: ObjectId, _command: &str, target: Option<ObjectId>) -> bool {
    let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) else {
        return false;
    };
    let Some((level, class_id)) = world.players.get(&player).map(|p| (p.core.level, p.data.class_id)) else {
        return false;
    };
    // If the player is too high level, display a message and return.
    if level > 39 || class_level(class_id) >= 2 {
        world.npc_show_chat_window_file(npc, player, "data/html/default/SupportBlessingHighLevel.htm");
        return true;
    }
    world.set_npc_target(npc, Some(player));
    world.do_cast(npc, BLESSING_OF_PROTECTION);
    // NOTE(java-quirk): the handler answers `false` after the cast.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beginner_mage_category() {
        assert!(is_beginner_mage(10));
        assert!(is_beginner_mage(17));
        assert!(!is_beginner_mage(18));
        assert!(!is_beginner_mage(0));
        assert!(is_beginner_mage(43));
        assert!(!is_beginner_mage(44));
    }

    #[test]
    fn class_levels() {
        assert_eq!(class_level(0), 0);
        assert_eq!(class_level(1), 1);
        assert_eq!(class_level(88), 2);
        assert_eq!(class_level(112), 3);
    }
}
