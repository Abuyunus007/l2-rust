//! The console GM: what the admin GUI (G-03) does through the game server's admin endpoint.
//!
//! There is no Java class for this; the Java server has no remote console. Everything the GUI
//! can do is a `//command` of a GM, so the endpoint hands the world a [`GuiRequest`] and the
//! world runs the command as a **virtual GM**: a player named "GUI" with the master access level
//! that is inside the world only while the command runs. Its packets go to a capturing sink, and
//! the system messages, chat lines and html pages it was sent come back as plain text lines.
// # What the virtual GM is not
// It is not registered as a world object (no region, no known list), so other players never see
// it and it does not count as online. `//kick GUI` finds nobody once the command is over.
// A command that needs the GM's position (`//recall`, `//teleportto`) finds none and does
// nothing.
//
// # The audit
// `admin_command` writes the GM audit line (`GUI [id]>command>target>`); the logging layer files
// it under `log/GMAudit/GUI.txt`, as it does for every GM name.

use std::sync::mpsc;

use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{
    ChatType, CreatureSay, NpcHtmlMessage, SayBody, SenderField, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use crate::command::LeaveReason;
use crate::outbox::PacketSink;
use crate::player::{Player, PlayerData};
use crate::world::{TICK, World, unix_ms_now};

/// The object id of the virtual GM. Real ids start far below it (`IdFactory`).
pub const GUI_GM_ID: ObjectId = ObjectId(0x7FFF_FF00);
/// The GM name in the audit log and in the shutdown log lines.
pub const GUI_GM_NAME: &str = "GUI";

/// What the GUI asks of the world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiRequest {
    /// An admin command without the slashes (`kick Amy`). `with_html` keeps the html pages the
    /// command shows (the console wants them, the buttons do not).
    Command { command: String, with_html: bool },
    /// `//teleport_character x y z` on the named character.
    Teleport { name: String, x: i32, y: i32, z: i32 },
    /// A private message from "[GM]".
    Message { name: String, text: String },
    /// What `//serverinfo` shows.
    Info,
}

/// The answer to a [`GuiRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiReply {
    /// What the command told its GM, one entry per line.
    Lines(Vec<String>),
    Info(ServerInfo),
}

/// Java `AdminServerInfo`, the numbers without the html.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub uptime_secs: u64,
    pub online: u32,
    /// Resident memory in bytes, 0 where the platform does not tell.
    pub memory_bytes: u64,
    pub ticks: u64,
    pub tick_ms: u32,
    /// The measured duration of `World::tick` over the last samples, in microseconds (M9-H02).
    pub tick_avg_us: u32,
    pub tick_p99_us: u32,
    pub tick_samples: u32,
    /// Seconds left of a running shutdown or restart countdown.
    pub shutdown_in_secs: Option<u32>,
}

/// Collects the packets of the virtual GM.
struct CaptureSink(mpsc::Sender<Vec<u8>>);

