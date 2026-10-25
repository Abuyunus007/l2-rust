#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `HallSiegeRepository` against a real MySQL/MariaDB server: the statements of
//! the conquerable hall sieges (`ClanHallSiegeEngine.loadAttackers`, `saveAttackers`, `loadGuards`,
//! the flag war tables of Bandit Stronghold and Wild Beast Reserve, the attacker list of Rainbow
//! Springs Chateau) on the unchanged schema.
//!
//! Runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service on
//! Linux). Never point it at a server with real data: the test only creates and drops a database
//! named `l2_test_*`, but it needs the rights to do so.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{HallSiegeRepository, Pool, connect_with};
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
            "l2_test_hall_sieges_{}_{}_{}",
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

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn base_engine_attackers_and_guards() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = HallSiegeRepository::new(db.pool.clone());
    assert!(repo.attackers(21).await.unwrap().is_empty());

    // saveAttackers: DELETE the hall's rows, then one INSERT per clan.
    repo.save_attackers(21, &[900, 901]).await.unwrap();
    repo.save_attackers(34, &[902]).await.unwrap();
    let mut clans = repo.attackers(21).await.unwrap();
    clans.sort_unstable();
    assert_eq!(clans, [900, 901]);
    repo.save_attackers(21, &[903]).await.unwrap();
    assert_eq!(repo.attackers(21).await.unwrap(), [903]);
    assert_eq!(repo.attackers(34).await.unwrap(), [902]);
    repo.save_attackers(21, &[]).await.unwrap();
    assert!(repo.attackers(21).await.unwrap().is_empty());
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM clanhall_siege_attackers").await,
        1
    );

    // loadGuards: the rows of the hall (Java ignores isSiegeBoss).
    db.exec(
        "INSERT INTO clanhall_siege_guards VALUES          (34,35408,178306,-17535,-2200,32768,7200,'false'),         (34,35409,178304,-17712,-2200,32768,0,'true'),         (64,35629,1,2,3,4,5,'false')",
    )
    .await;
    let guards = repo.guards(34).await.unwrap();
    assert_eq!(guards.len(), 2);
    assert_eq!(
        (
            guards[0].npc_id,
            guards[0].x,
            guards[0].y,
            guards[0].z,
            guards[0].heading,
            guards[0].respawn_delay
        ),
        (35408, 178_306, -17_535, -2200, 32_768, 7200)
    );
    assert_eq!(guards[1].respawn_delay, 0);
    assert_eq!(repo.guards(64).await.unwrap().len(), 1);
    assert!(repo.guards(21).await.unwrap().is_empty());
    db.drop_database().await;
}

#[tokio::test]
async fn flag_war_tables() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = HallSiegeRepository::new(db.pool.clone());

    // saveClan: INSERT INTO siegable_hall_flagwar_attackers VALUES(hall, flag, 0, clan).
    repo.insert_flag_war_clan(35, 35_422, 0, 900).await.unwrap();
    repo.insert_flag_war_clan(35, 35_423, 0, 901).await.unwrap();
    repo.insert_flag_war_clan(63, 35_606, 0, 902).await.unwrap();
    // The flag is the primary key: two clans cannot hold the same flag.
    assert!(repo.insert_flag_war_clan(35, 35_422, 0, 903).await.is_err());
    // saveNpc: UPDATE ... SET npc = ? WHERE clan_id = ?
    repo.update_flag_war_npc(35_428, 900).await.unwrap();
    // saveMember
    repo.insert_flag_war_member(35, 900, 0x6000_0001).await.unwrap();
    repo.insert_flag_war_member(35, 900, 0x6000_0002).await.unwrap();
    repo.insert_flag_war_member(35, 901, 0x6000_0003).await.unwrap();
    repo.insert_flag_war_member(63, 902, 0x6000_0004).await.unwrap();

    let mut clans = repo.flag_war_clans(35).await.unwrap();
    clans.sort_by_key(|c| c.clan_id);
    assert_eq!(clans.len(), 2);
    assert_eq!(
        (clans[0].clan_id, clans[0].flag, clans[0].npc),
        (900, 35_422, 35_428)
    );
    assert_eq!((clans[1].clan_id, clans[1].flag, clans[1].npc), (901, 35_423, 0));
    let mut members = repo.flag_war_members(900).await.unwrap();
    members.sort_unstable();
    assert_eq!(members, [0x6000_0001, 0x6000_0002]);
    assert_eq!(repo.flag_war_members(901).await.unwrap(), [0x6000_0003]);

    // clearTables: both tables, the hall's rows only.
    repo.clear_flag_war(35).await.unwrap();
    assert!(repo.flag_war_clans(35).await.unwrap().is_empty());
    assert!(repo.flag_war_members(900).await.unwrap().is_empty());
    assert_eq!(repo.flag_war_clans(63).await.unwrap().len(), 1);
    assert_eq!(repo.flag_war_members(902).await.unwrap(), [0x6000_0004]);
    db.drop_database().await;
}

#[tokio::test]
async fn rainbow_springs_attacker_list() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = HallSiegeRepository::new(db.pool.clone());
    assert!(repo.rainbow_attackers().await.unwrap().is_empty());

    // updateAttacker: INSERT INTO rainbowsprings_attacker_list VALUES (?,?)
    repo.insert_rainbow_attacker(900, 100).await.unwrap();
    repo.insert_rainbow_attacker(901, 2_500).await.unwrap();
    let mut rows = repo.rainbow_attackers().await.unwrap();
    rows.sort_by_key(|r| r.clan_id);
    assert_eq!(
        rows.iter()
            .map(|r| (r.clan_id, r.war_decrees_count))
            .collect::<Vec<_>>(),
        [(900, 100), (901, 2_500)]
    );
    // A NULL column reads as 0.
    db.exec("INSERT INTO rainbowsprings_attacker_list VALUES (NULL, NULL)")
        .await;
    assert!(
        repo.rainbow_attackers()
            .await
            .unwrap()
            .iter()
            .any(|r| r.clan_id == 0 && r.war_decrees_count == 0)
    );

    // updateAttacker with remove, then clearTables.
    repo.delete_rainbow_attacker(900).await.unwrap();
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM rainbowsprings_attacker_list WHERE clanId = 900")
            .await,
        0
    );
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM rainbowsprings_attacker_list")
            .await,
        2
    );
    repo.clear_rainbow_attackers().await.unwrap();
    assert_eq!(
        db.scalar("SELECT COUNT(*) FROM rainbowsprings_attacker_list")
            .await,
        0
    );
    db.drop_database().await;
}
