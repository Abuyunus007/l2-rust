//! The skill and buff admin commands: `//add_skill <skill_id> [level]`, `//remove_skill <skill_id>`,
//! `//give_all_skills`, `//remove_all_skills`, `//show_skills`, `//remove_skills <page>`,
//! `//skill_list`, `//skill_race`, `//skill_class`, `//skill_add_list`, `//skill_index`,
//! `//getbuffs [name [page]]`, `//getbuffs_ps`, `//stopbuff <objectId> <skillId>`,
//! `//stopallbuffs <objectId>`, `//viewskilleffects`, `//viewblockedeffects` and
//! `//areacancel <radius>`, with the admin pages they show.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSkill.java
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminBuffs.java
// Java: gameserver/data/xml/ClassListData.java (the class names of the pages)
// Java: dist/game/data/html/admin/{charskills,charskills_add,charskills_race,charskills_class,
// Java: charskills_add_list,getbuff_skills,getbuff_effects}.htm
//
// The command words are the ones `AdminCommands.xml` and the handlers register (`admin_add_skill`
// is `//add_skill`).
//
// # Pages
// Every list page goes through [`build_page`] (Java's `PageBuilder`, seven rows per page). The
// class names come from `ClassListData` (`World::set_class_list`); a class missing from it is a
// `NullPointerException` of the command, as in Java. Java keeps the skills of a page in a
// `HashSet`/`HashMap` keyed by the identity of the `Skill` object, whose order differs from run to
// run; here a class' skills are listed by skill id and a creature's buffs in the order of its
// effect list.
//
// # Left out, with the system that brings it
// - `//add_clan_skill`, `//give_clan_skills` and `//give_all_clan_skills`: the pledge skill tree
//   comes with M6-CLAN-03, so these words are not registered yet.
// - `//switch_gm_buffs`: the GM skill trees (`SkillTreeData.getGMSkillTree`) are not loaded, so
//   the word is not registered yet.
// - `//removereuse` sends `SkillCoolTime` in Java. That server packet is not in the protocol crate
//   yet (M6 packet card): the reuse timers are cleared, the client keeps its cool-down icons until
//   the next login.
// - The `//get_skills` / `//reset_skills` stash is Java's static `adminSkills`: one per server,
//   kept in `AdminWorld::skill_stash`.
// - The `gmaudit` line of each command is written by the framework (`admin/mod.rs`); the extra
//   lines of `showBuffs`, `//stopbuff`, `//stopallbuffs` and `//viewblockedeffects` are here.
// - Shortcuts of a removed skill (`deleteShortcut`) go with `World::delete_skill_shortcuts`.

use std::fmt::Write as _;

use l2_data::{ClassId, Race, SkillTemplate};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::{info, warn};

use super::commands::java_split_space;
use super::html::{AdminHtml, build_page};
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::effects::{BuffInfo, SkillFinishType, SkillKey};
use crate::inventory::java_concurrent_map_order;
use crate::outbox::PersistCommand;
use crate::rewards::exp::SkillWrites;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// `AdminSkill.PAGE_LIMIT` and `AdminBuffs.PAGE_LIMIT`.
const PAGE_LIMIT: i32 = 7;

/// Java `AdminSkill` and `AdminBuffs` registrations (the words the command list has).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_show_skills", admin_show_skills),
        ("admin_remove_skills", admin_remove_skills),
        ("admin_skill_list", admin_skill_list),
        ("admin_skill_race", admin_skill_race),
        ("admin_skill_class", admin_skill_class),
        ("admin_skill_add_list", admin_skill_add_list),
        ("admin_skill_index", admin_skill_index),
        ("admin_add_skill", admin_add_skill),
        ("admin_remove_skill", admin_remove_skill),
        ("admin_give_all_skills", admin_give_all_skills),
        ("admin_remove_all_skills", admin_remove_all_skills),
        ("admin_getbuffs", admin_getbuffs),
        ("admin_getbuffs_ps", admin_getbuffs),
        ("admin_stopbuff", admin_stopbuff),
        ("admin_stopallbuffs", admin_stopallbuffs),
        ("admin_viewblockedeffects", admin_viewblockedeffects),
        ("admin_viewskilleffects", admin_viewskilleffects),
        ("admin_viewskilleffects_ps", admin_viewskilleffects),
        ("admin_areacancel", admin_areacancel),
        ("admin_get_skills", admin_get_skills),
        ("admin_reset_skills", admin_reset_skills),
        ("admin_give_all_skills_fs", admin_give_all_skills_fs),
        ("admin_setskill", admin_setskill),
        ("admin_removereuse", admin_removereuse),
        ("admin_buff", admin_buff),
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

/// Java `Player.getAllSkills()`: the skills of a character as `(id, level)`, in the iteration
/// order of Java's `ConcurrentHashMap` of skill ids.
fn all_skills(world: &World, player: ObjectId) -> Vec<(i32, i32)> {
    let Some(p) = world.player(player) else {
        return Vec::new();
    };
    let known = &p.core.skills;
    let keys: Vec<i32> = known.iter().map(|(id, _)| *id).collect();
    java_concurrent_map_order(&keys)
        .into_iter()
        .filter_map(|id| known.iter().find(|(s, _)| *s == id).copied())
        .collect()
}

