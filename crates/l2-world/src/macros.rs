//! The macro list: `RequestMakeMacro`, `RequestDeleteMacro` and the `MacroList` packets that
//! follow a change.
// Java: network/clientpackets/{RequestMakeMacro,RequestDeleteMacro}.java
// Java: entity/actor/holders/player/MacroList.java (registerMacro, deleteMacro, sendUpdate)
// Java: network/serverpackets/SendMacroList.java
//
// # Where a macro lives
// `PlayerData.profile.macros` (one `MacroData` per `character_macroses` row), in the order Java's
// `LinkedHashMap` keeps: a macro that is saved again keeps its place, a new one goes last.
//
// # Database
// Java writes at once (`registerMacroInDb`: `INSERT`, after a `DELETE` of the macro it replaces;
// `deleteMacroFromDb`: `DELETE`). Here the rows follow with the next store of the character, which
// compares the macro list with what it stored last (`persist.rs`, the same path as the shortcuts).
// The result in the table is the same rows. The command text is cut to 255 characters when it is
// written, like `registerMacroInDb` does; the list in memory keeps the full commands.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::RequestMakeMacro;
use l2_protocol_game::server::{MacroList, SystemMessage};
use l2_util::ObjectId;

use crate::enter::macro_entry;
use crate::player::{MacroCmdData, MacroData};
use crate::world::World;

/// Java `RequestMakeMacro.runImpl`: more than this many macros refuse a new one.
const MAX_MACROS: usize = 48;
/// The longest all commands of a macro may be together (`_commandsLength > 255`).
const MAX_COMMANDS_LENGTH: usize = 255;
/// The longest description (`getDescr().length() > 32`).
const MAX_DESCR_LENGTH: usize = 32;
/// Java `MacroList._macroId`'s start: new macros count up from here.
pub const FIRST_MACRO_ID: i32 = 1000;

