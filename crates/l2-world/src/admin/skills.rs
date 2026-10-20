//! The skill and buff admin commands: `//add_skill <skill_id> [level]`, `//remove_skill <skill_id>`,
//! `//give_all_skills`, `//remove_all_skills`, `//skill_list`, `//getbuffs [name [page]]`,
//! `//stopbuff <objectId> <skillId>`, `//stopallbuffs <objectId>` and `//areacancel <radius>`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSkill.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminBuffs.java
//
// The command words are the ones `AdminCommands.xml` and the handlers register (`admin_add_skill`
// is `//add_skill`).
//
// # Left out, with the system that brings it
// - The admin pages. `//skill_list` shows `charskills_add.htm` (the races of the classes with a
//   skill tree, from `ClassListData`); `//add_skill`, `//remove_skill` and `//getbuffs` end with
//   the page of the target (`showMainPage`, `removeSkillsPage`, `showBuffs`). `ClassListData` is
//   not in the static data and there is no `PageBuilder` (pager) yet, so no page is sent. The
//   checks that come before each page are as Java.
// - `//add_clan_skill` and `//give_clan_skills`: clans are M6.
// - `//stopallbuffs` has `confirmDlg="true"` in `AdminCommands.xml`. The framework holds it back
//   until the ConfirmDlg card; the handler here is the part that runs after the dialog.
// - `//give_all_skills_fs`, `//setskill`, `//get_skills`, `//reset_skills`, `//buff`,
//   `//getbuffs_ps`, `//viewskilleffects`, `//viewblockedeffects`, `//removereuse` and
//   `//switch_gm_buffs` are not in this card.
// - The `gmaudit` line of each command is written by the framework (`admin/mod.rs`).
// - Shortcuts of a removed skill (`deleteShortcut`) are not modelled, as in `rewards/exp.rs`.

