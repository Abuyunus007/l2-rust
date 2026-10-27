//! The GM flags of the admin page: `//gmliston`, `//gmlistoff` (the GM list that `/gmlist`
//! shows), `//silence` (refuse private messages), `//diet` (no weight penalty) and `//tradeoff`
//! (refuse trades), each with the code that reads the flag.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminAdmin.java
// Java: data/xml/AdminData.java (the GM list), network/clientpackets/RequestGmList.java
// Java: entity/actor/Player.java (setSilenceMode, isSilenceMode, setDietMode, getWeightPenalty)
//
// # Where the flags are read
// - silence: the whisper channel (`chat.rs`) answers "not online" for a receiver who is silent,
//   `EtcStatusUpdate` shows the chat block icon.
// - diet: `refresh_overloaded` (no penalty, no skill), `EtcStatusUpdate`, the `PlayerWeight` and
//   `TargetWeight` skill conditions, and `validate_weight` (for a GM whose level allows
//   transactions).
// - tradeoff: `TradeRequest` (`trade/request.rs`), the flag is `TradeState::refusal`.
// - the GM list: `World::send_gm_list` (`RequestGmList`).
//
// # Left out, with the system that brings it
// - `RequestGmList` is decoded since L-03 (`ClientInput::GmList`); `RequestPetition` (the "no GM
//   online" check) is not ported.
// - Java's `Pet` weight check reads the owner's diet mode too (`Pet.java:1278`); the pet code is
//   not changed here.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// What a GM's admin page toggles on the character (Java `Player._silenceMode`,
/// `_silenceModeExcluded`, `_dietMode` and the entry in `AdminData._gmList`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GmFlags {
    /// Java `_silenceMode`.
    pub silence: bool,
    /// Java `_silenceModeExcluded`: who may still be answered (`SilenceModeExclude`).
    silence_excluded: Vec<ObjectId>,
    /// Java `_dietMode`.
    pub diet: bool,
    /// Java `PlayerAppearance._nameColor` after `//setcolor`: `None` is the colour of the access
    /// level. NOTE(java-quirk): Java keeps the name colour in no table (the `characters` table has
    /// no such column), so a changed colour lasts until the character leaves the world.
    pub name_color: Option<i32>,
    /// Java `GameClient._trace`: the five hops the client sent with `EnterWorld`.
    pub trace: Option<[[u8; 4]; 5]>,
}

impl GmFlags {
    /// Java `isSilenceMode(playerObjId)`: does this player block private messages from `sender`?
    pub fn is_silent_for(&self, sender: ObjectId, exclude_enabled: bool) -> bool {
        if exclude_enabled && self.silence && !self.silence_excluded.is_empty() {
            return !self.silence_excluded.contains(&sender);
        }
        self.silence
    }

    /// Java `addSilenceModeExcluded(playerObjId)`.
    pub fn add_silence_exclusion(&mut self, player: ObjectId) {
        if !self.silence_excluded.contains(&player) {
            self.silence_excluded.push(player);
        }
    }

    /// Java `setSilenceMode(mode)`: the exclusion list is cleared on each change.
    pub fn set_silence(&mut self, mode: bool) {
        self.silence = mode;
        self.silence_excluded.clear();
    }
}

/// Java `AdminAdmin.ADMIN_COMMANDS` (the flag words).
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_gmliston", admin_gmliston),
        ("admin_gmlistoff", admin_gmlistoff),
        ("admin_silence", admin_silence),
        ("admin_diet", admin_diet),
        ("admin_tradeoff", admin_tradeoff),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// `addGm(activeChar, false)`.
fn admin_gmliston(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.gm_list_set(gm, false);
    world.send_sys_message(gm, "Registered into GM list.");
    world.show_admin_html(gm, "gm_menu.htm");
    Ok(())
}

/// `addGm(activeChar, true)`.
fn admin_gmlistoff(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    world.gm_list_set(gm, true);
    world.send_sys_message(gm, "Removed from GM list.");
    world.show_admin_html(gm, "gm_menu.htm");
    Ok(())
}

fn admin_silence(world: &mut World, gm: ObjectId, _command: &str) -> Outcome {
    let silent = world.players.get(&gm).is_some_and(|p| p.gm.silence);
    world.set_silence_mode(gm, !silent);
    world.send(
        gm,
        &SystemMessage::new(if silent {
            SystemMessageId::MessageAcceptanceMode
        } else {
            SystemMessageId::MessageRefusalMode
        }),
    );
    world.show_admin_html(gm, "gm_menu.htm");
    Ok(())
}