/// Java `Player.removeSkill(skill)` (store = true) for each id: the skill leaves the creature
/// (`removeSkill(id, true)`, its effects too), its rows leave `character_skills` in one
/// `SkillChanges` command.
// `Player.removeSkill` deletes the shortcuts of each skill, one list sent per shortcut.
fn forget_skills(world: &mut World, player: ObjectId, skill_ids: &[i32]) {
    let mut removed = Vec::new();
    for &skill_id in skill_ids {
        if world.remove_skill(player, skill_id, true) {
            removed.push(skill_id);
        }
        world.delete_skill_shortcuts(player, skill_id);
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

fn null_pointer() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

// ------------------------------------------------------------------------------ classes

/// Every `PlayerClass`, in the order of the Java enum (by class id).
fn player_classes() -> impl Iterator<Item = ClassId> {
    (0u16..=136).map(ClassId).filter(|c| c.is_player_class())
}

/// Java `Enum.name()` of a playable race.
fn race_name(race: Race) -> &'static str {
    match race {
        Race::Human => "HUMAN",
        Race::Elf => "ELF",
        Race::DarkElf => "DARK_ELF",
        Race::Orc => "ORC",
        Race::Dwarf => "DWARF",
        Race::Kamael => "KAMAEL",
    }
}

/// The constants of Java's `Race` that are not playable (`Enum.valueOf` accepts them; no class has
/// them, so their pages are empty).
const OTHER_RACES: &[&str] = &[
    "ANIMAL",
    "BEAST",
    "BUG",
    "CASTLE_GUARD",
    "CONSTRUCT",
    "DEMONIC",
    "DIVINE",
    "DRAGON",
    "ELEMENTAL",
    "ETC",
    "FAIRY",
    "GIANT",
    "HUMANOID",
    "MERCENARY",
    "NONE",
    "PLANT",
    "SIEGE_WEAPON",
    "UNDEAD",
];

/// Java `Enum.valueOf(Race.class, name)`: `Some(race)` for a playable race, `None` for another
/// constant of the enum, Java's `IllegalArgumentException` for any other text.
fn race_from_name(name: &str) -> Result<(String, Option<Race>), JavaException> {
    let playable = [
        Race::Human,
        Race::Elf,
        Race::DarkElf,
        Race::Orc,
        Race::Dwarf,
        Race::Kamael,
    ];
    if let Some(race) = playable.into_iter().find(|r| race_name(*r) == name) {
        return Ok((name.to_owned(), Some(race)));
    }
    if OTHER_RACES.contains(&name) {
        return Ok((name.to_owned(), None));
    }
    Err(JavaException(format!(
        "java.lang.IllegalArgumentException: No enum constant org.l2jmobius.gameserver.entity.actor.enums.creature.Race.{name}"
    )))
}

/// Java `Enum.valueOf(PlayerClass.class, name)`.
fn class_from_name(name: &str) -> Result<ClassId, JavaException> {
    player_classes()
        .find(|c| c.enum_name() == Some(name))
        .ok_or_else(|| {
            JavaException(format!(
                "java.lang.IllegalArgumentException: No enum constant org.l2jmobius.gameserver.entity.actor.enums.player.PlayerClass.{name}"
            ))
        })
}

/// Java `StringUtil.enumToString(enum)`: `DARK_ELF` is `Dark Elf`.
fn enum_to_string(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut capitalize_next = true;
    for c in name.to_lowercase().chars() {
        if c == '_' {
            out.push(' ');
            capitalize_next = true;
        } else if capitalize_next {
            out.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// `SkillTreeData.getCompleteClassSkillTree(playerClass).isEmpty()` is false.
fn has_class_tree(world: &World, class: ClassId) -> bool {
    world
        .rewards
        .skill_trees
        .complete_class_tree(class)
        .next()
        .is_some()
}

/// Java `ClassListData.getInstance().getClass(playerClass).getClassName()`; a class the list does
/// not have is a `NullPointerException`.
pub(crate) fn class_name(world: &World, class: ClassId) -> Result<String, JavaException> {
    world
        .admin
        .class_list
        .get(class)
        .map(|c| c.name.clone())
        .ok_or_else(null_pointer)
}

/// `class_name` of the class a character plays (`player.getPlayerClass()`).
pub(crate) fn player_class_name(world: &World, player: ObjectId) -> Result<String, JavaException> {
    let class_id = world.player(player).map_or(-1, |p| p.data.class_id);
    world
        .admin
        .class_list
        .get_by_id(class_id)
        .map(|c| c.name.clone())
        .ok_or_else(null_pointer)
}

// ----------------------------------------------------------------------------- AdminSkill

/// Java `AdminSkill.showMainPage(activeChar)`: the target check, then `charskills.htm` for the
/// target.
fn show_main_page(world: &mut World, gm: ObjectId) -> Outcome {
    let Some(player) = target_player(world, gm) else {
        return Ok(());
    };
    let name = world.object_name(player).unwrap_or_default();
    let level = world.player(player).map_or(0, |p| p.data.level);
    let class = player_class_name(world, player)?;
    let mut html = world.admin_html_file(gm, "data/html/admin/charskills.htm");
    html.replace("%name%", &name);
    html.replace("%level%", &level.to_string());
    html.replace("%class%", &class);
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_show_skills`: only the exact word shows the page.
fn admin_show_skills(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_show_skills" {
        show_main_page(world, gm)?;
    }
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_remove_skills <page>`: `Integer.parseInt(command.substring(20))`,
/// a command shorter than that does nothing (`StringIndexOutOfBoundsException` is caught).
fn admin_remove_skills(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(value) = command.get(20..) else {
        return Ok(());
    };
    remove_skills_page(world, gm, parse_int(value)?)
}

/// Java `AdminSkill.removeSkillsPage(activeChar, pageValue)`: ten skills of the target per page,
/// each a link that removes it.
fn remove_skills_page(world: &mut World, gm: ObjectId, page_value: i32) -> Outcome {
    let Some(player) = world.target_of(gm).filter(|t| world.is_player(*t)) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
        return Ok(());
    };
    let skills = all_skills(world, player);
    let count = i32::try_from(skills.len()).unwrap_or(i32::MAX);
    let max_skills_per_page = 10;
    let mut max_pages = count / max_skills_per_page;
    if count > max_skills_per_page * max_pages {
        max_pages += 1;
    }
    let page = page_value.min(max_pages);
    let skills_start = max_skills_per_page.wrapping_mul(page);
    let mut skills_end = count;
    if skills_end - skills_start > max_skills_per_page {
        skills_end = skills_start + max_skills_per_page;
    }

    let name = world.object_name(player).unwrap_or_default();
    let level = world.player(player).map_or(0, |p| p.data.level);
    let class = player_class_name(world, player)?;
    let mut html = String::new();
    html.push_str("<html><body><table width=260><tr><td width=40><button value=\"Main\" action=\"bypass admin_admin\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td width=180><center>Character Selection Menu</center></td><td width=40><button value=\"Back\" action=\"bypass -h admin_show_skills\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br><br><center>Editing <font color=\"LEVEL\">");
    let _ = write!(
        html,
        "{name}</font></center><br><table width=270><tr><td>Lv: {level} {class}</td></tr></table><br><table width=270><tr><td>Note: Do not forget that modifying players skills can</td></tr><tr><td>ruin the game...</td></tr></table><br><center>Click on the skill you wish to remove:</center><br><center><table width=270><tr>"
    );
    for x in 0..max_pages {
        let _ = write!(
            html,
            "<td><a action=\"bypass -h admin_remove_skills {x}\">Page {}</a></td>",
            x + 1
        );
    }
    html.push_str("</tr></table></center><br><table width=270><tr><td width=80>Name:</td><td width=60>Level:</td><td width=40>Id:</td></tr>");
    for i in skills_start..skills_end {
        let Some(&(skill_id, skill_level)) = usize::try_from(i).ok().and_then(|at| skills.get(at)) else {
            // A negative page: Java indexes the array below 0.
            return Err(JavaException(format!(
                "java.lang.ArrayIndexOutOfBoundsException: Index {i} out of bounds for length {count}"
            )));
        };
        let skill_name =
            skill_template(world, skill_id, skill_level).map_or_else(String::new, |s| s.name.clone());
        let _ = write!(
            html,
            "<tr><td width=80><a action=\"bypass -h admin_remove_skill {skill_id}\">{skill_name}</a></td><td width=60>{skill_level}</td><td width=40>{skill_id}</td></tr>"
        );
    }
    html.push_str("</table><br><center><table>Remove skill by ID :<tr><td>Id: </td><td><edit var=\"id_to_remove\" width=110></td></tr></table></center><center><button value=\"Remove skill\" action=\"bypass -h admin_remove_skill $id_to_remove\" width=110 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center><br><center><button value=\"Back\" action=\"bypass -h admin_current_player\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>");
    world.send_admin_html(gm, &AdminHtml::from_html(&html));
    Ok(())
}

/// Java `AdminSkill.adminAddSkills(player)`: `charskills_add.htm` with the races that have a class
/// with a skill tree.
fn admin_add_skills(world: &mut World, gm: ObjectId) -> Outcome {
    let mut races: Vec<Race> = Vec::new();
    for class in player_classes() {
        if let Some(race) = class.race()
            && !races.contains(&race)
            && has_class_tree(world, class)
        {
            races.push(race);
        }
    }
    let mut list = String::new();
    for race in races {
        let name = race_name(race);
        let _ = write!(
            list,
            "<tr><td><a action=\"bypass admin_skill_race {name}\">{}</a></td></tr>",
            enum_to_string(name)
        );
    }
    let mut html = world.admin_html_file(gm, "data/html/admin/charskills_add.htm");
    html.replace("%racelist%", &list);
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_skill_list`.
fn admin_skill_list(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    admin_add_skills(world, gm)
}

/// Java `AdminSkill.onCommand`, `admin_skill_race [race]`: one word after the command is a race
/// (Java `split(" ")` length 2), anything else the race list.
fn admin_skill_race(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let split = super::commands::java_split_space(command);
    if split.len() == 2 {
        let (name, race) = race_from_name(split[1])?;
        admin_add_race_skills(world, gm, &name, race)
    } else {
        admin_add_skills(world, gm)
    }
}

/// Java `AdminSkill.adminAddRaceSkills(player, race)`: `charskills_race.htm` with the classes of
/// the race that have a skill tree, coloured by class level.
fn admin_add_race_skills(world: &mut World, gm: ObjectId, name: &str, race: Option<Race>) -> Outcome {
    let mut list = String::new();
    for class in player_classes() {
        if race.is_none() || class.race() != race || !has_class_tree(world, class) {
            continue;
        }
        let color = match class.class_level() {
            1 => "999999",
            2 => "F2B705",
            3 => "F26005",
            4 => "FF0022",
            _ => "777777",
        };
        let _ = write!(
            list,
            "<tr><td><a action=\"bypass admin_skill_class {}\"><font color=\"{color}\">{}</font></a></td></tr>",
            class.enum_name().unwrap_or_default(),
            class_name(world, class)?
        );
    }
    let mut html = world.admin_html_file(gm, "data/html/admin/charskills_race.htm");
    html.replace("%racename%", &enum_to_string(name));
    html.replace("%classlist%", &list);
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_skill_class <class> [page]`.
fn admin_skill_class(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let split = super::commands::java_split_space(command);
    if split.len() > 1 {
        let class = class_from_name(split[1])?;
        let page = match split.get(2) {
            Some(text) => parse_int(text)?,
            None => 0,
        };
        admin_add_class_skills(world, gm, class, page)
    } else {
        admin_add_skills(world, gm)
    }
}

/// `SkillData.getSkill(id, 1)` of a skill tree line; Java's `null` for a skill without that level
/// is a `NullPointerException` once the page reads its icon.
fn first_level_skill(world: &World, skill_id: i32) -> Result<&SkillTemplate, JavaException> {
    skill_template(world, skill_id, 1).ok_or_else(null_pointer)
}

/// Java `AdminSkill.adminAddClassSkills(player, playerClass, page)`: `charskills_class.htm` with the
/// skills of the class tree (each skill once), seven per page.
fn admin_add_class_skills(world: &mut World, gm: ObjectId, class: ClassId, page: i32) -> Outcome {
    // NOTE(java-quirk): Java collects the skills in a `HashSet<Skill>` (identity hash), so the
    // order is arbitrary; here it is the skill id.
    let mut ids: Vec<i32> = world
        .rewards
        .skill_trees
        .complete_class_tree(class)
        .map(|learn| learn.skill_id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    let mut skills = Vec::new();
    for id in ids {
        skills.push(first_level_skill(world, id)?);
    }

    let class_enum = class.enum_name().unwrap_or_default();
    let result = build_page(
        &skills,
        PAGE_LIMIT,
        &format!("bypass admin_skill_class {class_enum}"),
        page,
        |skill, sb| {
            // NOTE(java-quirk): `row` is never incremented, every row has the black background.
            let _ = write!(
                sb,
                "<table width=\"295\" bgcolor=\"000000\"><tr><td height=40 width=40><img src=\"{}\" width=32 height=32></td><td width=190><font color=\"B09878\"><a action=\"bypass admin_skill_add_list {class_enum} {}\">{} ({})</a></font></td></tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>",
                skill.icon, skill.id, skill.name, skill.id
            );
        },
    );
    let name = class_name(world, class)?;
    let mut html = world.admin_html_file(gm, "data/html/admin/charskills_class.htm");
    html.replace("%classname%", &name);
    html.replace("%skilllist%", &result.body);
    html.replace("%pages%", &result.pager_html());
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_skill_add_list <class> [skillId [page]]`.
fn admin_skill_add_list(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let split = super::commands::java_split_space(command);
    if split.len() > 1 {
        let class = class_from_name(split[1])?;
        let skill_id = match split.get(2) {
            Some(text) => parse_int(text)?,
            None => 1,
        };
        let page = match split.get(3) {
            Some(text) => parse_int(text)?,
            None => 0,
        };
        admin_add_class_skill_list(world, gm, class, skill_id, page)
    } else {
        admin_add_skills(world, gm)
    }
}

/// Java `AdminSkill.adminAddClassSkillList(player, playerClass, skillId, page)`:
/// `charskills_add_list.htm` with the levels of one skill of the class tree, lowest level first.
fn admin_add_class_skill_list(
    world: &mut World,
    gm: ObjectId,
    class: ClassId,
    skill_id: i32,
    page: i32,
) -> Outcome {
    // A `TreeMap<level, Skill>`: one entry per level, ordered by level.
    let mut levels: Vec<i32> = world
        .rewards
        .skill_trees
        .complete_class_tree(class)
        .filter(|learn| learn.skill_id == skill_id)
        .map(|learn| learn.skill_level)
        .collect();
    levels.sort_unstable();
    levels.dedup();
    let mut skills = Vec::new();
    for level in levels {
        skills.push(skill_template(world, skill_id, level).ok_or_else(null_pointer)?);
    }

    let class_enum = class.enum_name().unwrap_or_default();
    let result = build_page(
        &skills,
        PAGE_LIMIT,
        &format!("bypass admin_skill_add_list {class_enum} {skill_id}"),
        page,
        |skill, sb| {
            let _ = write!(
                sb,
                "<table width=\"295\" bgcolor=\"000000\"><tr><td height=40 width=40><img src=\"{}\" width=32 height=32></td><td width=190><font color=\"B09878\"><a action=\"bypass admin_add_skill {} {}\">{} ({})</a></font></td></tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>",
                skill.icon, skill.id, skill.level, skill.name, skill.level
            );
        },
    );
    let mut html = world.admin_html_file(gm, "data/html/admin/charskills_add_list.htm");
    html.replace("%skillid%", &skill_id.to_string());
    html.replace("%skilllist%", &result.body);
    html.replace("%classId%", class_enum);
    html.replace("%pages%", &result.pager_html());
    world.send_admin_html(gm, &html);
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_skill_index <page>`: `skills/<page>.htm`; a command shorter
/// than 18 characters does nothing.
fn admin_skill_index(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if let Some(value) = command.get(18..) {
        world.show_admin_html(gm, &format!("skills/{value}.htm"));
    }
    Ok(())
}

/// Java `AdminSkill.onCommand`, `admin_add_skill`: `adminAddSkill(activeChar, command.substring(15))`.
/// One or two words: the skill id and its level (1 without one). A bad number or an unknown skill
/// is "Error: there is no such skill.". Whatever the method throws (a class the list lacks, for the
/// page) is the usage line.
fn admin_add_skill(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let value = command.get(15..).unwrap_or("");
    if add_skill(world, gm, value).is_err() {
        world.send_sys_message(gm, "Usage: //add_skill <skill_id> <level>");
    }
    Ok(())
}

fn add_skill(world: &mut World, gm: ObjectId, value: &str) -> Outcome {
    let Some(player) = target_player(world, gm) else {
        // Java: `INVALID_TARGET`, then `showMainPage` says it again.
        return show_main_page(world, gm);
    };
    let words = split_words(value);
    if words.len() != 1 && words.len() != 2 {
        return show_main_page(world, gm);
    }

    let skill_id = parse_int(words[0]);
    let level = words.get(1).map_or(Ok(1), |w| parse_int(w));
    let template = match (skill_id, level) {
        (Ok(id), Ok(level)) => skill_template(world, id, level).map(|t| (t.id, t.level, t.name.clone())),
        _ => None,
    };
    let Some((skill_id, skill_level, name)) = template else {
        world.send_sys_message(gm, "Error: there is no such skill.");
        return show_main_page(world, gm);
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
    show_main_page(world, gm) // Back to start
}

/// Java `AdminSkill.onCommand`, `admin_remove_skill`: `Integer.parseInt(command.substring(19))`, then
/// `adminRemoveSkill`. A bad number is the usage line, and so is whatever the method throws.
fn admin_remove_skill(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(skill_id) = command.get(19..).and_then(|text| parse_int(text).ok()) else {
        world.send_sys_message(gm, "Usage: //remove_skill <skill_id>");
        return Ok(());
    };
    if remove_skill(world, gm, skill_id).is_err() {
        world.send_sys_message(gm, "Usage: //remove_skill <skill_id>");
    }
    Ok(())
}

fn remove_skill(world: &mut World, gm: ObjectId, skill_id: i32) -> Outcome {
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
        return remove_skills_page(world, gm, 0); // Back to previous page
    };

    world.send_message(
        player,
        &format!("Admin removed the skill {name} from your skills list."),
    );
    forget_skills(world, player, &[skill_id]);
    let player_name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(gm, &format!("You removed the skill {name} from {player_name}."));
    world.send_skill_list(gm);
    remove_skills_page(world, gm, 0) // Back to previous page
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

/// `GMAudit.logAction(gm [objectId], action, target, params)`, when `GMAudit` is on. The file
/// `log/GMAudit/<name>.txt` comes with the logging card (D-25).
fn gm_audit(world: &World, gm: ObjectId, action: &str, target: &str, params: &str) {
    if !world.settings.gm_audit {
        return;
    }
    let name = world.object_name(gm).unwrap_or_default();
    info!(target: "gmaudit", "{name} [{}]>{action}>{target}>{params}", gm.0);
}

/// `target.getName() + " (" + target.getObjectId() + ")"`, the audit target of the buff commands.
fn audit_target(world: &World, target: ObjectId) -> String {
    format!("{} ({})", world.object_name(target).unwrap_or_default(), target.0)
}

/// Java `AdminBuffs.showBuffs(activeChar, target, page, passive)`: `getbuff_skills.htm` with one row
/// per skill the creature is affected by (or has as passive), seven per page.
fn show_buffs(world: &mut World, gm: ObjectId, target: ObjectId, page: i32, passive: bool) {
    let now = world.tick;
    // `Map<Skill, Integer>`: the first info of a skill puts its remaining time, a later one of the
    // same skill the larger of its abnormal time and what is there.
    // NOTE(java-quirk): the later infos are compared by abnormal time, not by remaining time.
    let mut skills: Vec<(SkillKey, i32)> = Vec::new();
    if let Some(list) = world.effect_list(target) {
        let infos: Vec<&BuffInfo> = if passive {
            list.passives().iter().collect()
        } else {
            list.effects().collect()
        };
        for info in infos {
            let key = info.skill();
            match skills.iter_mut().find(|(k, _)| *k == key) {
                Some((_, time)) => *time = info.abnormal_time().max(*time),
                None => skills.push((key, info.time(now))),
            }
        }
    }

    let target_name = world.object_name(target).unwrap_or_default();
    let elements: Vec<(&SkillTemplate, i32)> = skills
        .iter()
        .filter_map(|(key, time)| skill_template(world, key.id, key.level).map(|s| (s, *time)))
        .collect();
    let link = format!(
        "bypass admin_getbuffs{} {target_name}",
        if passive { "_ps" } else { "" }
    );
    let result = build_page(&elements, PAGE_LIMIT, &link, page, |(skill, time), sb| {
        // NOTE(java-quirk): `row` is never incremented, every row has the black background.
        sb.push_str("<table width=\"295\" bgcolor=\"000000\">");
        let passive_skill = skill.is_passive();
        let _ = write!(
            sb,
            "<tr><td height=40 width=40><img src=\"{}\" width=32 height=32></td><td width={}><font color=\"B09878\"><a action=\"bypass admin_viewskilleffects{} {} {} {}\">{}",
            skill.icon,
            if passive_skill { "850" } else { "650" },
            if passive { "_ps" } else { "" },
            target.0,
            skill.id,
            skill.level,
            skill.name
        );
        if !skill.name.contains("Lv") && !skill.name.contains("Level") {
            let _ = write!(sb, " Lv. {}", skill.level);
        }
        let time_text = if passive_skill {
            String::new()
        } else if skill.is_toggle() {
            "T".to_owned()
        } else {
            format!("{time}s")
        };
        let _ = write!(
            sb,
            " ({})</a></font></td><td fixwidth={} height=32 valign=\"center\" align=\"center\">{time_text}</td><td><table><tr><td height=5></td></tr><tr><td fixwidth=360><button value=\"X\" action=\"bypass admin_stopbuff {} {}\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table></td></tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>",
            skill.id,
            if passive_skill { "1" } else { "200" },
            target.0,
            skill.id
        );
    });

    let mut html = world.admin_html_file(gm, "data/html/admin/getbuff_skills.htm");
    html.replace("%pages%", &result.pager_html());
    html.replace(
        "%buffsText%",
        if passive { "Hide Passives" } else { "Show Passives" },
    );
    html.replace("%passives%", if passive { "" } else { "_ps" });
    html.replace("%targetName%", &target_name);
    html.replace("%targetObjId%", &target.0.to_string());
    html.replace("%buffs%", &result.body);
    html.replace("%skillSize%", &skills.len().to_string());
    world.send_admin_html(gm, &html);

    gm_audit(world, gm, "getbuffs", &audit_target(world, target), "");
}

/// Java `AdminBuffs.onCommand`, `admin_getbuffs [name [page]]` and `admin_getbuffs_ps`: a name must
/// be an online character (else "The player X is not online."), a page must be a number (checked
/// only for an online character, as Java does); without a name the target must be a creature.
fn admin_getbuffs(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    let word = st.next_token().unwrap_or_default();
    let passive = word.ends_with("_ps");
    if let Some(name) = st.next_token() {
        let Some(player) = world.player_by_name(name) else {
            world.send_sys_message(gm, &format!("The player {name} is not online."));
            return Ok(());
        };
        let page = match st.next_token() {
            // Java `Integer.parseInt(page)` is not caught: it is the exception of the command.
            Some(page) => parse_int(page)?,
            None => 0,
        };
        show_buffs(world, gm, player, page, passive);
        return Ok(());
    }
    let Some(target) = target_creature(world, gm) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
        return Ok(());
    };
    show_buffs(world, gm, target, 0, passive);
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
/// that has them, then the buff page of the creature. A missing object or a skill id of 0 or less
/// does nothing.
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
    show_buffs(world, gm, target, 0, false);
    gm_audit(
        world,
        gm,
        "stopbuff",
        &audit_target(world, target),
        &skill_id.to_string(),
    );
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

/// Java `AdminBuffs.removeAllBuffs(activeChar, objId)`: `stopAllEffects()` of the creature, then its
/// buff page.
fn remove_all_buffs(world: &mut World, gm: ObjectId, object_id: i32) {
    let Some(target) = creature_by_id(world, object_id) else {
        return;
    };
    world.stop_all_effects(target);
    let name = world.object_name(target).unwrap_or_default();
    world.send_sys_message(gm, &format!("Removed all effects from {name} ({object_id})"));
    show_buffs(world, gm, target, 0, false);
    gm_audit(world, gm, "stopallbuffs", &audit_target(world, target), "");
}

/// Java `AdminBuffs.onCommand`, `admin_viewblockedeffects <objectId>` and `viewBlockedEffects`.
fn admin_viewblockedeffects(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    match int_arg(&mut st) {
        Ok(object_id) => view_blocked_effects(world, gm, object_id),
        Err(message) => {
            world.send_sys_message(gm, &format!("Failed viewing blocked effects: {message}"));
            world.send_sys_message(gm, "Usage: //viewblockedeffects <objectId>");
        }
    }
    Ok(())
}

/// Java `AdminBuffs.viewBlockedEffects(activeChar, objId)`: the abnormal types the creature is
/// protected from, as a small page built in code.
fn view_blocked_effects(world: &mut World, gm: ObjectId, object_id: i32) {
    let Some(object) = world.find_object(ObjectId(object_id)) else {
        world.send_sys_message(gm, &format!("Target with object id {object_id} not found."));
        return;
    };
    // `asCreature()` of another object is `null`, and Java does nothing for it.
    let Some(target) = creature_by_id(world, object.0) else {
        return;
    };
    let name = world.object_name(target).unwrap_or_default();
    let blocked: Vec<&'static str> = world
        .effect_list(target)
        .map(|list| list.blocked_abnormal_types().iter().map(|t| t.name()).collect())
        .unwrap_or_default();

    let mut html = String::from(
        "<html><table width=\"100%\"><tr><td width=45><button value=\"Main\" action=\"bypass admin_admin\" width=45 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td width=180><center><font color=\"LEVEL\">Blocked effects of ",
    );
    let back = if world.is_player(target) {
        format!(" {name}")
    } else {
        String::new()
    };
    let _ = write!(
        html,
        "{name}</font></td><td width=45><button value=\"Back\" action=\"bypass admin_getbuffs{back}\" width=45 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br>"
    );
    if !blocked.is_empty() {
        html.push_str("<br>Blocked buff slots: ");
        for slot in blocked {
            let _ = write!(html, "<br>{slot}");
        }
    }
    html.push_str("</html>");
    world.send_admin_html(gm, &AdminHtml::from_html(&html));
    gm_audit(world, gm, "viewblockedeffects", &audit_target(world, target), "");
}

/// Java `AdminBuffs.onCommand`, `admin_viewskilleffects[_ps] <objectId> <skillId> <skillLevel>
/// [page]`: every failure is the same two lines.
fn admin_viewskilleffects(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    let args = int_arg(&mut st)
        .and_then(|object| int_arg(&mut st).map(|skill| (object, skill)))
        .and_then(|(object, skill)| int_arg(&mut st).map(|level| (object, skill, level)))
        .and_then(|(object, skill, level)| {
            let page = if st.has_more() { int_arg(&mut st)? } else { 0 };
            Ok((object, skill, level, page))
        });
    let result = args.and_then(|(object, skill, level, page)| {
        view_skill_effects(world, gm, object, skill, level, page, command.contains("_ps"))
    });
    if let Err(message) = result {
        world.send_sys_message(gm, &format!("Failed viewing skill effects: {message}"));
        world.send_sys_message(gm, "Usage: //viewskilleffects <objectId skillId skillLevel>");
    }
    Ok(())
}

/// Java `AdminBuffs.viewSkillEffects(...)`: `getbuff_effects.htm` with one row per effect of the
/// skill on the creature. `Err` is the message of an exception Java throws.
fn view_skill_effects(
    world: &mut World,
    gm: ObjectId,
    object_id: i32,
    skill_id: i32,
    skill_level: i32,
    page: i32,
    passive: bool,
) -> Result<(), String> {
    let Some(object) = world.find_object(ObjectId(object_id)) else {
        world.send_sys_message(gm, &format!("Target with object id {object_id} not found."));
        return Ok(());
    };
    // `asCreature()` of another object is `null`; the `NullPointerException` of its effect list.
    // NOTE: the JVM's helpful message is not reproduced.
    let Some(target) = creature_by_id(world, object.0) else {
        return Err("null".to_owned());
    };

    let now = world.tick;
    let mut count = 0i32;
    // (skill, in use, remaining seconds, effect names)
    let mut rows: Vec<(&SkillTemplate, bool, i32, Vec<String>)> = Vec::new();
    if let Some(list) = world.effect_list(target) {
        let infos: Vec<&BuffInfo> = if passive {
            list.passives().iter().collect()
        } else {
            list.effects().collect()
        };
        for info in infos {
            let key = info.skill();
            if key.id != skill_id || key.level != skill_level {
                continue;
            }
            let Some(skill) = skill_template(world, key.id, key.level) else {
                continue;
            };
            count = count.saturating_add(i32::try_from(info.effects().len()).unwrap_or(i32::MAX));
            // `effect.getClass().getSimpleName()` is the `name` of the effect element.
            let names = info
                .effects()
                .iter()
                .map(|e| {
                    skill
                        .effects
                        .get(e.scope)
                        .get(e.index)
                        .map_or_else(String::new, |t| t.name.clone())
                })
                .collect();
            rows.push((skill, info.is_in_use(), info.time(now), names));
        }
    }

    let link = format!(
        "bypass admin_viewskilleffects{} {object_id} {skill_id} {skill_level}",
        if passive { "_ps" } else { "" }
    );
    let result = build_page(
        &rows,
        PAGE_LIMIT,
        &link,
        page,
        |(skill, in_use, time, names), sb| {
            for effect in names {
                // NOTE(java-quirk): `row` is never incremented, every row has the black background.
                sb.push_str("<table width=\"295\" bgcolor=\"000000\"><tr><td width=750>");
                if !in_use {
                    sb.push_str("<font color=\"FF0000\">");
                }
                let time_text = if skill.is_toggle() {
                    "T".to_owned()
                } else if skill.is_passive() {
                    "P".to_owned()
                } else {
                    format!("{time}s")
                };
                let _ = write!(sb, "{} Lv. {} ({effect})", skill.name, skill.level);
                if !in_use {
                    sb.push_str("</font>");
                }
                let _ = write!(
                    sb,
                    "</td><td width=180><center>{time_text}</center></td><td width=200><button value=\"X\" action=\"bypass admin_stopbuff {object_id} {}\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>",
                    skill.id
                );
            }
        },
    );

    let target_name = world.object_name(target).unwrap_or_default();
    let mut html = world.admin_html_file(gm, "data/html/admin/getbuff_effects.htm");
    html.replace("%pages%", &result.pager_html());
    html.replace(
        "%buffsText%",
        if passive { "Hide Passives" } else { "Show Passives" },
    );
    html.replace("%passives%", if passive { "" } else { "_ps" });
    html.replace("%targetName%", &target_name);
    html.replace("%targetObjId%", &target.0.to_string());
    html.replace("%buffs%", &result.body);
    html.replace("%effectSize%", &count.to_string());
    world.send_admin_html(gm, &html);
    Ok(())
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

// ---------------------------------------------------------- the GM's skill swap and buffs

/// `learn` for each `(id, level)`: Java `addSkill(skill, true)`, the rows in one command.
fn learn_all(world: &mut World, player: ObjectId, skills: &[(i32, i32)]) {
    let mut writes = SkillWrites::default();
    for &(skill_id, level) in skills {
        world.learn_skill(player, skill_id, level, &mut writes);
    }
    world.flush_skill_writes(player, writes);
}

/// Java `AdminSkill.onCommand`, `admin_get_skills`: the GM takes the target's skills. The GM's own
/// skills are kept for `//reset_skills`; the target keeps its skills.
fn admin_get_skills(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(player) = target_player(world, gm) else {
        return Ok(());
    };
    if world.object_name(player) == world.object_name(gm) {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouCannotUseThisOnYourself),
        );
    } else {
        let theirs = all_skills(world, player);
        let own = all_skills(world, gm);
        let own_ids: Vec<i32> = own.iter().map(|(id, _)| *id).collect();
        // Java overwrites `adminSkills` each time, even when a reset has not run yet.
        world.admin.skill_stash = Some(own);
        forget_skills(world, gm, &own_ids);
        learn_all(world, gm, &theirs);
        let name = world.object_name(player).unwrap_or_default();
        world.send_sys_message(gm, &format!("You now have all the skills of {name}."));
        world.send_skill_list(gm);
    }
    show_main_page(world, gm)
}

/// Java `AdminSkill.onCommand`, `admin_reset_skills`: the target's skills become the GM's, and the
/// GM gets his own skills back from the stash of `//get_skills`.
fn admin_reset_skills(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(player) = target_player(world, gm) else {
        return Ok(());
    };
    let Some(stash) = world.admin.skill_stash.take() else {
        world.send_sys_message(gm, "You must get the skills of someone in order to do this.");
        return show_main_page(world, gm);
    };
    let their_ids: Vec<i32> = all_skills(world, player).iter().map(|(id, _)| *id).collect();
    forget_skills(world, player, &their_ids);
    learn_all(world, player, &all_skills(world, gm));
    forget_skills(world, gm, &their_ids);
    learn_all(world, gm, &stash);

    let gm_name = world.object_name(gm).unwrap_or_default();
    world.send_message(player, &format!("[GM]{gm_name} updated your skills."));
    world.send_sys_message(gm, "You now have all your skills back.");
    world.send_skill_list(gm);
    world.send_skill_list(player);
    show_main_page(world, gm)
}

/// Java `AdminSkill.onCommand`, `admin_give_all_skills_fs`: `giveAvailableSkills(true, true, true)`
/// on the target, the Forgotten Scroll skills included.
fn admin_give_all_skills_fs(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let Some(player) = target_player(world, gm) else {
        return Ok(());
    };
    let mut writes = SkillWrites::default();
    let count = world.give_available_skills(player, true, true, true, &mut writes);
    world.flush_skill_writes(player, writes);
    let player_name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(gm, &format!("You gave {count} skills to {player_name}"));
    world.send_skill_list(player);
    Ok(())
}

/// The word at `index` of Java's `command.split(" ")`; a missing one is Java's
/// `ArrayIndexOutOfBoundsException`, reported by the dispatcher.
fn split_word<'a>(words: &[&'a str], index: usize) -> Result<&'a str, JavaException> {
    words.get(index).copied().ok_or_else(|| {
        JavaException(format!(
            "java.lang.ArrayIndexOutOfBoundsException: Index {index} out of bounds for length {}",
            words.len()
        ))
    })
}

/// Java `AdminSkill.onCommand`, `admin_setskill <id> <level>`: the GM himself learns the skill
/// (`addSkill(skill)`, not stored), then his skill list. An unknown skill is Java's
/// `NullPointerException` (`addSkill(null)`).
fn admin_setskill(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let id = parse_int(split_word(&words, 1)?)?;
    let level = parse_int(split_word(&words, 2)?)?;
    let Some((skill_id, skill_level, name)) =
        skill_template(world, id, level).map(|t| (t.id, t.level, t.name.clone()))
    else {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    };
    world.add_skill(gm, skill_id, skill_level);
    world.send_skill_list(gm);
    world.send_sys_message(
        gm,
        &format!("You added yourself skill {name}({id}) level {level}"),
    );
    Ok(())
}

/// Java `AdminBuffs.onCommand`, `admin_removereuse [name]`: the reuse timers and the disabled
/// skills of the named player (online) or the target are cleared.
// M6: Java sends `SkillCoolTime(player)` here; the packet is not in the protocol crate yet.
fn admin_removereuse(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token(); // the command word
    let player = match st.next_token() {
        Some(name) => match world.player_by_name(name) {
            Some(player) => player,
            None => {
                world.send_sys_message(gm, &format!("The player {name} is not online."));
                return Ok(());
            }
        },
        None => match world.target_of(gm).filter(|t| world.is_player(*t)) {
            Some(player) => player,
            None => {
                world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
                return Ok(());
            }
        },
    };
    world.reset_time_stamps(player);
    world.reset_disabled_skills(player);
    let name = world.object_name(player).unwrap_or_default();
    world.send_sys_message(gm, &format!("Skill reuse was removed from {name}."));
    Ok(())
}

/// Java `AdminBuffs.onCommand`, `admin_buff <skillId> [skillLevel]`: the skill's effects land on
/// the target creature from the GM. Without a level the skill's highest level is used.
fn admin_buff(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(target) = target_creature(world, gm) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
        return Ok(());
    };
    let mut st = Tokens::spaces(command);
    st.next_token(); // the command word
    if !st.has_more() {
        world.send_sys_message(gm, "Skill Id and level are not specified.");
        world.send_sys_message(gm, "Usage: //buff <skillId> <skillLevel>");
        return Ok(());
    }
    let skill_id = match int_arg(&mut st) {
        Ok(skill_id) => skill_id,
        Err(message) => return buff_failed(world, gm, &message),
    };
    let level = if st.has_more() {
        match int_arg(&mut st) {
            Ok(level) => level,
            Err(message) => return buff_failed(world, gm, &message),
        }
    } else {
        max_skill_level(world, skill_id)
    };
    let Some((template_id, template_level, name)) =
        skill_template(world, skill_id, level).map(|t| (t.id, t.level, t.name.clone()))
    else {
        world.send_sys_message(
            gm,
            &format!("Skill with id: {skill_id}, level: {level} not found."),
        );
        return Ok(());
    };
    world.send_sys_message(gm, &format!("Admin buffing {name} ({skill_id},{level})"));
    world.apply_effects(gm, target, SkillKey::new(template_id, template_level));
    Ok(())
}

/// Java `catch (Exception e)` of `admin_buff`: the message, then the usage line.
fn buff_failed(world: &mut World, gm: ObjectId, message: &str) -> Outcome {
    world.send_sys_message(gm, &format!("Failed buffing: {message}"));
    world.send_sys_message(gm, "Usage: //buff <skillId> <skillLevel>");
    Ok(())
}

/// Java `SkillData.getMaxLevel(skillId)`: 0 for an unknown skill.
fn max_skill_level(world: &World, skill_id: i32) -> i32 {
    world
        .data
        .stat_data()
        .map_or(0, |data| data.skills.max_level(skill_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_to_string_is_java_enum_to_string() {
        assert_eq!(enum_to_string("HUMAN"), "Human");
        assert_eq!(enum_to_string("DARK_ELF"), "Dark Elf");
        assert_eq!(enum_to_string("SIEGE_WEAPON"), "Siege Weapon");
    }

    #[test]
    fn race_and_class_names_follow_enum_value_of() {
        assert_eq!(
            race_from_name("DARK_ELF").unwrap(),
            ("DARK_ELF".to_owned(), Some(Race::DarkElf))
        );
        assert_eq!(race_from_name("UNDEAD").unwrap().1, None);
        assert_eq!(
            race_from_name("Elf").unwrap_err().0,
            "java.lang.IllegalArgumentException: No enum constant org.l2jmobius.gameserver.entity.actor.enums.creature.Race.Elf"
        );
        assert_eq!(class_from_name("GLADIATOR").unwrap(), ClassId(2));
        assert_eq!(class_from_name("JUDICATOR").unwrap(), ClassId(136));
        assert!(class_from_name("gladiator").is_err());
        assert_eq!(player_classes().count(), 103);
    }
}
