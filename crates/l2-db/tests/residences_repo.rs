#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `ResidenceRepository` against a real MySQL/MariaDB server: Java's residence
//! statements (`Castle.load`, `updateOwnerInDB`, `addToTreasuryNoTax`, the function tables,
//! `Fort.load`, `ClanHallTable.load`, `CHSiegeManager`, ...) on the unchanged schema, with the
//! base rows of Java's `castle.sql`, `fort.sql`, `clanhall.sql` and `siegable_clanhall.sql`.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{FunctionSave, FunctionTable, Pool, ResidenceRepository, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// The statements of a schema file; whole-line `#` comments are dropped.
fn statements(file: &str) -> Vec<String> {
    let text: String = file
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    text.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

struct TestDb {
    admin: MySqlConnection,
    name: String,
    pool: Pool,
}

impl TestDb {
    /// `None` (with a printed note) when `L2_TEST_DATABASE_URL` is not set.
    async fn create() -> Option<Self> {
        let url = match std::env::var("L2_TEST_DATABASE_URL") {
            Ok(url) if !url.trim().is_empty() => url,
            _ => {
                eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
                return None;
            }
        };
        let admin_options = MySqlConnectOptions::from_str(&url).unwrap();
        let mut admin = MySqlConnection::connect_with(&admin_options).await.unwrap();
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let name = format!(
            "l2_test_residences_{}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            nanos % 1_000_000_000
        );
        assert!(name.starts_with("l2_test_"));
        sqlx::query(AssertSqlSafe(format!(
            "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        )))
        .execute(&mut admin)
        .await
        .unwrap();
        let pool = connect_with(admin_options.database(&name), 10).await.unwrap();
        for file in l2_db::GAME_SCHEMA {
            for statement in statements(file) {
                sqlx::query(AssertSqlSafe(statement))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        Some(Self { admin, name, pool })
    }

    async fn exec(&self, sql: &str) {
        sqlx::query(AssertSqlSafe(sql.to_owned()))
            .execute(&self.pool)
            .await
            .unwrap();
    }

    async fn scalar(&self, sql: &str) -> i64 {
        sqlx::query_scalar(AssertSqlSafe(format!("SELECT CAST(({sql}) AS SIGNED)")))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn text(&self, sql: &str) -> String {
        sqlx::query_scalar(AssertSqlSafe(sql.to_owned()))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn castle_rows_owner_treasury_and_functions_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    // The base rows of Java's castle.sql (two of them) and two clans.
    db.exec("INSERT INTO castle VALUES (1,'Gludio',0,0,0,'true',0,'false',0),(8,'Rune',10,500,1700000000000,'false',1700000003600,'true',3)").await;
    db.exec(
        "INSERT INTO clan_data (clan_id, clan_name, hasCastle) VALUES (900, 'Lions', 8), (901, 'Wolves', 0)",
    )
    .await;
    let repo = ResidenceRepository::new(db.pool.clone());

    assert_eq!(repo.castle_ids().await.unwrap(), [1, 8]);
    let rune = repo.castle(8).await.unwrap().unwrap();
    assert_eq!(rune.name, "Rune");
    assert_eq!((rune.tax_percent, rune.treasury), (10, 500));
    assert_eq!(
        (rune.siege_date, rune.reg_time_end),
        (1_700_000_000_000, 1_700_000_003_600)
    );
    assert_eq!(
        (rune.reg_time_over.as_str(), rune.show_npc_crest.as_str()),
        ("false", "true")
    );
    assert_eq!(rune.ticket_buy_count, 3);
    assert!(repo.castle(2).await.unwrap().is_none());
    assert_eq!(repo.castle_owner_ids(8).await.unwrap(), [900]);
    assert!(repo.castle_owner_ids(1).await.unwrap().is_empty());

    // D-50: the proofs of alliance of a clan, as the table has them.
    db.exec("UPDATE clan_data SET blood_alliance_count = 7 WHERE clan_id = 900")
        .await;
    assert_eq!(repo.blood_alliance_count(900).await.unwrap(), Some(7));
    assert_eq!(repo.blood_alliance_count(901).await.unwrap(), Some(0));
    assert_eq!(repo.blood_alliance_count(777).await.unwrap(), None);

    // Castle.updateOwnerInDB: Rune goes to the Wolves.
    repo.update_castle_owner(8, 901).await.unwrap();
    assert_eq!(repo.castle_owner_ids(8).await.unwrap(), [901]);
    assert_eq!(
        db.scalar("SELECT hasCastle FROM clan_data WHERE clan_id = 900")
            .await,
        0
    );
    // ... and is lost by everybody (the second statement finds no clan 0).
    repo.update_castle_owner(8, 0).await.unwrap();
    assert!(repo.castle_owner_ids(8).await.unwrap().is_empty());

    repo.update_castle_treasury(8, 12_345_678_901).await.unwrap();
    repo.update_castle_tax_percent(8, 15).await.unwrap();
    repo.update_castle_show_npc_crest(8, false).await.unwrap();
    repo.update_castle_ticket_buy_count(8, 7).await.unwrap();
    let rune = repo.castle(8).await.unwrap().unwrap();
    assert_eq!(
        (rune.treasury, rune.tax_percent, rune.ticket_buy_count),
        (12_345_678_901, 15, 7)
    );
    assert_eq!(rune.show_npc_crest, "false");
    assert_eq!(
        db.text("SELECT showNpcCrest FROM castle WHERE id = 8").await,
        "false"
    );

    // CastleFunction.dbSave (REPLACE) and Castle.removeFunction.
    let save = FunctionSave {
        table: FunctionTable::Castle,
        residence_id: 8,
        function_type: 2,
        lvl: 40,
        lease: 2500,
        rate: 86_400_000,
        end_time: 1_700_000_000_000,
    };
    repo.save_function(&save).await.unwrap();
    repo.save_function(&FunctionSave { lvl: 50, ..save })
        .await
        .unwrap();
    let functions = repo.functions(FunctionTable::Castle, 8).await.unwrap();
    assert_eq!(functions.len(), 1);
    assert_eq!(
        (functions[0].function_type, functions[0].lvl, functions[0].lease),
        (2, 50, 2500)
    );
    assert_eq!(
        (functions[0].rate, functions[0].end_time),
        (86_400_000, 1_700_000_000_000)
    );
    assert!(repo.functions(FunctionTable::Fort, 8).await.unwrap().is_empty());
    repo.delete_function(FunctionTable::Castle, 8, 2).await.unwrap();
    assert!(repo.functions(FunctionTable::Castle, 8).await.unwrap().is_empty());

    // CastleManager.removeCirclet of an offline member: only that item of that owner goes.
    db.exec("INSERT INTO items (owner_id, object_id, item_id, count, loc) VALUES (1001, 50, 8182, 1, 'INVENTORY'), (1001, 51, 57, 1, 'INVENTORY'), (1002, 52, 8182, 1, 'INVENTORY')").await;
    repo.delete_circlets(1001, 8182).await.unwrap();
    assert_eq!(db.scalar("SELECT COUNT(*) FROM items").await, 2);
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM items WHERE item_id = 8182").await,
        1
    );
    db.drop_database().await;
}

#[tokio::test]
async fn fort_and_hall_rows_with_javas_statements() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    db.exec(
        "INSERT INTO fort VALUES (101,'Shanty',0,0,0,0,0,0,0),(102,'Southern',0,1700000000000,900,1,2,1,3)",
    )
    .await;
    db.exec("INSERT INTO clanhall VALUES ('22','Moonstone Hall','900','500000','Clan hall located in the Town of Gludio','Gludio','1700000000000','2','1'),('23','Onyx Hall','0','500000','x','Gludio','0','2','0')").await;
    db.exec("INSERT INTO siegable_clanhall (`clanHallId`, `name`, `ownerId`, `desc`, `location`, `nextSiege`, `siegeLength`, `schedule_config`) VALUES (21, 'Fortress of Resistance', 900, 'Contestable Clan Hall', 'Dion', 0, 3600000, '14;0;0;12;00'), (34, NULL, NULL, NULL, NULL, NULL, NULL, NULL)").await;
    let repo = ResidenceRepository::new(db.pool.clone());

    assert_eq!(repo.fort_ids().await.unwrap(), [101, 102]);
    let southern = repo.fort(102).await.unwrap().unwrap();
    assert_eq!(southern.name, "Southern");
    assert_eq!(
        (southern.owner, southern.last_owned_time),
        (900, 1_700_000_000_000)
    );
    assert_eq!(
        (
            southern.fort_type,
            southern.state,
            southern.castle_id,
            southern.supply_level
        ),
        (1, 2, 1, 3)
    );
    assert_eq!(
        repo.forts_of_owner(900).await.unwrap(),
        std::slice::from_ref(&southern)
    );
    assert!(repo.forts_of_owner(901).await.unwrap().is_empty());

    // Fort.updateOwnerInDB binds state and castleId 0; setFortState and saveFortVariables.
    repo.update_fort_owner(101, 901, 1_700_000_100_000).await.unwrap();
    repo.update_fort_state(102, 1, 0).await.unwrap();
    repo.update_fort_supply_level(102, 0).await.unwrap();
    let shanty = repo.fort(101).await.unwrap().unwrap();
    assert_eq!(
        (shanty.owner, shanty.last_owned_time, shanty.state),
        (901, 1_700_000_100_000, 0)
    );
    let southern = repo.fort(102).await.unwrap().unwrap();
    assert_eq!(
        (southern.state, southern.castle_id, southern.supply_level),
        (1, 0, 0)
    );

    let halls = repo.clan_halls().await.unwrap();
    assert_eq!(halls.len(), 2);
    assert_eq!(halls[0].id, 22);
    assert_eq!(
        (halls[0].owner_id, halls[0].lease, halls[0].grade),
        (900, 500_000, 2)
    );
    assert_eq!((halls[0].paid_until, halls[0].paid), (1_700_000_000_000, true));
    assert_eq!(halls[0].location, "Gludio");
    assert!(!halls[1].paid);
    assert_eq!(
        repo.clan_halls_of_owner(900).await.unwrap(),
        std::slice::from_ref(&halls[0])
    );

    // AuctionableHall.updateDb.
    repo.update_clan_hall(22, 0, 0, false).await.unwrap();
    assert!(repo.clan_halls_of_owner(900).await.unwrap().is_empty());

    let siegable = repo.siegable_halls().await.unwrap();
    assert_eq!(siegable.len(), 2);
    let resistance = siegable.iter().find(|h| h.clan_hall_id == 21).unwrap();
    assert_eq!(resistance.owner_id, Some(900));
    assert_eq!(resistance.schedule_config.as_deref(), Some("14;0;0;12;00"));
    assert_eq!(resistance.siege_length, Some(3_600_000));
    let empty = siegable.iter().find(|h| h.clan_hall_id == 34).unwrap();
    assert_eq!((empty.owner_id, empty.name.as_deref()), (None, None));
    assert_eq!(repo.siegable_halls_of_owner(900).await.unwrap().len(), 1);
    repo.update_siegable_hall(21, 0, 1_700_000_200_000).await.unwrap();
    assert!(repo.siegable_halls_of_owner(900).await.unwrap().is_empty());
    assert_eq!(
        db.scalar("SELECT nextSiege FROM siegable_clanhall WHERE clanHallId = 21")
            .await,
        1_700_000_200_000
    );

    // The fort and hall function tables.
    for (table, id) in [(FunctionTable::Fort, 102), (FunctionTable::ClanHall, 22)] {
        let save = FunctionSave {
            table,
            residence_id: id,
            function_type: 1,
            lvl: 2,
            lease: 1000,
            rate: 604_800_000,
            end_time: 5,
        };
        repo.save_function(&save).await.unwrap();
        let rows = repo.functions(table, id).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].lvl, rows[0].rate, rows[0].end_time), (2, 604_800_000, 5));
        repo.delete_function(table, id, 1).await.unwrap();
        assert!(repo.functions(table, id).await.unwrap().is_empty());
    }
    db.drop_database().await;
}
