//! The GM's own commands and the small target ones: `//superhaste <0-4> [seconds]` (and `//speed`),
//! `//hide on|off`, `//target <name>`, `//targetsay <text>`, `//gmspeed <0-10>` and
//! `//switch_gm_buffs`, with their menu words.
//!
//! Not modelled: the silence mode of `//hide` (`Player.setSilenceMode`, the refusal of private
//! messages), so `//hide` does not set it.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminSuperHaste,AdminHide,
// Java: AdminTarget,AdminTargetSay}.java, entity/actor/Player.java (setHiding)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, CreatureSay, SayBody, SenderField, SystemMessage};
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, parse_int};
use crate::effects::{SkillFinishType, SkillKey};
use crate::objects::ObjectKind;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminSuperHaste.SUPER_HASTE_ID`.
const SUPER_HASTE_ID: i32 = 7029;

/// Java `AdminSuperHaste`, `AdminHide`, `AdminTarget` and `AdminTargetSay`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_superhaste", admin_superhaste),
        ("admin_superhaste_menu", admin_superhaste_menu),
        ("admin_speed", admin_superhaste),
        ("admin_speed_menu", admin_superhaste_menu),
        ("admin_hide", admin_hide),
        ("admin_target", admin_target),
        ("admin_targetsay", admin_targetsay),
        ("admin_gmspeed", admin_gmspeed),
        ("admin_switch_gm_buffs", admin_switch_gm_buffs),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `AdminSuperHaste.onCommand`, `admin_superhaste <level 0-4> [seconds]` (and `admin_speed`).
/// The old haste goes first; a level of 1 to 4 gives the new one, for `seconds` (0 when missing).
/// Anything that does not parse, and a level without a skill template (Java's `NullPointerException`),
/// is the usage text.
fn admin_superhaste(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let usage = "Usage: //superhaste <Effect level (0-4)> <Time in seconds>";
    let Some(level) = words.get(1).and_then(|w| parse_int(w).ok()) else {
        world.send_message(gm, usage);
        return Ok(());
    };
    // Java: `isAffectedBySkill(...)` decides REMOVED (a message) only for level 0.
    let was_affected = world.is_affected_by_skill(gm, SUPER_HASTE_ID);
    let finish = if level == 0 && was_affected {
        SkillFinishType::Removed
    } else {
        SkillFinishType::Normal
    };
    world.stop_skill_effects(gm, finish, SUPER_HASTE_ID);
    if (1..=4).contains(&level) {
        let time = match words.get(2) {
            Some(text) => match parse_int(text) {
                Ok(time) => time,
                Err(_) => {
                    world.send_message(gm, usage);
                    return Ok(());
                }
            },
            None => 0,
        };
        let has_template = world
            .data
            .stat_data()
            .is_some_and(|s| s.skills.get(SUPER_HASTE_ID, level).is_some());
        if !has_template {
            world.send_message(gm, usage);
            return Ok(());
        }
        world.apply_effects_timed(gm, gm, SkillKey::new(SUPER_HASTE_ID, level), true, time);
    }
    Ok(())
}

/// Java `AdminSuperHaste`, `admin_superhaste_menu` and `admin_speed_menu`: `gm_menu.htm`.
fn admin_superhaste_menu(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.show_admin_html(gm, "gm_menu.htm");
    Ok(())
}

/// Java `Player.setHiding(hide)`: `false` (and nothing changes) when the GM is not a GM or already
/// is in that state. Silence mode is not modelled (see the module comment).
fn set_hiding(world: &mut World, gm: ObjectId, hide: bool) -> bool {
    if !world.is_gm(gm) {
        return false;
    }
    let invisible = world.objects.get(&gm).is_some_and(|e| e.invisible);
    if invisible && hide {
        return false;
    }
    if !invisible && !hide {
        return false;
    }
    world.set_invul(gm, hide);
    world.set_invisible(gm, hide);
    if hide {
        // Java: the players who target the hidden GM lose the target.
        for player in world.visible_players(gm) {
            if world.target_of(player) == Some(gm) {
                world.set_target(player, None);
            }
        }
    }
    world.broadcast_user_info(gm);
    true
}

/// Java `AdminHide.onCommand`, `admin_hide on|off`.
fn admin_hide(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let usage = "//hide [on|off]";
    match words.get(1).copied() {
        Some("on") => {
            if set_hiding(world, gm, true) {
                world.send_sys_message(gm, "Now, you cannot be seen.");
            } else {
                world.send_sys_message(gm, "Currently, you cannot be seen.");
            }
        }
        Some("off") => {
            if set_hiding(world, gm, false) {
                world.send_sys_message(gm, "Now, you can be seen.");
            } else {
                world.send_sys_message(gm, "Currently, you can be seen.");
            }
        }
        _ => world.send_sys_message(gm, usage),
    }
    Ok(())
}

/// Java `AdminTarget.handleTarget`, `admin_target <name>`: the named player is clicked as if the
/// GM had clicked it. Java's `substring(13)` fails for a command without the name.
fn admin_target(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.chars().count() < 13 {
        world.send_sys_message(gm, "Please specify correct name.");
        return Ok(());
    }
    let name: String = command.chars().skip(13).collect();
    match world.player_by_name(&name) {
        Some(player) => world.on_action_object(gm, player, true),
        None => world.send_sys_message(gm, &format!("Player {name} not found")),
    }
    Ok(())
}

/// Java `AdminTargetSay.onCommand`, `admin_targetsay <text>`: the target says the text to the
/// players around it, as a general chat line for a player and an NPC chat line otherwise.
// NOTE(java-quirk): with no target at all Java's `obj.isCreature()` is a NullPointerException, which
// the framework reports to the GM; the text of that exception is not reproduced exactly here.
fn admin_targetsay(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let Some(obj) = world.target_of(gm) else {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    };
    let kind = world.objects.get(&obj).map(|e| e.kind);
    if kind == Some(ObjectKind::StaticObject) || !kind.is_some_and(|k| k.is_creature()) {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return Ok(());
    }
    if command.chars().count() < 16 {
        world.send_sys_message(gm, "Usage: //targetsay <text>");
        return Ok(());
    }
    let message: String = command.chars().skip(16).collect();
    let name = world.object_name(obj).unwrap_or_default();
    let chat_type = if kind == Some(ObjectKind::Player) {
        ChatType::General
    } else {
        ChatType::NpcGeneral
    };
    world.broadcast_creature_packet(
        obj,
        &CreatureSay {
            sender_object_id: obj.0,
            chat_type,
            sender: SenderField::Name(name),
            message_id: -1,
            body: SayBody::Text(message),
        },
    );
    Ok(())
}

/// Java `AdminBuffs.onCommand`, `admin_switch_gm_buffs`: the GM gets the other set of GM skills
/// (the aura set when it has the aura skill 7041, else the normal set), and the skill list is sent.
/// Java does not save these skills to the database, and neither does this.
fn admin_switch_gm_buffs(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    if world.settings.gm_give_special_skills == world.settings.gm_give_special_aura_skills {
        world.send_sys_message(gm, "There is nothing to switch.");
        return Ok(());
    }
    let to_aura = world.known_skill(gm, 7041).is_some();
    switch_skills(world, gm, to_aura);
    world.send_skill_list(gm);
    let kind = if to_aura { "aura" } else { "one" };
    world.send_sys_message(
        gm,
        &format!("You have succefully changed to target {kind} special skills."),
    );
    Ok(())
}

/// Java `SkillTreeData.switchSkills(gmchar, toAuraSkills)`: the skills of the set being left go,
/// then the set of `toAuraSkills` is added.
fn switch_skills(world: &mut World, gm: ObjectId, to_aura: bool) {
    let trees = std::sync::Arc::clone(&world.rewards.skill_trees);
    let (leave, enter) = if to_aura {
        (trees.game_master_skills(), trees.game_master_aura_skills())
    } else {
        (trees.game_master_aura_skills(), trees.game_master_skills())
    };
    for learn in leave {
        world.remove_skill(gm, learn.skill_id, false);
    }
    for learn in enter {
        world.add_skill(gm, learn.skill_id, learn.skill_level);
    }
}

impl World {
    /// Java `CreatureStat.getGmSpeedMultiplier` source: the multiplier `//gmspeed` set, 1 otherwise.
    pub fn gm_speed_multiplier(&self, id: ObjectId) -> f64 {
        self.admin.gm_speed.get(&id).copied().unwrap_or(1.0)
    }
}

