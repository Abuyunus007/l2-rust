//! The server list, the character counts that go into it, and the "may this account enter
//! game server N" check.
//!
//! Java: `loginserver/network/serverpackets/ServerList.java`,
//! `LoginController.isLoginPossible` / `setCharactersOnServer` / `getCharactersOnAccount`,
//! `GameServerTable.GameServerInfo.canLogin` / `getServerAddress`.
//!
//! What the list needs from the game server side comes through the [`GsView`] trait, so this
//! file does not depend on how the game server links are stored. [`GameServerTable`] implements
//! it from the data it has today; M1-LS-04 fills in the missing fields (see the
//! `TODO(M1-LS-04)` marks) and nothing here has to change.

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use std::time::Duration;

use l2_protocol_login::server::{ServerCharInfo, ServerEntry, ServerList};
use l2_util::time::now_millis;
use tokio::sync::watch;
use tracing::warn;

use crate::state::{GameServerTable, LoginState, ServerStatus};
use crate::subnet::GameServerAddress;

/// How long the server list waits for the game servers' `ReplyCharacters`.
// Java: ServerList constructor (10 × 50 ms)
pub const CHARACTER_WAIT: Duration = Duration::from_millis(500);

/// A game server as the server list and the login check see it: a copy, taken at one moment.
// Java: GameServerTable.GameServerInfo (the getters these two users call)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GsServerSnapshot {
    pub id: i32,
    pub status: ServerStatus,
    pub authed: bool,
    pub port: i32,
    pub current_players: i32,
    pub max_players: i32,
    /// Bit flags, see `ServerEntry::server_type`.
    pub server_type: i32,
    pub brackets: bool,
    /// The host list from `GameServerAuth`: subnet → address to show clients inside it.
    pub addresses: Vec<GameServerAddress>,
}

impl GsServerSnapshot {
    /// Java `getServerAddress(InetAddress)`: the first entry whose subnet contains the client.
    pub fn server_address(&self, client: IpAddr) -> Option<&str> {
        self.addresses
            .iter()
            .find(|a| a.matches(client))
            .map(GameServerAddress::server_address)
    }

    // Java: GameServerInfo.canLogin
    pub fn can_login(&self, access_level: i32) -> bool {
        // DOWN lets nobody in.
        if self.status == ServerStatus::Down {
            return false;
        }
        // GM-only and full servers only let in accounts with a positive access level.
        if self.status == ServerStatus::GmOnly || self.current_players >= self.max_players {
            return access_level > 0;
        }
        access_level >= 0
    }
}

/// The game server side, as far as the client protocol needs it.
pub trait GsView: Sync {
    /// Every registered game server, ascending by id (Java `getRegisteredGameServers`).
    fn servers(&self) -> Vec<GsServerSnapshot>;

    /// One registered game server.
    fn server(&self, id: i32) -> Option<GsServerSnapshot>;

    /// Sends `RequestCharacters(account)` to every authed game server and returns their ids,
    /// so the server list knows which replies to wait for.
    // Java: LoginController.getCharactersOnAccount
    fn request_characters(&self, account: &str) -> Vec<i32>;
}

impl GsView for GameServerTable {
    fn servers(&self) -> Vec<GsServerSnapshot> {
        self.registered_ids()
            .into_iter()
            .filter_map(|id| self.server(id))
            .collect()
    }

    fn server(&self, id: i32) -> Option<GsServerSnapshot> {
        self.registered(id).map(|info| GsServerSnapshot {
            id: info.id,
            status: info.status,
            authed: info.authed,
            // TODO(M1-LS-04): port, player count, max players, type, brackets and the host list
            // come from the game server link (`GameServerAuth` / `ServerStatus`).
            port: 0,
            current_players: 0,
            max_players: 0,
            server_type: 0,
            brackets: false,
            addresses: Vec::new(),
        })
    }

    fn request_characters(&self, _account: &str) -> Vec<i32> {
        // TODO(M1-LS-04): send `RequestCharacters` over each authed link.
        Vec::new()
    }
}

