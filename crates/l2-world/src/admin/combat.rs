//! The admin commands that hurt or revive: `//res`, `//res_monster`, `//kill`, `//kill_monster`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminRes,AdminKill}.java
//
// `//kill` is a real death (M4-DEATH-01): the target loses `maxHp + 1` (a player `maxHp + maxCp +
// 1`) through `reduceCurrentHp`, so the death chain of `death.rs` runs, the corpse and the rewards
// hooks included. `//res` is `doResurrect`: a dead creature comes back, a player gets back the exp
// it lost (M4-EXP-01 hook).
//
// # Left out, with the system that brings it
// - `stopAllEffects()` for a non-GM player: effects are M5.
// - Champions (`ChampionMonstersConfig`): not rolled, so the champion branch of `kill` is not taken.
// - `AdminKill.kill`'s invulnerability switch around the hit (`setInvul(false)` and back): the
//   invulnerability state is not ported yet (`//invul`, see the M4-ADMIN-02 report).

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::debug;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::objects::ObjectKind;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminRes` and `AdminKill` registrations.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_res", admin_res),
        ("admin_res_monster", admin_res),
        ("admin_kill", admin_kill),
        ("admin_kill_monster", admin_kill),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// `World.sendPacket(SystemMessageId)` of the admin's client.
fn send_system_message(world: &World, to: ObjectId, id: SystemMessageId) {
    world.send(to, &SystemMessage::new(id));
}

/// Java `GameObject instanceof Creature` as the world knows it (`asCreature() != null`).
fn is_creature(world: &World, id: ObjectId) -> bool {
    world.objects.get(&id).is_some_and(|e| e.kind.is_creature())
}

/// The argument of `command.split(" ")[1]`, which Java throws for when there is none.
fn split_argument(command: &str) -> Result<&str, JavaException> {
    let words = java_split_space(command);
    words.get(1).copied().ok_or_else(|| {
        JavaException(format!(
            "java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length {}",
            words.len()
        ))
    })
}

// ----------------------------------------------------------------------------------- AdminRes

/// Java `AdminRes.onCommand`: `//res [name | radius]` for players, `//res_monster [radius]` for
/// everything else.
fn admin_res(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_res ") {
        handle_res(world, gm, Some(split_argument(command)?))
    } else if command == "admin_res" {
        handle_res(world, gm, None)
    } else if command.starts_with("admin_res_monster ") {
        handle_non_player_res(world, gm, split_argument(command)?)
    } else if command == "admin_res_monster" {
        handle_non_player_res(world, gm, "")
    } else {
        Ok(())
    }
}

/// Java `AdminRes.handleRes(activeChar, resParam)`.
fn handle_res(world: &mut World, gm: ObjectId, res_param: Option<&str>) -> Outcome {
    let mut obj = world.target_of(gm);
    if let Some(param) = res_param {
        // Check if a player name was specified as a param.
        if let Some(player) = world.player_by_name(param) {
            obj = Some(player);
        } else {
            // Otherwise, check if the param was a radius.
            let Ok(radius) = parse_int(param) else {
                world.send_sys_message(gm, "Enter a valid player name or radius.");
                return Ok(());
            };
            for player in world.visible_objects_in_range(gm, radius) {
                if world.is_player(player) {
                    do_resurrect(world, player)?;
                }
            }
            world.send_sys_message(
                gm,
                &format!("Resurrected all players within a {radius} unit radius."),
            );
            return Ok(());
        }
    }

    // `if (obj == null) obj = activeChar;`
    do_resurrect(world, obj.unwrap_or(gm))
}

/// Java `AdminRes.handleNonPlayerRes(activeChar, radiusStr)`. An empty `radius` is the command
/// without a number.
fn handle_non_player_res(world: &mut World, gm: ObjectId, radius: &str) -> Outcome {
    let obj = world.target_of(gm);
    if !radius.is_empty() {
        let Ok(radius) = parse_int(radius) else {
            world.send_sys_message(gm, "Enter a valid radius.");
            return Ok(());
        };
        for creature in world.visible_objects_in_range(gm, radius) {
            if is_creature(world, creature) && !world.is_player(creature) {
                do_resurrect(world, creature)?;
            }
        }
        world.send_sys_message(
            gm,
            &format!("Resurrected all non-players within a {radius} unit radius."),
        );
    }

    // `obj == null || obj.isPlayer()`: not a monster the admin can resurrect.
    let Some(target) = obj.filter(|t| !world.is_player(*t)) else {
        send_system_message(world, gm, SystemMessageId::IncorrectItem);
        return Ok(());
    };
    do_resurrect(world, target)
}

