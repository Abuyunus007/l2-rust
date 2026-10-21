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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use l2_db::{
    CharacterRepository, CharacterRow, HennaRepository, HennaRow, ItemLocation, ItemRepository,
    ItemReuseRepository, ItemReuseRow, MacroRepository, Pool, RecommendationRepository, ShortcutRepository,
    SkillRepository, SkillSaveRow, SubclassRepository, SummonRepository, VariableRepository,
};
use l2_net::{Control, Encode};
use l2_protocol_game::client::EnterWorld;
use l2_protocol_game::server::LeaveWorld;
use l2_protocol_game::{ClientPacket, ConnectionState};
use l2_util::ObjectId;
use l2_world::{
    ClientInput, ItemData, LeaveReason, MacroCmdData, MacroData, PacketSink, PlayerData, PlayerProfile,
    SavedEffect, SavedItemReuse, SavedSummonEffect, ShortcutData, SkillData, WorldCommand,
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
    fn load_player<'a>(
        &'a self,
        account: &'a str,
        char_id: i32,
    ) -> BoxFuture<'a, Result<PlayerData, LoadError>>;
}

/// [`PlayerLoader`] on the game database. Only reads (D-39).
pub struct DbPlayerLoader {
    characters: CharacterRepository,
    items: ItemRepository,
    skills: SkillRepository,
    item_reuse: ItemReuseRepository,
    shortcuts: ShortcutRepository,
    hennas: HennaRepository,
    subclasses: SubclassRepository,
    macros: MacroRepository,
    reco: RecommendationRepository,
    variables: VariableRepository,
    summons: SummonRepository,
}

impl DbPlayerLoader {
    pub fn new(pool: Pool) -> Self {
        Self {
            characters: CharacterRepository::new(pool.clone()),
            items: ItemRepository::new(pool.clone()),
            skills: SkillRepository::new(pool.clone()),
            item_reuse: ItemReuseRepository::new(pool.clone()),
            shortcuts: ShortcutRepository::new(pool.clone()),
            hennas: HennaRepository::new(pool.clone()),
            subclasses: SubclassRepository::new(pool.clone()),
            macros: MacroRepository::new(pool.clone()),
            reco: RecommendationRepository::new(pool.clone()),
            variables: VariableRepository::new(pool.clone()),
            summons: SummonRepository::new(pool),
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
                ..Default::default()
            });
        }

        // Java: Item.restoreFromDb -> restoreAttributes (item_attributes, item_elementals). Java
        // catches a failed read and logs it; the items then have no augmentation or elements.
        if let Err(e) =
            crate::persist::restore_item_attributes(&self.items, char_id, data.items.iter_mut()).await
        {
            warn!(
                "Could not restore augmentation and elemental data for the items of {}: {e}",
                data.name
            );
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

        // Java: restoreEffects reads the buffs of the last logout at EnterWorld. They are kept on
        // the profile until the world enters the character, and the rows stay in the table until
        // the next store replaces them (see `persist.rs`).
        data.profile.class_index = class_index;
        data.profile.effects_to_restore = self
            .skills
            .skill_effects_of(char_id, class_index)
            .await?
            .iter()
            .map(saved_effect_of)
            .collect();

        // Java: Servitor.restoreEffects reads these rows when a servitor of the same skill spawns.
        // They are read here for the active class index and kept by the world as its view of the
        // table (`summon/servitor.rs`).
        data.profile.summon_effects = self
            .summons
            .servitor_effects_of(char_id, class_index)
            .await?
            .into_iter()
            .map(|r| SavedSummonEffect {
                summon_skill_id: r.summon_skill_id,
                skill_id: r.skill_id,
                skill_level: r.skill_level,
                remaining_time: r.remaining_time,
            })
            .collect();

        // Java: restoreItemReuse reads the item reuse timers at load. Like the buffs they stay in
        // the table until the next store replaces them (this loader only reads, D-39). Java
        // catches a failed read and logs it, so does this.
        data.profile.item_reuse_to_restore = match self.item_reuse.item_reuse_of(char_id).await {
            Ok(rows) => rows.iter().map(saved_item_reuse_of).collect(),
            Err(e) => {
                warn!("Could not restore {} Item Reuse data: {e}", data.name);
                Vec::new()
            }
        };

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

        // Java: restoreHenna (the active class index, like the skills and shortcuts)
        data.profile.hennas = henna_slots(&self.hennas.hennas_of(char_id, class_index).await?);

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
    fn load_player<'a>(
        &'a self,
        account: &'a str,
        char_id: i32,
    ) -> BoxFuture<'a, Result<PlayerData, LoadError>> {
        Box::pin(self.load(account, char_id))
    }
}

