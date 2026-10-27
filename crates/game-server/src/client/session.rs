//! One game client connection before it enters the world: protocol check, key exchange,
//! authentication against the login server and the character list.
// Java: org/l2jmobius/gameserver/network/GameClient.java (state, close, onDisconnection)
// Java: org/l2jmobius/gameserver/network/clientpackets/ProtocolVersion.java
// Java: org/l2jmobius/gameserver/network/clientpackets/AuthLogin.java
// Java: org/l2jmobius/gameserver/LoginServerThread.java (PlayerAuthResponse -> CharSelectionInfo)
//
// Design (docs/design/world-loop.md): everything before the world (this file and the character
// screen) runs in the connection task and may use the database. Once a character is in the world,
// its in-game packets become `WorldCommand::Input` (M2-ENTER-01).

use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use l2_net::{Control, Encode, FrameHandler, PacketReader};
use l2_protocol_game::client::{AuthLogin, GameGuardReply, ProtocolVersion, RequestHardWareInfo};
use l2_protocol_game::server::charscreen::active_slot;
use l2_protocol_game::server::{CharSelectionInfo, KeyPacket, LoginFail};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};
use l2_util::WorldRng;
use l2_world::{PlayerData, SessionId, WorldCommand};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{debug, info, warn};

use super::ClientContext;
use super::charlist::CharacterInfo;
use super::cipher::GameFrameCipher;
use super::registry::SessionEntry;
use crate::ls_link::SessionKey;

/// Java `GameGuardReply.VALID`.
const GAME_GUARD_VALID: [u8; 20] = [
    0x88, 0x40, 0x1c, 0xa7, 0x83, 0x42, 0xe9, 0x15, 0xde, 0xc3, 0x68, 0xf6, 0x2d, 0x23, 0xf1, 0x3f, 0xee,
    0x68, 0x5b, 0xc5,
];

/// One client connection. Java: `GameClient`.
pub struct ClientSession {
    pub(super) ctx: Arc<ClientContext>,
    session: SessionId,
    pub(super) ip: IpAddr,
    pub(super) rng: WorldRng,
    pub(super) state: ConnectionState,
    /// Java `_protocolOk`: set by a `ProtocolVersion` the server accepts.
    protocol_ok: bool,
    /// Set once `ProtocolVersion` was answered; a second one is ignored.
    key_sent: bool,
    /// Java `_accountName`, set when `AuthLogin` was accepted by the login server.
    pub(super) account: Option<String>,
    pub(super) session_key: Option<SessionKey>,
    /// Java `_charSlotMapping` (`setCharSelection`): the list the client was shown, by slot.
    pub(super) characters: Vec<CharacterInfo>,
    /// The character `CharacterSelect` picked, until the world takes it over (M2-ENTER-01 clears
    /// it) or the client leaves.
    pub(super) selected: Option<i32>,
    /// The `CharacterSelect` flood protector of this client (`CharacterSelect` and
    /// `CharacterRestore`).
    pub(super) select_flood: l2_world::FloodAction,
    /// Shared with the registry entry and the packet sink: is the account's character in the
    /// world?
    pub(super) in_world: Arc<AtomicBool>,
    /// The character `CharacterSelect` loaded, until `EnterWorld` hands it to the world.
    pub(super) pending: Option<Box<PlayerData>>,
    /// What the world says about the character in the world (it left, and why).
    pub(super) sink_events: Option<UnboundedReceiver<l2_world::LeaveReason>>,
    /// The character this connection has in the world (set at `EnterWorld`).
    pub(super) in_world_object: Option<l2_world::ObjectId>,
    /// Java `GameClient._secondaryAuth`: the secondary password of the account, with the feature
    /// on (L-04).
    pub(super) secondary: Option<super::secondary::SecondaryAuth>,
    /// Java `GameClient._isAuthedGG` (`setGameGuardOk`): the last `GameGuardReply` was the valid
    /// one. Nothing reads it, like in Java.
    pub(super) game_guard_ok: bool,
    /// Java `GameClient._hardwareInfo`: the last hardware report of the client. The checks that use
    /// it (HWID punishments, `MaxPlayersPerHWID`) are not ported.
    pub(super) hardware: Option<Box<RequestHardWareInfo>>,
}

impl ClientSession {
    pub fn new(ctx: Arc<ClientContext>, session: SessionId, ip: IpAddr) -> Self {
        Self {
            ctx,
            session,
            ip,
            rng: WorldRng::from_entropy(),
            state: ConnectionState::Connected,
            protocol_ok: false,
            key_sent: false,
            account: None,
            session_key: None,
            characters: Vec::new(),
            selected: None,
            select_flood: l2_world::FloodAction::default(),
            in_world: Arc::default(),
            pending: None,
            sink_events: None,
            in_world_object: None,
            secondary: None,
            game_guard_ok: false,
            hardware: None,
        }
    }

