//! `//skill_test <skillId>`: the caster (the target creature, or the GM) shows the cast of a level 1
//! skill toward the GM, as the client draws it. The skill is not used.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminTest.java
//
// # Left out
// - `doCast` (the other branch of `adminTestSkill`): Java only reaches it with `msu` false, which
//   `admin_skill_test` never passes, so it is not ported.

use l2_protocol_game::server::{MagicSkillUse, SkillPosition};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminTest.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_skill_test", admin_skill_test)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

const USAGE: &str = "Command format is //skill_test <ID>";

/// Java `AdminTest.onCommand`: the id of the second token. A missing or bad id, and a missing
/// target (Java's `target.isCreature()` on null), is the usage line.
fn admin_skill_test(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut tokens = Tokens::new(command);
    tokens.next_token();
    let Some(id) = tokens.next_token().and_then(|v| parse_int(v).ok()) else {
        world.send_sys_message(gm, USAGE);
        return Ok(());
    };
    // `target.isCreature()` with no target is a `NullPointerException`: the usage line.
    let Some(target) = world.target_of(gm) else {
        world.send_sys_message(gm, USAGE);
        return Ok(());
    };
    let caster = if world.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
        target
    } else {
        gm
    };
    let Some((hit_time, reuse_delay)) = skill_timing(world, id, 1) else {
        return Ok(());
    };
    world.set_target(caster, Some(gm));
    let at = |id: ObjectId| {
        world
            .position_of(id)
            .map(|(x, y, z)| SkillPosition { x, y, z })
            .unwrap_or(SkillPosition { x: 0, y: 0, z: 0 })
    };
    // `new MagicSkillUse(caster, activeChar, id, 1, hitTime, reuseDelay)`: the caster's place, the
    // GM as the target.
    let packet = MagicSkillUse {
        creature_id: caster.0,
        target_id: gm.0,
        skill_id: id,
        skill_level: 1,
        hit_time,
        reuse_delay,
        creature: at(caster),
        ground_location: None,
        target: at(gm),
    };
    world.broadcast_creature_packet(caster, &packet);
    Ok(())
}

/// Java `SkillData.getSkill(id, level)` (a level above the highest one gives the highest one): the
/// hit time and the reuse delay of the skill.
fn skill_timing(world: &World, skill_id: i32, level: i32) -> Option<(i32, i32)> {
    let skills = &world.data.stat_data()?.skills;
    let template = skills.get(skill_id, level).or_else(|| {
        let max = skills.max_level(skill_id);
        (max > 0 && level > max)
            .then(|| skills.get(skill_id, max))
            .flatten()
    })?;
    Some((template.hit_time, template.reuse_delay))
}
