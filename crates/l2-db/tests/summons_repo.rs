#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Integration test of `SummonRepository` against a real MySQL/MariaDB server.
//!
//! It runs only when `L2_TEST_DATABASE_URL` is set (CI sets it for a throwaway MariaDB service
//! on Linux), e.g. `mysql://root:pw@127.0.0.1:3306/`. Never point it at a server with real
//! data: the test only creates and drops a database named `l2_test_*`, but it needs the rights
//! to do so. The database is built from `sql/game/character_summons.sql` and
//! `sql/game/character_summon_skills_save.sql`.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use l2_db::{NewSummonSkillSave, Pool, SummonRepository, SummonSkillSaveRow, connect_with};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

const SCHEMA: [&str; 2] = [
    include_str!("../sql/game/character_summons.sql"),
    include_str!("../sql/game/character_summon_skills_save.sql"),
];

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// The statements of a schema file; whole-line `#` comments are dropped (a file may end with
/// one, which would otherwise be sent as an empty statement).
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

/// A throwaway database with the servitor tables.
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
            "l2_test_summon_{}_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            nanos % 1_000_000_000
        );
        assert!(name.starts_with("l2_test_"));
        // The name is generated above from the pid, a counter and the clock.
        sqlx::query(AssertSqlSafe(format!(
            "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        )))
        .execute(&mut admin)
        .await
        .unwrap();

        let pool = connect_with(admin_options.database(&name), 10).await.unwrap();
        for file in SCHEMA {
            for statement in statements(file) {
                sqlx::query(AssertSqlSafe(statement))
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }
        Some(Self { admin, name, pool })
    }

    async fn drop_database(mut self) {
        self.pool.close().await;
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{}`", self.name)))
            .execute(&mut self.admin)
            .await
            .unwrap();
    }
}

const OWNER: i32 = 268_600_001;
const KAT: i32 = 1111;
const MEW: i32 = 1225;

/// `CharSummonTable.saveSummon` (a `REPLACE`) and `removeServitor` (every row of the owner).
#[tokio::test]
async fn servitor_rows_are_replaced_and_removed_like_java() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SummonRepository::new(db.pool.clone());
    repo.save_summon(OWNER, KAT, 300, 200, 1_000_000).await.unwrap();
    repo.save_summon(OWNER, KAT, 250, 100, 900_000).await.unwrap();
    repo.save_summon(OWNER, MEW, 1, 1, 5).await.unwrap();
    let rows: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT CAST(summonSkillId AS SIGNED), CAST(curHp AS SIGNED), CAST(curMp AS SIGNED), \n         CAST(time AS SIGNED) FROM character_summons WHERE ownerId = ? ORDER BY summonSkillId",
    )
    .bind(OWNER)
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        [(i64::from(KAT), 250, 100, 900_000), (i64::from(MEW), 1, 1, 5)]
    );
    repo.remove_summon(OWNER).await.unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM character_summons")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
    db.drop_database().await;
}

/// `Servitor.storeEffect`: the rows of one reference skill are replaced, `buff_index` counts from
/// 1, and the loader reads every reference skill of the class index in `buff_index` order.
#[tokio::test]
async fn servitor_buffs_are_stored_per_reference_skill() {
    let Some(db) = TestDb::create().await else {
        return;
    };
    let repo = SummonRepository::new(db.pool.clone());
    let save = |skill_id, remaining_time| NewSummonSkillSave {
        skill_id,
        skill_level: 1,
        remaining_time,
    };
    repo.store_servitor_effects(OWNER, 0, KAT, &[save(1040, 100), save(1068, 50)])
        .await
        .unwrap();
    repo.store_servitor_effects(OWNER, 0, MEW, &[save(1204, 10)])
        .await
        .unwrap();
    repo.store_servitor_effects(OWNER, 1, KAT, &[save(1035, 7)])
        .await
        .unwrap();
    // A new store of Kat replaces its rows only.
    repo.store_servitor_effects(OWNER, 0, KAT, &[save(1068, 40)])
        .await
        .unwrap();
    let row = |summon_skill_id, skill_id, remaining_time| SummonSkillSaveRow {
        summon_skill_id,
        skill_id,
        skill_level: 1,
        remaining_time,
    };
    let mut rows = repo.servitor_effects_of(OWNER, 0).await.unwrap();
    rows.sort_by_key(|r| r.summon_skill_id);
    assert_eq!(rows, [row(KAT, 1068, 40), row(MEW, 1204, 10)]);
    assert_eq!(
        repo.servitor_effects_of(OWNER, 1).await.unwrap(),
        [row(KAT, 1035, 7)]
    );
    // The delete alone (`restoreEffects`).
    repo.store_servitor_effects(OWNER, 0, KAT, &[]).await.unwrap();
    assert_eq!(
        repo.servitor_effects_of(OWNER, 0).await.unwrap(),
        [row(MEW, 1204, 10)]
    );
    db.drop_database().await;
}
