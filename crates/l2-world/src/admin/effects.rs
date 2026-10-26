//! The effect admin commands: `//invis`, `//vis`, `//setinvis`, `//invis_menu`, `//earthquake`,
//! `//bighead`, `//shrinkhead`, `//para`, `//unpara` (with `_all` and the menus), `//social`,
//! `//effect`, `//ave_abnormal|special|event`, `//set_displayeffect`, `//event_trigger`, and the
//! sound and sky words of `sound.rs`. Their menus are the admin pages of `effects_menu.htm` and
//! the `ave_*` and `social` ones.
//!
//! Java's `AdminEffects` is one class with one `if`/`else if` chain on the command text, and the
//! order of that chain decides which branch a menu word takes (`admin_para_all_menu` is a `para`
//! word, not a menu). The chain is ported in the same order in [`admin_effects`].
//!
//! `//clearteams`, `//setteam` and `//setteam_close` set the team of players through
//! `World::set_team` (`duel.rs`). A creature that is not a player keeps its team (no team field
//! for NPCs yet).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEffects.java

use l2_data::skill::AbnormalVisualEffect;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    Earthquake, ExChangeNpcState, MagicSkillUse, OnEventTrigger, SkillPosition, SocialAction, SystemMessage,
};
use l2_util::ObjectId;

use super::sound;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::objects::ObjectKind;
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `SocialAction.LEVEL_UP`: the one social id a player takes above 18.
const SOCIAL_LEVEL_UP: i32 = 2122;

/// Java `AdminEffects.ADMIN_COMMANDS`, the words that reach [`admin_effects`].
pub(crate) fn register(handlers: &mut AdminHandlers) {
    const WORDS: &[&str] = &[
        "admin_invis",
        "admin_invisible",
        "admin_setinvis",
        "admin_vis",
        "admin_visible",
        "admin_invis_menu",
        "admin_earthquake",
        "admin_earthquake_menu",
        "admin_bighead",
        "admin_shrinkhead",
        "admin_unpara_all",
        "admin_para_all",
        "admin_unpara",
        "admin_para",
        "admin_unpara_all_menu",
        "admin_para_all_menu",
        "admin_unpara_menu",
        "admin_para_menu",
        "admin_social",
        "admin_effect",
        "admin_effect_menu",
        "admin_ave_abnormal",
        "admin_ave_special",
        "admin_ave_event",
        "admin_social_menu",
        "admin_play_sounds",
        "admin_play_sound",
        "admin_atmosphere",
        "admin_atmosphere_menu",
        "admin_set_displayeffect",
        "admin_set_displayeffect_menu",
        "admin_event_trigger",
        "admin_clearteams",
        "admin_setteam",
        "admin_setteam_close",
    ];
    let handler: AdminHandler = admin_effects;
    for word in WORDS {
        handlers.register(word, handler);
    }
}

/// The words of a command, the command word first (Java `StringTokenizer` with the default
/// delimiters).
pub(crate) fn words_of(command: &str) -> Vec<&str> {
    let mut st = Tokens::new(command);
    let mut out = Vec::new();
    while let Some(word) = st.next_token() {
        out.push(word);
    }
    out
}