    pub(super) fn session_id(&self) -> SessionId {
        self.session
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn account(&self) -> Option<&str> {
        self.account.as_deref()
    }

    /// The list last sent to the client, by slot.
    pub fn characters(&self) -> &[CharacterInfo] {
        &self.characters
    }

    // Java: ProtocolVersion.runImpl
    fn on_protocol_version(&mut self, packet: ProtocolVersion, ctl: &mut Control<'_>) {
        if packet.version == -2 {
            // "This is just a ping attempt from the new C2 client."
            ctl.close();
            return;
        }
        if self.key_sent {
            // NOTE(java-quirk): Java would generate a new key and send another KeyPacket, which
            // desynchronises the cipher. A client never does this; ignore it.
            debug!("Repeated ProtocolVersion from {}", self.ip);
            return;
        }
        self.key_sent = true;
        // Java: client.enableCrypt() — 8 random bytes plus the fixed tail. With
        // PacketEncryption off no cipher is installed and the key is only announced.
        let mut random = [0u8; 8];
        for b in &mut random {
            *b = self.rng.next_i32() as u8;
        }
        let key = l2_crypto::game_key_from_random(random);
        let use_encryption = self.ctx.packet_encryption;
        if use_encryption {
            ctl.set_cipher(Box::new(GameFrameCipher::new(key)));
        }
        let mut short_key = [0u8; 8];
        short_key.copy_from_slice(&key[..8]);
        let accepted = self.ctx.protocol_list.contains(&packet.version);
        let reply = KeyPacket {
            result: if accepted {
                KeyPacket::RESULT_OK
            } else {
                KeyPacket::RESULT_WRONG_PROTOCOL
            },
            key: short_key,
            use_encryption,
            server_id: self.ctx.gate.server_id(),
        };
        if accepted {
            self.protocol_ok = true;
            ctl.send(reply.to_bytes());
        } else {
            // Java: LOGGER_ACCOUNTING.warning + client.close(KeyPacket(.., 0))
            warn!("Wrong protocol version {} from {}.", packet.version, self.ip);
            ctl.send_and_close(reply.to_bytes());
        }
    }

    // Java: AuthLogin.runImpl + LoginServerThread (PlayerAuthResponse)
    async fn on_auth_login(&mut self, packet: AuthLogin, ctl: &mut Control<'_>) {
        if packet.login_name.is_empty() || !self.protocol_ok {
            ctl.close();
            return;
        }
        if self.account.is_some() {
            // Java: "Avoid potential exploits." A second AuthLogin is ignored.
            return;
        }
        let key = SessionKey {
            login_ok1: packet.login_key1,
            login_ok2: packet.login_key2,
            play_ok1: packet.play_key1,
            play_ok2: packet.play_key2,
        };
        let account = packet.login_name;
        if !self.ctx.gate.auth_player(&account, key).await {
            // Java: a duplicate login on this server closes right away (close(null)); a refused
            // session key sends LoginFail(SYSTEM_ERROR_LOGIN_LATER) first. The link answers both
            // with `false` and already sent PlayerLogout for the refusal.
            // NOTE: the link answers a duplicate login with `false` as well; Java closes that one
            // without a packet, here it gets the LoginFail too.
            info!("Session key refused for account '{account}' from {}.", self.ip);
            ctl.send_and_close(
                LoginFail {
                    reason: LoginFail::SYSTEM_ERROR_LOGIN_LATER,
                }
                .to_bytes(),
            );
            return;
        }
        // Java: setConnectionState(AUTHENTICATED), setSessionId, new CharSelectionInfo(...)
        self.state = ConnectionState::Authenticated;
        self.account = Some(account.clone());
        self.session_key = Some(key);
        self.ctx.registry.insert(
            &account,
            SessionEntry {
                session: self.session,
                handle: ctl.handle().clone(),
                in_world: Arc::clone(&self.in_world),
            },
        );
        info!("Account '{account}' authenticated from {}.", self.ip);
        // Java `setAccountName`: the secondary password is read.
        self.load_secondary_auth().await;
        self.send_character_list(ctl, None).await;
    }

    /// Loads the account's characters and sends `CharSelectionInfo`. `active` picks the
    /// highlighted slot; `None` lets the most recently used character win (Java `_activeId = -1`).
    // Java: new CharSelectionInfo(account, playOkID1[, activeId]) + client.setCharSelection
    pub async fn send_character_list(&mut self, ctl: &mut Control<'_>, active: Option<usize>) {
        if let Some((characters, packet)) = self.load_character_list(active).await {
            ctl.send(packet.to_bytes());
            self.characters = characters;
        }
    }

    /// The account's characters and the `CharSelectionInfo` that shows them, without touching the
    /// slot table of the session (Java builds the packet first and calls `setCharSelection` after).
    pub(super) async fn load_character_list(
        &self,
        active: Option<usize>,
    ) -> Option<(Vec<CharacterInfo>, CharSelectionInfo)> {
        let (Some(account), Some(key)) = (self.account.clone(), self.session_key) else {
            return None;
        };
        let mut characters = self.ctx.characters.character_list(&account).await;
        let last_access: Vec<i64> = characters.iter().map(|c| c.last_access).collect();
        let selected = active.or_else(|| active_slot(&last_access));
        for (i, c) in characters.iter_mut().enumerate() {
            c.entry.selected = Some(i) == selected;
        }
        let packet = CharSelectionInfo {
            account_name: account,
            session_id: key.play_ok1,
            max_characters: self.ctx.max_characters,
            characters: characters.iter().map(|c| c.entry.clone()).collect(),
        };
        Some((characters, packet))
    }

    /// Java `GameGuardReply.runImpl`: SHA-1 of the eight answer bytes against the one valid value.
    fn on_game_guard_reply(&mut self, packet: &GameGuardReply) {
        if l2_db::sha1_digest(&packet.reply) == GAME_GUARD_VALID {
            self.game_guard_ok = true;
        }
    }

    // Java: Logout.runImpl with no player: client.disconnect()
    fn on_logout_without_player(&mut self, ctl: &mut Control<'_>) {
        ctl.close();
    }
}

impl FrameHandler for ClientSession {
    // Java: GameClient.onConnected (only logs)
    async fn on_connect(&mut self, _ctl: &mut Control<'_>) {
        debug!("Client connected: {}", self.ip);
    }

