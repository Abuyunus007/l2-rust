//! `GameServerAuth`: the registration decision, and saving a new server in the database.
// Java: org/l2jmobius/loginserver/network/gameserverpackets/receive/GameServerAuth.java (`handleRegProcess`)
// Java: org/l2jmobius/loginserver/data/GameServerTable.java (`registerServerOnDB`)
//
// The decision is a plain function of the table, the config switch and the packet, so every
// branch is tested without a socket. Java decides and mutates in several steps; here the whole
// decision runs under one lock of the table, which only closes the small races Java has
// between "is the id free" and "register it".
//
// Hex id: a game server sends its hex id as raw bytes and compares them byte for byte with what
// is registered. What the database stores is `BigInteger(bytes).toString(16)`, and reading it
// back gives the *minimal* two's complement bytes (`hex_to_string` / `string_to_hex` in
// `l2-db`). Those are the same bytes only when the game server's bytes were minimal already.

use std::sync::Arc;

use l2_db::{DbError, LoginRepository, hex_to_string};
use l2_protocol_login::link::{GameServerAuth, LoginServerFail};
use tracing::{error, info};

use crate::gs::link::GsLink;
use crate::gs::table::{insert_if_absent, insert_with_first_available_id};
use crate::state::{GameServerInfo, GameServerTable, ServerStatus, lock};

/// A server that was accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    /// The id the server got. Differs from the requested one when an alternative was assigned.
    pub server_id: i32,
    /// True for a server that was not in the table before: it is saved in the database.
    pub is_new: bool,
    /// The hex id as registered (the bytes the game server sent).
    pub hex_id: Vec<u8>,
    /// The `host` column to save (`GameServerInfo.getExternalHost`).
    pub external_host: Option<String>,
}

impl GameServerTable {
    /// Decides a `GameServerAuth` and, when it is accepted, attaches `link` to the registered
    /// row (creating it for a new server). On a refusal the error is the `LoginServerFail`
    /// reason code to send before closing the link.
    ///
    /// `accept_new` is `AcceptNewGameServer`.
    // Java: GameServerAuth.handleRegProcess
    pub fn register_game_server(
        &self,
        accept_new: bool,
        request: &GameServerAuth,
        link: &Arc<GsLink>,
    ) -> Result<Registration, u8> {
        let desired_id = i32::from(request.desired_id);
        let mut registered = lock(&self.registered);

        let new_info = || {
            let mut info = GameServerInfo::new(desired_id, request.hex_id.clone());
            info.link = Some(Arc::clone(link));
            info
        };

        let Some(known) = registered.get_mut(&desired_id) else {
            if !accept_new {
                info!(
                    "Rejected GS auth for desiredServerId={desired_id} because new game servers are not accepted."
                );
                return Err(LoginServerFail::REASON_WRONG_HEXID);
            }
            let mut info = new_info();
            self.attach_request(&mut info, request, link);
            if insert_if_absent(&mut registered, desired_id, info).is_err() {
                info!(
                    "Rejected GS auth for desiredServerId={desired_id} because the id became reserved during registration."
                );
                return Err(LoginServerFail::REASON_ID_RESERVED);
            }
            return Ok(Self::registration(&registered, desired_id, true));
        };

        // NOTE(java-quirk): the raw bytes from the game server are compared with the registered
        // bytes. A hex id read from the database is always minimal (no redundant leading 0x00 or
        // 0xFF), so a game server that sends a longer form of the same number is "wrong" after the
        // login server restarted.
        if known.hex_id == request.hex_id {
            if known.authed {
                info!(
                    "Rejected GS auth for desiredServerId={desired_id} due to already authenticated session."
                );
                return Err(LoginServerFail::REASON_ALREADY_LOGGED_IN);
            }
            let name = self.server_name(desired_id);
            known.attach(
                name,
                Arc::clone(link),
                request.port,
                &request.subnets,
                &request.hosts,
                request.max_players,
            );
            return Ok(Self::registration(&registered, desired_id, false));
        }

        if !accept_new || !request.accept_alternate {
            info!(
                "Rejected GS auth for desiredServerId={desired_id} due to wrong hexId and no alternative id allowed."
            );
            return Err(LoginServerFail::REASON_WRONG_HEXID);
        }

        match insert_with_first_available_id(&mut registered, self.server_names(), new_info()) {
            Ok(id) => {
                // The row is in the table now; attach under its new id so the log lines name it.
                let name = self.server_name(id);
                if let Some(row) = registered.get_mut(&id) {
                    row.attach(
                        name,
                        Arc::clone(link),
                        request.port,
                        &request.subnets,
                        &request.hosts,
                        request.max_players,
                    );
                }
                Ok(Self::registration(&registered, id, true))
            }
            Err(_) => {
                info!(
                    "Rejected GS auth for desiredServerId={desired_id} due to no free alternative id available."
                );
                Err(LoginServerFail::REASON_NO_FREE_ID)
            }
        }
    }

