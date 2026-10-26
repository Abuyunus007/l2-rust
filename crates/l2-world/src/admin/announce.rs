//! Announcements: `//announce`, `//announce_crit`, `//announce_screen` and the table of stored
//! ones (`//announces list|add|edit|remove|show|restart`), with the automatic ones on the timer
//! wheel and the announcements a player sees when it enters the world.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminAnnouncements.java
// Java: data/sql/AnnouncementsTable.java, mechanics/announce/{Announcement,AutoAnnouncement,
// Java: AnnouncementType,IAnnouncement}.java, entity/World.java (broadcastToAllOnlinePlayers)
// Java: mechanics/html/{PageBuilder,pagehandlers/NextPrevPageHandler,formatters/
// Java: BypassParserFormatter,styles/ButtonsStyle}.java, util/HtmlUtil.java (sendCBHtml)
//
// # The table
// The `announcements` rows are read at the start (`World::load_announcements`) and written as Java
// writes them. The id of a new row is the database's auto increment, so adding one is a
// request/answer through the outbox (`PersistCommand::Announcement`, then
// `WorldCommand::AnnouncementStored`, D-15); the GM hears "successfully added" when the answer is
// in. The Java game server reads the table only at its own start, so what Kain adds shows there
// after Sieghardt's next restart (D-39).
//
// NOTE(java-quirk): `AutoAnnouncement` starts its timer in the constructor, before the row is
// stored. If the INSERT fails Java keeps a running announcement that is in no table (it cannot be
// removed). Here the timer starts when the row is stored, so a failed INSERT starts nothing.
//
// Left out: `EventAnnouncement` (made by the event scripts, M8). Its rows have no table.

use std::collections::BTreeMap;

use l2_protocol_game::server::{ChatType, CreatureSay, ExShowScreenMessage, SayBody, SenderField, ShowBoard};
use l2_util::ObjectId;
use tracing::warn;

