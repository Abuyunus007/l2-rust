#![allow(clippy::unwrap_used, clippy::expect_used)]
//! A fake game server registers with a running login server and the `gameservers` table.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service,
//! e.g. `mysql://root@127.0.0.1:3306/`). Never point it at a server with real data: the test
//! only creates and drops a database named `l2_test_gs_*`, but it needs the rights to do so.
//! The handshake itself is covered without a database in `src/gs/mod.rs`.

use std::path::Path;

use l2_db::string_to_hex;
use l2_testkit::fake_game_server::{FakeGameServer, auth_request};
use l2_testkit::test_db::{DEFAULT_SERVER_INI, SAMPLE_HEXID, TestDb};
use login_server::state::ExitReason;
use login_server::{Paths, Server};

#[tokio::test]
async fn game_servers_register_against_the_database() {
    let Some(db) = TestDb::create("gs").await else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    db.write_server_folder(root.path(), DEFAULT_SERVER_INI);
    let paths = Paths::resolve(root.path(), Path::new("./config"), Path::new("./data"));

    let server = Server::start(&paths).await.unwrap();
    let state = std::sync::Arc::clone(server.state());
    let address = state.game_servers.listen_addr().unwrap();

    // The sample server, with the hex id as Java reads it from the table.
    let sample = string_to_hex(SAMPLE_HEXID).unwrap();
    let (response, _first) = FakeGameServer::register(address, &auth_request(2, sample, false)).await;
    assert_eq!(
        (response.server_id, response.server_name.as_str()),
        (2, "Sieghardt")
    );
    assert!(state.game_servers.registered(2).unwrap().authed);
    assert_eq!(
        state.repository.game_servers().await.unwrap().len(),
        1,
        "nothing new saved"
    );

    // A new server is saved with the BigInteger text of its hex id and its external host.
    let mut request = auth_request(3, vec![0x2A, 0xD6, 0x6B], false);
    request.hosts = vec!["gs.example.org".to_owned()];
    let (response, _second) = FakeGameServer::register(address, &request).await;
    assert_eq!((response.server_id, response.server_name.as_str()), (3, "Kain"));
    let rows = state.repository.game_servers().await.unwrap();
    let row = rows.iter().find(|row| row.server_id == 3).unwrap();
    assert_eq!(row.hexid, "2ad66b");
    assert_eq!(row.host, "gs.example.org");

    // A wrong hex id with an alternative gets the first free id (1) and is saved too.
    let (response, _third) = FakeGameServer::register(address, &auth_request(2, vec![9, 9, 9], true)).await;
    assert_eq!((response.server_id, response.server_name.as_str()), (1, "Bartz"));
    assert_eq!(state.repository.game_servers().await.unwrap().len(), 3);

    let result = server.shutdown(false).await;
    db.drop_db().await;
    assert_eq!(result, ExitReason::Shutdown);
}