fn admin_diet(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // NOTE(java-quirk): Java reads the second word, and only when it is not "on" reads a third
    // one and compares that with "off". `//diet off` has no third word, so the exception makes
    // it a toggle, like a bare `//diet`.
    let mut st = Tokens::new(command);
    st.next_token();
    let explicit = match st.next_token() {
        None => None,
        Some(mode) if mode.eq_ignore_ascii_case("on") => Some(true),
        Some(_) => match st.next_token() {
            None => None,
            Some(mode) if mode.eq_ignore_ascii_case("off") => Some(false),
            Some(_) => {
                // Neither branch: no change and no message.
                world.refresh_overloaded(gm);
                world.show_admin_html(gm, "gm_menu.htm");
                return Ok(());
            }
        },
    };
    let on = explicit.unwrap_or_else(|| !world.players.get(&gm).is_some_and(|p| p.gm.diet));
    if let Some(player) = world.players.get_mut(&gm) {
        player.gm.diet = on;
    }
    world.send_sys_message(gm, if on { "Diet mode on" } else { "Diet mode off" });
    // The `finally` block.
    world.refresh_overloaded(gm);
    world.show_admin_html(gm, "gm_menu.htm");
    Ok(())
}

fn admin_tradeoff(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    // `command.substring(15)`: a command shorter than that throws and toggles.
    let mode: Option<String> = (command.chars().count() >= 15).then(|| command.chars().skip(15).collect());
    let set = |world: &mut World, on: bool| {
        if let Some(player) = world.players.get_mut(&gm) {
            player.trade.refusal = on;
        }
        world.send_sys_message(
            gm,
            if on {
                "Trade refusal enabled"
            } else {
                "Trade refusal disabled"
            },
        );
    };
    match mode {
        Some(mode) if mode.eq_ignore_ascii_case("on") => set(world, true),
        Some(mode) if mode.eq_ignore_ascii_case("off") => set(world, false),
        Some(_) => {}
        None => {
            let refusing = world.players.get(&gm).is_some_and(|p| p.trade.refusal);
            set(world, !refusing);
        }
    }
    world.show_admin_html(gm, "gm_menu.htm");
    Ok(())
}

impl World {
    /// Java `Player.setSilenceMode(mode)`: the flag, the exclusion list cleared, `EtcStatusUpdate`.
    pub(crate) fn set_silence_mode(&mut self, id: ObjectId, mode: bool) {
        if let Some(player) = self.players.get_mut(&id) {
            player.gm.set_silence(mode);
        }
        self.send_etc_status_update(id);
    }

    /// Java `AdminData.sendListToPlayer(player)` (`RequestGmList`): the GMs the player may see;
    /// a GM sees the hidden ones too.
    pub(crate) fn send_gm_list(&self, id: ObjectId) {
        let include_hidden = self.is_gm(id);
        if !self.is_gm_online(include_hidden) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::ThereAreNoGmsCurrentlyVisibleInThePublicListAsTheyMayBePerformingOtherFunctionsAtTheMoment,
                ),
            );
            return;
        }
        self.send(id, &SystemMessage::new(SystemMessageId::GmList));
        // Java walks a `ConcurrentHashMap`; the order here is the object id.
        let mut gms: Vec<(ObjectId, String)> = self
            .gm_list_ids(include_hidden)
            .into_iter()
            .filter_map(|gm| self.players.get(&gm).map(|p| (gm, p.data.name.clone())))
            .collect();
        gms.sort_by_key(|(object_id, _)| object_id.0);
        for (_, name) in gms {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::GmC1).with(SystemMessageParam::Text(name)),
            );
        }
    }

    /// Java `EnterWorld`, the GM part: `GMStartupSilence` and `GMStartupDietMode` (the GM list
    /// entry is `admin_on_player_enter`, `gm_tools.rs`). Called for every player who enters.
    pub(crate) fn gm_flags_on_enter(&mut self, id: ObjectId) {
        if !self.is_gm(id) {
            return;
        }
        let access = self.players.get(&id).map_or(0, |p| p.data.access_level);
        let has = |word: &str| {
            self.data
                .admin_rights()
                .is_some_and(|rights| rights.has_access(word, access))
        };
        // `break gmStartupProcess` of GMStartupBuilderHide skips silence and diet.
        let builder_hide = self.settings.gm_startup_builder_hide && has("admin_hide");
        let silence = !builder_hide && self.settings.gm_startup_silence && has("admin_silence");
        let diet = !builder_hide && self.settings.gm_startup_diet_mode && has("admin_diet");
        if let Some(player) = self.players.get_mut(&id) {
            player.gm.diet = diet;
            if silence {
                player.gm.set_silence(true);
            }
        }
        if diet {
            self.refresh_overloaded(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_exclusion_lets_an_answered_player_through() {
        let mut flags = GmFlags::default();
        let a = ObjectId(1);
        let b = ObjectId(2);
        assert!(!flags.is_silent_for(a, true));
        flags.set_silence(true);
        assert!(flags.is_silent_for(a, true));
        flags.add_silence_exclusion(a);
        assert!(!flags.is_silent_for(a, true));
        assert!(flags.is_silent_for(b, true));
        // Without `SilenceModeExclude` everybody is blocked.
        assert!(flags.is_silent_for(a, false));
        // The list is cleared on each change.
        flags.set_silence(true);
        assert!(flags.is_silent_for(a, true));
    }
}
