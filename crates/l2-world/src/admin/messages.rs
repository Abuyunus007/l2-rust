//! The system message test commands: `//msg <id>` sends the message as it is, and `//msgx <id>
//! [item:id] [skill:id] [npc:id] [zone:x,y,z] [castle:id] [str:'text']` adds the parameters.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminMessages.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::commands::java_split_space;
use super::{AdminHandler, AdminHandlers, JavaException, is_numeric, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminMessages.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[("admin_msg", admin_msg), ("admin_msgx", admin_msgx)];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// Java `admin_msg <id>`: `command.substring(10).trim()` is the id. Any failure, an id the message
/// table does not have included, is the usage text. A command without the space is no branch.
fn admin_msg(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if !command.starts_with("admin_msg ") {
        return Ok(());
    }
    let text: String = command.chars().skip(10).collect();
    let id = super::parse_int(text.trim())
        .ok()
        .and_then(SystemMessageId::from_id);
    match id {
        Some(id) => {
            world.send(gm, &SystemMessage::new(id));
        }
        None => world.send_sys_message(gm, "Command format: //msg <SYSTEM_MSG_ID>"),
    }
    Ok(())
}

/// Java `admin_msgx <id> [parameters]`. The message is sent even when a parameter failed (that
/// parameter gets an "Exception:" line instead).
fn admin_msgx(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if !command.starts_with("admin_msgx ") {
        return Ok(());
    }
    let tokens = java_split_space(command);
    if tokens.len() <= 2 || !is_numeric(tokens[1]) {
        world.send_sys_message(
            gm,
            "Command format: //msgx <SYSTEM_MSG_ID> [item:Id] [skill:Id] [npc:Id] [zone:x,y,x] [castle:Id] [str:'text']",
        );
        return Ok(());
    }
    // NOTE(java-quirk): `new SystemMessage(id)` is outside Java's try: an id the table does not
    // have is the uncaught NullPointerException of the command.
    let id = parse_int(tokens[1])?;
    let Some(id) = SystemMessageId::from_id(id) else {
        return Err(JavaException("java.lang.NullPointerException".to_owned()));
    };
    let mut message = SystemMessage::new(id);
    let mut last_pos: i32 = 0;
    for token in &tokens[2..] {
        let result = param(command, token, &mut last_pos);
        match result {
            Ok(Some(param)) => message = message.with(param),
            Ok(None) => {}
            Err(text) => world.send_sys_message(gm, &format!("Exception: {text}")),
        }
    }
    world.send(gm, &message);
    Ok(())
}

/// One `msgx` parameter. `Err` is the message of the Java exception the parameter raised; `Ok(None)`
/// is a word that is not a parameter.
fn param(command: &str, token: &str, last_pos: &mut i32) -> Result<Option<SystemMessageParam>, String> {
    if let Some(value) = token.strip_prefix("item:") {
        return int_of(value).map(|v| Some(SystemMessageParam::ItemName(v)));
    }
    if let Some(value) = token.strip_prefix("skill:") {
        return int_of(value).map(|v| Some(SystemMessageParam::SkillName { id: v, level: 1 }));
    }
    if let Some(value) = token.strip_prefix("npc:") {
        return int_of(value).map(|v| Some(SystemMessageParam::NpcName(v)));
    }
    if token.starts_with("zone:") {
        return zone(token).map(Some);
    }
    if let Some(value) = token.strip_prefix("castle:") {
        return int_of(value).map(|v| Some(SystemMessageParam::CastleName(v)));
    }
    if token.starts_with("str:") {
        // Java: the text between the first two quotes after `lastPos`.
        let pos = index_of(command, '\'', *last_pos + 1);
        *last_pos = index_of(command, '\'', pos + 1);
        let text = java_substring(command, pos + 1, *last_pos)?;
        return Ok(Some(SystemMessageParam::Text(text)));
    }
    Ok(None)
}

/// `Integer.parseInt(text)` for a parameter: the bare message of the NumberFormatException.
fn int_of(text: &str) -> Result<i32, String> {
    parse_int(text).map_err(|_| format!("For input string: \"{text}\""))
}

/// `zone:x,y,z` (the whole token): Java's `val.substring(5, val.indexOf(","))` and the two
/// slices after the commas, on the token as Java sees it.
fn zone(token: &str) -> Result<SystemMessageParam, String> {
    let first = index_of(token, ',', 0);
    let last = last_index_of(token, ',');
    let x = int_of(&java_substring(token, 5, first)?)?;
    let y = int_of(&java_substring(token, first + 1, last)?)?;
    let z = int_of(&java_substring(token, last + 1, token.chars().count() as i32)?)?;
    Ok(SystemMessageParam::ZoneName { x, y, z })
}

/// Java `String.indexOf(ch, from)` in characters; -1 when it is not there.
fn index_of(text: &str, needle: char, from: i32) -> i32 {
    let from = from.max(0) as usize;
    text.chars()
        .enumerate()
        .skip(from)
        .find(|(_, c)| *c == needle)
        .map_or(-1, |(i, _)| i as i32)
}

/// Java `String.lastIndexOf(ch)` in characters; -1 when it is not there.
fn last_index_of(text: &str, needle: char) -> i32 {
    text.chars()
        .enumerate()
        .filter(|(_, c)| *c == needle)
        .last()
        .map_or(-1, |(i, _)| i as i32)
}

/// Java `String.substring(begin, end)` in characters. A bad range is the
/// StringIndexOutOfBoundsException, whose text (JDK 11 and later) is `begin b, end e, length n`.
// GUESS: the exact exception texts of Java's substring are the JDK 17 ones; this is the only place
// the text is copied, so a different JDK would print another line here.
fn java_substring(text: &str, begin: i32, end: i32) -> Result<String, String> {
    let length = text.chars().count() as i32;
    if begin < 0 || end > length || begin > end {
        return Err(format!("begin {begin}, end {end}, length {length}"));
    }
    Ok(text
        .chars()
        .skip(begin as usize)
        .take((end - begin) as usize)
        .collect())
}
