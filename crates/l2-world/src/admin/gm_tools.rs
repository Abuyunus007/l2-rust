//! GM tools: `//kick`, `//kick_non_gm`, `//character_disconnect`, `//gm`, `//gmchat`, `//snoop`,
//! `//online` and `//serverinfo`, with the GM list (Java `AdminData._gmList`) and the snoop
//! tables of `Player`.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminKick,AdminDisconnect,AdminGm,
// Java: AdminGmChat,AdminOnline,AdminServerInfo}.java
// Java: data/xml/AdminData.java (the GM list), entity/actor/Player.java (broadcastSnoop,
// Java: addSnooper, addSnooped), network/clientpackets/SnoopQuit.java, serverpackets/Snoop.java
//
// # The GM list
// `EnterWorld` puts every GM into the list, hidden unless `GMStartupAutoList` is on and the GM
// has the right for `//gmliston`. The list is what `broadcastToGMs` and the petition system
// use. `//gmliston` and `//gmlistoff` (AdminAdmin) change the hidden flag through
// [`World::gm_list_set`].
//
// # Snooping
// Java hooks `CreatureSay.runImpl`: every chat line a snooped player receives is also sent to
// each snooper, followed by a `Snoop` packet. Here `World::send_bytes` recognises the encoded
// `CreatureSay` of a player with snoopers.
//
// NOTE(java-quirk): a GM who snoops itself, or two GMs who snoop each other, make Java forward
// the same line forever (a `StackOverflowError`). The forwarding stops after a few levels.
//
// Left out, with the reason:
// - `%os_ver%`, the memory and the JVM thread and garbage collector numbers of `//serverinfo`
//   have no Rust counterpart; see [`server_info`].

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::IpAddr;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, CreatureSay, SayBody, SenderField, Snoop, SystemMessage};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens};
use crate::command::LeaveReason;
use crate::world::{World, game_time_minutes, unix_ms_now};
use crate::zones::ZoneId;

type Outcome = Result<(), JavaException>;

/// Java `AdminData._gmList`: the GMs and whether each is hidden.
#[derive(Debug, Default)]
pub(crate) struct GmList {
    entries: BTreeMap<ObjectId, bool>,
}

/// The snoop sets of `Player`: `_snoopListener` and `_snoopedPlayer`.
#[derive(Debug, Default)]
pub(crate) struct SnoopTable {
    /// Player -> the GMs who snoop it.
    listeners: HashMap<ObjectId, BTreeSet<ObjectId>>,
    /// GM -> the players it snoops.
    snooped: HashMap<ObjectId, BTreeSet<ObjectId>>,
    /// Forwarding depth, against the endless loops of the quirk.
    depth: Cell<u8>,
}

/// The most nested forwards of a snooped line.
const MAX_SNOOP_DEPTH: u8 = 4;

/// Java `AdminKick`, `AdminDisconnect`, `AdminGm`, `AdminGmChat`, `AdminOnline`,
/// `AdminServerInfo`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let kick: AdminHandler = admin_kick;
    handlers.register("admin_kick", kick);
    handlers.register("admin_kick_non_gm", kick);
    handlers.register("admin_character_disconnect", admin_character_disconnect);
    handlers.register("admin_gm", admin_gm);
    let chat: AdminHandler = admin_gm_chat;
    for command in ["admin_gmchat", "admin_snoop", "admin_gmchat_menu"] {
        handlers.register(command, chat);
    }
    handlers.register("admin_online", admin_online);
    handlers.register("admin_serverinfo", server_info);
}

impl World {
    /// Java `AdminData.addGm(player, hidden)`, and the hidden flag of `//gmliston`.
    pub(crate) fn gm_list_set(&mut self, id: ObjectId, hidden: bool) {
        self.admin.ctl.gm_list.entries.insert(id, hidden);
    }

    /// Java `AdminData.deleteGm(player)`.
    pub(crate) fn gm_list_remove(&mut self, id: ObjectId) {
        self.admin.ctl.gm_list.entries.remove(&id);
    }