/// What the replies of the game servers have told so far.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharacterState {
    /// Servers `RequestCharacters` went to.
    expected: BTreeSet<i32>,
    /// Servers that answered (also with zero characters).
    replied: BTreeSet<i32>,
    /// Server id → character count. Only counts above 0, as in Java.
    chars: BTreeMap<i32, i32>,
    /// Server id → deletion times (epoch ms). Only non-empty lists, as in Java.
    delete_times: BTreeMap<i32, Vec<i64>>,
}

impl CharacterState {
    /// Every server that was asked has answered (also true when nobody was asked).
    pub fn is_complete(&self) -> bool {
        self.expected.is_subset(&self.replied)
    }

    pub fn chars(&self) -> &BTreeMap<i32, i32> {
        &self.chars
    }

    pub fn delete_times(&self) -> &BTreeMap<i32, Vec<i64>> {
        &self.delete_times
    }
}

/// The character info one logged-in client collects from the game servers. The client's
/// session waits on it; the game server link records into it through the
/// [`ClientRegistry`](super::registry::ClientRegistry).
// Java: LoginClient._charsOnServers / _charsToDelete
#[derive(Debug)]
pub struct CharacterReplies {
    state: watch::Sender<CharacterState>,
}

impl Default for CharacterReplies {
    fn default() -> Self {
        Self {
            state: watch::channel(CharacterState::default()).0,
        }
    }
}

impl CharacterReplies {
    /// Notes the servers a `RequestCharacters` was sent to.
    pub fn expect(&self, servers: impl IntoIterator<Item = i32>) {
        self.state.send_modify(|s| s.expected.extend(servers));
    }

    /// A `ReplyCharacters` arrived: remembers the count and the deletion times.
    // Java: LoginController.setCharactersOnServer / LoginClient.setCharsOnServ
    pub fn record_reply(&self, server_id: i32, chars: i32, delete_times: &[i64]) {
        self.state.send_modify(|s| {
            s.replied.insert(server_id);
            if chars > 0 {
                s.chars.insert(server_id, chars);
            }
            if !delete_times.is_empty() {
                s.delete_times.insert(server_id, delete_times.to_vec());
            }
        });
    }

    pub fn snapshot(&self) -> CharacterState {
        self.state.borrow().clone()
    }

    /// Waits until every asked server has replied, but at most `timeout`, and returns what is
    /// there then.
    ///
    /// Java sleeps 50 ms up to ten times until *both* of its maps exist, which usually means the
    /// full 500 ms (the deletion map only appears when some character is being deleted). This
    /// returns as soon as the replies are in.
    pub async fn wait_for_replies(&self, timeout: Duration) -> CharacterState {
        let mut updates = self.state.subscribe();
        // A timeout is fine: the partial result is what Java sends too.
        let _ = tokio::time::timeout(timeout, updates.wait_for(CharacterState::is_complete)).await;
        self.snapshot()
    }
}

/// Who asks for the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientView {
    pub ip: IpAddr,
    pub access_level: i32,
    pub last_server: i32,
}

/// Java `InetAddress.getByName(address).getAddress()` cut to 4 bytes, with the loopback address
/// when there is no address or it does not parse (the fallback in `ServerData`).
fn entry_ip(address: Option<&str>) -> [u8; 4] {
    const LOOPBACK: [u8; 4] = [127, 0, 0, 1];
    let Some(text) = address else {
        // `InetAddress.getByName(null)` is the loopback address.
        return LOOPBACK;
    };
    match text.parse::<IpAddr>().map(|ip| ip.to_canonical()) {
        Ok(IpAddr::V4(v4)) => v4.octets(),
        // NOTE(java-quirk): Java copies the first four bytes of an IPv6 address.
        Ok(IpAddr::V6(v6)) => {
            let o = v6.octets();
            [o[0], o[1], o[2], o[3]]
        }
        Err(_) => {
            // TODO(M1-LS-04): Java resolves host names here (DNS). Resolve them when the game
            // server registers (async) and keep the IP in the host list.
            warn!("ServerList: cannot use '{text}' as a game server address, sending {LOOPBACK:?}.");
            LOOPBACK
        }
    }
}

