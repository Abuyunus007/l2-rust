//! Entering the world from the connection's side: loading the character at `CharacterSelect`,
//! `EnterWorld`, the in-game packets, `RequestRestart`, and the [`PacketSink`] that connects the
//! world thread to the connection.
// Java: network/clientpackets/{CharacterSelect,EnterWorld,Logout,RequestRestart,Say2}.java
// Java: network/GameClient.java (load), network/Disconnection.java
// Java: entity/actor/Player.java (restore: the character, items, skills, macros, shortcuts...)
//
// Design (docs/design/world-loop.md): the database is read here, in the connection task, before
// the character enters the world (D-15). The world never sees a database. Once the world has the
// character, every in-game packet becomes a `WorldCommand::Input`, and everything the world wants
// to say comes back through [`ConnectionSink`] as plain bytes.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use l2_db::{
    CharacterRepository, CharacterRow, ItemLocation, ItemRepository, MacroRepository, Pool,
    RecommendationRepository, ShortcutRepository, SkillRepository, SubclassRepository, VariableRepository,
};
use l2_net::{Control, Encode};
use l2_protocol_game::client::EnterWorld;
use l2_protocol_game::server::LeaveWorld;
use l2_protocol_game::{ClientPacket, ConnectionState};
use l2_util::ObjectId;
use l2_world::{
    ClientInput, ItemData, LeaveReason, MacroCmdData, MacroData, PacketSink, PlayerData, PlayerProfile,
    ShortcutData, SkillData, WorldCommand,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::{debug, error, info, warn};

use super::BoxFuture;
use super::session::ClientSession;

/// How long a restart waits for the world to answer. In M2 nothing can refuse a restart, so the
/// answer comes at once; the limit only keeps a dead world from holding the connection.
const RESTART_WAIT: Duration = Duration::from_secs(10);

/// `PlayerVariables.RESTORE_LOCATION`.
const RESTORE_LOCATION: &str = "RESTORE_LOCATION";
/// The recommendation hourglass time Java uses when a character has no `character_reco_bonus`
/// row (`Player.loadRecommendations`).
const DEFAULT_RECO_TIME_LEFT_MS: i64 = 3_600_000;

/// Why a character could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("character {0} does not exist")]
    NotFound(i32),
    #[error("character {0} belongs to another account")]
    WrongAccount(i32),
    #[error("database error: {0}")]
    Database(String),
}

impl From<l2_db::DbError> for LoadError {
    fn from(e: l2_db::DbError) -> Self {
        Self::Database(e.to_string())
    }
}

/// Where the full character comes from at `CharacterSelect`. [`DbPlayerLoader`] in the server, a
/// fake in tests.
pub trait PlayerLoader: Send + Sync + 'static {
    /// Java `Player.load(objectId)`: the character with its items, skills, shortcuts and macros.
    fn load_player<'a>(&'a self, account: &'a str, char_id: i32) -> BoxFuture<'a, Result<PlayerData, LoadError>>;
}

/// [`PlayerLoader`] on the game database. Only reads (D-39).
pub struct DbPlayerLoader {
    characters: CharacterRepository,
    items: ItemRepository,
    skills: SkillRepository,
    shortcuts: ShortcutRepository,
    subclasses: SubclassRepository,
    macros: MacroRepository,
    reco: RecommendationRepository,
    variables: VariableRepository,
}

impl DbPlayerLoader {
    pub fn new(pool: Pool) -> Self {
        Self {
            characters: CharacterRepository::new(pool.clone()),
            items: ItemRepository::new(pool.clone()),
            skills: SkillRepository::new(pool.clone()),
            shortcuts: ShortcutRepository::new(pool.clone()),
            subclasses: SubclassRepository::new(pool.clone()),
            macros: MacroRepository::new(pool.clone()),
            reco: RecommendationRepository::new(pool.clone()),
            variables: VariableRepository::new(pool),
        }
    }