    fn attach_request(&self, info: &mut GameServerInfo, request: &GameServerAuth, link: &Arc<GsLink>) {
        info.attach(
            self.server_name(info.id),
            Arc::clone(link),
            request.port,
            &request.subnets,
            &request.hosts,
            request.max_players,
        );
    }

    fn registration(
        registered: &std::collections::BTreeMap<i32, GameServerInfo>,
        id: i32,
        is_new: bool,
    ) -> Registration {
        // The row was just inserted or found, so it is there.
        let row = registered.get(&id);
        Registration {
            server_id: id,
            is_new,
            hex_id: row.map(|row| row.hex_id.clone()).unwrap_or_default(),
            external_host: row.and_then(GameServerInfo::external_host),
        }
    }

    /// Marks the row down when a registered game server's link ended and logs it.
    pub fn link_closed(&self, server_id: i32, link_id: u64, login_status: ServerStatus) {
        if self.set_down(server_id, link_id, login_status) {
            info!(
                "Server [{server_id}] {} is now marked as disconnected.",
                self.server_name(server_id).unwrap_or("null")
            );
        }
    }
}

/// Where new game servers are saved. The login server uses the database; tests use a recorder.
pub trait GameServerStore: Send + Sync + 'static {
    /// Inserts a `gameservers` row (`INSERT INTO gameservers (hexid,server_id,host)`).
    fn save(
        &self,
        server_id: i32,
        hex_id: &str,
        host: &str,
    ) -> impl Future<Output = Result<(), DbError>> + Send;
}

impl GameServerStore for LoginRepository {
    async fn save(&self, server_id: i32, hex_id: &str, host: &str) -> Result<(), DbError> {
        self.register_game_server(server_id, hex_id, host).await
    }
}