    // Java: GamePacketHandler.handle
    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        let mut reader = PacketReader::new(&body);
        let Ok(opcode) = reader.u8() else {
            return;
        };
        let packet = match decode_client(self.state, opcode, &mut reader) {
            Ok(Some(packet)) => packet,
            Ok(None) => {
                debug!(
                    "Ignored packet 0x{opcode:02X} in {:?} from {}",
                    self.state, self.ip
                );
                return;
            }
            Err(e) => {
                debug!("Bad packet 0x{opcode:02X} from {}: {e}", self.ip);
                return;
            }
        };
        match packet {
            ClientPacket::ProtocolVersion(p) => self.on_protocol_version(p, ctl),
            ClientPacket::AuthLogin(p) => self.on_auth_login(p, ctl).await,
            ClientPacket::Logout(_) if !self.in_world.load(Ordering::Acquire) => {
                self.on_logout_without_player(ctl);
            }
            ClientPacket::EnterWorld(p) => self.on_enter_world(&p, ctl),
            // Between CharacterSelect and EnterWorld the session answers it itself.
            ClientPacket::RequestKeyMapping(_) if self.state == ConnectionState::Entering => {
                self.on_request_key_mapping_entering(ctl);
            }
            ClientPacket::RequestRestart(_) => self.on_request_restart(ctl).await,
            ClientPacket::NewCharacter(_) => self.on_new_character(ctl),
            ClientPacket::CharacterCreate(p) => self.on_character_create(p, ctl).await,
            ClientPacket::CharacterDelete(p) => self.on_character_delete(p, ctl).await,
            ClientPacket::CharacterRestore(p) => self.on_character_restore(p, ctl).await,
            ClientPacket::CharacterSelect(p) => self.on_character_select(p, ctl).await,
            ClientPacket::RequestGotoLobby(_) => self.on_request_goto_lobby(ctl).await,
            ClientPacket::RequestEx2ndPasswordCheck(_) => self.on_2nd_password_check(ctl),
            ClientPacket::RequestEx2ndPasswordReq(p) => self.on_2nd_password_req(p, ctl).await,
            ClientPacket::RequestEx2ndPasswordVerify(p) => self.on_2nd_password_verify(p, ctl).await,
            ClientPacket::RequestHardWareInfo(p) => self.hardware = Some(p),
            ClientPacket::GameGuardReply(p) => self.on_game_guard_reply(&p),
            other => {
                if self.state != ConnectionState::InGame || !self.on_in_game_packet(other.clone()) {
                    // Java ignores what it has no handler for: so does this.
                    debug!("Packet not handled yet from {}: {other:?}", self.ip);
                }
            }
        }
    }

    // Java: GameClient.onDisconnection
    async fn on_disconnect(&mut self) {
        debug!("Client disconnected: {}", self.ip);
        let Some(account) = self.account.clone() else {
            return;
        };
        self.ctx.registry.remove_if_session(&account, self.session);
        self.release_selected_character().await;
        if self.in_world.load(Ordering::Acquire) {
            // The world stores the character; the persistence task tells the login server
            // afterwards (D-15). Java sends PlayerLogout first and stores later.
            self.ctx.world.send(WorldCommand::Disconnected {
                session: self.session,
            });
        } else {
            self.ctx.gate.player_logout(&account).await;
        }
    }
}