/// Java `AdminEffects.onCommand`: the whole chain, with the menu page at the end.
fn admin_effects(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let words = words_of(command);

    if command == "admin_invis_menu" {
        // Java toggles, then `command = ""`, so only `gm_menu.htm` is shown after it.
        if world.objects.get(&gm).is_some_and(|e| !e.invisible) {
            hide_now(world, gm);
            world.send_sys_message(gm, "Now, you cannot be seen.");
        } else {
            show_now(world, gm);
            world.send_sys_message(gm, "Now, you can be seen.");
        }
        world.show_admin_html(gm, "gm_menu.htm");
        return Ok(());
    } else if command.starts_with("admin_invis") {
        hide_now(world, gm);
        world.send_sys_message(gm, "Now, you cannot be seen.");
    } else if command.starts_with("admin_vis") {
        show_now(world, gm);
        world.send_sys_message(gm, "Now, you can be seen.");
    } else if command.starts_with("admin_setinvis") {
        setinvis(world, gm);
    } else if command.starts_with("admin_earthquake") {
        earthquake(world, gm, &words);
    } else if command.starts_with("admin_atmosphere") {
        sound::atmosphere(world, gm, &words)?;
    } else if command.starts_with("admin_play_sounds") {
        sound::play_sounds(world, gm, command)?;
    } else if command.starts_with("admin_play_sound") {
        sound::play_sound(world, gm, command)?;
    } else if command == "admin_para_all" {
        for player in world.visible_players(gm) {
            if !world.is_gm(player) {
                para_player(world, player, AbnormalVisualEffect::Paralyze);
            }
        }
    } else if command == "admin_unpara_all" {
        for player in world.visible_players(gm) {
            unpara_player(world, player, AbnormalVisualEffect::Paralyze);
        }
    } else if command.starts_with("admin_para") {
        para_target(world, gm, &words, true);
    } else if command.starts_with("admin_unpara") {
        para_target(world, gm, &words, false);
    } else if command.starts_with("admin_bighead") {
        if let Some(target) = creature_target(world, gm) {
            world.start_abnormal_visual_effect(target, &[AbnormalVisualEffect::BigHead]);
        }
    } else if command.starts_with("admin_shrinkhead") {
        if let Some(target) = creature_target(world, gm) {
            world.stop_abnormal_visual_effect(target, &[AbnormalVisualEffect::BigHead]);
        }
    } else if command == "admin_clearteams" {
        for player in world.visible_players(gm) {
            // Java `Team.NONE`.
            world.set_team(player, 0);
        }
    } else if command.starts_with("admin_setteam_close") {
        setteam_close(world, gm, &words);
    } else if command.starts_with("admin_setteam") {
        setteam(world, gm, &words);
    } else if command.starts_with("admin_social") {
        social(world, gm, command, &words);
    } else if command.starts_with("admin_ave_abnormal")
        || command.starts_with("admin_ave_special")
        || command.starts_with("admin_ave_event")
    {
        if !ave(world, gm, command, &words)? {
            // Java `return false`: no menu page.
            return Ok(());
        }
    } else if command.starts_with("admin_effect") {
        msu(world, gm, &words);
    } else if command.starts_with("admin_set_displayeffect") {
        if !set_display_effect(world, gm, &words) {
            // Java `return false`: no menu page.
            return Ok(());
        }
    } else if command.starts_with("admin_event_trigger") {
        event_trigger(world, gm, &words);
    }

    if command.contains("menu") || command.contains("ave_") {
        show_main_page(world, gm, command);
    }
    Ok(())
}

// ------------------------------------------------------------------------------ helpers

/// Java `Creature` check (`isCreature()`): a player, an NPC, a summon, a door or a static object.
fn is_creature(world: &World, id: ObjectId) -> bool {
    world.objects.get(&id).is_some_and(|e| e.kind.is_creature())
}

/// The target of the GM when it is a creature. `None` is Java's `null` or non-creature target,
/// where the handlers either answer the invalid target or do nothing.
fn creature_target(world: &World, gm: ObjectId) -> Option<ObjectId> {
    world.target_of(gm).filter(|t| is_creature(world, *t))
}

fn name_of(world: &World, id: ObjectId) -> String {
    world.object_name(id).unwrap_or_default()
}

fn send_nothing_happened(world: &World, to: ObjectId) {
    world.send(to, &SystemMessage::new(SystemMessageId::NothingHappened));
}

/// Java `activeChar.setInvisible(true)`, `broadcastUserInfo()`, then each visible creature that
/// targets the GM lets go of it (`setTarget(null)`, `abortAttack`, `abortCast`, idle AI).
fn hide_now(world: &mut World, gm: ObjectId) {
    world.set_invisible(gm, true);
    world.broadcast_user_info(gm);
    for other in world.visible_objects(gm) {
        if is_creature(world, other) && world.target_of(other) == Some(gm) {
            world.set_target(other, None);
            world.abort_attack(other);
            world.abort_cast(other);
            world.set_intention_idle(other);
        }
    }
}

/// Java `activeChar.setInvisible(false)` and `broadcastUserInfo()`.
fn show_now(world: &mut World, gm: ObjectId) {
    world.set_invisible(gm, false);
    world.broadcast_user_info(gm);
}

/// Java `Creature.hasAbnormalVisualEffect(ave)`: the mask of the field the effect belongs to.
fn has_ave(world: &World, id: ObjectId, ave: AbnormalVisualEffect) -> bool {
    let (base, special, event) = world.abnormal_visual_effects(id);
    let field = if ave.is_event() {
        event
    } else if ave.is_special() {
        special
    } else {
        base
    };
    field & ave.mask() == ave.mask()
}

/// Java `AdminEffects.performAbnormalVisualEffect`: a creature gets the effect or loses it.
fn perform_ave(world: &mut World, ave: AbnormalVisualEffect, target: ObjectId) -> bool {
    if !is_creature(world, target) {
        return false;
    }
    if has_ave(world, target, ave) {
        world.stop_abnormal_visual_effect(target, &[ave]);
    } else {
        world.start_abnormal_visual_effect(target, &[ave]);
    }
    true
}