    /// Java `AdminData.getAllGms(includeHidden)` for the GMs in the world.
    pub(crate) fn gm_list_ids(&self, include_hidden: bool) -> Vec<ObjectId> {
        self.admin
            .ctl
            .gm_list
            .entries
            .iter()
            .filter(|(id, hidden)| (include_hidden || !**hidden) && self.players.contains_key(id))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Java `AdminData.isGmOnline(includeHidden)`.
    pub(crate) fn is_gm_online(&self, include_hidden: bool) -> bool {
        !self.gm_list_ids(include_hidden).is_empty()
    }

    /// Java `AdminData.broadcastToGMs(packet)`.
    pub(crate) fn broadcast_to_gms(&self, packet: &impl l2_net::Encode) {
        let bytes = packet.to_bytes();
        for gm in self.gm_list_ids(true) {
            self.send_bytes(gm, bytes.clone());
        }
    }

    /// What `EnterWorld` does for the GM list and the counters of `//online`.
    pub(crate) fn admin_on_player_enter(&mut self, id: ObjectId) {
        if self.is_gm(id) {
            let level = self.players.get(&id).map_or(0, |p| p.data.access_level);
            let auto_list = self.admin.ctl.settings.gm_startup_auto_list
                && self
                    .data
                    .admin_rights()
                    .is_some_and(|rights| rights.has_access("admin_gmliston", level));
            self.gm_list_set(id, !auto_list);
        }
        // `World.MAX_CONNECTED_COUNT` (Java's GUI raises it once a second).
        let count = self.players.len();
        if count > self.admin.ctl.max_connected {
            self.admin.ctl.max_connected = count;
        }
    }

    /// What `Player.deleteMe` does for the GM list and the snoop sets.
    pub(crate) fn admin_on_player_leave(&mut self, id: ObjectId) {
        self.gm_list_remove(id);
        self.debug_forget(id);
        self.petition_forget_player(id);
        let snoop = &mut self.admin.ctl.snoop;
        // `for (Player player : _snoopedPlayer) player.removeSnooper(this)`
        for snooped in snoop.snooped.remove(&id).unwrap_or_default() {
            if let Some(listeners) = snoop.listeners.get_mut(&snooped) {
                listeners.remove(&id);
            }
        }
        // `for (Player player : _snoopListener) player.removeSnooped(this)`
        for listener in snoop.listeners.remove(&id).unwrap_or_default() {
            if let Some(snooped) = snoop.snooped.get_mut(&listener) {
                snooped.remove(&id);
            }
        }
        snoop.listeners.retain(|_, set| !set.is_empty());
        snoop.snooped.retain(|_, set| !set.is_empty());
    }

    /// `SnoopQuit.runImpl`.
    pub(crate) fn on_snoop_quit(&mut self, player: ObjectId, snoop_id: i32) {
        let target = ObjectId(snoop_id);
        // `World.getPlayer(_snoopID)`
        if !self.players.contains_key(&target) {
            return;
        }
        let snoop = &mut self.admin.ctl.snoop;
        if let Some(set) = snoop.listeners.get_mut(&target) {
            set.remove(&player);
        }
        if let Some(set) = snoop.snooped.get_mut(&player) {
            set.remove(&target);
        }
    }

    /// Java `CreatureSay.runImpl` -> `Player.broadcastSnoop`, for the bytes a player is sent.
    pub(crate) fn snoop_sent_bytes(&self, to: ObjectId, bytes: &[u8]) {
        let Some(listeners) = self.admin.ctl.snoop.listeners.get(&to) else {
            return;
        };
        let Some(say) = decode_say(bytes) else {
            return;
        };
        let depth = &self.admin.ctl.snoop.depth;
        if depth.get() >= MAX_SNOOP_DEPTH {
            return;
        }
        let name = self
            .players
            .get(&to)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        depth.set(depth.get() + 1);
        for listener in listeners {
            if *listener == to {
                continue;
            }
            self.send_bytes(*listener, bytes.to_vec());
            self.send(
                *listener,
                &Snoop {
                    convo_id: to.0,
                    name: name.clone(),
                    chat_type: say.chat_type,
                    speaker: say.speaker.clone(),
                    message: say.message.clone(),
                },
            );
        }
        depth.set(depth.get() - 1);
    }

    /// Whether anybody snoops anybody (the hook of `send_bytes` looks here first).
    pub(crate) fn has_snoopers(&self) -> bool {
        !self.admin.ctl.snoop.listeners.is_empty()
    }
}

/// The fields of a `CreatureSay` with a sender name and a text, which are the only ones a
/// `Snoop` can show. Anything else (a line made of a system message id) is not snooped.
struct SnoopedSay {
    chat_type: ChatType,
    speaker: String,
    message: String,
}

fn decode_say(bytes: &[u8]) -> Option<SnoopedSay> {
    use l2_net::PacketReader;
    let (&opcode, rest) = bytes.split_first()?;
    if opcode != CreatureSay::OPCODE.id() {
        return None;
    }
    let mut reader = PacketReader::new(rest);
    let _sender = reader.i32().ok()?;
    let chat_type = ChatType::from_client_id(reader.i32().ok()?)?;
    let speaker = reader.string();
    let _message_id = reader.i32().ok()?;
    let message = reader.string();
    // Exactly the packet of the `(sender, type, name, text)` form: the writer ends with the
    // text's terminator.
    let expected =
        1 + 4 + 4 + (speaker.encode_utf16().count() + 1) * 2 + 4 + (message.encode_utf16().count() + 1) * 2;
    (bytes.len() == expected).then_some(SnoopedSay {
        chat_type,
        speaker,
        message,
    })
}

// ------------------------------------------------------------------------------ AdminKick

/// Java `AdminKick.onCommand`.
fn admin_kick(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_kick") {
        let mut st = Tokens::new(command);
        if super::control::count_tokens(command) > 1 {
            st.next_token();
            let name = st.next_token().unwrap_or_default();
            if let Some(player) = world.player_by_name(name) {
                let kicked = world.object_name(player).unwrap_or_default();
                world.disconnect_with_leave_world(player);
                world.send_sys_message(gm, &format!("You kicked {kicked} from the game."));
            }
        }
    }