    // Java: Player.restore
    async fn load(&self, account: &str, char_id: i32) -> Result<PlayerData, LoadError> {
        let row = self
            .characters
            .restore_character(char_id)
            .await?
            .ok_or(LoadError::NotFound(char_id))?;
        if row.account_name.as_deref() != Some(account) {
            return Err(LoadError::WrongAccount(char_id));
        }
        let mut data = player_from_row(&row);

        // Java: restoreSubClassData. A subclass that is active shows its own level, exp and sp
        // (`PlayerStat.getLevel` etc.); one that does not exist is dropped for the base class.
        let mut class_index = 0;
        if data.class_id != data.base_class {
            let subs = self.subclasses.subclasses_of(char_id).await?;
            match subs.iter().find(|s| s.class_id == data.class_id) {
                Some(sub) => {
                    class_index = sub.class_index;
                    data.level = sub.level;
                    data.exp = sub.exp;
                    data.sp = sub.sp;
                }
                None => {
                    warn!(
                        "{} reverted to base class. Possibly has tried a relogin exploit while subclassing.",
                        data.name
                    );
                    data.class_id = data.base_class;
                }
            }
        }

        // Java: getInventory().restore() (INVENTORY and PAPERDOLL)
        for r in self
            .items
            .inventory_items(char_id, ItemLocation::Inventory, ItemLocation::Paperdoll)
            .await?
        {
            data.items.push(ItemData {
                object_id: ObjectId(r.object_id),
                item_id: r.item_id.unwrap_or(0),
                count: i64::try_from(r.count).unwrap_or(i64::MAX),
                enchant_level: r.enchant_level.unwrap_or(0),
                loc: r.loc.unwrap_or_default(),
                loc_data: r.loc_data.unwrap_or(0),
                custom_type1: r.custom_type1.unwrap_or(0),
                custom_type2: r.custom_type2.unwrap_or(0),
                mana_left: r.mana_left,
                time: r.time,
            });
        }

        // Java: restoreSkills
        let mut skills = self.skills.skills_of(char_id, class_index).await?;
        skills.sort_by_key(|s| s.skill_id);
        data.skills = skills
            .into_iter()
            .map(|s| SkillData {
                skill_id: s.skill_id,
                skill_level: s.skill_level,
            })
            .collect();

        // Java: getMacros().restoreMe()
        data.profile.macros = self
            .macros
            .macros_of(char_id)
            .await?
            .into_iter()
            .map(|m| MacroData {
                id: m.id,
                icon: m.icon.unwrap_or(0),
                name: m.name.unwrap_or_default(),
                descr: m.descr.unwrap_or_default(),
                acronym: m.acronym.unwrap_or_default(),
                commands: parse_macro_commands(m.commands.as_deref().unwrap_or("")),
            })
            .collect();

        // Java: restoreShortcuts (the active class index)
        data.shortcuts = self
            .shortcuts
            .shortcuts_of(char_id, class_index)
            .await?
            .into_iter()
            .map(|s| ShortcutData {
                slot: s.slot,
                page: s.page,
                kind: s.kind.unwrap_or(0),
                shortcut_id: s.shortcut_id.and_then(|v| i32::try_from(v).ok()).unwrap_or(0),
                level: s.level.unwrap_or(0),
            })
            .collect();

        // Java: Player.loadRecommendations (called from `checkRecoBonusTask` at EnterWorld)
        match self.reco.reco_bonus_of(char_id).await? {
            Some(reco) => {
                data.profile.recom_have = reco.rec_have;
                data.profile.recom_left = reco.rec_left;
                data.profile.recom_time_left_ms = reco.time_left;
            }
            None => data.profile.recom_time_left_ms = DEFAULT_RECO_TIME_LEFT_MS,
        }

        // Java: PlayerVariables.restoreMe
        data.profile.variables = self.variables.variables_of(char_id).await?.into_iter().collect();
        apply_restore_location(&mut data);
        Ok(data)
    }
}