/// Java `AdminRes.doResurrect(targetChar)`. A target that is not a creature makes Java's
/// `asCreature()` return `null`, and `isDead()` on it throws a `NullPointerException`, which the
/// dispatcher reports to the GM.
// NOTE: the JVM's helpful NullPointerException message depends on the class file's debug
// information; only the class name is reproduced here.
fn do_resurrect(world: &mut World, target: ObjectId) -> Outcome {
    if !is_creature(world, target) {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    }
    if !world.is_dead(target) {
        return Ok(());
    }

    // If the target is a player, then restore the XP lost on death.
    if world.is_player(target) {
        // M4-EXP-01: `restoreExp(100.0)`, the `restore_exp` hook of `death.rs`.
        world.restore_exp(target, 100.0);
    } else {
        // `DecayTaskManager.getInstance().cancel(targetChar)`: a dead NPC's corpse stays.
        world.cancel_decay_task(target);
    }
    world.do_revive(target);
    Ok(())
}

// ----------------------------------------------------------------------------------- AdminKill

/// Java `AdminKill.onCommand`. NOTE(java-quirk): `//kill_monster` is the same command as `//kill`,
/// it also kills players.
fn admin_kill(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    st.next_token(); // skip command
    if !st.has_more() {
        match world.target_of(gm).filter(|t| is_creature(world, *t)) {
            Some(target) => kill(world, gm, target),
            None => send_system_message(world, gm, SystemMessageId::InvalidTarget),
        }
        return Ok(());
    }

    let Some(first) = st.next_token() else {
        return Ok(());
    };
    if let Some(player) = world.player_by_name(first) {
        if st.has_more() {
            match st.next_token().map(parse_int) {
                Some(Ok(radius)) => {
                    kill_in_range(world, gm, player, radius);
                    world.send_sys_message(
                        gm,
                        &format!("Killed all characters within a {radius} unit radius."),
                    );
                }
                _ => world.send_sys_message(gm, "Invalid radius."),
            }
            return Ok(());
        }
        kill(world, gm, player);
    } else {
        match parse_int(first) {
            Ok(radius) => {
                kill_in_range(world, gm, gm, radius);
                world.send_sys_message(
                    gm,
                    &format!("Killed all characters within a {radius} unit radius."),
                );
            }
            Err(_) => world.send_sys_message(gm, "Usage: //kill <player_name | radius>"),
        }
    }
    Ok(())
}

/// `World.forEachVisibleObjectInRange(center, Creature.class, radius, ...)` minus the GM.
fn kill_in_range(world: &mut World, gm: ObjectId, center: ObjectId, radius: i32) {
    let victims: Vec<ObjectId> = world
        .visible_objects_in_range(center, radius)
        .into_iter()
        .filter(|o| *o != gm && is_creature(world, *o))
        .collect();
    for victim in victims {
        kill(world, gm, victim);
    }
}

/// Java `AdminKill.kill(activeChar, target)`: `reduceCurrentHp(maxHp + maxCp + 1)` for a player,
/// `reduceCurrentHp(maxHp + 1)` for anything else. The GM is the attacker, so an NPC's aggro list
/// and the player's damage messages see it, and a death goes through `death.rs`.
fn kill(world: &mut World, gm: ObjectId, target: ObjectId) {
    let amount = match world.objects.get(&target).map(|e| e.kind) {
        Some(ObjectKind::Player) => {
            // M5: `stopAllEffects()` for a player that is not a GM (effects are not ported).
            let Some(view) = world.stats(target) else {
                return;
            };
            f64::from(view.max_hp()) + f64::from(view.max_cp()) + 1.0
        }
        Some(ObjectKind::Npc) => {
            let Some(view) = world.stats(target) else {
                return;
            };
            f64::from(view.max_hp()) + 1.0
        }
        // Doors and static objects are creatures in Java; their hit points belong to the door and
        // static object code, which is not ported.
        _ => {
            debug!("{gm}: //kill {target}: not a character or NPC; nothing to kill yet");
            return;
        }
    };
    world.reduce_current_hp(target, amount, Some(gm));
}