    if command.starts_with("admin_kick_non_gm") {
        let mut counter = 0;
        let mut ids: Vec<ObjectId> = world.players.keys().copied().collect();
        ids.sort_unstable();
        for player in ids {
            if !world.is_gm(player) {
                counter += 1;
                world.disconnect_with_leave_world(player);
            }
        }
        world.send_sys_message(gm, &format!("Kicked {counter} players."));
    }
    Ok(())
}

impl World {
    /// Java `Disconnection.of(player).storeAndDeleteWith(LeaveWorld.STATIC_PACKET)`.
    pub(crate) fn disconnect_with_leave_world(&mut self, player: ObjectId) {
        self.leave(player, LeaveReason::Logout);
    }
}

/// Java `AdminDisconnect.onCommand`.
fn admin_character_disconnect(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_character_disconnect" {
        let Some(player) = world.target_of(gm).filter(|t| world.players.contains_key(t)) else {
            return Ok(());
        };
        if player == gm {
            world.send_sys_message(gm, "You cannot logout your own character.");
        } else {
            let name = world.object_name(player).unwrap_or_default();
            world.send_sys_message(gm, &format!("Character {name} disconnected from server."));
            world.disconnect_with_leave_world(player);
        }
    }
    Ok(())
}

// --------------------------------------------------------------------------------- AdminGm

/// Java `AdminGm.onCommand`: `//gm` turns the GM status off for this session.
fn admin_gm(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_gm" && world.is_gm(gm) {
        world.gm_list_remove(gm);
        // `setAccessLevel(0)`: the name and title colours of level 0, `broadcastUserInfo`. The
        // `accesslevel` column is not touched, so the next login is a GM again.
        if let Some(player) = world.players.get_mut(&gm) {
            player.data.access_level = 0;
        }
        world.broadcast_user_info(gm);
        world.send_sys_message(gm, "You no longer have GM status.");
        let name = world.object_name(gm).unwrap_or_default();
        tracing::info!("GM: {name}({}) turned his GM status off", gm.0);
    }
    Ok(())
}

// ----------------------------------------------------------------------------- AdminGmChat

/// Java `AdminGmChat.onCommand`.
fn admin_gm_chat(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command.starts_with("admin_gmchat") {
        // `handleGmChat`: a command too short for the offset is ignored ("Who cares?").
        let offset = if command.starts_with("admin_gmchat_menu") {
            18
        } else {
            13
        };
        if let Some(text) = command.get(offset..) {
            let name = world.object_name(gm).unwrap_or_default();
            world.broadcast_to_gms(&CreatureSay {
                sender_object_id: 0,
                chat_type: ChatType::Alliance,
                sender: SenderField::Name(name),
                message_id: -1,
                body: SayBody::Text(text.to_owned()),
            });
        }
    } else if command.starts_with("admin_snoop") {
        snoop(world, gm, command);
    }

    if command.starts_with("admin_gmchat_menu") {
        world.show_admin_html(gm, "gm_menu.htm");
    }
    Ok(())
}