impl PlayerLoader for DbPlayerLoader {
    fn load_player<'a>(&'a self, account: &'a str, char_id: i32) -> BoxFuture<'a, Result<PlayerData, LoadError>> {
        Box::pin(self.load(account, char_id))
    }
}

/// The columns of the `characters` row as a [`PlayerData`] without items, skills, shortcuts and
/// the other tables.
// Java: Player.restore (the first query)
pub fn player_from_row(row: &CharacterRow) -> PlayerData {
    let base_class = i32::from(row.base_class);
    let active_class = i32::from(row.classid.unwrap_or(0));
    let u32_i32 = |v: u32| i32::try_from(v).unwrap_or(i32::MAX);
    PlayerData {
        object_id: ObjectId(row.object_id()),
        account: row.account_name.clone().unwrap_or_default(),
        name: row.char_name.clone(),
        title: row.title.clone().unwrap_or_default(),
        class_id: active_class,
        base_class,
        race: i32::from(row.race.unwrap_or(0)),
        sex: i32::from(row.sex.unwrap_or(0)),
        face: i32::from(row.face.unwrap_or(0)),
        hair_style: i32::from(row.hair_style.unwrap_or(0)),
        hair_color: i32::from(row.hair_color.unwrap_or(0)),
        level: i32::from(row.level.unwrap_or(0)),
        exp: row.exp.map_or(0, |e| i64::try_from(e).unwrap_or(i64::MAX)),
        sp: i64::try_from(row.sp).unwrap_or(i64::MAX),
        karma: row.karma.map_or(0, u32_i32),
        pvp_kills: i32::from(row.pvpkills.unwrap_or(0)),
        pk_kills: i32::from(row.pkkills.unwrap_or(0)),
        access_level: row.accesslevel.unwrap_or(0),
        x: row.x.unwrap_or(0),
        y: row.y.unwrap_or(0),
        z: row.z.unwrap_or(0),
        heading: row.heading.unwrap_or(0),
        cur_hp: f64::from(row.cur_hp.unwrap_or(0)),
        max_hp: f64::from(row.max_hp.unwrap_or(0)),
        cur_mp: f64::from(row.cur_mp.unwrap_or(0)),
        max_mp: f64::from(row.max_mp.unwrap_or(0)),
        cur_cp: f64::from(row.cur_cp.unwrap_or(0)),
        max_cp: f64::from(row.max_cp.unwrap_or(0)),
        online_time: i64::from(row.onlinetime.unwrap_or(0)),
        items: Vec::new(),
        skills: Vec::new(),
        shortcuts: Vec::new(),
        profile: PlayerProfile {
            fame: u32_i32(row.fame),
            // Java: `rset.getInt("nobless") == 1`
            noble: row.nobless == 1,
            title_color: i32::try_from(row.title_color).unwrap_or(l2_world::DEFAULT_TITLE_COLOR),
            vitality_points: i32::from(row.vitality_points),
            bookmark_slot: i32::from(row.bookmarkslot),
            death_penalty_level: i32::from(row.death_penalty_level),
            ..PlayerProfile::default()
        },
    }
}

/// Java `MacroList.restoreMe`: `type,d1,d2,cmd;type,d1,d2,cmd;...`. A part with fewer than three
/// fields is skipped.
pub fn parse_macro_commands(text: &str) -> Vec<MacroCmdData> {
    let mut commands = Vec::new();
    for part in text.split(';').filter(|p| !p.is_empty()) {
        // `StringTokenizer(part, ",")` skips empty tokens.
        let tokens: Vec<&str> = part.split(',').filter(|t| !t.is_empty()).collect();
        if tokens.len() < 3 {
            continue;
        }
        // `MacroType.values()[Integer.parseInt(..)]` and `Integer.parseInt` throw for bad data;
        // Java then drops the whole macro list ("could not store shortcuts"). A bad command is
        // skipped here and the rest is kept.
        let (Ok(kind), Ok(d1), Ok(d2)) = (
            tokens[0].parse::<i32>(),
            tokens[1].parse::<i32>(),
            tokens[2].parse::<i32>(),
        ) else {
            continue;
        };
        commands.push(MacroCmdData {
            kind,
            d1,
            d2,
            cmd: tokens.get(3).map(|t| (*t).to_owned()).unwrap_or_default(),
        });
    }
    commands
}

