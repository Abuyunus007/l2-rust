//! The client listener on `GameserverPort` and the per-connection sessions.
// Java: org/l2jmobius/gameserver/GameServer.java
//   `new ConnectionManager<>(new InetSocketAddress(ServerConfig.PORT_GAME), GameClient::new, new GamePacketHandler())`
// Java: org/l2jmobius/gameserver/network/GameClient.java, GamePacketHandler.java
//
// NOTE(java-quirk): Java binds `InetSocketAddress(PORT_GAME)`, i.e. every interface, and never
// looks at `GameserverHostname`. Here the hostname is the bind address (`*` and the default
// `0.0.0.0` mean every interface), which behaves the same with the shipped config.

pub mod charlist;
pub mod cipher;
pub mod registry;
pub mod session;

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use l2_config::{IniFile, NetworkConfig};
use l2_net::{ConnectionConfig, FrameCipher, NoCipher};
use l2_world::SessionId;
use tokio::net::TcpListener;
use tracing::info;

use self::charlist::{CharacterSource, DbCharacterList};
use self::registry::{KickOutcome, SessionRegistry};
use self::session::ClientSession;
use crate::ls_link::{LsLink, SessionKey};
use crate::state::WorldHandle;
use crate::{Server, StartupError};

/// A boxed future, for the object-safe traits of this module.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// File name of the network settings (shared with the login server's format).
pub const NETWORK_CONFIG_FILE: &str = "Network.ini";

/// What the sessions need from the login server link. [`LsLink`] in the server, a fake in tests.
pub trait LoginGate: Send + Sync + 'static {
    /// Java `addWaitingClientAndSendRequest` + the `PlayerAuthResponse`: is this session key valid?
    fn auth_player<'a>(&'a self, account: &'a str, key: SessionKey) -> BoxFuture<'a, bool>;
    /// Java `sendLogout`.
    fn player_logout<'a>(&'a self, account: &'a str) -> BoxFuture<'a, ()>;
    /// Java `ServerConfig.SERVER_ID` (shown in `KeyPacket`).
    fn server_id(&self) -> i32;
}

/// The link and the id from `hexid.txt` for the time before the first registration.
struct LinkGate {
    link: LsLink,
    hexid_server_id: i32,
}

impl LoginGate for LinkGate {
    fn auth_player<'a>(&'a self, account: &'a str, key: SessionKey) -> BoxFuture<'a, bool> {
        Box::pin(self.link.auth_player(account, key))
    }

    fn player_logout<'a>(&'a self, account: &'a str) -> BoxFuture<'a, ()> {
        Box::pin(self.link.player_logout(account))
    }

    fn server_id(&self) -> i32 {
        self.link
            .status()
            .server_id
            .map_or(self.hexid_server_id, i32::from)
    }
}

/// Everything the sessions share.
pub struct ClientContext {
    pub gate: Arc<dyn LoginGate>,
    pub characters: Arc<dyn CharacterSource>,
    pub registry: Arc<SessionRegistry>,
    pub world: WorldHandle,
    /// `AllowedProtocolRevisions`.
    pub protocol_list: Vec<i32>,
    /// `PacketEncryption`.
    pub packet_encryption: bool,
    /// `CharMaxNumber`.
    pub max_characters: i32,
}

impl ClientContext {
    /// A session for a new connection.
    pub fn session(
        self: &Arc<Self>,
        session: SessionId,
        peer: SocketAddr,
    ) -> (ClientSession, Box<dyn FrameCipher>) {
        // Every connection starts without a cipher: ProtocolVersion and KeyPacket are in clear.
        (
            ClientSession::new(Arc::clone(self), session, peer.ip()),
            Box::new(NoCipher),
        )
    }
}

/// The address the listener binds, from `GameserverHostname` and `GameserverPort`.
pub fn bind_address(hostname: &str, port: i32) -> String {
    let host = match hostname.trim() {
        "" | "*" => "0.0.0.0",
        other => other,
    };
    format!("{host}:{port}")
}

/// Binds the client port and starts accepting game clients. Needs the login server link, so it
/// runs after `ls_link::start`.
pub async fn start_listener(server: &mut Server) -> Result<(), StartupError> {
    let state = Arc::clone(server.state());
    let address = bind_address(
        &state.config.server.gameserver_hostname,
        state.config.server.port_game,
    );
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|source| StartupError::Listen {
            what: "Game client",
            address: address.clone(),
            source,
        })?;
    server.client_addr = listener.local_addr().ok();
    let shown = server.client_addr.map_or(address, |a| a.to_string());

    let link = server.ls_link().cloned().ok_or_else(|| {
        StartupError::Internal("the login server link must start before the client listener".into())
    })?;
    let registry = Arc::new(SessionRegistry::default());
    let ctx = Arc::new(ClientContext {
        gate: Arc::new(LinkGate {
            link: link.clone(),
            hexid_server_id: state.hexid.as_ref().map_or(0, |h| h.server_id),
        }),
        characters: Arc::new(DbCharacterList::new(
            state.pool.clone(),
            Arc::clone(&state.datapack),
        )),
        registry: Arc::clone(&registry),
        world: state.world.clone(),
        protocol_list: state.config.manual.server.protocol_list.clone(),
        packet_encryption: state.config.server.packet_encryption,
        max_characters: state.config.server.max_characters_number_per_account,
    });

    // Java: ConnectionConfig (Network.ini). A missing file gives Java's defaults.
    let processors = std::thread::available_parallelism().map_or(1, |n| i32::try_from(n.get()).unwrap_or(1));
    let network = NetworkConfig::from_ini(
        &IniFile::load(state.paths.config_dir.join(NETWORK_CONFIG_FILE))?,
        processors,
    );
    let config = Arc::new(ConnectionConfig {
        drop_packets: network.drop_packets,
        drop_packet_threshold: usize::try_from(network.drop_packet_threshold).unwrap_or(250),
        tcp_nodelay: true,
    });

    // KickPlayer from the login server goes through the registry.
    if let Some(mut kicks) = link.take_kicks() {
        let world = state.world.clone();
        let registry = Arc::clone(&registry);
        let link = link.clone();
        server.spawn(async move {
            while let Some(kick) = kicks.recv().await {
                if registry.kick(&kick.account, &world) == KickOutcome::NotConnected {
                    link.player_logout(&kick.account).await;
                }
            }
        });
    }

    let next_id = AtomicU64::new(1);
    let factory =
        move |peer: SocketAddr| Some(ctx.session(SessionId(next_id.fetch_add(1, Ordering::Relaxed)), peer));
    server.spawn(l2_net::listen(listener, config, server.cancel_token(), factory));
    info!("Game client listener started on {shown}.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_address_uses_hostname_and_port() {
        assert_eq!(bind_address("127.0.0.1", 7778), "127.0.0.1:7778");
        assert_eq!(bind_address("0.0.0.0", 7777), "0.0.0.0:7777");
        assert_eq!(bind_address("*", 7777), "0.0.0.0:7777");
        assert_eq!(bind_address("", 0), "0.0.0.0:0");
    }
}

#[cfg(test)]
mod session_tests;