impl PacketSink for CaptureSink {
    fn send(&self, bytes: Vec<u8>) {
        // The receiver lives until the command is over; a send after that is impossible.
        let _ = self.0.send(bytes);
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

fn gui_player_data(access_level: i32) -> PlayerData {
    PlayerData {
        object_id: GUI_GM_ID,
        account: GUI_GM_NAME.to_owned(),
        name: GUI_GM_NAME.to_owned(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level,
        x: 0,
        y: 0,
        z: 0,
        heading: 0,
        cur_hp: 1.0,
        max_hp: 1.0,
        cur_mp: 1.0,
        max_mp: 1.0,
        cur_cp: 1.0,
        max_cp: 1.0,
        online_time: 0,
        items: Vec::new(),
        skills: Vec::new(),
        shortcuts: Vec::new(),
        profile: Default::default(),
    }
}

impl World {
    /// Answers one request of the admin GUI (`AdminQuery::Gui`).
    pub(crate) fn on_gui_request(&mut self, request: GuiRequest) -> GuiReply {
        match request {
            GuiRequest::Command { command, with_html } => {
                GuiReply::Lines(self.run_console_command(&command, with_html))
            }
            GuiRequest::Teleport { name, x, y, z } => {
                let Some(victim) = self.player_by_name(&name) else {
                    return GuiReply::Lines(vec![format!("{name} is not online.")]);
                };
                let (_, packets) = self.with_gui_gm(|world, gm| {
                    // `//teleport_character` moves the GM's target.
                    if let Some(player) = world.players.get_mut(&gm) {
                        player.target = Some(victim);
                    }
                    world.admin_command(gm, &format!("admin_teleport_character {x} {y} {z}"), false);
                });
                GuiReply::Lines(plain_lines(&packets, false))
            }
            GuiRequest::Message { name, text } => {
                let Some(victim) = self.player_by_name(&name) else {
                    return GuiReply::Lines(vec![format!("{name} is not online.")]);
                };
                // NOTE: Java has no private GM message; this is a whisper whose sender is "[GM]".
                self.send(
                    victim,
                    &CreatureSay {
                        sender_object_id: 0,
                        chat_type: ChatType::Whisper,
                        sender: SenderField::Name("[GM]".to_owned()),
                        message_id: -1,
                        body: SayBody::Text(text),
                    },
                );
                GuiReply::Lines(vec![format!("Message sent to {name}.")])
            }
            GuiRequest::Info => GuiReply::Info(self.server_info()),
        }
    }

    /// Runs `command` (without the slashes) as the virtual GM and returns what it was told.
    pub fn run_console_command(&mut self, command: &str, with_html: bool) -> Vec<String> {
        let command = command.trim().trim_start_matches('/').trim();
        if command.is_empty() {
            return vec!["Enter a command, for example: server_abort".to_owned()];
        }
        let full = format!("admin_{command}");
        let (granted, packets) = self.with_gui_gm(|world, gm| {
            if !world.is_gm(gm) {
                return false;
            }
            // `useConfirm` is off: the GUI's own "are you sure" replaces the ConfirmDlg.
            world.admin_command(gm, &full, false);
            true
        });
        let mut lines = plain_lines(&packets, with_html);
        if !granted {
            lines.insert(
                0,
                "The GUI has no GM rights: AccessLevels.xml/AdminCommands.xml list no master access level."
                    .to_owned(),
            );
        }
        lines
    }

    /// Java `AdminServerInfo`: the numbers.
    fn server_info(&self) -> ServerInfo {
        let ctl = &self.admin.ctl;
        let tick_ms = TICK.as_millis() as u64;
        let started = ctl.settings.server_start_ms;
        let uptime_secs = if started > 0 {
            u64::try_from(unix_ms_now().saturating_sub(started) / 1000).unwrap_or(0)
        } else {
            self.tick * tick_ms / 1000
        };
        let counting = ctl.shutdown.timer.is_some()
            && matches!(
                ctl.shutdown.mode,
                super::control::ShutdownMode::GmShutdown | super::control::ShutdownMode::GmRestart
            );
        ServerInfo {
            uptime_secs,
            online: u32::try_from(self.players.len()).unwrap_or(u32::MAX),
            memory_bytes: super::control::used_memory_bytes(),
            ticks: self.tick,
            tick_ms: u32::try_from(tick_ms).unwrap_or(u32::MAX),
            tick_avg_us: self.tick_stats.average_us(),
            tick_p99_us: self.tick_stats.p99_us(),
            tick_samples: self.tick_stats.samples(),
            shutdown_in_secs: counting.then(|| u32::try_from(ctl.shutdown.seconds).unwrap_or(0)),
        }
    }

    /// Puts the virtual GM into the world, runs `f`, takes it out again and returns what it was
    /// sent.
    fn with_gui_gm<R>(&mut self, f: impl FnOnce(&mut World, ObjectId) -> R) -> (R, Vec<Vec<u8>>) {
        let id = GUI_GM_ID;
        let (sender, receiver) = mpsc::channel();
        let player = Player::from_data(gui_player_data(self.data.master_access_level()), self.tick);
        self.players.insert(id, player);
        self.sinks.insert(id, Box::new(CaptureSink(sender)));
        let result = f(self, id);
        self.players.remove(&id);
        self.sinks.remove(&id);
        self.admin.pending_confirm.remove(&id);
        self.admin.shown_doors.remove(&id);
        self.admin.gm_speed.remove(&id);
        (result, receiver.try_iter().collect())
    }
}

/// The text of the packets a GM would have seen.
fn plain_lines(packets: &[Vec<u8>], with_html: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for bytes in packets {
        let Some((&opcode, body)) = bytes.split_first() else {
            continue;
        };
        match ServerOpcode::from_id(opcode, None) {
            Some(ServerOpcode::SystemMessage) => {
                if let Ok(message) = SystemMessage::decode(&mut PacketReader::new(body)) {
                    lines.push(system_message_text(&message));
                }
            }
            Some(ServerOpcode::NpcHtmlMessage) if with_html => {
                if let Ok(page) = NpcHtmlMessage::decode(&mut PacketReader::new(body)) {
                    lines.extend(html_to_lines(&page.html));
                }
            }
            Some(ServerOpcode::Say2) => {
                if let Some(line) = say_text(body) {
                    lines.push(line);
                }
            }
            _ => {}
        }
    }
    lines
}

/// The client's text of a system message with its `$s1`.. filled in.
pub fn system_message_text(message: &SystemMessage) -> String {
    let mut text = message.id.message().to_owned();
    for (index, param) in message.params.iter().enumerate() {
        let value = match param {
            SystemMessageParam::Text(s) | SystemMessageParam::PlayerName(s) => s.clone(),
            SystemMessageParam::IntNumber(n) => n.to_string(),
            other => format!("{other:?}"),
        };
        text = text.replace(&format!("$s{}", index + 1), &value);
    }
    text
}

/// `CreatureSay` has no decoder (see its module); the server only sends the form with a sender
/// name, and the GUI only needs the text.
fn say_text(body: &[u8]) -> Option<String> {
    let mut r = PacketReader::new(body);
    let _sender_object_id = r.i32().ok()?;
    let chat_type = ChatType::from_client_id(r.i32().ok()?)?;
    let sender = r.string();
    let message_id = r.i32().ok()?;
    if message_id != -1 {
        return None;
    }
    let text = r.string();
    Some(match chat_type {
        ChatType::Announcement => format!("[announcement] {text}"),
        ChatType::CriticalAnnounce => format!("[critical announcement] {text}"),
        _ if sender.is_empty() => text,
        _ => format!("{sender}: {text}"),
    })
}

/// An html page as plain text: tags dropped, line breaks kept.
pub fn html_to_lines(html: &str) -> Vec<String> {
    let mut text = String::with_capacity(html.len());
    let mut chars = html.chars();
    while let Some(c) = chars.next() {
        if c != '<' {
            text.push(c);
            continue;
        }
        let mut tag = String::new();
        for t in chars.by_ref() {
            if t == '>' {
                break;
            }
            tag.push(t);
        }
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "br" | "br1" | "tr" | "p" | "center" | "table" | "body" | "html"
        ) {
            text.push('\n');
        } else if matches!(name.as_str(), "td" | "th") {
            text.push(' ');
        }
    }
    let text = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_protocol_game::SystemMessageId;

    #[test]
    fn html_becomes_lines() {
        let html = "<html><body>Hello &lt;you&gt;<br>a<br1>b<table><tr><td>x</td><td>y</td></tr></table></body></html>";
        assert_eq!(html_to_lines(html), ["Hello <you>", "a", "b", "x y"]);
    }

    #[test]
    fn system_messages_fill_their_parameters() {
        let message = SystemMessage::new(
            SystemMessageId::TheServerWillBeComingDownInS1SecondSPleaseFindASafePlaceToLogOut,
        )
        .with(SystemMessageParam::IntNumber(30));
        assert_eq!(
            system_message_text(&message),
            "The server will be coming down in 30 second(s). Please find a safe place to log out."
        );
        assert_eq!(system_message_text(&SystemMessage::text("hi $s2")), "hi $s2");
    }
}