/// Java `AdminGmChat.snoop`.
fn snoop(world: &mut World, gm: ObjectId, command: &str) {
    let mut target = None;
    if command.len() > 12 {
        target = command.get(12..).and_then(|name| world.player_by_name(name));
    }
    if target.is_none() {
        target = world.target_of(gm);
    }
    let Some(target) = target else {
        world.send(gm, &SystemMessage::new(SystemMessageId::SelectTarget));
        return;
    };
    if !world.players.contains_key(&target) {
        world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
        return;
    }
    let snoop = &mut world.admin.ctl.snoop;
    snoop.listeners.entry(target).or_default().insert(gm);
    snoop.snooped.entry(gm).or_default().insert(target);
}

// ----------------------------------------------------------------------------- AdminOnline

/// Java `AdminOnline.onCommand`.
fn admin_online(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if !command.eq_ignore_ascii_case("admin_online") {
        return Ok(());
    }
    let mut ips: Vec<IpAddr> = Vec::new();
    let (mut total, mut online, mut offline) = (0, 0, 0);
    let (mut peace, mut not_peace, mut instanced, mut combat) = (0, 0, 0, 0);
    let mut ids: Vec<ObjectId> = world.players.keys().copied().collect();
    ids.sort_unstable();
    for id in ids {
        let Some(player) = world.players.get(&id) else {
            continue;
        };
        if let Some(ip) = player.client_ip
            && !ips.contains(&ip)
        {
            ips.push(ip);
        }
        total += 1;
        if world.is_offline_trader(id) {
            offline += 1;
        } else {
            online += 1;
        }
        if world.is_inside_zone(id, ZoneId::Peace) {
            peace += 1;
        } else {
            not_peace += 1;
        }
        if world.objects.get(&id).is_some_and(|e| e.instance_id > 0) {
            instanced += 1;
        }
        if world.has_attack_stance_task(id)
            || world.pvp_flag_of(id) > 0
            || world.is_inside_zone(id, ZoneId::Pvp)
            || world.is_inside_zone(id, ZoneId::Siege)
        {
            combat += 1;
        }
    }
    let max_connected = world.admin.ctl.max_connected.max(world.players.len());
    world.admin.ctl.max_connected = max_connected;
    world.send_sys_message(gm, "Online Player Report");
    world.send_sys_message(gm, &format!("Total count: {total}"));
    world.send_sys_message(gm, &format!("Total online: {online}"));
    world.send_sys_message(gm, &format!("Total offline: {offline}"));
    world.send_sys_message(gm, &format!("Max connected: {max_connected}"));
    world.send_sys_message(gm, &format!("Unique IPs: {}", ips.len()));
    world.send_sys_message(gm, &format!("In peace zone: {peace}"));
    world.send_sys_message(gm, &format!("Not in peace zone: {not_peace}"));
    world.send_sys_message(gm, &format!("In instances: {instanced}"));
    world.send_sys_message(gm, &format!("In combat: {combat}"));
    Ok(())
}

// -------------------------------------------------------------------------- AdminServerInfo

/// `DateTimeFormatter.ofPattern("hh:mm a")` of the local time of `now_ms`.
fn format_local_time(now_ms: i64) -> String {
    let local = now_ms.saturating_add(l2_util::time::local_offset_millis(now_ms));
    let minutes = local.div_euclid(60_000).rem_euclid(24 * 60);
    let (hour, minute) = (minutes / 60, minutes % 60);
    let twelve = if hour % 12 == 0 { 12 } else { hour % 12 };
    format!("{twelve:02}:{minute:02} {}", if hour < 12 { "AM" } else { "PM" })
}

/// Java `AdminServerInfo.getServerUpTime`.
fn format_up_time(millis: i64) -> String {
    let millis = millis.max(0);
    let days = millis / 86_400_000;
    let hours = (millis % 86_400_000) / 3_600_000;
    let minutes = (millis % 3_600_000) / 60_000;
    format!("{days} Days, {hours} Hours, {minutes} Minutes")
}

