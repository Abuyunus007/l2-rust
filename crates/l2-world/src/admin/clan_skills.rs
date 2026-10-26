//! The clan skill admin commands: `//add_clan_skill <skill_id> <level>` gives one clan skill to the
//! clan of the targeted clan leader, `//give_clan_skills` and `//give_all_clan_skills [squad]` give
//! every clan skill the clan's level allows.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSkill.java (adminGiveClanSkills,
// Java: adminAddClanSkill), data/xml/SkillTreeData.java (getMaxPledgeSkills)

use std::collections::BTreeMap;

use l2_data::SkillLearn;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::skills::show_main_page;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::clan::model::CLAN_SKILL_SUB_TYPE;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminSkill.ADMIN_COMMANDS`, the clan words.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_add_clan_skill", admin_add_clan_skill),
        ("admin_give_clan_skills", admin_give_clan_skills),
        ("admin_give_all_clan_skills", admin_give_all_clan_skills),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The target when it is a player, else Java's `INVALID_TARGET` and `None`.
fn target_player(world: &World, gm: ObjectId) -> Option<ObjectId> {
    let target = world.target_of(gm).filter(|t| world.is_player(*t));
    if target.is_none() {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
    }
    target
}

/// Java `S1_IS_NOT_A_CLAN_LEADER` with the player's name, to the GM.
fn not_a_clan_leader(world: &World, gm: ObjectId, player: ObjectId) {
    let name = world.object_name(player).unwrap_or_default();
    world.send(
        gm,
        &SystemMessage::new(SystemMessageId::S1IsNotAClanLeader).with(SystemMessageParam::Text(name)),
    );
}

/// Java `Clan.getSkills().get(id)`'s level, `None` when the clan has no such skill.
fn clan_level_of(world: &World, clan_id: i32, skill_id: i32) -> Option<i32> {
    world.clan(clan_id).and_then(|clan| {
        clan.skills
            .iter()
            .find(|(id, _)| *id == skill_id)
            .map(|(_, level)| *level)
    })
}

/// Java `SkillTreeData.getMaxPledgeSkills(clan, includeSquad)`: per skill id, the last line of the
/// trees that the clan's level allows and that is above what the clan has.
// GUESS: Java keeps the lines in a `HashMap` keyed by the skill id, so with two lines of one id the
// one put last wins; here the order is the tree's order, and the map is sorted by id.
fn max_pledge_skills(world: &World, clan_id: i32, include_squad: bool) -> BTreeMap<i32, SkillLearn> {
    let mut result = BTreeMap::new();
    let Some(level) = world.clan(clan_id).map(|c| c.level) else {
        return result;
    };
    let trees = std::sync::Arc::clone(&world.rewards.skill_trees);
    for skill in trees.pledge_skills() {
        if !skill.residence_skill && level >= skill.get_level {
            let old = clan_level_of(world, clan_id, skill.skill_id);
            if old.is_none_or(|old| old < skill.skill_level) {
                result.insert(skill.skill_id, skill.clone());
            }
        }
    }
    if include_squad {
        for skill in trees.sub_pledge_skills() {
            if level >= skill.get_level {
                let old = clan_level_of(world, clan_id, skill.skill_id);
                if old.is_none_or(|old| old < skill.skill_level) {
                    result.insert(skill.skill_id, skill.clone());
                }
            }
        }
    }
    result
}

/// Java `Clan.broadcastToOnlineMembers(new PledgeSkillList(clan))` and `member.sendSkillList()` for
/// each online member.
fn announce_clan_skills(world: &mut World, clan_id: i32) {
    if let Some(clan) = world.clan(clan_id) {
        let list = world.pledge_skill_list(clan);
        world.broadcast_to_clan(clan_id, &list);
    }
    for member in world.clan_online_members(clan_id, 0) {
        world.send_skill_list(member);
    }
}

/// Java `AdminSkill.adminAddClanSkill`, `admin_add_clan_skill <skill_id> <level>`. A bad or
/// missing number is the usage text (and no page).
fn admin_add_clan_skill(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let val = java_split_space(command);
    let parsed = match (val.get(1), val.get(2)) {
        (Some(id), Some(level)) => parse_int(id).ok().zip(parse_int(level).ok()),
        _ => None,
    };
    let Some((id, level)) = parsed else {
        world.send_sys_message(gm, "Usage: //add_clan_skill <skill_id> <level>");
        return Ok(());
    };
    let Some(player) = target_player(world, gm) else {
        show_main_page(world, gm)?;
        return Ok(());
    };
    if !world.is_clan_leader(player) {
        not_a_clan_leader(world, gm, player);
        show_main_page(world, gm)?;
        return Ok(());
    }
    if !(370..=391).contains(&id) || !(1..=3).contains(&level) {
        world.send_sys_message(gm, "Usage: //add_clan_skill <skill_id> <level>");
        show_main_page(world, gm)?;
        return Ok(());
    }
    let template = world.data.stat_data().and_then(|s| s.skills.get(id, level));
    let Some(template) = template else {
        world.send_sys_message(gm, "Error: there is no such skill.");
        return Ok(());
    };
    let skill_name = template.name.clone();
    let clan_id = world.clan_id_of(player);
    let announce = SystemMessage::new(SystemMessageId::TheClanSkillS1HasBeenAdded)
        .with(SystemMessageParam::SkillName { id, level });
    world.send(player, &announce);
    world.broadcast_to_clan(clan_id, &announce);
    world.clan_add_new_skill(clan_id, id, level, CLAN_SKILL_SUB_TYPE);
    let clan_name = world.clan(clan_id).map(|c| c.name.clone()).unwrap_or_default();
    world.send_sys_message(
        gm,
        &format!("You gave the Clan Skill: {skill_name} to the clan {clan_name}."),
    );
    announce_clan_skills(world, clan_id);
    show_main_page(world, gm)?;
    Ok(())
}

/// Java `AdminSkill.adminGiveClanSkills(activeChar, includeSquad)`. A player who is not the clan
/// leader is reported to the GM, and the skills are given anyway (Java has no `return` there).
fn give_clan_skills(world: &mut World, gm: ObjectId, include_squad: bool) {
    let Some(player) = target_player(world, gm) else {
        return;
    };
    let clan_id = world.clan_id_of(player);
    if clan_id == 0 || world.clan(clan_id).is_none() {
        world.send(
            gm,
            &SystemMessage::new(SystemMessageId::TheTargetMustBeAClanMember),
        );
        return;
    }
    if !world.is_clan_leader(player) {
        not_a_clan_leader(world, gm, player);
    }
    let skills = max_pledge_skills(world, clan_id, include_squad);
    for learn in skills.values() {
        // `clan.addNewSkill(SkillData.getSkill(id, level))`: the clan's own list (`-2`).
        world.clan_add_new_skill(clan_id, learn.skill_id, learn.skill_level, CLAN_SKILL_SUB_TYPE);
    }
    announce_clan_skills(world, clan_id);
    let count = skills.len();
    let player_name = world.object_name(player).unwrap_or_default();
    let clan_name = world.clan(clan_id).map(|c| c.name.clone()).unwrap_or_default();
    world.send_sys_message(
        gm,
        &format!("You gave {count} skills to {player_name}'s clan {clan_name}."),
    );
    world.send_message(player, &format!("Your clan received {count} skills."));
}

/// Java `admin_give_clan_skills`: the clan's own tree.
fn admin_give_clan_skills(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    give_clan_skills(world, gm, false);
    Ok(())
}

/// Java `admin_give_all_clan_skills`: the clan's tree and the squad tree.
fn admin_give_all_clan_skills(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    give_clan_skills(world, gm, true);
    Ok(())
}