/// Java `CharacterSelect.runImpl`: "Restore player location." The variable is `x;y;z`.
///
/// NOTE: Java removes the variable (and its row) right away; this server never deletes rows of
/// the Java game server, so the row stays and the Java server applies it again at its next
/// login.
pub fn apply_restore_location(data: &mut PlayerData) {
    let Some(text) = data.profile.variables.get(RESTORE_LOCATION) else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let parts: Vec<i32> = text.split(';').filter_map(|p| p.trim().parse().ok()).collect();
    if let [x, y, z] = parts[..] {
        data.x = x;
        data.y = y;
        data.z = z;
    } else {
        warn!("{}: bad RESTORE_LOCATION '{text}', ignored.", data.name);
    }
}

/// A packet from the world about a player of this connection.
pub type SinkEvents = UnboundedReceiver<LeaveReason>;

/// The world's way to a client: packets go into the connection's queue (the connection encrypts
/// them in order), and a way out closes the connection or hands control back to the session.
pub struct ConnectionSink {
    handle: l2_net::ConnectionHandle,
    in_world: Arc<AtomicBool>,
    events: UnboundedSender<LeaveReason>,
}

impl ConnectionSink {
    pub fn new(
        handle: l2_net::ConnectionHandle,
        in_world: Arc<AtomicBool>,
    ) -> (Self, SinkEvents) {
        let (events, receiver) = unbounded_channel();
        (
            Self {
                handle,
                in_world,
                events,
            },
            receiver,
        )
    }
}

impl PacketSink for ConnectionSink {
    fn send(&self, bytes: Vec<u8>) {
        // A closed connection drops the packet; the session ends on its own.
        let _ = self.handle.send(bytes);
    }

    fn left_world(&self, reason: LeaveReason) {
        match reason {
            // The connection stays: the session goes back to the character screen. The character
            // is stored and no longer in the world, so a kick has to close the connection itself.
            LeaveReason::Restart => self.in_world.store(false, Ordering::Release),
            // Java `storeAndDeleteWith(packet)`: the last packet was queued, then the client is
            // closed. The flag stays set: the session's disconnect hook then sends a harmless
            // `Disconnected` (the world has forgotten the session) instead of telling the login
            // server before the store is done.
            LeaveReason::Logout | LeaveReason::Kicked | LeaveReason::ServerShutdown => self.handle.close(),
            LeaveReason::Disconnected => {}
        }
        let _ = self.events.send(reason);
    }
}

impl ClientSession {
    /// Java `EnterWorld.runImpl`: the character the session loaded enters the world.
    pub(super) fn on_enter_world(&mut self, packet: &EnterWorld, ctl: &mut Control<'_>) {
        let Some(player) = self.pending.take() else {
            // Java: "EnterWorld failed! player returned 'null'." and LeaveWorld.
            warn!("EnterWorld without a selected character from {}.", self.ip);
            ctl.send_and_close(LeaveWorld.to_bytes());
            return;
        };
        self.state = ConnectionState::InGame;
        // NOTE: Java sends the five trace route addresses to the login server
        // (`sendClientTracert`); the link has no such message yet.
        debug!("EnterWorld of {} from {}: tracert {:?}", player.name, self.ip, packet.tracert);

        let (sink, events) = ConnectionSink::new(ctl.handle().clone(), Arc::clone(&self.in_world));
        let sent = self.ctx.world.send(WorldCommand::EnterWorld {
            session: self.session_id(),
            player,
            sink: Box::new(sink),
            ip: Some(self.ip),
        });
        if !sent {
            // The world is gone (shutting down): `selected` stays set, so the disconnect hook
            // clears the online flag.
            warn!("The world is not running; {} cannot enter.", self.ip);
            ctl.send_and_close(LeaveWorld.to_bytes());
            return;
        }
        // After the send, so a kick that sees the flag queues its `Kick` behind the
        // `EnterWorld`.
        self.in_world.store(true, Ordering::Release);
        self.sink_events = Some(events);
        // The world owns the character now; a disconnect is reported to it, not to the database.
        self.selected = None;
    }