/// Java `AdminEffects.performSocial(action, target, activeChar)`.
fn perform_social(world: &mut World, action: i32, target: ObjectId, gm: ObjectId) -> bool {
    let Some(kind) = world.objects.get(&target).map(|e| e.kind) else {
        return false;
    };
    if !kind.is_creature() {
        return false;
    }
    if world.is_chest(target) {
        send_nothing_happened(world, gm);
        return false;
    }
    if kind == ObjectKind::Npc && !(1..=3).contains(&action) {
        send_nothing_happened(world, gm);
        return false;
    }
    if kind == ObjectKind::Player && (action < 2 || (action > 18 && action != SOCIAL_LEVEL_UP)) {
        send_nothing_happened(world, gm);
        return false;
    }
    world.broadcast_creature_packet(
        target,
        &SocialAction {
            object_id: target.0,
            action_id: action,
        },
    );
    true
}

/// `AdminEffects.showMainPage`: `effects_menu.htm`, or the page the word belongs to.
fn show_main_page(world: &mut World, gm: ObjectId, command: &str) {
    let file = if command.contains("ave_abnormal") {
        "ave_abnormal"
    } else if command.contains("ave_special") {
        "ave_special"
    } else if command.contains("ave_event") {
        "ave_event"
    } else if command.contains("social") {
        "social"
    } else {
        "effects_menu"
    };
    world.show_admin_html(gm, &format!("{file}.htm"));
}

// ------------------------------------------------------------------------------ branches

/// Java `admin_setinvis`: the target toggles; a non-creature target is an invalid target.
fn setinvis(world: &mut World, gm: ObjectId) {
    let Some(target) = creature_target(world, gm) else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return;
    };
    let invisible = world.objects.get(&target).is_some_and(|e| e.invisible);
    world.set_invisible(target, !invisible);
    let now_invisible = world.objects.get(&target).is_some_and(|e| e.invisible);
    let state = if now_invisible { "invisible" } else { "visible" };
    let name = name_of(world, target);
    world.send_sys_message(gm, &format!("You've made {name} {state}."));
    if world.is_player(target) {
        world.broadcast_user_info(target);
    }
}

/// Java `admin_earthquake <intensity> <duration>`: an earthquake at the GM's position, to the GM
/// and around it.
fn earthquake(world: &mut World, gm: ObjectId, words: &[&str]) {
    let parsed = match (words.get(1), words.get(2)) {
        (Some(intensity), Some(duration)) => match (parse_int(intensity), parse_int(duration)) {
            (Ok(intensity), Ok(duration)) => Some((intensity, duration)),
            _ => None,
        },
        _ => None,
    };
    let Some((intensity, duration)) = parsed else {
        world.send_sys_message(gm, "Usage: //earthquake <intensity> <duration>");
        return;
    };
    let (x, y, z) = world.position_of(gm).unwrap_or((0, 0, 0));
    world.broadcast_creature_packet(
        gm,
        &Earthquake {
            x,
            y,
            z,
            intensity,
            duration,
        },
    );
}

/// Java `Creature.startAbnormalVisualEffect(true, ave)`, `setParalyzed(true)`, `startParalyze()`.
fn para_player(world: &mut World, id: ObjectId, ave: AbnormalVisualEffect) {
    world.start_abnormal_visual_effect(id, &[ave]);
    set_paralyzed(world, id, true);
    world.start_paralyze(id);
}

/// Java `Creature.stopAbnormalVisualEffect(true, ave)`, `setParalyzed(false)`.
fn unpara_player(world: &mut World, id: ObjectId, ave: AbnormalVisualEffect) {
    world.stop_abnormal_visual_effect(id, &[ave]);
    set_paralyzed(world, id, false);
}

/// Java `Creature.setParalyzed(value)`: the flag of the creature's control state.
fn set_paralyzed(world: &mut World, id: ObjectId, value: bool) {
    if let Some(control) = world.control_mut(id) {
        control.paralyzed = value;
    }
}

/// Java `admin_para` / `admin_unpara`, menus included (`admin_para_menu` has no type, so `1`):
/// the type `1` is the paralysis, any other is the flesh stone. A missing target does nothing.
fn para_target(world: &mut World, gm: ObjectId, words: &[&str], paralyze: bool) {
    let ave = if words.get(1).copied().unwrap_or("1") == "1" {
        AbnormalVisualEffect::Paralyze
    } else {
        AbnormalVisualEffect::FleshStone
    };
    if let Some(target) = creature_target(world, gm) {
        if paralyze {
            para_player(world, target, ave);
        } else {
            unpara_player(world, target, ave);
        }
    }
}