/// Saves a newly registered server. Like Java, a failure is only logged: the server stays
/// registered and authed for this session and is registered again on its next start.
// Java: GameServerTable.registerServerOnDB
pub async fn save_new_server<S: GameServerStore>(store: &S, registration: &Registration) {
    // NOTE(java-quirk): Java binds `getExternalHost()` as it is. When no entry of the server's
    // address list covers 0.0.0.0 that is `null`, the NOT NULL `host` column refuses it, and
    // nothing is saved.
    let Some(host) = registration.external_host.as_deref() else {
        error!("Error while saving gameserver! (no external host: no address entry covers 0.0.0.0)");
        return;
    };
    // NOTE(java-quirk): `new BigInteger(new byte[0])` throws in Java, so a game server that sent an
    // empty hex id is never saved either.
    let hex_id = match hex_to_string(&registration.hex_id) {
        Ok(text) => text,
        Err(error) => {
            error!("Error while saving gameserver! {error}");
            return;
        }
    };
    if let Err(error) = store.save(registration.server_id, &hex_id, host).await {
        error!("Error while saving gameserver! {error}");
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use l2_db::string_to_hex;
    use l2_net::{CancellationToken, ConnectionConfig, FrameHandler, NoCipher, spawn_connection};
    use tokio::net::{TcpListener, TcpStream};

    use super::*;
    use crate::keys::GameServerKeys;
    use crate::server_names::ServerNames;

    /// A store that remembers what it was asked to save.
    #[derive(Default)]
    pub struct RecordingStore {
        pub saved: Mutex<Vec<(i32, String, String)>>,
        pub fail: bool,
    }

    impl GameServerStore for RecordingStore {
        async fn save(&self, server_id: i32, hex_id: &str, host: &str) -> Result<(), DbError> {
            if self.fail {
                return Err(DbError::InvalidHexId("test failure".to_owned()));
            }
            self.saved
                .lock()
                .unwrap()
                .push((server_id, hex_id.to_owned(), host.to_owned()));
            Ok(())
        }
    }

    struct Idle;
    impl FrameHandler for Idle {
        async fn on_frame(&mut self, _body: Vec<u8>, _ctl: &mut l2_net::Control<'_>) {}
    }

    /// A real (loopback) connection so a `GsLink` can be built. The returned stream is the far
    /// end and must stay alive for the test.
    pub async fn test_link(id: u64) -> (Arc<GsLink>, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (client, accepted) = tokio::join!(TcpStream::connect(address), listener.accept());
        let (server_side, _) = accepted.unwrap();
        let handle = spawn_connection(
            server_side,
            Arc::new(ConnectionConfig::default()),
            Box::new(NoCipher),
            Idle,
            CancellationToken::new(),
        )
        .unwrap();
        (Arc::new(GsLink::new(id, handle)), client.unwrap())
    }

    pub fn auth(desired_id: u8, hex_id: &[u8], accept_alternate: bool) -> GameServerAuth {
        GameServerAuth {
            desired_id,
            accept_alternate,
            reserve_host: false,
            port: 7777,
            max_players: 100,
            hex_id: hex_id.to_vec(),
            subnets: vec!["10.0.0.0/8".to_owned(), "0.0.0.0/0".to_owned()],
            hosts: vec!["192.168.0.5".to_owned(), "play.example.org".to_owned()],
        }
    }

    /// Names 1 and 2; server 1 is registered with hex id `12 34`.
    pub fn table() -> GameServerTable {
        let mut rng = <rand_chacha::ChaCha8Rng as rand::SeedableRng>::seed_from_u64(1);
        let keys = GameServerKeys::generate(&mut rng, 1).unwrap();
        let names = ServerNames::from([(1, "Bartz".to_owned()), (2, "Sieghardt".to_owned())]);
        let registered = BTreeMap::from([(1, GameServerInfo::new(1, vec![0x12, 0x34]))]);
        GameServerTable::new(names, registered, keys)
    }

    #[tokio::test]
    async fn known_id_with_the_same_hex_id_is_attached() {
        let table = table();
        let (link, _far) = test_link(1).await;
        let request = auth(1, &[0x12, 0x34], false);
        let registration = table.register_game_server(true, &request, &link).unwrap();
        assert_eq!(
            registration,
            Registration {
                server_id: 1,
                is_new: false,
                hex_id: vec![0x12, 0x34],
                external_host: Some("play.example.org".to_owned()),
            }
        );
        let row = table.registered(1).unwrap();
        assert!(row.authed);
        assert_eq!(row.port, 7777);
        assert_eq!(row.max_players, 100);
        assert_eq!(row.addresses.len(), 2);
        assert_eq!(row.link.as_ref().map(|l| l.id()), Some(1));
        // Clients inside 10.0.0.0/8 get the LAN host, everyone else the public one.
        assert_eq!(
            row.server_address("10.1.2.3".parse().unwrap()),
            Some("192.168.0.5")
        );
        assert_eq!(
            row.server_address("8.8.8.8".parse().unwrap()),
            Some("play.example.org")
        );
    }

    #[tokio::test]
    async fn a_second_session_for_an_authed_server_is_refused() {
        let table = table();
        let (first, _far1) = test_link(1).await;
        let (second, _far2) = test_link(2).await;
        let request = auth(1, &[0x12, 0x34], true);
        table.register_game_server(true, &request, &first).unwrap();
        assert_eq!(
            table.register_game_server(true, &request, &second),
            Err(LoginServerFail::REASON_ALREADY_LOGGED_IN)
        );
        // The refused link did not take over the row.
        assert_eq!(table.registered(1).unwrap().link.unwrap().id(), 1);
    }

    #[tokio::test]
    async fn unknown_id_is_registered_when_new_servers_are_accepted() {
        let table = table();
        let (link, _far) = test_link(1).await;
        let request = auth(5, &[0xAB, 0xCD], false);
        let registration = table.register_game_server(true, &request, &link).unwrap();
        assert_eq!(registration.server_id, 5);
        assert!(registration.is_new);
        assert_eq!(registration.hex_id, [0xAB, 0xCD]);
        assert!(table.has_registered(5));
        assert!(table.registered(5).unwrap().authed);
        // Id 5 is not in servername.xml; Java registers it anyway.
        assert_eq!(table.server_name(5), None);
    }

    #[tokio::test]
    async fn unknown_id_is_refused_when_new_servers_are_not_accepted() {
        let table = table();
        let (link, _far) = test_link(1).await;
        assert_eq!(
            table.register_game_server(false, &auth(5, &[1], true), &link),
            Err(LoginServerFail::REASON_WRONG_HEXID)
        );
        assert!(!table.has_registered(5));
    }

    #[tokio::test]
    async fn wrong_hex_id_without_an_alternative_is_refused() {
        let table = table();
        let (link, _far) = test_link(1).await;
        // The game server does not allow another id.
        assert_eq!(
            table.register_game_server(true, &auth(1, &[9, 9], false), &link),
            Err(LoginServerFail::REASON_WRONG_HEXID)
        );
        // The login server does not accept new servers.
        assert_eq!(
            table.register_game_server(false, &auth(1, &[9, 9], true), &link),
            Err(LoginServerFail::REASON_WRONG_HEXID)
        );
        assert!(!table.registered(1).unwrap().authed);
        assert_eq!(table.registered_count(), 1);
    }

    #[tokio::test]
    async fn wrong_hex_id_with_an_alternative_gets_the_first_free_id() {
        let table = table();
        let (link, _far) = test_link(1).await;
        let registration = table
            .register_game_server(true, &auth(1, &[9, 9], true), &link)
            .unwrap();
        assert_eq!(registration.server_id, 2, "id 1 is taken, 2 is the next name");
        assert!(registration.is_new);
        let row = table.registered(2).unwrap();
        assert_eq!(row.id, 2);
        assert_eq!(row.hex_id, [9, 9]);
        assert!(row.authed);
        assert!(!table.registered(1).unwrap().authed);
    }

    #[tokio::test]
    async fn no_free_alternative_id() {
        let table = table();
        let (first, _far1) = test_link(1).await;
        let (second, _far2) = test_link(2).await;
        table
            .register_game_server(true, &auth(1, &[9, 9], true), &first)
            .unwrap();
        // Ids 1 and 2 are both registered now.
        assert_eq!(
            table.register_game_server(true, &auth(1, &[7, 7], true), &second),
            Err(LoginServerFail::REASON_NO_FREE_ID)
        );
        assert_eq!(table.registered_count(), 2);
    }

    #[tokio::test]
    async fn no_free_alternative_id_without_server_names() {
        let mut rng = <rand_chacha::ChaCha8Rng as rand::SeedableRng>::seed_from_u64(1);
        let keys = GameServerKeys::generate(&mut rng, 1).unwrap();
        let registered = BTreeMap::from([(1, GameServerInfo::new(1, vec![1]))]);
        let table = GameServerTable::new(ServerNames::new(), registered, keys);
        let (link, _far) = test_link(1).await;
        assert_eq!(
            table.register_game_server(true, &auth(1, &[2], true), &link),
            Err(LoginServerFail::REASON_NO_FREE_ID)
        );
    }

    #[test]
    fn register_refuses_a_taken_id() {
        // The `REASON_ID_RESERVED` path: `register` is false when the id has a server.
        let table = table();
        assert!(!table.register(1, GameServerInfo::new(1, vec![5])));
        assert!(table.register(7, GameServerInfo::new(7, vec![5])));
        assert!(!table.register(7, GameServerInfo::new(7, vec![6])));
        assert_eq!(
            table.register_with_first_available_id(GameServerInfo::new(1, vec![8])),
            Some(2)
        );
        assert_eq!(
            table.register_with_first_available_id(GameServerInfo::new(1, vec![9])),
            None
        );
    }

    #[tokio::test]
    async fn hex_id_with_a_leading_zero_byte_does_not_survive_the_database() {
        // NOTE(java-quirk) in the code under test: the game server sends 00 12 34 56 ...
        let sent = [0x00, 0x12, 0x34, 0x56];
        let table = table();
        let (link, _far) = test_link(1).await;
        let registration = table
            .register_game_server(true, &auth(5, &sent, false), &link)
            .unwrap();
        // What is saved is the BigInteger text, which has no leading zeros.
        let saved = hex_to_string(&registration.hex_id).unwrap();
        assert_eq!(saved, "123456");
        // After a login server restart the row is read back as the minimal bytes ...
        let reloaded = string_to_hex(&saved).unwrap();
        assert_eq!(reloaded, [0x12, 0x34, 0x56]);
        assert_ne!(reloaded, sent);
        // ... so the same game server, sending the same bytes, is turned away.
        let restarted = GameServerTable::new(
            table.server_names().clone(),
            BTreeMap::from([(5, GameServerInfo::new(5, reloaded))]),
            GameServerKeys::generate(
                &mut <rand_chacha::ChaCha8Rng as rand::SeedableRng>::seed_from_u64(2),
                1,
            )
            .unwrap(),
        );
        let (again, _far2) = test_link(2).await;
        assert_eq!(
            restarted.register_game_server(true, &auth(5, &sent, false), &again),
            Err(LoginServerFail::REASON_WRONG_HEXID)
        );
        // A hex id that was minimal to begin with keeps working.
        let (minimal, _far3) = test_link(3).await;
        assert!(
            restarted
                .register_game_server(true, &auth(5, &[0x12, 0x34, 0x56], false), &minimal)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn going_down_keeps_the_row_but_not_the_link() {
        let table = table();
        let (link, _far) = test_link(7).await;
        table
            .register_game_server(true, &auth(1, &[0x12, 0x34], false), &link)
            .unwrap();
        // A cleanup for some other link changes nothing.
        assert!(!table.set_down(1, 99, ServerStatus::Normal));
        assert!(table.registered(1).unwrap().authed);

        assert!(table.set_down(1, 7, ServerStatus::Normal));
        let row = table.registered(1).unwrap();
        assert!(!row.authed);
        assert_eq!(row.port, 0);
        assert_eq!(row.status, ServerStatus::Down);
        assert!(row.link.is_none());
        assert_eq!(table.registered_count(), 1);
        // It can register again.
        let (next, _far2) = test_link(8).await;
        assert!(
            table
                .register_game_server(true, &auth(1, &[0x12, 0x34], false), &next)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn a_down_server_follows_the_login_server_status() {
        let mut row = GameServerInfo::new(1, vec![1]);
        row.status = ServerStatus::Good;
        row.set_down(ServerStatus::GmOnly);
        // NOTE(java-quirk) under test: GM-only login server wins over DOWN.
        assert_eq!(row.status, ServerStatus::GmOnly);
        row.set_down(ServerStatus::Normal);
        assert_eq!(row.status, ServerStatus::Down);
        row.set_status(ServerStatus::Good, ServerStatus::Down);
        assert_eq!(row.status, ServerStatus::Down);
        row.set_status(ServerStatus::Good, ServerStatus::Auto);
        assert_eq!(row.status, ServerStatus::Good);
    }

    #[tokio::test]
    async fn unusable_subnets_are_skipped() {
        let table = table();
        let (link, _far) = test_link(1).await;
        let mut request = auth(5, &[3], false);
        request.subnets = vec!["bogus".to_owned(), "0.0.0.0/0".to_owned()];
        request.hosts = vec!["a".to_owned(), "b".to_owned()];
        let registration = table.register_game_server(true, &request, &link).unwrap();
        assert_eq!(table.registered(5).unwrap().addresses.len(), 1);
        assert_eq!(registration.external_host.as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn new_servers_are_saved_with_the_java_hex_text() {
        let store = Arc::new(RecordingStore::default());
        // The sample row of `gameservers.sql`: a negative number.
        let hex_id = string_to_hex("-2ad66b3f483c22be097019f55c8abdf0").unwrap();
        save_new_server(
            &*store,
            &Registration {
                server_id: 2,
                is_new: true,
                hex_id: hex_id.clone(),
                external_host: Some("play.example.org".to_owned()),
            },
        )
        .await;
        assert_eq!(
            *store.saved.lock().unwrap(),
            [(
                2,
                "-2ad66b3f483c22be097019f55c8abdf0".to_owned(),
                "play.example.org".to_owned()
            )]
        );
        // And the text reads back to the same bytes.
        assert_eq!(
            hex_to_string(&hex_id).unwrap(),
            "-2ad66b3f483c22be097019f55c8abdf0"
        );
    }

    #[tokio::test]
    async fn saving_problems_are_logged_not_fatal() {
        let store = Arc::new(RecordingStore::default());
        for registration in [
            // No address entry covers 0.0.0.0: Java binds null and the insert fails.
            Registration {
                server_id: 3,
                is_new: true,
                hex_id: vec![1],
                external_host: None,
            },
            // An empty hex id: `new BigInteger(byte[0])` throws.
            Registration {
                server_id: 4,
                is_new: true,
                hex_id: vec![],
                external_host: Some("h".to_owned()),
            },
        ] {
            save_new_server(&*store, &registration).await;
        }
        assert!(store.saved.lock().unwrap().is_empty());

        let failing = Arc::new(RecordingStore {
            fail: true,
            ..RecordingStore::default()
        });
        save_new_server(
            &*failing,
            &Registration {
                server_id: 3,
                is_new: true,
                hex_id: vec![1],
                external_host: Some("h".to_owned()),
            },
        )
        .await;
    }
}