/// Java `String.length()`: UTF-16 units.
fn java_length(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Java `String.equalsIgnoreCase`: the same length, and each pair of UTF-16 units equal after
/// `toUpperCase` or after `toLowerCase`.
fn equals_ignore_case(a: &str, b: &str) -> bool {
    let (a, b): (Vec<u16>, Vec<u16>) = (a.encode_utf16().collect(), b.encode_utf16().collect());
    a.len() == b.len()
        && a.iter().zip(&b).all(|(&x, &y)| {
            x == y || fold_unit(x, true) == fold_unit(y, true) || fold_unit(x, false) == fold_unit(y, false)
        })
}

/// `Character.toUpperCase(char)` or `toLowerCase(char)` of one UTF-16 unit: one unit in, one out.
fn fold_unit(unit: u16, upper: bool) -> u16 {
    let Some(c) = char::from_u32(u32::from(unit)) else {
        // A lone surrogate has no case.
        return unit;
    };
    let mut mapped: Box<dyn Iterator<Item = char>> = if upper {
        Box::new(c.to_uppercase())
    } else {
        Box::new(c.to_lowercase())
    };
    match (mapped.next(), mapped.next()) {
        (Some(m), None) => u16::try_from(u32::from(m)).unwrap_or(unit),
        _ => unit,
    }
}

/// The `MacroList` packets of a list at a revision: one packet per macro, or a single packet
/// without a macro when the list is empty (Java `MacroList.sendUpdate`).
pub fn macro_list_packets(macros: &[MacroData], revision: i32) -> Vec<MacroList> {
    if macros.is_empty() {
        return vec![MacroList {
            revision,
            count: 0,
            macro_entry: None,
        }];
    }
    macros
        .iter()
        .map(|m| MacroList {
            revision,
            count: macros.len() as u8,
            macro_entry: Some(macro_entry(m)),
        })
        .collect()
}

impl World {
    /// Java `RequestMakeMacro.runImpl`.
    pub(crate) fn on_request_make_macro(&mut self, id: ObjectId, request: RequestMakeMacro) {
        // `MacroType.values()[(type < 1) || (type > 6) ? 0 : type]`.
        let commands_length: usize = request.commands.iter().map(|c| java_length(&c.command)).sum();
        let new_macro = MacroData {
            id: request.id,
            icon: i32::from(request.icon),
            name: request.name,
            descr: request.descr,
            acronym: request.acronym,
            commands: request
                .commands
                .into_iter()
                .map(|c| MacroCmdData {
                    kind: if (1..=6).contains(&c.kind) {
                        i32::from(c.kind)
                    } else {
                        0
                    },
                    d1: c.d1,
                    d2: i32::from(c.d2),
                    cmd: c.command,
                })
                .collect(),
        };

        // Enter the name of the macro.
        if new_macro.name.is_empty() {
            self.send(id, &SystemMessage::new(SystemMessageId::EnterTheNameOfTheMacro));
            return;
        }
        // Invalid macro. Refer to the Help file for instructions.
        if commands_length > MAX_COMMANDS_LENGTH {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::InvalidMacroReferToTheHelpFileForInstructions),
            );
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let macros = &player.data.profile.macros;
        // You may create up to 48 macros.
        // NOTE(java-quirk): `size() > 48` lets a player with 48 macros make a 49th.
        if macros.len() > MAX_MACROS {
            self.send(id, &SystemMessage::new(SystemMessageId::YouMayCreateUpTo48Macros));
            return;
        }
        // That name is already assigned to another macro.
        // NOTE(java-quirk): Java sends "Enter the name of the macro" for this too.
        if macros
            .iter()
            .any(|m| equals_ignore_case(&m.name, &new_macro.name) && m.id != new_macro.id)
        {
            self.send(id, &SystemMessage::new(SystemMessageId::EnterTheNameOfTheMacro));
            return;
        }
        // Macro descriptions may contain up to 32 characters.
        if java_length(&new_macro.descr) > MAX_DESCR_LENGTH {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::MacroDescriptionsMayContainUpTo32Characters),
            );
            return;
        }

        self.register_macro(id, new_macro);
    }

    /// Java `MacroList.registerMacro`.
    pub(crate) fn register_macro(&mut self, id: ObjectId, mut new_macro: MacroData) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        if new_macro.id == 0 {
            // `setId(_macroId++)`, skipping the ids that are taken.
            loop {
                new_macro.id = player.leftover.macro_id;
                player.leftover.macro_id += 1;
                if !player.data.profile.macros.iter().any(|m| m.id == new_macro.id) {
                    break;
                }
            }
            player.data.profile.macros.push(new_macro);
        } else {
            // `_macroses.put(id, macro)` keeps the place of the macro it replaces.
            match player
                .data
                .profile
                .macros
                .iter_mut()
                .find(|m| m.id == new_macro.id)
            {
                Some(old) => *old = new_macro,
                None => player.data.profile.macros.push(new_macro),
            }
        }
        self.send_macro_update(id);
    }

    /// Java `RequestDeleteMacro.runImpl` -> `Player.deleteMacro` -> `MacroList.deleteMacro`.
    pub(crate) fn on_request_delete_macro(&mut self, id: ObjectId, macro_id: i32) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.data.profile.macros.retain(|m| m.id != macro_id);
        // The shortcuts of the macro go too, one list each.
        let places: Vec<(i32, i32)> = player
            .data
            .shortcuts
            .iter()
            .filter(|s| s.kind == crate::shortcuts::TYPE_MACRO && s.shortcut_id == macro_id)
            .map(|s| (s.slot, s.page))
            .collect();
        for (slot, page) in places {
            self.delete_shortcut(id, slot, page);
        }
        self.send_macro_update(id);
    }

    /// Java `MacroList.sendUpdate`: the revision goes up and the whole list is sent.
    pub(crate) fn send_macro_update(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.leftover.macro_revision += 1;
        let packets = macro_list_packets(&player.data.profile.macros, player.leftover.macro_revision);
        for packet in packets {
            self.send(id, &packet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals_ignore_case_like_java() {
        assert!(equals_ignore_case("Heal", "hEAL"));
        assert!(!equals_ignore_case("Heal", "Heals"));
        assert!(equals_ignore_case("", ""));
        assert!(!equals_ignore_case("a", "b"));
    }

    #[test]
    fn an_empty_list_is_one_packet_without_a_macro() {
        let packets = macro_list_packets(&[], 7);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].revision, 7);
        assert_eq!(packets[0].count, 0);
        assert!(packets[0].macro_entry.is_none());
    }
}