/// One `character_skills_save` row as the world keeps it. A `systime` that does not fit an `i64`
/// (a row no game server wrote) becomes the largest time, so its reuse timer never ends.
pub fn saved_effect_of(row: &SkillSaveRow) -> SavedEffect {
    SavedEffect {
        skill_id: row.skill_id,
        skill_level: row.skill_level,
        remaining_time: row.remaining_time,
        reuse_delay: row.reuse_delay,
        systime: i64::try_from(row.systime).unwrap_or(i64::MAX),
        restore_type: row.restore_type,
    }
}

/// One `character_item_reuse_save` row as the world keeps it (a `systime` that does not fit an
/// `i64` becomes the largest time, like `saved_effect_of`).
pub fn saved_item_reuse_of(row: &ItemReuseRow) -> SavedItemReuse {
    SavedItemReuse {
        item_id: row.item_id,
        item_obj_id: row.item_obj_id,
        reuse_delay: i64::from(row.reuse_delay),
        systime: i64::try_from(row.systime).unwrap_or(i64::MAX),
    }
}

/// The dye ids of slots 1 to 3 from the `character_hennas` rows (Java `restoreHenna`): a row
/// outside slots 1 to 3 is skipped, and so is a symbol of 0 (or NULL, which Java reads as 0).
/// The slot keeps its 0 then, which is an empty slot. A dye id the datapack does not know is
/// kept here and dropped when the world uses it.
// Java: Player.restoreHenna
fn henna_slots(rows: &[HennaRow]) -> [i32; 3] {
    let mut slots = [0; 3];
    for row in rows {
        if !(1..=3).contains(&row.slot) {
            continue;
        }
        let symbol_id = row.symbol_id.unwrap_or(0);
        if symbol_id == 0 {
            continue;
        }
        if let Some(slot) = slots.get_mut((row.slot - 1) as usize) {
            *slot = symbol_id;
        }
    }
    slots
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
            // Java `setVitalityPoints`: `clamp(value, MIN_VITALITY_POINTS, MAX_VITALITY_POINTS)`
            vitality_points: i32::from(row.vitality_points).clamp(1, 20_000),
            bookmark_slot: i32::from(row.bookmarkslot),
            death_penalty_level: i32::from(row.death_penalty_level),
            // Java `setLastAccess(rset.getLong("lastAccess"))`
            last_access: i64::try_from(row.last_access).unwrap_or(i64::MAX),
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
    pub fn new(handle: l2_net::ConnectionHandle, in_world: Arc<AtomicBool>) -> (Self, SinkEvents) {
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
        debug!(
            "EnterWorld of {} from {}: tracert {:?}",
            player.name, self.ip, packet.tracert
        );

        let (sink, events) = ConnectionSink::new(ctl.handle().clone(), Arc::clone(&self.in_world));
        let object_id = player.object_id;
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
        self.in_world_object = Some(object_id);
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
            ClientPacket::MoveToLocation(p) => ClientInput::MoveTo {
                target_x: p.target_x,
                target_y: p.target_y,
                target_z: p.target_z,
                origin_x: p.origin_x,
                origin_y: p.origin_y,
                origin_z: p.origin_z,
                movement_mode: p.movement_mode,
            },
            ClientPacket::ValidatePosition(p) => ClientInput::ValidatePosition {
                x: p.x,
                y: p.y,
                z: p.z,
                heading: p.heading,
            },
            ClientPacket::CannotMoveAnymore(p) => ClientInput::CannotMoveAnymore {
                x: p.x,
                y: p.y,
                z: p.z,
                heading: p.heading,
            },
            ClientPacket::Appearing(_) => ClientInput::Appearing,
            ClientPacket::Action(p) => ClientInput::Action {
                object_id: p.object_id,
                action_id: p.action_id,
            },
            ClientPacket::RequestTargetCanceld(p) => ClientInput::TargetCanceled { unselect: p.unselect },
            ClientPacket::DlgAnswer(p) => ClientInput::DlgAnswer {
                message_id: p.message_id,
                answer: p.answer,
                requester_id: p.requester_id,
            },
            ClientPacket::RequestBypassToServer(p) => ClientInput::Bypass { command: p.command },
            ClientPacket::RequestLinkHtml(p) => ClientInput::LinkHtml { link: p.link },
            ClientPacket::SendBypassBuildCmd(p) => ClientInput::BuildCmd { command: p.command },
            ClientPacket::BypassUserCmd(p) => ClientInput::UserCommand { id: p.command },
            // Java: AttackRequest (opcodes ATTACK and ATTACK_REQUEST)
            ClientPacket::AttackRequest(p) => ClientInput::AttackRequest {
                object_id: p.object_id,
                attack_id: p.attack_id,
            },
            ClientPacket::UseItem(p) => ClientInput::UseItem {
                object_id: p.object_id,
                ctrl_pressed: p.ctrl_pressed,
            },
            ClientPacket::RequestUnEquipItem(p) => ClientInput::UnEquipItem {
                paperdoll_slot: p.paperdoll_slot,
            },
            ClientPacket::RequestDestroyItem(p) => ClientInput::DestroyItem {
                object_id: p.object_id,
                count: p.count,
            },
            ClientPacket::RequestDropItem(p) => ClientInput::DropItem {
                object_id: p.object_id,
                count: p.count,
                x: p.x,
                y: p.y,
                z: p.z,
            },
            ClientPacket::RequestItemList(_) => ClientInput::ItemList,
            // Java: TradeRequest, AnswerTradeRequest, AddTradeItem, TradeDone
            ClientPacket::TradeRequest(p) => ClientInput::TradeRequest {
                object_id: p.object_id,
            },
            ClientPacket::AnswerTradeRequest(p) => ClientInput::AnswerTradeRequest { response: p.response },
            ClientPacket::AddTradeItem(p) => ClientInput::AddTradeItem {
                trade_id: p.trade_id,
                object_id: p.object_id,
                count: p.count,
            },
            ClientPacket::TradeDone(p) => ClientInput::TradeDone { response: p.response },
            // Java: RequestMagicSkillUse
            ClientPacket::RequestMagicSkillUse(p) => ClientInput::MagicSkillUse {
                magic_id: p.magic_id,
                ctrl_pressed: p.ctrl_pressed,
                shift_pressed: p.shift_pressed,
            },
            // Java: RequestAutoSoulShot
            ClientPacket::RequestAutoSoulShot(p) => ClientInput::AutoSoulShot {
                item_id: p.item_id,
                shot_type: p.shot_type,
            },
            // Java: RequestShortcutReg, RequestShortcutDel
            ClientPacket::RequestShortCutReg(p) => ClientInput::ShortcutReg {
                type_id: p.type_id,
                slot: p.slot,
                id: p.id,
                level: p.level,
                character_type: p.character_type,
            },
            ClientPacket::RequestShortCutDel(p) => ClientInput::ShortcutDel { slot: p.slot },
            ClientPacket::RequestAcquireSkillInfo(p) => ClientInput::AcquireSkillInfo {
                id: p.id,
                level: p.level,
                skill_type: p.skill_type,
            },
            ClientPacket::RequestAcquireSkill(p) => ClientInput::AcquireSkill {
                id: p.id,
                level: p.level,
                skill_type: p.skill_type,
                sub_type: p.sub_type,
            },
            // Java: RequestRestartPoint
            ClientPacket::RequestRestartPoint(p) => ClientInput::RestartPoint {
                point_type: p.requested_point_type,
            },
            // Java: RequestVoteNew
            ClientPacket::RequestVoteNew(p) => ClientInput::VoteNew {
                target_id: p.target_id,
            },
            // Java: RequestActionUse (M6-SUMMON-01: the summon actions)
            ClientPacket::RequestActionUse(p) => ClientInput::ActionUse {
                action_id: p.action_id,
                ctrl_pressed: p.ctrl_pressed,
                shift_pressed: p.shift_pressed,
            },
            // The pet packets are PET-01's: the world hears them as not ported yet.
            ClientPacket::RequestPetGetItem(_) => ClientInput::Other {
                opcode: l2_protocol_game::client::RequestPetGetItem::OPCODE.id(),
                ex_opcode: None,
            },
            ClientPacket::RequestGetItemFromPet(_) => ClientInput::Other {
                opcode: l2_protocol_game::client::RequestGetItemFromPet::OPCODE.id(),
                ex_opcode: None,
            },
            ClientPacket::RequestGiveItemToPet(_) => ClientInput::Other {
                opcode: l2_protocol_game::client::RequestGiveItemToPet::OPCODE.id(),
                ex_opcode: None,
            },
            ClientPacket::RequestPetUseItem(_) => ClientInput::Other {
                opcode: l2_protocol_game::client::RequestPetUseItem::OPCODE.id(),
                ex_opcode: None,
            },
            ClientPacket::RequestChangePetName(_) => ClientInput::Other {
                opcode: l2_protocol_game::client::RequestChangePetName::OPCODE.id(),
                ex_opcode: None,
            },
            // Java: RequestJoinParty, RequestAnswerJoinParty, RequestWithDrawalParty,
            // RequestOustPartyMember, RequestChangePartyLeader
            ClientPacket::RequestJoinParty(p) => ClientInput::JoinParty {
                name: p.name,
                distribution_type_id: p.party_distribution_type_id,
            },
            ClientPacket::RequestAnswerJoinParty(p) => ClientInput::AnswerJoinParty { response: p.response },
            ClientPacket::RequestWithDrawalParty(_) => ClientInput::WithdrawParty,
            ClientPacket::RequestOustPartyMember(p) => ClientInput::OustPartyMember { name: p.name },
            ClientPacket::RequestChangePartyLeader(p) => ClientInput::ChangePartyLeader { name: p.name },
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
        // Registered before the restart is forwarded, so the acknowledgement cannot be missed.
        let stored = self.in_world_object.map(|id| self.ctx.left_acks.wait_for(id));
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
        // Java stores the character synchronously before `RestartResponse`. Here the persistence
        // task stores it: wait until it is done (and the character is marked offline) before the
        // list and a following select read the database (race found by M2-E2E-01).
        if let Some(stored) = stored
            && tokio::time::timeout(RESTART_WAIT, stored).await.is_err()
        {
            warn!(
                "The store after the restart of {} took too long; continuing.",
                self.ip
            );
        }
        self.in_world_object = None;
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

/// Logs a failed load like Java (`Could not restore in slot: n`).
pub fn log_load_failure(char_id: i32, error: &LoadError) {
    error!("Could not restore character {char_id}: {error}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> CharacterRow {
        CharacterRow {
            account_name: Some("acc".into()),
            char_id: 0x6000_0001,
            char_name: "Hero".into(),
            level: Some(40),
            max_hp: Some(1_000),
            cur_hp: Some(900),
            max_cp: Some(500),
            cur_cp: Some(400),
            max_mp: Some(300),
            cur_mp: Some(200),
            face: Some(1),
            hair_style: Some(2),
            hair_color: Some(3),
            sex: Some(1),
            heading: Some(555),
            x: Some(-1),
            y: Some(-2),
            z: Some(-3),
            exp: Some(123_456_789_012),
            exp_before_death: Some(0),
            sp: 4_000_000_000,
            karma: Some(7),
            fame: 99,
            pvpkills: Some(5),
            pkkills: Some(6),
            clanid: Some(0),
            race: Some(2),
            classid: Some(20),
            base_class: 18,
            transform_id: 0,
            deletetime: 0,
            cancraft: Some(0),
            title: None,
            title_color: 0x0012_3456,
            accesslevel: Some(0),
            online: Some(0),
            onlinetime: Some(3_600),
            char_slot: Some(0),
            newbie: Some(0),
            last_access: 1_700_000_000_000,
            clan_privs: Some(0),
            wantspeace: Some(0),
            isin7sdungeon: 0,
            power_grade: Some(0),
            nobless: 1,
            subpledge: 0,
            lvl_joined_academy: 0,
            apprentice: 0,
            sponsor: 0,
            clan_join_expiry_time: 0,
            clan_create_expiry_time: 0,
            death_penalty_level: 3,
            bookmarkslot: 9,
            vitality_points: 15_000,
            language: None,
            faction: 0,
            pccafe_points: 0,
        }
    }

    #[test]
    fn a_characters_row_becomes_player_data() {
        let p = player_from_row(&row());
        assert_eq!(
            (p.object_id, p.account.as_str(), p.name.as_str()),
            (ObjectId(0x6000_0001), "acc", "Hero")
        );
        // A NULL title is an empty title.
        assert_eq!(p.title, "");
        assert_eq!((p.class_id, p.base_class, p.race, p.sex), (20, 18, 2, 1));
        assert_eq!((p.level, p.exp, p.sp), (40, 123_456_789_012, 4_000_000_000));
        assert_eq!((p.karma, p.pvp_kills, p.pk_kills), (7, 5, 6));
        assert_eq!((p.x, p.y, p.z, p.heading), (-1, -2, -3, 555));
        assert_eq!(
            (p.cur_hp, p.max_hp, p.cur_mp, p.max_mp, p.cur_cp, p.max_cp),
            (900.0, 1000.0, 200.0, 300.0, 400.0, 500.0)
        );
        assert_eq!(p.online_time, 3_600);
        assert!(p.items.is_empty() && p.skills.is_empty() && p.shortcuts.is_empty());
        let profile = &p.profile;
        assert_eq!(
            (profile.fame, profile.noble, profile.title_color),
            (99, true, 0x0012_3456)
        );
        assert_eq!(
            (
                profile.vitality_points,
                profile.bookmark_slot,
                profile.death_penalty_level
            ),
            (15_000, 9, 3)
        );
        // Java `_lastAccess`, in milliseconds.
        assert_eq!(profile.last_access, 1_700_000_000_000);
    }

    #[test]
    fn macro_commands_follow_macro_list_restore_me() {
        let commands = parse_macro_commands("1,1177,0,;3,0,0,/say hi;5,57,1;bad;2,x,0;7,1");
        assert_eq!(
            commands,
            [
                MacroCmdData {
                    kind: 1,
                    d1: 1177,
                    d2: 0,
                    cmd: String::new()
                },
                MacroCmdData {
                    kind: 3,
                    d1: 0,
                    d2: 0,
                    cmd: "/say hi".into()
                },
                MacroCmdData {
                    kind: 5,
                    d1: 57,
                    d2: 1,
                    cmd: String::new()
                },
            ]
        );
        assert!(parse_macro_commands("").is_empty());
    }

    #[test]
    fn the_restore_location_variable_moves_the_character() {
        let mut p = player_from_row(&row());
        apply_restore_location(&mut p);
        assert_eq!((p.x, p.y, p.z), (-1, -2, -3));
        p.profile
            .variables
            .insert(RESTORE_LOCATION.into(), "83597;147915;-3405".into());
        apply_restore_location(&mut p);
        assert_eq!((p.x, p.y, p.z), (83_597, 147_915, -3_405));
        // A broken value is ignored.
        p.profile.variables.insert(RESTORE_LOCATION.into(), "1;2".into());
        apply_restore_location(&mut p);
        assert_eq!((p.x, p.y, p.z), (83_597, 147_915, -3_405));
    }

    fn henna_row(slot: i32, symbol_id: Option<i32>) -> HennaRow {
        HennaRow { slot, symbol_id }
    }

    #[test]
    fn a_saved_effect_row_keeps_its_columns() {
        let row = SkillSaveRow {
            skill_id: 1040,
            skill_level: 3,
            remaining_time: 42,
            reuse_delay: 2000,
            systime: 1_700_000_000_000,
            restore_type: 1,
        };
        assert_eq!(
            saved_effect_of(&row),
            SavedEffect {
                skill_id: 1040,
                skill_level: 3,
                remaining_time: 42,
                reuse_delay: 2000,
                systime: 1_700_000_000_000,
                restore_type: 1,
            }
        );
    }

    #[test]
    fn hennas_fill_the_slots_like_restore_henna() {
        assert_eq!(henna_slots(&[]), [0, 0, 0]);
        assert_eq!(henna_slots(&[henna_row(2, Some(7))]), [0, 7, 0]);
        assert_eq!(
            henna_slots(&[
                henna_row(1, Some(1)),
                henna_row(2, Some(2)),
                henna_row(3, Some(3))
            ]),
            [1, 2, 3]
        );
        // A slot outside 1..=3, a symbol of 0 and a NULL symbol (read as 0) are skipped.
        assert_eq!(
            henna_slots(&[
                henna_row(0, Some(5)),
                henna_row(4, Some(6)),
                henna_row(1, Some(0)),
                henna_row(2, None),
                henna_row(3, Some(9)),
            ]),
            [0, 0, 9]
        );
    }

    /// The game tables the loader reads and `create_character` writes (the same list as
    /// `charscreen_db_tests`).
    const LOADER_SCHEMA: [&str; 27] = [
        include_str!("../../../l2-db/sql/game/characters.sql"),
        include_str!("../../../l2-db/sql/game/items.sql"),
        include_str!("../../../l2-db/sql/game/item_attributes.sql"),
        include_str!("../../../l2-db/sql/game/item_elementals.sql"),
        include_str!("../../../l2-db/sql/game/item_variables.sql"),
        include_str!("../../../l2-db/sql/game/character_skills.sql"),
        include_str!("../../../l2-db/sql/game/character_skills_save.sql"),
        include_str!("../../../l2-db/sql/game/character_item_reuse_save.sql"),
        include_str!("../../../l2-db/sql/game/character_shortcuts.sql"),
        include_str!("../../../l2-db/sql/game/character_macroses.sql"),
        include_str!("../../../l2-db/sql/game/character_reco_bonus.sql"),
        include_str!("../../../l2-db/sql/game/character_contacts.sql"),
        include_str!("../../../l2-db/sql/game/character_friends.sql"),
        include_str!("../../../l2-db/sql/game/character_hennas.sql"),
        include_str!("../../../l2-db/sql/game/character_quests.sql"),
        include_str!("../../../l2-db/sql/game/character_recipebook.sql"),
        include_str!("../../../l2-db/sql/game/character_subclasses.sql"),
        include_str!("../../../l2-db/sql/game/heroes.sql"),
        include_str!("../../../l2-db/sql/game/olympiad_nobles.sql"),
        include_str!("../../../l2-db/sql/game/seven_signs.sql"),
        include_str!("../../../l2-db/sql/game/pets.sql"),
        include_str!("../../../l2-db/sql/game/merchant_lease.sql"),
        include_str!("../../../l2-db/sql/game/character_raid_points.sql"),
        include_str!("../../../l2-db/sql/game/character_instance_time.sql"),
        include_str!("../../../l2-db/sql/game/character_variables.sql"),
        include_str!("../../../l2-db/sql/game/mods_wedding.sql"),
        include_str!("../../../l2-db/sql/game/clan_data.sql"),
    ];

    /// A throwaway database with the game tables; `None` without `L2_TEST_DATABASE_URL`.
    async fn loader_setup() -> Option<(l2_testkit::test_db::TestDb, l2_db::Pool)> {
        let db = l2_testkit::test_db::TestDb::create("henna").await?;
        for file in LOADER_SCHEMA {
            let text: String = file
                .lines()
                .filter(|line| !line.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");
            for statement in text.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                db.exec(statement).await;
            }
        }
        let (user, password, host, port) = db.parts();
        let mut options = sqlx::mysql::MySqlConnectOptions::new()
            .host(&host)
            .port(port)
            .username(&user)
            .database(db.name());
        if !password.is_empty() {
            options = options.password(&password);
        }
        let pool = l2_db::connect_with(options, 5).await.unwrap();
        Some((db, pool))
    }

    #[tokio::test]
    async fn the_loader_reads_the_hennas_of_the_active_class_index() {
        let Some((db, pool)) = loader_setup().await else {
            return;
        };
        let (_dir, pack) = crate::client::testdata::datapack();
        let screen = crate::client::charscreen::CharScreen::new(
            Arc::new(crate::client::charscreen::DbCharScreen::new(pool.clone())),
            pack,
            Arc::new(l2_config::GameConfig::default()),
            Arc::new(l2_world::IdFactory::new([]).unwrap()),
            0,
        );
        let create = l2_protocol_game::client::CharacterCreate {
            name: "Dyer".to_owned(),
            race: 0,
            is_female: true,
            class_id: 10,
            int: 0,
            str: 0,
            con: 0,
            men: 0,
            dex: 0,
            wit: 0,
            hair_style: 2,
            hair_color: 1,
            face: 1,
        };
        let mut rng = l2_util::WorldRng::from_seed(7);
        let id = crate::client::charscreen::create_character(&screen, "acc", &create, &mut rng)
            .await
            .unwrap()
            .char_id;
        // Slots 1 and 2 hold dyes, slot 3 has symbol 0, slot 4 is not a slot. Class index 1 has a
        // dye of its own, which a base-class character does not load.
        db.exec(&format!(
            "INSERT INTO character_hennas (charId, symbol_id, slot, class_index) VALUES \
             ({id}, 1, 1, 0), ({id}, 2, 2, 0), ({id}, 0, 3, 0), ({id}, 3, 4, 0), ({id}, 4, 1, 1)"
        ))
        .await;

        let loaded = DbPlayerLoader::new(pool.clone())
            .load_player("acc", id)
            .await
            .unwrap();
        assert_eq!(loaded.profile.hennas, [1, 2, 0]);

        pool.close().await;
        db.drop_db().await;
    }

    /// `Item.restoreAttributes` at login: the augmentation and elements of the character's items;
    /// the `-1` rows mean nothing, and another character's items are not touched.
    #[tokio::test]
    async fn the_loader_restores_item_augmentations_and_elements() {
        let Some((db, pool)) = loader_setup().await else {
            return;
        };
        let (_dir, pack) = crate::client::testdata::datapack();
        let screen = crate::client::charscreen::CharScreen::new(
            Arc::new(crate::client::charscreen::DbCharScreen::new(pool.clone())),
            pack,
            Arc::new(l2_config::GameConfig::default()),
            Arc::new(l2_world::IdFactory::new([]).unwrap()),
            0,
        );
        let create = l2_protocol_game::client::CharacterCreate {
            name: "Augur".to_owned(),
            race: 0,
            is_female: false,
            class_id: 0,
            int: 0,
            str: 0,
            con: 0,
            men: 0,
            dex: 0,
            wit: 0,
            hair_style: 0,
            hair_color: 0,
            face: 0,
        };
        let mut rng = l2_util::WorldRng::from_seed(7);
        let id = crate::client::charscreen::create_character(&screen, "acc", &create, &mut rng)
            .await
            .unwrap()
            .char_id;
        let items = ItemRepository::new(pool.clone());
        let new_item = |object_id: i32| l2_db::NewItem {
            owner_id: id,
            item_id: 57,
            count: 1,
            loc: ItemLocation::Inventory,
            loc_data: 0,
            enchant_level: 0,
            object_id,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: -1,
        };
        for object_id in [0x6100_0001, 0x6100_0002, 0x6100_0003] {
            items.insert_item(&new_item(object_id)).await.unwrap();
        }
        items.set_augmentation(0x6100_0001, 0x0003_0002).await.unwrap();
        items
            .replace_elementals(
                0x6100_0001,
                &[
                    l2_db::ItemElementalRow {
                        elem_type: 4,
                        elem_value: 20,
                    },
                    l2_db::ItemElementalRow {
                        elem_type: 1,
                        elem_value: 10,
                    },
                ],
            )
            .await
            .unwrap();
        // "Nothing" rows: augmentation -1, element type -1.
        items.set_augmentation(0x6100_0002, -1).await.unwrap();
        items
            .replace_elementals(
                0x6100_0002,
                &[l2_db::ItemElementalRow {
                    elem_type: -1,
                    elem_value: 5,
                }],
            )
            .await
            .unwrap();

        let loaded = DbPlayerLoader::new(pool.clone())
            .load_player("acc", id)
            .await
            .unwrap();
        let find = |object_id: i32| {
            loaded
                .items
                .iter()
                .find(|i| i.object_id == ObjectId(object_id))
                .unwrap()
        };
        let first = find(0x6100_0001);
        assert_eq!(first.augmentation, Some(0x0003_0002));
        assert_eq!(
            first.elementals,
            [
                l2_world::effects::item_extras::ItemElemental {
                    element: 1,
                    value: 10
                },
                l2_world::effects::item_extras::ItemElemental {
                    element: 4,
                    value: 20
                },
            ]
        );
        for object_id in [0x6100_0002, 0x6100_0003] {
            let plain = find(object_id);
            assert_eq!((plain.augmentation, plain.elementals.len()), (None, 0));
        }

        pool.close().await;
        db.drop_db().await;
    }
}