/// Java: the `ServerList` constructor and `writeImpl`, as a packet struct.
///
/// `now_ms` turns the absolute deletion times of `ReplyCharacters` into the seconds the client
/// wants.
pub fn build_server_list(
    servers: &[GsServerSnapshot],
    client: &ClientView,
    characters: &CharacterState,
    now_ms: i64,
) -> ServerList {
    let entries = servers
        .iter()
        // GM-only servers are hidden from accounts below access level 1.
        .filter(|gs| !(gs.status == ServerStatus::GmOnly && client.access_level < 1))
        .map(|gs| {
            // Banned accounts see every server as down. The second half of Java's condition
            // can never fire after the filter above; it is kept for parity.
            let status = if client.access_level < 0
                || (gs.status == ServerStatus::GmOnly && client.access_level <= 0)
            {
                ServerStatus::Down
            } else {
                gs.status
            };
            ServerEntry {
                // NOTE(java-quirk): `writeByte(id)` keeps the low 8 bits.
                server_id: gs.id as u8,
                ip: entry_ip(gs.server_address(client.ip)),
                port: gs.port,
                // NOTE(java-quirk): `_ageLimit = 0`; the age limit a game server reports is
                // never shown.
                age_limit: 0,
                // `GameServerInfo.IS_PVP` is a constant `true`.
                pvp: true,
                current_players: gs.current_players as u16,
                max_players: gs.max_players as u16,
                up: status != ServerStatus::Down,
                server_type: gs.server_type,
                brackets: gs.brackets,
            }
        })
        .collect();

    // Only servers with at least one character are listed (`setCharactersOnServer` skips 0).
    let char_info = (!characters.chars.is_empty()).then(|| {
        characters
            .chars
            .iter()
            .map(|(&id, &count)| ServerCharInfo {
                server_id: id as u8,
                chars: count as u8,
                seconds_until_delete: characters
                    .delete_times
                    .get(&id)
                    .map(|times| times.iter().map(|&t| ((t - now_ms) / 1000) as i32).collect())
                    .unwrap_or_default(),
            })
            .collect()
    });

    ServerList {
        last_server: client.last_server as u8,
        servers: entries,
        char_info,
    }
}

/// Builds the list for a client, first giving the game servers up to `wait` to report its
/// characters.
pub async fn make_server_list(
    gs: &dyn GsView,
    replies: &CharacterReplies,
    client: &ClientView,
    wait: Duration,
) -> ServerList {
    let servers = gs.servers();
    let characters = replies.wait_for_replies(wait).await;
    build_server_list(&servers, client, &characters, now_millis())
}

/// The outcome of the login check before the database is touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerLogin {
    /// Refused: unknown or not connected server, or the server does not take this account.
    Denied,
    /// Allowed. `remember` is true when `accounts.lastServer` has to change.
    Allowed { remember: bool },
}

// Java: LoginController.isLoginPossible (without the database write)
pub fn check_server_login(
    gs: &dyn GsView,
    access_level: i32,
    last_server: i32,
    server_id: i32,
) -> ServerLogin {
    match gs.server(server_id) {
        Some(server) if server.authed && server.can_login(access_level) => ServerLogin::Allowed {
            remember: last_server != server_id,
        },
        _ => ServerLogin::Denied,
    }
}