    /// Sends an in-game packet to the world.
    pub(super) fn forward(&self, input: ClientInput) {
        let _ = self.ctx.world.send(WorldCommand::Input {
            session: self.session_id(),
            input,
        });
    }

    /// Hands a decoded packet of the IN_GAME state to the world. `true` if it was one the world
    /// can take.
    pub(super) fn on_in_game_packet(&mut self, packet: ClientPacket) -> bool {
        let input = match packet {
            ClientPacket::Say2(p) => ClientInput::Say {
                chat_type: p.chat_type,
                text: p.text,
                target: p.target,
            },
            ClientPacket::Logout(_) => ClientInput::Logout,
            ClientPacket::RequestNetPing(_) => ClientInput::NetPing,
            ClientPacket::RequestSkillCoolTime(_) => ClientInput::SkillCoolTime,
            ClientPacket::RequestKeyMapping(_) => ClientInput::KeyMapping,
            // Java accepts the packet in this state, but there is no handler for it yet.
            ClientPacket::Unported(opcode) => ClientInput::Other {
                opcode: opcode.id(),
                ex_opcode: None,
            },
            ClientPacket::UnportedEx(ex) => ClientInput::Other {
                opcode: l2_protocol_game::ClientOpcode::ExPacket.id(),
                ex_opcode: Some(ex.id()),
            },
            _ => return false,
        };
        self.forward(input);
        true
    }

    /// Java `RequestKeyMapping.runImpl` between `CharacterSelect` and `EnterWorld`: the player
    /// exists in Java then, so it answers with the stored key mapping.
    pub(super) fn on_request_key_mapping_entering(&self, ctl: &mut Control<'_>) {
        let Some(player) = &self.pending else {
            return;
        };
        if let Some(packet) = l2_world::enter::ui_setting(player, &self.ctx.settings) {
            ctl.send(packet.to_bytes());
        }
    }

    /// Java `RequestRestart.runImpl` + the character list: the world stores the character and
    /// answers `RestartResponse`; then the connection is back at the character screen.
    pub(super) async fn on_request_restart(&mut self, ctl: &mut Control<'_>) {
        if !self.in_world.load(Ordering::Acquire) {
            return;
        }
        let Some(mut events) = self.sink_events.take() else {
            return;
        };
        self.forward(ClientInput::Restart);
        let answer = tokio::time::timeout(RESTART_WAIT, events.recv()).await;
        match answer {
            Ok(Some(LeaveReason::Restart)) => {}
            Ok(Some(other)) => {
                // Kicked or shut down while restarting: the connection is closing anyway.
                debug!("Restart of {} ended with {other:?}.", self.ip);
                return;
            }
            Ok(None) | Err(_) => {
                warn!("The world did not answer the restart of {}.", self.ip);
                self.sink_events = Some(events);
                return;
            }
        }
        // Java: `client.setConnectionState(AUTHENTICATED)`, `RestartResponse`, `CharSelectionInfo`.
        // The world queued the RestartResponse; the list follows through the same queue so the
        // order on the wire is the order of sending.
        self.state = ConnectionState::Authenticated;
        self.selected = None;
        info!("{} is back at the character screen.", self.ip);
        if let Some((characters, list)) = self.load_character_list(None).await {
            let _ = ctl.handle().send(list.to_bytes());
            self.characters = characters;
        }
    }
}

/// `CharacterSelect` helper: the map of variables as the loader returns it.
pub type Variables = BTreeMap<String, String>;

/// Logs a failed load like Java (`Could not restore in slot: n`).
pub fn log_load_failure(char_id: i32, error: &LoadError) {
    error!("Could not restore character {char_id}: {error}");
}
