//! What a registered game server sends: players in and out, the check of a player arriving at
//! the game server, status updates, character counts and the account changes.
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/PlayerInGame.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/PlayerLogout.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/ChangeAccessLevel.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/PlayerAuthRequest.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/ServerStatus.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/PlayerTracert.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/ReplyCharacters.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/RequestTempBan.java
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/ChangePassword.java
// Java: org/l2jmobius/loginserver/GameServerThread.java (`addAccountOnGameServer`,
//       `removeAccountOnGameServer`)
//
// Java does the work in the constructor of each packet class. Here every packet is one function
// that does the same steps in the same order. The database goes through [`AccountStore`] so the
// handlers can be tested without one; failures are logged and the packet is otherwise dropped,
// as Java catches the `SQLException` and carries on.

use std::sync::Arc;

use l2_db::{ChangePasswordResult, DbError, LoginRepository, TracertHops, hash_password};
use l2_protocol_login::link::{
    ChangeAccessLevel, ChangePassword, GsPacket, PlayerAuthRequest, PlayerAuthResponse, PlayerInGame,
    PlayerLogout, PlayerTracert, ReplyCharacters, RequestTempBan, ServerStatus,
};
use tracing::{debug, info, warn};

use crate::client::session_key::SessionKey;
use crate::gs::link::GsLink;
use crate::state::{LoginState, ServerStatus as Status};

// Java: ChangePassword.MSG_*
const MSG_PASSWORD_CHANGED: &str = "You have successfully changed your password!";
const MSG_PASSWORD_CHANGE_FAILED: &str = "The password change was unsuccessful!";
const MSG_CURRENT_PASSWORD_MISMATCH: &str = "The typed current password doesn't match with your current one.";

/// The account queries a game server's packets lead to. The login server uses
/// [`LoginRepository`]; tests use a recorder.
pub trait AccountStore: Send + Sync + 'static {
    /// `UPDATE accounts SET accessLevel` (`ChangeAccessLevel`).
    fn set_access_level(
        &self,
        login: &str,
        access_level: i32,
    ) -> impl Future<Output = Result<(), DbError>> + Send;

    /// `UPDATE accounts SET pcIp, hop1..hop4` (`PlayerTracert`).
    fn set_tracert(
        &self,
        login: &str,
        hops: &TracertHops,
    ) -> impl Future<Output = Result<(), DbError>> + Send;

    /// The `ban_temp` row of `account_data` (`RequestTempBan`).
    fn temp_ban(&self, login: &str, ban_time_ms: i64) -> impl Future<Output = Result<(), DbError>> + Send;

    /// The conditional password update (`ChangePassword`); both hashes are already hashed.
    fn change_password(
        &self,
        login: &str,
        current_hash: &str,
        new_hash: &str,
    ) -> impl Future<Output = Result<ChangePasswordResult, DbError>> + Send;
}

impl AccountStore for LoginRepository {
    async fn set_access_level(&self, login: &str, access_level: i32) -> Result<(), DbError> {
        self.update_access_level(login, access_level).await
    }

    async fn set_tracert(&self, login: &str, hops: &TracertHops) -> Result<(), DbError> {
        self.update_tracert(login, hops).await
    }

    async fn temp_ban(&self, login: &str, ban_time_ms: i64) -> Result<(), DbError> {
        LoginRepository::temp_ban(self, login, ban_time_ms).await
    }

    async fn change_password(
        &self,
        login: &str,
        current_hash: &str,
        new_hash: &str,
    ) -> Result<ChangePasswordResult, DbError> {
        LoginRepository::change_password(self, login, current_hash, new_hash).await
    }
}

