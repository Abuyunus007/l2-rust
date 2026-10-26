//! The sound and sky admin commands: `//play_sounds [page]`, `//play_sound <name>`, and
//! `//atmosphere <signsky dawn|dusk>|<sky day|night|red> <duration>` with its menu.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEffects.java (the play_sound(s)
// and atmosphere branches, adminAtmosphere, playAdminSound)

use l2_protocol_game::server::{ExRedSky, PlaySound, SsqInfo, SunRise, SunSet};
use l2_util::ObjectId;

use super::{JavaException, parse_int};
use crate::world::World;

type Outcome = Result<(), JavaException>;

const ATMOSPHERE_USAGE: &str = "Usage: //atmosphere <signsky dawn|dusk>|<sky day|night|red> <duration>";

/// Java `command.substring(from)`: `None` when the command is shorter (Java's
/// `StringIndexOutOfBoundsException`).
fn substring_from(command: &str, from: usize) -> Option<String> {
    (command.chars().count() >= from).then(|| command.chars().skip(from).collect())
}

/// Java `AdminEffects`, `admin_play_sounds`: `songs/songs.htm`, or `songs/songs<rest>.htm` where
/// `<rest>` is the command from the 18th character on (`admin_play_sounds2` shows page 2, and
/// `admin_play_sounds 2` shows `songs/songs 2.htm`, as Java does).
pub(crate) fn play_sounds(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_play_sounds" {
        world.show_admin_html(gm, "songs/songs.htm");
    } else {
        match substring_from(command, "admin_play_sounds".len() + 1) {
            Some(page) => world.show_admin_html(gm, &format!("songs/songs{page}.htm")),
            None => world.send_sys_message(gm, "Usage: //play_sounds <pagenumber>"),
        }
    }
    Ok(())
}

/// Java `AdminEffects`, `admin_play_sound` (`playAdminSound`): the sound goes to the GM and
/// around it, then the GM is told. Java's `Player.broadcastPacket` sends the GM its own copy as
/// well, so the GM gets the sound twice.
pub(crate) fn play_sound(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    match substring_from(command, "admin_play_sound".len() + 1) {
        Some(sound) => {
            let packet = PlaySound::with_unknown1(1, &sound);
            world.send(gm, &packet);
            world.broadcast_creature_packet(gm, &packet);
            world.send_sys_message(gm, &format!("Playing {sound}."));
        }
        None => world.send_sys_message(gm, "Usage: //play_sound <soundname>"),
    }
    Ok(())
}

/// Java `AdminEffects`, `admin_atmosphere` (`words` is the command split on spaces, the command
/// word first). The menu word has no arguments, so it gets the usage text, as in Java.
pub(crate) fn atmosphere(world: &mut World, gm: ObjectId, words: &[&str]) -> Outcome {
    // Java reads type, state and duration; a missing token or a bad number is the usage text.
    let parsed = match (words.get(1), words.get(2), words.get(3)) {
        (Some(kind), Some(state), Some(duration)) => {
            parse_int(duration).ok().map(|duration| (*kind, *state, duration))
        }
        _ => None,
    };
    match parsed {
        Some((kind, state, duration)) => air_packet(world, gm, kind, state, duration),
        None => world.send_sys_message(gm, ATMOSPHERE_USAGE),
    }
    Ok(())
}

/// Java `AdminEffects.adminAtmosphere(type, state, duration, activeChar)`: the packet goes to
/// every online player; a state the type does not have sends nothing.
fn air_packet(world: &mut World, gm: ObjectId, kind: &str, state: &str, duration: i32) {
    match kind {
        "signsky" => match state {
            "dawn" => world.broadcast_to_all_online_players(&SsqInfo { state: 2 }),
            "dusk" => world.broadcast_to_all_online_players(&SsqInfo { state: 1 }),
            _ => {}
        },
        "sky" => match state {
            "night" => world.broadcast_to_all_online_players(&SunSet),
            "day" => world.broadcast_to_all_online_players(&SunRise),
            "red" => {
                // Java: `duration != 0 ? duration : 10`.
                let seconds = if duration != 0 { duration } else { 10 };
                world.broadcast_to_all_online_players(&ExRedSky { duration: seconds });
            }
            _ => {}
        },
        _ => world.send_sys_message(gm, ATMOSPHERE_USAGE),
    }
}