/// Java `admin_social <social_id> [player_name|radius]`, `admin_social <social_id>` and the menu
/// word (no arguments). A bad social id is silent, as Java's `catch (Exception e)` is.
fn social(world: &mut World, gm: ObjectId, command: &str, words: &[&str]) {
    let params = &words[1.min(words.len())..];
    match params {
        [social, target] => {
            let Ok(action) = parse_int(social) else {
                return;
            };
            if let Some(player) = world.player_by_name(target) {
                if perform_social(world, action, player, gm) {
                    let name = name_of(world, player);
                    world.send_message(gm, &format!("{name} was affected by your request."));
                }
            } else {
                match parse_int(target) {
                    Ok(radius) => {
                        for object in world.visible_objects_in_range(gm, radius) {
                            perform_social(world, action, object, gm);
                        }
                        world.send_message(gm, &format!("{radius} units radius affected by your request."));
                    }
                    Err(_) => world.send_sys_message(gm, "Incorrect parameter"),
                }
            }
        }
        [social] => {
            let Ok(action) = parse_int(social) else {
                return;
            };
            let obj = world.target_of(gm).unwrap_or(gm);
            if perform_social(world, action, obj, gm) {
                let name = name_of(world, obj);
                world.send_message(gm, &format!("{name} was affected by your request."));
            } else {
                send_nothing_happened(world, gm);
            }
        }
        _ => {
            if !command.contains("menu") {
                world.send_sys_message(gm, "Usage: //social <social_id> [player_name|radius]");
            }
        }
    }
}

/// Java `admin_ave_abnormal|special|event <AbnormalVisualEffect> [radius]`. Returns `false` when
/// the effect name is unknown (Java `return false`: no menu page).
fn ave(world: &mut World, gm: ObjectId, command: &str, words: &[&str]) -> Result<bool, JavaException> {
    let params = &words[1.min(words.len())..];
    let Some(&first) = params.first() else {
        let word = command.replace("admin_", "");
        world.send_sys_message(gm, &format!("Usage: //{word} <AbnormalVisualEffect> [radius]"));
        return Ok(true);
    };
    let Some(effect) = AbnormalVisualEffect::from_name(first) else {
        return Ok(false);
    };
    let mut radius = 0;
    let mut second = None;
    if let [_, value] = params {
        second = Some(*value);
        if is_numeric(value) {
            radius = parse_int(value)?;
        }
    }
    if radius > 0 {
        for object in world.visible_objects_in_range(gm, radius) {
            perform_ave(world, effect, object);
        }
        let radius_text = second.unwrap_or_default();
        world.send_sys_message(
            gm,
            &format!("Affected all characters in radius {radius_text} by {first} abnormal visual effect."),
        );
    } else {
        let obj = world.target_of(gm).unwrap_or(gm);
        if perform_ave(world, effect, obj) {
            let name = name_of(world, obj);
            world.send_message(gm, &format!("{name} affected by {first} abnormal visual effect."));
        } else {
            send_nothing_happened(world, gm);
        }
    }
    Ok(true)
}

/// Java `admin_effect <skill> [level [hittime]]`: the Magic Skill Use packet, its caster being
/// the target (or the GM), its target the GM.
fn msu(world: &mut World, gm: ObjectId, words: &[&str]) {
    let parsed = (|| -> Option<(i32, i32, i32)> {
        let skill = parse_int(words.get(1)?).ok()?;
        let level = match words.get(2) {
            Some(text) => parse_int(text).ok()?,
            None => 1,
        };
        let hit_time = match words.get(3) {
            Some(text) => parse_int(text).ok()?,
            None => 1,
        };
        Some((skill, level, hit_time))
    })();
    let Some((skill, level, hit_time)) = parsed else {
        world.send_sys_message(gm, "Usage: //effect skill [level | level hittime]");
        return;
    };
    let obj = world.target_of(gm).unwrap_or(gm);
    if !is_creature(world, obj) {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return;
    }
    let (cx, cy, cz) = world.position_of(obj).unwrap_or((0, 0, 0));
    let (tx, ty, tz) = world.position_of(gm).unwrap_or((0, 0, 0));
    // Java `MagicSkillUse(creature)`: a player caster sends where its skill is aimed, if any.
    let ground = if world.is_player(obj) {
        world
            .cast_state(obj)
            .and_then(|s| s.current_skill_world_position)
            .map(|(x, y, z)| SkillPosition { x, y, z })
    } else {
        None
    };
    let packet = MagicSkillUse {
        creature_id: obj.0,
        target_id: gm.0,
        skill_id: skill,
        skill_level: level,
        hit_time,
        reuse_delay: 0,
        creature: SkillPosition { x: cx, y: cy, z: cz },
        ground_location: ground,
        target: SkillPosition { x: tx, y: ty, z: tz },
    };
    world.broadcast_creature_packet(obj, &packet);
    let name = name_of(world, obj);
    world.send_message(
        gm,
        &format!("{name} performs MSU {skill}/{level} by your request."),
    );
}

