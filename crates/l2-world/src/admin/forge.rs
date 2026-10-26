//! `//forge`, `//forge_values`, `//forge_send`: a GM builds a server packet by hand.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminPForge.java
// Java: network/serverpackets/AdminForgePacket.java
//
// The numbers are read the way Java reads them: `Integer.decode`, `Long.decode` (decimal,
// `0x`/`#` hexadecimal, a leading `0` octal), `Double.parseDouble` and `BigInteger(String)`.
//
// NOTE(java-quirk): `AdminForgePacket` reads its values when the packet is written to the
// client, not when the command runs. A value that is not a number makes Java's writer throw on
// the network thread; here the values are read first and a packet with such a value is not sent
// (the page comes back as after a successful send).
// NOTE(java-quirk): the method `cs` (a packet from the client) is "not implemented yet" in Java:
// the values are checked, then the usage is shown.
// NOTE(java-quirk): `validateOpCodes` accepts an op code that is not a number as long as it is
// not the first one (the `catch` returns `i > 0`).

use l2_protocol_game::server::{AdminForgePacket, ForgePart};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AdminPForge` registration.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let handler: AdminHandler = admin_forge;
    for command in ["admin_forge", "admin_forge_values", "admin_forge_send"] {
        handlers.register(command, handler);
    }
}

/// Java `Integer.decode` / `Long.decode` / `Short.decode` / `Byte.decode`: the number in the
/// range `min..=max`, or `None` for a `NumberFormatException`.
fn java_decode(text: &str, min: i128, max: i128) -> Option<i128> {
    let (negative, rest) = match text.as_bytes().first()? {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16, hex)
    } else if let Some(hex) = rest.strip_prefix('#') {
        (16, hex)
    } else if rest.starts_with('0') && rest.len() > 1 {
        (8, &rest[1..])
    } else {
        (10, rest)
    };
    // A second sign is refused.
    if digits.starts_with(['-', '+']) || digits.is_empty() {
        return None;
    }
    let magnitude = i128::from_str_radix(digits, radix).ok()?;
    let value = if negative { -magnitude } else { magnitude };
    (min..=max).contains(&value).then_some(value)
}

fn decode_int(text: &str) -> Option<i32> {
    java_decode(text, i128::from(i32::MIN), i128::from(i32::MAX)).and_then(|v| i32::try_from(v).ok())
}

fn decode_long(text: &str) -> Option<i64> {
    java_decode(text, i128::from(i64::MIN), i128::from(i64::MAX)).and_then(|v| i64::try_from(v).ok())
}

/// Java `new BigInteger(text).toByteArray()` for a decimal text: the minimal two's complement
/// bytes, most significant first.
fn big_integer_bytes(text: &str) -> Option<Vec<u8>> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // The magnitude, base 256, most significant byte first.
    let mut magnitude: Vec<u8> = vec![0];
    for digit in digits.bytes().map(|b| u32::from(b - b'0')) {
        let mut carry = digit;
        for byte in magnitude.iter_mut().rev() {
            let value = u32::from(*byte) * 10 + carry;
            *byte = (value & 0xFF) as u8;
            carry = value >> 8;
        }
        while carry > 0 {
            magnitude.insert(0, (carry & 0xFF) as u8);
            carry >>= 8;
        }
    }
    while magnitude.len() > 1 && magnitude.first() == Some(&0) {
        magnitude.remove(0);
    }
    if !negative || magnitude.iter().all(|b| *b == 0) {
        // A leading byte with the high bit set needs a zero in front.
        if magnitude.first().is_some_and(|b| b & 0x80 != 0) {
            magnitude.insert(0, 0);
        }
        return Some(magnitude);
    }
    // Two's complement of a negative number: invert and add one, in as many bytes as the
    // magnitude needs (plus one when the result would not keep its sign).
    let mut bytes = magnitude.clone();
    bytes.insert(0, 0);
    for byte in &mut bytes {
        *byte = !*byte;
    }
    for byte in bytes.iter_mut().rev() {
        let (value, overflow) = byte.overflowing_add(1);
        *byte = value;
        if !overflow {
            break;
        }
    }
    // Drop a redundant 0xFF in front (Java gives the shortest form).
    while bytes.len() > 1 && bytes[0] == 0xFF && bytes[1] & 0x80 != 0 {
        bytes.remove(0);
    }
    Some(bytes)
}

/// Java `AdminForgePacket.generate(type, value, buffer)` as a part; `None` when the value does
/// not parse (Java throws), `Some(None)` for a type Java ignores.
fn forge_part(kind: char, value: &str) -> Option<Option<ForgePart>> {
    Some(Some(match kind {
        'C' | 'c' => ForgePart::Byte((decode_int(value)? & 0xFF) as u8),
        'D' | 'd' => ForgePart::Int(decode_int(value)?),
        'H' | 'h' => ForgePart::Short(decode_int(value)? as i16),
        'F' | 'f' => {
            // `Double.parseDouble` trims and accepts a trailing type suffix.
            let text = value.trim();
            let text = text.strip_suffix(['d', 'D', 'f', 'F']).unwrap_or(text);
            ForgePart::Double(text.parse::<f64>().ok()?)
        }
        'S' | 's' => ForgePart::Text(value.to_owned()),
        'B' | 'b' | 'X' | 'x' => ForgePart::Bytes(big_integer_bytes(value)?),
        'Q' | 'q' => ForgePart::Long(decode_long(value)?),
        _ => return Some(None),
    }))
}

/// Java `String.split(";")`: trailing empty strings are dropped.
fn split_semicolon(text: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = text.split(';').collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// Java `AdminPForge.validateOpCodes`.
fn validate_op_codes(op_codes: Option<&[String]>) -> bool {
    let Some(op_codes) = op_codes else {
        return false;
    };
    if op_codes.is_empty() || op_codes.len() > 3 {
        return false;
    }
    for (i, op_code) in op_codes.iter().enumerate() {
        let Some(value) = decode_long(op_code) else {
            return i > 0;
        };
        if value < 0 {
            return false;
        }
        match i {
            0 if value > 255 => return false,
            1 if value > 65535 => return false,
            2 if value > 4_294_967_295 => return false,
            _ => {}
        }
    }
    true
}

/// Java `AdminPForge.validateFormat`.
fn validate_format(format: &str) -> bool {
    format.chars().all(|c| "bBxXcChHdDqQfFsS".contains(c))
}

fn validate_method(method: &str) -> bool {
    matches!(method, "sc" | "sb" | "cs")
}

/// Java `AdminPForge.onCommand`.
fn admin_forge(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_forge" {
        show_main_page(world, gm);
    } else if command.starts_with("admin_forge_values ") {
        forge_values(world, gm, command)?;
    } else if command.starts_with("admin_forge_send ") {
        forge_send(world, gm, command)?;
    }
    Ok(())
}

fn show_main_page(world: &mut World, gm: ObjectId) {
    world.show_admin_html(gm, "pforge/main.htm");
}

fn show_values_usage(world: &mut World, gm: ObjectId) {
    world.send_sys_message(gm, "Usage: //forge_values opcode1[ opcode2[ opcode3]] ;[ format]");
    show_main_page(world, gm);
}

fn show_send_usage(world: &mut World, gm: ObjectId) {
    world.send_sys_message(
        gm,
        "Usage: //forge_send sc|sb|cs opcode1[;opcode2[;opcode3]][ format value1 ... valueN] ",
    );
    show_main_page(world, gm);
}

/// Java `AdminPForge.showValuesPage`.
fn show_values_page(world: &mut World, gm: ObjectId, op_codes: &[String], format: Option<&str>) {
    let Some(mut values) = world.get_htm(gm, "data/html/admin/pforge/values.htm") else {
        return;
    };
    let (op_format, mut send_bypass) = match op_codes {
        [a, b, c] => ("chd", format!("{a};{b};{c}")),
        [a, b] => ("ch", format!("{a};{b}")),
        [a, ..] => ("c", a.clone()),
        [] => ("c", String::new()),
    };
    values = values
        .replace("%opformat%", op_format)
        .replace("%opcodes%", &send_bypass);
    let editors = match format {
        None => {
            values = values.replace("%format%", "");
            String::new()
        }
        Some(format) => {
            values = values.replace("%format%", format);
            send_bypass.push(' ');
            send_bypass.push_str(format);
            match world.get_htm(gm, "data/html/admin/pforge/inc/editor.htm") {
                Some(template) => {
                    let mut editors = String::new();
                    for (index, ch) in format.chars().enumerate() {
                        editors.push_str(
                            &template
                                .replace("%format%", &ch.to_string())
                                .replace("%editor_index%", &index.to_string()),
                        );
                        send_bypass.push_str(&format!(" $v{index}"));
                    }
                    editors
                }
                None => String::new(),
            }
        }
    };
    values = values
        .replace("%editors%", &editors)
        .replace("%send_bypass%", &send_bypass);
    // `new NpcHtmlMessage(valuesHtml)`.
    if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(0, &values) {
        world.send_html(gm, &message);
    }
}

/// The `admin_forge_values` branch. Java's `try` catches every exception: the usage.
fn forge_values(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    if !st.has_more() {
        show_values_usage(world, gm);
        return Ok(());
    }
    // `getOpCodes`: the tokens up to a token that is just ";".
    let mut op_codes: Option<Vec<String>> = None;
    while let Some(token) = st.next_token() {
        if token == ";" {
            break;
        }
        op_codes.get_or_insert_with(Vec::new).push(token.to_owned());
    }
    if !validate_op_codes(op_codes.as_deref()) {
        world.send_sys_message(gm, "Invalid op codes!");
        show_values_usage(world, gm);
        return Ok(());
    }
    let mut format = None;
    if st.has_more() {
        let candidate = st.next_token().unwrap_or_default();
        if !validate_format(candidate) {
            world.send_sys_message(gm, "Format invalid!");
            show_values_usage(world, gm);
            return Ok(());
        }
        format = Some(candidate.to_owned());
    }
    show_values_page(world, gm, &op_codes.unwrap_or_default(), format.as_deref());
    Ok(())
}

/// Java `Creature.getTitle()` of a player or an NPC; empty for other objects.
fn creature_title(world: &World, id: ObjectId) -> String {
    if let Some(player) = world.players.get(&id) {
        return player.data.title.clone();
    }
    world
        .npcs
        .get(&id)
        .and_then(|npc| world.npc_world.template(npc.npc_id))
        .map(|template| template.title.clone())
        .unwrap_or_default()
}

/// The value of a `$` placeholder (Java's `switch (value)`), or the text itself.
fn substitute(world: &World, gm: ObjectId, value: &str) -> String {
    let player = world.players.get(&gm);
    let target = world.target_of(gm);
    let target_core = target.and_then(|t| world.core_of(t));
    let target_is_playable =
        target.is_some_and(|t| world.players.contains_key(&t) || world.summons.summons.contains_key(&t));
    match value {
        "$oid" => gm.0.to_string(),
        // No boats.
        "$boid" => "0".to_owned(),
        "$title" => player.map(|p| p.data.title.clone()).unwrap_or_default(),
        "$name" => player.map(|p| p.data.name.clone()).unwrap_or_default(),
        "$x" => player.map_or(0, |p| p.core.x).to_string(),
        "$y" => player.map_or(0, |p| p.core.y).to_string(),
        "$z" => player.map_or(0, |p| p.core.z).to_string(),
        "$heading" => player.map_or(0, |p| p.core.heading).to_string(),
        "$toid" => target.map_or(0, |t| t.0).to_string(),
        "$tboid" if target_is_playable => "0".to_owned(),
        "$ttitle" => target.map(|t| creature_title(world, t)).unwrap_or_default(),
        "$tname" => target.and_then(|t| world.object_name(t)).unwrap_or_default(),
        "$tx" => target_core.map_or(0, |c| c.x).to_string(),
        "$ty" => target_core.map_or(0, |c| c.y).to_string(),
        "$tz" => target_core.map_or(0, |c| c.z).to_string(),
        "$theading" => target_core.map_or(0, |c| c.heading).to_string(),
        other => other.to_owned(),
    }
}

/// The `admin_forge_send` branch. Java's `try` catches every exception: the usage.
fn forge_send(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    st.next_token();
    if !st.has_more() {
        show_send_usage(world, gm);
        return Ok(());
    }
    let method = st.next_token().unwrap_or_default();
    if !validate_method(method) {
        world.send_sys_message(gm, "Invalid method!");
        show_send_usage(world, gm);
        return Ok(());
    }
    // `st.nextToken().split(";")`: a missing token is a `NoSuchElementException`.
    let Some(op_token) = st.next_token() else {
        show_send_usage(world, gm);
        return Ok(());
    };
    let op_codes: Vec<String> = split_semicolon(op_token).into_iter().map(str::to_owned).collect();
    if !validate_op_codes(Some(&op_codes)) {
        world.send_sys_message(gm, "Invalid op codes!");
        show_send_usage(world, gm);
        return Ok(());
    }
    let mut format = None;
    if st.has_more() {
        let candidate = st.next_token().unwrap_or_default();
        if !validate_format(candidate) {
            world.send_sys_message(gm, "Format invalid!");
            show_send_usage(world, gm);
            return Ok(());
        }
        format = Some(candidate.to_owned());
    }

    let mut packet = AdminForgePacket::default();
    // A value that does not parse spoils the packet (see the module notes).
    let mut spoiled = false;
    let mut part = |kind: char, value: &str, packet: &mut AdminForgePacket| match forge_part(kind, value) {
        Some(Some(part)) => packet.add_part(part),
        Some(None) => {}
        None => spoiled = true,
    };
    for (i, op_code) in op_codes.iter().enumerate() {
        let kind = match i {
            0 => 'c',
            1 => 'h',
            _ => 'd',
        };
        if method == "cs" {
            // `write(type, opCode, bb)` with `Byte.decode`, `Short.decode`, `Integer.decode`:
            // only the exception it may throw matters, the usage comes either way.
            continue;
        }
        part(kind, op_code, &mut packet);
    }
    if let Some(format) = &format {
        for (i, ch) in format.chars().enumerate() {
            let Some(value) = st.next_token() else {
                world.send_sys_message(gm, "Not enough values!");
                show_send_usage(world, gm);
                return Ok(());
            };
            let _ = i;
            let value = substitute(world, gm, value);
            if method != "cs" {
                part(ch, &value, &mut packet);
            }
        }
    }

    match method {
        "sc" => {
            if !spoiled {
                world.send(gm, &packet);
            }
        }
        "sb" => {
            if !spoiled {
                // `Player.broadcastPacket`: the player and everybody who sees it.
                world.send(gm, &packet);
                world.broadcast_packet(gm, &packet);
            }
        }
        // `throw new UnsupportedOperationException("Not implemented yet!")`, caught by the
        // `catch (Exception e)` of the branch.
        _ => {
            show_send_usage(world, gm);
            return Ok(());
        }
    }
    show_values_page(world, gm, &op_codes, format.as_deref());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_is_javas() {
        assert_eq!(decode_int("12"), Some(12));
        assert_eq!(decode_int("-12"), Some(-12));
        assert_eq!(decode_int("0x1F"), Some(31));
        assert_eq!(decode_int("#ff"), Some(255));
        assert_eq!(decode_int("010"), Some(8));
        assert_eq!(decode_int("0"), Some(0));
        assert_eq!(decode_int("0x"), None);
        assert_eq!(decode_int("--1"), None);
        assert_eq!(decode_int("08"), None);
        assert_eq!(decode_int("2147483648"), None);
        assert_eq!(decode_int("-2147483648"), Some(i32::MIN));
        assert_eq!(decode_int("0xFFFFFFFF"), None);
        assert_eq!(decode_long("0xFFFFFFFF"), Some(4_294_967_295));
        assert_eq!(decode_int(""), None);
    }

    #[test]
    fn big_integers_are_minimal_twos_complement() {
        assert_eq!(big_integer_bytes("0"), Some(vec![0]));
        assert_eq!(big_integer_bytes("127"), Some(vec![0x7F]));
        assert_eq!(big_integer_bytes("128"), Some(vec![0x00, 0x80]));
        assert_eq!(big_integer_bytes("255"), Some(vec![0x00, 0xFF]));
        assert_eq!(big_integer_bytes("256"), Some(vec![0x01, 0x00]));
        assert_eq!(big_integer_bytes("-1"), Some(vec![0xFF]));
        assert_eq!(big_integer_bytes("-128"), Some(vec![0x80]));
        assert_eq!(big_integer_bytes("-129"), Some(vec![0xFF, 0x7F]));
        assert_eq!(big_integer_bytes("-256"), Some(vec![0xFF, 0x00]));
        assert_eq!(big_integer_bytes("x"), None);
        assert_eq!(big_integer_bytes(""), None);
    }

    #[test]
    fn op_codes_are_checked_like_java() {
        let codes = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(validate_op_codes(Some(&codes(&["0x4a"]))));
        assert!(validate_op_codes(Some(&codes(&["255", "65535", "4294967295"]))));
        assert!(!validate_op_codes(Some(&codes(&["256"]))));
        assert!(!validate_op_codes(Some(&codes(&["1", "65536"]))));
        assert!(!validate_op_codes(Some(&codes(&["1", "2", "3", "4"]))));
        assert!(!validate_op_codes(Some(&codes(&["-1"]))));
        assert!(!validate_op_codes(None));
        // NOTE(java-quirk): a word is fine after the first op code, never as the first one.
        assert!(!validate_op_codes(Some(&codes(&["abc"]))));
        assert!(validate_op_codes(Some(&codes(&["1", "abc"]))));
    }

    #[test]
    fn formats_and_methods() {
        assert!(validate_format("cdhqfsbx"));
        assert!(!validate_format("cz"));
        assert!(validate_method("sb"));
        assert!(!validate_method("xx"));
    }

    #[test]
    fn parts_follow_the_format_letter() {
        assert_eq!(forge_part('c', "0x1ff"), Some(Some(ForgePart::Byte(0xFF))));
        assert_eq!(forge_part('d', "-2"), Some(Some(ForgePart::Int(-2))));
        assert_eq!(forge_part('h', "0x10002"), Some(Some(ForgePart::Short(2))));
        assert_eq!(forge_part('f', "1.5"), Some(Some(ForgePart::Double(1.5))));
        assert_eq!(
            forge_part('s', "Hi"),
            Some(Some(ForgePart::Text("Hi".to_owned())))
        );
        assert_eq!(forge_part('q', "5"), Some(Some(ForgePart::Long(5))));
        assert_eq!(forge_part('d', "nope"), None);
        assert_eq!(forge_part('z', "1"), Some(None));
    }
}