use super::control::AdminTimer;
use super::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::dialog::{HtmlActionScope, build_html_action_cache};
use crate::outbox::PersistCommand;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `AnnouncementType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncementType {
    Normal,
    Critical,
    Event,
    AutoNormal,
    AutoCritical,
}

impl AnnouncementType {
    const ALL: [Self; 5] = [
        Self::Normal,
        Self::Critical,
        Self::Event,
        Self::AutoNormal,
        Self::AutoCritical,
    ];

    /// Java `ordinal()`: the value of the `type` column.
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// Java `name()`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Critical => "CRITICAL",
            Self::Event => "EVENT",
            Self::AutoNormal => "AUTO_NORMAL",
            Self::AutoCritical => "AUTO_CRITICAL",
        }
    }

    /// Java `findById`: an unknown id is a warning and `NORMAL`.
    pub fn find_by_id(id: i32) -> Self {
        Self::ALL
            .into_iter()
            .find(|t| t.ordinal() == id)
            .unwrap_or_else(|| {
                warn!("AnnouncementType: Unexistent id specified: {id}!");
                Self::Normal
            })
    }

    /// Java `findByName`: ignores case, an unknown name is a warning and `NORMAL`.
    pub fn find_by_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(name))
            .unwrap_or_else(|| {
                warn!("AnnouncementType: Unexistent name specified: {name}!");
                Self::Normal
            })
    }

    fn is_auto(self) -> bool {
        matches!(self, Self::AutoNormal | Self::AutoCritical)
    }
}

/// One `announcements` row as the database holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnouncementRecord {
    pub id: i32,
    /// `type`: the `AnnouncementType` ordinal.
    pub kind: i32,
    pub initial: i64,
    pub delay: i64,
    pub repeat: i32,
    pub author: String,
    pub content: String,
}

/// A write of the `announcements` table with Java's statement (`Announcement` and
/// `AutoAnnouncement`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnouncementWrite {
    /// `INSERT INTO announcements (type, content, author[, initial, delay, repeat])`. The
    /// persistence task answers with `WorldCommand::AnnouncementStored { ticket, .. }`.
    Insert {
        ticket: u32,
        kind: i32,
        content: String,
        author: String,
        /// `(initial, delay, repeat)` of an `AutoAnnouncement`.
        auto: Option<(i64, i64, i32)>,
    },
    /// `UPDATE announcements SET type, content, author[, initial, delay, repeat] WHERE id`.
    Update {
        id: i32,
        kind: i32,
        content: String,
        author: String,
        auto: Option<(i64, i64, i32)>,
    },
    /// `DELETE FROM announcements WHERE id = ?`.
    Delete { id: i32 },
}

/// The automatic part of Java's `AutoAnnouncement`.
#[derive(Debug, Clone)]
struct AutoPart {
    /// `_initial`, milliseconds.
    initial: i64,
    /// `_delay`, milliseconds.
    delay: i64,
    /// `_repeat`: -1 forever.
    repeat: i32,
    /// `_currentState`.
    current_state: i32,
    /// `_task`.
    timer: Option<TimerId>,
    /// Ends the runs of an earlier `restartMe`.
    generation: u64,
}

/// Java `Announcement` / `AutoAnnouncement`.
#[derive(Debug, Clone)]
pub(crate) struct Announcement {
    id: i32,
    kind: AnnouncementType,
    content: String,
    author: String,
    auto: Option<AutoPart>,
}

/// An announcement whose row the persistence task is inserting.
struct PendingAnnouncement {
    gm: ObjectId,
    announcement: Announcement,
}

/// Java `AnnouncementsTable`.
#[derive(Default)]
pub(crate) struct AnnouncementTable {
    /// `_announcements`, ordered by id like Java's `ConcurrentSkipListMap`.
    map: BTreeMap<i32, Announcement>,
    pending: BTreeMap<u32, PendingAnnouncement>,
    next_ticket: u32,
    next_generation: u64,
}

/// Java `System.lineSeparator()`.
const LINE_SEPARATOR: &str = if cfg!(windows) { "\r\n" } else { "\n" };

/// Java `String.split(regex)` for a plain separator: trailing empty strings are dropped, and a
/// text without the separator is itself.
fn java_split<'a>(text: &'a str, separator: &str) -> Vec<&'a str> {
    let mut parts: Vec<&str> = text.split(separator).collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// Java `AdminAnnouncements`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let handler: AdminHandler = admin_announcements;
    for command in [
        "admin_announce",
        "admin_announce_crit",
        "admin_announce_screen",
        "admin_announces",
    ] {
        handlers.register(command, handler);
    }
}

impl World {
    /// Java `AnnouncementsTable.load()`: the rows read at the start. Call before players enter.
    pub fn load_announcements(&mut self, rows: Vec<AnnouncementRecord>) {
        self.admin.ctl.announcements.map.clear();
        for row in rows {
            let kind = AnnouncementType::find_by_id(row.kind);
            let auto = match kind {
                AnnouncementType::Normal | AnnouncementType::Critical => None,
                AnnouncementType::AutoNormal | AnnouncementType::AutoCritical => Some(AutoPart {
                    initial: row.initial,
                    delay: row.delay,
                    repeat: row.repeat,
                    current_state: row.repeat,
                    timer: None,
                    generation: 0,
                }),
                // `default: continue`
                AnnouncementType::Event => continue,
            };
            let announcement = Announcement {
                id: row.id,
                kind,
                content: row.content,
                author: row.author,
                auto,
            };
            let id = announcement.id;
            self.admin.ctl.announcements.map.insert(id, announcement);
            if kind.is_auto() {
                self.restart_announcement(id);
            }
        }
    }

    /// The number of stored announcements.
    pub fn announcement_count(&self) -> usize {
        self.admin.ctl.announcements.map.len()
    }

    /// Java `AutoAnnouncement.restartMe()`.
    fn restart_announcement(&mut self, id: i32) {
        let table = &mut self.admin.ctl.announcements;
        table.next_generation += 1;
        let generation = table.next_generation;
        let Some(auto) = table.map.get_mut(&id).and_then(|a| a.auto.as_mut()) else {
            return;
        };
        let old = auto.timer.take();
        auto.current_state = auto.repeat;
        auto.generation = generation;
        let delay = u64::try_from(auto.initial).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        let timer = self.timers.schedule_at(
            due,
            TimerKind::Admin(AdminTimer::AutoAnnouncement { id, generation }),
        );
        if let Some(auto) = self
            .admin
            .ctl
            .announcements
            .map
            .get_mut(&id)
            .and_then(|a| a.auto.as_mut())
        {
            auto.timer = Some(timer);
        }
        if let Some(old) = old {
            self.timers.cancel(old);
        }
    }

    /// Java `AutoAnnouncement.run()`.
    pub(crate) fn on_auto_announcement_timer(&mut self, id: i32, generation: u64) {
        let Some(announcement) = self.admin.ctl.announcements.map.get_mut(&id) else {
            return;
        };
        let critical = announcement.kind == AnnouncementType::AutoCritical;
        let content = announcement.content.clone();
        let Some(auto) = announcement.auto.as_mut().filter(|a| a.generation == generation) else {
            return;
        };
        auto.timer = None;
        if auto.current_state != -1 && auto.current_state <= 0 {
            return;
        }
        if auto.current_state != -1 {
            auto.current_state -= 1;
        }
        let delay = u64::try_from(auto.delay).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(delay);
        let timer = self.timers.schedule_at(
            due,
            TimerKind::Admin(AdminTimer::AutoAnnouncement { id, generation }),
        );
        if let Some(auto) = self
            .admin
            .ctl
            .announcements
            .map
            .get_mut(&id)
            .and_then(|a| a.auto.as_mut())
        {
            auto.timer = Some(timer);
        }
        for line in java_split(&content, LINE_SEPARATOR) {
            self.broadcast_announcement(line, critical);
        }
    }

    /// Java `AnnouncementsTable.showAnnouncements(player)`: the packets, the normal ones first,
    /// then the critical and the event ones.
    pub(crate) fn announcement_packets(&self, player: ObjectId) -> Vec<CreatureSay> {
        let Some(name) = self.players.get(&player).map(|p| p.data.name.clone()) else {
            return Vec::new();
        };
        let mut packets = Vec::new();
        for kind in [
            AnnouncementType::Normal,
            AnnouncementType::Critical,
            AnnouncementType::Event,
        ] {
            for announcement in self.admin.ctl.announcements.map.values() {
                if announcement.kind == kind {
                    packets.push(CreatureSay {
                        sender_object_id: 0,
                        chat_type: if kind == AnnouncementType::Critical {
                            ChatType::CriticalAnnounce
                        } else {
                            ChatType::Announcement
                        },
                        sender: SenderField::Name(name.clone()),
                        message_id: -1,
                        body: SayBody::Text(announcement.content.clone()),
                    });
                }
            }
        }
        packets
    }

    /// The answer to `AnnouncementWrite::Insert`: the id the database gave, or a failure.
    pub(crate) fn on_announcement_stored(&mut self, ticket: u32, result: Result<i32, ()>) {
        let Some(pending) = self.admin.ctl.announcements.pending.remove(&ticket) else {
            return;
        };
        let PendingAnnouncement { gm, mut announcement } = pending;
        // `addAnnouncement`: only a stored announcement is put into the table.
        if let Ok(id) = result {
            announcement.id = id;
            let auto = announcement.auto.is_some();
            self.admin.ctl.announcements.map.insert(id, announcement);
            if auto {
                self.restart_announcement(id);
            }
        } else {
            warn!("Announcement: Couldn't store announcement.");
        }
        // The rest of the `add` branch, which ran straight after `addAnnouncement` in Java.
        if self.players.contains_key(&gm) {
            self.send_sys_message(gm, "Announcement has been successfully added!");
            let _ = announces_list(self, gm, None);
        }
    }

    /// Java `HtmlUtil.sendCBHtml(player, html)` (npc origin 0): the page goes to the community
    /// board window in up to three parts.
    pub(crate) fn send_cb_html(&mut self, to: ObjectId, html: &str) {
        const PART: usize = 16250;
        if let Some(player) = self.players.get_mut(&to) {
            player.talk.clear(HtmlActionScope::CommBoardHtml);
            build_html_action_cache(&mut player.talk, HtmlActionScope::CommBoardHtml, 0, html);
        }
        let units: Vec<u16> = html.encode_utf16().collect();
        let part = |from: usize, to: usize| -> String {
            String::from_utf16_lossy(units.get(from..to.min(units.len())).unwrap_or_default())
        };
        let boards: [ShowBoard; 3] = if units.len() < PART {
            [
                ShowBoard::page(Some(html), "101"),
                ShowBoard::page(None, "102"),
                ShowBoard::page(None, "103"),
            ]
        } else if units.len() < PART * 2 {
            [
                ShowBoard::page(Some(&part(0, PART)), "101"),
                ShowBoard::page(Some(&part(PART, units.len())), "102"),
                ShowBoard::page(None, "103"),
            ]
        } else if units.len() < PART * 3 {
            [
                ShowBoard::page(Some(&part(0, PART)), "101"),
                ShowBoard::page(Some(&part(PART, PART * 2)), "102"),
                ShowBoard::page(Some(&part(PART * 2, units.len())), "103"),
            ]
        } else {
            [
                ShowBoard::page(
                    Some("<html><body><br><center>Error: HTML was too long!</center></body></html>"),
                    "101",
                ),
                ShowBoard::page(None, "102"),
                ShowBoard::page(None, "103"),
            ]
        };
        for board in &boards {
            self.send(to, board);
        }
    }
}

// ------------------------------------------------------------------------ AdminAnnouncements

/// Java `AdminAnnouncements.onCommand`.
fn admin_announcements(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::new(command);
    let cmd = st.next_token().unwrap_or_default();
    match cmd {
        "admin_announce" | "admin_announce_crit" | "admin_announce_screen" => {
            if !st.has_more() {
                world.send_sys_message(gm, "Syntax: //announce <text to announce here>");
                return Ok(());
            }
            let mut announce = st.next_token().unwrap_or_default().to_owned();
            while let Some(token) = st.next_token() {
                announce.push(' ');
                announce.push_str(token);
            }
            if cmd == "admin_announce_screen" {
                // `broadcastToAllOnlinePlayersOnScreen`: ten seconds.
                world.broadcast_to_all_online_players(&ExShowScreenMessage::text(&announce, 10_000));
            } else {
                if world.admin.ctl.settings.gm_announcer_name {
                    let name = world.object_name(gm).unwrap_or_default();
                    announce = format!("{announce} [{name}]");
                }
                world.broadcast_announcement(&announce, cmd == "admin_announce_crit");
            }
            world.show_admin_html(gm, "gm_menu.htm");
        }
        "admin_announces" => {
            let sub = st.next_token().unwrap_or_default();
            match sub {
                "add" => return announces_add(world, gm, &mut st),
                "edit" => return announces_edit(world, gm, &mut st),
                "remove" => {
                    let Some(token) = st.next_token() else {
                        world.send_sys_message(gm, "Syntax: //announces remove <announcement id>");
                        return Ok(());
                    };
                    if !is_numeric(token) {
                        world.send_sys_message(gm, "Syntax: //announces remove <announcement id>");
                        return Ok(());
                    }
                    let id = parse_int(token)?;
                    if delete_announcement(world, id) {
                        world.send_sys_message(gm, "Announcement has been successfully removed!");
                    } else {
                        world.send_sys_message(gm, "Announcement does not exist!");
                    }
                    return announces_list(world, gm, None);
                }
                "restart" => return announces_restart(world, gm, &mut st),
                "show" => return announces_show(world, gm, &mut st),
                "list" => {
                    let mut page = 0;
                    if let Some(token) = st.next_token() {
                        let token = token.replace("page=", "");
                        if is_numeric(&token) {
                            page = parse_int(&token)?;
                        }
                    }
                    return announces_list(world, gm, Some(page));
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

const SYNTAX_ADD: &str = "Syntax: //announces add <type> <delay> <repeat> <text>";

/// Java `AnnouncementsTable.deleteAnnouncement(id)`.
fn delete_announcement(world: &mut World, id: i32) -> bool {
    let Some(announcement) = world.admin.ctl.announcements.map.remove(&id) else {
        return false;
    };
    if let Some(timer) = announcement.auto.as_ref().and_then(|a| a.timer) {
        world.timers.cancel(timer);
    }
    world
        .outbox
        .persist(PersistCommand::Announcement(AnnouncementWrite::Delete { id }));
    true
}

/// The `add` branch.
fn announces_add(world: &mut World, gm: ObjectId, st: &mut Tokens<'_>) -> Outcome {
    if !st.has_more() {
        let Some(content) = world.get_htm(gm, "data/html/admin/announces-add.htm") else {
            return Err(npe());
        };
        world.send_cb_html(gm, &content);
        return Ok(());
    }
    let kind = AnnouncementType::find_by_name(st.next_token().unwrap_or_default());

    // The initial delay.
    let Some(token) = st.next_token() else {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    };
    if !is_numeric(token) {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    }
    let init_delay = parse_int(token)?.wrapping_mul(1000);

    // The delay.
    let Some(token) = st.next_token() else {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    };
    if !is_numeric(token) {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    }
    let delay = parse_int(token)?.wrapping_mul(1000);
    if delay < 10 * 1000 && kind.is_auto() {
        world.send_sys_message(gm, "Delay cannot be less then 10 seconds!");
        return Ok(());
    }

    // The repeat.
    let Some(token) = st.next_token() else {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    };
    if !is_numeric(token) {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    }
    let mut repeat = parse_int(token)?;
    if repeat == 0 {
        repeat = -1;
    }

    // The text.
    let Some(first) = st.next_token() else {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    };
    let mut content = first.to_owned();
    while let Some(token) = st.next_token() {
        content.push(' ');
        content.push_str(token);
    }

    let author = world.object_name(gm).unwrap_or_default();
    let auto = kind.is_auto().then_some(AutoPart {
        initial: i64::from(init_delay),
        delay: i64::from(delay),
        repeat,
        current_state: repeat,
        timer: None,
        generation: 0,
    });
    let announcement = Announcement {
        id: 0,
        kind,
        content: content.clone(),
        author: author.clone(),
        auto: auto.clone(),
    };
    // `AnnouncementsTable.addAnnouncement(announce)`: the row first, the table when it is stored.
    let table = &mut world.admin.ctl.announcements;
    table.next_ticket += 1;
    let ticket = table.next_ticket;
    table
        .pending
        .insert(ticket, PendingAnnouncement { gm, announcement });
    world
        .outbox
        .persist(PersistCommand::Announcement(AnnouncementWrite::Insert {
            ticket,
            kind: kind.ordinal(),
            content,
            author,
            auto: auto.map(|a| (a.initial, a.delay, a.repeat)),
        }));
    Ok(())
}

fn npe() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

/// The id token of `edit`, `show` and `restart`.
fn announcement_id(
    world: &mut World,
    gm: ObjectId,
    st: &mut Tokens<'_>,
    syntax: &str,
) -> Result<Option<i32>, JavaException> {
    let Some(token) = st.next_token() else {
        world.send_sys_message(gm, syntax);
        return Ok(None);
    };
    if !is_numeric(token) {
        world.send_sys_message(gm, syntax);
        return Ok(None);
    }
    Ok(Some(parse_int(token)?))
}

/// The page of `show` and `edit`: the file with the announcement's values.
fn announcement_page(world: &mut World, gm: ObjectId, path: &str, announcement: &Announcement) -> Outcome {
    let Some(mut content) = world.get_htm(gm, path) else {
        return Err(npe());
    };
    let (initial, delay, repeat) = match &announcement.auto {
        Some(auto) => (auto.initial / 1000, auto.delay / 1000, auto.repeat),
        None => (0, 0, 0),
    };
    content = content
        .replace("%id%", &announcement.id.to_string())
        .replace("%type%", announcement.kind.name())
        .replace("%initial%", &initial.to_string())
        .replace("%delay%", &delay.to_string())
        .replace("%repeat%", &repeat.to_string())
        .replace("%author%", &announcement.author)
        .replace("%content%", &announcement.content);
    world.send_cb_html(gm, &content);
    Ok(())
}

/// The `edit` branch.
fn announces_edit(world: &mut World, gm: ObjectId, st: &mut Tokens<'_>) -> Outcome {
    let Some(id) = announcement_id(world, gm, st, "Syntax: //announces edit <id>")? else {
        return Ok(());
    };
    let Some(announcement) = world.admin.ctl.announcements.map.get(&id).cloned() else {
        world.send_sys_message(gm, "Announcement does not exist!");
        return Ok(());
    };
    if !st.has_more() {
        return announcement_page(world, gm, "data/html/admin/announces-edit.htm", &announcement);
    }

    let kind = AnnouncementType::find_by_name(st.next_token().unwrap_or_default());
    match announcement.kind {
        AnnouncementType::AutoCritical | AnnouncementType::AutoNormal => {
            if !kind.is_auto() {
                world.send_sys_message(
                    gm,
                    "Announce type can be changed only to AUTO_NORMAL or AUTO_CRITICAL!",
                );
                return Ok(());
            }
        }
        AnnouncementType::Normal | AnnouncementType::Critical => {
            if !matches!(kind, AnnouncementType::Normal | AnnouncementType::Critical) {
                world.send_sys_message(gm, "Announce type can be changed only to NORMAL or CRITICAL!");
                return Ok(());
            }
        }
        AnnouncementType::Event => {}
    }

    // The initial delay.
    let Some(token) = st.next_token() else {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    };
    if !is_numeric(token) {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    }
    let init_delay = parse_int(token)?;

    // The delay.
    let Some(token) = st.next_token() else {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    };
    if !is_numeric(token) {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    }
    let delay = parse_int(token)?;
    if delay < 10 && kind.is_auto() {
        world.send_sys_message(gm, "Delay cannot be less then 10 seconds!");
        return Ok(());
    }

    // The repeat.
    let Some(token) = st.next_token() else {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    };
    if !is_numeric(token) {
        world.send_sys_message(gm, SYNTAX_ADD);
        return Ok(());
    }
    let mut repeat = parse_int(token)?;
    if repeat == 0 {
        repeat = -1;
    }

    // The text: the old one when there is none.
    let mut content = String::new();
    if let Some(first) = st.next_token() {
        content.push_str(first);
        while let Some(token) = st.next_token() {
            content.push(' ');
            content.push_str(token);
        }
    }
    if content.is_empty() {
        content = announcement.content.clone();
    }

    let author = world.object_name(gm).unwrap_or_default();
    let mut write_auto = None;
    if let Some(stored) = world.admin.ctl.announcements.map.get_mut(&id) {
        stored.kind = kind;
        stored.content = content.clone();
        stored.author = author.clone();
        // NOTE: Java does not restart the running task; the next run reads the new delay.
        if let Some(auto) = stored.auto.as_mut() {
            auto.initial = i64::from(init_delay.wrapping_mul(1000));
            auto.delay = i64::from(delay.wrapping_mul(1000));
            auto.repeat = repeat;
            write_auto = Some((auto.initial, auto.delay, auto.repeat));
        }
    }
    world
        .outbox
        .persist(PersistCommand::Announcement(AnnouncementWrite::Update {
            id,
            kind: kind.ordinal(),
            content,
            author,
            auto: write_auto,
        }));
    world.send_sys_message(gm, "Announcement has been successfully edited!");
    announces_list(world, gm, None)
}

/// The `restart` branch.
fn announces_restart(world: &mut World, gm: ObjectId, st: &mut Tokens<'_>) -> Outcome {
    if !st.has_more() {
        let ids: Vec<i32> = world
            .admin
            .ctl
            .announcements
            .map
            .values()
            .filter(|a| a.auto.is_some())
            .map(|a| a.id)
            .collect();
        for id in ids {
            world.restart_announcement(id);
        }
        world.send_sys_message(gm, "Auto announcements has been successfully restarted");
        return Ok(());
    }
    let Some(id) = announcement_id(world, gm, st, "Syntax: //announces show <announcement id>")? else {
        return Ok(());
    };
    match world
        .admin
        .ctl
        .announcements
        .map
        .get(&id)
        .map(|a| a.auto.is_some())
    {
        Some(true) => {
            world.restart_announcement(id);
            world.send_sys_message(gm, "Auto announcement has been successfully restarted");
        }
        Some(false) => world.send_sys_message(gm, "This option has effect only on auto announcements!"),
        None => world.send_sys_message(gm, "Announcement does not exist!"),
    }
    Ok(())
}

/// The `show` branch.
fn announces_show(world: &mut World, gm: ObjectId, st: &mut Tokens<'_>) -> Outcome {
    let Some(id) = announcement_id(world, gm, st, "Syntax: //announces show <announcement id>")? else {
        return Ok(());
    };
    match world.admin.ctl.announcements.map.get(&id).cloned() {
        Some(announcement) => {
            announcement_page(world, gm, "data/html/admin/announces-show.htm", &announcement)
        }
        None => {
            world.send_sys_message(gm, "Announcement does not exist!");
            announces_list(world, gm, None)
        }
    }
}

/// `NextPrevPageHandler.apply` with the `BypassParserFormatter` and the `ButtonsStyle`.
fn next_prev_pager(bypass: &str, current: i32, pages: i32) -> String {
    let link = |page: i32, name: &str, disabled: bool| -> String {
        if disabled {
            format!("<td>{name}</td>")
        } else {
            format!(
                "<td><button action=\"{bypass} page={page}\" value=\"{name}\" width=\"40\" height=\"15\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>"
            )
        }
    };
    const SEPARATOR: &str = "<td align=center> | </td>";
    let mut out = String::new();
    out.push_str(&link(0, "<<", current - 1 < 0));
    out.push_str(SEPARATOR);
    out.push_str(&link(current - 1, "<", current <= 0));
    out.push_str(SEPARATOR);
    out.push_str(&format!(
        "<td align=\"center\">Page: {}/{}</td>",
        current + 1,
        pages + 1
    ));
    out.push_str(SEPARATOR);
    out.push_str(&link(current + 1, ">", current >= pages));
    out.push_str(SEPARATOR);
    out.push_str(&link(pages, ">>", current + 1 > pages));
    out
}

/// The `list` branch (`page` is `None` for the callers that go to the first page).
fn announces_list(world: &mut World, gm: ObjectId, page: Option<i32>) -> Outcome {
    const PER_PAGE: usize = 8;
    let Some(content) = world.get_htm(gm, "data/html/admin/announces-list.htm") else {
        return Err(npe());
    };
    let announcements: Vec<&Announcement> = world.admin.ctl.announcements.map.values().collect();
    // `PageBuilder.build`: `currentPage(page)` keeps at least 0.
    let mut current = page.unwrap_or(0).max(0);
    let total = announcements.len();
    let pages = i32::try_from(total / PER_PAGE + usize::from(total % PER_PAGE > 0)).unwrap_or(i32::MAX);
    let pager = if pages > 1 {
        next_prev_pager("bypass admin_announces list", current, pages - 1)
    } else {
        String::new()
    };
    if current > pages {
        current = pages - 1;
    }
    let start = usize::try_from(current).unwrap_or(0).saturating_mul(PER_PAGE);
    let mut body = String::new();
    for announcement in announcements.iter().skip(start).take(PER_PAGE) {
        body.push_str("<tr>");
        body.push_str("<td width=5></td>");
        body.push_str(&format!("<td width=80>{}</td>", announcement.id));
        body.push_str(&format!("<td width=100>{}</td>", announcement.kind.name()));
        body.push_str(&format!("<td width=100>{}</td>", announcement.author));
        if announcement.kind.is_auto() {
            body.push_str(&format!(
                "<td width=60><button action=\"bypass -h admin_announces restart {}\" value=\"Restart\" width=\"60\" height=\"21\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>",
                announcement.id
            ));
        } else {
            body.push_str("<td width=60><button action=\"\" value=\"\" width=\"60\" height=\"21\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>");
        }
        let show = format!(
            "<td width=60><button action=\"bypass -h admin_announces show {}\" value=\"Show\" width=\"60\" height=\"21\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>",
            announcement.id
        );
        body.push_str(&show);
        if announcement.kind == AnnouncementType::Event {
            body.push_str("<td width=60></td>");
        } else {
            body.push_str(&format!(
                "<td width=60><button action=\"bypass -h admin_announces edit {}\" value=\"Edit\" width=\"60\" height=\"21\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>",
                announcement.id
            ));
        }
        body.push_str(&format!(
            "<td width=60><button action=\"bypass -h admin_announces remove {}\" value=\"Remove\" width=\"60\" height=\"21\" back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>",
            announcement.id
        ));
        body.push_str("<td width=5></td>");
        body.push_str("</tr>");
    }
    let html = content
        .replace("%pages%", &pager)
        .replace("%announcements%", &body);
    world.send_cb_html(gm, &html);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_are_found_by_id_and_name() {
        assert_eq!(AnnouncementType::find_by_id(3), AnnouncementType::AutoNormal);
        assert_eq!(AnnouncementType::find_by_id(99), AnnouncementType::Normal);
        assert_eq!(
            AnnouncementType::find_by_name("auto_critical"),
            AnnouncementType::AutoCritical
        );
        assert_eq!(
            AnnouncementType::find_by_name("nonsense"),
            AnnouncementType::Normal
        );
        assert_eq!(AnnouncementType::Event.ordinal(), 2);
    }

    #[test]
    fn java_split_drops_trailing_empty_strings() {
        assert_eq!(java_split("a\nb\n\n", "\n"), ["a", "b"]);
        assert_eq!(java_split("", "\n"), [""]);
        assert_eq!(java_split("no separator", "\n"), ["no separator"]);
    }

    #[test]
    fn the_pager_disables_the_ends() {
        let first = next_prev_pager("bypass x", 0, 2);
        assert!(first.starts_with("<td><<</td>"));
        assert!(first.contains("Page: 1/3"));
        assert!(first.contains("bypass x page=1"));
        let last = next_prev_pager("bypass x", 2, 2);
        assert!(last.ends_with("<td>>></td>"));
        assert!(last.contains("bypass x page=1\" value=\"<\""));
    }
}