/// Runs the packet of the registered game server `server_id`, whose connection is `link`.
// Java: GameServerPacketHandler.handle (the AUTHED cases)
pub(crate) async fn handle<S: AccountStore>(
    state: &LoginState,
    store: &S,
    link: &Arc<GsLink>,
    server_id: i32,
    packet: GsPacket,
) {
    match packet {
        GsPacket::PlayerInGame(packet) => player_in_game(state, link, &packet),
        GsPacket::PlayerLogout(packet) => player_logout(state, link, &packet),
        GsPacket::ChangeAccessLevel(packet) => change_access_level(store, &packet).await,
        GsPacket::PlayerAuthRequest(packet) => player_auth_request(state, link, &packet),
        GsPacket::ServerStatus(packet) => server_status(state, server_id, &packet),
        GsPacket::PlayerTracert(packet) => player_tracert(store, packet).await,
        GsPacket::ReplyCharacters(packet) => reply_characters(state, server_id, &packet),
        GsPacket::RequestTempBan(packet) => request_temp_ban(state, store, &packet).await,
        GsPacket::ChangePassword(packet) => change_password(state, store, &packet).await,
        // Registration packets never get here: the connection state machine handles them.
        GsPacket::BlowFishKey(_) | GsPacket::GameServerAuth(_) => {
            debug!("game server packet ignored in the registered state: {packet:?}");
        }
    }
}

/// Puts the account on the game server and drops its login server client.
// Java: GameServerThread.addAccountOnGameServer
pub fn add_account_on_game_server(state: &LoginState, link: &GsLink, account: &str) {
    link.add_account(account);
    state.clients.remove(account);
}

/// Takes the account off the game server and drops its login server client.
// Java: GameServerThread.removeAccountOnGameServer
pub fn remove_account_on_game_server(state: &LoginState, link: &GsLink, account: &str) {
    link.remove_account(account);
    state.clients.remove(account);
}

// Java: PlayerInGame
fn player_in_game(state: &LoginState, link: &GsLink, packet: &PlayerInGame) {
    for account in packet.accounts.iter().filter(|account| !account.is_empty()) {
        add_account_on_game_server(state, link, account);
    }
}

// Java: PlayerLogout
fn player_logout(state: &LoginState, link: &GsLink, packet: &PlayerLogout) {
    // NOTE(java-quirk): this also drops a login server client the account has by now, e.g. a new
    // login that happened between the player leaving and this packet.
    if !packet.account.is_empty() {
        remove_account_on_game_server(state, link, &packet.account);
    }
}

// Java: ChangeAccessLevel
async fn change_access_level<S: AccountStore>(store: &S, packet: &ChangeAccessLevel) {
    // Java: LoginController.setAccountAccessLevel
    if let Err(error) = store.set_access_level(&packet.account, packet.level).await {
        warn!("Could not set accessLevel: {error}");
    }
    // NOTE(java-quirk): the line is logged also when the update failed.
    info!("Changed {} access level to {}", packet.account, packet.level);
}

/// The game server asks whether a player that arrived at it holds the keys this login server
/// gave out. On success the login server forgets the client; the game server owns it now.
// Java: PlayerAuthRequest
fn player_auth_request(state: &LoginState, link: &GsLink, packet: &PlayerAuthRequest) {
    let sent = SessionKey {
        login_ok1: packet.login_ok1,
        login_ok2: packet.login_ok2,
        play_ok1: packet.play_ok1,
        play_ok2: packet.play_ok2,
    };
    let authed = state
        .clients
        .session_key(&packet.account)
        .is_some_and(|stored| stored.matches(&sent, state.config.login.show_licence));
    if authed {
        state.clients.remove(&packet.account);
    }
    link.send(&PlayerAuthResponse {
        account: packet.account.clone(),
        authed,
    });
}