/// Java `admin_set_displayeffect <id>`: the NPC target's display effect, announced to the players
/// around it when it changes. Returns `false` for a target that is not an NPC (Java `return
/// false`: no menu page).
fn set_display_effect(world: &mut World, gm: ObjectId, words: &[&str]) -> bool {
    let npc = world
        .target_of(gm)
        .filter(|t| world.objects.get(t).is_some_and(|e| e.kind == ObjectKind::Npc));
    let Some(npc) = npc else {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return false;
    };
    match words.get(1).and_then(|text| parse_int(text).ok()) {
        Some(value) => {
            let changed = world.npcs.get_mut(&npc).is_some_and(|entry| {
                let changed = entry.display_effect != value;
                entry.display_effect = value;
                changed
            });
            if changed {
                world.broadcast_packet(
                    npc,
                    &ExChangeNpcState {
                        object_id: npc.0,
                        state: value,
                    },
                );
            }
        }
        None => world.send_sys_message(gm, "Usage: //set_displayeffect <id>"),
    }
    true
}

/// Java `admin_event_trigger <id> <true|false>`: the trigger goes to every visible player and to
/// the GM. `Boolean.parseBoolean` is true for "true" in any case, anything else is false.
fn event_trigger(world: &mut World, gm: ObjectId, words: &[&str]) {
    match (words.get(1).and_then(|t| parse_int(t).ok()), words.get(2)) {
        (Some(trigger_id), Some(enable)) => {
            let packet = OnEventTrigger {
                emitter_id: trigger_id,
                enabled: enable.eq_ignore_ascii_case("true"),
            };
            for player in world.visible_players(gm) {
                world.send(player, &packet);
            }
            world.send(gm, &packet);
        }
        _ => world.send_sys_message(gm, "Usage: //event_trigger id [true | false]"),
    }
}

/// Java `Team.valueOf(name.toUpperCase())` as the team id of `Creature.getTeam()`: 0 none, 1 blue,
/// 2 red. Any other name is Java's `IllegalArgumentException` (the usage text).
fn team_id(name: &str) -> Option<u8> {
    match name.to_uppercase().as_str() {
        "NONE" => Some(0),
        "BLUE" => Some(1),
        "RED" => Some(2),
        _ => None,
    }
}

/// Java `admin_setteam_close <none|blue|red> [radius]`: the players within `radius` (400 by
/// default) of the GM get the team.
fn setteam_close(world: &mut World, gm: ObjectId, words: &[&str]) {
    const USAGE: &str = "Usage: //setteam_close <none|blue|red> [radius]";
    let Some(name) = words.get(1) else {
        world.send_sys_message(gm, USAGE);
        return;
    };
    let radius = match words.get(2) {
        Some(text) => match parse_int(text) {
            Ok(radius) => radius,
            Err(_) => {
                world.send_sys_message(gm, USAGE);
                return;
            }
        },
        None => 400,
    };
    let Some(team) = team_id(name) else {
        world.send_sys_message(gm, USAGE);
        return;
    };
    for player in world.visible_objects_in_range(gm, radius) {
        if world.is_player(player) {
            world.set_team(player, team);
        }
    }
}

/// Java `admin_setteam <none|blue|red>`: the targeted creature gets the team. No target is the
/// usage text; a target that is not a creature does nothing. Only players have a team here.
fn setteam(world: &mut World, gm: ObjectId, words: &[&str]) {
    const USAGE: &str = "Usage: //setteam <none|blue|red>";
    let Some(team) = words.get(1).and_then(|name| team_id(name)) else {
        world.send_sys_message(gm, USAGE);
        return;
    };
    let Some(target) = world.target_of(gm) else {
        world.send_sys_message(gm, USAGE);
        return;
    };
    if is_creature(world, target) && world.is_player(target) {
        world.set_team(target, team);
    }
}
