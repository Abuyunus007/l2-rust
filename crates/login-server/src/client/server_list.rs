//! Builds the `ServerList` packet.
//!
//! Java: `loginserver/network/serverpackets/ServerList.java` (constructor).
//!
//! TODO(M1-LS-03): server addresses per client subnet, ports, player counts, GM-only hiding,
//! the accessLevel < 0 rule, and the character counts from `ReplyCharacters`. Until then every
//! registered server is listed as down at 127.0.0.1, which is what Java shows for registered
//! game servers that are not connected.

use l2_protocol_login::server::{ServerEntry, ServerList};

use crate::state::{LoginState, ServerStatus};

pub fn build_server_list(state: &LoginState, last_server: i32) -> ServerList {
    let servers = state
        .game_servers
        .registered_ids()
        .into_iter()
        .filter_map(|id| state.game_servers.registered(id))
        .map(|info| ServerEntry {
            server_id: info.id as u8, // NOTE(java-quirk): Java writeByte(id) keeps the low 8 bits
            ip: [127, 0, 0, 1],
            port: 0,
            age_limit: 0,
            pvp: true,
            current_players: 0,
            max_players: 0,
            up: info.authed && info.status != ServerStatus::Down,
            server_type: 1,
            brackets: false,
        })
        .collect();
    ServerList {
        last_server: last_server as u8,
        servers,
        char_info: None,
    }
}