/// Status, type, brackets, player limit and age of the game server, as it reports them.
// Java: ServerStatus
fn server_status(state: &LoginState, server_id: i32, packet: &ServerStatus) {
    let login_status = state.status();
    // Java skips everything when the server is not in the table, which cannot happen for a
    // registered link; `update` answers `None` then and nothing is read.
    state.game_servers.update(server_id, |info| {
        for &(id, value) in &packet.attributes {
            match id {
                ServerStatus::SERVER_LIST_STATUS => match Status::from_i32(value) {
                    Some(status) => info.set_status(status, login_status),
                    // Java stores any number; the server list only knows the six statuses.
                    None => warn!("Game server {server_id} sent the unknown status {value}, ignored."),
                },
                ServerStatus::SERVER_LIST_SQUARE_BRACKET => info.showing_brackets = value == ServerStatus::ON,
                ServerStatus::MAX_PLAYERS => info.max_players = value,
                ServerStatus::SERVER_TYPE => info.server_type = value,
                // NOTE(java-quirk): the Java game server writes the age as attribute 5
                // (`TEST_SERVER` here), but this side reads attribute 6. A real game server's age
                // is therefore never stored, and attribute 5 is not handled at all.
                ServerStatus::SERVER_AGE => info.age_limit = value,
                _ => {}
            }
        }
    });
}

// Java: PlayerTracert
async fn player_tracert<S: AccountStore>(store: &S, packet: PlayerTracert) {
    let hops = TracertHops {
        pc_ip: Some(packet.pc_ip),
        hops: [
            Some(packet.hop1),
            Some(packet.hop2),
            Some(packet.hop3),
            Some(packet.hop4),
        ],
    };
    // Java: LoginController.setAccountLastTracert
    if let Err(error) = store.set_tracert(&packet.account, &hops).await {
        warn!("Could not set last tracert: {error}");
    }
}

/// A game server answers `RequestCharacters`. Every reply is recorded, also one with no
/// characters: it tells the waiting server list that this game server has answered.
// Java: ReplyCharacters
fn reply_characters(state: &LoginState, server_id: i32, packet: &ReplyCharacters) {
    // Java: LoginController.setCharactersOnServer (nothing happens for an account that is not
    // logged in here).
    state.clients.set_characters_on_server(
        &packet.account,
        server_id,
        i32::from(packet.chars),
        &packet.delete_times,
    );
}

// Java: RequestTempBan (`banUser`)
async fn request_temp_ban<S: AccountStore>(state: &LoginState, store: &S, packet: &RequestTempBan) {
    if let Err(error) = store.temp_ban(&packet.account, packet.ban_time).await {
        warn!("RequestTempBan: {error}");
    }
    // NOTE(java-quirk): the game server sends the time the ban ends, and `addBanForAddress` takes
    // a duration and adds the current time to it. The address ban therefore lasts until about
    // twice the epoch time (practically forever). The reason, when there is one, is not stored.
    state.bans.add(&packet.ip, packet.ban_time);
}

/// A player asks the game server to change the account password.
// Java: ChangePassword
async fn change_password<S: AccountStore>(state: &LoginState, store: &S, packet: &ChangePassword) {
    // The answer goes to the game server that has the account online, which need not be this one.
    let owner = state
        .game_servers
        .account_on_game_server(&packet.account)
        .and_then(|id| state.game_servers.link(id));
    let Some(owner) = owner else {
        debug!(
            "Ignored ChangePassword for account '{}' requested by player '{}'.",
            packet.account, packet.character_name
        );
        return;
    };

    let current_hash = hash_password(&packet.old_pass);
    let new_hash = hash_password(&packet.new_pass);
    match store
        .change_password(&packet.account, &current_hash, &new_hash)
        .await
    {
        Ok(ChangePasswordResult::Changed) => {
            owner.change_password_response(&packet.character_name, MSG_PASSWORD_CHANGED);
        }
        Ok(ChangePasswordResult::UpdateFailed) => {
            warn!(
                "Password change failed for account '{}' requested by player '{}' (0 rows updated).",
                packet.account, packet.character_name
            );
            owner.change_password_response(&packet.character_name, MSG_PASSWORD_CHANGE_FAILED);
        }
        Ok(ChangePasswordResult::CurrentMismatch) => {
            owner.change_password_response(&packet.character_name, MSG_CURRENT_PASSWORD_MISMATCH);
        }
        // Java logs the exception and sends no answer.
        Err(error) => warn!(
            "Error while changing password for account '{}' requested by player '{}': {error}",
            packet.account, packet.character_name
        ),
    }
}