/// Java `AdminServerInfo.onCommand`.
///
/// The page is Java's. The JVM numbers have no counterpart in a Rust process: the heap numbers
/// are the resident memory of the process on Linux (0 elsewhere), `%os_ver%` is not known to the
/// standard library, and the thread and garbage collector counters read 0 / `none`.
fn server_info(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command != "admin_serverinfo" {
        return Ok(());
    }
    let now = unix_ms_now();
    let minutes = game_time_minutes(now);
    let ctl = &world.admin.ctl;
    let slots = format!("{}/{}", world.players.len(), ctl.settings.maximum_online_users);
    // `getPlayersCount("OFF_TRADE")`: the players without a client.
    let off_trade = world
        .players
        .keys()
        .filter(|id| !world.sinks.contains_key(id))
        .count();
    let online_gm = world
        .gm_list_ids(true)
        .into_iter()
        .filter(|id| world.sinks.contains_key(id))
        .count();
    let mut real_ips: BTreeSet<Option<IpAddr>> = BTreeSet::new();
    for (id, player) in &world.players {
        if world.sinks.contains_key(id) {
            real_ips.insert(player.client_ip);
        }
    }
    let used = super::control::used_memory_bytes() / 0x10_0000;
    let uptime = format_up_time(now - ctl.settings.server_start_ms);
    let night = minutes < 6 * 60;
    let geodata = if world.path_requests.is_some() {
        "Enabled"
    } else {
        "Disabled"
    };

    let mut html = world.admin_html_file(gm, "data/html/admin/serverinfo.htm");
    html.replace("%os_name%", std::env::consts::OS)
        .replace("%os_ver%", std::env::consts::ARCH)
        .replace("%slots%", &slots)
        .replace("%gameTime%", &format!("{}:{}", minutes / 60, minutes % 60))
        .replace("%dayNight%", if night { "Night" } else { "Day" })
        .replace("%geodata%", geodata)
        .replace("%serverTime%", &format_local_time(now))
        .replace("%serverUpTime%", &uptime)
        .replace("%onlineAll%", &world.players.len().to_string())
        .replace("%offlineTrade%", &off_trade.to_string())
        .replace("%onlineGM%", &online_gm.to_string())
        .replace("%onlineReal%", &real_ips.len().to_string())
        .replace("%usedMem%", &format!("{used} Mb"))
        .replace("%freeMem%", "0 Mb")
        .replace("%totalMem%", &format!("{used} Mb"))
        .replace("%live%", "0")
        .replace("%nondaemon%", "0")
        .replace("%daemon%", "0")
        .replace("%peak%", "0")
        .replace("%totalstarted%", "0")
        .replace("%gcol%", "none")
        .replace("%colcount%", "0")
        .replace("%coltime%", "0");
    world.send_admin_html(gm, &html);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_time_has_days_hours_minutes() {
        assert_eq!(format_up_time(0), "0 Days, 0 Hours, 0 Minutes");
        assert_eq!(
            format_up_time(2 * 86_400_000 + 3 * 3_600_000 + 4 * 60_000 + 59_000),
            "2 Days, 3 Hours, 4 Minutes"
        );
    }

    #[test]
    fn a_creature_say_is_read_back() {
        use l2_net::Encode;
        let say = CreatureSay {
            sender_object_id: 7,
            chat_type: ChatType::Whisper,
            sender: SenderField::Name("Bob".to_owned()),
            message_id: -1,
            body: SayBody::Text("hello".to_owned()),
        };
        let bytes = say.to_bytes();
        let read = decode_say(&bytes).unwrap();
        assert_eq!(read.chat_type, ChatType::Whisper);
        assert_eq!(read.speaker, "Bob");
        assert_eq!(read.message, "hello");
        // A line of a system message id (a char id, no text) is not snooped.
        let by_id = CreatureSay {
            sender_object_id: 0,
            chat_type: ChatType::General,
            sender: SenderField::CharId(5),
            message_id: 12,
            body: SayBody::Parameters(vec![]),
        };
        assert!(decode_say(&by_id.to_bytes()).is_none());
        assert!(decode_say(&[1, 2, 3]).is_none());
    }
}