use l2_data::SkillTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::warn;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::effects::SkillFinishType;
use crate::outbox::PersistCommand;
use crate::rewards::exp::SkillWrites;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminSkill` and `AdminBuffs` registrations (the words the command list has).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_add_skill", admin_add_skill),
        ("admin_remove_skill", admin_remove_skill),
        ("admin_skill_list", admin_skill_list),
        ("admin_give_all_skills", admin_give_all_skills),
        ("admin_remove_all_skills", admin_remove_all_skills),
        ("admin_getbuffs", admin_getbuffs),
        ("admin_stopbuff", admin_stopbuff),
        ("admin_stopallbuffs", admin_stopallbuffs),
        ("admin_areacancel", admin_areacancel),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The target of a command that needs a character (`target == null || !target.isPlayer()`): the
/// target, or `INVALID_TARGET` (and `None`).
fn target_player(world: &World, gm: ObjectId) -> Option<ObjectId> {
    let target = world.target_of(gm).filter(|t| world.is_player(*t));
    if target.is_none() {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
    }
    target
}

/// Java `AdminSkill.showMainPage(activeChar)`: the target check, then the page of the target.
// FOLLOW-UP(M5-ADMIN-04): the page `data/html/admin/charskills.htm` (`%class%` needs `ClassListData`).
fn show_main_page(world: &World, gm: ObjectId) {
    let _ = target_player(world, gm);
}

/// Java `new StringTokenizer(text)` as a list of words.
fn split_words(text: &str) -> Vec<&str> {
    let mut st = Tokens::new(text);
    let mut out = Vec::new();
    while let Some(word) = st.next_token() {
        out.push(word);
    }
    out
}

/// Java `Integer.parseInt(st.nextToken())` in a `catch (Exception e)` that prints `e.getMessage()`:
/// `null` for a missing token (`NoSuchElementException` has no message), else the message of the
/// `NumberFormatException`.
fn int_arg(st: &mut Tokens<'_>) -> Result<i32, String> {
    let Some(token) = st.next_token() else {
        return Err("null".to_owned());
    };
    parse_int(token).map_err(|_| format!("For input string: \"{token}\""))
}

/// Java `SkillData.getSkill(id, level)`: a level above the highest one of the skill gives the
/// highest one.
fn skill_template(world: &World, skill_id: i32, level: i32) -> Option<&SkillTemplate> {
    let skills = &world.data.stat_data()?.skills;
    skills.get(skill_id, level).or_else(|| {
        let max = skills.max_level(skill_id);
        (max > 0 && level > max)
            .then(|| skills.get(skill_id, max))
            .flatten()
    })
}

/// The level the character knows the skill at, 0 for none (`getSkillLevel`).
fn known_level(world: &World, id: ObjectId, skill_id: i32) -> i32 {
    world.player(id).map_or(0, |p| {
        p.core
            .skills
            .iter()
            .find(|(s, _)| *s == skill_id)
            .map_or(0, |(_, level)| *level)
    })
}

/// Java `Player.removeSkill(skill)` (store = true) for each id: the skill leaves the creature
/// (`removeSkill(id, true)`, its effects too), its rows leave `character_skills` in one
/// `SkillChanges` command.
// NOTE(java-quirk): Java also deletes the shortcuts of each skill; shortcuts are not modelled yet.
fn forget_skills(world: &mut World, player: ObjectId, skill_ids: &[i32]) {
    let mut removed = Vec::new();
    for &skill_id in skill_ids {
        if world.remove_skill(player, skill_id, true) {
            removed.push(skill_id);
        }
    }
    if removed.is_empty() {
        return;
    }
    if let Some(p) = world.players.get_mut(&player) {
        p.data.skills.retain(|s| !removed.contains(&s.skill_id));
    }
    world.outbox.persist(PersistCommand::SkillChanges {
        owner: player,
        upserts: Vec::new(),
        removed,
    });
}

/// `World.findObject(objectId).asCreature()` for a command's object id: a creature in the world,
/// else `None`.
fn creature_by_id(world: &World, object_id: i32) -> Option<ObjectId> {
    let id = ObjectId(object_id);
    world
        .find_object(id)
        .filter(|o| world.objects.get(o).is_some_and(|e| e.kind.is_creature()))
}

/// Java `target.isCreature()` for the target of the GM.
fn target_creature(world: &World, gm: ObjectId) -> Option<ObjectId> {
    world
        .target_of(gm)
        .filter(|t| world.objects.get(t).is_some_and(|e| e.kind.is_creature()))
}

// ----------------------------------------------------------------------------- AdminSkill

/// Java `AdminSkill.onCommand`, `admin_add_skill`: `adminAddSkill(activeChar, command.substring(15))`.
/// One or two words: the skill id and its level (1 without one). A bad number or an unknown skill
/// is "Error: there is no such skill.".
fn admin_add_skill(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let value = command.get(15..).unwrap_or("");
    let Some(player) = target_player(world, gm) else {
        // Java: `INVALID_TARGET`, then `showMainPage` says it again.
        show_main_page(world, gm);
        return Ok(());
    };
    let words = split_words(value);
    if words.len() != 1 && words.len() != 2 {
        show_main_page(world, gm);
        return Ok(());
    }

    let skill_id = parse_int(words[0]);
    let level = words.get(1).map_or(Ok(1), |w| parse_int(w));
    let template = match (skill_id, level) {
        (Ok(id), Ok(level)) => skill_template(world, id, level).map(|t| (t.id, t.level, t.name.clone())),
        _ => None,
    };
    let Some((skill_id, skill_level, name)) = template else {
        world.send_sys_message(gm, "Error: there is no such skill.");
        show_main_page(world, gm);
        return Ok(());
    };

    // Player's info, then the admin's.
    world.send_message(player, &format!("Admin gave you the skill {name}."));
    let mut writes = SkillWrites::default();
    if !world.learn_skill(player, skill_id, skill_level, &mut writes) {
        warn!("{gm}: add_skill could not add skill {skill_id}/{skill_level} to {player}");
    }
    world.flush_skill_writes(player, writes);
    world.send_skill_list(player);
    let player_name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(gm, &format!("You gave the skill {name} to {player_name}."));
    world.send_skill_list(gm);
    show_main_page(world, gm);
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_remove_skill`: `Integer.parseInt(command.substring(19))`, then
/// `adminRemoveSkill`. A bad number is the usage line.
fn admin_remove_skill(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(skill_id) = command.get(19..).and_then(|text| parse_int(text).ok()) else {
        world.send_sys_message(gm, "Usage: //remove_skill <skill_id>");
        return Ok(());
    };
    let Some(player) = target_player(world, gm) else {
        return Ok(());
    };
    // `SkillData.getSkill(idval, player.getSkillLevel(idval))`: nothing for a skill not known.
    let level = known_level(world, player, skill_id);
    let name = if level > 0 {
        skill_template(world, skill_id, level).map(|t| t.name.clone())
    } else {
        None
    };
    let Some(name) = name else {
        world.send_sys_message(gm, "Error: there is no such skill.");
        // Java: `removeSkillsPage(activeChar, 0)`, the page of the target: FOLLOW-UP.
        return Ok(());
    };

    world.send_message(
        player,
        &format!("Admin removed the skill {name} from your skills list."),
    );
    forget_skills(world, player, &[skill_id]);
    let player_name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(gm, &format!("You removed the skill {name} from {player_name}."));
    world.send_skill_list(gm);
    // Java: `removeSkillsPage(activeChar, 0)`: FOLLOW-UP (the page of the target).
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_skill_list`: `adminAddSkills`, the race list page. The page
/// is FOLLOW-UP (`ClassListData`), so nothing is sent yet.
// FOLLOW-UP(M5-ADMIN-04): `data/html/admin/charskills_add.htm` with `%racelist%`.
fn admin_skill_list(_world: &mut World, _gm: ObjectId, _command: &str) -> Outcome {
    Ok(())
}

/// Java `AdminSkill.adminGiveAllSkills(activeChar, false, false)`: `giveAvailableSkills(false, true,
/// false)` on the target, which sends the player the number of new skills.
fn admin_give_all_skills(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(player) = target_player(world, gm) else {
        return Ok(());
    };
    let mut writes = SkillWrites::default();
    let count = world.give_available_skills(player, false, true, false, &mut writes);
    world.flush_skill_writes(player, writes);
    let player_name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(gm, &format!("You gave {count} skills to {player_name}"));
    world.send_skill_list(player);
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_remove_all_skills`: every skill of the target goes.
fn admin_remove_all_skills(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(player) = target_player(world, gm) else {
        return Ok(());
    };
    let known: Vec<i32> = world
        .player(player)
        .map(|p| p.core.skills.iter().map(|(id, _)| *id).collect())
        .unwrap_or_default();
    forget_skills(world, player, &known);
    let player_name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(gm, &format!("You have removed all skills from {player_name}."));
    world.send_message(player, "Admin removed all skills from you.");
    world.send_skill_list(player);
    world.broadcast_user_info(player);
    Ok(())
}

// ------------------------------------------------------------------------------ AdminBuffs

/// Java `AdminBuffs.onCommand`, `admin_getbuffs [name [page]]`: a name must be an online character
/// (else "The player X is not online."), a page must be a number (checked only for an online
/// character, as Java does); without a name the target must be a creature.
fn admin_getbuffs(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    if let Some(name) = st.next_token() {
        if world.player_by_name(name).is_none() {
            world.send_sys_message(gm, &format!("The player {name} is not online."));
            return Ok(());
        }
        if let Some(page) = st.next_token() {
            // Java `Integer.parseInt(page)` is not caught: it is the exception of the command.
            parse_int(page)?;
        }
        // FOLLOW-UP(M5-ADMIN-04): `showBuffs` (`data/html/admin/getbuff_skills.htm`, `PageBuilder`).
        return Ok(());
    }
    if target_creature(world, gm).is_none() {
        world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
        return Ok(());
    }
    // FOLLOW-UP(M5-ADMIN-04): `showBuffs` of the target.
    Ok(())
}

/// Java `AdminBuffs.onCommand`, `admin_stopbuff <objectId> <skillId>` and `removeBuff`.
fn admin_stopbuff(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let args = int_arg(&mut st).and_then(|object| int_arg(&mut st).map(|skill| (object, skill)));
    match args {
        Ok((object_id, skill_id)) => remove_buff(world, gm, object_id, skill_id),
        Err(message) => {
            world.send_sys_message(gm, &format!("Failed removing effect: {message}"));
            world.send_sys_message(gm, "Usage: //stopbuff <objectId> <skillId>");
        }
    }
    Ok(())
}

/// Java `AdminBuffs.removeBuff(activeChar, objId, skillId)`: the skill's effects go from a creature
/// that has them. A missing object or a skill id of 0 or less does nothing.
fn remove_buff(world: &mut World, gm: ObjectId, object_id: i32, skill_id: i32) {
    let Some(target) = creature_by_id(world, object_id) else {
        return;
    };
    if skill_id <= 0 {
        return;
    }
    if world.is_affected_by_skill(target, skill_id) {
        world.stop_skill_effects(target, SkillFinishType::Removed, skill_id);
        let name = world.object_name(target).unwrap_or_default();
        world.send_sys_message(
            gm,
            &format!("Removed skill ID: {skill_id} effects from {name} ({object_id})."),
        );
    }
    // FOLLOW-UP(M5-ADMIN-04): `showBuffs` of the target after the change.
}

/// Java `AdminBuffs.onCommand`, `admin_stopallbuffs <objectId>` and `removeAllBuffs`.
fn admin_stopallbuffs(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    match int_arg(&mut st) {
        Ok(object_id) => remove_all_buffs(world, gm, object_id),
        Err(message) => {
            world.send_sys_message(gm, &format!("Failed removing all effects: {message}"));
            world.send_sys_message(gm, "Usage: //stopallbuffs <objectId>");
        }
    }
    Ok(())
}

/// Java `AdminBuffs.removeAllBuffs(activeChar, objId)`: `stopAllEffects()` of the creature.
fn remove_all_buffs(world: &mut World, gm: ObjectId, object_id: i32) {
    let Some(target) = creature_by_id(world, object_id) else {
        return;
    };
    world.stop_all_effects(target);
    let name = world.object_name(target).unwrap_or_default();
    world.send_sys_message(gm, &format!("Removed all effects from {name} ({object_id})"));
    // FOLLOW-UP(M5-ADMIN-04): `showBuffs` of the target after the change.
}

/// Java `AdminBuffs.onCommand`, `admin_areacancel <radius>`: `stopAllEffects()` of every player
/// within the radius of the GM (not the GM).
fn admin_areacancel(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let Some(val) = st.next_token() else {
        world.send_sys_message(gm, "Usage: //areacancel <radius>");
        return Ok(());
    };
    let Ok(radius) = parse_int(val) else {
        world.send_sys_message(gm, "Invalid radius value.");
        return Ok(());
    };
    let players: Vec<ObjectId> = world
        .visible_objects_in_range(gm, radius)
        .into_iter()
        .filter(|o| world.is_player(*o))
        .collect();
    for player in &players {
        world.stop_all_effects(*player);
    }
    world.send_sys_message(
        gm,
        &format!(
            "Effects canceled for {} player(s) within radius {radius}",
            players.len()
        ),
    );
    Ok(())
}