/// Java `isLoginPossible`: the check, and `accounts.lastServer` updated when it changes.
pub async fn is_login_possible(
    state: &LoginState,
    gs: &dyn GsView,
    account: &str,
    access_level: i32,
    last_server: i32,
    server_id: i32,
) -> bool {
    match check_server_login(gs, access_level, last_server, server_id) {
        ServerLogin::Denied => false,
        ServerLogin::Allowed { remember } => {
            if remember && let Err(e) = state.repository.update_last_server(account, server_id).await {
                // Like Java, the login still goes ahead.
                warn!("Could not set lastServer: {e}");
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use l2_net::{Decode, Encode, PacketReader};

    use super::*;

    fn snapshot(id: i32, status: ServerStatus) -> GsServerSnapshot {
        GsServerSnapshot {
            id,
            status,
            authed: true,
            port: 7777,
            current_players: 10,
            max_players: 100,
            server_type: 1,
            brackets: false,
            addresses: vec![
                GameServerAddress::new("192.168.0.0/16", "192.168.0.5").unwrap(),
                GameServerAddress::new("0.0.0.0/0", "203.0.113.7").unwrap(),
            ],
        }
    }

    fn client(access_level: i32) -> ClientView {
        ClientView {
            ip: "8.8.4.4".parse().unwrap(),
            access_level,
            last_server: 2,
        }
    }

    fn build(servers: &[GsServerSnapshot], client: &ClientView, characters: &CharacterState) -> ServerList {
        build_server_list(servers, client, characters, 1_000_000)
    }

    struct FakeGs {
        servers: Vec<GsServerSnapshot>,
    }

    impl GsView for FakeGs {
        fn servers(&self) -> Vec<GsServerSnapshot> {
            self.servers.clone()
        }
        fn server(&self, id: i32) -> Option<GsServerSnapshot> {
            self.servers.iter().find(|s| s.id == id).cloned()
        }
        fn request_characters(&self, _account: &str) -> Vec<i32> {
            self.servers.iter().filter(|s| s.authed).map(|s| s.id).collect()
        }
    }

    #[test]
    fn entries_carry_the_server_data() {
        let list = build(
            &[snapshot(1, ServerStatus::Normal)],
            &client(0),
            &CharacterState::default(),
        );
        assert_eq!(list.last_server, 2);
        assert_eq!(
            list.servers,
            [ServerEntry {
                server_id: 1,
                ip: [203, 0, 113, 7],
                port: 7777,
                age_limit: 0,
                pvp: true,
                current_players: 10,
                max_players: 100,
                up: true,
                server_type: 1,
                brackets: false,
            }]
        );
        assert_eq!(list.char_info, None);
    }

    #[test]
    fn the_address_depends_on_the_clients_subnet() {
        let servers = [snapshot(1, ServerStatus::Normal)];
        let mut lan = client(0);
        lan.ip = "192.168.1.20".parse().unwrap();
        assert_eq!(
            build(&servers, &lan, &CharacterState::default()).servers[0].ip,
            [192, 168, 0, 5]
        );
        let wan = client(0);
        assert_eq!(
            build(&servers, &wan, &CharacterState::default()).servers[0].ip,
            [203, 0, 113, 7]
        );
    }

    #[test]
    fn no_matching_subnet_or_bad_address_falls_back_to_loopback() {
        let mut none = snapshot(1, ServerStatus::Normal);
        none.addresses.clear();
        let mut bad = snapshot(2, ServerStatus::Normal);
        bad.addresses = vec![GameServerAddress::new("0.0.0.0/0", "no.such.host").unwrap()];
        let list = build(&[none, bad], &client(0), &CharacterState::default());
        assert_eq!(list.servers[0].ip, [127, 0, 0, 1]);
        assert_eq!(list.servers[1].ip, [127, 0, 0, 1]);
    }

    #[test]
    fn gm_only_servers_are_hidden_from_players_and_shown_to_gms() {
        let servers = [
            snapshot(1, ServerStatus::Normal),
            snapshot(2, ServerStatus::GmOnly),
        ];
        let ids = |list: &ServerList| list.servers.iter().map(|s| s.server_id).collect::<Vec<_>>();
        let none = CharacterState::default();
        assert_eq!(ids(&build(&servers, &client(0), &none)), [1]);
        let gm = build(&servers, &client(1), &none);
        assert_eq!(ids(&gm), [1, 2]);
        assert!(gm.servers[1].up, "GM-only is shown as up to a GM");
    }

    #[test]
    fn a_banned_account_sees_every_server_down() {
        let servers = [
            snapshot(1, ServerStatus::Normal),
            snapshot(2, ServerStatus::Good),
            snapshot(3, ServerStatus::GmOnly),
        ];
        let list = build(&servers, &client(-1), &CharacterState::default());
        // The GM-only server is hidden (level < 1); the others are down.
        assert_eq!(list.servers.len(), 2);
        assert!(list.servers.iter().all(|s| !s.up));
    }

    #[test]
    fn the_up_flag_follows_the_status() {
        let servers = [
            snapshot(1, ServerStatus::Down),
            snapshot(2, ServerStatus::Full),
            snapshot(3, ServerStatus::Auto),
        ];
        let list = build(&servers, &client(0), &CharacterState::default());
        let up: Vec<bool> = list.servers.iter().map(|s| s.up).collect();
        assert_eq!(up, [false, true, true]);
    }

    #[test]
    fn character_counts_and_delete_times_are_listed_per_server() {
        let replies = CharacterReplies::default();
        replies.expect([1, 2, 3]);
        // 1_000_000 ms is "now" in `build`: 90 s and 3_600.5 s from now, one already past.
        replies.record_reply(1, 2, &[1_090_000, 4_600_500]);
        replies.record_reply(2, 0, &[]);
        replies.record_reply(3, 1, &[]);
        let state = replies.snapshot();
        assert!(state.is_complete());
        let list = build(&[snapshot(1, ServerStatus::Normal)], &client(0), &state);
        assert_eq!(
            list.char_info,
            Some(vec![
                ServerCharInfo {
                    server_id: 1,
                    chars: 2,
                    seconds_until_delete: vec![90, 3600],
                },
                // Server 2 has no characters: not listed at all.
                ServerCharInfo {
                    server_id: 3,
                    chars: 1,
                    seconds_until_delete: vec![],
                },
            ])
        );
        // On the wire: ... u16 0, u8 2, [1, 2, 2, 90, 3600], [3, 1, 0].
        let bytes = list.to_bytes();
        let tail = &bytes[bytes.len() - (2 + 1 + 3 + 8 + 3)..];
        assert_eq!(&tail[..3], [0, 0, 2]);
        assert_eq!(&tail[3..6], [1, 2, 2]);
        assert_eq!(&tail[6..10], 90i32.to_le_bytes());
        assert_eq!(&tail[10..14], 3600i32.to_le_bytes());
        assert_eq!(&tail[14..], [3, 1, 0]);
    }

    #[test]
    fn past_deletion_times_are_negative_seconds() {
        let replies = CharacterReplies::default();
        replies.record_reply(1, 1, &[999_000]);
        let list = build(
            &[snapshot(1, ServerStatus::Normal)],
            &client(0),
            &replies.snapshot(),
        );
        let info = list.char_info.unwrap();
        assert_eq!(info[0].seconds_until_delete, [-1]);
    }

    #[test]
    fn without_replies_the_list_ends_with_a_zero_count() {
        let list = build(
            &[snapshot(1, ServerStatus::Normal)],
            &client(0),
            &CharacterState::default(),
        );
        let bytes = list.to_bytes();
        assert_eq!(&bytes[bytes.len() - 3..], [0, 0, 0], "u16 unknown, u8 count 0");
        let decoded = ServerList::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, list);
    }

    #[test]
    fn replies_with_zero_characters_leave_the_list_empty() {
        let replies = CharacterReplies::default();
        replies.expect([1]);
        replies.record_reply(1, 0, &[]);
        assert!(replies.snapshot().is_complete());
        assert!(replies.snapshot().chars().is_empty());
    }

    #[tokio::test]
    async fn waiting_returns_at_once_when_nobody_was_asked() {
        let replies = CharacterReplies::default();
        let started = std::time::Instant::now();
        let state = replies.wait_for_replies(Duration::from_secs(10)).await;
        assert!(state.is_complete());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn waiting_returns_when_the_last_reply_is_in() {
        let replies = Arc::new(CharacterReplies::default());
        replies.expect([1, 2]);
        let writer = Arc::clone(&replies);
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            writer.record_reply(1, 3, &[]);
            tokio::time::sleep(Duration::from_millis(20)).await;
            writer.record_reply(2, 1, &[]);
        });
        let started = std::time::Instant::now();
        let state = replies.wait_for_replies(Duration::from_secs(10)).await;
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "did not wait for the timeout"
        );
        assert!(state.is_complete());
        assert_eq!(state.chars().get(&1), Some(&3));
        assert_eq!(state.chars().get(&2), Some(&1));
        task.await.unwrap();
    }

    #[tokio::test]
    async fn waiting_gives_up_after_the_timeout_with_what_is_there() {
        let replies = CharacterReplies::default();
        replies.expect([1, 2]);
        replies.record_reply(1, 4, &[]);
        let state = replies.wait_for_replies(Duration::from_millis(60)).await;
        assert!(!state.is_complete());
        assert_eq!(state.chars().get(&1), Some(&4));
        assert_eq!(state.chars().get(&2), None);
    }

    #[tokio::test]
    async fn make_server_list_uses_the_replies() {
        let gs = FakeGs {
            servers: vec![snapshot(1, ServerStatus::Normal)],
        };
        let replies = CharacterReplies::default();
        replies.expect(gs.request_characters("hero"));
        replies.record_reply(1, 5, &[]);
        let list = make_server_list(&gs, &replies, &client(0), Duration::from_secs(10)).await;
        assert_eq!(list.char_info.unwrap()[0].chars, 5);
    }

    #[test]
    fn can_login_follows_java() {
        let mut gs = snapshot(1, ServerStatus::Normal);
        // Room on the server: any account that is not banned.
        assert!(gs.can_login(0));
        assert!(gs.can_login(1));
        assert!(!gs.can_login(-1));
        // Full: only access level above 0.
        gs.current_players = 100;
        assert!(!gs.can_login(0));
        assert!(gs.can_login(1));
        // GM only.
        gs.current_players = 0;
        gs.status = ServerStatus::GmOnly;
        assert!(!gs.can_login(0));
        assert!(gs.can_login(2));
        // Down: nobody.
        gs.status = ServerStatus::Down;
        assert!(!gs.can_login(1));
    }

    #[test]
    fn server_login_needs_an_authed_server() {
        let mut offline = snapshot(2, ServerStatus::Normal);
        offline.authed = false;
        let gs = FakeGs {
            servers: vec![snapshot(1, ServerStatus::Normal), offline],
        };
        assert_eq!(
            check_server_login(&gs, 0, 2, 1),
            ServerLogin::Allowed { remember: true }
        );
        // Same as the stored lastServer: no database write.
        assert_eq!(
            check_server_login(&gs, 0, 1, 1),
            ServerLogin::Allowed { remember: false }
        );
        assert_eq!(
            check_server_login(&gs, 0, 1, 2),
            ServerLogin::Denied,
            "not authed"
        );
        assert_eq!(check_server_login(&gs, 0, 1, 9), ServerLogin::Denied, "unknown");
        assert_eq!(check_server_login(&gs, -1, 1, 1), ServerLogin::Denied, "banned");
    }

    #[tokio::test]
    async fn the_game_server_table_view_lists_registered_servers_as_down() {
        let (state, _rx) =
            crate::state::test_support::state_with(crate::state::test_support::default_config());
        let servers = state.game_servers.servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].id, 1);
        assert_eq!(servers[0].status, ServerStatus::Down);
        assert!(!servers[0].authed);
        assert_eq!(state.game_servers.server(9), None);
        assert!(state.game_servers.request_characters("hero").is_empty());
    }
}