/// Java `AdminGmSpeed.onCommand`, `admin_gmspeed <0-10>`: the GM (or the targeted creature) moves
/// faster by that factor; 0 is the normal speed. With `UseSuperHasteAsGMSpeed` the command is
/// `admin_superhaste <token>` instead.
// NOTE(java-quirk): Java prints the percentage as `(value * 100) + "0"`, so the text is
// `[200.00]%` for 2; the number is printed the way Java prints a double.
fn admin_gmspeed(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = java_split_space(command);
    let usage = "//gmspeed [0...10]";
    let Some(token) = words.get(1).copied() else {
        world.send_sys_message(gm, usage);
        return Ok(());
    };
    if world.settings.use_super_haste_as_gm_speed {
        world.admin_command(gm, &format!("admin_superhaste {token}"), false);
        return Ok(());
    }
    // GUESS: Java's `StringUtil.isDouble` is read as Rust's f64 parse.
    let Ok(boost) = token.parse::<f64>() else {
        world.send_sys_message(gm, usage);
        return Ok(());
    };
    if !(0.0..=10.0).contains(&boost) {
        world.send_sys_message(gm, usage);
        return Ok(());
    }
    let target = world
        .target_of(gm)
        .filter(|t| world.objects.get(t).is_some_and(|e| e.kind.is_creature()))
        .unwrap_or(gm);
    let multiplier = if boost > 0.0 { boost } else { 1.0 };
    world.admin.gm_speed.insert(target, multiplier);
    if world.is_player(target) {
        world.broadcast_user_info(target);
    } else {
        world.broadcast_info(target);
    }
    let name = world.object_name(target).unwrap_or_default();
    world.send_sys_message(gm, &format!("[{name}] speed is [{:?}0]% fast.", boost * 100.0));
    Ok(())
}
