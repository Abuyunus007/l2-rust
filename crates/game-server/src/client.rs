//! The client listener on `GameserverPort`.
// Java: org/l2jmobius/gameserver/GameServer.java
//   `new ConnectionManager<>(new InetSocketAddress(ServerConfig.PORT_GAME), GameClient::new, new GamePacketHandler())`
//
// PLACEHOLDER (M2-GS-01): the port is bound so a port clash fails startup (exit code 1) and the
// address is known, but every connection is closed right away. M2-GS-03 replaces
// `accept_loop` with `l2_net::listen` and the client sessions (ProtocolVersion, AuthLogin,
// character screen); those send `WorldCommand`s through `GameState::world`.
//
// NOTE(java-quirk): Java binds `InetSocketAddress(PORT_GAME)`, i.e. every interface, and never
// looks at `GameserverHostname`. Here the hostname is the bind address (`*` and the default
// `0.0.0.0` mean every interface), which behaves the same with the shipped config.

use l2_net::CancellationToken;
use tokio::net::TcpListener;
use tracing::{debug, info, warn};

use crate::{Server, StartupError};

/// The address the listener binds, from `GameserverHostname` and `GameserverPort`.
pub fn bind_address(hostname: &str, port: i32) -> String {
    let host = match hostname.trim() {
        "" | "*" => "0.0.0.0",
        other => other,
    };
    format!("{host}:{port}")
}

/// Binds the client port and starts the accept loop.
pub async fn start_listener(server: &mut Server) -> Result<(), StartupError> {
    let state = server.state();
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
    info!("Game client listener started on {shown} (client sessions are not implemented yet).");
    let cancel = server.cancel_token();
    server.spawn(accept_loop(listener, cancel));
    Ok(())
}

/// Accepts connections and closes them. Replaced by M2-GS-03.
async fn accept_loop(listener: TcpListener, cancel: CancellationToken) {
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            () = cancel.cancelled() => return,
        };
        match accepted {
            Ok((stream, peer)) => {
                debug!("Connection from {peer} closed: client sessions are not implemented yet.");
                drop(stream);
            }
            Err(e) => warn!("accept failed: {e}"),
        }
    }
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
